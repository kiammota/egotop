use std::collections::HashMap;
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HorizontalLayout {
    FullWidth,
    Kerning,
    Smushing { rules: u32 },
}

#[derive(Debug, Clone)]
pub struct FigletFont {
    pub name: String,
    pub height: usize,
    pub hardblank: char,
    pub layout: HorizontalLayout,
    pub glyphs: HashMap<char, Vec<String>>,
}

impl FigletFont {
    pub fn render_lines(&self, text: &str) -> Vec<String> {
        let normalized = normalize_ascii_text(text);
        if normalized.is_empty() || self.height == 0 {
            return Vec::new();
        }

        let mut canvas = vec![String::new(); self.height];

        for ch in normalized.chars() {
            let upper = ch.to_ascii_uppercase();
            let glyph = self
                .glyphs
                .get(&ch)
                .or_else(|| self.glyphs.get(&upper))
                .or_else(|| self.glyphs.get(&' '));

            let Some(raw_glyph) = glyph else {
                continue;
            };

            let padded_glyph = pad_glyph_uniform(raw_glyph, self.height);
            append_glyph_with_layout(&mut canvas, &padded_glyph, self.hardblank, self.layout);
        }

        // Convert hardblanks to normal spaces only after horizontal smushing/kerning
        let finalized: Vec<String> = canvas
            .into_iter()
            .map(|row| row.replace(self.hardblank, " "))
            .collect();

        normalize_block_lines(finalized)
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
        if height == 0 {
            return None;
        }
        let old_layout: i32 = header_parts[4].parse().unwrap_or(0);
        let comment_lines: usize = header_parts[5].parse().ok()?;
        let full_layout: Option<u32> = header_parts.get(7).and_then(|s| s.parse().ok());

        for _ in 0..comment_lines {
            lines.next()?;
        }

        let layout = decode_flf_layout(old_layout, full_layout);

        let mut glyphs = HashMap::new();
        // Required ASCII 32..=126
        for code in 32u32..=126u32 {
            let Some(ch) = char::from_u32(code) else {
                continue;
            };
            if let Some(glyph_lines) = read_flf_glyph(&mut lines, height) {
                glyphs.insert(ch, pad_glyph_uniform(&glyph_lines, height));
            } else {
                break;
            }
        }

        // Standard 7 German characters (196, 214, 220, 228, 246, 252, 223) if present,
        // followed by optional code-tagged characters.
        let german_codes = [196u32, 214, 220, 228, 246, 252, 223];
        for code in german_codes {
            let Some(glyph_lines) = read_flf_glyph(&mut lines, height) else {
                break;
            };
            if let Some(ch) = char::from_u32(code) {
                glyphs.insert(ch, pad_glyph_uniform(&glyph_lines, height));
            }
        }

        while let Some(tag_line) = lines.next() {
            let trimmed = tag_line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let code_token = trimmed.split_whitespace().next().unwrap_or("");
            if let Some(code) = parse_flf_codetag(code_token)
                && let Some(glyph_lines) = read_flf_glyph(&mut lines, height)
                && let Some(ch) = char::from_u32(code)
            {
                glyphs.insert(ch, pad_glyph_uniform(&glyph_lines, height));
            }
        }

        if glyphs.is_empty() {
            None
        } else {
            Some(Self {
                name: name.to_string(),
                height,
                hardblank,
                layout,
                glyphs,
            })
        }
    }
}

fn read_flf_glyph<'a>(
    lines: &mut impl Iterator<Item = &'a str>,
    height: usize,
) -> Option<Vec<String>> {
    let mut glyph_lines = Vec::with_capacity(height);
    for row_idx in 0..height {
        let raw = lines.next()?;
        glyph_lines.push(clean_flf_row(raw, row_idx + 1 == height));
    }
    Some(glyph_lines)
}

