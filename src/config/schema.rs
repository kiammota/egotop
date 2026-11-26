use serde::{Deserialize, Deserializer, Serialize};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to read config file '{path}': {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Invalid TOML syntax in '{path}': {message}")]
    Parse { path: PathBuf, message: String },
    #[error("Configuration validation error in '{field}': {message}")]
    Validation { field: String, message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    pub general: GeneralConfig,
    pub theme: ThemeConfig,
    pub typography: TypographyConfig,
    pub header: HeaderConfig,
    pub background: BackgroundConfig,
    pub layout: LayoutConfig,
    pub widgets: Vec<WidgetConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            theme: ThemeConfig::default(),
            typography: TypographyConfig::default(),
            header: HeaderConfig::default(),
            background: BackgroundConfig::default(),
            layout: LayoutConfig::default(),
            widgets: default_widgets(),
        }
    }
}

pub fn default_widgets() -> Vec<WidgetConfig> {
    vec![
        WidgetConfig {
            id: WidgetId::Greeting,
            enabled: true,
            title: Some("Bem-vindo".to_string()),
            position: Anchor::TopCenter,
            page: Some(1),
            z_index: 20,
            align: Align::Center,
            font: Some("ANSI Shadow".to_string()),
            reveal_animation: Some(true),
            show_border: Some(true),
            ..WidgetConfig::default()
        },
        WidgetConfig {
            id: WidgetId::System,
            enabled: true,
            title: Some("Sistema".to_string()),
            position: Anchor::CenterLeft,
            page: Some(1),
            width: Some(SizeSpec::Percent(50)),
            z_index: 10,
            align: Align::Left,
            ..WidgetConfig::default()
        },
        WidgetConfig {
            id: WidgetId::Hardware,
            enabled: true,
            title: Some("Hardware".to_string()),
            position: Anchor::CenterRight,
            page: Some(1),
            width: Some(SizeSpec::Percent(50)),
            z_index: 10,
            align: Align::Left,
            graph_style: Some(GraphStyle::Bar),
            ..WidgetConfig::default()
        },
        WidgetConfig {
            id: WidgetId::Activity,
            enabled: true,
            title: Some("Atividade".to_string()),
            position: Anchor::BottomLeft,
            page: Some(1),
            width: Some(SizeSpec::Percent(60)),
            z_index: 10,
            graph_style: Some(GraphStyle::Sparkline),
            ..WidgetConfig::default()
        },
        WidgetConfig {
            id: WidgetId::Session,
            enabled: true,
            title: Some("Sessão & Uptime".to_string()),
            position: Anchor::BottomRight,
            page: Some(1),
            width: Some(SizeSpec::Percent(40)),
            z_index: 10,
            ..WidgetConfig::default()
        },
        WidgetConfig {
            id: WidgetId::Message,
            enabled: false,
            title: Some("Mensagem".to_string()),
            position: Anchor::BottomCenter,
            page: Some(1),
            z_index: 15,
            align: Align::Center,
            ..WidgetConfig::default()
        },
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct GeneralConfig {
    pub name: String,
    pub greeting: String,
    pub user_override: Option<String>,
    pub message: String,
    #[serde(alias = "refresh_rate")]
    pub refresh_rate_ms: u64,
    pub animation_fps: u32,
    pub time_format: String,
    pub date_format: String,
    pub hide_unavailable: bool,
    pub view_mode: ViewMode,
    pub default_page: u8,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            name: "egotop".to_string(),
            greeting: "Olá, {user}!".to_string(),
            user_override: None,
            message: "Bem-vindo de volta ao seu ambiente Linux.".to_string(),
            refresh_rate_ms: 1000,
            animation_fps: 30,
            time_format: "%H:%M:%S".to_string(),
            date_format: "%d/%m/%Y".to_string(),
            hide_unavailable: false,
            view_mode: ViewMode::Dashboard,
            default_page: 1,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ViewMode {
    #[default]
    Dashboard,
    Compact,
    Minimal,
}

impl ViewMode {
    pub fn next(self) -> Self {
        match self {
            Self::Dashboard => Self::Compact,
            Self::Compact => Self::Minimal,
            Self::Minimal => Self::Dashboard,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Dashboard => "Dashboard",
            Self::Compact => "Compacto",
            Self::Minimal => "Minimal",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ThemeConfig {
    pub name: String,
    pub background: String,
    pub panel_background: String,
    pub primary: String,
    pub secondary: String,
    pub accent: String,
    pub text: String,
    pub muted: String,
    pub border: String,
    pub danger: String,
    pub border_type: BorderStyle,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            name: "matrix".to_string(),
            background: "#050A07".to_string(),
            panel_background: "#08110C".to_string(),
            primary: "#39FF88".to_string(),
            secondary: "#00D9FF".to_string(),
            accent: "#FFC857".to_string(),
            text: "#E5E5E5".to_string(),
            muted: "#5F7D6B".to_string(),
            border: "#1F7A4C".to_string(),
            danger: "#FF5555".to_string(),
            border_type: BorderStyle::Rounded,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum BorderStyle {
    #[default]
    Rounded,
    Plain,
    Double,
    Thick,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct TypographyConfig {
    pub font: String,
    pub logo_font: String,
    pub use_nerd_fonts: bool,
    pub ascii_fallback: bool,
    pub logo_preset: String,
    pub custom_logo: Option<String>,
    pub custom_logo_file: Option<String>,
}

impl Default for TypographyConfig {
    fn default() -> Self {
        Self {
            font: "ANSI Shadow".to_string(),
            logo_font: "Standard".to_string(),
            use_nerd_fonts: true,
            ascii_fallback: false,
            logo_preset: "auto".to_string(),
            custom_logo: None,
            custom_logo_file: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct HeaderConfig {
    pub enabled: bool,
    pub show_clock: bool,
    pub show_date: bool,
    pub show_indicators: bool,
    pub show_fps: bool,
}

impl Default for HeaderConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            show_clock: true,
            show_date: true,
            show_indicators: true,
            show_fps: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct BackgroundConfig {
    pub effect: EffectKind,
    pub enabled: bool,
    pub speed: f32,
    pub density: f32,
    pub opacity: f32,
    pub region: Option<RegionConfig>,
    pub matrix: MatrixConfig,
    pub water: WaterConfig,
    pub particles: ParticlesConfig,
    pub scanlines: ScanlinesConfig,
}

impl Default for BackgroundConfig {
    fn default() -> Self {
        Self {
            effect: EffectKind::Matrix,
            enabled: true,
            speed: 1.0,
            density: 0.60,
            opacity: 0.38,
            region: None,
            matrix: MatrixConfig::default(),
            water: WaterConfig::default(),
            particles: ParticlesConfig::default(),
            scanlines: ScanlinesConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum EffectKind {
    #[default]
    Matrix,
    Water,
    Particles,
    Scanlines,
    None,
}

impl EffectKind {
    pub fn next(self) -> Self {
        match self {
            Self::Matrix => Self::Water,
            Self::Water => Self::Particles,
            Self::Particles => Self::Scanlines,
            Self::Scanlines => Self::None,
            Self::None => Self::Matrix,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Matrix => "Matrix",
            Self::Water => "Water",
            Self::Particles => "Particles",
            Self::Scanlines => "Scanlines",
            Self::None => "None",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum RegionUnit {
    #[default]
    Percent,
    Cells,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct RegionConfig {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    pub unit: RegionUnit,
}

impl Default for RegionConfig {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
            unit: RegionUnit::Percent,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum MatrixCharset {
    #[default]
    Katakana,
    Hex,
    Ascii,
    Binary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct MatrixConfig {
    pub enabled: bool,
    pub speed: f32,
    pub density: f32,
    pub min_length: usize,
    pub max_length: usize,
    pub color: String,
    pub head_color: String,
    pub charset: MatrixCharset,
    pub region: Option<RegionConfig>,
}

impl Default for MatrixConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            speed: 1.0,
            density: 0.60,
            min_length: 5,
            max_length: 18,
            color: "#1F7A4C".to_string(),
            head_color: "#39FF88".to_string(),
            charset: MatrixCharset::Katakana,
            region: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct WaterConfig {
    pub enabled: bool,
    pub speed: f32,
    pub amplitude: f32,
    pub density: f32,
    pub color: String,
    pub crest_color: String,
    pub region: Option<RegionConfig>,
}

impl Default for WaterConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            speed: 1.0,
            amplitude: 0.6,
            density: 0.7,
            color: "#0088AA".to_string(),
            crest_color: "#00D9FF".to_string(),
            region: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ParticlesConfig {
    pub enabled: bool,
    pub speed: f32,
    pub density: f32,
    pub count: usize,
    pub color: String,
    pub chars: String,
    pub region: Option<RegionConfig>,
}

impl Default for ParticlesConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            speed: 0.9,
            density: 0.5,
            count: 45,
            color: "#39FF88".to_string(),
            chars: "·•✦∘*+".to_string(),
            region: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ScanlinesConfig {
    pub enabled: bool,
    pub speed: f32,
    pub spacing: u16,
    pub intensity: f32,
    pub color: String,
    pub region: Option<RegionConfig>,
}

impl Default for ScanlinesConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            speed: 1.0,
            spacing: 2,
            intensity: 0.35,
            color: "#164A2E".to_string(),
            region: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct LayoutConfig {
    pub mode: LayoutMode,
    pub padding: u16,
    pub gap: u16,
    pub min_width_for_split: u16,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            mode: LayoutMode::Responsive,
            padding: 1,
            gap: 1,
            min_width_for_split: 88,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum LayoutMode {
    #[default]
    Responsive,
    Free,
    Compact,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum WidgetId {
    Greeting,
    Logo,
    System,
    Hardware,
    Activity,
    Session,
    Uptime,
    Message,
}

impl WidgetId {
    pub fn default_title(self) -> &'static str {
        match self {
            Self::Greeting => "Bem-vindo",
            Self::Logo => "Logo",
            Self::System => "Sistema",
            Self::Hardware => "Hardware",
            Self::Activity => "Atividade",
            Self::Session => "Sessão",
            Self::Uptime => "Uptime",
            Self::Message => "Mensagem",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Anchor {
    TopLeft,
    TopCenter,
    TopRight,
    CenterLeft,
    #[default]
    Center,
    CenterRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
    Absolute,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum GraphStyle {
    #[default]
    Sparkline,
    Bar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeSpec {
    Cells(u16),
    Percent(u16),
}

impl Serialize for SizeSpec {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Cells(n) => serializer.serialize_u16(*n),
            Self::Percent(p) => serializer.serialize_str(&format!("{p}%")),
        }
    }
}

impl<'de> Deserialize<'de> for SizeSpec {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Num(u16),
            Text(String),
        }

        match Raw::deserialize(deserializer)? {
            Raw::Num(n) => Ok(Self::Cells(n)),
            Raw::Text(s) => {
                let trimmed = s.trim();
                if let Some(pct) = trimmed.strip_suffix('%') {
                    let val: u16 = pct
                        .trim()
                        .parse()
                        .map_err(|_| serde::de::Error::custom("invalid percentage size"))?;
                    Ok(Self::Percent(val.min(100)))
                } else {
                    let val: u16 = trimmed
                        .parse()
                        .map_err(|_| serde::de::Error::custom("invalid cell size"))?;
                    Ok(Self::Cells(val))
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct WidgetConfig {
    pub id: WidgetId,
    pub enabled: bool,
    pub title: Option<String>,
    pub position: Anchor,
    pub page: Option<u8>,
    pub x: Option<u16>,
    pub y: Option<u16>,
    pub offset_x: i16,
    pub offset_y: i16,
    pub width: Option<SizeSpec>,
    pub height: Option<SizeSpec>,
    pub align: Align,
    pub padding: u16,
    pub z_index: i32,
    pub font: Option<String>,
    pub color: Option<String>,
    pub border_color: Option<String>,
    pub show_border: Option<bool>,
    pub fields: Option<Vec<String>>,
    pub graph_style: Option<GraphStyle>,
    pub reveal_animation: Option<bool>,
}

impl Default for WidgetConfig {
    fn default() -> Self {
        Self {
            id: WidgetId::System,
            enabled: true,
            title: None,
            position: Anchor::Center,
            page: Some(1),
            x: None,
            y: None,
            offset_x: 0,
            offset_y: 0,
            width: None,
            height: None,
            align: Align::Left,
            padding: 1,
            z_index: 10,
            font: None,
            color: None,
            border_color: None,
            show_border: Some(true),
            fields: None,
            graph_style: None,
            reveal_animation: None,
        }
    }
}

impl Config {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.general.animation_fps == 0 || self.general.animation_fps > 120 {
            return Err(ConfigError::Validation {
                field: "general.animation_fps".to_string(),
                message: "must be between 1 and 120".to_string(),
            });
        }
        if self.general.refresh_rate_ms < 100 {
            return Err(ConfigError::Validation {
                field: "general.refresh_rate_ms".to_string(),
                message: "must be at least 100 ms".to_string(),
            });
        }
        if !(0.0..=1.0).contains(&self.background.density) {
            return Err(ConfigError::Validation {
                field: "background.density".to_string(),
                message: "must be between 0.0 and 1.0".to_string(),
            });
        }
        if !(0.0..=1.0).contains(&self.background.opacity) {
            return Err(ConfigError::Validation {
                field: "background.opacity".to_string(),
                message: "must be between 0.0 and 1.0".to_string(),
            });
        }
        if self.background.speed < 0.0 || self.background.speed > 10.0 {
            return Err(ConfigError::Validation {
                field: "background.speed".to_string(),
                message: "must be between 0.0 and 10.0".to_string(),
            });
        }
        if self.background.matrix.min_length == 0
            || self.background.matrix.min_length > self.background.matrix.max_length
        {
            return Err(ConfigError::Validation {
                field: "background.matrix.min_length".to_string(),
                message: "must be >= 1 and <= max_length".to_string(),
            });
        }
        if self.background.scanlines.spacing == 0 {
            return Err(ConfigError::Validation {
                field: "background.scanlines.spacing".to_string(),
                message: "must be >= 1".to_string(),
            });
        }
        for (field, color) in [
            ("theme.background", &self.theme.background),
            ("theme.panel_background", &self.theme.panel_background),
            ("theme.primary", &self.theme.primary),
            ("theme.secondary", &self.theme.secondary),
            ("theme.accent", &self.theme.accent),
            ("theme.text", &self.theme.text),
            ("theme.muted", &self.theme.muted),
            ("theme.border", &self.theme.border),
            ("theme.danger", &self.theme.danger),
        ] {
            if !is_valid_color_spec(color) {
                return Err(ConfigError::Validation {
                    field: field.to_string(),
                    message: format!("invalid color '{color}' (use #RRGGBB, named color, or 'reset')"),
                });
            }
        }
        Ok(())
    }
}

pub fn is_valid_color_spec(spec: &str) -> bool {
    let s = spec.trim().to_ascii_lowercase();
    if matches!(
        s.as_str(),
        "reset"
            | "default"
            | "transparent"
            | "black"
            | "red"
            | "green"
            | "yellow"
            | "blue"
            | "magenta"
            | "cyan"
            | "gray"
            | "grey"
            | "darkgray"
            | "darkgrey"
            | "lightred"
            | "lightgreen"
            | "lightyellow"
            | "lightblue"
            | "lightmagenta"
            | "lightcyan"
            | "white"
    ) {
        return true;
    }
    if let Some(hex) = s.strip_prefix('#') {
        return hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit());
    }
    false
}
