use crate::animation::AnimationCatalog;
use crate::config::animation_loader::{
    CURRENT_ANIMATION_SCHEMA_VERSION, MAX_ANIMATION_DURATION_MS, MAX_BACKGROUNDS,
    MAX_KEYFRAMES_PER_TRACK, MAX_LAYERS, MAX_PARTICLES_PER_TRACK, MAX_PATH_POINTS, MAX_TRACKS,
};
use crate::config::schema::is_valid_color_spec;
use crate::config::{
    AnimationDefinition, AnimationPackage, BorderStyle, Config, LayoutConfig, PackageOrigin,
    ThemeConfig, ThemeEntry, ViewMode, WidgetConfig, load_packages_from_dir,
    validate_safe_asset_path,
};
use crate::runtime::diagnostics::{
    Diagnostic, line_col_from_offset, locate_field_in_toml, locate_value_in_toml,
};
use crate::runtime::registry::{ResourceId, ResourceKind, ResourceRegistry};
use crate::ui::borders::BorderChars;
use crate::ui::fonts::{FigletFont, available_fonts};
use crate::ui::theme::{Theme, builtin_theme_names, preset_theme};
use crate::window::WindowConfig;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// Known context of sibling resources used during Semantic Validation.
pub struct ValidationContext<'a> {
    pub themes: &'a BTreeMap<String, ThemeEntry>,
    pub animations: &'a AnimationCatalog,
    pub fonts: &'a BTreeSet<String>,
    pub registry: &'a ResourceRegistry,
}

