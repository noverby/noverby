# mojo-gui — Project Plan

Multi-renderer reactive GUI framework for Mojo. Write a GUI app **once**, run it in the browser via WASM **and** natively on desktop — like Dioxus does for Rust.

---

## Status Dashboard

| Target | Renderer | Status | Platform |
|--------|----------|--------|----------|
| Web (WASM) | TypeScript DOM interpreter | ✅ Complete | All browsers |
| Desktop | Blitz (Stylo + Vello + Winit) | ✅ Complete | Linux Wayland |
| Desktop | Blitz | 🔲 Untested | macOS |
| Desktop | Blitz (Wine) | ✅ Verified | Windows (via Wine) |
| XR Native | OpenXR + Blitz offscreen | 🔧 In progress (Steps 5.1–5.3 ✅) | Linux (headless tests pass) |
| XR Browser | WebXR + JS interpreter | 📋 Future (Phase 5) | — |

| Area | Metric |
|------|--------|
| Core Mojo test suites | 52 |
| JS integration test suites | 30 (~3,375 tests) |
| Desktop integration test suites | 1 (75 tests, verified on Linux + Wine) |
| XR shim integration tests | 30 (headless — real Blitz documents, no XR runtime or GPU needed) |
| Shared example apps | 4 (Counter, Todo, Benchmark, MultiView) |
| Test/demo app modules | 15 (in `examples/apps/`) |
| Binary mutation opcodes | 18 |
| Desktop Blitz C FFI functions | ~45 |
| XR C FFI functions | ~80 |
| XR Mojo FFI wrapper methods | ~70 (XRBlitz struct) |

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
├── xr/             — XR renderer (OpenXR native + WebXR browser, Phase 5)
│   ├── native/     — OpenXR native: Blitz DOM → Vello → offscreen textures → OpenXR
│   │   ├── shim/   — Rust cdylib: multi-panel Blitz + headless DOM + raycasting
│   │   └── src/    — Mojo: XRPanel, XRScene, XRBlitz FFI, XRMutationInterpreter
│   └── web/        — WebXR browser renderer (future)
├── docs/plan/      — Detailed plan documents
├── build/          — Build output (gitignored)
├── justfile        — Root task runner (web + desktop + xr commands)
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
| [CSS Support Scope](docs/plan/css-support.md) | Blitz v0.2.0 CSS feature audit — supported, partial, and unsupported features with app authoring recommendations |
| [XR README](xr/README.md) | XR renderer architecture, key types, build instructions, design decisions, per-step roadmap |

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

### JS Integration Tests (30 suites, ~3,375 tests)

Full end-to-end tests that load the WASM binary, instantiate apps, simulate events, and verify DOM mutations via the TypeScript runtime. Covers every shared example, test/demo app, and mutation protocol conformance.

```text
just test-js                 # Run all JS integration tests (Deno)
```

### Browser End-to-End Tests

Headless browser tests via Servo + WebDriver. Verifies that examples render correctly in a real browser.

```text
just test-browser            # Run all browser tests (headless Servo)
just test-browser-app counter  # Single app
```

### XR Shim Integration Tests (30 tests)

Rust integration tests for the XR Blitz shim. Each panel owns a real Blitz `BaseDocument` with Stylo CSS styling and Taffy layout. Tests run in headless mode — no XR runtime or GPU needed. Covers: session lifecycle, panel lifecycle, DOM operations (create/append/insert/replace/remove), attributes, text nodes, placeholders, serialization, events, raycasting, focus, frame loop, reference spaces, ID mapping, stack operations, multi-panel isolation, Blitz document structure, nested elements with attributes, and layout resolution.

