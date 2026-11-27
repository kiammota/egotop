use std::env;
use std::ffi::CStr;
use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct DisplayInfo {
    pub name: String,
    pub resolution: Option<String>,
    pub refresh_hz: Option<f32>,
    pub size_inches: Option<f32>,
    pub is_internal: bool,
}

#[derive(Debug, Clone)]
pub struct OsInfo {
    pub distro_name: Option<String>,
    pub distro_id: Option<String>,
    pub kernel: Option<String>,
    pub arch: Option<String>,
    pub hostname: Option<String>,
    pub host_model: Option<String>,
    pub username: Option<String>,
    pub shell: Option<String>,
    pub terminal: Option<String>,
    pub terminal_font: Option<String>,
    pub packages: Vec<(String, usize)>,
    pub locale: Option<String>,
    pub gtk_theme: Option<String>,
    pub icons: Option<String>,
    pub ui_font: Option<String>,
    pub cursor_theme: Option<String>,
    pub displays: Vec<DisplayInfo>,
}

pub fn collect_static_os_info() -> OsInfo {
    let (distro_name, distro_id) = read_os_release();
    let (kernel, arch, hostname) = read_uname();
    let host_model = read_host_model();
    let username = detect_username();
    let shell = detect_shell();
    let terminal = detect_terminal();
    let terminal_font = detect_terminal_font(terminal.as_deref());
    let packages = collect_package_counts();
    let locale = detect_locale();
    let (gtk_theme, icons, ui_font, cursor_theme) = detect_gtk_settings();
    let displays = collect_drm_displays();

    OsInfo {
        distro_name,
        distro_id,
        kernel,
        arch,
        hostname,
        host_model,
        username,
        shell,
        terminal,
        terminal_font,
        packages,
        locale,
        gtk_theme,
        icons,
        ui_font,
        cursor_theme,
        displays,
    }
}

pub fn parse_os_release(content: &str) -> (Option<String>, Option<String>) {
    let mut pretty = None;
    let mut name = None;
    let mut version = None;
    let mut id = None;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let val = v.trim().trim_matches('"').trim_matches('\'').to_string();
        match k.trim() {
            "PRETTY_NAME" if !val.is_empty() => pretty = Some(val),
            "NAME" if !val.is_empty() => name = Some(val),
            "VERSION_ID" if !val.is_empty() => version = Some(val),
            "ID" if !val.is_empty() => id = Some(val),
            _ => {}
        }
    }

    let display = pretty.or_else(|| match (name, version) {
        (Some(n), Some(v)) => Some(format!("{n} {v}")),
        (Some(n), None) => Some(n),
        _ => None,
    });

    (display, id)
}

fn read_os_release() -> (Option<String>, Option<String>) {
    if let Ok(content) = fs::read_to_string("/etc/os-release") {
        return parse_os_release(&content);
    }
    if let Ok(content) = fs::read_to_string("/usr/lib/os-release") {
        return parse_os_release(&content);
    }
    (None, None)
}

fn read_uname() -> (Option<String>, Option<String>, Option<String>) {
    let mut uts: libc::utsname = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::uname(&mut uts) };
    if rc != 0 {
        return (None, None, None);
    }
    let cstr = |buf: &[libc::c_char]| -> Option<String> {
        let s = unsafe { CStr::from_ptr(buf.as_ptr()) }
            .to_string_lossy()
            .trim()
            .to_string();
        if s.is_empty() { None } else { Some(s) }
    };
    (
        cstr(&uts.release),
        cstr(&uts.machine),
        cstr(&uts.nodename),
    )
}

fn detect_username() -> Option<String> {
    if let Ok(u) = env::var("USER") {
        let t = u.trim();
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }
    if let Ok(u) = env::var("LOGNAME") {
        let t = u.trim();
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }
    let uid = unsafe { libc::geteuid() };
    let pwd = unsafe { libc::getpwuid(uid) };
    if !pwd.is_null() {
        let name_ptr = unsafe { (*pwd).pw_name };
        if !name_ptr.is_null() {
            let name = unsafe { CStr::from_ptr(name_ptr) }
                .to_string_lossy()
                .trim()
                .to_string();
            if !name.is_empty() {
                return Some(name);
            }
        }
    }
    None
}