#[derive(Debug, Clone)]
pub struct LayoutPreset {
    pub name: String,
    pub view_mode: Option<ViewMode>,
    pub layout: LayoutConfig,
    pub widgets: Vec<WidgetConfig>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RawLayoutFile {
    name: Option<String>,
    view_mode: Option<ViewMode>,
    layout: LayoutConfig,
    widgets: Vec<WidgetConfig>,
}

/// Typed payload emitted cleanly by Stage 1 (`parse_resource`).
#[derive(Debug, Clone)]
pub enum ParsedPayload {
    ApplicationConfig(Box<Config>),
    WindowConfig(Box<WindowConfig>),
    Theme(Box<ThemeEntry>),
    Animation(Box<AnimationPackage>),
    Layout(Box<LayoutPreset>),
    Font { name: String, font: FigletFont },
}

#[derive(Debug, Clone)]
pub struct ParsedResource {
    pub id: ResourceId,
    pub path: std::path::PathBuf,
    pub raw_text: String,
    pub toml_value: Option<toml::Value>,
    pub payload: ParsedPayload,
    pub dependencies: Vec<ResourceId>,
}

/// Stage 1: Reads/parses raw file text into a typed `ParsedResource`, emitting structured
/// syntax or deserialization diagnostics on failure.
pub fn parse_resource(
    kind: ResourceKind,
    key_hint: &str,
    path: &Path,
    raw_text: String,
) -> Result<ParsedResource, Vec<Diagnostic>> {
    if kind == ResourceKind::Font {
        return parse_font_resource(key_hint, path, raw_text);
    }

    let toml_value = match toml::from_str::<toml::Value>(&raw_text) {
        Ok(v) => v,
        Err(err) => {
            let (line, col) = err
                .span()
                .map(|s| line_col_from_offset(&raw_text, s.start))
                .map(|(l, c)| (Some(l), Some(c)))
                .unwrap_or((None, None));
            return Err(vec![
                Diagnostic::error("E1001", format!("TOML syntax error: {}", err.message()))
                    .with_location(Some(path), line, col)
                    .with_suggestion("Fix the TOML syntax around the reported line and column"),
            ]);
        }
    };

    match kind {
        ResourceKind::ApplicationConfig => {
            let cfg: Config = toml::from_str(&raw_text)
                .map_err(|e| vec![toml_deserialize_diagnostic(path, &raw_text, &e)])?;
            let mut deps = Vec::new();
            let theme_key = cfg.theme.name.trim().to_ascii_lowercase();
            if !theme_key.is_empty() && theme_key != "custom" {
                deps.push(ResourceId::new(ResourceKind::Theme, theme_key));
            }
            if !cfg.general.startup_animation.trim().is_empty() {
                deps.push(ResourceId::new(
                    ResourceKind::Animation,
                    cfg.general.startup_animation.clone(),
                ));
            }
            if !cfg.typography.font.trim().is_empty() {
                deps.push(ResourceId::new(
                    ResourceKind::Font,
                    cfg.typography.font.clone(),
                ));
            }
            Ok(ParsedResource {
                id: ResourceId::app_config(),
                path: path.to_path_buf(),
                raw_text,
                toml_value: Some(toml_value),
                payload: ParsedPayload::ApplicationConfig(Box::new(cfg)),
                dependencies: deps,
            })
        }
        ResourceKind::WindowConfig => {
            let win: WindowConfig = toml::from_str(&raw_text)
                .map_err(|e| vec![toml_deserialize_diagnostic(path, &raw_text, &e)])?;
            Ok(ParsedResource {
                id: ResourceId::window_config(),
                path: path.to_path_buf(),
                raw_text,
                toml_value: Some(toml_value),
                payload: ParsedPayload::WindowConfig(Box::new(win)),
                dependencies: Vec::new(),
            })
        }
        ResourceKind::Theme => {
            let cfg = if let Some(wrapped) = toml_value.get("theme") {
                wrapped
                    .clone()
                    .try_into::<ThemeConfig>()
                    .map_err(|e| vec![toml_deserialize_diagnostic(path, &raw_text, &e)])?
            } else {
                toml::from_str::<ThemeConfig>(&raw_text)
                    .map_err(|e| vec![toml_deserialize_diagnostic(path, &raw_text, &e)])?
            };
            let name = if cfg.name.trim().is_empty() {
                key_hint.to_string()
            } else {
                cfg.name.trim().to_string()
            };
            let mut theme = Theme::from_config(&cfg);
            theme.name = name.clone();
            let id = ResourceId::new(ResourceKind::Theme, &name);
            Ok(ParsedResource {
                id,
                path: path.to_path_buf(),
                raw_text,
                toml_value: Some(toml_value),
                payload: ParsedPayload::Theme(Box::new(ThemeEntry {
                    name,
                    theme,
                    source_path: Some(path.to_path_buf()),
                })),
                dependencies: Vec::new(),
            })
        }
        ResourceKind::Animation => parse_animation_resource(key_hint, path, raw_text, toml_value),
        ResourceKind::Layout => {
            let raw_layout: RawLayoutFile = toml::from_str(&raw_text)
                .map_err(|e| vec![toml_deserialize_diagnostic(path, &raw_text, &e)])?;
            let name = raw_layout
                .name
                .filter(|n| !n.trim().is_empty())
                .unwrap_or_else(|| key_hint.to_string());
            let mut deps = Vec::new();
            for w in &raw_layout.widgets {
                if let Some(font) = &w.font {
                    deps.push(ResourceId::new(ResourceKind::Font, font));
                }
            }
            let id = ResourceId::new(ResourceKind::Layout, &name);
            Ok(ParsedResource {
                id,
                path: path.to_path_buf(),
                raw_text,
                toml_value: Some(toml_value),
                payload: ParsedPayload::Layout(Box::new(LayoutPreset {
                    name,
                    view_mode: raw_layout.view_mode,
                    layout: raw_layout.layout,
                    widgets: raw_layout.widgets,
                })),
                dependencies: deps,
            })
        }
        ResourceKind::Background => Err(vec![
            Diagnostic::error(
                "E1000",
                "Background resources are validated via ApplicationConfig or Animation packages",
            )
            .with_location(Some(path), None, None),
        ]),
        ResourceKind::Font => unreachable!(),
    }
}

fn parse_font_resource(
    key_hint: &str,
    path: &Path,
    raw_text: String,
) -> Result<ParsedResource, Vec<Diagnostic>> {
    let Some(font) = FigletFont::from_flf_str(key_hint, &raw_text) else {
        return Err(vec![
            Diagnostic::error("E1001", "Invalid FIGlet .flf font header or glyph table")
                .with_location(Some(path), Some(1), Some(1))
                .with_expected_actual("flf2a<hardblank> <height> ...", "Malformed FLF header"),
        ]);
    };
    let id = ResourceId::new(ResourceKind::Font, key_hint);
    Ok(ParsedResource {
        id,
        path: path.to_path_buf(),
        raw_text,
        toml_value: None,
        payload: ParsedPayload::Font {
            name: key_hint.to_string(),
            font,
        },
        dependencies: Vec::new(),
    })
}

fn parse_animation_resource(
    key_hint: &str,
    path: &Path,
    raw_text: String,
    toml_value: toml::Value,
) -> Result<ParsedResource, Vec<Diagnostic>> {
    // Parse header + tracks first without choking before Schema Validator gets a chance
    let is_package_manifest = path
        .file_name()
        .and_then(|s| s.to_str())
        .is_some_and(|n| n == "animation.toml");

    if is_package_manifest
        && let Some(pkg_dir) = path.parent()
        && let Some(root_parent) = pkg_dir.parent()
    {
        let map = load_packages_from_dir(root_parent);
        if let Some(pkg) = map
            .into_values()
            .find(|p| matches!(&p.origin, PackageOrigin::Community { manifest_path, .. } if manifest_path == path))
        {
            let id = ResourceId::new(ResourceKind::Animation, &pkg.definition.name);
            return Ok(ParsedResource {
                id,
                path: path.to_path_buf(),
                raw_text,
                toml_value: Some(toml_value),
                payload: ParsedPayload::Animation(Box::new(pkg)),
                dependencies: Vec::new(),
            });
        }
    }

    let mut def = AnimationDefinition::parse_toml(&raw_text).map_err(|msg| {
        vec![
            Diagnostic::error("E1005", msg)
                .with_location(Some(path), Some(1), Some(1))
                .with_suggestion("Check [animation], [[tracks]], [[backgrounds]], and schema version"),
        ]
    })?;
    if def.name == "custom" {
        def.name = key_hint.to_string();
    }
    let id = ResourceId::new(ResourceKind::Animation, &def.name);
    let pkg = AnimationPackage {
        definition: def,
        origin: PackageOrigin::Community {
            root_dir: is_package_manifest
                .then(|| path.parent().map(Path::to_path_buf))
                .flatten(),
            manifest_path: path.to_path_buf(),
        },
        assets: BTreeMap::new(),
        has_preview: is_package_manifest
            && path
                .parent()
                .is_some_and(|p| p.join("preview.png").is_file()),
    };
    Ok(ParsedResource {
        id,
        path: path.to_path_buf(),
        raw_text,
        toml_value: Some(toml_value),
        payload: ParsedPayload::Animation(Box::new(pkg)),
        dependencies: Vec::new(),
    })
}

/// Stage 2: Validates structure, required/optional fields, unknown keys, types, and schema limits.
pub fn validate_schema(parsed: &ParsedResource) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let path = &parsed.path;
    let raw = &parsed.raw_text;

