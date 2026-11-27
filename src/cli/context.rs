use anyhow::Result;
use clap::{Args, Subcommand};

#[derive(Debug, Args)]
pub struct ContextArgs {
    #[command(subcommand)]
    pub command: ContextCommand,
}

#[derive(Debug, Subcommand)]
pub enum ContextCommand {
    /// List declared runtime contexts (when supported)
    List,
}

pub fn handle(args: ContextArgs) -> Result<i32> {
    match args.command {
        ContextCommand::List => {
            println!("No contexts available: runtime contexts are not implemented in this version of egotop.");
            Ok(0)
        }
    }
}
