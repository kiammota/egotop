use crate::animation::{BackgroundEffect, resolve_region};
use crate::config::{BackgroundConfig, EffectKind};
use crate::ui::theme::{Theme, blend_color, parse_color};
use rand::Rng;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

#[derive(Debug, Clone)]
struct Particle {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    phase: f32,
    glyph_idx: usize,
}

pub struct ParticlesEffect {
    width: u16,
    height: u16,
    items: Vec<Particle>,
}

impl ParticlesEffect {
    pub fn new() -> Self {
        Self {
            width: 0,
            height: 0,
            items: Vec::new(),
        }
    }

    fn target_count(width: u16, height: u16, cfg: &BackgroundConfig) -> usize {
        let area_factor = ((width as f32 * height as f32) / 120.0).max(8.0);
        let configured = cfg.particles.count.max(1) as f32;
        ((configured.min(area_factor * 2.0)) * cfg.density * cfg.particles.density)
            .round()
            .max(4.0) as usize
    }

    fn spawn_particle(rng: &mut impl Rng, width: u16, height: u16) -> Particle {
        let w = width.max(1) as f32;
        let h = height.max(1) as f32;
        Particle {
            x: rng.random_range(0.0..w),
            y: rng.random_range(0.0..h),
            vx: rng.random_range(-3.5..3.5),
            vy: rng.random_range(-2.2..2.2),
            phase: rng.random_range(0.0..std::f32::consts::TAU),
            glyph_idx: rng.random::<u64>() as usize,
        }
    }
}

impl Default for ParticlesEffect {
    fn default() -> Self {
        Self::new()
    }
}

impl BackgroundEffect for ParticlesEffect {
    fn kind(&self) -> EffectKind {
        EffectKind::Particles
    }

    fn resize(&mut self, width: u16, height: u16, cfg: &BackgroundConfig) {
        self.width = width;
        self.height = height;
        let count = Self::target_count(width, height, cfg);
        let mut rng = rand::rng();
        self.items = (0..count)
            .map(|_| Self::spawn_particle(&mut rng, width, height))
            .collect();
    }

    fn update(&mut self, dt_secs: f32, cfg: &BackgroundConfig) {
        if !cfg.enabled || self.width == 0 || self.height == 0 {
            return;
        }
        let wanted = Self::target_count(self.width, self.height, cfg);
        let mut rng = rand::rng();
        while self.items.len() < wanted {
            self.items.push(Self::spawn_particle(&mut rng, self.width, self.height));
        }
        if self.items.len() > wanted {
            self.items.truncate(wanted);
        }

        let speed = cfg.speed.max(0.05) * cfg.particles.speed.max(0.05);
        let w = self.width as f32;
        let h = self.height as f32;

        for p in &mut self.items {
            p.x = (p.x + p.vx * speed * dt_secs).rem_euclid(w);
            p.y = (p.y + p.vy * speed * dt_secs).rem_euclid(h);
            p.phase = (p.phase + dt_secs * 2.5 * speed) % std::f32::consts::TAU;
        }
    }

    fn render(&self, buf: &mut Buffer, area: Rect, cfg: &BackgroundConfig, theme: &Theme) {
        if !cfg.enabled || area.width == 0 || area.height == 0 {
            return;
        }
        let target = resolve_region(area, cfg.particles.region.as_ref(), cfg.region.as_ref());
        if target.width == 0 || target.height == 0 {
            return;
        }

        let base_color = parse_color(&cfg.particles.color, theme.primary);
        let opacity = cfg.opacity.clamp(0.05, 1.0);
        let glyphs: Vec<char> = if cfg.particles.chars.is_empty() {
            vec!['·', '•', '✦', '∘', '*']
        } else {
            cfg.particles.chars.chars().collect()
        };

        for p in &self.items {
            let x = area.x + (p.x.floor() as u16).min(area.width.saturating_sub(1));
            let y = area.y + (p.y.floor() as u16).min(area.height.saturating_sub(1));
            if x < target.left() || x >= target.right() || y < target.top() || y >= target.bottom() {
                continue;
            }
            let pulse = 0.45 + 0.55 * ((p.phase.sin() + 1.0) * 0.5);
            let fg = blend_color(base_color, theme.background, (opacity * pulse).clamp(0.1, 1.0));
            let ch = glyphs[p.glyph_idx % glyphs.len()];

            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_char(ch)
                    .set_style(Style::default().fg(fg).bg(ratatui::style::Color::Reset));
            }
        }
    }
}
