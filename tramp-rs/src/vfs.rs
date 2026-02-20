#![allow(dead_code)]

//! Virtual filesystem layer.
//!
//! The VFS sits between the Nushell plugin commands and the transport
//! backends.  Its responsibilities are:
//!
//! - Resolve a [`TrampPath`] to a concrete [`Backend`] instance.
//! - Connection pooling: reuse open SSH sessions keyed by
//!   `(backend, user, host, port)`.
//! - Provide a **synchronous** API that the Nushell plugin commands can call
//!   (internally it owns a tokio runtime and blocks on async operations).
//!
//! Phase 1 supports single-hop SSH only.  Chained paths produce an error
//! with a clear message pointing at the roadmap.

use bytes::Bytes;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::backend::ssh::SshBackend;
use crate::backend::{Backend, DirEntry, ExecResult, Metadata};
use crate::errors::{TrampError, TrampResult};
use crate::protocol::{BackendKind, Hop, TrampPath};

// ---------------------------------------------------------------------------
// Connection key
// ---------------------------------------------------------------------------

/// Cache key for an open backend connection.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ConnectionKey {
    backend: BackendKind,
    user: Option<String>,
    host: String,
    port: Option<u16>,
}

impl From<&Hop> for ConnectionKey {
    fn from(hop: &Hop) -> Self {
        Self {
            backend: hop.backend,
            user: hop.user.clone(),
            host: hop.host.clone(),
            port: hop.port,
        }
    }
}

// ---------------------------------------------------------------------------
// VFS
// ---------------------------------------------------------------------------

/// The virtual filesystem manager.
///
/// Holds a tokio runtime (created once) and a pool of open backend
/// connections.  All public methods are synchronous — they block the
/// calling thread on the internal runtime.
pub struct Vfs {
    runtime: tokio::runtime::Runtime,
    /// Connection pool keyed by `(backend, user, host, port)`.
    ///
    /// The `Arc<dyn Backend>` is shared so that multiple concurrent calls
    /// can reuse the same underlying session.
    pool: Mutex<HashMap<ConnectionKey, Arc<dyn Backend>>>,
}

impl Vfs {
    /// Create a new VFS with its own tokio runtime.
    pub fn new() -> TrampResult<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|e| TrampError::Internal(format!("failed to create tokio runtime: {e}")))?;

        Ok(Self {
            runtime,
            pool: Mutex::new(HashMap::new()),
        })
    }

    // -----------------------------------------------------------------------
    // Resolve a TrampPath to a backend
    // -----------------------------------------------------------------------

    /// Validate a [`TrampPath`] and return the single hop for Phase 1.
    ///
    /// Returns an error for chained (multi-hop) paths or unsupported backends.
    fn validate_single_hop(path: &TrampPath) -> TrampResult<&Hop> {
        if path.hops.len() > 1 {
            return Err(TrampError::ChainedPathNotSupported);
        }

        let hop = &path.hops[0];
        match hop.backend {
            BackendKind::Ssh => Ok(hop),
            other => Err(TrampError::BackendNotSupported(other.to_string())),
        }
    }

    /// Get or create a backend connection for the given hop.
    ///
    /// This must be called from within the tokio runtime context.
    async fn get_or_connect(
        pool: &Mutex<HashMap<ConnectionKey, Arc<dyn Backend>>>,
        hop: &Hop,
    ) -> TrampResult<Arc<dyn Backend>> {
        let key = ConnectionKey::from(hop);

        // Fast path: check if we already have a connection.
        {
            let pool_guard = pool
                .lock()
                .map_err(|e| TrampError::Internal(format!("pool lock poisoned: {e}")))?;
            if let Some(backend) = pool_guard.get(&key) {
                return Ok(Arc::clone(backend));
            }
        }

        // Slow path: create a new connection.
        let backend: Arc<dyn Backend> = match hop.backend {
            BackendKind::Ssh => {
                let ssh = SshBackend::connect(&hop.host, hop.user.as_deref(), hop.port).await?;
                Arc::new(ssh)
            }
            other => return Err(TrampError::BackendNotSupported(other.to_string())),
        };

        // Store in pool.
        {
            let mut pool_guard = pool
                .lock()
                .map_err(|e| TrampError::Internal(format!("pool lock poisoned: {e}")))?;
            pool_guard.insert(key, Arc::clone(&backend));
        }

        Ok(backend)
    }

    /// Resolve a [`TrampPath`] to a `(backend, remote_path)` pair.
    fn resolve(&self, path: &TrampPath) -> TrampResult<(Arc<dyn Backend>, String)> {
        let hop = Self::validate_single_hop(path)?;
        let pool = &self.pool;
        let backend = self.runtime.block_on(Self::get_or_connect(pool, hop))?;
        Ok((backend, path.remote_path.clone()))
    }

    // -----------------------------------------------------------------------
    // Public synchronous API
    // -----------------------------------------------------------------------

    /// Read the contents of a remote file.
    pub fn read(&self, path: &TrampPath) -> TrampResult<Bytes> {
        let (backend, remote_path) = self.resolve(path)?;
        self.runtime.block_on(backend.read(&remote_path))
    }

    /// Write data to a remote file, creating or truncating it.
    pub fn write(&self, path: &TrampPath, data: Bytes) -> TrampResult<()> {
        let (backend, remote_path) = self.resolve(path)?;
        self.runtime.block_on(backend.write(&remote_path, data))
    }

    /// List entries in a remote directory.
    pub fn list(&self, path: &TrampPath) -> TrampResult<Vec<DirEntry>> {
        let (backend, remote_path) = self.resolve(path)?;
        self.runtime.block_on(backend.list(&remote_path))
    }

    /// Get metadata for a remote path.
    pub fn stat(&self, path: &TrampPath) -> TrampResult<Metadata> {
        let (backend, remote_path) = self.resolve(path)?;
        self.runtime.block_on(backend.stat(&remote_path))
    }

    /// Execute a command on the remote host described by `path`.
    pub fn exec(&self, path: &TrampPath, cmd: &str, args: &[&str]) -> TrampResult<ExecResult> {
        let (backend, _) = self.resolve(path)?;
        self.runtime.block_on(backend.exec(cmd, args))
    }

    /// Delete a remote file.
    pub fn delete(&self, path: &TrampPath) -> TrampResult<()> {
        let (backend, remote_path) = self.resolve(path)?;
        self.runtime.block_on(backend.delete(&remote_path))
    }

    // -----------------------------------------------------------------------
    // Connection management
    // -----------------------------------------------------------------------

    /// Return the number of currently pooled connections.
    pub fn connection_count(&self) -> usize {
        self.pool.lock().map(|p| p.len()).unwrap_or(0)
    }

    /// Drop all pooled connections.
    pub fn disconnect_all(&self) {
        if let Ok(mut pool) = self.pool.lock() {
            pool.clear();
        }
    }

    /// Drop the pooled connection for a specific host (matching any user/port
    /// combination whose host field equals `host`).
    pub fn disconnect_host(&self, host: &str) {
        if let Ok(mut pool) = self.pool.lock() {
            pool.retain(|key, _| key.host != host);
        }
    }

    /// Return a snapshot of the currently active connection keys (for
    /// display in `tramp connections`).
    pub fn active_connections(&self) -> Vec<String> {
        let pool = match self.pool.lock() {
            Ok(p) => p,
            Err(_) => return Vec::new(),
        };

        pool.keys()
            .map(|key| {
                let mut s = format!("{}", key.backend);
                s.push(':');
                if let Some(ref user) = key.user {
                    s.push_str(user);
                    s.push('@');
                }
                s.push_str(&key.host);
                if let Some(port) = key.port {
                    s.push('#');
                    s.push_str(&port.to_string());
                }
                s
            })
            .collect()
    }
}

