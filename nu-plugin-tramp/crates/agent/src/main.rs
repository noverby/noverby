//! `tramp-agent` — Lightweight RPC agent for nu-plugin-tramp.
//!
//! This binary runs on the remote host and speaks a length-prefixed
//! MsgPack-RPC protocol over stdin/stdout.  It is started by the plugin
//! (piped through the SSH connection) and handles all filesystem, process,
//! and system operations natively — avoiding the overhead of spawning shell
//! commands and parsing their text output.
//!
//! ## Wire protocol
//!
//! ```text
//! ┌──────────────────┬──────────────────────────┐
//! │ 4 bytes BE u32   │  MessagePack payload      │
//! │ (payload length) │  (Request | Response | …) │
//! └──────────────────┴──────────────────────────┘
//! ```
//!
//! The agent reads [`Request`] messages from stdin, dispatches them to the
//! appropriate handler, and writes [`Response`] messages to stdout.  It also
//! sends unsolicited [`Notification`] messages (e.g. `fs.changed`) when
//! watched paths change.
//!
//! ## Shutdown
//!
//! The agent exits cleanly when:
//! - stdin reaches EOF (the SSH connection was closed)
//! - it receives SIGTERM or SIGINT

mod ops;
mod rpc;

use std::sync::Arc;

use ops::process::ProcessTable;
use ops::watch::WatchState;
use rpc::{Incoming, Response, error_code, read_message, write_notification, write_response};
use tokio::io::{self, AsyncWriteExt, BufReader, BufWriter};
use tokio::sync::Mutex;

// ---------------------------------------------------------------------------
// Method dispatch
// ---------------------------------------------------------------------------

