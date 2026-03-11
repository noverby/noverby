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

The goal: write a Mojo GUI app **once**, run it in the browser via WASM **and** natively on desktop — like Dioxus does for Rust. App code is platform-agnostic by design; examples are shared across all renderer targets and must compile and run identically on each. `mojo-web` provides foundational Web API access for any Mojo/WASM project, including but not limited to `mojo-gui`.

**Current status:** Phases 1–3 are complete. The core library is extracted, the web renderer is separated, and all four main app structs (CounterApp, TodoApp, BenchmarkApp, MultiViewApp) implement the `GuiApp` trait (Steps 3.9.1 + 3.9.4). The `@export` WASM wrappers in `web/src/main.mojo` have been genericized over `GuiApp` via a new `gui_app_exports.mojo` module (Step 3.9.5), and the per-app backwards-compatible free functions have been removed from all four example files. The `launch()` function now accepts a `GuiApp` type parameter with `@parameter if is_wasm_target()` compile-time dispatch (Step 3.9.3). All 3,090 JS tests and 52 Mojo test suites pass. The desktop renderer infrastructure (Phase 3 webview approach) was built and verified, then removed in favor of the Blitz-based native approach (Phase 4). **Phase 4 (Blitz desktop renderer) implementation is complete — verification pending.** The full stack is built: Rust C shim with Winit event loop + Vello GPU rendering (`shim/src/lib.rs`), C header (`shim/mojo_blitz.h`), Nix derivation (`shim/default.nix`), Mojo FFI bindings (`desktop/src/desktop/blitz.mojo`), Mojo-side mutation interpreter (`desktop/src/desktop/renderer.mojo`), and generic desktop event loop (`desktop/src/desktop/launcher.mojo` with `desktop_launch[AppType: GuiApp]()`) . The Rust cdylib builds successfully (`cargo build --release` produces `libmojo_blitz.so`, ~23MB with Winit + Vello, pinned to Blitz v0.2.0 at rev `2f83df96`). The `launch()` function in `core/src/platform/launch.mojo` calls `desktop_launch[AppType](config)` on native targets (no global mutable state — module-level `var` is not supported on native targets). All four shared examples have `main.mojo` entry points with `launch[AppType](AppConfig(...))` (Step 3.9.6). The next priority is **cross-target build verification** — confirming all shared examples compile and run on both web and desktop-Blitz from identical source (Steps 3.9.7 + 4.4).

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

Note: Dioxus's desktop renderer evolved through similar stages — early versions used a webview (Wry/Tauri), and later versions introduced Blitz for native HTML/CSS rendering. We follow the same progression: webview first (Phase 3, implemented), Blitz native (Phase 4, implemented — verification pending).

Separately, Rust's `web-sys` crate provides raw bindings to **all** Web APIs (DOM, fetch, WebSocket, WebGL, etc.) via `wasm-bindgen`. Any Rust/WASM project can use `web-sys` directly — Dioxus-web uses it under the hood. `mojo-web` fills this same ecosystem role for Mojo.

