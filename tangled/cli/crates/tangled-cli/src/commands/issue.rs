use crate::cli::{
    Cli, IssueCommand, IssueCommentArgs, IssueCreateArgs, IssueEditArgs, IssueListArgs,
    IssueShowArgs,
};
use anyhow::{anyhow, Result};
use tangled_api::Issue;

pub async fn run(_cli: &Cli, cmd: IssueCommand) -> Result<()> {
    match cmd {
        IssueCommand::List(args) => list(args).await,
        IssueCommand::Create(args) => create(args).await,
        IssueCommand::Show(args) => show(args).await,
        IssueCommand::Edit(args) => edit(args).await,
        IssueCommand::Comment(args) => comment(args).await,
    }
}

async fn list(args: IssueListArgs) -> Result<()> {
    let session = crate::util::load_session_with_refresh().await?;
    let pds = session
        .pds
        .clone()
        .or_else(|| std::env::var("TANGLED_PDS_BASE").ok())
        .unwrap_or_else(|| "https://bsky.social".into());
    let client = crate::util::make_client(&pds);

    let repo_filter_at = if let Some(repo) = &args.repo {
        let (owner, name) = parse_repo_ref(repo, &session.handle);
        let info = client
            .get_repo_info(owner, name, Some(session.access_jwt.as_str()))
            .await?;
        Some(format!("at://{}/sh.tangled.repo/{}", info.did, info.rkey))
    } else {
        None
    };

    let items = client
        .list_issues(
            &session.did,
            repo_filter_at.as_deref(),
            Some(session.access_jwt.as_str()),
        )
        .await?;
    if items.is_empty() {
        println!("No issues found (showing only issues you created)");
    } else {
        println!("RKEY\tTITLE\tREPO");
        for it in items {
            println!("{}\t{}\t{}", it.rkey, it.issue.title, it.issue.repo);
        }
    }
    Ok(())
}

async fn create(args: IssueCreateArgs) -> Result<()> {
    let session = crate::util::load_session_with_refresh().await?;
    let pds = session
        .pds
        .clone()
        .or_else(|| std::env::var("TANGLED_PDS_BASE").ok())
        .unwrap_or_else(|| "https://bsky.social".into());
    let client = crate::util::make_client(&pds);

    let repo = args
        .repo
        .as_ref()
        .ok_or_else(|| anyhow!("--repo is required for issue create"))?;
    let (owner, name) = parse_repo_ref(repo, &session.handle);
    let info = client
        .get_repo_info(owner, name, Some(session.access_jwt.as_str()))
        .await?;
    let title = args
        .title
        .as_deref()
        .ok_or_else(|| anyhow!("--title is required for issue create"))?;
    let rkey = client
        .create_issue(
            &session.did,
            &info.did,
            &info.rkey,
            title,
            args.body.as_deref(),
            &pds,
            &session.access_jwt,
        )
        .await?;
    println!("Created issue rkey={} in {}/{}", rkey, owner, name);
    Ok(())
}

async fn show(args: IssueShowArgs) -> Result<()> {
    // For now, show only accepts at-uri or did:rkey or rkey (for your DID)
    let session = crate::util::load_session_with_refresh().await?;
    let id = args.id;
    let (did, rkey) = parse_record_id(&id, &session.did)?;
    let pds = session
        .pds
        .clone()
        .or_else(|| std::env::var("TANGLED_PDS_BASE").ok())
        .unwrap_or_else(|| "https://bsky.social".into());
    let client = crate::util::make_client(&pds);
    // Fetch all issues by this DID and find rkey
    let items = client
        .list_issues(&did, None, Some(session.access_jwt.as_str()))
        .await?;
    if let Some(it) = items.into_iter().find(|i| i.rkey == rkey) {
        println!("TITLE: {}", it.issue.title);
        if !it.issue.body.is_empty() {
            println!("BODY:\n{}", it.issue.body);
        }
        println!("REPO: {}", it.issue.repo);
        println!("AUTHOR: {}", it.author_did);
        println!("RKEY: {}", rkey);
    } else {
        println!("Issue not found for did={} rkey={}", did, rkey);
    }
    Ok(())
}

async fn edit(args: IssueEditArgs) -> Result<()> {
    // Simple edit: fetch existing record and putRecord with new title/body
    let session = crate::util::load_session_with_refresh().await?;
    let (did, rkey) = parse_record_id(&args.id, &session.did)?;
    let pds = session
        .pds
        .clone()
        .or_else(|| std::env::var("TANGLED_PDS_BASE").ok())
        .unwrap_or_else(|| "https://bsky.social".into());
    // Get existing
    let client = crate::util::make_client(&pds);
    let mut rec: Issue = client
        .get_issue_record(&did, &rkey, Some(session.access_jwt.as_str()))
        .await?;
    if let Some(t) = args.title.as_deref() {
        rec.title = t.to_string();
    }
    if let Some(b) = args.body.as_deref() {
        rec.body = b.to_string();
    }
    // Put record back
    client
        .put_issue_record(&did, &rkey, &rec, Some(session.access_jwt.as_str()))
        .await?;

    // Optional state change
    if let Some(state) = args.state.as_deref() {
        let state_nsid = match state {
            "open" => "sh.tangled.repo.issue.state.open",
            "closed" => "sh.tangled.repo.issue.state.closed",
            other => {
                return Err(anyhow!(format!(
                    "unknown state '{}', expected 'open' or 'closed'",
                    other
                )))
            }
        };
        let issue_at = rec.repo.clone();
        client
            .set_issue_state(
                &session.did,
                &issue_at,
                state_nsid,
                &pds,
                &session.access_jwt,
            )
            .await?;
    }
    println!("Updated issue {}:{}", did, rkey);
    Ok(())
}

async fn comment(args: IssueCommentArgs) -> Result<()> {
    let session = crate::util::load_session_with_refresh().await?;
    let (did, rkey) = parse_record_id(&args.id, &session.did)?;
    let pds = session
        .pds
        .clone()
        .or_else(|| std::env::var("TANGLED_PDS_BASE").ok())
        .unwrap_or_else(|| "https://bsky.social".into());
    // Resolve issue AT-URI
    let client = crate::util::make_client(&pds);
    let issue_at = client
        .get_issue_record(&did, &rkey, Some(session.access_jwt.as_str()))
        .await?
        .repo;
    if let Some(body) = args.body.as_deref() {
        client
            .comment_issue(&session.did, &issue_at, body, &pds, &session.access_jwt)
            .await?;
        println!("Comment posted");
    }
    if args.close {
        client
            .set_issue_state(
                &session.did,
                &issue_at,
                "sh.tangled.repo.issue.state.closed",
                &pds,
                &session.access_jwt,
            )
            .await?;
        println!("Issue closed");
    }
    Ok(())
}

fn parse_repo_ref<'a>(spec: &'a str, default_owner: &'a str) -> (&'a str, &'a str) {
    if let Some((owner, name)) = spec.split_once('/') {
        (owner, name)
    } else {
        (default_owner, spec)
    }
}

fn parse_record_id<'a>(id: &'a str, default_did: &'a str) -> Result<(String, String)> {
    if let Some(rest) = id.strip_prefix("at://") {
        let parts: Vec<&str> = rest.split('/').collect();
        if parts.len() >= 4 {
            return Ok((parts[0].to_string(), parts[3].to_string()));
        }
    }
    if let Some((did, rkey)) = id.split_once(':') {
        return Ok((did.to_string(), rkey.to_string()));
    }
    Ok((default_did.to_string(), id.to_string()))
}
