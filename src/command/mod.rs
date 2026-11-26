use crate::config::{Anchor, BorderStyle, EffectKind, LayoutMode, ViewMode};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedCommand {
    Colorscheme(Option<String>),
    Reload,
    Animation(Option<String>),
    Effect(Option<EffectKind>),
    Window(WindowCommand),
    Layout(Option<LayoutSelection>),
    Help,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutSelection {
    Mode(LayoutMode),
    View(ViewMode),
}

#[derive(Debug, Clone, Default)]
pub struct CommandLineState {
    pub active: bool,
    pub buffer: String,
    pub history: Vec<String>,
    pub history_index: Option<usize>,
}

impl CommandLineState {
    pub fn open(&mut self) {
        self.active = true;
        self.buffer.clear();
        self.history_index = None;
    }

    pub fn cancel(&mut self) {
        self.active = false;
        self.buffer.clear();
        self.history_index = None;
    }

    pub fn push_char(&mut self, c: char) {
        self.buffer.push(c);
        self.history_index = None;
    }

    pub fn backspace(&mut self) {
        self.buffer.pop();
        self.history_index = None;
    }

    pub fn submit(&mut self) -> String {
        let cmd = self.buffer.trim().to_string();
        if !cmd.is_empty() && self.history.last().map(String::as_str) != Some(&cmd) {
            self.history.push(cmd.clone());
        }
        self.active = false;
        self.buffer.clear();
        self.history_index = None;
        cmd
    }

    pub fn history_prev(&mut self) {
        if self.history.is_empty() {
            return;
        }
        let next_idx = match self.history_index {
            None => self.history.len() - 1,
            Some(0) => 0,
            Some(i) => i - 1,
        };
        self.history_index = Some(next_idx);
        self.buffer = self.history[next_idx].clone();
    }

    pub fn history_next(&mut self) {
        let Some(idx) = self.history_index else {
            return;
        };
        if idx + 1 >= self.history.len() {
            self.history_index = None;
            self.buffer.clear();
        } else {
            self.history_index = Some(idx + 1);
            self.buffer = self.history[idx + 1].clone();
        }
    }

    pub fn autocomplete(&mut self, theme_names: &[String], anim_names: &[String]) {
        let trimmed = self.buffer.trim_start();
        let commands = [
            "colorscheme",
            "reload",
            "animation",
            "window",
            "layout",
            "effect",
            "help",
            "q",
        ];

        if !trimmed.contains(' ') {
            if let Some(found) = commands.iter().find(|c| c.starts_with(trimmed)) {
                self.buffer = format!("{found} ");
            }
            return;
        }

        let mut parts = trimmed.splitn(2, char::is_whitespace);
        let cmd = parts.next().unwrap_or("");
        let arg = parts.next().unwrap_or("").trim_start();

        let candidates: Vec<String> = match cmd {
            "colorscheme" | "theme" => theme_names.to_vec(),
            "animation" | "anim" => anim_names.to_vec(),
            "effect" => ["matrix", "water", "particles", "scanlines", "none"]
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
            "layout" => ["responsive", "free", "compact", "minimal", "dashboard"]
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
            "window" => [
                "center",
                "anchor",
                "border",
                "bg",
                "drag",
                "resize",
                "padding",
                "toggle",
                "reset",
            ]
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
            _ => Vec::new(),
        };

        if let Some(found) = candidates
            .into_iter()
            .find(|c| c.to_ascii_lowercase().starts_with(&arg.to_ascii_lowercase()))
        {
            self.buffer = format!("{cmd} {found}");
        }
    }
}

pub fn parse_command(raw: &str) -> Result<ParsedCommand, String> {
    let trimmed = raw.trim().trim_start_matches(':').trim();
    if trimmed.is_empty() {
        return Err("Comando vazio".to_string());
    }

    let mut tokens = trimmed.split_whitespace();
    let head = tokens.next().unwrap_or("").to_ascii_lowercase();
    let rest: Vec<&str> = tokens.collect();

    match head.as_str() {
        "q" | "quit" | "exit" => Ok(ParsedCommand::Quit),
        "h" | "help" => Ok(ParsedCommand::Help),
        "r" | "reload" => Ok(ParsedCommand::Reload),
        "colorscheme" | "theme" => Ok(ParsedCommand::Colorscheme(
            rest.first().map(|s| (*s).to_string()),
        )),
        "animation" | "anim" => Ok(ParsedCommand::Animation(
            rest.first().map(|s| (*s).to_string()),
        )),
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
                other => return Err(format!("Efeito desconhecido: '{other}'")),
            };
            Ok(ParsedCommand::Effect(Some(kind)))
        }
        "layout" => {
            let Some(arg) = rest.first() else {
                return Ok(ParsedCommand::Layout(None));
            };
            let sel = match arg.to_ascii_lowercase().as_str() {
                "responsive" => LayoutSelection::Mode(LayoutMode::Responsive),
                "free" => LayoutSelection::Mode(LayoutMode::Free),
                "compact" => LayoutSelection::View(ViewMode::Compact),
                "minimal" => LayoutSelection::View(ViewMode::Minimal),
                "dashboard" => LayoutSelection::View(ViewMode::Dashboard),
                other => return Err(format!("Layout desconhecido: '{other}'")),
            };
            Ok(ParsedCommand::Layout(Some(sel)))
        }
        "window" | "win" => parse_window_command(&rest),
        other => Err(format!("Comando desconhecido: ':{other}' (use :help)")),
    }
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
                .ok_or_else(|| "Uso: :window padding <0..10>".to_string())?;
            let val: u16 = val_str
                .parse()
                .map_err(|_| format!("Padding inválido: '{val_str}'"))?;
            WindowCommand::SetPadding(val.min(10))
        }
        "anchor" => {
            let a_str = args
                .get(1)
                .ok_or_else(|| "Uso: :window anchor <top-left|center|bottom-right|...>".to_string())?;
            WindowCommand::SetAnchor(parse_anchor(a_str)?)
        }
        "border" => {
            let Some(arg) = args.get(1).map(|s| s.to_ascii_lowercase()) else {
                return Ok(ParsedCommand::Window(WindowCommand::ToggleBorder(None)));
            };
            match arg.as_str() {
                "on" | "true" => WindowCommand::ToggleBorder(Some(true)),
                "off" | "false" => WindowCommand::ToggleBorder(Some(false)),
                "rounded" => WindowCommand::SetBorderStyle(BorderStyle::Rounded),
                "solid" | "plain" => WindowCommand::SetBorderStyle(BorderStyle::Plain),
                "double" => WindowCommand::SetBorderStyle(BorderStyle::Double),
                "thick" => WindowCommand::SetBorderStyle(BorderStyle::Thick),
                other => return Err(format!("Estilo de borda inválido: '{other}'")),
            }
        }
        "bg" | "background" => {
            let Some(arg) = args.get(1).map(|s| s.to_ascii_lowercase()) else {
                return Err("Uso: :window bg <solid|transparent>".to_string());
            };
            match arg.as_str() {
                "solid" => WindowCommand::SetBackground(WindowBackgroundMode::Solid),
                "transparent" | "trans" => {
                    WindowCommand::SetBackground(WindowBackgroundMode::Transparent)
                }
                other => return Err(format!("Modo de fundo inválido: '{other}'")),
            }
        }
        other => return Err(format!("Subcomando :window inválido: '{other}'")),
    };

    Ok(ParsedCommand::Window(cmd))
}

fn parse_toggle_arg(arg: Option<&str>) -> Result<Option<bool>, String> {
    match arg.map(|s| s.to_ascii_lowercase()).as_deref() {
        None | Some("toggle") => Ok(None),
        Some("on") | Some("true") => Ok(Some(true)),
        Some("off") | Some("false") => Ok(Some(false)),
        Some(other) => Err(format!("Valor inválido '{other}' (use on|off)")),
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
        other => Err(format!("Âncora inválida: '{other}'")),
    }
}
