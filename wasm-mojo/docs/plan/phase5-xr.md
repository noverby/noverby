# Phase 5: XR Renderer (Future)

> **Status:** Future — not yet started. Depends on Phase 4 (Blitz desktop renderer) being fully verified.

Render DOM-based UI panels into XR environments. The mutation protocol is unchanged — each XR panel receives the same binary opcode stream. The Blitz stack (blitz-dom + Stylo + Taffy + Vello) is reused for native OpenXR; the existing web renderer's JS interpreter is extended for WebXR.

**Compile targets (complete picture):**

- `mojo build --target wasm64-wasi` → web renderer (needs `mojo-gui/web` JS runtime)
- `mojo build --target wasm64-wasi --feature webxr` → WebXR renderer (extends web renderer with XR session)
- `mojo build` → desktop renderer (Blitz native, implementation complete — verification pending)
- `mojo build --feature xr` → OpenXR native renderer (Blitz panels → OpenXR swapchain)

---

## Step 5.1 — Design the XR panel abstraction

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

---

## Step 5.2 — Build the OpenXR + Blitz Rust shim

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

---

## Step 5.3 — Implement Mojo FFI bindings for OpenXR shim

Create `xr/native/src/xr_blitz.mojo` — typed `XRBlitz` struct via `DLHandle`, wrapping all shim functions. Follows the same pattern as `desktop/src/desktop/blitz.mojo`.

---

## Step 5.4 — Implement XR scene manager and panel routing

Create `xr/native/src/scene.mojo` — manages the collection of XR panels:

- Routes mutation buffers to the correct panel's Blitz document
- Polls events from the shim and dispatches to the correct panel's `GuiApp`
- Provides spatial layout helpers (arrange panels in an arc, pin to hand, anchor to world)

---

## Step 5.5 — Implement `xr_launch[AppType: GuiApp]()`

Create `xr/native/src/xr_launcher.mojo`:

- Creates an OpenXR session via the shim
- Creates a default panel for the app
- Runs the XR frame loop: wait frame → poll input → raycast panels → dispatch events → re-render dirty panels → composite → end frame
- Integrates with `launch()` via a new `@parameter if` branch (or `--feature xr` flag)

---

## Step 5.6 — Implement WebXR JS runtime

Create `xr/web/runtime/`:

- `xr-session.ts` — WebXR session lifecycle (`navigator.xr.requestSession('immersive-vr')`)
- `xr-panels.ts` — render DOM subtrees to WebGL/WebGPU textures, manage panel 3D transforms
- `xr-input.ts` — XR input source → raycast against panel quads → translate to DOM pointer events → dispatch via existing `EventBridge`
- `xr-runtime.ts` — entry point that extends the existing web runtime; replaces `requestAnimationFrame` with `XRSession.requestAnimationFrame`

---

## Step 5.7 — Wire `launch()` for XR targets

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

---

## Step 5.8 — Verify shared examples in XR

All existing shared examples (counter, todo, bench, app) should work as single-panel XR apps without modification:

- `launch[CounterApp](AppConfig(title="Counter", ...))` opens one XR panel
- The counter renders into the panel; clicks from XR controllers are translated to DOM events
- No app code changes required

---

## Step 5.9 — Multi-panel XR API (stretch goal)

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

## XR Architecture Diagrams

### OpenXR Native (`mojo-gui/xr/native/`)

Reuses the Blitz stack from the desktop renderer:

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

### WebXR Browser (`mojo-gui/xr/web/`)

Extends the existing web renderer:

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

---

## Key Design Decisions

- **The mutation protocol is unchanged.** Each XR panel receives the same binary opcode stream as any other renderer. The core framework doesn't know it's running in XR.
- **Blitz stack is reused, not forked.** The OpenXR native renderer uses the same `blitz-dom` + Stylo + Taffy + Vello pipeline as the desktop renderer. The only difference is the final render target (offscreen texture vs. Winit surface) and the compositor (OpenXR quad layers vs. window manager).
- **Panels are the spatial primitive.** A panel is a 2D DOM document placed at a 3D position/rotation in the XR scene. Apps create panels via a new `XRPanel` API; each panel can host a separate `GuiApp` or a view within one.
- **Input is bridged, not reinvented.** XR controller rays are intersected with panel quads in 3D; the resulting 2D hit coordinates are translated to standard DOM pointer/click events and dispatched through the existing `HandlerRegistry`. App code doesn't know the click came from a VR controller.
- **wgpu is the unifying GPU layer.** It targets Vulkan/Metal/DX12 natively (for OpenXR) and WebGPU in the browser (for WebXR), providing a single rendering abstraction across both XR backends.

---

## Target Project Structure

