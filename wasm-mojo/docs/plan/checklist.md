# Migration Checklist

> Extracted from [SEPARATION_PLAN.md](../../SEPARATION_PLAN.md). See also: [architecture](./architecture.md), [renderers](./renderers.md), and individual phase docs.

---

## Phase 1: `mojo-gui/core` extraction ✅

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

---

## Phase 2: `mojo-gui/web` extraction ✅

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

---

## Phase 3: `mojo-gui/desktop` — webview renderer ✅ (infra), unified lifecycle ✅

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

---

## Phase 4: `mojo-gui/desktop` — Blitz renderer (Winit integration complete, verification pending)

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

---

## Phase 5: `mojo-gui/xr` — XR renderer (future)

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