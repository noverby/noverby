# Implementation Plan

This document describes the phased plan for rewriting systemd as a pure Rust drop-in replacement. See [CHANGELOG.md](../../CHANGELOG.md) for detailed recent changes.

## Current Status

**🟢 NixOS boots successfully with systemd-rs as PID 1** — reaches `multi-user.target` with login prompt in ~6 seconds (cloud-hypervisor VM, full networking via networkd + resolved). **7,522 unit tests passing** across 75 crates.

| Phase | Status | Details |
|-------|--------|---------|
| Phase 0 — Foundation | ✅ Complete | [phase-0.md](phase-0.md) |
| Phase 1 — Core System | ✅ Complete | [phase-1.md](phase-1.md) |
| Phase 2 — Essential System Services | 🔶 In progress | [phase-2.md](phase-2.md) |
| Phase 3 — Network Stack | 🔶 Partial | [phase-3.md](phase-3.md) |
| Phase 4 — Extended Services | 🔶 Partial | [phase-4.md](phase-4.md) |
| Phase 5 — Utilities, Boot & Polish | 🔶 Partial | [phase-5.md](phase-5.md) |

Legend: ✅ = complete, 🔶 = partial, ❌ = not started

## Plan Documents

- **[status.md](status.md)** — Detailed current status with unit file directive coverage table
- **[structure.md](structure.md)** — Project structure (Cargo workspace layout, 69 crates)
- **[phase-0.md](phase-0.md)** — Phase 0: Foundation (Workspace & Core Library)
- **[phase-1.md](phase-1.md)** — Phase 1: Core System (PID 1 + systemctl + journald)
- **[phase-2.md](phase-2.md)** — Phase 2: Essential System Services
- **[phase-3.md](phase-3.md)** — Phase 3: Network Stack
- **[phase-4.md](phase-4.md)** — Phase 4: Extended Services
- **[phase-5.md](phase-5.md)** — Phase 5: Utilities, Boot & Polish
- **[integration.md](integration.md)** — Integration testing with nixos-rs