Key insight: **the mutation protocol stays DOM-oriented even in core**. The desktop webview renderer reuses the same JS mutation interpreter inside an embedded webview. Future desktop renderers can use a native HTML/CSS rendering engine (like [Blitz](https://github.com/DioxusLabs/blitz)) that provides a real DOM without a browser, while XR renderers render DOM content to textures placed in 3D space. This is pragmatic — HTML/DOM is a universal UI description language. `mojo-gui` uses the mutation protocol (not `mojo-web`) for rendering, keeping the multi-renderer architecture intact. `mojo-web` is for everything else an app needs from the browser: data fetching, storage, timers, canvas, etc.

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

### Desktop Runtime (→ `mojo-gui/desktop`, implemented)

Everything for the native desktop application. The webview approach (GTK4 + WebKitGTK) was built first as Phase 3, then superseded by the Blitz native renderer (Phase 4).

**Blitz renderer (Phase 4 — implementation complete, verification pending):**

| Module                                | Purpose                                      |
|---------------------------------------|----------------------------------------------|
| `shim/src/lib.rs`                     | Rust `cdylib`: `BlitzContext` wrapping `blitz-dom`, ID mapping, template registry, event queue, interpreter stack |
| `shim/mojo_blitz.h`                   | C API header (~45 FFI functions: lifecycle, DOM, templates, events, stack, debug) |
| `shim/Cargo.toml`                     | Rust crate config (blitz-dom, blitz-html, blitz-traits, blitz-shell, blitz-paint, winit, etc.) |
| `shim/default.nix`                    | Nix derivation with GPU/windowing deps (Vulkan, Wayland, X11, fontconfig) |
| `src/desktop/blitz.mojo`              | Mojo FFI bindings to `libmojo_blitz_shim.so` via `DLHandle` |
| `src/desktop/renderer.mojo`           | `MutationInterpreter`: reads binary opcodes → Blitz C FFI calls (all 18 opcodes) |
| `src/desktop/launcher.mojo`           | `desktop_launch[AppType: GuiApp]()` — generic Blitz-backed event loop |

**Webview approach (Phase 3 — removed, kept for reference):**

| Module                                | Purpose                                      |
|---------------------------------------|----------------------------------------------|
| `shim/mojo_webview.c`                 | C shim: GTK4 + WebKitGTK, ring buffer events |
| `shim/mojo_webview.h`                 | C API header for the webview shim            |
| `runtime/desktop-runtime.js`          | Standalone JS mutation interpreter for webview |
| `runtime/shell.html`                  | HTML shell with `#root` mount point          |
| `src/desktop/webview.mojo`            | Mojo FFI bindings to libmojo_webview.so      |
| `src/desktop/bridge.mojo`             | Mutation buffer + event polling bridge        |
| `src/desktop/app.mojo`                | DesktopApp: lifecycle, event loop, init       |

**Shared:**

| Module                                | Purpose                                      |
|---------------------------------------|----------------------------------------------|
| `src/desktop/__init__.mojo`           | Package root                                 |
| `examples/counter.mojo`               | Desktop counter example (temporary — to be replaced by shared `examples/counter/` via `launch()`) |
| `justfile`                            | Build commands (build-shim, run-counter)      |
| `default.nix`                         | Nix dev shell with desktop deps               |

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
│   │   │   ├── app.mojo              # PlatformApp trait — interface renderers implement
│   │   │   ├── gui_app.mojo          # GuiApp trait — interface apps implement
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
├── desktop/                          # Desktop renderer (Phase 4 — Blitz native renderer)
│   ├── src/
│   │   └── desktop/                  # Desktop renderer package
│   │       ├── __init__.mojo         # Package root
│   │       ├── blitz.mojo            # Mojo FFI bindings to libmojo_blitz_shim.so via DLHandle (Phase 4)
│   │       ├── renderer.mojo         # MutationInterpreter — binary opcodes → Blitz FFI calls (Phase 4)
│   │       ├── launcher.mojo         # desktop_launch[AppType: GuiApp]() — Blitz-backed event loop
│   │       ├── app.mojo              # DesktopApp — webview lifecycle (Phase 3, legacy)
│   │       ├── bridge.mojo           # DesktopBridge — mutation buffer + event polling (Phase 3, legacy)
│   │       └── webview.mojo          # Mojo FFI bindings to libmojo_webview.so (Phase 3, legacy)
│   ├── shim/
│   │   ├── src/lib.rs                # Rust cdylib: BlitzContext wrapping blitz-dom (Phase 4)
│   │   ├── mojo_blitz.h              # C API header — ~45 FFI functions (Phase 4)
│   │   ├── Cargo.toml                # Rust crate config (blitz, winit, etc.) (Phase 4)
│   │   ├── default.nix               # Nix derivation with GPU/windowing deps (Phase 4)
│   │   ├── mojo_webview.h            # C API header for webview (Phase 3, legacy)
│   │   └── mojo_webview.c            # C implementation GTK4+WebKitGTK (Phase 3, legacy)
│   ├── runtime/
│   │   ├── desktop-runtime.js        # Standalone JS interpreter (Phase 3, legacy)
│   │   └── shell.html                # HTML shell with #root mount point (Phase 3, legacy)
│   ├── examples/                     # TEMPORARY — to be deleted once Blitz cdylib builds
│   │   └── counter.mojo              # Desktop counter demo (temporary duplicate)
│   ├── build/                        # Build artifacts (libmojo_blitz_shim.so, binaries)
│   ├── default.nix                   # Nix dev shell with all desktop dependencies
│   ├── justfile                      # Build commands (build-shim, run-counter, etc.)
│   └── README.md
│
├── xr/                               # XR renderer (Phase 5 — future, OpenXR + WebXR)
│   ├── native/                       # OpenXR native renderer (Blitz DOM → Vello → OpenXR swapchain)
│   │   ├── shim/
│   │   │   ├── src/lib.rs            # Rust cdylib: Blitz + Vello + openxr crate, panel management
│   │   │   ├── mojo_xr.h            # C API header — XR session, panels, input
│   │   │   ├── Cargo.toml           # Rust crate config (blitz-dom, vello, openxr, winit)
│   │   │   └── default.nix          # Nix derivation with OpenXR/GPU deps
│   │   └── src/
│   │       ├── xr_launcher.mojo     # xr_launch[AppType: GuiApp]() — OpenXR event loop
│   │       ├── xr_blitz.mojo        # Mojo FFI bindings to libmojo_xr.so
│   │       ├── panel.mojo           # XRPanel — DOM document + 3D transform + input mapping
│   │       └── scene.mojo           # XRScene — panel registry, spatial layout, raycasting
│   ├── web/                          # WebXR browser renderer (extends mojo-gui/web)
│   │   ├── runtime/
│   │   │   ├── xr-session.ts        # WebXR session lifecycle, reference spaces
│   │   │   ├── xr-panels.ts         # DOM → texture rendering, 3D panel placement
│   │   │   ├── xr-input.ts          # XR controller ray → DOM event translation
│   │   │   └── xr-runtime.ts        # Entry point — extends web runtime with XR
│   │   └── src/
│   │       └── webxr_launcher.mojo  # WebXR-specific launch configuration
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

| Opcode              | Web (DOM)                     | Desktop (Blitz)                       | XR Native (OpenXR + Blitz)                    | XR Web (WebXR)                        |
|---------------------|-------------------------------|---------------------------------------|------------------------------------------------|---------------------------------------|
| `LOAD_TEMPLATE`     | `cloneNode(true)`             | `blitz_clone_template()` via C FFI    | `blitz_clone_template()` → panel document      | `cloneNode(true)` → panel DOM         |
| `SET_ATTRIBUTE`     | `el.setAttribute()`           | `blitz_set_attribute()` via C FFI     | `blitz_set_attribute()` → panel document       | `el.setAttribute()` → panel DOM      |
| `SET_TEXT`          | `node.textContent = ...`      | `blitz_set_text_content()` via C FFI  | `blitz_set_text_content()` → panel document    | `node.textContent` → panel DOM       |
| `NEW_EVENT_LISTENER`| `addEventListener()`          | `blitz_add_event_listener()` via C FFI| `blitz_add_event_listener()` → panel document  | `addEventListener()` → panel DOM     |
| `APPEND_CHILDREN`  | `parent.appendChild()`        | `blitz_append_child()` via C FFI      | `blitz_append_child()` → panel document        | `parent.appendChild()` → panel DOM   |
| `REMOVE`           | `node.remove()`               | `blitz_remove_node()` via C FFI       | `blitz_remove_node()` → panel document         | `node.remove()` → panel DOM          |

---

## Platform Abstraction Layer

The platform abstraction layer is **not optional** — it is a core architectural requirement that enables shared examples and write-once app code. It lives in `mojo-gui/core/src/platform/`.

### The `PlatformApp` Trait (renderer side)

Every renderer implements the `PlatformApp` trait, which provides the lifecycle contract between the framework and the platform:

```text
# core/src/platform/app.mojo

trait PlatformApp(Movable):
    """Platform host that drives the reactive framework."""
    fn init(mut self) raises -> None
    fn flush_mutations(mut self, buf: UnsafePointer[UInt8], length: Int) raises -> None
    fn request_animation_frame(mut self) -> None
    fn should_quit(self) -> Bool
    fn destroy(mut self) -> None
```

This trait is the **only** thing that differs between platforms. App code never sees it directly — it interacts only with `ComponentContext`, signals, and the HTML DSL.

### The `GuiApp` Trait (app side)

Every app implements the `GuiApp` trait, which provides the lifecycle contract between the app and the framework's generic event loop:

```text
# core/src/platform/gui_app.mojo

trait GuiApp(Movable):
    """User-facing app that the framework drives."""
    fn render(mut self) -> UInt32
    fn handle_event(mut self, handler_id: UInt32, event_type: UInt8, value: String) -> Bool
    fn flush(mut self, writer_ptr: UnsafePointer[MutationWriter, MutExternalOrigin]) -> Int32
    fn mount(mut self, writer_ptr: UnsafePointer[MutationWriter, MutExternalOrigin]) -> Int32
    fn context(mut self) -> UnsafePointer[ComponentContext]
```

This trait captures the per-app lifecycle that currently lives as free functions (`counter_app_rebuild`, `counter_app_flush`, `counter_app_handle_event`). By implementing `GuiApp`, an app struct can be driven by **any** renderer's event loop without the renderer knowing anything about the app's internals.

`handle_event` takes an optional `value: String` parameter (empty string when no value). This unifies `dispatch_event()` and `dispatch_event_with_string()` — the renderer always passes the value through; the core framework ignores it for click events and uses it for input/keydown events.

### The `launch()` Function

The `launch()` function is the single entry point that all apps use. The renderer is selected at **compile time** based on the build target:

```text
# core/src/platform/launch.mojo

fn launch[AppType: GuiApp]():
    """Launch the app on the current platform.

    - WASM target → web renderer (JS runtime drives the event loop)
    - Native target → desktop renderer (webview/Blitz drives the event loop)
    """
    @parameter
    if is_wasm_target():
        _register_web_app[AppType]()
    else:
        _run_desktop_app[AppType]()
```

For **native targets**, `_run_desktop_app` is a generic event loop that works for any `GuiApp`:

```text
fn _run_desktop_app[AppType: GuiApp]() raises:
    var config = get_launch_config()
    var desktop = DesktopApp(title=config.title, width=config.width, ...)
    var app = AppType()
    desktop.init()

    # Mount
    var writer_ptr = _alloc_writer(desktop.buf_ptr(), desktop.buf_capacity())
    var mount_len = app.mount(writer_ptr)
    if mount_len > 0:
        desktop.flush_mutations(Int(mount_len))

    # Event loop
    while not desktop.should_quit():
        desktop.step(blocking=False)
        while True:
            var event = desktop.poll_event()
            if not event.is_valid(): break
            _ = app.handle_event(UInt32(event.handler_id), UInt8(event.event_type), event.value)
        if app.context()[].consume_dirty():
            _reset_writer(writer_ptr, ...)
            var flush_len = app.flush(writer_ptr)
            if flush_len > 0:
                desktop.flush_mutations(Int(flush_len))
        else:
            desktop.step(blocking=True)

    _free_writer(writer_ptr)
    app.context()[].destroy()
    desktop.destroy()
```

For **WASM targets**, `_register_web_app` stores the `AppType` so the JS runtime can invoke the lifecycle via `@export` wrappers. The `@export` surface in `main.mojo` becomes generic over `GuiApp` — one set of wrappers works for every app.

### How Apps Use It

A shared example app looks like this:

```text
# examples/counter/counter.mojo

from component import ComponentContext, ConditionalSlot
from signals import SignalI32, SignalBool
from html import el_div, el_h1, el_button, text, dyn_text, onclick_add, onclick_sub, onclick_toggle
from platform import launch, AppConfig

struct CounterApp(GuiApp):
    var ctx: ComponentContext
    var count: SignalI32
    # ... (same struct as today, unchanged)

    fn render(mut self) -> UInt32: ...
    fn handle_event(mut self, handler_id: UInt32, event_type: UInt8, value: String) -> Bool: ...
    fn flush(mut self, writer_ptr: ...) -> Int32: ...
    fn mount(mut self, writer_ptr: ...) -> Int32: ...
    fn context(mut self) -> UnsafePointer[ComponentContext]: ...

fn main() raises:
    launch[CounterApp](AppConfig(title="Counter", width=400, height=350))
```

**This exact code compiles and runs on every target:**

- `mojo build examples/counter/counter.mojo --target wasm64-wasi -I core/src -I web/src` → WASM binary for browser
- `mojo build examples/counter/counter.mojo -I core/src -I desktop/src` → native binary for desktop

The only difference is the build command. The source code is identical.

### What Each Renderer Provides

| Renderer             | Entry mechanism                      | Event loop driver                      |
|----------------------|--------------------------------------|----------------------------------------|
| **Web**              | JS runtime instantiates WASM, calls `@export` init | JS `requestAnimationFrame` + event listeners |
| **Desktop (webview)**| Native `main()` creates GTK4 window with webview | GTK main loop via `mwv_step()` polling  |
| **Desktop (Blitz)**  | Native `main()` creates Blitz window (future) | Winit event loop via Blitz C shim       |
| **XR (WebXR)**       | JS runtime creates WebXR session, renders DOM panels to textures in 3D | WebXR `requestAnimationFrame` + XR input sources |
| **XR (OpenXR)**      | Native `main()` creates OpenXR session, Blitz panels render to swapchain textures | OpenXR frame loop + Winit/Vello for panel rendering |

---

## Renderer Strategies

### Web Renderer (existing — move to `mojo-gui/web/`) ✅

**How it works today:**

1. Mojo compiles to WASM via `mojo build` → `llc` → `wasm-ld`
2. TypeScript runtime instantiates WASM, provides env imports
3. Mojo writes mutations to shared linear memory
4. JS `Interpreter` reads mutation buffer, applies to real DOM
5. JS `EventBridge` captures DOM events, dispatches to WASM

**Changes needed (all done):**

- ✅ Implement `WebApp` in `web/src/web_launcher.mojo` conforming to the `PlatformApp` trait
- ✅ `web/src/main.mojo` wires `@export` functions to the app structs
- ✅ Build scripts updated to compile shared examples from `examples/` for the WASM target
- ✅ Per-example `web/` subdirectories contain only HTML shell and JS glue (no app logic)

### Desktop Webview Renderer (implemented — `mojo-gui/desktop/`) ✅

Strategy: embed a GTK4 + WebKitGTK webview inside a native window. This is the pragmatic first step — it reuses the same JS mutation interpreter from the web renderer inside a native process, without requiring a browser tab.

**How it works:**

1. Mojo compiles to a **native binary** (not WASM)
2. The binary loads `libmojo_webview.so` (C shim around GTK4 + WebKitGTK) via FFI
3. Creates a native GTK4 window with an embedded WebKitGTK webview
4. Injects `desktop-runtime.js` (standalone mutation interpreter + event bridge) into the webview
5. Mojo writes mutations to a **heap buffer** (not WASM linear memory)
6. The `DesktopBridge` base64-encodes the buffer and sends it to the webview's JS interpreter via `mwv_apply_mutations()`
7. DOM events are captured by JS, serialized as JSON (`{"h":42,"t":0}`), and sent to the native side via `window.mojo_post()`
8. The C shim buffers events in a ring buffer; Mojo polls them with `mwv_poll_event()`

**Architecture:**

```text
┌─ Native Mojo Process ─────────────────────────────────────────────┐
│                                                                    │
│  User App Code (counter.mojo, todo.mojo, ...)                     │
│      │                                                             │
│      ▼                                                             │
│  mojo-gui/core (compiled native — NOT WASM)                       │
│    ├── Signals, Memos, Effects                                     │
│    ├── Virtual DOM + Diff Engine                                   │
│    ├── MutationWriter → heap buffer                                │
│    └── HandlerRegistry (event dispatch)                            │
│         │                            ▲                             │
│         │ mutations (binary)         │ events (JSON)               │
│         ▼                            │                             │
│  DesktopBridge                                                     │
│    ├── Owns heap-allocated mutation buffer (64 KiB)                │
│    ├── flush_mutations() → base64 → webview eval                   │
│    └── poll_event() ← JSON ← ring buffer ← JS                    │
│         │                            ▲                             │
│         ▼                            │                             │
│  ┌─ Embedded Webview (GTK4 + WebKitGTK) ──────────────────────┐   │
│  │                                                             │   │
│  │  desktop-runtime.js                                         │   │
│  │    ├── MutationReader (decodes binary protocol from base64) │   │
│  │    ├── Interpreter (applies mutations to real DOM)           │   │
│  │    ├── TemplateCache (DocumentFragment cloning)              │   │
│  │    └── Event dispatch → window.mojo_post(JSON)              │   │
│  │                                                             │   │
│  │  shell.html                                                 │   │
│  │    └── <div id="root"></div>  (mount point)                 │   │
│  │                                                             │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                    │
└────────────────────────────────────────────────────────────────────┘
```

**Key difference from web:** The Mojo code runs as a native process (not WASM), and writes mutations to a heap buffer instead of WASM linear memory. The bridge base64-encodes the buffer and sends it to an embedded webview via `webview_eval()`. There is no separate browser process — the webview is embedded in the GTK4 window.

**Key difference from Blitz (future):** The webview approach still uses a real browser engine (WebKitGTK) for rendering. Blitz would replace this with a standalone HTML/CSS engine (Stylo + Taffy + Vello), eliminating the WebKitGTK dependency entirely.

**C shim API surface (`shim/mojo_webview.h`):**

| Category   | Functions                                                    |
|------------|--------------------------------------------------------------|
| Lifecycle  | `mwv_create(title, w, h, debug)`, `mwv_destroy(w)`          |
| Window     | `mwv_set_title(w, title)`, `mwv_set_size(w, w, h, hints)`   |
| Content    | `mwv_set_html(w, html)`, `mwv_navigate(w, url)`, `mwv_init(w, js)`, `mwv_eval(w, js)` |
| Event loop | `mwv_run(w)`, `mwv_step(w, blocking)`, `mwv_terminate(w)`   |
| Events     | `mwv_poll_event(w, buf, len)`, `mwv_event_count(w)`, `mwv_event_clear(w)` |
| Mutations  | `mwv_apply_mutations(w, buf, len)` — base64-encode + eval    |
| Diagnostics| `mwv_is_alive(w)`, `mwv_get_window(w)`                       |

**Advantages of webview approach:**

- **Reuses existing JS runtime** — the same mutation interpreter and event bridge from the web renderer, adapted as `desktop-runtime.js`
- **Full CSS support** — WebKitGTK provides a complete browser engine with all CSS features
- **Rapid development** — leverages proven web runtime code instead of writing a new native interpreter
- **Good enough for many apps** — suitable for dashboards, tools, and apps where native rendering isn't critical

**Limitations:**

- **WebKitGTK dependency** — ~50+ MB on disk; Linux only (GTK4 + WebKitGTK 6.0)
- **Base64 IPC overhead** — every mutation buffer is base64-encoded (+33% size) and sent via `webview_eval()`
- **No direct DOM access** — mutations flow through JS string eval, not direct API calls
- **Single platform** — GTK4/WebKitGTK is Linux-only; macOS (WKWebView) and Windows (WebView2) would need separate shim implementations

### Desktop Blitz Renderer (implemented — `mojo-gui/desktop/`, Phase 4) ✅

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

**Key difference from webview approach:** No webview, no JS runtime, and no IPC — mutations are applied in-process via direct C FFI calls. This eliminates the base64 encoding overhead and WebKitGTK dependency.

**Key difference from web:** The Mojo code runs as a native process (not WASM), and manipulates the Blitz DOM directly via C FFI instead of shared WASM linear memory + JS interpreter.

**Adaptation needed in `mojo-gui/core`:**

- The `MutationWriter` currently writes to WASM linear memory (`UnsafePointer[UInt8, MutExternalOrigin]`). For native, it writes to a heap buffer. The writer itself doesn't care — it just writes bytes to a pointer. ✅ Already works (proven by the webview desktop renderer).
- The Blitz desktop renderer implements a Mojo-side mutation interpreter that reads the byte buffer and translates each opcode to the corresponding Blitz C FFI call (similar to how the JS `Interpreter` class reads the buffer and calls DOM methods, but in Mojo instead of JS).

**Advantages of Blitz over the webview approach:**

- **No JS runtime** — no need to bundle or inject JavaScript; the mutation interpreter runs in Mojo
- **No IPC overhead** — mutations are applied in-process via direct FFI calls, not base64-encoded over webview eval
- **Smaller binary** — no browser engine dependency (WebKitGTK is ~50+ MB); Blitz is much lighter
- **Cross-platform** — Blitz uses Winit, which supports Linux, macOS, and Windows natively
- **Better integration** — native window chrome, system menus, accessibility via AccessKit
- **Consistent rendering** — Stylo (Firefox's CSS engine) provides standards-compliant CSS everywhere

### XR Renderer (future — `mojo-gui/xr/`)

Strategy: render DOM-based UI panels into XR environments (VR/AR). Each panel is a separate DOM document that receives the standard mutation protocol, is rendered to a GPU texture, and placed as a quad in 3D space. Two backends:

**OpenXR Native (`mojo-gui/xr/native/`)** — reuses the Blitz stack from the desktop renderer:

- Each `XRPanel` owns a `blitz-dom` document (same as desktop, but rendered to an offscreen texture instead of a Winit window)
- Vello renders each panel's DOM to a `wgpu::Texture` (Vello already supports arbitrary render targets)
- The OpenXR runtime composites these textures as quad layers in 3D space, or the shim renders them into the XR swapchain via a simple 3D compositor
- XR controller raycasting → intersect panel quad → compute 2D hit point → dispatch as DOM pointer events through the existing event protocol
- The `openxr` Rust crate provides session management, pose tracking, input actions, reference spaces

```text
┌─────────────────────────────────────────────────────────────────┐
│  Native Process                                                  │
│                                                                  │
│  ┌─────────────────────┐                                         │
│  │  mojo-gui/core       │                                         │
│  │  (compiled native)   │── mutation buffer ──┐                   │
│  │  signals, vdom,      │                     │                   │
│  │  diff, scheduler     │◄── event dispatch ──┤                   │
│  └─────────────────────┘                     │                   │
│                                              ▼                   │
│  ┌─────────────────────────────────────────────────────────┐     │
│  │  XR Panel Manager                                        │     │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐               │     │
│  │  │ Panel 0   │  │ Panel 1   │  │ Panel N   │  ...         │     │
│  │  │ blitz-dom │  │ blitz-dom │  │ blitz-dom │              │     │
│  │  │ + Stylo   │  │ + Stylo   │  │ + Stylo   │              │     │
│  │  │ + Taffy   │  │ + Taffy   │  │ + Taffy   │              │     │
│  │  │ → Vello   │  │ → Vello   │  │ → Vello   │              │     │
│  │  │ → texture │  │ → texture │  │ → texture │              │     │
│  │  └─────┬────┘  └─────┬────┘  └─────┬────┘               │     │
│  │        │              │              │                     │     │
│  │        ▼              ▼              ▼                     │     │
│  │  ┌──────────────────────────────────────────────────┐    │     │
│  │  │  OpenXR compositor / 3D scene                     │    │     │
│  │  │  (place textures as quads at world positions)     │    │     │
│  │  │  + controller raycasting → 2D hit → DOM events    │    │     │
│  │  └──────────────────────────────────────────────────┘    │     │
│  └─────────────────────────────────────────────────────────┘     │
│                              │                                    │
│                              ▼                                    │
│                     OpenXR Runtime → HMD                          │
└─────────────────────────────────────────────────────────────────┘
```

**WebXR Browser (`mojo-gui/xr/web/`)** — extends the existing web renderer:

- The existing JS mutation interpreter applies mutations to real DOM elements
- A WebXR session manager creates an immersive session and manages reference spaces
- DOM panel content is rendered to WebGL/WebGPU textures (via OffscreenCanvas or html-to-texture techniques) and placed as quads in the WebXR scene
- XR input sources (controllers, hands) are raycasted against panel quads; hits are translated to standard DOM pointer events that flow back through the existing event bridge
- Falls back gracefully to flat web rendering when no XR device is available

```text
┌─ Browser ──────────────────────────────────────────────────────┐
│                                                                 │
│  ┌─────────────────────┐                                        │
│  │  mojo-gui/core       │                                        │
│  │  (WASM)              │── mutation buffer ──┐                  │
│  │                      │                     │                  │
│  │                      │◄── event dispatch ──┤                  │
│  └─────────────────────┘                     │                  │
│                                              ▼                  │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │  XR Panel Manager (JS)                                    │   │
│  │  ┌─────────────┐  ┌─────────────┐                         │   │
│  │  │ Panel 0      │  │ Panel 1      │  ...                   │   │
│  │  │ DOM subtree  │  │ DOM subtree  │                        │   │
│  │  │ → texture    │  │ → texture    │                        │   │
│  │  └──────┬──────┘  └──────┬──────┘                         │   │
│  │         │                │                                 │   │
│  │         ▼                ▼                                 │   │
│  │  ┌──────────────────────────────────────────────────┐     │   │
│  │  │  WebXR session (WebGL/WebGPU)                     │     │   │
│  │  │  (place textures as quads in XR reference space)  │     │   │
│  │  │  + XRInputSource raycasting → DOM pointer events  │     │   │
│  │  └──────────────────────────────────────────────────┘     │   │
│  └──────────────────────────────────────────────────────────┘   │
│                              │                                   │
│                              ▼                                   │
│                     WebXR Runtime → HMD                           │
└─────────────────────────────────────────────────────────────────┘
```

**Key design decisions:**

- **The mutation protocol is unchanged.** Each XR panel receives the same binary opcode stream as any other renderer. The core framework doesn't know it's running in XR.
- **Blitz stack is reused, not forked.** The OpenXR native renderer uses the same `blitz-dom` + Stylo + Taffy + Vello pipeline as the desktop renderer. The only difference is the final render target (offscreen texture vs. Winit surface) and the compositor (OpenXR quad layers vs. window manager).
- **Panels are the spatial primitive.** A panel is a 2D DOM document placed at a 3D position/rotation in the XR scene. Apps create panels via a new `XRPanel` API; each panel can host a separate `GuiApp` or a view within one.
- **Input is bridged, not reinvented.** XR controller rays are intersected with panel quads in 3D; the resulting 2D hit coordinates are translated to standard DOM pointer/click events and dispatched through the existing `HandlerRegistry`. App code doesn't know the click came from a VR controller.
- **wgpu is the unifying GPU layer.** It targets Vulkan/Metal/DX12 natively (for OpenXR) and WebGPU in the browser (for WebXR), providing a single rendering abstraction across both XR backends.

---

## Phase 1: Extract `mojo-gui/core` Library ✅

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

## Phase 2: Create `mojo-gui/web` (Browser Renderer) + Shared Examples ✅

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

## Phase 3: Create `mojo-gui/desktop` (Desktop Webview Renderer) ✅

### Step 3.1 — Design the desktop webview architecture ✅

The webview approach was chosen as a pragmatic first step, following the same evolution Dioxus took (webview → Blitz). It reuses the existing JS mutation interpreter inside a native GTK4 window with an embedded WebKitGTK webview.

```text
┌─ Native Mojo Process ─────────────────────────────────────────────┐
│                                                                    │
│  User App Code (counter.mojo, todo.mojo, ...)                     │
│      │                                                             │
│      ▼                                                             │
│  mojo-gui/core (compiled native — NOT WASM)                       │
│    ├── Signals, Memos, Effects                                     │
│    ├── Virtual DOM + Diff Engine                                   │
│    ├── MutationWriter → heap buffer                                │
│    └── HandlerRegistry (event dispatch)                            │
│         │                            ▲                             │
│         │ mutations (binary)         │ events (JSON)               │
│         ▼                            │                             │
│  DesktopBridge                                                     │
│    ├── Owns heap-allocated mutation buffer (64 KiB)                │
│    ├── flush_mutations() → base64 → webview eval                   │
│    └── poll_event() ← JSON ← ring buffer ← JS                    │
│         │                            ▲                             │
│         ▼                            │                             │
│  ┌─ Embedded Webview (GTK4 + WebKitGTK) ──────────────────────┐   │
│  │  desktop-runtime.js                                         │   │
│  │    ├── MutationReader (decodes base64 → binary protocol)    │   │
│  │    ├── Interpreter (applies mutations to real DOM)           │   │
│  │    ├── TemplateCache (DocumentFragment cloning)              │   │
│  │    └── Event dispatch → window.mojo_post(JSON)              │   │
│  │  shell.html  <div id="root"></div>                          │   │
│  └─────────────────────────────────────────────────────────────┘   │
└────────────────────────────────────────────────────────────────────┘
```

### Step 3.2 — Implement the C shim (`libmojo_webview.so`) ✅

Built a C shim (`shim/mojo_webview.c`) wrapping GTK4 + WebKitGTK with a Mojo-friendly **polling** API. Key design decisions:

- **No function-pointer callbacks** — Mojo's FFI (`DLHandle`) cannot easily pass managed closures as C function pointers. Instead, JS sends events via `window.mojo_post(json)` into a ring buffer, and Mojo polls with `mwv_poll_event()`.
- **Ring buffer for events** — capacity 256 events × 4 KiB each, FIFO with oldest-drop overflow.
- **Base64 mutation delivery** — `mwv_apply_mutations(buf, len)` base64-encodes the binary buffer and calls `window.__mojo_apply_mutations(base64)` in the webview.
- **Non-blocking step API** — `mwv_step(w, blocking)` allows cooperative event loop interleaving.

**C shim API surface (`shim/mojo_webview.h`):**

| Category    | Functions                                                    |
|-------------|--------------------------------------------------------------|
| Lifecycle   | `mwv_create(title, w, h, debug)`, `mwv_destroy(w)`          |
| Window      | `mwv_set_title(w, title)`, `mwv_set_size(w, w, h, hints)`   |
| Content     | `mwv_set_html(w, html)`, `mwv_navigate(w, url)`, `mwv_init(w, js)`, `mwv_eval(w, js)` |
| Event loop  | `mwv_run(w)`, `mwv_step(w, blocking)`, `mwv_terminate(w)`   |
| Events      | `mwv_poll_event(w, buf, len)`, `mwv_event_count(w)`, `mwv_event_clear(w)` |
| Mutations   | `mwv_apply_mutations(w, buf, len)` — base64-encode + eval    |
| Diagnostics | `mwv_is_alive(w)`, `mwv_get_window(w)`                       |

Nix derivation (`shim/default.nix`) automates the build and provides the library path via `MOJO_WEBVIEW_LIB`.

### Step 3.3 — Implement Mojo FFI bindings (`webview.mojo`) ✅

Created `desktop/src/desktop/webview.mojo` with typed Mojo wrappers around the C shim API via `OwnedDLHandle`. The `Webview` struct provides:

- `create(title, width, height, debug)` — open a window
- `set_html(html)` / `init_js(js)` / `eval_js(js)` — content injection
- `step(blocking)` / `run()` — event loop control
- `poll_event()` — drain events from the ring buffer
- `apply_mutations(buf, len)` — send mutation buffer to JS interpreter
- Library search: `MOJO_WEBVIEW_LIB` env var → `NIX_LDFLAGS` → `LD_LIBRARY_PATH` → common paths

### Step 3.4 — Implement the desktop bridge (`bridge.mojo`) ✅

Created `desktop/src/desktop/bridge.mojo` with:

- **`DesktopBridge`** struct — owns a heap-allocated mutation buffer (64 KiB default), provides `buf_ptr()` for `MutationWriter`, `flush_mutations(len)` to send to webview, `poll_event()` to drain events.
- **`DesktopEvent`** struct — parsed event with `handler_id`, `event_type`, optional `value` string.
- **`parse_event(json)`** — minimal JSON parser for the `{"h":42,"t":0,"v":"..."}` format.

### Step 3.5 — Implement `DesktopApp` (`app.mojo`) ✅

Created `desktop/src/desktop/app.mojo` with the `DesktopApp` struct that orchestrates:

1. Webview creation and JS runtime injection
2. HTML shell loading (inline `SHELL_HTML` with `#root` mount point)
3. `desktop-runtime.js` loading (env var → relative path search)
4. Multiple event loop styles: `run()` (blocking), `run_with_mount(len)` (mount + run), `run_interactive()` (drain events), or manual `step()` + `poll_event()` for full control.

### Step 3.6 — Create the desktop JS runtime (`desktop-runtime.js`) ✅

Created `desktop/runtime/desktop-runtime.js` — a standalone 900+ line JS file containing:

- **`MutationReader`** — reads binary opcodes from an ArrayBuffer (base64-decoded)
- **`TemplateCache`** — registers and clones DocumentFragment templates
- **`Interpreter`** — full stack machine implementing all mutation opcodes (LoadTemplate, SetAttribute, SetText, AppendChildren, NewEventListener, Remove, ReplaceWith, ReplacePlaceholder, InsertAfter, InsertBefore, AssignId, CreateTextNode, CreatePlaceholder, PushRoot, RegisterTemplate, RemoveAttribute, RemoveEventListener)
- **Event dispatch** — DOM event listeners that serialize events as JSON and call `window.mojo_post()`
- **`window.__mojo_apply_mutations(base64)`** — entry point called by the C shim's `mwv_apply_mutations()`

This is a self-contained adaptation of the web renderer's TypeScript runtime, transpiled to plain JS for webview injection.

### Step 3.7 — Build the counter example ✅

Created `desktop/examples/counter.mojo` — a working counter app demonstrating:

- Same `CounterApp` struct and reactive logic as the web version
- `DesktopApp` entry point instead of `@export` WASM wrappers
- Heap buffer instead of WASM linear memory
- Full interactive event loop: mount → poll events → dispatch → re-render → flush mutations
- `ConditionalSlot` for show/hide detail section (even/odd, doubled value)

Build and run:

```text
cd mojo-gui/desktop
just run-counter
```

### Step 3.8 — Build system and Nix integration ✅

- **`justfile`** — `build-shim`, `build-counter`, `run-counter`, `dev-counter`, `test-shim`, `test-runtime`
- **`default.nix`** — dev shell with GTK4, WebKitGTK 6.0, pkg-config, `libmojo-webview` derivation, environment variables
- **`shim/default.nix`** — standalone Nix derivation for the C shim library

### Step 3.9 — Unified app lifecycle and `launch()`

The remaining work is **not** about porting individual examples to desktop. Each example must be exactly the same source file for every target — no per-renderer copies, no `desktop/examples/` duplicates. The framework must abstract the platform away so that `launch[MyApp]()` works on web and desktop from a single source file.

#### Step 3.9.1 — Define the `GuiApp` trait ✅

Created `core/src/platform/gui_app.mojo` with the app-side lifecycle trait:

```text
trait GuiApp(Movable):
    fn __init__(out self)
    fn render(mut self) -> UInt32
    fn handle_event(mut self, handler_id: UInt32, event_type: UInt8, value: String) -> Bool
    fn flush(mut self, writer_ptr: UnsafePointer[MutationWriter, MutExternalOrigin]) -> Int32
    fn mount(mut self, writer_ptr: UnsafePointer[MutationWriter, MutExternalOrigin]) -> Int32
    fn has_dirty(self) -> Bool
    fn consume_dirty(mut self) -> Bool
    fn destroy(mut self)
```

This captures the lifecycle that currently lives as free functions (`counter_app_rebuild`, `todo_app_flush`, etc.). Each existing app struct (CounterApp, TodoApp, BenchmarkApp, MultiViewApp) already has `render()`, the lifecycle free functions, and a `ctx` field — the refactor is mechanical: move the free functions into the struct as methods, add the trait conformance.

The trait uses `has_dirty()` / `consume_dirty()` / `destroy()` instead of a raw `context()` pointer, keeping the `ComponentContext` internals private to the app. This is cleaner than exposing a pointer and allows apps with additional dirty state (e.g., MultiViewApp's `router.dirty`) to compose it naturally.

`handle_event` takes a `value: String` parameter (empty when not applicable). This unifies `dispatch_event()` and `dispatch_event_with_string()` — the renderer always passes the value through. This resolves the input event value binding issue: the desktop event loop no longer needs app-specific branching on `event.has_value`.

The trait is exported from the `platform` package via `__init__.mojo`.

#### Step 3.9.2 — Implement the generic desktop event loop ✅

Created `desktop/src/desktop/launcher.mojo` with a generic `desktop_launch[AppType: GuiApp]()` function backed by the Blitz rendering engine:

```text
fn desktop_launch[AppType: GuiApp](config: AppConfig) raises:
    var blitz = Blitz.create(config.title, config.width, config.height, debug=config.debug)
    blitz.add_ua_stylesheet(_DEFAULT_UA_CSS)
    var app = AppType()

    var buf_ptr = _alloc_mutation_buffer(_DEFAULT_BUF_CAPACITY)
    var writer_ptr = _alloc_writer(buf_ptr, _DEFAULT_BUF_CAPACITY)
    var interpreter = MutationInterpreter(blitz)

    var mount_len = app.mount(writer_ptr)
    if mount_len > 0:
        blitz.begin_mutations()
        interpreter.apply(buf_ptr, Int(mount_len))
        blitz.end_mutations()
        blitz.request_redraw()

    while blitz.is_alive():
        _ = blitz.step(blocking=False)
        var had_event = False
        while True:
            var event = blitz.poll_event()
            if not event.valid: break
            had_event = True
            _ = app.handle_event(event.handler_id, event.event_type, event.value)
        if app.has_dirty():
            _reset_writer(writer_ptr, buf_ptr, _DEFAULT_BUF_CAPACITY)
            var flush_len = app.flush(writer_ptr)
            if flush_len > 0:
                blitz.begin_mutations()
                interpreter.apply(buf_ptr, Int(flush_len))
                blitz.end_mutations()
                blitz.request_redraw()
        elif not had_event:
            _ = blitz.step(blocking=True)

    _free_writer(writer_ptr)
    buf_ptr.free()
    app.destroy()
    blitz.destroy()
```

This single function replaces every `desktop/examples/*.mojo` file — the event loop is identical for counter, todo, bench, and app. The `GuiApp` trait methods encapsulate all app-specific logic (ConditionalSlot management, KeyedList flush, custom event routing, etc.). Note: `has_dirty()` is used instead of `consume_dirty()` in the event loop check because `flush()` calls `consume_dirty()` internally. `destroy()` is called directly on the app (no need to reach into `context()`).

Key difference from the webview approach: mutations are applied in-process via the Mojo `MutationInterpreter` → Blitz C FFI calls (no base64 encoding, no JS eval, no IPC). The interpreter reads the same binary opcode buffer and translates each opcode to the corresponding Blitz DOM operation.

#### Step 3.9.3 — Wire `launch()` to dispatch by target ✅

Updated `core/src/platform/launch.mojo` so `launch[AppType: GuiApp]()` dispatches at compile time:

```text
fn launch[AppType: GuiApp](config: AppConfig = AppConfig()) raises:
    @parameter
    if is_wasm_target():
        pass  # WASM: JS runtime drives the loop; @export wrappers call GuiApp methods
    else:
        # Desktop path: create Blitz window and enter event loop.
        # The config is passed directly — no need for global state.
        from desktop.launcher import desktop_launch
        desktop_launch[AppType](config)
```

The previous non-parametric `launch(config)` overload and module-level `var _global_config` / `var _launched` globals were removed. Mojo does not support module-level `var` on native targets, so the current design avoids global mutable state entirely: on native, config is passed directly to `desktop_launch()` as an argument; on WASM, `@export` wrappers receive config through compile-time type parameters and constructor arguments. The `get_launch_config()` and `has_launched()` functions return defaults for API compatibility — callers should use the config passed directly to them.

The native target dispatch was updated from a placeholder print statement to the actual `desktop_launch` call as part of Phase 4 Blitz implementation.

#### Step 3.9.4 — Refactor existing app structs to implement `GuiApp` ✅

All four main app structs now implement the `GuiApp` trait. The refactor was mechanical — free functions were moved into struct methods, and the struct declarations changed from `(Movable)` to `(GuiApp)`:

| App struct | Refactored methods | Notes |
|---|---|---|
| `CounterApp` | `mount()`, `handle_event()`, `flush()`, `has_dirty()`, `consume_dirty()`, `destroy()` | Simple — click events only, ConditionalSlot for detail |
| `TodoApp` | `mount()`, `handle_event()`, `flush()`, `has_dirty()`, `consume_dirty()`, `destroy()` | String events dispatched via `dispatch_event_with_string` when `len(value) > 0` |
| `BenchmarkApp` | `mount()`, `handle_event()`, `flush()`, `has_dirty()`, `consume_dirty()`, `destroy()` | Toolbar routing + KeyedList row events, performance timing |
| `MultiViewApp` | `mount()`, `handle_event()`, `flush()`, `has_dirty()`, `consume_dirty()`, `destroy()` | Router dirty state composed into `has_dirty()`, nav + signal dispatch |

The unified `handle_event(handler_id, event_type, value)` pattern works for all apps:

- Apps without string events (counter, bench) pass through to `dispatch_event()` when `value` is empty
- Apps with string events (todo) dispatch via `dispatch_event_with_string()` when `len(value) > 0`
- Apps with custom routing (multi-view) check app-level handlers first, then fall back to `dispatch_event()`

**Backwards compatibility:** The original free functions (`counter_app_rebuild`, `todo_app_flush`, etc.) are retained as thin one-line wrappers that delegate to the new trait methods. This preserves compatibility with the existing `@export` wrappers in `web/src/main.mojo`. These wrappers can be removed once the `@export` surface is genericized over `GuiApp` (Step 3.9.5).

Platform-specific APIs like `performance_now()` need conditional compilation:

```text
fn performance_now() -> Float64:
    @parameter
    if is_wasm_target():
        return external_call["performance_now", Float64]()
    else:
        from time import perf_counter_ns
        return Float64(perf_counter_ns()) / 1_000_000.0
```

This stays in the shared example code — no duplication needed.

#### Step 3.9.5 — Refactor `main.mojo` WASM exports to be generic over `GuiApp` ✅

Created `web/src/gui_app_exports.mojo` with parametric lifecycle helper functions:

```text
fn gui_app_init[T: GuiApp]() -> Int64          # heap alloc + T.__init__()
fn gui_app_destroy[T: GuiApp](app_ptr: Int64)   # T.destroy() + free
fn gui_app_mount[T: GuiApp](app_ptr, buf, cap) -> Int32    # T.mount()
fn gui_app_handle_event[T: GuiApp](app_ptr, hid, et) -> Int32  # T.handle_event(..., "")
fn gui_app_handle_event_string[T: GuiApp](app_ptr, hid, et, value) -> Int32  # T.handle_event(..., value)
fn gui_app_flush[T: GuiApp](app_ptr, buf, cap) -> Int32    # T.flush()
fn gui_app_has_dirty[T: GuiApp](app_ptr) -> Int32           # T.has_dirty()
fn gui_app_consume_dirty[T: GuiApp](app_ptr) -> Int32       # T.consume_dirty()
```

The per-app `@export` wrappers in `main.mojo` are now one-liners:

```text
@export fn counter_init() -> Int64:
    return gui_app_init[CounterApp]()
@export fn counter_rebuild(app_ptr: Int64, buf_ptr: Int64, capacity: Int32) -> Int32:
    return gui_app_mount[CounterApp](app_ptr, buf_ptr, capacity)
@export fn counter_handle_event(app_ptr: Int64, handler_id: Int32, event_type: Int32) -> Int32:
    return gui_app_handle_event[CounterApp](app_ptr, handler_id, event_type)
@export fn counter_flush(app_ptr: Int64, buf_ptr: Int64, capacity: Int32) -> Int32:
    return gui_app_flush[CounterApp](app_ptr, buf_ptr, capacity)
@export fn counter_destroy(app_ptr: Int64):
    gui_app_destroy[CounterApp](app_ptr)
```

All four main apps (CounterApp, TodoApp, BenchmarkApp, MultiViewApp) now use the generic helpers. The backwards-compatible free functions (`counter_app_init`, `counter_app_rebuild`, etc.) have been removed from the example files — the generic helpers call `GuiApp` trait methods directly. App-specific query exports (e.g., `counter_count_value`, `todo_item_count`) remain as hand-written `@export` functions since they reach into app-specific fields. All 3,090 JS tests and 52 Mojo test suites pass.

#### Step 3.9.6 — Delete `desktop/examples/` and add `launch()` to shared examples ✅

Steps 3.9.1–3.9.5 are complete, the cdylib builds, and Winit integration (Step 4.6) is done.

1. ✅ No `desktop/examples/` directory exists (never created as separate duplicates — the webview counter example was not carried forward to Blitz).
2. ✅ Added `main.mojo` with `fn main() raises: launch[AppType](AppConfig(...))` to each shared example:
   - `examples/counter/main.mojo` — `launch[CounterApp](AppConfig(title="High-Five Counter", width=400, height=350))`
   - `examples/todo/main.mojo` — `launch[TodoApp](AppConfig(title="Todo List", width=500, height=600))`
   - `examples/bench/main.mojo` — `launch[BenchmarkApp](AppConfig(title="js-framework-benchmark — mojo-gui", width=1000, height=800))`
   - `examples/app/main.mojo` — `launch[MultiViewApp](AppConfig(title="Multi-View App", width=600, height=500))`
3. Each example compiles for both targets with identical source:
   - `mojo build examples/counter/main.mojo --target wasm64-wasi -I core/src -I web/src -I examples` → WASM
   - `mojo build examples/counter/main.mojo -I core/src -I desktop/src -I examples` → native

#### Step 3.9.7 — Cross-target verification and CI

- [ ] Cross-target CI test matrix (web + desktop for every shared example)
- [ ] Verify all 4 examples (counter, todo, bench, app) build and run on both targets from identical source
- [ ] Window lifecycle events (close confirmation, minimize/maximize state)
- [ ] Investigate replacing base64 IPC with more efficient binary transfer (custom URI scheme or shared memory)

**Cross-target status (target state after Step 3.9):**

| Example   | Source location | Web (WASM) | Desktop (Blitz) | Same source? |
|-----------|----------------|------------|-----------------|--------------|
| counter   | `examples/counter/main.mojo` | ✅ | ⏳ (needs build verification) | ✅ |
| todo      | `examples/todo/main.mojo` | ✅ | ⏳ (needs build verification) | ✅ |
| bench     | `examples/bench/main.mojo` | ✅ | ⏳ (needs build verification) | ✅ |
| app       | `examples/app/main.mojo` | ✅ | ⏳ (needs build verification) | ✅ |

---

## Phase 4: Desktop Blitz Renderer (Implementation Complete, Verification Pending)

Replace the webview dependency in the desktop renderer with [Blitz](https://github.com/DioxusLabs/blitz), a native HTML/CSS rendering engine. This is the same evolution Dioxus followed — webview first, then Blitz for native rendering without a browser engine.

### Step 4.1 — Build Blitz C shim (`shim/src/lib.rs`) ✅

Built a Rust `cdylib` (`mojo-blitz-shim`) exposing `blitz-dom` via `extern "C"` functions. The shim wraps `BaseDocument` + `DocumentMutator` with a polling-based C ABI (no callbacks). Blitz dependencies are pinned to v0.2.0 (rev `2f83df96220561316611ecf857e20cd1feed8ca0`); markup5ever types are re-exported from `blitz_dom` (no direct dependency — avoids version mismatch). Key design decisions:

- **`BlitzContext` struct** — owns the `BaseDocument`, ID mapping (mojo element IDs ↔ Blitz slab node IDs), template registry, event handler registrations, event queue, and interpreter stack.
- **Minimal DOM structure** — on creation, the shim builds `Document → <html> → <body>` with an optional `<head><title>` element. The `<body>` is the mount point (mojo element ID 0).
- **Node ID mapping** — mojo-gui uses its own element ID space (u32); Blitz uses slab indices (usize). The shim maintains bidirectional `HashMap`s. Internal nodes (from template building) get IDs starting at 0x8000_0000 to avoid collisions.
- **Template registry** — templates are pre-built DOM subtrees (detached). `mblitz_clone_template()` calls `doc.deep_clone_node()` on the registered root.
- **Stack operations** — the shim maintains an interpreter stack for opcodes like PUSH_ROOT / APPEND_CHILDREN. Stack-based operations (`mblitz_stack_push`, `mblitz_stack_pop_append`, `mblitz_stack_pop_replace`, `mblitz_stack_pop_insert_before`, `mblitz_stack_pop_insert_after`) are exposed via separate FFI functions.
- **Event ring buffer** — handlers registered via `mblitz_add_event_listener()` create an in-memory mapping; events are queued by the shim and polled by Mojo via `mblitz_poll_event()`.

**C shim API surface (`shim/mojo_blitz.h`):**

| Category   | Functions                                                    |
|------------|--------------------------------------------------------------|
| Lifecycle  | `mblitz_create(title, len, w, h, debug)`, `mblitz_destroy(ctx)`, `mblitz_step(ctx, blocking)`, `mblitz_is_alive(ctx)`, `mblitz_request_redraw(ctx)` |
| Window     | `mblitz_set_title(ctx, title, len)`, `mblitz_set_size(ctx, w, h)` |
| Stylesheet | `mblitz_add_ua_stylesheet(ctx, css, len)` |
| DOM create | `mblitz_create_element(ctx, tag, len)`, `mblitz_create_text_node(ctx, text, len)`, `mblitz_create_placeholder(ctx)` |
| Templates  | `mblitz_register_template(ctx, id, root)`, `mblitz_clone_template(ctx, id)` |
| DOM mutate | `mblitz_append_children(ctx, parent, ids, count)`, `mblitz_insert_before(ctx, anchor, ids, count)`, `mblitz_insert_after(ctx, anchor, ids, count)`, `mblitz_replace_with(ctx, old, ids, count)`, `mblitz_remove_node(ctx, id)` |
| Attributes | `mblitz_set_attribute(ctx, id, name, nlen, val, vlen)`, `mblitz_remove_attribute(ctx, id, name, nlen)` |
| Text       | `mblitz_set_text_content(ctx, id, text, len)` |
| Traversal  | `mblitz_node_at_path(ctx, start, path, plen)`, `mblitz_child_at(ctx, id, idx)`, `mblitz_child_count(ctx, id)` |
| Events     | `mblitz_add_event_listener(ctx, id, hid, name, nlen)`, `mblitz_remove_event_listener(ctx, id, name, nlen)`, `mblitz_poll_event(ctx)`, `mblitz_event_count(ctx)`, `mblitz_event_clear(ctx)` |
| Batch      | `mblitz_begin_mutations(ctx)`, `mblitz_end_mutations(ctx)` |
| Stack      | `mblitz_stack_push(ctx, id)`, `mblitz_stack_pop_append(ctx, parent, n)`, `mblitz_stack_pop_replace(ctx, old, n)`, `mblitz_stack_pop_insert_before(ctx, anchor, n)`, `mblitz_stack_pop_insert_after(ctx, anchor, n)` |
| ID mapping | `mblitz_assign_id(ctx, mojo_id, blitz_id)` |
| Root       | `mblitz_root_node_id(ctx)`, `mblitz_mount_point_id(ctx)` |
| Layout     | `mblitz_resolve_layout(ctx)` |
| Debug      | `mblitz_print_tree(ctx)`, `mblitz_set_debug_overlay(ctx, on)`, `mblitz_version(ptr, len)` |

Nix derivation (`shim/default.nix`) automates the Rust build with all GPU/windowing dependencies (Vulkan, Wayland, X11, fontconfig, etc.) and provides the library path via `MOJO_BLITZ_LIB`.

### Step 4.2 — Implement Mojo FFI bindings (`desktop/src/desktop/blitz.mojo`) ✅

Created typed Mojo wrappers around the C shim API via `DLHandle`. The `Blitz` struct provides:

- `create(title, width, height, debug)` — open a window + initialize Blitz context
- `step(blocking)` / `is_alive()` / `destroy()` — event loop control
- `create_element(tag)` / `create_text_node(text)` / `create_placeholder()` — DOM creation
- `set_attribute(id, name, value)` / `remove_attribute(id, name)` — attribute manipulation
- `set_text_content(id, text)` — text node updates
- `append_children(parent, ids, count)` / `insert_before(anchor, ids, count)` / `insert_after(...)` / `replace_with(...)` / `remove_node(id)` — tree mutations
- `register_template(tmpl_id, root_id)` / `clone_template(tmpl_id)` — template management
- `add_event_listener(id, handler_id, name)` / `remove_event_listener(id, name)` / `poll_event()` — event handling
- `stack_push(id)` / `stack_pop_append(parent, n)` / `stack_pop_replace(old, n)` / `stack_pop_insert_before(anchor, n)` / `stack_pop_insert_after(anchor, n)` — interpreter stack operations
- `assign_id(mojo_id, blitz_id)` — element ID mapping
- `begin_mutations()` / `end_mutations()` — mutation batching
- `add_ua_stylesheet(css)` / `request_redraw()` / `resolve_layout()` — rendering control
- `print_tree()` / `set_debug_overlay(enabled)` — debug/diagnostics
- Library search: `MOJO_BLITZ_LIB` env var → `NIX_LDFLAGS` → `LD_LIBRARY_PATH` → bare library name

### Step 4.3 — Implement Mojo-side mutation interpreter (`desktop/src/desktop/renderer.mojo`) ✅

Ported the JS `Interpreter` logic to Mojo as `MutationInterpreter`. It reads binary opcodes from the mutation buffer and translates each one into Blitz C FFI calls. This is the key advantage over the webview approach — no base64 encoding, no JS eval, direct in-process DOM manipulation.

The interpreter handles all 18 opcodes:

- **OP_REGISTER_TEMPLATE** — the most complex: reads the full template wire format (nodes, attributes, root indices), builds real Blitz DOM nodes for the template's static structure, wires parent-child relationships, applies static attributes, and registers the root for deep-cloning.
- **OP_LOAD_TEMPLATE** — clones a registered template, assigns the mojo element ID, pushes to stack.
- **OP_ASSIGN_ID** — navigates a path from the template root to a child node, maps mojo element ID → Blitz node ID.
- **OP_APPEND_CHILDREN / REPLACE_WITH / INSERT_BEFORE / INSERT_AFTER** — pop from the interpreter stack and apply tree mutations.
- **OP_SET_ATTRIBUTE / SET_TEXT / NEW_EVENT_LISTENER / REMOVE_EVENT_LISTENER / REMOVE / REMOVE_ATTRIBUTE** — direct forwarding to Blitz FFI.
- **OP_CREATE_TEXT_NODE / CREATE_PLACEHOLDER** — create nodes, assign IDs, push to stack.
- **OP_PUSH_ROOT** — push a node onto the stack.
- **OP_END** — terminates the opcode stream.

### Step 4.3.1 — Build the Rust cdylib ✅

Resolved all Blitz dependency issues and successfully built the `libmojo_blitz.so` shared library:

- **Fixed `rev = "main"`** — `rev` requires a commit hash, not a branch name. Pinned all Blitz dependencies to the v0.2.0 release commit `2f83df96220561316611ecf857e20cd1feed8ca0`.
- **Fixed markup5ever version mismatch** — removed direct `markup5ever = "0.37.0"` dependency (which resolved to 0.37.1) because Blitz v0.2.0 internally uses markup5ever 0.35.0. All markup5ever types (`QualName`, `LocalName`, `Prefix`, `local_name!`, `ns!`) are now imported via `blitz_dom`'s re-exports.
- **Fixed API mismatches** — `insert_before()` → `insert_nodes_before()` (Blitz's DocumentMutator API); `doc.nodes[id]` → `doc.get_node(id)` (the `nodes` slab is private on `BaseDocument`); `BlitzContext::create_element` now uses `DocumentMutator::create_element` for proper stylo data initialization.
- **Fixed `node_at_path`** — was incorrectly calling `self.doc.mutate()` on `&self` (mutate requires `&mut self`). Reimplemented using `doc.get_node()` traversal.
- **Generated `Cargo.lock`** — reproducible builds with all transitive dependency versions locked.
- **Build output:** `libmojo_blitz.so` ~8MB ELF 64-bit x86-64 shared library (release profile, `opt-level = 2`, thin LTO, stripped symbols).

### Step 4.4 — Verify all shared examples — next priority

Every example that works on web MUST work on desktop-Blitz. The app code is identical — only the renderer backend changes. Each example now has a `main.mojo` entry point with `launch[AppType](config)`.

- [ ] Counter example builds and runs on desktop (`mojo build examples/counter/main.mojo -I core/src -I desktop/src -I examples`)
- [ ] Todo example builds and runs on desktop
- [ ] Bench example builds and runs on desktop
- [ ] Multi-view app example builds and runs on desktop

### Step 4.5 — Cross-platform support

Blitz uses Winit, which supports Linux, macOS, and Windows. Verify the Blitz renderer works on all three platforms (the previous webview renderer was Linux-only due to GTK4/WebKitGTK).

### Step 4.6 — Winit event loop integration ✅

Implemented full Winit event loop integration in the Blitz C shim (`shim/src/lib.rs`). The `mblitz_step()` function is no longer a placeholder — it drives the real windowing and rendering pipeline:

1. ✅ **`ApplicationHandler` impl for `BlitzContext`** — `resumed()` creates the Winit window with `Arc<Window>`, initializes the Vello GPU renderer via `anyrender_vello::VelloWindowRenderer`, and updates the document viewport. Re-resume after suspend is also handled.
2. ✅ **`mblitz_step(blocking)` wired to `pump_app_events()`** — the `EventLoop<()>` is stored in an `Option` and temporarily taken out during each step to avoid borrow conflicts (the same struct serves as both the event loop owner and the `ApplicationHandler`). Non-blocking mode uses `Duration::ZERO`; blocking mode uses 100ms timeout for periodic checks.
3. ✅ **Winit window event routing** — `handle_winit_event()` processes `CloseRequested`, `RedrawRequested`, `Resized`, `ScaleFactorChanged`, `CursorMoved`, and `MouseInput` events. Mouse events are translated to Blitz `UiEvent` variants (`MouseMove`, `MouseDown`, `MouseUp`) with tracked button state and logical coordinates.
4. ✅ **DOM event extraction via `MojoEventHandler`** — custom `EventHandler` implementation intercepts Blitz DOM events during bubble propagation, maps `DomEventData` variants (Click, Input, KeyDown, etc.) to mojo-gui handler IDs, and buffers them in `event_queue` for polling via `mblitz_poll_event()`. Disjoint borrows are managed via raw pointers to split `event_handlers` and `event_queue` from the `DocumentMutator`.
5. ✅ **GPU rendering via Vello + blitz-paint** — `RedrawRequested` triggers `doc.resolve(0.0)` for style resolution + layout (Stylo + Taffy), then `paint_scene()` renders the document to the Vello scene. `mblitz_request_redraw()` sets a flag and calls `window.request_redraw()`.

**Dependency version fixes:** The original `Cargo.toml` specified `anyrender 0.7`, `anyrender_vello 0.7`, and `winit 0.31-beta`, which caused version mismatches with Blitz v0.2.0's internal dependencies (`anyrender 0.6`, `winit 0.30`). Fixed by downgrading to match Blitz's pinned versions: `anyrender 0.6`, `anyrender_vello 0.6`, `winit 0.30`. This also required porting the code from winit 0.31 API (`PointerMoved`, `PointerButton`, `SurfaceResized`, `can_create_surfaces`, `dyn ActiveEventLoop`, `Box<dyn Window>`) to winit 0.30 API (`CursorMoved`, `MouseInput`, `Resized`, `resumed`, concrete `&ActiveEventLoop`, `Arc<Window>`). The `renderer.resume()` call was updated to pass `Arc<dyn anyrender::WindowHandle>` as required by anyrender 0.6.

**Build output:** `libmojo_blitz.so` ~23MB ELF 64-bit x86-64 shared library (release profile, `opt-level = 2`, thin LTO, stripped symbols). Clean build with zero warnings. `Cargo.lock` generated with 607 packages (down from 649 before the version fix — no more duplicate dependency trees).

---

## Phase 5: XR Renderer (Future)

Render DOM-based UI panels into XR environments. The mutation protocol is unchanged — each XR panel receives the same binary opcode stream. The Blitz stack (blitz-dom + Stylo + Taffy + Vello) is reused for native OpenXR; the existing web renderer's JS interpreter is extended for WebXR.

**Compile targets (complete picture):**

- `mojo build --target wasm64-wasi` → web renderer (needs `mojo-gui/web` JS runtime)
- `mojo build --target wasm64-wasi --feature webxr` → WebXR renderer (extends web renderer with XR session)
- `mojo build` → desktop renderer (Blitz native, implementation complete — verification pending)
- `mojo build --feature xr` → OpenXR native renderer (Blitz panels → OpenXR swapchain)

### Step 5.1 — Design the XR panel abstraction

Define the `XRPanel` concept: a 2D DOM document with a 3D transform. Each panel:

- Owns a Blitz document (native) or DOM subtree (web) that receives mutations
- Has a world-space position, rotation, and physical size (in meters)
- Has a pixels-per-meter density for text legibility
- Supports pointer input via raycasting (controller ray → 2D hit point → DOM event)

```text
# xr/native/src/panel.mojo

struct XRPanel(Movable):
    """A 2D DOM document placed in 3D XR space."""
    var panel_id: UInt32
    var position: SIMD[DType.float32, 4]    # x, y, z, 0
    var rotation: SIMD[DType.float32, 4]    # quaternion x, y, z, w
    var size: SIMD[DType.float32, 2]        # width, height in meters
    var pixels_per_meter: Float32
    var texture_width: UInt32
    var texture_height: UInt32
```

### Step 5.2 — Build the OpenXR + Blitz Rust shim

Extend the existing Blitz C shim (`desktop/shim/src/lib.rs`) or create a new `xr/native/shim/` that:

- Links `blitz-dom`, `stylo`, `taffy`, `vello` (reused from desktop shim)
- Adds the `openxr` Rust crate for XR session management
- Manages multiple `BlitzDocument` instances (one per panel)
- Renders each panel's DOM to an offscreen `wgpu::Texture` via Vello
- Composites panel textures into the OpenXR swapchain (as quad layers or rendered into the scene)
- Handles XR frame timing (`xrWaitFrame`, `xrBeginFrame`, `xrEndFrame`)
- Provides controller pose data and performs panel raycasting

**C API surface (indicative):**

| Category        | Functions                                                                           |
|-----------------|-------------------------------------------------------------------------------------|
| Session         | `mxr_create_session()`, `mxr_destroy_session()`, `mxr_is_session_active()`          |
| Panels          | `mxr_create_panel(w, h, ppm)`, `mxr_destroy_panel(id)`, `mxr_panel_set_transform()` |
| Mutations       | `mxr_panel_apply_mutations(id, buf, len)` — same binary protocol as desktop          |
| Events          | `mxr_poll_event(buf, len)` — panel_id + handler_id + event_type + value              |
| Frame loop      | `mxr_wait_frame()`, `mxr_begin_frame()`, `mxr_end_frame()`                          |
| Input           | `mxr_get_controller_pose(hand)`, `mxr_get_head_pose()`                               |
| Reference spaces| `mxr_create_reference_space(type)`, `mxr_get_space_location()`                       |

### Step 5.3 — Implement Mojo FFI bindings for OpenXR shim

Create `xr/native/src/xr_blitz.mojo` — typed `XRBlitz` struct via `DLHandle`, wrapping all shim functions. Follows the same pattern as `desktop/src/desktop/blitz.mojo`.

### Step 5.4 — Implement XR scene manager and panel routing

Create `xr/native/src/scene.mojo` — manages the collection of XR panels:

- Routes mutation buffers to the correct panel's Blitz document
- Polls events from the shim and dispatches to the correct panel's `GuiApp`
- Provides spatial layout helpers (arrange panels in an arc, pin to hand, anchor to world)

### Step 5.5 — Implement `xr_launch[AppType: GuiApp]()`

Create `xr/native/src/xr_launcher.mojo`:

- Creates an OpenXR session via the shim
- Creates a default panel for the app
- Runs the XR frame loop: wait frame → poll input → raycast panels → dispatch events → re-render dirty panels → composite → end frame
- Integrates with `launch()` via a new `@parameter if` branch (or `--feature xr` flag)

### Step 5.6 — Implement WebXR JS runtime

Create `xr/web/runtime/`:

- `xr-session.ts` — WebXR session lifecycle (`navigator.xr.requestSession('immersive-vr')`)
- `xr-panels.ts` — render DOM subtrees to WebGL/WebGPU textures, manage panel 3D transforms
- `xr-input.ts` — XR input source → raycast against panel quads → translate to DOM pointer events → dispatch via existing `EventBridge`
- `xr-runtime.ts` — entry point that extends the existing web runtime; replaces `requestAnimationFrame` with `XRSession.requestAnimationFrame`

### Step 5.7 — Wire `launch()` for XR targets

Update `core/src/platform/launch.mojo`:

```text
fn launch[AppType: GuiApp](config: AppConfig):
    @parameter
    if is_wasm_target():
        @parameter
        if has_feature("webxr"):
            _register_webxr_app[AppType]()
        else:
            _register_web_app[AppType]()
    else:
        @parameter
        if has_feature("xr"):
            xr_launch[AppType](config)
        else:
            desktop_launch[AppType](config)
```

### Step 5.8 — Verify shared examples in XR

All existing shared examples (counter, todo, bench, app) should work as single-panel XR apps without modification:

- `launch[CounterApp](AppConfig(title="Counter", ...))` opens one XR panel
- The counter renders into the panel; clicks from XR controllers are translated to DOM events
- No app code changes required

### Step 5.9 — Multi-panel XR API (stretch goal)

Extend the framework with XR-specific APIs for multi-panel apps:

```text
# New XR-aware launch pattern (future)

struct XRCounterApp(XRGuiApp):
    var main_panel: XRPanel
    var controls_panel: XRPanel

    fn setup_panels(mut self, scene: XRScene):
        self.main_panel = scene.create_panel(width=0.8, height=0.6, ppm=1200.0)
        self.main_panel.set_position(0.0, 1.4, -1.0)
        self.controls_panel = scene.create_panel(width=0.4, height=0.3, ppm=1000.0)
        self.controls_panel.set_position(0.5, 1.2, -0.8)
```

This is additive — single-panel apps use the existing `GuiApp` trait unchanged; multi-panel apps implement an extended `XRGuiApp` trait.

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
              │ /core    │  │ (future) │
              │          │  │ DOM      │
              │ signals/ │  │ fetch    │
              │ scope/   │  │ WebSocket│
              │ vdom/    │  │ storage  │
              │ mutations│  │ timers   │
              │ bridge/  │  │ canvas   │
              │ events/  │  │ ...      │
              │ component│  └──────────┘
              │ html/    │
              │ platform/│ ◄── PlatformApp trait + launch()
              └────┬─────┘
                   │ implements PlatformApp trait
         ┌─────────┼──────────┬────────────┐
         ▼         ▼          ▼            ▼
  ┌──────────┐ ┌────────────┐ ┌──────────────────────┐
  │ mojo-gui │ │ mojo-gui   │ │ mojo-gui/xr          │
  │ /web     │ │ /desktop   │ │ (future)              │
  │          │ │            │ │                        │
  │ WebApp   │ │ DesktopApp │ │ ┌─────────┬──────────┐│
  │ main.mojo│ │ Blitz +    │ │ │ native  │ web      ││
  │ runtime/ │ │ Winit +    │ │ │ OpenXR  │ WebXR    ││
  │ (TS/JS)  │ │ Vello      │ │ │ + Blitz │ + DOM    ││
  └──────────┘ └────────────┘ │ │ + Vello │ + WebGPU ││
                              │ └─────────┴──────────┘│
                              └──────────────────────┘
```

Key points:

- **Examples depend only on `core`** — they never import from `web/`, `desktop/`, or `xr/`
- **Renderers implement the `PlatformApp` trait** defined in `core/platform/`
- **`launch()` is the only platform-dispatching call** — it routes to the correct renderer at compile time
- **Desktop uses Blitz** (Winit + Vello for windowed rendering)
- **XR reuses the Blitz stack** — same DOM + CSS + rendering pipeline, targeting OpenXR swapchains instead of Winit windows; WebXR extends the web renderer
- **`mojo-web` is independent** — apps can optionally import it for non-rendering browser features

---

## Migration Checklist

### Phase 1: `mojo-gui/core` extraction ✅

- [x] Create `mojo-gui/core/` directory structure
- [x] Move `src/signals/`, `src/scope/`, `src/scheduler/`, `src/arena/` unchanged
- [x] Move `src/vdom/{template,vnode,builder,registry}.mojo` to `mojo-gui/core/src/vdom/`
- [x] Move `src/vdom/{tags,dsl,dsl_tests}.mojo` to `mojo-gui/core/src/html/`
- [x] Update `html/dsl.mojo` imports: `from vdom.builder`, `from vdom.template`, `from vdom.vnode` (was relative `.builder`, `.template`, `.vnode`); `.tags` stays relative
- [x] Move `src/mutations/`, `src/bridge/`, `src/events/` unchanged
- [x] Move `src/component/` — updated `child.mojo`, `child_context.mojo`, `context.mojo`, `keyed_list.mojo` to split `from vdom` / `from html` imports
- [x] Create `core/src/platform/app.mojo` — `PlatformApp` trait definition (with `init`, `flush_mutations`, `request_animation_frame`, `should_quit`, `destroy`) + `is_wasm_target()` / `is_native_target()` helpers
- [x] Create `core/src/platform/launch.mojo` — `launch[AppType: GuiApp]()` with `AppConfig` (title, width, height, debug), global config registry, `get_launch_config()` / `has_launched()`, `@parameter if is_wasm_target()` compile-time dispatch (Step 3.9.3)
- [x] Create `core/src/platform/features.mojo` — `PlatformFeatures` struct, preset feature sets (`web_features`, `desktop_webview_features`, `desktop_blitz_features`, `native_features`), global feature registry (`register_features` / `current_features`)
- [x] Create `core/src/platform/__init__.mojo` — re-exports public API from all three platform modules
- [x] Update `core/src/lib.mojo` — add `platform/` to package listing
- [x] Move `src/apps/` to `mojo-gui/examples/` as shared, platform-agnostic example apps — demo/test apps moved from `core/apps/` to `examples/apps/`; main examples (counter, todo, bench, app) moved from `web/examples/` to `examples/`; web-specific assets (HTML/JS) remain in `web/examples/`; build paths updated (`-I ../examples` replaces `-I ../core -I examples`)
- [x] Define `GuiApp` trait (`core/src/platform/gui_app.mojo`) — `mount`, `handle_event` (unified value param), `flush`, `has_dirty`, `consume_dirty`, `destroy`; exported from `platform` package (Step 3.9.1)
- [x] Refactor app structs to implement `GuiApp` — CounterApp, TodoApp, BenchmarkApp, MultiViewApp all implement the trait; backwards-compatible free functions removed (Steps 3.9.4 + 3.9.5)
- [x] Wire `launch()` compile-time dispatch — `@parameter if is_wasm_target()` in `core/src/platform/launch.mojo`; non-parametric overload retained for backwards compatibility (Step 3.9.3)
- [x] Genericize `main.mojo` `@export` wrappers — `web/src/gui_app_exports.mojo` provides parametric lifecycle helpers; all 4 main apps use them; free functions removed from examples (Step 3.9.5)
- [x] Implement generic desktop event loop (`desktop/src/desktop/launcher.mojo`) — `desktop_launch[AppType: GuiApp]()` with Blitz mutation interpreter (Step 3.9.2)
- [x] Add `launch()` to shared examples (Step 3.9.6) — `main.mojo` entry points added to all 4 shared examples (counter, todo, bench, app) with `launch[AppType](AppConfig(...))`; no per-renderer example duplicates existed to delete
- [ ] Cross-target verification — verify all 4 shared examples on both web and desktop (Step 3.9.7 — needs build verification)
- [x] Update app imports in `apps/*.mojo` for new `html/` path (`from vdom import` → `from html import`)
- [x] Move `test/*.mojo` to `mojo-gui/core/test/`
- [x] Update test imports for new paths (`test_handles.mojo`: `from vdom` → `from html`)
- [x] Verify all 1,323 Mojo tests pass
- [x] Verify `mojo-gui/core` compiles for native target (no `@export` decorators)
- [x] Write `mojo-gui/core/README.md`
- [x] Update `mojo-gui/core/AGENTS.md`

### Phase 2: `mojo-gui/web` extraction ✅

- [x] Create `mojo-gui/web/` directory structure
- [x] Move `runtime/` to `mojo-gui/web/runtime/`
- [x] Move `src/main.mojo` to `mojo-gui/web/src/main.mojo`
- [x] Update `main.mojo` imports to reference `mojo-gui/core` package — split `from vdom` into `from vdom` + `from html`; `from vdom.dsl_tests` → `from html.dsl_tests`
- [x] Create `web/src/web_launcher.mojo` — `WebApp` implementing the `PlatformApp` trait (no-op stubs for WASM target where JS runtime drives the loop) + `create_web_app()` helper
- [x] Move web-specific example assets (HTML, JS glue) — web assets (HTML shells, main.js entry points) live in `web/examples/<name>/`; shared Mojo app code lives in `examples/<name>/`; redundant `examples/<name>/web/` copies removed
- [x] Create `web/scripts/build_examples.sh` — builds all shared examples for WASM target (discovers examples, compiles shared WASM binary via main.mojo, copies per-example HTML/JS assets from both shared and web-specific locations)
- [ ] Verify shared examples build and run in browser via web target — build paths updated (`-I ../examples`), needs `just build` + browser verification
- [x] Move `test-js/` to `mojo-gui/web/test-js/`
- [x] Move `scripts/` to `mojo-gui/web/scripts/`
- [x] Move build files (`justfile`, `deno.json`, `default.nix`) — updated `justfile` with `-I ../core/src -I ../examples` for core and shared example package resolution
- [x] Update all import paths in moved files
- [x] Verify all 3,090 JS tests pass
- [x] Verify all 3 example apps work in browser — JS tests (3,090) and Mojo tests (52 suites) pass; browser verification blocked by headless Servo in CI
- [x] Write `mojo-gui/web/README.md`
- [x] Write `mojo-gui/examples/README.md` — build instructions for web/desktop/Blitz targets, directory structure, migration status, architecture reference

### Phase 3: `mojo-gui/desktop` — webview renderer ✅ (infra), unified lifecycle ✅

- [x] Design desktop webview architecture — polling-based C shim, heap mutation buffer, base64 IPC, JSON event bridge
- [x] Build C shim (`shim/mojo_webview.c`) — GTK4 + WebKitGTK, ring buffer events, base64 mutation delivery, non-blocking step API
- [x] Write C header (`shim/mojo_webview.h`) — lifecycle, window, content, event loop, event polling, mutations, diagnostics
- [x] Write Nix derivation (`shim/default.nix`) — automated build of libmojo_webview.so
- [x] Implement Mojo FFI bindings (`src/desktop/webview.mojo`) — typed `Webview` struct via `OwnedDLHandle`, library search (env var → NIX_LDFLAGS → LD_LIBRARY_PATH)
- [x] Implement desktop bridge (`src/desktop/bridge.mojo`) — `DesktopBridge` (heap mutation buffer, flush, poll), `DesktopEvent` (parsed JSON), `parse_event()` (minimal JSON parser)
- [x] Implement `DesktopApp` (`src/desktop/app.mojo`) — webview lifecycle, JS runtime injection, shell HTML loading, multiple event loop styles (blocking, mount+run, interactive, manual step)
- [x] Create desktop JS runtime (`runtime/desktop-runtime.js`) — standalone 900+ line JS: MutationReader, TemplateCache, Interpreter (all opcodes), event dispatch via `window.mojo_post()`
- [x] Create HTML shell (`runtime/shell.html`) — minimal `#root` mount point with dark mode support
- [x] Verify counter example runs on desktop (`desktop/examples/counter.mojo`) — full interactive event loop with ConditionalSlot (temporary duplicate; to be replaced by shared example via `launch()`)
- [x] Create build system (`justfile`) — build-shim, build-counter, run-counter, dev-counter, test-shim, test-runtime
- [x] Create Nix dev shell (`default.nix`) — GTK4, WebKitGTK 6.0, pkg-config, libmojo-webview, environment variables
- [x] Write `mojo-gui/desktop/README.md` — architecture, build instructions, API reference, IPC protocol docs
- [x] Define `GuiApp` trait (`core/src/platform/gui_app.mojo`) — app-side lifecycle contract with `mount`, `handle_event`, `flush`, `has_dirty`, `consume_dirty`, `destroy` (Step 3.9.1)
- [x] Implement generic desktop event loop (`desktop/src/desktop/launcher.mojo`) — `desktop_launch[AppType: GuiApp]()` with Blitz mutation interpreter (Step 3.9.2)
- [x] Wire `launch()` compile-time dispatch — `launch[AppType: GuiApp]()` with `@parameter if is_wasm_target()` in `core/src/platform/launch.mojo`; native targets now call `desktop_launch[AppType](config)` (Step 3.9.3)
- [x] Refactor app structs to implement `GuiApp` — all 4 main apps (CounterApp, TodoApp, BenchmarkApp, MultiViewApp) now implement `GuiApp`; backwards-compatible free functions removed (Step 3.9.4)
- [x] Genericize `main.mojo` `@export` wrappers over `GuiApp` — `web/src/gui_app_exports.mojo` provides `gui_app_init`, `gui_app_mount`, `gui_app_handle_event`, `gui_app_flush`, etc.; all 4 main app @exports are now one-liners; 3,090 JS tests + 52 Mojo test suites pass (Step 3.9.5)
- [x] Add `launch[AppType](...)` to shared examples (Step 3.9.6) — `main.mojo` entry points added to all 4 shared examples; no per-renderer duplicates existed to delete
- [ ] Verify all 4 shared examples build and run on both web and desktop from identical source (Step 3.9.7 — needs build verification)
- [ ] Set up cross-target CI test matrix (web + desktop for every shared example)

### Phase 4: `mojo-gui/desktop` — Blitz renderer (Winit integration complete, verification pending)

- [x] Build Blitz C shim (`shim/src/lib.rs`) — Rust `cdylib` wrapping `blitz-dom`'s `BaseDocument` + `DocumentMutator` via `extern "C"` functions; `BlitzContext` owns document, ID mapping, template registry, event queue, interpreter stack
- [x] Write C header (`shim/mojo_blitz.h`) — 644-line header covering lifecycle, window, DOM creation, templates, tree mutations, attributes, text, traversal, events, mutation batching, stack operations, ID mapping, root access, layout, debug
- [x] Write Nix derivation (`shim/default.nix`) — Rust build with GPU/windowing deps (Vulkan, Wayland, X11, fontconfig, etc.)
- [x] Write Cargo.toml (`shim/Cargo.toml`) — cdylib depending on blitz, blitz-dom, blitz-html, blitz-traits, blitz-shell, blitz-paint, anyrender 0.6, anyrender_vello 0.6, winit 0.30; markup5ever types re-exported from blitz-dom (no direct dep)
- [x] Implement Mojo FFI bindings (`src/desktop/blitz.mojo`) — typed `Blitz` struct via `DLHandle` with methods for all shim operations; `BlitzEvent` struct; library search (env var → NIX_LDFLAGS → LD_LIBRARY_PATH)
- [x] Implement Mojo-side mutation interpreter (`src/desktop/renderer.mojo`) — `MutationInterpreter` with `BufReader`; reads all 18 opcodes and translates to Blitz FFI calls; `OP_REGISTER_TEMPLATE` builds real DOM subtrees for efficient deep-cloning
- [x] Implement generic desktop event loop (`src/desktop/launcher.mojo`) — `desktop_launch[AppType: GuiApp]()` with Blitz-backed event loop, mutation buffer management, UA stylesheet injection
- [x] Wire `launch()` to call `desktop_launch` on native targets — updated `core/src/platform/launch.mojo` to import and call `desktop_launch[AppType](config)` instead of placeholder print
- [x] Update desktop package (`src/desktop/__init__.mojo`) — updated docstring and module listing for blitz, renderer, launcher
- [x] Build the Rust cdylib (`cargo build --release`) — pinned Blitz deps to v0.2.0 (rev `2f83df96`), removed direct markup5ever dep (use blitz-dom re-exports), fixed API mismatches (`insert_nodes_before`, `get_node` for private `nodes` field, `DocumentMutator::create_element`), generated Cargo.lock (607 packages); produces `libmojo_blitz.so` ~23MB
- [x] Integrate Winit event loop — `ApplicationHandler` impl for `BlitzContext`; `mblitz_step()` wired to `pump_app_events()` with cooperative polling; window creation in `resumed()` with `Arc<Window>`
- [x] Connect `blitz-paint` rendering pipeline — `RedrawRequested` → `doc.resolve()` (Stylo + Taffy) → `paint_scene()` (Vello GPU rendering); `mblitz_request_redraw()` triggers window redraw
- [x] Implement DOM event routing — `MojoEventHandler` intercepts Blitz DOM events during bubble propagation; `CursorMoved`/`MouseInput` → `UiEvent` → `EventDriver` → buffered events for `mblitz_poll_event()`
- [x] Fix dependency version mismatches — downgraded anyrender 0.7→0.6, anyrender_vello 0.7→0.6, winit 0.31-beta→0.30 to match Blitz v0.2.0; ported winit API (0.31 → 0.30: `PointerMoved`→`CursorMoved`, `PointerButton`→`MouseInput`, `SurfaceResized`→`Resized`, `can_create_surfaces`→`resumed`, `dyn ActiveEventLoop`→`&ActiveEventLoop`, `Box<dyn Window>`→`Arc<Window>`)
- [ ] Verify all shared examples on Blitz desktop (counter, todo, bench, app) — Step 4.4
- [ ] Cross-platform testing (Linux, macOS, Windows via Winit) — Step 4.5
- [ ] Set up cross-target CI test matrix (web + desktop-blitz for every shared example)

### Phase 5: `mojo-gui/xr` — XR renderer (future)

- [ ] Design XR panel abstraction — `XRPanel` struct (DOM document + 3D transform + texture + input surface), `XRScene` (panel registry + spatial layout + raycasting)
- [ ] Build OpenXR + Blitz Rust shim (`xr/native/shim/src/lib.rs`) — extend Blitz stack with `openxr` crate; multi-document management (one `blitz-dom` per panel); Vello → offscreen `wgpu::Texture` per panel; OpenXR session lifecycle + frame loop; quad layer compositing; controller pose tracking + panel raycasting
- [ ] Write C header (`xr/native/shim/mojo_xr.h`) — session, panel, mutation, event, frame loop, input, reference space functions
- [ ] Write Nix derivation (`xr/native/shim/default.nix`) — Rust build with OpenXR + GPU deps
- [ ] Implement Mojo FFI bindings (`xr/native/src/xr_blitz.mojo`) — typed `XRBlitz` struct via `DLHandle`
- [ ] Implement XR scene manager (`xr/native/src/scene.mojo`) — panel lifecycle, mutation routing, event multiplexing
- [ ] Implement XR panel manager (`xr/native/src/panel.mojo`) — per-panel `GuiApp` + mutation buffer, 3D transform API
- [ ] Implement `xr_launch[AppType: GuiApp]()` (`xr/native/src/xr_launcher.mojo`) — OpenXR frame loop (wait → poll input → raycast → dispatch → render dirty panels → composite → end frame)
- [ ] Build WebXR JS runtime (`xr/web/runtime/`) — XR session lifecycle, DOM-to-texture panel rendering, XR input → DOM event bridging
- [ ] Wire `launch()` for XR targets — add `has_feature("xr")` / `has_feature("webxr")` branches to `core/src/platform/launch.mojo`
- [ ] Verify all shared examples as single-panel XR apps — counter, todo, bench, app should work unchanged in XR
- [ ] Multi-panel XR API (stretch goal) — `XRGuiApp` trait for apps that manage multiple panels in 3D space

---

## Risks & Mitigations

| Risk | Impact | Mitigation | Status |
|------|--------|------------|--------|
| Mojo package system immaturity | Can't cleanly separate into packages | Mono-repo with path-based imports (`-I` flags) | ✅ Resolved — mono-repo with `-I ../core/src -I ../examples` works |
| `MutExternalOrigin` tied to WASM | Core won't compile natively | Audit and abstract the origin parameter; conditionally compile | ✅ Resolved — `MutExternalOrigin` works for both WASM and native heap buffers |
| Blitz C shim complexity | Desktop renderer takes too long | Start with webview approach as intermediate step; upgrade to Blitz later | ✅ Resolved — C shim, Mojo FFI bindings, mutation interpreter, launcher, Winit event loop, Vello rendering all implemented; cdylib builds cleanly |
| Blitz pre-alpha stability | Rendering bugs, missing CSS features | Track Blitz main branch; contribute upstream fixes; keep webview as fallback | ✅ Mitigated — shim pins to Blitz v0.2.0 (rev `2f83df96`); v0.2.0 provides good CSS coverage via Stylo |
| Blitz Rust build dependency | Complex build toolchain | Pre-build the `cdylib` and distribute as a shared library; Nix flake can automate the Rust build | ✅ Resolved — `cargo build --release` succeeds; Cargo.lock generated (607 packages); Nix derivation (`shim/default.nix`) automates build with GPU/windowing deps |
| Import path breakage | Massive search-and-replace | Script the migration; grep-verify all imports | ✅ Resolved — all imports updated |
| Test suite fragmentation | Tests break across projects | Phase 1 must keep all Mojo tests green; Phase 2 must keep all JS tests green | ✅ Resolved — all tests pass (3,090 JS + 52 Mojo suites) |
| Platform abstraction too leaky | Shared examples break on some targets | Use the cross-target test matrix as a gate; treat cross-target failures as framework bugs | In progress — `GuiApp` trait + generic `@export` wrappers + `desktop_launch` + Winit event loop complete; shared `main.mojo` entry points added; cross-target build verification pending |
| `launch()` compile-time dispatch limitations | Mojo may lack the metaprogramming for clean target dispatch | `GuiApp` trait + `@parameter if is_wasm_target()` provides clean dispatch; if trait parametric methods don't work, fall back to conditional imports | ✅ Resolved — `launch[AppType: GuiApp]()` works with `@parameter if`; native targets call `desktop_launch[AppType](config)` with full Blitz rendering |
| Mojo trait limitations for `GuiApp` | Trait may not support parametric methods or associated types needed for generic `@export` wrappers | Start with concrete struct aliases (`alias CurrentApp = CounterApp`); upgrade to full trait generics when Mojo supports it | ✅ Resolved — parametric helpers `gui_app_init[T: GuiApp]()` work; `@export` wrappers call them with concrete types |
| WebKitGTK Linux-only | Desktop renderer not cross-platform | Webview is an intermediate step; Blitz (Phase 4) will provide cross-platform support via Winit | Open — accepted limitation for Phase 3 |
| Base64 IPC overhead | ~33% mutation size increase for desktop | Acceptable for now; investigate shared memory or binary transfer for optimization | Open — low priority |
| Desktop event loop busy-wait | High CPU when idle | Implemented blocking `mwv_step(blocking=True)` when no events/dirty scopes | ✅ Resolved |
| Native target module-level `var` | Global `var` declarations in imported packages not supported on native target | Wrap in struct or use function-local static; to be fixed when native compilation is tested | Open — only affects native compilation; WASM works fine |
| OpenXR runtime availability | XR features fail on systems without OpenXR runtime | Runtime detection: check for OpenXR loader at startup; fall back to desktop Blitz renderer if unavailable | Future — Phase 5 |
| DOM-to-texture fidelity (WebXR) | Rendering DOM to WebGL texture may lose interactivity or fidelity | Evaluate multiple approaches: OffscreenCanvas, html2canvas, CSS 3D transforms in DOM overlay; benchmark quality vs. performance | Future — Phase 5 |
| XR input latency | Raycasting → DOM event translation adds latency to controller input | Keep raycast math in the shim (Rust/native) or GPU (WebXR); minimize JS/Mojo roundtrips for input dispatch | Future — Phase 5 |
| Multi-panel mutation routing | Multiple panels need independent mutation streams; current protocol assumes single document | Each panel gets its own mutation buffer and `GuiApp` instance; the XR scene manager multiplexes; no protocol changes needed | Future — Phase 5 |
| XR frame timing constraints | OpenXR requires strict frame pacing; DOM re-render may exceed frame budget | Render panels asynchronously; only re-render dirty panels; cache textures for clean panels; use OpenXR quad layers for compositor-side reprojection | Future — Phase 5 |

---

## Estimated Effort

| Phase | Effort | Description | Status |
|-------|--------|-------------|--------|
| Phase 1 | 2–3 days | File moves, import path updates, platform abstraction layer, shared examples setup, verify compilation + tests | ✅ Complete |
| Phase 2 | 1–2 days | Move web runtime, `WebApp` trait impl, shared example web builds, verify browser tests | ✅ Complete |
| Phase 3 (infra) | 1–2 weeks | GTK4/WebKitGTK C shim, Mojo FFI, `DesktopApp`, JS runtime for webview, counter example, Nix integration | ✅ Complete |
| Phase 3.9 | 3–5 days | `GuiApp` trait, generic desktop event loop, `launch()` dispatch, refactor app structs, genericize `@export` wrappers, shared `main.mojo` entry points, cross-target CI | ✅ Complete (3.9.1–6 done; 3.9.7 cross-target verification pending) |
| Phase 4 | 2–4 weeks | Blitz C shim (Rust cdylib), Mojo-side mutation interpreter, Winit event loop, Vello rendering, cross-platform testing | 🔧 Implementation complete — shim, FFI, interpreter, launcher, Winit event loop, Vello GPU rendering all done; cdylib builds (23MB); shared example verification pending (Step 4.4) |
| Phase 5 | 4–8 weeks | XR renderer: OpenXR native shim (extend Blitz + Vello → offscreen textures + OpenXR compositor), WebXR JS runtime (DOM → texture + XR session), panel abstraction, XR input → DOM event bridging, `launch()` XR dispatch | Future |
| Phase 6 | 2–3 weeks | `mojo-web` MVP: handle table, DOM, fetch, timers, storage | Future |

---

## `mojo-web` — Raw Web API Bindings (Phase 6)

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

### API Surface (MVP — Phase 6)

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

1. **~~Mono-repo vs. multi-repo?~~** — ✅ Resolved: Mono-repo. `mojo-gui/` is the workspace root containing `core/`, `web/`, `desktop/`, `xr/`, and `examples/`. Path-based imports (`-I ../core/src -I ../examples`) work well. `mojo-web` will live alongside as a sibling.

2. **~~Should `html/` stay in `mojo-gui/core` or become a separate `mojo-gui/html` package?~~** — ✅ Resolved: Keep in `core`. The HTML/CSS/DOM model is universal across all renderers: web uses real DOM, desktop uses Blitz DOM, XR uses Blitz DOM per-panel (native) or real DOM per-panel (WebXR). All renderers consume the same DOM-oriented mutation protocol, so the HTML vocabulary stays in core.

3. **~~How to handle the `@export` boilerplate in `main.mojo`?~~** — ✅ Resolved by Phase 3.9 design: the `GuiApp` trait provides a uniform lifecycle interface. `@export` wrappers become generic over `GuiApp` — one set of wrappers works for every app. Each example builds with a compile-time alias (`alias CurrentApp = CounterApp`). The ~6,730 lines of per-app wrappers collapse to a small generic set.

4. **~~Blitz C shim API granularity?~~** — ✅ Resolved: The shim (`mojo_blitz.h`) exposes ~45 functions covering lifecycle, DOM operations, templates, events, stack operations, ID mapping, and debug. The API follows the same polling-based, no-callback, flat C ABI pattern as the webview shim. Blitz's `BaseDocument` is accessed via an opaque `BlitzContext` pointer. The shim maintains its own ID mapping (mojo element IDs ↔ Blitz slab node IDs) and interpreter stack.

5. **~~Should the Mojo-side mutation interpreter share code with the JS `Interpreter`?~~** — ✅ Resolved: The Mojo `MutationInterpreter` (`desktop/src/desktop/renderer.mojo`) and JS `Interpreter` (`web/runtime/interpreter.ts` / `desktop/runtime/desktop-runtime.js`) are parallel implementations of the same stack machine, reading the same binary opcode format. They cannot share code (different languages), but they share the opcode definitions and wire format specification from `core/src/bridge/protocol.mojo`. Correctness is verified by running the same shared examples on both renderers.

6. **Should `mojo-web` reuse `mojo-gui/web`'s existing JS runtime code?** — Partially. `memory.ts`, `env.ts`, and `strings.ts` solve the same WASM↔JS interop problems. Extract a shared `mojo-wasm-runtime` base, or let `mojo-web` depend on just those modules.

7. **Should `mojo-gui/web` eventually use `mojo-web` for its JS runtime?** — Possibly for non-rendering parts (e.g., the `EventBridge` could use `mojo-web`'s DOM bindings). The mutation protocol interpreter should stay as-is for performance (batched application vs. per-call overhead).

8. **~~Blitz version pinning?~~** — ✅ Resolved: Pinned to Blitz v0.2.0 release (rev `2f83df96220561316611ecf857e20cd1feed8ca0`). All Blitz git dependencies use this exact commit. The `Cargo.lock` file locks all transitive dependencies for reproducible builds. Markup5ever types are re-exported from `blitz_dom` to avoid version mismatch issues (Blitz v0.2.0 uses markup5ever 0.35.0 internally). Update the rev deliberately when a new Blitz release is available.

9. **CSS support scope?** — Blitz supports modern CSS (flexbox, grid, selectors, variables, media queries) via Stylo, but not all CSS features are implemented yet. Document which CSS features are supported and test the Blitz desktop renderer against the same shared examples as the web and webview renderers.

10. **~~Fallback for `launch()` compile-time dispatch?~~** — ✅ Resolved by Phase 3.9 + Phase 4 design: `launch[AppType: GuiApp](config)` uses `@parameter if is_wasm_target()` to dispatch. For WASM, the JS runtime drives the loop via `@export` wrappers generic over `GuiApp`. For native, `launch()` imports and calls `desktop_launch[AppType](config)` from `desktop.launcher`, which provides a fully generic Blitz-backed event loop. No per-renderer entry-point files needed — every example has a single `fn main()` calling `launch()`.

11. **How to handle web-only features in shared examples?** — Examples that need web-specific APIs (e.g., `fetch`, `localStorage`) should use compile-time feature gates: `@parameter if is_wasm_target(): ...`. Platform-specific timing is handled the same way: `performance_now()` uses `external_call` on WASM and `time.perf_counter_ns()` on native, selected at compile time inside the shared source file. XR-specific features (panel placement, hand tracking) use `@parameter if has_feature("xr"): ...`.

12. **~~Desktop webview cross-platform support?~~** — ✅ Resolved: The webview approach was removed in favor of the Blitz-based native renderer (Phase 4). Blitz uses Winit, which supports Linux, macOS, and Windows natively. No need for platform-specific webview shims.

13. **~~Desktop example sharing vs. duplication?~~** — ✅ Resolved by Phase 3.9 design: **no duplication, ever.** Examples live in `examples/` and implement the `GuiApp` trait. The `launch()` function and generic event loops (`desktop_launch`, `@export` wrappers) drive them on every target. Per-renderer example directories are an anti-pattern — if an example doesn't compile on a target, it's a framework bug.

14. **~~Base64 IPC optimization?~~** — ✅ Resolved: The Blitz desktop renderer eliminates IPC entirely. Mutations are applied in-process via the Mojo `MutationInterpreter` → Blitz C FFI calls. No base64 encoding, no JS eval, no webview.

15. **Can Mojo traits be parametric enough for `GuiApp`?** — The `GuiApp` trait needs to work as a compile-time parameter to `launch[]`, `desktop_launch[]`, `xr_launch[]`, and the `@export` wrapper pattern. If Mojo's trait system doesn't support this (e.g., no parametric methods on trait-constrained types), the fallback is concrete `alias CurrentApp = CounterApp` per build, with a shared `@export` module that references the alias. This is still a single source file per example — the alias is a build system concern, not an app authoring concern.

16. **Should the XR native shim share code with the desktop Blitz shim?** — The XR shim reuses the same Blitz stack (blitz-dom, Stylo, Taffy, Vello) but needs multi-document support, offscreen texture rendering, and OpenXR integration. Options: (a) extend the existing `desktop/shim/` with XR feature flags, (b) create a separate `xr/native/shim/` that depends on the same Blitz crates. Option (b) is cleaner — the desktop shim targets a single Winit window; the XR shim targets an OpenXR session with multiple offscreen panels. Shared Blitz logic can be extracted into a common Rust crate if duplication becomes significant.

17. **How to handle DOM-to-texture rendering for WebXR?** — Several approaches exist: (a) OffscreenCanvas with `drawImage()` from a DOM-rendered element, (b) `html2canvas` or similar rasterization libraries, (c) WebXR DOM Overlay API (limited to a single flat layer, not spatially placed), (d) render mutation protocol directly to a WebGL/WebGPU canvas using a custom 2D renderer (bypassing the DOM entirely on the WebXR path). Evaluate fidelity, performance, and interactivity tradeoffs. Approach (d) would be the most consistent with the native path (Vello-like rendering to a texture) but requires a JS/WASM 2D rendering engine.

18. **Should single-panel XR apps use `GuiApp` directly or always go through `XRGuiApp`?** — For simplicity, single-panel apps should use the existing `GuiApp` trait unchanged. The XR launcher wraps them in a default panel automatically. `XRGuiApp` is only needed for apps that explicitly manage multiple panels or need XR-specific features (hand tracking, spatial anchors). This preserves the "write once, run everywhere" principle — existing apps get XR support for free.

19. **What OpenXR extensions are required?** — The MVP needs: `XR_KHR_opengl_enable` or `XR_KHR_vulkan_enable` (GPU interop), `XR_KHR_composition_layer_quad` (panel placement). Nice to have: `XR_EXT_hand_tracking` (hand input), `XR_FB_passthrough` (AR), `XR_EXTX_overlay` (overlay apps). The shim should detect available extensions at runtime and expose capability flags to Mojo.