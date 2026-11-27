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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct ContainerStyle {
    pub background: BackgroundFill,
    pub foreground: Option<String>,
    pub border_color: Option<String>,
    pub border_style: Option<BorderStyle>,
    pub gradient: Option<ContainerGradient>,
    pub effects: EffectPolicy,
}

pub fn collect_all_container_ids(container: &ContainerConfig, out: &mut Vec<String>) {
    out.push(container.id.clone());
    for child in &container.children {
        match child {
            ContainerNode::Container(sub) => collect_all_container_ids(sub, out),
            ContainerNode::Component(comp) => out.push(comp.id.clone()),
        }
    }
}

pub fn has_container_id(container: &ContainerConfig, id: &str) -> bool {
    if container.id == id {
        return true;
    }
    container.children.iter().any(|child| match child {
        ContainerNode::Container(sub) => has_container_id(sub, id),
        ContainerNode::Component(comp) => comp.id == id,
    })
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ContainerDirection {
    Horizontal,
    #[default]
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerSize {
    Fixed(u16),
    Percent(u16),
    Fill(u16),
}

impl Default for ContainerSize {
    fn default() -> Self {
        Self::Fill(1)
    }
}

impl Serialize for ContainerSize {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
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
                if t == "fill" || t == "auto" {
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
    pub style: BorderStyle,
    pub color: Option<String>,
    pub title: Option<String>,
}

impl Default for ContainerBorderConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            style: BorderStyle::Rounded,
            color: None,
            title: None,
        }
    }
}

