use crate::animation::{BackgroundEffect, resolve_region};
use crate::config::{BackgroundConfig, EffectKind, MatrixCharset};
use crate::ui::theme::{Theme, blend_color, parse_color};
use rand::Rng;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

#[derive(Debug, Clone)]
struct DropColumn {
    active: bool,
    y: f32,
    speed: f32,
    length: usize,
    glyph_seed: usize,
}

pub struct MatrixEffect {
    width: u16,
    height: u16,
    columns: Vec<DropColumn>,
    tick_counter: usize,
}

impl MatrixEffect {
    pub fn new() -> Self {
        Self {
            width: 0,
            height: 0,
            columns: Vec::new(),
            tick_counter: 0,
        }
    }

    fn spawn_column(
        height: u16,
        rng: &mut impl Rng,
        cfg: &BackgroundConfig,
        random_start: bool,
    ) -> DropColumn {
        let density = (cfg.density * cfg.matrix.density).clamp(0.05, 1.0);
        let active = rng.random::<f32>() <= density;
        let min_len = cfg.matrix.min_length.max(2);
        let max_len = cfg.matrix.max_length.max(min_len);
        let length = rng.random_range(min_len..=max_len);
        let speed = rng.random_range(6.0..18.0) * cfg.speed.max(0.1) * cfg.matrix.speed.max(0.1);
        let h = height.max(10) as f32;
        let y = if random_start {
            rng.random_range(-(length as f32)..h)
        } else {
            -(rng.random_range(0.0..(h * 0.6)))
        };

        DropColumn {
            active,
            y,
            speed,
            length,
            glyph_seed: rng.random::<u64>() as usize,
        }
    }
}

impl Default for MatrixEffect {
    fn default() -> Self {
        Self::new()
    }
}

impl BackgroundEffect for MatrixEffect {
    fn kind(&self) -> EffectKind {
        EffectKind::Matrix
    }

    fn resize(&mut self, width: u16, height: u16, cfg: &BackgroundConfig) {
        if self.width == width && self.height == height && !self.columns.is_empty() {
            return;
        }
        self.width = width;
        self.height = height;
        let mut rng = rand::rng();
        self.columns = (0..width)
            .map(|_| Self::spawn_column(height, &mut rng, cfg, true))
            .collect();
    }

    fn update(&mut self, dt_secs: f32, cfg: &BackgroundConfig) {
        if !cfg.enabled || !cfg.matrix.enabled {
            return;
        }
        self.tick_counter = self.tick_counter.wrapping_add(1);
        let mut rng = rand::rng();
        let max_y = self.height as f32;
        let height = self.height;

        for col_idx in 0..self.columns.len() {
            let speed_scale = cfg.speed.max(0.05) * cfg.matrix.speed.max(0.05);
            let col = &mut self.columns[col_idx];
            if col.active {
                col.y += (col.speed * speed_scale * dt_secs).min(4.0);
                if self.tick_counter.is_multiple_of(4) && rng.random::<f32>() < 0.25 {
                    col.glyph_seed = col.glyph_seed.wrapping_add(rng.random_range(1..17));
                }
                if col.y - (col.length as f32) > max_y {
                    *col = Self::spawn_column(height, &mut rng, cfg, false);
                }
            } else if rng.random::<f32>() < 0.015 * cfg.density * cfg.matrix.density {
                *col = Self::spawn_column(height, &mut rng, cfg, false);
                col.active = true;
            }
        }
    }

    fn render(&self, buf: &mut Buffer, area: Rect, cfg: &BackgroundConfig, theme: &Theme) {
        if !cfg.enabled || !cfg.matrix.enabled || area.width == 0 || area.height == 0 {
            return;
        }
        let target = resolve_region(area, cfg.matrix.region.as_ref(), cfg.region.as_ref());
        if target.width == 0 || target.height == 0 {
            return;
        }

        let trail_color = parse_color(&cfg.matrix.color, theme.border);
        let head_color = parse_color(&cfg.matrix.head_color, theme.primary);
        let opacity = cfg.opacity.clamp(0.05, 1.0);
        let charset = matrix_charset(cfg.matrix.charset);

        for x in target.left()..target.right() {
            let rel_x = (x - area.x) as usize;
            let Some(col) = self.columns.get(rel_x) else {
                continue;
            };
            if !col.active {
                continue;
            }

            let head_y = col.y.floor() as isize;
            for offset in 0..col.length {
                let py = head_y - offset as isize;
                if py < (target.top() - area.y) as isize || py >= (target.bottom() - area.y) as isize {
                    continue;
                }
                let abs_y = area.y + py as u16;
                let fade = 1.0 - (offset as f32 / col.length as f32);
                let fg = if offset == 0 {
                    blend_color(head_color, theme.background, (opacity + 0.28).min(1.0))
                } else {
                    blend_color(trail_color, theme.background, opacity * fade)
                };

                let glyph_idx = col
                    .glyph_seed
                    .wrapping_add((py as usize).wrapping_mul(13))
                    .wrapping_add(rel_x.wrapping_mul(7))
                    % charset.len();
                let ch = charset[glyph_idx];

                if let Some(cell) = buf.cell_mut((x, abs_y)) {
                    cell.set_char(ch).set_style(Style::default().fg(fg).bg(theme.background));
                }
            }
        }
    }
}

fn matrix_charset(kind: MatrixCharset) -> &'static [char] {
    match kind {
        MatrixCharset::Katakana => &[
            'ｱ', 'ｲ', 'ｳ', 'ｴ', 'ｵ', 'ｶ', 'ｷ', 'ｸ', 'ｹ', 'ｺ', 'ｻ', 'ｼ', 'ｽ', 'ｾ', 'ｿ', 'ﾀ',
            'ﾁ', 'ﾂ', 'ﾃ', 'ﾄ', 'ﾅ', 'ﾆ', 'ﾇ', 'ﾈ', 'ﾉ', 'ﾊ', 'ﾋ', 'ﾌ', 'ﾍ', 'ﾎ', 'ﾏ', 'ﾐ',
            'ﾑ', 'ﾒ', 'ﾓ', 'ﾔ', 'ﾕ', 'ﾖ', 'ﾗ', 'ﾘ', 'ﾙ', 'ﾚ', 'ﾛ', 'ﾜ', 'ﾝ', '0', '1', '7',
        ],
        MatrixCharset::Hex => &[
            '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'A', 'B', 'C', 'D', 'E', 'F',
        ],
        MatrixCharset::Ascii => &[
            '!', '@', '#', '$', '%', '&', '*', '+', '-', '=', ':', ';', '<', '>', '?', 'X', 'Z',
            '0', '1',
        ],
        MatrixCharset::Binary => &['0', '1'],
    }
}
