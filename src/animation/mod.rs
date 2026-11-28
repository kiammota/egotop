pub mod backgrounds;
pub mod easing;
pub mod geometry;
pub mod matrix;
pub mod none;
pub mod particles;
pub mod scanlines;
pub mod scene;
pub mod text_fx;
pub mod timeline;
pub mod transition;
pub mod water;

use crate::config::{BackgroundConfig, EffectKind, RegionConfig, RegionUnit};
use crate::ui::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

pub use backgrounds::{BackgroundLayerKind, BackgroundLayerSpec, BackgroundPipeline};
pub use easing::EasingCurve;
pub use geometry::{
    GeometryRenderer, MORPH_SAMPLE_COUNT, ShapeDrawParams, ShapePrimitive, ShapeSpec,
    morph_contours, rasterize_contour,
};
pub use matrix::MatrixEffect;
pub use none::NoneEffect;
pub use particles::ParticlesEffect;
pub use scanlines::ScanlinesEffect;
pub use scene::{CoordAnchor, CoordValue, LayerSpec, SceneCell, SceneDrawItem};
pub use text_fx::{TextDirection, TextEffectKind, TextRenderParams, evaluate_text_cells};
pub use timeline::{LoopConfig, LoopMode, TimelineClock, evaluate_scene_tracks};
pub use transition::{
    AnimationCatalog, AnimationCatalogEntry, NativeTransition, TransitionPlayer,
};
pub use water::WaterEffect;

/// The three strictly independent animation slot categories.
///
/// Each Lua animation script must declare exactly one of these via `type = "intro" | "background" | "canvas"`.
/// The engine validates the declared type against the target slot and refuses mismatches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnimSlotKind {
    /// Entry/startup animation: runs once with full-screen access, finite lifetime.
    Intro,
    /// Continuous background loop at Z=0, always behind all UI containers.
    Background,
    /// Per-container animation with rigid clipping to the container's inner bounds.
    Canvas,
}

impl AnimSlotKind {
    /// Parses a slot kind from a Lua `type` string.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "intro" | "entry" | "startup" => Some(Self::Intro),
            "background" | "bg" => Some(Self::Background),
            "canvas" | "container" | "panel" => Some(Self::Canvas),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Intro => "intro",
            Self::Background => "background",
            Self::Canvas => "canvas",
        }
    }

    /// Returns the default slot for scripts loaded from a directory.
    pub fn default_for_directory(dir_name: &str) -> Self {
        match dir_name {
            "animations" => Self::Intro,
            "effects" => Self::Background,
            _ => Self::Canvas,
        }
    }
}

impl std::fmt::Display for AnimSlotKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

pub trait BackgroundEffect: Send {
    fn kind(&self) -> EffectKind;
    fn resize(&mut self, width: u16, height: u16, cfg: &BackgroundConfig);
    fn update(&mut self, dt_secs: f32, cfg: &BackgroundConfig);
    fn render(&self, buf: &mut Buffer, area: Rect, cfg: &BackgroundConfig, theme: &Theme);
}

pub fn create_effect(
    kind: EffectKind,
    width: u16,
    height: u16,
    cfg: &BackgroundConfig,
) -> Box<dyn BackgroundEffect> {
    let mut effect: Box<dyn BackgroundEffect> = match kind {
        EffectKind::Matrix => Box::new(MatrixEffect::new()),
        EffectKind::Water => Box::new(WaterEffect::new()),
        EffectKind::Particles => Box::new(ParticlesEffect::new()),
        EffectKind::Scanlines => Box::new(ScanlinesEffect::new()),
        EffectKind::None => Box::new(NoneEffect),
    };
    effect.resize(width, height, cfg);
    effect
}

pub fn resolve_region(
    area: Rect,
    effect_region: Option<&RegionConfig>,
    global_region: Option<&RegionConfig>,
) -> Rect {
    let Some(reg) = effect_region.or(global_region) else {
        return area;
    };
    if area.width == 0 || area.height == 0 {
        return Rect::ZERO;
    }

    let (rx, ry, rw, rh) = match reg.unit {
        RegionUnit::Percent => {
            let x = area.x + ((area.width as u32 * reg.x.min(100) as u32) / 100) as u16;
            let y = area.y + ((area.height as u32 * reg.y.min(100) as u32) / 100) as u16;
            let w = ((area.width as u32 * reg.width.min(100) as u32) / 100).max(1) as u16;
            let h = ((area.height as u32 * reg.height.min(100) as u32) / 100).max(1) as u16;
            (x, y, w, h)
        }
        RegionUnit::Cells => (
            area.x.saturating_add(reg.x),
            area.y.saturating_add(reg.y),
            reg.width,
            reg.height,
        ),
    };

    let clamped_x = rx.min(area.right());
    let clamped_y = ry.min(area.bottom());
    let clamped_w = rw.min(area.right().saturating_sub(clamped_x));
    let clamped_h = rh.min(area.bottom().saturating_sub(clamped_y));
    Rect::new(clamped_x, clamped_y, clamped_w, clamped_h)
}