impl ContainerBorderConfig {
    pub fn titled(title: impl Into<String>) -> Self {
        Self {
            enabled: true,
            style: BorderStyle::Rounded,
            color: None,
            title: Some(title.into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ContainerConfig {
    pub id: String,
    pub visible: bool,
    pub transparent: bool,
    pub direction: ContainerDirection,
    pub width: ContainerSize,
    pub height: ContainerSize,
    pub min_width: Option<u16>,
    pub max_width: Option<u16>,
    pub min_height: Option<u16>,
    pub max_height: Option<u16>,
    pub padding: u16,
    pub gap: u16,
    pub border: ContainerBorderConfig,
    pub background: Option<String>,
    pub children: Vec<ContainerNode>,
}

impl Default for ContainerConfig {
    fn default() -> Self {
        Self {
            id: "container".to_string(),
            visible: true,
            transparent: true,
            direction: ContainerDirection::Vertical,
            width: ContainerSize::Fill(1),
            height: ContainerSize::Fill(1),
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: 0,
            gap: 0,
            border: ContainerBorderConfig::default(),
            background: None,
            children: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "node", rename_all = "lowercase")]
pub enum ContainerNode {
    Container(ContainerConfig),
    Component(ComponentConfig),
}

impl ContainerNode {
    pub fn id(&self) -> &str {
        match self {
            Self::Container(c) => &c.id,
            Self::Component(c) => &c.id,
        }
    }

    pub fn visible(&self) -> bool {
        match self {
            Self::Container(c) => c.visible,
            Self::Component(c) => c.visible,
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum ComponentKind {
    // Static
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
    Activity,
    Session,
    Header,
    // Animated
    Spinner,
    Typewriter,
    Matrix,
    Particles,
    Frames,
    AsciiObject,
}

impl Default for ComponentKind {
    fn default() -> Self {
        Self::Text
    }
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
            | Self::Activity
            | Self::Session
            | Self::Header => ComponentCategory::Dynamic,
            Self::Spinner
            | Self::Typewriter
            | Self::Matrix
            | Self::Particles
            | Self::Frames
            | Self::AsciiObject => ComponentCategory::Animated,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct ComponentContent {
    pub text: Option<String>,
    pub title: Option<String>,
    pub preset: Option<String>,
    pub frames: Vec<String>,
    pub fields: Option<Vec<String>>,
    pub metric: Option<String>,
    pub separator_char: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ComponentVisual {
    pub align: Align,
    pub padding: u16,
    pub border: ContainerBorderConfig,
    pub color: Option<String>,
    pub background: Option<String>,
    pub font: Option<String>,
    pub graph_style: Option<GraphStyle>,
}

impl Default for ComponentVisual {
    fn default() -> Self {
        Self {
            align: Align::Left,
            padding: 0,
            border: ContainerBorderConfig::default(),
            color: None,
            background: None,
            font: None,
            graph_style: None,
        }
    }
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ComponentConfig {
    pub id: String,
    pub visible: bool,
    pub category: ComponentCategory,
    #[serde(rename = "type")]
    pub kind: ComponentKind,
    pub width: ContainerSize,
    pub height: ContainerSize,
    pub min_width: Option<u16>,
    pub max_width: Option<u16>,
    pub min_height: Option<u16>,
    pub max_height: Option<u16>,
    pub content: ComponentContent,
    pub visual: ComponentVisual,
    pub animation: Option<ComponentAnimationConfig>,
    pub refresh_ms: Option<u64>,
}

impl Default for ComponentConfig {
    fn default() -> Self {
        Self {
            id: "component".to_string(),
            visible: true,
            category: ComponentCategory::Static,
            kind: ComponentKind::Text,
            width: ContainerSize::Fill(1),
            height: ContainerSize::Fill(1),
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            content: ComponentContent::default(),
            visual: ComponentVisual::default(),
            animation: None,
            refresh_ms: None,
        }
    }
}

pub fn default_ui_tree() -> ContainerConfig {
    ContainerConfig {
        id: "root".to_string(),
        visible: true,
        direction: ContainerDirection::Vertical,
        width: ContainerSize::Percent(100),
        height: ContainerSize::Percent(100),
        padding: 1,
        gap: 1,
        border: ContainerBorderConfig::default(),
        children: vec![
            ContainerNode::Component(ComponentConfig {
                id: "header".to_string(),
                visible: true,
                category: ComponentCategory::Dynamic,
                kind: ComponentKind::Header,
                width: ContainerSize::Fill(1),
                height: ContainerSize::Fixed(1),
                ..ComponentConfig::default()
            }),
            ContainerNode::Component(ComponentConfig {
                id: "greeting".to_string(),
                visible: true,
                category: ComponentCategory::Static,
                kind: ComponentKind::Greeting,
                width: ContainerSize::Fill(1),
                height: ContainerSize::Fixed(10),
                min_height: Some(6),
                visual: ComponentVisual {
                    align: Align::Center,
                    padding: 0,
                    border: ContainerBorderConfig::titled("Welcome"),
                    font: Some("ANSI Shadow".to_string()),
                    ..ComponentVisual::default()
                },
                ..ComponentConfig::default()
            }),
            ContainerNode::Container(ContainerConfig {
                id: "middle_row".to_string(),
                visible: true,
                direction: ContainerDirection::Horizontal,
                width: ContainerSize::Fill(1),
                height: ContainerSize::Fill(1),
                min_height: Some(6),
                gap: 1,
                children: vec![
                    ContainerNode::Component(ComponentConfig {
                        id: "system".to_string(),
                        visible: true,
                        category: ComponentCategory::Dynamic,
                        kind: ComponentKind::System,
                        width: ContainerSize::Percent(50),
                        height: ContainerSize::Fill(1),
                        visual: ComponentVisual {
                            align: Align::Left,
                            padding: 1,
                            border: ContainerBorderConfig::titled("System"),
                            ..ComponentVisual::default()
                        },
                        ..ComponentConfig::default()
                    }),
                    ContainerNode::Component(ComponentConfig {
                        id: "hardware".to_string(),
                        visible: true,
                        category: ComponentCategory::Dynamic,
                        kind: ComponentKind::Hardware,
                        width: ContainerSize::Percent(50),
                        height: ContainerSize::Fill(1),
                        visual: ComponentVisual {
                            align: Align::Left,
                            padding: 1,
                            border: ContainerBorderConfig::titled("Hardware"),
                            graph_style: Some(GraphStyle::Bar),
                            ..ComponentVisual::default()
                        },
                        ..ComponentConfig::default()
                    }),
                ],
                ..ContainerConfig::default()
            }),
            ContainerNode::Container(ContainerConfig {
                id: "bottom_row".to_string(),
                visible: true,
                direction: ContainerDirection::Horizontal,
                width: ContainerSize::Fill(1),
                height: ContainerSize::Fill(1),
                min_height: Some(6),
                gap: 1,
                children: vec![
                    ContainerNode::Component(ComponentConfig {
                        id: "activity".to_string(),
                        visible: true,
                        category: ComponentCategory::Dynamic,
                        kind: ComponentKind::Activity,
                        width: ContainerSize::Percent(60),
                        height: ContainerSize::Fill(1),
                        visual: ComponentVisual {
                            align: Align::Left,
                            padding: 1,
                            border: ContainerBorderConfig::titled("Activity"),
                            graph_style: Some(GraphStyle::Sparkline),
                            ..ComponentVisual::default()
                        },
                        ..ComponentConfig::default()
                    }),
                    ContainerNode::Component(ComponentConfig {
                        id: "session".to_string(),
                        visible: true,
                        category: ComponentCategory::Dynamic,
                        kind: ComponentKind::Session,
                        width: ContainerSize::Percent(40),
                        height: ContainerSize::Fill(1),
                        visual: ComponentVisual {
                            align: Align::Left,
                            padding: 1,
                            border: ContainerBorderConfig::titled("Session & Uptime"),
                            ..ComponentVisual::default()
                        },
                        ..ComponentConfig::default()
                    }),
                ],
                ..ContainerConfig::default()
            }),
            ContainerNode::Component(ComponentConfig {
                id: "status_spinner".to_string(),
                visible: true,
                category: ComponentCategory::Animated,
                kind: ComponentKind::Spinner,
                width: ContainerSize::Fill(1),
                height: ContainerSize::Fixed(1),
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
                ..ComponentConfig::default()
            }),
        ],
        ..ContainerConfig::default()
    }
}
