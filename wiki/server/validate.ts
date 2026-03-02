/**
 * GET /validate — Hasura auth webhook handler.
 *
 * This is the main endpoint that Hasura calls on every GraphQL request.
 * It receives the original request headers and must return either:
 * - 200 with Hasura session variables (authenticated)
 * - 401 with an error (unauthenticated)
 *
 * The handler supports dual authentication:
 * 1. If a `DPoP` header is present → validate as atproto DPoP-bound token
 * 2. Otherwise → validate as NHost JWT
 * 3. If neither validates → return 401
 */

import { validateAtprotoToken } from "./atproto.ts";
import {
	type HasuraSessionVariables,
	unauthorizedResponse,
	userSession,
} from "./hasura.ts";
import { validateNhostJwt } from "./nhost.ts";
import { findOrCreateAtprotoUser } from "./users.ts";

/**
 * Extract the Bearer token from an Authorization header value.
 * Returns `null` if the header is missing or malformed.
 */
function extractBearerToken(authHeader: string | null): string | null {
	if (!authHeader) return null;
	const match = authHeader.match(/^Bearer\s+(.+)$/i);
	return match?.[1] ?? null;
}

/**
 * Fetch the Bluesky profile display name for a given DID.
 * This is a best-effort operation — returns undefined on failure.
 */
async function fetchBlueskyDisplayName(
	did: string,
): Promise<string | undefined> {
	try {
		const response = await fetch(
			`https://public.api.bsky.app/xrpc/app.bsky.actor.getProfile?actor=${encodeURIComponent(did)}`,
			{
				headers: { Accept: "application/json" },
				signal: AbortSignal.timeout(5000),
			},
		);

		if (!response.ok) return undefined;

		const profile = (await response.json()) as {
			displayName?: string;
			handle?: string;
		};

		return profile.displayName || undefined;
	} catch {
		return undefined;
	}
}

/**
 * Handle a validation request from Hasura.
 *
 * @param request - The incoming HTTP request (forwarded headers from the original GraphQL request).
 * @returns A Response with either session variables (200) or an error (401).
 */
export async function handleValidate(request: Request): Promise<Response> {
	const authHeader = request.headers.get("authorization");
	const dpopHeader = request.headers.get("dpop");
	const token = extractBearerToken(authHeader);

	// No token at all → unauthenticated
	if (!token) {
		return new Response(JSON.stringify(unauthorizedResponse()), {
			status: 401,
			headers: { "Content-Type": "application/json" },
		});
	}

	// Path 1: atproto DPoP-bound token
	if (dpopHeader) {
		return await handleAtprotoAuth(token, dpopHeader, request);
	}

	// Path 2: NHost JWT
	return await handleNhostAuth(token);
}

/**
 * Validate an atproto DPoP-bound access token and return Hasura session variables.
 */
async function handleAtprotoAuth(
	accessToken: string,
	dpopProof: string,
	_request: Request,
): Promise<Response> {
	// The DPoP proof references the resource server URL (i.e. the Hasura endpoint),
	// not our webhook URL. We need to reconstruct the original request URL.
	// Hasura forwards the original headers, including the Host.
	// For the webhook validation, we use the Hasura endpoint URL that the client
	// targeted, which we can derive from the forwarded headers or configuration.
	const hasuraEndpoint = Deno.env.get("HASURA_ENDPOINT");
	if (!hasuraEndpoint) {
		console.error("HASURA_ENDPOINT not set — cannot validate DPoP htu");
		return new Response(
			JSON.stringify(unauthorizedResponse("server configuration error")),
			{ status: 401, headers: { "Content-Type": "application/json" } },
		);
	}

	// The client sends the DPoP proof bound to the Hasura GraphQL endpoint
	// (POST to the graphql URL). We validate against that.
	const httpMethod = "POST";
	const httpUrl = hasuraEndpoint;

	const result = await validateAtprotoToken(
		accessToken,
		dpopProof,
		httpMethod,
		httpUrl,
	);

	if (!result) {
		return new Response(JSON.stringify(unauthorizedResponse()), {
			status: 401,
			headers: { "Content-Type": "application/json" },
		});
	}

	try {
		// Fetch display name from Bluesky profile (best-effort, non-blocking for auth)
		const displayName = await fetchBlueskyDisplayName(result.did);

		// Find or create the user, mapping the DID to a Hasura UUID
		const userId = await findOrCreateAtprotoUser(
			result.did,
			displayName,
			result.handle,
		);

		const session: HasuraSessionVariables = {
			...userSession(userId),
			"X-Hasura-Atproto-Did": result.did,
		};

		if (result.handle) {
			session["X-Hasura-Atproto-Handle"] = result.handle;
		}

		return new Response(JSON.stringify(session), {
			status: 200,
			headers: { "Content-Type": "application/json" },
		});
	} catch (err) {
		console.error("Failed to resolve atproto user:", err);
		return new Response(
			JSON.stringify(unauthorizedResponse("user resolution failed")),
			{ status: 401, headers: { "Content-Type": "application/json" } },
		);
	}
}

/**
 * Validate an NHost JWT and return Hasura session variables.
 */
async function handleNhostAuth(token: string): Promise<Response> {
	const result = await validateNhostJwt(token);

	if (!result) {
		return new Response(JSON.stringify(unauthorizedResponse()), {
			status: 401,
			headers: { "Content-Type": "application/json" },
		});
	}

	const session = userSession(result.userId, result.role);

	return new Response(JSON.stringify(session), {
		status: 200,
		headers: { "Content-Type": "application/json" },
	});
}
