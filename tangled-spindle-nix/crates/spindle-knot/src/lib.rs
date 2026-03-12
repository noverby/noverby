//! Knot event consumer for pipeline events in `tangled-spindle-nix`.
//!
//! This crate provides an event consumer that subscribes to knot HTTP event
//! streams and filters for `sh.tangled.pipeline` events. Matches the upstream
//! Go spindle's knot event consumer behavior.
//!
//! # Event Flow
//!
//! 1. When a repository is registered with this spindle (via Jetstream ingestion),
//!    the consumer subscribes to that repo's knot server event stream.
//! 2. The consumer filters for `sh.tangled.pipeline` events on subscribed knots.
//! 3. Pipeline events are forwarded to the main server for processing and
//!    enqueuing into the job queue.
//!
//! # Connection Management
//!
//! - Cursor-based replay on reconnection.
//! - Dynamic source management: knots can be added/removed at runtime as repos
//!   are registered/unregistered with this spindle.
//! - Automatic reconnection with exponential backoff.
