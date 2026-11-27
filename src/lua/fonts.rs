use crate::config::paths;
use crate::lua::{BUILTIN_LUA_MODULES, configure_lua_vm};
use crate::ui::fonts::{FigletFont, HorizontalLayout};
use mlua::{Lua, Table, Value};
use std::collections::HashMap;
use std::path::Path;

/// Resolves and evaluates a font module (`fonts.<normalized_name>` or a custom `.lua` file
/// in `~/.config/egotop/fonts/`) into a `FigletFont` usable across greetings and animations.
pub fn load_lua_font(name: &str) -> Option<FigletFont> {
    let lua = Lua::new();
    configure_lua_vm(&lua, None).ok()?;

    let normalized = name.trim().to_ascii_lowercase().replace([' ', '-'], "_");
    let custom_path = paths::fonts_path().join(format!("{normalized}.lua"));

    let tbl: Table = if custom_path.exists() {
        eval_font_file(&lua, &custom_path)?
    } else {
        let code = format!(
            "local m = require('fonts.{normalized}'); return type(m.setup) == 'function' and m.setup() or m"
        );
        lua.load(&code).eval().ok()?
    };

    font_from_lua_table(name, &tbl)
}

fn eval_font_file(lua: &Lua, path: &Path) -> Option<Table> {
    let raw = std::fs::read_to_string(path).ok()?;
    let val: Value = lua.load(&raw).set_name(path.display().to_string()).eval().ok()?;
    match val {
        Value::Table(t) => {
            if let Ok(Value::Function(f)) = t.get::<Value>("setup") {
                f.call::<Table>(()).ok()
            } else {
                Some(t)
            }
        }
        Value::Function(f) => f.call::<Table>(()).ok(),
        _ => None,
    }
}

pub fn font_from_lua_table(fallback_name: &str, tbl: &Table) -> Option<FigletFont> {
    let name = tbl
        .get::<String>("name")
        .unwrap_or_else(|_| fallback_name.to_string());
    let chars_tbl = tbl.get::<Table>("characters").ok()?;
    let spacing = tbl.get::<usize>("spacing").unwrap_or(1);
    let layout = match tbl
        .get::<String>("layout")
        .ok()
        .map(|s| s.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("kerning") | Some("fitting") => HorizontalLayout::Kerning,
        Some("full") | Some("fullwidth") => HorizontalLayout::FullWidth,
        _ => {
            if spacing > 0 {
                HorizontalLayout::FullWidth
            } else {
                HorizontalLayout::Kerning
            }
        }
    };

    let mut glyphs = HashMap::new();
    let mut height = tbl.get::<usize>("height").unwrap_or(0);

    for pair in chars_tbl.pairs::<String, Value>() {
        let Ok((key, val)) = pair else {
            continue;
        };
        let Some(ch) = key.chars().next() else {
            continue;
        };
        let Value::Table(lines_tbl) = val else {
            continue;
        };
        let mut lines = Vec::new();
        for line_val in lines_tbl.sequence_values::<String>().flatten() {
            let expanded = if ch == ' ' {
                line_val
            } else if spacing > 0 {
                format!("{line_val}{}", " ".repeat(spacing))
            } else {
                line_val
            };
            lines.push(expanded);
        }
        if !lines.is_empty() {
            let max_chars = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
            for row in &mut lines {
                let len = row.chars().count();
                if len < max_chars {
                    row.push_str(&" ".repeat(max_chars - len));
                }
            }
            height = height.max(lines.len());
            glyphs.insert(ch, lines);
        }
    }

    if glyphs.is_empty() || height == 0 {
        return None;
    }

    // Ensure fallback space glyph if omitted
    glyphs
        .entry(' ')
        .or_insert_with(|| vec!["   ".to_string(); height]);

    Some(FigletFont {
        name,
        height,
        hardblank: '$',
        layout,
        glyphs,
    })
}

pub fn list_lua_fonts() -> Vec<String> {
    let mut names = Vec::new();
    for (mod_name, _, _) in BUILTIN_LUA_MODULES {
        if let Some(short) = mod_name.strip_prefix("fonts.")
            && !short.starts_with('_')
        {
            names.push(short.to_string());
        }
    }
    let dir = paths::fonts_path();
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
