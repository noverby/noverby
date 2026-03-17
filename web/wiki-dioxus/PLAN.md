# RadikalWiki Dioxus Port Plan

Port of `web/wiki` (React/TypeScript) to Rust using [Dioxus](https://github.com/DioxusLabs/dioxus) targeting WebAssembly.

## Architecture Decisions

| Concern | React (original) | Dioxus (port) |
|---------|------------------|---------------|
| Language | TypeScript | Rust |
| UI Framework | React 18 + MUI 7 | Dioxus 0.7 + custom CSS |
| Routing | React Router 7 | Dioxus Router |
| State | React Context + GQty | Dioxus signals + cynic |
| GraphQL | GQty (auto-generated) | cynic (struct-first, schema-validated) |
| Auth | @nhost/react | NHost REST API via reqwest |
| Styling | Emotion CSS-in-JS + MUI | CSS modules / Tailwind-style utility classes |
| i18n | i18next | rust-i18n |
| Rich Text | Slate.js | Custom read-only renderer (write later) |
| Charts | DevExpress | plotters (SVG) or charming |
| Maps | MapLibre GL | maplibre-rs or JS interop |
| Build | Rsbuild (RSPack) | dx (Dioxus CLI 0.7) |

## Phases

### Phase 1: Project Scaffolding

- [x] Create Cargo.toml with dioxus (web feature), dioxus-router
- [x] Create Dioxus.toml for dx CLI
- [x] Create main.rs entry point
- [x] Create justfile for dev/build commands
- [x] Add default.nix for Nix integration
- [x] Initial commit

### Phase 2: App Shell & Routing

- [x] Define route enum (Home, Login, Register, ResetPassword, SetPassword, Unverified, Path)
- [x] Create Layout component (responsive shell with bottom bar / side drawer)
- [x] Create SearchField, BreadCrumbs placeholder
- [x] Wire up basic navigation

### Phase 3: Authentication

- [x] NHost client (sign in, sign up, sign out, session refresh via REST)
- [x] Session context (user id, email, display name, access token)
- [x] Auth pages: Login, Register, ResetPassword, SetPassword, Unverified
- [x] Auth-gated UI (show login/register when unauthenticated)

### Phase 4: GraphQL & Data Model

- [x] Set up cynic with schema.graphql
- [x] Node query/mutation operations (query by id, query by path, insert, update, delete)
- [x] use_node hook equivalent returning reactive node data
- [x] Path resolution (recursive key-based lookup like PathLoader)

### Phase 5: Content Rendering

- [x] MimeLoader equivalent — route by node.mimeId
- [x] FolderApp — list children with icons
- [x] ContentApp — render Slate.js JSON as read-only HTML
- [x] FileApp — display files (images, PDF embed, audio/video)
- [x] HomeApp — welcome screen with login/register or greeting

### Phase 6: Interactive Features

- [x] EditorApp — contenteditable rich text editor with save/publish
- [x] VoteApp / PolicyApp / PollApp — voting UI with poll options display
- [x] SpeakApp — speaker queue with join/remove actions via GraphQL mutations
- [x] MemberApp — member management with invite input
- [x] SortApp — HTML5 drag-and-drop reordering with save

### Phase 7: Polish

- [x] i18n with Danish and English translations
- [x] M3-inspired theming (light/dark toggle, color tokens)
- [x] Error boundaries and loading states
- [x] Responsive design (mobile drawer, desktop sidebar)
- [x] Breadcrumb navigation
- [x] GraphQL search with live results
- [x] User menu popover with all options
- [x] Language toggle (Da/En) in user menu
- [x] App rail sidebar for large screens
- [x] ?app= query parameter routing
- [x] Full-path folder navigation
- [x] Snackbar notifications

### Phase 8: Build & Deploy

- [x] Nix package derivation
- [x] Optimized WASM build (dx build --release runs wasm-opt)
- [x] Asset hashing (handled by Dioxus CLI manganis)

## File Structure

```text
web/wiki-dioxus/
├── PLAN.md
├── Cargo.toml
├── Dioxus.toml
├── justfile
├── default.nix
├── assets/
│   └── style.css            # M3-inspired theme (light/dark CSS vars)
├── graphql/
│   └── schema.graphql       # Hasura schema (from wiki/core/gql/schema.gql)
└── src/
    ├── main.rs              # Entry point, router setup, snackbar
    ├── route.rs             # Route enum definition (#[derive(Routable)])
    ├── nhost.rs             # NHost auth client (REST API)
    ├── graphql.rs           # cynic GraphQL client + queries + mutations
    ├── session.rs           # Session context (global signals + localStorage)
    ├── i18n.rs              # Internationalization (Da/En inline translations)
    ├── theme.rs             # Theme context (light/dark toggle)
    ├── snackbar.rs          # Snackbar notification system
    └── components/
        ├── mod.rs
        ├── layout.rs        # Layout, Breadcrumbs, SearchBar, UserMenu, AppRail, Drawer
        ├── home.rs          # HomeApp (welcome screen)
        ├── auth.rs          # Login, Register, ResetPassword, SetPassword, Unverified
        ├── folder.rs        # FolderApp (child list with full-path navigation)
        ├── content.rs       # ContentApp (Slate.js JSON read-only renderer)
        ├── file.rs          # FileApp (image, video, audio, PDF, download)
        ├── node.rs          # NodeApp (generic node viewer)
        ├── loader.rs        # PathPage, MimeLoader, ?app= routing
        ├── vote.rs          # VoteApp, PolicyApp, PollApp
        ├── speak.rs         # SpeakApp (join/remove via mutations)
        ├── member.rs        # MemberApp (member list + invite)
        ├── editor.rs        # EditorApp (contenteditable + save)
        └── sort.rs          # SortApp (HTML5 drag-and-drop)
```

## Notes

- EditorApp uses `contenteditable` for editing — a full Slate.js port would need deeper JS interop
- SortApp uses native HTML5 drag-and-drop events
- Slate.js read-only rendering is fully implemented in pure Rust
- GraphQL mutations are implemented for node insert/delete (used by SpeakApp)
- The NHost GraphQL endpoint is at `https://{subdomain}.hasura.{region}.nhost.run/v1/graphql`
- Auth endpoint is at `https://{subdomain}.auth.{region}.nhost.run/v1`
