use crate::ui::theme::{adapt_color, blend_color};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use serde::{Deserialize, Serialize};
use std::f32::consts::{PI, TAU};

/// Fixed resampled vertex count used so any closed or open contour can morph cleanly.
pub const MORPH_SAMPLE_COUNT: usize = 64;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ShapePrimitive {
    #[default]
    Circle,
    Square,
    Rectangle,
    Triangle,
    Polygon,
    Line,
    Path,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum GeometryRenderer {
    #[default]
    Braille,
    Block,
    Quadrant,
    Ascii,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShapeSpec {
    pub primitive: ShapePrimitive,
    pub sides: u16,
    pub custom_points: Vec<(f32, f32)>,
    pub closed: bool,
}

impl Default for ShapeSpec {
    fn default() -> Self {
        Self {
            primitive: ShapePrimitive::Circle,
            sides: 6,
            custom_points: Vec::new(),
            closed: true,
        }
    }
}

impl ShapeSpec {
    pub fn from_primitive(primitive: ShapePrimitive, sides: u16, path: Vec<(f32, f32)>) -> Self {
        let closed = !(matches!(primitive, ShapePrimitive::Line)
            || (primitive == ShapePrimitive::Path && path.len() < 3));
        Self {
            primitive,
            sides: sides.max(3),
            custom_points: path,
            closed,
        }
    }

    /// Generates an ordered, arc-length-normalized polyline of `MORPH_SAMPLE_COUNT` points
    /// inside normalized object space `[-1.0, 1.0] x [-1.0, 1.0]`.
    pub fn normalized_contour(&self) -> Vec<(f32, f32)> {
        let mut raw = match self.primitive {
            ShapePrimitive::Circle => {
                let mut pts = Vec::with_capacity(MORPH_SAMPLE_COUNT);
                for i in 0..MORPH_SAMPLE_COUNT {
                    let theta = -PI * 0.5 + (i as f32 / MORPH_SAMPLE_COUNT as f32) * TAU;
                    pts.push((theta.cos(), theta.sin()));
                }
                return pts;
            }
            ShapePrimitive::Square => vec![
                (0.0, -0.82),
                (0.82, -0.82),
                (0.82, 0.82),
                (-0.82, 0.82),
                (-0.82, -0.82),
            ],
            ShapePrimitive::Rectangle => vec![
                (0.0, -0.62),
                (1.0, -0.62),
                (1.0, 0.62),
                (-1.0, 0.62),
                (-1.0, -0.62),
            ],
            ShapePrimitive::Triangle => regular_polygon_vertices(3, -PI * 0.5),
            ShapePrimitive::Polygon => {
                let n = self.sides.clamp(3, 32) as usize;
                regular_polygon_vertices(n, -PI * 0.5)
            }
            ShapePrimitive::Line => {
                return resample_open_polyline(&[(-1.0, 0.0), (1.0, 0.0)], MORPH_SAMPLE_COUNT);
            }
            ShapePrimitive::Path => {
                if self.custom_points.len() < 2 {
                    return Self::from_primitive(ShapePrimitive::Circle, 6, Vec::new())
                        .normalized_contour();
                }
                if !self.closed {
                    return resample_open_polyline(&self.custom_points, MORPH_SAMPLE_COUNT);
                }
                self.custom_points.clone()
            }
        };

        if raw.len() < 2 {
            raw = vec![(0.0, -1.0), (1.0, 1.0), (-1.0, 1.0)];
        }
        resample_closed_polygon(&raw, MORPH_SAMPLE_COUNT)
    }
}

fn regular_polygon_vertices(sides: usize, start_angle: f32) -> Vec<(f32, f32)> {
    (0..sides)
        .map(|i| {
            let theta = start_angle + (i as f32 / sides as f32) * TAU;
            (theta.cos(), theta.sin())
        })
        .collect()
}

/// Resamples a closed polygon uniformly by arc length around its perimeter.
pub fn resample_closed_polygon(vertices: &[(f32, f32)], count: usize) -> Vec<(f32, f32)> {
    if vertices.is_empty() || count == 0 {
        return Vec::new();
    }
    let n = vertices.len();
    let mut seg_lengths = Vec::with_capacity(n);
    let mut total_len = 0.0f32;
    for i in 0..n {
        let a = vertices[i];
        let b = vertices[(i + 1) % n];
        let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
        seg_lengths.push(len);
        total_len += len;
    }
    if total_len <= 1e-6 {
        return vec![vertices[0]; count];
    }

    let mut out = Vec::with_capacity(count);
    let step = total_len / count as f32;
    let mut seg_idx = 0usize;
    let mut accum = 0.0f32;

    for i in 0..count {
        let target = i as f32 * step;
        while seg_idx + 1 < n && accum + seg_lengths[seg_idx] < target {
            accum += seg_lengths[seg_idx];
            seg_idx += 1;
        }
        let seg_len = seg_lengths[seg_idx].max(1e-6);
        let local_t = ((target - accum) / seg_len).clamp(0.0, 1.0);
        let a = vertices[seg_idx];
        let b = vertices[(seg_idx + 1) % n];
        out.push((a.0 + (b.0 - a.0) * local_t, a.1 + (b.1 - a.1) * local_t));
    }
    out
}

/// Resamples an open polyline uniformly by arc length.
pub fn resample_open_polyline(points: &[(f32, f32)], count: usize) -> Vec<(f32, f32)> {
    if points.is_empty() || count == 0 {
        return Vec::new();
    }
    if points.len() == 1 || count == 1 {
        return vec![points[0]; count];
    }
    let mut seg_lengths = Vec::with_capacity(points.len() - 1);
    let mut total_len = 0.0f32;
    for pair in points.windows(2) {
        let len = ((pair[1].0 - pair[0].0).powi(2) + (pair[1].1 - pair[0].1).powi(2)).sqrt();
        seg_lengths.push(len);
        total_len += len;
    }
    if total_len <= 1e-6 {
        return vec![points[0]; count];
    }

    let mut out = Vec::with_capacity(count);
    let step = total_len / (count - 1) as f32;
    let mut seg_idx = 0usize;
    let mut accum = 0.0f32;

    for i in 0..count {
        let target = i as f32 * step;
        while seg_idx + 1 < seg_lengths.len() && accum + seg_lengths[seg_idx] < target {
            accum += seg_lengths[seg_idx];
            seg_idx += 1;
        }
        let seg_len = seg_lengths[seg_idx].max(1e-6);
        let local_t = ((target - accum) / seg_len).clamp(0.0, 1.0);
        let a = points[seg_idx];
        let b = points[seg_idx + 1];
        out.push((a.0 + (b.0 - a.0) * local_t, a.1 + (b.1 - a.1) * local_t));
    }
    out
}

/// Smoothly morphs two normalized contours by linear interpolation between aligned samples.
pub fn morph_contours(from: &[(f32, f32)], to: &[(f32, f32)], t: f32) -> Vec<(f32, f32)> {
    let t = t.clamp(0.0, 1.0);
    if from.is_empty() {
        return to.to_vec();
    }
    if to.is_empty() {
        return from.to_vec();
    }
    let len = from.len().min(to.len());
    let mut out = Vec::with_capacity(len);
    for i in 0..len {
        let a = from[i];
        let b = to[i];
        out.push((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t));
    }
    out
}

/// Parameters controlling rasterization of a contour onto a terminal region.
#[derive(Debug, Clone)]
pub struct ShapeDrawParams {
    pub center_x: f32,
    pub center_y: f32,
    pub half_w: f32,
    pub half_h: f32,
    pub rotation_deg: f32,
    pub scale: f32,
    pub stroke: bool,
    pub fill: bool,
    pub closed: bool,
    pub stroke_color: Color,
    pub fill_color: Color,
    pub opacity: f32,
    pub renderer: GeometryRenderer,
    pub stroke_char: char,
    pub fill_char: char,
    pub truecolor: bool,
}

/// Rasterizes a normalized contour (`[-1, 1]`) into a Ratatui buffer inside `clip`.
pub fn rasterize_contour(
    contour: &[(f32, f32)],
    params: &ShapeDrawParams,
    clip: Rect,
    buf: &mut Buffer,
) {
    if contour.is_empty()
        || clip.width == 0
        || clip.height == 0
        || params.opacity <= 0.01
        || (!params.stroke && !params.fill)
    {
        return;
    }

    let (sub_x, sub_y) = match params.renderer {
        GeometryRenderer::Braille => (2usize, 4usize),
        GeometryRenderer::Quadrant | GeometryRenderer::Block => (2usize, 2usize),
        GeometryRenderer::Ascii => (1usize, 1usize),
    };

    let pw = (clip.width as usize) * sub_x;
    let ph = (clip.height as usize) * sub_y;
    if pw == 0 || ph == 0 {
        return;
    }

    let rad = params.rotation_deg.to_radians();
    let (sin_r, cos_r) = rad.sin_cos();
    let scale = params.scale.max(0.05);

    let transformed: Vec<(f32, f32)> = contour
        .iter()
        .map(|&(nx, ny)| {
            let sx = nx * scale;
            let sy = ny * scale;
            let rx = sx * cos_r - sy * sin_r;
            let ry = sx * sin_r + sy * cos_r;
            let cell_x = params.center_x + rx * params.half_w;
            let cell_y = params.center_y + ry * params.half_h;
            (
                (cell_x - clip.x as f32) * sub_x as f32,
                (cell_y - clip.y as f32) * sub_y as f32,
            )
        })
        .collect();

    let pixel_count = pw * ph;
    let mut stroke_mask = vec![false; pixel_count];
    let mut fill_mask = vec![false; pixel_count];

    if params.fill && params.closed && transformed.len() >= 3 {
        fill_polygon(&transformed, pw, ph, &mut fill_mask);
    }

    if params.stroke {
        let segs = if params.closed {
            transformed.len()
        } else {
            transformed.len().saturating_sub(1)
        };
        for i in 0..segs {
            let a = transformed[i];
            let b = transformed[(i + 1) % transformed.len()];
            draw_subpixel_line(a, b, pw, ph, &mut stroke_mask);
        }
    }

    match params.renderer {
        GeometryRenderer::Braille => {
            render_braille_cells(&stroke_mask, &fill_mask, params, clip, buf);
        }
        GeometryRenderer::Quadrant | GeometryRenderer::Block => {
            render_quadrant_cells(&stroke_mask, &fill_mask, params, clip, buf);
        }
        GeometryRenderer::Ascii => {
            render_ascii_cells(&stroke_mask, &fill_mask, params, clip, buf);
        }
    }
}

fn draw_subpixel_line(
    a: (f32, f32),
    b: (f32, f32),
    pw: usize,
    ph: usize,
    mask: &mut [bool],
) {
    let mut x0 = a.0.round() as i32;
    let mut y0 = a.1.round() as i32;
    let x1 = b.0.round() as i32;
    let y1 = b.1.round() as i32;

    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    let max_steps = (dx.abs() + dy.abs() + 2).min(4096);
    for _ in 0..max_steps {
        if x0 >= 0 && y0 >= 0 && (x0 as usize) < pw && (y0 as usize) < ph {
            mask[y0 as usize * pw + x0 as usize] = true;
        }
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

fn fill_polygon(poly: &[(f32, f32)], pw: usize, ph: usize, mask: &mut [bool]) {
    let mut min_y = ph as i32;
    let mut max_y = -1i32;
    for &(_, py) in poly {
        let yi = py.floor() as i32;
        min_y = min_y.min(yi);
        max_y = max_y.max(yi + 1);
    }
    let y_start = min_y.max(0) as usize;
    let y_end = max_y.clamp(0, ph as i32) as usize;

    let mut nodes = Vec::with_capacity(poly.len());
    for py in y_start..y_end {
        let scan_y = py as f32 + 0.5;
        nodes.clear();
        for i in 0..poly.len() {
            let p1 = poly[i];
            let p2 = poly[(i + 1) % poly.len()];
            if (p1.1 < scan_y && p2.1 >= scan_y) || (p2.1 < scan_y && p1.1 >= scan_y) {
                let x = p1.0 + (scan_y - p1.1) / (p2.1 - p1.1) * (p2.0 - p1.0);
                nodes.push(x);
            }
        }
        nodes.sort_by(|a, b| a.total_cmp(b));
        for pair in nodes.chunks_exact(2) {
            let x0 = pair[0].ceil().max(0.0) as usize;
            let x1 = (pair[1].floor() as i32).min(pw as i32 - 1);
            if x1 < 0 {
                continue;
            }
            for px in x0..=(x1 as usize) {
                mask[py * pw + px] = true;
            }
        }
    }
}

fn render_braille_cells(
    stroke: &[bool],
    fill: &[bool],
    params: &ShapeDrawParams,
    clip: Rect,
    buf: &mut Buffer,
) {
    // Braille dot bit mapping per (dx in 0..2, dy in 0..4)
    const DOTS: [[u8; 4]; 2] = [[0x01, 0x02, 0x04, 0x40], [0x08, 0x10, 0x20, 0x80]];
    let pw = (clip.width as usize) * 2;
    let alpha = params.opacity.clamp(0.0, 1.0);

    for cy in 0..(clip.height as usize) {
        for cx in 0..(clip.width as usize) {
            let mut bits = 0u8;
            let mut has_stroke = false;
            let mut has_fill = false;

            for (dx, dot_col) in DOTS.iter().enumerate() {
                for (dy, &dot_bit) in dot_col.iter().enumerate() {
                    let idx = (cy * 4 + dy) * pw + (cx * 2 + dx);
                    let s = stroke[idx];
                    let f = fill[idx];
                    if s || f {
                        bits |= dot_bit;
                        has_stroke |= s;
                        has_fill |= f;
                    }
                }
            }

            if bits == 0 {
                continue;
            }
            let ch = char::from_u32(0x2800 + bits as u32).unwrap_or('⠿');
            let base_color = if has_stroke || !has_fill {
                params.stroke_color
            } else {
                params.fill_color
            };
            plot_tinted_char(
                buf,
                clip.x + cx as u16,
                clip.y + cy as u16,
                ch,
                base_color,
                alpha,
                params.truecolor,
            );
        }
    }
}

fn render_quadrant_cells(
    stroke: &[bool],
    fill: &[bool],
    params: &ShapeDrawParams,
    clip: Rect,
    buf: &mut Buffer,
) {
    const QUADRANTS: [char; 16] = [
        ' ', '▘', '▝', '▀', '▖', '▌', '▞', '▛', '▗', '▚', '▐', '▜', '▄', '▙', '▟', '█',
    ];
    let pw = (clip.width as usize) * 2;
    let alpha = params.opacity.clamp(0.0, 1.0);

    for cy in 0..(clip.height as usize) {
        for cx in 0..(clip.width as usize) {
            let tl = stroke[(cy * 2) * pw + cx * 2] || fill[(cy * 2) * pw + cx * 2];
            let tr = stroke[(cy * 2) * pw + cx * 2 + 1] || fill[(cy * 2) * pw + cx * 2 + 1];
            let bl = stroke[(cy * 2 + 1) * pw + cx * 2] || fill[(cy * 2 + 1) * pw + cx * 2];
            let br = stroke[(cy * 2 + 1) * pw + cx * 2 + 1] || fill[(cy * 2 + 1) * pw + cx * 2 + 1];
            let idx = (usize::from(tl))
                | (usize::from(tr) << 1)
                | (usize::from(bl) << 2)
                | (usize::from(br) << 3);
            if idx == 0 {
                continue;
            }
            let has_stroke = stroke[(cy * 2) * pw + cx * 2]
                || stroke[(cy * 2) * pw + cx * 2 + 1]
                || stroke[(cy * 2 + 1) * pw + cx * 2]
                || stroke[(cy * 2 + 1) * pw + cx * 2 + 1];
            let ch = QUADRANTS[idx];
            let base_color = if has_stroke {
                params.stroke_color
            } else {
                params.fill_color
            };
            plot_tinted_char(
                buf,
                clip.x + cx as u16,
                clip.y + cy as u16,
                ch,
                base_color,
                alpha,
                params.truecolor,
            );
        }
    }
}

fn render_ascii_cells(
    stroke: &[bool],
    fill: &[bool],
    params: &ShapeDrawParams,
    clip: Rect,
    buf: &mut Buffer,
) {
    let pw = clip.width as usize;
    let alpha = params.opacity.clamp(0.0, 1.0);

    for cy in 0..(clip.height as usize) {
        for cx in 0..pw {
            let idx = cy * pw + cx;
            let (ch, color) = if stroke[idx] {
                (params.stroke_char, params.stroke_color)
            } else if fill[idx] {
                (params.fill_char, params.fill_color)
            } else {
                continue;
            };
            plot_tinted_char(
                buf,
                clip.x + cx as u16,
                clip.y + cy as u16,
                ch,
                color,
                alpha,
                params.truecolor,
            );
        }
    }
}

fn plot_tinted_char(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    ch: char,
    fg: Color,
    alpha: f32,
    truecolor: bool,
) {
    if let Some(cell) = buf.cell_mut((x, y)) {
        let final_fg = if alpha >= 0.99 {
            fg
        } else {
            blend_color(fg, cell.bg, alpha)
        };
        cell.set_char(ch).set_fg(adapt_color(final_fg, truecolor));
    }
}
