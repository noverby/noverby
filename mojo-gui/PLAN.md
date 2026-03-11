# mojo-gui — Project Plan

Multi-renderer reactive GUI framework for Mojo. Write a GUI app **once**, run it in the browser via WASM **and** natively on desktop — like Dioxus does for Rust.

---

## Status Dashboard

| Target | Renderer | Status | Platform |
|--------|----------|--------|----------|
| Web (WASM) | TypeScript DOM interpreter | ✅ Complete | All browsers |
| Desktop | Blitz (Stylo + Vello + Winit) | ✅ Complete | Linux Wayland |
| Desktop | Blitz | 🔲 Untested | macOS, Windows, X11 |
| XR Native | OpenXR + Blitz offscreen | 📋 Future (Phase 5) | — |
| XR Browser | WebXR + JS interpreter | 📋 Future (Phase 5) | — |

| Area | Metric |
|------|--------|
| Core Mojo test suites | 52 |
| JS integration test suites | 29 (~3,090 tests) |
| Shared example apps | 4 (Counter, Todo, Benchmark, MultiView) |
| Test/demo app modules | 15 (in `examples/apps/`) |
| Binary mutation opcodes | 18 |
| Blitz C FFI functions | ~37 |

---

## Project Structure

```text
mojo-gui/
├── core/           — Renderer-agnostic reactive GUI framework (Mojo library)
│   ├── src/        — signals/, scope/, scheduler/, arena/, vdom/, mutations/,
│   │                 bridge/, events/, component/, html/, platform/
│   └── test/       — 52 Mojo test suites (run via wasmtime)
├── web/            — Browser renderer (WASM + TypeScript)
│   ├── src/        — @export WASM wrappers, gui_app_exports, web_launcher
│   ├── runtime/    — TypeScript: DOM interpreter, events, templates, protocol
│   ├── examples/   — HTML + JS shells for browser examples
│   ├── test-js/    — 29 JS integration test suites
│   └── scripts/    — Build pipeline (nu scripts)
├── desktop/        — Desktop renderer (Blitz native HTML/CSS engine via Rust cdylib)
│   ├── shim/       — Rust cdylib: BlitzContext, DOM ops, Winit event loop, Vello GPU
│   └── src/        — Mojo FFI bindings, MutationInterpreter, desktop_launch
├── examples/       — Shared example apps (run on every renderer target unchanged)
│   ├── counter/    — Reactive counter with conditional detail
│   ├── todo/       — Full todo app with input binding and keyed list
│   ├── bench/      — JS Framework Benchmark implementation
│   ├── app/        — Multi-view app with client-side routing
│   └── apps/       — 15 test/demo app modules (batch, effects, memos, errors, etc.)
├── docs/plan/      — Detailed plan documents
├── build/          — Build output (gitignored)
├── justfile        — Root task runner (web + desktop commands)
├── default.nix     — Nix dev shell (web + desktop + Wayland deps)
├── CHANGELOG.md    — Full development history (Phases 0–41 + separation)
└── README.md       — Project overview and quick start
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

## Testing & Build Infrastructure

### Mojo Tests (52 suites)

Run via wasmtime on WASM-compiled test binaries. Covers: signals, memos, effects, batching, scopes, scheduling, VNode diffing, mutation protocol, templates, DSL, components, conditional rendering, error boundaries, suspense, routing, and all test/demo apps.

```text
just test                    # Build + run all Mojo test suites
just test signals            # Build + run only test_signals
just test signals mutations  # Build + run matching suites
```

### JS Integration Tests (29 suites, ~3,090 tests)

Full end-to-end tests that load the WASM binary, instantiate apps, simulate events, and verify DOM mutations via the TypeScript runtime. Covers every shared example and test/demo app.

```text
just test-js                 # Run all JS integration tests (Deno)
```

### Browser End-to-End Tests

Headless browser tests via Servo + WebDriver. Verifies that examples render correctly in a real browser.

```text
just test-browser            # Run all browser tests (headless Servo)
just test-browser-app counter  # Single app
```

### Build Commands

```text
# ── Web ──────────────────────────────────────────────────────
just build                   # Build WASM binary (web/justfile)
just serve                   # Serve examples at localhost:4507
just build-web counter       # Build single example for web

