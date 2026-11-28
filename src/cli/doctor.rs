use crate::animation::AnimationCatalog;
use crate::cli::config::{resolve_config_path, validate_config_file};
use crate::config::{load_all_themes, load_from_path, load_layout_names, paths};
use crate::runtime::DiagnosticSeverity;
use crate::system::SystemCollector;
use crate::ui::fonts::available_fonts;
use crate::ui::theme::terminal_supports_truecolor;
use anyhow::Result;
use crossterm::terminal;
use std::path::Path;

pub fn handle(config_override: Option<&Path>) -> Result<i32> {
    let mut failures = 0usize;
    let mut warnings = 0usize;

    println!("egotop doctor — installation & environment diagnostics\n");

    let root = paths::root_path();
    if root.is_dir() {
        println!("[OK]   Config directory: {}", root.display());
    } else {
        warnings += 1;
        println!(
            "[WARN] Config directory not created yet: {} (created on first run)",
            root.display()
        );
    }

    let cfg_path = resolve_config_path(config_override);
    if cfg_path.is_file() {
        match validate_config_file(&cfg_path) {
            Ok(diags) => {
                let err_count = diags
                    .iter()
                    .filter(|d| d.severity == DiagnosticSeverity::Error)
                    .count();
                if err_count == 0 {
                    if let Ok(cfg) = load_from_path(&cfg_path) {
                        println!(
                            "[OK]   Main config: {} (theme='{}', background='{}')",
                            cfg_path.display(),
                            cfg.theme.name,
                            cfg.background.active_effect_label()
                        );
                    }
                } else {
                    failures += 1;
                    println!(
                        "[FAIL] Main config: {} ({err_count} validation error(s))",
                        cfg_path.display()
                    );
                }
            }
            Err(err) => {
                failures += 1;
                println!("[FAIL] Main config '{}': {err}", cfg_path.display());
            }
        }
    } else if config_override.is_some() {
        failures += 1;
        println!(
            "[FAIL] Main config path does not exist: {}",
            cfg_path.display()
        );
    } else {
        warnings += 1;
        println!(
            "[WARN] Main config '{}' does not exist yet (defaults will be generated on launch)",
            cfg_path.display()
        );
    }

    let themes = load_all_themes();
    println!("[OK]   Themes available: {}", themes.len());

    let catalog = AnimationCatalog::load();
    println!("[OK]   Animations available: {}", catalog.entries.len());

    println!("[OK]   Embedded fonts available: {}", available_fonts().len());

    let layouts = load_layout_names();
    println!("[OK]   Layout presets discovered: {}", layouts.len());

    let mut collector = SystemCollector::new();
    let snap = collector.collect(80, 24);
    println!(
        "[OK]   System collector: {} ({}) on {}",
        snap.os.distro_name.as_deref().unwrap_or("Linux"),
        snap.os.kernel.as_deref().unwrap_or("unknown kernel"),
        snap.session
            .window_manager
            .as_deref()
            .or(snap.session.session_type.as_deref())
            .unwrap_or("terminal")
    );

    let truecolor = terminal_supports_truecolor();
    let term_size = terminal::size()
        .map(|(w, h)| format!("{w}x{h}"))
        .unwrap_or_else(|_| "non-TTY".to_string());
    println!("[OK]   Terminal: size={term_size}, truecolor={truecolor}");

    println!(
        "\nSummary: {} failure(s), {} warning(s)",
        failures, warnings
    );

    if failures > 0 {
        Ok(1)
    } else {
        Ok(0)
    }
}
