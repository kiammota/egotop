use crate::animation::{
    AnimationCatalog, BackgroundEffect, GeometryRenderer, MatrixEffect, ParticlesEffect,
    ShapeDrawParams, ShapePrimitive, ShapeSpec, TextDirection, TextEffectKind, TextRenderParams,
    TimelineClock, evaluate_text_cells, rasterize_contour,
};
use crate::config::{BackgroundConfig, Config};
use crate::ui::container::{ComponentAnimationConfig, ComponentConfig, ComponentKind};
use crate::ui::layout::ComputedLayout;
use crate::ui::theme::{Theme, parse_color};
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use std::collections::HashMap;

const SPINNER_DOTS: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
const SPINNER_LINE: &[&str] = &["-", "\\", "|", "/"];
const SPINNER_BRAILLE: &[&str] = &["⣾", "⣽", "⣻", "⢿", "⡿", "⣟", "⣯", "⣷"];
const SPINNER_PULSE: &[&str] = &["·", "•", "●", "•"];

#[derive(Debug, Clone)]
pub struct SpinnerState {
    pub elapsed_secs: f32,
    pub frame_idx: usize,
}

#[derive(Debug, Clone)]
pub struct TypewriterState {
    pub elapsed_secs: f32,
    pub progress: f32,
}

#[derive(Debug, Clone)]
pub struct FramesState {
    pub elapsed_secs: f32,
    pub frame_idx: usize,
}

pub struct MatrixComponentState {
    pub effect: MatrixEffect,
    pub width: u16,
    pub height: u16,
}

pub struct ParticlesComponentState {
    pub effect: ParticlesEffect,
    pub width: u16,
    pub height: u16,
}

#[derive(Debug, Clone)]
pub struct AsciiObjectState {
    pub clock: TimelineClock,
    pub phase: f32,
}

#[derive(Debug, Clone)]
pub struct MarqueeState {
    pub offset_cols: f32,
}

pub enum ComponentAnimState {
    Spinner(SpinnerState),
    Typewriter(TypewriterState),
    Frames(FramesState),
    Matrix(MatrixComponentState),
    Particles(ParticlesComponentState),
    AsciiObject(AsciiObjectState),
    Marquee(MarqueeState),
}

#[derive(Default)]
pub struct ComponentAnimationStore {
    states: HashMap<String, ComponentAnimState>,
}

