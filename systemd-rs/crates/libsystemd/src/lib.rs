#![allow(clippy::result_large_err)]
#![allow(clippy::large_enum_variant)]

//! `libsystemd` is the core library for systemd-rs, providing shared
//! functionality used by the service manager (`systemd`), control tool
//! (`systemctl`), and all other systemd-rs components.
//!
//! It contains:
//! - Unit file parsing (INI-style with systemd extensions)
//! - Dependency graph engine (topological sort, cycle detection)
//! - sd_notify protocol implementation
//! - Socket activation support
//! - Platform abstractions (cgroups, eventfd, subreaper, etc.)
//! - Service lifecycle management
//! - Configuration loading
//! - Control interface (JSON-RPC 2.0)

pub mod config;
pub mod control;
pub mod dbus_wait;
pub mod entrypoints;
pub mod fd_store;
pub mod generators;
pub mod journal;
pub mod lock_ext;
pub mod logging;
pub mod notification_handler;
pub mod platform;
pub mod runtime_info;
pub mod services;
pub mod shutdown;
pub mod signal_handler;
pub mod socket_activation;
pub mod sockets;
pub mod unit_name;
pub mod units;

#[cfg(test)]
mod tests;
