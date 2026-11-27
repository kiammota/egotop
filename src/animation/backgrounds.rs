use crate::animation::resolve_region;
use crate::animation::scene::{collect_ascii_sprite_cells, composite_scene_items, resolve_scene_color, SceneDrawItem};
use crate::animation::{
    BackgroundEffect, MatrixEffect, NoneEffect, ParticlesEffect, ScanlinesEffect, WaterEffect,
};
use crate::config::{BackgroundConfig, EffectKind, RegionConfig};
use crate::ui::theme::{Theme, adapt_color, blend_color, terminal_supports_truecolor};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// All supported animated background layer algorithms.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum BackgroundLayerKind {
    #[serde(alias = "matrix")]
    #[default]
    MatrixRain,
    Starfield,
    Particles,
    #[serde(alias = "water")]
    Waves,
    Noise,
    Gradient,
    FloatingChars,
    CustomAscii,
    Scanlines,
    None,
}

impl BackgroundLayerKind {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "matrix" | "matrix_rain" => Some(Self::MatrixRain),
            "starfield" | "stars" => Some(Self::Starfield),
            "particles" | "particle" => Some(Self::Particles),
            "waves" | "wave" | "water" => Some(Self::Waves),
            "noise" | "plasma" => Some(Self::Noise),
            "gradient" | "aurora" => Some(Self::Gradient),
            "floating_chars" | "floating" | "chars" => Some(Self::FloatingChars),
            "custom_ascii" | "ascii" => Some(Self::CustomAscii),
            "scanlines" | "crt" => Some(Self::Scanlines),
            "none" | "off" => Some(Self::None),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::MatrixRain => "matrix_rain",
            Self::Starfield => "starfield",
            Self::Particles => "particles",
            Self::Waves => "waves",
            Self::Noise => "noise",
            Self::Gradient => "gradient",
            Self::FloatingChars => "floating_chars",
            Self::CustomAscii => "custom_ascii",
            Self::Scanlines => "scanlines",
            Self::None => "none",
        }
    }

    pub fn all_selectable() -> &'static [Self] {
        &[
            Self::MatrixRain,
            Self::Starfield,
            Self::Particles,
            Self::Waves,
            Self::Noise,
            Self::Gradient,
            Self::FloatingChars,
            Self::CustomAscii,
            Self::Scanlines,
            Self::None,
        ]
    }

    pub fn from_effect_kind(kind: EffectKind) -> Self {
        match kind {
            EffectKind::Matrix => Self::MatrixRain,
            EffectKind::Water => Self::Waves,
            EffectKind::Particles => Self::Particles,
            EffectKind::Scanlines => Self::Scanlines,
            EffectKind::None => Self::None,
        }
    }
}

/// Declarative configuration for a single composable background layer inside TOML or runtime.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct BackgroundLayerSpec {
    pub effect: BackgroundLayerKind,
    pub z_index: i32,
    pub visible: bool,
    pub opacity: f32,
    pub speed: f32,
    pub density: f32,
    pub intensity: f32,
    pub color: Option<String>,
    pub secondary_color: Option<String>,
    pub chars: Option<String>,
    pub direction: Option<String>,
    pub asset: Option<String>,
    pub sprite: Option<String>,
    pub region: Option<RegionConfig>,
}

impl Default for BackgroundLayerSpec {
    fn default() -> Self {
        Self {
            effect: BackgroundLayerKind::None,
            z_index: 0,
            visible: true,
            opacity: 1.0,
            speed: 1.0,
            density: 0.6,
            intensity: 0.75,
            color: None,
            secondary_color: None,
            chars: None,
            direction: None,
            asset: None,
            sprite: None,
            region: None,
        }
    }
}

/// Stateful runtime instance for an animated background layer.
pub enum BackgroundLayerRuntime {
    Legacy(Box<dyn BackgroundEffect>),
    Lua {
        name: String,
        spec: Box<BackgroundLayerSpec>,
        adapter: Box<crate::lua::LuaEffectAdapter>,
    },
    Procedural {
        spec: Box<BackgroundLayerSpec>,
        elapsed: f32,
        stars: Vec<StarDot>,
        glyphs: Vec<FloatingGlyph>,
        ascii_sprite: String,
    },
}

#[derive(Debug, Clone)]
pub struct StarDot {
    pub x: f32,
    pub y: f32,
    pub depth: f32,
}

