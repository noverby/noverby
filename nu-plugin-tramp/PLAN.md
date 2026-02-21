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

## 2. Repository Layout (Workspace)

```text
nu-plugin-tramp/
├── Cargo.toml              # workspace manifest
├── Cargo.lock
├── PLAN.md                 # this document
├── README.md               # user-facing documentation
├── default.nix             # Nix package derivations (plugin + agent)
├── hm-module.nix           # Home Manager module
└── crates/
    ├── plugin/             # nu-plugin-tramp — Nushell plugin
    │   ├── Cargo.toml
    │   └── src/
    │       ├── main.rs         # plugin entry point + commands
    │       ├── protocol.rs     # TRAMP URI parser
    │       ├── errors.rs       # error types
    │       ├── vfs.rs          # VFS layer (caching, connection pool)
    │       └── backend/
    │           ├── mod.rs      # Backend trait + registry
    │           ├── ssh.rs      # SSH backend (SFTP + exec)
    │           ├── exec.rs     # Docker / K8s / sudo backends
    │           └── runner.rs   # CommandRunner abstraction (local/remote)
    └── agent/              # tramp-agent — lightweight RPC agent
        ├── Cargo.toml
        └── src/
            ├── main.rs         # agent entry point (stdin/stdout RPC loop)
            ├── rpc.rs          # MsgPack-RPC framing + message types
            └── ops/
                ├── mod.rs      # operation module index
                ├── file.rs     # file.stat, file.read, file.write, …
                ├── dir.rs      # dir.list, dir.create, dir.remove
                ├── process.rs  # process.run, process.start, …
                ├── system.rs   # system.info, system.getenv, system.statvfs
                ├── batch.rs    # batch (N ops in 1 round-trip)
                └── watch.rs    # watch.add, watch.remove, watch.list
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

- [x] TRAMP URI parser (single-hop SSH only)
- [x] SSH backend via `openssh` + `openssh-sftp-client`
- [x] `open`, `ls`, `save` working end-to-end
- [x] `rm` support
- [x] Nushell plugin compiles and registers correctly
- [x] README with install + usage
- [x] Nix package derivation (`nu-plugin-tramp/default.nix`)

### Phase 2 — Daily Driver

- [x] Connection pooling + keepalive
- [x] `cd` with relative path resolution
- [x] Stat + small file cache (with TTL)
- [x] Streaming for large files
- [x] `cp` between remotes
- [x] `tramp ping/connections/disconnect` commands

### Phase 3 — Power Features

- [x] Path chaining (SSH → Docker, SSH → Sudo, triple chains, etc.)
- [x] Docker backend
- [x] Kubernetes (`kubectl exec`) backend
- [x] `sudo` backend
- [x] Push execution model (`tramp exec` command)

### Phase 4 — Polish & Usability

- [x] `tramp info` command — show remote system info (OS, arch, hostname, user, disk usage) in one round-trip
- [x] Richer `DirEntry` metadata — inode, nlinks, uid/gid, owner/group names, symlink targets
- [x] Batch stat in listings — combine `stat` calls for all entries in a directory into a single remote command
- [x] Configurable cache TTL via `$env.TRAMP_CACHE_TTL` environment variable
- [x] Home Manager module for auto-registration (`hm-module.nix`)
- [x] Glob/wildcard support for `tramp ls --glob` and `tramp cp --glob`
- [ ] Tab completion for remote paths

### Phase 5 — RPC Agent (inspired by [emacs-tramp-rpc](https://github.com/ArthurHeymans/emacs-tramp-rpc))

The biggest performance opportunity is replacing shell-command-parsing with a
lightweight **RPC agent** deployed on the remote host. `emacs-tramp-rpc`
demonstrates 2–38× speedups over traditional shell-based TRAMP by using this
approach.

#### 5.1 — `tramp-agent` binary ✅

A small, statically-linked Rust binary (~1 MB) that runs on the remote host
and speaks a length-prefixed MessagePack-RPC protocol over stdin/stdout
(piped through the SSH connection).

```text
┌──────────────────┐  SSH stdin/stdout  ┌───────────────────┐
│  nu-plugin-tramp │ ◄───────────────► │   tramp-agent     │
│  (local Nushell) │   MsgPack-RPC     │   (remote Rust)   │
└──────────────────┘                    └───────────────────┘
```

RPC methods exposed by the agent:

| Category   | Methods                                                        |
|------------|----------------------------------------------------------------|
| File       | `file.stat`, `file.stat_batch`, `file.truename`               |
| File I/O   | `file.read`, `file.write`, `file.copy`, `file.rename`,        |
|            | `file.delete`, `file.set_modes`                               |
| Directory  | `dir.list`, `dir.create`, `dir.remove`                        |
| Process    | `process.run`, `process.start`, `process.read`,               |
|            | `process.write`, `process.kill`                               |
| System     | `system.info`, `system.getenv`, `system.statvfs`              |
| Batch      | `batch` (multiple ops in one round-trip)                       |
| Watch      | `watch.add`, `watch.remove`, `watch.list`                     |

Key advantages over shell parsing:

| Aspect             | Current (shell parsing)        | With tramp-agent              |
|--------------------|-------------------------------|-------------------------------|
| Communication      | Individual SSH exec commands   | MsgPack-RPC over single pipe  |
| Latency            | N operations = N round-trips   | N operations = 1 round-trip   |
| Binary data        | base64 encode/decode           | Native binary (MsgPack bin)   |
| Stat + list        | Separate commands, text parse  | Native `lstat()` syscalls     |
| Shell dependency   | Requires sh, stat, cat, etc.  | None (self-contained binary)  |
| Cache invalidation | TTL-based (5s default)         | inotify/kqueue push events    |

#### 5.2 — Automatic deployment

On first connection (when the agent is not yet present on the remote), the
plugin:

1. Detects the remote OS and architecture (`uname -sm`)
2. Checks a local cache (`~/.cache/nu-plugin-tramp/VERSION/ARCH/tramp-agent`)
3. Downloads a pre-built binary from GitHub Releases (or builds from source
   if Rust is installed on the remote)
4. Uploads via SFTP to `~/.cache/tramp-agent/tramp-agent` on the remote
5. Starts the agent and switches the backend to RPC mode

If deployment fails, the plugin falls back to the current shell-parsing
approach transparently — no user action required.

```text
Connect via SSH
       │
       ▼
