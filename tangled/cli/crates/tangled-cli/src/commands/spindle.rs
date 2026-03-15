use crate::cli::{
    Cli, SpindleCommand, SpindleConfigArgs, SpindleListArgs, SpindleLogsArgs, SpindleRunArgs,
    SpindleSecretAddArgs, SpindleSecretCommand, SpindleSecretListArgs, SpindleSecretRemoveArgs,
};
use anyhow::{anyhow, Result};
use futures_util::StreamExt;
use tokio_tungstenite::{connect_async, tungstenite::Message};

pub async fn run(_cli: &Cli, cmd: SpindleCommand) -> Result<()> {
    match cmd {
        SpindleCommand::List(args) => list(args).await,
        SpindleCommand::Config(args) => config(args).await,
        SpindleCommand::Run(args) => run_pipeline(args).await,
        SpindleCommand::Logs(args) => logs(args).await,
        SpindleCommand::Secret(cmd) => secret(cmd).await,
    }
}

async fn list(args: SpindleListArgs) -> Result<()> {
    let session = crate::util::load_session_with_refresh().await?;

    let pds = session
        .pds
        .clone()
        .or_else(|| std::env::var("TANGLED_PDS_BASE").ok())
        .unwrap_or_else(|| "https://bsky.social".into());
    let pds_client = crate::util::make_client(&pds);

    let (owner, name) = parse_repo_ref(
        args.repo.as_deref().unwrap_or(&session.handle),
        &session.handle,
    );
    let info = pds_client
        .get_repo_info(owner, name, Some(session.access_jwt.as_str()))
        .await?;

    let pipelines = pds_client
        .list_pipelines(&info.did, Some(session.access_jwt.as_str()))
        .await?;

    if pipelines.is_empty() {
        println!("No pipelines found for {}/{}", owner, name);
    } else {
        println!("RKEY\tKIND\tREPO\tWORKFLOWS");
        for p in pipelines {
            let workflows = p
                .pipeline
                .workflows
                .iter()
                .map(|w| w.name.as_str())
                .collect::<Vec<_>>()
                .join(",");
            println!(
                "{}\t{}\t{}\t{}",
                p.rkey,
                p.pipeline.trigger_metadata.kind,
                p.pipeline.trigger_metadata.repo.repo,
                workflows
            );
        }
    }
    Ok(())
}

async fn config(args: SpindleConfigArgs) -> Result<()> {
    let session = crate::util::load_session_with_refresh().await?;

    if args.enable && args.disable {
        return Err(anyhow!("Cannot use --enable and --disable together"));
    }

    if !args.enable && !args.disable && args.url.is_none() {
        return Err(anyhow!("Must provide --enable, --disable, or --url"));
    }

    let pds = session
        .pds
        .clone()
        .or_else(|| std::env::var("TANGLED_PDS_BASE").ok())
        .unwrap_or_else(|| "https://bsky.social".into());
    let pds_client = crate::util::make_client(&pds);

    let (owner, name) = parse_repo_ref(
        args.repo.as_deref().unwrap_or(&session.handle),
        &session.handle,
    );
    let info = pds_client
        .get_repo_info(owner, name, Some(session.access_jwt.as_str()))
        .await?;

    let new_spindle = if args.disable {
        None
    } else if let Some(url) = args.url.as_deref() {
        Some(url)
    } else if args.enable {
        // Default spindle URL
        Some("https://spindle.tangled.sh")
    } else {
        return Err(anyhow!("Invalid flags combination"));
    };

    pds_client
        .update_repo_spindle(
            &info.did,
            &info.rkey,
            new_spindle,
            &pds,
            &session.access_jwt,
        )
        .await?;

    if args.disable {
        println!("Disabled spindle for {}/{}", owner, name);
    } else {
        println!(
            "Enabled spindle for {}/{} ({})",
            owner,
            name,
            new_spindle.unwrap_or_default()
        );
    }
    Ok(())
}

