pub mod auth;
pub mod issue;
pub mod knot;
pub mod pr;
pub mod repo;
pub mod spindle;

use anyhow::Result;

use crate::cli::{Cli, Command};

pub async fn dispatch(cli: Cli) -> Result<()> {
    match &cli.command {
        Command::Auth(cmd) => auth::run(&cli, cmd.clone()).await,
        Command::Repo(cmd) => repo::run(&cli, cmd.clone()).await,
        Command::Issue(cmd) => issue::run(&cli, cmd.clone()).await,
        Command::Pr(cmd) => pr::run(&cli, cmd.clone()).await,
        Command::Knot(cmd) => knot::run(&cli, cmd.clone()).await,
        Command::Spindle(cmd) => spindle::run(&cli, cmd.clone()).await,
    }
}

// All subcommands are currently implemented with stubs where needed.
