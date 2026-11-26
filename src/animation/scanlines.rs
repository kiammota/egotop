use crate::animation::{BackgroundEffect, resolve_region};
use crate::config::{BackgroundConfig, EffectKind};
use crate::ui::theme::{Theme, blend_color, parse_color};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

pub struct ScanlinesEffect {
    sweep_y: f32,
    height: u16,
}

impl ScanlinesEffect {
    pub fn new() -> Self {
        Self {
            sweep_y: 0.0,
            height: 0,
        }
    }
}

impl Default for ScanlinesEffect {
    fn default() -> Self {
        Self::new()
    }
}

impl BackgroundEffect for ScanlinesEffect {
    fn kind(&self) -> EffectKind {
        EffectKind::Scanlines
    }

    fn resize(&mut self, _width: u16, height: u16, _cfg: &BackgroundConfig) {
        self.height = height;
    }

    fn update(&mut self, dt_secs: f32, cfg: &BackgroundConfig) {
        if !cfg.enabled || self.height == 0 {
            return;
        }
        let speed = cfg.speed.max(0.05) * cfg.scanlines.speed.max(0.05);
        self.sweep_y = (self.sweep_y + dt_secs * speed * 10.0) % (self.height.max(1) as f32);
    }

    fn render(&self, buf: &mut Buffer, area: Rect, cfg: &BackgroundConfig, theme: &Theme) {
        if !cfg.enabled || area.width == 0 || area.height == 0 {
            return;
        }
        let target = resolve_region(area, cfg.scanlines.region.as_ref(), cfg.region.as_ref());
        if target.width == 0 || target.height == 0 {
            return;
        }

        let spacing = cfg.scanlines.spacing.max(1);
        let intensity = cfg.scanlines.intensity.clamp(0.05, 1.0);
        let line_color = parse_color(&cfg.scanlines.color, theme.border);
        let band_fg = blend_color(line_color, theme.background, (cfg.opacity * intensity).clamp(0.08, 0.8));
        let sweep_row = target.top() + (self.sweep_y.floor() as u16 % target.height.max(1));

        for y in target.top()..target.bottom() {
            let is_sweep = y == sweep_row;
            let is_scanline = (y - target.top()).is_multiple_of(spacing);
            if !is_sweep && !is_scanline {
                continue;
            }

            let fg = if is_sweep {
                blend_color(theme.primary, theme.background, (cfg.opacity * 0.75).clamp(0.15, 0.9))
            } else {
                band_fg
            };
            let ch = if is_sweep { '─' } else { '┄' };

            for x in target.left()..target.right() {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(ch)
                        .set_style(Style::default().fg(fg).bg(theme.background));
                }
            }
        }
    }
}
