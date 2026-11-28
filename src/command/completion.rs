use crate::animation::AnimationCatalog;
use crate::command::registry::{CommandRegistry, CompletionProviderKind};
use crate::config::ThemeEntry;
use crate::ui::borders::{BorderChars, suggested_chars_for_property};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionItem {
    pub insert_text: String,
    pub label: String,
    pub description: String,
    pub expected_arg: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CompletionResult {
    pub items: Vec<CompletionItem>,
    pub replace_start: usize,
    pub replace_end: usize,
}

pub struct CompletionContext<'a> {
    pub themes: &'a [ThemeEntry],
    pub animations: &'a AnimationCatalog,
    pub layouts: &'a [String],
    pub containers: &'a [String],
}

pub struct CompletionEngine;

impl CompletionEngine {
    pub fn complete(
        input: &str,
        cursor: usize,
        registry: &CommandRegistry,
        ctx: &CompletionContext<'_>,
    ) -> CompletionResult {
        let clamped_cursor = clamp_char_boundary(input, cursor);
        let before = &input[..clamped_cursor];
        let trimmed_start = before.trim_start();
        let leading_ws = before.len() - trimmed_start.len();

        // Extend token end slightly if cursor is mid-word
        let token_end = find_token_end(input, clamped_cursor);

        // Determine if cursor is still in the command name token (no space after trimmed_start)
        let first_ws = trimmed_start.find(char::is_whitespace);
        if first_ws.is_none() {
            let prefix = trimmed_start.to_ascii_lowercase();
            let items = finalize_suggestions(complete_commands(&prefix, registry), &prefix);
            return CompletionResult {
                items,
                replace_start: leading_ws,
                replace_end: token_end,
            };
        }

        // Parse completed tokens before the token currently being edited
        let (cmd_token, rest_before) = trimmed_start.split_at(first_ws.unwrap());
        let cmd_lower = cmd_token.to_ascii_lowercase();
        let Some(desc) = registry.find(&cmd_lower) else {
            return CompletionResult {
                items: Vec::new(),
                replace_start: clamped_cursor,
                replace_end: clamped_cursor,
            };
        };

        let (prev_args, current_token, replace_start) =
            split_args_and_current(rest_before, leading_ws + cmd_token.len(), clamped_cursor);

        let items = finalize_suggestions(
            match desc.provider {
                CompletionProviderKind::None => Vec::new(),
                CompletionProviderKind::Colorscheme => {
                    if prev_args.is_empty() {
                        complete_themes(&current_token, ctx.themes)
                    } else {
                        Vec::new()
                    }
                }
                CompletionProviderKind::Animation => {
                    complete_animation(&prev_args, &current_token, ctx.animations)
                }
                CompletionProviderKind::Background => {
                    complete_background(&prev_args, &current_token, ctx.animations)
                }
                CompletionProviderKind::Intro => {
                    let mut items = complete_from_pairs(&current_token.to_ascii_lowercase(), &[("replay", "Replay the configured intro"), ("off", "Disable the intro")], None);
                    if prev_args.is_empty() {
                        for name in crate::lua::list_scripts_for_slot(crate::animation::AnimSlotKind::Intro) {
                            if name.to_ascii_lowercase().starts_with(&current_token.to_ascii_lowercase()) {
                                items.push(CompletionItem { insert_text: name.clone(), label: name, description: "Intro animation".into(), expected_arg: None });
                            }
                        }
                    }
                    items
                }
                CompletionProviderKind::Canvas => {
                    let p = current_token.to_ascii_lowercase();
                    if prev_args.is_empty() {
                        complete_from_pairs(&p, &ctx.containers.iter().map(|n| (n.as_str(), "Container id")).collect::<Vec<_>>(), None)
                    } else if prev_args.len() == 1 {
                        let mut items = complete_from_pairs(&p, &[("off", "Disable this canvas"), ("pause", "Pause this canvas"), ("resume", "Resume this canvas")], None);
                        for name in crate::lua::list_scripts_for_slot(crate::animation::AnimSlotKind::Canvas) {
                            if name.to_ascii_lowercase().starts_with(&p) { items.push(CompletionItem { insert_text: name.clone(), label: name, description: "Canvas animation".into(), expected_arg: None }); }
                        }
                        items
                    } else { Vec::new() }
                }
                CompletionProviderKind::Border => {
                    complete_border(&prev_args, &current_token, ctx.containers)
                }
                CompletionProviderKind::Layout => {
                    complete_layout(&prev_args, &current_token, ctx.layouts)
                }
                CompletionProviderKind::Container => {
                    complete_container(&prev_args, &current_token, ctx.containers)
                }
                CompletionProviderKind::SetOption => complete_set_option(&prev_args, &current_token),
                CompletionProviderKind::WriteTarget => {
                    if prev_args.is_empty() {
                        complete_from_pairs(
                            &current_token,
                            &[
                                ("all", "Write egotop.toml"),
                                ("config", "Write ~/.config/egotop/egotop.toml"),
                                (
                                    "theme",
                                    "Write active theme colors and borders into egotop.toml",
                                ),
                            ],
                            None,
                        )
                    } else {
                        Vec::new()
                    }
                }
            },
            &current_token,
        );

        CompletionResult {
            items,
            replace_start,
            replace_end: token_end,
        }
    }
}

