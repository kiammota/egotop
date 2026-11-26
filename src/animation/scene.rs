use crate::animation::geometry::{ShapeDrawParams, rasterize_contour};
use crate::ui::theme::{Theme, adapt_color, blend_color, parse_color};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use serde::{Deserialize, Deserializer, Serialize};
use unicode_width::UnicodeWidthChar;

/// Position or size coordinate inside a terminal scene.
#[derive(Debug, Clone, PartialEq)]
pub enum CoordValue {
    Cells(f32),
    Percent(f32),
    Anchor(CoordAnchor),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordAnchor {
    Start,
    Center,
    End,
    Cursor,
}

impl Default for CoordValue {
    fn default() -> Self {
        Self::Anchor(CoordAnchor::Center)
    }
}

impl CoordValue {
    pub fn parse(raw: &str) -> Result<Self, String> {
        let s = raw.trim().to_ascii_lowercase();
        match s.as_str() {
            "center" | "middle" => Ok(Self::Anchor(CoordAnchor::Center)),
            "left" | "top" | "start" => Ok(Self::Anchor(CoordAnchor::Start)),
            "right" | "bottom" | "end" => Ok(Self::Anchor(CoordAnchor::End)),
            "cursor" => Ok(Self::Anchor(CoordAnchor::Cursor)),
            _ if s.ends_with('%') => {
                let pct = s
                    .trim_end_matches('%')
                    .trim()
                    .parse::<f32>()
                    .map_err(|_| format!("Invalid percentage coordinate '{raw}'"))?;
                Ok(Self::Percent(pct))
            }
            _ => {
                let val = s
                    .parse::<f32>()
                    .map_err(|_| format!("Invalid coordinate '{raw}'"))?;
                Ok(Self::Cells(val))
            }
        }
    }

    pub fn resolve_axis(&self, origin: u16, span: u16, cursor_coord: Option<u16>) -> f32 {
        match *self {
            Self::Cells(v) => origin as f32 + v,
            Self::Percent(pct) => origin as f32 + (span as f32) * (pct / 100.0),
            Self::Anchor(CoordAnchor::Start) => origin as f32,
            Self::Anchor(CoordAnchor::Center) => origin as f32 + (span as f32) * 0.5,
            Self::Anchor(CoordAnchor::End) => origin as f32 + (span as f32).saturating_sub(1.0),
            Self::Anchor(CoordAnchor::Cursor) => cursor_coord
                .map(|c| c as f32)
                .unwrap_or_else(|| origin as f32 + (span as f32) * 0.5),
        }
    }

    pub fn resolve_size(&self, span: u16) -> f32 {
        match *self {
            Self::Cells(v) => v.max(0.0),
            Self::Percent(pct) => ((span as f32) * (pct / 100.0)).max(0.0),
            Self::Anchor(_) => (span as f32) * 0.5,
        }
    }
}

trait SaturatingSubF32 {
    fn saturating_sub(self, rhs: f32) -> f32;
}
impl SaturatingSubF32 for f32 {
    fn saturating_sub(self, rhs: f32) -> f32 {
        (self - rhs).max(0.0)
    }
}

impl Serialize for CoordValue {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Cells(v) => serializer.serialize_f32(*v),
            Self::Percent(p) => serializer.serialize_str(&format!("{p}%")),
            Self::Anchor(CoordAnchor::Start) => serializer.serialize_str("start"),
            Self::Anchor(CoordAnchor::Center) => serializer.serialize_str("center"),
            Self::Anchor(CoordAnchor::End) => serializer.serialize_str("end"),
            Self::Anchor(CoordAnchor::Cursor) => serializer.serialize_str("cursor"),
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum CoordInput {
    Int(i32),
    Float(f32),
    Text(String),
}

impl<'de> Deserialize<'de> for CoordValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match CoordInput::deserialize(deserializer)? {
            CoordInput::Int(i) => Ok(Self::Cells(i as f32)),
            CoordInput::Float(f) => Ok(Self::Cells(f)),
            CoordInput::Text(s) => Self::parse(&s).map_err(serde::de::Error::custom),
        }
    }
}

/// Named scene layer controlling composition order (`z_index`), visibility, and opacity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct LayerSpec {
    pub id: String,
    pub z_index: i32,
    pub visible: bool,
    pub opacity: f32,
}

impl Default for LayerSpec {
    fn default() -> Self {
        Self {
            id: "default".to_string(),
            z_index: 10,
            visible: true,
            opacity: 1.0,
        }
    }
}

/// Drawable primitives emitted by tracks in z-order during scene evaluation.
#[derive(Debug, Clone)]
pub enum SceneDrawItem {
    Shape {
        layer_z: i32,
        track_order: usize,
        contour: Vec<(f32, f32)>,
        params: ShapeDrawParams,
    },
    StyledRun {
        layer_z: i32,
        track_order: usize,
        cells: Vec<SceneCell>,
    },
}

