use crate::ui::theme::{Theme, blend_color, parse_color};
use mlua::{Function, Lua, Table, UserData, UserDataFields, UserDataMethods, Value};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use std::sync::{Arc, Mutex};
use unicode_width::UnicodeWidthChar;

#[derive(Debug, Clone)]
pub struct CanvasCell {
    pub ch: String,
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub intensity: f32,
}

/// Reusable, clipping, z-buffered cell canvas shared between Rust and Lua frame render callbacks.
#[derive(Debug, Clone)]
pub struct CanvasBuffer {
    pub width: u16,
    pub height: u16,
    cells: Vec<Option<CanvasCell>>,
    zbuf: Vec<f32>,
}

impl CanvasBuffer {
    pub fn new(width: u16, height: u16) -> Self {
        let len = (width as usize).saturating_mul(height as usize);
        Self {
            width,
            height,
            cells: vec![None; len],
            zbuf: vec![f32::NEG_INFINITY; len],
        }
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        if self.width == width && self.height == height {
            return;
        }
        self.width = width;
        self.height = height;
        let len = (width as usize).saturating_mul(height as usize);
        self.cells = vec![None; len];
        self.zbuf = vec![f32::NEG_INFINITY; len];
    }

    pub fn begin_frame(&mut self) {
        self.cells.fill(None);
        self.zbuf.fill(f32::NEG_INFINITY);
    }

    pub fn clear(&mut self, ch: &str, fg: Option<Color>, bg: Option<Color>) {
        self.zbuf.fill(f32::NEG_INFINITY);
        if ch == " " && fg.is_none() && bg.is_none() {
            self.cells.fill(None);
            return;
        }
        let glyph = normalize_cell_glyph(ch);
        let cell = CanvasCell {
            ch: glyph,
            fg,
            bg,
            intensity: 1.0,
        };
        for slot in &mut self.cells {
            *slot = Some(cell.clone());
        }
    }

    /// Writes a cell at integer coordinates `(x, y)` with strict automatic clipping to `[0..width, 0..height)`.
    pub fn put(
        &mut self,
        x: i32,
        y: i32,
        ch: &str,
        fg: Option<Color>,
        bg: Option<Color>,
        intensity: f32,
    ) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return;
        }
        if intensity <= 0.0 {
            return;
        }
        let idx = (y as usize) * (self.width as usize) + (x as usize);
        if idx >= self.cells.len() {
            return;
        }
        self.cells[idx] = Some(CanvasCell {
            ch: normalize_cell_glyph(ch),
            fg,
            bg,
            intensity: intensity.clamp(0.0, 2.0),
        });
    }

    /// Z-buffered write: only overwrites the cell if `z >= current_zbuf[x, y]`.
    #[allow(clippy::too_many_arguments)]
    pub fn put_z(
        &mut self,
        x: i32,
        y: i32,
        z: f32,
        ch: &str,
        fg: Option<Color>,
        bg: Option<Color>,
        intensity: f32,
    ) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return;
        }
        if intensity <= 0.0 {
            return;
        }
        let idx = (y as usize) * (self.width as usize) + (x as usize);
        if idx >= self.cells.len() {
            return;
        }
        if z < self.zbuf[idx] {
            return;
        }
        self.zbuf[idx] = z;
        self.cells[idx] = Some(CanvasCell {
            ch: normalize_cell_glyph(ch),
            fg,
            bg,
            intensity: intensity.clamp(0.0, 2.0),
        });
    }

    pub fn text(
        &mut self,
        x: i32,
        y: i32,
        s: &str,
        fg: Option<Color>,
        bg: Option<Color>,
        intensity: f32,
    ) {
        if y < 0 || y >= self.height as i32 || intensity <= 0.0 {
            return;
        }
        let mut col = x;
        for ch in s.chars() {
            if col >= self.width as i32 {
                break;
            }
            let w = ch.width().unwrap_or(1).max(1) as i32;
            if col >= 0 {
                let mut buf = [0u8; 4];
                let glyph = ch.encode_utf8(&mut buf);
                self.put(col, y, glyph, fg, bg, intensity);
            }
            col += w;
        }
    }

    pub fn get_cell(&self, x: u16, y: u16) -> Option<&CanvasCell> {
        if x >= self.width || y >= self.height {
            return None;
        }
        self.cells
            .get((y as usize) * (self.width as usize) + (x as usize))
            .and_then(|c| c.as_ref())
    }

    pub fn blit_to_buffer(&self, buf: &mut Buffer, area: Rect, fallback_fg: Color) {
        let max_w = self.width.min(area.width);
        let max_h = self.height.min(area.height);
        for y in 0..max_h {
            for x in 0..max_w {
                let Some(cell) = self.get_cell(x, y) else {
                    continue;
                };
                if let Some(dst) = buf.cell_mut((area.x + x, area.y + y)) {
                    dst.set_symbol(&cell.ch);
                    let base_fg = cell.fg.unwrap_or(fallback_fg);
                    dst.set_fg(scale_color_intensity(base_fg, cell.intensity));
                    if let Some(bg) = cell.bg {
                        dst.set_bg(bg);
                    }
                }
            }
        }
    }
}

