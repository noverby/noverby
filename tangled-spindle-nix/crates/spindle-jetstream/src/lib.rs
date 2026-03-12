//! Jetstream client for AT Protocol event ingestion in `tangled-spindle-nix`.
//!
//! This crate provides a WebSocket client that connects to the AT Protocol
//! Jetstream endpoint and ingests events relevant to the spindle runner.
//! Matches the upstream Go spindle's `ingester.go` behavior.
//!
//! # Event Types
//!
//! The client filters for the following AT Protocol collections:
//! - `sh.tangled.spindle.member` — Spindle membership records.
//! - `sh.tangled.repo` — Repository records pointing at this spindle.
//! - `sh.tangled.repo.collaborator` — Repository collaborator records.
//!
//! # Connection Management
//!
//! - Cursor-based replay on reconnection (persisted via `spindle-db`).
//! - DID-based subscription management (`add_did`, `remove_did`).
//! - Automatic reconnection with exponential backoff.
//!
//! # Architecture
//!
//! The client runs as a long-lived async task, feeding parsed events into
//! a channel for the main server to process via ingestion handlers
//! (`ingest_member`, `ingest_repo`, `ingest_collaborator`).
