pub mod animation_loader;
pub mod loader;
pub mod paths;
pub mod schema;
pub mod theme_loader;

pub use animation_loader::{
    CommunityAnimationEntry, DeclarativeAnimation, DeclarativeStep, DeclarativeStepEffect,
    EasingKind, StepDirection, StepTarget, load_community_animations,
};
pub use loader::{
    ConfigLoadResult, default_config_path, load_from_path, load_or_create,
    load_or_create_window_config, save_config_to_path, save_window_config,
};
pub use schema::{
    Align, Anchor, BackgroundConfig, BorderStyle, Config, ConfigError, EffectKind, GeneralConfig,
    GraphStyle, HeaderConfig, LayoutConfig, LayoutMode, MatrixCharset, MatrixConfig,
    ParticlesConfig, RegionConfig, RegionUnit, ScanlinesConfig, SizeSpec, ThemeConfig,
    TypographyConfig, ViewMode, WaterConfig, WidgetConfig, WidgetId,
};
pub use theme_loader::{ThemeEntry, load_all_themes};
