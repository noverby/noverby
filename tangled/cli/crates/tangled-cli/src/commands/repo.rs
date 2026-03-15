use anyhow::{anyhow, Result};
use git2::{build::RepoBuilder, Cred, FetchOptions, RemoteCallbacks};
use serde_json;
use std::path::PathBuf;

use crate::cli::{
    Cli, OutputFormat, RepoCloneArgs, RepoCommand, RepoCreateArgs, RepoDeleteArgs, RepoInfoArgs,
    RepoListArgs, RepoRefArgs,
};

pub async fn run(cli: &Cli, cmd: RepoCommand) -> Result<()> {
    match cmd {
        RepoCommand::List(args) => list(cli, args).await,
        RepoCommand::Create(args) => create(args).await,
        RepoCommand::Clone(args) => clone(args).await,
        RepoCommand::Info(args) => info(args).await,
        RepoCommand::Delete(args) => delete(args).await,
        RepoCommand::Star(args) => star(args).await,
        RepoCommand::Unstar(args) => unstar(args).await,
    }
}

async fn list(cli: &Cli, args: RepoListArgs) -> Result<()> {
    let session = crate::util::load_session_with_refresh().await?;

    // Use the PDS to list repo records for the user
    let pds = session
        .pds
        .clone()
        .or_else(|| std::env::var("TANGLED_PDS_BASE").ok())
        .unwrap_or_else(|| "https://bsky.social".into());
    let pds_client = tangled_api::TangledClient::new(pds);
    // Default to the logged-in user handle if --user is not provided
    let effective_user = args.user.as_deref().unwrap_or(session.handle.as_str());
    let repos = pds_client
        .list_repos(
            Some(effective_user),
            args.knot.as_deref(),
            args.starred,
            Some(session.access_jwt.as_str()),
        )
        .await?;

    match cli.format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&repos)?;
            println!("{}", json);
        }
        OutputFormat::Table => {
            println!("NAME\tKNOT\tPRIVATE");
            for r in repos {
                println!("{}\t{}\t{}", r.name, r.knot.unwrap_or_default(), r.private);
            }
        }
    }

    Ok(())
}

async fn create(args: RepoCreateArgs) -> Result<()> {
    let session = crate::util::load_session_with_refresh().await?;

    let base = std::env::var("TANGLED_API_BASE").unwrap_or_else(|_| "https://tngl.sh".into());
    let client = tangled_api::TangledClient::new(base);

    // Determine PDS base and target knot hostname
    let pds = session
        .pds
        .clone()
        .or_else(|| std::env::var("TANGLED_PDS_BASE").ok())
        .unwrap_or_else(|| "https://bsky.social".into());
    let knot = args.knot.unwrap_or_else(|| "tngl.sh".to_string());

    let opts = tangled_api::client::CreateRepoOptions {
        did: &session.did,
        name: &args.name,
        knot: &knot,
        description: args.description.as_deref(),
        default_branch: None,
        source: None,
        pds_base: &pds,
        access_jwt: &session.access_jwt,
    };
    client.create_repo(opts).await?;

    println!("Created repo '{}' (knot: {})", args.name, knot);
    Ok(())
}

async fn clone(args: RepoCloneArgs) -> Result<()> {
    let session = crate::util::load_session_with_refresh().await?;

    let (owner, name) = parse_repo_ref(&args.repo, &session.handle);
    let pds = session
        .pds
        .clone()
        .or_else(|| std::env::var("TANGLED_PDS_BASE").ok())
        .unwrap_or_else(|| "https://bsky.social".into());
    let pds_client = tangled_api::TangledClient::new(&pds);
    let info = pds_client
        .get_repo_info(owner, &name, Some(session.access_jwt.as_str()))
        .await?;

    let remote = if args.https {
        let owner_path = if owner.starts_with('@') {
            owner.to_string()
        } else {
            format!("@{}", owner)
        };
        format!("https://tangled.org/{}/{}", owner_path, name)
    } else {
        let knot = if info.knot == "knot1.tangled.sh" {
            "tangled.org".to_string()
        } else {
            info.knot.clone()
        };
        format!("git@{}:{}/{}", knot, owner.trim_start_matches('@'), name)
    };

    let target = PathBuf::from(&name);
    println!("Cloning {} -> {:?}", remote, target);

    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(|_url, username_from_url, _allowed| {
        if let Some(user) = username_from_url {
            Cred::ssh_key_from_agent(user)
        } else {
            Cred::default()
        }
    });
    let mut fetch_opts = FetchOptions::new();
    fetch_opts.remote_callbacks(callbacks);
    if let Some(d) = args.depth {
        fetch_opts.depth(d as i32);
    }
    let mut builder = RepoBuilder::new();
    builder.fetch_options(fetch_opts);
    match builder.clone(&remote, &target) {
        Ok(_) => Ok(()),
        Err(e) => {
            println!("Failed to clone via libgit2: {}", e);
            println!(
                "Hint: try: git clone{} {}",
                args.depth
                    .map(|d| format!(" --depth {}", d))
                    .unwrap_or_default(),
                remote
            );
            Err(anyhow!(e.to_string()))
        }
    }
}

