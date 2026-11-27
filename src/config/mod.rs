pub mod animation_loader;
pub mod loader;
pub mod paths;
pub mod schema;
pub mod theme_loader;

pub use animation_loader::{
    AnimationDefinition, AnimationPackage, AnimationRenderMode, CommunityAnimationEntry,
    DeclarativeAnimation, DeclarativeStep, DeclarativeStepEffect, EasingKind, KeyframeSpec,
    MorphTransitionMode, PackageOrigin, StepDirection, StepTarget, TrackKind, TrackSpec,
    load_community_animations, load_community_packages, load_packages_from_dir,
    validate_safe_asset_path,
};
pub use loader::{
    ConfigLoadResult, default_config_path, load_from_path, load_layout_names, load_or_create,
    save_config_to_path,
};
pub use schema::{
    Align, BackgroundConfig, BorderStyle, Config, ConfigError, EffectKind, GeneralConfig,
    GraphStyle, HeaderConfig, MatrixCharset, MatrixConfig, ParticlesConfig, RegionConfig,
    RegionUnit, ScanlinesConfig, StartupConfig, StartupCursorConfig, StartupMode, StartupTrigger,
    ThemeConfig, TypographyConfig, UiConfig, ViewMode, WaterConfig,
};
pub use theme_loader::{ThemeEntry, load_all_themes};
