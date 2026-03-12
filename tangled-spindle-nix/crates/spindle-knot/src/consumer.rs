//! Knot event consumer for pipeline events.
//!
//! Subscribes to knot HTTP event streams (Server-Sent Events) and filters
//! for `sh.tangled.pipeline` events. Matches the upstream Go spindle's knot
//! event consumer behavior.
//!
//! # Protocol
//!
//! Each knot server exposes an SSE endpoint at `/events`. The consumer
//! connects with an optional `cursor` query parameter to replay missed events.
//! Events are newline-delimited JSON objects with `event:` and `data:` fields
//! following the SSE specification.
//!
//! # Connection Management
//!
//! - Cursor-based replay on reconnection.
//! - Dynamic source management: knots can be added/removed at runtime.
//! - Automatic reconnection with exponential backoff per knot.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, info, warn};

use crate::KnotError;

// ---------------------------------------------------------------------------
// AT Protocol constants
// ---------------------------------------------------------------------------

/// AT Protocol NSID for pipeline records.
pub const PIPELINE_NSID: &str = "sh.tangled.pipeline";

// ---------------------------------------------------------------------------
// Event types
// ---------------------------------------------------------------------------

/// A raw Server-Sent Event parsed from the knot stream.
#[derive(Debug, Clone, Default)]
struct SseEvent {
    /// The SSE `event:` field (event type name).
    event_type: Option<String>,
    /// The SSE `data:` field (may span multiple lines, joined by newlines).
    data: String,
    /// The SSE `id:` field (used as cursor).
    id: Option<String>,
}

/// A pipeline event received from a knot server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineEvent {
    /// The knot server hostname this event came from.
    pub knot: String,

    /// The event cursor ID (for resuming on reconnection).
    #[serde(default)]
    pub cursor: Option<String>,

    /// The DID of the repository owner.
    #[serde(default)]
    pub did: Option<String>,

    /// The record key identifying this pipeline.
    #[serde(default)]
    pub rkey: Option<String>,

    /// The event kind / type from the SSE stream.
    #[serde(default)]
    pub event_type: Option<String>,

    /// The raw JSON payload of the pipeline event.
    pub payload: serde_json::Value,
}

/// A pipeline record from the `sh.tangled.pipeline` collection.
///
/// This is the record payload carried inside a pipeline event.
/// It contains the workflow manifests, trigger metadata, and repo info
/// needed to execute a CI pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineRecord {
    /// The `$type` field (should be `"sh.tangled.pipeline"`).
    #[serde(rename = "$type", default)]
    pub r#type: Option<String>,

    /// The repository DID (owner).
    #[serde(default)]
    pub did: Option<String>,

    /// The repository name.
    #[serde(default)]
    pub repo: Option<String>,

    /// The knot server hostname.
    #[serde(default)]
    pub knot: Option<String>,

    /// Workflow manifests (raw YAML content, keyed by filename).
    #[serde(default)]
    pub workflows: Option<HashMap<String, WorkflowManifest>>,

    /// The trigger metadata (push, PR, manual).
    #[serde(default)]
    pub trigger: Option<serde_json::Value>,

    /// Clone options.
    #[serde(default)]
    pub clone: Option<serde_json::Value>,
}

/// A single workflow manifest within a pipeline record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowManifest {
    /// The workflow name (derived from filename).
    #[serde(default)]
    pub name: Option<String>,

    /// The raw YAML content of the workflow file.
    #[serde(default)]
    pub content: Option<String>,

    /// The engine identifier (e.g. `"nix"`, `"nixery"`).
    #[serde(default)]
    pub engine: Option<String>,
}

// ---------------------------------------------------------------------------
// Backoff configuration
// ---------------------------------------------------------------------------

/// Configuration for exponential backoff on reconnection.
#[derive(Debug, Clone)]
struct BackoffState {
    /// Current attempt count (reset on successful connection).
    attempt: u32,
    /// Initial delay.
    initial: Duration,
    /// Maximum delay.
    max: Duration,
    /// Multiplier applied after each failed attempt.
    multiplier: f64,
}

