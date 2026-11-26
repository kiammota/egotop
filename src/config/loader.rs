use crate::config::animation_loader::ensure_sample_animations;
use crate::config::paths;
use crate::config::schema::{Config, ConfigError};
use crate::window::WindowConfig;
use std::fs;
use std::path::{Path, PathBuf};

pub const DEFAULT_CONFIG_TOML: &str = include_str!("../../config/default.toml");

#[derive(Debug, Clone)]
pub struct ConfigLoadResult {
    pub config: Config,
    pub path: PathBuf,
    pub created_default: bool,
}

pub fn default_config_path() -> PathBuf {
    paths::main_config_path()
}

pub fn ensure_config_dirs() {
    for dir in [
        paths::root_path(),
        paths::themes_path(),
        paths::animations_path(),
        paths::fonts_path(),
        paths::layouts_path(),
    ] {
        let _ = fs::create_dir_all(dir);
    }
    ensure_sample_animations();
}

pub fn load_or_create(custom_path: Option<&Path>) -> Result<ConfigLoadResult, ConfigError> {
    if custom_path.is_none() {
        ensure_config_dirs();
    }
    let path = custom_path
        .map(Path::to_path_buf)
        .unwrap_or_else(default_config_path);

    let mut created_default = false;
    if !path.exists() {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).map_err(|source| ConfigError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        fs::write(&path, DEFAULT_CONFIG_TOML).map_err(|source| ConfigError::Io {
            path: path.clone(),
            source,
        })?;
        created_default = true;
    }

    let config = load_from_path(&path)?;
    Ok(ConfigLoadResult {
        config,
        path,
        created_default,
    })
}

pub fn load_from_path(path: &Path) -> Result<Config, ConfigError> {
    let raw = fs::read_to_string(path).map_err(|source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    })?;

    let config: Config = toml::from_str(&raw).map_err(|err| ConfigError::Parse {
        path: path.to_path_buf(),
        message: err.to_string(),
    })?;

    config.validate()?;
    Ok(config)
}

pub fn save_config_to_path(path: &Path, config: &Config) -> Result<(), ConfigError> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|source| ConfigError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let serialized = toml::to_string_pretty(config).map_err(|err| ConfigError::Parse {
        path: path.to_path_buf(),
        message: err.to_string(),
    })?;
    fs::write(path, serialized).map_err(|source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    })
}

pub fn load_or_create_window_config() -> WindowConfig {
    let path = paths::window_config_path();
    if !path.exists() {
        let default_win = WindowConfig::default();
        let _ = save_window_config(&default_win);
        return default_win;
    }
    fs::read_to_string(&path)
        .ok()
        .and_then(|raw| toml::from_str::<WindowConfig>(&raw).ok())
        .unwrap_or_default()
}

pub fn save_window_config(win: &WindowConfig) -> Result<(), ConfigError> {
    let path = paths::window_config_path();
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|source| ConfigError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let serialized = toml::to_string_pretty(win).map_err(|err| ConfigError::Parse {
        path: path.clone(),
        message: err.to_string(),
    })?;
    fs::write(&path, serialized).map_err(|source| ConfigError::Io { path, source })
}