fn detect_shell() -> Option<String> {
    let shell_path = env::var("SHELL").ok()?;
    let bin = Path::new(&shell_path)
        .file_name()
        .and_then(|s| s.to_str())?
        .to_string();

    let version = probe_version_once(&shell_path, "--version")
        .and_then(|out| extract_shell_version(&bin, &out));

    Some(match version {
        Some(v) => format!("{bin} {v}"),
        None => bin,
    })
}

fn extract_shell_version(bin: &str, raw: &str) -> Option<String> {
    let first_line = raw.lines().next()?.trim();
    if bin.contains("zsh") {
        // e.g., "zsh 5.9 (x86_64-pc-linux-gnu)"
        return first_line
            .split_whitespace()
            .nth(1)
            .map(ToString::to_string);
    }
    if bin.contains("fish") {
        // e.g., "fish, version 3.7.1"
        return first_line
            .split("version")
            .nth(1)
            .map(|s| s.trim().to_string());
    }
    if bin.contains("bash") {
        // e.g., "GNU bash, version 5.2.37(1)-release (...)"
        if let Some(rest) = first_line.split("version").nth(1) {
            let ver = rest.split_whitespace().next().unwrap_or("");
            let clean = ver.split('(').next().unwrap_or(ver).trim();
            if !clean.is_empty() {
                return Some(clean.to_string());
            }
        }
    }
    first_line
        .split_whitespace()
        .find(|w| w.chars().next().is_some_and(|c| c.is_ascii_digit()))
        .map(ToString::to_string)
}

fn detect_terminal() -> Option<String> {
    if env::var_os("KITTY_WINDOW_ID").is_some() || env::var("TERM").ok().as_deref() == Some("xterm-kitty") {
        let ver = probe_terminal_version("kitty");
        return Some(match ver {
            Some(v) => format!("Kitty {v}"),
            None => "Kitty".to_string(),
        });
    }
    if env::var_os("WEZTERM_PANE").is_some()
        || env::var("TERM_PROGRAM").ok().as_deref() == Some("WezTerm")
    {
        let ver = env::var("TERM_PROGRAM_VERSION")
            .ok()
            .or_else(|| probe_terminal_version("wezterm"));
        return Some(match ver {
            Some(v) => format!("WezTerm {v}"),
            None => "WezTerm".to_string(),
        });
    }
    if env::var_os("ALACRITTY_SOCKET").is_some()
        || env::var_os("ALACRITTY_LOG").is_some()
        || env::var("TERM").ok().as_deref() == Some("alacritty")
    {
        let ver = probe_terminal_version("alacritty");
        return Some(match ver {
            Some(v) => format!("Alacritty {v}"),
            None => "Alacritty".to_string(),
        });
    }
    if env::var("TERM").ok().as_deref() == Some("foot")
        || env::var("TERM").ok().as_deref() == Some("foot-extra")
    {
        let ver = probe_terminal_version("foot");
        return Some(match ver {
            Some(v) => format!("foot {v}"),
            None => "foot".to_string(),
        });
    }

    if let Ok(tp) = env::var("TERM_PROGRAM")
        && !tp.trim().is_empty()
    {
        let ver = env::var("TERM_PROGRAM_VERSION").ok();
        return Some(match ver {
            Some(v) if !v.trim().is_empty() => format!("{} {}", tp.trim(), v.trim()),
            _ => tp.trim().to_string(),
        });
    }

    if let Some(anc) = detect_terminal_from_proc_ancestors() {
        return Some(anc);
    }

    env::var("TERM").ok().and_then(|t| {
        let t = t.trim().to_string();
        if t.is_empty() || t == "dumb" {
            None
        } else {
            Some(t)
        }
    })
}