impl Default for BackoffState {
    fn default() -> Self {
        Self {
            attempt: 0,
            initial: Duration::from_secs(1),
            max: Duration::from_secs(60),
            multiplier: 2.0,
        }
    }
}

impl BackoffState {
    /// Compute the delay for the current attempt and increment the counter.
    fn next_delay(&mut self) -> Duration {
        let delay_secs = self.initial.as_secs_f64() * self.multiplier.powi(self.attempt as i32);
        let capped = delay_secs.min(self.max.as_secs_f64());
        self.attempt = self.attempt.saturating_add(1);
        Duration::from_secs_f64(capped)
    }

    /// Reset the backoff counter (called after a successful connection).
    fn reset(&mut self) {
        self.attempt = 0;
    }
}

// ---------------------------------------------------------------------------
// Per-knot connection state
// ---------------------------------------------------------------------------

/// Tracks the connection state for a single knot.
#[allow(dead_code)]
struct KnotConnection {
    /// The knot server hostname.
    knot: String,
    /// Last-seen cursor for this knot (for replay on reconnection).
    cursor: Option<String>,
    /// Handle to the spawned connection task.
    task: Option<tokio::task::JoinHandle<()>>,
    /// Cancellation token for this knot's connection task.
    cancel: tokio_util::sync::CancellationToken,
}

// ---------------------------------------------------------------------------
// Knot consumer
// ---------------------------------------------------------------------------

/// The knot event consumer manages connections to multiple knot servers
/// and streams pipeline events to a channel for processing.
///
/// # Dynamic Management
///
/// Knots can be added or removed at runtime. Each knot gets its own
/// connection task that handles SSE streaming, cursor tracking, and
/// reconnection with exponential backoff.
pub struct KnotConsumer {
    /// HTTP client for SSE connections.
    http_client: reqwest::Client,

    /// Active knot connections, keyed by hostname.
    connections: Arc<RwLock<HashMap<String, KnotConnection>>>,

    /// Channel sender for pipeline events.
    event_tx: mpsc::Sender<PipelineEvent>,

    /// Database handle for cursor persistence.
    db: Arc<spindle_db::Database>,

    /// Global shutdown token.
    shutdown: tokio_util::sync::CancellationToken,
}