fn parse_flf_codetag(token: &str) -> Option<u32> {
    let t = token.trim();
    if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        u32::from_str_radix(hex, 16).ok()
    } else if let Some(oct) = t.strip_prefix('0') {
        if oct.is_empty() {
            Some(0)
        } else {
            u32::from_str_radix(oct, 8).ok()
        }
    } else if let Ok(n) = t.parse::<i64>() {
        u32::try_from(n).ok()
    } else {
        None
    }
}

fn clean_flf_row(raw: &str, is_last_row: bool) -> String {
    let mut s = raw.trim_end_matches('\r').to_string();
    if let Some(end_marker) = s.chars().last() {
        s.pop();
        if is_last_row && s.ends_with(end_marker) {
            s.pop();
        }
    }
    s
}

fn decode_flf_layout(old_layout: i32, full_layout: Option<u32>) -> HorizontalLayout {
    if let Some(full) = full_layout {
        let smush = (full & 128) != 0;
        let kern = (full & 64) != 0;
        if smush {
            HorizontalLayout::Smushing { rules: full & 63 }
        } else if kern {
            HorizontalLayout::Kerning
        } else {
            HorizontalLayout::FullWidth
        }
    } else if old_layout < 0 {
        HorizontalLayout::FullWidth
    } else if old_layout == 0 {
        HorizontalLayout::Kerning
    } else {
        HorizontalLayout::Smushing {
            rules: (old_layout as u32) & 63,
        }
    }
}

fn pad_glyph_uniform(rows: &[String], height: usize) -> Vec<String> {
    let max_w = rows.iter().map(|r| r.width()).max().unwrap_or(0);
    let mut out = Vec::with_capacity(height);
    for r in 0..height {
        let line = rows.get(r).map(String::as_str).unwrap_or("");
        let w = line.width();
        let mut padded = String::from(line);
        if w < max_w {
            padded.push_str(&" ".repeat(max_w - w));
        }
        out.push(padded);
    }
    out
}

pub fn normalize_block_lines(lines: Vec<String>) -> Vec<String> {
    let max_w = lines.iter().map(|l| l.width()).max().unwrap_or(0);
    if max_w == 0 {
        return Vec::new();
    }
    lines
        .into_iter()
        .map(|mut l| {
            let w = l.width();
            if w < max_w {
                l.push_str(&" ".repeat(max_w - w));
            }
            l
        })
        .collect()
}

fn append_glyph_with_layout(
    canvas: &mut [String],
    glyph: &[String],
    hardblank: char,
    layout: HorizontalLayout,
) {
    if canvas.is_empty() || glyph.is_empty() {
        return;
    }

    if matches!(layout, HorizontalLayout::FullWidth) || canvas[0].is_empty() {
        for (row, g_row) in canvas.iter_mut().zip(glyph.iter()) {
            row.push_str(g_row);
        }
        return;
    }

    let allow_smush = matches!(layout, HorizontalLayout::Smushing { .. });
    let rules = match layout {
        HorizontalLayout::Smushing { rules } => rules,
        _ => 0,
    };

    // Compute maximum horizontal overlap allowed across all rows without colliding
    let mut max_overlap: Option<usize> = None;
    for (c_row, g_row) in canvas.iter().zip(glyph.iter()) {
        let c_chars: Vec<char> = c_row.chars().collect();
        let g_chars: Vec<char> = g_row.chars().collect();

        let trail_spaces = c_chars.iter().rev().take_while(|&&c| c == ' ').count();
        let lead_spaces = g_chars.iter().take_while(|&&c| c == ' ').count();

        let mut row_overlap = trail_spaces + lead_spaces;
        if allow_smush && trail_spaces < c_chars.len() && lead_spaces < g_chars.len() {
            let left_ch = c_chars[c_chars.len() - 1 - trail_spaces];
            let right_ch = g_chars[lead_spaces];
            if smush_chars(left_ch, right_ch, hardblank, rules).is_some() {
                row_overlap += 1;
            }
        }
        max_overlap = Some(match max_overlap {
            Some(cur) => cur.min(row_overlap),
            None => row_overlap,
        });
    }

    let overlap = max_overlap.unwrap_or(0);
    if overlap == 0 {
        for (row, g_row) in canvas.iter_mut().zip(glyph.iter()) {
            row.push_str(g_row);
        }
        return;
    }

    for (c_row, g_row) in canvas.iter_mut().zip(glyph.iter()) {
        let mut c_chars: Vec<char> = c_row.chars().collect();
        let g_chars: Vec<char> = g_row.chars().collect();

        // Slide glyph left by `overlap` cells, merging overlapping boundary if smushing
        let c_len = c_chars.len();
        let g_take_start = overlap.min(g_chars.len());
        let merge_start = c_len.saturating_sub(overlap);

        for offset in 0..overlap {
            let ci = merge_start + offset;
            if ci >= c_chars.len() || offset >= g_chars.len() {
                continue;
            }
            let left_ch = c_chars[ci];
            let right_ch = g_chars[offset];
            if left_ch == ' ' {
                c_chars[ci] = right_ch;
            } else if right_ch == ' ' {
                // Keep left_ch
            } else if allow_smush
                && let Some(merged) = smush_chars(left_ch, right_ch, hardblank, rules)
            {
                c_chars[ci] = merged;
            }
        }

        let mut new_row: String = c_chars.into_iter().collect();
        for &ch in g_chars.iter().skip(g_take_start) {
            new_row.push(ch);
        }
        *c_row = new_row;
    }
}

