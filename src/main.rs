use anyhow::Result;
use clap::{Parser, ValueEnum};
use crossterm::terminal;
use egotop::app::App;
use egotop::config::loader::DEFAULT_CONFIG_TOML;
use egotop::config::{EffectKind, StartupMode, load_or_create};
use std::path::PathBuf;
use std::process;

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliStartup {
    Normal,
    Cursor,
}

impl From<CliStartup> for StartupMode {
    fn from(value: CliStartup) -> Self {
        match value {
            CliStartup::Normal => Self::Normal,
            CliStartup::Cursor => Self::Cursor,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliEffect {
    Matrix,
    Water,
    Particles,
    Scanlines,
    None,
}

impl From<CliEffect> for EffectKind {
    fn from(value: CliEffect) -> Self {
        match value {
            CliEffect::Matrix => Self::Matrix,
            CliEffect::Water => Self::Water,
            CliEffect::Particles => Self::Particles,
            CliEffect::Scanlines => Self::Scanlines,
            CliEffect::None => Self::None,
        }
    }
}

#[derive(Debug, Parser)]
#[command(
    name = "egotop",
    version,
    about = "Interactive animated terminal welcome dashboard for Linux & Hyprland"
)]
struct Cli {
    /// Path to TOML configuration file (defaults to ~/.config/egotop/egotop.toml)
    #[arg(short, long, value_name = "PATH")]
    config: Option<PathBuf>,

    /// Override background animation effect for this session
    #[arg(short, long, value_enum)]
    effect: Option<CliEffect>,

    /// Initial page to display (1..=3)
    #[arg(short, long, value_parser = clap::value_parser!(u8).range(1..=3))]
    page: Option<u8>,

    /// Startup presentation mode (overrides TOML [startup].mode)
    #[arg(long, value_enum)]
    startup: Option<CliStartup>,

    /// Validate configuration file and exit
    #[arg(long)]
    check_config: bool,

    /// Print the default TOML configuration to stdout and exit
    #[arg(long)]
    print_default_config: bool,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("egotop error: {err:#}");
        process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    if cli.print_default_config {
        print!("{DEFAULT_CONFIG_TOML}");
        return Ok(());
    }

    let loaded = load_or_create(cli.config.as_deref())?;
    let mut config = loaded.config;

    if cli.check_config {
        println!(
            "Configuration OK: {} (theme='{}', effect='{:?}', widgets={})",
            loaded.path.display(),
            config.theme.name,
            config.background.effect,
            config.widgets.len()
        );
        return Ok(());
    }

    if let Some(eff) = cli.effect {
        let kind: EffectKind = eff.into();
        config.background.effect = kind;
        config.background.enabled = kind != EffectKind::None;
    }

    if let Some(p) = cli.page {
        config.general.default_page = p;
    }

    if let Some(st) = cli.startup {
        config.startup.mode = st.into();
    }

    let toast = loaded
        .created_default
        .then(|| format!("Default configuration created at {}", loaded.path.display()));

    let (cols, rows) = terminal::size().unwrap_or((120, 36));
    let mut app = App::new(config, loaded.path, toast, cols, rows);
    app.run()
}