impl KnotConsumer {
    /// Create a new knot consumer.
    ///
    /// # Arguments
    /// * `db` — Database handle for cursor persistence.
    /// * `event_tx` — Channel sender for pipeline events.
    /// * `shutdown` — Global cancellation token.
    pub fn new(
        db: Arc<spindle_db::Database>,
        event_tx: mpsc::Sender<PipelineEvent>,
        shutdown: tokio_util::sync::CancellationToken,
    ) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(0)) // No timeout for SSE streams
            .connect_timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        Self {
            http_client,
            connections: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
            db,
            shutdown,
        }
    }

    /// Subscribe to a knot server's event stream.
    ///
    /// If already subscribed, this is a no-op. The connection task is spawned
    /// immediately and will begin streaming events.
    pub async fn subscribe(&self, knot: &str) -> Result<(), KnotError> {
        let mut connections = self.connections.write().await;

        if connections.contains_key(knot) {
            debug!(knot = %knot, "already subscribed to knot, skipping");
            return Ok(());
        }

        // Load cursor from database
        let cursor = self.db.get_knot_cursor(knot).map_err(|e| {
            KnotError::Database(format!("failed to load cursor for knot {knot}: {e}"))
        })?;

        let cancel = self.shutdown.child_token();
        let task = self.spawn_connection(knot.to_string(), cursor.clone(), cancel.clone());

        connections.insert(
            knot.to_string(),
            KnotConnection {
                knot: knot.to_string(),
                cursor,
                task: Some(task),
                cancel,
            },
        );

        info!(knot = %knot, "subscribed to knot event stream");
        Ok(())
    }

    /// Unsubscribe from a knot server's event stream.
    ///
    /// Cancels the connection task and removes the knot from tracking.
    pub async fn unsubscribe(&self, knot: &str) -> Result<(), KnotError> {
        let mut connections = self.connections.write().await;

        if let Some(conn) = connections.remove(knot) {
            conn.cancel.cancel();
            if let Some(task) = conn.task {
                // Give the task a moment to clean up, but don't block indefinitely
                let _ = tokio::time::timeout(Duration::from_secs(5), task).await;
            }
            info!(knot = %knot, "unsubscribed from knot event stream");
        } else {
            debug!(knot = %knot, "not subscribed to knot, nothing to unsubscribe");
        }

        Ok(())
    }

    /// Get a list of all currently subscribed knots.
    pub async fn subscribed_knots(&self) -> Vec<String> {
        self.connections.read().await.keys().cloned().collect()
    }

    /// Initialize subscriptions for all knots tracked in the database.
    ///
    /// Called on startup to resume watching knots from the previous session.
    pub async fn restore_subscriptions(&self) -> Result<(), KnotError> {
        let knots = self.db.get_knot_names().map_err(|e| {
            KnotError::Database(format!("failed to load knot names from database: {e}"))
        })?;

        for knot in &knots {
            if let Err(e) = self.subscribe(knot).await {
                warn!(%e, knot = %knot, "failed to restore knot subscription");
            }
        }

        info!(
            count = knots.len(),
            "restored knot subscriptions from database"
        );
        Ok(())
    }

    /// Spawn a connection task for a single knot.
    fn spawn_connection(
        &self,
        knot: String,
        initial_cursor: Option<String>,
        cancel: tokio_util::sync::CancellationToken,
    ) -> tokio::task::JoinHandle<()> {
        let http_client = self.http_client.clone();
        let event_tx = self.event_tx.clone();
        let db = self.db.clone();
        let connections = self.connections.clone();

        tokio::spawn(async move {
            run_knot_connection(
                http_client,
                knot,
                initial_cursor,
                event_tx,
                db,
                connections,
                cancel,
            )
            .await;
        })
    }
}

/// Run the connection loop for a single knot server.
///
/// Connects to the knot's SSE endpoint, processes events, and reconnects
/// with exponential backoff on failures. Runs until the cancellation token
/// is triggered.
async fn run_knot_connection(
    http_client: reqwest::Client,
    knot: String,
    initial_cursor: Option<String>,
    event_tx: mpsc::Sender<PipelineEvent>,
    db: Arc<spindle_db::Database>,
    connections: Arc<RwLock<HashMap<String, KnotConnection>>>,
    cancel: tokio_util::sync::CancellationToken,
) {
    let mut cursor = initial_cursor;
    let mut backoff = BackoffState::default();

    loop {
        if cancel.is_cancelled() {
            info!(knot = %knot, "knot connection shutting down");
            return;
        }

        let url = build_events_url(&knot, cursor.as_deref());
        info!(knot = %knot, url = %url, "connecting to knot event stream");

        match stream_events(
            &http_client,
            &url,
            &knot,
            &mut cursor,
            &event_tx,
            &db,
            &connections,
            &cancel,
        )
        .await
        {
            Ok(()) => {
                // Clean disconnect (shutdown or server closed)
                info!(knot = %knot, "knot event stream closed cleanly");
                backoff.reset();
            }
            Err(e) => {
                let delay = backoff.next_delay();
                warn!(
                    %e,
                    knot = %knot,
                    attempt = backoff.attempt,
                    delay_secs = delay.as_secs_f64(),
                    "knot connection failed, reconnecting"
                );
                tokio::select! {
                    () = tokio::time::sleep(delay) => {}
                    () = cancel.cancelled() => return,
                }
            }
        }
    }
}