# ── Desktop ──────────────────────────────────────────────────
just build-shim              # Build Blitz cdylib (first time / shim changes)
just build-desktop counter   # Build single example for desktop
just build-desktop-all       # Build all 4 examples for desktop
just run-desktop counter     # Build + run a desktop example (Wayland)

# ── Cross-target ─────────────────────────────────────────────
just build-all               # Build web + all desktop examples
just test-all                # Run Mojo + JS test suites
just clean                   # Remove all build artifacts
```

### Import Conventions

Apps and core modules use `-I` flag paths. The build target determines which renderer is linked:

```text
# Web (WASM):
mojo build examples/counter/main.mojo --target wasm64-wasi -I core/src -I web/src -I examples

# Desktop (native):
mojo build examples/counter/main.mojo -I core/src -I desktop/src -I examples
```

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
- Moved tests to `core/test/` (52 test suites)

### Separation Phase 2: Create `web/` — ✅ Complete

Moved all browser/WASM-specific files into `web/`:

- `web/src/main.mojo` — @export WASM wrappers
- `web/src/apps/` → later moved to `examples/apps/` — 15 test/demo app modules
- `web/runtime/` — TypeScript runtime (DOM interpreter, events, templates) — 11 modules
- `web/test-js/` — JS integration tests (29 suites, ~3,090 tests)
- `web/scripts/` — Build pipeline (nu scripts)

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

All 4 shared example apps (Counter, Todo, Benchmark, MultiView) implement `GuiApp` and compile for both targets from identical source. Each has a single `main.mojo`:

```text
from platform.launch import launch, AppConfig
from counter import CounterApp

fn main() raises:
    launch[CounterApp](AppConfig(title="High-Five Counter", width=400, height=350))
