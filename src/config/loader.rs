use crate::config::schema::{Config, ConfigError};
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
    if let Some(cfg_dir) = dirs::config_dir() {
        return cfg_dir.join("egotop.toml");
    }
    if let Some(home) = dirs::home_dir() {
        return home.join(".config").join("egotop.toml");
    }
    PathBuf::from("egotop.toml")
}

pub fn load_or_create(custom_path: Option<&Path>) -> Result<ConfigLoadResult, ConfigError> {
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
