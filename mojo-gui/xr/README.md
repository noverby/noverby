# mojo-gui/xr — XR Renderer (Phase 5)

Render mojo-gui panels into 3D XR (extended reality) environments via OpenXR. Each XR panel owns an independent Blitz DOM document rendered to an offscreen GPU texture by Vello. The OpenXR compositor places these textures as quad layers in the XR scene.

> **Status:** Steps 5.1–5.3 — Real Blitz documents wired up. Each panel owns a `BaseDocument` with Stylo CSS styling and Taffy layout. 30 integration tests pass (headless). Mojo FFI bindings (`XRBlitz`, ~70 methods) and per-panel mutation interpreter (`XRMutationInterpreter`, all 18 opcodes) complete. OpenXR runtime integration and Vello offscreen GPU rendering are not yet wired up.

## Architecture

```text
┌─────────────────────────────────────────────────────────────────┐
│  Native Process                                                  │
│                                                                  │
│  ┌─────────────────────┐                                         │
│  │  mojo-gui/core       │                                         │
│  │  (compiled native)   │── mutation buffer ──┐                   │
│  │  signals, vdom,      │    (per-panel)      │                   │
│  │  diff, scheduler     │◄── event dispatch ──┤                   │
│  └─────────────────────┘                     │                   │
│                                              ▼                   │
│  ┌─────────────────────────────────────────────────────────┐     │
│  │  XR Panel Manager (xr/native/src/xr/)                    │     │
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

### How It Works

1. **Each panel is a Blitz DOM document.** The same `blitz-dom` + Stylo + Taffy + Vello stack from the desktop renderer is reused. The only difference is the render target: offscreen GPU texture instead of a Winit window surface.

2. **The mutation protocol is unchanged.** Each panel receives the same binary opcode stream (`LOAD_TEMPLATE`, `SET_ATTRIBUTE`, `SET_TEXT`, `APPEND_CHILDREN`, etc.) as the web and desktop renderers. The core framework doesn't know it's running in XR.

3. **Input is bridged, not reinvented.** XR controller rays are intersected with panel quads in 3D space. The resulting 2D hit coordinates are translated to standard DOM pointer/click events and dispatched through the existing `HandlerRegistry`. App code doesn't know the click came from a VR controller.

4. **Panels are the spatial primitive.** A panel is a 2D DOM document placed at a 3D position/rotation in the XR scene. Apps create panels via the `XRScene` manager; each panel can host a separate `GuiApp` instance.

5. **Single-panel apps work unchanged.** Existing apps that call `launch[CounterApp](config)` get XR support for free — the XR launcher wraps them in a default panel automatically. No app code changes required.

## Packages

| Component | Path | Description |
|-----------|------|-------------|
| **Panel types** | `native/src/xr/panel.mojo` | `XRPanel`, `PanelConfig`, `PanelState`, `Vec3`, `Quaternion`, preset configs |
| **Scene manager** | `native/src/xr/scene.mojo` | `XRScene`, `XREvent`, `RaycastHit`, spatial layout helpers (`arrange_arc`, `arrange_grid`, `arrange_stack`) |
| **XR Blitz FFI** | `native/src/xr/xr_blitz.mojo` | `XRBlitz` struct — ~70 typed methods wrapping all `mxr_*` C functions via DLHandle, plus `XREvent`, `XRPose`, `XRRaycastHit` types and constants |
| **XR interpreter** | `native/src/xr/renderer.mojo` | `XRMutationInterpreter` — per-panel binary opcode interpreter (all 18 opcodes), `BufReader` for little-endian buffer decoding |
| **XR Blitz shim** | `native/shim/src/lib.rs` | Rust cdylib — multi-panel Blitz BaseDocument, headless mode, raycasting, event ring buffer, DOM serialization, Stylo+Taffy layout |
| **C API header** | `native/shim/mojo_xr.h` | Flat C ABI — session lifecycle, panel management, mutations, events, frame loop, input, raycasting, debug |
| **Nix derivation** | `native/shim/default.nix` | Rust build with Blitz + OpenXR + GPU deps |
| **WebXR runtime** | `web/runtime/` | 🔮 Future — WebXR session lifecycle, DOM-to-texture rendering, XR input bridging |

## Project Structure

```text
xr/
├── native/                       # OpenXR native renderer
│   ├── shim/
│   │   ├── src/lib.rs            # Rust cdylib: multi-panel Blitz BaseDocument + raycasting + layout
│   │   ├── mojo_xr.h            # C API header (~80 functions)
│   │   ├── Cargo.toml           # Blitz + OpenXR + wgpu + Vello deps
│   │   └── default.nix          # Nix derivation with OpenXR/GPU deps
│   └── src/xr/
│       ├── __init__.mojo        # Package root with re-exports
│       ├── panel.mojo           # XRPanel, PanelConfig, Vec3, Quaternion, presets
│       ├── scene.mojo           # XRScene, XREvent, RaycastHit, layout helpers
│       ├── xr_blitz.mojo        # XRBlitz FFI bindings to libmojo_xr.so (~70 methods)
│       ├── renderer.mojo        # XRMutationInterpreter (per-panel opcode interpreter)
│       └── xr_launcher.mojo     # 🔮 xr_launch[AppType: GuiApp]() entry point (Step 5.5)
├── web/                          # WebXR browser renderer (future)
│   ├── runtime/                  # 🔮 TypeScript: XR session, DOM→texture, input bridging
│   └── src/                      # 🔮 Mojo: WebXR launch configuration
└── README.md                     # This file
```

## Key Types

### XRPanel

A 2D DOM document placed in 3D XR space. Each panel has:

- **Identity** — unique `panel_id` assigned by the shim
- **3D Transform** — `position` (Vec3, meters), `rotation` (Quaternion)
- **Physical size** — `width_m`, `height_m` in meters
- **Pixel density** — `pixels_per_meter` (determines texture resolution and text legibility)
- **Display options** — `curved` (cylindrical surface), `interact` (accepts pointer input)
- **Runtime state** — `visible`, `focused`, `dirty`, `mounted`

### PanelConfig

Configuration for creating a panel. Provides presets for common use cases:

| Preset | Size | PPM | Use Case |
|--------|------|-----|----------|
| `default_panel_config()` | 0.8m × 0.6m | 1200 | General-purpose reading panel (similar to a 27" monitor) |
| `dashboard_panel_config()` | 1.6m × 0.9m | 1000 | Wide curved dashboard |
| `tooltip_panel_config()` | 0.3m × 0.15m | 800 | Small non-interactive HUD overlay |
| `hand_anchored_panel_config()` | 0.2m × 0.15m | 1400 | Panel attached to a controller/hand |

### XRScene

Top-level manager for all XR panels in a session:

- **Panel lifecycle** — `create_panel()`, `destroy_panel()`
- **Focus management** — exclusive keyboard/text focus, click-to-focus
- **Dirty tracking** — only re-render panels whose DOM has changed
- **Raycasting** — ray-plane intersection against all visible panels
- **Spatial layout helpers** — `arrange_arc()`, `arrange_grid()`, `arrange_stack()`

## Build & Test

### Prerequisites

All dependencies are provided by the Nix dev shell. If not using Nix, you need:

- [Rust toolchain](https://rustup.rs/) (for the XR shim)
- GPU/rendering libraries: Vulkan, libxkbcommon, fontconfig (for Vello)
- [OpenXR loader](https://github.com/KhronosGroup/OpenXR-SDK) (for XR session management)
- An OpenXR runtime: [Monado](https://monado.freedesktop.org/) (open source), SteamVR, or Meta Quest Link

### Build the XR shim

```sh
# Build the Rust cdylib (headless mode works without OpenXR runtime)
cd xr/native/shim && cargo build --release --lib

