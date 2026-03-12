//! SQLite database layer for `tangled-spindle-nix`.
//!
//! This crate provides persistent storage for the spindle runner using SQLite
//! (via `rusqlite` with WAL mode). It manages:
//!
//! - **Repos** — Tracked repositories that this spindle watches.
//! - **Members** — Spindle membership records (DIDs allowed to use this spindle).
//! - **Events** — Pipeline event log (for WebSocket backfill on `/events`).
//! - **Status** — Workflow execution status tracking.
//! - **Cursor** — Jetstream cursor persistence for reconnection.
//!
//! # Schema
//!
//! Migrations are embedded via `include_str!` and applied automatically on
//! database open. The schema matches the upstream Go spindle's SQLite tables.
