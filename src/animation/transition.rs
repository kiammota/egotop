use crate::animation::backgrounds::BackgroundLayerSpec;
use crate::animation::scene::composite_scene_items;
use crate::animation::timeline::{LoopMode, TimelineClock, evaluate_scene_tracks};
use crate::config::{
    AnimationDefinition, AnimationPackage, AnimationRenderMode, DeclarativeStepEffect,
    PackageOrigin, StepDirection, StepTarget, TrackKind, TrackSpec, load_community_packages,
};
use crate::ui::theme::{Theme, adapt_color, blend_color, parse_color, terminal_supports_truecolor};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use std::collections::BTreeMap;

/// Built-in preset identifiers preserved for backwards compatibility with existing API callers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeTransition {
    CrtPowerOn,
    Curtains,
    Fade,
    Slide,
    MatrixBoot,
    CursorExpand,
    None,
}

impl NativeTransition {
    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "crt" | "crt_power_on" | "crtpoweron" => Some(Self::CrtPowerOn),
            "curtains" | "curtain" => Some(Self::Curtains),
            "fade" => Some(Self::Fade),
            "slide" => Some(Self::Slide),
            "matrix_boot" | "matrixboot" | "boot" => Some(Self::MatrixBoot),
            "cursor_expand" | "cursorexpand" | "expand" => Some(Self::CursorExpand),
            "none" | "off" => Some(Self::None),
            _ => None,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::CrtPowerOn => "crt_power_on",
            Self::Curtains => "curtains",
            Self::Fade => "fade",
            Self::Slide => "slide",
            Self::MatrixBoot => "matrix_boot",
            Self::CursorExpand => "cursor-expand",
            Self::None => "none",
        }
    }

    pub fn all() -> &'static [Self] {
        &[
            Self::CrtPowerOn,
            Self::Curtains,
            Self::Fade,
            Self::Slide,
            Self::MatrixBoot,
            Self::CursorExpand,
            Self::None,
        ]
    }
}

#[derive(Debug, Clone)]
pub struct AnimationCatalogEntry {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub is_community: bool,
    pub package: AnimationPackage,
}

#[derive(Debug, Clone)]
pub struct AnimationCatalog {
    pub entries: Vec<AnimationCatalogEntry>,
}

impl AnimationCatalog {
    /// Loads all bundled and community animation packages through the single unified parser.
    pub fn load() -> Self {
        let mut by_id: BTreeMap<String, AnimationCatalogEntry> = BTreeMap::new();

        for pkg in bundled_animation_packages() {
            let entry = catalog_entry_from_package(pkg, false);
            by_id.insert(entry.id.to_ascii_lowercase(), entry);
        }

        for (_key, pkg) in load_community_packages() {
            let entry = catalog_entry_from_package(pkg, true);
            by_id.insert(entry.id.to_ascii_lowercase(), entry);
        }

        if let Some(template_pkg) = by_id.get("none").map(|e| e.package.clone()) {
            for lua_name in crate::lua::list_lua_animations() {
                let key = lua_name.to_ascii_lowercase();
                by_id.entry(key).or_insert_with(|| {
                    let mut pkg = template_pkg.clone();
                    pkg.definition.name = lua_name.clone();
                    pkg.definition.display_name = Some(lua_name.clone());
                    pkg.definition.description =
                        Some(format!("Lua animation module 'animations.{lua_name}'"));
                    AnimationCatalogEntry {
                        id: lua_name.clone(),
                        display_name: lua_name.clone(),
                        description: format!("Lua animation module 'animations.{lua_name}'"),
                        is_community: true,
                        package: pkg,
                    }
                });
            }
        }

        Self {
            entries: by_id.into_values().collect(),
        }
    }

    pub fn resolve_package(&self, name: &str) -> AnimationPackage {
        let needle = name.trim().to_ascii_lowercase();
        let canonical = NativeTransition::from_name(name)
            .map(|n| n.id().to_ascii_lowercase())
            .unwrap_or_else(|| needle.clone());

        if let Some(found) = self
            .entries
            .iter()
            .find(|e| e.id.eq_ignore_ascii_case(&needle) || e.id.eq_ignore_ascii_case(&canonical))
        {
            return found.package.clone();
        }
        if let Some(fallback) = self
            .entries
            .iter()
            .find(|e| e.id.eq_ignore_ascii_case("matrix_boot"))
        {
            return fallback.package.clone();
        }
        bundled_animation_packages()
            .into_iter()
            .next()
            .expect("at least one bundled animation exists")
    }
}

