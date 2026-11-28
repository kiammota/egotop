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

/// Isolated, clipping, z-buffered cell canvas bound exclusively to a single rendering unit
/// (a container, the background layer, or an independent animation).
#[derive(Debug, Clone)]
pub struct CanvasBuffer {
    pub owner_name: String,
    pub owner_description: String,
    pub width: u16,
    pub height: u16,
    pub cell_aspect: f64,
    cells: Vec<Option<CanvasCell>>,
    zbuf: Vec<f32>,
}

impl CanvasBuffer {
    pub fn new(width: u16, height: u16) -> Self {
        Self::new_named("canvas", "", width, height)
    }

    pub fn new_named(
        owner_name: impl Into<String>,
        owner_description: impl Into<String>,
        width: u16,
        height: u16,
    ) -> Self {
        let len = (width as usize).saturating_mul(height as usize);
        Self {
            owner_name: owner_name.into(),
            owner_description: owner_description.into(),
            width,
            height,
            cell_aspect: 0.5,
            cells: vec![None; len],
            zbuf: vec![f32::NEG_INFINITY; len],
        }
    }

    pub fn set_identity(&mut self, name: impl Into<String>, description: impl Into<String>) {
        self.owner_name = name.into();
        self.owner_description = description.into();
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

    /// Writes a cell at local coordinates `(x, y)` with strict automatic clipping to `[0..width, 0..height)`.
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

    /// Draws multi-line ASCII art anchored at local `(x, y)` with strict clipping.
    pub fn ascii(
        &mut self,
        x: i32,
        y: i32,
        art: &str,
        fg: Option<Color>,
        bg: Option<Color>,
        intensity: f32,
    ) {
        if intensity <= 0.0 {
            return;
        }
        for (row_offset, line) in art.lines().enumerate() {
            let py = y + row_offset as i32;
            if py < 0 {
                continue;
            }
            if py >= self.height as i32 {
                break;
            }
            self.text(x, py, line, fg, bg, intensity);
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

    /// Blits cells strictly within `area ∩ buf.area()` so this canvas can never touch external cells.
    pub fn blit_to_buffer(&self, buf: &mut Buffer, area: Rect, fallback_fg: Color) {
        let clipped = area.intersection(*buf.area());
        if clipped.width == 0 || clipped.height == 0 {
            return;
        }
        let max_w = self.width.min(clipped.width);
        let max_h = self.height.min(clipped.height);
        for y in 0..max_h {
            for x in 0..max_w {
                let Some(cell) = self.get_cell(x, y) else {
                    continue;
                };
                if let Some(dst) = buf.cell_mut((clipped.x + x, clipped.y + y)) {
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

fn detail_for(width: u16, height: u16) -> f64 {
    ((width as f64 * height as f64 / 2400.0).sqrt()).clamp(0.0, 1.0)
}

fn raster_line(
    buffer: &mut CanvasBuffer,
    mut x0: i32,
    mut y0: i32,
    x1: i32,
    y1: i32,
    ch: &str,
    fg: Option<Color>,
    bg: Option<Color>,
) {
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        buffer.put(x0, y0, ch, fg, bg, 1.0);
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

fn raster_polygon(
    buffer: &mut CanvasBuffer,
    points: &[(f64, f64)],
    fg: Option<Color>,
    bg: Option<Color>,
) {
    if points.len() < 3 || buffer.width == 0 || buffer.height == 0 {
        return;
    }
    let pts: Vec<(f64, f64)> = points
        .iter()
        .map(|(u, v)| {
            (
                u.clamp(0.0, 1.0) * (buffer.width - 1) as f64,
                v.clamp(0.0, 1.0) * (buffer.height - 1) as f64,
            )
        })
        .collect();
    let min_y = pts
        .iter()
        .map(|p| p.1.floor() as i32)
        .min()
        .unwrap_or(0)
        .max(0);
    let max_y = pts
        .iter()
        .map(|p| p.1.ceil() as i32)
        .max()
        .unwrap_or(0)
        .min(buffer.height as i32 - 1);
    for y in min_y..=max_y {
        let mut xs = Vec::new();
        for i in 0..pts.len() {
            let (x1, y1) = pts[i];
            let (x2, y2) = pts[(i + 1) % pts.len()];
            if (y1 <= y as f64 && y2 > y as f64) || (y2 <= y as f64 && y1 > y as f64) {
                xs.push(x1 + (y as f64 - y1) * (x2 - x1) / (y2 - y1));
            }
        }
        xs.sort_by(|a, b| a.total_cmp(b));
        for pair in xs.chunks_exact(2) {
            for x in pair[0].ceil() as i32..=pair[1].floor() as i32 {
                buffer.put(x, y, "█", fg, bg, 1.0);
            }
        }
    }
}

/// Isolated Lua UserData handle bound explicitly to ONE unit's `CanvasBuffer`.
/// Exposes only local dimensions, unit identity (`name`, `description`), and local clipped drawing.
#[derive(Clone)]
pub struct LuaCanvasHandle {
    buffer: Arc<Mutex<CanvasBuffer>>,
    theme: Theme,
}

#[cfg(test)]
mod responsive_canvas_tests {
    use super::*;

    #[test]
    fn normalized_geometry_subpixels_shading_and_sampling_rasterize_in_bounds() {
        let lua = Lua::new();
        let buffer = Arc::new(Mutex::new(CanvasBuffer::new(20, 10)));
        let theme = Theme::from_config(&crate::config::ThemeConfig::default());
        lua.globals()
            .set("canvas", LuaCanvasHandle::new(buffer.clone(), theme))
            .unwrap();
        lua.load(r##"
            assert(canvas.width == 20 and canvas.height == 10)
            assert(math.abs(canvas.aspect - 1.0) < 0.001)
            assert(canvas.detail > 0 and canvas.detail < 1)
            canvas:set_cell_aspect(0.4)
            assert(math.abs(canvas.aspect - 0.8) < 0.001)
            canvas:line_uv(0, 0, 1, 1, "#FFFFFF")
            canvas:dot_uv(0.5, 0.5, "#FF0000")
            canvas:shade(2, 2, 0.8, "#00FF00")
            canvas:sample(function(u, v, t) return (u > 0.45 and u < 0.55 and v > 0.45 and v < 0.55) and "X" or " " end, 1.25)
        "##).exec().unwrap();
        let guard = buffer.lock().unwrap();
        assert!(guard.get_cell(0, 0).is_some());
        assert!(guard.get_cell(10, 5).is_some());
        assert!(guard.cells.iter().flatten().count() <= 200);
    }

    #[test]
    fn explicit_canvas_script_types_resolve_without_cross_slot_leaks() {
        assert!(crate::lua::list_scripts_for_slot(crate::animation::AnimSlotKind::Canvas)
            .iter().any(|name| name == "delorean"));
        assert!(!crate::lua::list_scripts_for_slot(crate::animation::AnimSlotKind::Background)
            .iter().any(|name| name == "delorean"));
        assert!(crate::lua::validate_script_for_slot("delorean", crate::animation::AnimSlotKind::Canvas).is_ok());
        assert!(crate::lua::validate_script_for_slot("delorean", crate::animation::AnimSlotKind::Background).is_err());
    }

    #[test]
    fn delorean_canvas_resizes_from_small_to_large_without_runtime_errors() {
        let theme = Theme::from_config(&crate::config::ThemeConfig::default());
        let mut scene = crate::lua::LuaCanvasInstance::from_script_name_named(
            "logo",
            "Test canvas",
            "delorean",
            8,
            5,
            24,
            1.0,
            1.0,
            theme.clone(),
        );
        scene.update(1.0 / 24.0, 1.0);
        assert!(
            scene.last_error().is_none(),
            "small render: {:?}",
            scene.last_error()
        );
        let mut small = Buffer::empty(Rect::new(0, 0, 8, 5));
        scene.render(&mut small, Rect::new(0, 0, 8, 5), &theme);
        assert!(small.content.iter().any(|cell| cell.symbol() != " "));
        scene.resize(100, 40);
        scene.update(1.0 / 24.0, 1.0);
        assert!(
            scene.last_error().is_none(),
            "large render: {:?}",
            scene.last_error()
        );
        let mut buffer = Buffer::empty(Rect::new(0, 0, 100, 40));
        scene.render(&mut buffer, Rect::new(0, 0, 100, 40), &theme);
        assert!(buffer.content.iter().any(|cell| cell.symbol() != " "));

        let mut tree = crate::lua::LuaCanvasInstance::from_script_name_named(
            "system",
            "Tree canvas",
            "christmas_tree",
            8,
            5,
            24,
            1.0,
            1.0,
            theme.clone(),
        );
        tree.update(1.0 / 24.0, 1.0);
        assert!(
            tree.last_error().is_none(),
            "small tree: {:?}",
            tree.last_error()
        );
        tree.resize(100, 40);
        tree.update(1.0 / 24.0, 1.0);
        assert!(
            tree.last_error().is_none(),
            "resized tree: {:?}",
            tree.last_error()
        );
    }
}

impl LuaCanvasHandle {
    pub fn new(buffer: Arc<Mutex<CanvasBuffer>>, theme: Theme) -> Self {
        Self { buffer, theme }
    }
}

impl UserData for LuaCanvasHandle {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("name", |_, this| {
            Ok(this.buffer.lock().unwrap().owner_name.clone())
        });
        fields.add_field_method_get("description", |_, this| {
            Ok(this.buffer.lock().unwrap().owner_description.clone())
        });
        fields.add_field_method_get("width", |_, this| Ok(this.buffer.lock().unwrap().width));
        fields.add_field_method_get("height", |_, this| Ok(this.buffer.lock().unwrap().height));
        fields.add_field_method_get("aspect", |_, this| {
            let b = this.buffer.lock().unwrap();
            Ok(if b.height == 0 {
                1.0
            } else {
                b.width as f64 * b.cell_aspect / b.height as f64
            })
        });
        fields.add_field_method_get("cell_aspect", |_, this| {
            Ok(this.buffer.lock().unwrap().cell_aspect)
        });
        fields.add_field_method_get("detail", |_, this| {
            let b = this.buffer.lock().unwrap();
            Ok(detail_for(b.width, b.height))
        });
    }

    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("set_cell_aspect", |_, this, aspect: f64| {
            if aspect.is_finite() && (0.2..=1.0).contains(&aspect) {
                this.buffer.lock().unwrap().cell_aspect = aspect;
            }
            Ok(())
        });
        methods.add_method("uv", |_, this, (u, v): (f64, f64)| {
            let b = this.buffer.lock().unwrap();
            Ok((
                u.clamp(0.0, 1.0) * b.width.saturating_sub(1) as f64,
                v.clamp(0.0, 1.0) * b.height.saturating_sub(1) as f64,
            ))
        });
        methods.add_method("line_uv", |_, this, (u1, v1, u2, v2, fg, bg): (f64, f64, f64, f64, Option<Value>, Option<Value>)| {
            let fg = fg.as_ref().and_then(|v| parse_lua_color(v, true)).or(Some(this.theme.text));
            let bg = bg.as_ref().and_then(|v| parse_lua_color(v, false));
            let mut b = this.buffer.lock().unwrap();
            let x1 = (u1.clamp(0.0,1.0) * b.width.saturating_sub(1) as f64).round() as i32;
            let y1 = (v1.clamp(0.0,1.0) * b.height.saturating_sub(1) as f64).round() as i32;
            let x2 = (u2.clamp(0.0,1.0) * b.width.saturating_sub(1) as f64).round() as i32;
            let y2 = (v2.clamp(0.0,1.0) * b.height.saturating_sub(1) as f64).round() as i32;
            raster_line(&mut b, x1, y1, x2, y2, "•", fg, bg);
            Ok(())
        });
        methods.add_method(
            "rect_uv",
            |_,
             this,
             (u, v, w, h, fg, bg, fill): (
                f64,
                f64,
                f64,
                f64,
                Option<Value>,
                Option<Value>,
                Option<bool>,
            )| {
                let fg = fg
                    .as_ref()
                    .and_then(|v| parse_lua_color(v, true))
                    .or(Some(this.theme.text));
                let bg = bg.as_ref().and_then(|v| parse_lua_color(v, false));
                let mut b = this.buffer.lock().unwrap();
                let x0 = (u.clamp(0.0, 1.0) * b.width as f64).floor() as i32;
                let y0 = (v.clamp(0.0, 1.0) * b.height as f64).floor() as i32;
                let x1 = ((u + w).clamp(0.0, 1.0) * b.width as f64).ceil() as i32 - 1;
                let y1 = ((v + h).clamp(0.0, 1.0) * b.height as f64).ceil() as i32 - 1;
                for y in y0..=y1 {
                    for x in x0..=x1 {
                        if fill.unwrap_or(false) || x == x0 || x == x1 || y == y0 || y == y1 {
                            b.put(
                                x,
                                y,
                                if fill.unwrap_or(false) { "█" } else { "#" },
                                fg,
                                bg,
                                1.0,
                            );
                        }
                    }
                }
                Ok(())
            },
        );
        methods.add_method(
            "circle_uv",
            |_, this, (u, v, r, fg, bg): (f64, f64, f64, Option<Value>, Option<Value>)| {
                let fg = fg
                    .as_ref()
                    .and_then(|v| parse_lua_color(v, true))
                    .or(Some(this.theme.text));
                let bg = bg.as_ref().and_then(|v| parse_lua_color(v, false));
                let mut b = this.buffer.lock().unwrap();
                if b.width == 0 || b.height == 0 {
                    return Ok(());
                }
                let aspect = b.width as f64 * b.cell_aspect / b.height as f64;
                let ru = if aspect > 0.0 { r / aspect } else { r };
                let steps = ((r.max(0.0) * b.height as f64 * std::f64::consts::TAU).ceil()
                    as usize)
                    .clamp(12, 2048);
                for i in 0..steps {
                    let a = i as f64 / steps as f64 * std::f64::consts::TAU;
                    let x = ((u + ru * a.cos()) * b.width as f64).round() as i32;
                    let y = ((v + r * a.sin()) * b.height as f64).round() as i32;
                    b.put(x, y, "●", fg, bg, 1.0);
                }
                Ok(())
            },
        );
        methods.add_method("poly_uv", |_, this, (points, fg, bg, fill): (Table, Option<Value>, Option<Value>, Option<bool>)| {
            let pts: Vec<(f64,f64)> = points.sequence_values::<Table>().filter_map(Result::ok).filter_map(|p| Some((p.get(1).ok()?,p.get(2).ok()?))).collect();
            let fg=fg.as_ref().and_then(|v|parse_lua_color(v,true)).or(Some(this.theme.text)); let bg=bg.as_ref().and_then(|v|parse_lua_color(v,false)); let mut b=this.buffer.lock().unwrap();
            let (bw,bh)=(b.width,b.height);
            if fill.unwrap_or(false) { raster_polygon(&mut b,&pts,fg,bg); }
            if pts.len()>=2 { for i in 0..pts.len() {let (u1,v1)=pts[i]; let (u2,v2)=pts[(i+1)%pts.len()]; raster_line(&mut b,(u1.clamp(0.0,1.0)*bw.saturating_sub(1) as f64).round() as i32,(v1.clamp(0.0,1.0)*bh.saturating_sub(1) as f64).round() as i32,(u2.clamp(0.0,1.0)*bw.saturating_sub(1) as f64).round() as i32,(v2.clamp(0.0,1.0)*bh.saturating_sub(1) as f64).round() as i32,"#",fg,bg);} }
            Ok(())
        });
        methods.add_method(
            "tri_uv",
            |_, this, (points, fg, bg, fill): (Table, Option<Value>, Option<Value>, Option<bool>)| {
                let pts: Vec<(f64, f64)> = points
                    .sequence_values::<Table>()
                    .filter_map(Result::ok)
                    .filter_map(|p| Some((p.get(1).ok()?, p.get(2).ok()?)))
                    .collect();
                if pts.len() == 3 {
                    let fg = fg
                        .as_ref()
                        .and_then(|v| parse_lua_color(v, true))
                        .or(Some(this.theme.text));
                    let bg = bg.as_ref().and_then(|v| parse_lua_color(v, false));
                    let mut b = this.buffer.lock().unwrap();
                    let (bw, bh) = (b.width, b.height);
                    if fill.unwrap_or(false) { raster_polygon(&mut b,&pts,fg,bg); }
                    for i in 0..3 {
                        let (u1, v1) = pts[i];
                        let (u2, v2) = pts[(i + 1) % 3];
                        raster_line(
                            &mut b,
                            (u1.clamp(0.0,1.0) * bw.saturating_sub(1) as f64).round() as i32,
                            (v1.clamp(0.0,1.0) * bh.saturating_sub(1) as f64).round() as i32,
                            (u2.clamp(0.0,1.0) * bw.saturating_sub(1) as f64).round() as i32,
                            (v2.clamp(0.0,1.0) * bh.saturating_sub(1) as f64).round() as i32,
                            "#",
                            fg,
                            bg,
                        );
                    }
                }
                Ok(())
            },
        );
        methods.add_method(
            "dot_uv",
            |_, this, (u, v, fg): (f64, f64, Option<Value>)| {
                let mut b = this.buffer.lock().unwrap();
                if b.width == 0 || b.height == 0 {
                    return Ok(());
                }
                let sx = (u.clamp(0.0, 1.0) * b.width as f64 * 2.0).floor() as i32;
                let sy = (v.clamp(0.0, 1.0) * b.height as f64 * 4.0).floor() as i32;
                let x = (sx / 2).min(b.width as i32 - 1);
                let y = (sy / 4).min(b.height as i32 - 1);
                let bit = match (sx % 2, sy % 4) {
                    (0, 0) => 0,
                    (0, 1) => 1,
                    (0, 2) => 2,
                    (0, 3) => 6,
                    (1, 0) => 3,
                    (1, 1) => 4,
                    (1, 2) => 5,
                    _ => 7,
                };
                let idx = y as usize * b.width as usize + x as usize;
                let old = b.cells[idx]
                    .as_ref()
                    .map(|c| c.ch.chars().next().unwrap_or('\u{2800}') as u32)
                    .unwrap_or(0x2800);
                let glyph = char::from_u32(0x2800 | ((old.saturating_sub(0x2800)) | (1 << bit)))
                    .unwrap_or('⣿')
                    .to_string();
                let col = fg
                    .as_ref()
                    .and_then(|v| parse_lua_color(v, true))
                    .or(Some(this.theme.text));
                b.put(x, y, &glyph, col, None, 1.0);
                Ok(())
            },
        );
        methods.add_method(
            "shade",
            |_, this, (x, y, luminance, fg, bg): (i32, i32, f64, Option<Value>, Option<Value>)| {
                const RAMP: &str =
                    " .'`^\",:;Il!i><~+_-?][}{1)(|\\/tfjrxnuvczXYUJCLQ0OZmwqpdbkhao*#MW&8%B@$";
                const SMALL: &str = " .:-=+*#%@";
                let b = this.buffer.lock().unwrap();
                let ramp = if detail_for(b.width, b.height) < 0.35 {
                    SMALL
                } else {
                    RAMP
                };
                drop(b);
                let idx = (luminance.clamp(0.0, 1.0) * (ramp.chars().count() - 1) as f64).round()
                    as usize;
                let ch = ramp.chars().nth(idx).unwrap_or(' ');
                let col = fg
                    .as_ref()
                    .and_then(|v| parse_lua_color(v, true))
                    .or(Some(this.theme.text));
                this.buffer.lock().unwrap().put(
                    x,
                    y,
                    &ch.to_string(),
                    col,
                    bg.as_ref().and_then(|v| parse_lua_color(v, false)),
                    1.0,
                );
                Ok(())
            },
        );
        methods.add_method(
            "sample",
            |_, this, (func, time): (Function, Option<f64>)| {
                let (w, h) = {
                    let b = this.buffer.lock().unwrap();
                    (b.width, b.height)
                };
                let mut masks = vec![0u8; w as usize * h as usize];
                let mut glyphs = vec![None::<String>; masks.len()];
                let bits = [[0u8, 1, 2, 6], [3, 4, 5, 7]];
                for y in 0..h {
                    for x in 0..w {
                        for sy in 0..4 {
                            for sx in 0..2 {
                                let u = (x as f64 + (sx as f64 + 0.5) / 2.0) / w.max(1) as f64;
                                let v = (y as f64 + (sy as f64 + 0.5) / 4.0) / h.max(1) as f64;
                                let value = func.call::<Value>((u, v, time.unwrap_or(0.0)));
                                match value {
                                    Ok(Value::Boolean(true)) => {
                                        masks[y as usize * w as usize + x as usize] |=
                                            1 << bits[sx][sy]
                                    }
                                    Ok(Value::Integer(n)) if n > 0 => {
                                        masks[y as usize * w as usize + x as usize] |=
                                            1 << bits[sx][sy]
                                    }
                                    Ok(Value::Number(n)) if n >= 0.5 => {
                                        masks[y as usize * w as usize + x as usize] |=
                                            1 << bits[sx][sy]
                                    }
                                    Ok(Value::String(s)) => {
                                        glyphs[y as usize * w as usize + x as usize] =
                                            Some(s.to_string_lossy().to_string())
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
                let mut b = this.buffer.lock().unwrap();
                for y in 0..h {
                    for x in 0..w {
                        let i = y as usize * w as usize + x as usize;
                        if let Some(glyph) = glyphs[i].as_deref() {
                            b.put(x as i32, y as i32, glyph, Some(this.theme.text), None, 1.0);
                        } else if masks[i] != 0 {
                            let ch = char::from_u32(0x2800 + masks[i] as u32)
                                .unwrap_or('⣿')
                                .to_string();
                            b.put(x as i32, y as i32, &ch, Some(this.theme.text), None, 1.0);
                        }
                    }
                }
                Ok(())
            },
        );
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

        methods.add_method(
            "ascii",
            |_,
             this,
             (x, y, art, fg, bg, intensity): (
                f64,
                f64,
                Value,
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
                let text = match art {
                    Value::String(s) => s.to_str().map(|x| x.to_string()).unwrap_or_default(),
                    Value::Table(tbl) => tbl
                        .sequence_values::<String>()
                        .flatten()
                        .collect::<Vec<_>>()
                        .join("\n"),
                    _ => String::new(),
                };
                this.buffer.lock().unwrap().ascii(
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
            let r = tbl.get::<f64>("r").or_else(|_| tbl.get::<f64>(1)).ok()?;
            let g = tbl.get::<f64>("g").or_else(|_| tbl.get::<f64>(2)).ok()?;
            let b = tbl.get::<f64>("b").or_else(|_| tbl.get::<f64>(3)).ok()?;
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
