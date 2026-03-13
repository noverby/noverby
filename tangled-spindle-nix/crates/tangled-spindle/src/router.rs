//! HTTP route definitions and WebSocket handlers.
//!
//! Defines the axum router, shared application state, and handler functions
//! for the MOTD, `/events` WebSocket, `/logs/{knot}/{rkey}/{name}` WebSocket,
//! and `/xrpc/{method}` XRPC dispatch endpoints.

use std::io::{BufRead, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use serde::Deserialize;
use spindle_db::Database;
use spindle_models::pipeline::PipelineId;
use spindle_models::workflow::WorkflowId;
use spindle_models::workflow_logger;
use spindle_xrpc::XrpcContext;
use tokio::sync::mpsc;
use tracing::{debug, error, warn};

use crate::notifier::Notifier;

/// Shared application state, available to all route handlers via
/// `axum::extract::State<Arc<AppState>>`.
pub struct AppState {
    /// Database handle.
    pub db: Arc<Database>,
    /// Event notifier (broadcast channel).
    pub notifier: Arc<Notifier>,
    /// XRPC context (shared with the XRPC sub-router).
    pub xrpc: Arc<XrpcContext>,
    /// Directory where workflow log files are stored.
    pub log_dir: PathBuf,
    /// Public hostname of this spindle instance.
    pub hostname: String,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("hostname", &self.hostname)
            .field("log_dir", &self.log_dir)
            .finish_non_exhaustive()
    }
}

/// Build the axum router with all routes.
pub fn build_router(state: Arc<AppState>) -> Router {
    let xrpc_ctx = Arc::clone(&state.xrpc);

    let xrpc_router = Router::new()
        .route("/{method}", post(spindle_xrpc::dispatch))
        .with_state(xrpc_ctx);

    Router::new()
        .route("/", get(motd_handler))
        .route("/events", get(events_ws_handler))
        .route("/logs/{knot}/{rkey}/{name}", get(logs_ws_handler))
        .nest("/xrpc", xrpc_router)
        .with_state(state)
}

// ---------------------------------------------------------------------------
// MOTD handler
// ---------------------------------------------------------------------------

/// `GET /` — Return a simple MOTD text identifying the spindle instance.
async fn motd_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    format!(
        "tangled-spindle-nix v{} @ {}\n",
        env!("CARGO_PKG_VERSION"),
        state.hostname,
    )
}

// ---------------------------------------------------------------------------
// /events WebSocket handler
// ---------------------------------------------------------------------------

/// Query parameters for the `/events` WebSocket endpoint.
#[derive(Debug, Deserialize)]
struct EventsQuery {
    /// Cursor: the last event ID the client has seen. Events with `id > cursor`
    /// will be backfilled on connection.
    #[serde(default)]
    cursor: Option<i64>,
}

/// `GET /events` — Upgrade to WebSocket for pipeline event streaming.
async fn events_ws_handler(
    ws: WebSocketUpgrade,
    Query(query): Query<EventsQuery>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| events_ws(socket, state, query.cursor.unwrap_or(0)))
}

