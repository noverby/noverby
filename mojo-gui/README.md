# mojo-gui — Multi-Renderer Reactive GUI Framework for Mojo

Write a Mojo GUI app once, run it in the browser via WASM **and** natively on desktop — inspired by [Dioxus](https://dioxuslabs.com/) for Rust.

## Architecture

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
              │ /core    │  │ (future) │
              │          │  │          │
              │ signals/ │  │ DOM      │
              │ scope/   │  │ fetch    │
              │ vdom/    │  │ WebSocket│
              │ mutations│  │ storage  │
              │ bridge/  │  │ timers   │
              │ events/  │  │ canvas   │
              │ component│  │ ...      │
              │ html/    │  └──────────┘
              └────┬─────┘
                   │ consumed by
              ┌────┼────────────┐
              ▼    ▼            ▼
     ┌──────────┐ ┌──────────┐ ┌──────────┐
     │ mojo-gui │ │ mojo-gui │ │ mojo-gui │
     │ /web     │ │ /desktop │ │ /native  │
     │          │ │ (future) │ │ (future) │
     │ main.mojo│ │ webview  │ │          │
     │ runtime/ │ │ + reused │ │ widget   │
     │ examples/│ │ JS interp│ │ mapping  │
     └──────────┘ └──────────┘ └──────────┘
```

## Packages

| Package | Status | Description |
|---------|--------|-------------|
| [`core/`](core/) | ✅ Active | Renderer-agnostic reactive GUI framework — signals, virtual DOM, diff engine, binary mutation protocol, component framework, HTML vocabulary |
| [`web/`](web/) | ✅ Active | Browser renderer — compiles Mojo to WASM, TypeScript runtime interprets mutations into real DOM |
| `desktop/` | 🔮 Future | Desktop renderer — embeds a webview, reuses the JS interpreter, communicates via IPC |
| `native/` | 🔮 Future | Native renderer — maps DOM-like mutations directly to platform widgets (GTK, Cocoa, Win32) |

## How It Works

The **binary mutation protocol** is the renderer contract. The core framework never touches real DOM, widgets, or any platform API. Instead, it writes a stream of binary opcodes (`OP_LOAD_TEMPLATE`, `OP_SET_ATTRIBUTE`, `OP_SET_TEXT`, `OP_APPEND_CHILDREN`, etc.) to a byte buffer. Each renderer implements an interpreter that consumes this stream:

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

| Opcode | Web (DOM) | Desktop (Webview) | Native (future) |
|--------|-----------|-------------------|-----------------|
| `LOAD_TEMPLATE` | `cloneNode(true)` | Same (webview has DOM) | Create widget tree |
| `SET_ATTRIBUTE` | `el.setAttribute()` | Same | Set widget property |
| `SET_TEXT` | `node.textContent = ...` | Same | Set label text |
| `NEW_EVENT_LISTENER` | `addEventListener()` | Same | Register widget callback |
| `APPEND_CHILDREN` | `parent.appendChild()` | Same | Add child widget |
| `REMOVE` | `node.remove()` | Same | Destroy widget |

## Quick Start

### Prerequisites

All dependencies are provided by the Nix dev shell. If not using Nix, you need:

- [Mojo](https://docs.modular.com/mojo/) 0.26.1+
- [Deno](https://deno.land/)
- [LLVM](https://llvm.org/) (for `llc`)
- [wabt](https://github.com/WebAssembly/wabt) (for `wasm-ld`)
- [wasmtime](https://wasmtime.dev/)
- [just](https://github.com/casey/just)

### Build & Run (Web)

```sh
cd web/

# Build WASM binary
just build

# Serve examples
just serve

# Open in browser:
#   http://localhost:4507/examples/counter/
#   http://localhost:4507/examples/todo/
#   http://localhost:4507/examples/bench/
#   http://localhost:4507/examples/app/
```

### Run Tests

```sh
cd web/

# Mojo-side tests (via wasmtime)
just test

# JS integration tests (via Deno)
just test-js

# All tests
just test-all

# Browser end-to-end tests (headless Servo)
just test-browser
```

## Project Structure

```text
mojo-gui/
├── core/                         # Renderer-agnostic GUI framework
│   ├── src/
│   │   ├── signals/              # Reactive primitives (signals, memos, effects)
│   │   ├── scope/                # Scope lifecycle, arena allocator
│   │   ├── scheduler/            # Height-ordered dirty scope queue
│   │   ├── arena/                # ElementId type and allocator
│   │   ├── vdom/                 # Virtual DOM (Template, VNode, diff)
│   │   ├── mutations/            # CreateEngine, DiffEngine
│   │   ├── bridge/               # MutationWriter + binary opcode protocol
│   │   ├── events/               # HandlerRegistry, action tags
│   │   ├── component/            # AppShell, ComponentContext, KeyedList, Router
│   │   ├── html/                 # HTML tags, DSL constructors, VNodeBuilder
│   │   └── lib.mojo              # Package root
│   ├── apps/                     # Demo/test apps
│   ├── test/                     # Mojo-side unit tests
│   └── README.md
│
├── web/                          # Browser renderer (WASM + TypeScript)
│   ├── runtime/                  # TypeScript runtime (DOM interpreter, events)
│   ├── src/main.mojo             # @export WASM wrappers
│   ├── examples/                 # Browser example apps
│   ├── test-js/                  # JS integration tests
│   ├── scripts/                  # Build pipeline
│   ├── justfile                  # Build commands
│   └── README.md
│
├── desktop/                      # Desktop renderer (future — webview)
├── native/                       # Native renderer (future — platform widgets)
└── README.md                     # This file
```

## Import Conventions

After the separation from the original `wasm-mojo` monolith, imports are split between `vdom` (renderer-agnostic structures) and `html` (HTML-specific vocabulary):

```text
# Renderer-agnostic virtual DOM types
from vdom import VNode, VNodeStore, Template, TemplateBuilder

# HTML vocabulary — element constructors, VNodeBuilder
from html import el_div, el_button, text, dyn_text, VNodeBuilder, to_template

# Other core packages
from signals import Runtime, SignalI32, SignalBool, SignalString
from component import ComponentContext, AppShell, KeyedList
from bridge import MutationWriter
from events import HandlerRegistry
```

**Rule of thumb**: If it's an HTML element constructor (`el_*`), tag constant (`TAG_*`), `VNodeBuilder`, `to_template`, `Node`, or `NODE_*` constant → import from `html`. If it's `VNode`, `VNodeStore`, `Template`, `TemplateNode`, `DynamicNode`, `AttributeValue` → import from `vdom`.

## Origin

This project was extracted from the [`wasm-mojo`](../wasm-mojo/) monolith following the [Separation Plan](../wasm-mojo/SEPARATION_PLAN.md). The separation enables:

1. **Multi-renderer support** — The same app code runs on web, desktop, and (future) native
2. **Clean dependency boundaries** — Core framework has zero browser/WASM dependencies
3. **Independent development** — Renderers can evolve independently
4. **Ecosystem foundation** — Other projects can build on `mojo-gui/core` for custom renderers

## License

See the repository root [LICENSE](../LICENSE).