# Run integration tests (headless — no XR runtime or GPU needed)
cd xr/native/shim && cargo test
```

### Run a shared example in XR (future — Step 5.5)

```sh
# When xr_launch is implemented:
# mojo build examples/counter/main.mojo --feature xr -I core/src -I xr/native/src -I examples
# ./build/counter-xr
```

## Compile Targets (after Phase 5)

```text
mojo build --target wasm64-wasi                  → web renderer
mojo build --target wasm64-wasi --feature webxr  → WebXR browser
mojo build                                       → desktop renderer (Blitz)
mojo build --feature xr                          → OpenXR native renderer
```

## Design Decisions

1. **Separate shim from desktop.** The XR shim (`xr/native/shim/`) is independent from the desktop shim (`desktop/shim/`). They share the same Blitz crate dependencies but have different responsibilities: the desktop shim manages a single Winit window; the XR shim manages an OpenXR session with multiple offscreen panels. Shared logic can be extracted into a common Rust crate if duplication becomes significant.

2. **Mutation protocol unchanged.** Each XR panel receives the same binary opcode stream as the web and desktop renderers. No protocol changes are needed for XR — the scene manager multiplexes mutation buffers to the correct panel.

3. **Headless mode for testing.** The shim provides `mxr_create_headless()` which creates a fully functional session without OpenXR or GPU. All DOM operations, raycasting, and event dispatch work in headless mode, enabling CI integration tests.

4. **Single-panel apps use GuiApp directly.** The XR launcher wraps any `GuiApp` in a default panel automatically. Multi-panel apps will use an `XRGuiApp` trait (Step 5.9, stretch goal) that receives the scene manager.

5. **wgpu unifies GPU access.** Both the native path (Vello → wgpu → Vulkan/Metal) and the future WebXR path (Vello → wgpu → WebGPU) use the same GPU abstraction, minimizing platform-specific code.

## Roadmap

| Step | Description | Status |
|------|-------------|--------|
| 5.1 | Design the XR panel abstraction | ✅ Complete — `XRPanel`, `PanelConfig`, `XRScene`, `XREvent`, `RaycastHit`, layout helpers, C API header, Rust shim scaffold with headless DOM and 24 integration tests |
| 5.2 | Build the OpenXR + Blitz Rust shim | 🔧 In progress — **real Blitz documents ✅** (HeadlessNode replaced with BaseDocument, 30 tests pass, Stylo+Taffy layout resolves). Remaining: Vello offscreen rendering, OpenXR session lifecycle, `_into()` FFI variants for struct-return functions |
| 5.3 | Mojo FFI bindings (`xr_blitz.mojo`) | ✅ Complete — `XRBlitz` struct (~70 methods), `XRMutationInterpreter` (all 18 opcodes), `XREvent`/`XRPose`/`XRRaycastHit` types, all constants |
| 5.4 | XR scene manager and panel routing | 🔲 Pending |
| 5.5 | `xr_launch[AppType: GuiApp]()` | 🔲 Pending |
| 5.6 | WebXR JS runtime | 🔲 Future |
| 5.7 | Wire `launch()` for XR targets | 🔲 Pending |
| 5.8 | Verify shared examples in XR | 🔲 Pending |
| 5.9 | Multi-panel XR API (stretch goal) | 🔮 Future |

## XR-Specific Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| OpenXR runtime unavailable | XR features fail on systems without an OpenXR runtime | Runtime detection at startup; fall back to desktop Blitz renderer |
| XR input latency | Raycasting → DOM event translation adds latency | Keep raycast math in Rust (shim-side); minimize FFI roundtrips |
| Frame timing constraints | OpenXR requires strict frame pacing; DOM re-render may exceed budget | Only re-render dirty panels; cache textures for clean panels; use quad layers for compositor-side reprojection |
| DOM-to-texture fidelity (WebXR) | Rendering DOM to WebGL texture may lose interactivity | Evaluate OffscreenCanvas, html2canvas, and custom 2D renderers |

## See Also

- [Phase 5 detailed plan](../docs/plan/phase5-xr.md) — full step-by-step design with architecture diagrams
- [Architecture](../docs/plan/architecture.md) — platform abstraction layer and dependency graph
- [Renderers](../docs/plan/renderers.md) — comparison of all renderer strategies
- [Desktop renderer](../desktop/README.md) — the Blitz desktop renderer that XR extends
- [Project plan](../PLAN.md) — overall roadmap and status dashboard