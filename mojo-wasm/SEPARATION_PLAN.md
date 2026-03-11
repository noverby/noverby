# Separation Plan — `mojo-wasm` → `mojo-gui` + `mojo-web`

## Executive Summary

Split the current `mojo-wasm` monolith into two projects:

1. **`mojo-gui`** — Multi-renderer reactive GUI framework
   - **`core/`** — Renderer-agnostic reactive GUI framework (Mojo library)
   - **`web/`** — Browser renderer (WASM + TypeScript)
   - **`desktop/`** — Desktop renderer ([Blitz](https://github.com/DioxusLabs/blitz) native HTML/CSS engine — future; GTK4 + WebKitGTK webview — future)
   - **`xr/`** — XR renderer (WebXR in browser, OpenXR native — future)
   - **`examples/`** — Shared example apps that run on **every** renderer target unchanged
2. **`mojo-web`** — Raw Web API bindings for Mojo/WASM (like Rust's `web-sys`) — future

The goal: write a Mojo GUI app **once**, run it in the browser via WASM **and** natively on desktop — like Dioxus does for Rust.

**Current status:** Phases 1–2 are **implemented and verified** — the monolith has been split into `core/`, `web/`, and `examples/` with all 3,090 JS tests + 52 Mojo test suites passing. Phases 3–6 are future work.

---

## Current Project Structure

```text
mojo-wasm/
├── core/                         # Renderer-agnostic GUI framework
│   ├── src/
│   │   ├── signals/              # Reactive primitives (signals, memos, effects)
│   │   ├── scope/                # Scope lifecycle and arena allocator
│   │   ├── scheduler/            # Height-ordered dirty scope queue
│   │   ├── arena/                # ElementId type and allocator
│   │   ├── vdom/                 # Virtual DOM primitives (template, vnode, builder, registry)
│   │   ├── html/                 # HTML vocabulary — tags, DSL, DSL tests (split from vdom/)
│   │   ├── mutations/            # Mutation engines (create, diff)
│   │   ├── bridge/               # Binary mutation protocol (MutationWriter + opcodes)
│   │   ├── events/               # Event handler registry and action tags
│   │   └── component/            # Component framework (AppShell, ComponentContext, lifecycle)
│   ├── test/                     # Mojo-side unit tests (52 suites)
│   └── README.md
│
├── web/                          # Browser renderer (WASM + TypeScript)
│   ├── src/
│   │   ├── main.mojo             # @export WASM wrappers
│   │   └── apps/                 # Test/demo app modules
│   ├── runtime/                  # TypeScript runtime (DOM interpreter, events, templates)
│   ├── test-js/                  # JS integration tests (3,090 tests)
│   ├── scripts/                  # Build pipeline (nu scripts)
│   ├── justfile                  # Web build commands
│   ├── deno.json                 # Deno configuration
│   └── README.md
│
├── examples/                     # Shared example apps (run on ALL targets)
│   ├── counter/                  # Reactive counter with conditional detail
│   ├── todo/                     # Full todo app with input binding and keyed list
│   ├── bench/                    # JS Framework Benchmark implementation
│   ├── app/                      # Multi-view app with client-side routing
│   └── lib/                      # Shared JS runtime (app launcher, env, events, interpreter)
│
├── justfile                      # Root-level convenience aliases (delegates to web/)
├── default.nix                   # Nix dev shell
└── docs/plan/                    # Plan documents
```

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
| Phase 1 | [Extract `core/`](docs/plan/phase1-core.md) | ✅ Complete — implemented and verified |
| Phase 2 | [Create `web/`](docs/plan/phase2-web.md) | ✅ Complete — implemented and verified |
| Phase 3 | [Desktop Webview + Unified Lifecycle](docs/plan/phase3-desktop.md) | 📋 Planned |
| Phase 4 | [Desktop Blitz Renderer](docs/plan/phase4-blitz.md) | 📋 Planned |
| Phase 5 | [XR Renderer](docs/plan/phase5-xr.md) | Future |
| Phase 6 | [`mojo-web` Raw Web API Bindings](docs/plan/phase6-mojo-web.md) | Future |

### Cross-Cutting

| Document | Description |
|----------|-------------|
| [Migration Checklist](docs/plan/checklist.md) | Per-phase task checklists with completion status |
| [Risks, Effort & Open Questions](docs/plan/risks.md) | Risk mitigations, estimated effort, and open design questions |

---

## What Was Done (Phases 1–2)

### Phase 1: Extract `core/` — ✅ Complete

Moved all renderer-agnostic modules from the monolith `src/` into `core/src/`:

- **Copied unchanged:** `signals/`, `scope/`, `scheduler/`, `arena/`, `mutations/`, `bridge/`, `events/`, `component/`
- **Split `vdom/` into `vdom/` + `html/`:**
  - `vdom/` retains: `template.mojo`, `vnode.mojo`, `builder.mojo`, `registry.mojo` (renderer-agnostic primitives)
  - `html/` receives: `tags.mojo`, `dsl.mojo`, `dsl_tests.mojo` (HTML vocabulary and DSL helpers)
- **Updated imports across all files:**
  - `html/dsl.mojo`: `from .builder` → `from vdom.builder`, `from .template` → `from vdom.template`, `from .vnode` → `from vdom.vnode`
  - `html/dsl_tests.mojo`: same pattern for cross-package references
  - `vdom/template.mojo`, `vdom/builder.mojo`: `from .tags` → `from html.tags` (TAG_UNKNOWN)
  - `component/context.mojo`, `component/child.mojo`: split `from vdom import` into `from vdom import` (VNode, VNodeStore) + `from html import` (Node, DSL types, VNodeBuilder)
  - `component/child_context.mojo`, `component/keyed_list.mojo`: `VNodeBuilder` import moved from `vdom` to `html`
- **Moved tests:** `test/` → `core/test/` (52 test suites)
  - Updated `test_handles.mojo`: `from vdom import` → `from html import` for DSL symbols

### Phase 2: Create `web/` — ✅ Complete

Moved all browser/WASM-specific files into `web/`:

- **Moved:** `src/main.mojo` → `web/src/main.mojo` (updated imports: split `from vdom` into `from vdom` + `from html`, changed `vdom.dsl_tests` → `html.dsl_tests`)
- **Moved:** `src/apps/` → `web/src/apps/` (updated all 14 test app files: `from vdom import` → `from html import`)
- **Moved:** `runtime/` → `web/runtime/`
- **Moved:** `test-js/` → `web/test-js/`
- **Moved:** `scripts/` → `web/scripts/` (updated `build-test-binaries.nu` paths for new core/test, core/src, examples locations)
- **Moved:** `deno.json` → `web/deno.json`
- **Created:** `web/justfile` with updated build flags: `-I ../core/src -I ../examples -I src`
- **Updated example `main.js` files:** WASM path changed from `../../build/out.wasm` → `../../web/build/out.wasm`
- **Updated root `justfile`:** delegates all commands to `web/justfile`
- **Deleted old directories:** `src/`, `runtime/`, `test/`, `test-js/`, `scripts/`, `build/`, `deno.json`, `deno.lock`

### Verification

All tests pass after the separation:

- ✅ **3,090 JS tests** — `just test-js` (web/test-js/)
- ✅ **52 Mojo test suites** — `just test` (core/test/)
- ✅ **WASM build** — `just build` produces `web/build/out.wasm`

---

## Quick Reference: Current Next Steps

The **immediate priority** is Phase 3: Desktop renderer + unified lifecycle.

Key tasks:

1. Define `GuiApp` trait in `core/src/platform/gui_app.mojo` — app-side lifecycle contract
2. Define `PlatformApp` trait in `core/src/platform/app.mojo` — renderer-side contract
3. Implement `launch[AppType: GuiApp]()` with compile-time target dispatch
4. Create `desktop/` directory with webview or Blitz renderer
5. Refactor shared examples to use `launch()` instead of free functions
6. Verify all 4 examples run on both web and desktop from identical source

See [Phase 3](docs/plan/phase3-desktop.md) and [Architecture](docs/plan/architecture.md) for details.