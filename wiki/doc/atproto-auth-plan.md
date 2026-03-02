# AT Protocol Authentication Migration Plan

Port RadikalWiki from NHost authentication to Bluesky AT Protocol OAuth,
while keeping NHost auth working during the transition period.

## Current NHost Integration Surface

### Core Infrastructure (4 files)

| File | NHost Usage |
|---|---|
| `core/nhost.tsx` | `NhostClient` singleton (auth + storage) |
| `core/gql/index.ts` | `nhost.auth.isAuthenticated()`, `nhost.auth.getAccessToken()` for GraphQL headers |
| `core/hooks/useFile.ts` | `nhost.storage.getPublicUrl()` |
| `core/hooks/useFiles.ts` | `nhost.storage.getPublicUrl()` |

### `@nhost/react` Hook Usage (15 component files)

| Hook | Used In |
|---|---|
| `useAuthenticationStatus` | `AuthForm`, `Layout`, `HomeApp`, `UnknownApp`, `[...path]`, `set-password`, `unverified` |
| `useAuthenticated` | `useApps` |
| `useUserId` | `useApps`, `FolderList`, `InvitesUserList`, `Drawer`, `HomeList`, `Layout`, `SpeakCard`, `CandidateList`, `QuestionList`, `VoteApp` |
| `useUserEmail` | `useApps`, `InvitesUserList`, `Layout` |
| `useUserDisplayName` | `HomeApp`, `Layout`, `SpeakDial`, `AddChangeButton` |

### `nhost.auth.*` Direct Calls (3 files)

| Method | File |
|---|---|
| `signIn`, `sendVerificationEmail` | `AuthForm.tsx` |
| `signUp` | `AuthForm.tsx` |
| `changePassword` | `AuthForm.tsx` |
| `resetPassword` | `AuthForm.tsx` |
| `signOut` | `UserMenu.tsx` |

### `nhost.storage.*` Direct Calls (2 component files)

| Method | File |
|---|---|
| `upload` | `FileUploader.tsx` |
| `getPresignedUrl` | `DownloadButton.tsx` |

### Auth Routes (5 pages)

| Route | Page File |
|---|---|
| `/user/login` | `src/pages/user/login.tsx` |
| `/user/register` | `src/pages/user/register.tsx` |
| `/user/reset-password` | `src/pages/user/reset-password.tsx` |
| `/user/set-password` | `src/pages/user/set-password.tsx` |
| `/user/unverified` | `src/pages/user/unverified.tsx` |

---

## Design Decisions

| Question | Decision |
|---|---|
| Hasura auth strategy | Deno auth webhook that validates both NHost JWTs and atproto DPoP tokens |
| Email for invitations | Ask users to provide email in-app after atproto login |
| File storage | Keep NHost storage — `nhost.storage.*` calls stay unchanged |
| Migration strategy | Dual auth — support both NHost and atproto during transition |
| Auth server hosting | NixOS `home` server, reverse-proxied by Caddy |

---

## Architecture Overview

```text
                    ┌──────────────────────┐
                    │   Browser (React)    │
                    │                      │
                    │  NHost auth (legacy) │
                    │  atproto OAuth+DPoP  │
                    └──────┬───────────────┘
                           │
              Authorization: Bearer <token>
                           │
                    ┌──────▼───────────────┐
                    │   Hasura GraphQL     │
                    │   (webhook mode)     │
                    └──────┬───────────────┘
                           │
              GET /validate (forward headers)
                           │
                    ┌──────▼───────────────┐
                    │   Deno Auth Webhook  │
                    │                      │
                    │  1. Try NHost JWT    │
                    │  2. Try atproto DPoP │
                    │  3. Return Hasura    │
                    │     session vars     │
                    └──────────────────────┘
```

---

## Phase 1: Auth Webhook Server

Create a lightweight Deno server that acts as a Hasura authentication webhook.
Hasura switches from JWT mode to webhook mode, calling this server on every
request to resolve session variables.

### 1.1 Create `wiki/server/` Directory

```text
wiki/server/
├── main.ts           # Deno HTTP server entrypoint
├── validate.ts       # GET /validate handler
├── nhost.ts          # NHost JWT validation (JWKS)
├── atproto.ts        # atproto DPoP token validation
├── hasura.ts         # Hasura session variable builders
├── users.ts          # User lookup/creation via Hasura admin
└── deno.json         # Deno config + imports
```

