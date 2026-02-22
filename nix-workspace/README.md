# nix-workspace

A Nickel-powered workspace manager for Nix flakes.

`nix-workspace` replaces [flakelight](https://github.com/accelbread/flakelight) and similar flake frameworks with a configuration layer built on [Nickel](https://nickel-lang.org/). It leverages Nickel's contract system and gradual typing to provide validated, well-documented workspace configuration with clear error messages — for both humans and AI agents.

## How it works

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
2. **Nix layer** — A library consumes the evaluated config and produces standard flake outputs using nixpkgs builders.
3. **Flake shim** — A thin `flake.nix` that calls `nix-workspace` with the workspace root.

## Quick start

### 1. Create `flake.nix`

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    nix-workspace.url = "github:noverby/nix-workspace";
  };

  outputs = inputs:
    inputs.nix-workspace ./. {
      inherit inputs;
    };
}
```

### 2. Create `workspace.ncl`

```nickel
{
  name = "my-project",
  description = "My Nix workspace",

  systems = ["x86_64-linux", "aarch64-linux"],

  nixpkgs = {
    allow-unfree = true,
  },

  packages = {
    my-tool = {
      src = "./src",
      build-system = "rust",
      description = "A CLI tool",
    },
  },

  shells = {
    default = {
      packages = ["cargo", "rustc", "rust-analyzer"],
    },
  },
}
```

### 3. Build and develop

```shell
nix build .#my-tool
nix develop
```

## Directory conventions

Place `.ncl` files in convention directories and they are auto-discovered:

| Directory      | Flake output                          | Description                  |
|----------------|---------------------------------------|------------------------------|
| `packages/`    | `packages.<system>.<name>`            | Package definitions          |
| `shells/`      | `devShells.<system>.<name>`           | Development shells           |
| `machines/`    | `nixosConfigurations.<name>`          | NixOS machine configs        |
| `modules/`     | `nixosModules.<name>`                 | NixOS modules                |
| `home/`        | `homeModules.<name>`                  | Home-manager modules         |
| `overlays/`    | `overlays.<name>`                     | Nixpkgs overlays             |
| `lib/`         | `lib.<name>`                          | Library functions            |
| `templates/`   | `templates.<name>`                    | Flake templates              |
| `checks/`      | `checks.<system>.<name>`             | CI checks                    |

Convention directories are configurable:

```nickel
{
  conventions = {
    packages.path = "pkgs",           # Use pkgs/ instead of packages/
    overlays.auto-discover = false,   # Disable auto-discovery for overlays
  },
}
```

## System multiplexing

Declare systems once — they apply everywhere:

```nickel
{
  systems = ["x86_64-linux", "aarch64-linux"],

  packages = {
    my-tool = {
      build-system = "rust",
    },
    linux-only = {
      systems = ["x86_64-linux"],   # Override for this package only
      build-system = "rust",
    },
  },
}
```

You never write `packages.x86_64-linux.my-tool` — you write `packages.my-tool` and the system dimension is managed for you.

## Contracts

`nix-workspace` ships Nickel contracts that validate your configuration and provide clear error messages:

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

### Contract hierarchy

```text
Workspace
├── WorkspaceConfig          # Top-level workspace.ncl structure
├── NixpkgsConfig            # nixpkgs settings
├── PackageConfig            # Package definition
├── ShellConfig              # Development shell
├── ConventionConfig         # Directory convention override
└── Common
    ├── System               # Valid Nix system triple
    ├── Name                 # Valid derivation name
    ├── NonEmptyString       # Non-empty string
    ├── RelativePath         # Relative file path
    └── ModuleRef            # Module name or path reference
```

### Standalone validation

You can validate your workspace configuration without building anything:

```shell
nickel typecheck workspace.ncl
nickel export workspace.ncl    # Produces validated JSON
```

## Package build systems

The `build-system` field selects the Nix builder:

| Value       | Nix builder                              |
|-------------|------------------------------------------|
| `"generic"` | `stdenv.mkDerivation` (default)          |
| `"rust"`    | `rustPlatform.buildRustPackage`           |
| `"go"`      | `buildGoModule`                           |

### Package config fields

```nickel
{
  src = "./src",                           # Source directory (relative)
  build-system = "generic",                # Builder to use
  description = "My package",              # Human-readable description
  systems = ["x86_64-linux"],              # Override workspace systems
  build-inputs = ["openssl", "zlib"],      # Runtime dependencies
  native-build-inputs = ["pkg-config"],    # Build-time dependencies
  env = { MY_VAR = "value" },             # Build environment variables
  meta = {                                 # Package metadata
    homepage = "https://example.com",
    license = "MIT",
  },
  override = { },                          # Escape hatch: raw Nix attrs
}
```

## Shell config fields

```nickel
{
  packages = ["cargo", "rustc"],           # Packages in the shell
  env = { RUST_LOG = "debug" },            # Environment variables
  shell-hook = "echo hello",               # Script to run on entry
  tools = { rust-analyzer = "" },          # Tool specifications
  systems = ["x86_64-linux"],              # Override workspace systems
  inputs-from = ["my-tool"],               # Include build inputs from packages
}
```

## Project structure

```text
nix-workspace/
├── SPEC.md                    # Full specification
├── README.md                  # This file
├── flake.nix                  # Project flake (exposes the library)
│
├── lib/                       # Nix library
│   ├── default.nix            # Main entry point (mkWorkspace)
│   ├── discover.nix           # Directory auto-discovery
│   ├── systems.nix            # System multiplexing
│   ├── eval-nickel.nix        # Nickel evaluation via IFD
│   └── builders/
│       ├── packages.nix       # Package builder
│       └── shells.nix         # Shell builder
│
├── contracts/                 # Nickel contracts
│   ├── workspace.ncl          # WorkspaceConfig contract
│   ├── package.ncl            # PackageConfig contract
│   ├── shell.ncl              # ShellConfig contract
│   └── common.ncl             # Shared types (System, Name, etc.)
│
├── examples/
│   └── minimal/               # Minimal workspace example
│       ├── flake.nix
│       ├── workspace.ncl
│       └── packages/
│           └── hello.ncl
│
└── tests/
    └── unit/                  # Nickel contract unit tests
        ├── common.ncl
        ├── package.ncl
        └── workspace.ncl
```

## Development

```shell
# Enter the dev shell
nix develop

# Run contract checks
nix flake check

# Validate contracts manually
nickel typecheck contracts/common.ncl
nickel typecheck contracts/package.ncl
nickel typecheck contracts/shell.ncl
nickel typecheck contracts/workspace.ncl

# Run unit tests
nickel eval tests/unit/common.ncl
nickel eval tests/unit/package.ncl
nickel eval tests/unit/workspace.ncl
```

## Roadmap

See [SPEC.md](./SPEC.md) for the full specification and milestone details.

- **v0.1 — Foundation** — Core contracts, package/shell discovery, system multiplexing ← _current_
- **v0.2 — NixOS integration** — Machine and module configs
- **v0.3 — Subworkspaces** — Monorepo support with auto-namespacing
- **v0.4 — Plugin system** — Extensible build systems and conventions
- **v0.5 — Standalone CLI** — `nix-workspace init`, `check`, `build`, `shell`
- **v1.0 — Production ready** — Full coverage, migration guides, editor integration

## License

See [LICENSE](../LICENSE).