fn clamp_char_boundary(s: &str, idx: usize) -> usize {
    let mut i = idx.min(s.len());
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn find_token_end(input: &str, cursor: usize) -> usize {
    let rest = &input[cursor..];
    match rest.find(char::is_whitespace) {
        Some(rel) => cursor + rel,
        None => input.len(),
    }
}

fn split_args_and_current(
    rest_before: &str,
    base_offset: usize,
    cursor: usize,
) -> (Vec<String>, String, usize) {
    let ends_with_ws = rest_before
        .chars()
        .last()
        .is_some_and(char::is_whitespace);

    let tokens: Vec<&str> = rest_before.split_whitespace().collect();
    if ends_with_ws {
        let prev = tokens.into_iter().map(str::to_string).collect();
        (prev, String::new(), cursor)
    } else if let Some((last, head)) = tokens.split_last() {
        let prev = head.iter().map(|s| (*s).to_string()).collect();
        let last_start = base_offset + rest_before.rfind(last).unwrap_or(0);
        (prev, (*last).to_string(), last_start)
    } else {
        (Vec::new(), String::new(), cursor)
    }
}

/// Removes exact matches against the token being typed (so completed tokens do not
/// suggest themselves) and deduplicates suggestions while preserving priority order.
fn finalize_suggestions(items: Vec<CompletionItem>, current_token: &str) -> Vec<CompletionItem> {
    let token_lower = current_token.trim().to_ascii_lowercase();
    let mut seen = Vec::with_capacity(items.len());
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let candidate_lower = item.insert_text.to_ascii_lowercase();
        if !token_lower.is_empty() && candidate_lower == token_lower {
            continue;
        }
        if !seen.contains(&candidate_lower) {
            seen.push(candidate_lower);
            out.push(item);
        }
    }
    out
}

fn complete_commands(prefix: &str, registry: &CommandRegistry) -> Vec<CompletionItem> {
    let mut prefix_matches = Vec::new();
    let mut sub_matches = Vec::new();

    for cmd in registry.commands() {
        if !prefix.is_empty() && cmd.name.eq_ignore_ascii_case(prefix) {
            continue;
        }
        let expected_arg = cmd.args.first().map(|a| a.name.to_string());
        let item = CompletionItem {
            insert_text: cmd.name.to_string(),
            label: format!(":{}", cmd.name),
            description: cmd.description.to_string(),
            expected_arg,
        };
        if cmd.name.starts_with(prefix) || cmd.aliases.iter().any(|a| a.starts_with(prefix)) {
            prefix_matches.push(item);
        } else if !prefix.is_empty() && cmd.name.contains(prefix) {
            sub_matches.push(item);
        }
    }

    prefix_matches.extend(sub_matches);
    prefix_matches
}