### 1.2 `GET /validate` — Dual Auth Handler

The webhook receives the original request headers from Hasura and must return
either session variables or a 401.

```text
Request headers from Hasura:
  Authorization: Bearer <token>
  X-Auth-Provider: nhost | atproto    (optional hint from client)
  DPoP: <proof>                       (present for atproto requests)

Response (200):
  {
    "X-Hasura-Role": "user",
    "X-Hasura-User-Id": "<uuid>"
  }

Response (401):
  { "error": "unauthorized" }
```

Logic:

1. If `DPoP` header is present → validate as atproto token
   - Verify the DPoP proof against the access token
   - Extract the DID from the token
   - Look up user by DID in the `users` table (via Hasura admin API)
   - If no user exists, create one with `defaultRole: "user"` and DID as metadata
   - Return `X-Hasura-User-Id` = the user's UUID (mapped from DID)
2. Else → validate as NHost JWT
   - Fetch NHost JWKS from `https://<subdomain>.auth.<region>.nhost.run/v1/.well-known/jwks.json`
   - Verify the JWT signature and claims
   - Extract `x-hasura-user-id` and `x-hasura-default-role` from the JWT claims
   - Return them as Hasura session variables
3. If neither validates → return 401

### 1.3 atproto Token Validation

Use `@atproto/oauth-client-node` for server-side token introspection, or
validate manually:

- Decode the DPoP access token (it's a JWT-like structure bound to a DPoP proof)
- Verify the DPoP proof:
  - The `jti` is unique (replay protection)
  - The `htm` matches the HTTP method
  - The `htu` matches the request URL
  - The `ath` matches the hash of the access token
  - The proof was signed by the key in the DPoP proof header
- Extract the `sub` claim = DID
- Optionally verify the DID resolves and the token issuer is the user's PDS

### 1.4 User Mapping Table

Add a `user_providers` table (or `user_identities`) to the Hasura-managed
Postgres database to link atproto DIDs to existing NHost user UUIDs:

```sql
CREATE TABLE public.user_providers (
  id          uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id     uuid NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
  provider    text NOT NULL,          -- 'nhost' | 'atproto'
  provider_id text NOT NULL,          -- NHost UUID or atproto DID
  created_at  timestamptz NOT NULL DEFAULT now(),
  UNIQUE (provider, provider_id)
);
```

When the webhook validates an atproto token:

1. Look up `user_providers` by `(provider='atproto', provider_id=<DID>)`
2. If found → return the linked `user_id` as `X-Hasura-User-Id`
3. If not found → create a new `auth.users` row + `user_providers` entry
4. The user's `displayName` is fetched from their Bluesky profile on first login

This lets existing NHost users link their Bluesky account later, and new users
get a fresh account automatically.

### 1.5 Hasura Configuration Change

Switch Hasura from JWT mode to webhook mode:

```yaml
# Before:
HASURA_GRAPHQL_JWT_SECRET: '{"type":"RS256","jwk_url":"https://...nhost.run/v1/.well-known/jwks.json"}'

# After:
HASURA_GRAPHQL_AUTH_HOOK: "http://localhost:4180/validate"
HASURA_GRAPHQL_AUTH_HOOK_MODE: "GET"
```

The unauthorized role stays `public` for unauthenticated requests.

---

## Phase 2: atproto OAuth Client (`core/atproto.ts`)

### 2.1 New Dependencies

```json
{
  "dependencies": {
    "@atproto/oauth-client-browser": "^0.3.0",
    "@atproto/api": "^0.13.0"
  }
}
```

### 2.2 `core/atproto.ts`

Create the browser-side atproto OAuth client. This replaces `core/nhost.tsx`
for authentication (but `nhost.tsx` stays for storage).

The app must serve a `client-metadata.json` at its public URL. The AT
Protocol authorization server fetches this `client_id` URL to verify
the OAuth client's identity, so the metadata **must match the origin
where the app is served**. A static file hardcoding one domain would
break on other origins (e.g. `rebuild.radikal.wiki` vs `radikal.wiki`).

Instead of a static file, an rsbuild plugin (`pluginClientMetadata` in
`rsbuild.config.ts`) generates `client-metadata.json` at build time
using the `PUBLIC_SITE_URL` environment variable (defaults to
`https://radikal.wiki`). The generated document looks like:

```json
{
  "client_id": "${PUBLIC_SITE_URL}/client-metadata.json",
  "client_name": "RadikalWiki",
  "client_uri": "${PUBLIC_SITE_URL}",
  "redirect_uris": ["${PUBLIC_SITE_URL}/auth/callback"],
  "scope": "atproto transition:generic",
  "grant_types": ["authorization_code", "refresh_token"],
  "response_types": ["code"],
  "token_endpoint_auth_method": "none",
  "application_type": "web",
  "dpop_bound_access_tokens": true
}
```

For multi-origin deployments, set `PUBLIC_SITE_URL` per build target:

- `PUBLIC_SITE_URL=https://radikal.wiki` for production
- `PUBLIC_SITE_URL=https://rebuild.radikal.wiki` for staging

In dev mode (`http://localhost`), atproto treats the client as a
loopback client and skips the metadata fetch, so no special config
is needed.

The `core/atproto.ts` module mirrors the build-generated metadata but
uses `window.location.origin` so it works across environments without
relying on the static file at runtime:

```typescript
import { BrowserOAuthClient } from "@atproto/oauth-client-browser";

const origin = typeof window !== "undefined" ? window.location.origin : "";

const atprotoClient = new BrowserOAuthClient({
  clientMetadata: {
    client_id: `${origin}/client-metadata.json`,
    redirect_uris: [`${origin}/auth/callback`],
    scope: "atproto transition:generic",
    grant_types: ["authorization_code", "refresh_token"],
    response_types: ["code"],
    token_endpoint_auth_method: "none",
    application_type: "web",
    dpop_bound_access_tokens: true,
  },
  handleResolver: "https://bsky.social",
});

export { atprotoClient };
```

Key methods:

- `atprotoClient.signIn(handle)` → redirects to Bluesky authorization
- `atprotoClient.init()` → restores session on page load / handles callback
- Session object exposes: `did`, `handle`, access token, DPoP proof generation

---

## Phase 3: Auth Context & Hooks

### 3.1 `core/hooks/useAtproto.tsx` — atproto Auth Provider + Hooks

Create a React context that wraps the atproto OAuth client and exposes hooks
with the same shape as the `@nhost/react` hooks the codebase already uses.
This makes Phase 5 (component migration) a simple import swap.

```text
AtprotoAuthProvider
  ├── Initializes atprotoClient.init() on mount
  ├── Listens for session events (login, logout, token refresh)
  ├── Fetches Bluesky profile (displayName, avatar) on session start
  ├── Stores session in context state
  └── Provides: AtprotoAuthContext

Exported hooks:
  useAtprotoAuth()           → { isAuthenticated, isLoading, did, handle, session }
  useAtprotoSignIn(handle)   → triggers OAuth flow
  useAtprotoSignOut()        → clears session
  useAtprotoProfile()        → { displayName, avatarUrl, handle }
```

### 3.2 `core/hooks/useAuth.ts` — Unified Auth Facade (Dual Auth)

This is the key abstraction for the dual-auth transition period. It wraps
both NHost and atproto auth behind a single interface matching the `@nhost/react`
hook API:

```typescript
// Checks both providers, atproto takes precedence
export function useAuthenticationStatus(): { isAuthenticated: boolean; isLoading: boolean }
export function useAuthenticated(): boolean
export function useUserId(): string | null           // UUID from NHost or mapped UUID from atproto
export function useUserEmail(): string | null         // From NHost or user-provided
export function useUserDisplayName(): string | null   // From NHost or Bluesky profile
export function useSignOut(): () => Promise<void>     // Signs out of whichever is active
```

Implementation:

- If atproto session is active → use atproto values
- Else if NHost session is active → use NHost values (existing behavior)
- `useUserId()` always returns the Hasura UUID (the webhook maps DID → UUID)

### 3.3 Profile Email Collection (Verified)

After a new atproto user's first login, if they have no email on record,
show a one-time dialog asking for their email address. The dialog submits
the email to the auth webhook server (`POST /email/request-verification`),
which sends a verification email with a signed token link. The email is
only written to `auth.users.email` after the user clicks the link.

**Security rationale:** The `members` table uses `email` as an identity
key for invites — `members.email` is matched against `users.email` to
resolve pending invitations. If users could self-set their email without
verification, they could claim another user's pending invites by setting
their email to the victim's address.

The dialog is skippable but can be prompted again from settings.

---

## Phase 4: GraphQL Headers Update (`core/gql/index.ts`)

Update `getHeaders()` to support both auth providers:

```text
Current logic:
  if HASURA_GRAPHQL_ADMIN_SECRET → admin headers
  else if nhost.auth.isAuthenticated() → Bearer <NHost JWT>
  else → x-hasura-role: public

New logic:
  if HASURA_GRAPHQL_ADMIN_SECRET → admin headers
  else if atproto session active → Bearer <atproto token> + DPoP proof header
  else if nhost.auth.isAuthenticated() → Bearer <NHost JWT>
  else → x-hasura-role: public
```

For the atproto path, the `fetch` call needs both headers:

- `Authorization: Bearer <access_token>`
- `DPoP: <proof>` (generated per-request by the atproto session)

The `graphql-ws` subscription client also needs updated `connectionParams`
to pass the same headers.

**Important:** The `BrowserOAuthClient` session object provides a `fetchHandler`
or `dpopFetch` that automatically attaches DPoP proofs. Consider using this
as the fetcher for GQty instead of raw fetch + manual header management.

---

## Phase 5: App Shell Changes (`src/App.tsx`)

### 5.1 Add `AtprotoAuthProvider` Alongside `NhostProvider`

During the transition, both providers wrap the app:

```text
<NhostProvider nhost={nhost}>          ← stays for storage + legacy auth
  <AtprotoAuthProvider>                ← new
    <SessionProvider>
      ...
    </SessionProvider>
  </AtprotoAuthProvider>
</NhostProvider>
```

### 5.2 Add Callback Route

Add a route for the OAuth callback:

```text
<Route path="/auth/callback" element={<AuthCallback />} />
```

The `AuthCallback` component calls `atprotoClient.init()` which processes
the OAuth callback URL, exchanges the code for tokens, and redirects to `/`.

---

## Phase 6: Auth UI Changes

### 6.1 Rework `AuthForm.tsx`

Add a "Sign in with Bluesky" option to the existing login form:

```text
┌─────────────────────────────┐
│        🦋 RadikalWiki       │
│                             │
│  ┌───────────────────────┐  │
│  │ @handle.bsky.social   │  │
│  └───────────────────────┘  │
│  ┌───────────────────────┐  │
│  │ Sign in with Bluesky  │  │
│  └───────────────────────┘  │
│                             │
│  ─── or sign in with ───   │
│       email & password      │
│                             │
│  ┌───────────────────────┐  │
│  │ Email                 │  │
│  └───────────────────────┘  │
│  ┌───────────────────────┐  │
│  │ Password              │  │
│  └───────────────────────┘  │
│  ┌───────────────────────┐  │
│  │       Log in          │  │
│  └───────────────────────┘  │
│                             │
│  [Register]  [Reset pass]   │
└─────────────────────────────┘
```

The Bluesky button:

1. Validates the handle input
2. Calls `atprotoClient.signIn(handle)`
3. Browser redirects to Bluesky authorization server
4. User grants access
5. Redirects back to `/auth/callback`
6. `AtprotoAuthProvider` picks up the session

### 6.2 Route Changes

| Route | Action |
|---|---|
| `/user/login` | **Keep** — add Bluesky sign-in section above existing email/password |
| `/user/register` | **Keep** — for NHost users during transition |
| `/user/reset-password` | **Keep** — for NHost users during transition |
| `/user/set-password` | **Keep** — for NHost users during transition |
| `/user/unverified` | **Keep** — for NHost users during transition |
| `/auth/callback` | **Add** — atproto OAuth callback handler |

### 6.3 Account Linking UI

Add a section in user settings (or a prompt on login) for existing NHost
users to link their Bluesky account:

1. User is logged in via NHost
2. They enter their Bluesky handle and initiate OAuth
3. On successful atproto auth, the server creates a `user_providers` row
   linking the atproto DID to their existing NHost user UUID
4. Future atproto logins resolve to the same user

---

## Phase 7: Component Import Migration

Replace `@nhost/react` imports with `core/hooks/useAuth` across all 15
component files. This is a mechanical find-and-replace since the unified
hooks in Phase 3.2 expose the same API.

### Files to Update

```text
src/components/auth/AuthForm.tsx         — useAuthenticationStatus + nhost.auth.*
src/components/folder/FolderList.tsx      — useUserId
src/components/invite/InvitesUserList.tsx — useUserId, useUserEmail
src/components/layout/Drawer.tsx          — useUserId
src/components/layout/HomeApp.tsx         — useAuthenticationStatus, useUserDisplayName
src/components/layout/HomeList.tsx        — useUserId
src/components/layout/Layout.tsx          — useAuthenticationStatus, useUserDisplayName, useUserEmail, useUserId
src/components/layout/UserMenu.tsx        — useAuthenticationStatus + nhost.auth.signOut
src/components/node/UnknownApp.tsx        — useAuthenticationStatus
src/components/speak/SpeakCard.tsx        — useUserId
src/components/speak/SpeakDial.tsx        — useUserDisplayName
src/components/vote/AddChangeButton.tsx   — useUserDisplayName
src/components/vote/CandidateList.tsx     — useUserId
src/components/vote/QuestionList.tsx      — useUserId
src/components/vote/VoteApp.tsx           — useUserId
src/pages/[...path].tsx                   — useAuthenticationStatus
src/pages/user/set-password.tsx           — useAuthenticationStatus
src/pages/user/unverified.tsx             — useAuthenticationStatus
core/hooks/useApps.ts                     — useAuthenticated, useUserId, useUserEmail
```

For each file:

```text
// Before:
import { useUserId } from "@nhost/react";

// After:
import { useUserId } from "core/hooks/useAuth";
```

For `UserMenu.tsx`:

```text
// Before:
import { nhost } from "nhost";
const handleLogout = async () => { await nhost.auth.signOut(); ... };

// After:
import { useSignOut } from "core/hooks/useAuth";
const signOut = useSignOut();
const handleLogout = async () => { await signOut(); ... };
```

NHost storage imports (`nhost.storage.*`) in `useFile.ts`, `useFiles.ts`,
`FileUploader.tsx`, and `DownloadButton.tsx` remain **unchanged** — NHost
storage is kept.

---

## Phase 8: NixOS Deployment

### 8.1 NixOS Module: `wiki/server/wiki-auth.nix`

Create a NixOS module for the Deno auth webhook server, following the
pattern established by `tangled-spindle.nix`:

```nix
{ config, pkgs, lib, ... }:
let
  cfg = config.services.wiki-auth;
in {
  options.services.wiki-auth = {
    enable = lib.mkEnableOption "RadikalWiki auth webhook";
    port = lib.mkOption {
      type = lib.types.port;
      default = 4180;
    };
    nhostSubdomain = lib.mkOption { type = lib.types.str; };
    nhostRegion = lib.mkOption { type = lib.types.str; };
    hasuraAdminSecret = lib.mkOption { type = lib.types.str; };
    hasuraEndpoint = lib.mkOption { type = lib.types.str; };
    environmentFile = lib.mkOption {
      type = lib.types.path;
      description = "Environment file with secrets";
    };
  };

  config = lib.mkIf cfg.enable {
    systemd.services.wiki-auth = {
      description = "RadikalWiki Auth Webhook";
      after = [ "network.target" ];
      wantedBy = [ "multi-user.target" ];
      serviceConfig = {
        ExecStart = "${pkgs.deno}/bin/deno run --allow-net --allow-env wiki/server/main.ts";
        EnvironmentFile = cfg.environmentFile;
        Environment = [
          "PORT=${toString cfg.port}"
          "NHOST_SUBDOMAIN=${cfg.nhostSubdomain}"
          "NHOST_REGION=${cfg.nhostRegion}"
          "HASURA_ENDPOINT=${cfg.hasuraEndpoint}"
        ];
        DynamicUser = true;
        Restart = "on-failure";
        RestartSec = 5;
      };
    };
  };
}
```

### 8.2 Add to `home.nix`

```nix
wiki-auth
{
  services.wiki-auth = {
    enable = true;
    nhostSubdomain = "...";
    nhostRegion = "...";
    hasuraEndpoint = "https://....hasura.....nhost.run/v1/graphql";
    environmentFile = "/var/lib/wiki-auth/env";
  };

  # Caddy reverse proxy (if the webhook needs to be reachable externally,
  # e.g. for Hasura Cloud — skip if Hasura is on the same machine)
  services.caddy.virtualHosts."wiki-auth.overby.me" = {
    extraConfig = ''
      reverse_proxy localhost:4180
    '';
  };
}
```

### 8.3 Hasura Environment Update

Update the NHost Hasura configuration (via NHost dashboard or environment
variables) to point to the webhook:

```text
HASURA_GRAPHQL_AUTH_HOOK=https://wiki-auth.overby.me/validate
HASURA_GRAPHQL_AUTH_HOOK_MODE=GET
HASURA_GRAPHQL_UNAUTHORIZED_ROLE=public
```

If Hasura is NHost-managed (cloud), the webhook URL must be publicly
reachable, hence the Caddy virtualhost above.

---

## Phase 9: Database Migration

### 9.1 Create `user_providers` Table

Run via Hasura console or a migration:

```sql
CREATE TABLE public.user_providers (
  id          uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id     uuid NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
  provider    text NOT NULL CHECK (provider IN ('nhost', 'atproto')),
  provider_id text NOT NULL,
  handle      text,
  created_at  timestamptz NOT NULL DEFAULT now(),
  UNIQUE (provider, provider_id)
);

-- Index for fast lookups by the auth webhook
CREATE INDEX idx_user_providers_lookup
  ON public.user_providers (provider, provider_id);

-- Backfill existing NHost users
INSERT INTO public.user_providers (user_id, provider, provider_id)
SELECT id, 'nhost', id::text FROM auth.users;
```

### 9.2 Track in Hasura

- Track the `user_providers` table in Hasura
- Add object relationship `user_providers.user_id → users.id`
- Add array relationship on `users` → `user_providers`
- Set permissions: users can read their own providers, admin can write

### 9.3 Server-Side Verified Email Flow

The existing `users.email` column works for NHost users (set on registration).
For atproto users, this column starts as `null` and is populated only after
the user verifies their email address via a confirmation link.

**Security constraint:** The `members` table uses `email` as an identity key
for the invite system (`members_parent_id_email_key` unique constraint,
`emailUser` relationship). Allowing users to self-set their email via a
Hasura permission would let them claim other users' pending invites.
**Do NOT grant the `user` role update permission on `auth.users.email`.**

Instead, email updates go through the auth webhook server:

1. **`POST /email/request-verification`** (authenticated)
   - Validates the auth token (NHost JWT or atproto DPoP)
   - Checks the email isn't already taken by another user
   - Creates a signed JWT: `{ sub: userId, email, aud: "email-verify", exp: +1h }`
   - Sends a verification email with a link to `/email/verify?token=<jwt>`
   - Returns `200 { ok: true }`

2. **`GET /email/verify?token=<jwt>`** (unauthenticated, from email link)
   - Verifies the JWT signature and expiry
   - Re-checks email uniqueness (race condition guard)
   - Writes the email to `auth.users` via the **admin secret** (the only write path)
   - Returns an HTML confirmation page with a link back to the wiki

**Files:**

- `wiki/server/email.ts` — token creation/verification, SMTP sending, Hasura admin writes
- `wiki/server/main.ts` — routes + auth extraction for the email endpoints
- `wiki/src/components/auth/EmailCollectionDialog.tsx` — calls the server endpoint,
  shows "check your inbox" state instead of direct mutation

**Environment variables** (added to NixOS module + env file):

- `SMTP_HOST`, `SMTP_PORT`, `SMTP_FROM`, `SMTP_SECURE` — SMTP configuration
- `SMTP_USER`, `SMTP_PASS` — SMTP credentials (optional for local MTA)
- `EMAIL_SECRET` — signing secret (falls back to `HASURA_ADMIN_SECRET`)
- `PUBLIC_URL` — webhook base URL for building verification links
- `WIKI_URL` — frontend URL for redirect after verification

---

## Execution Order

| Step | Phase | Description | Depends On | Effort |
|---|---|---|---|---|
| 1 | 9.1–9.2 | Create `user_providers` table + Hasura tracking | — | Small |
| 2 | 1 | Build Deno auth webhook server | Step 1 | Medium |
| 3 | 8 | NixOS module + deploy webhook to `home` | Step 2 | Small |
| 4 | 1.5 | Switch Hasura to webhook mode | Step 3 | Small |
| — | — | **Checkpoint: existing NHost auth still works through webhook** | — | — |
| 5 | 2 | Create `core/atproto.ts` + `public/client-metadata.json` | — | Small |
| 6 | 3.1 | Create `AtprotoAuthProvider` + atproto hooks | Step 5 | Medium |
| 7 | 3.2 | Create unified `useAuth` facade hooks | Step 6 | Medium |
| 8 | 4 | Update `core/gql/index.ts` for dual-auth headers | Step 7 | Small |
| 9 | 5 | Update `App.tsx` — add providers + callback route | Steps 7–8 | Small |
| 10 | 6 | Update `AuthForm.tsx` — add Bluesky sign-in | Step 9 | Medium |
| — | — | **Checkpoint: atproto login works alongside NHost** | — | — |
| 11 | 7 | Migrate all component imports to `useAuth` facade | Step 7 | Medium (mechanical) |
| 12 | 9.3+3.3 | Server-side verified email flow + collection dialog | Step 2, 10 | Medium |
| 13 | 6.3 | Add account linking UI | Steps 10–11 | Medium |

---

## Post-Transition: NHost Auth Removal (Future)

Once all users have migrated to atproto, the NHost auth path can be removed:

1. Remove `@nhost/react` dependency and `NhostProvider` from `App.tsx`
2. Remove NHost JWT validation from the Deno webhook
3. Remove `core/nhost.tsx` auth usage (keep storage-only client)
4. Simplify `useAuth` hooks to atproto-only
5. Remove legacy routes (`/user/register`, `/user/reset-password`, etc.)
6. Remove `user_providers` entries with `provider='nhost'`
7. **Keep** `nhost.storage.*` calls in `useFile`, `useFiles`,
   `FileUploader`, and `DownloadButton` — storage stays on NHost

---

## Files Created/Modified Summary

### New Files

| File | Purpose |
|---|---|
| `wiki/server/main.ts` | Deno auth webhook entrypoint |
| `wiki/server/validate.ts` | `/validate` endpoint handler |
| `wiki/server/nhost.ts` | NHost JWT verification |
| `wiki/server/atproto.ts` | atproto DPoP token verification |
| `wiki/server/hasura.ts` | Hasura session variable builders |
| `wiki/server/users.ts` | User lookup/creation via Hasura admin |
| `wiki/server/deno.json` | Deno config for server |
| `wiki/core/atproto.ts` | Browser atproto OAuth client |
| `wiki/core/hooks/useAtproto.tsx` | atproto auth provider + hooks |
| `wiki/core/hooks/useAuth.ts` | Unified dual-auth facade hooks |
| `wiki/src/pages/auth/callback.tsx` | OAuth callback page |
| `wiki/src/components/auth/BlueskySignIn.tsx` | Bluesky handle input + sign-in button |
| `wiki/server/email.ts` | Server-side email verification (tokens, SMTP, DB writes) |
| `wiki/src/components/auth/EmailCollectionDialog.tsx` | Post-login email prompt for atproto users |
| `wiki/src/components/auth/AccountLinkDialog.tsx` | Link NHost ↔ atproto accounts |
| `wiki/server/wiki-auth.nix` | NixOS module for the webhook |

### Modified Files

| File | Change |
|---|---|
| `wiki/rsbuild.config.ts` | Add `pluginClientMetadata` — generates `client-metadata.json` at build time from `PUBLIC_SITE_URL` |
| `wiki/package.json` | Add `@atproto/oauth-client-browser`, `@atproto/api` |
| `wiki/core/gql/index.ts` | Dual-auth `getHeaders()` + DPoP support |
| `wiki/core/nhost.tsx` | No changes (kept for storage) |
| `wiki/src/App.tsx` | Add `AtprotoAuthProvider`, callback route |
| `wiki/src/components/auth/AuthForm.tsx` | Add Bluesky sign-in section |
| `wiki/src/components/layout/UserMenu.tsx` | Use `useSignOut` from `useAuth` |
| `config/nixos/home.nix` | Add `wiki-auth` service |
| 15+ component files | Swap `@nhost/react` imports → `useAuth` |

### Unchanged Files (NHost Storage)

| File | Reason |
|---|---|
| `wiki/core/hooks/useFile.ts` | Uses `nhost.storage` — stays |
| `wiki/core/hooks/useFiles.ts` | Uses `nhost.storage` — stays |
| `wiki/src/components/util/FileUploader.tsx` | Uses `nhost.storage.upload` — stays |
| `wiki/src/components/content/DownloadButton.tsx` | Uses `nhost.storage.getPresignedUrl` — stays |