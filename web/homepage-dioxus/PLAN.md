# PLAN: Port `web/homepage` to Dioxus with Pure Rust WebGL

## Goal

Replace the React/TypeScript homepage at `web/homepage/` with a pure Rust
implementation using **Dioxus** (web target, compiled to WASM) and raw **WebGL**
via `web-sys` bindings. The new project lives at `web/homepage-dioxus/` and can
replace the original once validated.

---

## 1. Current Architecture

```text
web/homepage/
├── src/
│   ├── index.tsx            # ReactDOM entry point
│   ├── App.tsx              # Router: /, /search, /x, /yt
│   ├── components/
│   │   └── Graph.tsx        # 3D force graph (react-force-graph-3d + three.js)
│   └── pages/
│       ├── index.tsx         # Shows <Graph /> on /
│       ├── search.tsx        # Redirects search queries → startpage.com
│       ├── x.tsx             # Redirects x.com links → xcancel.com
│       └── yt.tsx            # Embeds YouTube video in full-screen iframe
├── public/
│   ├── icons/               # ~28 .avif node icons (me.avif, github.avif, …)
│   ├── favicon.ico
│   └── _redirects            # Netlify/Vercel SPA + matrix well-known rules
├── backend/
│   └── src/handle.rs         # Axum handler proxying an RSS XML feed
├── package.json              # React 19, react-force-graph-3d, three, react-router-dom
├── rsbuild.config.ts
├── justfile
└── default.nix
```

**Key complexity**: `Graph.tsx` — 28 nodes, 43 links, custom Three.js
`nodeThreeObject` callback with textured sprites, colored semi-transparent
spheres, HTML tooltip labels, click-to-navigate, and animated directional
link particles.

---

## 2. Target Architecture

```text
web/homepage-dioxus/
├── src/
│   ├── main.rs               # Dioxus entry + router definition
│   ├── pages/
│   │   ├── index.rs          # / — mounts the WebGL canvas
│   │   ├── search.rs         # /search — redirect
│   │   ├── x.rs              # /x — redirect
│   │   └── yt.rs             # /yt — YouTube embed
│   └── graph/
│       ├── mod.rs            # Graph component (Dioxus ↔ WebGL bridge)
│       ├── data.rs           # Node/Link data (28 nodes, 43 links)
│       ├── simulation.rs     # Force-directed layout (velocity Verlet)
│       ├── renderer.rs       # WebGL rendering (shaders, draw calls)
│       ├── camera.rs         # Perspective camera + orbit controls
│       ├── texture.rs        # AVIF image loading → WebGL textures
│       ├── interaction.rs    # Raycasting, hover tooltips, click navigation
│       └── particles.rs      # Animated directional particles on links
├── public/                   # Symlink or copy from ../homepage/public
│   ├── icons/                # Same .avif files
│   ├── favicon.ico
│   └── _redirects
├── Cargo.toml
├── Dioxus.toml
├── justfile
└── default.nix
```

---

## 3. Crate Dependencies

```toml
[dependencies]
dioxus = { version = "0.6", features = ["web", "router"] }
web-sys = { version = "0.3", features = [
    "Window", "Document", "HtmlCanvasElement",
    "WebGlRenderingContext", "WebGlProgram", "WebGlShader",
    "WebGlBuffer", "WebGlTexture", "WebGlUniformLocation",
    "MouseEvent", "HtmlImageElement", "Performance",
    "Url", "UrlSearchParams", "Location",
] }
wasm-bindgen = "0.2"
wasm-bindgen-futures = "0.4"
js-sys = "0.3"
glam = "0.29"            # Vec3, Mat4, perspective projection
gloo-timers = "0.3"      # requestAnimationFrame loop
regex = "1"              # URL pattern matching for /search, /x, /yt
log = "0.4"
wasm-logger = "0.2"
```

No Three.js. No npm. No JS runtime.

---

## 4. Implementation Phases

### Phase 1 — Scaffolding & Router (Day 1)

**Files**: `main.rs`, `pages/*.rs`, `Cargo.toml`, `Dioxus.toml`

- [ ] `dx new` scaffold or manual Dioxus web project setup
- [ ] Define `#[derive(Routable)]` enum with 4 routes:

  ```rust
  #[derive(Routable, Clone)]
  enum Route {
      #[route("/")]
      Index {},
      #[route("/search")]
      Search {},
      #[route("/x")]
      X {},
      #[route("/yt")]
      Yt {},
  }
  ```

- [ ] Implement `Search` page: parse `?url=` query param, extract `q=` value
  via regex, redirect to `https://startpage.com/search?q={match}` using
  `web_sys::window().location().set_href()`
