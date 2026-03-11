# mojo-gui — Project Plan

Multi-renderer reactive GUI framework for Mojo. Write a GUI app **once**, run it in the browser via WASM **and** natively on desktop — like Dioxus does for Rust.

## Project Structure

```text
mojo-gui/
├── core/       — Renderer-agnostic reactive GUI framework (Mojo library)
├── web/        — Browser renderer (WASM + TypeScript)
├── desktop/    — Desktop renderer (Blitz native HTML/CSS engine via Rust cdylib)
├── examples/   — Shared example apps (run on every renderer target unchanged)
├── build/      — Build output
├── docs/plan/  — Detailed plan documents
└── CHANGELOG.md — Full development history (Phases 0–41 + separation)
```

---

## Plan Documents

### Architecture & Design

| Document | Description |
|----------|-------------|
| [Architecture](docs/plan/architecture.md) | Design principles, module map, project structure, platform abstraction layer, dependency graph |
| [Renderers](docs/plan/renderers.md) | Renderer strategies: Web, Desktop Blitz, XR (OpenXR + WebXR) |

### Future Phases

| Phase | Document | Status |
|-------|----------|--------|
| Phase 5 | [XR Renderer](docs/plan/phase5-xr.md) | 📋 Future |
| Phase 6 | [`mojo-web` Raw Web API Bindings](docs/plan/phase6-mojo-web.md) | 📋 Future |

### Cross-Cutting

| Document | Description |
|----------|-------------|
| [Risks, Effort & Open Questions](docs/plan/risks.md) | Risk mitigations, estimated effort, and open design questions |

---

## What Was Done

### Pre-separation: Reactive Framework (Phases 0–41) — ✅ Complete

The core reactive framework was built incrementally over 42 phases in the original `mojo-wasm` monolith. See [CHANGELOG.md](CHANGELOG.md) for the full history. Key capabilities:

- **Signals & reactivity** — `SignalI32`, `SignalBool`, `SignalString` with automatic subscriber tracking
- **Derived signals (Memos)** — `MemoI32`, `MemoBool`, `MemoString` with equality-gated propagation and recursive worklist-based dirtying
- **Effects** — Reactive side effects with drain-and-run flush pattern
- **Batch signal writes** — `begin_batch()` / `end_batch()` for grouped writes with single propagation pass
- **Virtual DOM** — Templates, VNodes, DSL builders, create/diff engines
- **Binary mutation protocol** — 18 opcodes serialized to a shared buffer, interpreted by each renderer
- **Component framework** — `AppShell`, `ComponentContext`, `ChildComponentContext`, `KeyedList`, `ConditionalSlot`, `Router`
- **Error boundaries & suspense** — Nested error boundaries with crash/retry, suspense boundaries with pending/resolve lifecycle
- **HTML DSL** — `el_div(el_h1(dyn_text()), el_button(text("Up!"), onclick_add(count, 1)))` — Dioxus `rsx!`-style nesting
- **Two-way input binding** — `bind_value(signal)` + `oninput_set_string(signal)` for Dioxus-style controlled inputs
- **Client-side routing** — `Router` struct with URL path matching, `ConditionalSlot` view switching, JS history integration

### Separation Phase 1: Extract `core/` — ✅ Complete

Moved all renderer-agnostic modules from the monolith into `core/src/`:

- `signals/`, `scope/`, `scheduler/`, `arena/`, `mutations/`, `bridge/`, `events/`, `component/`
- Split `vdom/` into `vdom/` (renderer-agnostic primitives) + `html/` (HTML vocabulary and DSL helpers)
- Moved tests to `core/test/` (52 test suites, 1,323 tests)

### Separation Phase 2: Create `web/` — ✅ Complete

Moved all browser/WASM-specific files into `web/`:

- `web/src/main.mojo` — @export WASM wrappers
- `web/src/apps/` — 15 test/demo app modules
- `web/runtime/` — TypeScript runtime (DOM interpreter, events, templates)
- `web/test-js/` — JS integration tests (3,090 tests, 29 suites)
- `web/scripts/` — Build pipeline

### Separation Phase 3: Desktop + Unified Lifecycle — ✅ Complete

**`GuiApp` trait** (`core/src/platform/gui_app.mojo`) — the app-side lifecycle contract:

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

**Compile-time target dispatch** (`core/src/platform/launch.mojo`):

```text
fn launch[AppType: GuiApp](config: AppConfig = AppConfig()) raises:
    @parameter
    if is_wasm_target():
        pass  # JS runtime drives the loop; @export wrappers call GuiApp methods
    else:
        from desktop.launcher import desktop_launch
        desktop_launch[AppType](config)
```

All 4 shared example apps (Counter, Todo, Benchmark, MultiView) implement `GuiApp` and compile for both targets from identical source.

### Separation Phase 4: Desktop Blitz Renderer — ✅ Complete

Replaced the initial webview dependency with [Blitz](https://github.com/DioxusLabs/blitz), a native HTML/CSS rendering engine using Stylo (CSS) + Taffy (layout) + Vello (GPU rendering) + Winit (windowing, Wayland-only) + AccessKit (a11y). No JS runtime, no IPC — mutations applied in-process via direct C FFI calls.

- **Blitz C shim** (`desktop/shim/src/lib.rs`) — Rust cdylib with ~45 FFI functions
- **Mojo FFI bindings** (`desktop/src/desktop/blitz.mojo`) — typed `Blitz` struct
- **Mutation interpreter** (`desktop/src/desktop/renderer.mojo`) — binary opcodes → Blitz FFI calls
- **Generic desktop launcher** (`desktop/src/desktop/launcher.mojo`) — Winit event loop integration

Runtime verified — all 4 desktop windows launch and render on Wayland with Vello GPU rendering.

### Separation Phase 4.7: Project Infrastructure — ✅ Complete

- Root `justfile` with web + desktop commands
- Root `default.nix` combining web and desktop dependencies
- READMEs updated across all sub-projects

---

## Current Next Steps

### Short-term

1. **Phase 5: [XR Renderer](docs/plan/phase5-xr.md)** — XR panel abstraction, OpenXR + Blitz shim for native, WebXR JS runtime for browser
2. **Phase 6: [`mojo-web` Raw Bindings](docs/plan/phase6-mojo-web.md)** — Extract raw Web API bindings (DOM, fetch, WebSocket, etc.) as a standalone package

### Medium-term

3. **Cross-target CI** — Set up CI matrix testing web + desktop-Blitz for every shared example
4. **macOS support** — Verify Blitz renderer on macOS (currently Linux Wayland-only; X11 not supported)