fn normalize_cell_glyph(ch: &str) -> String {
    match ch.chars().next() {
        Some(c) => {
            let mut buf = [0u8; 4];
            c.encode_utf8(&mut buf).to_string()
        }
        None => " ".to_string(),
    }
}

/// Lua UserData handle wrapping a shared `CanvasBuffer`.
#[derive(Clone)]
pub struct LuaCanvasHandle {
    pub buffer: Arc<Mutex<CanvasBuffer>>,
    pub theme: Theme,
}

impl LuaCanvasHandle {
    pub fn new(buffer: Arc<Mutex<CanvasBuffer>>, theme: Theme) -> Self {
        Self { buffer, theme }
    }
}

impl UserData for LuaCanvasHandle {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("width", |_, this| {
            Ok(this.buffer.lock().unwrap().width)
        });
        fields.add_field_method_get("height", |_, this| {
            Ok(this.buffer.lock().unwrap().height)
        });
    }

    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method(
            "clear",
            |_, this, (ch, fg, bg): (Option<String>, Option<Value>, Option<Value>)| {
                let glyph = ch.as_deref().unwrap_or(" ");
                let fg_col = fg.as_ref().and_then(|v| parse_lua_color(v, true));
                let bg_col = bg.as_ref().and_then(|v| parse_lua_color(v, false));
                this.buffer.lock().unwrap().clear(glyph, fg_col, bg_col);
                Ok(())
            },
        );

        methods.add_method(
            "put",
            |_,
             this,
             (x, y, ch, fg, bg, intensity): (
                f64,
                f64,
                String,
                Option<Value>,
                Option<Value>,
                Option<f32>,
            )| {
                let fg_col = fg
                    .as_ref()
                    .and_then(|v| parse_lua_color(v, true))
                    .or(Some(this.theme.text));
                let bg_col = bg.as_ref().and_then(|v| parse_lua_color(v, true));
                let intens = intensity.unwrap_or(1.0);
                this.buffer.lock().unwrap().put(
                    x.floor() as i32,
                    y.floor() as i32,
                    &ch,
                    fg_col,
                    bg_col,
                    intens,
                );
                Ok(())
            },
        );

        methods.add_method(
            "put_z",
            |_,
             this,
             (x, y, z, ch, fg, bg, intensity): (
                f64,
                f64,
                f64,
                String,
                Option<Value>,
                Option<Value>,
                Option<f32>,
            )| {
                let fg_col = fg
                    .as_ref()
                    .and_then(|v| parse_lua_color(v, true))
                    .or(Some(this.theme.text));
                let bg_col = bg.as_ref().and_then(|v| parse_lua_color(v, true));
                let intens = intensity.unwrap_or(1.0);
                this.buffer.lock().unwrap().put_z(
                    x.floor() as i32,
                    y.floor() as i32,
                    z as f32,
                    &ch,
                    fg_col,
                    bg_col,
                    intens,
                );
                Ok(())
            },
        );

        methods.add_method(
            "text",
            |_,
             this,
             (x, y, text, fg, bg, intensity): (
                f64,
                f64,
                String,
                Option<Value>,
                Option<Value>,
                Option<f32>,
            )| {
                let fg_col = fg
                    .as_ref()
                    .and_then(|v| parse_lua_color(v, true))
                    .or(Some(this.theme.text));
                let bg_col = bg.as_ref().and_then(|v| parse_lua_color(v, true));
                let intens = intensity.unwrap_or(1.0);
                this.buffer.lock().unwrap().text(
                    x.floor() as i32,
                    y.floor() as i32,
                    &text,
                    fg_col,
                    bg_col,
                    intens,
                );
                Ok(())
            },
        );

        // Legacy compatibility aliases (`cell`, `fill`, `blend`)
        methods.add_method(
            "cell",
            |_, this, (x, y, ch, fg, bg): (f64, f64, String, Option<Value>, Option<Value>)| {
                let fg_col = fg
                    .as_ref()
                    .and_then(|v| parse_lua_color(v, true))
                    .or(Some(this.theme.text));
                let bg_col = bg.as_ref().and_then(|v| parse_lua_color(v, true));
                this.buffer.lock().unwrap().put(
                    x.floor() as i32,
                    y.floor() as i32,
                    &ch,
                    fg_col,
                    bg_col,
                    1.0,
                );
                Ok(())
            },
        );

        methods.add_method(
            "fill",
            |_,
             this,
             (x, y, w, h, ch, fg, bg): (
                i32,
                i32,
                i32,
                i32,
                String,
                Option<Value>,
                Option<Value>,
            )| {
                let fg_col = fg
                    .as_ref()
                    .and_then(|v| parse_lua_color(v, true))
                    .or(Some(this.theme.text));
                let bg_col = bg.as_ref().and_then(|v| parse_lua_color(v, true));
                let mut buf = this.buffer.lock().unwrap();
                for dy in 0..h.max(0) {
                    for dx in 0..w.max(0) {
                        buf.put(x + dx, y + dy, &ch, fg_col, bg_col, 1.0);
                    }
                }
                Ok(())
            },
        );

        methods.add_method("blend", |_, _, (c1, c2, t): (Value, Value, f32)| {
            let a = parse_lua_color(&c1, false).unwrap_or(Color::White);
            let b = parse_lua_color(&c2, false).unwrap_or(Color::White);
            let out = blend_color(a, b, t.clamp(0.0, 1.0));
            Ok(color_to_hex(out))
        });

        // Math & color convenience methods callable as `canvas:lerp(...)` etc.
        methods.add_method("clamp", |_, _, (v, lo, hi): (f64, f64, f64)| {
            Ok(v.clamp(lo.min(hi), lo.max(hi)))
        });
        methods.add_method("lerp", |_, _, (a, b, t): (f64, f64, f64)| {
            Ok(a + (b - a) * t)
        });
        methods.add_method("hsv_to_rgb", |lua, _, (h, s, v): (f64, f64, f64)| {
            let (r, g, b) = hsv_to_rgb_u8(h, s, v);
            rgb_table(lua, r, g, b)
        });
        methods.add_method("rgb_scale", |lua, _, (col, factor): (Value, f32)| {
            let base = parse_lua_color(&col, false).unwrap_or(Color::White);
            let scaled = scale_color_intensity(base, factor);
            let (r, g, b) = color_to_rgb_u8(scaled);
            rgb_table(lua, r, g, b)
        });
        methods.add_method(
            "noise",
            |_, _, (x, y, z): (f64, Option<f64>, Option<f64>)| {
                Ok(value_noise_3d(x, y.unwrap_or(0.0), z.unwrap_or(0.0)))
            },
        );
    }
}

