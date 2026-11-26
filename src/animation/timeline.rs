use crate::animation::geometry::{ShapeDrawParams, ShapeSpec, morph_contours};
use crate::animation::scene::{
    SceneCell, SceneDrawItem, collect_ascii_sprite_cells, resolve_scene_color,
};
use crate::animation::text_fx::{TextDirection, TextRenderParams, evaluate_text_cells};
use crate::config::{AnimationDefinition, KeyframeSpec, MorphTransitionMode, TrackKind, TrackSpec};
use crate::ui::theme::{Theme, blend_color};
use ratatui::layout::Rect;
use ratatui::style::Color;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Loop repetition policies supported by timelines and background layers.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum LoopMode {
    #[default]
    Once,
    Infinite,
    Repeat,
    PingPong,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct LoopConfig {
    pub mode: LoopMode,
    /// Number of loops when `mode = "repeat"` or finite `"ping_pong"`. `0` = infinite for `ping_pong`.
    pub count: u32,
    /// Optional delay in milliseconds before starting each repetition.
    pub interval_ms: u64,
}

impl Default for LoopConfig {
    fn default() -> Self {
        Self {
            mode: LoopMode::Once,
            count: 0,
            interval_ms: 0,
        }
    }
}

/// Non-blocking playback clock supporting speed scaling, pause/resume/stop,
/// `once`, `infinite`, `repeat`, and `ping_pong` loop semantics.
#[derive(Debug, Clone)]
pub struct TimelineClock {
    pub duration_secs: f32,
    pub loop_cfg: LoopConfig,
    pub speed: f32,
    pub target_fps: u32,
    pub paused: bool,
    pub stopped: bool,
    pub finished: bool,
    pub total_elapsed_secs: f32,
    pub cycle_elapsed_secs: f32,
    pub interval_remaining_secs: f32,
    pub cycle_index: u32,
    pub reverse: bool,
}

impl TimelineClock {
    pub fn new(duration_ms: u64, loop_cfg: LoopConfig, speed: f32, target_fps: u32) -> Self {
        let duration_secs = if duration_ms == 0 {
            0.0
        } else {
            (duration_ms as f32 / 1000.0).max(0.02)
        };
        let finished = duration_secs <= 0.0 && loop_cfg.mode == LoopMode::Once;
        Self {
            duration_secs,
            loop_cfg,
            speed: speed.clamp(0.1, 10.0),
            target_fps: target_fps.clamp(1, 120),
            paused: false,
            stopped: false,
            finished,
            total_elapsed_secs: 0.0,
            cycle_elapsed_secs: 0.0,
            interval_remaining_secs: 0.0,
            cycle_index: 0,
            reverse: false,
        }
    }

    pub fn pause(&mut self) {
        self.paused = true;
    }

    pub fn resume(&mut self) {
        self.paused = false;
        self.stopped = false;
    }

    pub fn stop(&mut self) {
        self.stopped = true;
        self.paused = false;
        self.finished = true;
    }

    pub fn restart(&mut self) {
        self.paused = false;
        self.stopped = false;
        self.finished = self.duration_secs <= 0.0 && self.loop_cfg.mode == LoopMode::Once;
        self.total_elapsed_secs = 0.0;
        self.cycle_elapsed_secs = 0.0;
        self.interval_remaining_secs = 0.0;
        self.cycle_index = 0;
        self.reverse = false;
    }

    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed.clamp(0.1, 10.0);
    }

    pub fn advance(&mut self, dt_secs: f32) {
        if self.paused || self.stopped || self.finished || self.duration_secs <= 0.0 {
            return;
        }
        let step = dt_secs.max(0.0) * self.speed;
        self.total_elapsed_secs += step;

        if self.interval_remaining_secs > 0.0 {
            self.interval_remaining_secs = (self.interval_remaining_secs - step).max(0.0);
            if self.interval_remaining_secs > 0.0 {
                return;
            }
        }

        self.cycle_elapsed_secs += step;
        while self.cycle_elapsed_secs >= self.duration_secs {
            match self.loop_cfg.mode {
                LoopMode::Once => {
                    self.cycle_elapsed_secs = self.duration_secs;
                    self.finished = true;
                    return;
                }
                LoopMode::Infinite => {
                    self.cycle_elapsed_secs -= self.duration_secs;
                    self.cycle_index = self.cycle_index.saturating_add(1);
                    if self.loop_cfg.interval_ms > 0 {
                        self.interval_remaining_secs = self.loop_cfg.interval_ms as f32 / 1000.0;
                        self.cycle_elapsed_secs = 0.0;
                        return;
                    }
                }
                LoopMode::Repeat => {
                    let limit = self.loop_cfg.count;
                    self.cycle_index = self.cycle_index.saturating_add(1);
                    if limit > 0 && self.cycle_index >= limit {
                        self.cycle_elapsed_secs = self.duration_secs;
                        self.finished = true;
                        return;
                    }
                    self.cycle_elapsed_secs -= self.duration_secs;
                    if self.loop_cfg.interval_ms > 0 {
                        self.interval_remaining_secs = self.loop_cfg.interval_ms as f32 / 1000.0;
                        self.cycle_elapsed_secs = 0.0;
                        return;
                    }
                }
                LoopMode::PingPong => {
                    self.cycle_index = self.cycle_index.saturating_add(1);
                    if self.loop_cfg.count > 0 && self.cycle_index >= self.loop_cfg.count {
                        self.cycle_elapsed_secs = self.duration_secs;
                        self.finished = true;
                        return;
                    }
                    self.reverse = !self.reverse;
                    self.cycle_elapsed_secs -= self.duration_secs;
                    if self.loop_cfg.interval_ms > 0 {
                        self.interval_remaining_secs = self.loop_cfg.interval_ms as f32 / 1000.0;
                        self.cycle_elapsed_secs = 0.0;
                        return;
                    }
                }
            }
        }
    }

    /// Returns normalized playback time inside `[0.0, 1.0]`, accounting for ping-pong reversal.
    pub fn normalized_time(&self) -> f32 {
        if self.duration_secs <= 0.0 {
            return 1.0;
        }
        let raw = (self.cycle_elapsed_secs / self.duration_secs).clamp(0.0, 1.0);
        if self.reverse { 1.0 - raw } else { raw }
    }

    pub fn playhead_ms(&self) -> f32 {
        self.normalized_time() * (self.duration_secs * 1000.0)
    }
}