- [ ] Implement `X` page: parse `?url=`, extract path after `(x|twitter).com`,
  redirect to `https://xcancel.com{path}`
- [ ] Implement `Yt` page: parse `?url=`, extract 11-char video ID via regex,
  render full-screen `iframe` via `rsx!`
- [ ] `Index` page: placeholder `<canvas>` element, wired up in Phase 2
- [ ] `Dioxus.toml`: set title to `"Niclas Overby Ⓝ"`, configure
  `web.app.base_path`, asset dir → `public/`
- [ ] `justfile` with `dev`, `build`, `serve` targets using `dx`
- [ ] Verify the 3 redirect pages work in `dx serve`

### Phase 2 — Graph Data & Force Simulation (Day 2)

**Files**: `graph/data.rs`, `graph/simulation.rs`

- [ ] `data.rs` — define structs and static data:

  ```rust
  struct GraphNode {
      id: &'static str,
      desc: &'static str,
      icon: &'static str,
      color: Option<&'static str>,  // None = icon-only, Some = sphere+icon
      opacity: Option<f32>,
      url: Option<&'static str>,
  }
  struct GraphLink {
      source: &'static str,
      target: &'static str,
  }
  ```

  Port all 28 nodes and 43 links verbatim from `Graph.tsx`.

- [ ] `simulation.rs` — force-directed layout using velocity Verlet integration:
  - **Link force**: Hooke's law spring between connected nodes
    (`strength ~0.03`, `distance ~100`)
  - **Charge force**: repulsive N-body (Barnes-Hut optional for 28 nodes;
    brute force O(n²) is fine at this scale), `strength ~ -300`
  - **Center force**: pull toward origin `(0,0,0)`, `strength ~0.05`
  - **Velocity decay**: `0.6` per tick (damping)
  - **Alpha cooling**: start `1.0`, decay `0.99` per tick, stop below `0.001`
  - Expose `SimulationState` with `positions: Vec<Vec3>`,
    `velocities: Vec<Vec3>`, `fn tick(&mut self)`
  - Positions initialized randomly in `[-50, 50]³`

### Phase 3 — WebGL Renderer (Days 3–5)

**Files**: `graph/renderer.rs`, `graph/camera.rs`

This is the core effort. All rendering via `web_sys::WebGlRenderingContext`.

#### 3a — Canvas & GL Context Setup

- [ ] Create/acquire `<canvas>` from Dioxus `onmounted` event
- [ ] Get `WebGlRenderingContext` (WebGL 1 for max compatibility)
- [ ] Enable `BLEND` (for transparency), `DEPTH_TEST`
- [ ] Set clear color `#222222` (matches current `backgroundColor`)
- [ ] Handle canvas resize → update viewport + projection matrix

#### 3b — Shader Programs

Three shader programs:

1. **Billboard Sprite Shader** (textured quads, always face camera)
   - Vertex: takes `a_position` (quad corner), `u_center` (world pos),
     `u_size`, `u_viewMatrix`, `u_projMatrix`. Offsets quad corners in
     view-space so it always faces camera.
   - Fragment: samples `u_texture`, discards if `alpha < 0.01`

2. **Sphere Shader** (for category nodes with colored halos)
   - Vertex: standard MVP transform of a UV-sphere mesh
   - Fragment: Lambert diffuse shading, `u_color`, `u_opacity`,
     single directional light

3. **Line/Particle Shader** (links + directional particles)
   - Vertex: simple line from `a_start` to `a_end` (use `GL_LINES`)
   - Fragment: solid color `rgba(255, 255, 255, 0.2)` for links
   - Particles: small billboarded quads animated along link direction
     (2 particles per link, `width ~1px`)

#### 3c — Geometry Generation

- [ ] **Quad geometry**: unit quad `[-0.5, -0.5] → [0.5, 0.5]`, 4 verts,
  6 indices (2 triangles). Reused for all sprites.
- [ ] **Sphere geometry**: generate UV-sphere (16 segments × 12 rings),
  `radius = 15` (matching Three.js `SphereGeometry(15)`). Upload once as
  VBO/IBO, draw instanced per category node.
- [ ] **Line geometry**: dynamic VBO updated each frame from simulation
  positions.

#### 3d — Camera

- [ ] **Perspective projection**: `fov = 75°`, `near = 0.1`, `far = 10000`,
  aspect from canvas size. Use `glam::Mat4::perspective_rh_gl`.
- [ ] **Orbit controls**: track `theta`, `phi`, `distance` (initial ~300).
  Mouse drag rotates, scroll zooms. Convert spherical → cartesian for
  eye position, build view matrix with `Mat4::look_at_rh`.
- [ ] Wire mouse events from `<canvas>` (via Dioxus `onmousedown`,
  `onmousemove`, `onmouseup`, `onwheel`) to camera state.

