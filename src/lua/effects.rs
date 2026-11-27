use crate::animation::BackgroundEffect;
use crate::config::{BackgroundConfig, EffectKind};
use crate::lua::configure_lua_vm;
use crate::ui::theme::{Theme, blend_color, parse_color};
use mlua::{Function, Lua, RegistryKey, Table, UserData, UserDataMethods, Value};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
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
        let fps = tbl.get::<u32>("fps").unwrap_or(24).clamp(1, 60);
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

#[derive(Debug, Clone)]
struct CachedCell {
    x: u16,
    y: u16,
    ch: String,
    fg: Color,
    bg: Option<Color>,
}

#[derive(Clone)]
struct DrawCanvasHandle {
    cells: Arc<Mutex<Vec<CachedCell>>>,
    width: u16,
    height: u16,
    theme: Theme,
}

impl UserData for DrawCanvasHandle {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("cell", |_, this, (x, y, ch, fg, bg): (i32, i32, String, Option<String>, Option<String>)| {
            if x < 0 || y < 0 || x >= this.width as i32 || y >= this.height as i32 {
                return Ok(());
            }
            let fg_col = fg
                .as_deref()
                .map(|s| parse_color(s, this.theme.primary))
                .unwrap_or(this.theme.text);
            let bg_col = bg.as_deref().map(|s| parse_color(s, Color::Reset));
            this.cells.lock().unwrap().push(CachedCell {
                x: x as u16,
                y: y as u16,
                ch,
                fg: fg_col,
                bg: bg_col,
            });
            Ok(())
        });

        methods.add_method("text", |_, this, (x, y, txt, fg, bg): (i32, i32, String, Option<String>, Option<String>)| {
            if y < 0 || y >= this.height as i32 {
                return Ok(());
            }
            let fg_col = fg
                .as_deref()
                .map(|s| parse_color(s, this.theme.primary))
                .unwrap_or(this.theme.text);
            let bg_col = bg.as_deref().map(|s| parse_color(s, Color::Reset));
            let mut guard = this.cells.lock().unwrap();
            for (i, ch) in txt.chars().enumerate() {
                let px = x + i as i32;
                if px >= 0 && px < this.width as i32 {
                    guard.push(CachedCell {
                        x: px as u16,
                        y: y as u16,
                        ch: ch.to_string(),
                        fg: fg_col,
                        bg: bg_col,
                    });
                }
            }
            Ok(())
        });

        methods.add_method("fill", |_, this, (x, y, w, h, ch, fg, bg): (i32, i32, i32, i32, String, Option<String>, Option<String>)| {
            let fg_col = fg
                .as_deref()
                .map(|s| parse_color(s, this.theme.muted))
                .unwrap_or(this.theme.muted);
            let bg_col = bg.as_deref().map(|s| parse_color(s, Color::Reset));
            let glyph = if ch.is_empty() { " ".to_string() } else { ch };
            let mut guard = this.cells.lock().unwrap();
            for dy in 0..h {
                for dx in 0..w {
                    let px = x + dx;
                    let py = y + dy;
                    if px >= 0 && py >= 0 && px < this.width as i32 && py < this.height as i32 {
                        guard.push(CachedCell {
                            x: px as u16,
                            y: py as u16,
                            ch: glyph.clone(),
                            fg: fg_col,
                            bg: bg_col,
                        });
                    }
                }
            }
            Ok(())
        });

        methods.add_method("blend", |_, _, (c1, c2, t): (String, String, f32)| {
            let a = parse_color(&c1, Color::White);
            let b = parse_color(&c2, Color::Black);
            Ok(color_to_hex(blend_color(a, b, t.clamp(0.0, 1.0))))
        });
    }
}

fn color_to_hex(c: Color) -> String {
    match c {
        Color::Rgb(r, g, b) => format!("#{r:02X}{g:02X}{b:02X}"),
        _ => "#FFFFFF".to_string(),
    }
}

struct LuaEffectInner {
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

/// Hosts a Lua effect module (`effects.<name>.setup(opts)`) behind EgoTop's `BackgroundEffect` trait,
/// throttling Lua updates to the effect's target FPS and caching the resulting cells.
pub struct LuaEffectAdapter {
    spec: LuaEffectSpec,
    inner: Option<LuaEffectInner>,
    cached_cells: Vec<CachedCell>,
    theme_snapshot: Theme,
}

impl LuaEffectAdapter {
    pub fn from_spec(spec: LuaEffectSpec, width: u16, height: u16, theme: Theme) -> Self {
        let mut adapter = Self {
            spec,
            inner: None,
            cached_cells: Vec::new(),
            theme_snapshot: theme,
        };
        adapter.initialize(width, height);
        adapter
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
            fps: 24,
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
        self.theme_snapshot = theme;
    }

    pub fn last_error(&self) -> Option<&str> {
        self.inner.as_ref().and_then(|i| i.last_error.as_deref())
    }