/// Evaluates all visible scene tracks (geometry, text, ASCII sprites, and particles)
/// at `clock.playhead_ms()`, producing compositable draw items sorted by layer z-index.
#[allow(clippy::too_many_arguments)]
pub fn evaluate_scene_tracks(
    def: &AnimationDefinition,
    assets: &BTreeMap<String, String>,
    clock: &TimelineClock,
    area: Rect,
    cursor_origin: Option<(u16, u16)>,
    theme: &Theme,
    truecolor: bool,
) -> Vec<SceneDrawItem> {
    let time_ms = clock.playhead_ms();
    let layer_map: BTreeMap<&str, (i32, bool, f32)> = def
        .layers
        .iter()
        .map(|l| (l.id.as_str(), (l.z_index, l.visible, l.opacity.clamp(0.0, 1.0))))
        .collect();

    let mut items = Vec::new();
    for (track_order, track) in def.tracks.iter().enumerate() {
        let (layer_z, layer_vis, layer_opacity) = track
            .layer
            .as_deref()
            .and_then(|id| layer_map.get(id).copied())
            .unwrap_or((track.z_index, true, 1.0));
        if !layer_vis || layer_opacity <= 0.01 {
            continue;
        }

        let eval = evaluate_track_state(track, time_ms, clock.duration_secs * 1000.0, area, cursor_origin, theme);
        if !eval.visible || eval.opacity * layer_opacity <= 0.01 {
            continue;
        }
        let combined_opacity = (eval.opacity * layer_opacity).clamp(0.0, 1.0);

        match track.kind {
            TrackKind::Shape => {
                let params = ShapeDrawParams {
                    center_x: eval.pos_x,
                    center_y: eval.pos_y,
                    half_w: (eval.size_w * 0.5).max(1.0),
                    half_h: (eval.size_h * 0.5).max(1.0),
                    rotation_deg: eval.rotation,
                    scale: eval.scale,
                    stroke: track.stroke,
                    fill: track.fill,
                    closed: eval.closed,
                    stroke_color: eval.color,
                    fill_color: eval.fill_color,
                    opacity: combined_opacity,
                    renderer: track.renderer,
                    stroke_char: track.stroke_char.chars().next().unwrap_or('*'),
                    fill_char: track.fill_char.chars().next().unwrap_or('#'),
                    truecolor,
                };
                items.push(SceneDrawItem::Shape {
                    layer_z,
                    track_order,
                    contour: eval.contour,
                    params,
                });
            }
            TrackKind::Text => {
                let palette: Vec<Color> = track
                    .palette
                    .iter()
                    .map(|p| resolve_scene_color(Some(p), theme, eval.color))
                    .collect();
                let chars_pool = track.chars.as_deref().unwrap_or("01ｱｶｻﾀﾅﾊ#$*");
                let text_params = TextRenderParams {
                    text: &eval.text,
                    effect: track.text_effect,
                    direction: track.direction,
                    center_x: eval.pos_x,
                    center_y: eval.pos_y,
                    progress: eval.local_progress,
                    elapsed_secs: clock.total_elapsed_secs,
                    speed: eval.speed,
                    intensity: eval.intensity,
                    opacity: combined_opacity,
                    base_color: eval.color,
                    palette: &palette,
                    bg_fallback: theme.background,
                    charset: chars_pool,
                };
                let cells = evaluate_text_cells(&text_params, area);
                items.push(SceneDrawItem::StyledRun {
                    layer_z,
                    track_order,
                    cells,
                });
            }
            TrackKind::Ascii => {
                let sprite = resolve_track_sprite(track, assets, eval.frame_index);
                if !sprite.is_empty() {
                    let cells = collect_ascii_sprite_cells(
                        sprite,
                        eval.pos_x,
                        eval.pos_y,
                        eval.scale,
                        eval.color,
                        combined_opacity,
                        area,
                    );
                    items.push(SceneDrawItem::StyledRun {
                        layer_z,
                        track_order,
                        cells,
                    });
                }
            }
            TrackKind::Particles => {
                let cells = evaluate_particle_track(
                    track,
                    &eval,
                    combined_opacity,
                    clock.total_elapsed_secs,
                    area,
                    theme,
                );
                items.push(SceneDrawItem::StyledRun {
                    layer_z,
                    track_order,
                    cells,
                });
            }
            TrackKind::Transition | TrackKind::Background => {}
        }
    }

    items
}

