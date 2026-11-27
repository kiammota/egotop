use std::path::PathBuf;

pub fn root_path() -> PathBuf {
    dirs::config_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join(".config")))
        .map(|d| d.join("egotop"))
        .unwrap_or_else(|| PathBuf::from(".egotop"))
}

pub fn main_config_path() -> PathBuf {
    root_path().join("egotop.toml")
}

pub fn themes_path() -> PathBuf {
    root_path().join("themes")
}

pub fn animations_path() -> PathBuf {
    root_path().join("animations")
}

pub fn fonts_path() -> PathBuf {
    root_path().join("fonts")
}

pub fn layouts_path() -> PathBuf {
    root_path().join("layouts")
}
