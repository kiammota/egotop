use crate::config::TypographyConfig;
use crate::system::SystemSnapshot;
use crate::ui::fonts::{load_font, normalize_block_lines};
use std::fs;
use std::path::PathBuf;
use unicode_width::UnicodeWidthStr;

const LOGO_ARCH: &str = include_str!("../../assets/logos/arch.txt");
const LOGO_HYPRLAND: &str = include_str!("../../assets/logos/hyprland.txt");
const LOGO_EGOTOP: &str = include_str!("../../assets/logos/egotop.txt");
const LOGO_LINUX: &str = include_str!("../../assets/logos/linux.txt");

#[derive(Debug, Clone, Copy)]
pub struct IconSet {
    pub use_nerd: bool,
}

impl IconSet {
    pub fn from_config(cfg: &TypographyConfig) -> Self {
        Self {
            use_nerd: cfg.use_nerd_fonts && !cfg.ascii_fallback,
        }
    }

    pub fn os(self) -> &'static str {
        if self.use_nerd { "󰣇" } else { "[OS]" }
    }

    pub fn kernel(self) -> &'static str {
        if self.use_nerd { "" } else { "[KN]" }
    }

    pub fn host(self) -> &'static str {
        if self.use_nerd { "󰒋" } else { "[HT]" }
    }

    pub fn user(self) -> &'static str {
        if self.use_nerd { "" } else { "[US]" }
    }

    pub fn shell(self) -> &'static str {
        if self.use_nerd { "" } else { "[SH]" }
    }

    pub fn terminal(self) -> &'static str {
        if self.use_nerd { "" } else { "[TM]" }
    }

    pub fn wm(self) -> &'static str {
        if self.use_nerd { "" } else { "[WM]" }
    }

    pub fn cpu(self) -> &'static str {
        if self.use_nerd { "" } else { "[CPU]" }
    }

    pub fn memory(self) -> &'static str {
        if self.use_nerd { "" } else { "[RAM]" }
    }

    pub fn disk(self) -> &'static str {
        if self.use_nerd { "󰋊" } else { "[DSK]" }
    }

    pub fn gpu(self) -> &'static str {
        if self.use_nerd { "󰢮" } else { "[GPU]" }
    }

    pub fn clock(self) -> &'static str {
        if self.use_nerd { "" } else { "[CLK]" }
    }

    pub fn uptime(self) -> &'static str {
        if self.use_nerd { "󰔟" } else { "[UP]" }
    }

    pub fn activity(self) -> &'static str {
        if self.use_nerd { "󰍛" } else { "[ACT]" }
    }
}

pub fn render_ascii_heading(
    text: &str,
    font_name: &str,
    ascii_fallback: bool,
    max_width: u16,
) -> Vec<String> {
    if text.trim().is_empty() || max_width == 0 {
        return Vec::new();
    }

    let font = load_font(font_name, ascii_fallback);
    let rendered = font.render_lines(text);
    let widest = rendered.iter().map(|l| l.width()).max().unwrap_or(0);

    if widest <= max_width as usize {
        return rendered;
    }

    // Try Standard before falling back to Small
    if !font_name.eq_ignore_ascii_case("Standard") {
        let std_font = load_font("Standard", ascii_fallback);
        let std_lines = std_font.render_lines(text);
        let std_widest = std_lines.iter().map(|l| l.width()).max().unwrap_or(0);
        if std_widest <= max_width as usize {
            return std_lines;
        }
    }

    // Responsive fallback to Small font if chosen font overflows available width
    let small = load_font("Small", ascii_fallback);
    let small_lines = small.render_lines(text);
    let small_widest = small_lines.iter().map(|l| l.width()).max().unwrap_or(0);
    if small_widest <= max_width as usize {
        return small_lines;
    }

    // Ultra-narrow terminal fallback: plain centered text
    vec![text.to_string()]
}

pub fn resolve_logo_lines(cfg: &TypographyConfig, snapshot: &SystemSnapshot) -> Vec<String> {
    if let Some(custom) = &cfg.custom_logo {
        let lines: Vec<String> = custom.lines().map(ToString::to_string).collect();
        if !lines.is_empty() {
            return normalize_block_lines(lines);
        }
    }

    if let Some(file_spec) = &cfg.custom_logo_file {
        let expanded = expand_tilde(file_spec);
        if let Ok(raw) = fs::read_to_string(expanded) {
            let lines: Vec<String> = raw.lines().map(ToString::to_string).collect();
            if !lines.is_empty() {
                return normalize_block_lines(lines);
            }
        }
    }

    let preset = cfg.logo_preset.trim().to_ascii_lowercase();
    let chosen = match preset.as_str() {
        "none" | "off" => return Vec::new(),
        "arch" => LOGO_ARCH,
        "hyprland" => LOGO_HYPRLAND,
        "linux" | "tux" => LOGO_LINUX,
        "egotop" => LOGO_EGOTOP,
        _ => {
            if snapshot
                .os
                .distro_id
                .as_deref()
                .is_some_and(|id| id.eq_ignore_ascii_case("arch"))
            {
                LOGO_ARCH
            } else if snapshot.session.is_hyprland {
                LOGO_HYPRLAND
            } else {
                LOGO_LINUX
            }
        }
    };

    normalize_block_lines(chosen.lines().map(ToString::to_string).collect())
}

fn expand_tilde(path_str: &str) -> PathBuf {
    if let Some(rest) = path_str.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(rest);
    }
    PathBuf::from(path_str)
}

#[derive(Debug, Clone)]
pub struct RevealState {
    pub progress: f32,
    pub duration_secs: f32,
    pub seed: u64,
}

impl RevealState {
    pub fn new(duration_secs: f32) -> Self {
        Self {
            progress: 0.0,
            duration_secs: duration_secs.max(0.1),
            seed: 0x5EED_CAFE_BABE_1337,
        }
    }

    pub fn complete() -> Self {
        Self {
            progress: 1.0,
            duration_secs: 1.0,
            seed: 0x5EED_CAFE_BABE_1337,
        }
    }

    pub fn advance(&mut self, dt_secs: f32) {
        if self.progress < 1.0 {
            self.progress = (self.progress + dt_secs / self.duration_secs).min(1.0);
        }
    }

    pub fn is_done(&self) -> bool {
        self.progress >= 1.0
    }

    pub fn mask_line(&self, line: &str, row_idx: usize) -> String {
        if self.is_done() {
            return line.to_string();
        }
        const GLYPHS: &[char] = &['0', '1', 'ｱ', 'ｶ', 'ｻ', 'ﾀ', 'ﾅ', 'ﾊ', '#', '*', ':'];
        let total = line.chars().count().max(1);
        let revealed_count = ((total as f32) * self.progress).floor() as usize;

        line.chars()
            .enumerate()
            .map(|(col, ch)| {
                if ch == ' ' {
                    ' '
                } else if col < revealed_count {
                    ch
                } else if col <= revealed_count + 2 {
                    let hash = ((self.seed as usize)
                        .wrapping_add(row_idx * 31)
                        .wrapping_add(col * 17)
                        .wrapping_add((self.progress * 100.0) as usize))
                        % GLYPHS.len();
                    GLYPHS[hash]
                } else {
                    ' '
                }
            })
            .collect()
    }
}