struct EvaluatedTrackState {
    pos_x: f32,
    pos_y: f32,
    size_w: f32,
    size_h: f32,
    color: Color,
    fill_color: Color,
    rotation: f32,
    scale: f32,
    opacity: f32,
    intensity: f32,
    speed: f32,
    visible: bool,
    contour: Vec<(f32, f32)>,
    closed: bool,
    text: String,
    frame_index: usize,
    local_progress: f32,
}

fn evaluate_track_state(
    track: &TrackSpec,
    time_ms: f32,
    total_duration_ms: f32,
    area: Rect,
    cursor_origin: Option<(u16, u16)>,
    theme: &Theme,
) -> EvaluatedTrackState {
    let base_cx = track.position[0].resolve_axis(area.x, area.width, cursor_origin.map(|c| c.0));
    let base_cy = track.position[1].resolve_axis(area.y, area.height, cursor_origin.map(|c| c.1));
    let base_sw = track.size[0].resolve_size(area.width).max(2.0);
    let base_sh = track.size[1].resolve_size(area.height).max(2.0);
    let base_color = resolve_scene_color(track.color.as_deref(), theme, theme.primary);
    let base_fill = resolve_scene_color(track.fill_color.as_deref(), theme, theme.secondary);
    let base_shape = ShapeSpec::from_primitive(
        track.primitive,
        track.sides,
        track.path.iter().map(|p| (p[0], p[1])).collect(),
    );
    let base_contour = if track.kind == TrackKind::Shape {
        base_shape.normalized_contour()
    } else {
        Vec::new()
    };

    let default_progress = if total_duration_ms > 0.0 {
        (time_ms / total_duration_ms).clamp(0.0, 1.0)
    } else {
        1.0
    };

    if track.keyframes.is_empty() {
        return EvaluatedTrackState {
            pos_x: base_cx,
            pos_y: base_cy,
            size_w: base_sw,
            size_h: base_sh,
            color: base_color,
            fill_color: base_fill,
            rotation: track.rotation,
            scale: track.scale,
            opacity: track.opacity.clamp(0.0, 1.0),
            intensity: track.intensity,
            speed: track.speed,
            visible: track.visible,
            contour: base_contour,
            closed: base_shape.closed,
            text: track.text.clone().unwrap_or_default(),
            frame_index: 0,
            local_progress: default_progress,
        };
    }

    let mut kfs: Vec<&KeyframeSpec> = track.keyframes.iter().collect();
    kfs.sort_by_key(|k| k.at_ms);

    let (k0, k1, raw_t) = find_keyframe_segment(&kfs, time_ms);
    let eased_t = k1.easing.apply(raw_t);

    let resolve_kf_pos = |k: &KeyframeSpec| -> (f32, f32) {
        let px = k
            .position
            .as_ref()
            .map(|p| p[0].resolve_axis(area.x, area.width, cursor_origin.map(|c| c.0)))
            .unwrap_or(base_cx);
        let py = k
            .position
            .as_ref()
            .map(|p| p[1].resolve_axis(area.y, area.height, cursor_origin.map(|c| c.1)))
            .unwrap_or(base_cy);
        (px, py)
    };
    let resolve_kf_size = |k: &KeyframeSpec| -> (f32, f32) {
        let sw = k
            .size
            .as_ref()
            .map(|s| s[0].resolve_size(area.width).max(2.0))
            .unwrap_or(base_sw);
        let sh = k
            .size
            .as_ref()
            .map(|s| s[1].resolve_size(area.height).max(2.0))
            .unwrap_or(base_sh);
        (sw, sh)
    };

    let (x0, y0) = resolve_kf_pos(k0);
    let (x1, y1) = resolve_kf_pos(k1);
    let (w0, h0) = resolve_kf_size(k0);
    let (w1, h1) = resolve_kf_size(k1);

    let c0 = resolve_scene_color(k0.color.as_deref(), theme, base_color);
    let c1 = resolve_scene_color(k1.color.as_deref(), theme, base_color);
    let fc0 = resolve_scene_color(k0.fill_color.as_deref(), theme, base_fill);
    let fc1 = resolve_scene_color(k1.fill_color.as_deref(), theme, base_fill);

    let rot0 = k0.rotation.unwrap_or(track.rotation);
    let rot1 = k1.rotation.unwrap_or(rot0);
    let sc0 = k0.scale.unwrap_or(track.scale);
    let sc1 = k1.scale.unwrap_or(sc0);
    let op0 = k0.opacity.unwrap_or(track.opacity);
    let op1 = k1.opacity.unwrap_or(op0);
    let int0 = k0.intensity.unwrap_or(track.intensity);
    let int1 = k1.intensity.unwrap_or(int0);
    let spd0 = k0.speed.unwrap_or(track.speed);
    let spd1 = k1.speed.unwrap_or(spd0);

    let visible = if raw_t >= 0.999 {
        k1.visible.unwrap_or(k0.visible.unwrap_or(track.visible))
    } else {
        k0.visible.unwrap_or(track.visible)
    };

    let contour = if track.kind == TrackKind::Shape {
        let s0 = ShapeSpec::from_primitive(
            k0.shape.unwrap_or(track.primitive),
            k0.sides.unwrap_or(track.sides),
            k0.path
                .as_ref()
                .map(|p| p.iter().map(|pt| (pt[0], pt[1])).collect())
                .unwrap_or_else(|| track.path.iter().map(|pt| (pt[0], pt[1])).collect()),
        );
        let s1 = ShapeSpec::from_primitive(
            k1.shape.unwrap_or(s0.primitive),
            k1.sides.unwrap_or(s0.sides),
            k1.path
                .as_ref()
                .map(|p| p.iter().map(|pt| (pt[0], pt[1])).collect())
                .unwrap_or_else(|| s0.custom_points.clone()),
        );
        match k1.transition {
            MorphTransitionMode::Snap => {
                if raw_t >= 0.999 {
                    s1.normalized_contour()
                } else {
                    s0.normalized_contour()
                }
            }
            MorphTransitionMode::Morph | MorphTransitionMode::Fade => {
                morph_contours(&s0.normalized_contour(), &s1.normalized_contour(), eased_t)
            }
        }
    } else {
        Vec::new()
    };

    let text = if raw_t >= 0.999 {
        k1.text
            .clone()
            .or_else(|| k0.text.clone())
            .or_else(|| track.text.clone())
            .unwrap_or_default()
    } else {
        k0.text
            .clone()
            .or_else(|| track.text.clone())
            .unwrap_or_default()
    };

    let f0 = k0.frame.unwrap_or(0);
    let f1 = k1.frame.unwrap_or(f0);
    let frame_index = (f0 as f32 + (f1 as f32 - f0 as f32) * raw_t).round() as usize;

    EvaluatedTrackState {
        pos_x: lerp(x0, x1, eased_t),
        pos_y: lerp(y0, y1, eased_t),
        size_w: lerp(w0, w1, eased_t),
        size_h: lerp(h0, h1, eased_t),
        color: blend_color(c1, c0, eased_t),
        fill_color: blend_color(fc1, fc0, eased_t),
        rotation: lerp(rot0, rot1, eased_t),
        scale: lerp(sc0, sc1, eased_t),
        opacity: lerp(op0, op1, eased_t).clamp(0.0, 1.0),
        intensity: lerp(int0, int1, eased_t),
        speed: lerp(spd0, spd1, eased_t),
        visible,
        contour,
        closed: base_shape.closed,
        text,
        frame_index,
        local_progress: raw_t.clamp(0.0, 1.0),
    }
}