    if let Some(val) = &parsed.toml_value {
        check_unknown_fields(parsed.id.kind, val, path, raw, &mut diags);
    }

    match &parsed.payload {
        ParsedPayload::ApplicationConfig(cfg) => {
            if cfg.general.animation_fps == 0 || cfg.general.animation_fps > 120 {
                let (l, c) = locate_value_in_toml(raw, "animation_fps");
                diags.push(
                    Diagnostic::error("E1005", "animation_fps out of allowed range")
                        .with_location(Some(path), l, c)
                        .with_field("general.animation_fps")
                        .with_expected_actual("1..=120", cfg.general.animation_fps.to_string()),
                );
            }
            if cfg.general.refresh_rate_ms < 100 {
                let (l, c) = locate_value_in_toml(raw, "refresh_rate_ms");
                diags.push(
                    Diagnostic::error("E1005", "refresh_rate_ms must be at least 100 ms")
                        .with_location(Some(path), l, c)
                        .with_field("general.refresh_rate_ms")
                        .with_expected_actual(">= 100", cfg.general.refresh_rate_ms.to_string()),
                );
            }
            if !(0.0..=1.0).contains(&cfg.background.density) {
                let (l, c) = locate_value_in_toml(raw, "density");
                diags.push(
                    Diagnostic::error("E1005", "background.density out of range")
                        .with_location(Some(path), l, c)
                        .with_field("background.density")
                        .with_expected_actual("0.0..=1.0", cfg.background.density.to_string()),
                );
            }
            if !(0.0..=1.0).contains(&cfg.background.opacity) {
                let (l, c) = locate_value_in_toml(raw, "opacity");
                diags.push(
                    Diagnostic::error("E1005", "background.opacity out of range")
                        .with_location(Some(path), l, c)
                        .with_field("background.opacity")
                        .with_expected_actual("0.0..=1.0", cfg.background.opacity.to_string()),
                );
            }
            if !(0.0..=10.0).contains(&cfg.background.speed) {
                let (l, c) = locate_value_in_toml(raw, "speed");
                diags.push(
                    Diagnostic::error("E1005", "background.speed out of range")
                        .with_location(Some(path), l, c)
                        .with_field("background.speed")
                        .with_expected_actual("0.0..=10.0", cfg.background.speed.to_string()),
                );
            }
            if cfg.background.matrix.min_length == 0
                || cfg.background.matrix.min_length > cfg.background.matrix.max_length
            {
                let (l, c) = locate_value_in_toml(raw, "min_length");
                diags.push(
                    Diagnostic::error(
                        "E1005",
                        "background.matrix.min_length must be >= 1 and <= max_length",
                    )
                    .with_location(Some(path), l, c)
                    .with_field("background.matrix.min_length")
                    .with_expected_actual(
                        format!("1..={}", cfg.background.matrix.max_length),
                        cfg.background.matrix.min_length.to_string(),
                    ),
                );
            }
            if cfg.background.scanlines.spacing == 0 {
                let (l, c) = locate_value_in_toml(raw, "spacing");
                diags.push(
                    Diagnostic::error("E1005", "background.scanlines.spacing must be >= 1")
                        .with_location(Some(path), l, c)
                        .with_field("background.scanlines.spacing")
                        .with_expected_actual(">= 1", "0"),
                );
            }
            if cfg.startup.cursor.blink_interval < 50 {
                let (l, c) = locate_value_in_toml(raw, "blink_interval");
                diags.push(
                    Diagnostic::error("E1005", "startup.cursor.blink_interval must be >= 50 ms")
                        .with_location(Some(path), l, c)
                        .with_field("startup.cursor.blink_interval")
                        .with_expected_actual(
                            ">= 50",
                            cfg.startup.cursor.blink_interval.to_string(),
                        ),
                );
            }
        }
        ParsedPayload::WindowConfig(win) => {
            if win.padding > 10 {
                let (l, c) = locate_value_in_toml(raw, "padding");
                diags.push(
                    Diagnostic::error("E1005", "window padding exceeds maximum (10)")
                        .with_location(Some(path), l, c)
                        .with_field("padding")
                        .with_expected_actual("0..=10", win.padding.to_string()),
                );
            }
        }
        ParsedPayload::Theme(entry) => {
            if entry.name.trim().is_empty() {
                diags.push(
                    Diagnostic::error("E1005", "Theme name cannot be empty")
                        .with_location(Some(path), Some(1), Some(1))
                        .with_field("name"),
                );
            }
        }
        ParsedPayload::Animation(pkg) => {
            let def = &pkg.definition;
            if def.schema_version == 0 || def.schema_version > CURRENT_ANIMATION_SCHEMA_VERSION {
                let (l, c) = locate_value_in_toml(raw, "version");
                diags.push(
                    Diagnostic::error(
                        "E1005",
                        format!("Unsupported animation schema version {}", def.schema_version),
                    )
                    .with_location(Some(path), l, c)
                    .with_field("animation.version")
                    .with_expected_actual(
                        format!("1..={CURRENT_ANIMATION_SCHEMA_VERSION}"),
                        def.schema_version.to_string(),
                    ),
                );
            }
            if def.duration_ms > MAX_ANIMATION_DURATION_MS {
                diags.push(
                    Diagnostic::error("E1005", "Animation duration exceeds safety limit")
                        .with_location(Some(path), None, None)
                        .with_field("animation.duration_ms")
                        .with_expected_actual(
                            format!("<= {MAX_ANIMATION_DURATION_MS}"),
                            def.duration_ms.to_string(),
                        ),
                );
            }
            if def.layers.len() > MAX_LAYERS
                || def.tracks.len() > MAX_TRACKS
                || def.backgrounds.len() > MAX_BACKGROUNDS
            {
                diags.push(
                    Diagnostic::error("E1005", "Animation exceeds layer/track/background limit")
                        .with_location(Some(path), None, None),
                );
            }
            for tr in &def.tracks {
                if tr.keyframes.len() > MAX_KEYFRAMES_PER_TRACK
                    || tr.count > MAX_PARTICLES_PER_TRACK
                    || tr.path.len() > MAX_PATH_POINTS
                {
                    diags.push(
                        Diagnostic::error(
                            "E1005",
                            format!("Track '{}' exceeds resource limits", tr.id),
                        )
                        .with_location(Some(path), None, None)
                        .with_field(format!("tracks.{}.keyframes", tr.id)),
                    );
                }
            }
        }
        ParsedPayload::Layout(preset) => {
            if preset.layout.min_width_for_split == 0 {
                let (l, c) = locate_value_in_toml(raw, "min_width_for_split");
                diags.push(
                    Diagnostic::error("E1005", "layout.min_width_for_split must be >= 1")
                        .with_location(Some(path), l, c)
                        .with_field("layout.min_width_for_split")
                        .with_expected_actual(">= 1", "0"),
                );
            }
        }
        ParsedPayload::Font { .. } => {}
    }

