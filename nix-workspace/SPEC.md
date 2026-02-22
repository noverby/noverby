# nix-workspace

A Nickel-powered workspace manager for Nix flakes.

## Overview

`nix-workspace` replaces [flakelight](https://github.com/accelbread/flakelight) and similar flake frameworks with a configuration layer built on [Nickel](https://nickel-lang.org/). It leverages Nickel's contract system and gradual typing to provide validated, well-documented workspace configuration with clear error messages — for both humans and AI agents.

The core idea: Nickel handles the *what* (declarative structure, validation, error reporting) and Nix handles the *how* (building derivations, evaluating modules). Users write `workspace.ncl` instead of complex Nix expressions, and `nix-workspace` produces standard flake outputs.

## Motivation

### Problems with Nix the language

- **No type system** — Configuration errors surface deep in evaluation as cryptic stack traces ("attribute 'x' missing at /nix/store/...-source/...nix:47:13") rather than at the point of misconfiguration.
- **Poor error messages** — Nix errors are positional, deeply nested, and lack context about *what the user was trying to do*. AI agents struggle to parse and act on them.
- **Boilerplate** — Every flake repeats the same `eachSystem`, `inputs.nixpkgs.legacyPackages`, and output-wiring patterns.
- **Footguns** — Namespace collisions, implicit behaviors, and the gap between "what you wrote" and "what Nix evaluated" cause subtle bugs.

### Why Nickel

- **Contracts** — Rich validation with custom error messages. A `NixosConfiguration` contract can tell you *exactly* which field is wrong and what it expected.
- **Gradual typing** — Start untyped, add contracts incrementally. No all-or-nothing commitment.
- **Merge semantics** — Nickel's record merging is well-defined and predictable, unlike Nix's `//` and `mkMerge` interactions.
- **Structured errors** — Nickel errors include the contract name, the expected type, and the offending value — machine-parseable and human-readable.

## Architecture

### Hybrid evaluation model

```text
┌─────────────────┐     Nickel      ┌──────────────┐      JSON       ┌─────────────┐
│  workspace.ncl  │ ──evaluate──▶   │  Validated    │ ──export──▶    │  Nix library │
│  (user config)  │                 │  config tree  │                │  (builders)  │
└─────────────────┘                 └──────────────┘                └──────┬──────┘
                                                                          │
                                                                          ▼
                                                                   ┌─────────────┐
                                                                   │ Flake       │
                                                                   │ outputs     │
                                                                   └─────────────┘
```

1. **Nickel layer** — Defines workspace structure. Contracts validate all configuration. Exports a JSON-serializable config tree.
2. **Nix layer** — A library (`nix-workspace.lib`) consumes the evaluated config and produces standard flake outputs using nixpkgs builders, the NixOS module system, etc.
3. **Flake shim** — A thin `flake.nix` that calls `nix-workspace` with the workspace root, similar to how flakelight works today.

This hybrid avoids depending on the experimental `nickel-nix` bridge while getting Nickel's benefits where they matter most — the configuration surface.

### Evaluation phases

```text
Phase 1: Discovery
  Scan workspace root for workspace.ncl, subworkspaces, and convention directories.

Phase 2: Nickel evaluation
  Evaluate workspace.ncl with contracts applied.
  Merge subworkspace configs with automatic namespacing.
  Produce validated JSON config tree.

Phase 3: Nix construction
  Nix library reads JSON config.
  Builds flake outputs (packages, shells, NixOS configs, etc.).
  Applies system multiplexing.

Phase 4: Output
  Standard flake outputs, indistinguishable from a hand-written flake.
```

## User interface

### Flake integration (primary mode)

```nix
# flake.nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    nix-workspace.url = "github:example/nix-workspace";
  };

  outputs = inputs:
    inputs.nix-workspace ./. {
      inherit inputs;
    };
}
```

### Workspace configuration

```nickel
# workspace.ncl
{
  name = "my-project",
  description = "My Nix workspace",

  systems = ["x86_64-linux", "aarch64-linux"],

  # Nixpkgs configuration
  nixpkgs = {
    allow-unfree = true,
  },

  # Auto-discovered from packages/ directory, or declared explicitly
  packages = {
    my-tool = {
      src = "./src",
      build-system = "rust",
      description = "A CLI tool",
    },
  },

  # Auto-discovered from machines/ directory
  machines = {
    gravitas = {
      system = "x86_64-linux",
      modules = ["desktop", "development"],
    },
  },

  # Auto-discovered from shells/ directory
  shells = {
    default = {
      packages = ["cargo", "rustc", "rust-analyzer"],
    },
  },
}
```

### Standalone mode (future)

```shell
# Initialize a new workspace
nix-workspace init

# Validate workspace configuration
nix-workspace check

# Show workspace structure
nix-workspace info

# Build a package
nix-workspace build my-tool

# Enter a dev shell
nix-workspace shell
```

The CLI tool would be written in Rust, using the Nickel library for evaluation. It can either delegate to `nix build` / `nix develop` under the hood or generate a `flake.nix` on the fly.

## Directory conventions

### Default directory mapping

| Directory      | Flake output                          | Description                          |
|----------------|---------------------------------------|--------------------------------------|
| `packages/`    | `packages.<system>.<name>`            | Package definitions                  |
| `shells/`      | `devShells.<system>.<name>`           | Development shells                   |
| `machines/`    | `nixosConfigurations.<name>`          | NixOS machine configurations         |
| `modules/`     | `nixosModules.<name>`                 | NixOS modules                        |
| `home/`        | `homeModules.<name>`                  | Home-manager modules                 |
| `overlays/`    | `overlays.<name>`                     | Nixpkgs overlays                     |
| `lib/`         | `lib.<name>`                          | Library functions                    |
| `templates/`   | `templates.<name>`                    | Flake templates                      |
| `checks/`      | `checks.<system>.<name>`             | CI checks                            |

These names are inspired by Cargo (`packages/`) and Deno (`modules/`) conventions rather than mapping 1:1 to Nix flake output attribute names. The mapping is configurable:

```nickel
# workspace.ncl
{
  conventions = {
    # Rename the packages directory
    packages.path = "pkgs",

    # Disable auto-discovery for overlays
    overlays.auto-discover = false,
  },
}
```

### Subworkspaces

A subdirectory with its own `workspace.ncl` is a subworkspace. Outputs are automatically namespaced:

```text
my-monorepo/
├── workspace.ncl              # Root workspace
├── packages/
│   └── shared-lib.ncl         # → packages.<system>.shared-lib
├── rust-nixos/
│   ├── workspace.ncl          # Subworkspace
│   └── packages/
│       └── default.ncl        # → packages.<system>.rust-nixos
├── mojo-zed/
│   ├── workspace.ncl          # Subworkspace
│   └── packages/
│       └── default.ncl        # → packages.<system>.mojo-zed
│       └── lsp.ncl            # → packages.<system>.mojo-zed-lsp
└── backend/
    ├── workspace.ncl          # Subworkspace
    └── packages/
        └── default.ncl        # → packages.<system>.backend
        └── migrate.ncl        # → packages.<system>.backend-migrate
```

Namespacing rules:

- A subworkspace's `default` output uses the subworkspace directory name: `mojo-zed`
- Named outputs are prefixed with the subworkspace name: `mojo-zed-lsp`
- Root workspace outputs have no prefix
- All names are valid Nix derivation names (alphanumeric, hyphens, underscores)

Subworkspaces can declare dependencies on sibling subworkspaces:

```nickel
# mojo-zed/workspace.ncl
{
  name = "mojo-zed",
  dependencies = {
    wasm = "mojo-wasm",
  },
}
```

## Nickel contracts

### Contract hierarchy

```text
Workspace
├── WorkspaceConfig          # Top-level workspace.ncl structure
├── NixpkgsConfig            # nixpkgs settings (allowUnfree, overlays, etc.)
├── SubworkspaceConfig       # Subworkspace declaration
│
├── PackageConfig            # Package definition
│   ├── RustPackage          # Rust-specific (Cargo.toml, features, etc.)
│   ├── GoPackage            # Go-specific
│   └── GenericPackage       # Language-agnostic (stdenv.mkDerivation)
│
├── ShellConfig              # Development shell
├── MachineConfig            # NixOS configuration
│   ├── System               # "x86_64-linux" | "aarch64-linux" | ...
│   └── ModuleRef            # Reference to a NixOS module
│
├── HomeConfig               # Home-manager configuration
├── ModuleConfig             # NixOS/HM module definition
├── OverlayConfig            # Nixpkgs overlay
├── CheckConfig              # CI check definition
└── TemplateConfig           # Flake template
```

### Example contract

```nickel
# contracts/machine.ncl
{
  MachineConfig = {
    system
      | System
      | doc "Target system architecture",

    state-version
      | String
      | doc "NixOS state version (e.g. \"25.05\")"
      | default = "25.05",

    modules
      | Array ModuleRef
      | doc "NixOS modules to include"
      | default = [],

    special-args
      | { _ : Dyn }
      | doc "Extra arguments passed to modules"
      | default = {},
  },

  System = std.contract.from_predicate (fun s =>
    std.array.elem s [
      "x86_64-linux",
      "aarch64-linux",
      "x86_64-darwin",
      "aarch64-darwin",
    ]
  ),

  ModuleRef
    | doc "A reference to a NixOS module by name or path"
    = String,
}
```

### Error output

When a contract fails, the error includes:

```text
error: contract violation in machines/gravitas.ncl
  ┌─ machines/gravitas.ncl:3:13
  │
3 │   system = "x86-linux",
  │             ^^^^^^^^^^^ this value
  │
  expected: System (one of "x86_64-linux", "aarch64-linux", "x86_64-darwin", "aarch64-darwin")
       got: "x86-linux"
      hint: did you mean "x86_64-linux"?
  contract: MachineConfig.system
```

### Structured diagnostics

For programmatic consumption (AI agents, editors, CI), errors are also emitted as JSON:

```json
{
  "diagnostics": [
    {
      "code": "NW001",
      "severity": "error",
      "file": "machines/gravitas.ncl",
      "line": 3,
      "column": 13,
      "field": "system",
      "message": "Expected System (one of \"x86_64-linux\", \"aarch64-linux\", \"x86_64-darwin\", \"aarch64-darwin\"), got \"x86-linux\"",
      "hint": "Did you mean \"x86_64-linux\"?",
      "contract": "MachineConfig.system",
      "context": {
        "workspace": "my-project",
        "output": "nixosConfigurations.gravitas"
      }
    }
  ]
}
```

Diagnostic codes are prefixed `NW` (nix-workspace) and grouped:

- `NW0xx` — Contract violations (type/value errors)
- `NW1xx` — Discovery errors (missing files, bad directory structure)
- `NW2xx` — Namespace conflicts (duplicate names, invalid derivation names)
- `NW3xx` — Module errors (missing dependencies, circular imports)
- `NW4xx` — System errors (unsupported system, missing input)

## System multiplexing

Systems are declared once and applied automatically:

```nickel
# workspace.ncl
{
  systems = ["x86_64-linux", "aarch64-linux"],

  packages = {
    my-tool = {
      # Available on all systems by default
      build-system = "rust",
    },
    linux-only = {
      # Override systems for specific packages
      systems = ["x86_64-linux"],
      build-system = "rust",
    },
  },
}
```

The Nix layer handles the `eachSystem` expansion. Users never write `packages.x86_64-linux.my-tool` — they write `packages.my-tool` and the system dimension is managed for them.

## Module system

`nix-workspace` supports plugins that extend the core functionality:

```nickel
# workspace.ncl
{
  plugins = [
    "nix-workspace-rust",      # Rust build support (Cargo integration)
    "nix-workspace-home",      # Home-manager integration
    "nix-workspace-deploy",    # Deployment support
  ],
}
```

### Plugin interface

A plugin is a Nickel record that can:

1. **Add contracts** — New configuration types (e.g., `RustPackage` with `cargo-features`, `edition`, etc.)
2. **Add conventions** — New directory mappings (e.g., `crates/` → Rust packages)
3. **Add builders** — Nix functions that construct outputs from config (e.g., `buildRustPackage`)
4. **Extend existing contracts** — Add fields to `PackageConfig`, `ShellConfig`, etc.

```nickel
# Plugin: nix-workspace-rust
{
  contracts = {
    RustPackage = {
      edition
        | [| '2021, '2024 |]
        | default = '2024,
      features
        | Array String
        | default = [],
      cargo-lock
        | String
        | default = "./Cargo.lock",
    },
  },

  conventions = {
    crates = {
      path = "crates",
      output = "packages",
      builder = "rust",
    },
  },
}
```

## Comparison

| Feature                    | nix-workspace          | flakelight                | flake-parts              |
|----------------------------|------------------------|---------------------------|--------------------------|
| Configuration language     | Nickel                 | Nix                       | Nix                      |
| Type validation            | Nickel contracts       | Nix module system types   | Nix module system types  |
| Error messages             | Structured + readable  | Nix stack traces          | Nix stack traces         |
| AI agent support           | JSON diagnostics       | None                      | None                     |
| Auto-discovery             | Convention directories | nixDir + aliases          | Manual                   |
| Subworkspaces              | Native                 | Manual imports            | Manual imports           |
| System multiplexing        | Automatic              | Automatic                 | perSystem module         |
| Plugin system              | Nickel plugins         | Flakelight modules        | flake-parts modules      |
| Standalone mode            | Planned                | No                        | No                       |

## Project structure

```text
nix-workspace/
├── SPEC.md                    # This specification
├── README.md                  # User-facing documentation
├── default.nix                # Flake entry point (package + dev shell)
│
├── lib/                       # Nix library (flake output builders)
│   ├── default.nix            # Main entry point: nix-workspace function
│   ├── discover.nix           # Directory auto-discovery
│   ├── systems.nix            # System multiplexing
│   ├── namespacing.nix        # Subworkspace name resolution
│   └── builders/              # Output-type-specific builders
│       ├── packages.nix
│       ├── shells.nix
│       ├── machines.nix
│       ├── modules.nix
│       ├── overlays.nix
│       └── checks.nix
│
├── contracts/                 # Nickel contracts
│   ├── workspace.ncl          # Top-level workspace contract
│   ├── package.ncl            # Package contracts
│   ├── machine.ncl            # NixOS machine contracts
│   ├── shell.ncl              # Dev shell contracts
│   ├── module.ncl             # Module contracts
│   └── common.ncl             # Shared types (System, ModuleRef, etc.)
│
├── src/                       # Rust CLI (standalone mode)
│   ├── Cargo.toml
│   └── ...
│
├── examples/                  # Example workspaces
│   ├── minimal/               # Single package workspace
│   │   ├── flake.nix
│   │   ├── workspace.ncl
│   │   └── packages/
│   │       └── hello.ncl
│   ├── monorepo/              # Multi-subworkspace monorepo
│   │   ├── flake.nix
│   │   ├── workspace.ncl
│   │   ├── lib-a/
│   │   │   ├── workspace.ncl
│   │   │   └── packages/
│   │   │       └── default.ncl
│   │   └── app-b/
│   │       ├── workspace.ncl
│   │       └── packages/
│   │           └── default.ncl
│   └── nixos/                 # NixOS machine configuration
│       ├── flake.nix
│       ├── workspace.ncl
│       ├── machines/
│       │   └── my-machine.ncl
│       └── modules/
│           └── desktop.ncl
│
└── tests/                     # Test suite
    ├── unit/                  # Nickel contract tests
    │   ├── workspace.ncl
    │   ├── package.ncl
    │   └── machine.ncl
    ├── integration/           # Full workspace evaluation tests
    │   ├── discovery.nix
    │   ├── namespacing.nix
    │   ├── systems.nix
    │   └── subworkspaces.nix
    └── errors/                # Error message snapshot tests
        ├── invalid-system.ncl
        ├── missing-field.ncl
        ├── namespace-conflict.ncl
        └── expected/          # Expected error output
            ├── invalid-system.json
            ├── missing-field.json
            └── namespace-conflict.json
```

## Milestones

### v0.1 — Foundation

- [x] Nickel contracts for core workspace config (`WorkspaceConfig`, `System`)
- [x] Nix library entry point (`nix-workspace` function callable from `flake.nix`)
- [x] Package auto-discovery from `packages/` directory
- [x] Dev shell auto-discovery from `shells/` directory
- [x] System multiplexing
- [x] Basic structured error output
- [x] Example: minimal workspace
- [x] Unit tests for contracts

### v0.2 — NixOS integration

- [x] `MachineConfig` contract with full validation
- [x] `ModuleConfig` contract
- [x] Machine auto-discovery from `machines/` directory
- [x] Module auto-discovery from `modules/` directory
- [x] Home-manager module support (`home/` directory)
- [x] Example: NixOS machine configuration
- [x] Integration tests for machine building

### v0.3 — Subworkspaces

- [ ] Subworkspace discovery and config merging
- [ ] Automatic output namespacing with hyphen separator
- [ ] Cross-subworkspace dependency resolution
- [ ] Namespace conflict detection with `NW2xx` diagnostics
- [ ] Example: monorepo with subworkspaces
- [ ] Integration tests for namespacing

### v0.4 — Plugin system

- [ ] Plugin interface definition (contracts, conventions, builders)
- [ ] Plugin loading and merging
- [ ] Built-in plugins: `nix-workspace-rust`, `nix-workspace-go`
- [ ] Custom convention directory support
- [ ] Plugin contract extension mechanism

### v0.5 — Standalone CLI

- [ ] Rust CLI skeleton (`nix-workspace init`, `check`, `info`)
- [ ] `nix-workspace build` delegating to `nix build`
- [ ] `nix-workspace shell` delegating to `nix develop`
- [ ] JSON diagnostic output via `--format json`
- [ ] On-the-fly `flake.nix` generation for non-flake projects

### v1.0 — Production ready

- [ ] Complete contract coverage for all flake output types
- [ ] Comprehensive error catalog with all `NWxxx` codes documented
- [ ] Migration guide from flakelight / flake-parts
- [ ] Editor integration (LSP diagnostics via Nickel LSP)
- [ ] CI integration guide (GitHub Actions, etc.)
- [ ] Full documentation and examples