fn find_keyframe_segment<'a>(
    kfs: &'a [&'a KeyframeSpec],
    time_ms: f32,
) -> (&'a KeyframeSpec, &'a KeyframeSpec, f32) {
    if kfs.len() == 1 || time_ms <= kfs[0].at_ms as f32 {
        return (kfs[0], kfs[0], 0.0);
    }
    for pair in kfs.windows(2) {
        let a = pair[0];
        let b = pair[1];
        if time_ms <= b.at_ms as f32 {
            let span = (b.at_ms.saturating_sub(a.at_ms)) as f32;
            let raw_t = if span <= 0.0 {
                1.0
            } else {
                ((time_ms - a.at_ms as f32) / span).clamp(0.0, 1.0)
            };
            return (a, b, raw_t);
        }
    }
    let last = kfs[kfs.len() - 1];
    (last, last, 1.0)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn resolve_track_sprite<'a>(
    track: &'a TrackSpec,
    assets: &'a BTreeMap<String, String>,
    frame_idx: usize,
) -> &'a str {
    if !track.frames.is_empty() {
        return track.frames[frame_idx % track.frames.len()].as_str();
    }
    if let Some(asset_key) = &track.asset
        && let Some(content) = assets.get(asset_key)
    {
        return content.as_str();
    }
    track.sprite.as_deref().unwrap_or("")
}