fn probe_terminal_version(bin: &str) -> Option<String> {
    let out = probe_version_once(bin, "--version")?;
    let line = out.lines().next()?.trim();
    line.split_whitespace()
        .find(|tok| tok.chars().next().is_some_and(|c| c.is_ascii_digit()))
        .map(|s| s.trim_end_matches(',').to_string())
}

fn probe_version_once(program: &str, arg: &str) -> Option<String> {
    let output = Command::new(program).arg(arg).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.is_empty() { None } else { Some(stderr) }
    } else {
        Some(stdout)
    }
}

fn detect_terminal_from_proc_ancestors() -> Option<String> {
    let known = [
        ("kitty", "Kitty"),
        ("alacritty", "Alacritty"),
        ("wezterm-gui", "WezTerm"),
        ("foot", "foot"),
        ("ghostty", "Ghostty"),
        ("konsole", "Konsole"),
        ("gnome-terminal", "GNOME Terminal"),
        ("xfce4-terminal", "xfce4-terminal"),
        ("urxvt", "urxvt"),
        ("st", "st"),
        ("tmux", "tmux"),
    ];

    let mut pid = std::process::id();
    for _ in 0..10 {
        let stat_path = format!("/proc/{pid}/stat");
        let stat = fs::read_to_string(&stat_path).ok()?;
        let rparen = stat.rfind(')')?;
        let comm = stat
            .get(stat.find('(')? + 1..rparen)
            .unwrap_or("")
            .to_string();
        for (needle, label) in known {
            if comm.eq_ignore_ascii_case(needle) {
                return Some(label.to_string());
            }
        }
        let rest = stat.get(rparen + 2..)?;
        let mut fields = rest.split_whitespace();
        let _state = fields.next()?;
        let ppid: u32 = fields.next()?.parse().ok()?;
        if ppid <= 1 || ppid == pid {
            break;
        }
        pid = ppid;
    }
    None
}

fn read_host_model() -> Option<String> {
    let read_clean = |file: &str| -> Option<String> {
        let s = fs::read_to_string(format!("/sys/devices/virtual/dmi/id/{file}")).ok()?;
        let t = s.trim();
        if t.is_empty()
            || t.eq_ignore_ascii_case("To Be Filled By O.E.M.")
            || t.eq_ignore_ascii_case("Default string")
            || t.eq_ignore_ascii_case("System Product Name")
            || t.eq_ignore_ascii_case("Not Specified")
        {
            None
        } else {
            Some(t.to_string())
        }
    };
    match (read_clean("board_vendor"), read_clean("board_name")) {
        (Some(v), Some(n)) => Some(format!("{v} {n}")),
        (None, Some(n)) => Some(n),
        _ => read_clean("product_name"),
    }
}

fn collect_package_counts() -> Vec<(String, usize)> {
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir("/var/lib/pacman/local") {
        let count = entries
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|ft| ft.is_dir()))
            .count();
        if count > 0 {
            out.push(("pacman".to_string(), count));
        }
    }
    let mut flatpak_count = 0usize;
    if let Ok(entries) = fs::read_dir("/var/lib/flatpak/app") {
        flatpak_count += entries.flatten().count();
    }
    if let Some(home) = dirs::home_dir()
        && let Ok(entries) = fs::read_dir(home.join(".local/share/flatpak/app"))
    {
        flatpak_count += entries.flatten().count();
    }
    if flatpak_count > 0 {
        out.push(("flatpak".to_string(), flatpak_count));
    }
    if out.is_empty()
        && let Ok(status) = fs::read_to_string("/var/lib/dpkg/status")
    {
        let count = status
            .lines()
            .filter(|l| l.starts_with("Package: "))
            .count();
        if count > 0 {
            out.push(("dpkg".to_string(), count));
        }
    }
    out
}

