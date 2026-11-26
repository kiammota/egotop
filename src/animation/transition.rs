use crate::config::{
    CommunityAnimationEntry, DeclarativeAnimation, DeclarativeStep, DeclarativeStepEffect,
    EasingKind, StepDirection, load_community_animations,
};
use crate::ui::theme::{Theme, blend_color, parse_color};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeTransition {
    CrtPowerOn,
    Curtains,
    Fade,
    Slide,
    MatrixBoot,
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
            Self::None => "none",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::CrtPowerOn => "CRT Power On",
            Self::Curtains => "Curtains",
            Self::Fade => "Fade",
            Self::Slide => "Slide",
            Self::MatrixBoot => "Matrix Boot",
            Self::None => "None",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::CrtPowerOn => "Linha horizontal de TV antiga expandindo para o quadro completo.",
            Self::Curtains => "Cortinas simétricas abrindo para revelar a interface.",
            Self::Fade => "Aparecimento gradual suave até as cores finais.",
            Self::Slide => "Entrada lateral progressiva da janela principal.",
            Self::MatrixBoot => "Sequência de inicialização com caracteres resolvendo o conteúdo.",
            Self::None => "Exibição instantânea sem transição inicial.",
        }
    }

    pub fn all() -> &'static [Self] {
        &[
            Self::CrtPowerOn,
            Self::Curtains,
            Self::Fade,
            Self::Slide,
            Self::MatrixBoot,
            Self::None,
        ]
    }
}

#[derive(Debug, Clone)]
pub enum AnimationSource {
    Native(NativeTransition),
    Community(DeclarativeAnimation),
}

#[derive(Debug, Clone)]
pub struct AnimationCatalogEntry {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub is_community: bool,
    pub source: AnimationSource,
}

#[derive(Debug, Clone)]
pub struct AnimationCatalog {
    pub entries: Vec<AnimationCatalogEntry>,
}

impl AnimationCatalog {
    pub fn load() -> Self {
        let mut entries = Vec::new();
        for &native in NativeTransition::all() {
            entries.push(AnimationCatalogEntry {
                id: native.id().to_string(),
                display_name: native.display_name().to_string(),
                description: native.description().to_string(),
                is_community: false,
                source: AnimationSource::Native(native),
            });
        }

        let community: BTreeMap<String, CommunityAnimationEntry> = load_community_animations();
        for (_key, item) in community {
            let spec = item.spec;
            let display_name = spec
                .display_name
                .clone()
                .unwrap_or_else(|| spec.name.clone());
            let description = spec.description.clone().unwrap_or_else(|| {
                format!(
                    "Animação comunitária ({} passos, {:.1}s)",
                    spec.steps.len(),
                    spec.total_duration_secs()
                )
            });
            entries.push(AnimationCatalogEntry {
                id: spec.name.clone(),
                display_name,
                description,
                is_community: true,
                source: AnimationSource::Community(spec),
            });
        }

        Self { entries }
    }

    pub fn resolve(&self, name: &str) -> AnimationSource {
        let needle = name.trim().to_ascii_lowercase();
        if let Some(found) = self
            .entries
            .iter()
            .find(|e| e.id.to_ascii_lowercase() == needle)
        {
            return found.source.clone();
        }
        if let Some(native) = NativeTransition::from_name(name) {
            return AnimationSource::Native(native);
        }
        AnimationSource::Native(NativeTransition::MatrixBoot)
    }
}

#[derive(Debug, Clone)]
pub struct TransitionPlayer {
    pub active_id: String,
    pub source: AnimationSource,
    pub elapsed_secs: f32,
    pub duration_secs: f32,
    pub finished: bool,
}

impl TransitionPlayer {
    pub fn new(id: &str, catalog: &AnimationCatalog, default_duration_ms: u64) -> Self {
        let source = catalog.resolve(id);
        let duration_secs = match &source {
            AnimationSource::Native(NativeTransition::None) => 0.0,
            AnimationSource::Native(_) => (default_duration_ms.max(200) as f32) / 1000.0,
            AnimationSource::Community(spec) => spec.total_duration_secs(),
        };
        let finished = duration_secs <= 0.0;
        Self {
            active_id: id.to_string(),
            source,
            elapsed_secs: 0.0,
            duration_secs,
            finished,
        }
    }

    pub fn restart(&mut self, id: &str, catalog: &AnimationCatalog, default_duration_ms: u64) {
        *self = Self::new(id, catalog, default_duration_ms);
    }

    pub fn update(&mut self, dt_secs: f32) {
        if self.finished {
            return;
        }
        self.elapsed_secs += dt_secs.max(0.0);
        if self.elapsed_secs >= self.duration_secs {
            self.elapsed_secs = self.duration_secs;
            self.finished = true;
        }
    }

    pub fn progress(&self) -> f32 {
        if self.finished || self.duration_secs <= 0.0 {
            1.0
        } else {
            (self.elapsed_secs / self.duration_secs).clamp(0.0, 1.0)
        }
    }

