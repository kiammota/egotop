use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct FigletFont {
    pub name: String,
    pub height: usize,
    pub glyphs: HashMap<char, Vec<String>>,
}

impl FigletFont {
    pub fn render_lines(&self, text: &str) -> Vec<String> {
        let normalized = normalize_ascii_text(text);
        let mut rows = vec![String::new(); self.height];

        for ch in normalized.chars() {
            let upper = ch.to_ascii_uppercase();
            let glyph = self
                .glyphs
                .get(&ch)
                .or_else(|| self.glyphs.get(&upper))
                .or_else(|| self.glyphs.get(&' '));

            if let Some(lines) = glyph {
                for (r, row) in rows.iter_mut().enumerate() {
                    if let Some(part) = lines.get(r) {
                        row.push_str(part);
                    }
                }
            }
        }

        // Trim trailing spaces uniformly while keeping non-empty lines aligned
        let max_len = rows
            .iter()
            .map(|r| r.trim_end().chars().count())
            .max()
            .unwrap_or(0);
        if max_len == 0 {
            return Vec::new();
        }
        rows.into_iter()
            .map(|r| r.trim_end().to_string())
            .collect()
    }

    pub fn from_flf_str(name: &str, content: &str) -> Option<Self> {
        let mut lines = content.lines();
        let header = lines.next()?;
        if !header.starts_with("flf2a") {
            return None;
        }
        let header_parts: Vec<&str> = header.split_whitespace().collect();
        if header_parts.len() < 6 {
            return None;
        }
        let hardblank = header_parts[0].chars().last().unwrap_or('$');
        let height: usize = header_parts[1].parse().ok()?;
        let comment_lines: usize = header_parts[5].parse().ok()?;

        for _ in 0..comment_lines {
            lines.next()?;
        }

        let mut glyphs = HashMap::new();
        for code in 32u32..=126u32 {
            let Some(ch) = char::from_u32(code) else { continue };
            let mut glyph_lines = Vec::with_capacity(height);
            for row_idx in 0..height {
                let Some(raw) = lines.next() else {
                    break;
                };
                let cleaned = clean_flf_row(raw, hardblank, row_idx + 1 == height);
                glyph_lines.push(cleaned);
            }
            if glyph_lines.len() == height {
                glyphs.insert(ch, glyph_lines);
            }
        }

        if glyphs.is_empty() {
            None
        } else {
            Some(Self {
                name: name.to_string(),
                height,
                glyphs,
            })
        }
    }
}

fn clean_flf_row(raw: &str, hardblank: char, is_last_row: bool) -> String {
    let mut s = raw.trim_end_matches('\r').to_string();
    if let Some(end_marker) = s.chars().last() {
        s.pop();
        if is_last_row && s.ends_with(end_marker) {
            s.pop();
        }
    }
    s.replace(hardblank, " ")
}

pub fn normalize_ascii_text(input: &str) -> String {
    input
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'Á' | 'À' | 'Â' | 'Ã' | 'Ä' => 'A',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'É' | 'È' | 'Ê' | 'Ë' => 'E',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'Í' | 'Ì' | 'Î' | 'Ï' => 'I',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'Ó' | 'Ò' | 'Ô' | 'Õ' | 'Ö' => 'O',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'Ú' | 'Ù' | 'Û' | 'Ü' => 'U',
            'ç' => 'c',
            'Ç' => 'C',
            'ñ' => 'n',
            'Ñ' => 'N',
            other => other,
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinFont {
    AnsiShadow,
    Standard,
    Big,
    Slant,
    Small,
    Block,
    Doom,
}

impl BuiltinFont {
    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "ansi shadow" | "ansi-shadow" | "ansishadow" | "shadow" => Some(Self::AnsiShadow),
            "standard" => Some(Self::Standard),
            "big" => Some(Self::Big),
            "slant" => Some(Self::Slant),
            "small" => Some(Self::Small),
            "block" => Some(Self::Block),
            "doom" => Some(Self::Doom),
            _ => None,
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::AnsiShadow => "ANSI Shadow",
            Self::Standard => "Standard",
            Self::Big => "Big",
            Self::Slant => "Slant",
            Self::Small => "Small",
            Self::Block => "Block",
            Self::Doom => "Doom",
        }
    }
}