Agent already deployed? ──yes──► Start agent, use RPC
       │ no
       ▼
Detect remote arch (uname -sm)
       │
       ▼
Check local cache
       │
       ├─ Found ──────────────► Upload via SFTP, start
       │
       ▼
Download from GitHub Releases
       │
       ├─ Success ────────────► Cache locally, upload, start
       │
       ▼
Build with cargo (if available)
       │
       ├─ Success ────────────► Cache locally, upload, start
       │
       ▼
Fall back to shell-parsing mode (current behavior)
```

#### 5.3 — Filesystem watching (push cache invalidation) ✅

When the agent is running, it uses `inotify` (Linux) or `kqueue` (macOS) to
watch directories that the client has recently accessed. On filesystem
changes, the agent sends an `fs.changed` notification with the affected
paths. The VFS layer uses these to invalidate specific cache entries
immediately, rather than waiting for TTL expiry.

This is especially valuable for workflows where an editor and shell are both
operating on the same remote directory — changes are reflected instantly.

#### 5.4 — Batch and parallel operations ✅

The `batch` RPC method allows the client to send multiple operations in a
single request:

```text
Client sends:
  batch { requests: [
    { method: "file.stat", params: { path: "/app/config.toml" } },
    { method: "file.stat", params: { path: "/app/data.json" } },
    { method: "dir.list",  params: { path: "/app" } },
  ]}

Agent responds:
  { results: [ {result: ...}, {result: ...}, {result: ...} ] }
```

The `commands.run_parallel` method runs multiple commands concurrently using
OS threads (inspired by emacs-tramp-rpc's magit optimization that sends ~60
git commands in a single round-trip).

#### 5.5 — Agent for exec backends

The agent concept extends beyond SSH. For Docker and Kubernetes backends,
the agent binary can be copied into containers (`docker cp` / `kubectl cp`)
and started via `docker exec` / `kubectl exec`, giving the same RPC
performance benefits inside containers.

#### 5.6 — Protocol design ✅

- **Framing**: 4-byte big-endian length prefix + MessagePack payload
- **Request**: `{ version: "2.0", id: N, method: "...", params: {...} }`
- **Response**: `{ version: "2.0", id: N, result: ... }` or `{ ..., error: { code, message } }`
- **Notification** (server → client): `{ version: "2.0", method: "fs.changed", params: { paths: [...] } }`
- **Binary data**: MessagePack `bin` type for file content (no base64)
- **Concurrent requests**: Multiple requests can be in-flight; responses are matched by `id`

Dependencies for the agent binary:

```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
rmp-serde = "1.3"
rmpv = { version = "1.3", features = ["with-serde"] }
tokio = { version = "1", features = ["rt-multi-thread", "io-util", "io-std", "fs", "process", "sync"] }
notify = "6.1"     # inotify/kqueue
libc = "0.2"       # for native stat, uid/gid resolution
```

Build profile for minimal binary size:

```toml
[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
panic = "abort"
strip = true
```

### Phase 6 — Future

- [ ] Streaming for very large files (chunked RPC reads)
- [ ] PTY support via agent (remote terminal emulation)
- [ ] `tramp watch` command — subscribe to filesystem change notifications
- [ ] Agent version management and auto-upgrade
- [ ] TCP/Unix socket transport for agent (local Docker without SSH)
- [ ] Cross-compilation matrix in CI for agent binaries (x86_64/aarch64 × Linux/macOS)

---

## 10. Non-Goals (v1)

- GUI or TUI
- Windows support (SFTP client may not build; untested)
- Non-Nushell shells
- Encrypted secrets management (delegate to `agenix`/`ragenix` already in the monorepo)
- FUSE mount (use SSHFS if you need that separately)