/// Registers math and color utilities into a Lua table (`egotop.math` and `ctx.math`).
pub fn create_math_table(lua: &Lua) -> mlua::Result<Table> {
    let tbl = lua.create_table()?;
    tbl.set("sin", lua.create_function(|_, x: f64| Ok(x.sin()))?)?;
    tbl.set("cos", lua.create_function(|_, x: f64| Ok(x.cos()))?)?;
    tbl.set(
        "clamp",
        lua.create_function(|_, (v, lo, hi): (f64, f64, f64)| Ok(v.clamp(lo.min(hi), lo.max(hi))))?,
    )?;
    tbl.set(
        "lerp",
        lua.create_function(|_, (a, b, t): (f64, f64, f64)| Ok(a + (b - a) * t))?,
    )?;
    tbl.set(
        "smoothstep",
        lua.create_function(|_, (edge0, edge1, x): (f64, f64, f64)| {
            let denom = (edge1 - edge0).abs().max(1e-9);
            let t = ((x - edge0) / denom).clamp(0.0, 1.0);
            Ok(t * t * (3.0 - 2.0 * t))
        })?,
    )?;
    tbl.set(
        "hsv_to_rgb",
        lua.create_function(|lua, (h, s, v): (f64, f64, f64)| {
            let (r, g, b) = hsv_to_rgb_u8(h, s, v);
            rgb_table(lua, r, g, b)
        })?,
    )?;
    tbl.set(
        "rgb_scale",
        lua.create_function(|lua, (col, factor): (Value, f32)| {
            let base = parse_lua_color(&col, false).unwrap_or(Color::White);
            let scaled = scale_color_intensity(base, factor);
            let (r, g, b) = color_to_rgb_u8(scaled);
            rgb_table(lua, r, g, b)
        })?,
    )?;
    tbl.set(
        "rgb_lerp",
        lua.create_function(|lua, (c1, c2, t): (Value, Value, f32)| {
            let a = parse_lua_color(&c1, false).unwrap_or(Color::White);
            let b = parse_lua_color(&c2, false).unwrap_or(Color::White);
            let (r, g, b_ch) = color_to_rgb_u8(blend_color(a, b, t.clamp(0.0, 1.0)));
            rgb_table(lua, r, g, b_ch)
        })?,
    )?;
    tbl.set(
        "rgb_hex",
        lua.create_function(|_, col: Value| {
            let c = parse_lua_color(&col, false).unwrap_or(Color::White);
            Ok(color_to_hex(c))
        })?,
    )?;
    tbl.set(
        "noise",
        lua.create_function(|_, (x, y, z): (f64, Option<f64>, Option<f64>)| {
            Ok(value_noise_3d(x, y.unwrap_or(0.0), z.unwrap_or(0.0)))
        })?,
    )?;
    tbl.set(
        "hash",
        lua.create_function(|_, (x, y, seed): (i64, i64, Option<i64>)| {
            Ok(hash_01(x, y, seed.unwrap_or(0)))
        })?,
    )?;
    Ok(tbl)
}