#[derive(Debug, Clone)]
pub struct FloatingGlyph {
    pub x: f32,
    pub y: f32,
    pub phase: f32,
    pub speed_mul: f32,
    pub ch: char,
}

impl BackgroundLayerRuntime {
    pub fn from_legacy(effect: Box<dyn BackgroundEffect>) -> Self {
        Self::Legacy(effect)
    }

    pub fn from_spec(
        spec: BackgroundLayerSpec,
        width: u16,
        height: u16,
        cfg: &BackgroundConfig,
        assets: &BTreeMap<String, String>,
    ) -> Self {
        Self::from_spec_with_theme(spec, width, height, cfg, assets, None)
    }

    pub fn from_lua_spec(
        lua_spec: &crate::lua::LuaEffectSpec,
        width: u16,
        height: u16,
        theme: &Theme,
    ) -> Self {
        let layer_kind =
            BackgroundLayerKind::parse(&lua_spec.name).unwrap_or(BackgroundLayerKind::CustomAscii);
        let spec = BackgroundLayerSpec {
            effect: layer_kind,
            speed: lua_spec.speed,
            density: lua_spec.density,
            color: lua_spec.color.clone(),
            secondary_color: lua_spec.head_color.clone(),
            chars: lua_spec.charset.clone(),
            asset: Some(lua_spec.name.clone()),
            ..Default::default()
        };
        let adapter = crate::lua::LuaEffectAdapter::from_spec(
            lua_spec.clone(),
            width,
            height,
            theme.clone(),
        );
        Self::Lua {
            name: lua_spec.name.to_ascii_lowercase(),
            spec: Box::new(spec),
            adapter: Box::new(adapter),
        }
    }

    pub fn from_spec_with_theme(
        spec: BackgroundLayerSpec,
        width: u16,
        height: u16,
        cfg: &BackgroundConfig,
        assets: &BTreeMap<String, String>,
        theme_opt: Option<&Theme>,
    ) -> Self {
        let candidate = spec
            .asset
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| spec.effect.label());

        if spec.effect == BackgroundLayerKind::None {
            return Self::Legacy(Box::new(NoneEffect));
        }

        // Unified Lua canvas resolution for all named animations/effects without special-casing any effect.
        if !matches!(
            spec.effect,
            BackgroundLayerKind::Gradient | BackgroundLayerKind::CustomAscii
        ) || spec.asset.as_deref().is_some_and(|a| crate::lua::resolve_canvas_module_name(a).is_some())
        {
            if let Some(mod_name) = crate::lua::resolve_canvas_module_name(candidate) {
                let _ = mod_name;
                let lua_spec = crate::lua::LuaEffectSpec {
                    name: candidate.to_string(),
                    fps: 30,
                    density: spec.density,
                    speed: spec.speed,
                    color: spec.color.clone(),
                    head_color: spec.secondary_color.clone(),
                    charset: spec.chars.clone(),
                    spacing: None,
                    char_glyph: None,
                    count: None,
                };
                let theme = theme_opt
                    .cloned()
                    .unwrap_or_else(|| Theme::from_config(&crate::config::ThemeConfig::default()));
                let adapter =
                    crate::lua::LuaEffectAdapter::from_spec(lua_spec, width, height, theme);
                return Self::Lua {
                    name: candidate.to_ascii_lowercase(),
                    spec: Box::new(spec),
                    adapter: Box::new(adapter),
                };
            }
        }

