pub mod animation_runtime;
pub mod api;
pub mod builtins;
pub mod canvas;
pub mod effects;
pub mod fonts;
pub mod serializer;

use crate::config::{Config, ThemeConfig, ThemeEntry, paths};
use crate::ui::theme::Theme;
use mlua::{Lua, Value};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

pub use animation_runtime::{AnimationLayer, LuaAnimationRuntime};
pub use crate::animation::AnimSlotKind;
pub use builtins::{BUILTIN_LUA_MODULES, ensure_lua_config_tree, ensure_lua_tree_at};
pub use canvas::{CanvasBuffer, LuaCanvasHandle};
pub use effects::{LuaCanvasInstance, LuaEffectAdapter, LuaEffectSpec};
pub use fonts::{list_lua_fonts, load_lua_font};
pub use serializer::serialize_config_to_lua;

#[derive(Debug, Clone)]
pub struct LuaConfigBundle {
    pub config: Config,
    pub theme: Theme,
    pub theme_config: ThemeConfig,
    pub effect_specs: Vec<LuaEffectSpec>,
    pub wait_animation: Option<String>,
    pub diagnostics: Vec<String>,
}

impl Default for LuaConfigBundle {
    fn default() -> Self {
        let cfg = Config::default();
        let theme = Theme::from_config(&cfg.theme);
        let theme_config = cfg.theme.clone();
        Self {
            config: cfg,
            theme,
            theme_config,
            effect_specs: Vec::new(),
            wait_animation: None,
            diagnostics: Vec::new(),
        }
    }
}

/// Configures `package.path` and installs an embedded builtin fallback searcher
/// while giving user files in `~/.config/egotop/` highest priority.
pub fn configure_lua_vm(lua: &Lua, extra_root: Option<&Path>) -> mlua::Result<()> {
    let mut search_roots = Vec::new();
    if let Some(r) = extra_root {
        search_roots.push(r.to_path_buf());
    }
    search_roots.push(paths::root_path());
    search_roots.push(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lua"));

    let mut path_parts = Vec::new();
    for root in &search_roots {
        let s = root.display();
        path_parts.push(format!("{s}/?.lua"));
        path_parts.push(format!("{s}/?/init.lua"));
    }

    let globals = lua.globals();
    let package: mlua::Table = globals.get("package")?;
    let existing_path: String = package.get("path").unwrap_or_default();
    let new_path = if existing_path.is_empty() {
        path_parts.join(";")
    } else {
        format!("{};{existing_path}", path_parts.join(";"))
    };
    package.set("path", new_path)?;

    // Install embedded module fallback as the last searcher in `package.searchers`
    let searchers: mlua::Table = package.get("searchers")?;
    let fallback = lua.create_function(|lua, modname: String| {
        if let Some(src) = builtins::find_builtin_source(&modname) {
            let chunk = lua.load(src).set_name(format!("@{modname}"));
            let func = chunk.into_function()?;
            Ok(Value::Function(func))
        } else {
            Ok(Value::String(
                lua.create_string(format!("\n\tno embedded egotop module '{modname}'"))?,
            ))
        }
    })?;
    let next_idx = searchers.raw_len() + 1;
    searchers.set(next_idx, fallback)?;

    Ok(())
}

/// Evaluates an `egotop.lua` entrypoint script and collects the full declared configuration bundle.
pub fn load_lua_config_from_path(path: &Path) -> Result<LuaConfigBundle, String> {
    let code = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read Lua config '{}': {e}", path.display()))?;
    let root = path.parent().filter(|p| !p.as_os_str().is_empty());
    load_lua_config_str(&code, &path.display().to_string(), root)
}

pub fn load_lua_config_str(
    code: &str,
    chunk_name: &str,
    config_root: Option<&Path>,
) -> Result<LuaConfigBundle, String> {
    let lua = Lua::new();
    configure_lua_vm(&lua, config_root).map_err(|e| format!("Lua VM init error: {e}"))?;

    let state = Arc::new(Mutex::new(LuaConfigBundle::default()));
    api::register_egotop_api(&lua, Arc::clone(&state))
        .map_err(|e| format!("Failed to register egotop Lua API: {e}"))?;

    lua.load(code)
        .set_name(chunk_name)
        .exec()
        .map_err(|e| format!("Lua error in {chunk_name}: {e}"))?;

    let mut bundle = state
        .lock()
        .map_err(|_| "Lua config state mutex poisoned".to_string())?
        .clone();

    bundle.theme = Theme::from_config(&bundle.theme_config);
    if !bundle.config.general.startup_animation.is_empty() && bundle.wait_animation.is_none() {
        bundle.wait_animation = Some(bundle.config.general.startup_animation.clone());
    }
    Ok(bundle)
}

/// Loads all Lua themes from `~/.config/egotop/themes/*.lua` as well as embedded builtin themes.
pub fn load_all_lua_themes() -> BTreeMap<String, ThemeEntry> {
    let mut map = BTreeMap::new();
    let lua = Lua::new();
    if configure_lua_vm(&lua, None).is_err() {
        return map;
    }

    // 1. Embedded builtin theme modules
    for (mod_name, rel_path, _) in BUILTIN_LUA_MODULES {
        if let Some(short) = mod_name.strip_prefix("themes.")
            && !short.starts_with('_')
            && let Some(entry) = eval_theme_module(&lua, mod_name, None)
        {
            let _ = rel_path;
            map.insert(entry.name.to_ascii_lowercase(), entry);
        }
    }

    // 2. Disk themes under ~/.config/egotop/themes/*.lua (can override or add new ones)
    let dir = paths::themes_path();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("lua") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            if stem.starts_with('_') {
                continue;
            }
            if let Ok(raw) = std::fs::read_to_string(&path)
                && let Some(mut te) = eval_theme_chunk(&lua, &raw, stem, Some(path.clone()))
            {
                if te.name.is_empty() {
                    te.name = stem.to_string();
                }
                map.insert(te.name.to_ascii_lowercase(), te);
            }
        }
    }

    map
}

