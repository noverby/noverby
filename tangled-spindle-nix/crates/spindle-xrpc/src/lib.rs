//! XRPC route handlers and service auth for `tangled-spindle-nix`.
//!
//! This crate provides the AT Protocol XRPC endpoint handlers and service
//! authentication verification, matching the upstream Go spindle's XRPC
//! implementation.
//!
//! # Endpoints
//!
//! - **Service auth** — Verify incoming XRPC requests using AT Protocol
//!   service authentication (JWT-based DID verification).
//! - **Member management** — Add/remove spindle members via XRPC calls.
//! - **Secret management** — CRUD operations for per-repo secrets
//!   (`putRecord`, `getRecord`, `listRecords`, `deleteRecord`).
//! - **Pipeline control** — Cancel running pipelines.
//!
//! # Architecture
//!
//! Handlers are implemented as `axum` extractors and handler functions,
//! mounted under `/xrpc/*` on the main HTTP server. Each handler delegates
//! to the appropriate `spindle-db`, `spindle-rbac`, or `spindle-secrets`
//! crate for business logic.
