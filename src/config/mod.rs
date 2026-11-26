pub mod loader;
pub mod schema;

pub use loader::{ConfigLoadResult, default_config_path, load_or_create, load_from_path};
pub use schema::{
    Align, Anchor, BackgroundConfig, BorderStyle, Config, ConfigError, EffectKind,
    GeneralConfig, GraphStyle, HeaderConfig, LayoutConfig, LayoutMode, MatrixCharset,
    MatrixConfig, ParticlesConfig, RegionConfig, RegionUnit, ScanlinesConfig, SizeSpec,
    ThemeConfig, TypographyConfig, ViewMode, WaterConfig, WidgetConfig, WidgetId,
};
