# mojo-gui/desktop — Desktop Renderer

Native desktop GUI backend for **mojo-gui** using [Blitz](https://github.com/DioxusLabs/blitz) (Stylo + Taffy + Vello + Winit + AccessKit).

> **Status: 🔮 Future** — This package is planned but not yet implemented.

## Architecture

The desktop renderer will interpret the same binary mutation protocol as the web renderer, but instead of targeting a browser DOM, it drives a native rendering pipeline:

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
│         │ mutations (binary)         │ events                      │
│         ▼                            │                             │
│  DesktopBridge                                                     │
│    ├── Owns heap-allocated mutation buffer                         │
│    ├── flush_mutations() → Blitz DOM interpreter                   │
│    └── poll_event() ← Winit event loop                            │
│         │                            ▲                             │
│         ▼                            │                             │
│  ┌─ Blitz Rendering Pipeline ─────────────────────────────────┐   │
│  │                                                             │   │
│  │  Stylo    — CSS parsing + cascade + selector matching       │   │
│  │  Taffy    — Flexbox / Grid / Block layout engine            │   │
│  │  Vello    — GPU-accelerated 2D rendering (via wgpu)         │   │
│  │  Winit    — Cross-platform window management                │   │
│  │  AccessKit — Native accessibility tree                      │   │
│  │                                                             │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                    │
└────────────────────────────────────────────────────────────────────┘
```

### How it will work

1. **Mojo compiles to a native binary** (not WASM). The core framework runs at full native speed.

2. **Mutations flow Mojo → Blitz**: The `MutationWriter` writes the same binary opcode stream as the web renderer, but into a heap buffer instead of WASM linear memory. A native mutation interpreter maps opcodes directly to Blitz's DOM tree via FFI.

3. **Events flow Winit → Mojo**: Window and input events are captured by Winit's event loop. The desktop bridge translates these into the core framework's event format and dispatches them to the `HandlerRegistry`.

4. **Rendering**: Stylo resolves CSS styles, Taffy computes layout, and Vello paints to a GPU surface. No browser engine, no JS interpreter, no IPC overhead.

### Comparison with the web renderer

| Aspect | Web (`mojo-gui/web`) | Desktop (`mojo-gui/desktop`) |
|--------|---------------------|------------------------------|
| Mojo target | `wasm64-wasi` | Native (default) |
| Mutation buffer | WASM linear memory | Heap buffer |
| CSS engine | Browser | Stylo (Firefox CSS engine) |
| Layout engine | Browser | Taffy (Flexbox/Grid) |
| Rendering | Browser compositor | Vello (GPU via wgpu) |
| Windowing | Browser tab | Winit (cross-platform) |
| Accessibility | Browser ARIA | AccessKit |
| Event delivery | WASM export calls | Winit event loop |
| Entry point | `@export` wrappers | `fn main()` |
| Performance | WASM overhead | Full native speed |

The key insight: **the user's app code is identical**. Only the entry point and renderer differ — exactly like Dioxus for Rust.

## Planned Directory Structure

```text
desktop/
├── src/
│   ├── __init__.mojo         # Package root
│   ├── blitz.mojo             # Mojo FFI bindings to Blitz
│   ├── bridge.mojo            # Mutation interpreter + event bridge
│   └── app.mojo               # DesktopApp entry point and event loop
├── examples/
│   └── counter.mojo           # Desktop counter demo
└── README.md                  # This file
```

## Design Goals

- **No embedded browser engine** — Blitz provides DOM/CSS semantics without a full browser runtime.
- **Cross-platform** — Winit supports Linux (Wayland/X11), macOS, and Windows out of the box.
- **Native accessibility** — AccessKit bridges to platform a11y APIs (AT-SPI on Linux, NSAccessibility on macOS, UIA on Windows).
- **GPU rendering** — Vello renders via wgpu, supporting Vulkan, Metal, and DX12.
- **Zero IPC overhead** — Mutations are interpreted in-process; no base64 encoding, no JS eval, no serialization.

## Related

- [Blitz](https://github.com/DioxusLabs/blitz) — Native HTML/CSS rendering engine
- [Stylo](https://wiki.mozilla.org/Quantum/Stylo) — Firefox's CSS engine
- [Taffy](https://github.com/DioxusLabs/taffy) — Flexbox/Grid layout engine
- [Vello](https://github.com/linebender/vello) — GPU 2D renderer
- [Winit](https://github.com/rust-windowing/winit) — Cross-platform windowing
- [AccessKit](https://github.com/AccessKit/accesskit) — Native accessibility