fn evaluate_particle_track(
    track: &TrackSpec,
    eval: &EvaluatedTrackState,
    opacity: f32,
    elapsed_secs: f32,
    area: Rect,
    theme: &Theme,
) -> Vec<SceneCell> {
    let count = track.count.clamp(1, 500);
    let chars: Vec<char> = track
        .chars
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or("·•✦*+∘")
        .chars()
        .collect();
    let alt_color = resolve_scene_color(track.fill_color.as_deref(), theme, theme.accent);
    let half_w = (eval.size_w * 0.5).max(4.0);
    let half_h = (eval.size_h * 0.5).max(3.0);
    let t = elapsed_secs * eval.speed.max(0.1);

    let mut cells = Vec::with_capacity(count);
    for i in 0..count {
        let seed_a = ((i * 7919 + 17) % 1000) as f32 / 1000.0;
        let seed_b = ((i * 6271 + 53) % 1000) as f32 / 1000.0;
        let angle = seed_a * std::f32::consts::TAU + t * 0.35 * (if i % 2 == 0 { 1.0 } else { -1.0 });

        let (px, py) = match track.direction {
            TextDirection::Up => {
                let y_prog = (seed_b - t * 0.25).rem_euclid(1.0);
                (
                    eval.pos_x + (seed_a - 0.5) * 2.0 * half_w + (t + seed_a * 4.0).sin() * 1.5,
                    eval.pos_y - half_h + y_prog * 2.0 * half_h,
                )
            }
            TextDirection::Down => {
                let y_prog = (seed_b + t * 0.25).rem_euclid(1.0);
                (
                    eval.pos_x + (seed_a - 0.5) * 2.0 * half_w,
                    eval.pos_y - half_h + y_prog * 2.0 * half_h,
                )
            }
            TextDirection::Left => {
                let x_prog = (seed_a - t * 0.25).rem_euclid(1.0);
                (
                    eval.pos_x - half_w + x_prog * 2.0 * half_w,
                    eval.pos_y + (seed_b - 0.5) * 2.0 * half_h,
                )
            }
            TextDirection::Right => {
                let x_prog = (seed_a + t * 0.25).rem_euclid(1.0);
                (
                    eval.pos_x - half_w + x_prog * 2.0 * half_w,
                    eval.pos_y + (seed_b - 0.5) * 2.0 * half_h,
                )
            }
            TextDirection::Center => {
                let rad = ((seed_b + t * 0.35).fract()) * eval.scale.max(0.2);
                (
                    eval.pos_x + angle.cos() * half_w * rad,
                    eval.pos_y + angle.sin() * half_h * rad,
                )
            }
        };

        let xi = px.round() as i32;
        let yi = py.round() as i32;
        if xi < area.x as i32
            || xi >= area.right() as i32
            || yi < area.y as i32
            || yi >= area.bottom() as i32
        {
            continue;
        }
        let ch = chars[(i + (t * 2.0) as usize) % chars.len()];
        let fg = if i % 3 == 0 {
            blend_color(alt_color, eval.color, 0.7)
        } else {
            eval.color
        };
        cells.push(SceneCell {
            x: xi as u16,
            y: yi as u16,
            ch,
            fg,
            bg: None,
            opacity,
        });
    }
    cells
}
