# nu-plugin-tramp

> A TRAMP-inspired remote filesystem plugin for [Nushell](https://www.nushell.sh/), written in Rust.

The key design principle — inherited from Emacs TRAMP — is that **the shell
and all its tools stay local**. Only file I/O crosses the transport boundary.
Nushell's structured data model makes this a natural fit: `tramp open` returns
a typed Nushell value; pipes operate locally; `tramp save` writes back remotely.

## Features

- **TRAMP-style URIs** — `/ssh:user@host#port:/remote/path`
- **Full `~/.ssh/config` support** — host aliases, keys, jump hosts all work
- **SSH agent forwarding** — no extra configuration needed
- **ControlMaster multiplexing** — fast subsequent operations on the same host
- **Connection pooling with health-checks** — sessions are reused across commands; stale connections are detected and transparently reconnected
- **SFTP fast-path** — file read/write/delete uses the SFTP subsystem for efficient binary-safe transfer without base64 overhead or shell argument limits; falls back to exec transparently if SFTP is unavailable
- **Stat & directory listing cache** — metadata is cached with a short TTL to avoid redundant remote round-trips
- **Remote working directory** — `tramp cd` lets you navigate remote hosts with relative paths
- **Cross-host copy** — `tramp cp` transfers files between any combination of local and remote paths

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

| Segment       | Required | Example          |
|---------------|----------|------------------|
| `backend`     | ✅       | `ssh`            |
| `user`        | ❌       | `admin`          |
| `host`        | ✅       | `myvm`, `10.0.0.1` |
| `port`        | ❌       | `2222`           |
| `remote-path` | ✅       | `/etc/config`    |

### Examples

```text
/ssh:myvm:/etc/config
/ssh:admin@myvm:/etc/config
/ssh:admin@myvm#2222:/etc/config
```

### Chained paths (Phase 3)

Multi-hop paths are parsed but not yet executed:

```text
/ssh:jumpbox|ssh:myvm:/etc/config
/ssh:myvm|docker:mycontainer:/app/config.toml
```

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
| `tramp ping`          | Test connectivity to a remote host                 |
| `tramp connections`   | List active pooled connections                     |
| `tramp disconnect`    | Close connections (by host or `--all`)             |

## Requirements

- **Nushell** ≥ 0.110
- **OpenSSH** client installed and in `$PATH` (`ssh`, `ssh-agent`)
- **GNU coreutils** on the remote host (`stat` for listings; `cat`, `rm`, `base64` as fallback when SFTP is unavailable)

## Architecture

```text
Nushell command
      │
      ▼
┌─────────────────────────────────────────┐
│        nu-plugin-tramp plugin           │
│                                         │
│  ┌─────────────┐   ┌─────────────────┐  │
│  │ Path Parser │──▶│ Backend Resolver│  │
│  └─────────────┘   └───────┬─────────┘  │
│                            │            │
│                   ┌────────▼────────┐   │
│                   │   VFS Layer     │   │
│                   │  ┌───────────┐  │   │
│                   │  │ Stat/List │  │   │
│                   │  │  Cache    │  │   │
│                   │  └───────────┘  │   │
│                   └────────┬────────┘   │
│                            │            │
│              ┌─────────────┼──────┐     │
│              ▼             ▼      ▼     │
│           ┌─────┐    ┌────────┐  ...    │
│           │ SSH │    │ Docker │         │
│           └─────┘    └────────┘         │
└─────────────────────────────────────────┘
```

### Layers

1. **Path Parser** (`src/protocol.rs`) — Parses TRAMP URIs into structured types with round-trip fidelity; supports relative path resolution against a remote CWD
2. **Backend Trait** (`src/backend/mod.rs`) — Async trait defining `read`, `write`, `list`, `stat`, `exec`, `delete`, and `check` (health-check)
3. **VFS** (`src/vfs.rs`) — Resolves paths to backends, manages connection pooling with health-checks, provides stat/list caching with TTL, bridges async↔sync
4. **SSH Backend** (`src/backend/ssh.rs`) — Uses SFTP for file read/write/delete (fast-path) with automatic fallback to remote command execution; listing and stat use exec for structured GNU `stat` output

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

### Phase 3 — Power Features

- [ ] Path chaining (jump hosts, sudo, Docker-in-SSH)
- [ ] Docker backend (`docker exec`)
- [ ] Kubernetes backend (`kubectl exec`)
- [ ] `sudo` backend
- [ ] Push execution model (run commands natively on remote)
- [ ] Home Manager module for auto-registration

## License

MIT