/**
 * Browser-side atproto OAuth client.
 *
 * This module initializes and exports the BrowserOAuthClient from
 * @atproto/oauth-client-browser. It handles:
 * - OAuth authorization flow (redirect to Bluesky authorization server)
 * - Session restoration on page load
 * - OAuth callback processing
 * - DPoP-bound token management
 *
 * The app must serve a `client-metadata.json` at the public URL
 * (e.g. https://radikal.wiki/client-metadata.json) — see
 * `public/client-metadata.json`.
 *
 * Usage:
 *   import { atprotoClient } from "core/atproto";
 *   // Start sign-in flow:
 *   await atprotoClient.signIn("alice.bsky.social");
 *   // Restore session on page load / handle callback:
 *   const result = await atprotoClient.init();
 *
 * Session holder:
 *   The module also exports a module-level session holder that bridges
 *   the React context (AtprotoAuthProvider) with non-React code like
 *   the GQL module. The provider calls `setAtprotoSession()` when the
 *   session changes, and `getAtprotoSession()` returns the current
 *   session (or null) for use in `core/gql/index.ts`.
 */

import {
	BrowserOAuthClient,
	type BrowserOAuthClientOptions,
} from "@atproto/oauth-client-browser";

// In local dev the atproto spec requires redirect_uri to use 127.0.0.1,
// so we must ensure the entire session runs on that origin.  If the user
// navigated to http://localhost:PORT, redirect to http://127.0.0.1:PORT
// so that IndexedDB (origin-scoped) is consistent before and after the
// OAuth callback.
if (
	typeof window !== "undefined" &&
	window.location.hostname === "localhost" &&
	window.location.protocol === "http:"
) {
	window.location.replace(
		`http://127.0.0.1${window.location.port ? `:${window.location.port}` : ""}${window.location.pathname}${window.location.search}${window.location.hash}`,
	);
}

const origin = typeof window !== "undefined" ? window.location.origin : "";

/**
 * The atproto OAuth client configured for RadikalWiki.
 *
 * We always provide explicit `clientMetadata` to avoid the library's
 * built-in `buildLoopbackClientId(window.location)` helper which
 * includes `location.pathname` in the loopback client ID.  The
 * atproto loopback spec only allows `http://localhost` (+ optional
 * query params), so any page path like `/user/login` causes:
 *
 *   TypeError: Invalid loopback client ID: Value must not contain a path component
 *
 * On **https** origins (production / staging) we use the standard
 * discoverable client metadata pointing at `client-metadata.json`.
 *
 * On **http** origins (local dev) we build a loopback client ID
 * ourselves: `http://localhost?redirect_uri=…&scope=…`.  The
 * redirect URI uses `127.0.0.1` (required by the spec) and the
 * `/auth/callback` path so the OAuth flow lands on the right page.
 *
 * `handleResolver` points to the default Bluesky AppView which can
 * resolve handles to DIDs for any PDS in the AT Protocol network.
 */
function buildClientMetadata(): BrowserOAuthClientOptions["clientMetadata"] {
	if (origin.startsWith("https:")) {
		return {
			client_id: `${origin}/client-metadata.json`,
			redirect_uris: [`${origin}/auth/callback`],
			scope: "atproto transition:generic",
			grant_types: ["authorization_code", "refresh_token"],
			response_types: ["code"],
			token_endpoint_auth_method: "none",
			application_type: "web",
			dpop_bound_access_tokens: true,
		};
	}

	// Local dev (http://localhost:PORT) — construct a valid loopback
	// client ID.  The spec requires the client_id to be
	// `http://localhost` with scope/redirect_uri in query params, and
	// the redirect URI host to be `127.0.0.1` (not `localhost`).
	const loc = typeof window !== "undefined" ? window.location : undefined;
	const port = loc?.port ? `:${loc.port}` : "";
	const redirectUri = `http://127.0.0.1${port}/auth/callback`;
	const scope = "atproto transition:generic";
	const clientId = `http://localhost?redirect_uri=${encodeURIComponent(redirectUri)}&scope=${encodeURIComponent(scope)}`;

	return {
		client_id: clientId,
		redirect_uris: [redirectUri],
		scope,
		grant_types: ["authorization_code", "refresh_token"],
		response_types: ["code"],
		token_endpoint_auth_method: "none",
		application_type: "web",
		dpop_bound_access_tokens: true,
	};
}

const atprotoClient = new BrowserOAuthClient({
	handleResolver: "https://bsky.social",
	clientMetadata: buildClientMetadata(),
});

// ---------------------------------------------------------------------------
// Module-level session holder
// ---------------------------------------------------------------------------
// Bridges the React AtprotoAuthProvider with the module-level GQL fetcher.
// The provider calls setAtprotoSession() on login/logout, and the GQL
// module calls getAtprotoSession() to decide how to authenticate requests.

/**
 * The raw session object from BrowserOAuthClient.
 *
 * When present, the session exposes a `fetchHandler` (or can be used
 * via the client's `fetch()` method) that automatically attaches
 * DPoP-bound Authorization headers to outgoing requests.
 */
// biome-ignore lint/suspicious/noExplicitAny: atproto session type is opaque — its shape varies across versions
let currentSession: any | null = null;

/**
 * Store the current atproto session (called by AtprotoAuthProvider).
 * Pass `null` to clear on sign-out.
 */
// biome-ignore lint/suspicious/noExplicitAny: atproto session type is opaque
export function setAtprotoSession(session: any | null): void {
	currentSession = session;
}

/**
 * Get the current atproto session, or `null` if not authenticated.
 * Used by `core/gql/index.ts` to build authenticated fetch requests.
 */
// biome-ignore lint/suspicious/noExplicitAny: atproto session type is opaque
export function getAtprotoSession(): any | null {
	return currentSession;
}

/**
 * Check whether an atproto session is currently active.
 * Lightweight check for the GQL headers logic — avoids importing
 * React context in a non-React module.
 */
export function isAtprotoAuthenticated(): boolean {
	return currentSession != null;
}

export { atprotoClient };
