# Implementation Plan

This plan has been split into smaller documents for easier navigation. See [docs/plan/README.md](docs/plan/README.md) for the full index.

## Quick Links

- **[Current Status](docs/plan/status.md)** — Phase status table and unit file directive coverage
- **[Project Structure](docs/plan/structure.md)** — Cargo workspace layout (69 crates)
- **[Phase 0 — Foundation](docs/plan/phase-0.md)** — Workspace & Core Library
- **[Phase 1 — Core System](docs/plan/phase-1.md)** — PID 1 + systemctl + journald
- **[Phase 2 — Essential System Services](docs/plan/phase-2.md)** — udevd, tmpfiles, sysusers, logind, etc.
- **[Phase 3 — Network Stack](docs/plan/phase-3.md)** — networkd, resolved, timesyncd, etc.
- **[Phase 4 — Extended Services](docs/plan/phase-4.md)** — machined, nspawn, portabled, homed, cryptsetup, etc.
- **[Phase 5 — Utilities, Boot & Polish](docs/plan/phase-5.md)** — analyze, cgls, cgtop, generators, NixOS integration
- **[Integration Testing](docs/plan/integration.md)** — rust-nixos boot testing with cloud-hypervisor

See [CHANGELOG.md](CHANGELOG.md) for detailed recent changes.