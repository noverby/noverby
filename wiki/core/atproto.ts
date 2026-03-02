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
 */

import { BrowserOAuthClient } from "@atproto/oauth-client-browser";

const origin = typeof window !== "undefined" ? window.location.origin : "";

/**
 * The atproto OAuth client configured for RadikalWiki.
 *
 * `clientMetadata` mirrors the static `public/client-metadata.json` but
 * uses `window.location.origin` so it works across environments
 * (localhost dev, staging, production).
 *
 * `handleResolver` points to the default Bluesky AppView which can
 * resolve handles to DIDs for any PDS in the AT Protocol network.
 */
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
