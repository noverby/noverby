# Separation Plan — `wasm-mojo` → `mojo-gui` + `mojo-web`

## Executive Summary

Split the current `wasm-mojo` monolith into two projects:

1. **`mojo-gui`** — Multi-renderer reactive GUI framework
   - **`mojo-gui/core`** — Renderer-agnostic reactive GUI framework (Mojo library)
   - **`mojo-gui/web`** — Browser renderer (WASM + TypeScript)
   - **`mojo-gui/desktop`** — Desktop renderer ([Blitz](https://github.com/DioxusLabs/blitz) native HTML/CSS engine, future)
   - **`mojo-gui/native`** — Native renderer (platform widgets, future)
   - **`mojo-gui/examples`** — Shared example apps that run on **every** renderer target unchanged
2. **`mojo-web`** — Raw Web API bindings for Mojo/WASM (like Rust's `web-sys`)

The goal: write a Mojo GUI app **once**, run it in the browser via WASM **and** natively on desktop — like Dioxus does for Rust. App code is platform-agnostic by design; examples are shared across all renderer targets and must compile and run identically on each. `mojo-web` provides foundational Web API access for any Mojo/WASM project, including but not limited to `mojo-gui`.

---

## Architectural Inspiration: Dioxus

Dioxus separates concerns as:

| Dioxus crate        | Role                                         |
|---------------------|----------------------------------------------|
| `dioxus-core`       | VirtualDom, signals, scopes, mutations       |
| `dioxus-html`       | HTML elements, attributes, events            |
| `dioxus-web`        | Browser renderer (WASM + JS interop)         |
| `dioxus-native`     | Native renderer (Blitz HTML/CSS engine)      |

Critically, Dioxus examples live in the workspace root and compile against any renderer. The user calls `dioxus::launch(App)` and the renderer is selected at compile time via feature flags. **We follow this same model:** examples are never renderer-specific, and `launch()` is the only platform-dependent call.

Separately, Rust's `web-sys` crate provides raw bindings to **all** Web APIs (DOM, fetch, WebSocket, WebGL, etc.) via `wasm-bindgen`. Any Rust/WASM project can use `web-sys` directly — Dioxus-web uses it under the hood. `mojo-web` fills this same ecosystem role for Mojo.

Key insight: **the mutation protocol stays DOM-oriented even in core**. Desktop renderers use a native HTML/CSS rendering engine (like [Blitz](https://github.com/DioxusLabs/blitz)) that provides a real DOM without a browser, while future native renderers map DOM concepts to platform widgets. This is pragmatic — HTML/DOM is a universal UI description language. `mojo-gui` uses the mutation protocol (not `mojo-web`) for rendering, keeping the multi-renderer architecture intact. `mojo-web` is for everything else an app needs from the browser: data fetching, storage, timers, canvas, etc.

---

## Design Principle: Shared Examples, Abstracted Platform

A core design principle of this separation is that **all example apps are platform-agnostic and shared across every renderer target**. This means:

1. **App code never imports a renderer.** Apps import only from `mojo-gui/core` (signals, components, HTML DSL). They do NOT import from `mojo-gui/web` or `mojo-gui/desktop`.

2. **The `launch()` function is the abstraction boundary.** Each renderer provides a `launch()` entry point. The app defines its root component; the renderer drives the event loop.

3. **Examples live in `mojo-gui/examples/`, not per-renderer.** There is ONE counter app, ONE todo app, ONE bench app. Each is built for web via `mojo build --target wasm64-wasi` and for desktop via `mojo build` (native). The app source is identical — only the build target differs.

4. **Renderer-specific entry points are thin wrappers.** The `web/` and `desktop/` directories provide only the machinery to drive the shared app code on their respective platforms. They do not contain app logic.

5. **If an example doesn't work on a target, it's a framework bug.** The framework must abstract away platform differences so that any app written against `core` works on every supported renderer.

This principle applies equally to user-authored apps: if you write an app against `mojo-gui/core`, it should run on web, desktop, and (eventually) native without modification.

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

### Platform Abstraction (→ `mojo-gui/core`, new)

A thin abstraction layer that lets apps remain platform-agnostic:

| Module                              | Purpose                                  |
|--------------------------------------|------------------------------------------|
| `src/platform/launch.mojo`          | `launch()` — platform-dispatching entry point (compile-time target selection) |
| `src/platform/app.mojo`             | `App` trait — interface every renderer's app host must implement |
| `src/platform/features.mojo`        | Feature detection — what capabilities are available on the current target |

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
| `test-js/`                    | JS integration tests                         |
| `scripts/`                    | Build scripts (Mojo → WASM pipeline)         |
| `justfile`                    | Build commands                               |
| `default.nix`                 | Nix dev shell                                |

### Example Apps (→ `mojo-gui/examples/`, shared across all targets)

| Module                        | Destination                                  |
|-------------------------------|----------------------------------------------|
| `src/apps/counter.mojo`      | `mojo-gui/examples/counter/app.mojo` — shared app logic |
| `src/apps/todo.mojo`         | `mojo-gui/examples/todo/app.mojo` — shared app logic |
| `src/apps/bench.mojo`        | `mojo-gui/examples/bench/app.mojo` — shared app logic |
| `examples/counter/`          | `mojo-gui/examples/counter/web/` — web-specific assets (HTML, JS glue) |
| `examples/todo/`             | `mojo-gui/examples/todo/web/` — web-specific assets |
| `examples/bench/`            | `mojo-gui/examples/bench/web/` — web-specific assets |
| `test/*.mojo`                | `mojo-gui/core/test/` (Mojo-side unit tests) |
| `test-js/*.test.ts`          | `mojo-gui/web/test-js/` (browser integration tests) |

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
│   │   ├── platform/                 # Platform abstraction layer
│   │   │   ├── launch.mojo           # launch() — target-dispatching entry point
│   │   │   ├── app.mojo              # App trait — interface renderers implement
│   │   │   └── features.mojo         # Runtime feature detection
│   │   ├── html/                     # HTML vocabulary (submodule)
│   │   │   ├── tags.mojo             # TAG_DIV, TAG_SPAN, ... (moved from vdom/tags.mojo)
│   │   │   ├── dsl.mojo              # el_div(), el_button(), ... (moved from vdom/dsl.mojo)
│   │   │   └── dsl_tests.mojo        # DSL tests (moved from vdom/dsl_tests.mojo)
│   │   └── lib.mojo                  # Package root: re-exports public API
│   ├── test/                         # Mojo-side unit tests
│   │   ├── test_signals.mojo
│   │   ├── test_scopes.mojo
│   │   ├── test_memo.mojo
│   │   └── ...
│   ├── AGENTS.md
│   ├── README.md
│   └── CHANGELOG.md
│
├── examples/                         # Shared example apps (run on ALL targets)
│   ├── counter/
│   │   ├── app.mojo                  # Counter app logic (platform-agnostic)
│   │   └── web/                      # Web-specific assets (HTML shell, JS glue)
│   │       ├── index.html
│   │       └── main.ts
│   ├── todo/
│   │   ├── app.mojo                  # Todo app logic (platform-agnostic)
│   │   └── web/
│   │       ├── index.html
│   │       └── main.ts
│   ├── bench/
│   │   ├── app.mojo                  # Bench app logic (platform-agnostic)
│   │   └── web/
│   │       ├── index.html
│   │       └── main.ts
│   └── README.md                     # How to build & run examples on each target
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
│   │   ├── main.mojo                 # @export WASM wrappers
│   │   └── web_launcher.mojo         # WebApp — implements App trait for WASM target
│   ├── test-js/                      # JS integration tests
│   │   ├── harness.ts
│   │   ├── counter.test.ts
│   │   └── ...
│   ├── scripts/                      # Build pipeline (Mojo → WASM)
│   │   ├── build_test_binaries.sh
│   │   ├── run_test_binaries.sh
│   │   ├── build_examples.sh         # Builds all shared examples for web target
│   │   └── precompile.mojo
│   ├── deno.json
│   ├── justfile
│   └── README.md
│
├── desktop/                          # Desktop renderer (Phase 3 — Blitz native engine)
│   ├── src/
│   │   ├── desktop_launcher.mojo     # DesktopApp — implements App trait for native target
│   │   ├── blitz.mojo               # Blitz DOM management (FFI to blitz-dom C shim)
│   │   ├── renderer.mojo            # Mutation interpreter → Blitz DOM operations
│   │   └── events.mojo             # Blitz/Winit events → HandlerRegistry dispatch
│   ├── shim/
│   │   ├── mojo_blitz.h            # C shim header for Blitz Rust API
│   │   └── mojo_blitz.rs           # Rust cdylib exposing Blitz via C ABI
│   ├── scripts/
│   │   └── build_examples.sh        # Builds all shared examples for desktop target
│   └── README.md
│
├── native/                           # Native renderer (Phase 4 — future, platform widgets)
│   ├── src/
│   │   ├── native_launcher.mojo      # NativeApp — implements App trait for native widgets
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

| Opcode              | Web (DOM)                     | Desktop (Blitz)                       | Native (future)             |
|---------------------|-------------------------------|---------------------------------------|-----------------------------|
| `LOAD_TEMPLATE`     | `cloneNode(true)`             | `blitz_clone_template()` via C FFI    | Create widget tree          |
| `SET_ATTRIBUTE`     | `el.setAttribute()`           | `blitz_set_attribute()` via C FFI     | Set widget property         |
| `SET_TEXT`          | `node.textContent = ...`      | `blitz_set_text_content()` via C FFI  | Set label text              |
| `NEW_EVENT_LISTENER`| `addEventListener()`          | `blitz_add_event_listener()` via C FFI| Register widget callback    |
| `APPEND_CHILDREN`  | `parent.appendChild()`        | `blitz_append_child()` via C FFI      | Add child widget            |
| `REMOVE`           | `node.remove()`               | `blitz_remove_node()` via C FFI       | Destroy widget              |

---

## Platform Abstraction Layer

The platform abstraction layer is **not optional** — it is a core architectural requirement that enables shared examples and write-once app code. It lives in `mojo-gui/core/src/platform/`.

### The `App` Trait

Every renderer implements the `App` trait, which provides the lifecycle contract between the framework and the platform:

```text
# core/src/platform/app.mojo

trait App:
    """Platform host that drives the reactive framework."""
    fn init(inout self, shell: AppShell) -> None
    fn flush_mutations(inout self, buffer: UnsafePointer[UInt8], len: Int) -> None
    fn poll_events(inout self) -> None
    fn request_redraw(inout self) -> None
    fn run(inout self) -> None
```

This trait is the **only** thing that differs between platforms. App code never sees it directly — it interacts only with `ComponentContext`, signals, and the HTML DSL.

### The `launch()` Function

The `launch()` function is the single entry point that all apps use. The renderer is selected at **compile time** based on the build target:

```text
# core/src/platform/launch.mojo

fn launch[app_builder: fn(ctx: ComponentContext) -> None]():
    """Launch the app on the current platform.

    - WASM target → web renderer (JS runtime drives the event loop)
    - Native target → desktop renderer (Blitz drives the event loop)
    """
    # Target selection happens at compile time.
    # For WASM builds, the web renderer's @export wrappers call into the app_builder.
    # For native builds, the desktop renderer creates a window and runs the event loop.
    ...
```

### How Apps Use It

A shared example app looks like this:

```text
# examples/counter/app.mojo

from mojo_gui.core.component import ComponentContext
from mojo_gui.core.html.dsl import el_div, el_h1, el_button, text, dyn_text, onclick_add
from mojo_gui.core.platform import launch

fn counter_app(ctx: ComponentContext):
    var count = ctx.use_signal(0)
    ctx.setup_view(
        el_div(
            el_h1(dyn_text()),
            el_button(text("+1"), onclick_add(count, 1)),
            el_button(text("-1"), onclick_add(count, -1)),
        ),
        String("counter"),
    )

fn main():
    launch[counter_app]()
```

**This exact code compiles and runs on every target:**

- `mojo build examples/counter/app.mojo --target wasm64-wasi -I core/src -I web/src` → WASM binary for browser
- `mojo build examples/counter/app.mojo -I core/src -I desktop/src --link-against libmojo_blitz.so` → native binary for desktop

The only difference is the build command. The source code is identical.

### What Each Renderer Provides

| Renderer   | Entry mechanism                      | Event loop driver                      |
|------------|--------------------------------------|----------------------------------------|
| **Web**    | JS runtime instantiates WASM, calls `@export` init | JS `requestAnimationFrame` + event listeners |
| **Desktop**| Native `main()` creates Blitz window | Winit event loop via Blitz C shim      |
| **Native** | Native `main()` creates platform window | Platform-specific event loop (GTK, Cocoa, etc.) |

---

## Renderer Strategies

### Web Renderer (existing — move to `mojo-gui/web/`)

**How it works today:**

1. Mojo compiles to WASM via `mojo build` → `llc` → `wasm-ld`
2. TypeScript runtime instantiates WASM, provides env imports
3. Mojo writes mutations to shared linear memory
4. JS `Interpreter` reads mutation buffer, applies to real DOM
5. JS `EventBridge` captures DOM events, dispatches to WASM

**Changes needed:**

- Implement `WebApp` in `web/src/web_launcher.mojo` conforming to the `App` trait
- `web/src/main.mojo` wires `@export` functions to the `App` trait methods
- Build scripts updated to compile shared examples from `examples/` for the WASM target
- Per-example `web/` subdirectories contain only HTML shell and JS glue (no app logic)

### Desktop Renderer (new — `mojo-gui/desktop/`)

Strategy: native HTML/CSS rendering via [Blitz](https://github.com/DioxusLabs/blitz). This is the same approach Dioxus uses for `dioxus-native`. Blitz is a radically modular HTML/CSS rendering engine that provides:

- **Stylo** (Firefox's CSS engine) — CSS parsing and style resolution
- **Taffy** — Flexbox, grid, and block layout
- **Parley** — Text layout and shaping
- **Vello** via **anyrender** — GPU-accelerated 2D rendering
- **Winit** — Cross-platform windowing and input
- **AccessKit** — Accessibility

Blitz provides a real DOM (`blitz-dom`) without requiring a browser or webview. The mutation protocol maps naturally to Blitz's DOM operations.

1. Mojo compiles to a **native binary** (no WASM)
2. The native binary links against a Blitz C shim (Rust `cdylib` exposing `blitz-dom` + `blitz-shell` via C ABI)
3. Mojo mutation interpreter reads the byte buffer and calls Blitz DOM operations via FFI (createElement, setAttribute, appendChild, etc.)
4. Blitz handles style resolution, layout, and GPU rendering
5. Winit/Blitz events flow back to Mojo via callback or polling

**Architecture:**

```text
┌──────────────────────────────────────────────────────────┐
│  Native Process                                           │
│                                                           │
│  ┌─────────────────────┐                                  │
│  │  mojo-gui/core       │                                  │
│  │  (compiled native)   │                                  │
│  │                      │─── mutation buffer ──┐           │
│  │  signals, vdom,      │                      │           │
│  │  diff, scheduler     │◄── event dispatch ──┐│           │
│  └─────────────────────┘                     ││           │
│                                              ▼│           │
│  ┌──────────────────────────────────────────┐ │           │
│  │  desktop/renderer.mojo                    │ │           │
│  │  (Mutation interpreter → Blitz FFI calls) │ │           │
│  └──────────┬───────────────────────────────┘ │           │
│             │ C FFI                            │           │
│  ┌──────────▼───────────────────────────────┐ │           │
│  │  Blitz (Rust cdylib via C shim)           │ │           │
│  │  ┌────────────────────────────────────┐   │ │           │
│  │  │  blitz-dom    — DOM tree + styles  │   │ │           │
│  │  │  Stylo        — CSS resolution     │   │ │           │
│  │  │  Taffy        — Layout engine      │   │ │           │
│  │  │  Vello        — GPU rendering      │   │ │           │
│  │  │  Winit        — Window + input ────│───┘ │           │
│  │  └────────────────────────────────────┘     │           │
│  └─────────────────────────────────────────────┘           │
└────────────────────────────────────────────────────────────┘
```

**Key difference from web:** The Mojo code runs as a native process (not WASM), and manipulates the Blitz DOM directly via C FFI instead of shared WASM linear memory + JS interpreter. There is no webview, no JS runtime, and no IPC — mutations are applied in-process.

**Adaptation needed in `mojo-gui/core`:**

- The `MutationWriter` currently writes to WASM linear memory (`UnsafePointer[UInt8, MutExternalOrigin]`). For native, it writes to a heap buffer. The writer itself doesn't care — it just writes bytes to a pointer. ✅ Already works.
- The desktop renderer implements a Mojo-side mutation interpreter that reads the byte buffer and translates each opcode to the corresponding Blitz C FFI call (similar to how the JS `Interpreter` class reads the buffer and calls DOM methods).

**Advantages of Blitz over a webview approach:**

- **No JS runtime** — no need to bundle or inject JavaScript; the mutation interpreter runs in Mojo
- **No IPC overhead** — mutations are applied in-process via direct FFI calls, not serialized over IPC
- **Smaller binary** — no browser engine dependency (WebKitGTK is ~50+ MB); Blitz is much lighter
- **Better integration** — native window chrome, system menus, accessibility via AccessKit
- **Consistent rendering** — Stylo (Firefox's CSS engine) provides standards-compliant CSS everywhere

### Native Renderer (future — `mojo-gui/native/`)

Strategy: direct platform widget mapping. A true native renderer that maps DOM-like mutations to platform-specific widgets (GTK, Cocoa, Win32) rather than rendering HTML/CSS:

- `LOAD_TEMPLATE` → create a widget subtree from a cached layout
- `SET_TEXT` → update a label/text widget
- `SET_ATTRIBUTE` → set widget properties (style, class → theme variants)
- `NEW_EVENT_LISTENER` → register widget callbacks

This requires platform-specific backends and a mapping from HTML semantics to native widget concepts. This is a larger effort than the Blitz-based desktop renderer, which stays in the HTML/CSS world.

---

## Phase 1: Extract `mojo-gui/core` Library

### Step 1.1 — Create `mojo-gui/core` directory structure

Create the new project skeleton. The reactive core, vdom, component framework, and **platform abstraction layer** become a standalone Mojo library.

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
| *(new)*                              | `src/platform/launch.mojo`           |
| *(new)*                              | `src/platform/app.mojo`              |
| *(new)*                              | `src/platform/features.mojo`         |
| `test/*.mojo` (Mojo-side tests)     | `test/*`                              |

**Files to move to shared examples:**

| From (`wasm-mojo/`)                  | To (`mojo-gui/examples/`)            |
|--------------------------------------|---------------------------------------|
| `src/apps/counter.mojo`             | `counter/app.mojo`                   |
| `src/apps/todo.mojo`                | `todo/app.mojo`                      |
| `src/apps/bench.mojo`               | `bench/app.mojo`                     |
| `src/apps/*.mojo` (others)          | `<name>/app.mojo`                    |

**Import path changes:**

| Old import                           | New import                            |
|--------------------------------------|---------------------------------------|
| `from vdom.tags import TAG_DIV, ...` | `from html.tags import TAG_DIV, ...`  |
| `from vdom.dsl import el_div, ...`   | `from html.dsl import el_div, ...`    |

The `vdom/dsl.mojo` module currently imports from `vdom.tags`, `vdom.template`, `vdom.vnode`, and `events.registry`. When moved to `html/dsl.mojo`, imports from `vdom.*` stay the same (sibling package), only `html.tags` changes.

### Step 1.2 — Introduce the Platform Abstraction Layer

Create `core/src/platform/` with the `App` trait and `launch()` function. This is the key enabler for shared examples.

**`core/src/platform/app.mojo`** — The trait every renderer must implement:

```text
trait App:
    fn init(inout self, shell: AppShell) -> None
    fn flush_mutations(inout self, buffer: UnsafePointer[UInt8], len: Int) -> None
    fn poll_events(inout self) -> None
    fn request_redraw(inout self) -> None
    fn run(inout self) -> None
```

**`core/src/platform/launch.mojo`** — Compile-time target dispatch:

```text
fn launch[app_builder: fn(ctx: ComponentContext) -> None]():
    """Launch the app. Renderer is selected by build target."""
    @parameter
    if _is_wasm_target():
        # Web path: the JS runtime calls @export functions.
        # launch() registers the app_builder for the JS runtime to invoke.
        _register_web_app[app_builder]()
    else:
        # Desktop path: create a Blitz window and run the event loop.
        _run_desktop_app[app_builder]()
```

**Why this matters for shared examples:** With `launch()`, every example app calls the same function. The compile target determines the renderer. No `#ifdef`, no renderer-specific imports in app code.

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

## Phase 2: Create `mojo-gui/web` (Browser Renderer) + Shared Examples

### Step 2.1 — Move web-specific files

| From (`wasm-mojo/`)                  | To (`mojo-gui/web/`)                 |
|--------------------------------------|---------------------------------------|
| `runtime/*`                          | `runtime/*`                           |
| `src/main.mojo`                      | `src/main.mojo`                       |
| `test-js/*`                          | `test-js/*`                           |
| `scripts/*`                          | `scripts/*`                           |
| `justfile`                           | `justfile`                            |
| `deno.json`, `deno.lock`            | `deno.json`, `deno.lock`             |
| `default.nix`                        | `default.nix`                         |

### Step 2.2 — Create `WebApp` implementing the `App` trait

Create `web/src/web_launcher.mojo` that implements the `App` trait for the WASM target:

```text
# web/src/web_launcher.mojo

from mojo_gui.core.platform.app import App

struct WebApp(App):
    """Browser renderer — mutations flow to JS Interpreter via shared memory."""

    fn init(inout self, shell: AppShell) -> None:
        # WASM linear memory is set up by the JS runtime.
        # The mutation buffer pointer is provided by the JS side.
        ...

    fn flush_mutations(inout self, buffer: UnsafePointer[UInt8], len: Int) -> None:
        # No-op for WASM — the JS runtime reads directly from shared memory.
        # Just signal the JS side that mutations are ready.
        ...

    fn poll_events(inout self) -> None:
        # No-op for WASM — the JS EventBridge dispatches events via @export calls.
        ...

    fn request_redraw(inout self) -> None:
        # No-op for WASM — JS uses requestAnimationFrame.
        ...

    fn run(inout self) -> None:
        # No-op for WASM — the JS runtime drives the event loop.
        ...
```

### Step 2.3 — Wire `main.mojo` to import from `mojo-gui/core`

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

### Step 2.4 — Set up shared example build for web

Move web-specific example assets (HTML shells, JS glue) from `wasm-mojo/examples/` to `mojo-gui/examples/<name>/web/`, while the app logic lives in `mojo-gui/examples/<name>/app.mojo`.

Create `web/scripts/build_examples.sh` that builds **all** shared examples for the web target:

```text
#!/bin/bash
# Build all shared examples for the web target
for example_dir in ../examples/*/; do
    name=$(basename "$example_dir")
    if [ -f "$example_dir/app.mojo" ]; then
        echo "Building $name for web..."
        mojo build "$example_dir/app.mojo" \
            --target wasm64-wasi \
            -I ../core/src \
            -I ../web/src \
            -o "dist/$name.wasm"
    fi
done
```

Each example's `web/` subdirectory contains only:
- `index.html` — HTML shell with a `<div id="app">` mount point
- `main.ts` — JS glue that loads the WASM module and connects the runtime

These are **not app code** — they are renderer infrastructure. The same `app.mojo` is used for every target.

### Step 2.5 — Verify the existing test suite passes

After the file moves:

1. All 1,323 Mojo tests pass (compiled via wasmtime)
2. All 3,090 JS tests pass (compiled via Deno)
3. All three shared example apps work in the browser (built from `examples/`)

### Step 2.6 — Extract `main.mojo` WASM exports into generated boilerplate

Currently `main.mojo` is ~6,730 lines of `@export` wrappers. Many of these are mechanical (create app, destroy app, init, rebuild, flush, dispatch_event × N apps). Consider generating these from a manifest to make adding new apps easier. With the shared example model, each example's `@export` surface is identical — only the `app_builder` function pointer differs.

---

## Phase 3: Create `mojo-gui/desktop` (Desktop Renderer)

### Step 3.1 — Design the desktop architecture

**Blitz approach** (like Dioxus Native / `dioxus-native`):

```text
┌─ Native Mojo Process ─────────────────────────────────┐
│                                                        │
│  examples/counter/app.mojo (SAME code as web!)         │
│      │  calls launch[counter_app]()                    │
│      ▼                                                 │
│  mojo-gui/core (reactive framework + platform layer)   │
│      │ detects native target → DesktopApp              │
│      │ writes mutations to buffer                      │
│      ▼                                                 │
│  desktop/renderer.mojo                                 │
│      │ reads mutation opcodes from buffer               │
│      │ translates to Blitz C FFI calls                  │
│      ▼                                                 │
│  ┌─ Blitz (Rust cdylib) ───────────────────────┐      │
│  │  blitz-dom    — DOM tree, style resolution   │      │
│  │  Stylo/Taffy  — CSS + layout                 │      │
│  │  Vello        — GPU-accelerated rendering    │      │
│  │  Winit        — window management + input    │      │
│  │                                              │      │
│  │  Events (click, input, etc.) ────────────────│──┐   │
│  └──────────────────────────────────────────────┘  │   │
│                                                    │   │
│  desktop/events.mojo ◄─────────────────────────────┘   │
│      │ routes to HandlerRegistry.dispatch()             │
│      ▼                                                 │
│  mojo-gui/core (re-renders, produces new mutations)    │
└────────────────────────────────────────────────────────┘
```

### Step 3.2 — Implement `DesktopApp` conforming to the `App` trait

Create `desktop/src/desktop_launcher.mojo`:

```text
# desktop/src/desktop_launcher.mojo

from mojo_gui.core.platform.app import App

struct DesktopApp(App):
    """Desktop renderer — mutations flow to Blitz via C FFI."""
    var title: String
    var width: Int
    var height: Int

    fn init(inout self, shell: AppShell) -> None:
        mblitz_init()
        mblitz_create_window(self.title, self.width, self.height)

    fn flush_mutations(inout self, buffer: UnsafePointer[UInt8], len: Int) -> None:
        # Read opcodes from buffer, call Blitz C FFI for each
        interpret_mutations(buffer, len)
        mblitz_request_redraw()

    fn poll_events(inout self) -> None:
        # Poll Blitz/Winit events and dispatch to HandlerRegistry
        while mblitz_poll_event(event_buf):
            route_event_to_handler(event_buf)

    fn request_redraw(inout self) -> None:
        mblitz_request_redraw()

    fn run(inout self) -> None:
        # Main event loop — cooperative with Blitz
        while not self.should_quit:
            self.poll_events()
            # core framework processes dirty scopes, writes mutations
            self.shell.process()
            self.flush_mutations(self.shell.mutation_buffer(), self.shell.mutation_len())
            mblitz_tick()
```

### Step 3.3 — Implement Blitz C shim and FFI

The Mojo native binary needs to interact with [Blitz](https://github.com/DioxusLabs/blitz) via a C-compatible API. Blitz is written in Rust, so we build a thin Rust `cdylib` that exposes the necessary `blitz-dom` and `blitz-shell` functionality via `extern "C"` functions.

**C shim API surface (`shim/mojo_blitz.h`):**

| Category   | Functions                                                    |
|------------|--------------------------------------------------------------|
| Lifecycle  | `mblitz_init()`, `mblitz_shutdown()`, `mblitz_tick()`       |
| Window     | `mblitz_create_window(title, w, h)`, `mblitz_request_redraw()` |
| DOM        | `mblitz_create_element(tag)`, `mblitz_create_text(text)`    |
| DOM        | `mblitz_set_attribute(node, name, value)`, `mblitz_remove_attribute(node, name)` |
| DOM        | `mblitz_set_text_content(node, text)`                        |
| DOM        | `mblitz_append_child(parent, child)`, `mblitz_insert_before(parent, child, ref)` |
| DOM        | `mblitz_remove_node(node)`, `mblitz_clone_node(node, deep)` |
| Events     | `mblitz_add_event_listener(node, event_type)`, `mblitz_poll_event(out_event)` |
| Templates  | `mblitz_register_template(id, html)`, `mblitz_clone_template(id)` |

**Rust shim (`shim/mojo_blitz.rs`):**

The Rust side wraps `blitz-dom`'s `Document`, `Node`, and related types. It maintains:
- A `Document` instance (Blitz's DOM tree)
- A node handle table (`HashMap<u32, NodeId>`) mapping integer IDs to Blitz node IDs
- An event queue that collects Winit/Blitz events for Mojo to poll

**Mojo FFI (`src/desktop/blitz.mojo`):**

Loads the `cdylib` via `OwnedDLHandle` and exposes typed Mojo wrappers around the C functions.

### Step 3.4 — Implement the mutation interpreter

Unlike the web renderer (where a JS `Interpreter` class reads the mutation buffer), the desktop renderer implements the interpreter **in Mojo** (`desktop/renderer.mojo`):

**Mutations (Mojo → Blitz):**

1. Mojo writes mutations to a byte buffer (same as WASM)
2. `desktop/renderer.mojo` reads opcodes from the buffer sequentially
3. For each opcode, calls the corresponding Blitz C FFI function:
   - `LOAD_TEMPLATE` → `mblitz_clone_template(id)` → push to stack
   - `SET_ATTRIBUTE` → `mblitz_set_attribute(node, name, value)`
   - `SET_TEXT` → `mblitz_set_text_content(node, text)`
   - `APPEND_CHILDREN` → `mblitz_append_child(parent, child)`
   - `NEW_EVENT_LISTENER` → `mblitz_add_event_listener(node, type)`
   - `REMOVE` → `mblitz_remove_node(node)`
4. After all mutations are applied, calls `mblitz_request_redraw()` to trigger a re-layout and repaint

This is a direct port of the JS `Interpreter` class logic into Mojo, replacing DOM API calls with Blitz C FFI calls.

**Events (Blitz → Mojo):**

1. Blitz/Winit captures user input events (click, keypress, etc.)
2. The C shim queues events in a ring buffer
3. `desktop/events.mojo` polls via `mblitz_poll_event()` each tick
4. Routes to `HandlerRegistry.dispatch()` in `mojo-gui/core`

### Step 3.5 — Build shared examples for desktop

Create `desktop/scripts/build_examples.sh` that builds **all** shared examples for the native target:

```text
#!/bin/bash
# Build all shared examples for the desktop target
for example_dir in ../examples/*/; do
    name=$(basename "$example_dir")
    if [ -f "$example_dir/app.mojo" ]; then
        echo "Building $name for desktop..."
        mojo build "$example_dir/app.mojo" \
            -I ../core/src \
            -I ../desktop/src \
            --link-against libmojo_blitz.so \
            -o "dist/$name"
    fi
done
```

**Critical validation:** Every example that works on web MUST work on desktop. If an example fails on desktop, it is a framework bug — not an app bug. The app code is the same.

### Step 3.6 — Cross-target example test matrix

Establish an automated test matrix that verifies all shared examples on all targets:

| Example   | Web (WASM + browser) | Desktop (native + Blitz) | Status |
|-----------|---------------------|--------------------------|--------|
| counter   | ✅                  | 🔲                       | Phase 3 |
| todo      | ✅                  | 🔲                       | Phase 3 |
| bench     | ✅                  | 🔲                       | Phase 3 |

The CI pipeline should:
1. Build each shared example for each target
2. Run integration tests for each target (browser tests via Deno, desktop tests via headless Blitz or screenshot comparison)
3. Fail if any example works on one target but not another

---

## Phase 4: Native Renderer (Future)

Like Dioxus's future native widget renderer, this maps DOM-oriented mutations to platform-specific widgets. The shared examples continue to work unchanged — `launch()` dispatches to the native renderer when compiled with `--feature native`.

**Compile targets (complete picture):**

- `mojo build --target wasm64-wasi` → web renderer (needs `mojo-gui/web` JS runtime)
- `mojo build` → desktop renderer (Blitz native HTML/CSS engine, no WASM)
- `mojo build --feature native` → native renderer (platform widgets, future)

---

## Dependency Graph

```text
                    ┌──────────────┐
                    │  Shared      │
                    │  Examples    │
                    │ (examples/)  │
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
              │ platform/│ ◄── App trait + launch()
              └────┬─────┘
                   │ implements App trait
              ┌────┼────────────┐
              ▼    ▼            ▼
     ┌──────────┐ ┌──────────┐ ┌──────────┐
     │ mojo-gui │ │ mojo-gui │ │ mojo-gui │
     │ /web     │ │ /desktop │ │ /native  │
     │          │ │          │ │ (future) │
     │ WebApp   │ │DesktopApp│ │          │
     │ main.mojo│ │ Blitz FFI│ │ NativeApp│
     │ runtime/ │ │ renderer │ │ widget   │
     └──────────┘ └──────────┘ └──────────┘
```

Key points:
- **Examples depend only on `core`** — they never import from `web/`, `desktop/`, or `native/`
- **Renderers implement the `App` trait** defined in `core/platform/`
- **`launch()` is the only platform-dispatching call** — it routes to the correct renderer at compile time
- **`mojo-web` is independent** — apps can optionally import it for non-rendering browser features

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
- [x] Create `core/src/platform/app.mojo` — `PlatformApp` trait definition (with `init`, `flush_mutations`, `request_animation_frame`, `should_quit`, `destroy`) + `is_wasm_target()` / `is_native_target()` helpers
- [x] Create `core/src/platform/launch.mojo` — `launch()` with `AppConfig` (title, width, height, debug), global config registry, `get_launch_config()` / `has_launched()`
- [x] Create `core/src/platform/features.mojo` — `PlatformFeatures` struct, preset feature sets (`web_features`, `desktop_webview_features`, `desktop_blitz_features`, `native_features`), global feature registry (`register_features` / `current_features`)
- [x] Create `core/src/platform/__init__.mojo` — re-exports public API from all three platform modules
- [x] Update `core/src/lib.mojo` — add `platform/` to package listing
- [ ] Move `src/apps/` to `mojo-gui/examples/` as shared, platform-agnostic example apps
- [ ] Refactor each example app to use `launch[app_builder]()` instead of renderer-specific entry points
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
- [x] Create `web/src/web_launcher.mojo` — `WebApp` implementing the `PlatformApp` trait (no-op stubs for WASM target where JS runtime drives the loop) + `create_web_app()` helper
- [x] Move web-specific example assets (HTML, JS glue) from `examples/` to `examples/<name>/web/` — counter, todo, bench HTML shells and main.js entry points copied to `mojo-gui/examples/<name>/web/`
- [x] Create `web/scripts/build_examples.sh` — builds all shared examples for WASM target (discovers examples, compiles shared WASM binary via main.mojo, copies per-example HTML/JS assets from both shared and web-specific locations)
- [ ] Verify shared examples build and run in browser via web target
- [x] Move `test-js/` to `mojo-gui/web/test-js/`
- [x] Move `scripts/` to `mojo-gui/web/scripts/`
- [x] Move build files (`justfile`, `deno.json`, `default.nix`) — updated `justfile` with `-I ../core/src` for core package resolution
- [x] Update all import paths in moved files
- [x] Verify all 3,090 JS tests pass
- [x] Verify all 3 example apps work in browser — JS tests (3,090) and Mojo tests (52 suites) pass; browser verification blocked by headless Servo in CI
- [x] Write `mojo-gui/web/README.md`
- [x] Write `mojo-gui/examples/README.md` — build instructions for web/desktop/Blitz targets, directory structure, migration status, architecture reference

### Phase 3: `mojo-gui/desktop` (new development — Blitz)

- [ ] Build Blitz C shim (`shim/mojo_blitz.rs`) — Rust `cdylib` exposing `blitz-dom`, `blitz-shell`, and `blitz-renderer-vello` via `extern "C"` functions
- [ ] Write C header (`shim/mojo_blitz.h`) — DOM operations, window lifecycle, event polling
- [ ] Implement Mojo FFI bindings (`src/desktop/blitz.mojo`) — typed wrappers via `OwnedDLHandle`
- [ ] Implement `DesktopApp` (`src/desktop/desktop_launcher.mojo`) — implements `App` trait, drives Blitz event loop
- [ ] Implement Mojo-side mutation interpreter (`src/desktop/renderer.mojo`) — reads opcode buffer, calls Blitz C FFI (port of JS `Interpreter` logic to Mojo)
- [ ] Implement event bridge (`src/desktop/events.mojo`) — poll Blitz/Winit events via `mblitz_poll_event()`, route to `HandlerRegistry.dispatch()`
- [ ] Create `desktop/scripts/build_examples.sh` — builds all shared examples for native target
- [ ] Verify counter example runs on desktop (same `examples/counter/app.mojo` as web)
- [ ] Verify todo example runs on desktop (same `examples/todo/app.mojo` as web)
- [ ] Verify bench example runs on desktop (same `examples/bench/app.mojo` as web)
- [ ] Set up cross-target CI test matrix (web + desktop for every shared example)
- [ ] Write `mojo-gui/desktop/README.md`

---

## Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Mojo package system immaturity | Can't cleanly separate into packages | Mono-repo with path-based imports; symlinks for dev |
| `MutExternalOrigin` tied to WASM | Core won't compile natively | Audit and abstract the origin parameter; conditionally compile |
| Blitz C shim complexity | Desktop renderer takes too long | Start with Linux; Blitz already supports Linux/macOS/Windows via Winit |
| Blitz pre-alpha stability | Rendering bugs, missing CSS features | Track Blitz main branch; contribute upstream fixes; fall back to web renderer for complex UIs |
| Blitz Rust build dependency | Complex build toolchain | Pre-build the `cdylib` and distribute as a shared library; Nix flake can automate the Rust build |
| Import path breakage | Massive search-and-replace | Script the migration; grep-verify all imports |
| Test suite fragmentation | Tests break across projects | Phase 1 must keep all Mojo tests green; Phase 2 must keep all JS tests green |
| Platform abstraction too leaky | Shared examples break on some targets | Use the cross-target test matrix as a gate; treat cross-target failures as framework bugs |
| `launch()` compile-time dispatch limitations | Mojo may lack the metaprogramming for clean target dispatch | Fall back to separate `main_web.mojo` / `main_desktop.mojo` thin wrappers that both call the same `app_builder`; app code stays shared even if the entry point file differs |

---

## Estimated Effort

| Phase | Effort | Description |
|-------|--------|-------------|
| Phase 1 | 2–3 days | File moves, import path updates, platform abstraction layer, shared examples setup, verify compilation + tests |
| Phase 2 | 1–2 days | Move web runtime, `WebApp` trait impl, shared example web builds, verify browser tests |
| Phase 3 | 2–4 weeks | Blitz C shim, Mojo FFI, `DesktopApp` trait impl, mutation interpreter, desktop entry point, verify all shared examples on desktop |
| Phase 4 | TBD | Native renderer (future, depends on widget mapping complexity) |
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

**Important for shared examples:** If an example needs a web-only feature (e.g., `fetch`), it should use `mojo-web` behind a feature gate or platform check so the example still compiles on non-web targets. For most GUI examples (counter, todo, bench), no web-specific APIs are needed — they work identically on all targets.

```text
┌────────────────────────────────────────────────┐
│  Shared Example App                             │
│                                                 │
│  GUI rendering:     Non-rendering web features: │
│  mojo-gui/core      mojo-web (optional,         │
│  (mutation protocol) gated on web target)       │
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

1. **Mono-repo vs. multi-repo?** — Mono-repo is the natural fit: `mojo-gui/` is the workspace root containing `core/`, `web/`, `desktop/`, `native/`, and `examples/`. `mojo-web` could live alongside as a sibling or in a separate repo. Safer to keep together until Mojo has a package manager. Can split later.

2. **Should `html/` stay in `mojo-gui/core` or become a separate `mojo-gui/html` package?** — Keep in `core` for now. A native renderer that doesn't use HTML elements would need a different DSL (e.g., `el_box()`, `el_label()`), but that's Phase 4+ territory.

3. **How to handle the `@export` boilerplate in `main.mojo`?** — Consider a code generator that reads app definitions and emits WASM/native entry points. With the `App` trait and `launch()`, the boilerplate should be much smaller — each example only needs to call `launch[app_builder]()`.

4. **Blitz C shim API granularity?** — Start with a minimal API covering the mutation opcodes + window lifecycle + event polling. Expand as needed. Consider whether to expose Blitz's `Document` directly or maintain an opaque handle table in the shim.

5. **Should the Mojo-side mutation interpreter share code with the JS `Interpreter`?** — The logic is the same (stack machine reading opcodes), but the implementations are in different languages. Keep them as parallel implementations with shared test vectors to verify correctness.

6. **Should `mojo-web` reuse `mojo-gui/web`'s existing JS runtime code?** — Partially. `memory.ts`, `env.ts`, and `strings.ts` solve the same WASM↔JS interop problems. Extract a shared `mojo-wasm-runtime` base, or let `mojo-web` depend on just those modules.

7. **Should `mojo-gui/web` eventually use `mojo-web` for its JS runtime?** — Possibly for non-rendering parts (e.g., the `EventBridge` could use `mojo-web`'s DOM bindings). The mutation protocol interpreter should stay as-is for performance (batched application vs. per-call overhead).

8. **Blitz version pinning?** — Blitz is currently pre-alpha. Pin to a specific git commit in the Rust shim's `Cargo.toml` and update deliberately. Track the [Blitz roadmap](https://github.com/DioxusLabs/blitz) for stability milestones.

9. **CSS support scope?** — Blitz supports modern CSS (flexbox, grid, selectors, variables, media queries) via Stylo, but not all CSS features are implemented yet. Document which CSS features are supported and test the desktop renderer against the same shared examples as the web renderer.

10. **Fallback for `launch()` compile-time dispatch?** — If Mojo's metaprogramming isn't mature enough for clean `@parameter if` target detection, fall back to separate thin entry-point files (`main_web.mojo`, `main_desktop.mojo`) that both import and call the same shared `app_builder` function. The app code is still shared — only the 3-line entry point differs per target. This is strictly a build-system concern, not an app-authoring concern.

11. **How to handle web-only features in shared examples?** — Examples that need web-specific APIs (e.g., `fetch`, `localStorage`) should use compile-time feature gates: `@parameter if _is_wasm_target(): ...`. For most GUI examples, this isn't needed — they use only signals, components, and the HTML DSL, all of which are platform-agnostic.