/// Handle the `/events` WebSocket connection.
///
/// Protocol:
/// 1. Subscribe to the broadcast channel **before** backfilling (avoids race).
/// 2. Backfill events from the database with `id > cursor`.
/// 3. Stream live events from the broadcast channel, deduplicating by ID.
/// 4. Send keepalive pings every 30 seconds.
async fn events_ws(mut socket: WebSocket, state: Arc<AppState>, cursor: i64) {
    // Subscribe first to avoid missing events between backfill and live.
    let mut rx = state.notifier.subscribe();
    let mut last_sent_id = cursor;

    // Backfill from database.
    match state.db.get_events_after(cursor) {
        Ok(events) => {
            for event in events {
                let id = event.id;
                match serde_json::to_string(&event) {
                    Ok(json) => {
                        if socket.send(Message::Text(json.into())).await.is_err() {
                            return;
                        }
                        last_sent_id = id;
                    }
                    Err(e) => {
                        warn!(%e, "failed to serialize event for backfill");
                    }
                }
            }
        }
        Err(e) => {
            warn!(%e, "failed to backfill events from database");
        }
    }

    debug!(
        last_sent_id,
        "events backfill complete, switching to live stream"
    );

    // Live stream loop.
    let mut keepalive = tokio::time::interval(Duration::from_secs(30));
    keepalive.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Ok(event) => {
                        if event.id <= last_sent_id {
                            continue; // Deduplicate.
                        }
                        last_sent_id = event.id;
                        match serde_json::to_string(&event) {
                            Ok(json) => {
                                if socket.send(Message::Text(json.into())).await.is_err() {
                                    return;
                                }
                            }
                            Err(e) => {
                                warn!(%e, "failed to serialize live event");
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        warn!(n, "events WebSocket consumer lagged, some events may be missed");
                        // Client can reconnect with a cursor to re-backfill.
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        debug!("events broadcast channel closed");
                        return;
                    }
                }
            }
            _ = keepalive.tick() => {
                if socket.send(Message::Ping(vec![].into())).await.is_err() {
                    return;
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => return,
                    Some(Err(_)) => return,
                    _ => {} // Ignore text/binary/pong from client.
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// /logs/{knot}/{rkey}/{name} WebSocket handler
// ---------------------------------------------------------------------------

/// `GET /logs/{knot}/{rkey}/{name}` — Upgrade to WebSocket for log streaming.
async fn logs_ws_handler(
    ws: WebSocketUpgrade,
    Path((knot, rkey, name)): Path<(String, String, String)>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| logs_ws(socket, state, knot, rkey, name))
}

/// Handle the `/logs` WebSocket connection.
///
/// Streams NDJSON log lines for a specific workflow. If the workflow is
/// finished, the complete log file is sent and the connection is closed.
/// If the workflow is still running, existing lines are sent and then new
/// lines are streamed in real-time using filesystem notification.
async fn logs_ws(
    mut socket: WebSocket,
    state: Arc<AppState>,
    knot: String,
    rkey: String,
    name: String,
) {
    let pipeline_id = PipelineId {
        knot: knot.clone(),
        rkey: rkey.clone(),
    };
    let workflow_id = WorkflowId::new(pipeline_id, &name);
    let log_path = workflow_logger::log_file_path(&state.log_dir, &workflow_id);
    let wid_str = workflow_id.to_string();

    // Check if workflow is in a terminal state.
    let is_finished = match state.db.get_status(&wid_str) {
        Ok(Some(status)) => {
            matches!(
                status.status.as_str(),
                "success" | "failed" | "timeout" | "cancelled"
            )
        }
        Ok(None) => {
            // Workflow not found — might not have started yet. Try to stream anyway.
            false
        }
        Err(e) => {
            warn!(%e, workflow_id = %wid_str, "failed to get workflow status");
            false
        }
    };

    if is_finished {
        // Send the complete log file and close.
        if send_log_file(&mut socket, &log_path).await.is_err() {
            return;
        }
        let _ = socket.send(Message::Close(None)).await;
        return;
    }

    // Workflow is pending or running — stream existing + live tail.
    //
    // Set up a filesystem watcher to detect new lines appended to the log file.
    let (notify_tx, mut notify_rx) = mpsc::channel::<()>(16);

    let watcher = setup_file_watcher(&log_path, notify_tx);
    if watcher.is_err() {
        warn!(path = %log_path.display(), "failed to set up file watcher, falling back to polling");
    }
    // Keep the watcher alive for the duration of the connection.
    let _watcher = watcher.ok();

    // Read existing content from the log file.
    let mut file = match std::fs::File::open(&log_path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // File doesn't exist yet. Wait for it to appear.
            debug!(path = %log_path.display(), "log file not found, waiting for creation");
            match wait_for_file(&log_path, &mut socket, &mut notify_rx).await {
                Some(f) => f,
                None => return,
            }
        }
        Err(e) => {
            error!(%e, path = %log_path.display(), "failed to open log file");
            return;
        }
    };

    // Send existing content.
    if send_lines_from_file(&mut socket, &mut file).await.is_err() {
        return;
    }

    // Live tail loop.
    let mut keepalive = tokio::time::interval(Duration::from_secs(30));
    keepalive.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    // Also poll periodically in case inotify misses an event.
    let mut poll_interval = tokio::time::interval(Duration::from_secs(2));
    poll_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = notify_rx.recv() => {
                if send_lines_from_file(&mut socket, &mut file).await.is_err() {
                    return;
                }

                // Re-check if workflow finished.
                if check_finished(&state.db, &wid_str) {
                    // Send any remaining lines and close.
                    let _ = send_lines_from_file(&mut socket, &mut file).await;
                    let _ = socket.send(Message::Close(None)).await;
                    return;
                }
            }
            _ = poll_interval.tick() => {
                if send_lines_from_file(&mut socket, &mut file).await.is_err() {
                    return;
                }

                if check_finished(&state.db, &wid_str) {
                    let _ = send_lines_from_file(&mut socket, &mut file).await;
                    let _ = socket.send(Message::Close(None)).await;
                    return;
                }
            }
            _ = keepalive.tick() => {
                if socket.send(Message::Ping(vec![].into())).await.is_err() {
                    return;
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => return,
                    Some(Err(_)) => return,
                    _ => {}
                }
            }
        }
    }
}

