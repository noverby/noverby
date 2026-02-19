# RadikalWiki

A full open-source platform for democratic organizations — enabling voting, polls, speaker queue management, collaborative document editing, and conference communication.

Built for and used by Danish political organizations to run conferences, manage proposals, and conduct live votes.

![RadikalWiki screenshot](doc/radikalwiki.avif)

## Features

- **Voting & Polls** — Create and manage votes on policies, candidates, and positions with real-time results and charts
- **Speaker Queue** — Manage speaker lists with an admin panel and participant dial-in
- **Document Editing** — Rich text editor powered by Slate.js with publishing, authoring, and export to DOCX/XLSX
- **Event Management** — Organize conferences and events with structured content trees
- **Groups & Members** — Manage organizations, groups, memberships, and invitations
- **Permissions** — Fine-grained access control for content and administrative actions
- **Screen/Projection Mode** — Dedicated views for projecting live results and speaker queues
- **Maps** — Interactive map integration with MapLibre GL
- **Material Design 3** — Adaptive theming with dynamic color support
- **Real-time Updates** — GraphQL subscriptions for live data across all clients
- **Authentication** — User registration, login, password reset, and email verification via NHost

## Tech Stack

| Layer | Technology |
|---|---|
| Runtime | [Deno](https://deno.land) |
| Build | [Rsbuild](https://rsbuild.dev) |
| Frontend | [React](https://react.dev) + [React Router](https://reactrouter.com) |
| UI | [MUI (Material UI)](https://mui.com) |
| Editor | [Slate.js](https://www.slatejs.org) |
| Charts | [DevExpress React Chart](https://devexpress.github.io/devextreme-reactive/react/chart/) |
| Maps | [MapLibre GL](https://maplibre.org) via [react-map-gl](https://visgl.github.io/react-map-gl/) |
| GraphQL | [GQty](https://gqty.dev) with subscriptions via [graphql-ws](https://github.com/enisdenjo/graphql-ws) |
| Backend | [NHost](https://nhost.io) (Auth, Storage, Hasura GraphQL) |
| Dev Environment | [Nix](https://nixos.org) + [just](https://just.systems) |

## Getting Started

### Prerequisites

- [Deno](https://deno.land) installed
- [just](https://just.systems) command runner (or use [Nix](https://nixos.org) devshell which provides both)

### Development

```sh
# Install dependencies
just install

# Start development server (generates GraphQL types and opens browser)
just dev
```

### Build & Preview

```sh
# Production build
just build

# Preview production build locally
just start
```

### Other Commands

```sh
# Regenerate GraphQL types from schema
just build-gqty

# Lint the codebase
just lint
```

## Project Structure

```text
wiki/
├── core/           # Shared logic — GraphQL client, hooks, theming, i18n, utilities
│   ├── gql/        # Generated GraphQL types and client (GQty)
│   ├── hooks/      # Custom React hooks
│   ├── theme/      # Material Design 3 theming
│   └── types/      # TypeScript type definitions
├── src/
│   ├── components/ # UI components organized by domain
│   │   ├── admin/      # Admin panel
│   │   ├── auth/       # Authentication views
│   │   ├── content/    # Document editor and content management
│   │   ├── event/      # Event management
│   │   ├── group/      # Group management
│   │   ├── member/     # Member management
│   │   ├── poll/       # Polls with live charts
│   │   ├── speak/      # Speaker queue
│   │   └── vote/       # Voting on policies, candidates, positions
│   └── pages/      # Route pages
├── public/         # Static assets
└── doc/            # Documentation assets
```

## License

This project is licensed under the [GNU Affero General Public License v3.0 (AGPL-3.0)](LICENSE).