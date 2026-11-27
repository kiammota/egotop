use std::env;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorCapability {
    Truecolor,
    Color256,
    Color16,
}

impl ColorCapability {
    pub fn label(self) -> &'static str {
        match self {
            Self::Truecolor => "24-bit Truecolor",
            Self::Color256 => "256 colors",
            Self::Color16 => "16 colors",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckSeverity {
    Info,
    Warning,
    Error,
}

impl CheckSeverity {
    pub fn tag(self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Warning => "WARN",
            Self::Error => "ERR",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticFinding {
    pub severity: CheckSeverity,
    pub label: String,
    pub detail: String,
}

#[derive(Debug, Clone)]
pub struct TerminalDiagnostics {
    pub color_capability: ColorCapability,
    pub utf8_supported: bool,
    pub unicode_supported: bool,
    pub term: Option<String>,
    pub colorterm: Option<String>,
    pub width: u16,
    pub height: u16,
    pub findings: Vec<DiagnosticFinding>,
}

impl TerminalDiagnostics {
    pub fn detect(width: u16, height: u16) -> Self {
        let term = env::var("TERM").ok().filter(|s| !s.trim().is_empty());
        let colorterm = env::var("COLORTERM").ok().filter(|s| !s.trim().is_empty());
        let locale = env::var("LC_ALL")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| env::var("LC_CTYPE").ok().filter(|s| !s.trim().is_empty()))
            .or_else(|| env::var("LANG").ok().filter(|s| !s.trim().is_empty()));

        Self::detect_from_env(
            width,
            height,
            term,
            colorterm,
            locale,
            env::var_os("KITTY_WINDOW_ID").is_some()
                || env::var_os("WEZTERM_PANE").is_some()
                || env::var_os("WT_SESSION").is_some(),
        )
    }

    pub fn detect_from_env(
        width: u16,
        height: u16,
        term: Option<String>,
        colorterm: Option<String>,
        locale: Option<String>,
        known_truecolor_terminal: bool,
    ) -> Self {
        let mut findings = Vec::new();

        let term_low = term.as_deref().unwrap_or("").to_ascii_lowercase();
        let colorterm_low = colorterm.as_deref().unwrap_or("").to_ascii_lowercase();

        let color_capability = if colorterm_low.contains("truecolor")
            || colorterm_low.contains("24bit")
            || known_truecolor_terminal
        {
            findings.push(DiagnosticFinding {
                severity: CheckSeverity::Info,
                label: "Color".to_string(),
                detail: "24-bit Truecolor supported".to_string(),
            });
            ColorCapability::Truecolor
        } else if term_low.contains("256color") {
            findings.push(DiagnosticFinding {
                severity: CheckSeverity::Warning,
                label: "Color".to_string(),
                detail: "256 colors only (Truecolor unavailable, adapting palette)".to_string(),
            });
            ColorCapability::Color256
        } else {
            findings.push(DiagnosticFinding {
                severity: CheckSeverity::Warning,
                label: "Color".to_string(),
                detail: "16 colors fallback (set COLORTERM=truecolor for full fidelity)"
                    .to_string(),
            });
            ColorCapability::Color16
        };

        let locale_low = locale.as_deref().unwrap_or("").to_ascii_lowercase();
        let utf8_supported = locale_low.contains("utf-8") || locale_low.contains("utf8");
        let unicode_supported = utf8_supported && term_low != "dumb" && term_low != "linux";

        if !utf8_supported {
            findings.push(DiagnosticFinding {
                severity: CheckSeverity::Warning,
                label: "Encoding".to_string(),
                detail: "UTF-8 locale not detected (enabling ASCII fallback)".to_string(),
            });
        } else if !unicode_supported {
            findings.push(DiagnosticFinding {
                severity: CheckSeverity::Warning,
                label: "Unicode".to_string(),
                detail: format!("Limited Unicode in TERM='{}' (enabling ASCII fallback)", term.as_deref().unwrap_or("unknown")),
            });
        } else {
            findings.push(DiagnosticFinding {
                severity: CheckSeverity::Info,
                label: "Unicode".to_string(),
                detail: "UTF-8 & Unicode supported".to_string(),
            });
        }

        if width == 0 || height == 0 {
            findings.push(DiagnosticFinding {
                severity: CheckSeverity::Error,
                label: "Dimensions".to_string(),
                detail: format!("Invalid terminal dimensions {width}x{height}"),
            });
        } else if width < 40 || height < 12 {
            findings.push(DiagnosticFinding {
                severity: CheckSeverity::Warning,
                label: "Dimensions".to_string(),
                detail: format!("Small terminal {width}x{height} (recommended >= 80x24)"),
            });
        } else {
            findings.push(DiagnosticFinding {
                severity: CheckSeverity::Info,
                label: "Dimensions".to_string(),
                detail: format!("{width}x{height}"),
            });
        }

        Self {
            color_capability,
            utf8_supported,
            unicode_supported,
            term,
            colorterm,
            width,
            height,
            findings,
        }
    }

    pub fn should_use_ascii_fallback(&self) -> bool {
        !self.utf8_supported || !self.unicode_supported
    }

    pub fn should_adapt_colors(&self) -> bool {
        self.color_capability != ColorCapability::Truecolor
    }

    pub fn warning_count(&self) -> usize {
        self.findings
            .iter()
            .filter(|f| f.severity == CheckSeverity::Warning)
            .count()
    }

    pub fn error_count(&self) -> usize {
        self.findings
            .iter()
            .filter(|f| f.severity == CheckSeverity::Error)
            .count()
    }

    pub fn summary(&self) -> String {
        let errs = self.error_count();
        let warns = self.warning_count();
        let term_s = self.term.as_deref().unwrap_or("unset");
        let ct_s = self.colorterm.as_deref().unwrap_or("unset");
        let uni = if self.unicode_supported {
            "UTF-8/Unicode"
        } else {
            "ASCII fallback"
        };
        format!(
            "[{} ERR, {} WARN] {} | {} | {}x{} | TERM={} COLORTERM={}",
            errs,
            warns,
            self.color_capability.label(),
            uni,
            self.width,
            self.height,
            term_s,
            ct_s
        )
    }
}
