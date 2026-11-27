use crate::config::{Align, BorderStyle, GraphStyle, MatrixCharset};
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum BackgroundFill {
    #[default]
    Inherit,
    Transparent,
    Solid(String),
}

impl Serialize for BackgroundFill {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Inherit => serializer.serialize_str("inherit"),
            Self::Transparent => serializer.serialize_str("transparent"),
            Self::Solid(c) => serializer.serialize_str(c),
        }
    }
}

impl<'de> Deserialize<'de> for BackgroundFill {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        let low = s.trim().to_ascii_lowercase();
        match low.as_str() {
            "" | "inherit" => Ok(Self::Inherit),
            "transparent" | "reset" => Ok(Self::Transparent),
            _ => Ok(Self::Solid(s.trim().to_string())),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum GradientDirection {
    #[default]
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContainerGradient {
    pub direction: GradientDirection,
    pub colors: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum EffectPolicy {
    #[default]
    Exclude,
    Allow,
    Isolated,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct ContainerStyle {
    pub background: BackgroundFill,
    pub foreground: Option<String>,
    pub border_color: Option<String>,
    pub border_style: Option<BorderStyle>,
    pub gradient: Option<ContainerGradient>,
    pub opacity: Option<f32>,
    pub effects: EffectPolicy,
}

/// Interface region in which a container is placed by the automatic layout engine.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum LayoutRegion {
    Top,
    Left,
    #[default]
    #[serde(alias = "center", alias = "flow")]
    Main,
    Right,
    Bottom,
}

impl LayoutRegion {
    pub fn label(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Left => "left",
            Self::Main => "main",
            Self::Right => "right",
            Self::Bottom => "bottom",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "top" | "header" => Some(Self::Top),
            "left" => Some(Self::Left),
            "main" | "center" | "flow" | "body" => Some(Self::Main),
            "right" => Some(Self::Right),
            "bottom" | "footer" => Some(Self::Bottom),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContainerSize {
    #[default]
    Auto,
    Fixed(u16),
    Percent(u16),
    Fill(u16),
}

impl Serialize for ContainerSize {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Auto => serializer.serialize_str("auto"),
            Self::Fixed(n) => serializer.serialize_u16(*n),
            Self::Percent(p) => serializer.serialize_str(&format!("{p}%")),
            Self::Fill(1) => serializer.serialize_str("fill"),
            Self::Fill(w) => serializer.serialize_str(&format!("fill({w})")),
        }
    }
}

impl<'de> Deserialize<'de> for ContainerSize {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Num(u16),
            Text(String),
        }

        match Raw::deserialize(deserializer)? {
            Raw::Num(n) => Ok(Self::Fixed(n)),
            Raw::Text(s) => {
                let t = s.trim().to_ascii_lowercase();
                if t.is_empty() || t == "auto" {
                    return Ok(Self::Auto);
                }
                if t == "fill" {
                    return Ok(Self::Fill(1));
                }
                if let Some(inner) = t.strip_prefix("fill(").and_then(|v| v.strip_suffix(')')) {
                    let weight: u16 = inner
                        .trim()
                        .parse()
                        .map_err(|_| serde::de::Error::custom("invalid fill weight"))?;
                    return Ok(Self::Fill(weight.max(1)));
                }
                if let Some(pct) = t.strip_suffix('%') {
                    let val: u16 = pct
                        .trim()
                        .parse()
                        .map_err(|_| serde::de::Error::custom("invalid percentage size"))?;
                    return Ok(Self::Percent(val.min(100)));
                }
                let val: u16 = t
                    .parse()
                    .map_err(|_| serde::de::Error::custom("invalid size specification"))?;
                Ok(Self::Fixed(val))
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct ContainerBorderConfig {
    pub enabled: bool,
    pub style: Option<BorderStyle>,
    pub color: Option<String>,
    pub title: Option<String>,
}

impl Default for ContainerBorderConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            style: None,
            color: None,
            title: None,
        }
    }
}

impl ContainerBorderConfig {
    pub fn titled(title: impl Into<String>) -> Self {
        Self {
            enabled: true,
            style: None,
            color: None,
            title: Some(title.into()),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ComponentCategory {
    #[default]
    Static,
    Dynamic,
    Animated,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ComponentKind {
    // Static
    #[default]
    Text,
    AsciiLogo,
    Separator,
    Greeting,
    // Dynamic
    Cpu,
    Memory,
    Temperature,
    Processes,
    Network,
    Uptime,
    Graph,
    System,
    Hardware,
    Environment,
    Terminal,
    Storage,
    Activity,
    Session,
    Header,
    // Animated / Procedural / Lua Canvas
    Spinner,
    Typewriter,
    Matrix,
    Particles,
    Frames,
    AsciiObject,
    Canvas,
}

impl ComponentKind {
    pub fn expected_category(self) -> ComponentCategory {
        match self {
            Self::Text | Self::AsciiLogo | Self::Separator | Self::Greeting => {
                ComponentCategory::Static
            }
            Self::Cpu
            | Self::Memory
            | Self::Temperature
            | Self::Processes
            | Self::Network
            | Self::Uptime
            | Self::Graph
            | Self::System
            | Self::Hardware
            | Self::Environment
            | Self::Terminal
            | Self::Storage
            | Self::Activity
            | Self::Session
            | Self::Header => ComponentCategory::Dynamic,
            Self::Spinner
            | Self::Typewriter
            | Self::Matrix
            | Self::Particles
            | Self::Frames
            | Self::AsciiObject
            | Self::Canvas => ComponentCategory::Animated,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Header => "header",
            Self::Greeting => "greeting",
            Self::System => "system",
            Self::Hardware => "hardware",
            Self::Environment => "environment",
            Self::Terminal => "terminal",
            Self::Storage => "storage",
            Self::Activity => "activity",
            Self::Cpu => "cpu",
            Self::Memory => "memory",
            Self::Temperature => "temperature",
            Self::Processes => "processes",
            Self::Network => "network",
            Self::Uptime => "uptime",
            Self::Session => "session",
            Self::Graph => "graph",
            Self::AsciiLogo => "ascii_logo",
            Self::Separator => "separator",
            Self::Text => "text",
            Self::Spinner => "spinner",
            Self::Typewriter => "typewriter",
            Self::Matrix => "matrix",
            Self::Particles => "particles",
            Self::Frames => "frames",
            Self::AsciiObject => "ascii_object",
            Self::Canvas => "canvas",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct ComponentContent {
    pub text: Option<String>,
    pub title: Option<String>,
    pub preset: Option<String>,
    pub script: Option<String>,
    pub frames: Vec<String>,
    pub fields: Option<Vec<String>>,
    pub metric: Option<String>,
    pub separator_char: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ComponentAnimationConfig {
    pub fps: Option<u32>,
    pub speed: f32,
    pub loop_anim: bool,
    pub style: Option<String>,
    pub charset: Option<MatrixCharset>,
    pub density: Option<f32>,
}

impl Default for ComponentAnimationConfig {
    fn default() -> Self {
        Self {
            fps: None,
            speed: 1.0,
            loop_anim: true,
            style: None,
            charset: None,
            density: None,
        }
    }
}

/// Dumped Lua bytecode callbacks for inline container `init` / `update` / `render` / `resize` functions.
/// Loaded into an isolated per-container Lua VM so scripts can never access other containers' canvases.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InlineLuaScript {
    pub init_chunk: Option<Vec<u8>>,
    pub update_chunk: Option<Vec<u8>>,
    pub render_chunk: Option<Vec<u8>>,
    pub resize_chunk: Option<Vec<u8>>,
}

/// Independent, non-nestable UI container.
/// Each container owns an isolated rendering canvas and cannot contain child containers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ContainerConfig {
    #[serde(alias = "id")]
    pub name: String,
    pub description: String,
    pub visible: bool,
    pub region: LayoutRegion,
    pub order: i32,
    pub row: Option<u16>,
    pub width: ContainerSize,
    pub height: ContainerSize,
    pub min_width: Option<u16>,
    pub max_width: Option<u16>,
    pub min_height: Option<u16>,
    pub max_height: Option<u16>,
    pub padding: u16,
    pub transparent: bool,
    pub align: Align,
    pub border: ContainerBorderConfig,
    pub color: Option<String>,
    pub background: Option<String>,
    pub gradient: Option<ContainerGradient>,
    pub opacity: Option<f32>,
    pub font: Option<String>,
    pub graph_style: Option<GraphStyle>,
    pub effects: EffectPolicy,
    pub category: ComponentCategory,
    #[serde(rename = "type", alias = "kind")]
    pub kind: ComponentKind,
    pub content: ComponentContent,
    pub animation: Option<ComponentAnimationConfig>,
    pub refresh_ms: Option<u64>,
    #[serde(skip)]
    pub inline_lua: Option<InlineLuaScript>,
}

impl Default for ContainerConfig {
    fn default() -> Self {
        Self {
            name: "container".to_string(),
            description: String::new(),
            visible: true,
            region: LayoutRegion::Main,
            order: 0,
            row: None,
            width: ContainerSize::Auto,
            height: ContainerSize::Auto,
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: 0,
            transparent: false,
            align: Align::Left,
            border: ContainerBorderConfig::default(),
            color: None,
            background: None,
            gradient: None,
            opacity: None,
            font: None,
            graph_style: None,
            effects: EffectPolicy::Exclude,
            category: ComponentCategory::Static,
            kind: ComponentKind::Text,
            content: ComponentContent::default(),
            animation: None,
            refresh_ms: None,
            inline_lua: None,
        }
    }
}

pub fn collect_container_names(containers: &[ContainerConfig], out: &mut Vec<String>) {
    for c in containers {
        out.push(c.name.clone());
    }
}

pub fn has_container(containers: &[ContainerConfig], name: &str) -> bool {
    containers.iter().any(|c| c.name == name)
}

pub fn find_container<'a>(
    containers: &'a [ContainerConfig],
    name: &str,
) -> Option<&'a ContainerConfig> {
    containers.iter().find(|c| c.name == name)
}

pub fn remove_container(containers: &mut Vec<ContainerConfig>, name: &str) -> bool {
    let before = containers.len();
    containers.retain(|c| c.name != name);
    containers.len() != before
}

pub fn toggle_container_visible(containers: &mut [ContainerConfig], name: &str) -> Option<bool> {
    if let Some(c) = containers.iter_mut().find(|c| c.name == name) {
        c.visible = !c.visible;
        Some(c.visible)
    } else {
        None
    }
}

pub fn default_containers() -> Vec<ContainerConfig> {
    vec![
        ContainerConfig {
            name: "header".to_string(),
            description: "Top status bar with brand, active mode, effect, and clock".to_string(),
            visible: true,
            region: LayoutRegion::Top,
            row: Some(0),
            width: ContainerSize::Percent(100),
            height: ContainerSize::Fixed(1),
            transparent: true,
            category: ComponentCategory::Dynamic,
            kind: ComponentKind::Header,
            ..ContainerConfig::default()
        },
        ContainerConfig {
            name: "greeting".to_string(),
            description: "Welcome banner with FIGlet greeting and system logo".to_string(),
            visible: true,
            region: LayoutRegion::Top,
            row: Some(1),
            width: ContainerSize::Percent(100),
            height: ContainerSize::Fixed(9),
            min_height: Some(6),
            align: Align::Center,
            border: ContainerBorderConfig::titled("Welcome"),
            font: Some("ANSI Shadow".to_string()),
            category: ComponentCategory::Static,
            kind: ComponentKind::Greeting,
            ..ContainerConfig::default()
        },
        ContainerConfig {
            name: "system".to_string(),
            description: "Operating system, host, kernel, shell, and terminal metadata".to_string(),
            visible: true,
            region: LayoutRegion::Main,
            row: Some(0),
            width: ContainerSize::Percent(50),
            height: ContainerSize::Fill(1),
            min_height: Some(6),
            padding: 1,
            border: ContainerBorderConfig::titled("System"),
            category: ComponentCategory::Dynamic,
            kind: ComponentKind::System,
            ..ContainerConfig::default()
        },
        ContainerConfig {
            name: "hardware".to_string(),
            description: "CPU, RAM, disk, and GPU usage telemetry bars".to_string(),
            visible: true,
            region: LayoutRegion::Main,
            row: Some(0),
            width: ContainerSize::Percent(50),
            height: ContainerSize::Fill(1),
            min_height: Some(6),
            padding: 1,
            border: ContainerBorderConfig::titled("Hardware"),
            graph_style: Some(GraphStyle::Bar),
            category: ComponentCategory::Dynamic,
            kind: ComponentKind::Hardware,
            ..ContainerConfig::default()
        },
        ContainerConfig {
            name: "environment".to_string(),
            description: "Window manager, session protocol, display, and desktop theme".to_string(),
            visible: true,
            region: LayoutRegion::Main,
            row: Some(1),
            width: ContainerSize::Percent(40),
            height: ContainerSize::Fill(1),
            min_height: Some(5),
            padding: 1,
            border: ContainerBorderConfig::titled("Environment"),
            category: ComponentCategory::Dynamic,
            kind: ComponentKind::Environment,
            ..ContainerConfig::default()
        },
        ContainerConfig {
            name: "terminal".to_string(),
            description: "Terminal emulator, shell, font, and viewport dimensions".to_string(),
            visible: true,
            region: LayoutRegion::Main,
            row: Some(1),
            width: ContainerSize::Percent(30),
            height: ContainerSize::Fill(1),
            min_height: Some(5),
            padding: 1,
            border: ContainerBorderConfig::titled("Terminal"),
            category: ComponentCategory::Dynamic,
            kind: ComponentKind::Terminal,
            ..ContainerConfig::default()
        },
        ContainerConfig {
            name: "storage".to_string(),
            description: "Mounted filesystems and storage capacity bars".to_string(),
            visible: true,
            region: LayoutRegion::Main,
            row: Some(1),
            width: ContainerSize::Fill(1),
            height: ContainerSize::Fill(1),
            min_height: Some(5),
            padding: 1,
            border: ContainerBorderConfig::titled("Storage"),
            category: ComponentCategory::Dynamic,
            kind: ComponentKind::Storage,
            ..ContainerConfig::default()
        },
        ContainerConfig {
            name: "activity".to_string(),
            description: "Load averages, process counts, and CPU/RAM history sparklines".to_string(),
            visible: true,
            region: LayoutRegion::Main,
            row: Some(2),
            width: ContainerSize::Percent(40),
            height: ContainerSize::Fill(1),
            min_height: Some(6),
            padding: 1,
            border: ContainerBorderConfig::titled("Activity"),
            graph_style: Some(GraphStyle::Sparkline),
            category: ComponentCategory::Dynamic,
            kind: ComponentKind::Activity,
            ..ContainerConfig::default()
        },
        ContainerConfig {
            name: "network".to_string(),
            description: "Network interfaces and RX/TX transfer telemetry".to_string(),
            visible: true,
            region: LayoutRegion::Main,
            row: Some(2),
            width: ContainerSize::Percent(30),
            height: ContainerSize::Fill(1),
            min_height: Some(6),
            padding: 1,
            border: ContainerBorderConfig::titled("Network"),
            category: ComponentCategory::Dynamic,
            kind: ComponentKind::Network,
            ..ContainerConfig::default()
        },
        ContainerConfig {
            name: "session".to_string(),
            description: "System uptime, boot timestamp, and active session status".to_string(),
            visible: true,
            region: LayoutRegion::Main,
            row: Some(2),
            width: ContainerSize::Fill(1),
            height: ContainerSize::Fill(1),
            min_height: Some(6),
            padding: 1,
            border: ContainerBorderConfig::titled("Session & Uptime"),
            category: ComponentCategory::Dynamic,
            kind: ComponentKind::Session,
            ..ContainerConfig::default()
        },
        ContainerConfig {
            name: "status_spinner".to_string(),
            description: "Footer activity spinner and command-line bar".to_string(),
            visible: true,
            region: LayoutRegion::Bottom,
            row: Some(0),
            width: ContainerSize::Percent(100),
            height: ContainerSize::Fixed(3),
            border: ContainerBorderConfig {
                enabled: true,
                style: None,
                color: None,
                title: None,
            },
            category: ComponentCategory::Animated,
            kind: ComponentKind::Spinner,
            content: ComponentContent {
                text: Some("Monitoring system telemetry".to_string()),
                ..ComponentContent::default()
            },
            animation: Some(ComponentAnimationConfig {
                fps: Some(12),
                speed: 1.0,
                loop_anim: true,
                style: Some("braille".to_string()),
                ..ComponentAnimationConfig::default()
            }),
            ..ContainerConfig::default()
        },
    ]
}