fn eval_theme_module(
    lua: &Lua,
    module_name: &str,
    source_path: Option<std::path::PathBuf>,
) -> Option<ThemeEntry> {
    let code = format!("local m = require('{module_name}'); return m.setup and m.setup() or m");
    let stem = module_name.split('.').next_back().unwrap_or("custom");
    eval_theme_chunk(lua, &code, stem, source_path)
}

fn eval_theme_chunk(
    lua: &Lua,
    code: &str,
    fallback_name: &str,
    source_path: Option<std::path::PathBuf>,
) -> Option<ThemeEntry> {
    let val: Value = lua.load(code).eval().ok()?;
    let tbl = match val {
        Value::Table(t) => {
            if let Ok(Value::Function(setup_fn)) = t.get::<Value>("setup") {
                setup_fn.call::<mlua::Table>(()).ok()?
            } else {
                t
            }
        }
        Value::Function(f) => f.call::<mlua::Table>(()).ok()?,
        _ => return None,
    };
    let mut tcfg = ThemeConfig::default();
    api::apply_theme_table(&tbl, &mut tcfg);
    if tcfg.name.trim().is_empty() {
        tcfg.name = fallback_name.to_string();
    }
    let theme = Theme::from_config(&tcfg);
    Some(ThemeEntry {
        name: tcfg.name.clone(),
        theme,
        source_path,
    })
}

pub fn list_lua_animations() -> Vec<String> {
    let mut names = Vec::new();
    for (mod_name, _, _) in BUILTIN_LUA_MODULES {
        if let Some(short) = mod_name.strip_prefix("animations.")
            && !short.starts_with('_')
        {
            names.push(short.to_string());
        }
    }
    let dir = paths::animations_path();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) == Some("lua")
                && let Some(stem) = p.file_stem().and_then(|s| s.to_str())
                && !stem.starts_with('_')
                && !names.iter().any(|n| n == stem)
            {
                names.push(stem.to_string());
            }
        }
    }
    names.sort();
    names
}