impl Default for Vfs {
    fn default() -> Self {
        Self::new().expect("failed to create VFS")
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::BackendKind;

    fn make_path(host: &str, remote: &str) -> TrampPath {
        TrampPath {
            hops: vec![Hop {
                backend: BackendKind::Ssh,
                host: host.to_string(),
                user: None,
                port: None,
            }],
            remote_path: remote.to_string(),
        }
    }

    #[test]
    fn rejects_chained_paths() {
        let path = TrampPath {
            hops: vec![
                Hop {
                    backend: BackendKind::Ssh,
                    host: "jump".to_string(),
                    user: None,
                    port: None,
                },
                Hop {
                    backend: BackendKind::Ssh,
                    host: "target".to_string(),
                    user: None,
                    port: None,
                },
            ],
            remote_path: "/etc/config".to_string(),
        };

        let err = Vfs::validate_single_hop(&path).unwrap_err();
        assert!(matches!(err, TrampError::ChainedPathNotSupported));
    }

    #[test]
    fn rejects_unsupported_backend() {
        let path = TrampPath {
            hops: vec![Hop {
                backend: BackendKind::Docker,
                host: "mycontainer".to_string(),
                user: None,
                port: None,
            }],
            remote_path: "/app".to_string(),
        };

        let err = Vfs::validate_single_hop(&path).unwrap_err();
        assert!(matches!(err, TrampError::BackendNotSupported(_)));
    }

    #[test]
    fn accepts_single_ssh_hop() {
        let path = make_path("myvm", "/etc/config");
        let hop = Vfs::validate_single_hop(&path).unwrap();
        assert_eq!(hop.host, "myvm");
        assert_eq!(hop.backend, BackendKind::Ssh);
    }

    #[test]
    fn vfs_creates_successfully() {
        let vfs = Vfs::new().unwrap();
        assert_eq!(vfs.connection_count(), 0);
    }

    #[test]
    fn disconnect_all_clears_pool() {
        let vfs = Vfs::new().unwrap();
        assert_eq!(vfs.connection_count(), 0);
        vfs.disconnect_all();
        assert_eq!(vfs.connection_count(), 0);
    }

    #[test]
    fn active_connections_empty_initially() {
        let vfs = Vfs::new().unwrap();
        assert!(vfs.active_connections().is_empty());
    }
}