/// Build the events URL for a knot server.
///
/// Format: `https://{knot}/events[?cursor={cursor}]`
fn build_events_url(knot: &str, cursor: Option<&str>) -> String {
    let base = if knot.starts_with("http://") || knot.starts_with("https://") {
        format!("{knot}/events")
    } else {
        format!("https://{knot}/events")
    };

    match cursor {
        Some(c) if !c.is_empty() => format!("{base}?cursor={c}"),
        _ => base,
    }
}

/// Connect to a knot event stream and process events until disconnection.
async fn stream_events(
    http_client: &reqwest::Client,
    url: &str,
    knot: &str,
    cursor: &mut Option<String>,
    event_tx: &mpsc::Sender<PipelineEvent>,
    db: &Arc<spindle_db::Database>,
    connections: &Arc<RwLock<HashMap<String, KnotConnection>>>,
    cancel: &tokio_util::sync::CancellationToken,
) -> Result<(), KnotError> {
    let response = http_client
        .get(url)
        .header("Accept", "text/event-stream")
        .header("Cache-Control", "no-cache")
        .send()
        .await
        .map_err(|e| KnotError::Connection(format!("HTTP request failed for {knot}: {e}")))?;

    if !response.status().is_success() {
        return Err(KnotError::Connection(format!(
            "knot {knot} returned HTTP {}",
            response.status()
        )));
    }

    info!(knot = %knot, "connected to knot event stream");

    // Process the SSE stream
    let mut bytes_stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut current_event = SseEvent::default();

    use futures_util::StreamExt;

    loop {
        tokio::select! {
            chunk = bytes_stream.next() => {
                match chunk {
                    Some(Ok(bytes)) => {
                        buffer.push_str(&String::from_utf8_lossy(&bytes));

                        // Process complete lines from the buffer
                        while let Some(newline_pos) = buffer.find('\n') {
                            let line = buffer[..newline_pos].trim_end_matches('\r').to_string();
                            buffer = buffer[newline_pos + 1..].to_string();

                            if line.is_empty() {
                                // Empty line = end of event
                                if !current_event.data.is_empty() {
                                    if let Err(e) = process_sse_event(
                                        knot,
                                        &current_event,
                                        cursor,
                                        event_tx,
                                        db,
                                        connections,
                                    ).await {
                                        debug!(%e, knot = %knot, "failed to process SSE event");
                                    }
                                }
                                current_event = SseEvent::default();
                            } else if let Some(value) = line.strip_prefix("event:") {
                                current_event.event_type = Some(value.trim().to_string());
                            } else if let Some(value) = line.strip_prefix("data:") {
                                if !current_event.data.is_empty() {
                                    current_event.data.push('\n');
                                }
                                current_event.data.push_str(value.trim());
                            } else if let Some(value) = line.strip_prefix("id:") {
                                current_event.id = Some(value.trim().to_string());
                            } else if line.starts_with(':') {
                                // SSE comment, ignore
                            } else {
                                debug!(knot = %knot, line = %line, "unknown SSE line format");
                            }
                        }
                    }
                    Some(Err(e)) => {
                        return Err(KnotError::Connection(format!(
                            "stream read error for {knot}: {e}"
                        )));
                    }
                    None => {
                        // Stream ended
                        return Ok(());
                    }
                }
            }
            () = cancel.cancelled() => {
                info!(knot = %knot, "shutdown requested, closing knot connection");
                return Ok(());
            }
        }
    }
}

