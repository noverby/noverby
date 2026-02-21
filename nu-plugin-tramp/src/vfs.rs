#![allow(dead_code)]

//! Virtual filesystem layer.
//!
//! The VFS sits between the Nushell plugin commands and the transport
//! backends.  Its responsibilities are:
//!
//! - Resolve a [`TrampPath`] to a concrete [`Backend`] instance.
//! - Connection pooling: reuse open SSH sessions keyed by
//!   `(backend, user, host, port)`.
//! - Connection health-checking: verify pooled connections are alive
//!   before reusing them; reconnect on failure.
//! - Stat cache: cache metadata results with a configurable TTL to
//!   avoid redundant remote calls.
//! - Provide a **synchronous** API that the Nushell plugin commands can call
//!   (internally it owns a tokio runtime and blocks on async operations).
//!
//! Phase 1 supports single-hop SSH only.  Chained paths produce an error
//! with a clear message pointing at the roadmap.

use bytes::Bytes;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::backend::ssh::SshBackend;
use crate::backend::{Backend, DirEntry, ExecResult, Metadata};
use crate::errors::{TrampError, TrampResult};
use crate::protocol::{BackendKind, Hop, TrampPath};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Default time-to-live for cached stat entries.
const DEFAULT_STAT_TTL: Duration = Duration::from_secs(5);

/// Default time-to-live for cached directory listings.
const DEFAULT_LIST_TTL: Duration = Duration::from_secs(5);

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
// Stat cache
// ---------------------------------------------------------------------------

/// A single cached entry with its insertion timestamp.
#[derive(Debug, Clone)]
struct CacheEntry<T> {
    value: T,
    inserted_at: Instant,
}

impl<T> CacheEntry<T> {
    fn new(value: T) -> Self {
        Self {
            value,
            inserted_at: Instant::now(),
        }
    }

    fn is_expired(&self, ttl: Duration) -> bool {
        self.inserted_at.elapsed() > ttl
    }
}

/// A TTL-based cache for stat (metadata) results keyed by
/// `(ConnectionKey, remote_path)`.
struct StatCache {
    entries: HashMap<(ConnectionKey, String), CacheEntry<Metadata>>,
    ttl: Duration,
}

impl StatCache {
    fn new(ttl: Duration) -> Self {
        Self {
            entries: HashMap::new(),
            ttl,
        }
    }

    /// Look up a cached stat result.  Returns `None` if the entry is
    /// missing or expired (expired entries are removed eagerly).
    fn get(&mut self, key: &ConnectionKey, path: &str) -> Option<Metadata> {
        let cache_key = (key.clone(), path.to_string());
        if let Some(entry) = self.entries.get(&cache_key) {
            if entry.is_expired(self.ttl) {
                self.entries.remove(&cache_key);
                None
            } else {
                Some(entry.value.clone())
            }
        } else {
            None
        }
    }

    /// Insert or replace a stat result in the cache.
    fn insert(&mut self, key: &ConnectionKey, path: &str, metadata: Metadata) {
        let cache_key = (key.clone(), path.to_string());
        self.entries.insert(cache_key, CacheEntry::new(metadata));
    }

    /// Invalidate (remove) a specific cached entry.
    fn invalidate(&mut self, key: &ConnectionKey, path: &str) {
        let cache_key = (key.clone(), path.to_string());
        self.entries.remove(&cache_key);
    }

    /// Invalidate all entries under a given connection key whose path starts
    /// with `prefix`.  Useful after writes or deletes.
    fn invalidate_prefix(&mut self, key: &ConnectionKey, prefix: &str) {
        self.entries
            .retain(|(k, p), _| !(k == key && p.starts_with(prefix)));
    }

    /// Invalidate all entries for a given connection key.
    fn invalidate_connection(&mut self, key: &ConnectionKey) {
        self.entries.retain(|(k, _), _| k != key);
    }

    /// Remove all expired entries (garbage collection).
    fn evict_expired(&mut self) {
        let ttl = self.ttl;
        self.entries.retain(|_, entry| !entry.is_expired(ttl));
    }