fn smush_chars(left: char, right: char, hardblank: char, rules: u32) -> Option<char> {
    if left == ' ' {
        return Some(right);
    }
    if right == ' ' {
        return Some(left);
    }
    // Universal smushing when no specific rule bits (0..=5) are set
    if rules == 0 {
        if left == hardblank {
            return Some(right);
        }
        if right == hardblank {
            return Some(left);
        }
        return Some(right);
    }

    // Rule 32 (bit 5): Hardblank smushing
    if left == hardblank || right == hardblank {
        return ((rules & 32) != 0 && left == hardblank && right == hardblank).then_some(hardblank);
    }

    // Rule 1 (bit 0): Equal character smushing
    if (rules & 1) != 0 && left == right {
        return Some(left);
    }

    // Rule 2 (bit 1): Underscore smushing
    if (rules & 2) != 0 {
        let replace_set = "|/\\[]{}()<>";
        if left == '_' && replace_set.contains(right) {
            return Some(right);
        }
        if right == '_' && replace_set.contains(left) {
            return Some(left);
        }
    }

    // Rule 4 (bit 2): Hierarchy smushing
    if (rules & 4) != 0 {
        let rank = |c: char| -> Option<u8> {
            match c {
                '|' => Some(0),
                '/' | '\\' => Some(1),
                '[' | ']' => Some(2),
                '{' | '}' => Some(3),
                '(' | ')' => Some(4),
                '<' | '>' => Some(5),
                _ => None,
            }
        };
        if let (Some(r1), Some(r2)) = (rank(left), rank(right))
            && r1 != r2
        {
            return Some(if r1 > r2 { left } else { right });
        }
    }

    // Rule 8 (bit 3): Opposite pair smushing
    if (rules & 8) != 0
        && matches!(
            (left, right),
            ('[', ']') | (']', '[') | ('{', '}') | ('}', '{') | ('(', ')') | (')', '(')
        )
    {
        return Some('|');
    }

    // Rule 16 (bit 4): Big X smushing
    if (rules & 16) != 0 {
        match (left, right) {
            ('/', '\\') => return Some('|'),
            ('\\', '/') => return Some('Y'),
            ('>', '<') => return Some('X'),
            _ => {}
        }
    }

    None
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
            '’' | '‘' => '\'',
            '“' | '”' => '"',
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
    use std::sync::{Mutex, OnceLock};
    static FONT_CACHE: OnceLock<Mutex<HashMap<String, FigletFont>>> = OnceLock::new();

    let requested = name.trim();
    let effective_name = if ascii_fallback {
        match requested.to_ascii_lowercase().as_str() {
            "ansi shadow" | "ansi_shadow" => "Doom",
            "block" => "Standard",
            _ => requested,
        }
    } else {
        requested
    };

    let cache_key = format!(
        "{}:{}",
        effective_name.to_ascii_lowercase(),
        ascii_fallback
    );
    let cache = FONT_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(guard) = cache.lock()
        && let Some(cached) = guard.get(&cache_key)
    {
        return cached.clone();
    }

    if let Some(lua_font) = crate::lua::load_lua_font(effective_name) {
        if let Ok(mut guard) = cache.lock() {
            guard.insert(cache_key, lua_font.clone());
        }
        return lua_font;
    }

    let kind = BuiltinFont::from_name(effective_name).unwrap_or(BuiltinFont::Standard);
    let built = build_builtin_font(kind, ascii_fallback);
    if let Ok(mut guard) = cache.lock() {
        guard.insert(cache_key, built.clone());
    }
    built
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

const SUPPORTED_CHARS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!?.,:-_/+()@'\" ";

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
            BuiltinFont::Small => glyph_small_3x(ch).iter().map(|r| format!("{r} ")).collect(),
            BuiltinFont::Block => render_block_glyph(ch, !ascii_fallback),
            BuiltinFont::Standard => render_outline_glyph(ch),
            BuiltinFont::Slant => render_slant_glyph(ch),
            BuiltinFont::Big => render_big_glyph(ch),
            BuiltinFont::Doom => render_doom_glyph(ch),
            BuiltinFont::AnsiShadow => ansi_shadow_glyph(ch)
                .iter()
                .map(|r| format!("{r} "))
                .collect(),
        };
        glyphs.insert(ch, pad_glyph_uniform(&lines, height));
    }

    FigletFont {
        name: effective.display_name().to_string(),
        height,
        hardblank: '$',
        layout: HorizontalLayout::FullWidth,
        glyphs,
    }
}