fn catalog_entry_from_package(pkg: AnimationPackage, is_community: bool) -> AnimationCatalogEntry {
    let def = &pkg.definition;
    let display_name = def.display_name.clone().unwrap_or_else(|| def.name.clone());
    let description = def.description.clone().unwrap_or_else(|| {
        format!(
            "Declarative animation ({} tracks, {:.1}s)",
            def.tracks.len(),
            def.duration_ms as f32 / 1000.0
        )
    });
    AnimationCatalogEntry {
        id: def.name.clone(),
        display_name,
        description,
        is_community,
        package: pkg,
    }
}

fn bundled_animation_packages() -> Vec<AnimationPackage> {
    struct BundledSpec {
        manifest: &'static str,
        assets: &'static [(&'static str, &'static str)],
    }

    const SPECS: &[BundledSpec] = &[
        BundledSpec {
            manifest: include_str!("../../assets/animations/crt_power_on.toml"),
            assets: &[],
        },
        BundledSpec {
            manifest: include_str!("../../assets/animations/curtains.toml"),
            assets: &[],
        },
        BundledSpec {
            manifest: include_str!("../../assets/animations/fade.toml"),
            assets: &[],
        },
        BundledSpec {
            manifest: include_str!("../../assets/animations/slide.toml"),
            assets: &[],
        },
        BundledSpec {
            manifest: include_str!("../../assets/animations/matrix_boot.toml"),
            assets: &[],
        },
        BundledSpec {
            manifest: include_str!("../../assets/animations/cursor_expand.toml"),
            assets: &[],
        },
        BundledSpec {
            manifest: include_str!("../../assets/animations/none.toml"),
            assets: &[],
        },
        BundledSpec {
            manifest: include_str!("../../assets/animations/cyber_boot.toml"),
            assets: &[],
        },
        BundledSpec {
            manifest: include_str!("../../assets/animations/neon_curtains.toml"),
            assets: &[],
        },
        BundledSpec {
            manifest: include_str!("../../assets/animations/shape-morph/animation.toml"),
            assets: &[(
                "assets/frame_ring.txt",
                include_str!("../../assets/animations/shape-morph/assets/frame_ring.txt"),
            )],
        },
        BundledSpec {
            manifest: include_str!("../../assets/animations/orbital-prism/animation.toml"),
            assets: &[],
        },
        BundledSpec {
            manifest: include_str!("../../assets/animations/neon-banner/animation.toml"),
            assets: &[(
                "assets/emblem.txt",
                include_str!("../../assets/animations/neon-banner/assets/emblem.txt"),
            )],
        },
        BundledSpec {
            manifest: include_str!("../../assets/animations/aurora-horizon/animation.toml"),
            assets: &[],
        },
        BundledSpec {
            manifest: include_str!("../../assets/animations/matrix-overload/animation.toml"),
            assets: &[(
                "assets/core.txt",
                include_str!("../../assets/animations/matrix-overload/assets/core.txt"),
            )],
        },
        BundledSpec {
            manifest: include_str!("../../assets/animations/starlight-warp/animation.toml"),
            assets: &[],
        },
        BundledSpec {
            manifest: include_str!("../../assets/animations/pulse-scanner/animation.toml"),
            assets: &[],
        },
    ];

    SPECS
        .iter()
        .filter_map(|spec| {
            let def = AnimationDefinition::parse_toml(spec.manifest).ok()?;
            let mut assets = BTreeMap::new();
            for &(k, v) in spec.assets {
                assets.insert(k.to_string(), v.to_string());
            }
            Some(AnimationPackage {
                definition: def,
                origin: PackageOrigin::Bundled,
                assets,
                has_preview: false,
            })
        })
        .collect()
}

/// Unified Animation & Scene Player used identically for startup transitions and full scenes.
#[derive(Debug, Clone)]
pub struct TransitionPlayer {
    pub active_id: String,
    pub package: AnimationPackage,
    pub clock: TimelineClock,
    pub elapsed_secs: f32,
    pub duration_secs: f32,
    pub finished: bool,
    pub paused: bool,
    pub origin: Option<(u16, u16)>,
    pub truecolor: bool,
}

