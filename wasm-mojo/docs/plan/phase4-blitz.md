# Phase 4: Desktop Blitz Renderer (Builds Verified, Runtime Pending)

Replace the webview dependency in the desktop renderer with [Blitz](https://github.com/DioxusLabs/blitz), a native HTML/CSS rendering engine. This is the same evolution Dioxus followed — webview first, then Blitz for native rendering without a browser engine.

**See also:** [Architecture & Design](./architecture.md) · [Phase 3 (Desktop Webview)](./phase3-desktop.md) · [Checklist](./checklist.md)

---

## Overview

Blitz is a radically modular HTML/CSS rendering engine that provides:

- **Stylo** (Firefox's CSS engine) — CSS parsing and style resolution
- **Taffy** — Flexbox, grid, and block layout
- **Parley** — Text layout and shaping
- **Vello** via **anyrender** — GPU-accelerated 2D rendering
- **Winit** — Cross-platform windowing and input
- **AccessKit** — Accessibility

Blitz provides a real DOM (`blitz-dom`) without requiring a browser or webview. The mutation protocol maps naturally to Blitz's DOM operations.

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

**Advantages of Blitz over the webview approach:**

- **No JS runtime** — no need to bundle or inject JavaScript; the mutation interpreter runs in Mojo
- **No IPC overhead** — mutations are applied in-process via direct FFI calls, not base64-encoded over webview eval
- **Smaller binary** — no browser engine dependency (WebKitGTK is ~50+ MB); Blitz is much lighter
- **Cross-platform** — Blitz uses Winit, which supports Linux, macOS, and Windows natively
- **Better integration** — native window chrome, system menus, accessibility via AccessKit
- **Consistent rendering** — Stylo (Firefox's CSS engine) provides standards-compliant CSS everywhere

---

## Step 4.1 — Build Blitz C shim (`shim/src/lib.rs`) ✅

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

---

## Step 4.2 — Implement Mojo FFI bindings (`desktop/src/desktop/blitz.mojo`) ✅

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

---

## Step 4.3 — Implement Mojo-side mutation interpreter (`desktop/src/desktop/renderer.mojo`) ✅

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

---

## Step 4.3.1 — Build the Rust cdylib ✅

Resolved all Blitz dependency issues and successfully built the `libmojo_blitz.so` shared library:

- **Fixed `rev = "main"`** — `rev` requires a commit hash, not a branch name. Pinned all Blitz dependencies to the v0.2.0 release commit `2f83df96220561316611ecf857e20cd1feed8ca0`.
- **Fixed markup5ever version mismatch** — removed direct `markup5ever = "0.37.0"` dependency (which resolved to 0.37.1) because Blitz v0.2.0 internally uses markup5ever 0.35.0. All markup5ever types (`QualName`, `LocalName`, `Prefix`, `local_name!`, `ns!`) are now imported via `blitz_dom`'s re-exports.
- **Fixed API mismatches** — `insert_before()` → `insert_nodes_before()` (Blitz's DocumentMutator API); `doc.nodes[id]` → `doc.get_node(id)` (the `nodes` slab is private on `BaseDocument`); `BlitzContext::create_element` now uses `DocumentMutator::create_element` for proper stylo data initialization.
- **Fixed `node_at_path`** — was incorrectly calling `self.doc.mutate()` on `&self` (mutate requires `&mut self`). Reimplemented using `doc.get_node()` traversal.
- **Generated `Cargo.lock`** — reproducible builds with all transitive dependency versions locked.
- **Build output:** `libmojo_blitz.so` ~8MB ELF 64-bit x86-64 shared library (release profile, `opt-level = 2`, thin LTO, stripped symbols).

---

## Step 4.4 — Verify all shared examples — builds ✅, runtime pending

Every example that works on web MUST work on desktop-Blitz. The app code is identical — only the renderer backend changes. Each example now has a `main.mojo` entry point with `launch[AppType](config)`.

### Build verification ✅

All 4 shared examples compile for **both** desktop-Blitz and web from identical source:

- [x] Counter example builds on desktop (`mojo build examples/counter/main.mojo -I core/src -I desktop/src -I examples`)
- [x] Todo example builds on desktop
- [x] Bench example builds on desktop
- [x] Multi-view app example builds on desktop
- [x] Web build (`just build` in `web/`) still succeeds with all shared examples
- [x] All 3,090 JS tests pass
- [x] All 52 Mojo test suites pass

### Mojo 0.26.1 API migration (completed as part of build verification)

The Mojo FFI and platform modules required updates for Mojo 0.26.1 compatibility:

| Change | Files affected |
|--------|---------------|
| `info.os_is_wasi()` → `is_defined["MOJO_TARGET_WASM"]()` | `core/src/platform/app.mojo` |
| `alias` → `comptime` for module-level constants | `desktop/src/desktop/renderer.mojo`, `launcher.mojo`, `blitz.mojo` |
| `DLHandle` → `_DLHandle` (from `sys.ffi`) | `desktop/src/desktop/blitz.mojo` |
| `env_get_string()` → `getenv()` (from `os`) | `desktop/src/desktop/blitz.mojo` |
| `UnsafePointer[T]` → `UnsafePointer[T, Origin]` (explicit origin) | `blitz.mojo`, `renderer.mojo`, `launcher.mojo` |
| `UnsafePointer[T].alloc(n)` → `alloc[T](n)` (standalone function) | `blitz.mojo`, `renderer.mojo`, `launcher.mojo` |
| `UnsafePointer.address_of(x)` → `UnsafePointer(to=x)` | `desktop/src/desktop/renderer.mojo` |
| `s[i]` → `s[byte=i]` for string byte access | `desktop/src/desktop/blitz.mojo` |
| `List[T]` implicit copy → explicit `.copy()` or `^` transfer | `desktop/src/desktop/renderer.mojo` |
| `from platform import launch` → `from platform.launch import launch` | All 4 shared example `main.mojo` files |
| Circular dependency break: `from platform import` → `from platform.gui_app import` / `from platform.launch import` | `desktop/src/desktop/launcher.mojo` |

**Known Mojo 0.26.1 issue:** Re-exporting parametric functions through `__init__.mojo` triggers a "not subscriptable" error. Workaround: import `launch` directly from `platform.launch` instead of `platform`. All other symbols re-export correctly. Documented in `core/src/platform/__init__.mojo`.

### Runtime verification — pending

- [ ] Counter example runs interactively on desktop (requires `libmojo_blitz.so` + GPU)
- [ ] Todo example runs interactively on desktop
- [ ] Bench example runs interactively on desktop
- [ ] Multi-view app example runs interactively on desktop

---

## Step 4.5 — Cross-platform support

Blitz uses Winit, which supports Linux, macOS, and Windows. Verify the Blitz renderer works on all three platforms (the previous webview renderer was Linux-only due to GTK4/WebKitGTK).

---

## Step 4.6 — Winit event loop integration ✅

Implemented full Winit event loop integration in the Blitz C shim (`shim/src/lib.rs`). The `mblitz_step()` function is no longer a placeholder — it drives the real windowing and rendering pipeline:

1. ✅ **`ApplicationHandler` impl for `BlitzContext`** — `resumed()` creates the Winit window with `Arc<Window>`, initializes the Vello GPU renderer via `anyrender_vello::VelloWindowRenderer`, and updates the document viewport. Re-resume after suspend is also handled.
2. ✅ **`mblitz_step(blocking)` wired to `pump_app_events()`** — the `EventLoop<()>` is stored in an `Option` and temporarily taken out during each step to avoid borrow conflicts (the same struct serves as both the event loop owner and the `ApplicationHandler`). Non-blocking mode uses `Duration::ZERO`; blocking mode uses 100ms timeout for periodic checks.
3. ✅ **Winit window event routing** — `handle_winit_event()` processes `CloseRequested`, `RedrawRequested`, `Resized`, `ScaleFactorChanged`, `CursorMoved`, and `MouseInput` events. Mouse events are translated to Blitz `UiEvent` variants (`MouseMove`, `MouseDown`, `MouseUp`) with tracked button state and logical coordinates.
4. ✅ **DOM event extraction via `MojoEventHandler`** — custom `EventHandler` implementation intercepts Blitz DOM events during bubble propagation, maps `DomEventData` variants (Click, Input, KeyDown, etc.) to mojo-gui handler IDs, and buffers them in `event_queue` for polling via `mblitz_poll_event()`. Disjoint borrows are managed via raw pointers to split `event_handlers` and `event_queue` from the `DocumentMutator`.
5. ✅ **GPU rendering via Vello + blitz-paint** — `RedrawRequested` triggers `doc.resolve(0.0)` for style resolution + layout (Stylo + Taffy), then `paint_scene()` renders the document to the Vello scene. `mblitz_request_redraw()` sets a flag and calls `window.request_redraw()`.

**Dependency version fixes:** The original `Cargo.toml` specified `anyrender 0.7`, `anyrender_vello 0.7`, and `winit 0.31-beta`, which caused version mismatches with Blitz v0.2.0's internal dependencies (`anyrender 0.6`, `winit 0.30`). Fixed by downgrading to match Blitz's pinned versions: `anyrender 0.6`, `anyrender_vello 0.6`, `winit 0.30`. This also required porting the code from winit 0.31 API (`PointerMoved`, `PointerButton`, `SurfaceResized`, `can_create_surfaces`, `dyn ActiveEventLoop`, `Box<dyn Window>`) to winit 0.30 API (`CursorMoved`, `MouseInput`, `Resized`, `resumed`, concrete `&ActiveEventLoop`, `Arc<Window>`). The `renderer.resume()` call was updated to pass `Arc<dyn anyrender::WindowHandle>` as required by anyrender 0.6.

**Build output:** `libmojo_blitz.so` ~23MB ELF 64-bit x86-64 shared library (release profile, `opt-level = 2`, thin LTO, stripped symbols). Clean build with zero warnings. `Cargo.lock` generated with 607 packages (down from 649 before the version fix — no more duplicate dependency trees).

---

## Key Files

| File | Purpose |
|------|---------|
| `desktop/shim/src/lib.rs` | Rust `cdylib`: `BlitzContext` wrapping `blitz-dom`, ID mapping, template registry, event queue, interpreter stack |
| `desktop/shim/mojo_blitz.h` | C API header (~45 FFI functions: lifecycle, DOM, templates, events, stack, debug) |
| `desktop/shim/Cargo.toml` | Rust crate config (blitz-dom, blitz-html, blitz-traits, blitz-shell, blitz-paint, winit, etc.) |
| `desktop/shim/default.nix` | Nix derivation with GPU/windowing deps (Vulkan, Wayland, X11, fontconfig) |
| `desktop/src/desktop/blitz.mojo` | Mojo FFI bindings to `libmojo_blitz_shim.so` via `_DLHandle` |
| `desktop/src/desktop/renderer.mojo` | `MutationInterpreter`: reads binary opcodes → Blitz C FFI calls (all 18 opcodes) |
| `desktop/src/desktop/launcher.mojo` | `desktop_launch[AppType: GuiApp]()` — generic Blitz-backed event loop |

---

## Remaining Work

1. **Step 4.4 runtime** — Verify all 4 shared examples run interactively on desktop-Blitz (requires `libmojo_blitz.so` build + GPU)
2. **Step 4.5** — Cross-platform testing (Linux, macOS, Windows via Winit)
3. **Cross-target CI** — Set up CI matrix testing web + desktop-Blitz for every shared example