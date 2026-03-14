# tangled-spindle-nix

A Rust reimplementation of the [Tangled Spindle](https://docs.tangled.org/spindles.html) CI runner that replaces Docker-based isolation with **native Nix** for dependency management and **systemd service-level sandboxing** for isolation. Includes a NixOS module (`services.tangled-spindles`) for declarative multi-runner deployment, modeled after [`services.github-runners`](https://github.com/NixOS/nixpkgs/blob/master/nixos/modules/services/continuous-integration/github-runner/options.nix).

## Motivation

The upstream spindle runner (`tangled.org/core/spindle`) is written in Go and requires Docker (or Podman with Docker compatibility) to isolate pipeline steps. Every step runs in a fresh container whose image is built on-the-fly via [Nixery](https://nixery.dev). This works, but:

1. **Docker is heavy** — it requires a daemon, container runtime, overlay filesystem driver, and root-equivalent access via the Docker socket.
2. **Nixery round-trips are wasteful on NixOS** — a NixOS host already has a Nix store. Pulling Nixery layers over HTTPS just to unpack them into an overlay filesystem is redundant when `nix build` can produce the same closure locally.
3. **systemd already provides process isolation** — the NixOS module defines a hardened systemd service per runner with `DynamicUser=`, `ProtectSystem=strict`, namespaces, seccomp filters, and more. All child processes (workflow steps) inherit this sandboxing automatically — no per-step transient units needed.
4. **NixOS modules are the idiomatic way to run services** — a NixOS module lets you declaratively spin up N spindle runners with per-runner configuration, secrets, and hardening, the same way `services.github-runners` does.

## Architecture Overview

```text
┌──────────────────────────────────────────────────────────────┐
│                    tangled-spindle-nix                        │
│                                                              │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────────────┐  │
│  │   Jetstream   │  │   Knot Event │  │    HTTP Server     │  │
│  │   Consumer    │  │   Consumer   │  │  /events /logs     │  │
│  │              │  │              │  │  /xrpc/*           │  │
│  └──────┬───────┘  └──────┬───────┘  └────────┬───────────┘  │
│         │                 │                   │              │
│         ▼                 ▼                   │              │
│  ┌──────────────────────────────┐             │              │
│  │         Job Queue            │◄────────────┘              │
│  └──────────────┬───────────────┘                            │
│                 │                                            │
│                 ▼                                            │
│  ┌──────────────────────────────┐                            │
│  │        Engine: nix           │                            │
│  │                              │                            │
│  │  1. nix build (closure)      │                            │
│  │  2. fork/exec step (child)   │                            │
│  │  3. stream stdout/stderr     │                            │
│  └──────────────────────────────┘                            │
│                                                              │
│  ┌──────────────────────────────┐                            │
│  │     State: SQLite + RBAC     │                            │
│  └──────────────────────────────┘                            │
│                                                              │
│  ┌──────────────────────────────┐                            │
│  │  Secrets: SQLite | OpenBao   │                            │
│  └──────────────────────────────┘                            │
└──────────────────────────────────────────────────────────────┘
```

Instead of Docker containers, each workflow step is executed as a **child process** of the runner daemon. The runner's systemd service provides all sandboxing:

| Concern | Docker (upstream) | nix engine (this project) |
|---------|-------------------|---------------------------|
| Process isolation | Container namespaces | systemd service-level sandboxing (inherited by children) |
| Dependencies | Nixery image pull over HTTPS | `nix build` from local store |
| Filesystem | Overlay FS | Service-level `ProtectSystem=strict`, `PrivateTmp=`, `ReadWritePaths=` |
| User isolation | Container root mapped to host uid | `DynamicUser=true` on the runner service |
| Resource limits | Docker cgroup limits | Service-level `CPUQuota=`, `MemoryMax=`, `TasksMax=` |
| Network isolation | Docker bridge network | Service-level `PrivateNetwork=` (configurable) |
| Log streaming | Docker attach stdout/stderr | Piped stdout/stderr from child process |
| Deployment | Binary + Docker daemon | NixOS module (`services.tangled-spindles`) |
| Language | Go | Rust |

### How the upstream Go spindle works (for reference)

1. **Jetstream ingestion** — Listens on the AT Protocol Jetstream for `sh.tangled.spindle.member` and `sh.tangled.repo` records. When a repo record points at this spindle's hostname, the spindle subscribes to that repo's knot for `sh.tangled.pipeline` events.
2. **Pipeline processing** — When a `sh.tangled.pipeline` event arrives, the spindle parses the workflow manifests, validates the engine, and enqueues a job.
3. **Engine execution** — The engine (currently only "nixery") sets up the execution environment, runs each step sequentially, and streams logs.
4. **Docker/Nixery engine** — Each step runs in a fresh Docker container. The base image is constructed on-the-fly by Nixery from the workflow's `dependencies` field. State persists across steps via a shared `/tangled/workspace` bind mount.
5. **Streaming** — WebSocket endpoints (`/events`, `/logs/{knot}/{rkey}/{name}`) stream pipeline status updates and step logs in real time.
6. **XRPC** — AT Protocol XRPC endpoints for service auth, member management, secret management, etc.

### How this Rust reimplementation differs

| Concern | Upstream (Go + Docker) | tangled-spindle-nix (Rust + Nix) |
|---------|----------------------|--------------------------------------|
| Step execution | Docker containers | Child processes (fork/exec) |
| Dependency resolution | Nixery (HTTP image pull) | `nix build` (local Nix store) |
| Filesystem isolation | Overlay filesystem | Service-level systemd hardening |
| User isolation | Container root mapped to host uid | `DynamicUser=true` per runner service |
| Network isolation | Docker bridge network | Service-level `PrivateNetwork=` (configurable) |
| Resource limits | Docker cgroup limits | Service-level systemd cgroup limits |
| Log streaming | Docker attach stdout/stderr | Piped stdout/stderr from child process |
| Deployment | Binary + Docker daemon | NixOS module (`services.tangled-spindles`) |
| Language | Go | Rust |

## Crate Structure

```text
tangled-spindle-nix/
├── Cargo.toml                    # Workspace root
├── Cargo.lock
├── PLAN.md                       # This file
├── README.md
├── default.nix                   # Flakelight module (packages + devShell + nixosModules)
├── nixos-module.nix              # NixOS module: services.tangled-spindles
├── justfile
└── crates/
    ├── tangled-spindle/          # Main binary (server + CLI)
    │   ├── Cargo.toml
    │   └── src/
    │       ├── main.rs
    │       ├── cli.rs            # CLI argument parsing
    │       ├── server.rs         # HTTP server (axum), WebSocket streams
    │       ├── config.rs         # Configuration (env vars, matching upstream)
    │       └── router.rs         # Route definitions (/events, /logs, /xrpc/*)
    │
    ├── spindle-engine/           # Engine trait + nix engine
    │   ├── Cargo.toml
    │   └── src/
    │       ├── lib.rs
    │       ├── traits.rs         # Engine trait definition
    │       ├── nix_engine.rs     # Nix engine: build closure, fork/exec steps
    │       ├── nix_deps.rs       # Nix dependency resolution (dependencies → nix build)
    │       └── workspace.rs      # Per-workflow workspace management
    │
    ├── spindle-jetstream/        # Jetstream client for AT Protocol
    │   ├── Cargo.toml
    │   └── src/
    │       ├── lib.rs
    │       ├── client.rs         # WebSocket Jetstream consumer
    │       └── ingester.rs       # Event ingestion (member, repo, collaborator)
    │
    ├── spindle-knot/             # Knot event consumer
    │   ├── Cargo.toml
    │   └── src/
    │       ├── lib.rs
    │       └── consumer.rs       # Pipeline event consumer from knot servers
    │
    ├── spindle-db/               # SQLite database layer
    │   ├── Cargo.toml
    │   └── src/
    │       ├── lib.rs
    │       ├── migrations.rs     # Schema migrations
    │       ├── repos.rs          # Repo tracking
    │       ├── members.rs        # Spindle member management
    │       ├── events.rs         # Pipeline event log
    │       └── status.rs         # Workflow status tracking
    │
    ├── spindle-rbac/             # RBAC enforcement (casbin-based, matching upstream)
    │   ├── Cargo.toml
    │   └── src/
    │       ├── lib.rs
    │       └── enforcer.rs
    │
    ├── spindle-secrets/          # Secrets manager (SQLite + OpenBao)
    │   ├── Cargo.toml
    │   └── src/
    │       ├── lib.rs
    │       ├── traits.rs         # Manager trait
    │       ├── sqlite.rs         # SQLite secrets backend
    │       └── openbao.rs        # OpenBao proxy backend
    │
    ├── spindle-queue/            # Bounded job queue with configurable workers
    │   ├── Cargo.toml
    │   └── src/
    │       ├── lib.rs
    │       └── queue.rs
    │
    ├── spindle-models/           # Shared types (Pipeline, Workflow, Step, LogLine, etc.)
    │   ├── Cargo.toml
    │   └── src/
    │       ├── lib.rs
    │       ├── pipeline.rs
    │       ├── workflow.rs
    │       ├── step.rs
    │       ├── status.rs
    │       ├── log_line.rs
    │       ├── pipeline_env.rs
    │       └── secret_mask.rs
    │
    └── spindle-xrpc/             # XRPC route handlers + service auth
        ├── Cargo.toml
        └── src/
            ├── lib.rs
            ├── service_auth.rs
            └── handlers.rs       # putRecord, getRecord, listRecords, etc.
```

## Phase Plan

### Phase 0 — Workspace Setup & Core Types

**Goal**: Establish the Cargo workspace, Nix build, dev shell, and shared model types.

- [x] Create `Cargo.toml` workspace with all member crates
- [x] Set up `default.nix` with `rustPlatform.buildRustPackage`, dev shell, and `nixosModules.tangled-spindle-nix`
- [x] Set up `justfile` with common commands (`build`, `test`, `check`, `clippy`, `run`)
- [x] Implement `spindle-models` crate:
  - `PipelineId`, `WorkflowId` (matching upstream `models.go`)
  - `StatusKind` enum (`Pending`, `Running`, `Failed`, `Timeout`, `Cancelled`, `Success`)
  - `LogLine`, `LogKind`, `StepStatus`, `StepKind` types
  - `Step` trait, `Workflow`, `Pipeline` structs
  - `PipelineEnvVars` builder (matching upstream `pipeline_env.go`)
  - `SecretMask` (log redaction for secret values)
  - `WorkflowLogger` trait + `FileWorkflowLogger` + `NullLogger`
- [x] Write unit tests for model serialization, `WorkflowId::to_string()` normalization, and `PipelineEnvVars`

### Phase 1 — Configuration & Database

**Goal**: Configuration loading and SQLite persistence, matching the upstream schema.

- [x] Implement `spindle-config` inside `tangled-spindle` (or as part of its `config.rs`):
  - Parse the same `SPINDLE_SERVER_*` and `SPINDLE_NIXERY_PIPELINES_*` env vars
  - Add new `SPINDLE_ENGINE` env var (default `"nix"`) for engine selection
  - Derive `did:web:{hostname}` just like upstream
- [x] Implement `spindle-db`:
  - SQLite via `rusqlite` with WAL mode
  - Migration system (embed SQL via `include_str!`)
  - Tables: `repos`, `spindle_members`, `dids`, `events`, `workflow_status`, `last_time_us`
  - Query functions: `add_repo`, `get_repo`, `get_all_dids`, `add_did`, `remove_did`, `save_last_time_us`, `get_events(cursor)`, `status_pending`, `status_running`, `status_failed`, `status_success`, `status_timeout`, `status_cancelled`, `get_status`, `knots`
- [x] Implement `spindle-rbac`:
  - Use `casbin-rs` with the same model/policy as upstream
  - `add_spindle`, `add_spindle_owner`, `add_spindle_member`, `remove_spindle_member`, `is_spindle_invite_allowed`, `add_repo`, `add_collaborator`, `is_collaborator_invite_allowed`, `get_spindle_users_by_role`
- [x] Write integration tests for DB and RBAC (using temp SQLite databases)

### Phase 2 — Jetstream & Knot Event Consumers

**Goal**: Ingest AT Protocol events, matching upstream `ingester.go` and event consumer behavior.

- [x] Implement `spindle-jetstream`:
  - WebSocket client connecting to the Jetstream endpoint
  - Filter for `sh.tangled.spindle.member`, `sh.tangled.repo`, `sh.tangled.repo.collaborator` collections
  - DID-based subscription management (`add_did`, `remove_did`)
  - Cursor persistence via `spindle-db`
  - Reconnection with exponential backoff
- [x] Implement `spindle-knot`:
  - Event consumer that subscribes to knot HTTP event streams
  - Filters for `sh.tangled.pipeline` events
  - Cursor-based replay on reconnection
  - Dynamic source management (add/remove knots at runtime)
- [x] Implement ingestion logic in the main crate:
  - `ingest_member` — add/remove spindle members, update DID watch list
  - `ingest_repo` — add repos to watch list when `spindle` field matches hostname, subscribe to knot
  - `ingest_collaborator` — resolve repo owner, add collaborator to RBAC
- [x] Write tests using mock WebSocket servers

### Phase 3 — Secrets Manager

**Goal**: Support both SQLite and OpenBao secrets backends, matching upstream.

- [x] Implement `spindle-secrets`:
  - `Manager` trait: `get_secrets_unlocked(repo) -> Vec<UnlockedSecret>`, `put_secret(repo, key, value)`, `delete_secret(repo, key)`, `list_secrets(repo) -> Vec<String>`
  - `SqliteManager` — encrypted secrets in SQLite (using `aes-gcm` for at-rest encryption)
  - `OpenBaoManager` — HTTP client to OpenBao proxy at `SPINDLE_SERVER_SECRETS_OPENBAO_PROXY_ADDR`
    - KV v2 mount at configurable path
    - Secret path convention: `{mount}/repos/{sanitized_repo_path}/{key}`
    - Path sanitization: `did:plc:alice/myrepo` → `did_plc_alice_myrepo`
  - `Stopper` trait for managers that need cleanup (OpenBao token renewal)
- [x] Write integration tests (SQLite: in-memory DB; OpenBao: mock HTTP server)

### Phase 4 — The Nix Engine ★

**Goal**: The core differentiator. Replace Docker+Nixery with Nix+child processes.

This is the heart of the project. The engine must implement the same `Engine` trait the upstream Go code defines:

```rust
#[async_trait]
pub trait Engine: Send + Sync {
    /// Transform an incoming pipeline workflow into our internal Workflow representation.
    fn init_workflow(&self, twf: PipelineWorkflow, tpl: Pipeline) -> Result<Workflow>;

    /// Set up the execution environment for a workflow (build Nix closure, create workspace dir).
    async fn setup_workflow(&self, wid: &WorkflowId, wf: &Workflow, logger: &dyn WorkflowLogger) -> Result<()>;

    /// Return the configured workflow timeout.
    fn workflow_timeout(&self) -> Duration;

    /// Tear down the execution environment.
    async fn destroy_workflow(&self, wid: &WorkflowId) -> Result<()>;

    /// Execute a single step within the workflow's environment.
    async fn run_step(
        &self,
        wid: &WorkflowId,
        wf: &Workflow,
        step_idx: usize,
        secrets: &[UnlockedSecret],
        logger: &dyn WorkflowLogger,
    ) -> Result<()>;
}
```

#### 4a — Nix Dependency Resolution (`nix_deps.rs`)

Transform workflow `dependencies` into a Nix environment:

- [x] Parse the `dependencies` map from the workflow YAML (same format as upstream):

  ```yaml
  dependencies:
    nixpkgs:
      - nodejs
      - go
    nixpkgs/nixpkgs-unstable:
      - bun
    git+https://tangled.org/@example.com/my_pkg:
      - my_pkg
  ```

- [x] Generate a Nix expression that produces a combined `PATH` environment:

  ```nix
  let
    nixpkgs = import <nixpkgs> {};
    nixpkgs-unstable = import (builtins.fetchTarball {
      url = "https://github.com/NixOS/nixpkgs/archive/nixpkgs-unstable.tar.gz";
    }) {};
    custom = import (builtins.fetchGit {
      url = "https://tangled.org/@example.com/my_pkg";
    }) {};
  in
  nixpkgs.buildEnv {
    name = "spindle-workflow-env";
    paths = [
      nixpkgs.nodejs
      nixpkgs.go
      nixpkgs-unstable.bun
      custom.my_pkg
    ];
  }
  ```

  Or, more efficiently, use `nix build --expr` or `nix shell` with flake references.

- [x] Build the Nix closure: `nix build --no-link --print-out-paths -f /tmp/spindle-env-{wid}.nix`
- [x] Cache built closures by content-addressing the dependency set (hash the sorted dependency map)
- [x] Handle build failures gracefully, streaming Nix build output to the workflow logger

#### 4b — Workspace Management (`workspace.rs`)

- [x] Create per-workflow workspace directories under a configurable root (default `/var/lib/tangled-spindle-{name}/workspaces/{wid}`)
- [x] The workspace persists across steps within a single workflow (matching Docker's `/tangled/workspace` bind mount)
- [x] Clone step: implement the shared clone logic (matching upstream `models/clone.go`):
  - `git clone --depth={depth} --branch={branch} {repo_url} {workspace_dir}`
  - Optionally `--recurse-submodules`
  - Optionally skip clone
- [x] Clean up workspace on `destroy_workflow`

#### 4c — Step Execution (`nix_engine.rs`)

Each workflow step runs as a **child process** of the runner daemon. The runner's systemd service provides all sandboxing — child processes inherit it automatically, just like `github-runners`.

- [x] Implement step execution via `tokio::process::Command`:

  ```rust
  let child = Command::new("/bin/bash")
      .args(["-euo", "pipefail", "-c", &step.command])
      .current_dir(&workspace_dir)
      .env_clear()
      .env("PATH", format!("{nix_env}/bin:{nix_env}/sbin:/usr/bin:/bin"))
      .env("HOME", &workspace_dir)
      .env("CI", "true")
      .envs(pipeline_env_vars)
      .envs(secrets_as_env)
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .kill_on_drop(true)
      .spawn()?;
  ```

- [x] Stream stdout and stderr line-by-line to the `WorkflowLogger`, applying `SecretMask`
- [x] Handle exit codes: 0 = success, non-zero = step failure
- [x] Handle workflow timeout: wrap execution in `tokio::time::timeout`, kill child on expiry
- [x] Implement `destroy_workflow`: kill any still-running child processes, clean workspace

#### 4d — Secret Injection

- [x] Secrets are injected as environment variables into each step's child process
- [x] Secret values are masked in log output via `SecretMask`

### Phase 5 — HTTP Server & WebSocket Streaming

**Goal**: Expose the same HTTP API as the upstream spindle.

- [x] Implement HTTP server using `axum`:
  - `GET /` — MOTD
  - `GET /events` — WebSocket: pipeline status event stream (with cursor-based backfill)
  - `GET /logs/{knot}/{rkey}/{name}` — WebSocket: real-time log streaming for a workflow
  - `POST /xrpc/*` — XRPC endpoints (service auth, secrets, membership, etc.)
- [x] Implement `spindle-xrpc`:
  - Service auth verification (bearer token for v1; AT Protocol JWT deferred to Phase 8)
  - Member management endpoints
  - Secret CRUD endpoints
  - Pipeline cancel endpoint
- [x] WebSocket streaming:
  - `/events`: Backfill from SQLite `events` table, then live-stream via `tokio::sync::broadcast`
  - `/logs/{knot}/{rkey}/{name}`: Tail log file using `notify` (inotify) for live logs, serve complete file for finished workflows
  - Keep-alive pings every 30 seconds
- [x] Implement the `Notifier` pattern (broadcast channel for new events)
- [ ] Write integration tests for the HTTP API

### Phase 6 — Main Server & Pipeline Orchestration

**Goal**: Wire everything together into the main `tangled-spindle` binary.

- [x] Implement `server.rs`:
  - Load config from environment
  - Initialize DB, RBAC, secrets manager, engine, job queue, jetstream client, knot consumer
  - Configure owner in RBAC
  - Start all subsystems concurrently
  - Graceful shutdown on SIGTERM/SIGINT
- [x] Implement pipeline processing (matching upstream `processPipeline`):
  - Parse `sh.tangled.pipeline` events
  - Validate trigger metadata and repo ownership
  - Map workflows to engines
  - Build pipeline environment variables
  - Enqueue job to the bounded queue
- [x] Implement workflow execution orchestration (matching upstream `engine.go:StartWorkflows`):
  - Run all workflows in parallel (`tokio::spawn`)
  - For each workflow: `setup_workflow` → run steps sequentially → `destroy_workflow`
  - Status transitions: `pending` → `running` → (`success` | `failed` | `timeout` | `cancelled`)
  - Extract secrets from vault for the repo
  - Stream control log lines (step start/end) and data log lines (stdout/stderr)
- [x] Implement job queue with configurable concurrency (matching upstream `queue.go`)

### Phase 7 — NixOS Module

**Goal**: A production-ready NixOS module for declarative spindle deployment, following the same pattern as `services.github-runners`.

```nix
# Example usage:
services.tangled-spindles = {
  runner1 = {
    enable = true;
    hostname = "spindle1.example.com";
    owner = "did:plc:abc123";
    tokenFile = "/run/secrets/spindle1-token";
  };
  runner2 = {
    enable = true;
    hostname = "spindle2.example.com";
    owner = "did:plc:def456";
    tokenFile = "/run/secrets/spindle2-token";
    engine.maxJobs = 4;
    engine.workflowTimeout = "30m";
  };
};
```

#### Module options (`nixos-module.nix`)

- [x] `services.tangled-spindles` — `attrsOf (submodule { ... })`, one entry per runner instance
- [x] Per-runner options:
  - `enable` — `bool`, default `false`
  - `package` — `package`, default `tangled-spindle-nix`
  - `hostname` — `str`, **required**. Public hostname of this spindle instance
  - `owner` — `str`, **required**. DID of the spindle owner
  - `tokenFile` — `str`, **required**. Path to the authentication token file
  - `listenAddr` — `str`, default `"127.0.0.1:6555"`. Address the HTTP server binds to
  - `jetstreamEndpoint` — `str`, default `"wss://jetstream1.us-west.bsky.network/subscribe"`
  - `plcUrl` — `str`, default `"https://plc.directory"`
  - `dbPath` — `str`, default `null` (uses `StateDirectory` path)
  - `logDir` — `str`, default `null` (uses `LogsDirectory` path)
  - `dev` — `bool`, default `false`
  - `engine` — submodule:
    - `maxJobs` — `int`, default `2`. Max concurrent workflow executions
    - `queueSize` — `int`, default `100`. Max pending jobs in queue
    - `workflowTimeout` — `str`, default `"5m"`
    - `nixery` — `str`, default `"nixery.tangled.sh"`. Nixery URL (for compatibility / fallback)
    - `extraNixFlags` — `listOf str`, default `[]`. Extra flags passed to `nix build`
  - `secrets` — submodule:
    - `provider` — `enum ["sqlite" "openbao"]`, default `"sqlite"`
    - `openbao.proxyAddr` — `str`, default `"http://127.0.0.1:8200"`
    - `openbao.mount` — `str`, default `"spindle"`
  - `extraEnvironment` — `attrsOf str`, default `{}`. Additional env vars for the service
  - `extraPackages` — `listOf package`, default `[]`. Extra packages in `PATH`
  - `serviceOverrides` — `attrs`, default `{}`. Override systemd service options
  - `user` — `nullOr str`, default `null` (DynamicUser)
  - `group` — `nullOr str`, default `null` (DynamicUser)

#### systemd service generation

For each enabled runner, generate a systemd service `tangled-spindle-{name}`. The runner daemon executes workflow steps as child processes — all sandboxing is inherited automatically, no `systemd-run` or special permissions needed.

- [x] `ExecStart` — `${package}/bin/tangled-spindle`
- [x] `Environment` — Map all config options to `SPINDLE_SERVER_*` env vars
- [x] `StateDirectory` — `tangled-spindle/{name}` (SQLite DB, workspace root)
- [x] `LogsDirectory` — `tangled-spindle/{name}` (workflow log files)
- [x] `RuntimeDirectory` — `tangled-spindle/{name}` (runtime data)
- [x] Sandboxing (applied to the service, inherited by all child processes):
  - `DynamicUser=true` (unless `user` is set)
  - `ProtectSystem=strict`
  - `ProtectHome=yes`
  - `PrivateTmp=yes`
  - `ProtectKernelTunables=yes`
  - `ProtectKernelModules=yes`
  - `ProtectControlGroups=yes`
  - `ProtectClock=yes`
  - `ProtectKernelLogs=yes`
  - `ProtectHostname=yes`
  - `NoNewPrivileges=yes`
  - `PrivateDevices=yes`
  - `PrivateMounts=yes`
  - `PrivateUsers=yes`
  - `RemoveIPC=yes`
  - `RestrictSUIDSGID=yes`
  - `RestrictNamespaces=yes`
  - `RestrictRealtime=yes`
  - `ProtectProc=invisible`
  - `ReadWritePaths=/var/lib/tangled-spindle/{name} /var/log/tangled-spindle/{name}`
  - `MemoryDenyWriteExecute=false` (needed for Nix/Node)
  - `SystemCallFilter=~@clock ~@cpu-emulation ~@module ~@mount ~@obsolete ~@raw-io ~@reboot ~capset ~setdomainname ~sethostname`
  - `RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX AF_NETLINK`
- [x] Resource limits (configurable per runner):
  - `CPUQuota=` — optional CPU limit
  - `MemoryMax=` — optional memory limit
  - `TasksMax=` — optional process count limit
- [x] Token file handling:
  - `ExecStartPre=+` script (runs as root) to copy token file into state directory with appropriate permissions
  - `InaccessiblePaths=-{tokenFile}` after copying
- [x] `PATH` includes: `bash`, `coreutils`, `git`, `gnutar`, `gzip`, `nix`, plus `extraPackages`
- [x] Restart policy: `on-failure` with rate limiting
- [x] `After=network-online.target nix-daemon.service`
- [x] `Wants=network-online.target nix-daemon.service`
- [x] `PrivateNetwork=false` (default — steps need network for git, API calls, etc.)

### Phase 8 — Integration Testing & Hardening

**Goal**: End-to-end tests and production hardening.

- [ ] NixOS VM integration test (`nixos/tests`-style):
  - Start a VM with the NixOS module configured
  - Simulate a pipeline event
  - Verify the workflow runs and steps execute as child processes
  - Verify log streaming works
  - Verify workspace cleanup
  - Verify sandboxing is inherited (test that a step cannot read `/etc/shadow`, write to `/`, etc.)
- [ ] Fuzz testing for YAML workflow parsing
- [ ] Stress test: queue saturation with many concurrent pipelines
- [ ] Security review:
  - Verify DynamicUser isolation between different runner services
  - Verify secret masking in logs
  - Verify that step processes inherit the service sandbox
  - Verify workspace cleanup between workflows
- [ ] Performance benchmarks: compare startup latency vs Docker (expected: significantly faster due to no image pull, no container overhead)

## Key Rust Dependencies

| Crate | Purpose |
|-------|---------|
| `tokio` | Async runtime |
| `axum` | HTTP server + WebSocket |
| `tokio-tungstenite` | WebSocket client (Jetstream, knot events) |
| `rusqlite` | SQLite database |
| `serde` / `serde_json` / `serde_yaml` | Serialization |
| `clap` | CLI argument parsing |
| `tracing` / `tracing-subscriber` | Structured logging |
| `reqwest` | HTTP client (OpenBao, XRPC, ID resolution) |
| `casbin` | RBAC enforcement |
| `notify` | Filesystem watching (log file tailing) |
| `tokio::process` | Spawning `nix build` and step child processes |
| `ring` or `aes-gcm` | Secret encryption at rest (SQLite backend) |
| `base64` | Encoding |
| `regex` | Workflow ID normalization |
| `uuid` | Unique identifiers |

## Compatibility Goals

- **Wire-compatible** with the upstream spindle protocol: same WebSocket event format, same XRPC endpoints, same log line JSON schema. An appview / tangled.org frontend that works with the Go spindle should work identically with this implementation.
- **Config-compatible**: Accepts the same `SPINDLE_SERVER_*` environment variables. Existing deployment configs should work with minimal changes (mainly adding `SPINDLE_ENGINE=nix` or using the NixOS module).
- **Workflow-compatible**: Runs the same `.tangled/workflows/*.yml` pipeline manifests. The `dependencies` field maps to Nix packages the same way Nixery does.

## Non-Goals (out of scope for v1)

- Docker/Podman engine support (use the upstream Go spindle for that)
- macOS / non-Linux support (systemd is Linux-only)
- Windows support
- Custom engine plugin system (just the nix engine for now)
- Web UI (use the tangled.org appview)
- Multi-architecture builds (the host architecture is the only target)
- Per-step isolation (service-level sandboxing is sufficient, matching the github-runners model)

## Open Questions

1. **Nix evaluation strategy**: Should we use `nix build --expr` inline, write temp `.nix` files, or use `nix shell` with flake refs? Flake refs are most modern but require the dependency sources to be flakes. Writing temp `.nix` files is most flexible and matches what Nixery effectively does.

2. **Nix store GC**: On a busy CI machine, the Nix store will grow. Should we integrate automatic GC (e.g., `nix-collect-garbage --delete-older-than 7d` on a timer)? Or leave that to the operator? The NixOS module could optionally set up a systemd timer.

3. **Caching across workflows**: If two workflows have the same `dependencies` set, the Nix closure will be identical. We can skip the `nix build` entirely by content-addressing the dependency map. How aggressive should this caching be?

4. **Network access during steps**: Steps need network access by default for `git clone`, package downloads, and API calls. Should there be a per-runner option to enable `PrivateNetwork=true` for hermetic builds? This would be a service-level setting affecting all steps.