#### 3e — Render Loop

- [ ] `requestAnimationFrame` loop via `gloo_timers` or manual
  `web_sys::window().request_animation_frame()`
- [ ] Each frame:
  1. `simulation.tick()` (if alpha > threshold)
  2. Update line VBO with new positions
  3. Update particle positions (advance `t += speed` along each link)
  4. Clear framebuffer
  5. Set viewport, upload camera matrices
  6. Draw links (`GL_LINES`)
  7. Draw particles (billboarded quads)
  8. For each node: draw sphere (if `color.is_some()`), then draw icon sprite
  9. (Tooltip overlay handled in Phase 5)

### Phase 4 — Texture Loading (Day 4, parallel with Phase 3)

**Files**: `graph/texture.rs`

- [ ] For each node icon (`.avif`), load via `HtmlImageElement`:

  ```rust
  let img = HtmlImageElement::new()?;
  img.set_src(&format!("icons/{}", node.icon));
  // onload callback → upload to WebGL texture
  ```

- [ ] On load: `gl.tex_image_2d_with_html_image_element(...)`,
  set `TEXTURE_WRAP_S/T = CLAMP_TO_EDGE`, `MIN/MAG_FILTER = LINEAR`
- [ ] Store `WebGlTexture` handles in a `HashMap<&str, WebGlTexture>`
- [ ] Before textures are loaded, render nodes as colored fallback circles
- [ ] AVIF support: modern browsers handle it natively via `<img>`.
  If worried about fallback, convert icons to `.webp` or `.png` as well.

### Phase 5 — Interaction (Day 5–6)

**Files**: `graph/interaction.rs`

#### 5a — Raycasting (click & hover)

- [ ] On mouse click/move: unproject screen coords `(x, y)` through
  inverse `proj * view` to get a ray in world space
- [ ] For each node, test ray–sphere intersection
  (radius = `20` for "Niclas Overby", `9` for regular icons, `15` for
  category spheres)
- [ ] Return nearest hit node (if any)

#### 5b — Click Navigation

- [ ] On click hit: if node has `url`, call
  `web_sys::window().location().set_href(url)`
- [ ] Set `cursor: pointer` on canvas when hovering a clickable node

#### 5c — Tooltip on Hover

- [ ] On hover hit: render an HTML overlay `<div>` positioned at the
  projected 2D screen coords of the node. Style matching current:

  ```text
  white-space: pre; color: #ffffff; font-size: 30px;
  text-shadow: 0 0 5px #000000, 2px 2px 18px #ff0072;
  ```

  Content = `node.desc` (with `\n` → actual newlines via `white-space: pre`)

- [ ] This is an HTML element overlaid on the canvas, managed by Dioxus
  `rsx!`, positioned absolutely using projected coordinates

### Phase 6 — Particles & Visual Polish (Day 6)

**Files**: `graph/particles.rs`

- [ ] Each link gets 2 directional particles (matching
  `linkDirectionalParticles={2}`)
- [ ] Particles are small billboarded quads (size ~1px, matching
  `linkDirectionalParticleWidth={1}`)
- [ ] Animate along link: `pos = lerp(source, target, t)`,
  `t` wraps `0.0 → 1.0`, 2 particles offset by `0.5`
- [ ] Color: white with slight alpha
- [ ] Subtle glow optional (additive blend)

### Phase 7 — Build & Deploy Config (Day 7)

**Files**: `Dioxus.toml`, `justfile`, `default.nix`

- [ ] `Dioxus.toml`:

  ```toml
  [application]
  name = "homepage"

  [web.app]
  title = "Niclas Overby Ⓝ"

  [web.resource]
  style = []
  script = []

  [web.watcher]
  watch_path = ["src"]
  ```

- [ ] `justfile`:

  ```just
  dev:
      dx serve

  build:
      dx build --release

  serve:
      dx serve --release

  clean:
      dx clean
  ```

- [ ] `default.nix`: Nix derivation using `wasm-pack` or `trunk` or `dx`
  to build the WASM output. Provide `devShells.homepage-dioxus` with
  `rustup`, `wasm32-unknown-unknown` target, `dx` CLI, `just`.
- [ ] Copy/symlink `public/icons/`, `public/favicon.ico`, `public/_redirects`
  into the Dioxus asset output directory
- [ ] Verify `_redirects` SPA fallback still works for deployment target
  (Vercel/Netlify/Cloudflare)

### Phase 8 — Backend Integration (Day 7)

- [ ] Backend `handle.rs` is **unchanged** — it's already Rust/Axum and
  independent of the frontend framework
- [ ] Verify the backend Nix build still works via `imports = [./backend/...]`