fn complete_themes(prefix: &str, themes: &[ThemeEntry]) -> Vec<CompletionItem> {
    let p = prefix.to_ascii_lowercase();
    let mut prefix_matches = Vec::new();
    let mut sub_matches = Vec::new();

    for entry in themes {
        let lower = entry.name.to_ascii_lowercase();
        let origin = if entry.source_path.is_some() {
            "User theme"
        } else {
            "Built-in theme"
        };
        let item = CompletionItem {
            insert_text: entry.name.clone(),
            label: entry.name.clone(),
            description: format!("Apply {} colorscheme ({origin})", entry.name),
            expected_arg: None,
        };
        if lower.starts_with(&p) {
            prefix_matches.push(item);
        } else if !p.is_empty() && lower.contains(&p) {
            sub_matches.push(item);
        }
    }

    prefix_matches.extend(sub_matches);
    prefix_matches
}

fn complete_animation(
    prev_args: &[String],
    current: &str,
    catalog: &AnimationCatalog,
) -> Vec<CompletionItem> {
    let p = current.to_ascii_lowercase();
    if prev_args.is_empty() {
        let mut items = complete_from_pairs(
            &p,
            &[
                ("play", "Play an animation or scene"),
                ("set", "Set the startup animation"),
                ("pause", "Pause active animation playback"),
                ("resume", "Resume paused animation playback"),
                ("stop", "Stop active animation playback"),
                ("restart", "Restart active animation from t=0"),
                ("speed", "Set playback speed multiplier (0.1..10.0)"),
                ("list", "Open the interactive animation picker"),
            ],
            Some("<arg>"),
        );
        for entry in &catalog.entries {
            if entry.id.to_ascii_lowercase().starts_with(&p) {
                items.push(CompletionItem {
                    insert_text: entry.id.clone(),
                    label: entry.id.clone(),
                    description: entry.description.clone(),
                    expected_arg: None,
                });
            }
        }
        return items;
    }

    if prev_args.len() == 1 {
        let sub = prev_args[0].to_ascii_lowercase();
        if matches!(sub.as_str(), "play" | "set") {
            return catalog
                .entries
                .iter()
                .filter(|e| {
                    let id_l = e.id.to_ascii_lowercase();
                    id_l.starts_with(&p) || (!p.is_empty() && id_l.contains(&p))
                })
                .map(|e| CompletionItem {
                    insert_text: e.id.clone(),
                    label: e.id.clone(),
                    description: e.description.clone(),
                    expected_arg: None,
                })
                .collect();
        }
        if sub == "speed" {
            return complete_from_pairs(
                &p,
                &[
                    ("0.5", "Half playback speed"),
                    ("1.0", "Normal playback speed"),
                    ("1.5", "1.5x faster playback"),
                    ("2.0", "Double playback speed"),
                ],
                None,
            );
        }
    }

    Vec::new()
}

