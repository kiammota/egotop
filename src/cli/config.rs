use crate::animation::AnimationCatalog;
use crate::config::{
    Config, default_config_path, load_all_themes, load_from_path, paths,
};
use crate::runtime::pipeline::builtin_themes_lookup;
use crate::runtime::{
    Diagnostic, DiagnosticSeverity, ResourceKind, ResourceRegistry, ValidationContext,
    parse_resource, validate_schema, validate_semantics,
};
use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: Option<ConfigCommand>,
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Print loaded configuration as formatted TOML
    Show,
    /// Print the resolved configuration file path
    Path,
    /// Validate the configuration file schema and semantics
    Validate,
}

pub fn resolve_config_path(override_path: Option<&Path>) -> PathBuf {
    override_path
        .map(Path::to_path_buf)
        .unwrap_or_else(default_config_path)
}

pub fn handle(args: ConfigArgs, config_override: Option<&Path>) -> Result<i32> {
    match args.command {
        None => summary_command(config_override),
        Some(ConfigCommand::Show) => show_command(config_override),
        Some(ConfigCommand::Path) => path_command(config_override),
        Some(ConfigCommand::Validate) => validate_command(config_override),
    }
}

fn summary_command(config_override: Option<&Path>) -> Result<i32> {
    let path = resolve_config_path(config_override);
    let (cfg, from_disk) = if path.exists() {
        (load_from_path(&path)?, true)
    } else {
        (Config::default(), false)
    };

    let mut components = Vec::new();
    crate::ui::customizer::collect_component_ids(&cfg.ui.containers, &mut components);
    let enabled_components: Vec<String> = components
        .into_iter()
        .filter(|(_, visible)| *visible)
        .map(|(id, _)| id)
        .collect();

    println!("egotop configuration summary");
    println!("  path:              {} ({})", path.display(), if from_disk { "file" } else { "defaults; file not created yet" });
    println!("  name:              {}", cfg.general.name);
    println!("  theme:             {}", cfg.theme.name);
    println!("  border_type:       {}", cfg.theme.border_type.label());
    println!("  view_mode:         {}", cfg.general.view_mode.label());
    println!("  startup_mode:      {:?}", cfg.startup.mode);
    println!("  startup_animation: {}", cfg.general.startup_animation);
    println!(
        "  background:        {} (enabled={}, fps={})",
        cfg.background.active_effect_label(),
        cfg.background.enabled,
        cfg.general.animation_fps
    );
    println!("  font / logo_font:  {} / {}", cfg.typography.font, cfg.typography.logo_font);
    println!(
        "  components ({}):   {}",
        enabled_components.len(),
        if enabled_components.is_empty() {
            "none".to_string()
        } else {
            enabled_components.join(", ")
        }
    );
    Ok(0)
}

fn show_command(config_override: Option<&Path>) -> Result<i32> {
    let path = resolve_config_path(config_override);
    let cfg = if path.exists() {
        load_from_path(&path)?
    } else {
        eprintln!(
            "Note: '{}' does not exist yet; showing built-in defaults.",
            path.display()
        );
        Config::default()
    };

    if path.extension().and_then(|e| e.to_str()) == Some("toml") {
        let serialized = toml::to_string_pretty(&cfg).context("failed to serialize configuration")?;
        print!("{serialized}");
    } else {
        print!("{}", crate::lua::serialize_config_to_lua(&cfg));
    }
    Ok(0)
}

fn path_command(config_override: Option<&Path>) -> Result<i32> {
    let path = resolve_config_path(config_override);
    println!("{}", path.display());
    Ok(0)
}

pub fn validate_command(config_override: Option<&Path>) -> Result<i32> {
    let path = resolve_config_path(config_override);
    if !path.exists() {
        eprintln!("Validation failed: configuration file '{}' does not exist.", path.display());
        return Ok(1);
    }

    let diagnostics = validate_config_file(&path)?;
    let errors: Vec<&Diagnostic> = diagnostics
        .iter()
        .filter(|d| d.severity == DiagnosticSeverity::Error)
        .collect();

    if errors.is_empty() {
        let cfg = load_from_path(&path)?;
        let mut comps = Vec::new();
        crate::ui::customizer::collect_component_ids(&cfg.ui.containers, &mut comps);
        println!(
            "Configuration OK: {} (theme='{}', effect='{}', components={})",
            path.display(),
            cfg.theme.name,
            cfg.background.active_effect_label(),
            comps.len()
        );
        return Ok(0);
    }

    for err in &errors {
        for line in err.format_lines() {
            eprintln!("{line}");
        }
        eprintln!();
    }
    eprintln!(
        "Validation failed with {} error(s) in '{}'.",
        errors.len(),
        path.display()
    );
    Ok(1)
}

/// Runs the shared 3-stage `ConfigurationRuntime` validation pipeline on `path`
/// without duplicating parsing or semantic rules.
pub fn validate_config_file(path: &Path) -> Result<Vec<Diagnostic>> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read configuration file '{}'", path.display()))?;

    let parsed = match parse_resource(ResourceKind::ApplicationConfig, "egotop", path, raw) {
        Ok(p) => p,
        Err(diags) => return Ok(diags),
    };

    let mut diags = validate_schema(&parsed);
    let mut themes = builtin_themes_lookup();
    if paths::themes_path().is_dir() {
        themes.extend(load_all_themes());
    }
    let animations = AnimationCatalog::load();
    let custom_fonts = std::collections::BTreeSet::new();
    let registry = ResourceRegistry::new();
    let vctx = ValidationContext {
        themes: &themes,
        animations: &animations,
        fonts: &custom_fonts,
        registry: &registry,
    };
    diags.extend(validate_semantics(&parsed, &vctx));
    Ok(diags)
}
