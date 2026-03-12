//! Secrets manager for `tangled-spindle-nix`.
//!
//! This crate provides secret storage and retrieval for workflow execution,
//! supporting both SQLite (encrypted at rest) and OpenBao backends. Matches
//! the upstream Go spindle's secrets management interface.
//!
//! # Backends
//!
//! - **SQLite** — Secrets are encrypted using AES-GCM and stored in the same
//!   SQLite database as other spindle state. Suitable for single-node deployments.
//! - **OpenBao** — Proxies secret operations to an OpenBao (Vault-compatible)
//!   server via its HTTP API. Secrets are stored in a KV v2 mount at a
//!   configurable path. Suitable for multi-node or enterprise deployments.
//!
//! # Secret Path Convention
//!
//! Secrets are scoped per-repository using a sanitized path:
//! - `did:plc:alice/myrepo` → `did_plc_alice_myrepo`
//! - Full OpenBao path: `{mount}/repos/{sanitized_repo_path}/{key}`
//!
//! # Injection
//!
//! Secrets are injected into workflow steps as environment variables.
//! The [`UnlockedSecret`] type represents a decrypted secret ready for
//! injection. Secret values are masked in log output via
//! [`SecretMask`](spindle_models::SecretMask).