    /// Drop everything.
    fn clear(&mut self) {
        self.entries.clear();
    }

    /// Number of entries currently in the cache (including possibly expired).
    fn len(&self) -> usize {
        self.entries.len()
    }
}

/// A TTL-based cache for directory listing results.
struct ListCache {
    entries: HashMap<(ConnectionKey, String), CacheEntry<Vec<DirEntry>>>,
    ttl: Duration,
}

impl ListCache {
    fn new(ttl: Duration) -> Self {
        Self {
            entries: HashMap::new(),
            ttl,
        }
    }

    fn get(&mut self, key: &ConnectionKey, path: &str) -> Option<Vec<DirEntry>> {
        let cache_key = (key.clone(), path.to_string());
        if let Some(entry) = self.entries.get(&cache_key) {
            if entry.is_expired(self.ttl) {
                self.entries.remove(&cache_key);
                None
            } else {
                Some(entry.value.clone())
            }
        } else {
            None
        }
    }

    fn insert(&mut self, key: &ConnectionKey, path: &str, entries: Vec<DirEntry>) {
        let cache_key = (key.clone(), path.to_string());
        self.entries.insert(cache_key, CacheEntry::new(entries));
    }

    fn invalidate(&mut self, key: &ConnectionKey, path: &str) {
        let cache_key = (key.clone(), path.to_string());
        self.entries.remove(&cache_key);
    }

    fn invalidate_prefix(&mut self, key: &ConnectionKey, prefix: &str) {
        self.entries
            .retain(|(k, p), _| !(k == key && p.starts_with(prefix)));
    }

    fn invalidate_connection(&mut self, key: &ConnectionKey) {
        self.entries.retain(|(k, _), _| k != key);
    }

