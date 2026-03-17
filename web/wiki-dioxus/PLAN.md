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

- [ ] EditorApp — rich text editing (simplified, or JS interop with Slate)
- [ ] VoteApp / PolicyApp / PollApp — voting UI
- [ ] SpeakApp — speaker queue
- [ ] MemberApp — member management
- [ ] SortApp — drag-and-drop reordering

### Phase 7: Polish

- [ ] i18n with Danish and English translations
- [ ] M3-inspired theming (light/dark toggle, color tokens)
- [ ] Error boundaries and loading states
- [ ] Responsive design (mobile drawer, desktop sidebar)
- [ ] Snackbar notifications

### Phase 8: Build & Deploy

- [ ] Nix package derivation
- [ ] Optimized WASM build (wasm-opt)
- [ ] Asset hashing and cache busting

## File Structure

```text
web/wiki-dioxus/
├── PLAN.md
├── Cargo.toml
├── Dioxus.toml
├── justfile
├── default.nix
├── assets/
│   └── style.css
└── src/
    ├── main.rs              # Entry point, router setup
    ├── route.rs             # Route enum definition
    ├── nhost.rs             # NHost auth client
    ├── graphql.rs           # GraphQL client + operations
    ├── session.rs           # Session context (signals)
    ├── i18n.rs              # Internationalization
    ├── theme.rs             # Theme context (light/dark)
    ├── components/
    │   ├── mod.rs
    │   ├── layout.rs        # Layout, Bar, BottomBar, Drawer
    │   ├── search.rs        # SearchField
    │   ├── breadcrumbs.rs   # BreadCrumbs
    │   ├── home.rs          # HomeApp
    │   ├── auth.rs          # Login, Register, etc.
    │   ├── folder.rs        # FolderApp
    │   ├── content.rs       # ContentApp (read-only Slate renderer)
    │   ├── file.rs          # FileApp (image, video, audio, PDF)
    │   ├── node.rs          # NodeApp, UnknownApp
    │   ├── loader.rs        # Loader, MimeLoader, PathLoader, AppLoader
    │   ├── vote.rs          # VoteApp, PolicyApp, PollApp
    │   ├── speak.rs         # SpeakApp
    │   ├── member.rs        # MemberApp
    │   ├── editor.rs        # EditorApp
    │   └── sort.rs          # SortApp
    └── graphql/
        ├── mod.rs
        ├── schema.gql       # Copied from wiki/core/gql/schema.gql
        └── queries.graphql   # Hand-written queries
```

## Notes

- Phase 6 (interactive features) deferred — focus on read-only browsing first
- Slate.js editor may require JS interop for full editing; read-only rendering is pure Rust
- The NHost GraphQL endpoint is at `https://{subdomain}.hasura.{region}.nhost.run/v1/graphql`
- Auth endpoint is at `https://{subdomain}.auth.{region}.nhost.run/v1`
