# Homepage

Personal homepage for [overby.me](https://overby.me) — an interactive 3D graph visualization that maps out online presence, interests, and connections.

## Overview

The landing page renders a 3D force-directed graph (using [three.js](https://threejs.org/) via [react-force-graph-3d](https://github.com/vasturiano/react-force-graph-3d)) where nodes represent profiles, platforms, and life categories (Commerce, Improve, Connect, Immerse, Give). Clicking a node navigates to the corresponding URL.

Additional utility routes:

- `/search` — Redirects search queries to [Startpage](https://startpage.com)
- `/x` — Redirects X/Twitter links through [xcancel.com](https://xcancel.com)
- `/yt` — Embeds YouTube videos in a clean full-screen player

## Tech Stack

- **React 19** with TypeScript
- **Rsbuild** for bundling
- **Deno** as the JavaScript runtime
- **Three.js** / **react-force-graph-3d** for 3D graph rendering
- **React Router** for client-side routing
- **Nix** for reproducible dev environments
- **Just** as a command runner

## Getting Started

### Prerequisites

- [Deno](https://deno.land/)
- [Just](https://github.com/casey/just)

Or use the Nix dev shell which provides both.

### Development

```sh
just dev
```

### Build

```sh
just build
```

### Preview

```sh
just start
```

### Lint

```sh
just lint
```

## License

[AGPL-3.0](LICENSE)