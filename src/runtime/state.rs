use crate::animation::AnimationCatalog;
use crate::config::{Config, ThemeEntry};
use crate::runtime::pipeline::LayoutPreset;
use crate::window::WindowConfig;
use std::collections::{BTreeMap, BTreeSet};

/// Monotonically versioned active state preserving `LastKnownGood` across invalid updates.
#[derive(Debug, Clone)]
pub struct LastKnownGoodState {
    pub generation: u64,
    pub config: Config,
    pub window: WindowConfig,
    pub themes: BTreeMap<String, ThemeEntry>,
    pub animations: AnimationCatalog,
    pub layouts: BTreeMap<String, LayoutPreset>,
    pub custom_fonts: BTreeSet<String>,
}

impl LastKnownGoodState {
    pub fn new(
        config: Config,
        window: WindowConfig,
        themes: BTreeMap<String, ThemeEntry>,
        animations: AnimationCatalog,
        layouts: BTreeMap<String, LayoutPreset>,
        custom_fonts: BTreeSet<String>,
    ) -> Self {
        Self {
            generation: 1,
            config,
            window,
            themes,
            animations,
            layouts,
            custom_fonts,
        }
    }

    /// Commits a candidate snapshot ONLY if `candidate_generation > self.generation`,
    /// rejecting stale/out-of-order updates deterministically.
    pub fn try_commit_generation(&mut self, candidate_generation: u64) -> bool {
        if candidate_generation <= self.generation {
            return false;
        }
        self.generation = candidate_generation;
        true
    }
}

/// Summary of resources applied in an atomic runtime transaction.
#[derive(Debug, Clone, Default)]
pub struct RuntimeCommit {
    pub generation: u64,
    pub config_updated: bool,
    pub window_updated: bool,
    pub themes_updated: bool,
    pub animations_updated: bool,
    pub layouts_updated: bool,
    pub fonts_updated: bool,
    pub had_errors: bool,
    pub summary: String,
}
