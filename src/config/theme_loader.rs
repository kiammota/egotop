use crate::ui::theme::Theme;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ThemeEntry {
    pub name: String,
    pub theme: Theme,
    pub source_path: Option<PathBuf>,
}

pub fn load_all_themes() -> BTreeMap<String, ThemeEntry> {
    crate::lua::load_all_lua_themes()
}
