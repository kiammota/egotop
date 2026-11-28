use crate::config::paths;
use std::fs;
use std::path::Path;

pub const DEFAULT_EGOTOP_LUA: &str = include_str!("../../lua/egotop.lua");

/// All embedded Lua modules mapped by `(lua_module_name, relative_disk_path, source)`.
/// Users can customize or replace any of these files under `~/.config/egotop/`.
pub const BUILTIN_LUA_MODULES: &[(&str, &str, &str)] = &[
    // Entry point
    ("egotop_default", "egotop.lua", DEFAULT_EGOTOP_LUA),
    // Themes
    ("themes._util", "themes/_util.lua", include_str!("../../lua/themes/_util.lua")),
    ("themes.bamboo", "themes/bamboo.lua", include_str!("../../lua/themes/bamboo.lua")),
    ("themes.matrix", "themes/matrix.lua", include_str!("../../lua/themes/matrix.lua")),
    ("themes.cyberpunk", "themes/cyberpunk.lua", include_str!("../../lua/themes/cyberpunk.lua")),
    ("themes.nord", "themes/nord.lua", include_str!("../../lua/themes/nord.lua")),
    ("themes.amber", "themes/amber.lua", include_str!("../../lua/themes/amber.lua")),
    ("themes.dracula", "themes/dracula.lua", include_str!("../../lua/themes/dracula.lua")),
    ("themes.obsidian_solid", "themes/obsidian_solid.lua", include_str!("../../lua/themes/obsidian_solid.lua")),
    ("themes.glass_neon", "themes/glass_neon.lua", include_str!("../../lua/themes/glass_neon.lua")),
    ("themes.frosted_slate", "themes/frosted_slate.lua", include_str!("../../lua/themes/frosted_slate.lua")),
    ("themes.solar_amber", "themes/solar_amber.lua", include_str!("../../lua/themes/solar_amber.lua")),
    ("themes.cyber_void", "themes/cyber_void.lua", include_str!("../../lua/themes/cyber_void.lua")),
    ("themes.custom", "themes/custom.lua", include_str!("../../lua/themes/custom.lua")),
    // Effects
    ("effects.matrix", "effects/matrix.lua", include_str!("../../lua/effects/matrix.lua")),
    ("effects.water", "effects/water.lua", include_str!("../../lua/effects/water.lua")),
    ("effects.particles", "effects/particles.lua", include_str!("../../lua/effects/particles.lua")),
    ("effects.scanlines", "effects/scanlines.lua", include_str!("../../lua/effects/scanlines.lua")),
    ("effects.starfield", "effects/starfield.lua", include_str!("../../lua/effects/starfield.lua")),
    ("effects.noise", "effects/noise.lua", include_str!("../../lua/effects/noise.lua")),
    ("effects.floating_chars", "effects/floating_chars.lua", include_str!("../../lua/effects/floating_chars.lua")),
    ("effects.christmas_tree", "effects/christmas_tree.lua", include_str!("../../lua/effects/christmas_tree.lua")),
    ("effects.delorean", "effects/delorean.lua", include_str!("../../lua/effects/delorean.lua")),
    ("effects.none", "effects/none.lua", include_str!("../../lua/effects/none.lua")),
    // Animations
    ("animations.matrix_boot", "animations/matrix_boot.lua", include_str!("../../lua/animations/matrix_boot.lua")),
    ("animations.cyber_boot", "animations/cyber_boot.lua", include_str!("../../lua/animations/cyber_boot.lua")),
    ("animations.christmas", "animations/christmas.lua", include_str!("../../lua/animations/christmas.lua")),
    ("animations.crt_power_on", "animations/crt_power_on.lua", include_str!("../../lua/animations/crt_power_on.lua")),
    ("animations.cursor_expand", "animations/cursor_expand.lua", include_str!("../../lua/animations/cursor_expand.lua")),
    ("animations.curtains", "animations/curtains.lua", include_str!("../../lua/animations/curtains.lua")),
    ("animations.fade", "animations/fade.lua", include_str!("../../lua/animations/fade.lua")),
    ("animations.slide", "animations/slide.lua", include_str!("../../lua/animations/slide.lua")),
    ("animations.aurora_horizon", "animations/aurora_horizon.lua", include_str!("../../lua/animations/aurora_horizon.lua")),
    ("animations.shape_morph", "animations/shape_morph.lua", include_str!("../../lua/animations/shape_morph.lua")),
    ("animations.greeting_fire", "animations/greeting_fire.lua", include_str!("../../lua/animations/greeting_fire.lua")),
    ("animations.custom", "animations/custom.lua", include_str!("../../lua/animations/custom.lua")),
    ("animations.none", "animations/none.lua", include_str!("../../lua/animations/none.lua")),
    // Fonts
    ("fonts._bitmap", "fonts/_bitmap.lua", include_str!("../../lua/fonts/_bitmap.lua")),
    ("fonts.standard", "fonts/standard.lua", include_str!("../../lua/fonts/standard.lua")),
    ("fonts.ansi_shadow", "fonts/ansi_shadow.lua", include_str!("../../lua/fonts/ansi_shadow.lua")),
    ("fonts.doom", "fonts/doom.lua", include_str!("../../lua/fonts/doom.lua")),
    ("fonts.block", "fonts/block.lua", include_str!("../../lua/fonts/block.lua")),
    ("fonts.slant", "fonts/slant.lua", include_str!("../../lua/fonts/slant.lua")),
    ("fonts.small", "fonts/small.lua", include_str!("../../lua/fonts/small.lua")),
    ("fonts.banner", "fonts/banner.lua", include_str!("../../lua/fonts/banner.lua")),
    ("fonts.custom", "fonts/custom.lua", include_str!("../../lua/fonts/custom.lua")),
];

pub fn find_builtin_source(module_name: &str) -> Option<&'static str> {
    let normalized = module_name.trim().replace('/', ".");
    BUILTIN_LUA_MODULES
        .iter()
        .find(|(m, _, _)| *m == normalized)
        .map(|(_, _, src)| *src)
}

/// Ensures `~/.config/egotop/` and its `themes/`, `effects/`, `animations/`, and `fonts/`
/// directories exist, seeding any missing built-in Lua modules without overwriting user edits.
pub fn ensure_lua_config_tree() {
    ensure_lua_tree_at(&paths::root_path());
}

pub fn ensure_lua_tree_at(root: &Path) {
    for sub in ["", "themes", "effects", "animations", "fonts", "layouts"] {
        let dir = if sub.is_empty() {
            root.to_path_buf()
        } else {
            root.join(sub)
        };
        let _ = fs::create_dir_all(dir);
    }

    for (_, rel_path, source) in BUILTIN_LUA_MODULES {
        let dest = root.join(rel_path);
        if !dest.exists() {
            if let Some(parent) = dest.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::write(dest, source);
        }
    }
}