/// Hand-crafted, non-overlapping 6-row ANSI Shadow glyphs with clean 1-cell drop shadows.
fn ansi_shadow_glyph(ch: char) -> &'static [&'static str] {
    match ch {
        'A' => &[
            " █████╗ ",
            "██╔══██╗",
            "███████║",
            "██╔══██║",
            "██║  ██║",
            "╚═╝  ╚═╝",
        ],
        'B' => &[
            "██████╗ ",
            "██╔══██╗",
            "██████╔╝",
            "██╔══██╗",
            "██████╔╝",
            "╚═════╝ ",
        ],
        'C' => &[
            " ██████╗",
            "██╔════╝",
            "██║     ",
            "██║     ",
            "╚██████╗",
            " ╚═════╝",
        ],
        'D' => &[
            "██████╗ ",
            "██╔══██╗",
            "██║  ██║",
            "██║  ██║",
            "██████╔╝",
            "╚═════╝ ",
        ],
        'E' => &[
            "███████╗",
            "██╔════╝",
            "█████╗  ",
            "██╔══╝  ",
            "███████╗",
            "╚══════╝",
        ],
        'F' => &[
            "███████╗",
            "██╔════╝",
            "█████╗  ",
            "██╔══╝  ",
            "██║     ",
            "╚═╝     ",
        ],
        'G' => &[
            " ██████╗ ",
            "██╔════╝ ",
            "██║  ███╗",
            "██║   ██║",
            "╚██████╔╝",
            " ╚═════╝ ",
        ],
        'H' => &[
            "██╗  ██╗",
            "██║  ██║",
            "███████║",
            "██╔══██║",
            "██║  ██║",
            "╚═╝  ╚═╝",
        ],
        'I' => &[
            "██╗",
            "██║",
            "██║",
            "██║",
            "██║",
            "╚═╝",
        ],
        'J' => &[
            "     ██╗",
            "     ██║",
            "     ██║",
            "██   ██║",
            "╚█████╔╝",
            " ╚════╝ ",
        ],
        'K' => &[
            "██╗  ██╗",
            "██║ ██╔╝",
            "█████╔╝ ",
            "██╔═██╗ ",
            "██║  ██╗",
            "╚═╝  ╚═╝",
        ],
        'L' => &[
            "██╗     ",
            "██║     ",
            "██║     ",
            "██║     ",
            "███████╗",
            "╚══════╝",
        ],
        'M' => &[
            "███╗   ███╗",
            "████╗ ████║",
            "██╔████╔██║",
            "██║╚██╔╝██║",
            "██║ ╚═╝ ██║",
            "╚═╝     ╚═╝",
        ],
        'N' => &[
            "███╗   ██╗",
            "████╗  ██║",
            "██╔██╗ ██║",
            "██║╚██╗██║",
            "██║ ╚████║",
            "╚═╝  ╚═══╝",
        ],
        'O' => &[
            " ██████╗ ",
            "██╔═══██╗",
            "██║   ██║",
            "██║   ██║",
            "╚██████╔╝",
            " ╚═════╝ ",
        ],
        'P' => &[
            "██████╗ ",
            "██╔══██╗",
            "██████╔╝",
            "██╔═══╝ ",
            "██║     ",
            "╚═╝     ",
        ],
        'Q' => &[
            " ██████╗ ",
            "██╔═══██╗",
            "██║   ██║",
            "██║▄▄ ██║",
            "╚██████╔╝",
            " ╚══▀▀═╝ ",
        ],
        'R' => &[
            "██████╗ ",
            "██╔══██╗",
            "██████╔╝",
            "██╔══██╗",
            "██║  ██║",
            "╚═╝  ╚═╝",
        ],
        'S' => &[
            "███████╗",
            "██╔════╝",
            "███████╗",
            "╚════██║",
            "███████║",
            "╚══════╝",
        ],
        'T' => &[
            "████████╗",
            "╚══██╔══╝",
            "   ██║   ",
            "   ██║   ",
            "   ██║   ",
            "   ╚═╝   ",
        ],
        'U' => &[
            "██╗   ██╗",
            "██║   ██║",
            "██║   ██║",
            "██║   ██║",
            "╚██████╔╝",
            " ╚═════╝ ",
        ],
        'V' => &[
            "██╗   ██╗",
            "██║   ██║",
            "██║   ██║",
            "╚██╗ ██╔╝",
            " ╚████╔╝ ",
            "  ╚═══╝  ",
        ],
        'W' => &[
            "██╗    ██╗",
            "██║    ██║",
            "██║ █╗ ██║",
            "██║███╗██║",
            "╚███╔███╔╝",
            " ╚══╝╚══╝ ",
        ],
        'X' => &[
            "██╗  ██╗",
            "╚██╗██╔╝",
            " ╚███╔╝ ",
            " ██╔██╗ ",
            "██╔╝ ██╗",
            "╚═╝  ╚═╝",
        ],
        'Y' => &[
            "██╗   ██╗",
            "╚██╗ ██╔╝",
            " ╚████╔╝ ",
            "  ╚██╔╝  ",
            "   ██║   ",
            "   ╚═╝   ",
        ],
        'Z' => &[
            "███████╗",
            "╚══███╔╝",
            "  ███╔╝ ",
            " ███╔╝  ",
            "███████╗",
            "╚══════╝",
        ],
        '0' => &[
            " ██████╗ ",
            "██╔═████╗",
            "██║██╔██║",
            "████╔╝██║",
            "╚██████╔╝",
            " ╚═════╝ ",
        ],
        '1' => &[
            " ██╗",
            "███║",
            "╚██║",
            " ██║",
            " ██║",
            " ╚═╝",
        ],
        '2' => &[
            "██████╗ ",
            "╚════██╗",
            " █████╔╝",
            "██╔═══╝ ",
            "███████╗",
            "╚══════╝",
        ],
        '3' => &[
            "██████╗ ",
            "╚════██╗",
            " █████╔╝",
            " ╚═══██╗",
            "██████╔╝",
            "╚═════╝ ",
        ],
        '4' => &[
            "██╗  ██╗",
            "██║  ██║",
            "███████║",
            "╚════██║",
            "     ██║",
            "     ╚═╝",
        ],
        '5' => &[
            "███████╗",
            "██╔════╝",
            "███████╗",
            "╚════██║",
            "███████║",
            "╚══════╝",
        ],
        '6' => &[
            " ██████╗ ",
            "██╔════╝ ",
            "███████╗ ",
            "██╔═══██╗",
            "╚██████╔╝",
            " ╚═════╝ ",
        ],
        '7' => &[
            "███████╗",
            "╚════██║",
            "    ██╔╝",
            "   ██╔╝ ",
            "   ██║  ",
            "   ╚═╝  ",
        ],
        '8' => &[
            " █████╗ ",
            "██╔══██╗",
            "╚█████╔╝",
            "██╔══██╗",
            "╚█████╔╝",
            " ╚════╝ ",
        ],
        '9' => &[
            " █████╗ ",
            "██╔══██╗",
            "╚██████║",
            " ╚═══██║",
            " █████╔╝",
            " ╚════╝ ",
        ],
        '!' => &[
            "██╗",
            "██║",
            "██║",
            "╚═╝",
            "██╗",
            "╚═╝",
        ],
        '?' => &[
            "██████╗ ",
            "╚════██╗",
            "  ███╔═╝",
            "  ╚══╝  ",
            "  ██╗   ",
            "  ╚═╝   ",
        ],
        '.' => &[
            "   ",
            "   ",
            "   ",
            "   ",
            "██╗",
            "╚═╝",
        ],
        ',' => &[
            "   ",
            "   ",
            "   ",
            "██╗",
            "╚██╗",
            " ╚═╝",
        ],
        ':' => &[
            "   ",
            "██╗",
            "╚═╝",
            "██╗",
            "╚═╝",
            "   ",
        ],
        '-' => &[
            "      ",
            "      ",
            "█████╗",
            "╚════╝",
            "      ",
            "      ",
        ],
        '_' => &[
            "        ",
            "        ",
            "        ",
            "        ",
            "███████╗",
            "╚══════╝",
        ],
        '\'' => &[
            "██╗",
            "╚██╗",
            " ╚═╝",
            "   ",
            "   ",
            "   ",
        ],
        '"' => &[
            "██╗██╗",
            "╚═╝╚═╝",
            "      ",
            "      ",
            "      ",
            "      ",
        ],
        '/' => &[
            "   ██╗",
            "  ██╔╝",
            " ██╔╝ ",
            "██╔╝  ",
            "╚═╝   ",
            "      ",
        ],
        '+' => &[
            "      ",
            "  ██╗ ",
            "██████╗",
            "╚═██╔═╝",
            "  ╚═╝ ",
            "      ",
        ],
        '(' => &[
            " ██╗",
            "██╔╝",
            "██║ ",
            "██║ ",
            "╚██╗",
            " ╚═╝",
        ],
        ')' => &[
            "██╗ ",
            "╚██╗",
            " ██║",
            " ██║",
            "██╔╝",
            "╚═╝ ",
        ],
        '@' => &[
            " ██████╗ ",
            "██╔══███╗",
            "██║██╔██║",
            "██║╚═══╝ ",
            "╚██████╗ ",
            " ╚═════╝ ",
        ],
        ' ' => &[
            "   ",
            "   ",
            "   ",
            "   ",
            "   ",
            "   ",
        ],
        _ => &[
            "   ",
            "   ",
            "   ",
            "   ",
            "   ",
            "   ",
        ],
    }
}

