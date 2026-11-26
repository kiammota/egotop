use crate::animation::scene::SceneCell;
use crate::ui::theme::blend_color;
use ratatui::layout::Rect;
use ratatui::style::Color;
use serde::{Deserialize, Serialize};
use unicode_width::UnicodeWidthChar;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TextEffectKind {
    #[default]
    None,
    Typewriter,
    Fade,
    Reveal,
    Wave,
    Glitch,
    ColorCycle,
    CharByChar,
    Scroll,
    Pulse,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TextDirection {
    #[default]
    Left,
    Right,
    Up,
    Down,
    Center,
}

#[derive(Debug, Clone)]
pub struct TextRenderParams<'a> {
    pub text: &'a str,
    pub effect: TextEffectKind,
    pub direction: TextDirection,
    pub center_x: f32,
    pub center_y: f32,
    pub progress: f32,
    pub elapsed_secs: f32,
    pub speed: f32,
    pub intensity: f32,
    pub opacity: f32,
    pub base_color: Color,
    pub palette: &'a [Color],
    pub bg_fallback: Color,
    pub charset: &'a str,
}

/// Evaluates an animated text string and produces positioned `SceneCell`s inside `clip`.
pub fn evaluate_text_cells(params: &TextRenderParams<'_>, clip: Rect) -> Vec<SceneCell> {
    if params.text.is_empty() || params.opacity <= 0.01 || clip.width == 0 || clip.height == 0 {
        return Vec::new();
    }

    let chars: Vec<char> = params.text.chars().collect();
    let total = chars.len();
    let total_width: usize = chars.iter().map(|c| c.width().unwrap_or(0).max(1)).sum();
    let scramble_pool: Vec<char> = if params.charset.is_empty() {
        vec!['0', '1', '#', '$', '*', '%', '@', '+']
    } else {
        params.charset.chars().collect()
    };

    let t = params.progress.clamp(0.0, 1.0);
    let time = params.elapsed_secs * params.speed.max(0.05);
    let intensity = params.intensity.clamp(0.0, 2.0);
    let base_alpha = params.opacity.clamp(0.0, 1.0);

    let start_x = match params.effect {
        TextEffectKind::Scroll => {
            let travel = clip.width as f32 + total_width as f32;
            let phase = (time * 8.0) % travel.max(1.0);
            match params.direction {
                TextDirection::Right => (clip.x as f32 - total_width as f32) + phase,
                _ => clip.right() as f32 - phase,
            }
        }
        _ => params.center_x - (total_width as f32) * 0.5,
    };

    let mut out = Vec::with_capacity(total + 1);
    let mut col_offset = 0i32;

    for (idx, &ch) in chars.iter().enumerate() {
        let cw = ch.width().unwrap_or(0).max(1) as i32;
        let idx_ratio = if total <= 1 {
            0.0
        } else {
            idx as f32 / (total - 1) as f32
        };

        let ordered_ratio = match params.direction {
            TextDirection::Right => 1.0 - idx_ratio,
            TextDirection::Center => (idx_ratio - 0.5).abs() * 2.0,
            _ => idx_ratio,
        };

        let mut draw_ch = ch;
        let mut fg = params.base_color;
        let mut alpha = base_alpha;
        let mut dx = 0i32;
        let mut dy = 0i32;

        match params.effect {
            TextEffectKind::None => {}
            TextEffectKind::Typewriter => {
                let shown = ((total as f32) * t).ceil() as usize;
                let logical_idx = match params.direction {
                    TextDirection::Right => total.saturating_sub(1 + idx),
                    _ => idx,
                };
                if logical_idx >= shown {
                    col_offset += cw;
                    continue;
                }
            }
            TextEffectKind::Fade => {
                alpha *= t;
            }
            TextEffectKind::Reveal => {
                let threshold = t * 1.15;
                if ordered_ratio > threshold {
                    col_offset += cw;
                    continue;
                }
                if ordered_ratio + 0.15 > threshold && ch != ' ' {
                    let h = ((time * 24.0) as usize + idx * 13) % scramble_pool.len();
                    draw_ch = scramble_pool[h];
                    fg = palette_or(params.palette, 0, fg);
                }
            }
            TextEffectKind::Wave => {
                let wave = (time * 4.5 + idx as f32 * 0.55).sin();
                match params.direction {
                    TextDirection::Left | TextDirection::Right | TextDirection::Center => {
                        dy = (wave * (1.2 * intensity)).round() as i32;
                    }
                    TextDirection::Up | TextDirection::Down => {
                        dx = (wave * (1.2 * intensity)).round() as i32;
                    }
                }
                let shimmer = ((wave + 1.0) * 0.5).clamp(0.0, 1.0);
                let alt = palette_or(params.palette, 1, fg);
                fg = blend_color(alt, fg, shimmer);
            }
            TextEffectKind::Glitch => {
                let tick = (time * 18.0).floor() as usize;
                let hash = (tick * 73 + idx * 31) % 100;
                let prob = (22.0 * intensity).min(85.0) as usize;
                if ch != ' ' && hash < prob {
                    draw_ch = scramble_pool[(tick + idx * 7) % scramble_pool.len()];
                    if hash.is_multiple_of(3) {
                        dx = if hash.is_multiple_of(2) { 1 } else { -1 };
                    }
                    fg = palette_or(params.palette, hash % params.palette.len().max(1), fg);
                }
            }
            TextEffectKind::ColorCycle => {
                if !params.palette.is_empty() {
                    let phase = time * 2.2 + idx as f32 * 0.18;
                    let pal_pos = phase.rem_euclid(params.palette.len() as f32);
                    let i0 = pal_pos.floor() as usize % params.palette.len();
                    let i1 = (i0 + 1) % params.palette.len();
                    fg = blend_color(params.palette[i1], params.palette[i0], pal_pos.fract());
                }
            }
            TextEffectKind::CharByChar => {
                let window = 0.25f32;
                let start_t = ordered_ratio * (1.0 - window);
                let local = ((t - start_t) / window).clamp(0.0, 1.0);
                if local <= 0.01 {
                    col_offset += cw;
                    continue;
                }
                alpha *= local;
                dy = match params.direction {
                    TextDirection::Up => ((1.0 - local) * 2.0 * intensity).round() as i32,
                    _ => -((1.0 - local) * 2.0 * intensity).round() as i32,
                };
            }
            TextEffectKind::Scroll => {
                if matches!(params.direction, TextDirection::Up | TextDirection::Down) {
                    let dir_sign = if params.direction == TextDirection::Up { -1.0 } else { 1.0 };
                    let wrap_h = clip.height.max(1) as f32;
                    let shift = ((time * 3.0 * dir_sign) % wrap_h).round() as i32;
                    dy = shift;
                }
            }
            TextEffectKind::Pulse => {
                let pulse = (time * 3.8).sin() * 0.5 + 0.5;
                let alt = palette_or(params.palette, 0, params.bg_fallback);
                let mix = (0.35 + 0.65 * pulse).clamp(0.0, 1.0);
                fg = blend_color(fg, alt, mix);
                alpha *= (0.4 + 0.6 * pulse).clamp(0.0, 1.0);
            }
        }

        let px = start_x.round() as i32 + col_offset + dx;
        let py = params.center_y.round() as i32 + dy;
        if draw_ch != ' '
            && px >= clip.x as i32
            && px < clip.right() as i32
            && py >= clip.y as i32
            && py < clip.bottom() as i32
        {
            out.push(SceneCell {
                x: px as u16,
                y: py as u16,
                ch: draw_ch,
                fg,
                bg: None,
                opacity: alpha,
            });
        }
        col_offset += cw;
    }

    if params.effect == TextEffectKind::Typewriter && t < 0.99 && ((time * 4.0) as usize).is_multiple_of(2)
    {
        let shown = ((total as f32) * t).ceil() as i32;
        let cursor_x = start_x.round() as i32 + shown.min(total_width as i32);
        let cursor_y = params.center_y.round() as i32;
        if cursor_x >= clip.x as i32
            && cursor_x < clip.right() as i32
            && cursor_y >= clip.y as i32
            && cursor_y < clip.bottom() as i32
        {
            out.push(SceneCell {
                x: cursor_x as u16,
                y: cursor_y as u16,
                ch: '▌',
                fg: palette_or(params.palette, 0, params.base_color),
                bg: None,
                opacity: base_alpha,
            });
        }
    }

    out
}

fn palette_or(palette: &[Color], idx: usize, fallback: Color) -> Color {
    if palette.is_empty() {
        fallback
    } else {
        palette[idx % palette.len()]
    }
}
