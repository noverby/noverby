# overby.me Homepage — Dioxus Port Plan

Port of `web/homepage` (React/TypeScript/Three.js) to Rust using [Dioxus](https://github.com/DioxusLabs/dioxus) targeting WebAssembly.

## Original App Summary

Personal identity site for Niclas Overby featuring:

- **/** — Interactive 3D force-directed graph (Three.js) showing profiles, categories, and connections
- **/search** — Redirect proxy: extracts `?url=...q=<query>` and redirects to Startpage
- **/x** — Redirect proxy: rewrites X/Twitter URLs through xcancel.com
- **/yt** — YouTube embed: extracts video ID and renders fullscreen iframe player

30 graph nodes (personal profiles, platforms, categories) with 45 links, AVIF icon sprites, dark background (#222222), pink glow labels.

## Architecture Decisions

| Concern | React (original) | Dioxus (port) |
|---------|------------------|---------------|
| Language | TypeScript | Rust |
| UI Framework | React 19 | Dioxus 0.7 |
| Routing | React Router 7 | Dioxus Router |
| 3D Graphics | Three.js + react-force-graph-3d | JS interop (Three.js via eval/wasm-bindgen) |
| Styling | Inline CSS | Inline styles + minimal CSS |
| Build | Rsbuild (Deno) | dx (Dioxus CLI) |
| Backend | Rust/Axum (Scaleway serverless) | Unchanged (separate crate) |

### Key Challenge: 3D Force Graph

The homepage's core feature is a Three.js force-directed graph. Options:

1. **JS interop** (recommended) — Load Three.js + force-graph from CDN via `<script>`, initialize from Dioxus using `eval()` or `web_sys` DOM manipulation. The graph is self-contained and doesn't need Rust-side state.
2. **Pure Rust** — Use a Rust WebGL library (e.g., `three-d`, `kiss3d`). Would require reimplementing the force-graph layout algorithm. High effort, low benefit.
3. **Hybrid** — Render the graph in a `<canvas>` managed by JS, with Dioxus handling routing and the surrounding page shell.

Approach: **Option 3 (Hybrid)** — Dioxus owns routing and page structure; the 3D graph is initialized via JS interop on the homepage route.

## Phases

### Phase 1: Project Scaffolding

- [ ] Create Cargo.toml with dioxus 0.7 (web + router features)
- [ ] Create Dioxus.toml for dx CLI
- [ ] Create justfile (dev, build, check, fmt, clippy)
- [ ] Create default.nix (devShell + package)
- [ ] Copy public/icons/ assets
- [ ] Initial commit

### Phase 2: Routing & Utility Pages

- [ ] Define Route enum: Home, Search, X, Yt
- [ ] Implement Search page — parse `?url=` param, extract `q=`, redirect to Startpage
- [ ] Implement X page — parse `?url=` param, extract Twitter path, redirect to xcancel.com
- [ ] Implement Yt page — parse `?url=` param, extract video ID, render fullscreen `<iframe>`

### Phase 3: 3D Graph via JS Interop

- [ ] Add Three.js + 3d-force-graph as CDN `<script>` tags in Dioxus.toml or index.html
- [ ] Create Graph component that initializes the force graph via `document.eval()`
- [ ] Port node data (30 nodes with id, url, icon, color, desc, opacity)
- [ ] Port link data (45 connections)
- [ ] Port node rendering logic (sprites for profiles, sphere+icon for categories)
- [ ] Port label styling (white text, pink glow shadow)
- [ ] Port click handler (navigate to node.url)
- [ ] Set background color #222222, link particles

### Phase 4: Polish & Build

- [ ] Mastodon rel="me" verification link on homepage
- [ ] HTML title "Niclas Overby Ⓝ"
- [ ] Responsive viewport fill for graph canvas
- [ ] Nix package derivation (dx build --release)
- [ ] `_redirects` for Matrix well-known + SPA fallback

## File Structure

```text
web/homepage-dioxus/
├── PLAN.md
├── Cargo.toml
├── Dioxus.toml
├── justfile
├── default.nix
├── assets/
│   ├── style.css             # Minimal global styles
│   ├── icons/                # AVIF node icons (copied from homepage/public/icons/)
│   ├── favicon.ico
│   └── graph.js              # Three.js force graph initialization script
└── src/
    ├── main.rs               # Entry point, router, launch
    ├── route.rs              # Route enum
    └── components/
        ├── mod.rs
        ├── home.rs           # Homepage — loads graph via JS interop
        ├── search.rs         # Search redirect
        ├── x.rs              # X/Twitter redirect
        └── yt.rs             # YouTube embed
```

## Graph Data Reference

### Nodes (30)

| ID | Type | Icon | Color | URL |
|----|------|------|-------|-----|
| Niclas Overby | center | me.avif | — | — |
| Commerce | category | commerce.avif | #45b1e8 | — |
| Improve | category | improve.avif | #7fff00 | — |
| Connect | category | connect.avif | #e34234 | — |
| Immerse | category | immerse.avif | #ff7f50 | — |
| Give | category | give.avif | #6a5acd | — |
| Fediverse | hub | fediverse.avif | #000000 | fediverse.info |
| Atmosphere | hub | atmosphere.avif | #00ffff (10%) | atproto.com |
| Bridgy | hub | bridgy.avif | #ffffff (10%) | fed.brid.gy |
| + 21 profile nodes | profile | *.avif | — | various URLs |

### Links (45)

Center → 5 categories, categories → profiles, protocol hubs interconnected.

## Notes

- The backend (Rust/Axum RSS proxy on Scaleway) is **not part of this port** — it's a separate crate
- Icons are AVIF format for optimal compression; all 30 files total ~200KB
- The `_redirects` file handles Matrix federation well-known endpoint delegation
- The graph uses `react-force-graph-3d` which wraps `three-d-force-graph` which wraps Three.js — for JS interop we can use the underlying `3d-force-graph` library directly (no React wrapper needed)
