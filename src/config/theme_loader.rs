use crate::config::paths;
use crate::config::schema::ThemeConfig;
use crate::ui::theme::{Theme, builtin_theme_names, preset_theme};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ThemeEntry {
    pub name: String,
    pub theme: Theme,
    pub source_path: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct WrappedThemeFile {
    theme: ThemeConfig,
}

pub fn load_all_themes() -> BTreeMap<String, ThemeEntry> {
    let mut map = BTreeMap::new();

    for name in builtin_theme_names() {
        map.insert(
            (*name).to_string(),
            ThemeEntry {
                name: (*name).to_string(),
                theme: preset_theme(name),
                source_path: None,
            },
        );
    }

    let dir = paths::themes_path();
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }
            if let Ok(raw) = fs::read_to_string(&path)
                && let Some(cfg) = parse_theme_toml(&raw)
            {
                let stem = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("custom")
                    .to_string();
                let name = if cfg.name.trim().is_empty() {
                    stem
                } else {
                    cfg.name.trim().to_string()
                };
                let key = name.to_ascii_lowercase();
                let mut theme = Theme::from_config(&cfg);
                theme.name = name.clone();
                map.insert(
                    key,
                    ThemeEntry {
                        name,
                        theme,
                        source_path: Some(path),
                    },
                );
            }
        }
    }

    map
}

fn parse_theme_toml(raw: &str) -> Option<ThemeConfig> {
    if let Ok(wrapped) = toml::from_str::<WrappedThemeFile>(raw) {
        return Some(wrapped.theme);
    }
    toml::from_str::<ThemeConfig>(raw).ok()
}