fn complete_background(
    prev_args: &[String],
    current: &str,
    catalog: &AnimationCatalog,
) -> Vec<CompletionItem> {
    let p = current.to_ascii_lowercase();
    let presets = [
        ("matrix_rain", "Matrix falling glyphs rain"),
        ("starfield", "Parallax starfield drifting across space"),
        ("particles", "Floating glowing character particles"),
        ("waves", "Multi-wave harmonic interference surface"),
        ("noise", "Animated procedural noise field"),
        ("gradient", "Smooth animated aurora gradient bands"),
        ("floating_chars", "Gently swaying floating glyphs"),
        ("custom_ascii", "Animated ASCII art layer"),
        ("scanlines", "CRT horizontal sweep scanlines"),
        ("none", "Disable background layers"),
    ];

    if prev_args.is_empty() {
        let mut items = complete_from_pairs(
            &p,
            &[
                ("set", "Replace background with an effect or scene preset"),
                ("add", "Stack an extra background effect layer"),
                ("remove", "Remove a background effect layer by index or name"),
                ("toggle", "Toggle background rendering on/off"),
                ("stop", "Stop and clear all background effect layers"),
                ("pause", "Pause background animation updates"),
                ("resume", "Resume background animation updates"),
                ("speed", "Adjust background speed multiplier"),
                ("list", "Show active and available background effects"),
            ],
            Some("<arg>"),
        );
        items.extend(complete_from_pairs(&p, &presets, None));
        let all_anims = crate::lua::list_all_canvas_animations();
        for name in all_anims {
            if name.to_ascii_lowercase().starts_with(&p)
                && !items.iter().any(|i| i.insert_text.eq_ignore_ascii_case(&name))
            {
                items.push(CompletionItem {
                    insert_text: name.clone(),
                    label: name.clone(),
                    description: format!("Canvas background animation '{name}'"),
                    expected_arg: None,
                });
            }
        }
        return items;
    }

    if prev_args.len() == 1 {
        let sub = prev_args[0].to_ascii_lowercase();
        if matches!(sub.as_str(), "set" | "add" | "layer") {
            let mut items = complete_from_pairs(&p, &presets, None);
            let all_anims = crate::lua::list_all_canvas_animations();
            for name in all_anims {
                if name.to_ascii_lowercase().starts_with(&p)
                    && !items.iter().any(|i| i.insert_text.eq_ignore_ascii_case(&name))
                {
                    items.push(CompletionItem {
                        insert_text: name.clone(),
                        label: name.clone(),
                        description: format!("Canvas background animation '{name}'"),
                        expected_arg: None,
                    });
                }
            }
            for entry in &catalog.entries {
                if !entry.package.definition.backgrounds.is_empty()
                    && (entry.id.to_ascii_lowercase().starts_with(&p)
                        || (!p.is_empty() && entry.id.to_ascii_lowercase().contains(&p)))
                {
                    items.push(CompletionItem {
                        insert_text: entry.id.clone(),
                        label: entry.id.clone(),
                        description: format!("Scene background layers ({})", entry.display_name),
                        expected_arg: None,
                    });
                }
            }
            return items;
        }
        if sub == "toggle" {
            return complete_from_pairs(
                &p,
                &[
                    ("on", "Enable background rendering"),
                    ("off", "Disable background rendering"),
                ],
                None,
            );
        }
        if sub == "remove" || sub == "rm" || sub == "del" {
            return complete_from_pairs(
                &p,
                &[
                    ("1", "First background layer"),
                    ("matrix_rain", "Remove matrix layer"),
                    ("starfield", "Remove starfield layer"),
                    ("particles", "Remove particles layer"),
                    ("waves", "Remove waves layer"),
                    ("scanlines", "Remove scanlines layer"),
                ],
                None,
            );
        }
        if sub == "speed" {
            return complete_from_pairs(
                &p,
                &[
                    ("0.5", "Slow ambient motion"),
                    ("1.0", "Default background speed"),
                    ("1.5", "Energetic ambient motion"),
                    ("2.0", "Fast ambient motion"),
                ],
                None,
            );
        }
    }

    Vec::new()
}

fn complete_layout(prev_args: &[String], current: &str, layouts: &[String]) -> Vec<CompletionItem> {
    let p = current.to_ascii_lowercase();
    let mode_pairs = [
        ("compact", "Compact container layout"),
        ("minimal", "Minimal greeting-focused layout"),
        ("dashboard", "Full dashboard view mode"),
    ];

    if prev_args.is_empty() {
        let mut items = complete_from_pairs(
            &p,
            &[
                ("load", "Load a layout preset or mode"),
                ("mode", "Switch active layout mode"),
            ],
            Some("<name>"),
        );
        items.extend(complete_from_pairs(&p, &mode_pairs, None));
        for l in layouts {
            if l.to_ascii_lowercase().starts_with(&p)
                && !items.iter().any(|i| i.insert_text.eq_ignore_ascii_case(l))
            {
                items.push(CompletionItem {
                    insert_text: l.clone(),
                    label: l.clone(),
                    description: "Custom layout preset (~/.config/egotop/layouts/)".to_string(),
                    expected_arg: None,
                });
            }
        }
        return items;
    }

    if prev_args.len() == 1
        && matches!(
            prev_args[0].to_ascii_lowercase().as_str(),
            "load" | "mode"
        )
    {
        let mut items = complete_from_pairs(&p, &mode_pairs, None);
        for l in layouts {
            if l.to_ascii_lowercase().starts_with(&p)
                && !items.iter().any(|i| i.insert_text.eq_ignore_ascii_case(l))
            {
                items.push(CompletionItem {
                    insert_text: l.clone(),
                    label: l.clone(),
                    description: "Custom layout preset (~/.config/egotop/layouts/)".to_string(),
                    expected_arg: None,
                });
            }
        }
        return items;
    }

    Vec::new()
}

