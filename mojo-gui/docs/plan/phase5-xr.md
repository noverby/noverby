# Phase 5: XR Renderer

> **Status:** 🔧 In progress — Steps 5.1–5.5, 5.7–5.8 complete. Remaining: Vello offscreen rendering + OpenXR session lifecycle (5.2 remaining), WebXR JS runtime (5.6), multi-panel API (5.9).

Render DOM-based UI panels into XR environments. The mutation protocol is unchanged — each XR panel receives the same binary opcode stream. The Blitz stack (blitz-dom + Stylo + Taffy + Vello) is reused for native OpenXR; the existing web renderer's JS interpreter is extended for WebXR.

**Compile targets (complete picture):**

- `mojo build --target wasm64-wasi -I core/src -I web/src` → web renderer
- `mojo build --target wasm64-wasi --feature webxr` → WebXR renderer (future — extends web renderer with XR session)
- `mojo build -I core/src -I desktop/src -I xr/native/src` → desktop renderer (Blitz native)
- `mojo build -D MOJO_TARGET_XR -I core/src -I xr/native/src -I desktop/src` → OpenXR native renderer

---

## Step 5.1 — Design the XR panel abstraction — ✅ Complete

Designed and implemented the XR panel abstraction, scene manager, and Rust shim scaffold. Created the `xr/` directory structure with native and web sub-projects.

**Mojo types** (`xr/native/src/xr/`):

- `panel.mojo` — `XRPanel` (2D DOM document + 3D transform), `PanelConfig`, `PanelState`, `Vec3`, `Quaternion`. Panel presets: `default_panel_config()` (0.8m × 0.6m, 1200 ppm), `dashboard_panel_config()` (1.6m × 0.9m curved), `tooltip_panel_config()` (0.3m × 0.15m non-interactive), `hand_anchored_panel_config()` (0.2m × 0.15m).
- `scene.mojo` — `XRScene` (panel registry, focus management, dirty tracking, raycasting via ray-plane intersection, spatial layout helpers). `XREvent` with panel targeting and UV hit coordinates. `RaycastHit`. Layout helpers: `arrange_arc()`, `arrange_grid()`, `arrange_stack()`. Convenience constructors: `create_single_panel_scene()`, `create_dual_panel_scene()`.

**Rust shim scaffold** (`xr/native/shim/`):

- `src/lib.rs` — `XrSessionContext` with headless mode (`mxr_create_headless`), multi-panel DOM (`Panel` with `HeadlessNode` tree), ID mapping, interpreter stack, event ring buffer, per-panel DOM operations, raycasting, DOM serialization, and 20+ integration tests.
- `mojo_xr.h` — C API header (~80 functions).
- `Cargo.toml` — Blitz v0.2.0 (same rev as desktop), anyrender, anyrender_vello, wgpu, openxr.
- `default.nix` — Nix derivation with Blitz + OpenXR + GPU dependencies.

**Core platform updates** (`core/src/platform/features.mojo`):

- Added `has_xr`, `has_xr_hand_tracking`, `has_xr_passthrough` fields to `PlatformFeatures`.
- Added `xr_native_features()` and `xr_web_features()` preset constructors.

---

## Step 5.2 — Build the OpenXR + Blitz Rust shim — 🔧 In progress

Replaced the lightweight `HeadlessNode` DOM tree with real Blitz `BaseDocument` instances. Each XR panel now owns a full Blitz document with Stylo styling and Taffy layout. Added output-pointer FFI variants for large struct returns.

**What's done:**

- **Real Blitz documents** — Panel owns a `BaseDocument` with `id_to_node`/`node_to_id` maps (same pattern as desktop shim). All DOM operations delegate to Blitz. Template cloning via `deep_clone_node`. Layout resolution via `doc.resolve(0.0)` in render loop.
- **Output-pointer FFI variants** — `mxr_poll_event_into()`, `mxr_raycast_panels_into()`, `mxr_get_pose_into()` for struct returns >16 bytes (x86_64 SysV ABI limitation).
- **37 integration tests** — All pass headless; covers session lifecycle, panel lifecycle, DOM operations, attributes, text nodes, placeholders, serialization, events, raycasting, focus, frame loop, reference spaces, ID mapping, stack operations, multi-panel isolation, Blitz document structure, nested elements, layout resolution, and all output-pointer variants.

**What's remaining:**