    /// Post-processes the rendered Ratatui buffer inside `window_area` to apply
    /// the active startup/transition animation cleanly at frame level.
    pub fn apply_to_buffer(&self, buf: &mut Buffer, window_area: Rect, theme: &Theme) {
        if self.finished || window_area.width == 0 || window_area.height == 0 {
            return;
        }

        match &self.source {
            AnimationSource::Native(native) => {
                let t = self.progress();
                match native {
                    NativeTransition::None => {}
                    NativeTransition::CrtPowerOn => apply_crt_power_on(buf, window_area, t, theme, None),
                    NativeTransition::Curtains => apply_curtains(buf, window_area, t, StepDirection::Center, theme, None),
                    NativeTransition::Fade => apply_fade(buf, window_area, EasingKind::EaseOut.apply(t), theme),
                    NativeTransition::Slide => apply_slide(buf, window_area, EasingKind::EaseOut.apply(t), StepDirection::Left, theme),
                    NativeTransition::MatrixBoot => apply_matrix_scramble(buf, window_area, t, "01ｱｶｻﾀﾅﾊ#$", theme),
                }
            }
            AnimationSource::Community(spec) => {
                if let Some((step, local_t)) = self.active_declarative_step(spec) {
                    let eased = step.easing.apply(local_t);
                    let accent = step.color.as_deref();
                    match step.effect {
                        DeclarativeStepEffect::CrtLine => {
                            apply_crt_power_on(buf, window_area, eased, theme, accent);
                        }
                        DeclarativeStepEffect::Curtains => {
                            apply_curtains(buf, window_area, eased, step.direction, theme, accent);
                        }
                        DeclarativeStepEffect::Fade => {
                            apply_fade(buf, window_area, eased, theme);
                        }
                        DeclarativeStepEffect::Slide => {
                            apply_slide(buf, window_area, eased, step.direction, theme);
                        }
                        DeclarativeStepEffect::MatrixScramble => {
                            let chars = step.chars.as_deref().unwrap_or("01ｱｶｻﾀﾅﾊ#$");
                            apply_matrix_scramble(buf, window_area, eased, chars, theme);
                        }
                        DeclarativeStepEffect::Flash => {
                            apply_flash(buf, window_area, eased, theme, accent);
                        }
                        DeclarativeStepEffect::Pause | DeclarativeStepEffect::Unknown => {}
                    }
                }
            }
        }
    }

    fn active_declarative_step<'a>(
        &self,
        spec: &'a DeclarativeAnimation,
    ) -> Option<(&'a DeclarativeStep, f32)> {
        let mut cursor_ms = self.elapsed_secs * 1000.0;
        for step in &spec.steps {
            let dur = step.duration_ms.max(1) as f32;
            if cursor_ms <= dur {
                return Some((step, (cursor_ms / dur).clamp(0.0, 1.0)));
            }
            cursor_ms -= dur;
        }
        None
    }
}

fn apply_crt_power_on(
    buf: &mut Buffer,
    area: Rect,
    t: f32,
    theme: &Theme,
    color_override: Option<&str>,
) {
    let beam_color = color_override
        .map(|c| parse_color(c, theme.primary))
        .unwrap_or(theme.primary);
    let cy = area.y + area.height / 2;
    let cx = area.x + area.width / 2;

    if t < 0.35 {
        let h_prog = EasingKind::EaseOut.apply(t / 0.35);
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
        let v_prog = EasingKind::EaseInOut.apply((t - 0.35) / 0.65);
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
) {
    let eased = EasingKind::EaseInOut.apply(t);
    let edge_color = color_override
        .map(|c| parse_color(c, theme.secondary))
        .unwrap_or(theme.secondary);
    let half_w = area.width / 2;
    let open_cols = ((half_w as f32) * eased).round() as u16;
    let cx = area.x + half_w;

    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let visible = match direction {
                StepDirection::Left => (x - area.x) < ((area.width as f32) * eased).round() as u16,
                StepDirection::Right => (area.right() - 1 - x) < ((area.width as f32) * eased).round() as u16,
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

fn apply_fade(buf: &mut Buffer, area: Rect, alpha: f32, theme: &Theme) {
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if let Some(cell) = buf.cell_mut((x, y)) {
                let fg = blend_color(cell.fg, theme.background, alpha);
                let bg = blend_color(cell.bg, theme.background, alpha);
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

    // Snapshot original area cells to shift cleanly
    let mut snapshot = Vec::with_capacity((area.width as usize) * (area.height as usize));
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let c = buf.cell((x, y)).cloned();
            snapshot.push(c);
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
) {
    let glyphs: Vec<char> = if charset_str.is_empty() {
        vec!['0', '1', '#', '$']
    } else {
        charset_str.chars().collect()
    };
    let total_rows = area.height.max(1) as f32;
    let reveal_rows = total_rows * EasingKind::EaseInOut.apply(t);

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
                    let hash = ((x as usize) * 31 + (y as usize) * 17 + (t * 97.0) as usize)
                        % glyphs.len();
                    cell.set_char(glyphs[hash]).set_fg(theme.primary);
                } else {
                    cell.set_char(' ');
                }
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
) {
    let flash_color = color_override
        .map(|c| parse_color(c, theme.primary))
        .unwrap_or(theme.primary);
    let strength = (1.0 - (t * 2.0 - 1.0).abs()).clamp(0.0, 1.0) * 0.65;
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_fg(blend_color(flash_color, cell.fg, strength));
            }
        }
    }
}