/// A single positioned, alpha-aware glyph emitted onto the scene canvas.
#[derive(Debug, Clone)]
pub struct SceneCell {
    pub x: u16,
    pub y: u16,
    pub ch: char,
    pub fg: Color,
    pub bg: Option<Color>,
    pub opacity: f32,
}

/// Resolves theme tokens (`"primary"`, `"secondary"`, `"accent"`, `"text"`, `"muted"`)
/// in addition to `#RRGGBB` and named colors.
pub fn resolve_scene_color(spec: Option<&str>, theme: &Theme, fallback: Color) -> Color {
    let Some(raw) = spec else {
        return fallback;
    };
    match raw.trim().to_ascii_lowercase().as_str() {
        "primary" => theme.primary,
        "secondary" => theme.secondary,
        "accent" => theme.accent,
        "text" => theme.text,
        "muted" => theme.muted,
        "background" | "bg" => theme.background,
        "panel" => theme.panel_background,
        "border" => theme.border,
        "danger" => theme.danger,
        _ => parse_color(raw, fallback),
    }
}

/// Renders a multi-line ASCII sprite centered around `(cx, cy)` inside `clip`.
pub fn collect_ascii_sprite_cells(
    sprite: &str,
    cx: f32,
    cy: f32,
    scale: f32,
    fg: Color,
    opacity: f32,
    clip: Rect,
) -> Vec<SceneCell> {
    if opacity <= 0.01 || clip.width == 0 || clip.height == 0 {
        return Vec::new();
    }
    let lines: Vec<&str> = sprite.lines().collect();
    if lines.is_empty() {
        return Vec::new();
    }

    let max_cols = lines
        .iter()
        .map(|l| l.chars().count())
        .max()
        .unwrap_or(0);
    let height = lines.len();
    let origin_x = (cx - (max_cols as f32) * 0.5 * scale).round() as i32;
    let origin_y = (cy - (height as f32) * 0.5 * scale).round() as i32;

    let mut cells = Vec::new();
    for (row_idx, line) in lines.iter().enumerate() {
        let y_i = origin_y + ((row_idx as f32) * scale).round() as i32;
        if y_i < clip.y as i32 || y_i >= clip.bottom() as i32 {
            continue;
        }
        let mut col_cursor = 0usize;
        for ch in line.chars() {
            let cw = ch.width().unwrap_or(0).max(1);
            if ch != ' ' {
                let x_i = origin_x + ((col_cursor as f32) * scale).round() as i32;
                if x_i >= clip.x as i32 && x_i < clip.right() as i32 {
                    cells.push(SceneCell {
                        x: x_i as u16,
                        y: y_i as u16,
                        ch,
                        fg,
                        bg: None,
                        opacity,
                    });
                }
            }
            col_cursor += cw;
        }
    }
    cells
}

/// Composites sorted `SceneDrawItem` objects cleanly into `buf` within `clip`.
pub fn composite_scene_items(
    mut items: Vec<SceneDrawItem>,
    clip: Rect,
    buf: &mut Buffer,
    truecolor: bool,
) {
    if clip.width == 0 || clip.height == 0 {
        return;
    }
    items.sort_by_key(|item| match item {
        SceneDrawItem::Shape {
            layer_z,
            track_order,
            ..
        }
        | SceneDrawItem::StyledRun {
            layer_z,
            track_order,
            ..
        } => (*layer_z, *track_order),
    });

    for item in items {
        match item {
            SceneDrawItem::Shape {
                contour,
                params,
                ..
            } => {
                rasterize_contour(&contour, &params, clip, buf);
            }
            SceneDrawItem::StyledRun { cells, .. } => {
                for sc in cells {
                    if sc.x < clip.x
                        || sc.x >= clip.right()
                        || sc.y < clip.y
                        || sc.y >= clip.bottom()
                        || sc.opacity <= 0.01
                    {
                        continue;
                    }
                    if let Some(cell) = buf.cell_mut((sc.x, sc.y)) {
                        let alpha = sc.opacity.clamp(0.0, 1.0);
                        let fg = if alpha >= 0.99 {
                            sc.fg
                        } else {
                            blend_color(sc.fg, cell.bg, alpha)
                        };
                        cell.set_char(sc.ch).set_fg(adapt_color(fg, truecolor));
                        if let Some(bg) = sc.bg {
                            let final_bg = if alpha >= 0.99 {
                                bg
                            } else {
                                blend_color(bg, cell.bg, alpha)
                            };
                            cell.set_bg(adapt_color(final_bg, truecolor));
                        }
                    }
                }
            }
        }
    }
}
