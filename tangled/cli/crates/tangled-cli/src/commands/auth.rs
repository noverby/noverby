use anyhow::Result;
use dialoguer::{Input, Password};
use tangled_config::session::SessionManager;

use crate::cli::{AuthCommand, AuthLoginArgs, Cli};

pub async fn run(cli: &Cli, cmd: AuthCommand) -> Result<()> {
    match cmd {
        AuthCommand::Login(args) => login(cli, args).await,
        AuthCommand::Status => status(cli).await,
        AuthCommand::Logout => logout(cli).await,
    }
}

async fn login(_cli: &Cli, mut args: AuthLoginArgs) -> Result<()> {
    let handle: String = match args.handle.take() {
        Some(h) => h,
        None => Input::new().with_prompt("Handle").interact_text()?,
    };
    let password: String = match args.password.take() {
        Some(p) => p,
        None => Password::new().with_prompt("Password").interact()?,
    };
    let pds = args
        .pds
        .unwrap_or_else(|| "https://bsky.social".to_string());

    let client = tangled_api::TangledClient::new(&pds);
    let mut session = match client.login_with_password(&handle, &password, &pds).await {
        Ok(sess) => sess,
        Err(e) => {
            println!("\x1b[93mIf you're on your own PDS, make sure to pass the --pds flag\x1b[0m");
            return Err(e);
        }
    };
    session.pds = Some(pds.clone());
    SessionManager::default().save(&session)?;
    println!("Logged in as '{}' ({})", session.handle, session.did);
    Ok(())
}

async fn status(_cli: &Cli) -> Result<()> {
    let mgr = SessionManager::default();
    match mgr.load()? {
        Some(s) => println!("Logged in as '{}' ({})", s.handle, s.did),
        None => println!("Not logged in. Run: tangled auth login"),
    }
    Ok(())
}

async fn logout(_cli: &Cli) -> Result<()> {
    let mgr = SessionManager::default();
    if mgr.load()?.is_some() {
        mgr.clear()?;
        println!("Logged out");
    } else {
        println!("No session found");
    }
    Ok(())
}