- Vello offscreen rendering to GPU textures (needs wgpu device setup)
- OpenXR session lifecycle (`openxr` crate integration — `xrCreateSession`, `xrWaitFrame`, `xrBeginFrame`, `xrEndFrame`)
- Quad layer compositing (panel textures → OpenXR swapchain)
- Controller pose tracking via OpenXR input actions

---

## Step 5.3 — Implement Mojo FFI bindings for OpenXR shim — ✅ Complete

Created typed Mojo FFI bindings for all ~80 XR shim C functions, plus a per-panel mutation interpreter.

- **`xr/native/src/xr/xr_blitz.mojo`** — `XRBlitz` struct wrapping all `mxr_*` C functions via `DLHandle`. ~70 typed methods covering: session lifecycle, panel lifecycle, panel transform & display, mutation batching, per-panel DOM operations, templates, tree traversal, events, raycasting, frame loop, input, reference spaces, capabilities, ID mapping & stack, debug/inspection. Helper types: `XREvent`, `XRPose`, `XRRaycastHit`. Constants for events, hands, spaces, states.
- **`xr/native/src/xr/renderer.mojo`** — `XRMutationInterpreter` struct. Per-panel opcode interpreter targeting `XRBlitz` FFI calls scoped to a `panel_id`. All 18 opcodes handled.
- **`poll_event()`, `raycast_panels()`, `get_pose()`** — All fully functional via `_into()` output-pointer variants (resolved in Step 5.2b).

---

## Step 5.4 — Implement XR scene manager and panel routing — ✅ Complete (single-panel)

`XRScene` provides panel registry, focus management, dirty tracking, Mojo-side raycasting (ray-plane intersection), and spatial layout helpers (`arrange_arc`, `arrange_grid`, `arrange_stack`).

