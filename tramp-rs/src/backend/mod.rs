//! Backend trait and registry.
//!
//! Each transport (SSH, Docker, Kubernetes, sudo, …) implements the [`Backend`]
//! trait, which provides the primitive file-system and exec operations that the
//! VFS layer delegates to.

#![allow(dead_code)]

use async_trait::async_trait;
use bytes::Bytes;
use std::time::SystemTime;

pub mod ssh;

// ---------------------------------------------------------------------------
// Types returned by backend operations
// ---------------------------------------------------------------------------

/// The kind of a directory entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    File,
    Dir,
    Symlink,
}

/// A single entry returned by [`Backend::list`].
#[derive(Debug, Clone)]
pub struct DirEntry {
    pub name: String,
    pub kind: EntryKind,
    pub size: Option<u64>,
    pub modified: Option<SystemTime>,
    pub permissions: Option<u32>,
}

/// Metadata for a remote path returned by [`Backend::stat`].
#[derive(Debug, Clone)]
pub struct Metadata {
    pub kind: EntryKind,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub permissions: Option<u32>,
}

/// Result of running a command on the remote via [`Backend::exec`].
#[derive(Debug, Clone)]
pub struct ExecResult {
    pub stdout: Bytes,
    pub stderr: Bytes,
    pub exit_code: i32,
}

// ---------------------------------------------------------------------------
// Backend trait
// ---------------------------------------------------------------------------

/// A transport backend capable of performing remote file I/O and command
/// execution.
///
/// All operations are async.  The VFS layer is responsible for creating a
/// tokio runtime and blocking on these futures when called from the
/// synchronous Nushell plugin interface.
#[async_trait]
pub trait Backend: Send + Sync {
    /// Read the entire contents of a remote file.
    async fn read(&self, path: &str) -> Result<Bytes, crate::errors::TrampError>;

    /// Write `data` to a remote file, creating or truncating it.
    async fn write(&self, path: &str, data: Bytes) -> Result<(), crate::errors::TrampError>;

    /// List the entries in a remote directory.
    async fn list(&self, path: &str) -> Result<Vec<DirEntry>, crate::errors::TrampError>;

    /// Get metadata for a remote path.
    async fn stat(&self, path: &str) -> Result<Metadata, crate::errors::TrampError>;

    /// Execute a command on the remote and collect its output.
    async fn exec(&self, cmd: &str, args: &[&str])
    -> Result<ExecResult, crate::errors::TrampError>;

    /// Delete a remote file (or empty directory).
    async fn delete(&self, path: &str) -> Result<(), crate::errors::TrampError>;
}