impl TransitionPlayer {
    pub fn new(id: &str, catalog: &AnimationCatalog, default_duration_ms: u64) -> Self {
        let mut package = catalog.resolve_package(id);
        let def = &mut package.definition;
        let is_single_transition = def.mode == AnimationRenderMode::Transition
            && def.tracks.len() == 1
            && def.tracks[0].kind == TrackKind::Transition
            && def.loop_cfg.mode == LoopMode::Once;

        let effective_duration_ms = if def.name.eq_ignore_ascii_case("none") {
            0
        } else if is_single_transition && default_duration_ms > 0 {
            let d = default_duration_ms.max(200);
            def.duration_ms = d;
            def.tracks[0].duration_ms = Some(d);
            d
        } else {
            def.duration_ms
        };

        let clock = TimelineClock::new(
            effective_duration_ms,
            def.loop_cfg.clone(),
            def.speed,
            def.fps,
        );
        let duration_secs = clock.duration_secs;
        let finished = clock.finished;

        Self {
            active_id: def.name.clone(),
            package,
            clock,
            elapsed_secs: 0.0,
            duration_secs,
            finished,
            paused: false,
            origin: None,
            truecolor: terminal_supports_truecolor(),
        }
    }

    pub fn restart(&mut self, id: &str, catalog: &AnimationCatalog, default_duration_ms: u64) {
        let prev_origin = self.origin;
        *self = Self::new(id, catalog, default_duration_ms);
        self.origin = prev_origin;
    }

    pub fn restart_from_origin(
        &mut self,
        id: &str,
        catalog: &AnimationCatalog,
        default_duration_ms: u64,
        origin: (u16, u16),
    ) {
        *self = Self::new(id, catalog, default_duration_ms);
        self.origin = Some(origin);
    }

    pub fn pause(&mut self) {
        self.paused = true;
        self.clock.pause();
    }

    pub fn resume(&mut self) {
        self.paused = false;
        self.clock.resume();
    }

    pub fn stop(&mut self) {
        self.paused = false;
        self.clock.stop();
        self.finished = true;
    }

    pub fn replay_current(&mut self) {
        self.paused = false;
        self.clock.restart();
        self.elapsed_secs = 0.0;
        self.finished = self.clock.finished;
    }

    pub fn set_speed(&mut self, speed: f32) {
        self.clock.set_speed(speed);
    }

    pub fn speed(&self) -> f32 {
        self.clock.speed
    }

    pub fn update(&mut self, dt_secs: f32) {
        if self.paused || self.finished {
            return;
        }
        self.clock.advance(dt_secs);
        self.elapsed_secs = self.clock.total_elapsed_secs;
        self.finished = self.clock.finished;
    }

    pub fn progress(&self) -> f32 {
        if self.finished || self.duration_secs <= 0.0 {
            1.0
        } else {
            self.clock.normalized_time()
        }
    }

    pub fn has_scene_tracks(&self) -> bool {
        !self.finished
            && self
                .package
                .definition
                .tracks
                .iter()
                .any(|t| !matches!(t.kind, TrackKind::Transition | TrackKind::Background))
    }

    pub fn package_backgrounds(&self) -> &[BackgroundLayerSpec] {
        &self.package.definition.backgrounds
    }

    pub fn render_scene(&self, buf: &mut Buffer, area: Rect, theme: &Theme) {
        if self.finished || area.width == 0 || area.height == 0 || !self.has_scene_tracks() {
            return;
        }
        let items = evaluate_scene_tracks(
            &self.package.definition,
            &self.package.assets,
            &self.clock,
            area,
            self.origin,
            theme,
            self.truecolor,
        );
        composite_scene_items(items, area, buf, self.truecolor);
    }

    /// Applies transition-step post-processing onto `window_area` (for startup & wipe effects).
    pub fn apply_to_buffer(&self, buf: &mut Buffer, window_area: Rect, theme: &Theme) {
        if self.finished || window_area.width == 0 || window_area.height == 0 {
            return;
        }
        let time_ms = self.clock.playhead_ms();
        let def = &self.package.definition;

        let active_tracks: Vec<(&TrackSpec, f32)> = def
            .tracks
            .iter()
            .filter(|t| t.kind == TrackKind::Transition && t.visible)
            .filter_map(|t| {
                let dur = t.duration_ms.unwrap_or(def.duration_ms);
                if dur == 0 {
                    return None;
                }
                let start = t.start_ms as f32;
                let end = start + dur as f32;
                if time_ms >= start && time_ms <= end {
                    let local = ((time_ms - start) / dur as f32).clamp(0.0, 1.0);
                    Some((t, local))
                } else {
                    None
                }
            })
            .collect();

        for (track, local_t) in active_tracks {
            let eased = track.transition_easing.apply(local_t);
            let accent = track.color.as_deref();
            let area = if track.target == StepTarget::Screen {
                *buf.area()
            } else {
                window_area
            };
            match track.effect.unwrap_or(DeclarativeStepEffect::Fade) {
                DeclarativeStepEffect::CrtLine => {
                    apply_crt_power_on(buf, area, eased, theme, accent, self.truecolor);
                }
                DeclarativeStepEffect::Curtains => {
                    apply_curtains(
                        buf,
                        area,
                        eased,
                        track.transition_direction,
                        theme,
                        accent,
                        self.truecolor,
                    );
                }
                DeclarativeStepEffect::Fade => {
                    apply_fade(buf, area, eased, theme, self.truecolor);
                }
                DeclarativeStepEffect::Slide => {
                    apply_slide(buf, area, eased, track.transition_direction, theme);
                }
                DeclarativeStepEffect::MatrixScramble => {
                    let chars = track.chars.as_deref().unwrap_or("01ｱｶｻﾀﾅﾊ#$");
                    apply_matrix_scramble(buf, area, eased, chars, theme, self.truecolor);
                }
                DeclarativeStepEffect::Flash => {
                    apply_flash(buf, area, eased, theme, accent, track.intensity, self.truecolor);
                }
                DeclarativeStepEffect::CursorExpand => {
                    apply_cursor_expand(buf, area, self.origin, eased, theme, self.truecolor);
                }
                DeclarativeStepEffect::Pause | DeclarativeStepEffect::Unknown => {}
            }
        }
    }
}