    diags
}

/// Stage 3: Validates semantic cross-field and cross-resource constraints (colors, border
/// glyphs, referenced themes/animations/fonts, keyframe bounds, dependency cycles).
pub fn validate_semantics(
    parsed: &ParsedResource,
    ctx: &ValidationContext<'_>,
) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let path = &parsed.path;
    let raw = &parsed.raw_text;

    if ctx.registry.detect_cycle(&parsed.id, &parsed.dependencies) {
        diags.push(
            Diagnostic::error("E1009", "Circular resource dependency detected")
                .with_location(Some(path), Some(1), Some(1))
                .with_suggestion("Remove the cyclic reference between configuration files"),
        );
    }

    match &parsed.payload {
        ParsedPayload::ApplicationConfig(cfg) => {
            for (field, color) in [
                ("theme.background", &cfg.theme.background),
                ("theme.panel_background", &cfg.theme.panel_background),
                ("theme.primary", &cfg.theme.primary),
                ("theme.secondary", &cfg.theme.secondary),
                ("theme.accent", &cfg.theme.accent),
                ("theme.text", &cfg.theme.text),
                ("theme.muted", &cfg.theme.muted),
                ("theme.border", &cfg.theme.border),
                ("theme.danger", &cfg.theme.danger),
                ("startup.cursor.color", &cfg.startup.cursor.color),
            ] {
                if !is_valid_color_spec(color) {
                    let (l, c) = locate_value_in_toml(raw, field);
                    diags.push(
                        Diagnostic::error("E1004", format!("Invalid color value: \"{color}\""))
                            .with_location(Some(path), l, c)
                            .with_field(field)
                            .with_expected_actual("#RRGGBB, named color, or 'reset'", color.clone())
                            .with_suggestion(
                                "Use 6-digit hex like #39FF88, a standard color name, or 'reset'",
                            ),
                    );
                }
            }

            if let Err((field, message)) = cfg.theme.borders.validate() {
                let (l, c) = locate_value_in_toml(raw, &field);
                diags.push(
                    Diagnostic::error("E1004", message)
                        .with_location(Some(path), l, c)
                        .with_field(field)
                        .with_suggestion("Provide a single display-width-1 Unicode character"),
                );
            }

            let tname = cfg.theme.name.trim().to_ascii_lowercase();
            let theme_exists = tname.is_empty()
                || tname == "custom"
                || builtin_theme_names().iter().any(|b| b.eq_ignore_ascii_case(&tname))
                || ctx.themes.contains_key(&tname);
            if !theme_exists {
                let (l, c) = locate_value_in_toml(raw, "name");
                diags.push(
                    Diagnostic::error(
                        "E1006",
                        format!("Referenced theme '{}' does not exist", cfg.theme.name),
                    )
                    .with_location(Some(path), l, c)
                    .with_field("theme.name")
                    .with_expected_actual("built-in or ~/.config/egotop/themes/*.toml", cfg.theme.name.clone())
                    .with_suggestion("Run :colorscheme to list installed themes or create it in ~/.config/egotop/themes/"),
                );
            }

            let anim_name = cfg.general.startup_animation.trim().to_ascii_lowercase();
            if !anim_name.is_empty()
                && !ctx
                    .animations
                    .entries
                    .iter()
                    .any(|e| e.id.eq_ignore_ascii_case(&anim_name))
            {
                let (l, c) = locate_value_in_toml(raw, "startup_animation");
                diags.push(
                    Diagnostic::error(
                        "E1006",
                        format!(
                            "Referenced startup animation '{}' does not exist",
                            cfg.general.startup_animation
                        ),
                    )
                    .with_location(Some(path), l, c)
                    .with_field("general.startup_animation")
                    .with_expected_actual("installed animation id", cfg.general.startup_animation.clone()),
                );
            }

            validate_font_ref(
                &cfg.typography.font,
                "typography.font",
                path,
                raw,
                ctx.fonts,
                &mut diags,
            );
            validate_font_ref(
                &cfg.typography.logo_font,
                "typography.logo_font",
                path,
                raw,
                ctx.fonts,
                &mut diags,
            );
        }
        ParsedPayload::Theme(entry) => {
            if let Some(val) = &parsed.toml_value {
                let tbl = val.get("theme").and_then(|v| v.as_table()).or(val.as_table());
                if let Some(t) = tbl {
                    for field in [
                        "background",
                        "panel_background",
                        "primary",
                        "secondary",
                        "accent",
                        "text",
                        "muted",
                        "border",
                        "danger",
                    ] {
                        if let Some(color) = t.get(field).and_then(|v| v.as_str())
                            && !is_valid_color_spec(color)
                        {
                            let (l, c) = locate_value_in_toml(raw, field);
                            diags.push(
                                Diagnostic::error(
                                    "E1004",
                                    format!("Invalid color value: \"{color}\""),
                                )
                                .with_location(Some(path), l, c)
                                .with_field(format!("theme.{field}"))
                                .with_expected_actual("#RRGGBB", color.to_string())
                                .with_suggestion("Use #RRGGBB hex format such as #39FF88"),
                            );
                        }
                    }
                    if let Some(borders_tbl) = t.get("borders").and_then(|v| v.as_table()) {
                        let mut bc = BorderChars::from_style(BorderStyle::Rounded);
                        for (bk, bv) in borders_tbl {
                            if let Some(s) = bv.as_str()
                                && let Err(msg) = bc.set_property(bk, s)
                            {
                                let (l, c) = locate_value_in_toml(raw, bk);
                                diags.push(
                                    Diagnostic::error("E1004", msg)
                                        .with_location(Some(path), l, c)
                                        .with_field(format!("theme.borders.{bk}")),
                                );
                            }
                        }
                    }
                }
            }
            let _ = entry;
        }
        ParsedPayload::Animation(pkg) => {
            let def = &pkg.definition;
            if def.tracks.is_empty() && def.backgrounds.is_empty() {
                diags.push(
                    Diagnostic::error(
                        "E1007",
                        "Animation contains no playable tracks or background layers",
                    )
                    .with_location(Some(path), Some(1), Some(1)),
                );
            }
            for tr in &def.tracks {
                if let Some(c) = &tr.color
                    && !is_valid_scene_color_or_token(c)
                {
                    let (l, col) = locate_value_in_toml(raw, "color");
                    diags.push(
                        Diagnostic::error("E1004", format!("Invalid color value: \"{c}\""))
                            .with_location(Some(path), l, col)
                            .with_field(format!("tracks.{}.color", tr.id))
                            .with_expected_actual("#RRGGBB, named color, or theme token", c.clone()),
                    );
                }
                if let Some(asset) = &tr.asset {
                    if let Err(err) = validate_safe_asset_path(asset) {
                        diags.push(
                            Diagnostic::error("E1008", err)
                                .with_location(Some(path), None, None)
                                .with_field(format!("tracks.{}.asset", tr.id)),
                        );
                    } else if matches!(pkg.origin, PackageOrigin::Community { .. })
                        && !pkg.assets.contains_key(asset)
                    {
                        diags.push(
                            Diagnostic::error(
                                "E1008",
                                format!("Missing package asset '{asset}'"),
                            )
                            .with_location(Some(path), None, None)
                            .with_field(format!("tracks.{}.asset", tr.id)),
                        );
                    }
                }
                for kf in &tr.keyframes {
                    if def.duration_ms > 0 && kf.at_ms > def.duration_ms {
                        diags.push(
                            Diagnostic::error(
                                "E1007",
                                format!(
                                    "Keyframe at {}ms exceeds animation duration_ms ({}ms)",
                                    kf.at_ms, def.duration_ms
                                ),
                            )
                            .with_location(Some(path), None, None)
                            .with_field(format!("tracks.{}.keyframes.at_ms", tr.id))
                            .with_expected_actual(
                                format!("0..={}", def.duration_ms),
                                kf.at_ms.to_string(),
                            ),
                        );
                    }
                    if let Some(c) = &kf.color
                        && !is_valid_scene_color_or_token(c)
                    {
                        diags.push(
                            Diagnostic::error("E1004", format!("Invalid color value: \"{c}\""))
                                .with_location(Some(path), None, None)
                                .with_field(format!("tracks.{}.keyframes.color", tr.id))
                                .with_expected_actual("#RRGGBB or theme token", c.clone()),
                        );
                    }
                }
            }
        }
        ParsedPayload::Layout(preset) => {
            for w in &preset.widgets {
                if let Some(font) = &w.font {
                    validate_font_ref(
                        font,
                        "widgets.font",
                        path,
                        raw,
                        ctx.fonts,
                        &mut diags,
                    );
                }
                if let Some(c) = &w.color
                    && !is_valid_color_spec(c)
                {
                    diags.push(
                        Diagnostic::error("E1004", format!("Invalid widget color '{c}'"))
                            .with_location(Some(path), None, None)
                            .with_field("widgets.color"),
                    );
                }
            }
        }
        ParsedPayload::WindowConfig(win) => {
            if let Some(c) = &win.border.color
                && !is_valid_color_spec(c)
            {
                let (l, col) = locate_value_in_toml(raw, "color");
                diags.push(
                    Diagnostic::error("E1004", format!("Invalid window border color: \"{c}\""))
                        .with_location(Some(path), l, col)
                        .with_field("border.color"),
                );
            }
        }
        ParsedPayload::Font { .. } => {}
    }

    diags
}

