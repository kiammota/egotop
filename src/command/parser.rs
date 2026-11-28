use crate::config::schema::is_valid_color_spec;
use crate::config::{BorderStyle, ViewMode};
use crate::ui::borders::parse_border_style;
use crate::ui::container::{
    BackgroundFill, ComponentKind, EffectPolicy, GradientDirection,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContainerProperty {
    Background(BackgroundFill),
    Foreground(Option<String>),
    BorderColor(Option<String>),
    BorderStyle(BorderStyle),
    Effects(EffectPolicy),
    Animation(Option<String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContainerCommand {
    List,
    Inspect(String),
    Add {
        id: String,
        kind: Option<ComponentKind>,
    },
    Remove(String),
    Set {
        id: String,
        property: ContainerProperty,
    },
    Gradient {
        id: String,
        direction: GradientDirection,
        colors: Vec<String>,
    },
    Reset(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationAction {
    OpenPicker,
    Play,
    SetStartup,
    Pause,
    Resume,
    Stop,
    Restart,
    Speed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackgroundCommand {
    Set(String),
    Add(String),
    Remove(String),
    Toggle(Option<bool>),
    Stop,
    Pause,
    Resume,
    Speed(String),
    List,
}

/// Commands for the `:intro` slot (entry/startup animations).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntroCommand {
    /// Set and play the intro animation by name.
    Play(String),
    /// Re-play the current intro animation from the beginning.
    Replay,
    /// Disable/skip the intro animation.
    Off,
    /// Set the startup intro without playing it now.
    Set(String),
}

/// Commands for the `:canvas` slot (per-container animations).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanvasCommand {
    /// Set a canvas animation on a specific container.
    Set { container_id: String, script: String },
    /// Disable the canvas animation on a container.
    Off { container_id: String },
    /// Pause the canvas animation on a container.
    Pause { container_id: String },
    /// Resume the canvas animation on a container.
    Resume { container_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BorderCommand {
    GlobalStyle(BorderStyle),
    TargetStyle { id: String, style: BorderStyle },
    TargetColor { id: String, color: Option<String> },
    CharProp { prop: String, value: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutSelection {
    View(ViewMode),
    Preset(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteTarget {
    All,
    Config,
    Theme,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedCommand {
    Colorscheme(Option<String>),
    Reload,
    Messages,
    Errors,
    Write(WriteTarget),
    Animation {
        action: AnimationAction,
        name: Option<String>,
    },
    Background(BackgroundCommand),
    Intro(IntroCommand),
    Canvas(CanvasCommand),
    Border(BorderCommand),
    Layout(Option<LayoutSelection>),
    Container(ContainerCommand),
    Set {
        key: String,
        value: String,
    },
    Help,
    Check,
    Restore,
    WriteQuit,
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
        "wq" => Ok(ParsedCommand::WriteQuit),
        "h" | "help" => Ok(ParsedCommand::Help),
        "r" | "reload" => Ok(ParsedCommand::Reload),
        "check" => Ok(ParsedCommand::Check),
        "restore" => Ok(ParsedCommand::Restore),
        "msg" | "messages" => Ok(ParsedCommand::Messages),
        "err" | "errors" => Ok(ParsedCommand::Errors),
        "w" | "write" => {
            let target = match rest.first().map(|s| s.to_ascii_lowercase()).as_deref() {
                None | Some("all") => WriteTarget::All,
                Some("config") => WriteTarget::Config,
                Some("theme") => WriteTarget::Theme,
                Some(other) => {
                    return Err(format!(
                        "Unknown write target '{other}' (expected all, config, or theme)"
                    ));
                }
            };
            Ok(ParsedCommand::Write(target))
        }
        "colorscheme" | "theme" => Ok(ParsedCommand::Colorscheme(
            rest.first().map(|s| (*s).to_string()),
        )),
        "animation" | "anim" => parse_animation_command(&rest),
        "background" | "bg" => parse_background_command(&rest),
        "intro" => parse_intro_command(&rest),
        "canvas" => parse_canvas_command(&rest),
        "border" | "borders" => parse_border_command(&rest),
        "layout" => parse_layout_command(&rest),
        "container" => parse_container_command(&rest),
        "set" => parse_set_command(&rest),
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
        "pause" => Ok(ParsedCommand::Animation {
            action: AnimationAction::Pause,
            name: None,
        }),
        "resume" => Ok(ParsedCommand::Animation {
            action: AnimationAction::Resume,
            name: None,
        }),
        "stop" => Ok(ParsedCommand::Animation {
            action: AnimationAction::Stop,
            name: None,
        }),
        "restart" | "replay" => Ok(ParsedCommand::Animation {
            action: AnimationAction::Restart,
            name: None,
        }),
        "speed" => {
            let Some(val) = args.get(1) else {
                return Err("Usage: :animation speed <0.1..10.0>".to_string());
            };
            let parsed: f32 = val
                .parse()
                .map_err(|_| format!("Invalid animation speed '{val}'"))?;
            if !(0.05..=10.0).contains(&parsed) {
                return Err("Animation speed must be between 0.05 and 10.0".to_string());
            }
            Ok(ParsedCommand::Animation {
                action: AnimationAction::Speed,
                name: Some((*val).to_string()),
            })
        }
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

fn parse_border_command(args: &[&str]) -> Result<ParsedCommand, String> {
    let Some(first) = args.first() else {
        return Err(
            "Usage: :border <style> | :border <canvas_id> <style> | :border <canvas_id> color <color>"
                .to_string(),
        );
    };
    let low = first.trim().to_ascii_lowercase();

    // `:border <style>`
    if args.len() == 1 {
        return Ok(ParsedCommand::Border(BorderCommand::GlobalStyle(
            parse_border_style(first)?,
        )));
    }

    // `:border all|set|style <style>`
    if matches!(low.as_str(), "all" | "set" | "style") {
        let style_arg = args
            .get(1)
            .ok_or_else(|| "Usage: :border style <rounded|plain|double|thick|ascii|none>".to_string())?;
        return Ok(ParsedCommand::Border(BorderCommand::GlobalStyle(
            parse_border_style(style_arg)?,
        )));
    }

    // `:border <char_prop> <char>` (e.g. `:border horizontal ═`)
    if args.len() == 2
        && crate::ui::borders::BorderChars::PROPERTY_SPECS
            .iter()
            .any(|&(k, _)| k != "style" && k == low.as_str())
    {
        return Ok(ParsedCommand::Border(BorderCommand::CharProp {
            prop: low,
            value: args[1].to_string(),
        }));
    }

    // `:border <style> <target>` keeps the first completion position reserved
    // for styles and the second reserved for container IDs.
    if args.len() == 2
        && crate::ui::borders::BorderChars::STYLE_NAMES
            .iter()
            .any(|(style, _)| style.eq_ignore_ascii_case(first))
    {
        return Ok(ParsedCommand::Border(BorderCommand::TargetStyle {
            id: args[1].to_string(),
            style: parse_border_style(first)?,
        }));
    }

    // `:border <target> color <color|inherit|none>`
    if args.get(1).is_some_and(|s| s.eq_ignore_ascii_case("color")) {
        let color_arg = args
            .get(2)
            .ok_or_else(|| "Usage: :border <canvas_id> color <color|inherit|none>".to_string())?;
        let col_low = color_arg.trim().to_ascii_lowercase();
        let color = if matches!(col_low.as_str(), "inherit" | "none" | "reset") {
            None
        } else {
            if !is_valid_color_spec(color_arg) {
                return Err(format!("Invalid border color '{color_arg}'"));
            }
            Some((*color_arg).to_string())
        };
        return Ok(ParsedCommand::Border(BorderCommand::TargetColor {
            id: (*first).to_string(),
            color,
        }));
    }

    // `:border <target> <style>`
    let second = args[1];
    Ok(ParsedCommand::Border(BorderCommand::TargetStyle {
        id: (*first).to_string(),
        style: parse_border_style(second)?,
    }))
}

fn parse_background_command(args: &[&str]) -> Result<ParsedCommand, String> {
    let Some(first) = args.first() else {
        return Ok(ParsedCommand::Background(BackgroundCommand::List));
    };
    let cmd = match first.to_ascii_lowercase().as_str() {
        "list" => BackgroundCommand::List,
        "stop" | "clear" | "off" | "none" => BackgroundCommand::Stop,
        "pause" => BackgroundCommand::Pause,
        "resume" => BackgroundCommand::Resume,
        "toggle" => {
            let flag = args.get(1).and_then(|v| match v.to_ascii_lowercase().as_str() {
                "on" | "true" | "1" => Some(true),
                "off" | "false" | "0" => Some(false),
                _ => None,
            });
            BackgroundCommand::Toggle(flag)
        }
        "remove" | "rm" | "del" => {
            let Some(target) = args.get(1) else {
                return Err("Usage: :background remove <index|name>".to_string());
            };
            BackgroundCommand::Remove((*target).to_string())
        }
        "set" => {
            let Some(name) = args.get(1) else {
                return Err("Usage: :background set <name>".to_string());
            };
            BackgroundCommand::Set((*name).to_string())
        }
        "add" | "layer" => {
            let Some(name) = args.get(1) else {
                return Err("Usage: :background add <name>".to_string());
            };
            BackgroundCommand::Add((*name).to_string())
        }
        "speed" => {
            let Some(val) = args.get(1) else {
                return Err("Usage: :background speed <0.1..10.0>".to_string());
            };
            let parsed: f32 = val
                .parse()
                .map_err(|_| format!("Invalid background speed '{val}'"))?;
            if !(0.05..=10.0).contains(&parsed) {
                return Err("Background speed must be between 0.05 and 10.0".to_string());
            }
            BackgroundCommand::Speed((*val).to_string())
        }
        other => BackgroundCommand::Set(other.to_string()),
    };
    Ok(ParsedCommand::Background(cmd))
}

fn parse_intro_command(args: &[&str]) -> Result<ParsedCommand, String> {
    let Some(first) = args.first().map(|s| s.to_ascii_lowercase()) else {
        return Err("Usage: :intro <name|replay|off>".to_string());
    };

    let cmd = match first.as_str() {
        "replay" | "restart" => IntroCommand::Replay,
        "off" | "none" | "disable" => IntroCommand::Off,
        "set" => {
            let Some(name) = args.get(1) else {
                return Err("Usage: :intro set <name>".to_string());
            };
            IntroCommand::Set((*name).to_string())
        }
        other => IntroCommand::Play(other.to_string()),
    };

    Ok(ParsedCommand::Intro(cmd))
}

fn parse_canvas_command(args: &[&str]) -> Result<ParsedCommand, String> {
    let Some(container_id) = args.first() else {
        return Err("Usage: :canvas <container_id> <name|off|pause|resume>".to_string());
    };
    let container_id = container_id.to_string();

    let Some(sub) = args.get(1).map(|s| s.to_ascii_lowercase()) else {
        return Err(format!(
            "Usage: :canvas {container_id} <name|off|pause|resume>"
        ));
    };

    let cmd = match sub.as_str() {
        "off" | "none" | "disable" => CanvasCommand::Off { container_id },
        "pause" => CanvasCommand::Pause { container_id },
        "resume" => CanvasCommand::Resume { container_id },
        script => CanvasCommand::Set {
            container_id,
            script: script.to_string(),
        },
    };

    Ok(ParsedCommand::Canvas(cmd))
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
        "compact" => LayoutSelection::View(ViewMode::Compact),
        "minimal" => LayoutSelection::View(ViewMode::Minimal),
        "dashboard" => LayoutSelection::View(ViewMode::Dashboard),
        other => LayoutSelection::Preset(other.to_string()),
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

fn parse_container_command(args: &[&str]) -> Result<ParsedCommand, String> {
    let Some(sub) = args.first().map(|s| s.to_ascii_lowercase()) else {
        return Ok(ParsedCommand::Container(ContainerCommand::List));
    };

    match sub.as_str() {
        "list" => Ok(ParsedCommand::Container(ContainerCommand::List)),
        "inspect" => {
            let id = args
                .get(1)
                .ok_or_else(|| "Usage: :container inspect <id>".to_string())?;
            Ok(ParsedCommand::Container(ContainerCommand::Inspect(
                (*id).to_string(),
            )))
        }
        "add" => {
            let id = args
                .get(1)
                .ok_or_else(|| "Usage: :container add <id> [type]".to_string())?;
            let kind = match args.get(2) {
                Some(raw_kind) => Some(parse_component_kind(raw_kind)?),
                None => None,
            };
            Ok(ParsedCommand::Container(ContainerCommand::Add {
                id: (*id).to_string(),
                kind,
            }))
        }
        "remove" | "rm" | "del" => {
            let id = args
                .get(1)
                .ok_or_else(|| "Usage: :container remove <id>".to_string())?;
            Ok(ParsedCommand::Container(ContainerCommand::Remove(
                (*id).to_string(),
            )))
        }
        "reset" => {
            let id = args
                .get(1)
                .ok_or_else(|| "Usage: :container reset <id>".to_string())?;
            Ok(ParsedCommand::Container(ContainerCommand::Reset(
                (*id).to_string(),
            )))
        }
        "set" => {
            let id = args
                .get(1)
                .ok_or_else(|| "Usage: :container set <id> <property> <value>".to_string())?;
            let prop = args
                .get(2)
                .ok_or_else(|| "Usage: :container set <id> <property> <value>".to_string())?;
            let val = args
                .get(3)
                .ok_or_else(|| "Usage: :container set <id> <property> <value>".to_string())?;

            let property = match prop.to_ascii_lowercase().as_str() {
                "background" | "bg" => {
                    let low = val.to_ascii_lowercase();
                    let fill = match low.as_str() {
                        "inherit" => BackgroundFill::Inherit,
                        "transparent" | "reset" => BackgroundFill::Transparent,
                        _ => {
                            if !is_valid_color_spec(val) {
                                return Err(format!("Invalid background color '{val}'"));
                            }
                            BackgroundFill::Solid((*val).to_string())
                        }
                    };
                    ContainerProperty::Background(fill)
                }
                "foreground" | "fg" => {
                    let low = val.to_ascii_lowercase();
                    if low == "inherit" || low == "none" {
                        ContainerProperty::Foreground(None)
                    } else {
                        if !is_valid_color_spec(val) {
                            return Err(format!("Invalid foreground color '{val}'"));
                        }
                        ContainerProperty::Foreground(Some((*val).to_string()))
                    }
                }
                "border.color" => {
                    let low = val.to_ascii_lowercase();
                    if low == "inherit" || low == "none" {
                        ContainerProperty::BorderColor(None)
                    } else {
                        if !is_valid_color_spec(val) {
                            return Err(format!("Invalid border color '{val}'"));
                        }
                        ContainerProperty::BorderColor(Some((*val).to_string()))
                    }
                }
                "border.style" => {
                    ContainerProperty::BorderStyle(parse_border_style(val)?)
                }
                "effects" => {
                    let policy = match val.to_ascii_lowercase().as_str() {
                        "exclude" => EffectPolicy::Exclude,
                        "allow" => EffectPolicy::Allow,
                        "isolated" => EffectPolicy::Isolated,
                        other => {
                            return Err(format!(
                                "Invalid effect policy '{other}' (expected exclude, allow, or isolated)"
                            ));
                        }
                    };
                    ContainerProperty::Effects(policy)
                }
                "animation" | "anim" | "script" => {
                    let low = val.trim().to_ascii_lowercase();
                    if matches!(low.as_str(), "none" | "off" | "clear") {
                        ContainerProperty::Animation(None)
                    } else {
                        ContainerProperty::Animation(Some((*val).to_string()))
                    }
                }
                other => {
                    return Err(format!(
                        "Unknown container property '{other}' (expected background, foreground, border.color, border.style, effects, or animation)"
                    ));
                }
            };

            Ok(ParsedCommand::Container(ContainerCommand::Set {
                id: (*id).to_string(),
                property,
            }))
        }
        "gradient" => {
            let id = args
                .get(1)
                .ok_or_else(|| {
                    "Usage: :container gradient <id> <direction> <color1> <color2> [color...]"
                        .to_string()
                })?;
            let dir_str = args
                .get(2)
                .ok_or_else(|| {
                    "Usage: :container gradient <id> <horizontal|vertical> <color1> <color2> [color...]"
                        .to_string()
                })?;
            let direction = match dir_str.to_ascii_lowercase().as_str() {
                "horizontal" | "h" => GradientDirection::Horizontal,
                "vertical" | "v" => GradientDirection::Vertical,
                other => {
                    return Err(format!(
                        "Invalid gradient direction '{other}' (expected horizontal or vertical)"
                    ));
                }
            };
            let color_args = &args[3..];
            if color_args.len() < 2 {
                return Err("Gradient requires at least 2 colors".to_string());
            }
            let mut colors = Vec::with_capacity(color_args.len());
            for c in color_args {
                if !is_valid_color_spec(c) {
                    return Err(format!("Invalid gradient color '{c}'"));
                }
                colors.push((*c).to_string());
            }
            Ok(ParsedCommand::Container(ContainerCommand::Gradient {
                id: (*id).to_string(),
                direction,
                colors,
            }))
        }
        other => Err(format!(
            "Unknown :container subcommand '{other}' (expected list, inspect, add, remove, set, gradient, or reset)"
        )),
    }
}

pub fn parse_component_kind(raw: &str) -> Result<ComponentKind, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "text" => Ok(ComponentKind::Text),
        "ascii-logo" | "logo" => Ok(ComponentKind::AsciiLogo),
        "separator" => Ok(ComponentKind::Separator),
        "greeting" => Ok(ComponentKind::Greeting),
        "cpu" => Ok(ComponentKind::Cpu),
        "memory" | "ram" => Ok(ComponentKind::Memory),
        "temperature" | "temp" => Ok(ComponentKind::Temperature),
        "processes" | "procs" => Ok(ComponentKind::Processes),
        "network" | "net" => Ok(ComponentKind::Network),
        "uptime" => Ok(ComponentKind::Uptime),
        "graph" => Ok(ComponentKind::Graph),
        "system" => Ok(ComponentKind::System),
        "hardware" => Ok(ComponentKind::Hardware),
        "environment" | "env" => Ok(ComponentKind::Environment),
        "terminal" | "term" => Ok(ComponentKind::Terminal),
        "storage" | "disk" => Ok(ComponentKind::Storage),
        "activity" => Ok(ComponentKind::Activity),
        "session" => Ok(ComponentKind::Session),
        "header" => Ok(ComponentKind::Header),
        "spinner" => Ok(ComponentKind::Spinner),
        "typewriter" => Ok(ComponentKind::Typewriter),
        "matrix" => Ok(ComponentKind::Matrix),
        "particles" => Ok(ComponentKind::Particles),
        "frames" => Ok(ComponentKind::Frames),
        "ascii-object" => Ok(ComponentKind::AsciiObject),
        other => Err(format!("Unknown component type '{other}'")),
    }
}
