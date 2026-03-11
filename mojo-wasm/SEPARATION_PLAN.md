# Separation Plan — `mojo-wasm` → `mojo-gui` + `mojo-web`

## Executive Summary

Split the current `mojo-wasm` monolith into two projects:

1. **`mojo-gui`** — Multi-renderer reactive GUI framework
   - **`core/`** — Renderer-agnostic reactive GUI framework (Mojo library)
   - **`web/`** — Browser renderer (WASM + TypeScript)
   - **`desktop/`** — Desktop renderer (Blitz native HTML/CSS engine via Rust cdylib)
   - **`xr/`** — XR renderer (WebXR in browser, OpenXR native — future)
   - **`examples/`** — Shared example apps that run on **every** renderer target unchanged
2. **`mojo-web`** — Raw Web API bindings for Mojo/WASM (like Rust's `web-sys`) — future

The goal: write a Mojo GUI app **once**, run it in the browser via WASM **and** natively on desktop — like Dioxus does for Rust.

**Current status:**

- **Phases 1–2** — ✅ Complete. The monolith was split into `core/`, `web/`, and `examples/` within `mojo-wasm/`.
- **Phase 3** — ✅ Complete (infrastructure + unified lifecycle). The `mojo-gui/` project was created as a sibling directory with `core/src/platform/` (`GuiApp` trait, `launch()`, compile-time target dispatch), `desktop/` (Blitz renderer), and refactored shared examples using `launch[AppType]()`.
- **Phase 4** — ✅ Builds verified, runtime pending. The Blitz Rust cdylib (`libmojo_blitz.so`) compiles with full Winit event loop integration, and all 4 shared examples compile for both web and desktop from identical source. Interactive runtime verification (actually running the desktop windows) is pending GPU availability.
- **Phases 5–6** — 📋 Future work (XR renderer, `mojo-web` raw bindings).

---

## Project Layout

### `mojo-wasm/` — Original monolith (Phases 1–2 restructuring applied)

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

### `mojo-gui/` — New separated project (Phases 3–4 implemented)

```text
mojo-gui/
├── core/                         # Renderer-agnostic GUI framework
│   ├── src/
│   │   ├── signals/              # Reactive primitives (signals, memos, effects)
│   │   ├── scope/                # Scope lifecycle, arena allocator
│   │   ├── scheduler/            # Height-ordered dirty scope queue
│   │   ├── arena/                # ElementId type and allocator
│   │   ├── vdom/                 # Virtual DOM (Template, VNode, diff)
│   │   ├── html/                 # HTML tags, DSL constructors, VNodeBuilder
│   │   ├── mutations/            # CreateEngine, DiffEngine
│   │   ├── bridge/               # MutationWriter + binary opcode protocol
│   │   ├── events/               # HandlerRegistry, action tags
│   │   ├── component/            # AppShell, ComponentContext, KeyedList, Router
│   │   ├── platform/             # ★ NEW — GuiApp trait, launch(), target dispatch
│   │   │   ├── gui_app.mojo      # GuiApp trait — app-side lifecycle contract
│   │   │   ├── app.mojo          # is_wasm_target(), is_native_target()
│   │   │   ├── launch.mojo       # launch[AppType: GuiApp]() + AppConfig
│   │   │   ├── features.mojo     # PlatformFeatures, runtime feature detection
│   │   │   └── __init__.mojo     # Re-exports public API
│   │   └── lib.mojo              # Package root
│   ├── test/                     # Mojo-side unit tests (52+ suites)
│   ├── AGENTS.md
│   └── README.md
│
├── web/                          # Browser renderer (WASM + TypeScript)
│   ├── src/
│   │   ├── main.mojo             # @export WASM wrappers (one-liners via gui_app_exports)
│   │   ├── gui_app_exports.mojo  # ★ NEW — Generic @export helpers over GuiApp
│   │   └── web_launcher.mojo     # Web-side launch support
│   ├── runtime/                  # TypeScript runtime (DOM interpreter, events, templates)
│   ├── examples/                 # Browser example apps (HTML + JS shells)
│   │   ├── counter/              # index.html + main.js
│   │   ├── todo/                 # index.html + main.js
│   │   ├── bench/                # index.html + main.js
│   │   ├── app/                  # index.html + main.js
│   │   └── lib/                  # Shared JS runtime (app launcher, env, events, interpreter)
│   ├── test-js/                  # JS integration tests (3,090+ tests)
│   ├── scripts/                  # Build pipeline (nu scripts)
│   ├── justfile                  # Web build commands
│   ├── deno.json                 # Deno configuration
│   ├── default.nix               # Nix dev shell for web
│   └── README.md
│
├── desktop/                      # ★ NEW — Desktop renderer (Blitz native HTML/CSS)
│   ├── shim/                     # Rust cdylib wrapping Blitz
│   │   ├── src/lib.rs            # BlitzContext, DOM ops, Winit event loop, Vello GPU rendering
│   │   ├── mojo_blitz.h          # C API header (~45 FFI functions)
│   │   ├── Cargo.toml            # blitz-dom, blitz-html, blitz-traits, blitz-paint, winit, anyrender-vello
│   │   └── default.nix           # Nix derivation with GPU/windowing deps
│   ├── src/desktop/
│   │   ├── blitz.mojo            # Mojo FFI bindings to libmojo_blitz.so
│   │   ├── renderer.mojo         # MutationInterpreter: binary opcodes → Blitz FFI calls
│   │   ├── launcher.mojo         # desktop_launch[AppType: GuiApp]() — generic Blitz event loop
│   │   └── __init__.mojo
│   ├── README.md
│   └── .gitignore
│
├── examples/                     # ★ SHARED — Run on ALL targets from identical source
│   ├── counter/
│   │   ├── counter.mojo          # CounterApp struct (implements GuiApp)
│   │   └── main.mojo             # launch[CounterApp](AppConfig(...))
│   ├── todo/
│   │   ├── todo.mojo             # TodoApp struct (implements GuiApp)
│   │   └── main.mojo             # launch[TodoApp](AppConfig(...))
│   ├── bench/
│   │   ├── bench.mojo            # BenchmarkApp struct (implements GuiApp)
│   │   └── main.mojo             # launch[BenchmarkApp](AppConfig(...))
│   ├── app/
│   │   ├── app.mojo              # MultiViewApp struct (implements GuiApp)
│   │   └── main.mojo             # launch[MultiViewApp](AppConfig(...))
│   ├── apps/                     # Test/demo apps (batch_demo, effect_demo, etc.)
│   └── README.md
│
├── build/                        # Build output
├── justfile                      # Root task runner (web + desktop commands)
├── default.nix                   # Nix dev shell (web + desktop deps)
└── README.md
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
| Phase 1 | [Extract `core/`](docs/plan/phase1-core.md) | ✅ Complete |
| Phase 2 | [Create `web/`](docs/plan/phase2-web.md) | ✅ Complete |
| Phase 3 | [Desktop + Unified Lifecycle](docs/plan/phase3-desktop.md) | ✅ Complete (infra + lifecycle; CI pending) |
| Phase 4 | [Desktop Blitz Renderer](docs/plan/phase4-blitz.md) | ✅ Builds verified; runtime pending |
| Phase 5 | [XR Renderer](docs/plan/phase5-xr.md) | 📋 Future |
| Phase 6 | [`mojo-web` Raw Web API Bindings](docs/plan/phase6-mojo-web.md) | 📋 Future |

### Cross-Cutting

| Document | Description |
|----------|-------------|
| [Migration Checklist](docs/plan/checklist.md) | Per-phase task checklists with completion status |
| [Risks, Effort & Open Questions](docs/plan/risks.md) | Risk mitigations, estimated effort, and open design questions |

---

## What Was Done

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

### Phase 1–2 Verification

All tests pass after the separation:

- ✅ **3,090 JS tests** — `just test-js` (web/test-js/)
- ✅ **52 Mojo test suites** — `just test` (core/test/)
- ✅ **WASM build** — `just build` produces `web/build/out.wasm`

### Phase 3: Desktop + Unified Lifecycle — ✅ Complete

Phase 3 was implemented in the new `mojo-gui/` project (sibling to `mojo-wasm/`). It progressed through two stages: first a webview-based desktop renderer (GTK4 + WebKitGTK), then replaced by the Blitz native renderer (Phase 4). The unified lifecycle work is the lasting contribution.

#### Steps 3.1–3.8 — Desktop Webview Infrastructure ✅

Built a desktop renderer using GTK4 + WebKitGTK as a pragmatic first step:

- **C shim** (`libmojo_webview.so`) — polling-based API with ring buffer events, base64 mutation delivery
- **Mojo FFI bindings** — typed `Webview` struct via `OwnedDLHandle`, library search (env var → NIX_LDFLAGS → LD_LIBRARY_PATH)
- **Desktop bridge** — heap mutation buffer, flush, poll; `DesktopEvent` with minimal JSON parser
- **`DesktopApp`** — webview lifecycle, JS runtime injection, shell HTML loading, multiple event loop styles
- **Desktop JS runtime** (`desktop-runtime.js`) — standalone 900+ line JS: MutationReader, TemplateCache, Interpreter (all opcodes), event dispatch via `window.mojo_post()`
- **Counter example** — full interactive event loop with ConditionalSlot
- **Nix integration** — `justfile`, `default.nix` for GTK4/WebKitGTK dev shell

> Note: The webview infrastructure was superseded by the Blitz renderer (Phase 4), which eliminates the JS runtime, base64 IPC, and browser engine dependency.

#### Step 3.9 — Unified App Lifecycle ✅

This is the core contribution of Phase 3 — the platform abstraction that enables shared examples:

**Step 3.9.1 — `GuiApp` trait** (`core/src/platform/gui_app.mojo`):

```text
trait GuiApp(Movable):
    fn __init__(out self)
    fn render(mut self) -> UInt32
    fn handle_event(mut self, handler_id: UInt32, event_type: UInt8, value: String) -> Bool
    fn mount(mut self, writer_ptr: UnsafePointer[MutationWriter, MutExternalOrigin]) -> Int32
    fn flush(mut self, writer_ptr: UnsafePointer[MutationWriter, MutExternalOrigin]) -> Int32
    fn has_dirty(self) -> Bool
    fn consume_dirty(mut self) -> Bool
    fn destroy(mut self)
```

**Step 3.9.2 — Generic desktop event loop** (`desktop/src/desktop/launcher.mojo`):

`desktop_launch[AppType: GuiApp](config)` — creates Blitz window, mounts initial DOM, enters blocking event loop (poll events → dispatch → flush dirty → apply mutations → redraw).

**Step 3.9.3 — Compile-time target dispatch** (`core/src/platform/launch.mojo`):

```text
fn launch[AppType: GuiApp](config: AppConfig = AppConfig()) raises:
    @parameter
    if is_wasm_target():
        pass  # JS runtime drives the loop; @export wrappers call GuiApp methods
    else:
        from desktop.launcher import desktop_launch
        desktop_launch[AppType](config)
```

**Step 3.9.4 — All 4 app structs implement `GuiApp`:**

| App | Notes |
|-----|-------|
| `CounterApp` | Click events, ConditionalSlot for detail |
| `TodoApp` | String events via `dispatch_event_with_string` when `len(value) > 0` |
| `BenchmarkApp` | Toolbar routing + KeyedList row events, performance timing |
| `MultiViewApp` | Router dirty state composed into `has_dirty()`, nav + signal dispatch |

**Step 3.9.5 — Generic `@export` WASM wrappers** (`web/src/gui_app_exports.mojo`):

Parametric helpers (`gui_app_init[T]`, `gui_app_mount[T]`, `gui_app_flush[T]`, etc.) that make per-app `@export` functions one-liners. All 3,090 JS tests + 52 Mojo test suites pass.

**Step 3.9.6 — Shared examples with `launch()`:**

Each example has a `main.mojo` entry point:

```text
fn main() raises:
    launch[CounterApp](AppConfig(title="High-Five Counter", width=400, height=350))
```

Same source compiles for both targets:

- `mojo build examples/counter/main.mojo --target wasm64-wasi -I core/src -I web/src -I examples` → WASM
- `mojo build examples/counter/main.mojo -I core/src -I desktop/src -I examples` → native

### Phase 4: Desktop Blitz Renderer — ✅ Builds Verified, Runtime Pending

Replaced the webview dependency with [Blitz](https://github.com/DioxusLabs/blitz), a native HTML/CSS rendering engine using Stylo (CSS) + Taffy (layout) + Vello (GPU rendering) + Winit (windowing) + AccessKit (a11y). No JS runtime, no IPC — mutations are applied in-process via direct C FFI calls.

**Step 4.1 — Blitz C shim** (`desktop/shim/src/lib.rs`) ✅

Rust `cdylib` wrapping `blitz-dom` with ~45 FFI functions: lifecycle, DOM operations, template registry, event queue, interpreter stack, debug utilities.

**Step 4.2 — Mojo FFI bindings** (`desktop/src/desktop/blitz.mojo`) ✅

Typed `Blitz` struct via `_DLHandle` with all FFI functions wrapped as methods.

**Step 4.3 — Mojo-side mutation interpreter** (`desktop/src/desktop/renderer.mojo`) ✅

`MutationInterpreter` reads binary opcodes from the mutation buffer → Blitz C FFI calls for all 18 opcodes.

**Step 4.3.1 — Rust cdylib build** ✅

`libmojo_blitz.so` ~23MB (release, thin LTO, stripped), 607 crate dependencies, zero warnings.

**Step 4.4 — Shared example builds** ✅ (runtime pending)

All 4 shared examples compile for both web and desktop from identical source. Mojo 0.26.1 API migration completed as part of build verification. Interactive runtime verification requires `libmojo_blitz.so` + GPU.

**Step 4.6 — Winit event loop integration** ✅

Full `ApplicationHandler` impl: window creation via `Arc<Window>`, Vello GPU renderer via `anyrender_vello::VelloWindowRenderer`, Winit event routing (CloseRequested, RedrawRequested, Resized, CursorMoved, MouseInput), DOM event extraction via custom `MojoEventHandler`, style resolution + layout via `doc.resolve()`, GPU rendering via `paint_scene()`.

### Phase 4.7: Project Infrastructure — ✅ Complete

Created the missing project infrastructure for `mojo-gui/` so it functions as a standalone project:

**Step 4.7.1 — Root `justfile`** (`mojo-gui/justfile`) ✅

Task runner with commands for both renderers:

- **Web commands** — `build`, `build-if-changed`, `precompile`, `test`, `test-js`, `test-all`, `test-browser`, `serve` (delegate to `web/justfile`)
- **Desktop commands** — `build-shim` (Rust cdylib), `build-desktop <app>`, `build-desktop-all`, `run-desktop <app>`
- **Cross-target commands** — `build-web <app>`, `build-all`
- **Cleanup** — `clean` (removes `build/`, `web/build/`, cargo target)

**Step 4.7.2 — Root `default.nix`** (`mojo-gui/default.nix`) ✅

Nix dev shell combining web and desktop dependencies:

- Build tools: `just`, `mojo`
- Web renderer: `deno`, `wabt`, `llvm`, `lld`, `wasmtime`, `servo`, `jq`
- Desktop renderer build: `rustup`, `pkg-config`, `cmake`, `python3`
- Desktop renderer runtime: `fontconfig`, `freetype`, `libxkbcommon`, `wayland`, `vulkan-loader`, `vulkan-headers`, `libGL`, X11 libraries

**Step 4.7.3 — Updated `mojo-gui/README.md`** ✅

- Desktop status updated from "🔮 Future" to "✅ Builds verified"
- Added `examples/` and `platform/` to package table and architecture diagram
- Added "Unified App Lifecycle" section documenting `GuiApp` trait and `launch()`
- Added "Shared Examples" section with build instructions for both targets
- Added "Build & Run (Desktop)" quick start section
- Updated project structure tree to reflect actual Phase 4 implementation
- Added "Current Status" section with phase summary
- Updated import conventions to include `platform` package

**Step 4.7.4 — Updated `mojo-gui/desktop/README.md`** ✅

- Status updated from "🔮 Future — planned but not yet implemented" to "✅ Builds verified, runtime pending"
- Architecture diagram updated to show `MutationInterpreter` (actual) instead of `DesktopBridge` (planned)
- Added "Event loop" section documenting `desktop_launch` lifecycle
- Added "Key Files" table with all implemented files
- Added "Building" section with shim build, example build, and run instructions
- Added "Winit Event Loop Integration" section documenting `ApplicationHandler` impl
- Added "Remaining Work" section (runtime verification, cross-platform, CI)

---

## Quick Reference: Current Next Steps

The **immediate priorities** are runtime verification and cross-target CI:

### Short-term (unblocks desktop demos)

1. **Verify desktop runtime** — Run all 4 shared examples interactively on desktop-Blitz (requires `libmojo_blitz.so` build + GPU environment)
2. **Cross-target CI** — Set up CI matrix testing web + desktop-Blitz for every shared example
3. **Cross-platform testing** — Verify Blitz renderer on macOS and Windows (currently Linux-only)

### Medium-term (Phase 5–6)

4. **Phase 5: XR Renderer** — XR panel abstraction, OpenXR + Blitz shim for native, WebXR JS runtime for browser
5. **Phase 6: `mojo-web` Raw Bindings** — Extract raw Web API bindings (DOM, fetch, WebSocket, etc.) as a standalone package

See [Phase 4 Remaining Work](docs/plan/phase4-blitz.md#remaining-work), [Phase 5](docs/plan/phase5-xr.md), and [Phase 6](docs/plan/phase6-mojo-web.md) for details.