/// Also exposes top-level helper functions directly onto `ctx` (`ctx.sin`, `ctx.hsv_to_rgb`, etc.)
/// or `egotop` so shader scripts can call them concisely.
pub fn attach_math_helpers(lua: &Lua, target: &Table) -> mlua::Result<()> {
    let math_tbl = create_math_table(lua)?;
    for pair in math_tbl.clone().pairs::<String, Function>() {
        let (k, f) = pair?;
        if !target.contains_key(k.as_str())? {
            target.set(k, f)?;
        }
    }
    target.set("math", math_tbl)?;
    Ok(())
}

pub fn build_theme_lua_table(lua: &Lua, theme: &Theme) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.set("background", color_to_hex(theme.background))?;
    t.set("panel_background", color_to_hex(theme.panel_background))?;
    t.set("primary", color_to_hex(theme.primary))?;
    t.set("secondary", color_to_hex(theme.secondary))?;
    t.set("accent", color_to_hex(theme.accent))?;
    t.set("text", color_to_hex(theme.text))?;
    t.set("muted", color_to_hex(theme.muted))?;
    t.set("border", color_to_hex(theme.border))?;
    t.set("danger", color_to_hex(theme.danger))?;
    Ok(t)
}

pub fn parse_lua_color(val: &Value, none_if_transparent: bool) -> Option<Color> {
    match val {
        Value::Nil => None,
        Value::String(s) => {
            let txt = s.to_str().ok()?.trim().to_string();
            if txt.is_empty()
                || txt.eq_ignore_ascii_case("none")
                || txt.eq_ignore_ascii_case("transparent")
                || txt.eq_ignore_ascii_case("reset")
            {
                if none_if_transparent {
                    None
                } else {
                    Some(Color::Reset)
                }
            } else {
                Some(parse_color(&txt, Color::White))
            }
        }
        Value::Table(tbl) => {
            let r = tbl
                .get::<f64>("r")
                .or_else(|_| tbl.get::<f64>(1))
                .ok()?;
            let g = tbl
                .get::<f64>("g")
                .or_else(|_| tbl.get::<f64>(2))
                .ok()?;
            let b = tbl
                .get::<f64>("b")
                .or_else(|_| tbl.get::<f64>(3))
                .ok()?;
            Some(Color::Rgb(
                r.round().clamp(0.0, 255.0) as u8,
                g.round().clamp(0.0, 255.0) as u8,
                b.round().clamp(0.0, 255.0) as u8,
            ))
        }
        _ => None,
    }
}

