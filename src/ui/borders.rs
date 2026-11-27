use crate::config::BorderStyle;
use ratatui::symbols::border::Set;
use serde::{Deserialize, Serialize};
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct BorderCharsConfig {
    pub top_left: Option<String>,
    pub top_right: Option<String>,
    pub bottom_left: Option<String>,
    pub bottom_right: Option<String>,
    pub horizontal: Option<String>,
    pub vertical: Option<String>,
    pub left_separator: Option<String>,
    pub right_separator: Option<String>,
    pub top_separator: Option<String>,
    pub bottom_separator: Option<String>,
    pub intersection: Option<String>,
}

impl BorderCharsConfig {
    pub fn validate(&self) -> Result<(), (String, String)> {
        let fields = [
            ("theme.borders.top_left", &self.top_left),
            ("theme.borders.top_right", &self.top_right),
            ("theme.borders.bottom_left", &self.bottom_left),
            ("theme.borders.bottom_right", &self.bottom_right),
            ("theme.borders.horizontal", &self.horizontal),
            ("theme.borders.vertical", &self.vertical),
            ("theme.borders.left_separator", &self.left_separator),
            ("theme.borders.right_separator", &self.right_separator),
            ("theme.borders.top_separator", &self.top_separator),
            ("theme.borders.bottom_separator", &self.bottom_separator),
            ("theme.borders.intersection", &self.intersection),
        ];
        for (field, opt) in fields {
            if let Some(val) = opt
                && let Err(msg) = validate_border_char(val)
            {
                return Err((field.to_string(), msg));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BorderChars {
    pub top_left: String,
    pub top_right: String,
    pub bottom_left: String,
    pub bottom_right: String,
    pub horizontal: String,
    pub vertical: String,
    pub left_separator: String,
    pub right_separator: String,
    pub top_separator: String,
    pub bottom_separator: String,
    pub intersection: String,
}

impl BorderChars {
    /// Single source of truth for border properties exposed to runtime commands and autocomplete.
    pub const PROPERTY_SPECS: &'static [(&'static str, &'static str)] = &[
        ("style", "Border preset (rounded | plain | double | thick)"),
        ("horizontal", "Horizontal border line character"),
        ("vertical", "Vertical border line character"),
        ("top_left", "Top-left corner character"),
        ("top_right", "Top-right corner character"),
        ("bottom_left", "Bottom-left corner character"),
        ("bottom_right", "Bottom-right corner character"),
        ("left_separator", "Left T-junction separator character"),
        ("right_separator", "Right T-junction separator character"),
        ("top_separator", "Top T-junction separator character"),
        ("bottom_separator", "Bottom T-junction separator character"),
        ("intersection", "Cross intersection character"),
    ];

    pub const STYLE_NAMES: &'static [(&'static str, &'static str)] = &[
        ("rounded", "Rounded single-line box borders (╭─╮)"),
        ("plain", "Sharp single-line box borders (┌─┐)"),
        ("double", "Double-line box borders (╔═╗)"),
        ("thick", "Heavy thick box borders (┏━┓)"),
        ("ascii", "Classic ASCII character box borders (+-+)"),
        ("none", "No borders"),
    ];

    pub fn from_style(style: BorderStyle) -> Self {
        match style {
            BorderStyle::Rounded => Self {
                top_left: "╭".to_string(),
                top_right: "╮".to_string(),
                bottom_left: "╰".to_string(),
                bottom_right: "╯".to_string(),
                horizontal: "─".to_string(),
                vertical: "│".to_string(),
                left_separator: "├".to_string(),
                right_separator: "┤".to_string(),
                top_separator: "┬".to_string(),
                bottom_separator: "┴".to_string(),
                intersection: "┼".to_string(),
            },
            BorderStyle::Plain => Self {
                top_left: "┌".to_string(),
                top_right: "┐".to_string(),
                bottom_left: "└".to_string(),
                bottom_right: "┘".to_string(),
                horizontal: "─".to_string(),
                vertical: "│".to_string(),
                left_separator: "├".to_string(),
                right_separator: "┤".to_string(),
                top_separator: "┬".to_string(),
                bottom_separator: "┴".to_string(),
                intersection: "┼".to_string(),
            },
            BorderStyle::Double => Self {
                top_left: "╔".to_string(),
                top_right: "╗".to_string(),
                bottom_left: "╚".to_string(),
                bottom_right: "╝".to_string(),
                horizontal: "═".to_string(),
                vertical: "║".to_string(),
                left_separator: "╠".to_string(),
                right_separator: "╣".to_string(),
                top_separator: "╦".to_string(),
                bottom_separator: "╩".to_string(),
                intersection: "╬".to_string(),
            },
            BorderStyle::Thick => Self {
                top_left: "┏".to_string(),
                top_right: "┓".to_string(),
                bottom_left: "┗".to_string(),
                bottom_right: "┛".to_string(),
                horizontal: "━".to_string(),
                vertical: "┃".to_string(),
                left_separator: "┣".to_string(),
                right_separator: "┫".to_string(),
                top_separator: "┳".to_string(),
                bottom_separator: "┻".to_string(),
                intersection: "╋".to_string(),
            },
            BorderStyle::Ascii => Self {
                top_left: "+".to_string(),
                top_right: "+".to_string(),
                bottom_left: "+".to_string(),
                bottom_right: "+".to_string(),
                horizontal: "-".to_string(),
                vertical: "|".to_string(),
                left_separator: "+".to_string(),
                right_separator: "+".to_string(),
                top_separator: "+".to_string(),
                bottom_separator: "+".to_string(),
                intersection: "+".to_string(),
            },
            BorderStyle::None => Self {
                top_left: " ".to_string(),
                top_right: " ".to_string(),
                bottom_left: " ".to_string(),
                bottom_right: " ".to_string(),
                horizontal: " ".to_string(),
                vertical: " ".to_string(),
                left_separator: " ".to_string(),
                right_separator: " ".to_string(),
                top_separator: " ".to_string(),
                bottom_separator: " ".to_string(),
                intersection: " ".to_string(),
            },
        }
    }

    pub fn apply_overrides(&mut self, cfg: &BorderCharsConfig) -> Result<(), String> {
        let pairs = [
            ("top_left", &cfg.top_left),
            ("top_right", &cfg.top_right),
            ("bottom_left", &cfg.bottom_left),
            ("bottom_right", &cfg.bottom_right),
            ("horizontal", &cfg.horizontal),
            ("vertical", &cfg.vertical),
            ("left_separator", &cfg.left_separator),
            ("right_separator", &cfg.right_separator),
            ("top_separator", &cfg.top_separator),
            ("bottom_separator", &cfg.bottom_separator),
            ("intersection", &cfg.intersection),
        ];
        for (key, val_opt) in pairs {
            if let Some(val) = val_opt {
                self.set_property(key, val)?;
            }
        }
        Ok(())
    }

    pub fn set_property(&mut self, key: &str, value: &str) -> Result<(), String> {
        let validated = validate_border_char(value)?;
        match key.trim().to_ascii_lowercase().as_str() {
            "top_left" => self.top_left = validated,
            "top_right" => self.top_right = validated,
            "bottom_left" => self.bottom_left = validated,
            "bottom_right" => self.bottom_right = validated,
            "horizontal" => self.horizontal = validated,
            "vertical" => self.vertical = validated,
            "left_separator" => self.left_separator = validated,
            "right_separator" => self.right_separator = validated,
            "top_separator" => self.top_separator = validated,
            "bottom_separator" => self.bottom_separator = validated,
            "intersection" => self.intersection = validated,
            other => return Err(format!("Unknown border property 'border.{other}'")),
        }
        Ok(())
    }

    pub fn to_config(&self) -> BorderCharsConfig {
        BorderCharsConfig {
            top_left: Some(self.top_left.clone()),
            top_right: Some(self.top_right.clone()),
            bottom_left: Some(self.bottom_left.clone()),
            bottom_right: Some(self.bottom_right.clone()),
            horizontal: Some(self.horizontal.clone()),
            vertical: Some(self.vertical.clone()),
            left_separator: Some(self.left_separator.clone()),
            right_separator: Some(self.right_separator.clone()),
            top_separator: Some(self.top_separator.clone()),
            bottom_separator: Some(self.bottom_separator.clone()),
            intersection: Some(self.intersection.clone()),
        }
    }

    pub fn as_ratatui_set(&self) -> Set<'_> {
        Set {
            top_left: &self.top_left,
            top_right: &self.top_right,
            bottom_left: &self.bottom_left,
            bottom_right: &self.bottom_right,
            vertical_left: &self.vertical,
            vertical_right: &self.vertical,
            horizontal_top: &self.horizontal,
            horizontal_bottom: &self.horizontal,
        }
    }
}

pub fn validate_border_char(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim_matches(|c| c == '"' || c == '\'');
    let mut chars = trimmed.chars();
    let Some(ch) = chars.next() else {
        return Err("Border character cannot be empty".to_string());
    };
    if chars.next().is_some() {
        return Err(format!(
            "Invalid border character '{raw}': expected a single character"
        ));
    }
    if ch.is_control() {
        return Err("Border character cannot be a control character".to_string());
    }
    let width = trimmed.width();
    if width != 1 {
        return Err(format!(
            "Invalid border character '{raw}': display width must be 1 (got {width})"
        ));
    }
    Ok(ch.to_string())
}

pub fn parse_border_style(raw: &str) -> Result<BorderStyle, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "rounded" => Ok(BorderStyle::Rounded),
        "plain" | "solid" => Ok(BorderStyle::Plain),
        "double" => Ok(BorderStyle::Double),
        "thick" => Ok(BorderStyle::Thick),
        "ascii" => Ok(BorderStyle::Ascii),
        "none" | "off" => Ok(BorderStyle::None),
        other => Err(format!(
            "Invalid border style '{other}' (expected rounded, plain, double, thick, ascii, or none)"
        )),
    }
}