pub fn load_font(name: &str, ascii_fallback: bool) -> FigletFont {
    // 1. Check user custom .flf fonts in ~/.config/egotop/fonts/
    if let Some(cfg_dir) = dirs::config_dir() {
        let slug = name.trim().to_ascii_lowercase().replace(' ', "_");
        for candidate in [
            cfg_dir.join("egotop/fonts").join(format!("{name}.flf")),
            cfg_dir.join("egotop/fonts").join(format!("{slug}.flf")),
        ] {
            if let Ok(raw) = fs::read_to_string(&candidate)
                && let Some(font) = FigletFont::from_flf_str(name, &raw)
            {
                return font;
            }
        }
    }

    // 2. Check explicit file path
    let path = Path::new(name);
    if path.is_file()
        && let Ok(raw) = fs::read_to_string(path)
        && let Some(font) = FigletFont::from_flf_str(name, &raw)
    {
        return font;
    }

    // 3. Built-in font
    let kind = BuiltinFont::from_name(name).unwrap_or(BuiltinFont::Standard);
    build_builtin_font(kind, ascii_fallback)
}

pub fn available_fonts() -> &'static [&'static str] {
    &[
        "ANSI Shadow",
        "Standard",
        "Big",
        "Slant",
        "Small",
        "Block",
        "Doom",
    ]
}

// 5-row canonical bitmap (each string row uses '#' for lit pixel, '.' for empty)
fn glyph_bitmap_5x(ch: char) -> &'static [&'static str] {
    match ch {
        'A' => &[
            ".###.",
            "#...#",
            "#####",
            "#...#",
            "#...#",
        ],
        'B' => &[
            "####.",
            "#...#",
            "####.",
            "#...#",
            "####.",
        ],
        'C' => &[
            ".####",
            "#....",
            "#....",
            "#....",
            ".####",
        ],
        'D' => &[
            "####.",
            "#...#",
            "#...#",
            "#...#",
            "####.",
        ],
        'E' => &[
            "#####",
            "#....",
            "####.",
            "#....",
            "#####",
        ],
        'F' => &[
            "#####",
            "#....",
            "####.",
            "#....",
            "#....",
        ],
        'G' => &[
            ".####",
            "#....",
            "#..##",
            "#...#",
            ".####",
        ],
        'H' => &[
            "#...#",
            "#...#",
            "#####",
            "#...#",
            "#...#",
        ],
        'I' => &[
            "###",
            ".#.",
            ".#.",
            ".#.",
            "###",
        ],
        'J' => &[
            "..###",
            "...#.",
            "...#.",
            "#..#.",
            ".##..",
        ],
        'K' => &[
            "#...#",
            "#..#.",
            "###..",
            "#..#.",
            "#...#",
        ],
        'L' => &[
            "#....",
            "#....",
            "#....",
            "#....",
            "#####",
        ],
        'M' => &[
            "#...#",
            "##.##",
            "#.#.#",
            "#...#",
            "#...#",
        ],
        'N' => &[
            "#...#",
            "##..#",
            "#.#.#",
            "#..##",
            "#...#",
        ],
        'O' => &[
            ".###.",
            "#...#",
            "#...#",
            "#...#",
            ".###.",
        ],
        'P' => &[
            "####.",
            "#...#",
            "####.",
            "#....",
            "#....",
        ],
        'Q' => &[
            ".###.",
            "#...#",
            "#.#.#",
            "#..#.",
            ".##.#",
        ],
        'R' => &[
            "####.",
            "#...#",
            "####.",
            "#..#.",
            "#...#",
        ],
        'S' => &[
            ".####",
            "#....",
            ".###.",
            "....#",
            "####.",
        ],
        'T' => &[
            "#####",
            "..#..",
            "..#..",
            "..#..",
            "..#..",
        ],
        'U' => &[
            "#...#",
            "#...#",
            "#...#",
            "#...#",
            ".###.",
        ],
        'V' => &[
            "#...#",
            "#...#",
            "#...#",
            ".#.#.",
            "..#..",
        ],
        'W' => &[
            "#...#",
            "#...#",
            "#.#.#",
            "##.##",
            "#...#",
        ],
        'X' => &[
            "#...#",
            ".#.#.",
            "..#..",
            ".#.#.",
            "#...#",
        ],
        'Y' => &[
            "#...#",
            ".#.#.",
            "..#..",
            "..#..",
            "..#..",
        ],
        'Z' => &[
            "#####",
            "...#.",
            "..#..",
            ".#...",
            "#####",
        ],
        '0' => &[
            ".###.",
            "#..##",
            "#.#.#",
            "##..#",
            ".###.",
        ],
        '1' => &[
            ".#.",
            "##.",
            ".#.",
            ".#.",
            "###",
        ],
        '2' => &[
            "####.",
            "....#",
            ".###.",
            "#....",
            "#####",
        ],
        '3' => &[
            "####.",
            "....#",
            ".###.",
            "....#",
            "####.",
        ],
        '4' => &[
            "#...#",
            "#...#",
            "#####",
            "....#",
            "....#",
        ],
        '5' => &[
            "#####",
            "#....",
            "####.",
            "....#",
            "####.",
        ],
        '6' => &[
            ".###.",
            "#....",
            "####.",
            "#...#",
            ".###.",
        ],
        '7' => &[
            "#####",
            "...#.",
            "..#..",
            ".#...",
            ".#...",
        ],
        '8' => &[
            ".###.",
            "#...#",
            ".###.",
            "#...#",
            ".###.",
        ],
        '9' => &[
            ".###.",
            "#...#",
            ".####",
            "....#",
            ".###.",
        ],
        '!' => &[
            "#",
            "#",
            "#",
            ".",
            "#",
        ],
        '?' => &[
            ".###.",
            "#...#",
            "..##.",
            ".....",
            "..#..",
        ],
        ',' => &[
            "..",
            "..",
            "..",
            ".#",
            "#.",
        ],
        '.' => &[
            "..",
            "..",
            "..",
            "..",
            "#.",
        ],
        ':' => &[
            ".",
            "#",
            ".",
            "#",
            ".",
        ],
        '-' => &[
            "...",
            "...",
            "###",
            "...",
            "...",
        ],
        '_' => &[
            "....",
            "....",
            "....",
            "....",
            "####",
        ],
        '/' => &[
            "...#",
            "..#.",
            ".#..",
            "#...",
            "....",
        ],
        '+' => &[
            "...",
            ".#.",
            "###",
            ".#.",
            "...",
        ],
        '(' => &[
            ".#",
            "#.",
            "#.",
            "#.",
            ".#",
        ],
        ')' => &[
            "#.",
            ".#",
            ".#",
            ".#",
            "#.",
        ],
        '@' => &[
            ".###.",
            "#.#.#",
            "#.##.",
            "#....",
            ".####",
        ],
        ' ' => &[
            "...",
            "...",
            "...",
            "...",
            "...",
        ],
        _ => &[
            "...",
            "...",
            "...",
            "...",
            "...",
        ],
    }
}

