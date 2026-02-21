# nu-plugin-tramp

> A TRAMP-inspired remote filesystem plugin for [Nushell](https://www.nushell.sh/), written in Rust.

The key design principle — inherited from Emacs TRAMP — is that **the shell
and all its tools stay local**. Only file I/O crosses the transport boundary.
Nushell's structured data model makes this a natural fit: `tramp open` returns
a typed Nushell value; pipes operate locally; `tramp save` writes back remotely.

## Features

- **TRAMP-style URIs** — `/ssh:user@host#port:/remote/path`
- **Multiple backends** — SSH, Docker, Kubernetes, and Sudo
- **Path chaining** — reach nested environments: `/ssh:myvm|docker:ctr:/app/config.toml`
- **Full `~/.ssh/config` support** — host aliases, keys, jump hosts all work
- **SSH agent forwarding** — no extra configuration needed
- **ControlMaster multiplexing** — fast subsequent operations on the same host
- **Connection pooling with health-checks** — sessions are reused across commands; stale connections are detected and transparently reconnected
- **SFTP fast-path** — file read/write/delete uses the SFTP subsystem for efficient binary-safe transfer without base64 overhead or shell argument limits; falls back to exec transparently if SFTP is unavailable
- **Stat & directory listing cache** — metadata is cached with a short TTL to avoid redundant remote round-trips
- **Remote working directory** — `tramp cd` lets you navigate remote hosts with relative paths
- **Cross-host copy** — `tramp cp` transfers files between any combination of local and remote paths
- **Push execution** — `tramp exec` runs arbitrary commands on the remote

## Installation

### From source

```sh
cargo install --path .
```

### Register with Nushell

```nushell
plugin add ~/.cargo/bin/nu_plugin_tramp
plugin use tramp
```

### With Nix

The package is available as `nu-plugin-tramp` from the monorepo flake:

```sh
nix build .#nu-plugin-tramp
```

## Usage

### Read a remote file

```nushell
# Read a text file
tramp open /ssh:myvm:/etc/hostname

# Read and parse a remote JSON file
tramp open /ssh:myvm:/app/config.json | from json

# Read a remote TOML config
tramp open /ssh:admin@myvm:/app/config.toml | from toml
```

### List a remote directory

```nushell
# List files in a remote directory
tramp ls /ssh:myvm:/var/log

# Filter to only directories
tramp ls /ssh:myvm:/ | where type == dir

# Sort by size
tramp ls /ssh:myvm:/var/log | sort-by size --reverse
```

### Write to a remote file

```nushell
# Write a string
"hello world" | tramp save /ssh:myvm:/tmp/hello.txt

# Pipe structured data through a serialiser
open local-config.toml | to toml | tramp save /ssh:myvm:/app/config.toml

# Copy a local file to remote
open local-file.bin | tramp save /ssh:myvm:/tmp/remote-file.bin
```

### Delete a remote file

```nushell
tramp rm /ssh:myvm:/tmp/stale.lock
```

### Copy files

```nushell
# Remote → Local
tramp cp /ssh:myvm:/etc/hostname ./hostname

# Local → Remote
tramp cp ./config.toml /ssh:myvm:/app/config.toml

# Remote → Remote (even across different hosts)
tramp cp /ssh:vm1:/etc/config /ssh:vm2:/etc/config
```

### Execute remote commands

Run arbitrary commands on a remote target using push execution:

```nushell
# Run a command on a remote SSH host
tramp exec /ssh:myvm:/ -- ls -la /tmp

# Run inside a Docker container on a remote host
tramp exec /ssh:myvm|docker:ctr:/ -- hostname

# Run inside a local Docker container
tramp exec /docker:mycontainer:/ -- cat /etc/os-release

# Run as root via sudo
tramp exec /sudo:root:/ -- cat /etc/shadow
```

### Docker backend

Access files inside Docker containers, locally or through SSH:

```nushell
# Local Docker container
tramp open /docker:mycontainer:/app/config.toml
tramp ls /docker:mycontainer:/var/log

# Docker container on a remote host (chained)
tramp open /ssh:myvm|docker:webapp:/app/config.toml
tramp ls /ssh:myvm|docker:webapp:/var/log

# With a specific user inside the container
tramp open /docker:admin@mycontainer:/app/config.toml
```

### Kubernetes backend

Access files inside Kubernetes pods:

```nushell
# Access a pod (uses current kubectl context)
tramp open /k8s:mypod:/app/config.toml
tramp ls /k8s:mypod:/var/log

# Specify a container in a multi-container pod
tramp open /k8s:sidecar@mypod:/app/config.toml

# Pod on a remote host (chained through SSH)
tramp open /ssh:myvm|k8s:mypod:/app/config.toml
```

### Sudo backend

Access files as another user via sudo:

```nushell
# Read a root-only file
tramp open /sudo:root:/etc/shadow

# Sudo through SSH
tramp open /ssh:myvm|sudo:root:/etc/shadow

# List as another user
tramp ls /sudo:www-data:/var/www
```

### Path chaining

Chain multiple hops to reach nested environments. Each hop is separated by `|`:

```nushell
# SSH → Docker
tramp open /ssh:myvm|docker:webapp:/app/config.toml

# SSH → Sudo
tramp open /ssh:myvm|sudo:root:/etc/shadow

# SSH → Docker → Sudo (triple chain)
tramp ls /ssh:myvm|docker:webapp|sudo:root:/etc

# Use cd for repeated access
tramp cd /ssh:myvm|docker:webapp:/app
tramp open config.toml
tramp ls
```

### Remote working directory

Set a remote CWD so that subsequent commands can use relative paths:

```nushell
# Set the remote working directory
tramp cd /ssh:myvm:/app

# Now use relative paths — resolves to /ssh:myvm:/app/config.toml
tramp open config.toml

# List the current remote directory
tramp ls

# Navigate relatively
tramp cd subdir
tramp cd ..

# Show the current remote CWD
tramp pwd

# Clear the remote CWD
tramp cd --reset
```

Non-TRAMP commands (e.g. `git`) receive `$env.PWD` as-is and are unaffected —
only `tramp` subcommands participate in remote CWD resolution.

### Connection management

```nushell
# Test connectivity to a remote host (reports timing)
tramp ping /ssh:myvm:/

# List all active pooled connections
tramp connections

# Disconnect a specific host
tramp disconnect myvm

# Disconnect everything
tramp disconnect --all
```

## Path Format

```text
/<backend>:<user>@<host>#<port>:<remote-path>
```

| Segment       | Required | Example             |
|---------------|----------|---------------------|
| `backend`     | ✅       | `ssh`, `docker`, `k8s`, `sudo` |
| `user`        | ❌       | `admin`             |
| `host`        | ✅       | `myvm`, `10.0.0.1`, container/pod name, target user |
| `port`        | ❌       | `2222`              |
| `remote-path` | ✅       | `/etc/config`       |

### Backend reference

| Backend | Host field | User field | Example |
|---------|-----------|------------|---------|
| `ssh` | Hostname or IP | SSH user | `/ssh:admin@myvm#2222:/path` |
| `docker` | Container name/ID | `--user` inside container | `/docker:root@webapp:/path` |
| `k8s` / `kubernetes` | Pod name | `-c` container name | `/k8s:sidecar@mypod:/path` |
| `sudo` | Target user | (unused) | `/sudo:root:/path` |

### Single-hop examples

```text
/ssh:myvm:/etc/config
/ssh:admin@myvm#2222:/etc/config
/docker:mycontainer:/app/config.toml
/k8s:mypod:/tmp/data
/k8s:sidecar@mypod:/app/logs
/sudo:root:/etc/shadow
```

### Chained path examples

```text
/ssh:myvm|docker:webapp:/app/config.toml
/ssh:myvm|sudo:root:/etc/shadow
/ssh:jumpbox|docker:webapp|sudo:root:/etc/shadow
```

> **Note**: SSH-through-SSH chaining (`/ssh:jump|ssh:target:/path`) is not
> supported directly. Use `ProxyJump` in `~/.ssh/config` instead — it's
> more efficient and fully supported by the SSH backend.

## Commands

| Command               | Description                                        |
|-----------------------|----------------------------------------------------|
| `tramp`               | Show help and usage information                    |
| `tramp open`          | Read a remote file and return as Nushell value     |
| `tramp ls`            | List a remote directory as a table                 |
| `tramp save`          | Write piped data to a remote file                  |
| `tramp rm`            | Delete a remote file                               |
| `tramp cp`            | Copy files between local/remote locations          |
| `tramp cd`            | Set the remote working directory                   |
| `tramp pwd`           | Show the current remote working directory          |
| `tramp exec`          | Execute a command on the remote (push execution)   |
| `tramp ping`          | Test connectivity to a remote host                 |
| `tramp connections`   | List active pooled connections                     |
| `tramp disconnect`    | Close connections (by host or `--all`)             |

## Requirements

- **Nushell** ≥ 0.110
- **OpenSSH** client installed and in `$PATH` (`ssh`, `ssh-agent`) — for SSH backend
- **Docker CLI** installed and in `$PATH` — for Docker backend
- **kubectl** installed and configured — for Kubernetes backend
- **sudo** configured for non-interactive use (`NOPASSWD`) — for Sudo backend
- **GNU coreutils** on the remote/target (`stat` for listings; `cat`, `rm`, `base64` as fallback when SFTP is unavailable)

## Architecture

```text
Nushell command
      │
      ▼
┌─────────────────────────────────────────────┐
│          nu-plugin-tramp plugin             │
│                                             │
│  ┌─────────────┐   ┌───────────────────┐    │
│  │ Path Parser │──▶│ Chain Resolver    │    │
│  └─────────────┘   └───────┬───────────┘    │
│                            │                │
│                   ┌────────▼────────┐       │
│                   │   VFS Layer     │       │
│                   │  ┌───────────┐  │       │
│                   │  │ Stat/List │  │       │
│                   │  │  Cache    │  │       │
│                   │  └───────────┘  │       │
│                   └────────┬────────┘       │
│                            │                │
│         ┌──────────────────┼──────────┐     │
│         ▼         ▼        ▼          ▼     │
│      ┌─────┐  ┌────────┐ ┌─────┐ ┌──────┐  │
│      │ SSH │  │ Docker │ │ K8s │ │ Sudo │  │
│      └─────┘  └────────┘ └─────┘ └──────┘  │
│                   ▲        ▲        ▲       │
│                   └────────┴────────┘       │
│               CommandRunner abstraction     │
│            (LocalRunner / RemoteRunner)      │
└─────────────────────────────────────────────┘
```

### Layers

1. **Path Parser** (`src/protocol.rs`) — Parses TRAMP URIs into structured types with round-trip fidelity; supports multi-hop chained paths
2. **Backend Trait** (`src/backend/mod.rs`) — Async trait defining `read`, `write`, `list`, `stat`, `exec`, `delete`, and `check` (health-check)
3. **CommandRunner** (`src/backend/runner.rs`) — Abstraction for executing commands locally (`LocalRunner`) or through a parent backend (`RemoteRunner`), enabling path chaining
4. **ExecBackend** (`src/backend/exec.rs`) — Generic exec-based backend used by Docker, Kubernetes, and Sudo; wraps commands with a configurable prefix
5. **SSH Backend** (`src/backend/ssh.rs`) — Uses SFTP for file read/write/delete (fast-path) with automatic fallback to remote command execution; listing and stat use exec for structured GNU `stat` output
6. **VFS** (`src/vfs.rs`) — Resolves paths to backends, builds multi-hop chains, manages connection pooling with health-checks, provides stat/list caching with TTL, bridges async↔sync

### Chaining internals

When resolving a chained path like `/ssh:myvm|docker:ctr:/path`, the VFS:

1. Creates an `SshBackend` for the first hop (`ssh:myvm`)
2. Creates a `RemoteRunner` wrapping the SSH backend
3. Creates an `ExecBackend::docker(remote_runner, "ctr")` for the second hop
4. The Docker backend's commands (e.g. `docker exec ctr cat /path`) execute through the SSH session

This composable design means any combination of backends can be chained (except SSH-through-SSH, which should use ProxyJump).

## Roadmap

### Phase 1 — MVP ✅

- [x] TRAMP URI parser (single-hop SSH only)
- [x] SSH backend via `openssh` crate
- [x] `tramp open`, `tramp ls`, `tramp save`, `tramp rm`
- [x] Nushell plugin compiles and registers correctly
- [x] README with install + usage
- [x] Nix package derivation

### Phase 2 — Daily Driver ✅

- [x] Connection pooling with health-checks + automatic reconnection
- [x] `tramp cd` / `tramp pwd` with relative path resolution
- [x] Stat + directory listing cache (with 5s TTL)
- [x] `tramp cp` between local/remote/remote
- [x] `tramp ping`, `tramp connections`, `tramp disconnect` commands
- [x] SFTP fast-path for file read/write/delete with automatic exec fallback

### Phase 3 — Power Features ✅

- [x] Path chaining (SSH → Docker, SSH → Sudo, triple chains, etc.)
- [x] Docker backend (`docker exec`)
- [x] Kubernetes backend (`kubectl exec`)
- [x] Sudo backend
- [x] Push execution model (`tramp exec`)

### Phase 4 — Future

- [ ] Home Manager module for auto-registration
- [ ] Streaming for very large files
- [ ] Glob/wildcard support for `tramp ls` and `tramp cp`
- [ ] Tab completion for remote paths
- [ ] Configurable cache TTL via environment variables

## License

MIT