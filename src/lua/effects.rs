use crate::animation::BackgroundEffect;
use crate::config::{BackgroundConfig, EffectKind};
use crate::lua::canvas::{
    CanvasBuffer, LuaCanvasHandle, attach_math_helpers, build_theme_lua_table,
};
use crate::lua::configure_lua_vm;
use crate::ui::container::InlineLuaScript;
use crate::ui::theme::Theme;
use mlua::{Function, Lua, RegistryKey, Table, Value};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct LuaEffectSpec {
    pub name: String,
    pub fps: u32,
    pub density: f32,
    pub speed: f32,
    pub color: Option<String>,
    pub head_color: Option<String>,
    pub charset: Option<String>,
    pub spacing: Option<u16>,
    pub char_glyph: Option<String>,
    pub count: Option<usize>,
}

impl LuaEffectSpec {
    pub fn from_lua_table(tbl: &Table) -> Option<Self> {
        let name = tbl
            .get::<String>("name")
            .unwrap_or_else(|_| "none".to_string());
        let fps = tbl.get::<u32>("fps").unwrap_or(30).clamp(1, 60);
        let opts = tbl.get::<Table>("options").ok();
        let (density, speed, color, head_color, charset, spacing, char_glyph, count) =
            if let Some(o) = opts {
                (
                    o.get::<f32>("density").unwrap_or(0.6),
                    o.get::<f32>("speed").unwrap_or(1.0),
                    o.get::<Option<String>>("color").unwrap_or(None),
                    o.get::<Option<String>>("head_color").unwrap_or(None),
                    o.get::<Option<String>>("charset").unwrap_or(None),
                    o.get::<Option<u16>>("spacing").unwrap_or(None),
                    o.get::<Option<String>>("char").unwrap_or(None),
                    o.get::<Option<usize>>("count").unwrap_or(None),
                )
            } else {
                (0.6, 1.0, None, None, None, None, None, None)
            };

        Some(Self {
            name,
            fps,
            density,
            speed,
            color,
            head_color,
            charset,
            spacing,
            char_glyph,
            count,
        })
    }
}

struct LuaScriptInner {
    lua: Lua,
    module_key: RegistryKey,
    state_key: RegistryKey,
    init_called: bool,
    width: u16,
    height: u16,
    fps: u32,
    accum_dt: f32,
    elapsed: f32,
    last_error: Option<String>,
}

/// Isolated stateful Lua shader/canvas host bound to a single container or background unit.
/// Each instance owns a private Lua VM and an exclusive `CanvasBuffer`.
pub struct LuaCanvasInstance {
    pub owner_name: String,
    pub owner_description: String,
    pub script_name: String,
    inner: Option<LuaScriptInner>,
    canvas_buf: Arc<Mutex<CanvasBuffer>>,
    theme_snapshot: Theme,
}

impl LuaCanvasInstance {
    pub fn from_spec(spec: &LuaEffectSpec, width: u16, height: u16, theme: Theme) -> Self {
        Self::from_spec_named(
            "background",
            "Independent background surface",
            spec,
            width,
            height,
            theme,
        )
    }

