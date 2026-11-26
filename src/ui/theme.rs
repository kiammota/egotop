use crate::config::{BorderStyle, ThemeConfig};
use crate::ui::borders::BorderChars;
use ratatui::style::{Color, Style};
use ratatui::widgets::BorderType;

#[derive(Debug, Clone)]
pub struct Theme {
    pub name: String,
    pub background: Color,
    pub panel_background: Color,
    pub primary: Color,
    pub secondary: Color,
    pub accent: Color,
    pub text: Color,
    pub muted: Color,
    pub border: Color,
    pub danger: Color,
    pub border_type: BorderType,
    pub borders: BorderChars,
}

impl Theme {
    pub fn from_config(cfg: &ThemeConfig) -> Self {
        let mut base = preset_theme(&cfg.name);

        if !cfg.background.is_empty() {
            base.background = parse_color(&cfg.background, base.background);
        }
        if !cfg.panel_background.is_empty() {
            base.panel_background = parse_color(&cfg.panel_background, base.panel_background);
        }
        if !cfg.primary.is_empty() {
            base.primary = parse_color(&cfg.primary, base.primary);
        }
        if !cfg.secondary.is_empty() {
            base.secondary = parse_color(&cfg.secondary, base.secondary);
        }
        if !cfg.accent.is_empty() {
            base.accent = parse_color(&cfg.accent, base.accent);
        }
        if !cfg.text.is_empty() {
            base.text = parse_color(&cfg.text, base.text);
        }
        if !cfg.muted.is_empty() {
            base.muted = parse_color(&cfg.muted, base.muted);
        }
        if !cfg.border.is_empty() {
            base.border = parse_color(&cfg.border, base.border);
        }
        if !cfg.danger.is_empty() {
            base.danger = parse_color(&cfg.danger, base.danger);
        }

        base.set_border_style(cfg.border_type);
        let _ = base.borders.apply_overrides(&cfg.borders);
        base
    }

    pub fn set_border_style(&mut self, style: BorderStyle) {
        self.border_type = match style {
            BorderStyle::Rounded => BorderType::Rounded,
            BorderStyle::Plain => BorderType::Plain,
            BorderStyle::Double => BorderType::Double,
            BorderStyle::Thick => BorderType::Thick,
        };
        self.borders = BorderChars::from_style(style);
    }

    pub fn set_brightness_mode(&mut self, dark: bool) {
        if dark {
            self.background = Color::Rgb(5, 10, 7);
            self.panel_background = Color::Rgb(8, 17, 12);
            self.text = Color::Rgb(229, 229, 229);
        } else {
            self.background = Color::Rgb(242, 246, 243);
            self.panel_background = Color::Rgb(250, 252, 250);
            self.text = Color::Rgb(24, 34, 28);
            self.muted = Color::Rgb(92, 112, 100);
        }
    }

    pub fn base_style(&self) -> Style {
        Style::default().fg(self.text).bg(self.background)
    }

    pub fn panel_style(&self) -> Style {
        Style::default().fg(self.text).bg(self.panel_background)
    }

    pub fn border_style(&self, override_color: Option<&str>) -> Style {
        let color = override_color
            .map(|c| parse_color(c, self.border))
            .unwrap_or(self.border);
        Style::default().fg(color).bg(self.panel_background)
    }
}

pub fn builtin_theme_names() -> &'static [&'static str] {
    &["matrix", "cyberpunk", "nord", "amber", "dracula"]
}

