use crate::ui::fonts::available_fonts;
use anyhow::Result;
use clap::{Args, Subcommand};

#[derive(Debug, Args)]
pub struct FontsArgs {
    #[command(subcommand)]
    pub command: FontsCommand,
}

#[derive(Debug, Subcommand)]
pub enum FontsCommand {
    /// List embedded FIGlet/ASCII fonts
    List,
}

pub fn handle(args: FontsArgs) -> Result<i32> {
    match args.command {
        FontsCommand::List => list_command(),
    }
}

fn list_command() -> Result<i32> {
    println!("Available Lua fonts:");
    for font in crate::lua::list_lua_fonts() {
        println!("  - {font} [lua]");
    }
    for font in available_fonts() {
        println!("  - {font} [builtin]");
    }
    Ok(0)
}