For single-panel apps, `xr_launch` (Step 5.5) manages the panel directly via `XRBlitz` FFI — bypassing the scene for simplicity. Multi-panel routing through `XRScene` (scene creates/destroys panels via shim, multiplexes mutation buffers to correct panel's `GuiApp`) deferred to Step 5.9.

---

## Step 5.5 — Implement `xr_launch[AppType: GuiApp]()` — ✅ Complete

Implemented `xr_launch[AppType: GuiApp]()` — the XR-side counterpart to `desktop_launch`.

- **`xr/native/src/xr/launcher.mojo`** — Creates an XR session (headless or OpenXR), allocates a default panel sized from AppConfig, applies XR-specific UA stylesheet (larger fonts, dark background for headset legibility), mounts the app, and enters the XR frame loop: `wait_frame → begin_frame → poll_event → handle_event → flush → apply mutations → render_dirty_panels → end_frame`. Same mutation buffer management and GuiApp lifecycle as `desktop_launch`.

---

## Step 5.6 — Implement WebXR JS runtime — 🔧 In progress

Created `xr/web/runtime/` — the browser-side WebXR renderer that reuses the binary mutation protocol unchanged, rendering panel DOM content as textured quads in an immersive WebXR scene.

**Files created:**

| File | Description |
|------|-------------|
| `xr-types.ts` | TypeScript types mirroring native XR panel types: `Vec3`, `Quaternion`, `PanelConfig` (with presets: default, dashboard, tooltip, hand-anchored), `PanelState`, `XRPanelDescriptor`, `RaycastHit`, `XRInputRay`, `XRRuntimeConfig`, WebXR API compat interfaces |
| `xr-session.ts` | `XRSessionManager` — full session lifecycle: feature detection, session request (immersive-vr/ar/inline), WebGL2 context with `xrCompatible`, `XRWebGLLayer` binding, reference space negotiation (`local-floor` → `bounded-floor` → `local` → `viewer`), XR frame loop delegation, clean teardown |
| `xr-panel.ts` | `XRPanel` — offscreen DOM container, SVG foreignObject DOM→canvas rasterization (async), fallback text rasterizer, WebGL texture upload, ray-plane intersection raycasting, 4×4 model matrix from quaternion. `XRPanelManager` — panel lifecycle, focus management, throttled dirty texture updates, raycasting, spatial layout (`arrangeArc`, `arrangeGrid`, `arrangeStack`) |
| `xr-renderer.ts` | `XRQuadRenderer` — WebGL2 GLSL ES 3.0 shader (textured quad + alpha/opacity), VAO/VBO/EBO unit quad, per-view stereo rendering from `XRView` matrices, cursor dot visualization at UV hit, GL state save/restore |
| `xr-input.ts` | `XRInputHandler` — extracts rays from `XRInputSource.targetRaySpace`, raycasts against panels, per-source hover tracking (enter/leave/move with ~30Hz throttle), click synthesis from select events (selectstart→mousedown, selectend→mouseup+click), focus transitions, callback-based dispatch |
| `xr-runtime.ts` | `XRRuntime` — main entry point. WASM loading with full env imports. `createAppPanel()` for convention-based export discovery. Self-contained inline mutation interpreter (all 18 opcodes incl. `RegisterTemplate`). Handler map for XR input→WASM dispatch. "Enter VR" button. Flat-fallback mode. Per-frame: input → flush → rasterize → render → cursors |
| `mod.ts` | Module re-exports — single import path for the full public API |

**Key design decisions:**

1. **Mutation protocol unchanged** — each panel receives the same binary opcode stream; the inline interpreter covers all 18 opcodes
2. **DOM→texture via SVG foreignObject** — real CSS rendering fidelity; falls back to simple text renderer when SVG fails
3. **Callback-based input dispatch** — `XRInputHandler` emits synthetic DOM event names without touching the DOM; the runtime wires callbacks to WASM
4. **Flat fallback** — when WebXR is unavailable, panel containers become visible DOM elements with standard CSS
5. **Independent of `web/runtime/`** — self-contained inline mutation interpreter avoids import-time coupling; can swap in full `web/runtime/Interpreter` later

**Remaining:**

- End-to-end testing with a real WebXR device or browser emulator
- SVG foreignObject fidelity validation (external resources, CSS features)
- Integration with `web/runtime/Interpreter` for full DOM feature parity
- Browser E2E test suite for the WebXR path

---

## Step 5.7 — Wire `launch()` for XR targets — ✅ Complete

Updated `core/src/platform/launch.mojo` with compile-time dispatch:

```text
fn launch[AppType: GuiApp](config: AppConfig) raises:
    @parameter
    if is_wasm_target():
        pass  # JS runtime drives the loop; @export wrappers call GuiApp methods
    elif is_xr_target():
        from xr.launcher import xr_launch
        xr_launch[AppType](config)
    else:
        from desktop.launcher import desktop_launch
        desktop_launch[AppType](config)
```

Added `is_xr_target()` compile-time detection (checks `-D MOJO_TARGET_XR` define).

**Note:** All native builds must include both `-I desktop/src` and `-I xr/native/src` because Mojo's `@parameter if` does not suppress import resolution in dead branches. The linker only pulls in the active branch's code.

---

## Step 5.8 — Verify shared examples in XR — ✅ Complete

All 4 shared examples (Counter, Todo, Benchmark, MultiView) build and run as XR floating panels in headless mode.

**Issues discovered and fixed:**

1. **Mojo `@parameter if` import resolution** — Dead branches still resolve imports. Fix: all native builds include all renderer `-I` paths.
2. **`performance_now()` link failure on native** — Used `external_call` unconditionally. Fix: `@parameter if is_wasm_target()` gate; native path uses `perf_counter_ns`.
3. **Headless frame loop never exits** — `predicted_time == 0` sentinel broken (headless returns real timestamps). Fix: idle frame counter — exit after 1 consecutive frame with no events and no dirty scopes.

**Verification:**

```text
$ just build-xr-all    # ✅ All 4 examples build
$ just run-xr counter  # ✅ Exit code 0
$ just run-xr todo     # ✅ Exit code 0
$ just run-xr bench    # ✅ Exit code 0
$ just run-xr app      # ✅ Exit code 0
$ just test-xr         # ✅ 37/37 shim tests pass
$ just test-desktop    # ✅ 75/75 desktop shim tests pass (no regressions)
```

---

## Step 5.9 — Multi-panel XR API (stretch goal) — 🔮 Future

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
- **Separate XR shim from desktop shim.** The XR shim (`xr/native/shim/`) is a separate Rust cdylib from the desktop shim (`desktop/shim/`). Both reuse the same Blitz crates, but the XR shim targets multi-document management + offscreen rendering + OpenXR, while the desktop shim targets a single Winit window. This was decided in Step 5.1 (Open Question #1).

---

## Actual Project Structure

```text
xr/
├── native/                       # OpenXR native renderer
│   ├── shim/
│   │   ├── src/lib.rs            # Rust cdylib: multi-panel Blitz BaseDocument + raycasting + layout
│   │   │                         #   + output-pointer FFI variants for large struct returns
│   │   │                         #   + 37 integration tests (headless)
│   │   ├── tests/                # Rust integration tests
│   │   ├── mojo_xr.h            # C API header (~83 functions incl. 3 _into() variants)
│   │   ├── Cargo.toml           # blitz v0.2.0, anyrender, anyrender_vello, wgpu, openxr
│   │   └── default.nix          # Nix derivation with Blitz + OpenXR + GPU deps
│   └── src/xr/
│       ├── __init__.mojo         # Re-exports for XRBlitz, XRMutationInterpreter, types, constants
│       ├── launcher.mojo         # xr_launch[AppType: GuiApp]() — XR frame loop
│       ├── xr_blitz.mojo         # XRBlitz struct (~70 FFI methods via DLHandle)
│       ├── renderer.mojo         # XRMutationInterpreter (per-panel, all 18 opcodes)
│       ├── panel.mojo            # XRPanel, PanelConfig, Vec3, Quaternion, PanelState
│       └── scene.mojo            # XRScene — panel registry, focus, raycasting, layout helpers
├── web/                          # WebXR browser renderer (Step 5.6 — 🔧 in progress)
│   ├── runtime/
│   │   ├── mod.ts                # Module re-exports — single import path
│   │   ├── xr-types.ts           # Vec3, Quaternion, PanelConfig, RaycastHit, XRRuntimeConfig, compat types
│   │   ├── xr-session.ts         # XRSessionManager — lifecycle, ref spaces, GL setup, frame loop
│   │   ├── xr-panel.ts           # XRPanel + XRPanelManager — DOM containers, SVG rasterization, textures
│   │   ├── xr-renderer.ts        # XRQuadRenderer — WebGL2 textured quad shader, stereo, cursor
│   │   ├── xr-input.ts           # XRInputHandler — raycasting, hover, select→click, focus
│   │   └── xr-runtime.ts         # XRRuntime — main entry, WASM loading, inline interpreter, flat fallback
│   └── src/                      # (future — Mojo WASM exports for WebXR feature flag)
└── README.md                     # XR architecture, key types, build instructions, design decisions
```

---

## Migration Checklist

- [x] Design XR panel abstraction — `XRPanel` struct, `PanelConfig`, `PanelState`, `Vec3`, `Quaternion`, panel presets (Step 5.1)
- [x] Design XR scene manager — `XRScene` with focus management, dirty tracking, raycasting, spatial layout helpers (Step 5.1)
- [x] Build Rust shim scaffold — `XrSessionContext`, headless mode, multi-panel DOM, event ring buffer, raycasting, DOM serialization (Step 5.1)
- [x] Write C header (`mojo_xr.h`) — ~83 functions including 3 `_into()` output-pointer variants (Steps 5.1, 5.2b)
- [x] Write Nix derivation (`xr/native/shim/default.nix`) — Rust build with Blitz + OpenXR + GPU deps (Step 5.1)
- [x] Replace HeadlessNode with real Blitz BaseDocument per panel — Stylo CSS + Taffy layout (Step 5.2)
- [x] Add output-pointer FFI variants — `mxr_poll_event_into`, `mxr_raycast_panels_into`, `mxr_get_pose_into` (Step 5.2b)
- [ ] Implement Vello offscreen rendering to GPU textures (Step 5.2 remaining)
- [ ] Implement OpenXR session lifecycle — `openxr` crate integration (Step 5.2 remaining)
- [ ] Implement quad layer compositing — panel textures → OpenXR swapchain (Step 5.2 remaining)
- [ ] Implement controller pose tracking via OpenXR input actions (Step 5.2 remaining)
- [x] Implement Mojo FFI bindings (`xr_blitz.mojo`) — `XRBlitz` struct with ~70 typed methods via DLHandle (Step 5.3)
- [x] Implement XR mutation interpreter (`renderer.mojo`) — per-panel binary opcode interpreter, all 18 opcodes (Step 5.3)
- [x] Implement XR scene manager for single-panel routing (Step 5.4)
- [x] Implement `xr_launch[AppType: GuiApp]()` — XR frame loop with headless/OpenXR support (Step 5.5)
- [x] Build WebXR JS runtime (`xr/web/runtime/`) — XR session, DOM-to-texture, XR input bridging (Step 5.6)
- [ ] End-to-end test WebXR runtime with real device or emulator (Step 5.6 remaining)
- [x] Wire `launch()` for XR targets — `@parameter if is_xr_target()` compile-time dispatch (Step 5.7)
- [x] Verify all 4 shared examples build and run as XR floating panels in headless mode (Step 5.8)
- [ ] Multi-panel XR API — `XRGuiApp` trait for apps managing multiple panels (Step 5.9, stretch goal)

---

## Estimated Effort

| Task | Effort | Status |
|------|--------|--------|
| Steps 5.1–5.5, 5.7–5.8 (panel design, shim, FFI, launcher, verification) | ~3 weeks | ✅ Complete |
| Step 5.2 remaining (Vello offscreen + OpenXR session lifecycle) | 1–2 weeks | 🔲 Next |
| Step 5.6 (WebXR JS runtime) | 1–2 weeks | 🔧 In progress (~80% — runtime built, needs E2E testing) |
| Step 5.9 (Multi-panel API) | 1 week | 🔮 Stretch goal |
| **Phase 5 total** | **4–8 weeks** | **~75% complete** |

---

## XR-Specific Risks

| Risk | Impact | Mitigation | Status |
|------|--------|------------|--------|
| OpenXR runtime availability | XR features fail on systems without OpenXR runtime | Runtime detection: check for OpenXR loader at startup; fall back to desktop Blitz renderer if unavailable | 🔧 Headless mode implemented for testing; runtime detection pending |
| DOM-to-texture fidelity (WebXR) | Rendering DOM to WebGL texture may lose interactivity or fidelity | SVG foreignObject rasterization implemented with fallback text renderer; evaluate OffscreenCanvas, html2canvas for higher fidelity | 🔧 In progress — initial approach in `xr/web/runtime/xr-panel.ts`; needs real-device validation |
| XR input latency | Raycasting → DOM event translation adds latency to controller input | Keep raycast math in the shim (Rust/native) or GPU (WebXR); minimize JS/Mojo roundtrips for input dispatch | ✅ Rust-side raycasting implemented |
| Multi-panel mutation routing | Multiple panels need independent mutation streams; current protocol assumes single document | Each panel gets its own mutation buffer and `GuiApp` instance; the XR scene manager multiplexes; no protocol changes needed | ✅ Architecture proven (single-panel); multi-panel routing deferred to Step 5.9 |
| XR frame timing constraints | OpenXR requires strict frame pacing; DOM re-render may exceed frame budget | Render panels asynchronously; only re-render dirty panels; cache textures for clean panels; use OpenXR quad layers for compositor-side reprojection | 🔧 Dirty tracking per-panel implemented |
| Mojo `@parameter if` import resolution | Dead branches trigger import resolution; adding a renderer backend to `launch()` requires updating ALL native build commands | Include all renderer `-I` paths in every native build; documented in justfile and Architecture Decisions | ✅ Mitigated — workaround in place since Step 5.8 |

---

## Open Questions (XR-Specific)

1. ~~**Should the XR native shim share code with the desktop Blitz shim?**~~ **Resolved (Step 5.1):** Separate `xr/native/shim/` created. Both reuse the same Blitz crates but serve different purposes — desktop targets a single Winit window; XR targets an OpenXR session with multiple offscreen panels. Code duplication is minimal and manageable.

2. **How to handle DOM-to-texture rendering for WebXR?** — Several approaches exist: (a) OffscreenCanvas with `drawImage()` from a DOM-rendered element, (b) `html2canvas` or similar rasterization libraries, (c) WebXR DOM Overlay API (limited to a single flat layer, not spatially placed), (d) render mutation protocol directly to a WebGL/WebGPU canvas using a custom 2D renderer (bypassing the DOM entirely on the WebXR path). Evaluate fidelity, performance, and interactivity tradeoffs. Approach (d) would be the most consistent with the native path (Vello-like rendering to a texture) but requires a JS/WASM 2D rendering engine. *Deferred to Step 5.6.*

3. ~~**Should single-panel XR apps use `GuiApp` directly or always go through `XRGuiApp`?**~~ **Resolved (Step 5.5):** Single-panel apps use the existing `GuiApp` trait unchanged. `xr_launch` wraps them in a default panel automatically. `XRGuiApp` (Step 5.9) is only needed for apps that explicitly manage multiple panels.

4. **What OpenXR extensions are required?** — The MVP needs: `XR_KHR_opengl_enable` or `XR_KHR_vulkan_enable` (GPU interop), `XR_KHR_composition_layer_quad` (panel placement). Nice to have: `XR_EXT_hand_tracking` (hand input), `XR_FB_passthrough` (AR), `XR_EXTX_overlay` (overlay apps). The shim detects available extensions at runtime and exposes capability flags to Mojo (`has_extension()`, `has_hand_tracking()`, `has_passthrough()` — implemented in Step 5.3). *Actual OpenXR integration deferred to Step 5.2 remaining work.*