```

### Separation Phase 4: Desktop Blitz Renderer — ✅ Complete

Replaced the initial webview dependency with [Blitz](https://github.com/DioxusLabs/blitz), a native HTML/CSS rendering engine using Stylo (CSS) + Taffy (layout) + Vello (GPU rendering) + Winit (windowing, Wayland-only) + AccessKit (a11y). No JS runtime, no IPC — mutations applied in-process via direct C FFI calls.

- **Blitz C shim** (`desktop/shim/src/lib.rs`) — Rust cdylib with ~37 FFI functions (lifecycle, DOM ops, templates, events, stack ops, ID mapping, debug)
- **C header** (`desktop/shim/mojo_blitz.h`) — flat C ABI with opaque `MblitzContext*`, event struct, event type constants
- **Mojo FFI bindings** (`desktop/src/desktop/blitz.mojo`) — typed `Blitz` struct wrapping the C shim
- **Mutation interpreter** (`desktop/src/desktop/renderer.mojo`) — binary opcodes → Blitz FFI calls (Mojo equivalent of the JS `Interpreter`)
- **Generic desktop launcher** (`desktop/src/desktop/launcher.mojo`) — `desktop_launch[AppType: GuiApp](config)` with Winit event loop integration, blocking/non-blocking step, event polling, dirty-scope flush cycle
- **Nix derivation** (`desktop/shim/default.nix`) — automates Rust build with Wayland + GPU deps
- **User-agent CSS** — default stylesheet for consistent rendering of HTML elements

Runtime verified — all 4 desktop windows launch and render on Wayland with Vello GPU rendering.

### Phase 4.7: Project Infrastructure — ✅ Complete

- Root `justfile` with web + desktop commands (build, test, serve, clean)
- Root `default.nix` combining web and desktop dependencies (Mojo, Deno, LLVM, wabt, wasmtime, Rust, Wayland/Vulkan libs)
- READMs updated across all sub-projects

---

## Roadmap

### Immediate — Stabilization & Verification

These items close out remaining gaps from the completed phases before starting new feature work.

| # | Task | Effort | Notes |
|---|------|--------|-------|
| I-1 | **Cross-target build verification** | 1–2 days | Verify all 4 shared examples build and run correctly on both web and desktop. Catch any regressions from the separation. This was Step 3.9.7 — the last open item from Phase 3.9. |
| I-2 | **Desktop integration tests** | 3–5 days | The desktop renderer has no automated tests yet. Add a test harness that mounts each shared example via `desktop_launch`, simulates events through the Blitz event system, and verifies the resulting DOM state via `mblitz_print_tree()` / node inspection FFI. |
| I-3 | **Mutation protocol conformance tests** | 2–3 days | Add tests that verify the Mojo `MutationInterpreter` and JS `Interpreter` produce identical DOM trees given the same opcode sequence. Use the existing test/demo apps as fixtures. |
| I-4 | **Document CSS support scope** | 1 day | Audit which CSS features work in Blitz v0.2.0 (pinned at rev `2f83df96`). Document supported/unsupported features. Add CSS stress-test examples if gaps are found. |

### Short-term — Cross-Platform & CI

| # | Task | Effort | Notes |
|---|------|--------|-------|
| S-1 | **Cross-target CI pipeline** | 3–5 days | GitHub Actions (or similar) matrix: `{web, desktop-linux}` × `{counter, todo, bench, app}`. Run `just test-all` for web. Build + smoke-test for desktop (headless Wayland via `wlheadless` or `weston --headless`). Gate PRs on green matrix. |
| S-2 | **macOS desktop verification** | 2–3 days | Blitz uses Winit which supports macOS. Build the Blitz shim on macOS, verify `cargo build --release` succeeds, run the Counter example. Document any platform-specific quirks (GPU backend selection, font fallback, etc.). |
| S-3 | **Windows desktop verification** | 2–3 days | Same as S-2 for Windows. Winit supports Windows natively. Vulkan or DX12 backend via wgpu/vello. |

### Medium-term — Phase 5: XR Renderer

> **Full plan:** [docs/plan/phase5-xr.md](docs/plan/phase5-xr.md) — **Effort estimate: 4–8 weeks**

XR panel abstraction that reuses the binary mutation protocol unchanged. Each XR panel gets its own `GuiApp` instance and mutation buffer.

| Step | Description |
|------|-------------|
| 5.1 | Design the XR panel abstraction (`XRPanel` struct, scene graph, placement) |
| 5.2 | Build the OpenXR + Blitz Rust shim (offscreen Vello rendering → OpenXR swapchain textures) |
| 5.3 | Mojo FFI bindings for the OpenXR shim |
| 5.4 | XR scene manager and panel routing (multiplexes mutation buffers) |
| 5.5 | `xr_launch[AppType: GuiApp]()` — single-panel apps get XR for free |
| 5.6 | WebXR JS runtime (DOM → texture, XR session management) |
| 5.7 | Wire `launch()` for XR targets (`@parameter if has_feature("xr")`) |
| 5.8 | Verify shared examples in XR (all 4 apps render as floating panels) |
| 5.9 | Multi-panel XR API — `XRGuiApp` trait for apps managing multiple panels (stretch goal) |

**Compile targets after Phase 5:**

```text
mojo build --target wasm64-wasi             → web
mojo build --target wasm64-wasi --feature webxr  → WebXR browser
mojo build                                  → desktop (Blitz)
mojo build --feature xr                     → OpenXR native
```

### Medium-term — Phase 6: `mojo-web` Raw Bindings

> **Full plan:** [docs/plan/phase6-mojo-web.md](docs/plan/phase6-mojo-web.md) — **Effort estimate: 2–3 weeks**

Standalone Mojo library providing typed bindings to Web APIs — the Mojo equivalent of Rust's `web-sys`. Independent of `mojo-gui`; usable by any Mojo/WASM project.

| Module | Web APIs |
|--------|----------|
| `dom` | Document, Element, Node, Text, Event |
| `fetch` | fetch, Request, Response, Headers |
| `timers` | setTimeout, setInterval, requestAnimationFrame |
| `storage` | localStorage, sessionStorage |
| `console` | console.log, warn, error |
| `url` | URL, URLSearchParams |
| `websocket` | WebSocket |
| `canvas` | Canvas2D (WebGL future) |

Architecture: JS-side handle table (integer IDs → JS objects) + WASM imports + Mojo typed wrappers. Same proven pattern as the existing `mojo-wasm` JS interop.

**Relationship to `mojo-gui`:** `mojo-gui` uses the mutation protocol for rendering, NOT `mojo-web`. Apps can optionally import `mojo-web` for non-rendering web features (fetch for suspense, localStorage, WebSocket) behind `@parameter if is_wasm_target()` gates.

### Long-term

| # | Task | Notes |
|---|------|-------|
| L-1 | **Animation framework** | Declarative transitions and spring animations integrated with the reactive system. Mutations include interpolated style updates. |
| L-2 | **Hot reload (web)** | Watch mode that recompiles WASM on source change and hot-swaps into the running page. Build on the existing `just serve` + file watcher. |
| L-3 | **Component library** | Reusable UI components (Modal, Dropdown, Tabs, Toast, etc.) built on the HTML DSL. Ships as a Mojo package alongside `core`. |
| L-4 | **Devtools** | Reactive graph inspector — visualize signals, memos, effects, and their dependencies. Desktop: overlay via Blitz. Web: browser extension or side panel. |
| L-5 | **Server-side rendering** | Render the initial VNode tree to static HTML on the server, hydrate on the client. Requires serializing the VNode store. |
| L-6 | **Mobile targets** | Investigate Winit's Android/iOS support for Blitz-based mobile rendering. |

---

## Key Architecture Decisions

These decisions are settled and documented here for reference. See [architecture.md](docs/plan/architecture.md) for full rationale.

1. **Binary mutation protocol as the renderer contract.** Core never touches platform APIs. It writes opcodes to a byte buffer; each renderer interprets them. This enables multi-renderer support from identical app source.

2. **DOM-oriented model in core.** HTML/CSS is the universal UI description language across all targets: real DOM (web), Blitz DOM (desktop), Blitz DOM per-panel (XR native), real DOM per-panel (WebXR). The HTML vocabulary (`html/`) stays in core.

3. **`GuiApp` trait + `launch()` for platform abstraction.** Apps implement `GuiApp`; `launch[AppType]()` dispatches to the right renderer at compile time via `@parameter if is_wasm_target()`. No per-renderer app code.

4. **Shared examples as the correctness gate.** If an example doesn't work on a target, it's a framework bug — not an app authoring problem. Examples live in `examples/`, never per-renderer.

5. **Mono-repo with path-based imports.** `mojo-gui/` is the workspace root. `-I core/src -I examples` for cross-package imports. Works around Mojo's current package system limitations.

6. **Blitz over webview for desktop.** No JS runtime, no IPC, no base64 encoding. Direct in-process C FFI. Cross-platform via Winit. Consistent CSS via Stylo. Pinned to Blitz v0.2.0 (rev `2f83df96`).

---

## Open Risks

| Risk | Impact | Mitigation | Status |
|------|--------|------------|--------|
| Platform abstraction too leaky | Shared examples break on some targets | Cross-target CI matrix as gate; treat failures as framework bugs | In progress — `GuiApp` + `launch()` complete; CI pending (S-1) |
| Blitz pre-alpha stability | Rendering bugs, missing CSS | Track Blitz releases; pin versions; document CSS support scope | Mitigated — pinned to v0.2.0 |
| Native target module-level `var` | Global `var` not supported on native | Avoided in current design; config passed as arguments, not globals | ✅ Resolved |
| Mojo trait limitations | `GuiApp` may not support future needs | `alias CurrentApp = ...` as fallback; upgrade when Mojo improves | ✅ Resolved for current scope |
| OpenXR runtime availability (Phase 5) | XR fails without runtime | Detect at startup; fall back to desktop Blitz | Future |
| DOM-to-texture fidelity for WebXR (Phase 5) | Rendering quality/interactivity loss | Evaluate OffscreenCanvas, html2canvas, CSS 3D, custom 2D renderer | Future |

See [docs/plan/risks.md](docs/plan/risks.md) for the full risk register with 19 entries.