impl ComponentAnimationStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.states.clear();
    }

    pub fn get(&self, id: &str) -> Option<&ComponentAnimState> {
        self.states.get(id)
    }

    pub fn update_all(
        &mut self,
        dt_secs: f32,
        layout: &ComputedLayout,
        config: &Config,
        _catalog: &AnimationCatalog,
    ) {
        for placed in &layout.components {
            let comp = &placed.config;
            let anim_cfg = comp.animation.clone().unwrap_or_default();
            let speed = anim_cfg.speed.clamp(0.05, 10.0);
            let w = placed.inner_rect.width.max(1);
            let h = placed.inner_rect.height.max(1);

            match comp.kind {
                ComponentKind::Spinner => {
                    let fps = anim_cfg.fps.unwrap_or(10).clamp(1, 60) as f32;
                    let frames = spinner_frames(anim_cfg.style.as_deref());
                    let entry = self.states.entry(comp.id.clone()).or_insert_with(|| {
                        ComponentAnimState::Spinner(SpinnerState {
                            elapsed_secs: 0.0,
                            frame_idx: 0,
                        })
                    });
                    if let ComponentAnimState::Spinner(st) = entry {
                        st.elapsed_secs += dt_secs * speed;
                        let step = 1.0 / fps;
                        while st.elapsed_secs >= step {
                            st.elapsed_secs -= step;
                            st.frame_idx = (st.frame_idx + 1) % frames.len();
                        }
                    }
                }
                ComponentKind::Typewriter => {
                    let entry = self.states.entry(comp.id.clone()).or_insert_with(|| {
                        ComponentAnimState::Typewriter(TypewriterState {
                            elapsed_secs: 0.0,
                            progress: 0.0,
                        })
                    });
                    if let ComponentAnimState::Typewriter(st) = entry {
                        let text_len = comp.content.text.as_deref().unwrap_or("EgoTop").chars().count().max(1);
                        let duration = (text_len as f32 / (14.0 * speed)).max(0.4);
                        st.elapsed_secs += dt_secs;
                        if anim_cfg.loop_anim {
                            let cycle = duration + 1.0;
                            let t = st.elapsed_secs % cycle;
                            st.progress = (t / duration).clamp(0.0, 1.0);
                        } else {
                            st.progress = (st.elapsed_secs / duration).clamp(0.0, 1.0);
                        }
                    }
                }
                ComponentKind::Frames => {
                    let fps = anim_cfg.fps.unwrap_or(8).clamp(1, 60) as f32;
                    let frame_count = comp.content.frames.len().max(1);
                    let entry = self.states.entry(comp.id.clone()).or_insert_with(|| {
                        ComponentAnimState::Frames(FramesState {
                            elapsed_secs: 0.0,
                            frame_idx: 0,
                        })
                    });
                    if let ComponentAnimState::Frames(st) = entry {
                        st.elapsed_secs += dt_secs * speed;
                        let step = 1.0 / fps;
                        while st.elapsed_secs >= step {
                            st.elapsed_secs -= step;
                            if anim_cfg.loop_anim {
                                st.frame_idx = (st.frame_idx + 1) % frame_count;
                            } else if st.frame_idx + 1 < frame_count {
                                st.frame_idx += 1;
                            }
                        }
                    }
                }
                ComponentKind::Matrix => {
                    let bg_cfg = component_matrix_bg_config(config, &anim_cfg);
                    let entry = self.states.entry(comp.id.clone()).or_insert_with(|| {
                        let mut eff = MatrixEffect::new();
                        eff.resize(w, h, &bg_cfg);
                        ComponentAnimState::Matrix(MatrixComponentState {
                            effect: eff,
                            width: w,
                            height: h,
                        })
                    });
                    if let ComponentAnimState::Matrix(st) = entry {
                        if st.width != w || st.height != h {
                            st.effect.resize(w, h, &bg_cfg);
                            st.width = w;
                            st.height = h;
                        }
                        st.effect.update(dt_secs, &bg_cfg);
                    }
                }
                ComponentKind::Particles => {
                    let bg_cfg = component_particles_bg_config(config, &anim_cfg);
                    let entry = self.states.entry(comp.id.clone()).or_insert_with(|| {
                        let mut eff = ParticlesEffect::new();
                        eff.resize(w, h, &bg_cfg);
                        ComponentAnimState::Particles(ParticlesComponentState {
                            effect: eff,
                            width: w,
                            height: h,
                        })
                    });
                    if let ComponentAnimState::Particles(st) = entry {
                        if st.width != w || st.height != h {
                            st.effect.resize(w, h, &bg_cfg);
                            st.width = w;
                            st.height = h;
                        }
                        st.effect.update(dt_secs, &bg_cfg);
                    }
                }
                ComponentKind::AsciiObject => {
                    let entry = self.states.entry(comp.id.clone()).or_insert_with(|| {
                        ComponentAnimState::AsciiObject(AsciiObjectState {
                            clock: TimelineClock::new(
                                2400,
                                crate::animation::LoopConfig::default(),
                                speed,
                                30,
                            ),
                            phase: 0.0,
                        })
                    });
                    if let ComponentAnimState::AsciiObject(st) = entry {
                        st.clock.advance(dt_secs);
                        st.phase = (st.clock.cycle_elapsed_secs / st.clock.duration_secs.max(0.01))
                            .clamp(0.0, 1.0);
                    }
                }
                ComponentKind::Greeting => {
                    let marquee_speed = comp
                        .animation
                        .as_ref()
                        .map(|a| a.speed)
                        .unwrap_or(config.typography.marquee_speed)
                        .clamp(0.5, 60.0);
                    let entry = self.states.entry(comp.id.clone()).or_insert_with(|| {
                        ComponentAnimState::Marquee(MarqueeState { offset_cols: 0.0 })
                    });
                    if let ComponentAnimState::Marquee(st) = entry {
                        st.offset_cols += dt_secs * marquee_speed;
                    }
                }
                _ => {}
            }
        }
    }
}

