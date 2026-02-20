# tramp-rs

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
- **Connection pooling** — sessions are reused across commands within the plugin lifetime
- **Binary-safe writes** — uses base64 encoding to safely transfer arbitrary file content

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

The package is available as `nu_plugin_tramp` from the monorepo flake:

```sh
nix build .#nu_plugin_tramp
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

### Chained paths (Phase 2+)

Multi-hop paths are parsed but not yet executed:

```text
/ssh:jumpbox|ssh:myvm:/etc/config
/ssh:myvm|docker:mycontainer:/app/config.toml
```

## Commands

| Command      | Description                                    |
|--------------|------------------------------------------------|
| `tramp`      | Show help and usage information                |
| `tramp open` | Read a remote file and return as Nushell value |
| `tramp ls`   | List a remote directory as a table             |
| `tramp save` | Write piped data to a remote file              |
| `tramp rm`   | Delete a remote file                           |

## Requirements

- **Nushell** ≥ 0.110
- **OpenSSH** client installed and in `$PATH` (`ssh`, `ssh-agent`)
- **GNU coreutils** on the remote host (`stat`, `cat`, `rm`, `base64`)

## Architecture

```text
Nushell command
      │
      ▼
┌─────────────────────────────────────────┐
│           tramp-rs plugin               │
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
│           ┌─────┐    ┌────────┐  ...    │
│           │ SSH │    │ Docker │         │
│           └─────┘    └────────┘         │
└─────────────────────────────────────────┘
```

### Layers

1. **Path Parser** (`src/protocol.rs`) — Parses TRAMP URIs into structured types with round-trip fidelity
2. **Backend Trait** (`src/backend/mod.rs`) — Async trait defining `read`, `write`, `list`, `stat`, `exec`, `delete`
3. **VFS** (`src/vfs.rs`) — Resolves paths to backends, manages connection pooling, bridges async↔sync
4. **SSH Backend** (`src/backend/ssh.rs`) — Implements all operations via remote command execution over OpenSSH

## Roadmap

### Phase 1 — MVP ✅

- [x] TRAMP URI parser (single-hop SSH only)
- [x] SSH backend via `openssh` crate
- [x] `tramp open`, `tramp ls`, `tramp save`, `tramp rm`
- [x] Nushell plugin compiles and registers correctly
- [x] README with install + usage
- [x] Nix package derivation

### Phase 2 — Daily Driver

- [ ] SFTP fast-path for large/binary file transfers
- [ ] Connection pooling + keepalive
- [ ] `cd` with relative path resolution
- [ ] Stat + small file cache (with TTL)
- [ ] Streaming for large files
- [ ] `cp` between remotes
- [ ] `tramp ping`, `tramp connections`, `tramp disconnect`

### Phase 3 — Power Features

- [ ] Path chaining (jump hosts, sudo, Docker-in-SSH)
- [ ] Docker backend (`docker exec`)
- [ ] Kubernetes backend (`kubectl exec`)
- [ ] `sudo` backend
- [ ] Home Manager module for auto-registration

## License

MIT