    fn clear(&mut self) {
        self.entries.clear();
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
    /// Stat (metadata) cache with TTL.
    stat_cache: Mutex<StatCache>,
    /// Directory listing cache with TTL.
    list_cache: Mutex<ListCache>,
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
            stat_cache: Mutex::new(StatCache::new(DEFAULT_STAT_TTL)),
            list_cache: Mutex::new(ListCache::new(DEFAULT_LIST_TTL)),
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

    /// Create a fresh backend connection for the given hop.
    async fn connect_backend(hop: &Hop) -> TrampResult<Arc<dyn Backend>> {
        match hop.backend {
            BackendKind::Ssh => {
                let ssh = SshBackend::connect(&hop.host, hop.user.as_deref(), hop.port).await?;
                Ok(Arc::new(ssh))
            }
            other => Err(TrampError::BackendNotSupported(other.to_string())),
        }
    }

    /// Get or create a backend connection for the given hop.
    ///
    /// If a pooled connection exists, it is health-checked first.  Stale
    /// connections are dropped and a fresh one is opened transparently.
    ///
    /// This must be called from within the tokio runtime context.
    async fn get_or_connect(
        pool: &Mutex<HashMap<ConnectionKey, Arc<dyn Backend>>>,
        hop: &Hop,
    ) -> TrampResult<(ConnectionKey, Arc<dyn Backend>)> {
        let key = ConnectionKey::from(hop);

        // Fast path: check if we already have a connection.
        let existing = {
            let pool_guard = pool
                .lock()
                .map_err(|e| TrampError::Internal(format!("pool lock poisoned: {e}")))?;
            pool_guard.get(&key).cloned()
        };

        if let Some(backend) = existing {
            // Health-check the pooled connection.
            match backend.check().await {
                Ok(()) => return Ok((key, backend)),
                Err(_) => {
                    // Connection is stale — remove it from the pool and fall
                    // through to create a new one.
                    if let Ok(mut pool_guard) = pool.lock() {
                        pool_guard.remove(&key);
                    }
                }
            }
        }

        // Slow path: create a new connection.
        let backend = Self::connect_backend(hop).await?;

        // Store in pool.
        {
            let mut pool_guard = pool
                .lock()
                .map_err(|e| TrampError::Internal(format!("pool lock poisoned: {e}")))?;
            pool_guard.insert(key.clone(), Arc::clone(&backend));
        }

        Ok((key, backend))
    }

    /// Resolve a [`TrampPath`] to a `(connection_key, backend, remote_path)` triple.
    fn resolve(&self, path: &TrampPath) -> TrampResult<(ConnectionKey, Arc<dyn Backend>, String)> {
        let hop = Self::validate_single_hop(path)?;
        let pool = &self.pool;
        let (key, backend) = self.runtime.block_on(Self::get_or_connect(pool, hop))?;
        Ok((key, backend, path.remote_path.clone()))
    }

    // -----------------------------------------------------------------------
    // Cache helpers
    // -----------------------------------------------------------------------

    /// Invalidate caches that may be affected by a write to `path` under
    /// the given connection.
    fn invalidate_for_write(&self, key: &ConnectionKey, path: &str) {
        // Invalidate the exact stat entry.
        if let Ok(mut cache) = self.stat_cache.lock() {
            cache.invalidate(key, path);
        }
        // Invalidate the parent directory listing.
        if let Some(parent) = parent_dir(path)
            && let Ok(mut cache) = self.list_cache.lock()
        {
            cache.invalidate(key, parent);
        }
    }

    /// Invalidate caches that may be affected by a delete of `path` under
    /// the given connection.
    fn invalidate_for_delete(&self, key: &ConnectionKey, path: &str) {
        if let Ok(mut cache) = self.stat_cache.lock() {
            cache.invalidate(key, path);
            // Also invalidate anything under path (in case it was a directory).
            cache.invalidate_prefix(key, path);
        }
        if let Ok(mut cache) = self.list_cache.lock() {
            cache.invalidate(key, path);
            cache.invalidate_prefix(key, path);
            if let Some(parent) = parent_dir(path) {
                cache.invalidate(key, parent);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Public synchronous API
    // -----------------------------------------------------------------------

    /// Read the contents of a remote file.
    pub fn read(&self, path: &TrampPath) -> TrampResult<Bytes> {
        let (_key, backend, remote_path) = self.resolve(path)?;
        self.runtime.block_on(backend.read(&remote_path))
    }

    /// Write data to a remote file, creating or truncating it.
    pub fn write(&self, path: &TrampPath, data: Bytes) -> TrampResult<()> {
        let (key, backend, remote_path) = self.resolve(path)?;
        self.runtime.block_on(backend.write(&remote_path, data))?;
        self.invalidate_for_write(&key, &remote_path);
        Ok(())
    }

    /// List entries in a remote directory.
    pub fn list(&self, path: &TrampPath) -> TrampResult<Vec<DirEntry>> {
        let (key, backend, remote_path) = self.resolve(path)?;

        // Check list cache first.
        if let Ok(mut cache) = self.list_cache.lock()
            && let Some(entries) = cache.get(&key, &remote_path)
        {
            return Ok(entries);
        }

        let entries = self.runtime.block_on(backend.list(&remote_path))?;

        // Populate the list cache.
        if let Ok(mut cache) = self.list_cache.lock() {
            cache.insert(&key, &remote_path, entries.clone());
        }

        Ok(entries)
    }

    /// Get metadata for a remote path.
    pub fn stat(&self, path: &TrampPath) -> TrampResult<Metadata> {
        let (key, backend, remote_path) = self.resolve(path)?;

        // Check stat cache first.
        if let Ok(mut cache) = self.stat_cache.lock()
            && let Some(meta) = cache.get(&key, &remote_path)
        {
            return Ok(meta);
        }

        let meta = self.runtime.block_on(backend.stat(&remote_path))?;

        // Populate the stat cache.
        if let Ok(mut cache) = self.stat_cache.lock() {
            cache.insert(&key, &remote_path, meta.clone());
        }

        Ok(meta)
    }

    /// Execute a command on the remote host described by `path`.
    pub fn exec(&self, path: &TrampPath, cmd: &str, args: &[&str]) -> TrampResult<ExecResult> {
        let (_key, backend, _) = self.resolve(path)?;
        self.runtime.block_on(backend.exec(cmd, args))
    }

    /// Delete a remote file.
    pub fn delete(&self, path: &TrampPath) -> TrampResult<()> {
        let (key, backend, remote_path) = self.resolve(path)?;
        self.runtime.block_on(backend.delete(&remote_path))?;
        self.invalidate_for_delete(&key, &remote_path);
        Ok(())
    }

    /// Check whether a remote host is reachable (health-check the
    /// connection, opening one if necessary).
    pub fn ping(&self, path: &TrampPath) -> TrampResult<()> {
        let (_key, backend, _) = self.resolve(path)?;
        self.runtime.block_on(backend.check())
    }

    // -----------------------------------------------------------------------
    // Connection management
    // -----------------------------------------------------------------------

    /// Return the number of currently pooled connections.
    pub fn connection_count(&self) -> usize {
        self.pool.lock().map(|p| p.len()).unwrap_or(0)
    }

    /// Drop all pooled connections and clear all caches.
    pub fn disconnect_all(&self) {
        if let Ok(mut pool) = self.pool.lock() {
            pool.clear();
        }
        if let Ok(mut cache) = self.stat_cache.lock() {
            cache.clear();
        }
        if let Ok(mut cache) = self.list_cache.lock() {
            cache.clear();
        }
    }

    /// Drop the pooled connection for a specific host (matching any user/port
    /// combination whose host field equals `host`).
    pub fn disconnect_host(&self, host: &str) {
        if let Ok(mut pool) = self.pool.lock() {
            pool.retain(|key, _| key.host != host);
        }

        // Invalidate caches for any connection key matching this host,
        // regardless of whether it was in the pool.
        if let Ok(mut cache) = self.stat_cache.lock() {
            cache.entries.retain(|(k, _), _| k.host != host);
        }
        if let Ok(mut cache) = self.list_cache.lock() {
            cache.entries.retain(|(k, _), _| k.host != host);
        }
    }

    /// Return a snapshot of the currently active connection keys (for
    /// display in `tramp connections`).
    pub fn active_connections(&self) -> Vec<String> {
        let pool = match self.pool.lock() {
            Ok(p) => p,
            Err(_) => return Vec::new(),
        };

        pool.iter()
            .map(|(key, backend)| {
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
                // Append the backend's own description for richer output.
                let desc = backend.description();
                if !desc.is_empty() {
                    s.push_str(&format!(" ({})", desc));
                }
                s
            })
            .collect()
    }

    /// Return structured connection info for table display.
    pub fn active_connections_detailed(&self) -> Vec<ConnectionInfo> {
        let pool = match self.pool.lock() {
            Ok(p) => p,
            Err(_) => return Vec::new(),
        };

        pool.keys()
            .map(|key| ConnectionInfo {
                backend: key.backend.to_string(),
                user: key.user.clone(),
                host: key.host.clone(),
                port: key.port,
            })
            .collect()
    }

    /// Evict expired entries from all caches (garbage collection).
    /// This can be called periodically or before operations to keep
    /// memory usage bounded.
    pub fn evict_expired_caches(&self) {
        if let Ok(mut cache) = self.stat_cache.lock() {
            cache.evict_expired();
        }
    }

    /// Return current stat cache size (for diagnostics).
    pub fn stat_cache_size(&self) -> usize {
        self.stat_cache.lock().map(|c| c.len()).unwrap_or(0)
    }
}

impl Default for Vfs {
    fn default() -> Self {
        Self::new().expect("failed to create VFS")
    }
}

// ---------------------------------------------------------------------------
// Connection info (for structured output)
// ---------------------------------------------------------------------------

/// Structured info about an active connection, suitable for rendering as
/// a Nushell table row.
pub struct ConnectionInfo {
    pub backend: String,
    pub user: Option<String>,
    pub host: String,
    pub port: Option<u16>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Return the parent directory of a remote path, or `None` for `/`.
fn parent_dir(path: &str) -> Option<&str> {
    if path == "/" {
        return None;
    }
    let trimmed = path.trim_end_matches('/');
    match trimmed.rfind('/') {
        Some(0) => Some("/"),
        Some(idx) => Some(&trimmed[..idx]),
        None => None,
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

    fn make_key(host: &str) -> ConnectionKey {
        ConnectionKey {
            backend: BackendKind::Ssh,
            user: None,
            host: host.to_string(),
            port: None,
        }
    }

    // -- Validation ----------------------------------------------------------

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

    // -- VFS creation --------------------------------------------------------

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

    // -- Stat cache -----------------------------------------------------------

    #[test]
    fn stat_cache_insert_and_get() {
        let mut cache = StatCache::new(Duration::from_secs(60));
        let key = make_key("myvm");
        let meta = Metadata {
            kind: crate::backend::EntryKind::File,
            size: 1024,
            modified: None,
            permissions: Some(644),
        };

        cache.insert(&key, "/etc/config", meta.clone());
        let result = cache.get(&key, "/etc/config");
        assert!(result.is_some());
        assert_eq!(result.unwrap().size, 1024);
    }

    #[test]
    fn stat_cache_miss_for_unknown_path() {
        let mut cache = StatCache::new(Duration::from_secs(60));
        let key = make_key("myvm");
        assert!(cache.get(&key, "/etc/missing").is_none());
    }

    #[test]
    fn stat_cache_expires() {
        let mut cache = StatCache::new(Duration::from_millis(1));
        let key = make_key("myvm");
        let meta = Metadata {
            kind: crate::backend::EntryKind::File,
            size: 100,
            modified: None,
            permissions: None,
        };

        cache.insert(&key, "/tmp/file", meta);
        // Sleep just long enough for the TTL to expire.
        std::thread::sleep(Duration::from_millis(5));
        assert!(cache.get(&key, "/tmp/file").is_none());
    }

    #[test]
    fn stat_cache_invalidate_single() {
        let mut cache = StatCache::new(Duration::from_secs(60));
        let key = make_key("myvm");
        let meta = Metadata {
            kind: crate::backend::EntryKind::File,
            size: 100,
            modified: None,
            permissions: None,
        };

        cache.insert(&key, "/etc/config", meta);
        assert!(cache.get(&key, "/etc/config").is_some());

        cache.invalidate(&key, "/etc/config");
        assert!(cache.get(&key, "/etc/config").is_none());
    }

    #[test]
    fn stat_cache_invalidate_prefix() {
        let mut cache = StatCache::new(Duration::from_secs(60));
        let key = make_key("myvm");
        let meta = Metadata {
            kind: crate::backend::EntryKind::File,
            size: 100,
            modified: None,
            permissions: None,
        };

        cache.insert(&key, "/app/config", meta.clone());
        cache.insert(&key, "/app/data", meta.clone());
        cache.insert(&key, "/etc/other", meta);

        cache.invalidate_prefix(&key, "/app");
        assert!(cache.get(&key, "/app/config").is_none());
        assert!(cache.get(&key, "/app/data").is_none());
        assert!(cache.get(&key, "/etc/other").is_some());
    }

    #[test]
    fn stat_cache_invalidate_connection() {
        let mut cache = StatCache::new(Duration::from_secs(60));
        let key1 = make_key("vm1");
        let key2 = make_key("vm2");
        let meta = Metadata {
            kind: crate::backend::EntryKind::File,
            size: 100,
            modified: None,
            permissions: None,
        };

        cache.insert(&key1, "/etc/config", meta.clone());
        cache.insert(&key2, "/etc/config", meta);

        cache.invalidate_connection(&key1);
        assert!(cache.get(&key1, "/etc/config").is_none());
        assert!(cache.get(&key2, "/etc/config").is_some());
    }

    #[test]
    fn stat_cache_evict_expired() {
        let mut cache = StatCache::new(Duration::from_millis(1));
        let key = make_key("myvm");
        let meta = Metadata {
            kind: crate::backend::EntryKind::File,
            size: 100,
            modified: None,
            permissions: None,
        };

        cache.insert(&key, "/a", meta.clone());
        cache.insert(&key, "/b", meta);
        assert_eq!(cache.len(), 2);

        std::thread::sleep(Duration::from_millis(5));
        cache.evict_expired();
        assert_eq!(cache.len(), 0);
    }

    // -- List cache -----------------------------------------------------------

    #[test]
    fn list_cache_insert_and_get() {
        let mut cache = ListCache::new(Duration::from_secs(60));
        let key = make_key("myvm");
        let entries = vec![DirEntry {
            name: "file.txt".to_string(),
            kind: crate::backend::EntryKind::File,
            size: Some(42),
            modified: None,
            permissions: Some(644),
        }];

        cache.insert(&key, "/app", entries.clone());
        let result = cache.get(&key, "/app");
        assert!(result.is_some());
        assert_eq!(result.unwrap().len(), 1);
    }

    #[test]
    fn list_cache_expires() {
        let mut cache = ListCache::new(Duration::from_millis(1));
        let key = make_key("myvm");
        let entries = vec![];
        cache.insert(&key, "/app", entries);

        std::thread::sleep(Duration::from_millis(5));
        assert!(cache.get(&key, "/app").is_none());
    }

    // -- parent_dir helper ---------------------------------------------------

    #[test]
    fn parent_dir_root() {
        assert_eq!(parent_dir("/"), None);
    }

    #[test]
    fn parent_dir_top_level() {
        assert_eq!(parent_dir("/etc"), Some("/"));
    }

    #[test]
    fn parent_dir_nested() {
        assert_eq!(parent_dir("/etc/config"), Some("/etc"));
    }

    #[test]
    fn parent_dir_deep() {
        assert_eq!(parent_dir("/a/b/c/d"), Some("/a/b/c"));
    }

    #[test]
    fn parent_dir_trailing_slash() {
        assert_eq!(parent_dir("/etc/config/"), Some("/etc"));
    }

    // -- VFS stat cache integration ------------------------------------------

    #[test]
    fn vfs_stat_cache_starts_empty() {
        let vfs = Vfs::new().unwrap();
        assert_eq!(vfs.stat_cache_size(), 0);
    }

    #[test]
    fn vfs_disconnect_all_clears_caches() {
        let vfs = Vfs::new().unwrap();
        // Manually insert something into the stat cache.
        {
            let key = make_key("myvm");
            let meta = Metadata {
                kind: crate::backend::EntryKind::File,
                size: 100,
                modified: None,
                permissions: None,
            };
            let mut cache = vfs.stat_cache.lock().unwrap();
            cache.insert(&key, "/test", meta);
        }
        assert_eq!(vfs.stat_cache_size(), 1);

        vfs.disconnect_all();
        assert_eq!(vfs.stat_cache_size(), 0);
    }

    #[test]
    fn vfs_disconnect_host_clears_host_caches() {
        let vfs = Vfs::new().unwrap();
        let key1 = make_key("vm1");
        let key2 = make_key("vm2");
        let meta = Metadata {
            kind: crate::backend::EntryKind::File,
            size: 100,
            modified: None,
            permissions: None,
        };

        {
            let mut cache = vfs.stat_cache.lock().unwrap();
            cache.insert(&key1, "/test", meta.clone());
            cache.insert(&key2, "/test", meta);
        }
        assert_eq!(vfs.stat_cache_size(), 2);

        vfs.disconnect_host("vm1");
        assert_eq!(vfs.stat_cache_size(), 1);

        // Verify vm2's entry is still present.
        let mut cache = vfs.stat_cache.lock().unwrap();
        assert!(cache.get(&key2, "/test").is_some());
    }
}