pub fn spinner_frames(style: Option<&str>) -> &'static [&'static str] {
    match style.map(|s| s.to_ascii_lowercase()).as_deref() {
        Some("line") | Some("ascii") => SPINNER_LINE,
        Some("dots") => SPINNER_DOTS,
        Some("pulse") => SPINNER_PULSE,
        _ => SPINNER_BRAILLE,
    }
}

pub fn component_matrix_bg_config(
    config: &Config,
    anim: &ComponentAnimationConfig,
) -> BackgroundConfig {
    let mut bg = config.background.clone();
    bg.enabled = true;
    bg.matrix.enabled = true;
    bg.speed = anim.speed.clamp(0.1, 10.0);
    if let Some(d) = anim.density {
        bg.density = d.clamp(0.05, 1.0);
        bg.matrix.density = bg.density;
    }
    if let Some(cs) = anim.charset {
        bg.matrix.charset = cs;
    }
    bg
}

pub fn component_particles_bg_config(
    config: &Config,
    anim: &ComponentAnimationConfig,
) -> BackgroundConfig {
    let mut bg = config.background.clone();
    bg.enabled = true;
    bg.particles.enabled = true;
    bg.speed = anim.speed.clamp(0.1, 10.0);
    if let Some(d) = anim.density {
        bg.density = d.clamp(0.05, 1.0);
        bg.particles.density = bg.density;
    }
    bg
}

/// Renders into an isolated scratch `Buffer` sized to `target_rect`, then blits strictly
/// within `target_rect` on `frame.buffer_mut()` so no component can ever draw outside its bounds.
pub fn render_clipped<F>(frame: &mut Frame, target_rect: Rect, draw_fn: F)
where
    F: FnOnce(&mut Buffer, Rect),
{
    let clipped = target_rect.intersection(frame.area());
    if clipped.width == 0 || clipped.height == 0 {
        return;
    }
    let local_area = Rect::new(0, 0, clipped.width, clipped.height);
    let mut scratch = Buffer::empty(local_area);
    draw_fn(&mut scratch, local_area);

    let dest = frame.buffer_mut();
    for dy in 0..clipped.height {
        for dx in 0..clipped.width {
            let src_cell = &scratch[(dx, dy)];
            if let Some(dst_cell) = dest.cell_mut((clipped.x + dx, clipped.y + dy)) {
                *dst_cell = src_cell.clone();
            }
        }
    }
}

