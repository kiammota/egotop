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
            let items = complete_commands(&prefix, registry);
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

        let items = match desc.provider {
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
            CompletionProviderKind::Layout => {
                complete_layout(&prev_args, &current_token, ctx.layouts)
            }
            CompletionProviderKind::SetOption => complete_set_option(&prev_args, &current_token),
            CompletionProviderKind::Window => complete_window(&prev_args, &current_token),
            CompletionProviderKind::Effect => {
                if prev_args.is_empty() {
                    complete_from_pairs(
                        &current_token,
                        &[
                            ("matrix", "Matrix falling glyphs background"),
                            ("water", "Animated sine wave surface"),
                            ("particles", "Floating glowing particles"),
                            ("scanlines", "CRT horizontal scanline bands"),
                            ("none", "Static background with no ambient effect"),
                        ],
                        None,
                    )
                } else {
                    Vec::new()
                }
            }
            CompletionProviderKind::SaveTarget => {
                if prev_args.is_empty() {
                    complete_from_pairs(
                        &current_token,
                        &[
                            ("all", "Save egotop.toml and window.toml"),
                            ("config", "Save ~/.config/egotop/egotop.toml"),
                            ("window", "Save ~/.config/egotop/window.toml"),
                            ("theme", "Save active theme colors and borders into egotop.toml"),
                        ],
                        None,
                    )
                } else {
                    Vec::new()
                }
            }
        };

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

fn complete_commands(prefix: &str, registry: &CommandRegistry) -> Vec<CompletionItem> {
    let mut prefix_matches = Vec::new();
    let mut sub_matches = Vec::new();

    for cmd in registry.commands() {
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
                ("play", "Preview an animation on the main window"),
                ("set", "Set the startup animation"),
                ("list", "Open the interactive animation picker"),
            ],
            Some("<name>"),
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

    if prev_args.len() == 1
        && matches!(
            prev_args[0].to_ascii_lowercase().as_str(),
            "play" | "set"
        )
    {
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

    Vec::new()
}

fn complete_layout(prev_args: &[String], current: &str, layouts: &[String]) -> Vec<CompletionItem> {
    let p = current.to_ascii_lowercase();
    let mode_pairs = [
        ("responsive", "Responsive multi-band layout"),
        ("free", "Free anchor-based placement"),
        ("compact", "Compact layout for smaller windows"),
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
        if key == "border.style" {
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

fn complete_window(prev_args: &[String], current: &str) -> Vec<CompletionItem> {
    let p = current.to_ascii_lowercase();
    if prev_args.is_empty() {
        return complete_from_pairs(
            &p,
            &[
                ("center", "Center window in viewport"),
                ("anchor", "Set window anchor position"),
                ("border", "Toggle or change window border style"),
                ("bg", "Set window background (solid | transparent)"),
                ("drag", "Enable or disable window dragging"),
                ("resize", "Enable or disable window resizing"),
                ("padding", "Set internal window padding"),
                ("show", "Show the main window"),
                ("hide", "Hide the main window"),
                ("toggle", "Toggle window visibility"),
                ("reset", "Reset window geometry to default"),
            ],
            None,
        );
    }

    if prev_args.len() == 1 {
        return match prev_args[0].to_ascii_lowercase().as_str() {
            "anchor" => complete_from_pairs(
                &p,
                &[
                    ("top-left", "Top-left corner"),
                    ("top-center", "Top edge center"),
                    ("top-right", "Top-right corner"),
                    ("center-left", "Middle-left edge"),
                    ("center", "Viewport center"),
                    ("center-right", "Middle-right edge"),
                    ("bottom-left", "Bottom-left corner"),
                    ("bottom-center", "Bottom edge center"),
                    ("bottom-right", "Bottom-right corner"),
                ],
                None,
            ),
            "border" => complete_from_pairs(
                &p,
                &[
                    ("on", "Enable window border"),
                    ("off", "Disable window border"),
                    ("rounded", "Rounded border style"),
                    ("plain", "Plain sharp border style"),
                    ("double", "Double-line border style"),
                    ("thick", "Thick heavy border style"),
                ],
                None,
            ),
            "bg" | "background" => complete_from_pairs(
                &p,
                &[
                    ("solid", "Solid panel background"),
                    ("transparent", "Terminal cell transparency (Color::Reset)"),
                ],
                None,
            ),
            "drag" | "resize" => complete_from_pairs(
                &p,
                &[
                    ("on", "Enable"),
                    ("off", "Disable"),
                    ("toggle", "Toggle current state"),
                ],
                None,
            ),
            _ => Vec::new(),
        };
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