fn validate_font_ref(
    font_name: &str,
    field: &str,
    path: &Path,
    raw: &str,
    custom_fonts: &BTreeSet<String>,
    diags: &mut Vec<Diagnostic>,
) {
    let f = font_name.trim();
    if f.is_empty() {
        return;
    }
    let known_builtin = available_fonts()
        .iter()
        .any(|b| b.eq_ignore_ascii_case(f));
    let known_custom = custom_fonts.contains(&f.to_ascii_lowercase());
    if !known_builtin && !known_custom {
        let (l, c) = locate_value_in_toml(raw, field);
        diags.push(
            Diagnostic::error("E1006", format!("Referenced font '{f}' was not found"))
                .with_location(Some(path), l, c)
                .with_field(field)
                .with_expected_actual("built-in font or ~/.config/egotop/fonts/*.flf", f.to_string())
                .with_suggestion("Use ANSI Shadow, Standard, Big, Slant, Small, Block, Doom, or install a .flf file"),
        );
    }
}

fn is_valid_scene_color_or_token(spec: &str) -> bool {
    let low = spec.trim().to_ascii_lowercase();
    matches!(
        low.as_str(),
        "primary" | "secondary" | "accent" | "text" | "muted" | "background" | "bg" | "panel" | "border" | "danger"
    ) || is_valid_color_spec(spec)
}