pub fn render_animated_component(
    frame: &mut Frame,
    area: Rect,
    comp: &ComponentConfig,
    store: &ComponentAnimationStore,
    config: &Config,
    theme: &Theme,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let anim_cfg = comp.animation.clone().unwrap_or_default();
    let fg = comp
        .visual
        .color
        .as_deref()
        .map(|c| parse_color(c, theme.primary))
        .unwrap_or(theme.primary);
    let bg = comp
        .visual
        .background
        .as_deref()
        .map(|c| parse_color(c, theme.panel_background))
        .unwrap_or(theme.panel_background);

    match comp.kind {
        ComponentKind::Spinner => {
            let frames = spinner_frames(anim_cfg.style.as_deref());
            let idx = match store.get(&comp.id) {
                Some(ComponentAnimState::Spinner(st)) => st.frame_idx % frames.len(),
                _ => 0,
            };
            let glyph = frames[idx];
            let label = comp.content.text.as_deref().unwrap_or("");
            let line = if label.is_empty() {
                glyph.to_string()
            } else {
                format!("{glyph} {label}")
            };
            render_clipped(frame, area, |buf, local| {
                fill_bg(buf, local, bg);
                buf.set_stringn(
                    0,
                    0,
                    &line,
                    local.width as usize,
                    Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD),
                );
            });
        }
        ComponentKind::Typewriter => {
            let text = comp.content.text.as_deref().unwrap_or("EgoTop");
            let (progress, elapsed_secs) = match store.get(&comp.id) {
                Some(ComponentAnimState::Typewriter(st)) => (st.progress, st.elapsed_secs),
                _ => (1.0, 0.0),
            };
            render_clipped(frame, area, |buf, local| {
                fill_bg(buf, local, bg);
                let palette = [fg, theme.accent];
                let params = TextRenderParams {
                    text,
                    effect: TextEffectKind::Typewriter,
                    direction: TextDirection::Left,
                    center_x: (local.width as f32) * 0.5,
                    center_y: (local.height as f32) * 0.5,
                    progress,
                    elapsed_secs,
                    speed: anim_cfg.speed.max(0.1),
                    intensity: 1.0,
                    opacity: 1.0,
                    base_color: fg,
                    palette: &palette,
                    bg_fallback: bg,
                    charset: "",
                };
                let cells = evaluate_text_cells(&params, local);
                for cell in cells {
                    if let Some(target) = buf.cell_mut((cell.x, cell.y)) {
                        target.set_char(cell.ch);
                        target.set_fg(cell.fg);
                        target.set_bg(bg);
                    }
                }
            });
        }
        ComponentKind::Frames => {
            let fallback_frames = ["[●  ]", "[ ● ]", "[  ●]", "[ ● ]"];
            let (raw_frame, _) = if comp.content.frames.is_empty() {
                let idx = match store.get(&comp.id) {
                    Some(ComponentAnimState::Frames(st)) => st.frame_idx % fallback_frames.len(),
                    _ => 0,
                };
                (fallback_frames[idx], fallback_frames.len())
            } else {
                let idx = match store.get(&comp.id) {
                    Some(ComponentAnimState::Frames(st)) => {
                        st.frame_idx % comp.content.frames.len()
                    }
                    _ => 0,
                };
                (comp.content.frames[idx].as_str(), comp.content.frames.len())
            };
            render_clipped(frame, area, |buf, local| {
                fill_bg(buf, local, bg);
                for (row, line) in raw_frame.lines().take(local.height as usize).enumerate() {
                    buf.set_stringn(
                        0,
                        row as u16,
                        line,
                        local.width as usize,
                        Style::default().fg(fg).bg(bg),
                    );
                }
            });
        }
        ComponentKind::Matrix => {
            let bg_cfg = component_matrix_bg_config(config, &anim_cfg);
            render_clipped(frame, area, |buf, local| {
                fill_bg(buf, local, bg);
                if let Some(ComponentAnimState::Matrix(st)) = store.get(&comp.id) {
                    st.effect.render(buf, local, &bg_cfg, theme);
                }
            });
        }
        ComponentKind::Particles => {
            let bg_cfg = component_particles_bg_config(config, &anim_cfg);
            render_clipped(frame, area, |buf, local| {
                fill_bg(buf, local, bg);
                if let Some(ComponentAnimState::Particles(st)) = store.get(&comp.id) {
                    st.effect.render(buf, local, &bg_cfg, theme);
                }
            });
        }
        ComponentKind::AsciiObject => {
            let phase = match store.get(&comp.id) {
                Some(ComponentAnimState::AsciiObject(st)) => st.phase,
                _ => 0.0,
            };
            let (primitive, sides) = match anim_cfg.style.as_deref() {
                Some("circle") => (ShapePrimitive::Circle, 16),
                Some("square") => (ShapePrimitive::Square, 4),
                Some("triangle") => (ShapePrimitive::Triangle, 3),
                _ => (ShapePrimitive::Polygon, 6),
            };
            render_clipped(frame, area, |buf, local| {
                fill_bg(buf, local, bg);
                let spec = ShapeSpec::from_primitive(primitive, sides, Vec::new());
                let contour = spec.normalized_contour();
                let params = ShapeDrawParams {
                    center_x: (local.width as f32) * 0.5,
                    center_y: (local.height as f32) * 0.5,
                    half_w: (local.width as f32 * 0.35).max(1.0),
                    half_h: (local.height as f32 * 0.35).max(1.0),
                    rotation_deg: phase * 360.0,
                    scale: 0.85 + 0.15 * (phase * std::f32::consts::TAU).sin(),
                    stroke: true,
                    fill: false,
                    closed: spec.closed,
                    stroke_color: fg,
                    fill_color: fg,
                    opacity: 1.0,
                    renderer: GeometryRenderer::Braille,
                    stroke_char: '✦',
                    fill_char: '·',
                    truecolor: true,
                };
                rasterize_contour(&contour, &params, local, buf);
            });
        }
        _ => {}
    }
}

fn fill_bg(buf: &mut Buffer, area: Rect, bg: Color) {
    if bg == Color::Reset {
        return;
    }
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_bg(bg);
            }
        }
    }
}