/// Dispatch a single RPC request to the appropriate handler and return the
/// response.
async fn dispatch(
    method: &str,
    id: u64,
    params: &rmpv::Value,
    process_table: &ProcessTable,
    watch_state: &WatchState,
) -> Response {
    match method {
        // -- File operations --------------------------------------------------
        "file.stat" => ops::file::stat(id, params).await,
        "file.stat_batch" => ops::file::stat_batch(id, params).await,
        "file.truename" => ops::file::truename(id, params).await,
        "file.read" => ops::file::read(id, params).await,
        "file.write" => ops::file::write(id, params).await,
        "file.copy" => ops::file::copy(id, params).await,
        "file.rename" => ops::file::rename(id, params).await,
        "file.delete" => ops::file::delete(id, params).await,
        "file.set_modes" => ops::file::set_modes(id, params).await,

        // -- Directory operations ---------------------------------------------
        "dir.list" => ops::dir::list(id, params).await,
        "dir.create" => ops::dir::create(id, params).await,
        "dir.remove" => ops::dir::remove(id, params).await,

        // -- Process operations -----------------------------------------------
        "process.run" => ops::process::run(id, params).await,
        "process.start" => ops::process::start(id, params, process_table).await,
        "process.read" => ops::process::read(id, params, process_table).await,
        "process.write" => ops::process::write(id, params, process_table).await,
        "process.kill" => ops::process::kill(id, params, process_table).await,

        // -- System operations ------------------------------------------------
        "system.info" => ops::system::info(id, params).await,
        "system.getenv" => ops::system::getenv(id, params).await,
        "system.statvfs" => ops::system::statvfs(id, params).await,

        // -- Watch operations -------------------------------------------------
        "watch.add" => ops::watch::add(id, params, watch_state).await,
        "watch.remove" => ops::watch::remove(id, params, watch_state).await,
        "watch.list" => ops::watch::list(id, params, watch_state).await,

        // -- Batch operations -------------------------------------------------
        "batch" => {
            ops::batch::batch(
                id,
                params,
                Arc::new(ProcessTable::new()), // batch gets isolated table for safety
                Arc::new(WatchState::new()),   // batch gets isolated watches
            )
            .await
        }

        // -- Ping (health check) ---------------------------------------------
        "ping" => Response::ok(
            id,
            rmpv::Value::Map(vec![
                (
                    rmpv::Value::String("status".into()),
                    rmpv::Value::String("ok".into()),
                ),
                (
                    rmpv::Value::String("version".into()),
                    rmpv::Value::String(env!("CARGO_PKG_VERSION").into()),
                ),
                (
                    rmpv::Value::String("pid".into()),
                    rmpv::Value::Integer((std::process::id() as u64).into()),
                ),
            ]),
        ),

        // -- Unknown method ---------------------------------------------------
        _ => Response::err(
            id,
            error_code::METHOD_NOT_FOUND,
            format!("unknown method: {method}"),
        ),
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    // Log to stderr so it doesn't interfere with the MsgPack protocol on
    // stdout.  In production the plugin captures stderr for diagnostics.
    eprintln!(
        "tramp-agent v{} starting (pid {})",
        env!("CARGO_PKG_VERSION"),
        std::process::id()
    );

    // Shared state.
    let process_table = Arc::new(ProcessTable::new());
    let watch_state = Arc::new(WatchState::new());

    // Buffered stdin/stdout for the RPC protocol.
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin);
    let writer = Arc::new(Mutex::new(BufWriter::new(stdout)));

    // Spawn a task to forward filesystem watch notifications to stdout.
    {
        let writer = Arc::clone(&writer);
        let ws = Arc::clone(&watch_state);
        tokio::spawn(async move {
            let Some(mut rx) = ws.take_receiver().await else {
                return;
            };
            while let Some(notification) = rx.recv().await {
                let mut w = writer.lock().await;
                if let Err(e) = write_notification(&mut *w, &notification).await {
                    eprintln!("tramp-agent: failed to send notification: {e}");
                    break;
                }
            }
        });
    }

    // Main request loop — read requests from stdin and dispatch them.
    //
    // Requests are processed sequentially on the main task.  For true
    // concurrency the client should use the `batch` method with
    // `parallel: true`, or send requests without waiting for responses
    // (the protocol supports concurrent in-flight requests matched by id,
    // but for simplicity the initial implementation is sequential).
    loop {
        let incoming = match read_message(&mut reader).await {
            Ok(msg) => msg,
            Err(rpc::RpcError::ConnectionClosed) => {
                eprintln!("tramp-agent: stdin closed, shutting down");
                break;
            }
            Err(e) => {
                eprintln!("tramp-agent: read error: {e}");
                // Try to send an error response if we can figure out an id.
                // Since we can't, just log and continue (or break on I/O errors).
                if matches!(e, rpc::RpcError::Io(_)) {
                    break;
                }
                continue;
            }
        };

        match incoming {
            Incoming::Request(req) => {
                let response = dispatch(
                    &req.method,
                    req.id,
                    &req.params,
                    &process_table,
                    &watch_state,
                )
                .await;

                let mut w = writer.lock().await;
                if let Err(e) = write_response(&mut *w, &response).await {
                    eprintln!("tramp-agent: write error: {e}");
                    break;
                }
            }
        }
    }

    // Flush stdout before exiting.
    {
        let mut w = writer.lock().await;
        let _ = w.flush().await;
    }

    eprintln!("tramp-agent: exiting");
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rmpv::Value;

    fn empty_params() -> Value {
        Value::Map(vec![])
    }

    #[tokio::test]
    async fn dispatch_ping() {
        let pt = ProcessTable::new();
        let ws = WatchState::new();
        let resp = dispatch("ping", 1, &empty_params(), &pt, &ws).await;
        assert!(
            resp.error.is_none(),
            "ping should succeed: {:?}",
            resp.error
        );

        let result = resp.result.unwrap();
        let map = result.as_map().unwrap();

        let status = map
            .iter()
            .find(|(k, _)| k.as_str() == Some("status"))
            .unwrap()
            .1
            .as_str()
            .unwrap();
        assert_eq!(status, "ok");

        let version = map
            .iter()
            .find(|(k, _)| k.as_str() == Some("version"))
            .unwrap()
            .1
            .as_str()
            .unwrap();
        assert_eq!(version, env!("CARGO_PKG_VERSION"));
    }

    #[tokio::test]
    async fn dispatch_unknown_method() {
        let pt = ProcessTable::new();
        let ws = WatchState::new();
        let resp = dispatch("nonexistent.method", 42, &empty_params(), &pt, &ws).await;
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, error_code::METHOD_NOT_FOUND);
    }

    #[tokio::test]
    async fn dispatch_file_stat() {
        let pt = ProcessTable::new();
        let ws = WatchState::new();
        let params = Value::Map(vec![(
            Value::String("path".into()),
            Value::String("/".into()),
        )]);
        let resp = dispatch("file.stat", 2, &params, &pt, &ws).await;
        assert!(
            resp.error.is_none(),
            "stat / should succeed: {:?}",
            resp.error
        );

        let result = resp.result.unwrap();
        let map = result.as_map().unwrap();
        let kind = map
            .iter()
            .find(|(k, _)| k.as_str() == Some("kind"))
            .unwrap()
            .1
            .as_str()
            .unwrap();
        assert_eq!(kind, "dir");
    }

    #[tokio::test]
    async fn dispatch_system_info() {
        let pt = ProcessTable::new();
        let ws = WatchState::new();
        let resp = dispatch("system.info", 3, &empty_params(), &pt, &ws).await;
        assert!(
            resp.error.is_none(),
            "system.info should succeed: {:?}",
            resp.error
        );

        let result = resp.result.unwrap();
        let map = result.as_map().unwrap();

        // Should have os field.
        let os = map
            .iter()
            .find(|(k, _)| k.as_str() == Some("os"))
            .unwrap()
            .1
            .as_str()
            .unwrap();
        assert!(!os.is_empty());
    }

    #[tokio::test]
    async fn dispatch_dir_list() {
        let pt = ProcessTable::new();
        let ws = WatchState::new();
        let params = Value::Map(vec![(
            Value::String("path".into()),
            Value::String("/tmp".into()),
        )]);
        let resp = dispatch("dir.list", 4, &params, &pt, &ws).await;
        assert!(
            resp.error.is_none(),
            "dir.list /tmp should succeed: {:?}",
            resp.error
        );

        let result = resp.result.unwrap();
        let map = result.as_map().unwrap();
        assert!(
            map.iter().any(|(k, _)| k.as_str() == Some("entries")),
            "response should have entries field"
        );
    }

    #[tokio::test]
    async fn dispatch_process_run() {
        let pt = ProcessTable::new();
        let ws = WatchState::new();
        let params = Value::Map(vec![
            (
                Value::String("program".into()),
                Value::String("echo".into()),
            ),
            (
                Value::String("args".into()),
                Value::Array(vec![Value::String("dispatch_test".into())]),
            ),
        ]);
        let resp = dispatch("process.run", 5, &params, &pt, &ws).await;
        assert!(
            resp.error.is_none(),
            "process.run should succeed: {:?}",
            resp.error
        );

        let result = resp.result.unwrap();
        let map = result.as_map().unwrap();
        let stdout = map
            .iter()
            .find(|(k, _)| k.as_str() == Some("stdout"))
            .unwrap()
            .1
            .as_slice()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(stdout).trim(), "dispatch_test");
    }

    #[tokio::test]
    async fn dispatch_system_getenv() {
        let pt = ProcessTable::new();
        let ws = WatchState::new();
        let params = Value::Map(vec![(
            Value::String("name".into()),
            Value::String("HOME".into()),
        )]);
        let resp = dispatch("system.getenv", 6, &params, &pt, &ws).await;
        assert!(resp.error.is_none());

        let result = resp.result.unwrap();
        let map = result.as_map().unwrap();
        let value = &map
            .iter()
            .find(|(k, _)| k.as_str() == Some("value"))
            .unwrap()
            .1;
        assert!(!value.is_nil(), "HOME should be set");
    }

    #[tokio::test]
    async fn dispatch_system_statvfs() {
        let pt = ProcessTable::new();
        let ws = WatchState::new();
        let params = Value::Map(vec![(
            Value::String("path".into()),
            Value::String("/".into()),
        )]);
        let resp = dispatch("system.statvfs", 7, &params, &pt, &ws).await;
        assert!(
            resp.error.is_none(),
            "statvfs / should succeed: {:?}",
            resp.error
        );

        let result = resp.result.unwrap();
        let map = result.as_map().unwrap();
        let total = map
            .iter()
            .find(|(k, _)| k.as_str() == Some("total_bytes"))
            .unwrap()
            .1
            .as_u64()
            .unwrap();
        assert!(total > 0);
    }

    /// Verify that a full request → response round-trip works through the
    /// wire format (serialize request, write to buffer, read back, dispatch,
    /// write response, read response).
    #[tokio::test]
    async fn full_wire_round_trip() {
        use rpc::{Incoming, Request, read_message, write_response};

        // Build a request.
        let req = Request::new(100, "ping", Value::Map(vec![]));

        // Manually construct the length-prefixed wire bytes (simulating
        // what the client side would send).
        let mut wire = Vec::new();
        let payload = rmp_serde::to_vec_named(&req).unwrap();
        let len = payload.len() as u32;
        wire.extend_from_slice(&len.to_be_bytes());
        wire.extend_from_slice(&payload);

        // Read the request back (simulating the agent's read path).
        let mut cursor = std::io::Cursor::new(&wire);
        let incoming = read_message(&mut cursor).await.unwrap();

        let Incoming::Request(parsed_req) = incoming;
        assert_eq!(parsed_req.id, 100);
        assert_eq!(parsed_req.method, "ping");

        // Dispatch it.
        let pt = ProcessTable::new();
        let ws = WatchState::new();
        let resp = dispatch(
            &parsed_req.method,
            parsed_req.id,
            &parsed_req.params,
            &pt,
            &ws,
        )
        .await;

        assert!(resp.error.is_none());
        assert_eq!(resp.id, 100);

        // Write the response to a buffer.
        let mut resp_wire = Vec::new();
        write_response(&mut resp_wire, &resp).await.unwrap();

        // Verify we can decode the response payload.
        let resp_payload = &resp_wire[4..];
        let value: Value = rmp_serde::from_slice(resp_payload).unwrap();
        let map = value.as_map().unwrap();

        let id = map
            .iter()
            .find(|(k, _)| k.as_str() == Some("id"))
            .unwrap()
            .1
            .as_u64()
            .unwrap();
        assert_eq!(id, 100);
    }
}
