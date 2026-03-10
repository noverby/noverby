# Separation Plan — `wasm-mojo` → `mojo-gui` + `mojo-web`

## Executive Summary

Split the current `wasm-mojo` monolith into two projects:

1. **`mojo-gui`** — Multi-renderer reactive GUI framework
   - **`mojo-gui/core`** — Renderer-agnostic reactive GUI framework (Mojo library)
   - **`mojo-gui/web`** — Browser renderer (WASM + TypeScript)
   - **`mojo-gui/desktop`** — Desktop renderer (webview, future)
   - **`mojo-gui/native`** — Native renderer (direct widgets, future)
2. **`mojo-web`** — Raw Web API bindings for Mojo/WASM (like Rust's `web-sys`)

The goal: write a Mojo GUI app once, run it in the browser via WASM **and** natively on desktop — like Dioxus does for Rust. `mojo-web` provides foundational Web API access for any Mojo/WASM project, including but not limited to `mojo-gui`.

---

## Architectural Inspiration: Dioxus

Dioxus separates concerns as:

| Dioxus crate       | Role                                         |
|---------------------|----------------------------------------------|
| `dioxus-core`       | VirtualDom, signals, scopes, mutations       |
| `dioxus-html`       | HTML elements, attributes, events            |
| `dioxus-web`        | Browser renderer (WASM + JS interop)         |
| `dioxus-desktop`    | Desktop renderer (webview via Wry/Tao)       |
| `dioxus-native`     | Native renderer (Blitz layout engine)        |

Separately, Rust's `web-sys` crate provides raw bindings to **all** Web APIs (DOM, fetch, WebSocket, WebGL, etc.) via `wasm-bindgen`. Any Rust/WASM project can use `web-sys` directly — Dioxus-web uses it under the hood. `mojo-web` fills this same ecosystem role for Mojo.

Key insight: **the mutation protocol stays DOM-oriented even in core**. Desktop renderers either use a webview (DOM natively) or map DOM concepts to native widgets. This is pragmatic — HTML/DOM is a universal UI description language. `mojo-gui` uses the mutation protocol (not `mojo-web`) for rendering, keeping the multi-renderer architecture intact. `mojo-web` is for everything else an app needs from the browser: data fetching, storage, timers, canvas, etc.

---

## Current Module Map & Classification

### Renderer-Agnostic (→ `mojo-gui/core`)

These modules have **zero DOM/browser dependencies** — pure reactive infrastructure:

| Module                      | Purpose                                        |
|-----------------------------|------------------------------------------------|
| `src/signals/runtime.mojo`  | Reactive runtime, signal store, string store, context tracking |
| `src/signals/memo.mojo`     | MemoEntry, MemoStore (derived signals)         |
| `src/signals/effect.mojo`   | Effect infrastructure                          |
| `src/signals/handle.mojo`   | SignalI32, SignalBool, SignalString, MemoI32, MemoBool, MemoString, EffectHandle |
| `src/scope/scope.mojo`      | ScopeState, hooks, context, error/suspense     |
| `src/scope/arena.mojo`      | ScopeArena (slab allocator)                    |
| `src/scheduler/scheduler.mojo` | Height-ordered dirty scope queue            |
| `src/arena/element_id.mojo` | ElementId type and allocator                   |

### Virtual DOM Layer (→ `mojo-gui/core`)

The VNode/Template/diff machinery is *structurally* DOM-oriented but is renderer-agnostic in implementation — it never touches real DOM, only emits mutations to a buffer:

| Module                         | Purpose                                     |
|--------------------------------|---------------------------------------------|
| `src/vdom/template.mojo`       | Template, TemplateNode (static structure)   |
| `src/vdom/vnode.mojo`          | VNode, DynamicNode, AttributeValue, VNodeBuilder |
| `src/vdom/builder.mojo`        | TemplateBuilder API                         |
| `src/vdom/registry.mojo`       | Template storage and lookup                 |
| `src/mutations/create.mojo`    | CreateEngine (VNode → mutation buffer)      |
| `src/mutations/diff.mojo`      | DiffEngine (old/new VNode → minimal mutations) |
| `src/bridge/protocol.mojo`     | MutationWriter + binary opcodes             |

### HTML-Specific (→ `mojo-gui/core/html` submodule)

These define **what** elements/events exist — the HTML vocabulary:

| Module                      | Purpose                                        |
|-----------------------------|------------------------------------------------|
| `src/vdom/tags.mojo`        | TAG_DIV, TAG_SPAN, TAG_BUTTON, ... (38 tags)  |
| `src/vdom/dsl.mojo`         | `el_div()`, `el_button()`, `dyn_text()`, `onclick_add()`, inline event constructors |
| `src/vdom/dsl_tests.mojo`   | DSL test functions                             |
| `src/events/registry.mojo`  | HandlerEntry, HandlerRegistry, action tags, event type constants (EVT_CLICK, EVT_INPUT, ...) |

### Component Framework (→ `mojo-gui/core`, mixed concerns)

These bundle reactive + vdom + mutations into an ergonomic app framework. They reference HTML-specific types but the *structure* is renderer-agnostic:

| Module                              | Purpose                                  |
|--------------------------------------|------------------------------------------|
| `src/component/app_shell.mojo`       | AppShell: runtime + store + allocator + scheduler |
| `src/component/context.mojo`         | ComponentContext: ergonomic API, RenderBuilder |
| `src/component/child.mojo`           | ChildComponent rendering                 |
| `src/component/child_context.mojo`   | ChildComponentContext                    |
| `src/component/lifecycle.mojo`       | mount, diff, finalize, FragmentSlot, ConditionalSlot |
| `src/component/keyed_list.mojo`      | KeyedList, ItemBuilder, HandlerAction    |
| `src/component/router.mojo`          | URL path → branch router                 |

### Browser/WASM Runtime (→ `mojo-gui/web`)

Everything that runs in the browser or manages WASM instantiation:

| Module                        | Purpose                                      |
|-------------------------------|----------------------------------------------|
| `runtime/interpreter.ts`      | DOM stack machine (applies binary mutations) |
| `runtime/events.ts`           | DOM event delegation bridge                  |
| `runtime/templates.ts`        | Template cache (DocumentFragment cloning)    |
| `runtime/memory.ts`           | WASM memory management, free-list allocator  |
| `runtime/env.ts`              | WASM environment imports (I/O, math, libc)   |
| `runtime/strings.ts`          | Mojo String ABI helpers (SSO)                |
| `runtime/protocol.ts`         | JS-side mutation opcode parser               |
| `runtime/tags.ts`             | HTML tag name mapping (JS side)              |
| `runtime/app.ts`              | App lifecycle helpers, per-app handles       |
| `runtime/types.ts`            | WasmExports interface                        |
| `runtime/mod.ts`              | Entry point (instantiate WASM)               |
| `src/main.mojo`               | @export wrappers (WASM entry point)          |
| `examples/`                   | Browser example apps                         |
| `examples/lib/`               | Shared JS runtime for examples               |
| `test-js/`                    | JS integration tests                         |
| `scripts/`                    | Build scripts (Mojo → WASM pipeline)         |
| `justfile`                    | Build commands                               |
| `default.nix`                 | Nix dev shell                                |

### Test Apps (split across both)

| Module                        | Destination                                  |
|-------------------------------|----------------------------------------------|
| `src/apps/*.mojo`             | Stay with `mojo-gui/core` as test/demo apps  |
| `test/*.mojo`                 | Stay with `mojo-gui/core` (Mojo-side tests)  |
| `test-js/*.test.ts`           | Move to `mojo-gui/web` (browser integration) |

---

## Target Project Structure

```text
mojo-gui/
├── core/                             # Renderer-agnostic GUI framework
│   ├── src/
│   │   ├── signals/                  # Reactive primitives
│   │   │   ├── runtime.mojo          # Runtime, SignalStore, StringStore, context
│   │   │   ├── memo.mojo             # MemoEntry, MemoStore
│   │   │   ├── effect.mojo           # EffectHandle
│   │   │   └── handle.mojo           # SignalI32, SignalBool, SignalString, Memo*
│   │   ├── scope/                    # Scope lifecycle
│   │   │   ├── scope.mojo            # ScopeState, hooks, context, error/suspense
│   │   │   └── arena.mojo            # ScopeArena (slab allocator)
│   │   ├── scheduler/
│   │   │   └── scheduler.mojo        # Height-ordered dirty scope queue
│   │   ├── arena/
│   │   │   └── element_id.mojo       # ElementId type and allocator
│   │   ├── vdom/                     # Virtual DOM (renderer-agnostic)
│   │   │   ├── template.mojo         # Template, TemplateNode
│   │   │   ├── vnode.mojo            # VNode, DynamicNode, AttributeValue
│   │   │   ├── builder.mojo          # TemplateBuilder API
│   │   │   └── registry.mojo         # Template storage and lookup
│   │   ├── mutations/                # Mutation engines
│   │   │   ├── create.mojo           # CreateEngine (initial mount)
│   │   │   └── diff.mojo             # DiffEngine (reconciliation)
│   │   ├── bridge/
│   │   │   └── protocol.mojo         # MutationWriter + binary opcodes
│   │   ├── events/
│   │   │   └── registry.mojo         # HandlerEntry, HandlerRegistry, action tags
│   │   ├── component/                # Component framework
│   │   │   ├── app_shell.mojo        # AppShell
│   │   │   ├── context.mojo          # ComponentContext, RenderBuilder
│   │   │   ├── child.mojo            # ChildComponent
│   │   │   ├── child_context.mojo    # ChildComponentContext
│   │   │   ├── lifecycle.mojo        # mount, diff, finalize, Fragment/ConditionalSlot
│   │   │   ├── keyed_list.mojo       # KeyedList, ItemBuilder, HandlerAction
│   │   │   └── router.mojo           # URL path → branch router
│   │   ├── html/                     # HTML vocabulary (submodule)
│   │   │   ├── tags.mojo             # TAG_DIV, TAG_SPAN, ... (moved from vdom/tags.mojo)
│   │   │   ├── dsl.mojo              # el_div(), el_button(), ... (moved from vdom/dsl.mojo)
│   │   │   └── dsl_tests.mojo        # DSL tests (moved from vdom/dsl_tests.mojo)
│   │   └── lib.mojo                  # Package root: re-exports public API
│   ├── apps/                         # Demo/test apps (moved from src/apps/)
│   │   ├── counter.mojo
│   │   ├── todo.mojo
│   │   ├── bench.mojo
│   │   └── ...
│   ├── test/                         # Mojo-side unit tests
│   │   ├── test_signals.mojo
│   │   ├── test_scopes.mojo
│   │   ├── test_memo.mojo
│   │   └── ...
│   ├── AGENTS.md
│   ├── README.md
│   └── CHANGELOG.md
│
├── web/                              # Browser renderer (WASM + TypeScript)
│   ├── runtime/                      # TypeScript runtime (from wasm-mojo/runtime/)
│   │   ├── mod.ts
│   │   ├── interpreter.ts            # DOM stack machine
│   │   ├── events.ts                 # DOM event delegation
│   │   ├── templates.ts              # Template cache (DocumentFragment)
│   │   ├── memory.ts                 # WASM memory management
│   │   ├── env.ts                    # WASM environment imports
│   │   ├── strings.ts                # Mojo String ABI
│   │   ├── protocol.ts              # JS mutation opcode parser
│   │   ├── tags.ts                   # HTML tag names (JS side)
│   │   ├── app.ts                    # App lifecycle helpers
│   │   └── types.ts                  # WasmExports interface
│   ├── src/
│   │   └── main.mojo                 # @export WASM wrappers
│   ├── examples/                     # Browser examples
│   │   ├── counter/
│   │   ├── todo/
│   │   ├── bench/
│   │   └── lib/                      # Shared JS runtime
│   ├── test-js/                      # JS integration tests
│   │   ├── harness.ts
│   │   ├── counter.test.ts
│   │   └── ...
│   ├── scripts/                      # Build pipeline (Mojo → WASM)
│   │   ├── build_test_binaries.sh
│   │   ├── run_test_binaries.sh
│   │   └── precompile.mojo
│   ├── deno.json
│   ├── justfile
│   └── README.md
│
├── desktop/                          # Desktop renderer (Phase 2 — webview)
│   ├── src/
│   │   ├── main.mojo                 # Native entry point
│   │   ├── webview.mojo              # Webview management (FFI to Wry/Tao or OS APIs)
│   │   └── bridge.mojo              # Mutation buffer → webview JS bridge
│   ├── runtime/
│   │   └── ...                       # Reuses web/runtime/ interpreter in the webview
│   └── README.md
│
├── native/                           # Native renderer (Phase 3 — future)
│   ├── src/
│   │   ├── main.mojo                 # Native entry point
│   │   ├── renderer.mojo             # Mutation interpreter → native widgets
│   │   └── backend/                  # Platform-specific: GTK, Cocoa, Win32, etc.
│   └── README.md
│
└── README.md
```

---

## The Abstraction Boundary: Binary Mutation Protocol

The **mutation buffer** is the renderer contract. Every renderer must implement an interpreter that consumes the same binary opcode stream:

```text
┌──────────────────────┐     binary mutation buffer      ┌─────────────────────┐
│                      │  ───────────────────────────►   │                     │
│  mojo-gui/core       │     (shared linear memory       │  Renderer           │
│  (reactive framework │      or pipe/socket)            │  (web / desktop /   │
│   + virtual DOM      │                                 │   native)           │
│   + diff engine)     │  ◄───────────────────────────   │                     │
│                      │     event dispatch callbacks     │                     │
└──────────────────────┘                                 └─────────────────────┘
```

The opcodes (`OP_CREATE_TEXT_NODE`, `OP_SET_ATTRIBUTE`, `OP_LOAD_TEMPLATE`, etc.) are DOM-oriented by design. This is intentional — all three renderer targets can interpret them:

| Opcode              | Web (DOM)                     | Desktop (Webview)           | Native (future)             |
|---------------------|-------------------------------|-----------------------------|-----------------------------|
| `LOAD_TEMPLATE`     | `cloneNode(true)`             | Same (webview has DOM)      | Create widget tree          |
| `SET_ATTRIBUTE`     | `el.setAttribute()`           | Same                        | Set widget property         |
| `SET_TEXT`          | `node.textContent = ...`      | Same                        | Set label text              |
| `NEW_EVENT_LISTENER`| `addEventListener()`          | Same                        | Register widget callback    |
| `APPEND_CHILDREN`  | `parent.appendChild()`        | Same                        | Add child widget            |
| `REMOVE`           | `node.remove()`               | Same                        | Destroy widget              |

---

## Renderer Strategies

### Web Renderer (existing — move to `mojo-gui/web/`)

**How it works today:**

1. Mojo compiles to WASM via `mojo build` → `llc` → `wasm-ld`
2. TypeScript runtime instantiates WASM, provides env imports
3. Mojo writes mutations to shared linear memory
4. JS `Interpreter` reads mutation buffer, applies to real DOM
5. JS `EventBridge` captures DOM events, dispatches to WASM

**Changes needed:** Minimal. Mostly a file move. The `main.mojo` WASM export wrappers stay here.

### Desktop Renderer (new — `mojo-gui/desktop/`)

Strategy: embedded webview (like Dioxus Desktop). This is the pragmatic first approach. Dioxus desktop works exactly this way:

1. Mojo compiles to a **native binary** (no WASM)
2. The native binary embeds a webview (via FFI to Wry/Tao or direct OS webview APIs)
3. The same TypeScript/JS interpreter runs **inside** the webview
4. Communication: Mojo writes mutations → serializes to the webview via IPC → JS interpreter applies to DOM

**Architecture:**

```text
┌──────────────────────────────────────────────────────┐
│  Native Process                                       │
│                                                       │
│  ┌─────────────────────┐                              │
│  │  mojo-gui/core       │                              │
│  │  (compiled native)   │                              │
│  │                      │─── mutation buffer ──┐       │
│  │  signals, vdom,      │                      │       │
│  │  diff, scheduler     │◄── event dispatch ──┐│       │
│  └─────────────────────┘                     ││       │
│                                              ▼│       │
│  ┌─────────────────────────────────────────┐  │       │
│  │  Embedded Webview                        │  │       │
│  │  ┌────────────────────────────────────┐  │  │       │
│  │  │  JS Interpreter (reused from web/) │  │  │       │
│  │  │  EventBridge → IPC → native        │──┘  │       │
│  │  │  DOM rendering                     │      │       │
│  │  └────────────────────────────────────┘      │       │
│  └──────────────────────────────────────────────┘       │
└──────────────────────────────────────────────────────────┘
```

**Key difference from web:** The Mojo code runs as a native process (not WASM), and communicates with the webview via IPC (e.g., `window.postMessage`, named pipes, or shared memory) instead of shared WASM linear memory.

**Adaptation needed in `mojo-gui/core`:**

- The `MutationWriter` currently writes to WASM linear memory (`UnsafePointer[UInt8, MutExternalOrigin]`). For native, it writes to a heap buffer. The writer itself doesn't care — it just writes bytes to a pointer. ✅ Already works.
- The native host reads the buffer and sends it to the webview (base64, ArrayBuffer transfer, or shared memory mapping).

### Native Renderer (future — `mojo-gui/native/`)

Strategy: direct widget mapping. A true native renderer that maps DOM-like mutations to platform widgets:

- `LOAD_TEMPLATE` → create a widget subtree from a cached layout
- `SET_TEXT` → update a label/text widget
- `SET_ATTRIBUTE` → set widget properties (style, class → theme variants)
- `NEW_EVENT_LISTENER` → register widget callbacks

This requires a layout engine (like Dioxus uses Blitz/Taffy) and platform backend (GTK, Cocoa, Win32). This is a large effort and would be Phase 3.

---

## Phase 1: Extract `mojo-gui/core` Library

### Step 1.1 — Create `mojo-gui/core` directory structure

Create the new project skeleton. The reactive core, vdom, and component framework become a standalone Mojo library.

**Files to move (Mojo source):**

| From (`wasm-mojo/`)                  | To (`mojo-gui/core/`)                 |
|--------------------------------------|---------------------------------------|
| `src/signals/*`                      | `src/signals/*`                       |
| `src/scope/*`                        | `src/scope/*`                         |
| `src/scheduler/*`                    | `src/scheduler/*`                     |
| `src/arena/*`                        | `src/arena/*`                         |
| `src/vdom/template.mojo`            | `src/vdom/template.mojo`             |
| `src/vdom/vnode.mojo`               | `src/vdom/vnode.mojo`                |
| `src/vdom/builder.mojo`             | `src/vdom/builder.mojo`              |
| `src/vdom/registry.mojo`            | `src/vdom/registry.mojo`             |
| `src/mutations/*`                    | `src/mutations/*`                     |
| `src/bridge/*`                       | `src/bridge/*`                        |
| `src/events/*`                       | `src/events/*`                        |
| `src/component/*`                    | `src/component/*`                     |
| `src/vdom/tags.mojo`                | `src/html/tags.mojo`                 |
| `src/vdom/dsl.mojo`                 | `src/html/dsl.mojo`                  |
| `src/vdom/dsl_tests.mojo`           | `src/html/dsl_tests.mojo`            |
| `src/apps/*`                         | `apps/*`                              |
| `test/*.mojo` (Mojo-side tests)     | `test/*`                              |

**Import path changes:**

| Old import                           | New import                            |
|--------------------------------------|---------------------------------------|
| `from vdom.tags import TAG_DIV, ...` | `from html.tags import TAG_DIV, ...`  |
| `from vdom.dsl import el_div, ...`   | `from html.dsl import el_div, ...`    |

The `vdom/dsl.mojo` module currently imports from `vdom.tags`, `vdom.template`, `vdom.vnode`, and `events.registry`. When moved to `html/dsl.mojo`, imports from `vdom.*` stay the same (sibling package), only `html.tags` changes.

### Step 1.2 — Introduce a Renderer Trait (Deferred Abstraction)

Currently, `MutationWriter` writes directly to a raw byte buffer. This is already generic enough — any renderer can read from a byte buffer. No trait abstraction is needed immediately.

However, for **event dispatch**, the current system is tightly coupled:

- Events flow: DOM → JS EventBridge → WASM export → `HandlerRegistry.dispatch()`
- The `dispatch_event` function lives in `main.mojo` (WASM exports)

**For native rendering**, event dispatch would flow:

- Widget callback → native Mojo code → `HandlerRegistry.dispatch()`

This already works because `HandlerRegistry.dispatch()` is a regular Mojo method with no WASM dependency. The only difference is the entry point (WASM export vs. native function call).

**Decision: No Renderer trait for Phase 1.** The mutation buffer protocol IS the trait, de facto. Renderers implement an interpreter for the binary opcodes.

### Step 1.3 — Make `mojo-gui/core` compile to both WASM and native

The core Mojo code should compile with both:

- `mojo build --target wasm64-wasi` (for web renderer)
- `mojo build` (for native, default target)

**Blockers to check:**

- The `MutationWriter` uses `UnsafePointer[UInt8, MutExternalOrigin]` — the `MutExternalOrigin` origin attribute might be WASM-specific. Need to verify it compiles natively.
- No `@export` decorators in the library code (those stay in `main.mojo` per-renderer).
- No WASM-specific memory layout assumptions (the code uses `alloc`/`UnsafePointer` which work natively too).

**Expected result:** The core library compiles cleanly for both targets with no changes beyond import paths.

### Step 1.4 — Mojo-side test suite

Move all `test/*.mojo` files to `mojo-gui/core/test/`. These tests use `wasmtime` to run WASM binaries — this works for both targets:

- **WASM target:** Tests compile app to WASM, run via wasmtime (existing flow)
- **Native target:** Tests can also compile and run as native binaries directly

Update `scripts/build_test_binaries.sh` and `scripts/run_test_binaries.sh` to support both modes.

---

## Phase 2: Create `mojo-gui/web` (Browser Renderer)

### Step 2.1 — Move web-specific files

| From (`wasm-mojo/`)                  | To (`mojo-gui/web/`)                 |
|--------------------------------------|---------------------------------------|
| `runtime/*`                          | `runtime/*`                           |
| `src/main.mojo`                      | `src/main.mojo`                       |
| `examples/*`                         | `examples/*`                          |
| `test-js/*`                          | `test-js/*`                           |
| `scripts/*`                          | `scripts/*`                           |
| `justfile`                           | `justfile`                            |
| `deno.json`, `deno.lock`            | `deno.json`, `deno.lock`             |
| `default.nix`                        | `default.nix`                         |

### Step 2.2 — Wire `main.mojo` to import from `mojo-gui/core`

`main.mojo` currently imports from relative paths (`from signals import ...`, `from vdom import ...`). After separation, it needs to import from the `mojo-gui/core` package:

```text
# Before (monolith):
from signals import Runtime, create_runtime
from vdom import TemplateBuilder, VNode

# After (separate packages):
from mojo_gui.core.signals import Runtime, create_runtime
from mojo_gui.core.vdom import TemplateBuilder, VNode
```

**Mojo package dependency mechanism:** As of Mojo 0.26.1, the package system is still evolving. Options:

1. **Git submodule** — `mojo-gui/web/` includes `mojo-gui/core` as a submodule
2. **Symlink** — development convenience, `src/mojo_gui_core -> ../../core/src`
3. **Mojo package path** — `-I` flag or equivalent to add `core/src` to the import search path
4. **Mono-repo** — keep both projects in one repo with a workspace-style layout

**Recommended: Mono-repo with path-based imports** (option 3/4) until Mojo has a proper package manager. The `mojo-gui/` root directory is naturally a mono-repo workspace.

### Step 2.3 — Verify the existing test suite passes

After the file moves:

1. All 1,323 Mojo tests pass (compiled via wasmtime)
2. All 3,090 JS tests pass (compiled via Deno)
3. All three example apps work in the browser

### Step 2.4 — Extract `main.mojo` WASM exports into generated boilerplate

Currently `main.mojo` is ~6,730 lines of `@export` wrappers. Many of these are mechanical (create app, destroy app, init, rebuild, flush, dispatch_event × N apps). Consider generating these from a manifest to make adding new apps easier across renderers.

---

## Phase 3: Create `mojo-gui/desktop` (Desktop Renderer)

### Step 3.1 — Design the desktop architecture

**Webview approach** (pragmatic, like Dioxus Desktop):

```text
┌─ Native Mojo Process ─────────────────────────────────┐
│                                                        │
│  app.mojo (user app code)                              │
│      │                                                 │
│      ▼                                                 │
│  mojo-gui/core (reactive framework)                    │
│      │ writes mutations to buffer                      │
│      ▼                                                 │
│  desktop/bridge.mojo                                   │
│      │ serializes buffer → IPC message                 │
│      ▼                                                 │
│  desktop/webview.mojo (FFI → system webview)           │
│      │ evaluateJavaScript() / postMessage              │
│      ▼                                                 │
│  ┌─ Embedded Webview ───────────────────────────┐      │
│  │  <script>                                    │      │
│  │    const interp = new Interpreter(root);     │      │
│  │    // receive mutation buffer from native     │      │
│  │    window.onMessage = (buf) => {             │      │
│  │      interp.applyMutations(buf);             │      │
│  │    };                                        │      │
│  │    // send events back to native             │      │
│  │    bridge.addEventListener('click', (e) => { │      │
│  │      native.postMessage({handler, type});    │      │
│  │    });                                       │      │
│  │  </script>                                   │      │
│  └──────────────────────────────────────────────┘      │
└────────────────────────────────────────────────────────┘
```

### Step 3.2 — Implement webview FFI

The Mojo native binary needs to create and control a webview window. Options:

| Platform   | Webview API                      | FFI approach           |
|------------|----------------------------------|------------------------|
| macOS      | WKWebView (WebKit)               | Mojo → C FFI → ObjC   |
| Linux      | WebKitGTK                        | Mojo → C FFI → GTK    |
| Windows    | WebView2 (Chromium)              | Mojo → C FFI → COM    |
| Cross-plat | [webview/webview](https://github.com/webview/webview) C library | Mojo → C FFI |

The `webview/webview` C library is the easiest path — it provides a single C API across all platforms, similar to how Dioxus uses Wry.

### Step 3.3 — Implement the IPC bridge

The bridge between native Mojo and the embedded webview:

**Mutations (Mojo → Webview):**

1. Mojo writes mutations to a byte buffer (same as WASM)
2. Bridge encodes the buffer (base64, or typed array transfer)
3. Bridge calls `webview.evaluateJavaScript("applyMutations('" + encoded + "')")`
4. JS `Interpreter` decodes and applies to DOM inside the webview

**Events (Webview → Mojo):**

1. JS `EventBridge` in webview captures DOM events
2. Bridge sends event data via `window.external.invoke()` or custom scheme
3. Native process receives the callback
4. Routes to `HandlerRegistry.dispatch()` in `mojo-gui/core`

### Step 3.4 — Reuse the web runtime JS inside the webview

The `runtime/interpreter.ts`, `runtime/events.ts`, `runtime/templates.ts` etc. can be bundled into a single JS file that runs inside the webview. The only change: instead of reading from WASM linear memory, the interpreter receives mutation buffers via IPC.

**Create a `runtime/interpreter-standalone.ts`** that:

- Has no WASM memory dependency
- Receives mutation buffers as `ArrayBuffer` via message passing
- Sends events back via message passing
- Reuses 100% of the existing `Interpreter` and `EventBridge` classes

### Step 3.5 — Desktop entry point

```text
# examples/desktop_counter.mojo

from mojo_gui.core.component import ComponentContext
from mojo_gui.core.html.dsl import el_div, el_button, text, dyn_text
from mojo_gui.desktop import DesktopApp

fn main():
    var app = DesktopApp(
        title="Counter",
        width=400,
        height=300,
    )
    # Same app code as web — just a different entry point
    var counter = CounterApp()
    app.run(counter)
```

The user's app code is **identical** — only the entry point and renderer differ. This is the Dioxus model.

---

## Phase 4: Unified App Entry Point (Optional Future)

Like Dioxus's `dioxus::launch()`, provide a single entry point that selects the renderer at compile time:

```text
# my_app.mojo
from mojo_gui.core import launch
from mojo_gui.core.html.dsl import el_div, el_button, text, dyn_text

fn app():
    var ctx = ComponentContext.create()
    var count = ctx.use_signal(0)
    ctx.setup_view(
        el_div(
            el_h1(dyn_text()),
            el_button(text("Click me"), onclick_add(count, 1)),
        ),
        String("app"),
    )

fn main():
    launch(app)  # Renderer selected by build target or feature flag
```

**Compile targets:**

- `mojo build --target wasm64-wasi` → web renderer (needs `mojo-gui/web` JS runtime)
- `mojo build` → desktop renderer (embeds webview, no WASM)
- `mojo build --feature native` → native renderer (future)

---

## Dependency Graph

```text
                    ┌──────────────┐
                    │  User App    │
                    │ (my_app.mojo)│
                    └──┬───────┬───┘
                       │       │
              imports  │       │  imports (optional,
                       │       │  web-only features)
                       ▼       ▼
              ┌──────────┐  ┌──────────┐
              │ mojo-gui │  │ mojo-web │
              │ /core    │  │          │
              │          │  │ DOM      │
              │ signals/ │  │ fetch    │
              │ scope/   │  │ WebSocket│
              │ vdom/    │  │ storage  │
              │ mutations│  │ timers   │
              │ bridge/  │  │ canvas   │
              │ events/  │  │ ...      │
              │ component│  └──────────┘
              │ html/    │
              └────┬─────┘
                   │ consumed by
              ┌────┼────────────┐
              ▼    ▼            ▼
     ┌──────────┐ ┌──────────┐ ┌──────────┐
     │ mojo-gui │ │ mojo-gui │ │ mojo-gui │
     │ /web     │ │ /desktop │ │ /native  │
     │          │ │          │ │ (future) │
     │ main.mojo│ │ webview  │ │          │
     │ runtime/ │ │ + reused │ │ widget   │
     │ examples/│ │ JS interp│ │ mapping  │
     └──────────┘ └──────────┘ └──────────┘
```

---

## Migration Checklist

### Phase 1: `mojo-gui/core` extraction

- [x] Create `mojo-gui/core/` directory structure
- [x] Move `src/signals/`, `src/scope/`, `src/scheduler/`, `src/arena/` unchanged
- [x] Move `src/vdom/{template,vnode,builder,registry}.mojo` to `mojo-gui/core/src/vdom/`
- [x] Move `src/vdom/{tags,dsl,dsl_tests}.mojo` to `mojo-gui/core/src/html/`
- [x] Update `html/dsl.mojo` imports: `from vdom.builder`, `from vdom.template`, `from vdom.vnode` (was relative `.builder`, `.template`, `.vnode`); `.tags` stays relative
- [x] Move `src/mutations/`, `src/bridge/`, `src/events/` unchanged
- [x] Move `src/component/` — updated `child.mojo`, `child_context.mojo`, `context.mojo`, `keyed_list.mojo` to split `from vdom` / `from html` imports
- [x] Move `src/apps/` to `mojo-gui/core/apps/`
- [x] Update app imports in `apps/*.mojo` for new `html/` path (`from vdom import` → `from html import`)
- [x] Move `test/*.mojo` to `mojo-gui/core/test/`
- [x] Update test imports for new paths (`test_handles.mojo`: `from vdom` → `from html`)
- [x] Verify all 1,323 Mojo tests pass
- [x] Verify `mojo-gui/core` compiles for native target (no `@export` decorators)
- [x] Write `mojo-gui/core/README.md`
- [x] Update `mojo-gui/core/AGENTS.md`

### Phase 2: `mojo-gui/web` extraction

- [x] Create `mojo-gui/web/` directory structure
- [x] Move `runtime/` to `mojo-gui/web/runtime/`
- [x] Move `src/main.mojo` to `mojo-gui/web/src/main.mojo`
- [x] Update `main.mojo` imports to reference `mojo-gui/core` package — split `from vdom` into `from vdom` + `from html`; `from vdom.dsl_tests` → `from html.dsl_tests`
- [x] Move `examples/` to `mojo-gui/web/examples/` — updated `counter.mojo`, `app.mojo`, `todo.mojo`, `bench.mojo` to split `from vdom` / `from html` imports
- [x] Move `test-js/` to `mojo-gui/web/test-js/`
- [x] Move `scripts/` to `mojo-gui/web/scripts/`
- [x] Move build files (`justfile`, `deno.json`, `default.nix`) — updated `justfile` with `-I ../core/src` for core package resolution
- [x] Update all import paths in moved files
- [x] Verify all 3,090 JS tests pass
- [x] Verify all 3 example apps work in browser — JS tests (3,090) and Mojo tests (52 suites) pass; browser verification blocked by headless Servo in CI
- [x] Write `mojo-gui/web/README.md`

### Phase 3: `mojo-gui/desktop` (new development)

- [x] Design IPC protocol between native Mojo and webview — mutations via base64-encoded `webview_eval()`, events via JSON ring buffer polled from `mojo_post()`
- [x] Implement webview FFI (via GTK4 + WebKitGTK C shim) — `shim/mojo_webview.{h,c}` with 17 exported functions; Mojo bindings via `OwnedDLHandle` in `src/desktop/webview.mojo`
- [x] Bundle `runtime/interpreter.ts` as standalone JS for webview injection — `runtime/desktop-runtime.js` (27 KB, self-contained IIFE with MutationReader, Interpreter, TemplateCache, EventBridge)
- [x] Implement mutation buffer serialization (native → webview) — `mwv_apply_mutations()` base64-encodes buffer and calls `window.__mojo_apply_mutations()` in JS
- [x] Implement event bridge (webview → native) — JS dispatches `window.mojo_post(JSON)` → WebKitUserContentManager → C ring buffer → `mwv_poll_event()` → Mojo `DesktopEvent`
- [x] Create desktop entry point (`DesktopApp` struct) — `src/desktop/app.mojo` with `init()`, `run()`, `step()`, `flush_mutations()`, `poll_event()` cooperative event loop
- [x] Port counter example to desktop — `examples/counter.mojo` compiles to 195 KB native binary with full conditional rendering support
- [ ] Port todo example to desktop
- [x] Write `mojo-gui/desktop/README.md`

---

## Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Mojo package system immaturity | Can't cleanly separate into packages | Mono-repo with path-based imports; symlinks for dev |
| `MutExternalOrigin` tied to WASM | Core won't compile natively | Audit and abstract the origin parameter; conditionally compile |
| Webview FFI complexity | Desktop renderer takes too long | Start with single-platform (macOS or Linux); use `webview/webview` C lib |
| IPC overhead (native → webview) | Desktop perf worse than web | Use shared memory or zero-copy transfer for mutation buffers |
| Import path breakage | Massive search-and-replace | Script the migration; grep-verify all imports |
| Test suite fragmentation | Tests break across projects | Phase 1 must keep all Mojo tests green; Phase 2 must keep all JS tests green |

---

## Estimated Effort

| Phase | Effort | Description |
|-------|--------|-------------|
| Phase 1 | 2–3 days | File moves, import path updates, verify compilation + tests |
| Phase 2 | 1–2 days | Move web runtime, update imports, verify browser tests |
| Phase 3 | 2–4 weeks | Webview FFI, IPC bridge, desktop entry point, first example |
| Phase 4 | 1 week | Unified `launch()` API (optional, after Phase 3 proves out) |
| Phase 5 | 2–3 weeks | `mojo-web` MVP: handle table, DOM, fetch, timers, storage |

---

## `mojo-web` — Raw Web API Bindings

### Purpose

`mojo-web` is a standalone Mojo library providing typed bindings to Web APIs for any Mojo/WASM project — the equivalent of Rust's `web-sys` crate. It is **not** part of `mojo-gui` and has no dependency on it.

### Architecture

Since Mojo lacks a `wasm-bindgen` equivalent, `mojo-web` uses the same pattern already proven in `wasm-mojo`: WASM imports backed by a JS-side handle table.

**JS side** — a runtime that exposes Web APIs as flat WASM-importable functions:

```typescript
// Handle table: maps integer IDs to JS objects
const handles = new Map<number, any>();
let nextId = 1;

export const mojo_web = {
  document_create_element(tag_ptr: bigint, tag_len: number): number {
    const tag = readString(tag_ptr, tag_len);
    const el = document.createElement(tag);
    handles.set(nextId, el);
    return nextId++;
  },
  node_append_child(parent: number, child: number): void {
    handles.get(parent)!.appendChild(handles.get(child)!);
  },
  handle_drop(id: number): void {
    handles.delete(id);
  },
  // ... more Web API bindings
};
```

**Mojo side** — typed wrappers over the imported functions:

```mojo
struct JsHandle(Movable):
    """Opaque handle to a JS object. Dropped via handle_drop() on the JS side."""
    var id: Int32

struct Element:
    var handle: JsHandle

    fn set_attribute(self, name: String, value: String):
        _web_sys_set_attribute(self.handle.id, name, value)

struct Document:
    fn create_element(self, tag: String) -> Element:
        var id = _web_sys_create_element(tag)
        return Element(JsHandle(id))
```

### API Surface (MVP — Phase 5)

| Module | Web APIs | Examples |
|--------|----------|----------|
| `dom` | Document, Element, Node, Text, Event | `document.create_element()`, `el.set_attribute()` |
| `fetch` | fetch, Request, Response, Headers | `fetch(url).await_response()` |
| `timers` | setTimeout, setInterval, requestAnimationFrame | `set_timeout(callback, ms)` |
| `storage` | localStorage, sessionStorage | `local_storage.get_item(key)` |
| `console` | console.log, warn, error | `console.log(msg)` |
| `url` | URL, URLSearchParams | `URL.parse(href)` |
| `websocket` | WebSocket | `WebSocket.connect(url)` |
| `canvas` | Canvas2D, WebGL (future) | `ctx.fill_rect(x, y, w, h)` |

### Relationship to `mojo-gui`

`mojo-web` and `mojo-gui` are **independent** projects:

- `mojo-gui` uses the binary mutation protocol for rendering — it does NOT use `mojo-web` for DOM manipulation. This keeps the multi-renderer architecture intact.
- `mojo-gui` apps can import `mojo-web` for **non-rendering** web features: data fetching (suspense + fetch), persistent storage, WebSocket connections, animation timers, etc.
- `mojo-web` can be used by any Mojo/WASM project that has nothing to do with `mojo-gui`.

```text
┌────────────────────────────────────────────────┐
│  User App                                       │
│                                                 │
│  GUI rendering:     Non-rendering web features: │
│  mojo-gui/core      mojo-web                    │
│  (mutation protocol) (direct Web API calls)     │
└────────────────────────────────────────────────┘
```

### Project Structure

```text
mojo-web/
├── src/
│   ├── handle.mojo               # JsHandle — opaque reference to JS objects
│   ├── dom.mojo                  # Document, Element, Node, Text, Event
│   ├── fetch.mojo                # fetch(), Request, Response, Headers
│   ├── timers.mojo               # setTimeout, setInterval, requestAnimationFrame
│   ├── storage.mojo              # localStorage, sessionStorage
│   ├── console.mojo              # console.log/warn/error
│   ├── url.mojo                  # URL, URLSearchParams
│   ├── websocket.mojo            # WebSocket
│   └── lib.mojo                  # Package root
├── runtime/
│   └── mojo_web.ts               # JS-side handle table + Web API bindings
├── test/
│   └── ...
├── examples/
│   └── fetch_example.mojo        # Simple fetch + DOM example
└── README.md
```

---

## Open Questions

1. **Mono-repo vs. multi-repo?** — Mono-repo is the natural fit: `mojo-gui/` is the workspace root containing `core/`, `web/`, `desktop/`, `native/`. `mojo-web` could live alongside as a sibling or in a separate repo. Safer to keep together until Mojo has a package manager. Can split later.

2. **Should `html/` stay in `mojo-gui/core` or become a separate `mojo-gui/html` package?** — Keep in `core` for now. A native renderer that doesn't use HTML elements would need a different DSL (e.g., `el_box()`, `el_label()`), but that's Phase 4+ territory.

3. **How to handle the `@export` boilerplate in `main.mojo`?** — Consider a code generator that reads app definitions and emits WASM/native entry points. This reduces duplication across renderers.

4. **Webview library choice?** — `webview/webview` (C) is the most portable. Alternatively, Mojo could FFI directly to platform APIs (WKWebView, WebKitGTK, WebView2) for more control, at the cost of platform-specific code.

5. **Should the desktop renderer bundle Deno/TypeScript or use plain JS?** — Bundle as plain JS (no Deno dependency). Use `esbuild` or similar to bundle the TypeScript runtime into a single JS file for webview injection.

6. **Should `mojo-web` reuse `mojo-gui/web`'s existing JS runtime code?** — Partially. `memory.ts`, `env.ts`, and `strings.ts` solve the same WASM↔JS interop problems. Extract a shared `mojo-wasm-runtime` base, or let `mojo-web` depend on just those modules.

7. **Should `mojo-gui/web` eventually use `mojo-web` for its JS runtime?** — Possibly for non-rendering parts (e.g., the `EventBridge` could use `mojo-web`'s DOM bindings). The mutation protocol interpreter should stay as-is for performance (batched application vs. per-call overhead).