fn glyph_small_3x(ch: char) -> &'static [&'static str] {
    match ch {
        'A' => &[" _ ", "/_\\", "| |"],
        'B' => &[" _ ", "|_)", "|_)"],
        'C' => &[" _ ", "/  ", "\\_ "],
        'D' => &[" _ ", "| \\", "|_/"],
        'E' => &[" _ ", "|_ ", "|_ "],
        'F' => &[" _ ", "|_ ", "|  "],
        'G' => &[" _ ", "/ _", "\\_/"],
        'H' => &["   ", "|_|", "| |"],
        'I' => &[" ", "|", "|"],
        'J' => &["  _", "  |", "\\_|"],
        'K' => &["   ", "|_/", "| \\"],
        'L' => &["  ", "| ", "|_"],
        'M' => &["   ", "|\\/|", "|  |"],
        'N' => &["   ", "|\\|", "| |"],
        'O' => &[" _ ", "/ \\", "\\_/"],
        'P' => &[" _ ", "|_)", "|  "],
        'Q' => &[" _ ", "/ \\", "\\_\\"],
        'R' => &[" _ ", "|_)", "| \\"],
        'S' => &[" _ ", "(_ ", " _)"],
        'T' => &["___", " | ", " | "],
        'U' => &["   ", "| |", "\\_/"],
        'V' => &["   ", "\\ /", " V "],
        'W' => &["    ", "|  |", "|/\\|"],
        'X' => &["  ", "\\/", "/\\"],
        'Y' => &["   ", "\\_/", " | "],
        'Z' => &["__", " /", "/_"],
        '0' => &[" _ ", "/ \\", "\\_/"],
        '1' => &[" ", "/|", " |"],
        '2' => &[" _ ", " _)", "/_ "],
        '3' => &[" _ ", " _)", " _)"],
        '4' => &["   ", "|_|", "  |"],
        '5' => &[" _ ", "|_ ", " _)"],
        '6' => &[" _ ", "|_ ", "|_)"],
        '7' => &["__", " /", "/ "],
        '8' => &[" _ ", "(_)", "(_)"],
        '9' => &[" _ ", "(_|", " _|"],
        '!' => &["|", "|", "."],
        '?' => &[" _ ", " _)", " . "],
        ',' => &[" ", " ", ","],
        '.' => &[" ", " ", "."],
        ':' => &[" ", ".", "."],
        '-' => &["  ", "--", "  "],
        '_' => &["  ", "  ", "__"],
        ' ' => &["  ", "  ", "  "],
        _ => &["  ", "  ", "  "],
    }
}

