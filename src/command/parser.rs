use crate::config::{Anchor, BorderStyle, EffectKind, LayoutMode, ViewMode};
use crate::ui::borders::parse_border_style;
use crate::window::WindowBackgroundMode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowCommand {
    OpenInspector,
    Center,
    SetAnchor(Anchor),
    SetDraggable(Option<bool>),
    SetResizable(Option<bool>),
    ToggleBorder(Option<bool>),
    SetBorderStyle(BorderStyle),
    SetBackground(WindowBackgroundMode),
    SetPadding(u16),
    SetVisible(Option<bool>),
    Reset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationAction {
    OpenPicker,
    Play,
    SetStartup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutSelection {
    Mode(LayoutMode),
    View(ViewMode),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteTarget {
    All,
    Config,
    Window,
    Theme,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedCommand {
    Colorscheme(Option<String>),
    Reload,
    Write(WriteTarget),
    Animation {
        action: AnimationAction,
        name: Option<String>,
    },
    Effect(Option<EffectKind>),
    Window(WindowCommand),
    Layout(Option<LayoutSelection>),
    Set {
        key: String,
        value: String,
    },
    Help,
    Quit,
}

pub fn parse_command(raw: &str) -> Result<ParsedCommand, String> {
    let trimmed = raw.trim().trim_start_matches(':').trim();
    if trimmed.is_empty() {
        return Err("Empty command".to_string());
    }

    let mut tokens = trimmed.split_whitespace();
    let head = tokens.next().unwrap_or("").to_ascii_lowercase();
    let rest: Vec<&str> = tokens.collect();

    match head.as_str() {
        "q" | "quit" => Ok(ParsedCommand::Quit),
        "h" | "help" => Ok(ParsedCommand::Help),
        "r" | "reload" => Ok(ParsedCommand::Reload),
        "w" | "write" => {
            let target = match rest.first().map(|s| s.to_ascii_lowercase()).as_deref() {
                None | Some("all") => WriteTarget::All,
                Some("config") => WriteTarget::Config,
                Some("window") => WriteTarget::Window,
                Some("theme") => WriteTarget::Theme,
                Some(other) => {
                    return Err(format!(
                        "Unknown write target '{other}' (expected all, config, window, or theme)"
                    ));
                }
            };
            Ok(ParsedCommand::Write(target))
        }
        "colorscheme" | "theme" => Ok(ParsedCommand::Colorscheme(
            rest.first().map(|s| (*s).to_string()),
        )),
        "animation" | "anim" => parse_animation_command(&rest),
        "effect" | "fx" => {
            let Some(arg) = rest.first() else {
                return Ok(ParsedCommand::Effect(None));
            };
            let kind = match arg.to_ascii_lowercase().as_str() {
                "matrix" => EffectKind::Matrix,
                "water" => EffectKind::Water,
                "particles" => EffectKind::Particles,
                "scanlines" => EffectKind::Scanlines,
                "none" | "off" => EffectKind::None,
                other => return Err(format!("Unknown effect '{other}'")),
            };
            Ok(ParsedCommand::Effect(Some(kind)))
        }
        "layout" => parse_layout_command(&rest),
        "set" => parse_set_command(&rest),
        "window" | "win" => parse_window_command(&rest),
        other => Err(format!("Unknown command ':{other}' (type :help for usage)")),
    }
}

fn parse_animation_command(args: &[&str]) -> Result<ParsedCommand, String> {
    let Some(first) = args.first() else {
        return Ok(ParsedCommand::Animation {
            action: AnimationAction::OpenPicker,
            name: None,
        });
    };

    match first.to_ascii_lowercase().as_str() {
        "list" => Ok(ParsedCommand::Animation {
            action: AnimationAction::OpenPicker,
            name: None,
        }),
        "play" => {
            let Some(name) = args.get(1) else {
                return Err("Usage: :animation play <name>".to_string());
            };
            Ok(ParsedCommand::Animation {
                action: AnimationAction::Play,
                name: Some((*name).to_string()),
            })
        }
        "set" => {
            let Some(name) = args.get(1) else {
                return Err("Usage: :animation set <name>".to_string());
            };
            Ok(ParsedCommand::Animation {
                action: AnimationAction::SetStartup,
                name: Some((*name).to_string()),
            })
        }
        name => Ok(ParsedCommand::Animation {
            action: AnimationAction::SetStartup,
            name: Some(name.to_string()),
        }),
    }
}

fn parse_layout_command(args: &[&str]) -> Result<ParsedCommand, String> {
    let Some(first) = args.first() else {
        return Ok(ParsedCommand::Layout(None));
    };

    let lower_first = first.to_ascii_lowercase();
    let target = match lower_first.as_str() {
        "load" | "mode" => match args.get(1) {
            Some(arg) => *arg,
            None => return Err("Usage: :layout load <name>".to_string()),
        },
        _ => *first,
    };

    let sel = match target.to_ascii_lowercase().as_str() {
        "responsive" => LayoutSelection::Mode(LayoutMode::Responsive),
        "free" => LayoutSelection::Mode(LayoutMode::Free),
        "compact" => LayoutSelection::View(ViewMode::Compact),
        "minimal" => LayoutSelection::View(ViewMode::Minimal),
        "dashboard" => LayoutSelection::View(ViewMode::Dashboard),
        other => return Err(format!("Unknown layout '{other}'")),
    };
    Ok(ParsedCommand::Layout(Some(sel)))
}

fn parse_set_command(args: &[&str]) -> Result<ParsedCommand, String> {
    if args.len() < 2 {
        return Err("Usage: :set <property> <value>".to_string());
    }
    let key = args[0].to_string();
    let value = args[1..].join(" ");
    Ok(ParsedCommand::Set { key, value })
}

fn parse_window_command(args: &[&str]) -> Result<ParsedCommand, String> {
    let Some(sub) = args.first().map(|s| s.to_ascii_lowercase()) else {
        return Ok(ParsedCommand::Window(WindowCommand::OpenInspector));
    };

    let cmd = match sub.as_str() {
        "center" => WindowCommand::Center,
        "reset" => WindowCommand::Reset,
        "show" => WindowCommand::SetVisible(Some(true)),
        "hide" => WindowCommand::SetVisible(Some(false)),
        "toggle" => WindowCommand::SetVisible(None),
        "drag" => WindowCommand::SetDraggable(parse_toggle_arg(args.get(1).copied())?),
        "resize" => WindowCommand::SetResizable(parse_toggle_arg(args.get(1).copied())?),
        "padding" => {
            let val_str = args
                .get(1)
                .ok_or_else(|| "Usage: :window padding <0..10>".to_string())?;
            let val: u16 = val_str
                .parse()
                .map_err(|_| format!("Invalid padding value '{val_str}'"))?;
            WindowCommand::SetPadding(val.min(10))
        }
        "anchor" => {
            let a_str = args
                .get(1)
                .ok_or_else(|| "Usage: :window anchor <top-left|center|bottom-right|...>".to_string())?;
            WindowCommand::SetAnchor(parse_anchor(a_str)?)
        }
        "border" => {
            let Some(arg) = args.get(1).map(|s| s.to_ascii_lowercase()) else {
                return Ok(ParsedCommand::Window(WindowCommand::ToggleBorder(None)));
            };
            match arg.as_str() {
                "on" | "true" => WindowCommand::ToggleBorder(Some(true)),
                "off" | "false" => WindowCommand::ToggleBorder(Some(false)),
                other => WindowCommand::SetBorderStyle(parse_border_style(other)?),
            }
        }
        "bg" | "background" => {
            let Some(arg) = args.get(1).map(|s| s.to_ascii_lowercase()) else {
                return Err("Usage: :window bg <solid|transparent>".to_string());
            };
            match arg.as_str() {
                "solid" => WindowCommand::SetBackground(WindowBackgroundMode::Solid),
                "transparent" | "trans" => {
                    WindowCommand::SetBackground(WindowBackgroundMode::Transparent)
                }
                other => return Err(format!("Invalid background mode '{other}'")),
            }
        }
        other => return Err(format!("Unknown :window subcommand '{other}'")),
    };

    Ok(ParsedCommand::Window(cmd))
}

fn parse_toggle_arg(arg: Option<&str>) -> Result<Option<bool>, String> {
    match arg.map(|s| s.to_ascii_lowercase()).as_deref() {
        None | Some("toggle") => Ok(None),
        Some("on") | Some("true") => Ok(Some(true)),
        Some("off") | Some("false") => Ok(Some(false)),
        Some(other) => Err(format!("Invalid toggle value '{other}' (expected on or off)")),
    }
}

fn parse_anchor(raw: &str) -> Result<Anchor, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "top-left" => Ok(Anchor::TopLeft),
        "top-center" => Ok(Anchor::TopCenter),
        "top-right" => Ok(Anchor::TopRight),
        "center-left" => Ok(Anchor::CenterLeft),
        "center" => Ok(Anchor::Center),
        "center-right" => Ok(Anchor::CenterRight),
        "bottom-left" => Ok(Anchor::BottomLeft),
        "bottom-center" => Ok(Anchor::BottomCenter),
        "bottom-right" => Ok(Anchor::BottomRight),
        other => Err(format!("Invalid anchor '{other}'")),
    }
}