    fn initialize(&mut self, width: u16, height: u16) {
        let lua = Lua::new();
        if configure_lua_vm(&lua, None).is_err() {
            return;
        }
        let mod_name = format!(
            "effects.{}",
            self.spec.name.trim().to_ascii_lowercase().replace('-', "_")
        );
        let Ok(opts) = lua.create_table() else {
            return;
        };
        let _ = opts.set("fps", self.spec.fps);
        let _ = opts.set("density", self.spec.density);
        let _ = opts.set("speed", self.spec.speed);
        if let Some(c) = &self.spec.color {
            let _ = opts.set("color", c.as_str());
        }
        if let Some(hc) = &self.spec.head_color {
            let _ = opts.set("head_color", hc.as_str());
        }
        if let Some(cs) = &self.spec.charset {
            let _ = opts.set("charset", cs.as_str());
        }
        if let Some(sp) = self.spec.spacing {
            let _ = opts.set("spacing", sp);
        }
        if let Some(cg) = &self.spec.char_glyph {
            let _ = opts.set("char", cg.as_str());
        }
        if let Some(cnt) = self.spec.count {
            let _ = opts.set("count", cnt);
        }

        let load_fn: mlua::Result<Table> = (|| {
            let require: Function = lua.globals().get("require")?;
            let mod_tbl: Table = require.call(mod_name.as_str())?;
            let setup: Function = mod_tbl.get("setup")?;
            setup.call(opts)
        })();

        let Ok(fx_tbl) = load_fn else {
            return;
        };
        let fps = fx_tbl.get::<u32>("fps").unwrap_or(self.spec.fps).clamp(1, 60);
        let Ok(module_key) = lua.create_registry_value(fx_tbl) else {
            return;
        };
        let Ok(state_tbl) = lua.create_table() else {
            return;
        };
        let Ok(state_key) = lua.create_registry_value(state_tbl) else {
            return;
        };

        self.inner = Some(LuaEffectInner {
            lua,
            module_key,
            state_key,
            init_called: false,
            width,
            height,
            fps,
            accum_dt: 1.0, // trigger initial render immediately
            elapsed: 0.0,
            last_error: None,
        });
    }
}

fn build_ctx_table_for_theme(
    theme_snap: &Theme,
    lua: &Lua,
    w: u16,
    h: u16,
    elapsed: f32,
) -> mlua::Result<Table> {
    let ctx = lua.create_table()?;
    ctx.set("width", w)?;
    ctx.set("height", h)?;
    ctx.set("elapsed", elapsed)?;

    let theme = lua.create_table()?;
    theme.set("primary", color_to_hex(theme_snap.primary))?;
    theme.set("secondary", color_to_hex(theme_snap.secondary))?;
    theme.set("accent", color_to_hex(theme_snap.accent))?;
    theme.set("text", color_to_hex(theme_snap.text))?;
    theme.set("muted", color_to_hex(theme_snap.muted))?;
    ctx.set("theme", theme)?;
    Ok(ctx)
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
        if let Some(inner) = self.inner.as_mut() {
            inner.width = width;
            inner.height = height;
            inner.accum_dt = 1.0;
            if let (Ok(fx), Ok(st), Ok(ctx)) = (
                inner.lua.registry_value::<Table>(&inner.module_key),
                inner.lua.registry_value::<Table>(&inner.state_key),
                build_ctx_table_for_theme(&self.theme_snapshot, &inner.lua, width, height, inner.elapsed),
            ) && let Ok(Value::Function(resize_fn)) = fx.get::<Value>("resize")
                && let Err(e) = resize_fn.call::<()>((st, ctx))
            {
                inner.last_error = Some(e.to_string());
            }
        }
    }

    fn update(&mut self, dt_secs: f32, cfg: &BackgroundConfig) {
        if !cfg.enabled {
            self.cached_cells.clear();
            return;
        }
        let Some(inner) = self.inner.as_mut() else {
            return;
        };
        inner.accum_dt += dt_secs;
        let min_step = 1.0 / (inner.fps.max(1) as f32);
        if inner.accum_dt < min_step {
            return;
        }
        let step_dt = inner.accum_dt * cfg.speed.max(0.05);
        inner.accum_dt = 0.0;
        inner.elapsed += step_dt;

        let Ok(fx) = inner.lua.registry_value::<Table>(&inner.module_key) else {
            return;
        };
        let Ok(st) = inner.lua.registry_value::<Table>(&inner.state_key) else {
            return;
        };
        let Ok(ctx) = build_ctx_table_for_theme(
            &self.theme_snapshot,
            &inner.lua,
            inner.width,
            inner.height,
            inner.elapsed,
        ) else {
            return;
        };

        if !inner.init_called {
            inner.init_called = true;
            if let Ok(Value::Function(init_fn)) = fx.get::<Value>("init")
                && let Err(e) = init_fn.call::<()>((st.clone(), ctx.clone()))
            {
                inner.last_error = Some(e.to_string());
                return;
            }
        }

        if let Ok(Value::Function(update_fn)) = fx.get::<Value>("update")
            && let Err(e) = update_fn.call::<()>((st.clone(), step_dt, ctx.clone()))
        {
            inner.last_error = Some(e.to_string());
            return;
        }

        let buf = Arc::new(Mutex::new(Vec::new()));
        let draw_handle = DrawCanvasHandle {
            cells: Arc::clone(&buf),
            width: inner.width,
            height: inner.height,
            theme: self.theme_snapshot.clone(),
        };
        if let Ok(Value::Function(render_fn)) = fx.get::<Value>("render")
            && let Err(e) = render_fn.call::<()>((st, draw_handle, ctx))
        {
            inner.last_error = Some(e.to_string());
            return;
        }
        self.cached_cells = std::mem::take(&mut *buf.lock().unwrap());
    }

    fn render(&self, buf: &mut Buffer, area: Rect, cfg: &BackgroundConfig, _theme: &Theme) {
        if !cfg.enabled || area.width == 0 || area.height == 0 {
            return;
        }
        for cell in &self.cached_cells {
            let x = area.x + cell.x;
            let y = area.y + cell.y;
            if x < area.right()
                && y < area.bottom()
                && let Some(dst) = buf.cell_mut((x, y))
            {
                dst.set_symbol(&cell.ch);
                dst.set_fg(cell.fg);
                if let Some(bg) = cell.bg {
                    dst.set_bg(bg);
                }
            }
        }
    }
}
