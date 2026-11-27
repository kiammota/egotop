use crate::animation::{BackgroundEffect, resolve_region};
use crate::config::{BackgroundConfig, EffectKind};
use crate::ui::theme::{Theme, blend_color, parse_color};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

pub struct WaterEffect {
    phase: f32,
}

impl WaterEffect {
    pub fn new() -> Self {
        Self { phase: 0.0 }
    }
}

impl Default for WaterEffect {
    fn default() -> Self {
        Self::new()
    }
}

impl BackgroundEffect for WaterEffect {
    fn kind(&self) -> EffectKind {
        EffectKind::Water
    }

    fn resize(&mut self, _width: u16, _height: u16, _cfg: &BackgroundConfig) {}

    fn update(&mut self, dt_secs: f32, cfg: &BackgroundConfig) {
        if !cfg.enabled {
            return;
        }
        let speed = cfg.speed.max(0.05) * cfg.water.speed.max(0.05);
        self.phase = (self.phase + dt_secs * speed * 2.4) % (std::f32::consts::TAU * 100.0);
    }

    fn render(&self, buf: &mut Buffer, area: Rect, cfg: &BackgroundConfig, theme: &Theme) {
        if !cfg.enabled || area.width == 0 || area.height == 0 {
            return;
        }
        let target = resolve_region(area, cfg.water.region.as_ref(), cfg.region.as_ref());
        if target.width == 0 || target.height == 0 {
            return;
        }

        let deep_color = parse_color(&cfg.water.color, theme.secondary);
        let crest_color = parse_color(&cfg.water.crest_color, theme.primary);
        let opacity = cfg.opacity.clamp(0.05, 1.0);
        let amplitude = cfg.water.amplitude.clamp(0.1, 2.0);
        let density = (cfg.density * cfg.water.density).clamp(0.1, 1.0);

        for y in target.top()..target.bottom() {
            let yf = (y - target.top()) as f32;
            for x in target.left()..target.right() {
                let xf = (x - target.left()) as f32;

                let w1 = (xf * 0.22 + self.phase + yf * 0.45).sin();
                let w2 = (xf * 0.11 - self.phase * 0.7 + yf * 0.85).cos();
                let wave = ((w1 * 0.6 + w2 * 0.4) * amplitude).clamp(-1.0, 1.0);
                let norm = (wave + 1.0) * 0.5; // 0..1

                if norm < (1.0 - density) * 0.65 {
                    continue;
                }

                let (ch, color) = if norm > 0.82 {
                    ('≈', blend_color(crest_color, theme.background, (opacity + 0.2).min(1.0)))
                } else if norm > 0.62 {
                    ('~', blend_color(crest_color, theme.background, opacity * 0.85))
                } else if norm > 0.42 {
                    ('∼', blend_color(deep_color, theme.background, opacity * 0.7))
                } else {
                    ('·', blend_color(deep_color, theme.background, opacity * 0.45))
                };

                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(ch)
                        .set_style(Style::default().fg(color).bg(ratatui::style::Color::Reset));
                }
            }
        }
    }
}