fn apply_crt_power_on(
    buf: &mut Buffer,
    area: Rect,
    t: f32,
    theme: &Theme,
    color_override: Option<&str>,
    truecolor: bool,
) {
    let beam_color = adapt_color(
        color_override
            .map(|c| parse_color(c, theme.primary))
            .unwrap_or(theme.primary),
        truecolor,
    );
    let cy = area.y + area.height / 2;
    let cx = area.x + area.width / 2;

    if t < 0.35 {
        let h_prog = crate::config::EasingKind::EaseOut.apply(t / 0.35);
        let half_w = ((area.width as f32 * 0.5) * h_prog).round() as u16;
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    if y == cy && x >= cx.saturating_sub(half_w) && x <= cx.saturating_add(half_w) {
                        cell.set_char('━')
                            .set_style(Style::default().fg(beam_color).bg(theme.background));
                    } else {
                        cell.set_char(' ').set_bg(theme.background);
                    }
                }
            }
        }
    } else {
        let v_prog = crate::config::EasingKind::EaseInOut.apply((t - 0.35) / 0.65);
        let half_h = ((area.height as f32 * 0.5) * v_prog).ceil() as u16;
        let top_vis = cy.saturating_sub(half_h);
        let bot_vis = cy.saturating_add(half_h);
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                if y < top_vis || y > bot_vis {
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        cell.set_char(' ').set_bg(theme.background);
                    }
                } else if (y == top_vis || y == bot_vis)
                    && let Some(cell) = buf.cell_mut((x, y))
                {
                    cell.set_fg(beam_color);
                }
            }
        }
    }
}

fn apply_curtains(
    buf: &mut Buffer,
    area: Rect,
    t: f32,
    direction: StepDirection,
    theme: &Theme,
    color_override: Option<&str>,
    truecolor: bool,
) {
    let eased = crate::config::EasingKind::EaseInOut.apply(t);
    let edge_color = adapt_color(
        color_override
            .map(|c| parse_color(c, theme.secondary))
            .unwrap_or(theme.secondary),
        truecolor,
    );
    let half_w = area.width / 2;
    let open_cols = ((half_w as f32) * eased).round() as u16;
    let cx = area.x + half_w;

    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let visible = match direction {
                StepDirection::Left => (x - area.x) < ((area.width as f32) * eased).round() as u16,
                StepDirection::Right => {
                    (area.right() - 1 - x) < ((area.width as f32) * eased).round() as u16
                }
                _ => {
                    let dist = x.abs_diff(cx);
                    dist <= open_cols
                }
            };
            if !visible {
                let is_seam = x.abs_diff(cx) == open_cols + 1;
                if let Some(cell) = buf.cell_mut((x, y)) {
                    if is_seam {
                        cell.set_char('│')
                            .set_style(Style::default().fg(edge_color).bg(theme.background));
                    } else {
                        cell.set_char('▒')
                            .set_style(Style::default().fg(theme.border).bg(theme.background));
                    }
                }
            }
        }
    }
}

fn apply_fade(buf: &mut Buffer, area: Rect, alpha: f32, theme: &Theme, truecolor: bool) {
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if let Some(cell) = buf.cell_mut((x, y)) {
                let fg = adapt_color(blend_color(cell.fg, theme.background, alpha), truecolor);
                let bg = adapt_color(blend_color(cell.bg, theme.background, alpha), truecolor);
                cell.set_fg(fg).set_bg(bg);
            }
        }
    }
}