pub fn list_lua_effects() -> Vec<String> {
    let mut names = Vec::new();
    for (mod_name, _, _) in BUILTIN_LUA_MODULES {
        if let Some(short) = mod_name.strip_prefix("effects.")
            && !short.starts_with('_')
        {
            names.push(short.to_string());
        }
    }
    let dir = paths::effects_path();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) == Some("lua")
                && let Some(stem) = p.file_stem().and_then(|s| s.to_str())
                && !stem.starts_with('_')
                && !names.iter().any(|n| n == stem)
            {
                names.push(stem.to_string());
            }
        }
    }
    names.sort();
    names
}

/// Lists scripts with an explicit Lua module `type` matching the requested slot.
pub fn list_scripts_for_slot(slot: AnimSlotKind) -> Vec<String> {
    let expected = match slot { AnimSlotKind::Intro => "intro", AnimSlotKind::Background => "background", AnimSlotKind::Canvas => "canvas" };
    let roots = [paths::animations_path(), paths::effects_path(), Path::new(env!("CARGO_MANIFEST_DIR")).join("lua/animations"), Path::new(env!("CARGO_MANIFEST_DIR")).join("lua/effects")];
    let mut out = Vec::new();
    for dir in roots {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("lua") { continue; }
                let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else { continue; };
                if stem.starts_with('_') || stem.eq_ignore_ascii_case("none") { continue; }
                let Ok(source) = std::fs::read_to_string(&path) else { continue; };
                if (source.contains(&format!("type = \"{expected}\"")) || source.contains(&format!("type = '{expected}'"))) && !out.iter().any(|n: &String| n == stem) { out.push(stem.to_string()); }
            }
        }
    }
    out.sort();
    out
}

/// Validates that a script name is compatible with the target slot.
pub fn validate_script_for_slot(script_name: &str, target_slot: AnimSlotKind) -> Result<(), String> {
    let normalized = normalize_animation_alias(script_name);
    if normalized == "none" || list_scripts_for_slot(target_slot).iter().any(|n| n.eq_ignore_ascii_case(&normalized)) { return Ok(()); }
    for other in [AnimSlotKind::Intro, AnimSlotKind::Background, AnimSlotKind::Canvas] {
        if other != target_slot && list_scripts_for_slot(other).iter().any(|n| n.eq_ignore_ascii_case(&normalized)) {
            return Err(format!("'{script_name}' has type {other:?}; it cannot be used in the {target_slot:?} slot"));
        }
    }
    Err(format!("{} script '{script_name}' not found", match target_slot { AnimSlotKind::Intro => "Intro", AnimSlotKind::Background => "Background", AnimSlotKind::Canvas => "Canvas" }))
}

/// Maps well-known effect/preset aliases to their canonical script stem without special-casing runtime playback.
pub fn normalize_animation_alias(raw: &str) -> String {
    let norm = raw
        .trim()
        .to_ascii_lowercase()
        .replace([' ', '-'], "_");
    let stripped = norm
        .strip_prefix("effects.")
        .or_else(|| norm.strip_prefix("animations."))
        .unwrap_or(&norm);
    match stripped {
        "matrix_rain" => "matrix".to_string(),
        "waves" | "wave" => "water".to_string(),
        "stars" => "starfield".to_string(),
        "crt" => "scanlines".to_string(),
        "floating" | "chars" => "floating_chars".to_string(),
        "particle" => "particles".to_string(),
        other => other.to_string(),
    }
}