pub fn preset_theme(name: &str) -> Theme {
    match name.trim().to_ascii_lowercase().as_str() {
        "cyberpunk" => Theme {
            name: "cyberpunk".to_string(),
            background: Color::Rgb(10, 6, 18),
            panel_background: Color::Rgb(16, 10, 28),
            primary: Color::Rgb(0, 255, 213),
            secondary: Color::Rgb(255, 42, 133),
            accent: Color::Rgb(255, 221, 0),
            text: Color::Rgb(234, 242, 255),
            muted: Color::Rgb(118, 104, 150),
            border: Color::Rgb(180, 40, 140),
            danger: Color::Rgb(255, 68, 85),
            border_type: BorderType::Rounded,
            borders: BorderChars::from_style(BorderStyle::Rounded),
        },
        "nord" => Theme {
            name: "nord".to_string(),
            background: Color::Rgb(46, 52, 64),
            panel_background: Color::Rgb(59, 66, 82),
            primary: Color::Rgb(136, 192, 208),
            secondary: Color::Rgb(163, 190, 140),
            accent: Color::Rgb(235, 203, 139),
            text: Color::Rgb(236, 239, 244),
            muted: Color::Rgb(129, 161, 193),
            border: Color::Rgb(76, 86, 106),
            danger: Color::Rgb(191, 97, 106),
            border_type: BorderType::Rounded,
            borders: BorderChars::from_style(BorderStyle::Rounded),
        },
        "amber" => Theme {
            name: "amber".to_string(),
            background: Color::Rgb(12, 8, 2),
            panel_background: Color::Rgb(20, 13, 4),
            primary: Color::Rgb(255, 176, 0),
            secondary: Color::Rgb(255, 214, 90),
            accent: Color::Rgb(255, 110, 40),
            text: Color::Rgb(255, 232, 180),
            muted: Color::Rgb(140, 96, 32),
            border: Color::Rgb(160, 100, 0),
            danger: Color::Rgb(255, 70, 50),
            border_type: BorderType::Plain,
            borders: BorderChars::from_style(BorderStyle::Plain),
        },
        "dracula" => Theme {
            name: "dracula".to_string(),
            background: Color::Rgb(30, 31, 41),
            panel_background: Color::Rgb(40, 42, 54),
            primary: Color::Rgb(80, 250, 123),
            secondary: Color::Rgb(139, 233, 253),
            accent: Color::Rgb(241, 250, 140),
            text: Color::Rgb(248, 248, 242),
            muted: Color::Rgb(98, 114, 164),
            border: Color::Rgb(189, 147, 249),
            danger: Color::Rgb(255, 85, 85),
            border_type: BorderType::Rounded,
            borders: BorderChars::from_style(BorderStyle::Rounded),
        },
        _ => Theme {
            name: "matrix".to_string(),
            background: Color::Rgb(5, 10, 7),
            panel_background: Color::Rgb(8, 17, 12),
            primary: Color::Rgb(57, 255, 136),
            secondary: Color::Rgb(0, 217, 255),
            accent: Color::Rgb(255, 200, 87),
            text: Color::Rgb(229, 229, 229),
            muted: Color::Rgb(95, 125, 107),
            border: Color::Rgb(31, 122, 76),
            danger: Color::Rgb(255, 85, 85),
            border_type: BorderType::Rounded,
            borders: BorderChars::from_style(BorderStyle::Rounded),
        },
    }
}

pub fn parse_color(spec: &str, fallback: Color) -> Color {
    let s = spec.trim().to_ascii_lowercase();
    if let Some(hex) = s.strip_prefix('#')
        && hex.len() == 6
        && let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&hex[0..2], 16),
            u8::from_str_radix(&hex[2..4], 16),
            u8::from_str_radix(&hex[4..6], 16),
        )
    {
        return Color::Rgb(r, g, b);
    }
    match s.as_str() {
        "reset" | "default" | "transparent" => Color::Reset,
        "black" => Color::Black,
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "magenta" => Color::Magenta,
        "cyan" => Color::Cyan,
        "gray" | "grey" => Color::Gray,
        "darkgray" | "darkgrey" => Color::DarkGray,
        "lightred" => Color::LightRed,
        "lightgreen" => Color::LightGreen,
        "lightyellow" => Color::LightYellow,
        "lightblue" => Color::LightBlue,
        "lightmagenta" => Color::LightMagenta,
        "lightcyan" => Color::LightCyan,
        "white" => Color::White,
        _ => fallback,
    }
}

pub fn blend_color(fg: Color, bg: Color, alpha: f32) -> Color {
    let alpha = alpha.clamp(0.0, 1.0);
    let (fr, fg_c, fb) = color_to_rgb(fg).unwrap_or((57, 255, 136));
    let (br, bg_c, bb) = color_to_rgb(bg).unwrap_or((5, 10, 7));
    let r = (br as f32 + (fr as f32 - br as f32) * alpha).round() as u8;
    let g = (bg_c as f32 + (fg_c as f32 - bg_c as f32) * alpha).round() as u8;
    let b = (bb as f32 + (fb as f32 - bb as f32) * alpha).round() as u8;
    Color::Rgb(r, g, b)
}

pub fn color_to_rgb(color: Color) -> Option<(u8, u8, u8)> {
    match color {
        Color::Rgb(r, g, b) => Some((r, g, b)),
        Color::Black => Some((0, 0, 0)),
        Color::Red => Some((205, 49, 49)),
        Color::Green => Some((13, 188, 121)),
        Color::Yellow => Some((229, 229, 16)),
        Color::Blue => Some((36, 114, 200)),
        Color::Magenta => Some((188, 63, 188)),
        Color::Cyan => Some((17, 168, 205)),
        Color::Gray => Some((180, 180, 180)),
        Color::DarkGray => Some((102, 102, 102)),
        Color::LightRed => Some((241, 76, 76)),
        Color::LightGreen => Some((35, 209, 139)),
        Color::LightYellow => Some((245, 245, 67)),
        Color::LightBlue => Some((59, 142, 234)),
        Color::LightMagenta => Some((214, 112, 214)),
        Color::LightCyan => Some((41, 184, 219)),
        Color::White => Some((255, 255, 255)),
        _ => None,
    }
}
