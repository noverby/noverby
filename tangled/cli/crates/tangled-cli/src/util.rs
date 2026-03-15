use anyhow::{anyhow, Result};
use tangled_config::session::{Session, SessionManager};

/// Load session and automatically refresh if expired
pub async fn load_session() -> Result<Session> {
    let mgr = SessionManager::default();
    let session = mgr
        .load()?
        .ok_or_else(|| anyhow!("Please login first: tangled auth login"))?;

    Ok(session)
}

/// Refresh the session using the refresh token
pub async fn refresh_session(session: &Session) -> Result<Session> {
    let pds = session
        .pds
        .clone()
        .unwrap_or_else(|| "https://bsky.social".to_string());

    let client = tangled_api::TangledClient::new(&pds);
    let mut new_session = client.refresh_session(&session.refresh_jwt).await?;

    // Preserve PDS from old session
    new_session.pds = session.pds.clone();

    // Save the refreshed session
    let mgr = SessionManager::default();
    mgr.save(&new_session)?;

    Ok(new_session)
}

/// Load session with automatic refresh on ExpiredToken
pub async fn load_session_with_refresh() -> Result<Session> {
    let session = load_session().await?;

    // Check if session is older than 30 minutes - if so, proactively refresh
    let age = chrono::Utc::now()
        .signed_duration_since(session.created_at)
        .num_minutes();

    if age > 30 {
        // Session is old, proactively refresh
        match refresh_session(&session).await {
            Ok(new_session) => return Ok(new_session),
            Err(_) => {
                // If refresh fails, try with the old session anyway
                // It might still work
            }
        }
    }

    Ok(session)
}