/// Process a single parsed SSE event from a knot stream.
async fn process_sse_event(
    knot: &str,
    sse_event: &SseEvent,
    cursor: &mut Option<String>,
    event_tx: &mpsc::Sender<PipelineEvent>,
    db: &Arc<spindle_db::Database>,
    connections: &Arc<RwLock<HashMap<String, KnotConnection>>>,
) -> Result<(), KnotError> {
    // Update cursor if present
    if let Some(ref id) = sse_event.id {
        *cursor = Some(id.clone());

        // Persist cursor to database
        if let Err(e) = db.update_knot_cursor(knot, id) {
            warn!(%e, knot = %knot, cursor = %id, "failed to persist knot cursor");
        }

        // Update the in-memory cursor on the connection record
        let mut connections = connections.write().await;
        if let Some(conn) = connections.get_mut(knot) {
            conn.cursor = Some(id.clone());
        }
    }

    // Parse the event data as JSON
    let payload: serde_json::Value = serde_json::from_str(&sse_event.data).map_err(|e| {
        KnotError::Parse(format!(
            "failed to parse SSE event data as JSON for {knot}: {e}"
        ))
    })?;

    // Extract DID and rkey from the payload if present
    let did = payload
        .get("did")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let rkey = payload
        .get("rkey")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let pipeline_event = PipelineEvent {
        knot: knot.to_string(),
        cursor: sse_event.id.clone(),
        did,
        rkey,
        event_type: sse_event.event_type.clone(),
        payload,
    };

    debug!(
        knot = %knot,
        event_type = ?sse_event.event_type,
        cursor = ?sse_event.id,
        "received pipeline event"
    );

    // Send the event to the processing channel
    event_tx.send(pipeline_event).await.map_err(|_| {
        KnotError::Connection(format!("pipeline event channel closed for knot {knot}"))
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Backoff tests
    // -----------------------------------------------------------------------

    #[test]
    fn backoff_exponential_growth() {
        let mut backoff = BackoffState::default();

        let d0 = backoff.next_delay();
        assert_eq!(d0, Duration::from_secs(1));

        let d1 = backoff.next_delay();
        assert_eq!(d1, Duration::from_secs(2));

        let d2 = backoff.next_delay();
        assert_eq!(d2, Duration::from_secs(4));

        let d3 = backoff.next_delay();
        assert_eq!(d3, Duration::from_secs(8));
    }

    #[test]
    fn backoff_capped_at_max() {
        let mut backoff = BackoffState::default();

        // Run enough times to exceed max
        for _ in 0..20 {
            backoff.next_delay();
        }

        let d = backoff.next_delay();
        assert_eq!(d, Duration::from_secs(60));
    }

    #[test]
    fn backoff_reset() {
        let mut backoff = BackoffState::default();

        backoff.next_delay();
        backoff.next_delay();
        backoff.next_delay();
        assert_eq!(backoff.attempt, 3);

        backoff.reset();
        assert_eq!(backoff.attempt, 0);

        let d = backoff.next_delay();
        assert_eq!(d, Duration::from_secs(1));
    }

    // -----------------------------------------------------------------------
    // URL building tests
    // -----------------------------------------------------------------------

    #[test]
    fn build_events_url_no_cursor() {
        let url = build_events_url("knot.example.com", None);
        assert_eq!(url, "https://knot.example.com/events");
    }

    #[test]
    fn build_events_url_with_cursor() {
        let url = build_events_url("knot.example.com", Some("cursor-abc-123"));
        assert_eq!(url, "https://knot.example.com/events?cursor=cursor-abc-123");
    }

    #[test]
    fn build_events_url_empty_cursor_treated_as_none() {
        let url = build_events_url("knot.example.com", Some(""));
        assert_eq!(url, "https://knot.example.com/events");
    }

    #[test]
    fn build_events_url_with_http_prefix() {
        let url = build_events_url("http://localhost:3000", None);
        assert_eq!(url, "http://localhost:3000/events");
    }

    #[test]
    fn build_events_url_with_https_prefix() {
        let url = build_events_url("https://knot.example.com", None);
        assert_eq!(url, "https://knot.example.com/events");
    }

    // -----------------------------------------------------------------------
    // SSE parsing tests (via process_sse_event)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn process_sse_event_basic() {
        let db = Arc::new(spindle_db::Database::open_in_memory().unwrap());
        db.add_knot("knot.example.com").unwrap();

        let (tx, mut rx) = mpsc::channel(16);
        let connections: Arc<RwLock<HashMap<String, KnotConnection>>> =
            Arc::new(RwLock::new(HashMap::new()));

        // Insert a connection record so cursor update works
        {
            let mut conns = connections.write().await;
            conns.insert(
                "knot.example.com".to_string(),
                KnotConnection {
                    knot: "knot.example.com".to_string(),
                    cursor: None,
                    task: None,
                    cancel: tokio_util::sync::CancellationToken::new(),
                },
            );
        }

        let sse = SseEvent {
            event_type: Some("pipeline".to_string()),
            data: r#"{"did":"did:plc:alice","rkey":"abc123","status":"pending"}"#.to_string(),
            id: Some("cursor-001".to_string()),
        };

        let mut cursor = None;

        process_sse_event(
            "knot.example.com",
            &sse,
            &mut cursor,
            &tx,
            &db,
            &connections,
        )
        .await
        .unwrap();

        // Check cursor was updated
        assert_eq!(cursor.as_deref(), Some("cursor-001"));

        // Check cursor was persisted to DB
        let db_cursor = db.get_knot_cursor("knot.example.com").unwrap();
        assert_eq!(db_cursor.as_deref(), Some("cursor-001"));

        // Check event was sent
        let event = rx.try_recv().unwrap();
        assert_eq!(event.knot, "knot.example.com");
        assert_eq!(event.cursor.as_deref(), Some("cursor-001"));
        assert_eq!(event.did.as_deref(), Some("did:plc:alice"));
        assert_eq!(event.rkey.as_deref(), Some("abc123"));
        assert_eq!(event.event_type.as_deref(), Some("pipeline"));
    }

    #[tokio::test]
    async fn process_sse_event_no_id() {
        let db = Arc::new(spindle_db::Database::open_in_memory().unwrap());
        let (tx, mut rx) = mpsc::channel(16);
        let connections: Arc<RwLock<HashMap<String, KnotConnection>>> =
            Arc::new(RwLock::new(HashMap::new()));

        let sse = SseEvent {
            event_type: None,
            data: r#"{"status":"running"}"#.to_string(),
            id: None,
        };

        let mut cursor = Some("old-cursor".to_string());

        process_sse_event(
            "knot.example.com",
            &sse,
            &mut cursor,
            &tx,
            &db,
            &connections,
        )
        .await
        .unwrap();

        // Cursor should remain unchanged
        assert_eq!(cursor.as_deref(), Some("old-cursor"));

        // Event should still be sent
        let event = rx.try_recv().unwrap();
        assert_eq!(event.knot, "knot.example.com");
        assert!(event.cursor.is_none());
        assert!(event.did.is_none());
    }

    #[tokio::test]
    async fn process_sse_event_invalid_json() {
        let db = Arc::new(spindle_db::Database::open_in_memory().unwrap());
        let (tx, _rx) = mpsc::channel(16);
        let connections: Arc<RwLock<HashMap<String, KnotConnection>>> =
            Arc::new(RwLock::new(HashMap::new()));

        let sse = SseEvent {
            event_type: None,
            data: "not valid json{{{".to_string(),
            id: None,
        };

        let mut cursor = None;

        let result = process_sse_event(
            "knot.example.com",
            &sse,
            &mut cursor,
            &tx,
            &db,
            &connections,
        )
        .await;

        assert!(result.is_err());
        match result.unwrap_err() {
            KnotError::Parse(msg) => {
                assert!(msg.contains("failed to parse SSE event data"));
            }
            other => panic!("expected Parse error, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn process_sse_event_updates_connection_cursor() {
        let db = Arc::new(spindle_db::Database::open_in_memory().unwrap());
        db.add_knot("knot.example.com").unwrap();

        let (tx, _rx) = mpsc::channel(16);
        let connections: Arc<RwLock<HashMap<String, KnotConnection>>> =
            Arc::new(RwLock::new(HashMap::new()));

        {
            let mut conns = connections.write().await;
            conns.insert(
                "knot.example.com".to_string(),
                KnotConnection {
                    knot: "knot.example.com".to_string(),
                    cursor: None,
                    task: None,
                    cancel: tokio_util::sync::CancellationToken::new(),
                },
            );
        }

        let sse = SseEvent {
            event_type: Some("pipeline".to_string()),
            data: r#"{"status":"ok"}"#.to_string(),
            id: Some("cursor-042".to_string()),
        };

        let mut cursor = None;

        process_sse_event(
            "knot.example.com",
            &sse,
            &mut cursor,
            &tx,
            &db,
            &connections,
        )
        .await
        .unwrap();

        // Check in-memory connection cursor was updated
        let conns = connections.read().await;
        let conn = conns.get("knot.example.com").unwrap();
        assert_eq!(conn.cursor.as_deref(), Some("cursor-042"));
    }

    // -----------------------------------------------------------------------
    // PipelineRecord parsing tests
    // -----------------------------------------------------------------------

    #[test]
    fn parse_pipeline_record() {
        let json = serde_json::json!({
            "$type": "sh.tangled.pipeline",
            "did": "did:plc:alice",
            "repo": "my-repo",
            "knot": "knot.example.com",
            "workflows": {
                "test.yml": {
                    "name": "test",
                    "content": "dependencies:\n  nixpkgs:\n    - nodejs\nsteps:\n  - name: test\n    run: npm test",
                    "engine": "nix"
                }
            },
            "trigger": {
                "kind": "push",
                "repo": {
                    "knot": "knot.example.com",
                    "did": "did:plc:alice",
                    "repo": "my-repo"
                }
            }
        });

        let record: PipelineRecord = serde_json::from_value(json).unwrap();
        assert_eq!(record.r#type.as_deref(), Some("sh.tangled.pipeline"));
        assert_eq!(record.did.as_deref(), Some("did:plc:alice"));
        assert_eq!(record.repo.as_deref(), Some("my-repo"));
        assert_eq!(record.knot.as_deref(), Some("knot.example.com"));

        let workflows = record.workflows.unwrap();
        assert_eq!(workflows.len(), 1);
        let wf = workflows.get("test.yml").unwrap();
        assert_eq!(wf.name.as_deref(), Some("test"));
        assert_eq!(wf.engine.as_deref(), Some("nix"));
        assert!(wf.content.as_ref().unwrap().contains("npm test"));
    }

    #[test]
    fn parse_pipeline_record_minimal() {
        let json = serde_json::json!({
            "$type": "sh.tangled.pipeline"
        });

        let record: PipelineRecord = serde_json::from_value(json).unwrap();
        assert_eq!(record.r#type.as_deref(), Some("sh.tangled.pipeline"));
        assert!(record.did.is_none());
        assert!(record.repo.is_none());
        assert!(record.knot.is_none());
        assert!(record.workflows.is_none());
        assert!(record.trigger.is_none());
        assert!(record.clone.is_none());
    }

    #[test]
    fn parse_workflow_manifest() {
        let json = serde_json::json!({
            "name": "build",
            "content": "steps:\n  - name: build\n    run: cargo build",
            "engine": "nix"
        });

        let manifest: WorkflowManifest = serde_json::from_value(json).unwrap();
        assert_eq!(manifest.name.as_deref(), Some("build"));
        assert_eq!(manifest.engine.as_deref(), Some("nix"));
        assert!(manifest.content.as_ref().unwrap().contains("cargo build"));
    }

    #[test]
    fn parse_workflow_manifest_minimal() {
        let json = serde_json::json!({});

        let manifest: WorkflowManifest = serde_json::from_value(json).unwrap();
        assert!(manifest.name.is_none());
        assert!(manifest.content.is_none());
        assert!(manifest.engine.is_none());
    }

    // -----------------------------------------------------------------------
    // PipelineEvent serialization tests
    // -----------------------------------------------------------------------

    #[test]
    fn pipeline_event_serialization_roundtrip() {
        let event = PipelineEvent {
            knot: "knot.example.com".to_string(),
            cursor: Some("cursor-001".to_string()),
            did: Some("did:plc:alice".to_string()),
            rkey: Some("abc123".to_string()),
            event_type: Some("pipeline".to_string()),
            payload: serde_json::json!({"status": "pending"}),
        };

        let json = serde_json::to_string(&event).unwrap();
        let deserialized: PipelineEvent = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.knot, "knot.example.com");
        assert_eq!(deserialized.cursor.as_deref(), Some("cursor-001"));
        assert_eq!(deserialized.did.as_deref(), Some("did:plc:alice"));
        assert_eq!(deserialized.rkey.as_deref(), Some("abc123"));
        assert_eq!(deserialized.event_type.as_deref(), Some("pipeline"));
    }

    #[test]
    fn pipeline_event_deserialize_minimal() {
        let json = r#"{"knot":"knot.example.com","payload":{}}"#;
        let event: PipelineEvent = serde_json::from_str(json).unwrap();

        assert_eq!(event.knot, "knot.example.com");
        assert!(event.cursor.is_none());
        assert!(event.did.is_none());
        assert!(event.rkey.is_none());
        assert!(event.event_type.is_none());
    }

    // -----------------------------------------------------------------------
    // KnotConsumer lifecycle tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn consumer_subscribe_idempotent() {
        let db = Arc::new(spindle_db::Database::open_in_memory().unwrap());
        let (tx, _rx) = mpsc::channel(16);
        let shutdown = tokio_util::sync::CancellationToken::new();

        let consumer = KnotConsumer::new(db, tx, shutdown.clone());

        // Subscribe twice — second should be a no-op
        // Note: this will spawn a task that tries to connect and fails,
        // but the subscription tracking should work regardless.
        consumer.subscribe("knot.example.com").await.unwrap();
        consumer.subscribe("knot.example.com").await.unwrap();

        let knots = consumer.subscribed_knots().await;
        assert_eq!(knots.len(), 1);
        assert_eq!(knots[0], "knot.example.com");

        // Clean up
        shutdown.cancel();
    }

    #[tokio::test]
    async fn consumer_unsubscribe() {
        let db = Arc::new(spindle_db::Database::open_in_memory().unwrap());
        let (tx, _rx) = mpsc::channel(16);
        let shutdown = tokio_util::sync::CancellationToken::new();

        let consumer = KnotConsumer::new(db, tx, shutdown.clone());

        consumer.subscribe("knot.example.com").await.unwrap();
        assert_eq!(consumer.subscribed_knots().await.len(), 1);

        consumer.unsubscribe("knot.example.com").await.unwrap();
        assert!(consumer.subscribed_knots().await.is_empty());

        shutdown.cancel();
    }

    #[tokio::test]
    async fn consumer_unsubscribe_nonexistent() {
        let db = Arc::new(spindle_db::Database::open_in_memory().unwrap());
        let (tx, _rx) = mpsc::channel(16);
        let shutdown = tokio_util::sync::CancellationToken::new();

        let consumer = KnotConsumer::new(db, tx, shutdown.clone());

        // Unsubscribing from a non-existent knot should be a no-op
        consumer.unsubscribe("nonexistent.com").await.unwrap();

        shutdown.cancel();
    }

    #[tokio::test]
    async fn consumer_restore_subscriptions() {
        let db = Arc::new(spindle_db::Database::open_in_memory().unwrap());

        // Pre-populate knots in the database
        db.add_knot("knot-a.example.com").unwrap();
        db.add_knot("knot-b.example.com").unwrap();

        let (tx, _rx) = mpsc::channel(16);
        let shutdown = tokio_util::sync::CancellationToken::new();

        let consumer = KnotConsumer::new(db, tx, shutdown.clone());

        consumer.restore_subscriptions().await.unwrap();

        let mut knots = consumer.subscribed_knots().await;
        knots.sort();
        assert_eq!(knots.len(), 2);
        assert_eq!(knots[0], "knot-a.example.com");
        assert_eq!(knots[1], "knot-b.example.com");

        shutdown.cancel();
    }
}
