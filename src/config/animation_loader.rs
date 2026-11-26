use crate::config::paths;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

pub const CURRENT_ANIMATION_SCHEMA_VERSION: u32 = 1;

pub const SAMPLE_CYBER_BOOT: &str = include_str!("../../assets/animations/cyber_boot.toml");
pub const SAMPLE_NEON_CURTAINS: &str = include_str!("../../assets/animations/neon_curtains.toml");

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum DeclarativeStepEffect {
    CrtLine,
    Curtains,
    #[default]
    Fade,
    Slide,
    MatrixScramble,
    Flash,
    Pause,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum EasingKind {
    Linear,
    EaseIn,
    EaseOut,
    #[default]
    EaseInOut,
}

impl EasingKind {
    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::EaseIn => t * t,
            Self::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            Self::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum StepTarget {
    Screen,
    #[default]
    Window,
    Content,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum StepDirection {
    Left,
    Right,
    Top,
    Bottom,
    #[default]
    Center,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct DeclarativeStep {
    pub effect: DeclarativeStepEffect,
    pub duration_ms: u64,
    pub easing: EasingKind,
    pub target: StepTarget,
    pub direction: StepDirection,
    pub color: Option<String>,
    pub chars: Option<String>,
    pub intensity: f32,
}

impl Default for DeclarativeStep {
    fn default() -> Self {
        Self {
            effect: DeclarativeStepEffect::Fade,
            duration_ms: 400,
            easing: EasingKind::EaseInOut,
            target: StepTarget::Window,
            direction: StepDirection::Center,
            color: None,
            chars: None,
            intensity: 1.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct DeclarativeAnimation {
    pub schema_version: u32,
    pub name: String,
    pub display_name: Option<String>,
    pub author: Option<String>,
    pub description: Option<String>,
    pub steps: Vec<DeclarativeStep>,
}

impl Default for DeclarativeAnimation {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_ANIMATION_SCHEMA_VERSION,
            name: "custom".to_string(),
            display_name: None,
            author: None,
            description: None,
            steps: Vec::new(),
        }
    }
}

impl DeclarativeAnimation {
    pub fn total_duration_secs(&self) -> f32 {
        let total_ms: u64 = self.steps.iter().map(|s| s.duration_ms.max(1)).sum();
        (total_ms as f32 / 1000.0).max(0.05)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version == 0 {
            return Err("schema_version must be >= 1".to_string());
        }
        if self.steps.is_empty() {
            return Err("animation must define at least one [[steps]] entry".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct CommunityAnimationEntry {
    pub spec: DeclarativeAnimation,
    pub source_path: PathBuf,
}

pub fn load_community_animations() -> BTreeMap<String, CommunityAnimationEntry> {
    let mut map = BTreeMap::new();
    let dir = paths::animations_path();
    let Ok(entries) = fs::read_dir(&dir) else {
        return map;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }
        let Ok(raw) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(mut spec) = toml::from_str::<DeclarativeAnimation>(&raw) else {
            continue;
        };
        if spec.validate().is_err() {
            continue;
        }
        if spec.name.trim().is_empty() {
            spec.name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("custom")
                .to_string();
        }
        let key = spec.name.to_ascii_lowercase();
        map.insert(
            key,
            CommunityAnimationEntry {
                spec,
                source_path: path,
            },
        );
    }

    map
}

pub fn ensure_sample_animations() {
    let dir = paths::animations_path();
    if fs::create_dir_all(&dir).is_err() {
        return;
    }
    let samples = [
        ("cyber_boot.toml", SAMPLE_CYBER_BOOT),
        ("neon_curtains.toml", SAMPLE_NEON_CURTAINS),
    ];
    for (filename, content) in samples {
        let target = dir.join(filename);
        if !target.exists() {
            let _ = fs::write(target, content);
        }
    }
}