async fn run_pipeline(args: SpindleRunArgs) -> Result<()> {
    println!(
        "Spindle run (stub) repo={:?} branch={:?} wait={}",
        args.repo, args.branch, args.wait
    );
    Ok(())
}

async fn logs(args: SpindleLogsArgs) -> Result<()> {
    // Parse job_id: format is "knot:rkey:name" or just "name" (use repo context)
    let parts: Vec<&str> = args.job_id.split(':').collect();
    let (knot, rkey, name) = if parts.len() == 3 {
        (
            parts[0].to_string(),
            parts[1].to_string(),
            parts[2].to_string(),
        )
    } else if parts.len() == 1 {
        // Use repo context - need to get repo info
        let session = crate::util::load_session_with_refresh().await?;
        let pds = session
            .pds
            .clone()
            .or_else(|| std::env::var("TANGLED_PDS_BASE").ok())
            .unwrap_or_else(|| "https://bsky.social".into());
        let pds_client = crate::util::make_client(&pds);
        // Get repo info from current directory context or default to user's handle
        let info = pds_client
            .get_repo_info(
                &session.handle,
                &session.handle,
                Some(session.access_jwt.as_str()),
            )
            .await?;
        (info.knot, info.rkey, parts[0].to_string())
    } else {
        return Err(anyhow!(
            "Invalid job_id format. Expected 'knot:rkey:name' or 'name'"
        ));
    };

    // Build WebSocket URL - spindle base is typically https://spindle.tangled.sh
    let spindle_base = std::env::var("TANGLED_SPINDLE_BASE")
        .unwrap_or_else(|_| "wss://spindle.tangled.sh".to_string());
    let ws_url = format!("{}/spindle/logs/{}/{}/{}", spindle_base, knot, rkey, name);

    println!(
        "Connecting to logs stream for {}:{}:{}...",
        knot, rkey, name
    );

    // Connect to WebSocket
    let (ws_stream, _) = connect_async(&ws_url)
        .await
        .map_err(|e| anyhow!("Failed to connect to log stream: {}", e))?;

    let (mut _write, mut read) = ws_stream.split();

    // Stream log messages
    let mut line_count = 0;
    let max_lines = args.lines.unwrap_or(usize::MAX);

    while let Some(msg) = read.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                println!("{}", text);
                line_count += 1;
                if line_count >= max_lines {
                    break;
                }
            }
            Ok(Message::Close(_)) => {
                break;
            }
            Err(e) => {
                return Err(anyhow!("WebSocket error: {}", e));
            }
            _ => {}
        }
    }

    Ok(())
}

async fn secret(cmd: SpindleSecretCommand) -> Result<()> {
    match cmd {
        SpindleSecretCommand::List(args) => secret_list(args).await,
        SpindleSecretCommand::Add(args) => secret_add(args).await,
        SpindleSecretCommand::Remove(args) => secret_remove(args).await,
    }
}

async fn secret_list(args: SpindleSecretListArgs) -> Result<()> {
    let session = crate::util::load_session_with_refresh().await?;
    let pds = session
        .pds
        .clone()
        .or_else(|| std::env::var("TANGLED_PDS_BASE").ok())
        .unwrap_or_else(|| "https://bsky.social".into());
    let pds_client = crate::util::make_client(&pds);
    let (owner, name) = parse_repo_ref(&args.repo, &session.handle);
    let info = pds_client
        .get_repo_info(owner, name, Some(session.access_jwt.as_str()))
        .await?;
    let repo_at = format!("at://{}/sh.tangled.repo/{}", info.did, info.rkey);

    // Get spindle base from repo config or use default
    let spindle_base = info
        .spindle
        .clone()
        .or_else(|| std::env::var("TANGLED_SPINDLE_BASE").ok())
        .unwrap_or_else(|| "https://spindle.tangled.sh".to_string());
    let api = crate::util::make_client(&spindle_base);

    let secrets = api
        .list_repo_secrets(&pds, &session.access_jwt, &repo_at)
        .await?;
    if secrets.is_empty() {
        println!("No secrets configured for {}", args.repo);
    } else {
        println!("KEY\tCREATED AT\tCREATED BY");
        for s in secrets {
            println!("{}\t{}\t{}", s.key, s.created_at, s.created_by);
        }
    }
    Ok(())
}