```text
just test-xr                 # Run all XR shim integration tests (headless)
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
just test-desktop            # Run Blitz shim integration tests (headless)

# ── XR ───────────────────────────────────────────────────────
just test-xr                 # Run XR shim integration tests (headless)

# ── Cross-target ─────────────────────────────────────────────
just build-all               # Build web + all desktop examples
just test-all                # Run Mojo + JS test suites
just test-all-targets        # Run Mojo + JS + desktop + XR test suites
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

### Phase 5.3: Mojo FFI Bindings for OpenXR Shim — ✅ Complete

Implemented typed Mojo FFI bindings for all ~80 XR shim C functions, plus a per-panel mutation interpreter that translates binary opcodes into XR Blitz FFI calls. Follows the same architecture as the desktop renderer (`blitz.mojo` + `renderer.mojo`).

**New files:**

- **`xr/native/src/xr/xr_blitz.mojo`** — `XRBlitz` struct wrapping all `mxr_*` C functions via `DLHandle`. ~70 typed methods covering:
  - **Session lifecycle** — `create_session()`, `create_headless()`, `session_state()`, `is_alive()`, `destroy()`
  - **Panel lifecycle** — `create_panel()`, `destroy_panel()`, `panel_count()`
  - **Panel transform & display** — `panel_set_transform()`, `panel_set_size()`, `panel_set_visible()`, `panel_is_visible()`, `panel_set_curved()`
  - **Mutation batching** — `panel_begin_mutations()`, `panel_end_mutations()`, `panel_apply_mutations()` (Rust-side interpreter)
  - **Per-panel DOM operations** — `panel_create_element()`, `panel_create_text_node()`, `panel_create_placeholder()`, `panel_set_attribute()`, `panel_remove_attribute()`, `panel_set_text_content()`, `panel_append_children()`, `panel_insert_before()`, `panel_insert_after()`, `panel_replace_with()`, `panel_remove_node()`
  - **Templates** — `panel_register_template()`, `panel_clone_template()`
  - **Tree traversal** — `panel_node_at_path()`, `panel_child_at()`, `panel_child_count()`
  - **Events** — `panel_add_event_listener()`, `panel_add_event_listener_by_name()`, `panel_remove_event_listener()`, `poll_event()`, `event_count()`, `event_clear()`, `panel_inject_event()`
  - **Raycasting** — `raycast_panels()`, `set_focused_panel()`, `get_focused_panel()`
  - **Frame loop** — `wait_frame()`, `begin_frame()`, `render_dirty_panels()`, `end_frame()`
  - **Input** — `get_pose()`, `get_aim_ray()` (output-pointer pattern)
  - **Reference spaces** — `set_reference_space()`, `get_reference_space()`
  - **Capabilities** — `has_extension()`, `has_hand_tracking()`, `has_passthrough()`
  - **ID mapping & stack** — `panel_assign_id()`, `panel_resolve_id()`, `panel_stack_push()`, `panel_stack_pop()`
  - **Debug/inspection** — `panel_print_tree()`, `panel_serialize_subtree()`, `panel_get_node_tag()`, `panel_get_text_content()`, `panel_get_attribute_value()`, `panel_get_child_mojo_id()`, `version()`
  - **Helper types** — `XREvent` (with panel targeting + UV hit coords + hand), `XRPose`, `XRRaycastHit`
  - **Constants** — All `EVT_*`, `HAND_*`, `SPACE_*`, `STATE_*` constants mirroring `mojo_xr.h`
  - **Library search** — `MOJO_XR_LIB` env var → `NIX_LDFLAGS` → `LD_LIBRARY_PATH` → fallback

- **`xr/native/src/xr/renderer.mojo`** — `XRMutationInterpreter` struct. Per-panel opcode interpreter that reads the same binary mutation buffer as the desktop interpreter but targets `XRBlitz` FFI calls scoped to a `panel_id`. Handles all 18 opcodes: `END`, `APPEND_CHILDREN`, `ASSIGN_ID`, `CREATE_PLACEHOLDER`, `CREATE_TEXT_NODE`, `LOAD_TEMPLATE`, `REPLACE_WITH`, `REPLACE_PLACEHOLDER`, `INSERT_AFTER`, `INSERT_BEFORE`, `SET_ATTRIBUTE`, `SET_TEXT`, `NEW_EVENT_LISTENER`, `REMOVE_EVENT_LISTENER`, `REMOVE`, `PUSH_ROOT`, `REGISTER_TEMPLATE`, `REMOVE_ATTRIBUTE`. Includes `BufReader` for little-endian buffer decoding (same as desktop).

- **`xr/native/src/xr/__init__.mojo`** — Updated with re-exports for `XRBlitz`, `XRMutationInterpreter`, `XRPose`, `XRRaycastHit`, and all constants.

**Known limitations** (deferred to Step 5.5 — XR launcher):

- `poll_event()` — Returns empty event; needs `mxr_poll_event_into()` shim function (DLHandle can't return large C structs reliably). `panel_inject_event()` + `event_count()` work for testing.
- `raycast_panels()` — Returns miss; needs `mxr_raycast_panels_into()`. Same DLHandle struct-return limitation.
- `get_pose()` — Returns invalid pose; needs `mxr_get_pose_into()`. `get_aim_ray()` works (uses output pointers).
- Template registration via Mojo-side interpreter — Templates built as live DOM nodes can't be registered for clone-based instantiation without `mxr_panel_register_template_by_node()`. Works fine when using Rust-side interpreter (`panel_apply_mutations`).

---

### Phase 5.2: Real Blitz Documents in XR Shim — ✅ Complete

Replaced the lightweight `HeadlessNode` DOM tree in the XR shim with real Blitz `BaseDocument` instances — the same CSS engine used by the desktop renderer. Each XR panel now owns a full Blitz document with Stylo styling and Taffy layout.

**Key changes** (`xr/native/shim/src/lib.rs`):

- **Panel now owns a `BaseDocument`** — replaced `nodes: HashMap<u32, HeadlessNode>` with `doc: BaseDocument` plus `id_to_node`/`node_to_id` maps (same pattern as desktop shim). Mount point is `<body>` (was `<div>`).
- **All DOM operations delegate to Blitz** — `create_element` → `doc.mutate().create_element(QualName)`, `set_attribute` → `doc.mutate().set_attribute()`, etc. No more manual parent/child tracking.
- **Template cloning via `deep_clone_node`** — templates are stored as detached Blitz subtrees, deep-cloned on use (same as desktop shim).
- **DOM inspection uses Blitz node API** — `get_node_tag`, `get_text_content`, `get_attribute_value`, `serialize_subtree` all use `doc.get_node()` and `NodeData` matching.
- **Layout resolution in render loop** — `mxr_render_dirty_panels` calls `panel.doc.resolve(0.0)` to exercise Stylo + Taffy (future: Vello offscreen rendering).
- **New FFI functions** — `mxr_panel_assign_id`, `mxr_panel_resolve_id`, `mxr_panel_stack_push`, `mxr_panel_stack_pop` (mutation interpreter support).
- **6 new tests** (30 total, up from 24) — `id_mapping_assign_and_resolve`, `stack_push_and_pop`, `multi_panel_dom_isolation`, `blitz_document_structure`, `blitz_nested_elements_with_attributes`, `layout_resolve_in_render`.
- **Version bumped** to 0.2.0.

**What's NOT yet wired up** (deferred to Step 5.2b or 5.3):

- Vello offscreen rendering to GPU textures (needs wgpu device setup)
- OpenXR session lifecycle (`openxr` crate integration)
- UA stylesheet application to Blitz documents
- Binary opcode interpreter on the Rust side (Mojo-side interpreter calls individual FFI functions)

---

### Phase 5.1: XR Panel Abstraction Design — ✅ Complete

Designed and implemented the XR panel abstraction, scene manager, and Rust shim scaffold. Created the `xr/` directory structure with native and web sub-projects.

**Mojo types** (`xr/native/src/xr/`):

- `panel.mojo` — `XRPanel` (2D DOM document + 3D transform), `PanelConfig`, `PanelState`, `Vec3`, `Quaternion`. Panel presets: `default_panel_config()` (0.8m × 0.6m, 1200 ppm), `dashboard_panel_config()` (1.6m × 0.9m curved), `tooltip_panel_config()` (0.3m × 0.15m non-interactive), `hand_anchored_panel_config()` (0.2m × 0.15m).
- `scene.mojo` — `XRScene` (panel registry, focus management, dirty tracking, raycasting via ray-plane intersection, spatial layout helpers). `XREvent` with panel targeting and UV hit coordinates. `RaycastHit`. Layout helpers: `arrange_arc()`, `arrange_grid()`, `arrange_stack()`. Convenience constructors: `create_single_panel_scene()`, `create_dual_panel_scene()`.
- `__init__.mojo` — Package root with re-exports.

**Rust shim scaffold** (`xr/native/shim/`):

- `src/lib.rs` — `XrSessionContext` with headless mode (`mxr_create_headless`), multi-panel DOM (`Panel` with `HeadlessNode` tree), ID mapping, interpreter stack, event ring buffer, per-panel DOM operations (create/set/remove element/text/attribute/children), raycasting, DOM serialization, and 20+ integration tests covering: session lifecycle, panel creation/destruction, visibility, DOM element creation, text nodes, attributes, insert before/after, replace/remove, events (inject + poll, listener registration), raycasting (hit/miss/hidden-panel skip), focus management, frame loop, reference spaces, serialization, placeholder nodes, path navigation, UA stylesheets, version string.
- `mojo_xr.h` — C API header (~80 functions): session lifecycle, panel management, mutations, per-panel DOM operations, events, raycasting, frame loop, input (pose/aim ray), reference spaces, capabilities, debug/inspection.
- `Cargo.toml` — Blitz v0.2.0 (same rev as desktop), anyrender, anyrender_vello, wgpu, openxr.
- `default.nix` — Nix derivation with Blitz + OpenXR + GPU dependencies.

**Core platform updates** (`core/src/platform/features.mojo`):

- Added `has_xr`, `has_xr_hand_tracking`, `has_xr_passthrough` fields to `PlatformFeatures`.
- Added `xr_native_features()` and `xr_web_features()` preset constructors.
- Updated all existing presets (`web_features`, `desktop_blitz_features`, `native_features`) to include the new XR fields (all False for non-XR renderers).

---

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

### Phase 4.8: Stabilization & Verification — ✅ Complete (I-1, I-2, I-3, I-4)

**I-1: Cross-target build verification** — Verified all 4 shared examples (Counter, Todo, Benchmark, MultiView) build for both web (WASM) and desktop (native). 52 Mojo test suites pass (via wasmtime). 3,090 JS integration tests pass (via Deno). No regressions from the separation.

**I-3: Mutation protocol conformance tests** — New `conformance.test.ts` test suite (285 tests) covering:

- **Binary round-trip** — Every opcode (16 of 16) verified through MutationBuilder → MutationReader decode cycle, including unicode text, empty strings, and edge cases (max u32 IDs, deep paths, long strings, special characters)
- **RegisterTemplate serialization** — Round-trip verification for element nodes, static/dynamic children, static/dynamic attributes, dynamic text slots, and dynamic node slots
- **Canonical DOM output** — 12 UI patterns verified through the Interpreter with deterministic DOM serialization:
  - Text node mount, placeholder mount, template element mount
  - Template + dynamic text + SetText, template + dynamic attr + SetAttribute
  - Static template with text and attributes
  - SetText update, SetAttribute + RemoveAttribute cycle
  - Remove, ReplaceWith, InsertAfter, InsertBefore (including multi-node)
  - Counter-like mount + increment sequence
  - Todo-like mount + add item + remove item sequence
  - Conditional rendering (show/hide detail via placeholder swap)
  - Keyed list reorder simulation
  - Multiple templates, accumulation across apply calls
  - Complex app with all opcode types exercised (6-step lifecycle)
- **WASM ↔ JS byte-level comparison** — All 16 opcodes verified byte-identical between Mojo `write_op_*` exports and JS `MutationBuilder`, including string-carrying opcodes with `writeStringStruct`, unicode text, and multi-op sequences
- **Binary layout verification** — Exact byte offsets verified for PushRoot, AppendChildren, CreateTextNode, SetAttribute, End, Remove, AssignId, and LoadTemplate against the protocol spec
- **End-to-end WASM → Interpreter → DOM** — WASM-generated mutation buffers applied through the JS Interpreter, verifying correct DOM output for mount, text update, and attribute set/remove sequences

Total JS test count: 30 suites, ~3,375 tests (up from 29 suites, ~3,090 tests).

**I-2: Desktop integration tests** — Headless mode (`mblitz_create_headless`) and DOM inspection API added to the Blitz shim. 69 Rust integration tests in `desktop/shim/tests/integration.rs` covering:

- **Context lifecycle** — headless creation, mount point resolution, alive state
- **DOM operations** — element creation (21 tag types), text nodes (incl. unicode, empty), placeholder/comment nodes
- **Attributes** — set, get, overwrite, remove, multiple per element, nonexistent lookups
- **Tree structure** — append children (single, multiple), nested trees, insert before/after, replace (single/multi-node), remove with ID cleanup
- **Templates** — register, clone, verify independent copies
- **Path navigation** — `node_at_path` traversal to deeply nested children
- **Events** — inject click/input events, poll ordering, handler registration/removal, unicode values
- **Mutation batching** — batch flag state transitions
- **DOM serialization** — empty mount point, single/nested children, text, attributes, placeholders, subtree-only, quote escaping
- **ID mapping & stack** — bidirectional mapping, push/pop/pop-more-than-available, child mojo ID lookups (incl. out-of-bounds and unmapped)
- **Integration scenarios** — counter-like mount+update, conditional rendering (placeholder ↔ element swap), todo-like list add/remove
- **Stress tests** — 100 children, 20-deep nesting, 1000 rapid text updates, ID reassignment

New shim FFI functions: `mblitz_create_headless`, `mblitz_get_node_tag`, `mblitz_get_text_content`, `mblitz_get_attribute_value`, `mblitz_serialize_subtree`, `mblitz_inject_event`, `mblitz_get_child_mojo_id`. C header and Cargo config updated. `just test-desktop` recipe added.

**I-4: Document CSS support scope** — Created [docs/plan/css-support.md](docs/plan/css-support.md) documenting Blitz v0.2.0 CSS feature support across parsing (Stylo), layout (Taffy), and rendering (Vello). Covers fully supported features (box model, flexbox, grid, positioning, typography, backgrounds, borders, shadows, selectors, variables), partially supported features (fixed/sticky positioning, table layout, 2D transforms, intrinsic sizing), and unsupported features (transitions, animations, 3D transforms, filters, float, multi-column). Includes safe-to-use CSS subset for cross-renderer apps and upgrade audit checklist.

---

## Roadmap

### Immediate — Stabilization & Verification — ✅ Complete

All four stabilization tasks are complete.

| # | Task | Effort | Notes |
|---|------|--------|-------|
| I-1 | **Cross-target build verification** | 1–2 days | ✅ Complete — All 4 shared examples (Counter, Todo, Benchmark, MultiView) build for both web (WASM) and desktop (native). 52 Mojo test suites pass (via wasmtime). 3,090 JS integration tests pass (via Deno). No regressions from the separation. |
| I-2 | **Desktop integration tests** | 3–5 days | ✅ Complete — Headless mode added to Blitz shim (`mblitz_create_headless`). 69 Rust integration tests covering: context lifecycle, DOM element creation, text nodes (incl. unicode), attribute get/set/remove, tree structure (append/insert/replace/remove), template registration & cloning, event injection & polling, mutation batch markers, DOM serialization, node ID mapping & stack ops, counter-like mount+update scenario, conditional rendering (placeholder swap), todo-like list add/remove, stress tests (100 children, 20-deep nesting, 1000 rapid text updates). New DOM inspection FFI: `mblitz_get_node_tag`, `mblitz_get_text_content`, `mblitz_get_attribute_value`, `mblitz_serialize_subtree`, `mblitz_inject_event`, `mblitz_get_child_mojo_id`. Run via `just test-desktop`. |
| I-3 | **Mutation protocol conformance tests** | 2–3 days | ✅ Complete — `conformance.test.ts` (285 tests): binary round-trip for every opcode, RegisterTemplate serialization, canonical DOM output for 12 UI patterns (counter, todo, conditional, keyed list, nested templates, complex app), byte-level WASM↔JS comparison for all 16 opcodes + unicode, exact binary layout verification, and end-to-end WASM→Interpreter→DOM rendering. |
| I-4 | **Document CSS support scope** | 1 day | ✅ Complete — [docs/plan/css-support.md](docs/plan/css-support.md): comprehensive audit of Blitz v0.2.0 CSS support. Fully supported: box model, flexbox, CSS grid, relative/absolute positioning, typography, colors/backgrounds, borders, box shadows, overflow, selectors/cascade, CSS variables, calc(), media queries. Partially supported: `position: fixed/sticky`, `display: table/contents`, 2D transforms, `min-content`/`max-content`. Not supported: transitions, animations, 3D transforms, filters, float, multi-column, text-shadow, clip-path. Includes app authoring recommendations and desktop-safe CSS subset. |

### Short-term — Cross-Platform & CI

| # | Task | Effort | Notes |
|---|------|--------|-------|
| S-1 | **Cross-target CI pipeline** | 3–5 days | GitHub Actions (or similar) matrix: `{web, desktop-linux}` × `{counter, todo, bench, app}`. Run `just test-all` for web. Build + smoke-test for desktop (headless Wayland via `wlheadless` or `weston --headless`). Gate PRs on green matrix. |
| S-2 | **macOS desktop verification** | 2–3 days | Blitz uses Winit which supports macOS. Build the Blitz shim on macOS, verify `cargo build --release` succeeds, run the Counter example. Document any platform-specific quirks (GPU backend selection, font fallback, etc.). |
| S-3 | **Windows desktop verification (Wine)** | 2–3 days | ✅ Complete — Cross-compiled to `x86_64-pc-windows-gnu` from Linux via MinGW-w64. Produces `mojo_blitz.dll` (26MB PE32+ DLL). All 69 Rust integration tests pass under Wine (single-threaded; Wine's COM layer crashes with parallel threads — misaligned pointer in `windows-core` interface dispatch). Nix dev shell provides: `rust-bin` with Windows target std, MinGW-w64 cross-linker (stripped setup hooks to avoid polluting native CC/AR), Wine 10.0 for test execution. New justfile recipes: `build-shim-windows`, `test-desktop-wine`, `build-shim-all`. Cargo config (`.cargo/config.toml`) sets MinGW linker and Wine runner for the Windows target. |

### Medium-term — Phase 5: XR Renderer

> **Full plan:** [docs/plan/phase5-xr.md](docs/plan/phase5-xr.md) — **Effort estimate: 4–8 weeks**

XR panel abstraction that reuses the binary mutation protocol unchanged. Each XR panel gets its own `GuiApp` instance and mutation buffer.

| Step | Description | Status |
|------|-------------|--------|
| 5.1 | Design the XR panel abstraction (`XRPanel` struct, scene graph, placement) | ✅ Complete — `XRPanel`, `PanelConfig`, `Vec3`, `Quaternion`, `PanelState` (Mojo). `XRScene` with focus management, dirty tracking, raycasting (ray-plane intersection), spatial layout helpers (`arrange_arc`, `arrange_grid`, `arrange_stack`). Rust shim scaffold (`xr/native/shim/src/lib.rs`) with headless multi-panel DOM, event ring buffer, DOM serialization, raycasting, and 20+ integration tests. C API header (`mojo_xr.h`, ~80 functions). `PlatformFeatures` extended with `has_xr`, `has_xr_hand_tracking`, `has_xr_passthrough` and `xr_native_features()` / `xr_web_features()` presets. Panel presets: default, dashboard, tooltip, hand-anchored. |
| 5.2 | Build the OpenXR + Blitz Rust shim (offscreen Vello rendering → OpenXR swapchain textures) | 🔧 In progress — **real Blitz documents ✅** (HeadlessNode replaced with BaseDocument, 30 tests pass, Stylo+Taffy layout resolves). Remaining: Vello offscreen rendering, OpenXR session lifecycle, `_into()` FFI variants for struct-return functions. |
| 5.3 | Mojo FFI bindings for the OpenXR shim | ✅ Complete — `XRBlitz` struct (~70 methods wrapping all `mxr_*` C functions via DLHandle). `XRMutationInterpreter` (per-panel binary opcode interpreter, all 18 opcodes). Helper types: `XREvent`, `XRPose`, `XRRaycastHit`. Constants for events, hands, spaces, states. Library search via env vars / Nix / ld paths. |
| 5.4 | XR scene manager and panel routing (multiplexes mutation buffers) | 🔲 Pending |
| 5.5 | `xr_launch[AppType: GuiApp]()` — single-panel apps get XR for free | 🔲 Pending |
| 5.6 | WebXR JS runtime (DOM → texture, XR session management) | 🔲 Future |
| 5.7 | Wire `launch()` for XR targets (`@parameter if has_feature("xr")`) | 🔲 Pending |
| 5.8 | Verify shared examples in XR (all 4 apps render as floating panels) | 🔲 Pending |
| 5.9 | Multi-panel XR API — `XRGuiApp` trait for apps managing multiple panels (stretch goal) | 🔮 Future |

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
| OpenXR runtime availability (Phase 5) | XR fails without runtime | Detect at startup; fall back to desktop Blitz | In progress — headless mode (`mxr_create_headless`) implemented for testing without runtime |
| DOM-to-texture fidelity for WebXR (Phase 5) | Rendering quality/interactivity loss | Evaluate OffscreenCanvas, html2canvas, CSS 3D, custom 2D renderer | Future |
| XR input latency (Phase 5) | Raycasting → DOM event adds latency to controller input | Keep raycast math in the shim (Rust); minimize FFI roundtrips | In progress — Rust-side raycasting implemented |
| XR frame timing constraints (Phase 5) | OpenXR requires strict frame pacing; DOM re-render may exceed budget | Only re-render dirty panels; cache textures; use quad layers for compositor-side reprojection | In progress — dirty tracking per-panel implemented |

See [docs/plan/risks.md](docs/plan/risks.md) for the full risk register with 19 entries.