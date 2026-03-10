# mojo-gui/desktop — Desktop Renderer

Native desktop GUI backend for **mojo-gui** using GTK4 + WebKitGTK.

The same mojo-gui/core reactive framework (signals, virtual DOM, diff engine) drives the UI — but instead of compiling to WASM and running in a browser, the Mojo code runs as a **native binary** with an embedded webview for rendering.

## Architecture

```text
┌─ Native Mojo Process ─────────────────────────────────────────────┐
│                                                                    │
│  User App (counter.mojo, todo.mojo, ...)                          │
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
│    ├── Owns heap-allocated mutation buffer                         │
│    ├── flush_mutations() → base64 encode → webview eval            │
│    └── poll_event() ← JSON ← ring buffer ← JS                    │
│         │                            ▲                             │
│         ▼                            │                             │
│  ┌─ Embedded Webview (GTK4 + WebKitGTK) ──────────────────────┐   │
│  │                                                             │   │
│  │  desktop-runtime.js                                         │   │
│  │    ├── MutationReader (decodes binary protocol)             │   │
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

### How it works

1. **Mojo compiles to a native binary** (not WASM). The core framework runs at full native speed.

2. **Mutations flow Mojo → Webview**: The `MutationWriter` writes the same binary opcode stream as the web renderer, but into a heap buffer instead of WASM linear memory. The `DesktopBridge` sends this buffer to the webview via base64-encoded `webview_eval()` calls. The JS `Interpreter` inside the webview decodes and applies the mutations to the real DOM.

3. **Events flow Webview → Mojo**: DOM events are captured by the JS runtime, serialized as minimal JSON (`{"h":42,"t":0}`), and sent to the native side via `window.mojo_post()`. The C shim buffers these in a lock-free ring buffer. Mojo polls them with `poll_event()` and dispatches to the core framework's `HandlerRegistry`.

4. **The C shim** (`libmojo_webview.so`) wraps GTK4 + WebKitGTK and provides a Mojo-friendly polling API. No function-pointer callbacks — everything is poll-based, which maps cleanly to Mojo's FFI model (`DLHandle`).

### Comparison with the web renderer

| Aspect | Web (`mojo-gui/web`) | Desktop (`mojo-gui/desktop`) |
|--------|---------------------|------------------------------|
| Mojo target | `wasm64-wasi` | Native (default) |
| Mutation buffer | WASM linear memory | Heap buffer |
| JS runtime | Loaded by browser | Injected into webview |
| Event delivery | WASM export calls | JSON over ring buffer |
| Entry point | `@export` wrappers | `fn main()` |
| Performance | WASM overhead | Native speed + IPC overhead |

The key insight: **the user's app code is identical**. Only the entry point and renderer differ — exactly like Dioxus for Rust.

## Directory Structure

```text
desktop/
├── src/
│   ├── __init__.mojo         # Package root
│   ├── webview.mojo           # Mojo FFI bindings to libmojo_webview.so
│   ├── bridge.mojo            # Mutation buffer + event polling bridge
│   └── app.mojo               # DesktopApp entry point and event loop
├── runtime/
│   ├── desktop-runtime.js     # Standalone JS interpreter for the webview
│   └── shell.html             # HTML shell with #root mount point
├── shim/
│   ├── mojo_webview.h         # C API header
│   ├── mojo_webview.c         # C implementation (GTK4 + WebKitGTK)
│   └── default.nix            # Nix derivation for building the shim
├── examples/
│   └── counter.mojo           # Desktop counter demo
├── default.nix                # Nix dev shell with all dependencies
├── justfile                   # Build commands
└── README.md                  # This file
```

## Prerequisites

- **GTK4** and **WebKitGTK 6.0** development libraries
- **pkg-config** (for C compilation)
- **Mojo** compiler (≥ 0.26.1)
- **Deno** (optional, for JS syntax checking)

On NixOS / with Nix:

```sh
# The desktop dev shell provides everything:
nix develop .#mojo-gui-desktop
```

On Ubuntu/Debian:

```sh
sudo apt install libgtk-4-dev libwebkitgtk-6.0-dev pkg-config
```

On Fedora:

```sh
sudo dnf install gtk4-devel webkitgtk6.0-devel pkg-config
```

## Quick Start

```sh
cd mojo-gui/desktop

# 1. Build the C shim library
just build-shim

# 2. Run the counter example
just run-counter
```

Or step by step:

```sh
# Build the C shim
just build-shim

# Build the counter example as a native binary
just build-counter

# Set environment variables and run
export MOJO_WEBVIEW_LIB=$(pwd)/build/libmojo_webview.so
export MOJO_GUI_DESKTOP_RUNTIME=$(pwd)/runtime/desktop-runtime.js
export LD_LIBRARY_PATH=$(pwd)/build:$LD_LIBRARY_PATH
./build/counter
```

For development iteration:

```sh
# Run directly with `mojo run` (no separate compile step)
just dev-counter
```

## Writing a Desktop App

A desktop app follows the same pattern as a web app, with a different entry point:

```mojo
from memory import UnsafePointer, memset_zero
from bridge.protocol import MutationWriter
from component import ComponentContext
from signals.handle import SignalI32
from html import el_div, el_h1, el_button, text, dyn_text, onclick_add

from desktop.app import DesktopApp
from desktop.bridge import DesktopEvent