async fn secret_add(args: SpindleSecretAddArgs) -> Result<()> {
    let session = crate::util::load_session_with_refresh().await?;
    let pds = session
        .pds
        .clone()
        .or_else(|| std::env::var("TANGLED_PDS_BASE").ok())
        .unwrap_or_else(|| "https://bsky.social".into());
    let pds_client = crate::util::make_client(&pds);
    let (owner, name) = parse_repo_ref(&args.repo, &session.handle);
    let info = pds_client
        .get_repo_info(owner, name, Some(session.access_jwt.as_str()))
        .await?;
    let repo_at = format!("at://{}/sh.tangled.repo/{}", info.did, info.rkey);

    // Get spindle base from repo config or use default
    let spindle_base = info
        .spindle
        .clone()
        .or_else(|| std::env::var("TANGLED_SPINDLE_BASE").ok())
        .unwrap_or_else(|| "https://spindle.tangled.sh".to_string());
    let api = crate::util::make_client(&spindle_base);

    // Handle special value patterns: @file or - (stdin)
    let value = if args.value == "-" {
        // Read from stdin
        use std::io::Read;
        let mut buffer = String::new();
        std::io::stdin().read_to_string(&mut buffer)?;
        buffer
    } else if let Some(path) = args.value.strip_prefix('@') {
        // Read from file, expand ~ if needed
        let expanded_path = if path.starts_with("~/") {
            if let Ok(home) = std::env::var("HOME") {
                path.replacen("~/", &format!("{}/", home), 1)
            } else {
                path.to_string()
            }
        } else {
            path.to_string()
        };
        std::fs::read_to_string(&expanded_path)
            .map_err(|e| anyhow!("Failed to read file '{}': {}", expanded_path, e))?
    } else {
        // Use value as-is
        args.value
    };

    api.add_repo_secret(&pds, &session.access_jwt, &repo_at, &args.key, &value)
        .await?;
    println!("Added secret '{}' to {}", args.key, args.repo);
    Ok(())
}

async fn secret_remove(args: SpindleSecretRemoveArgs) -> Result<()> {
    let session = crate::util::load_session_with_refresh().await?;
    let pds = session
        .pds
        .clone()
        .or_else(|| std::env::var("TANGLED_PDS_BASE").ok())
        .unwrap_or_else(|| "https://bsky.social".into());
    let pds_client = crate::util::make_client(&pds);
    let (owner, name) = parse_repo_ref(&args.repo, &session.handle);
    let info = pds_client
        .get_repo_info(owner, name, Some(session.access_jwt.as_str()))
        .await?;
    let repo_at = format!("at://{}/sh.tangled.repo/{}", info.did, info.rkey);

    // Get spindle base from repo config or use default
    let spindle_base = info
        .spindle
        .clone()
        .or_else(|| std::env::var("TANGLED_SPINDLE_BASE").ok())
        .unwrap_or_else(|| "https://spindle.tangled.sh".to_string());
    let api = crate::util::make_client(&spindle_base);

    api.remove_repo_secret(&pds, &session.access_jwt, &repo_at, &args.key)
        .await?;
    println!("Removed secret '{}' from {}", args.key, args.repo);
    Ok(())
}

fn parse_repo_ref<'a>(spec: &'a str, default_owner: &'a str) -> (&'a str, &'a str) {
    if let Some((owner, name)) = spec.split_once('/') {
        (owner, name)
    } else {
        (default_owner, spec)
    }
}
