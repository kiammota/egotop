use std::env;

#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub desktop_env: Option<String>,
    pub window_manager: Option<String>,
    pub session_type: Option<String>,
    pub display: Option<String>,
    pub terminal_cols: u16,
    pub terminal_rows: u16,
    pub is_hyprland: bool,
}

pub fn collect_session_info(terminal_cols: u16, terminal_rows: u16) -> SessionInfo {
    let desktop_env = non_empty_env("XDG_CURRENT_DESKTOP")
        .or_else(|| non_empty_env("DESKTOP_SESSION"));

    let is_hyprland = env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some()
        || desktop_env
            .as_deref()
            .is_some_and(|d| d.to_ascii_lowercase().contains("hyprland"));

    let window_manager = if is_hyprland {
        Some("Hyprland".to_string())
    } else if env::var_os("SWAYSOCK").is_some() {
        Some("Sway".to_string())
    } else if env::var_os("I3SOCK").is_some() {
        Some("i3".to_string())
    } else {
        desktop_env.clone()
    };

    let wayland_disp = non_empty_env("WAYLAND_DISPLAY");
    let x11_disp = non_empty_env("DISPLAY");

    let session_type = non_empty_env("XDG_SESSION_TYPE")
        .map(|s| normalize_session_type(&s))
        .or_else(|| {
            if wayland_disp.is_some() {
                Some("Wayland".to_string())
            } else if x11_disp.is_some() {
                Some("X11".to_string())
            } else {
                None
            }
        });

    let display = wayland_disp.or(x11_disp);

    SessionInfo {
        desktop_env,
        window_manager,
        session_type,
        display,
        terminal_cols,
        terminal_rows,
        is_hyprland,
    }
}

fn normalize_session_type(raw: &str) -> String {
    match raw.trim().to_ascii_lowercase().as_str() {
        "wayland" => "Wayland".to_string(),
        "x11" => "X11".to_string(),
        "tty" => "TTY".to_string(),
        other => other.to_string(),
    }
}

fn non_empty_env(name: &str) -> Option<String> {
    env::var(name).ok().and_then(|v| {
        let trimmed = v.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}