fn complete_set_option(prev_args: &[String], current: &str) -> Vec<CompletionItem> {
    let p = current.to_ascii_lowercase();

    if prev_args.is_empty() {
        let mut items = Vec::new();
        if "background".starts_with(&p) {
            items.push(CompletionItem {
                insert_text: "background".to_string(),
                label: "background".to_string(),
                description: "Theme brightness preset (dark | light)".to_string(),
                expected_arg: Some("<dark|light>".to_string()),
            });
        }

        let border_prefix = p.strip_prefix("border.").unwrap_or(&p);
        let filter_border_only = p.starts_with("border.");

        for &(prop, desc) in BorderChars::PROPERTY_SPECS {
            let full_key = format!("border.{prop}");
            let matches = if filter_border_only {
                prop.starts_with(border_prefix)
            } else {
                full_key.starts_with(&p) || prop.starts_with(&p)
            };
            if matches {
                let expected = if prop == "style" {
                    Some("<style>".to_string())
                } else {
                    Some("<char>".to_string())
                };
                items.push(CompletionItem {
                    insert_text: full_key.clone(),
                    label: full_key,
                    description: desc.to_string(),
                    expected_arg: expected,
                });
            }
        }
        return items;
    }

    if prev_args.len() == 1 {
        let key = prev_args[0].to_ascii_lowercase();
        if key == "background" {
            return complete_from_pairs(
                &p,
                &[
                    ("dark", "Dark terminal palette"),
                    ("light", "Light terminal palette"),
                ],
                None,
            );
        }
        if key == "border.style" || key == "borders.style" {
            return complete_from_pairs(&p, BorderChars::STYLE_NAMES, None);
        }
        if let Some(prop) = key.strip_prefix("border.")
            && BorderChars::PROPERTY_SPECS.iter().any(|&(k, _)| k == prop)
        {
            return complete_from_pairs(&p, suggested_chars_for_property(prop), None);
        }
    }

    Vec::new()
}

fn complete_border(
    prev_args: &[String],
    current: &str,
    containers: &[String],
) -> Vec<CompletionItem> {
    let p = current.to_ascii_lowercase();
    if prev_args.is_empty() {
        let mut items = complete_from_pairs(&p, BorderChars::STYLE_NAMES, None);
        if "background".starts_with(&p) {
            items.push(CompletionItem {
                insert_text: "background".to_string(),
                label: "background".to_string(),
                description: "Global background canvas border".to_string(),
                expected_arg: Some("<style>".to_string()),
            });
        }
        if "all".starts_with(&p) {
            items.push(CompletionItem {
                insert_text: "all".to_string(),
                label: "all".to_string(),
                description: "Apply border style to all bordered canvases".to_string(),
                expected_arg: Some("<style>".to_string()),
            });
        }
        for id in containers {
            let l = id.to_ascii_lowercase();
            if l.starts_with(&p) || (!p.is_empty() && l.contains(&p)) {
                items.push(CompletionItem {
                    insert_text: id.clone(),
                    label: id.clone(),
                    description: format!("Set border for '{id}' container canvas"),
                    expected_arg: Some("<style>".to_string()),
                });
            }
        }
        for &(prop, desc) in BorderChars::PROPERTY_SPECS {
            if prop != "style" && prop.starts_with(&p) {
                items.push(CompletionItem {
                    insert_text: prop.to_string(),
                    label: prop.to_string(),
                    description: desc.to_string(),
                    expected_arg: Some("<char>".to_string()),
                });
            }
        }
        return items;
    }

    if prev_args.len() == 1 {
        let first = prev_args[0].to_ascii_lowercase();
        if matches!(first.as_str(), "all" | "set" | "style" | "background")
            || containers.iter().any(|c| c.eq_ignore_ascii_case(&first))
        {
            return complete_from_pairs(&p, BorderChars::STYLE_NAMES, None);
        }
        if BorderChars::PROPERTY_SPECS
            .iter()
            .any(|&(k, _)| k != "style" && k == first)
        {
            return complete_from_pairs(&p, suggested_chars_for_property(&first), None);
        }
    }

    Vec::new()
}