    pub fn from_spec_named(
        owner_name: impl Into<String>,
        owner_description: impl Into<String>,
        spec: &LuaEffectSpec,
        width: u16,
        height: u16,
        theme: Theme,
    ) -> Self {
        let name = owner_name.into();
        let desc = owner_description.into();
        let mut inst = Self {
            owner_name: name.clone(),
            owner_description: desc.clone(),
            script_name: spec.name.clone(),
            inner: None,
            canvas_buf: Arc::new(Mutex::new(CanvasBuffer::new_named(name, desc, width, height))),
            theme_snapshot: theme,
        };
        inst.initialize_with_spec(spec, width, height);
        inst
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_script_name(
        script_name: &str,
        width: u16,
        height: u16,
        fps: u32,
        speed: f32,
        density: f32,
        theme: Theme,
    ) -> Self {
        Self::from_script_name_named(
            script_name,
            "",
            script_name,
            width,
            height,
            fps,
            speed,
            density,
            theme,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_script_name_named(
        owner_name: impl Into<String>,
        owner_description: impl Into<String>,
        script_name: &str,
        width: u16,
        height: u16,
        fps: u32,
        speed: f32,
        density: f32,
        theme: Theme,
    ) -> Self {
        let spec = LuaEffectSpec {
            name: script_name.to_string(),
            fps: fps.clamp(1, 60),
            density,
            speed,
            color: None,
            head_color: None,
            charset: None,
            spacing: None,
            char_glyph: None,
            count: None,
        };
        Self::from_spec_named(owner_name, owner_description, &spec, width, height, theme)
    }

    pub fn from_inline_named(
        owner_name: impl Into<String>,
        owner_description: impl Into<String>,
        inline: &InlineLuaScript,
        width: u16,
        height: u16,
        fps: u32,
        theme: Theme,
    ) -> Self {
        let name = owner_name.into();
        let desc = owner_description.into();
        let mut inst = Self {
            owner_name: name.clone(),
            owner_description: desc.clone(),
            script_name: format!("inline:{name}"),
            inner: None,
            canvas_buf: Arc::new(Mutex::new(CanvasBuffer::new_named(name, desc, width, height))),
            theme_snapshot: theme,
        };
        inst.initialize_with_inline(inline, width, height, fps.clamp(1, 60));
        inst
    }

    pub fn set_theme(&mut self, theme: Theme) {
        self.theme_snapshot = theme;
    }

    pub fn last_error(&self) -> Option<&str> {
        self.inner.as_ref().and_then(|i| i.last_error.as_deref())
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        self.canvas_buf.lock().unwrap().resize(width, height);
        let Some(inner) = self.inner.as_mut() else {
            return;
        };
        if inner.width == width && inner.height == height {
            return;
        }
        inner.width = width;
        inner.height = height;
        inner.accum_dt = 1.0;

        if let (Ok(fx), Ok(st), Ok(ctx)) = (
            inner.lua.registry_value::<Table>(&inner.module_key),
            inner.lua.registry_value::<Table>(&inner.state_key),
            build_script_ctx(
                &self.owner_name,
                &self.owner_description,
                &self.theme_snapshot,
                &inner.lua,
                width,
                height,
                0.0,
                inner.elapsed,
                inner.fps,
                None,
            ),
        ) {
            let _ = ctx.set("state", st);
            if let Ok(Value::Function(resize_fn)) = fx.get::<Value>("resize")
                && let Err(e) = resize_fn.call::<()>(ctx)
            {
                inner.last_error = Some(e.to_string());
            }
        }
    }

    pub fn update(&mut self, dt_secs: f32, speed_mult: f32) {
        let Some(inner) = self.inner.as_mut() else {
            return;
        };
        inner.accum_dt += dt_secs;
        let min_step = 1.0 / (inner.fps.max(1) as f32);
        if inner.accum_dt < min_step {
            return;
        }
        let step_dt = inner.accum_dt * speed_mult.max(0.05);
        inner.accum_dt = 0.0;
        inner.elapsed += step_dt;

        let Ok(fx) = inner.lua.registry_value::<Table>(&inner.module_key) else {
            return;
        };
        let Ok(st) = inner.lua.registry_value::<Table>(&inner.state_key) else {
            return;
        };
        let Ok(ctx) = build_script_ctx(
            &self.owner_name,
            &self.owner_description,
            &self.theme_snapshot,
            &inner.lua,
            inner.width,
            inner.height,
            step_dt,
            inner.elapsed,
            inner.fps,
            Some(st),
        ) else {
            return;
        };

        if !inner.init_called {
            inner.init_called = true;
            if let Ok(Value::Function(init_fn)) = fx.get::<Value>("init")
                && let Err(e) = init_fn.call::<()>(ctx.clone())
            {
                inner.last_error = Some(e.to_string());
                return;
            }
        }

        if let Ok(Value::Function(update_fn)) = fx.get::<Value>("update")
            && let Err(e) = update_fn.call::<()>((step_dt, inner.elapsed, ctx.clone()))
        {
            inner.last_error = Some(e.to_string());
            return;
        }

        {
            let mut guard = self.canvas_buf.lock().unwrap();
            guard.resize(inner.width, inner.height);
            guard.begin_frame();
        }
        let canvas_handle =
            LuaCanvasHandle::new(Arc::clone(&self.canvas_buf), self.theme_snapshot.clone());

        if let Ok(Value::Function(render_fn)) = fx.get::<Value>("render")
            && let Err(e) = render_fn.call::<()>((canvas_handle, ctx))
        {
            inner.last_error = Some(e.to_string());
        }
    }

    pub fn render(&self, buf: &mut Buffer, area: Rect, theme: &Theme) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let guard = self.canvas_buf.lock().unwrap();
        guard.blit_to_buffer(buf, area, theme.text);
    }

    pub fn clear_buffer(&mut self) {
        self.canvas_buf.lock().unwrap().begin_frame();
    }

    fn initialize_with_inline(
        &mut self,
        inline: &InlineLuaScript,
        width: u16,
        height: u16,
        fps: u32,
    ) {
        let lua = Lua::new();
        if configure_lua_vm(&lua, None).is_err() {
            return;
        }
        let Ok(fx_tbl) = lua.create_table() else {
            return;
        };
        let load_bc = |key: &str, bytes: &Option<Vec<u8>>| -> Result<(), String> {
            if let Some(bc) = bytes {
                let func = lua
                    .load(bc.as_slice())
                    .into_function()
                    .map_err(|e| format!("Failed to load inline '{key}': {e}"))?;
                fx_tbl
                    .set(key, func)
                    .map_err(|e| format!("Failed to bind inline '{key}': {e}"))?;
            }
            Ok(())
        };

        let mut init_err = None;
        for (k, b) in [
            ("init", &inline.init_chunk),
            ("update", &inline.update_chunk),
            ("render", &inline.render_chunk),
            ("resize", &inline.resize_chunk),
        ] {
            if let Err(e) = load_bc(k, b) {
                init_err = Some(e);
                break;
            }
        }

        let Ok(module_key) = lua.create_registry_value(fx_tbl) else {
            return;
        };
        let Ok(state_tbl) = lua.create_table() else {
            return;
        };
        let Ok(state_key) = lua.create_registry_value(state_tbl) else {
            return;
        };

        self.inner = Some(LuaScriptInner {
            lua,
            module_key,
            state_key,
            init_called: false,
            width,
            height,
            fps,
            accum_dt: 1.0,
            elapsed: 0.0,
            last_error: init_err,
        });
    }

    fn initialize_with_spec(&mut self, spec: &LuaEffectSpec, width: u16, height: u16) {
        let lua = Lua::new();
        if configure_lua_vm(&lua, None).is_err() {
            return;
        }

        let normalized = spec.name.trim().to_ascii_lowercase().replace('-', "_");
        let mod_name = if normalized.contains('.') {
            normalized.clone()
        } else {
            format!("effects.{normalized}")
        };

        let Ok(opts) = lua.create_table() else {
            return;
        };
        let _ = opts.set("fps", spec.fps);
        let _ = opts.set("density", spec.density);
        let _ = opts.set("speed", spec.speed);
        if let Some(c) = &spec.color {
            let _ = opts.set("color", c.as_str());
        }
        if let Some(hc) = &spec.head_color {
            let _ = opts.set("head_color", hc.as_str());
        }
        if let Some(cs) = &spec.charset {
            let _ = opts.set("charset", cs.as_str());
        }
        if let Some(sp) = spec.spacing {
            let _ = opts.set("spacing", sp);
        }
        if let Some(cg) = &spec.char_glyph {
            let _ = opts.set("char", cg.as_str());
        }
        if let Some(cnt) = spec.count {
            let _ = opts.set("count", cnt);
        }

        let load_fn: mlua::Result<Table> = (|| {
            let require: Function = lua.globals().get("require")?;
            let mod_val: Value = require.call(mod_name.as_str())?;
            match mod_val {
                Value::Table(mod_tbl) => {
                    if let Ok(Value::Function(setup)) = mod_tbl.get::<Value>("setup") {
                        setup.call(opts)
                    } else {
                        Ok(mod_tbl)
                    }
                }
                Value::Function(setup) => setup.call(opts),
                _ => Err(mlua::Error::RuntimeError(format!(
                    "Lua effect '{mod_name}' did not return a module table or setup function"
                ))),
            }
        })();

        let fx_tbl = match load_fn {
            Ok(t) => t,
            Err(_) => {
                self.inner = None;
                return;
            }
        };

        let fps = fx_tbl.get::<u32>("fps").unwrap_or(spec.fps).clamp(1, 60);
        let Ok(module_key) = lua.create_registry_value(fx_tbl) else {
            return;
        };
        let Ok(state_tbl) = lua.create_table() else {
            return;
        };
        let Ok(state_key) = lua.create_registry_value(state_tbl) else {
            return;
        };

        self.inner = Some(LuaScriptInner {
            lua,
            module_key,
            state_key,
            init_called: false,
            width,
            height,
            fps,
            accum_dt: 1.0,
            elapsed: 0.0,
            last_error: None,
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn build_script_ctx(
    owner_name: &str,
    owner_description: &str,
    theme_snap: &Theme,
    lua: &Lua,
    w: u16,
    h: u16,
    dt: f32,
    elapsed: f32,
    fps: u32,
    state_tbl: Option<Table>,
) -> mlua::Result<Table> {
    let ctx = lua.create_table()?;
    ctx.set("name", owner_name)?;
    ctx.set("description", owner_description)?;
    ctx.set("width", w)?;
    ctx.set("height", h)?;
    ctx.set("dt", dt)?;
    ctx.set("t", elapsed)?;
    ctx.set("time", elapsed)?;
    ctx.set("elapsed", elapsed)?;
    ctx.set("fps", fps)?;
    if let Some(st) = state_tbl {
        ctx.set("state", st)?;
    }
    ctx.set("theme", build_theme_lua_table(lua, theme_snap)?)?;
    attach_math_helpers(lua, &ctx)?;
    Ok(ctx)
}

/// Hosts a Lua effect module (`effects.<name>`) behind EgoTop's `BackgroundEffect` trait,
/// rendering strictly onto the background's exclusive `CanvasBuffer` at its configured FPS.
pub struct LuaEffectAdapter {
    spec: LuaEffectSpec,
    host: LuaCanvasInstance,
}

impl LuaEffectAdapter {
    pub fn from_spec(spec: LuaEffectSpec, width: u16, height: u16, theme: Theme) -> Self {
        let host = LuaCanvasInstance::from_spec_named(
            "background",
            format!("Independent background effect '{}'", spec.name),
            &spec,
            width,
            height,
            theme,
        );
        Self { spec, host }
    }

    pub fn from_effect_name(
        name: &str,
        width: u16,
        height: u16,
        cfg: &BackgroundConfig,
        theme: Theme,
    ) -> Self {
        let spec = LuaEffectSpec {
            name: name.to_string(),
            fps: 30,
            density: cfg.density,
            speed: cfg.speed,
            color: None,
            head_color: None,
            charset: None,
            spacing: None,
            char_glyph: None,
            count: None,
        };
        Self::from_spec(spec, width, height, theme)
    }

    pub fn set_theme(&mut self, theme: Theme) {
        self.host.set_theme(theme);
    }

    pub fn last_error(&self) -> Option<&str> {
        self.host.last_error()
    }

    pub fn name(&self) -> &str {
        &self.spec.name
    }
}

impl BackgroundEffect for LuaEffectAdapter {
    fn kind(&self) -> EffectKind {
        match self.spec.name.to_ascii_lowercase().as_str() {
            "matrix" | "matrix_rain" => EffectKind::Matrix,
            "water" | "waves" => EffectKind::Water,
            "particles" => EffectKind::Particles,
            "scanlines" => EffectKind::Scanlines,
            _ => EffectKind::None,
        }
    }

    fn resize(&mut self, width: u16, height: u16, _cfg: &BackgroundConfig) {
        self.host.resize(width, height);
    }

    fn update(&mut self, dt_secs: f32, cfg: &BackgroundConfig) {
        if !cfg.enabled {
            self.host.clear_buffer();
            return;
        }
        self.host.update(dt_secs, cfg.speed);
    }

    fn render(&self, buf: &mut Buffer, area: Rect, cfg: &BackgroundConfig, theme: &Theme) {
        if !cfg.enabled || area.width == 0 || area.height == 0 {
            return;
        }
        self.host.render(buf, area, theme);
    }
}
