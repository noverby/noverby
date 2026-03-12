//! RBAC enforcement for `tangled-spindle-nix`.
//!
//! This crate provides role-based access control using `casbin-rs`, matching
//! the upstream Go spindle's RBAC model and policy definitions.
//!
//! # Roles
//!
//! - **Owner** — The spindle operator (configured via `SPINDLE_SERVER_OWNER`).
//! - **Member** — DIDs that have been invited to use this spindle.
//! - **Collaborator** — Per-repo collaborators with limited permissions.
//!
//! # Operations
//!
//! - `add_spindle` / `add_spindle_owner` / `add_spindle_member` / `remove_spindle_member`
//! - `is_spindle_invite_allowed`
//! - `add_repo` / `add_collaborator` / `is_collaborator_invite_allowed`
//! - `get_spindle_users_by_role`