const SUPPORTED_CHARS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!?.,:-_/+()@ ";

fn build_builtin_font(kind: BuiltinFont, ascii_fallback: bool) -> FigletFont {
    let mut glyphs = HashMap::new();
    let effective = if ascii_fallback && matches!(kind, BuiltinFont::AnsiShadow | BuiltinFont::Block) {
        if kind == BuiltinFont::Block {
            BuiltinFont::Standard
        } else {
            BuiltinFont::Doom
        }
    } else {
        kind
    };

    let height = match effective {
        BuiltinFont::Small => 3,
        BuiltinFont::Standard | BuiltinFont::Slant | BuiltinFont::Block => 5,
        BuiltinFont::AnsiShadow | BuiltinFont::Big | BuiltinFont::Doom => 6,
    };

    for ch in SUPPORTED_CHARS.chars() {
        let lines = match effective {
            BuiltinFont::Small => glyph_small_3x(ch)
                .iter()
                .map(|r| format!("{r} "))
                .collect(),
            BuiltinFont::Block => render_block_glyph(ch, !ascii_fallback),
            BuiltinFont::Standard => render_outline_glyph(ch, false, false),
            BuiltinFont::Slant => render_slant_glyph(ch),
            BuiltinFont::Big => render_big_glyph(ch),
            BuiltinFont::Doom => render_doom_glyph(ch),
            BuiltinFont::AnsiShadow => render_ansi_shadow_glyph(ch),
        };
        glyphs.insert(ch, lines);
    }

    FigletFont {
        name: effective.display_name().to_string(),
        height,
        glyphs,
    }
}

fn render_block_glyph(ch: char, unicode: bool) -> Vec<String> {
    let bmp = glyph_bitmap_5x(ch);
    let fill = if unicode { "██" } else { "##" };
    bmp.iter()
        .map(|row| {
            let mut out = String::new();
            for c in row.chars() {
                if c == '#' {
                    out.push_str(fill);
                } else {
                    out.push_str("  ");
                }
            }
            out.push(' ');
            out
        })
        .collect()
}

fn render_outline_glyph(ch: char, double_wide: bool, _bevel: bool) -> Vec<String> {
    let bmp = glyph_bitmap_5x(ch);
    let h = bmp.len();
    let w = bmp[0].len();
    let grid: Vec<Vec<bool>> = bmp
        .iter()
        .map(|r| r.chars().map(|c| c == '#').collect())
        .collect();

    let mut rows = Vec::with_capacity(h);
    for r in 0..h {
        let mut line = String::new();
        for c in 0..w {
            if grid[r][c] {
                let up = r > 0 && grid[r - 1][c];
                let down = r + 1 < h && grid[r + 1][c];
                let left = c > 0 && grid[r][c - 1];
                let right = c + 1 < w && grid[r][c + 1];

                let ch_out = match (up, down, left, right) {
                    (false, true, false, true) => '/',
                    (false, true, true, false) => '\\',
                    (true, false, false, true) => '\\',
                    (true, false, true, false) => '/',
                    (true, true, _, _) => '|',
                    (false, false, _, _) => '_',
                    (_, _, true, true) => '=',
                    _ => '#',
                };
                line.push(ch_out);
                if double_wide {
                    line.push(if ch_out == '|' { ' ' } else { ch_out });
                }
            } else {
                line.push(' ');
                if double_wide {
                    line.push(' ');
                }
            }
        }
        line.push(' ');
        rows.push(line);
    }
    rows
}

fn render_slant_glyph(ch: char) -> Vec<String> {
    let base = render_outline_glyph(ch, false, false);
    let h = base.len();
    base.into_iter()
        .enumerate()
        .map(|(r, line)| {
            let pad = " ".repeat(h.saturating_sub(1 + r));
            let trail = " ".repeat(r);
            format!("{pad}{line}{trail}")
        })
        .collect()
}

