# Separation Plan — `wasm-mojo` → `mojo-gui` + `mojo-web`

## Executive Summary

Split the current `wasm-mojo` monolith into two projects:

1. **`mojo-gui`** — Multi-renderer reactive GUI framework
   - **`mojo-gui/core`** — Renderer-agnostic reactive GUI framework (Mojo library)
   - **`mojo-gui/web`** — Browser renderer (WASM + TypeScript)
   - **`mojo-gui/desktop`** — Desktop renderer ([Blitz](https://github.com/DioxusLabs/blitz) native HTML/CSS engine — **implementation complete, verification pending**; GTK4 + WebKitGTK webview — legacy)
   - **`mojo-gui/xr`** — XR renderer (WebXR in browser, OpenXR native — future)
   - **`mojo-gui/examples`** — Shared example apps that run on **every** renderer target unchanged
2. **`mojo-web`** — Raw Web API bindings for Mojo/WASM (like Rust's `web-sys`)

The goal: write a Mojo GUI app **once**, run it in the browser via WASM **and** natively on desktop — like Dioxus does for Rust.

**Current status:** Phases 1–3 complete. Phase 4 (Blitz desktop renderer) implementation complete — cross-target builds verified (all 4 shared examples compile for both web and desktop), runtime verification pending (requires `libmojo_blitz.so` + GPU). Phase 5 (XR) and Phase 6 (`mojo-web`) are future work.

---

## Plan Documents

This plan has been split into focused sub-documents for easier navigation. Read only the file relevant to your current task.

### Architecture & Design

| Document | Description |
|----------|-------------|
| [Architecture](docs/plan/architecture.md) | Design principles, module map, target project structure, abstraction boundary, platform abstraction layer, dependency graph |
| [Renderers](docs/plan/renderers.md) | Renderer strategies: Web, Desktop Webview, Desktop Blitz, XR (OpenXR + WebXR) |

### Phase Documents

| Phase | Document | Status |
|-------|----------|--------|
| Phase 1 | [Extract `mojo-gui/core`](docs/plan/phase1-core.md) | ✅ Complete |
| Phase 2 | [Create `mojo-gui/web`](docs/plan/phase2-web.md) | ✅ Complete |
| Phase 3 | [Desktop Webview + Unified Lifecycle](docs/plan/phase3-desktop.md) | ✅ Complete |
| Phase 4 | [Desktop Blitz Renderer](docs/plan/phase4-blitz.md) | 🔧 Builds verified, runtime pending |
| Phase 5 | [XR Renderer](docs/plan/phase5-xr.md) | Future |
| Phase 6 | [`mojo-web` Raw Web API Bindings](docs/plan/phase6-mojo-web.md) | Future |

### Cross-Cutting

| Document | Description |
|----------|-------------|
| [Migration Checklist](docs/plan/checklist.md) | Per-phase task checklists with completion status |
| [Risks, Effort & Open Questions](docs/plan/risks.md) | Risk mitigations, estimated effort, and open design questions |

---

## Quick Reference: Current Next Steps

Cross-target **build verification** is complete ✅ — all 4 shared examples compile for both web and desktop-Blitz from identical source, and all tests pass (3,090 JS + 52 Mojo suites).

The immediate priority is **runtime verification** (Step 4.4 runtime):

1. Build `libmojo_blitz.so` via `cargo build --release` in `desktop/shim/`
2. Run all 4 shared examples interactively on **desktop-Blitz** (requires GPU)
3. Set up cross-target CI test matrix

See [Phase 4 → Step 4.4](docs/plan/phase4-blitz.md#step-44--verify-all-shared-examples--builds--runtime-pending) for details.