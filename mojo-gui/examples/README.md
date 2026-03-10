# mojo-gui Examples — Shared Cross-Platform Apps

This directory contains example applications that run on **every** renderer target unchanged. The same Mojo app code compiles and runs in the browser (via WASM) and natively on desktop — only the build command differs.

## Design Principle

> **If an example doesn't work on a target, it's a framework bug — not an app bug.**

Example apps import only from `mojo-gui/core` (signals, components, HTML DSL). They do NOT import from `mojo-gui/web` or `mojo-gui/desktop`. The `launch()` entry point and compile-time target selection handle the rest.

## Examples

| Example | Description | Signals | Components | Features |
|---------|-------------|---------|------------|----------|
| **counter** | Counter with conditional detail section | `SignalI32`, `SignalBool` | `ConditionalSlot` | Reactive text, toggle show/hide, even/odd display |
| **todo** | Todo list with input binding | `SignalI32`, `SignalString` | `KeyedList` | Two-way input binding, Enter key handling, add/remove items |
| **bench** | js-framework-benchmark implementation | `SignalI32` | `KeyedList` | Create/update/swap/delete 1K–10K rows, performance timing |

## Directory Structure

Each example follows the same layout:

```text
examples/
├── counter/
│   ├── app.mojo              # Shared app logic (platform-agnostic) [future]
│   └── web/                   # Web-specific assets
│       ├── index.html         # HTML shell with #root mount point
│       └── main.js            # JS glue (loads WASM, connects runtime)
├── todo/
│   ├── app.mojo              # Shared app logic [future]
│   └── web/
│       ├── index.html
│       └── main.js
├── bench/
│   ├── app.mojo              # Shared app logic [future]
│   └── web/
│       ├── index.html
│       └── main.js
└── README.md                  # This file
```

### Current State

The shared `app.mojo` files are planned but not yet extracted. Currently:

- **Web app logic** lives in `mojo-gui/web/examples/<name>/<name>.mojo` and is imported by `web/src/main.mojo` via `@export` WASM wrappers.
- **Desktop app logic** lives in `mojo-gui/desktop/examples/counter.mojo` and uses `DesktopApp` directly.
- **Test/demo apps** live in `mojo-gui/core/apps/` and are used by the WASM test harness.