fn apply_slide(
    buf: &mut Buffer,
    area: Rect,
    eased: f32,
    direction: StepDirection,
    theme: &Theme,
) {
    let shift_x = ((area.width as f32) * (1.0 - eased)).round() as u16;
    let shift_y = ((area.height as f32) * (1.0 - eased)).round() as u16;

    let mut snapshot = Vec::with_capacity((area.width as usize) * (area.height as usize));
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            snapshot.push(buf.cell((x, y)).cloned());
        }
    }

    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let rel_x = x - area.x;
            let rel_y = y - area.y;

            let src_coords: Option<(u16, u16)> = match direction {
                StepDirection::Right => rel_x.checked_sub(shift_x).map(|sx| (sx, rel_y)),
                StepDirection::Top => rel_y.checked_sub(shift_y).map(|sy| (rel_x, sy)),
                StepDirection::Bottom => {
                    let sy = rel_y + shift_y;
                    (sy < area.height).then_some((rel_x, sy))
                }
                _ => {
                    let sx = rel_x + shift_x;
                    (sx < area.width).then_some((sx, rel_y))
                }
            };

            if let Some(cell) = buf.cell_mut((x, y)) {
                if let Some((sx, sy)) = src_coords {
                    let idx = (sy as usize) * (area.width as usize) + (sx as usize);
                    if let Some(Some(src)) = snapshot.get(idx) {
                        *cell = src.clone();
                    }
                } else {
                    cell.set_char(' ').set_bg(theme.background);
                }
            }
        }
    }
}

fn apply_matrix_scramble(
    buf: &mut Buffer,
    area: Rect,
    t: f32,
    charset_str: &str,
    theme: &Theme,
    truecolor: bool,
) {
    let glyphs: Vec<char> = if charset_str.is_empty() {
        vec!['0', '1', '#', '$']
    } else {
        charset_str.chars().collect()
    };
    let total_rows = area.height.max(1) as f32;
    let reveal_rows = total_rows * crate::config::EasingKind::EaseInOut.apply(t);
    let color = adapt_color(theme.primary, truecolor);

    for y in area.top()..area.bottom() {
        let rel_y = (y - area.y) as f32;
        for x in area.left()..area.right() {
            if rel_y < reveal_rows {
                continue;
            }
            if let Some(cell) = buf.cell_mut((x, y)) {
                let sym = cell.symbol();
                if sym == " " {
                    continue;
                }
                if rel_y < reveal_rows + 2.0 {
                    let hash =
                        ((x as usize) * 31 + (y as usize) * 17 + (t * 97.0) as usize) % glyphs.len();
                    cell.set_char(glyphs[hash]).set_fg(color);
                } else {
                    cell.set_char(' ');
                }
            }
        }
    }
}

fn apply_cursor_expand(
    buf: &mut Buffer,
    area: Rect,
    origin: Option<(u16, u16)>,
    t: f32,
    theme: &Theme,
    truecolor: bool,
) {
    let eased = crate::config::EasingKind::EaseOut.apply(t);
    let (ox, oy) = origin.unwrap_or((area.x + area.width / 2, area.y + area.height / 2));

    let max_dx = ox.abs_diff(area.x).max(ox.abs_diff(area.right().saturating_sub(1))) as f32;
    let max_dy = oy.abs_diff(area.y).max(oy.abs_diff(area.bottom().saturating_sub(1))) as f32;
    let max_radius = (max_dx * max_dx + (max_dy * 2.0) * (max_dy * 2.0))
        .sqrt()
        .max(1.0);
    let cur_radius = max_radius * eased;
    let ring_color = adapt_color(theme.primary, truecolor);

    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let dx = x as f32 - ox as f32;
            let dy = (y as f32 - oy as f32) * 2.0;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist > cur_radius {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(' ').set_bg(theme.background);
                }
            } else if dist > (cur_radius - 2.2).max(0.0)
                && let Some(cell) = buf.cell_mut((x, y))
            {
                cell.set_fg(ring_color);
            }
        }
    }
}

fn apply_flash(
    buf: &mut Buffer,
    area: Rect,
    t: f32,
    theme: &Theme,
    color_override: Option<&str>,
    intensity: f32,
    truecolor: bool,
) {
    let flash_color = color_override
        .map(|c| parse_color(c, theme.primary))
        .unwrap_or(theme.primary);
    let strength = ((1.0 - (t * 2.0 - 1.0).abs()).clamp(0.0, 1.0) * 0.65 * intensity).clamp(0.0, 1.0);
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if let Some(cell) = buf.cell_mut((x, y)) {
                let c = adapt_color(blend_color(flash_color, cell.fg, strength), truecolor);
                cell.set_fg(c);
            }
        }
    }
}
