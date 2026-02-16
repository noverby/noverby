# pkg-config-rs

A pure Rust rewrite and drop-in replacement for [pkg-config](https://www.freedesktop.org/wiki/Software/pkg-config/) / [pkgconf](https://github.com/pkgconf/pkgconf).

## Overview

`pkg-config-rs` aims to be a fully compatible, single-binary replacement for `pkg-config` and `pkgconf`, written entirely in Rust. It provides:

- **`pkgconf`** — a CLI binary that is a drop-in replacement for both `pkg-config` and `pkgconf`
- **`libpkgconf`** — a Rust library crate providing the core functionality for parsing `.pc` files, resolving dependencies, managing compiler/linker flags, and comparing versions

The implementation is modeled after [pkgconf](https://github.com/pkgconf/pkgconf) (the modern, maintained C implementation),
**not** the legacy freedesktop.org `pkg-config`.
This means we follow pkgconf's architecture for the dependency graph solver,
fragment handling, cross-compilation personality support, and other advanced features.

## Project Status

🚧 **Work in Progress** — The project is structured and foundational modules are implemented. See the implementation plan below for current progress and remaining work.

## Building

```sh
cargo build --release
```

The resulting binary is at `target/release/pkgconf`. To use it as a `pkg-config` replacement:

```sh
ln -sf pkgconf pkg-config
```

## Usage

```sh
# Query cflags for a package
pkgconf --cflags glib-2.0

# Query linker flags
pkgconf --libs zlib

# Check if a package exists with a minimum version
pkgconf --atleast-version=1.2.8 zlib

# Print the version of a package
pkgconf --modversion openssl

# List all known packages
pkgconf --list-all
```

All standard `pkg-config` and `pkgconf` flags are supported. See `pkgconf --help` for the full list.

## Architecture

The project is organized as a Cargo workspace with two crates:

```text
pkg-config-rs/
├── Cargo.toml              # Workspace root
├── README.md
├── PLAN.md                 # This implementation plan
├── crates/
│   ├── libpkgconf/         # Core library crate
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs          # Public API, constants, env var names
│   │       ├── error.rs        # Error types and ErrorFlags
│   │       ├── version.rs      # RPM-style version comparison
│   │       ├── parser.rs       # .pc file parsing, variable expansion
│   │       ├── dependency.rs   # Dependency spec parsing & representation
│   │       ├── fragment.rs     # Compiler/linker flag fragments
│   │       ├── client.rs       # (TODO) Client state & configuration
│   │       ├── pkg.rs          # (TODO) Package model & graph traversal
│   │       ├── cache.rs        # (TODO) Package cache
│   │       ├── path.rs         # (TODO) Search path management
│   │       ├── personality.rs  # (TODO) Cross-compilation personalities
│   │       ├── queue.rs        # (TODO) Package queue & solver
│   │       └── audit.rs        # (TODO) Audit logging
│   └── pkgconf/            # CLI binary crate
│       ├── Cargo.toml
│       └── src/
│           └── main.rs         # CLI entry point (clap-based)
└── tests/
    └── data/               # Test .pc files for integration tests
```

### Module Mapping (pkgconf C → Rust)

| pkgconf C source            | Rust module              | Status |
| --------------------------- | ------------------------ | ------ |
| `libpkgconf/libpkgconf.h`  | `libpkgconf/src/lib.rs`  | ✅     |
| `libpkgconf/pkg.c`          | `pkg.rs`                 | ⬜     |
| `libpkgconf/parser.c`       | `parser.rs`              | ✅     |
| `libpkgconf/fragment.c`     | `fragment.rs`            | ✅     |
| `libpkgconf/dependency.c`   | `dependency.rs`          | ✅     |
| `libpkgconf/tuple.c`        | `parser.rs` (variables)  | ✅     |
| `libpkgconf/client.c`       | `client.rs`              | ⬜     |
| `libpkgconf/cache.c`        | `cache.rs`               | ⬜     |
| `libpkgconf/path.c`         | `path.rs`                | ⬜     |
| `libpkgconf/personality.c`  | `personality.rs`         | ⬜     |
| `libpkgconf/queue.c`        | `queue.rs`               | ⬜     |
| `libpkgconf/audit.c`        | `audit.rs`               | ⬜     |
| `libpkgconf/argvsplit.c`    | `parser.rs` (argv_split) | ✅     |
| `libpkgconf/fileio.c`       | Rust stdlib              | ✅     |
| `libpkgconf/buffer.c`       | Rust `String`/`Vec`      | ✅     |
| `libpkgconf/output.c`       | `main.rs` (stdout/err)   | ✅     |
| `cli/main.c`                | `main.rs`                | ✅     |
| `cli/core.c`                | `main.rs`                | 🔶     |
| `cli/core.h`                | `main.rs`                | ✅     |
| `cli/renderer-msvc.c`       | (TODO)                   | ⬜     |

**Legend:** ✅ Done · 🔶 Partial · ⬜ Not started

---

## Environment Variables

All standard `pkg-config` / `pkgconf` environment variables are supported:

| Variable | Description |
| -------- | ----------- |
| `PKG_CONFIG_PATH` | Prepended to the default search path |
| `PKG_CONFIG_LIBDIR` | Replaces the default search path |
| `PKG_CONFIG_SYSROOT_DIR` | Sysroot directory for cross-compilation |
| `PKG_CONFIG_TOP_BUILD_DIR` | Build root directory |
| `PKG_CONFIG_ALLOW_SYSTEM_CFLAGS` | Don't filter system include dirs |
| `PKG_CONFIG_ALLOW_SYSTEM_LIBS` | Don't filter system lib dirs |
| `PKG_CONFIG_DISABLE_UNINSTALLED` | Never use uninstalled packages |
| `PKG_CONFIG_MAXIMUM_TRAVERSE_DEPTH` | Max dependency graph depth |
| `PKG_CONFIG_IGNORE_CONFLICTS` | Ignore conflicts rules |
| `PKG_CONFIG_PURE_DEPGRAPH` | Don't merge private fragments |
| `PKG_CONFIG_LOG` | Write an audit log to this file |
| `PKG_CONFIG_RELOCATE_PATHS` | Enable prefix redefinition |
| `PKG_CONFIG_DONT_DEFINE_PREFIX` | Disable prefix redefinition |
| `PKG_CONFIG_DONT_RELOCATE_PATHS` | Disable path relocation |
| `PKG_CONFIG_MSVC_SYNTAX` | Output flags in MSVC syntax |
| `PKG_CONFIG_FDO_SYSROOT_RULES` | Use FDO sysroot rules |
| `PKG_CONFIG_DEBUG_SPEW` | Enable debug output |
| `PKG_CONFIG_PRELOADED_FILES` | Colon-separated list of .pc files to preload |