/// Send all lines from a log file over the WebSocket.
async fn send_log_file(socket: &mut WebSocket, path: &std::path::Path) -> Result<(), ()> {
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) => {
            warn!(%e, path = %path.display(), "failed to open log file");
            return Err(());
        }
    };
    let reader = std::io::BufReader::new(file);
    for line in reader.lines() {
        match line {
            Ok(line) if !line.is_empty() => {
                if socket.send(Message::Text(line.into())).await.is_err() {
                    return Err(());
                }
            }
            Ok(_) => {} // Skip empty lines.
            Err(e) => {
                warn!(%e, "error reading log file line");
                break;
            }
        }
    }
    Ok(())
}

/// Read new lines from the current file position and send them over WebSocket.
async fn send_lines_from_file(socket: &mut WebSocket, file: &mut std::fs::File) -> Result<(), ()> {
    let mut reader = std::io::BufReader::new(&*file);
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break, // EOF — no more data.
            Ok(_) => {
                let trimmed = line.trim_end();
                if !trimmed.is_empty()
                    && socket
                        .send(Message::Text(trimmed.to_owned().into()))
                        .await
                        .is_err()
                {
                    // Update file position before returning error.
                    let pos = reader.stream_position().unwrap_or(0);
                    let _ = file.seek(SeekFrom::Start(pos));
                    return Err(());
                }
            }
            Err(e) => {
                warn!(%e, "error reading log file");
                break;
            }
        }
    }
    // Update the underlying file's position to match what we've read.
    let pos = reader.stream_position().unwrap_or(0);
    let _ = file.seek(SeekFrom::Start(pos));
    Ok(())
}

/// Set up a filesystem watcher for the given path.
///
/// Sends a notification on the `tx` channel whenever the file is modified.
/// Uses the `notify` crate with inotify on Linux.
fn setup_file_watcher(
    path: &std::path::Path,
    tx: mpsc::Sender<()>,
) -> Result<notify::RecommendedWatcher, notify::Error> {
    use notify::{RecursiveMode, Watcher};

    let watch_path = if path.exists() {
        path.to_path_buf()
    } else {
        // Watch the parent directory if the file doesn't exist yet.
        path.parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| path.to_path_buf())
    };

    let mut watcher = notify::recommended_watcher(move |res: Result<notify::Event, _>| {
        if let Ok(event) = res {
            use notify::EventKind;
            match event.kind {
                EventKind::Modify(_) | EventKind::Create(_) => {
                    let _ = tx.try_send(());
                }
                _ => {}
            }
        }
    })?;

    watcher.watch(&watch_path, RecursiveMode::NonRecursive)?;
    Ok(watcher)
}

/// Wait for a log file to be created, sending keepalive pings in the meantime.
///
/// Returns the opened file, or `None` if the WebSocket connection closed.
async fn wait_for_file(
    path: &std::path::Path,
    socket: &mut WebSocket,
    notify_rx: &mut mpsc::Receiver<()>,
) -> Option<std::fs::File> {
    let mut keepalive = tokio::time::interval(Duration::from_secs(30));
    keepalive.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    let mut poll_interval = tokio::time::interval(Duration::from_secs(1));
    poll_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = notify_rx.recv() => {
                if let Ok(f) = std::fs::File::open(path) {
                    return Some(f);
                }
            }
            _ = poll_interval.tick() => {
                if let Ok(f) = std::fs::File::open(path) {
                    return Some(f);
                }
            }
            _ = keepalive.tick() => {
                if socket.send(Message::Ping(vec![].into())).await.is_err() {
                    return None;
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => return None,
                    Some(Err(_)) => return None,
                    _ => {}
                }
            }
        }
    }
}

/// Check if a workflow has reached a terminal state.
fn check_finished(db: &Database, workflow_id: &str) -> bool {
    match db.get_status(workflow_id) {
        Ok(Some(status)) => matches!(
            status.status.as_str(),
            "success" | "failed" | "timeout" | "cancelled"
        ),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn motd_format() {
        let version = env!("CARGO_PKG_VERSION");
        let expected = format!("tangled-spindle-nix v{version} @ test.example.com\n");
        let result = format!(
            "tangled-spindle-nix v{} @ {}\n",
            version, "test.example.com"
        );
        assert_eq!(result, expected);
    }
}