---

## 5. Mapping: React → Dioxus Equivalents

| React / Three.js | Dioxus / Pure Rust |
|---|---|
| `ReactDOM.createRoot().render()` | `dioxus::launch(App)` |
| `<BrowserRouter><Routes>` | `#[derive(Routable)]` + `Router` component |
| `useEffect(() => { redirect }, [])` | `use_effect(move \|\| { web_sys redirect })` |
| `useSearchParams()` | `web_sys::Url::new(location.href).search_params()` |
| `useRef` + DOM element | `onmounted` event + `MountedData` |
| `useState` / `useEffect` | `use_signal` / `use_effect` |
| `ForceGraph3D` | Custom WebGL renderer + force sim |
| `THREE.WebGLRenderer` | `web_sys::WebGlRenderingContext` raw calls |
| `THREE.PerspectiveCamera` | `glam::Mat4::perspective_rh_gl` + orbit logic |
| `THREE.TextureLoader.load()` | `HtmlImageElement` + `gl.tex_image_2d()` |
| `THREE.Sprite` + `SpriteMaterial` | Billboarded textured quad (custom shader) |
| `THREE.Mesh(SphereGeometry, MeshLambertMaterial)` | UV-sphere VBO + Lambert fragment shader |
| `THREE.Group.add(sprite, mesh)` | Draw sphere then sprite at same position |
| `nodeLabel` (HTML tooltip) | Dioxus `rsx!` overlay `<div>` positioned via projection |
| `onNodeClick` | Ray-sphere intersection test on canvas click |
| `linkDirectionalParticles` | Animated billboard quads along link vectors |
| `requestAnimationFrame` | `web_sys::window().request_animation_frame()` closure |

---

## 6. Risk Assessment

| Risk | Severity | Mitigation |
|---|---|---|
| AVIF loading in `<img>` element from WASM | Low | Browsers handle AVIF natively; fallback: convert to `.webp` |
| WebGL shader debugging | Medium | Use browser WebGL inspector; write shader compilation error logging |
| Force layout visual mismatch vs. d3-force | Medium | Tune spring/charge/center constants iteratively; 28 nodes is small enough to match by eye |
| `requestAnimationFrame` ownership in WASM closures | Medium | Use `Closure::wrap` + `Rc<RefCell<>>` pattern or `gloo` helpers |
| Canvas sizing / HiDPI | Low | Read `devicePixelRatio`, scale canvas buffer size accordingly |
| Tooltip positioning accuracy | Low | Project 3D world pos → NDC → screen coords; straightforward math |
| WASM binary size | Medium | Use `wasm-opt -Oz`, enable LTO, `opt-level = "z"` in release profile |

---

## 7. Estimated Effort

| Phase | Effort |
|---|---|
| 1 — Scaffolding & Router | 0.5 days |
| 2 — Data & Simulation | 0.5 days |
| 3 — WebGL Renderer | 3 days |
| 4 — Texture Loading | 0.5 days |
| 5 — Interaction | 1 day |
| 6 — Particles & Polish | 0.5 days |
| 7 — Build & Deploy | 0.5 days |
| 8 — Backend Integration | 0.25 days |
| **Total** | **~7 days** |

---

## 8. Acceptance Criteria

- [ ] `dx serve` launches the app, all 4 routes work
- [ ] `/` renders a 3D force-directed graph with all 28 nodes and 43 links
- [ ] Nodes display their `.avif` icons as textured sprites
- [ ] Category nodes (Commerce, Improve, Connect, Immerse, Give, Fediverse,
      Atmosphere, Bridgy) render a colored semi-transparent sphere behind icon
- [ ] "Niclas Overby" node is visually larger (40 vs 18 units)
- [ ] Mouse drag orbits the camera, scroll zooms
- [ ] Hovering a node shows its `desc` as a styled HTML tooltip
- [ ] Clicking a node with a `url` navigates to that URL
- [ ] Animated particles flow along links (2 per link)
- [ ] Background color is `#222222`
- [ ] `/search?url=...q=rust` redirects to `startpage.com/search?q=rust`
- [ ] `/x?url=https://x.com/user/status/123` redirects to `xcancel.com/user/status/123`
- [ ] `/yt?url=https://youtube.com/watch?v=dQw4w9WgXcQ` shows embedded player
- [ ] `<a rel="me" href="https://mas.to/@niclasoverby">` present in index page HTML
- [ ] `_redirects` file preserved for deployment
- [ ] Zero JavaScript in the output (only WASM + HTML + CSS)
- [ ] Backend `handle.rs` unchanged and functional
- [ ] `dx build --release` produces optimized WASM bundle
- [ ] Nix build works via `default.nix`