fn render_big_glyph(ch: char) -> Vec<String> {
    let bmp = glyph_bitmap_5x(ch);
    let h = bmp.len();
    let w = bmp[0].len();
    let grid: Vec<Vec<bool>> = bmp
        .iter()
        .map(|r| r.chars().map(|c| c == '#').collect())
        .collect();

    // 6-row tall wide ASCII style: top cap + 5 body rows
    let mut rows = Vec::with_capacity(6);
    let mut top = String::new();
    for &lit in grid[0].iter().take(w) {
        top.push_str(if lit { "__" } else { "  " });
    }
    top.push(' ');
    rows.push(top);

    for r in 0..h {
        let mut line = String::new();
        for c in 0..w {
            if grid[r][c] {
                let down = r + 1 < h && grid[r + 1][c];
                let left = c > 0 && grid[r][c - 1];
                let right = c + 1 < w && grid[r][c + 1];
                let pair = match (left, right, down) {
                    (false, false, _) => "||",
                    (false, true, false) => "|_",
                    (false, true, true) => "| ",
                    (true, false, false) => "_|",
                    (true, false, true) => " |",
                    (true, true, false) => "__",
                    (true, true, true) => "  ",
                };
                line.push_str(pair);
            } else {
                line.push_str("  ");
            }
        }
        line.push(' ');
        rows.push(line);
    }
    rows
}

fn render_doom_glyph(ch: char) -> Vec<String> {
    let bmp = glyph_bitmap_5x(ch);
    let h = bmp.len();
    let w = bmp[0].len();
    let grid: Vec<Vec<bool>> = bmp
        .iter()
        .map(|r| r.chars().map(|c| c == '#').collect())
        .collect();

    let mut rows = Vec::with_capacity(6);
    let mut top = String::new();
    for &lit in grid[0].iter().take(w) {
        top.push_str(if lit { "__" } else { "  " });
    }
    top.push(' ');
    rows.push(top);

    for r in 0..h {
        let mut line = String::new();
        for c in 0..w {
            if grid[r][c] {
                let up = r > 0 && grid[r - 1][c];
                let down = r + 1 < h && grid[r + 1][c];
                let left = c > 0 && grid[r][c - 1];
                let right = c + 1 < w && grid[r][c + 1];
                let tok = if !up && !left && right && down {
                    "/ "
                } else if !up && left && !right && down {
                    " \\"
                } else if !down {
                    "__"
                } else if left && right {
                    "=="
                } else {
                    "||"
                };
                line.push_str(tok);
            } else {
                line.push_str("  ");
            }
        }
        line.push(' ');
        rows.push(line);
    }
    rows
}

fn render_ansi_shadow_glyph(ch: char) -> Vec<String> {
    let bmp = glyph_bitmap_5x(ch);
    let h = bmp.len();
    let w = bmp[0].len();
    let grid: Vec<Vec<bool>> = bmp
        .iter()
        .map(|r| r.chars().map(|c| c == '#').collect())
        .collect();

    let on = |r: isize, c: isize| -> bool {
        if r < 0 || c < 0 || r >= h as isize || c >= w as isize {
            false
        } else {
            grid[r as usize][c as usize]
        }
    };

    let mut rows = Vec::with_capacity(6);
    for r in 0..=h {
        let mut line = String::new();
        for c in 0..=w {
            let ri = r as isize;
            let ci = c as isize;
            if on(ri, ci) {
                // Full solid pixel (2 cells wide for balanced aspect ratio, except last shadow col)
                line.push_str(if c < w { "██" } else { "█" });
            } else {
                let left = on(ri, ci - 1);
                let up = on(ri - 1, ci);
                let upleft = on(ri - 1, ci - 1);

                let shadow_left = match (left, up, upleft) {
                    (true, true, true) => '╝',
                    (true, true, false) => '╔',
                    (true, false, true) => '║',
                    (true, false, false) => '╗',
                    (false, true, true) => '═',
                    (false, true, false) => '╚',
                    (false, false, true) => '╝',
                    (false, false, false) => ' ',
                };

                if c < w {
                    let shadow_right = if up { '═' } else { ' ' };
                    line.push(shadow_left);
                    line.push(if shadow_left == '╗' || shadow_left == '╝' || shadow_left == '║' {
                        ' '
                    } else if shadow_left == '╔' || shadow_left == '╚' || shadow_left == '═' {
                        shadow_right
                    } else {
                        ' '
                    });
                } else {
                    line.push(shadow_left);
                }
            }
        }
        rows.push(line);
    }
    rows
}
