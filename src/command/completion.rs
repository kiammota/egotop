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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionQuery {
    pub arg_lead: String,
    /// Positional arguments before the token currently being completed.
    pub tokens: Vec<String>,
    /// Command name is position 0; its first positional argument is position 1.
    pub arg_index: usize,
    pub cmdline: String,
    pub cursor_pos: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenizedCommandLine {
    pub command: Option<String>,
    pub args_before: Vec<String>,
    pub query: CompletionQuery,
    pub replace_start: usize,
    pub replace_end: usize,
}

/// Tokenizes a command line up to a cursor while honoring single/double quotes
/// and backslash escapes. Replacement offsets cover only the active token.
pub fn tokenize_command_line(input: &str, cursor: usize) -> TokenizedCommandLine {
    let cursor = clamp_char_boundary(input, cursor);
    let lexed = lex_command_line(input);
    let active = lexed
        .iter()
        .position(|t| cursor >= t.start && cursor <= t.end);
    let (replace_start, replace_end, arg_lead, before_count) = if let Some(idx) = active {
        let tok = &lexed[idx];
        let lead_end = cursor.min(tok.end).max(tok.start);
        let mut lead = unquote_token(&input[tok.start..lead_end]);
        let mut start = tok.start;
        if idx == 0 && lead.starts_with(':') {
            lead.remove(0);
            start = start.saturating_add(1);
        }
        (start, tok.end, lead, idx)
    } else {
        let before_count = lexed.iter().take_while(|t| t.end <= cursor).count();
        (cursor, cursor, String::new(), before_count)
    };

    let command = lexed.first().map(|t| unquote_token(&input[t.start..t.end]));
    let args_before: Vec<String> = lexed
        .iter()
        .take(before_count)
        .skip(1)
        .map(|t| unquote_token(&input[t.start..t.end]))
        .collect();
    let arg_index = if active == Some(0) || command.is_none() && before_count == 0 {
        0
    } else {
        before_count.max(1)
    };
    TokenizedCommandLine {
        command,
        args_before: args_before.clone(),
        query: CompletionQuery {
            arg_lead,
            tokens: args_before,
            arg_index,
            cmdline: input.to_string(),
            cursor_pos: cursor,
        },
        replace_start,
        replace_end,
    }
}

#[derive(Debug)]
struct LexToken {
    start: usize,
    end: usize,
}

fn lex_command_line(input: &str) -> Vec<LexToken> {
    let mut out = Vec::new();
    let mut start = None;
    let mut quote = None;
    let mut escape = false;
    for (idx, ch) in input.char_indices() {
        if start.is_none() && !ch.is_whitespace() {
            start = Some(idx);
        }
        if escape {
            escape = false;
            continue;
        }
        if ch == '\\' {
            escape = true;
            continue;
        }
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            }
        } else if ch == '\'' || ch == '"' {
            quote = Some(ch);
        } else if ch.is_whitespace() {
            if let Some(s) = start.take() {
                out.push(LexToken { start: s, end: idx });
            }
        }
    }
    if let Some(s) = start {
        out.push(LexToken {
            start: s,
            end: input.len(),
        });
    }
    out
}

fn unquote_token(raw: &str) -> String {
    let mut out = String::new();
    let mut quote = None;
    let mut escape = false;
    for ch in raw.chars() {
        if escape {
            out.push(ch);
            escape = false;
            continue;
        }
        if ch == '\\' {
            escape = true;
            continue;
        }
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            } else {
                out.push(ch);
            }
        } else if ch == '\'' || ch == '"' {
            quote = Some(ch);
        } else {
            out.push(ch);
        }
    }
    if escape {
        out.push('\\');
    }
    out
}

pub struct CompletionEngine;

