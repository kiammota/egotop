use crate::animation::{AnimationCatalog, BackgroundPipeline, TransitionPlayer};
use crate::config::{Config, ThemeEntry};
use crate::runtime::pipeline::LayoutPreset;
use crate::runtime::state::{LastKnownGoodState, RuntimeCommit};
use crate::ui::theme::Theme;
use ratatui::layout::Rect;
use std::collections::BTreeMap;

/// Target application state manipulated atomically by Runtime Resource Managers.
pub struct ManagedAppTarget<'a> {
    pub config: &'a mut Config,
    pub theme: &'a mut Theme,
    pub themes_map: &'a mut BTreeMap<String, ThemeEntry>,
    pub theme_list: &'a mut Vec<ThemeEntry>,
    pub animations: &'a mut AnimationCatalog,
    pub layout_names: &'a mut Vec<String>,
    pub layouts_map: &'a mut BTreeMap<String, LayoutPreset>,
    pub background: &'a mut BackgroundPipeline,
    pub transition: &'a mut TransitionPlayer,
    pub viewport: Rect,
}

/// Applies only the resources touched in `commit` from validated `LastKnownGoodState`,
/// avoiding full application restarts or losing unrelated runtime state.
pub struct RuntimeResourceManager;

impl RuntimeResourceManager {
    pub fn apply_commit(
        commit: &RuntimeCommit,
        lkg: &LastKnownGoodState,
        target: &mut ManagedAppTarget<'_>,
    ) {
        if commit.had_errors {
            return;
        }

        if commit.themes_updated {
            ThemeManager::apply(lkg, target);
        }
        if commit.animations_updated {
            AnimationManager::apply(lkg, target);
        }
        if commit.layouts_updated {
            LayoutManager::apply(lkg, target);
        }
        if commit.config_updated {
            AppConfigManager::apply(lkg, target);
            BackgroundManager::apply(lkg, target);
        }
        if commit.fonts_updated {
            FontManager::apply(lkg, target);
        }
    }
}

pub struct AppConfigManager;
impl AppConfigManager {
    pub fn apply(lkg: &LastKnownGoodState, target: &mut ManagedAppTarget<'_>) {
        let prev_startup = target.config.general.startup_animation.clone();
        *target.config = lkg.config.clone();
        ThemeManager::resolve_active(lkg, target);
        if prev_startup != target.config.general.startup_animation {
            target.transition.restart(
                &target.config.general.startup_animation,
                target.animations,
                target.config.general.startup_duration_ms,
            );
        }
    }
}

pub struct ThemeManager;
impl ThemeManager {
    pub fn apply(lkg: &LastKnownGoodState, target: &mut ManagedAppTarget<'_>) {
        *target.themes_map = lkg.themes.clone();
        *target.theme_list = lkg.themes.values().cloned().collect();
        Self::resolve_active(lkg, target);
    }

    pub fn resolve_active(_lkg: &LastKnownGoodState, target: &mut ManagedAppTarget<'_>) {
        let key = target.config.theme.name.trim().to_ascii_lowercase();
        if let Some(entry) = target.themes_map.get(&key)
            && entry.source_path.is_some()
        {
            let mut resolved = entry.theme.clone();
            let _ = resolved.borders.apply_overrides(&target.config.theme.borders);
            *target.theme = resolved;
            return;
        }
        *target.theme = Theme::from_config(&target.config.theme);
    }
}

pub struct AnimationManager;
impl AnimationManager {
    pub fn apply(lkg: &LastKnownGoodState, target: &mut ManagedAppTarget<'_>) {
        *target.animations = lkg.animations.clone();
    }
}

pub struct BackgroundManager;
impl BackgroundManager {
    pub fn apply(lkg: &LastKnownGoodState, target: &mut ManagedAppTarget<'_>) {
        *target.background = BackgroundPipeline::from_config(
            &lkg.config.background,
            target.viewport.width.max(1),
            target.viewport.height.max(1),
        );
    }
}

pub struct LayoutManager;
impl LayoutManager {
    pub fn apply(lkg: &LastKnownGoodState, target: &mut ManagedAppTarget<'_>) {
        *target.layouts_map = lkg.layouts.clone();
        *target.layout_names = lkg.layouts.keys().cloned().collect();
    }
}

pub struct FontManager;
impl FontManager {
    pub fn apply(_lkg: &LastKnownGoodState, _target: &mut ManagedAppTarget<'_>) {
        // Custom `.flf` fonts are resolved by typography on demand using font names validated in LKG.
    }
}
