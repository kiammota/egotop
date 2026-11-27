use crate::config::paths;
use crate::ui::fonts::{FigletFont, available_fonts};
use anyhow::Result;
use clap::{Args, Subcommand};
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Args)]
pub struct FontsArgs {
    #[command(subcommand)]
    pub command: FontsCommand,
}

#[derive(Debug, Subcommand)]
pub enum FontsCommand {
    /// List built-in fonts and installed custom FIGlet (.flf) fonts
    List,
}

#[derive(Debug, Clone)]
pub struct CustomFontInfo {
    pub name: String,
    pub path: PathBuf,
    pub valid: bool,
}

pub fn handle(args: FontsArgs) -> Result<i32> {
    match args.command {
        FontsCommand::List => list_command(),
    }
}

fn list_command() -> Result<i32> {
    println!("Built-in fonts:");
    for font in available_fonts() {
        println!("  - {font} [built-in]");
    }

    let fonts_dir = paths::fonts_path();
    let custom = scan_custom_fonts();
    println!("\nCustom fonts directory: {}", fonts_dir.display());
    if custom.is_empty() {
        println!("  (none installed)");
    } else {
        for item in custom {
            let status = if item.valid { "ok" } else { "invalid .flf" };
            println!(
                "  - {} [{} ({})]",
                item.name,
                item.path.display(),
                status
            );
        }
    }
    Ok(0)
}

pub fn scan_custom_fonts() -> Vec<CustomFontInfo> {
    let dir = paths::fonts_path();
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("flf") {
            continue;
        }
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("custom")
            .to_string();
        let valid = fs::read_to_string(&path)
            .ok()
            .and_then(|raw| FigletFont::from_flf_str(&name, &raw))
            .is_some();
        out.push(CustomFontInfo { name, path, valid });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

pub fn discover_custom_font_names() -> BTreeSet<String> {
    scan_custom_fonts()
        .into_iter()
        .filter(|f| f.valid)
        .map(|f| f.name.to_ascii_lowercase())
        .collect()
}
