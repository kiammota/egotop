use crate::animation::BackgroundEffect;
use crate::config::{BackgroundConfig, EffectKind};
use crate::lua::canvas::{
    CanvasBuffer, LuaCanvasHandle, attach_math_helpers, build_theme_lua_table,
};
use crate::lua::configure_lua_vm;
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
    legacy_stateful: bool,
    width: u16,
    height: u16,
    fps: u32,
    accum_dt: f32,
    elapsed: f32,
    last_error: Option<String>,
}

/// Reusable stateful Lua shader/canvas host supporting `setup(config)` -> `init(ctx)`,
/// `update(dt, t, ctx)`, and `render(canvas, ctx)` with clipped truecolor & z-buffered frames.
pub struct LuaCanvasInstance {
    pub script_name: String,
    inner: Option<LuaScriptInner>,
    canvas_buf: Arc<Mutex<CanvasBuffer>>,
    theme_snapshot: Theme,
}

impl LuaCanvasInstance {
    pub fn from_spec(spec: &LuaEffectSpec, width: u16, height: u16, theme: Theme) -> Self {
        let mut inst = Self {
            script_name: spec.name.clone(),
            inner: None,
            canvas_buf: Arc::new(Mutex::new(CanvasBuffer::new(width, height))),
            theme_snapshot: theme,
        };
        inst.initialize_with_spec(spec, width, height);
        inst
    }

    pub fn from_script_name(
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
        Self::from_spec(&spec, width, height, theme)
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
            let _ = ctx.set("state", st.clone());
            if let Ok(Value::Function(resize_fn)) = fx.get::<Value>("resize") {
                let res = if inner.legacy_stateful {
                    resize_fn.call::<()>((st, ctx))
                } else {
                    resize_fn.call::<()>(ctx)
                };
                if let Err(e) = res {
                    inner.last_error = Some(e.to_string());
                }
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
            &self.theme_snapshot,
            &inner.lua,
            inner.width,
            inner.height,
            step_dt,
            inner.elapsed,
            inner.fps,
            Some(st.clone()),
        ) else {
            return;
        };

        if !inner.init_called {
            inner.init_called = true;
            if let Ok(Value::Function(init_fn)) = fx.get::<Value>("init") {
                let res = if inner.legacy_stateful {
                    init_fn.call::<()>((st.clone(), ctx.clone()))
                } else {
                    init_fn.call::<()>(ctx.clone())
                };
                if let Err(e) = res {
                    inner.last_error = Some(e.to_string());
                    return;
                }
            }
        }

        if let Ok(Value::Function(update_fn)) = fx.get::<Value>("update") {
            let res = if inner.legacy_stateful {
                update_fn.call::<()>((st.clone(), step_dt, ctx.clone()))
            } else {
                update_fn.call::<()>((step_dt, inner.elapsed, ctx.clone()))
            };
            if let Err(e) = res {
                inner.last_error = Some(e.to_string());
                return;
            }
        }

        {
            let mut guard = self.canvas_buf.lock().unwrap();
            guard.resize(inner.width, inner.height);
            guard.begin_frame();
        }
        let canvas_handle =
            LuaCanvasHandle::new(Arc::clone(&self.canvas_buf), self.theme_snapshot.clone());

        if let Ok(Value::Function(render_fn)) = fx.get::<Value>("render") {
            let res = if inner.legacy_stateful {
                render_fn.call::<()>((st, canvas_handle, ctx))
            } else {
                render_fn.call::<()>((canvas_handle, ctx))
            };
            if let Err(e) = res {
                inner.last_error = Some(e.to_string());
            }
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
            Err(e) => {
                self.inner = None;
                let _ = e;
                return;
            }
        };

        let fps = fx_tbl.get::<u32>("fps").unwrap_or(spec.fps).clamp(1, 60);
        let explicit_legacy = fx_tbl
            .get::<Option<bool>>("legacy")
            .ok()
            .flatten()
            .unwrap_or(false)
            || fx_tbl
                .get::<Option<u32>>("api_version")
                .ok()
                .flatten()
                .is_some_and(|v| v == 1);
        // Auto-detect legacy 3-arg `render(state, draw, ctx)` / 2-arg `init(state, ctx)` scripts
        // so user customizations on disk keep working cleanly alongside the new canvas API.
        let inferred_legacy = if !explicit_legacy {
            lua.load(
                r#"
                return function(fx)
                    if type(debug) ~= "table" or type(debug.getinfo) ~= "function" then return false end
                    if type(fx.render) == "function" then
                        local info = debug.getinfo(fx.render, "u")
                        if info and info.nparams == 3 then return true end
                    end
                    if type(fx.init) == "function" then
                        local info = debug.getinfo(fx.init, "u")
                        if info and info.nparams == 2 then return true end
                    end
                    return false
                end
                "#,
            )
            .eval::<Function>()
            .and_then(|f| f.call::<bool>(fx_tbl.clone()))
            .unwrap_or(false)
        } else {
            false
        };
        let legacy_stateful = explicit_legacy || inferred_legacy;

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
            legacy_stateful,
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
/// rendering onto the clipped `CanvasBuffer` at its configured FPS.
pub struct LuaEffectAdapter {
    spec: LuaEffectSpec,
    host: LuaCanvasInstance,
}

impl LuaEffectAdapter {
    pub fn from_spec(spec: LuaEffectSpec, width: u16, height: u16, theme: Theme) -> Self {
        let host = LuaCanvasInstance::from_spec(&spec, width, height, theme);
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
