# nu-plugin-tramp — PLAN

> A TRAMP-inspired remote filesystem plugin for [Nushell](https://www.nushell.sh/), written in Rust.

---

## 1. Overview

`nu-plugin-tramp` allows Nushell users to transparently access remote files using
TRAMP-style URI paths, e.g.:

```nushell
open /ssh:myvm:/etc/config
ls   /ssh:myvm:/var/log
open /ssh:myvm:/app/config.toml | upsert port 8080 | save /ssh:myvm:/app/config.toml
```

The key design principle — inherited from Emacs TRAMP — is that **the shell
and all its tools stay local**. Only file I/O crosses the transport boundary.
Nushell's structured data model makes this a natural fit: `open` returns a
typed Nushell value; pipes operate locally; `save` writes back remotely.

---

## 2. Repository Layout

```text
nu-plugin-tramp/
├── Cargo.toml          # workspace manifest
├── SPEC.md             # this document
├── README.md           # user-facing documentation
└── src/
    ├── main.rs         # Nushell plugin entry point
    ├── protocol.rs     # TRAMP URI parser
    ├── vfs.rs          # Virtual filesystem abstraction
    └── backend/
        ├── mod.rs      # Backend trait + registry
        └── ssh.rs      # SSH backend (v1)
```

Placed under `nu-plugin-tramp/` in the `noverby/noverby` monorepo root, alongside
existing projects such as `nixos-rs/`, `systemd-rs/`, etc.

---

## 3. Path Format

### 3.1 Basic URI

```text
/<backend>:<user>@<host>#<port>:<remote-path>
```

| Segment | Required | Example |
|---|---|---|
| `backend` | ✅ | `ssh`, `docker`, `k8s` |
| `user` | ❌ | `admin` |
| `host` | ✅ | `myvm`, `192.168.1.10` |
| `port` | ❌ | `2222` |
| `remote-path` | ✅ | `/etc/config` |

Examples:

```text
/ssh:myvm:/etc/config
/ssh:admin@myvm:/etc/config
/ssh:admin@myvm#2222:/etc/config
```

### 3.2 Chained URIs (Phase 2+)

Hops are separated by `|`:

```text
/ssh:jumpbox|ssh:myvm:/etc/config
/ssh:myvm|docker:mycontainer:/app/config.toml
/ssh:myvm|sudo:root:/etc/shadow
```

The parser must produce a `Vec<Hop>` to represent chains. Execution of
chains is deferred to Phase 2 but the type system must support it from day 1.

### 3.3 Parsed Types

```rust
pub struct TrampPath {
    pub hops: Vec<Hop>,
    pub remote_path: String,
}

pub struct Hop {
    pub backend: BackendKind,
    pub user: Option<String>,
    pub host: String,
    pub port: Option<u16>,
}

pub enum BackendKind {
    Ssh,
    Docker,
    Kubernetes,
    Sudo,
}
```

---

## 4. Architecture

```text
Nushell command
      │
      ▼
┌────────────���────────────────────────────┐
│        nu-plugin-tramp plugin           │
│                                         │
│  ┌─────────────┐   ┌─────────────────┐  │
│  │ Path Parser │──▶│ Backend Resolver│  │
│  └─────────────┘   └───────┬─────────┘  │
│                            │            │
│                   ┌────────▼────────┐   │
│                   │   VFS Layer     │   │
│                   └────────┬────────┘   │
│                            │            │
│              ┌─────────────┼──────┐     │
│              ▼             ▼      ▼     │
│           ┌─────┐    ┌────────┐  ...   │
│           │ SSH │    │ Docker │        │
│           └─────┘    └────────┘        │
└─────────────────────────────────────────┘
```

### 4.1 Layer 1 — Path Parser (`src/protocol.rs`)

- Parses a string into `TrampPath`
- Must detect whether a path is a tramp URI (starts with `/<known-backend>:`)
- Returns `None` / `Ok(None)` for non-tramp paths so Nushell handles them normally
- Must round-trip: `parse(format(path)) == path`

### 4.2 Layer 2 — Backend Trait (`src/backend/mod.rs`)

```rust
#[async_trait]
pub trait Backend: Send + Sync {
    async fn read(&self, path: &str) -> Result<Bytes>;
    async fn write(&self, path: &str, data: Bytes) -> Result<()>;
    async fn list(&self, path: &str) -> Result<Vec<DirEntry>>;
    async fn stat(&self, path: &str) -> Result<Metadata>;
    async fn exec(&self, cmd: &str, args: &[&str]) -> Result<ExecResult>;
    async fn delete(&self, path: &str) -> Result<()>;
}

pub struct DirEntry {
    pub name: String,
    pub kind: EntryKind,      // File | Dir | Symlink
    pub size: Option<u64>,
    pub modified: Option<SystemTime>,
    pub permissions: Option<u32>,
}

pub struct Metadata {
    pub kind: EntryKind,
    pub size: u64,
    pub modified: SystemTime,
    pub permissions: u32,
}

pub struct ExecResult {
    pub stdout: Bytes,
    pub stderr: Bytes,
    pub exit_code: i32,
}
```

### 4.3 Layer 3 — VFS (`src/vfs.rs`)

Responsibilities:

- Resolve a `TrampPath` to a concrete `Backend` instance
- Connection pooling: reuse open connections keyed by `(backend, user, host, port)`
- Optional: small stat cache with TTL (Phase 2)
- Optional: streaming for large files (Phase 2)
- For Phase 1: single-hop only; log a warning and return an error for chained paths

### 4.4 Layer 4 — SSH Backend (`src/backend/ssh.rs`)

Use the [`openssh`](https://crates.io/crates/openssh) crate (shells out to
system OpenSSH). This gives:

- Full `~/.ssh/config` support
- SSH agent forwarding
- `ControlMaster` multiplexing (fast subsequent ops)
- Key management delegated entirely to the user's existing setup

Operations:

| Operation | Implementation |
|---|---|
| `read` | `sftp.read(path)` or `exec cat <path>` |
| `write` | `sftp.write(path, data)` |
| `list` | `sftp.read_dir(path)` |
| `stat` | `sftp.metadata(path)` |
| `exec` | `session.command(cmd).args(args).output()` |
| `delete` | `sftp.remove_file(path)` or `exec rm <path>` |

Prefer SFTP subsystem where available; fall back to `exec` for compatibility.

---

## 5. Nushell Plugin Interface

The plugin is registered with Nushell via:

```nushell
plugin add ~/.cargo/bin/nu_plugin_tramp
plugin use tramp
```

It hooks into Nushell's built-in commands by intercepting path arguments that
match the TRAMP URI pattern.

### 5.1 Command Hooks

| Nushell command | Plugin behavior |
|---|---|
| `open /ssh:…` | Read remote file; return as Nushell value (auto-detect format: JSON, TOML, CSV, raw) |
| `save /ssh:…` | Serialise piped Nushell value; write to remote |
| `ls /ssh:…` | List remote directory; return as Nushell table |
| `cd /ssh:…` | Set `$env.PWD` to the tramp URI; resolve relative paths against it |
| `rm /ssh:…` | Delete remote file |
| `cp /ssh:… /ssh:…` | Read from source backend; write to destination backend |

### 5.2 Custom Commands (supplementary)

```nushell
# Explicit connection test
tramp ping /ssh:myvm:/

# Show active connections
tramp connections

# Disconnect
tramp disconnect myvm
```

### 5.3 `cd` Semantics

When `$env.PWD` is a TRAMP URI, any relative path passed to a TRAMP-aware
command is resolved against it:

```nushell
cd /ssh:myvm:/app
open config.toml     # resolves to /ssh:myvm:/app/config.toml
ls                   # lists /app on myvm
```

Non-TRAMP commands (e.g. `git`) receive `$env.PWD` as-is and will fail
gracefully — this is expected and documented.

---

## 6. Error Handling

All errors must implement `std::error::Error` and produce clear, user-facing
messages. Use `thiserror`.

| Error kind | Message example |
|---|---|
| Parse error | `invalid tramp path: missing remote path after ':'` |
| Connection failed | `ssh: could not connect to myvm: connection refused` |
| Auth failed | `ssh: authentication failed for admin@myvm` |
| File not found | `remote: no such file or directory: /etc/missing` |
| Permission denied | `remote: permission denied: /etc/shadow` |
| Chained path (v1) | `tramp: chained paths not yet supported (see roadmap)` |

---

## 7. Dependencies

```toml
[dependencies]
# Nushell plugin API
nu-plugin    = "0.101"
nu-protocol  = "0.101"

# SSH transport
openssh      = { version = "0.10", features = ["native-mux"] }
openssh-sftp-client = "0.14"

# Async runtime
tokio        = { version = "1", features = ["full"] }

# Utilities
bytes        = "1"
thiserror    = "2"
async-trait  = "0.1"
```

Pin `nu-plugin` / `nu-protocol` to the same version as the Nushell binary in
use. Nushell's plugin ABI is version-locked.

---

## 8. Nix Integration

Because the monorepo uses Nix + Crane, `nu-plugin-tramp` should expose:

```nix
# flake.nix addition (nu-plugin-tramp/default.nix)
{ crane, rust-overlay, ... }:
crane.buildPackage {
  src = ./.;
  pname = "nu-plugin-tramp";
}
```

And optionally a Home Manager module that auto-registers the plugin:

```nix
programs.nushell.plugins = [ pkgs.nu-plugin-tramp ];
```

---

## 9. Phased Roadmap

### Phase 1 — MVP (initial PR)

- [ ] TRAMP URI parser (single-hop SSH only)
- [ ] SSH backend via `openssh` + `openssh-sftp-client`
- [ ] `open`, `ls`, `save` working end-to-end
- [ ] `rm` support
- [ ] Nushell plugin compiles and registers correctly
- [ ] README with install + usage
- [ ] Nix package derivation (`nu-plugin-tramp/default.nix`)

### Phase 2 — Daily Driver

- [x] Connection pooling + keepalive
- [x] `cd` with relative path resolution
- [x] Stat + small file cache (with TTL)
- [x] Streaming for large files
- [x] `cp` between remotes
- [x] `tramp ping/connections/disconnect` commands

### Phase 3 — Power Features

- [ ] Path chaining (jump hosts, sudo, Docker-in-SSH)
- [ ] Docker backend
- [ ] Kubernetes (`kubectl exec`) backend
- [ ] `sudo` backend
- [ ] Push execution model (run commands natively on remote)
- [ ] Home Manager module for auto-registration

---

## 10. Non-Goals (v1)

- GUI or TUI
- Windows support (SFTP client may not build; untested)
- Non-Nushell shells
- Encrypted secrets management (delegate to `agenix`/`ragenix` already in the monorepo)
- FUSE mount (use SSHFS if you need that separately)