```text
xr/                               # XR renderer (Phase 5)
├── native/                       # OpenXR native renderer (Blitz DOM → Vello → OpenXR swapchain)
│   ├── shim/
│   │   ├── src/lib.rs            # Rust cdylib: Blitz + Vello + openxr crate, panel management
│   │   ├── mojo_xr.h            # C API header — XR session, panels, input
│   │   ├── Cargo.toml           # Rust crate config (blitz-dom, vello, openxr, winit)
│   │   └── default.nix          # Nix derivation with OpenXR/GPU deps
│   └── src/
│       ├── xr_launcher.mojo     # xr_launch[AppType: GuiApp]() — OpenXR event loop
│       ├── xr_blitz.mojo        # Mojo FFI bindings to libmojo_xr.so
│       ├── panel.mojo           # XRPanel — DOM document + 3D transform + input mapping
│       └── scene.mojo           # XRScene — panel registry, spatial layout, raycasting
├── web/                          # WebXR browser renderer (extends mojo-gui/web)
│   ├── runtime/
│   │   ├── xr-session.ts        # WebXR session lifecycle, reference spaces
│   │   ├── xr-panels.ts         # DOM → texture rendering, 3D panel placement
│   │   ├── xr-input.ts          # XR controller ray → DOM event translation
│   │   └── xr-runtime.ts        # Entry point — extends web runtime with XR
│   └── src/
│       └── webxr_launcher.mojo  # WebXR-specific launch configuration
└── README.md
```

---

## Migration Checklist

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

## Estimated Effort

| Task | Effort |
|------|--------|
| Phase 5 total | 4–8 weeks |

---

## XR-Specific Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| OpenXR runtime availability | XR features fail on systems without OpenXR runtime | Runtime detection: check for OpenXR loader at startup; fall back to desktop Blitz renderer if unavailable |
| DOM-to-texture fidelity (WebXR) | Rendering DOM to WebGL texture may lose interactivity or fidelity | Evaluate multiple approaches: OffscreenCanvas, html2canvas, CSS 3D transforms in DOM overlay; benchmark quality vs. performance |
| XR input latency | Raycasting → DOM event translation adds latency to controller input | Keep raycast math in the shim (Rust/native) or GPU (WebXR); minimize JS/Mojo roundtrips for input dispatch |
| Multi-panel mutation routing | Multiple panels need independent mutation streams; current protocol assumes single document | Each panel gets its own mutation buffer and `GuiApp` instance; the XR scene manager multiplexes; no protocol changes needed |
| XR frame timing constraints | OpenXR requires strict frame pacing; DOM re-render may exceed frame budget | Render panels asynchronously; only re-render dirty panels; cache textures for clean panels; use OpenXR quad layers for compositor-side reprojection |

---

## Open Questions (XR-Specific)

1. **Should the XR native shim share code with the desktop Blitz shim?** — The XR shim reuses the same Blitz stack (blitz-dom, Stylo, Taffy, Vello) but needs multi-document support, offscreen texture rendering, and OpenXR integration. Options: (a) extend the existing `desktop/shim/` with XR feature flags, (b) create a separate `xr/native/shim/` that depends on the same Blitz crates. Option (b) is cleaner — the desktop shim targets a single Winit window; the XR shim targets an OpenXR session with multiple offscreen panels. Shared Blitz logic can be extracted into a common Rust crate if duplication becomes significant.

2. **How to handle DOM-to-texture rendering for WebXR?** — Several approaches exist: (a) OffscreenCanvas with `drawImage()` from a DOM-rendered element, (b) `html2canvas` or similar rasterization libraries, (c) WebXR DOM Overlay API (limited to a single flat layer, not spatially placed), (d) render mutation protocol directly to a WebGL/WebGPU canvas using a custom 2D renderer (bypassing the DOM entirely on the WebXR path). Evaluate fidelity, performance, and interactivity tradeoffs. Approach (d) would be the most consistent with the native path (Vello-like rendering to a texture) but requires a JS/WASM 2D rendering engine.

3. **Should single-panel XR apps use `GuiApp` directly or always go through `XRGuiApp`?** — For simplicity, single-panel apps should use the existing `GuiApp` trait unchanged. The XR launcher wraps them in a default panel automatically. `XRGuiApp` is only needed for apps that explicitly manage multiple panels or need XR-specific features (hand tracking, spatial anchors). This preserves the "write once, run everywhere" principle — existing apps get XR support for free.

4. **What OpenXR extensions are required?** — The MVP needs: `XR_KHR_opengl_enable` or `XR_KHR_vulkan_enable` (GPU interop), `XR_KHR_composition_layer_quad` (panel placement). Nice to have: `XR_EXT_hand_tracking` (hand input), `XR_FB_passthrough` (AR), `XR_EXTX_overlay` (overlay apps). The shim should detect available extensions at runtime and expose capability flags to Mojo.