fn glyph_bitmap_5x(ch: char) -> &'static [&'static str] {
    match ch {
        'A' => &[".###.", "#...#", "#####", "#...#", "#...#"],
        'B' => &["####.", "#...#", "####.", "#...#", "####."],
        'C' => &[".####", "#....", "#....", "#....", ".####"],
        'D' => &["####.", "#...#", "#...#", "#...#", "####."],
        'E' => &["#####", "#....", "####.", "#....", "#####"],
        'F' => &["#####", "#....", "####.", "#....", "#...."],
        'G' => &[".####", "#....", "#..##", "#...#", ".####"],
        'H' => &["#...#", "#...#", "#####", "#...#", "#...#"],
        'I' => &["###", ".#.", ".#.", ".#.", "###"],
        'J' => &["..###", "...#.", "...#.", "#..#.", ".##.."],
        'K' => &["#...#", "#..#.", "###..", "#..#.", "#...#"],
        'L' => &["#....", "#....", "#....", "#....", "#####"],
        'M' => &["#...#", "##.##", "#.#.#", "#...#", "#...#"],
        'N' => &["#...#", "##..#", "#.#.#", "#..##", "#...#"],
        'O' => &[".###.", "#...#", "#...#", "#...#", ".###."],
        'P' => &["####.", "#...#", "####.", "#....", "#...."],
        'Q' => &[".###.", "#...#", "#.#.#", "#..#.", ".##.#"],
        'R' => &["####.", "#...#", "####.", "#..#.", "#...#"],
        'S' => &[".####", "#....", ".###.", "....#", "####."],
        'T' => &["#####", "..#..", "..#..", "..#..", "..#.."],
        'U' => &["#...#", "#...#", "#...#", "#...#", ".###."],
        'V' => &["#...#", "#...#", "#...#", ".#.#.", "..#.."],
        'W' => &["#...#", "#...#", "#.#.#", "##.##", "#...#"],
        'X' => &["#...#", ".#.#.", "..#..", ".#.#.", "#...#"],
        'Y' => &["#...#", ".#.#.", "..#..", "..#..", "..#.."],
        'Z' => &["#####", "...#.", "..#..", ".#...", "#####"],
        '0' => &[".###.", "#..##", "#.#.#", "##..#", ".###."],
        '1' => &[".#.", "##.", ".#.", ".#.", "###"],
        '2' => &["####.", "....#", ".###.", "#....", "#####"],
        '3' => &["####.", "....#", ".###.", "....#", "####."],
        '4' => &["#...#", "#...#", "#####", "....#", "....#"],
        '5' => &["#####", "#....", "####.", "....#", "####."],
        '6' => &[".###.", "#....", "####.", "#...#", ".###."],
        '7' => &["#####", "...#.", "..#..", ".#...", ".#..."],
        '8' => &[".###.", "#...#", ".###.", "#...#", ".###."],
        '9' => &[".###.", "#...#", ".####", "....#", ".###."],
        '!' => &["#", "#", "#", ".", "#"],
        '?' => &[".###.", "#...#", "..##.", ".....", "..#.."],
        ',' => &[".", ".", ".", "#", "#"],
        '.' => &[".", ".", ".", ".", "#"],
        ':' => &[".", "#", ".", "#", "."],
        '-' => &["...", "...", "###", "...", "..."],
        '_' => &["....", "....", "....", "....", "####"],
        '\'' => &["#", "#", ".", ".", "."],
        '"' => &["#.#", "#.#", "...", "...", "..."],
        '/' => &["...#", "..#.", ".#..", "#...", "...."],
        '+' => &["...", ".#.", "###", ".#.", "..."],
        '(' => &[".#", "#.", "#.", "#.", ".#"],
        ')' => &["#.", ".#", ".#", ".#", "#."],
        '@' => &[".###.", "#.#.#", "#.##.", "#....", ".####"],
        _ => &["...", "...", "...", "...", "..."],
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
        '\'' => &["'", " ", " "],
        '"' => &["\"", " ", " "],
        ' ' => &["  ", "  ", "  "],
        _ => &["  ", "  ", "  "],
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

fn render_outline_glyph(ch: char) -> Vec<String> {
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
                    _ => '#',
                };
                line.push(ch_out);
            } else {
                line.push(' ');
            }
        }
        line.push(' ');
        rows.push(line);
    }
    rows
}

fn render_slant_glyph(ch: char) -> Vec<String> {
    let base = render_outline_glyph(ch);
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
