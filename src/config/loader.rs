use crate::config::paths;
use crate::config::schema::{Config, ConfigError};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ConfigLoadResult {
    pub config: Config,
    pub path: PathBuf,
    pub created_default: bool,
}

pub fn default_config_path() -> PathBuf {
    paths::main_lua_path()
}

pub fn ensure_config_dirs() {
    crate::lua::ensure_lua_config_tree();
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
        if path.extension().and_then(|e| e.to_str()) == Some("lua") {
            fs::write(&path, crate::lua::builtins::DEFAULT_EGOTOP_LUA).map_err(|source| {
                ConfigError::Io {
                    path: path.clone(),
                    source,
                }
            })?;
        } else {
            save_config_to_path(&path, &Config::default())?;
        }
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
    if path.extension().and_then(|e| e.to_str()) == Some("lua") {
        let bundle = crate::lua::load_lua_config_from_path(path).map_err(|message| {
            ConfigError::Parse {
                path: path.to_path_buf(),
                message,
            }
        })?;
        bundle.config.validate()?;
        return Ok(bundle.config);
    }

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
    let serialized = if path.extension().and_then(|e| e.to_str()) == Some("lua") {
        crate::lua::serialize_config_to_lua(config)
    } else {
        toml::to_string_pretty(config).map_err(|err| ConfigError::Parse {
            path: path.to_path_buf(),
            message: err.to_string(),
        })?
    };
    fs::write(path, serialized).map_err(|source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    })
}

pub fn load_layout_names() -> Vec<String> {
    let mut names = Vec::new();
    let dir = paths::layouts_path();
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("toml")
                && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
            {
                names.push(stem.to_string());
            }
        }
    }
    names.sort();
    names
}