/// Resolves any canvas animation/effect name uniformly across disk overrides and built-in Lua modules
/// (`effects.*` and `animations.*`).
pub fn resolve_canvas_module_name(name: &str) -> Option<String> {
    let trimmed = name.trim().to_ascii_lowercase().replace([' ', '-'], "_");
    if trimmed.is_empty() || matches!(trimmed.as_str(), "none" | "off") {
        return None;
    }

    if let Some((ns, stem)) = trimmed.split_once('.') {
        let dir = if ns == "animations" {
            paths::animations_path()
        } else {
            paths::effects_path()
        };
        if dir.join(format!("{stem}.lua")).exists()
            || builtins::find_builtin_source(&format!("{ns}.{stem}")).is_some()
        {
            return Some(format!("{ns}.{stem}"));
        }
    }

    let canon = normalize_animation_alias(&trimmed);
    for (dir, ns) in [
        (paths::effects_path(), "effects"),
        (paths::animations_path(), "animations"),
    ] {
        if dir.join(format!("{canon}.lua")).exists() {
            return Some(format!("{ns}.{canon}"));
        }
        let mod_path = format!("{ns}.{canon}");
        if builtins::find_builtin_source(&mod_path).is_some() {
            return Some(mod_path);
        }
    }
    None
}

/// Returns the unified list of all registered Lua canvas animations across `effects/` and `animations/`.
pub fn list_all_canvas_animations() -> Vec<String> {
    let mut names = list_lua_effects();
    for anim in list_lua_animations() {
        if !names.iter().any(|n| n.eq_ignore_ascii_case(&anim)) {
            names.push(anim);
        }
    }
    names.retain(|n| !n.eq_ignore_ascii_case("none"));
    names.sort();
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animation::BackgroundEffect;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    #[test]
    fn lua_entrypoint_themes_effects_animations_and_fonts_work_end_to_end() {
        // 1. Default egotop.lua loads full ConfigBundle cleanly
        let bundle = load_lua_config_str(builtins::DEFAULT_EGOTOP_LUA, "egotop.lua", None).unwrap();
        assert_eq!(bundle.config.general.name, "egotop");
        assert_eq!(bundle.config.footer.text, "Monitoring system telemetry");
        assert!(!bundle.config.init.wait_for_input);
        assert!(bundle.effect_specs.is_empty(), "global backgrounds are opt-in");
        assert!(crate::ui::container::has_container(&bundle.config.ui.containers, "logo"));
        assert!(crate::ui::container::has_container(&bundle.config.ui.containers, "system"));
        assert!(crate::ui::container::has_container(&bundle.config.ui.containers, "hardware"));
        assert!(crate::ui::container::has_container(&bundle.config.ui.containers, "session"));

        // 2. All builtin Lua themes resolve cleanly (including bamboo and mixed transparency themes)
        let themes = load_all_lua_themes();
        assert!(themes.contains_key("bamboo"));
        assert!(themes.contains_key("matrix"));
        assert!(themes.contains_key("glass-neon"));
        assert_eq!(themes["glass-neon"].theme.background, ratatui::style::Color::Reset);

        // 3. Lua fonts resolve and render ASCII glyphs
        let custom_font = load_lua_font("custom").unwrap();
        let rendered = custom_font.render_lines("A");
        assert!(!rendered.is_empty());
        assert!(load_lua_font("ANSI Shadow").is_some());

        // 4. Lua background effect runs update & render without error
        let mut fx = LuaEffectAdapter::from_effect_name(
            "matrix",
            40,
            12,
            &bundle.config.background,
            bundle.theme.clone(),
        );
        fx.update(0.1, &bundle.config.background);
        assert!(fx.last_error().is_none(), "lua fx error: {:?}", fx.last_error());
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 12));
        fx.render(&mut buf, Rect::new(0, 0, 40, 12), &bundle.config.background, &bundle.theme);

        // 5. Lua programmable animation (christmas) runs sequence + snow + reveal to completion
        let mut anim = LuaAnimationRuntime::start("christmas", 60, 24, 30);
        for _ in 0..80 {
            anim.update(0.05);
        }
        assert!(anim.last_error.is_none());
        assert!(anim.is_finished());
        assert!((anim.reveal_progress() - 1.0).abs() < 0.01);
    }
}