fn detect_locale() -> Option<String> {
    for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(val) = env::var(var) {
            let t = val.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    if let Ok(raw) = fs::read_to_string("/etc/locale.conf") {
        for line in raw.lines() {
            if let Some(val) = line.strip_prefix("LANG=") {
                let clean = val.trim().trim_matches('"');
                if !clean.is_empty() {
                    return Some(clean.to_string());
                }
            }
        }
    }
    None
}

fn detect_gtk_settings() -> (Option<String>, Option<String>, Option<String>, Option<String>) {
    let mut gtk_theme = env::var("GTK_THEME").ok().filter(|s| !s.trim().is_empty());
    let mut icons = None;
    let mut ui_font = None;
    let mut cursor_theme = env::var("XCURSOR_THEME")
        .ok()
        .filter(|s| !s.trim().is_empty());
    let cursor_size = env::var("XCURSOR_SIZE").ok().filter(|s| !s.trim().is_empty());

    if let Some(cfg_dir) = dirs::config_dir() {
        for sub in ["gtk-3.0/settings.ini", "gtk-4.0/settings.ini"] {
            if let Ok(raw) = fs::read_to_string(cfg_dir.join(sub)) {
                for line in raw.lines() {
                    let Some((k, v)) = line.split_once('=') else {
                        continue;
                    };
                    let val = v.trim();
                    if val.is_empty() {
                        continue;
                    }
                    match k.trim() {
                        "gtk-theme-name" if gtk_theme.is_none() => {
                            gtk_theme = Some(val.to_string());
                        }
                        "gtk-icon-theme-name" if icons.is_none() => {
                            icons = Some(val.to_string());
                        }
                        "gtk-font-name" if ui_font.is_none() => {
                            ui_font = Some(val.to_string());
                        }
                        "gtk-cursor-theme-name" if cursor_theme.is_none() => {
                            cursor_theme = Some(val.to_string());
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    if let (Some(ct), Some(sz)) = (&cursor_theme, &cursor_size)
        && !ct.contains('(')
    {
        cursor_theme = Some(format!("{ct} ({sz}px)"));
    }
    (gtk_theme, icons, ui_font, cursor_theme)
}

fn detect_terminal_font(terminal: Option<&str>) -> Option<String> {
    let cfg_dir = dirs::config_dir()?;
    let term_low = terminal.unwrap_or("").to_ascii_lowercase();
    if term_low.contains("kitty")
        && let Ok(raw) = fs::read_to_string(cfg_dir.join("kitty/kitty.conf"))
    {
        let mut family = None;
        let mut size = None;
        for line in raw.lines() {
            let t = line.trim();
            if t.starts_with('#') {
                continue;
            }
            if let Some(rest) = t.strip_prefix("font_family") {
                let v = rest.trim();
                if !v.is_empty() {
                    family = Some(v.to_string());
                }
            } else if let Some(rest) = t.strip_prefix("font_size") {
                let v = rest.trim();
                if !v.is_empty() {
                    size = Some(v.to_string());
                }
            }
        }
        return match (family, size) {
            (Some(f), Some(s)) => Some(format!("{f} ({s}pt)")),
            (Some(f), None) => Some(f),
            _ => None,
        };
    }
    if term_low.contains("alacritty")
        && let Ok(raw) = fs::read_to_string(cfg_dir.join("alacritty/alacritty.toml"))
    {
        for line in raw.lines() {
            if let Some((k, v)) = line.split_once('=')
                && k.trim() == "family"
            {
                return Some(v.trim().trim_matches('"').to_string());
            }
        }
    }
    None
}

fn collect_drm_displays() -> Vec<DisplayInfo> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir("/sys/class/drm") else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("card") || !name.contains('-') {
            continue;
        }
        let status = fs::read_to_string(path.join("status")).unwrap_or_default();
        if status.trim() != "connected" {
            continue;
        }
        let resolution = fs::read_to_string(path.join("modes"))
            .ok()
            .and_then(|m| m.lines().next().map(|l| l.trim().to_string()))
            .filter(|s| !s.is_empty());
        let connector = name.split_once('-').map(|(_, c)| c).unwrap_or(&name);
        let is_internal = connector.starts_with("eDP") || connector.starts_with("LVDS");
        out.push(DisplayInfo {
            name: connector.to_string(),
            resolution,
            refresh_hz: None,
            size_inches: None,
            is_internal,
        });
    }
    out
}