pub fn scale_color_intensity(color: Color, intensity: f32) -> Color {
    if (intensity - 1.0).abs() < f32::EPSILON {
        return color;
    }
    let k = intensity.clamp(0.0, 2.0);
    let (r, g, b) = color_to_rgb_u8(color);
    Color::Rgb(
        ((r as f32) * k).round().clamp(0.0, 255.0) as u8,
        ((g as f32) * k).round().clamp(0.0, 255.0) as u8,
        ((b as f32) * k).round().clamp(0.0, 255.0) as u8,
    )
}

pub fn hsv_to_rgb_u8(h: f64, s: f64, v: f64) -> (u8, u8, u8) {
    let h = h.rem_euclid(1.0);
    let s = s.clamp(0.0, 1.0);
    let v = v.clamp(0.0, 1.0);
    let i = (h * 6.0).floor();
    let f = h * 6.0 - i;
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);
    let (r, g, b) = match (i as i64) % 6 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    (
        (r * 255.0).round() as u8,
        (g * 255.0).round() as u8,
        (b * 255.0).round() as u8,
    )
}

fn rgb_table(lua: &Lua, r: u8, g: u8, b: u8) -> mlua::Result<Table> {
    let tbl = lua.create_table()?;
    tbl.set("r", r)?;
    tbl.set("g", g)?;
    tbl.set("b", b)?;
    tbl.set(1, r)?;
    tbl.set(2, g)?;
    tbl.set(3, b)?;
    Ok(tbl)
}

pub fn color_to_rgb_u8(c: Color) -> (u8, u8, u8) {
    match c {
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Black => (0, 0, 0),
        Color::Red => (205, 49, 49),
        Color::Green => (13, 188, 121),
        Color::Yellow => (229, 229, 16),
        Color::Blue => (36, 114, 200),
        Color::Magenta => (188, 63, 188),
        Color::Cyan => (17, 168, 205),
        Color::Gray => (160, 160, 160),
        Color::DarkGray => (102, 102, 102),
        Color::LightRed => (241, 76, 76),
        Color::LightGreen => (35, 209, 139),
        Color::LightYellow => (245, 245, 67),
        Color::LightBlue => (59, 142, 234),
        Color::LightMagenta => (214, 112, 214),
        Color::LightCyan => (41, 184, 219),
        Color::White => (240, 240, 240),
        _ => (200, 200, 200),
    }
}

pub fn color_to_hex(c: Color) -> String {
    match c {
        Color::Reset => "transparent".to_string(),
        other => {
            let (r, g, b) = color_to_rgb_u8(other);
            format!("#{r:02X}{g:02X}{b:02X}")
        }
    }
}

fn hash_01(x: i64, y: i64, seed: i64) -> f64 {
    let mut n = (x as u64).wrapping_mul(374_761_393)
        ^ (y as u64).wrapping_mul(668_265_263)
        ^ (seed as u64).wrapping_mul(1_442_695_041);
    n = (n ^ (n >> 13)).wrapping_mul(1_274_126_177);
    n ^= n >> 16;
    (n & 0x00FF_FFFF) as f64 / 16_777_216.0
}

/// Smooth trilinear value noise in `[0.0, 1.0]`.
pub fn value_noise_3d(x: f64, y: f64, z: f64) -> f64 {
    let xi = x.floor() as i64;
    let yi = y.floor() as i64;
    let zi = z.floor() as i64;
    let xf = x - x.floor();
    let yf = y - y.floor();
    let zf = z - z.floor();

    let smooth = |t: f64| t * t * (3.0 - 2.0 * t);
    let u = smooth(xf);
    let v = smooth(yf);
    let w = smooth(zf);

    let corner = |dx: i64, dy: i64, dz: i64| -> f64 {
        let seed = (zi + dz).wrapping_mul(2_654_435_761);
        hash_01(xi + dx, yi + dy, seed)
    };

    let lerp = |a: f64, b: f64, t: f64| a + (b - a) * t;

    let x00 = lerp(corner(0, 0, 0), corner(1, 0, 0), u);
    let x10 = lerp(corner(0, 1, 0), corner(1, 1, 0), u);
    let x01 = lerp(corner(0, 0, 1), corner(1, 0, 1), u);
    let x11 = lerp(corner(0, 1, 1), corner(1, 1, 1), u);

    let y0 = lerp(x00, x10, v);
    let y1 = lerp(x01, x11, v);

    lerp(y0, y1, w)
}