async fn info(args: RepoInfoArgs) -> Result<()> {
    let session = crate::util::load_session_with_refresh().await?;
    let (owner, name) = parse_repo_ref(&args.repo, &session.handle);
    let pds = session
        .pds
        .clone()
        .or_else(|| std::env::var("TANGLED_PDS_BASE").ok())
        .unwrap_or_else(|| "https://bsky.social".into());
    let pds_client = tangled_api::TangledClient::new(&pds);
    let info = pds_client
        .get_repo_info(owner, &name, Some(session.access_jwt.as_str()))
        .await?;

    println!("NAME:        {}", info.name);
    println!("OWNER DID:   {}", info.did);
    println!("KNOT:        {}", info.knot);
    if let Some(spindle) = info.spindle.as_deref() {
        if !spindle.is_empty() {
            println!("SPINDLE:     {}", spindle);
        }
    }
    if let Some(desc) = info.description.as_deref() {
        if !desc.is_empty() {
            println!("DESCRIPTION: {}", desc);
        }
    }

    let knot_host = if info.knot == "knot1.tangled.sh" {
        "tangled.org".to_string()
    } else {
        info.knot.clone()
    };
    if args.stats {
        let client = tangled_api::TangledClient::default();
        if let Ok(def) = client
            .get_default_branch(&knot_host, &info.did, &info.name)
            .await
        {
            println!(
                "DEFAULT BRANCH: {} ({})",
                def.name,
                def.short_hash.unwrap_or(def.hash)
            );
            if let Some(msg) = def.message {
                if !msg.is_empty() {
                    println!("LAST COMMIT:   {}", msg);
                }
            }
        }
        if let Ok(langs) = client
            .get_languages(&knot_host, &info.did, &info.name)
            .await
        {
            if !langs.languages.is_empty() {
                println!("LANGUAGES:");
                for l in langs.languages.iter().take(6) {
                    println!("  - {} ({}%)", l.name, l.percentage);
                }
            }
        }
    }

    if args.contributors {
        println!("Contributors: not implemented yet");
    }
    Ok(())
}

async fn delete(args: RepoDeleteArgs) -> Result<()> {
    let session = crate::util::load_session_with_refresh().await?;
    let (owner, name) = parse_repo_ref(&args.repo, &session.handle);
    let pds = session
        .pds
        .clone()
        .or_else(|| std::env::var("TANGLED_PDS_BASE").ok())
        .unwrap_or_else(|| "https://bsky.social".into());
    let pds_client = tangled_api::TangledClient::new(&pds);
    let record = pds_client
        .get_repo_info(owner, &name, Some(session.access_jwt.as_str()))
        .await?;
    let did = record.did;
    let api = tangled_api::TangledClient::default();
    api.delete_repo(&did, &name, &pds, &session.access_jwt)
        .await?;
    println!("Deleted repo '{}'", name);
    Ok(())
}

async fn star(args: RepoRefArgs) -> Result<()> {
    let session = crate::util::load_session_with_refresh().await?;
    let (owner, name) = parse_repo_ref(&args.repo, &session.handle);
    let pds = session
        .pds
        .clone()
        .or_else(|| std::env::var("TANGLED_PDS_BASE").ok())
        .unwrap_or_else(|| "https://bsky.social".into());
    let pds_client = tangled_api::TangledClient::new(&pds);
    let info = pds_client
        .get_repo_info(owner, &name, Some(session.access_jwt.as_str()))
        .await?;
    let subject = format!("at://{}/sh.tangled.repo/{}", info.did, info.rkey);
    let api = tangled_api::TangledClient::default();
    api.star_repo(&pds, &session.access_jwt, &subject, &session.did)
        .await?;
    println!("Starred {}/{}", owner, name);
    Ok(())
}

async fn unstar(args: RepoRefArgs) -> Result<()> {
    let session = crate::util::load_session_with_refresh().await?;
    let (owner, name) = parse_repo_ref(&args.repo, &session.handle);
    let pds = session
        .pds
        .clone()
        .or_else(|| std::env::var("TANGLED_PDS_BASE").ok())
        .unwrap_or_else(|| "https://bsky.social".into());
    let pds_client = tangled_api::TangledClient::new(&pds);
    let info = pds_client
        .get_repo_info(owner, &name, Some(session.access_jwt.as_str()))
        .await?;
    let subject = format!("at://{}/sh.tangled.repo/{}", info.did, info.rkey);
    let api = tangled_api::TangledClient::default();
    api.unstar_repo(&pds, &session.access_jwt, &subject, &session.did)
        .await?;
    println!("Unstarred {}/{}", owner, name);
    Ok(())
}

fn parse_repo_ref<'a>(spec: &'a str, default_owner: &'a str) -> (&'a str, String) {
    if let Some((owner, name)) = spec.split_once('/') {
        (owner, name.to_string())
    } else {
        (default_owner, spec.to_string())
    }
}