fn complete_container(
    prev_args: &[String],
    current: &str,
    containers: &[String],
) -> Vec<CompletionItem> {
    let p = current.to_ascii_lowercase();
    if prev_args.is_empty() {
        return complete_from_pairs(
            &p,
            &[
                ("list", "List all container IDs"),
                ("inspect", "Inspect style of a container"),
                ("add", "Add a new container by ID"),
                ("remove", "Remove a container by ID"),
                ("set", "Set a style property on a container"),
                ("gradient", "Apply a multi-stop gradient to a container"),
                ("reset", "Reset container style to defaults"),
            ],
            None,
        );
    }

    let sub = prev_args[0].to_ascii_lowercase();
    if prev_args.len() == 1
        && matches!(
            sub.as_str(),
            "inspect" | "remove" | "rm" | "del" | "set" | "gradient" | "reset"
        )
    {
        return containers
            .iter()
            .filter(|id| id.to_ascii_lowercase().starts_with(&p) || id.to_ascii_lowercase().contains(&p))
            .map(|id| CompletionItem {
                insert_text: id.clone(),
                label: id.clone(),
                description: "Container ID".to_string(),
                expected_arg: None,
            })
            .collect();
    }

    if sub == "set" && prev_args.len() == 2 {
        return complete_from_pairs(
            &p,
            &[
                ("background", "inherit | transparent | <color>"),
                ("foreground", "Default content foreground color"),
                ("border.color", "Border color override"),
                ("border.style", "rounded | plain | double | thick | ascii | none"),
                ("effects", "exclude | allow | isolated"),
                ("animation", "Assign an animation/effect to this container canvas"),
            ],
            None,
        );
    }

    if sub == "set" && prev_args.len() == 3 {
        return match prev_args[2].to_ascii_lowercase().as_str() {
            "background" | "bg" => complete_from_pairs(
                &p,
                &[
                    ("inherit", "Inherit parent/global background"),
                    ("transparent", "Terminal transparency (Color::Reset)"),
                    ("#0A141E", "Hex solid color example"),
                ],
                None,
            ),
            "border.style" => complete_from_pairs(&p, BorderChars::STYLE_NAMES, None),
            "effects" => complete_from_pairs(
                &p,
                &[
                    ("exclude", "Block global effects inside container"),
                    ("allow", "Allow global effects inside container"),
                    ("isolated", "Block global effects, allow local container effects"),
                ],
                None,
            ),
            "animation" | "anim" | "script" => {
                let mut items = complete_from_pairs(
                    &p,
                    &[("none", "Disable animation on this container canvas")],
                    None,
                );
                for name in crate::lua::list_all_canvas_animations() {
                    let l = name.to_ascii_lowercase();
                    if (l.starts_with(&p) || (!p.is_empty() && l.contains(&p)))
                        && !items.iter().any(|i| i.insert_text.eq_ignore_ascii_case(&name))
                    {
                        items.push(CompletionItem {
                            insert_text: name.clone(),
                            label: name,
                            description: "Canvas animation script".to_string(),
                            expected_arg: None,
                        });
                    }
                }
                items
            }
            _ => Vec::new(),
        };
    }

    if sub == "gradient" && prev_args.len() == 2 {
        return complete_from_pairs(
            &p,
            &[
                ("horizontal", "Left-to-right gradient"),
                ("vertical", "Top-to-bottom gradient"),
            ],
            None,
        );
    }

    Vec::new()
}

fn complete_from_pairs(
    prefix: &str,
    pairs: &[(&str, &str)],
    expected_arg: Option<&str>,
) -> Vec<CompletionItem> {
    let p = prefix.to_ascii_lowercase();
    pairs
        .iter()
        .filter(|&&(name, _)| {
            let l = name.to_ascii_lowercase();
            l.starts_with(&p) || (!p.is_empty() && l.contains(&p))
        })
        .map(|&(name, desc)| CompletionItem {
            insert_text: name.to_string(),
            label: name.to_string(),
            description: desc.to_string(),
            expected_arg: expected_arg.map(str::to_string),
        })
        .collect()
}