fn toml_deserialize_diagnostic(path: &Path, raw: &str, err: &toml::de::Error) -> Diagnostic {
    let (line, col) = err
        .span()
        .map(|s| line_col_from_offset(raw, s.start))
        .map(|(l, c)| (Some(l), Some(c)))
        .unwrap_or((None, None));
    let msg = err.message();
    let code = if msg.contains("invalid type") || msg.contains("unknown variant") {
        "E1003"
    } else {
        "E1001"
    };
    Diagnostic::error(code, format!("Configuration parse error: {msg}"))
        .with_location(Some(path), line, col)
        .with_suggestion("Verify the field type and value match the schema")
}

fn check_unknown_fields(
    kind: ResourceKind,
    val: &toml::Value,
    path: &Path,
    raw: &str,
    diags: &mut Vec<Diagnostic>,
) {
    let Some(root) = val.as_table() else {
        return;
    };

    match kind {
        ResourceKind::ApplicationConfig => {
            check_allowed_keys(
                root,
                &[
                    "general",
                    "startup",
                    "theme",
                    "typography",
                    "header",
                    "background",
                    "layout",
                    "widgets",
                    "windows",
                ],
                "",
                path,
                raw,
                diags,
            );
            check_subtable(
                root,
                "general",
                &[
                    "name",
                    "greeting",
                    "user_override",
                    "message",
                    "refresh_rate_ms",
                    "refresh_rate",
                    "animation_fps",
                    "time_format",
                    "date_format",
                    "hide_unavailable",
                    "view_mode",
                    "default_page",
                    "startup_animation",
                    "startup_duration_ms",
                ],
                path,
                raw,
                diags,
            );
            check_subtable(
                root,
                "theme",
                &[
                    "name",
                    "background",
                    "panel_background",
                    "primary",
                    "secondary",
                    "accent",
                    "text",
                    "muted",
                    "border",
                    "danger",
                    "border_type",
                    "borders",
                ],
                path,
                raw,
                diags,
            );
            check_subtable(
                root,
                "startup",
                &["mode", "animation", "trigger", "cursor"],
                path,
                raw,
                diags,
            );
            check_subtable(
                root,
                "typography",
                &[
                    "font",
                    "logo_font",
                    "use_nerd_fonts",
                    "ascii_fallback",
                    "logo_preset",
                    "custom_logo",
                    "custom_logo_file",
                ],
                path,
                raw,
                diags,
            );
            check_subtable(
                root,
                "header",
                &["enabled", "show_clock", "show_date", "show_indicators", "show_fps"],
                path,
                raw,
                diags,
            );
            check_subtable(
                root,
                "background",
                &[
                    "effect",
                    "enabled",
                    "speed",
                    "density",
                    "opacity",
                    "region",
                    "matrix",
                    "water",
                    "particles",
                    "scanlines",
                ],
                path,
                raw,
                diags,
            );
            check_subtable(
                root,
                "layout",
                &["mode", "padding", "gap", "min_width_for_split"],
                path,
                raw,
                diags,
            );
        }
        ResourceKind::Theme => {
            let tbl = root.get("theme").and_then(|v| v.as_table()).unwrap_or(root);
            let prefix = if root.contains_key("theme") { "theme" } else { "" };
            check_allowed_keys(
                tbl,
                &[
                    "name",
                    "background",
                    "panel_background",
                    "primary",
                    "secondary",
                    "accent",
                    "text",
                    "muted",
                    "border",
                    "danger",
                    "border_type",
                    "borders",
                ],
                prefix,
                path,
                raw,
                diags,
            );
        }
        ResourceKind::WindowConfig => {
            check_allowed_keys(
                root,
                &[
                    "id",
                    "title",
                    "context",
                    "position",
                    "anchor",
                    "x",
                    "y",
                    "offset_x",
                    "offset_y",
                    "width",
                    "height",
                    "min_width",
                    "min_height",
                    "draggable",
                    "movable",
                    "resizable",
                    "minimizable",
                    "maximizable",
                    "fullscreen",
                    "visible",
                    "static",
                    "layer",
                    "z_index",
                    "overlap",
                    "padding",
                    "border",
                    "background",
                ],
                "",
                path,
                raw,
                diags,
            );
        }
        ResourceKind::Layout => {
            check_allowed_keys(
                root,
                &["name", "view_mode", "layout", "widgets"],
                "",
                path,
                raw,
                diags,
            );
        }
        ResourceKind::Animation | ResourceKind::Background | ResourceKind::Font => {}
    }
}

