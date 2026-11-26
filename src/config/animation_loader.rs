use crate::animation::backgrounds::BackgroundLayerSpec;
use crate::animation::easing::EasingCurve;
use crate::animation::geometry::{GeometryRenderer, ShapePrimitive};
use crate::animation::scene::{CoordValue, LayerSpec};
use crate::animation::text_fx::{TextDirection, TextEffectKind};
use crate::animation::timeline::{LoopConfig, LoopMode};
use crate::config::paths;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

pub const CURRENT_ANIMATION_SCHEMA_VERSION: u32 = 2;
pub const MAX_LAYERS: usize = 16;
pub const MAX_TRACKS: usize = 32;
pub const MAX_BACKGROUNDS: usize = 8;
pub const MAX_KEYFRAMES_PER_TRACK: usize = 64;
pub const MAX_PARTICLES_PER_TRACK: usize = 500;
pub const MAX_PATH_POINTS: usize = 128;
pub const MAX_ANIMATION_DURATION_MS: u64 = 300_000;
pub const MAX_ASSET_BYTES: u64 = 65_536;

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
    CursorExpand,
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
        match self {
            Self::Linear => EasingCurve::Linear.apply(t),
            Self::EaseIn => EasingCurve::EaseIn.apply(t),
            Self::EaseOut => EasingCurve::EaseOut.apply(t),
            Self::EaseInOut => EasingCurve::EaseInOut.apply(t),
        }
    }

    pub fn into_curve(self) -> EasingCurve {
        match self {
            Self::Linear => EasingCurve::Linear,
            Self::EaseIn => EasingCurve::EaseIn,
            Self::EaseOut => EasingCurve::EaseOut,
            Self::EaseInOut => EasingCurve::EaseInOut,
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
    Overlay,
    Background,
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

/// Legacy schema v1 declarative animation manifest (backwards-compatible with `[[steps]]`).
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
            schema_version: 1,
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

    /// Converts a legacy v1 step sequence into a unified `AnimationDefinition`.
    pub fn into_definition(self) -> AnimationDefinition {
        let mut cursor_ms = 0u64;
        let mut tracks = Vec::with_capacity(self.steps.len());
        let mut max_duration = 0u64;
        for (idx, step) in self.steps.iter().enumerate() {
            let dur = step.duration_ms;
            let track = TrackSpec {
                id: format!("step_{idx}"),
                kind: TrackKind::Transition,
                layer: Some("transition".to_string()),
                z_index: 100 + idx as i32,
                effect: Some(step.effect),
                transition_easing: step.easing.into_curve(),
                target: step.target,
                transition_direction: step.direction,
                color: step.color.clone(),
                chars: step.chars.clone(),
                intensity: step.intensity,
                start_ms: cursor_ms,
                duration_ms: Some(dur),
                ..TrackSpec::default()
            };
            cursor_ms = cursor_ms.saturating_add(dur.max(1));
            max_duration = max_duration.max(cursor_ms);
            tracks.push(track);
        }

        let is_instant = self
            .steps
            .iter()
            .all(|s| s.duration_ms == 0 || s.effect == DeclarativeStepEffect::Pause);

        AnimationDefinition {
            schema_version: self.schema_version,
            name: self.name,
            display_name: self.display_name,
            author: self.author,
            description: self.description,
            duration_ms: if is_instant { 0 } else { max_duration },
            fps: 30,
            speed: 1.0,
            target: StepTarget::Window,
            mode: AnimationRenderMode::Transition,
            loop_cfg: LoopConfig::default(),
            layers: vec![LayerSpec {
                id: "transition".to_string(),
                z_index: 100,
                visible: true,
                opacity: 1.0,
            }],
            tracks,
            backgrounds: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AnimationRenderMode {
    #[default]
    Scene,
    Transition,
    Composite,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TrackKind {
    #[default]
    Shape,
    Text,
    Ascii,
    Particles,
    Transition,
    Background,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum MorphTransitionMode {
    #[default]
    Morph,
    Snap,
    Fade,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct KeyframeSpec {
    pub at_ms: u64,
    pub shape: Option<ShapePrimitive>,
    pub sides: Option<u16>,
    pub path: Option<Vec<[f32; 2]>>,
    pub position: Option<[CoordValue; 2]>,
    pub size: Option<[CoordValue; 2]>,
    pub color: Option<String>,
    pub fill_color: Option<String>,
    pub rotation: Option<f32>,
    pub scale: Option<f32>,
    pub opacity: Option<f32>,
    pub intensity: Option<f32>,
    pub speed: Option<f32>,
    pub visible: Option<bool>,
    pub text: Option<String>,
    pub frame: Option<usize>,
    pub transition: MorphTransitionMode,
    pub easing: EasingCurve,
}

impl Default for KeyframeSpec {
    fn default() -> Self {
        Self {
            at_ms: 0,
            shape: None,
            sides: None,
            path: None,
            position: None,
            size: None,
            color: None,
            fill_color: None,
            rotation: None,
            scale: None,
            opacity: None,
            intensity: None,
            speed: None,
            visible: None,
            text: None,
            frame: None,
            transition: MorphTransitionMode::Morph,
            easing: EasingCurve::EaseInOut,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct TrackSpec {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: TrackKind,
    pub layer: Option<String>,
    pub z_index: i32,
    pub visible: bool,
    pub opacity: f32,
    pub position: [CoordValue; 2],
    pub size: [CoordValue; 2],
    pub color: Option<String>,
    pub fill_color: Option<String>,
    pub rotation: f32,
    pub scale: f32,
    pub speed: f32,
    pub intensity: f32,
    // Geometry properties
    pub primitive: ShapePrimitive,
    pub sides: u16,
    pub renderer: GeometryRenderer,
    pub fill: bool,
    pub stroke: bool,
    pub stroke_char: String,
    pub fill_char: String,
    pub path: Vec<[f32; 2]>,
    // Text properties
    pub text: Option<String>,
    pub text_effect: TextEffectKind,
    pub direction: TextDirection,
    pub palette: Vec<String>,
    pub chars: Option<String>,
    // ASCII / Particle properties
    pub asset: Option<String>,
    pub sprite: Option<String>,
    pub frames: Vec<String>,
    pub count: usize,
    // Transition step properties
    pub effect: Option<DeclarativeStepEffect>,
    pub transition_easing: EasingCurve,
    pub target: StepTarget,
    pub transition_direction: StepDirection,
    pub start_ms: u64,
    pub duration_ms: Option<u64>,
    // Keyframes
    pub keyframes: Vec<KeyframeSpec>,
}

impl Default for TrackSpec {
    fn default() -> Self {
        Self {
            id: "track".to_string(),
            kind: TrackKind::Shape,
            layer: None,
            z_index: 10,
            visible: true,
            opacity: 1.0,
            position: [CoordValue::default(), CoordValue::default()],
            size: [CoordValue::Cells(20.0), CoordValue::Cells(12.0)],
            color: None,
            fill_color: None,
            rotation: 0.0,
            scale: 1.0,
            speed: 1.0,
            intensity: 1.0,
            primitive: ShapePrimitive::Circle,
            sides: 6,
            renderer: GeometryRenderer::Braille,
            fill: false,
            stroke: true,
            stroke_char: "*".to_string(),
            fill_char: "#".to_string(),
            path: Vec::new(),
            text: None,
            text_effect: TextEffectKind::None,
            direction: TextDirection::Left,
            palette: Vec::new(),
            chars: None,
            asset: None,
            sprite: None,
            frames: Vec::new(),
            count: 36,
            effect: None,
            transition_easing: EasingCurve::EaseInOut,
            target: StepTarget::Window,
            transition_direction: StepDirection::Center,
            start_ms: 0,
            duration_ms: None,
            keyframes: Vec::new(),
        }
    }
}

/// Unified declarative animation definition shared identically by bundled and community packages.
#[derive(Debug, Clone, PartialEq)]
pub struct AnimationDefinition {
    pub schema_version: u32,
    pub name: String,
    pub display_name: Option<String>,
    pub author: Option<String>,
    pub description: Option<String>,
    pub duration_ms: u64,
    pub fps: u32,
    pub speed: f32,
    pub target: StepTarget,
    pub mode: AnimationRenderMode,
    pub loop_cfg: LoopConfig,
    pub layers: Vec<LayerSpec>,
    pub tracks: Vec<TrackSpec>,
    pub backgrounds: Vec<BackgroundLayerSpec>,
}

impl AnimationDefinition {
    pub fn parse_toml(raw: &str) -> Result<Self, String> {
        let parsed: RawAnimationManifest =
            toml::from_str(raw).map_err(|e| format!("Invalid animation TOML: {e}"))?;
        let def = parsed.into_definition()?;
        def.validate()?;
        Ok(def)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version == 0 || self.schema_version > CURRENT_ANIMATION_SCHEMA_VERSION {
            return Err(format!(
                "Unsupported schema_version {} (supported: 1..={CURRENT_ANIMATION_SCHEMA_VERSION})",
                self.schema_version
            ));
        }
        if self.name.trim().is_empty() {
            return Err("Animation name cannot be empty".to_string());
        }
        if self.fps == 0 || self.fps > 120 {
            return Err(format!("fps must be within 1..=120 (got {})", self.fps));
        }
        if self.duration_ms > MAX_ANIMATION_DURATION_MS {
            return Err(format!(
                "duration_ms {} exceeds limit {MAX_ANIMATION_DURATION_MS}",
                self.duration_ms
            ));
        }
        if self.layers.len() > MAX_LAYERS {
            return Err(format!("Too many layers (max {MAX_LAYERS})"));
        }
        if self.tracks.len() > MAX_TRACKS {
            return Err(format!("Too many tracks (max {MAX_TRACKS})"));
        }
        if self.backgrounds.len() > MAX_BACKGROUNDS {
            return Err(format!("Too many background layers (max {MAX_BACKGROUNDS})"));
        }
        if self.tracks.is_empty() && self.backgrounds.is_empty() {
            return Err(
                "Animation must define at least one [[tracks]], [[backgrounds]], or [[steps]] entry"
                    .to_string(),
            );
        }
        for track in &self.tracks {
            if track.keyframes.len() > MAX_KEYFRAMES_PER_TRACK {
                return Err(format!(
                    "Track '{}' exceeds max {MAX_KEYFRAMES_PER_TRACK} keyframes",
                    track.id
                ));
            }
            if track.count > MAX_PARTICLES_PER_TRACK {
                return Err(format!(
                    "Track '{}' particle count {} exceeds max {MAX_PARTICLES_PER_TRACK}",
                    track.id, track.count
                ));
            }
            if track.path.len() > MAX_PATH_POINTS {
                return Err(format!(
                    "Track '{}' path points exceed max {MAX_PATH_POINTS}",
                    track.id
                ));
            }
            if let Some(asset) = &track.asset {
                validate_safe_asset_path(asset)?;
            }
            for kf in &track.keyframes {
                if let Some(p) = &kf.path
                    && p.len() > MAX_PATH_POINTS
                {
                    return Err(format!(
                        "Keyframe path points exceed max {MAX_PATH_POINTS} in track '{}'",
                        track.id
                    ));
                }
            }
        }
        for bg in &self.backgrounds {
            if let Some(asset) = &bg.asset {
                validate_safe_asset_path(asset)?;
            }
        }
        Ok(())
    }

    pub fn is_looping(&self) -> bool {
        matches!(
            self.loop_cfg.mode,
            LoopMode::Infinite | LoopMode::Repeat | LoopMode::PingPong
        )
    }

    pub fn has_scene_elements(&self) -> bool {
        self.tracks
            .iter()
            .any(|t| !matches!(t.kind, TrackKind::Transition))
            || !self.backgrounds.is_empty()
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct AnimationHeader {
    name: Option<String>,
    display_name: Option<String>,
    author: Option<String>,
    description: Option<String>,
    version: Option<u32>,
    schema_version: Option<u32>,
    duration_ms: Option<u64>,
    fps: Option<u32>,
    speed: Option<f32>,
    target: Option<StepTarget>,
    mode: Option<AnimationRenderMode>,
    #[serde(rename = "loop")]
    loop_cfg: Option<LoopConfig>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct RawAnimationManifest {
    animation: Option<AnimationHeader>,
    schema_version: Option<u32>,
    version: Option<u32>,
    name: Option<String>,
    display_name: Option<String>,
    author: Option<String>,
    description: Option<String>,
    duration_ms: Option<u64>,
    fps: Option<u32>,
    speed: Option<f32>,
    target: Option<StepTarget>,
    mode: Option<AnimationRenderMode>,
    #[serde(rename = "loop")]
    loop_cfg: Option<LoopConfig>,
    layers: Vec<LayerSpec>,
    tracks: Vec<TrackSpec>,
    backgrounds: Vec<BackgroundLayerSpec>,
    steps: Vec<DeclarativeStep>,
}

impl RawAnimationManifest {
    fn into_definition(self) -> Result<AnimationDefinition, String> {
        let hdr = self.animation.unwrap_or_default();
        let schema_version = hdr
            .schema_version
            .or(hdr.version)
            .or(self.schema_version)
            .or(self.version)
            .unwrap_or(1);

        let name = hdr
            .name
            .or(self.name)
            .unwrap_or_else(|| "custom".to_string());
        let display_name = hdr.display_name.or(self.display_name);
        let author = hdr.author.or(self.author);
        let description = hdr.description.or(self.description);

        // If no v2 tracks/backgrounds were supplied but v1 [[steps]] are present, upgrade cleanly.
        if self.tracks.is_empty() && self.backgrounds.is_empty() && !self.steps.is_empty() {
            let legacy = DeclarativeAnimation {
                schema_version,
                name,
                display_name,
                author,
                description,
                steps: self.steps,
            };
            legacy.validate()?;
            return Ok(legacy.into_definition());
        }

        let mut tracks = self.tracks;
        // Also allow hybrid manifests with both `[[steps]]` and `[[tracks]]`.
        if !self.steps.is_empty() {
            let mut cursor_ms = 0u64;
            for (idx, step) in self.steps.iter().enumerate() {
                tracks.push(TrackSpec {
                    id: format!("step_{idx}"),
                    kind: TrackKind::Transition,
                    layer: Some("transition".to_string()),
                    z_index: 100 + idx as i32,
                    effect: Some(step.effect),
                    transition_easing: step.easing.into_curve(),
                    target: step.target,
                    transition_direction: step.direction,
                    color: step.color.clone(),
                    chars: step.chars.clone(),
                    intensity: step.intensity,
                    start_ms: cursor_ms,
                    duration_ms: Some(step.duration_ms),
                    ..TrackSpec::default()
                });
                cursor_ms = cursor_ms.saturating_add(step.duration_ms.max(1));
            }
        }

        let inferred_duration = tracks
            .iter()
            .map(|t| {
                let kf_max = t.keyframes.iter().map(|k| k.at_ms).max().unwrap_or(0);
                let step_end = t.duration_ms.map(|d| t.start_ms.saturating_add(d)).unwrap_or(0);
                kf_max.max(step_end)
            })
            .max()
            .unwrap_or(2000)
            .max(500);

        let loop_cfg = hdr.loop_cfg.or(self.loop_cfg).unwrap_or_default();
        let target = hdr.target.or(self.target).unwrap_or(StepTarget::Screen);
        let mode = hdr.mode.or(self.mode).unwrap_or_else(|| {
            if tracks.iter().all(|t| t.kind == TrackKind::Transition) && self.backgrounds.is_empty()
            {
                AnimationRenderMode::Transition
            } else {
                AnimationRenderMode::Scene
            }
        });

        Ok(AnimationDefinition {
            schema_version,
            name,
            display_name,
            author,
            description,
            duration_ms: hdr
                .duration_ms
                .or(self.duration_ms)
                .unwrap_or(inferred_duration),
            fps: hdr.fps.or(self.fps).unwrap_or(30),
            speed: hdr.speed.or(self.speed).unwrap_or(1.0).clamp(0.1, 10.0),
            target,
            mode,
            loop_cfg,
            layers: self.layers,
            tracks,
            backgrounds: self.backgrounds,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageOrigin {
    Bundled,
    Community {
        root_dir: Option<PathBuf>,
        manifest_path: PathBuf,
    },
}

/// Self-contained animation package with manifest, validated relative assets, and origin metadata.
#[derive(Debug, Clone)]
pub struct AnimationPackage {
    pub definition: AnimationDefinition,
    pub origin: PackageOrigin,
    pub assets: BTreeMap<String, String>,
    pub has_preview: bool,
}

#[derive(Debug, Clone)]
pub struct CommunityAnimationEntry {
    pub spec: DeclarativeAnimation,
    pub source_path: PathBuf,
}

/// Validates that an asset reference stays strictly relative inside its package directory.
pub fn validate_safe_asset_path(raw: &str) -> Result<(), String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("Asset path cannot be empty".to_string());
    }
    if trimmed.len() > 128
        || trimmed.contains('\0')
        || trimmed.contains("://")
        || trimmed.contains('\\')
    {
        return Err(format!("Unsafe asset path '{raw}'"));
    }
    let p = Path::new(trimmed);
    if p.is_absolute() {
        return Err(format!("Absolute asset path is forbidden: '{raw}'"));
    }
    for comp in p.components() {
        match comp {
            Component::Normal(_) | Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(format!(
                    "Path traversal in asset reference is forbidden: '{raw}'"
                ));
            }
        }
    }
    Ok(())
}

/// Loads all community animation packages from `~/.config/egotop/animations/`,
/// supporting both `<pkg>/animation.toml` directory bundles and standalone `*.toml` files.
pub fn load_community_packages() -> BTreeMap<String, AnimationPackage> {
    let dir = paths::animations_path();
    load_packages_from_dir(&dir)
}

/// Loads animation packages from a custom root directory (used by runtime loader & sandbox tests).
pub fn load_packages_from_dir(dir: &Path) -> BTreeMap<String, AnimationPackage> {
    let mut map = BTreeMap::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return map;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let manifest = path.join("animation.toml");
            if manifest.is_file()
                && let Ok(pkg) = load_package_directory(&path, &manifest)
            {
                let key = pkg.definition.name.to_ascii_lowercase();
                map.insert(key, pkg);
            }
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) == Some("toml")
            && let Ok(raw) = fs::read_to_string(&path)
            && let Ok(mut def) = AnimationDefinition::parse_toml(&raw)
        {
            // Standalone TOML files cannot reach external files outside a package folder.
            if def.tracks.iter().any(|t| t.asset.is_some())
                || def.backgrounds.iter().any(|b| b.asset.is_some())
            {
                continue;
            }
            if def.name == "custom" {
                def.name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("custom")
                    .to_string();
            }
            let key = def.name.to_ascii_lowercase();
            map.insert(
                key,
                AnimationPackage {
                    definition: def,
                    origin: PackageOrigin::Community {
                        root_dir: None,
                        manifest_path: path,
                    },
                    assets: BTreeMap::new(),
                    has_preview: false,
                },
            );
        }
    }

    map
}

fn load_package_directory(pkg_dir: &Path, manifest: &Path) -> Result<AnimationPackage, String> {
    let meta = fs::metadata(manifest).map_err(|e| format!("Cannot read manifest: {e}"))?;
    if meta.len() > MAX_ASSET_BYTES * 2 {
        return Err("animation.toml exceeds maximum manifest size".to_string());
    }
    let raw = fs::read_to_string(manifest).map_err(|e| format!("Cannot read manifest: {e}"))?;
    let mut def = AnimationDefinition::parse_toml(&raw)?;
    if def.name == "custom"
        && let Some(folder_name) = pkg_dir.file_name().and_then(|s| s.to_str())
    {
        def.name = folder_name.to_string();
    }

    let canonical_root =
        fs::canonicalize(pkg_dir).map_err(|e| format!("Cannot canonicalize package dir: {e}"))?;
    let mut assets = BTreeMap::new();

    let asset_keys: Vec<String> = def
        .tracks
        .iter()
        .filter_map(|t| t.asset.clone())
        .chain(def.backgrounds.iter().filter_map(|b| b.asset.clone()))
        .collect();

    for rel in asset_keys {
        if assets.contains_key(&rel) {
            continue;
        }
        validate_safe_asset_path(&rel)?;
        let candidate = canonical_root.join(&rel);
        let canonical_file = fs::canonicalize(&candidate)
            .map_err(|e| format!("Missing asset '{rel}': {e}"))?;
        if !canonical_file.starts_with(&canonical_root) {
            return Err(format!("Symlink escape blocked for asset '{rel}'"));
        }
        let file_meta =
            fs::metadata(&canonical_file).map_err(|e| format!("Cannot stat asset '{rel}': {e}"))?;
        if !file_meta.is_file() {
            return Err(format!("Asset '{rel}' is not a regular file"));
        }
        if file_meta.len() > MAX_ASSET_BYTES {
            return Err(format!(
                "Asset '{rel}' exceeds {MAX_ASSET_BYTES} bytes limit"
            ));
        }
        let content = fs::read_to_string(&canonical_file)
            .map_err(|e| format!("Asset '{rel}' must be valid UTF-8 text: {e}"))?;
        assets.insert(rel, content);
    }

    let has_preview = pkg_dir.join("preview.png").is_file();
    Ok(AnimationPackage {
        definition: def,
        origin: PackageOrigin::Community {
            root_dir: Some(pkg_dir.to_path_buf()),
            manifest_path: manifest.to_path_buf(),
        },
        assets,
        has_preview,
    })
}

/// Legacy v1 community loader kept for backwards compatibility with existing callers/tests.
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