        match spec.effect {
            BackgroundLayerKind::MatrixRain => {
                let mut m_cfg = cfg.clone();
                m_cfg.matrix.enabled = true;
                m_cfg.matrix.speed = (cfg.matrix.speed * spec.speed).max(0.05);
                m_cfg.matrix.density = spec.density.clamp(0.05, 1.0);
                if let Some(c) = &spec.color {
                    m_cfg.matrix.color = c.clone();
                }
                if let Some(hc) = &spec.secondary_color {
                    m_cfg.matrix.head_color = hc.clone();
                }
                if let Some(reg) = &spec.region {
                    m_cfg.matrix.region = Some(reg.clone());
                }
                let mut eff = MatrixEffect::new();
                eff.resize(width, height, &m_cfg);
                Self::Legacy(Box::new(eff))
            }
            BackgroundLayerKind::Particles => {
                let mut p_cfg = cfg.clone();
                p_cfg.particles.enabled = true;
                p_cfg.particles.speed = (cfg.particles.speed * spec.speed).max(0.05);
                p_cfg.particles.density = spec.density.clamp(0.05, 1.0);
                if let Some(c) = &spec.color {
                    p_cfg.particles.color = c.clone();
                }
                if let Some(chars) = &spec.chars {
                    p_cfg.particles.chars = chars.clone();
                }
                if let Some(reg) = &spec.region {
                    p_cfg.particles.region = Some(reg.clone());
                }
                let mut eff = ParticlesEffect::new();
                eff.resize(width, height, &p_cfg);
                Self::Legacy(Box::new(eff))
            }
            BackgroundLayerKind::Waves => {
                let mut w_cfg = cfg.clone();
                w_cfg.water.enabled = true;
                w_cfg.water.speed = (cfg.water.speed * spec.speed).max(0.05);
                w_cfg.water.density = spec.density.clamp(0.05, 1.0);
                w_cfg.water.amplitude = spec.intensity.clamp(0.1, 1.5);
                if let Some(c) = &spec.color {
                    w_cfg.water.color = c.clone();
                }
                if let Some(cc) = &spec.secondary_color {
                    w_cfg.water.crest_color = cc.clone();
                }
                if let Some(reg) = &spec.region {
                    w_cfg.water.region = Some(reg.clone());
                }
                let mut eff = WaterEffect::new();
                eff.resize(width, height, &w_cfg);
                Self::Legacy(Box::new(eff))
            }
            BackgroundLayerKind::Scanlines => {
                let mut s_cfg = cfg.clone();
                s_cfg.scanlines.enabled = true;
                s_cfg.scanlines.speed = (cfg.scanlines.speed * spec.speed).max(0.05);
                s_cfg.scanlines.intensity = spec.intensity.clamp(0.05, 1.0);
                if let Some(c) = &spec.color {
                    s_cfg.scanlines.color = c.clone();
                }
                if let Some(reg) = &spec.region {
                    s_cfg.scanlines.region = Some(reg.clone());
                }
                let mut eff = ScanlinesEffect::new();
                eff.resize(width, height, &s_cfg);
                Self::Legacy(Box::new(eff))
            }
            BackgroundLayerKind::None => Self::Legacy(Box::new(NoneEffect)),
            BackgroundLayerKind::Starfield => Self::Procedural {
                stars: seed_stars(width, height, spec.density),
                glyphs: Vec::new(),
                ascii_sprite: String::new(),
                spec: Box::new(spec),
                elapsed: 0.0,
            },
            BackgroundLayerKind::FloatingChars => Self::Procedural {
                stars: Vec::new(),
                glyphs: seed_floating_glyphs(width, height, &spec),
                ascii_sprite: String::new(),
                spec: Box::new(spec),
                elapsed: 0.0,
            },
            BackgroundLayerKind::CustomAscii => {
                let sprite = spec
                    .asset
                    .as_deref()
                    .and_then(|k| assets.get(k))
                    .cloned()
                    .or_else(|| spec.sprite.clone())
                    .unwrap_or_default();
                Self::Procedural {
                    stars: Vec::new(),
                    glyphs: Vec::new(),
                    ascii_sprite: sprite,
                    spec: Box::new(spec),
                    elapsed: 0.0,
                }
            }
            BackgroundLayerKind::Noise | BackgroundLayerKind::Gradient => Self::Procedural {
                stars: Vec::new(),
                glyphs: Vec::new(),
                ascii_sprite: String::new(),
                spec: Box::new(spec),
                elapsed: 0.0,
            },
        }
    }

    pub fn resize(&mut self, width: u16, height: u16, cfg: &BackgroundConfig) {
        match self {
            Self::Legacy(eff) => eff.resize(width, height, cfg),
            Self::Lua { adapter, .. } => adapter.resize(width, height, cfg),
            Self::Procedural {
                spec,
                stars,
                glyphs,
                ..
            } => {
                if spec.effect == BackgroundLayerKind::Starfield {
                    *stars = seed_stars(width, height, spec.density);
                } else if spec.effect == BackgroundLayerKind::FloatingChars {
                    *glyphs = seed_floating_glyphs(width, height, spec);
                }
            }
        }
    }

    pub fn update(&mut self, dt_secs: f32, cfg: &BackgroundConfig) {
        match self {
            Self::Legacy(eff) => eff.update(dt_secs, cfg),
            Self::Lua { spec, adapter, .. } => {
                let mut lcfg = cfg.clone();
                lcfg.speed = (cfg.speed * spec.speed).clamp(0.05, 10.0);
                lcfg.density = spec.density.clamp(0.05, 1.0);
                adapter.update(dt_secs, &lcfg);
            }
            Self::Procedural {
                spec,
                elapsed,
                stars,
                glyphs,
                ..
            } => {
                let dt = dt_secs.max(0.0) * spec.speed.clamp(0.05, 10.0);
                *elapsed += dt;
                if spec.effect == BackgroundLayerKind::Starfield {
                    let dir_right = spec
                        .direction
                        .as_deref()
                        .is_some_and(|d| d.eq_ignore_ascii_case("right"));
                    for star in stars.iter_mut() {
                        if dir_right {
                            star.x = (star.x + dt * (0.12 + star.depth * 0.45)).rem_euclid(1.0);
                        } else {
                            let dx = star.x - 0.5;
                            let dy = star.y - 0.5;
                            let step = 1.0 + dt * (0.25 + star.depth * 0.65);
                            star.x = 0.5 + dx * step;
                            star.y = 0.5 + dy * step;
                            if star.x < 0.0 || star.x >= 1.0 || star.y < 0.0 || star.y >= 1.0 {
                                star.x = 0.5 + ((star.x * 13.7).sin()) * 0.14;
                                star.y = 0.5 + ((star.y * 19.3).cos()) * 0.14;
                            }
                        }
                    }
                } else if spec.effect == BackgroundLayerKind::FloatingChars {
                    for g in glyphs.iter_mut() {
                        g.y = (g.y - dt * 0.06 * g.speed_mul).rem_euclid(1.0);
                        g.phase += dt * 1.8 * g.speed_mul;
                    }
                }
            }
        }
    }

    pub fn render(
        &self,
        buf: &mut Buffer,
        area: Rect,
        cfg: &BackgroundConfig,
        theme: &Theme,
        truecolor: bool,
    ) {
        match self {
            Self::Legacy(eff) => eff.render(buf, area, cfg, theme),
            Self::Lua { spec, adapter, .. } => {
                if !spec.visible || spec.opacity <= 0.01 {
                    return;
                }
                let reg = resolve_region(area, spec.region.as_ref(), cfg.region.as_ref());
                if reg.width == 0 || reg.height == 0 {
                    return;
                }
                adapter.render(buf, reg, cfg, theme);
            }
            Self::Procedural {
                spec,
                elapsed,
                stars,
                glyphs,
                ascii_sprite,
            } => {
                if !spec.visible || spec.opacity <= 0.01 {
                    return;
                }
                let reg = resolve_region(area, spec.region.as_ref(), cfg.region.as_ref());
                if reg.width == 0 || reg.height == 0 {
                    return;
                }
                let primary = resolve_scene_color(spec.color.as_deref(), theme, theme.primary);
                let secondary =
                    resolve_scene_color(spec.secondary_color.as_deref(), theme, theme.secondary);
                let alpha = spec.opacity.clamp(0.0, 1.0);

                match spec.effect {
                    BackgroundLayerKind::Starfield => {
                        render_starfield(buf, reg, stars, primary, secondary, alpha, truecolor);
                    }
                    BackgroundLayerKind::Noise => {
                        render_noise(
                            buf,
                            reg,
                            *elapsed,
                            spec.intensity,
                            primary,
                            secondary,
                            theme.background,
                            alpha,
                            truecolor,
                        );
                    }
                    BackgroundLayerKind::Gradient => {
                        render_gradient(
                            buf,
                            reg,
                            *elapsed,
                            spec.intensity,
                            primary,
                            secondary,
                            theme.background,
                            alpha,
                            truecolor,
                        );
                    }
                    BackgroundLayerKind::FloatingChars => {
                        render_floating_chars(buf, reg, glyphs, primary, secondary, alpha, truecolor);
                    }
                    BackgroundLayerKind::CustomAscii => {
                        render_custom_ascii(
                            buf,
                            reg,
                            ascii_sprite,
                            *elapsed,
                            spec.direction.as_deref(),
                            primary,
                            alpha,
                            truecolor,
                        );
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Global Background controller owning a single shared `CanvasBuffer` and composing
/// ordered internal layers behind all UI containers.
pub struct BackgroundPipeline {
    pub canvas: crate::lua::CanvasBuffer,
    pub layers: Vec<(i32, BackgroundLayerRuntime)>,
    pub paused: bool,
    pub speed: f32,
    pub truecolor: bool,
}

impl BackgroundPipeline {
    pub fn from_config(cfg: &BackgroundConfig, width: u16, height: u16) -> Self {
        Self::from_config_and_lua(cfg, &[], width, height, None)
    }

    pub fn from_config_and_lua(
        cfg: &BackgroundConfig,
        lua_specs: &[crate::lua::LuaEffectSpec],
        width: u16,
        height: u16,
        theme_opt: Option<&Theme>,
    ) -> Self {
        let mut layers = Vec::new();
        if cfg.enabled {
            if !lua_specs.is_empty() {
                let default_theme =
                    Theme::from_config(&crate::config::ThemeConfig::default());
                let theme = theme_opt.unwrap_or(&default_theme);
                for (idx, lspec) in lua_specs.iter().enumerate() {
                    if !lspec.name.eq_ignore_ascii_case("none") {
                        layers.push((
                            idx as i32,
                            BackgroundLayerRuntime::from_lua_spec(lspec, width, height, theme),
                        ));
                    }
                }
            } else if !cfg.layers.is_empty() {
                let empty_assets = BTreeMap::new();
                for spec in &cfg.layers {
                    if spec.effect != BackgroundLayerKind::None
                        || spec.asset.as_deref().is_some_and(|a| !a.eq_ignore_ascii_case("none"))
                    {
                        layers.push((
                            spec.z_index,
                            BackgroundLayerRuntime::from_spec_with_theme(
                                spec.clone(),
                                width,
                                height,
                                cfg,
                                &empty_assets,
                                theme_opt,
                            ),
                        ));
                    }
                }
                layers.sort_by_key(|(z, _)| *z);
            } else if let Some(preset) = cfg.preset.as_deref().filter(|p| !p.is_empty() && !p.eq_ignore_ascii_case("none")) {
                let spec = BackgroundLayerSpec {
                    effect: BackgroundLayerKind::parse(preset)
                        .unwrap_or(BackgroundLayerKind::CustomAscii),
                    speed: cfg.speed,
                    density: cfg.density,
                    asset: Some(preset.to_string()),
                    ..BackgroundLayerSpec::default()
                };
                layers.push((
                    0,
                    BackgroundLayerRuntime::from_spec_with_theme(
                        spec,
                        width,
                        height,
                        cfg,
                        &BTreeMap::new(),
                        theme_opt,
                    ),
                ));
            } else if cfg.effect != EffectKind::None {
                let spec = BackgroundLayerSpec {
                    effect: BackgroundLayerKind::from_effect_kind(cfg.effect),
                    speed: cfg.speed,
                    density: cfg.density,
                    asset: Some(cfg.effect.label().to_ascii_lowercase()),
                    ..BackgroundLayerSpec::default()
                };
                layers.push((
                    0,
                    BackgroundLayerRuntime::from_spec_with_theme(
                        spec,
                        width,
                        height,
                        cfg,
                        &BTreeMap::new(),
                        theme_opt,
                    ),
                ));
            }
        }
        Self {
            canvas: crate::lua::CanvasBuffer::new_named(
                "background",
                "Global background canvas",
                width,
                height,
            ),
            layers,
            paused: false,
            speed: cfg.speed.clamp(0.1, 10.0),
            truecolor: terminal_supports_truecolor(),
        }
    }

    pub fn set_theme(&mut self, theme: &Theme) {
        for (_, layer) in &mut self.layers {
            if let BackgroundLayerRuntime::Lua { adapter, .. } = layer {
                adapter.set_theme(theme.clone());
            }
        }
    }

    pub fn set_single_with_theme(
        &mut self,
        spec: BackgroundLayerSpec,
        width: u16,
        height: u16,
        cfg: &BackgroundConfig,
        assets: &BTreeMap<String, String>,
        theme_opt: Option<&Theme>,
    ) {
        self.layers.clear();
        let has_asset = spec
            .asset
            .as_deref()
            .is_some_and(|a| !a.eq_ignore_ascii_case("none"));
        if spec.effect != BackgroundLayerKind::None || has_asset {
            let z = spec.z_index;
            self.layers.push((
                z,
                BackgroundLayerRuntime::from_spec_with_theme(
                    spec, width, height, cfg, assets, theme_opt,
                ),
            ));
        }
    }

    pub fn add_layer_with_theme(
        &mut self,
        spec: BackgroundLayerSpec,
        width: u16,
        height: u16,
        cfg: &BackgroundConfig,
        assets: &BTreeMap<String, String>,
        theme_opt: Option<&Theme>,
    ) {
        let has_asset = spec
            .asset
            .as_deref()
            .is_some_and(|a| !a.eq_ignore_ascii_case("none"));
        if spec.effect == BackgroundLayerKind::None && !has_asset {
            return;
        }
        let z = spec.z_index;
        self.layers.push((
            z,
            BackgroundLayerRuntime::from_spec_with_theme(
                spec, width, height, cfg, assets, theme_opt,
            ),
        ));
        self.layers.sort_by_key(|(layer_z, _)| *layer_z);
    }

    pub fn set_single(
        &mut self,
        spec: BackgroundLayerSpec,
        width: u16,
        height: u16,
        cfg: &BackgroundConfig,
        assets: &BTreeMap<String, String>,
    ) {
        self.set_single_with_theme(spec, width, height, cfg, assets, None);
    }

    pub fn add_layer(
        &mut self,
        spec: BackgroundLayerSpec,
        width: u16,
        height: u16,
        cfg: &BackgroundConfig,
        assets: &BTreeMap<String, String>,
    ) {
        self.add_layer_with_theme(spec, width, height, cfg, assets, None);
    }

    pub fn set_specs(
        &mut self,
        specs: &[BackgroundLayerSpec],
        width: u16,
        height: u16,
        cfg: &BackgroundConfig,
        assets: &BTreeMap<String, String>,
    ) {
        self.layers.clear();
        for spec in specs {
            if spec.effect != BackgroundLayerKind::None {
                let z = spec.z_index;
                self.layers.push((
                    z,
                    BackgroundLayerRuntime::from_spec(spec.clone(), width, height, cfg, assets),
                ));
            }
        }
        self.layers.sort_by_key(|(z, _)| *z);
    }

    pub fn clear(&mut self) {
        self.layers.clear();
        self.canvas.begin_frame();
    }

    pub fn remove_layer(&mut self, index_or_name: &str) -> bool {
        if let Ok(idx) = index_or_name.parse::<usize>()
            && idx > 0
            && idx <= self.layers.len()
        {
            self.layers.remove(idx - 1);
            return true;
        }
        let target = crate::lua::normalize_animation_alias(index_or_name);
        if let Some(pos) = self.layers.iter().position(|(_, layer)| match layer {
            BackgroundLayerRuntime::Procedural { spec, .. } => {
                spec.effect.label().eq_ignore_ascii_case(index_or_name.trim())
                    || crate::lua::normalize_animation_alias(spec.effect.label()) == target
            }
            BackgroundLayerRuntime::Lua { name, .. } => {
                name.eq_ignore_ascii_case(index_or_name.trim())
                    || crate::lua::normalize_animation_alias(name) == target
            }
            BackgroundLayerRuntime::Legacy(eff) => {
                eff.kind().label().eq_ignore_ascii_case(index_or_name.trim())
            }
        }) {
            self.layers.remove(pos);
            true
        } else {
            false
        }
    }

    pub fn pause(&mut self) {
        self.paused = true;
    }

    pub fn resume(&mut self) {
        self.paused = false;
    }

    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed.clamp(0.1, 10.0);
    }

    pub fn resize(&mut self, width: u16, height: u16, cfg: &BackgroundConfig) {
        self.canvas.resize(width, height);
        for (_, layer) in &mut self.layers {
            layer.resize(width, height, cfg);
        }
    }

    pub fn update(&mut self, dt_secs: f32, cfg: &BackgroundConfig) {
        if self.paused || !cfg.enabled {
            return;
        }
        let scaled = dt_secs.max(0.0) * self.speed;
        for (_, layer) in &mut self.layers {
            layer.update(scaled, cfg);
        }
    }

    pub fn render(&self, buf: &mut Buffer, area: Rect, cfg: &BackgroundConfig, theme: &Theme) {
        if !cfg.enabled || area.width == 0 || area.height == 0 {
            return;
        }
        // Compose all internal background layers sequentially into an isolated local surface
        let local = Rect::new(0, 0, area.width, area.height);
        let mut layer_buf = Buffer::empty(local);
        for (_, layer) in &self.layers {
            layer.render(&mut layer_buf, local, cfg, theme, self.truecolor);
        }
        for dy in 0..area.height {
            for dx in 0..area.width {
                let s = &layer_buf[(dx, dy)];
                if s.symbol() == " " && s.bg == Color::Reset {
                    continue;
                }
                if let Some(d) = buf.cell_mut((area.x + dx, area.y + dy)) {
                    let preserved_bg = if s.bg == Color::Reset { d.bg } else { s.bg };
                    *d = s.clone();
                    d.set_bg(preserved_bg);
                }
            }
        }
    }

    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    pub fn primary_effect_kind(&self) -> EffectKind {
        self.layers
            .first()
            .map(|(_, l)| match l {
                BackgroundLayerRuntime::Legacy(eff) => eff.kind(),
                BackgroundLayerRuntime::Lua { adapter, .. } => adapter.kind(),
                BackgroundLayerRuntime::Procedural { spec, .. } => match spec.effect {
                    BackgroundLayerKind::MatrixRain => EffectKind::Matrix,
                    BackgroundLayerKind::Waves => EffectKind::Water,
                    BackgroundLayerKind::Particles => EffectKind::Particles,
                    BackgroundLayerKind::Scanlines => EffectKind::Scanlines,
                    BackgroundLayerKind::None => EffectKind::None,
                    _ => EffectKind::Particles,
                },
            })
            .unwrap_or(EffectKind::None)
    }
}

impl BackgroundEffect for BackgroundPipeline {
    fn kind(&self) -> EffectKind {
        self.primary_effect_kind()
    }

    fn resize(&mut self, width: u16, height: u16, cfg: &BackgroundConfig) {
        BackgroundPipeline::resize(self, width, height, cfg);
    }

    fn update(&mut self, dt_secs: f32, cfg: &BackgroundConfig) {
        BackgroundPipeline::update(self, dt_secs, cfg);
    }

    fn render(&self, buf: &mut Buffer, area: Rect, cfg: &BackgroundConfig, theme: &Theme) {
        BackgroundPipeline::render(self, buf, area, cfg, theme);
    }
}

fn seed_stars(width: u16, height: u16, density: f32) -> Vec<StarDot> {
    let area = (width as usize).saturating_mul(height as usize).max(80);
    let count = ((area as f32 * 0.045 * density.clamp(0.1, 2.0)).round() as usize).clamp(16, 280);
    (0..count)
        .map(|i| {
            let a = ((i * 7919 + 31) % 997) as f32 / 997.0;
            let b = ((i * 6271 + 73) % 991) as f32 / 991.0;
            let d = 0.2 + (((i * 3571 + 19) % 100) as f32 / 100.0) * 0.8;
            StarDot { x: a, y: b, depth: d }
        })
        .collect()
}

fn seed_floating_glyphs(
    width: u16,
    height: u16,
    spec: &BackgroundLayerSpec,
) -> Vec<FloatingGlyph> {
    let pool: Vec<char> = spec
        .chars
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or("·∘✦*◇+~")
        .chars()
        .collect();
    let area = (width as usize).saturating_mul(height as usize).max(80);
    let count =
        ((area as f32 * 0.03 * spec.density.clamp(0.1, 2.0)).round() as usize).clamp(12, 220);
    (0..count)
        .map(|i| FloatingGlyph {
            x: ((i * 4999 + 17) % 1000) as f32 / 1000.0,
            y: ((i * 8111 + 41) % 1000) as f32 / 1000.0,
            phase: (i as f32) * 0.73,
            speed_mul: 0.6 + ((i % 5) as f32) * 0.25,
            ch: pool[i % pool.len()],
        })
        .collect()
}

fn render_starfield(
    buf: &mut Buffer,
    reg: Rect,
    stars: &[StarDot],
    primary: Color,
    secondary: Color,
    alpha: f32,
    truecolor: bool,
) {
    for star in stars {
        let sx = reg.x + ((star.x * reg.width as f32) as u16).min(reg.width.saturating_sub(1));
        let sy = reg.y + ((star.y * reg.height as f32) as u16).min(reg.height.saturating_sub(1));
        let (ch, color) = if star.depth > 0.82 {
            ('✦', secondary)
        } else if star.depth > 0.58 {
            ('*', blend_color(secondary, primary, 0.5))
        } else if star.depth > 0.35 {
            ('+', primary)
        } else {
            ('·', blend_color(primary, Color::Black, 0.5))
        };
        if let Some(cell) = buf.cell_mut((sx, sy)) {
            let fg = if alpha >= 0.99 {
                color
            } else {
                blend_color(color, cell.bg, alpha)
            };
            cell.set_char(ch).set_fg(adapt_color(fg, truecolor));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_noise(
    buf: &mut Buffer,
    reg: Rect,
    elapsed: f32,
    intensity: f32,
    primary: Color,
    secondary: Color,
    bg_fallback: Color,
    alpha: f32,
    truecolor: bool,
) {
    let glyphs = [' ', '·', '░', '▒', '▓'];
    let strength = intensity.clamp(0.1, 1.5);
    for y in reg.top()..reg.bottom() {
        for x in reg.left()..reg.right() {
            let nx = x as f32 * 0.18;
            let ny = y as f32 * 0.32;
            let v = ((nx + elapsed * 1.1).sin() * (ny - elapsed * 0.8).cos()
                + ((nx * 0.7 - ny * 0.9) + elapsed * 1.6).sin())
                * 0.25
                + 0.5;
            let val = (v * strength).clamp(0.0, 0.999);
            let idx = (val * glyphs.len() as f32) as usize;
            let ch = glyphs[idx.min(glyphs.len() - 1)];
            if ch == ' ' {
                continue;
            }
            let c = blend_color(secondary, primary, val);
            let c = blend_color(c, bg_fallback, (0.35 + val * 0.65) * alpha);
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_char(ch).set_fg(adapt_color(c, truecolor));
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_gradient(
    buf: &mut Buffer,
    reg: Rect,
    elapsed: f32,
    intensity: f32,
    primary: Color,
    secondary: Color,
    bg_fallback: Color,
    alpha: f32,
    truecolor: bool,
) {
    let w = reg.width.max(1) as f32;
    let h = reg.height.max(1) as f32;
    let strength = (intensity * 0.45 * alpha).clamp(0.05, 0.85);
    for y in reg.top()..reg.bottom() {
        let ny = (y - reg.y) as f32 / h;
        for x in reg.left()..reg.right() {
            let nx = (x - reg.x) as f32 / w;
            let phase = (nx * 0.7 + ny * 0.5 + (elapsed * 0.45).sin() * 0.35).rem_euclid(1.0);
            let wave = 0.5 + 0.5 * (phase * std::f32::consts::TAU).sin();
            let tint = blend_color(secondary, primary, wave);
            if let Some(cell) = buf.cell_mut((x, y)) {
                let bg = blend_color(tint, bg_fallback, strength);
                cell.set_bg(adapt_color(bg, truecolor));
            }
        }
    }
}

fn render_floating_chars(
    buf: &mut Buffer,
    reg: Rect,
    glyphs: &[FloatingGlyph],
    primary: Color,
    secondary: Color,
    alpha: f32,
    truecolor: bool,
) {
    for (idx, g) in glyphs.iter().enumerate() {
        let sway = g.phase.sin() * 1.5;
        let x = reg.x as f32 + g.x * reg.width.saturating_sub(1) as f32 + sway;
        let y = reg.y as f32 + g.y * reg.height.saturating_sub(1) as f32;
        let xi = x.round() as i32;
        let yi = y.round() as i32;
        if xi < reg.x as i32 || xi >= reg.right() as i32 || yi < reg.y as i32 || yi >= reg.bottom() as i32 {
            continue;
        }
        let color = if idx % 3 == 0 { secondary } else { primary };
        if let Some(cell) = buf.cell_mut((xi as u16, yi as u16)) {
            let fg = if alpha >= 0.99 {
                color
            } else {
                blend_color(color, cell.bg, alpha)
            };
            cell.set_char(g.ch).set_fg(adapt_color(fg, truecolor));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_custom_ascii(
    buf: &mut Buffer,
    reg: Rect,
    sprite: &str,
    elapsed: f32,
    direction: Option<&str>,
    color: Color,
    alpha: f32,
    truecolor: bool,
) {
    if sprite.trim().is_empty() {
        return;
    }
    let drift_x = match direction.map(|d| d.to_ascii_lowercase()).as_deref() {
        Some("scroll") | Some("left") => -((elapsed * 6.0) % reg.width.max(1) as f32),
        Some("right") => (elapsed * 6.0) % reg.width.max(1) as f32,
        _ => (elapsed * 1.4).sin() * 3.0,
    };
    let cx = reg.x as f32 + reg.width as f32 * 0.5 + drift_x;
    let cy = reg.y as f32 + reg.height as f32 * 0.5;
    let cells = collect_ascii_sprite_cells(sprite, cx, cy, 1.0, color, alpha, reg);
    let item = SceneDrawItem::StyledRun {
        layer_z: 0,
        track_order: 0,
        cells,
    };
    composite_scene_items(vec![item], reg, buf, truecolor);
}
