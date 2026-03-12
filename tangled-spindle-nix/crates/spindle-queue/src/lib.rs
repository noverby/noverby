//! Bounded job queue with configurable workers for `tangled-spindle-nix`.
//!
//! This crate provides a bounded, async job queue that limits the number of
//! concurrent workflow executions. Matches the upstream Go spindle's `queue.go`
//! behavior.
//!
//! # Configuration
//!
//! - **Max jobs** — Maximum number of workflows executing concurrently
//!   (default: 2, configurable via `SPINDLE_ENGINE_MAX_JOBS` / NixOS module).
//! - **Queue size** — Maximum number of pending jobs waiting for a worker
//!   (default: 100). Jobs submitted beyond this limit are rejected.
//!
//! # Architecture
//!
//! The queue uses a `tokio::sync::Semaphore` to limit concurrency and a
//! bounded `tokio::sync::mpsc` channel for backpressure. Each dequeued job
//! is spawned as a `tokio::task` that acquires a semaphore permit before
//! executing the workflow.
