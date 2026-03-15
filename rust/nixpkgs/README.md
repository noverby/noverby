# rust-nixpkgs

Rust replacements for the C toolchain that builds Nix packages.

## Overview

`rust-nixpkgs` is the build-time counterpart to [rust-nixos](../nixos). Where rust-nixos replaces runtime system components (systemd, bash, coreutils) in a running NixOS system, rust-nixpkgs replaces the **build tools** — the stdenv toolchain that Nix uses to compile and package software.

The nixpkgs standard environment (`stdenv`) is built on a stack of C-based GNU tools: bash runs the build scripts, coreutils provides filesystem primitives, make drives compilation, tar/gzip/xz handle source archives, sed/awk/grep do text processing, and patchelf/strip do binary fixup. Every package in nixpkgs is built by this toolchain.

`rust-nixpkgs` provides Nix abstractions — a component registry and stdenv override mechanism — so that each C tool can be incrementally replaced with a Rust drop-in. Individual rewrites live as sibling subprojects at the monorepo root (e.g. `../make-rs`, `../sed-rs`) or come from existing Rust projects already in nixpkgs (e.g. uutils for coreutils, brush for bash).

## Architecture

```text
┌──────────────────────────────────────────────────────────┐
│                    rust-nixpkgs                             │
│                                                          │
│  components/          Nix declarations for each tool     │
│    ├── shell.nix        bash → brush (available)         │
│    ├── coreutils.nix    coreutils → uutils (available)   │
│    ├── make.nix         gnumake → make-rs (planned)      │
│    ├── sed.nix          gnused → sed-rs (planned)        │
│    ├── ...              ...                              │
│    └── default.nix      Registry loader                  │
│                                                          │
│  lib.nix              Helper functions                   │
│  stdenv.nix           Rust stdenv assembler              │
│  default.nix          Flakelight entry (overlay, pkgs)   │
│  PLAN.md              Phased replacement roadmap         │
└──────────┬──────────────────────────────────┬────────────┘
           │                                  │
           ▼                                  ▼
   Existing Rust rewrites              Repo-root subprojects
   (already in nixpkgs)               (created as needed)

   • uutils-coreutils                 • make-rs/
   • brush                            • sed-rs/
   • ripgrep (not drop-in)            • tar-rs/
                                      • patch-rs/
                                      • patchelf-rs/
                                      • ...
```

## Components

The component registry tracks every tool in the stdenv `initialPath` plus the binary fixup tools used by `mkDerivation`. Each component has a status:

| Component | Original | Rust Replacement | Phase | Status |
|-----------|----------|-----------------|-------|--------|
| Shell | bash | [brush](https://github.com/reubeno/brush) | 1 | ✅ Available |
| Core utilities | coreutils | [uutils](https://github.com/uutils/coreutils) | 1 | ✅ Available |
| Text search | gnugrep | grep-rs | 2 | ⏳ Planned |
| Stream editor | gnused | sed-rs | 2 | ⏳ Planned |
| Pattern processing | gawk | awk-rs | 2 | ⏳ Planned |
| File comparison | diffutils | diffutils-rs | 2 | ⏳ Planned |
| File search | findutils | findutils-rs | 2 | ⏳ Planned |
| Tape archive | gnutar | tar-rs | 3 | ⏳ Planned |
| Gzip compression | gzip | gzip-rs | 3 | ⏳ Planned |
| Bzip2 compression | bzip2 | bzip2-rs | 3 | ⏳ Planned |
| XZ compression | xz | xz-rs | 3 | ⏳ Planned |
| Build driver | gnumake | make-rs | 4 | ⏳ Planned |
| Patch application | gnupatch | patch-rs | 4 | ⏳ Planned |
| ELF patching | patchelf | patchelf-rs | 5 | ⏳ Planned |
| Symbol stripping | binutils strip | strip-rs | 5 | ⏳ Planned |

## How It Works

### Component Registry

Each component is declared in `components/*.nix` as a small attrset:

```nix
# components/coreutils.nix
{ pkgs, lib, mkComponent, status, source, ... }:
mkComponent {
  name = "coreutils";
  original = pkgs.coreutils;
  replacement = pkgs.uutils-coreutils-noprefix;
  status = status.available;
  source = source.nixpkgs;
  phase = 1;
  description = "Core file, text, and shell utilities";
  notes = "Using uutils-coreutils-noprefix";
}
```

Components with `replacement = null` are tracked for status reporting but skipped when assembling the stdenv. This lets us declare the full target map up front and fill in replacements incrementally.

### Stdenv Override

The `stdenv.nix` module reads the component registry and produces a modified stdenv where every C tool with a Rust replacement is swapped out of the `initialPath`:

```nix
# The overlay provides stdenvRs — a stdenv with Rust tools
pkgs.stdenvRs.mkDerivation {
  pname = "hello";
  # ... this package is built with uutils, brush, etc.
}
```

### Drop-in Requirement

Replacements must be **flag-compatible drop-ins** for the originals. This means:

- Same binary names (`ls`, `grep`, `make`, not `rg`, `fd`, `just`)
- Same CLI flags (GNU extensions included where configure scripts rely on them)
- Same output format (scripts parse stdout of these tools)
- Same exit codes

Tools like `ripgrep`, `fd`, and `just` are excellent but are **not** drop-ins — they have different flags, output formats, and semantics. The goal is to replace the C implementations without changing any build scripts.

## Adding a New Rewrite

### Option A: Existing Rust Package from Nixpkgs

If a drop-in Rust replacement already exists in nixpkgs:

1. Edit the component file in `components/` (e.g. `components/coreutils.nix`)
2. Set `replacement = pkgs.<package-name>;`
3. Set `status = status.available;`
4. Set `source = source.nixpkgs;`

### Option B: New Repo-Root Subproject

For a new Rust rewrite developed in this monorepo:

1. Create the subproject at the repo root (e.g. `make-rs/`)
2. Add a `default.nix` following the pattern in [rust-pkg-config](../pkg-config) or [rust-systemd](../systemd)
3. Import it in the root `flake.nix`
4. Update the component file to reference it:

```nix
# components/make.nix
{ pkgs, lib, mkComponent, status, source, ... }:
mkComponent {
  name = "make";
  original = pkgs.gnumake;
  replacement = pkgs.make-rs;       # ← point to the new package
  status = status.available;         # ← update status
  source = source.repo;
  phase = 4;
  description = "Build system driver (GNU Make)";
  notes = "Rust rewrite at ../make-rs";
}
```

5. Add `rust-nixpkgs` scope to your commit: `feat(rust-nixpkgs): Wire in make-rs replacement`

## Usage

### Prerequisites

- [Nix](https://nixos.org/) with flakes enabled
- [just](https://github.com/casey/just) (available in the dev shell)

### Commands

```shell
# Enter the dev shell
cd rust-nixpkgs
direnv allow  # or: nix develop .#rust-nixpkgs

# Show component status
just status

# Build the status test derivation
just build

# Test building a simple package with the Rust stdenv
just test

# Compare stdenv initialPath (original vs. Rust)
just compare
```

## Relationship to Other Projects

| Project | Scope | Approach |
|---------|-------|----------|
| **rust-nixpkgs** | Build-time tools (stdenv) | Replace the toolchain that builds packages |
| [rust-nixos](../nixos) | Runtime system (NixOS) | Replace components in the running OS |
| [rust-systemd](../systemd) | Init system | Full systemd rewrite, used by rust-nixos |
| [rust-pkg-config](../pkg-config) | Build dependency lookup | Drop-in pkg-config replacement |

Together, these projects work toward a fully oxidized Nix ecosystem: packages are **built** with Rust tools (rust-nixpkgs), the resulting **system** runs Rust services (rust-nixos + rust-systemd), and the **build system** itself uses Rust utilities (rust-pkg-config).

## Project Structure

```text
rust-nixpkgs/
├── default.nix          # Flakelight entry: devShell, overlay, packages
├── lib.nix              # Component registry helpers (mkComponent, mkRustStdenv)
├── stdenv.nix           # Rust stdenv assembler
├── components/
│   ├── default.nix      # Registry loader (imports all component files)
│   ├── shell.nix        # bash → brush
│   ├── coreutils.nix    # coreutils → uutils
│   ├── findutils.nix    # find/xargs → findutils-rs (planned)
│   ├── diffutils.nix    # diff/cmp → diffutils-rs (planned)
│   ├── sed.nix          # gnused → sed-rs (planned)
│   ├── grep.nix         # gnugrep → grep-rs (planned)
│   ├── awk.nix          # gawk → awk-rs (planned)
│   ├── tar.nix          # gnutar → tar-rs (planned)
│   ├── gzip.nix         # gzip → gzip-rs (planned)
│   ├── bzip2.nix        # bzip2 → bzip2-rs (planned)
│   ├── xz.nix           # xz → xz-rs (planned)
│   ├── make.nix         # gnumake → make-rs (planned)
│   ├── patch.nix        # gnupatch → patch-rs (planned)
│   ├── patchelf.nix     # patchelf → patchelf-rs (planned)
│   └── strip.nix        # binutils strip → strip-rs (planned)
├── PLAN.md              # Phased replacement roadmap
├── README.md            # This file
└── justfile             # Build, test, and status commands
```

## License

See [LICENSE](../LICENSE).