fn check_subtable(
    root: &toml::map::Map<String, toml::Value>,
    section: &str,
    allowed: &[&str],
    path: &Path,
    raw: &str,
    diags: &mut Vec<Diagnostic>,
) {
    if let Some(sub) = root.get(section).and_then(|v| v.as_table()) {
        check_allowed_keys(sub, allowed, section, path, raw, diags);
    }
}

fn check_allowed_keys(
    tbl: &toml::map::Map<String, toml::Value>,
    allowed: &[&str],
    prefix: &str,
    path: &Path,
    raw: &str,
    diags: &mut Vec<Diagnostic>,
) {
    for key in tbl.keys() {
        if !allowed.iter().any(|&a| a == key) {
            let field_path = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}.{key}")
            };
            let (l, c) = locate_field_in_toml(raw, &field_path);
            diags.push(
                Diagnostic::error("E1002", format!("Unknown configuration field '{field_path}'"))
                    .with_location(Some(path), l, c)
                    .with_field(field_path)
                    .with_expected_actual(allowed.join(", "), key.clone())
                    .with_suggestion("Remove or rename the unknown key"),
            );
        }
    }
}

/// Utility helper returning built-in preset theme table for lookup during validation.
pub fn builtin_themes_lookup() -> BTreeMap<String, ThemeEntry> {
    let mut map = BTreeMap::new();
    for &name in builtin_theme_names() {
        map.insert(
            name.to_string(),
            ThemeEntry {
                name: name.to_string(),
                theme: preset_theme(name),
                source_path: None,
            },
        );
    }
    map
}