impl CompletionEngine {
    pub fn complete(
        input: &str,
        cursor: usize,
        registry: &CommandRegistry,
        ctx: &CompletionContext<'_>,
    ) -> CompletionResult {
        let parsed = tokenize_command_line(input, cursor);
        if parsed.query.arg_index == 0 {
            let items = finalize_suggestions(
                complete_commands(&parsed.query.arg_lead, registry),
                &parsed.query.arg_lead,
            );
            return CompletionResult {
                items,
                replace_start: parsed.replace_start,
                replace_end: parsed.replace_end,
            };
        }
        let cmd_lower = parsed
            .command
            .as_deref()
            .unwrap_or("")
            .trim_start_matches(':')
            .to_ascii_lowercase();
        let Some(desc) = registry.find(&cmd_lower) else {
            return CompletionResult {
                items: Vec::new(),
                replace_start: parsed.replace_start,
                replace_end: parsed.replace_end,
            };
        };
        let prev_args = &parsed.args_before;
        let current_token = &parsed.query.arg_lead;

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
                    complete_background(&prev_args, &current_token)
                }
                CompletionProviderKind::Intro => {
                    if prev_args.is_empty() {
                        complete_from_pairs(
                            &current_token,
                            &[
                                ("set", "Choose an intro"),
                                ("replay", "Replay the configured intro"),
                                ("off", "Disable the intro"),
                            ],
                            None,
                        )
                    } else if prev_args.len() == 1 && prev_args[0].eq_ignore_ascii_case("set") {
                        script_completions(crate::animation::AnimSlotKind::Intro)
                    } else {
                        Vec::new()
                    }
                }
                CompletionProviderKind::Canvas => {
                    let p = current_token.to_ascii_lowercase();
                    if prev_args.is_empty() {
                        complete_from_pairs(
                            &p,
                            &ctx.containers
                                .iter()
                                .map(|n| (n.as_str(), "Canvas container"))
                                .collect::<Vec<_>>(),
                            None,
                        )
                    } else if prev_args.len() == 1
                        && ctx
                            .containers
                            .iter()
                            .any(|c| c.eq_ignore_ascii_case(&prev_args[0]))
                    {
                        let mut items = complete_from_pairs(
                            &p,
                            &[
                                ("off", "Disable this canvas"),
                                ("clear", "Clear this canvas"),
                                ("pause", "Pause this canvas"),
                                ("resume", "Resume this canvas"),
                            ],
                            None,
                        );
                        items.extend(script_completions(crate::animation::AnimSlotKind::Canvas));
                        items
                    } else {
                        Vec::new()
                    }
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
                CompletionProviderKind::SetOption => {
                    complete_set_option(&prev_args, &current_token)
                }
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
            replace_start: parsed.replace_start,
            replace_end: parsed.replace_end,
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

/// Deduplicates suggestions and ranks them by subsequence/prefix relevance.
fn finalize_suggestions(items: Vec<CompletionItem>, current_token: &str) -> Vec<CompletionItem> {
    let mut ranked = Vec::<(i32, usize, CompletionItem)>::new();
    let mut seen = std::collections::HashSet::new();
    for (order, item) in items.into_iter().enumerate() {
        let key = item.insert_text.to_ascii_lowercase();
        if !seen.insert(key) {
            continue;
        }
        if !current_token.is_empty() && item.insert_text.eq_ignore_ascii_case(current_token) {
            continue;
        }
        if let Some(score) = fuzzy_score(current_token, &item.insert_text) {
            ranked.push((score, order, item));
        }
    }
    ranked.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    ranked.into_iter().map(|(_, _, item)| item).collect()
}

pub fn fuzzy_score(pattern: &str, candidate: &str) -> Option<i32> {
    let needle: Vec<char> = pattern.to_ascii_lowercase().chars().collect();
    let hay: Vec<char> = candidate.to_ascii_lowercase().chars().collect();
    if needle.is_empty() {
        return Some(0);
    }
    if needle.len() > hay.len() {
        return None;
    }
    if hay.starts_with(&needle) {
        return Some(10_000 - hay.len() as i32);
    }

    let mut score = 0i32;
    let mut next = 0usize;
    let mut previous = None;
    for ch in needle {
        let rel = hay[next..].iter().position(|c| *c == ch)?;
        let idx = next + rel;
        let boundary = idx == 0 || matches!(hay[idx - 1], '_' | '-' | '.' | ' ');
        score += if boundary { 120 } else { 20 };
        if previous.is_some_and(|p| idx == p + 1) {
            score += 90;
        }
        if let Some(prev) = previous {
            score -= (idx.saturating_sub(prev + 1) as i32) * 3;
        }
        previous = Some(idx);
        next = idx + 1;
    }
    Some(score)
}

fn complete_commands(_prefix: &str, registry: &CommandRegistry) -> Vec<CompletionItem> {
    let mut items = Vec::new();
    for cmd in registry.commands() {
        let expected_arg = cmd.args.first().map(|a| a.name.to_string());
        items.push(CompletionItem {
            insert_text: cmd.name.to_string(),
            label: format!(":{}", cmd.name),
            description: cmd.description.to_string(),
            expected_arg,
        });
        for alias in cmd.aliases {
            items.push(CompletionItem {
                insert_text: (*alias).to_string(),
                label: format!(":{alias} → :{}", cmd.name),
                description: format!("Alias for :{} — {}", cmd.name, cmd.description),
                expected_arg: cmd.args.first().map(|a| a.name.to_string()),
            });
        }
    }
    items
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

fn script_completions(slot: crate::animation::AnimSlotKind) -> Vec<CompletionItem> {
    crate::lua::list_scripts_for_slot(slot)
        .into_iter()
        .map(|name| CompletionItem {
            insert_text: name.clone(),
            label: name,
            description: format!("{slot:?} Lua script"),
            expected_arg: None,
        })
        .collect()
}

fn complete_background(prev_args: &[String], current: &str) -> Vec<CompletionItem> {
    if prev_args.is_empty() {
        complete_from_pairs(
            current,
            &[
                ("set", "Set background animation"),
                ("off", "Disable background"),
                ("pause", "Pause background"),
                ("resume", "Resume background"),
                ("list", "List background scripts"),
            ],
            None,
        )
    } else if prev_args.len() == 1 && prev_args[0].eq_ignore_ascii_case("set") {
        script_completions(crate::animation::AnimSlotKind::Background)
    } else {
        Vec::new()
    }
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

    if prev_args.len() == 1 && matches!(prev_args[0].to_ascii_lowercase().as_str(), "load" | "mode")
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
        return complete_from_pairs(&p, BorderChars::STYLE_NAMES, None);
    }

    if prev_args.len() == 1 {
        let first = prev_args[0].to_ascii_lowercase();
        if BorderChars::STYLE_NAMES
            .iter()
            .any(|(style, _)| style.eq_ignore_ascii_case(&first))
        {
            return containers
                .iter()
                .map(|id| CompletionItem {
                    insert_text: id.clone(),
                    label: id.clone(),
                    description: "Container target".into(),
                    expected_arg: None,
                })
                .collect();
        }
        if BorderChars::PROPERTY_SPECS
            .iter()
            .any(|&(k, _)| k != "style" && k == first)
        {
            return complete_from_pairs(&p, suggested_chars_for_property(&first), None);
        }
        if matches!(first.as_str(), "all" | "set" | "style" | "background")
            || containers.iter().any(|c| c.eq_ignore_ascii_case(&first))
        {
            return complete_from_pairs(&p, BorderChars::STYLE_NAMES, None);
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
            .filter(|id| {
                id.to_ascii_lowercase().starts_with(&p) || id.to_ascii_lowercase().contains(&p)
            })
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
                (
                    "border.style",
                    "rounded | plain | double | thick | ascii | none",
                ),
                ("effects", "exclude | allow | isolated"),
                (
                    "animation",
                    "Assign an animation/effect to this container canvas",
                ),
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
                    (
                        "isolated",
                        "Block global effects, allow local container effects",
                    ),
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
                        && !items
                            .iter()
                            .any(|i| i.insert_text.eq_ignore_ascii_case(&name))
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
    let _p = prefix.to_ascii_lowercase();
    pairs
        .iter()
        .map(|&(name, desc)| CompletionItem {
            insert_text: name.to_string(),
            label: name.to_string(),
            description: desc.to_string(),
            expected_arg: expected_arg.map(str::to_string),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_trailing_space_and_quoted_arguments() {
        let q = tokenize_command_line(":background set ", 16);
        assert_eq!(q.query.arg_index, 2);
        assert_eq!(q.query.tokens, ["set"]);
        let q = tokenize_command_line(":canvas \"hello panel\" de", 24);
        assert_eq!(q.command.as_deref(), Some(":canvas"));
        assert_eq!(q.query.arg_index, 2);
        assert_eq!(q.query.tokens, ["hello panel"]);
    }

    #[test]
    fn fuzzy_score_prefers_prefix_and_matches_word_initials() {
        assert!(
            fuzzy_score("mat", "matrix_rain").unwrap() > fuzzy_score("mt", "matrix_rain").unwrap()
        );
        assert!(fuzzy_score("mt", "matrix_rain").is_some());
        assert!(fuzzy_score("mr", "matrix_rain").is_some());
        assert!(fuzzy_score("zz", "matrix_rain").is_none());
    }

    #[test]
    fn completion_positions_are_contextual() {
        let registry = CommandRegistry::new();
        let catalog = AnimationCatalog {
            entries: Vec::new(),
        };
        let ctx = CompletionContext {
            themes: &[],
            animations: &catalog,
            layouts: &[],
            containers: &["system".into()],
        };
        let border = CompletionEngine::complete(":border ", 8, &registry, &ctx);
        assert!(border.items.iter().any(|i| i.insert_text == "double"));
        assert!(!border.items.iter().any(|i| i.insert_text == "system"));
        let bg = CompletionEngine::complete(":background set ", 17, &registry, &ctx);
        assert!(!bg.items.iter().any(|i| {
            ["crt_power_on", "cursor_expand", "christmas_tree"].contains(&i.insert_text.as_str())
        }));
        let intro = CompletionEngine::complete(":intro set ", 11, &registry, &ctx);
        assert!(intro.items.iter().any(|i| i.insert_text == "crt_power_on"));
        assert!(
            !intro
                .items
                .iter()
                .any(|i| i.insert_text == "christmas_tree")
        );
        let canvas = CompletionEngine::complete(":canvas ", 8, &registry, &ctx);
        assert_eq!(
            canvas
                .items
                .iter()
                .map(|i| i.insert_text.as_str())
                .collect::<Vec<_>>(),
            ["system"]
        );
    }
}