fn main() raises:
    # 1. Create the desktop app (opens a GTK window with webview)
    var desktop = DesktopApp(
        title="My App",
        width=800,
        height=600,
    )

    # 2. Create your mojo-gui/core component (same as web!)
    var ctx = ComponentContext.create()
    var count = ctx.use_signal(0)
    ctx.setup_view(
        el_div(
            el_h1(dyn_text()),
            el_button(text("Click me"), onclick_add(count, 1)),
        ),
        String("my-app"),
    )

    # 3. Initialize the webview + JS runtime
    desktop.init()

    # 4. Mount initial DOM
    var buf = desktop.buf_ptr()
    var cap = desktop.buf_capacity()
    var ext_buf = UnsafePointer[UInt8, MutExternalOrigin](
        unsafe_from_address=Int(buf)
    )
    var writer_ptr = UnsafePointer[MutationWriter, MutExternalOrigin].alloc(1)
    writer_ptr.init_pointee_move(MutationWriter(ext_buf, cap))

    var vnode_idx = ...  # render your component
    var mount_len = ctx.mount(writer_ptr, vnode_idx)
    if mount_len > 0:
        desktop.flush_mutations(Int(mount_len))

    # 5. Event loop
    while desktop.is_alive():
        var closed = desktop.step(blocking=False)
        if closed: break

        # Poll and dispatch events
        while True:
            var event = desktop.poll_event()
            if not event.is_valid(): break
            _ = ctx.dispatch_event(
                UInt32(event.handler_id), UInt8(event.event_type)
            )

        # Re-render if dirty
        if ctx.consume_dirty():
            memset_zero(buf, cap)
            writer_ptr.destroy_pointee()
            writer_ptr.init_pointee_move(MutationWriter(ext_buf, cap))
            # ... diff and flush mutations ...

        # Idle wait
        _ = desktop.step(blocking=True)

    # 6. Cleanup
    writer_ptr.destroy_pointee()
    writer_ptr.free()
    ctx.destroy()
    desktop.destroy()
```

## IPC Protocol

### Mutations (Mojo → Webview)

The binary mutation buffer (same opcodes as web) is sent via:

1. Mojo writes mutations to a heap buffer via `MutationWriter`
2. The C shim base64-encodes the buffer
3. Calls `webview_eval("window.__mojo_apply_mutations('base64...')")` 
4. The JS `desktop-runtime.js` decodes the base64 to an `ArrayBuffer`
5. The `Interpreter` class processes the mutations and applies them to the DOM

### Events (Webview → Mojo)

DOM events are sent as minimal JSON:

```json
{"h": 42, "t": 0}
{"h": 42, "t": 1, "v": "hello"}
```

Where:
- `h` = handler ID (assigned by `NewEventListener` mutation)
- `t` = event type tag (`0`=click, `1`=input, `2`=keydown, etc.)
- `v` = optional string value (for input/change/key events)

The flow:
1. JS event listener fires in the webview
2. `desktop-runtime.js` serializes to JSON
3. Calls `window.mojo_post(json)` (injected by the C shim)
4. WebKitGTK's `UserContentManager` receives the message
5. C shim stores it in a ring buffer (capacity: 256 events)
6. Mojo calls `poll_event()` to drain events one at a time

## C Shim API

The `libmojo_webview.so` C shim wraps GTK4 + WebKitGTK with a flat C API designed for Mojo's `DLHandle` FFI:

| Function | Purpose |
|----------|---------|
| `mwv_create(title, w, h, debug)` | Create window + webview |
| `mwv_destroy(w)` | Destroy and free all resources |
| `mwv_set_title(w, title)` | Set window title |
| `mwv_set_size(w, w, h, hints)` | Set window size |
| `mwv_set_html(w, html)` | Load HTML content |
| `mwv_init(w, js)` | Inject JS for every page load |
| `mwv_eval(w, js)` | Evaluate JS in the webview |
| `mwv_run(w)` | Blocking event loop |
| `mwv_step(w, blocking)` | Single event loop iteration |
| `mwv_poll_event(w, buf, len)` | Pop next event from ring buffer |
| `mwv_apply_mutations(w, buf, len)` | Base64-encode + eval mutations |
| `mwv_is_alive(w)` | Check if window is still open |

No function-pointer callbacks — all communication is poll-based.

## Environment Variables

| Variable | Purpose | Default |
|----------|---------|---------|
| `MOJO_WEBVIEW_LIB` | Full path to `libmojo_webview.so` | Searches `LD_LIBRARY_PATH` |
| `MOJO_GUI_DESKTOP_RUNTIME` | Full path to `desktop-runtime.js` | Searches relative paths |
| `LD_LIBRARY_PATH` | Library search path for GTK/WebKit | System default |

## Just Commands

| Command | Description |
|---------|-------------|
| `just build-shim` | Compile `libmojo_webview.so` |
| `just build-counter` | Compile the counter example |
| `just run-counter` | Build + run the counter example |
| `just dev-counter` | Run counter via `mojo run` (faster iteration) |
| `just test-shim` | Verify exported C symbols |
| `just test-runtime` | Syntax-check `desktop-runtime.js` |
| `just test` | Run all non-interactive tests |
| `just clean` | Remove build artifacts |
| `just env` | Print required environment variables |

## Limitations & Future Work

- **Linux only** (GTK4 + WebKitGTK). macOS (WKWebView) and Windows (WebView2) support would require platform-specific C shim implementations or switching to the cross-platform [webview/webview](https://github.com/webview/webview) library.
- **Base64 IPC overhead**: Every mutation buffer is base64-encoded, adding ~33% size overhead. Future optimization: use shared memory or binary ArrayBuffer transfer via custom URI scheme.
- **No window menu, tray icon, or system integration** yet. These require additional GTK4 FFI bindings.
- **Single window only**. Multi-window support would need changes to the C shim and Mojo-side window management.
- **Event ring buffer** has a fixed capacity of 256 events. Events are dropped (oldest first) if the buffer overflows — this shouldn't happen in practice since the event loop drains events every frame.