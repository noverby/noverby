mod cli;
mod commands;
mod util;

use anyhow::Result;
use clap::Parser;
use cli::Cli;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    commands::dispatch(cli).await
}
