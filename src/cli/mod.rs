pub mod config;
pub mod context;
pub mod doctor;
pub mod fonts;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use crossterm::terminal;
use egotop_app_launch::launch_tui;
use std::path::PathBuf;

use crate::config::{EffectKind, StartupMode, load_or_create};
use crate::lua::builtins::DEFAULT_EGOTOP_LUA;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliStartup {
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
pub enum CliEffect {
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

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Inspect, locate, show, or validate configuration
    Config(config::ConfigArgs),
    /// List available built-in and custom FIGlet fonts
    Fonts(fonts::FontsArgs),
    /// List declared runtime contexts
    Context(context::ContextArgs),
    /// Verify installation, configuration, and runtime environment health
    Doctor,
}

#[derive(Debug, Parser)]
#[command(
    name = "egotop",
    version,
    about = "Interactive animated terminal welcome dashboard for Linux & Hyprland"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Path to configuration file (defaults to ~/.config/egotop/egotop.lua)
    #[arg(short, long, global = true, value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// Override background animation effect for this session
    #[arg(short, long, value_enum)]
    pub effect: Option<CliEffect>,

    /// Initial page to display (1..=3)
    #[arg(short, long, value_parser = clap::value_parser!(u8).range(1..=3))]
    pub page: Option<u8>,

    /// Startup presentation mode (overrides TOML [startup].mode)
    #[arg(long, value_enum)]
    pub startup: Option<CliStartup>,

    /// Validate configuration file and exit
    #[arg(long)]
    pub check_config: bool,

    /// Print the default Lua configuration to stdout and exit
    #[arg(long)]
    pub print_default_config: bool,
}

/// Executes the CLI, returning an explicit process exit code cleanly.
pub fn run() -> Result<i32> {
    let cli = Cli::parse();

    if cli.print_default_config {
        print!("{DEFAULT_EGOTOP_LUA}");
        return Ok(0);
    }

    if let Some(cmd) = cli.command {
        return match cmd {
            Commands::Config(args) => config::handle(args, cli.config.as_deref()),
            Commands::Fonts(args) => fonts::handle(args),
            Commands::Context(args) => context::handle(args),
            Commands::Doctor => doctor::handle(cli.config.as_deref()),
        };
    }

    if cli.check_config {
        let target = config::resolve_config_path(cli.config.as_deref());
        return config::validate_command(Some(&target));
    }

    launch_tui(cli)
}

mod egotop_app_launch {
    use super::*;

    pub fn launch_tui(cli: Cli) -> Result<i32> {
        let loaded = load_or_create(cli.config.as_deref())?;
        let mut cfg = loaded.config;

        if let Some(eff) = cli.effect {
            let kind: EffectKind = eff.into();
            cfg.background.effect = kind;
            cfg.background.enabled = kind != EffectKind::None;
        }
        if let Some(p) = cli.page {
            cfg.general.default_page = p;
        }
        if let Some(st) = cli.startup {
            cfg.startup.mode = st.into();
        }

        let toast = loaded
            .created_default
            .then(|| format!("Default configuration created at {}", loaded.path.display()));

        let (cols, rows) = terminal::size().unwrap_or((120, 36));
        let mut app = crate::app::App::new(cfg, loaded.path, toast, cols, rows);
        app.run()?;
        Ok(0)
    }
}