pub fn suggested_chars_for_property(prop: &str) -> &'static [(&'static str, &'static str)] {
    match prop {
        "horizontal" => &[
            ("─", "Single light horizontal line"),
            ("━", "Thick horizontal line"),
            ("═", "Double horizontal line"),
            ("┄", "Dashed horizontal line"),
            ("-", "ASCII hyphen"),
        ],
        "vertical" => &[
            ("│", "Single light vertical line"),
            ("┃", "Thick vertical line"),
            ("║", "Double vertical line"),
            ("┆", "Dashed vertical line"),
            ("|", "ASCII pipe"),
        ],
        "top_left" => &[
            ("╭", "Rounded corner"),
            ("┌", "Plain corner"),
            ("╔", "Double corner"),
            ("┏", "Thick corner"),
            ("+", "ASCII corner"),
        ],
        "top_right" => &[
            ("╮", "Rounded corner"),
            ("┐", "Plain corner"),
            ("╗", "Double corner"),
            ("┓", "Thick corner"),
            ("+", "ASCII corner"),
        ],
        "bottom_left" => &[
            ("╰", "Rounded corner"),
            ("└", "Plain corner"),
            ("╚", "Double corner"),
            ("┗", "Thick corner"),
            ("+", "ASCII corner"),
        ],
        "bottom_right" => &[
            ("╯", "Rounded corner"),
            ("┘", "Plain corner"),
            ("╝", "Double corner"),
            ("┛", "Thick corner"),
            ("+", "ASCII corner"),
        ],
        _ => &[
            ("┼", "Single intersection"),
            ("╋", "Thick intersection"),
            ("╬", "Double intersection"),
            ("+", "ASCII intersection"),
        ],
    }
}