The `web/` subdirectories here contain copies of the web-specific assets (HTML shells, JS entry points) as a reference for the target structure. See the [Migration Checklist](#migration-status) below for progress.

## Building Examples

### Web Target (WASM + Browser)

Build from the `mojo-gui/web/` directory:

```sh
cd mojo-gui/web

# Build all examples (compiles Mojo → WASM, bundles JS runtime)
just build

# Or build and serve a specific example:
just serve-counter
just serve-todo
just serve-bench
```

The web build:
1. Compiles Mojo source to WASM via `mojo build --target wasm64-wasi`
2. The JS runtime (`runtime/`) instantiates the WASM module
3. The `Interpreter` reads binary mutations from WASM shared memory
4. The `EventBridge` captures DOM events and dispatches to WASM

### Desktop Target (Native + Webview)

Build from the `mojo-gui/desktop/` directory:

```sh
cd mojo-gui/desktop

# Ensure the C shim library is built (requires GTK4 + WebKitGTK):
nix build .#mojo-webview-shim  # or build manually, see desktop/shim/

# Run the counter example:
export MOJO_WEBVIEW_LIB=/path/to/libmojo_webview.so
export MOJO_GUI_DESKTOP_RUNTIME=runtime/desktop-runtime.js
mojo run -I ../core/src -I ../core -I src examples/counter.mojo
```

The desktop build:
1. Compiles Mojo source to a native binary (no WASM)
2. Creates a GTK4 window with an embedded WebKitGTK webview
3. Injects the JS mutation interpreter into the webview
4. Mutations are base64-encoded and sent via IPC to the webview
5. Events flow back from JS via a polling ring buffer

### Desktop Target — Blitz (Future)

The Blitz-based desktop renderer will eliminate the webview dependency:

```sh
cd mojo-gui/desktop

# Build with Blitz native HTML/CSS engine:
mojo build examples/counter/app.mojo \
    -I ../core/src \
    -I ../desktop/src \
    --link-against libmojo_blitz.so \
    -o dist/counter
```

This will use Stylo (Firefox's CSS engine) + Taffy (layout) + Vello (GPU rendering) instead of a webview. See `SEPARATION_PLAN.md` Phase 3 for details.

## How Web Assets Work

Each example's `web/` subdirectory contains only renderer infrastructure — no app logic:

- **`index.html`** — Minimal HTML shell with a `<div id="root">` mount point and app-specific styling.
- **`main.js`** — Entry point that uses the shared `launch()` function from `examples/lib/app.js`. Convention-based WASM export discovery means zero app-specific JS is needed:

```js
import { launch } from "../lib/app.js";

launch({
    app: "counter",
    wasm: new URL("../../build/out.wasm", import.meta.url),
});
```

The `launch()` function automatically discovers WASM exports by naming convention (`counter_init`, `counter_rebuild`, `counter_flush`, `counter_handle_event`) and wires up the Interpreter and EventBridge.

## Writing a New Example

1. **Create the app struct** in `mojo-gui/core/` or `mojo-gui/web/examples/<name>/`:

```mojo
struct MyApp(Movable):
    var ctx: ComponentContext
    var count: SignalI32

    fn __init__(out self):
        self.ctx = ComponentContext.create()
        self.count = self.ctx.use_signal(0)
        self.ctx.setup_view(
            el_div(
                el_h1(dyn_text()),
                el_button(text(String("+1")), onclick_add(self.count, 1)),
            ),
            String("my-app"),
        )
```

2. **Add `@export` wrappers** in `web/src/main.mojo` for the web target.

3. **Add web assets** in `examples/<name>/web/`:
   - `index.html` — copy from an existing example and customize the title/styles
   - `main.js` — use the shared `launch()` with your app name

4. **Add desktop entry point** (optional) in `desktop/examples/<name>.mojo`.

5. **Verify on all targets** — the app must work identically on every renderer.

## Migration Status

Progress toward fully shared, platform-agnostic examples:

### Phase 1: Core extraction ✅
- [x] Platform abstraction layer (`core/src/platform/`)
- [x] `PlatformApp` trait definition
- [x] `launch()` function with `AppConfig`
- [x] `PlatformFeatures` runtime capability detection
- [ ] Extract shared `app.mojo` files from web/desktop examples

### Phase 2: Web extraction
- [x] `WebApp` implementing `PlatformApp` trait
- [x] Web-specific assets in `examples/<name>/web/`
- [ ] Refactor web examples to use `launch()` pattern
- [ ] Generate `@export` boilerplate from manifest

### Phase 3: Desktop (Blitz)
- [ ] Blitz C shim and FFI bindings
- [ ] `DesktopApp` (Blitz) implementing `PlatformApp` trait
- [ ] Mojo-side mutation interpreter
- [ ] Verify all shared examples on desktop
- [ ] Cross-target CI test matrix

## Architecture Reference

```text
┌────────────────────────────────────────────────┐
│  Shared Example App                             │
│                                                 │
│  imports only:                                  │
│    mojo-gui/core (signals, components, DSL)     │
│    platform.launch() (entry point)              │
│                                                 │
│  NEVER imports:                                 │
│    mojo-gui/web (JS runtime, WASM exports)      │
│    mojo-gui/desktop (webview, Blitz FFI)        │
└───────────────┬─────────────────────────────────┘
                │ compile target selects renderer
        ┌───────┼───────────┐
        ▼       ▼           ▼
   ┌────────┐ ┌─────────┐ ┌────────┐
   │  web   │ │ desktop │ │ native │
   │ (WASM) │ │ (Blitz) │ │(future)│
   └────────┘ └─────────┘ └────────┘
```
