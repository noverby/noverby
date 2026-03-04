/**
 * Deno HTTP server entrypoint for the RadikalWiki auth webhook.
 *
 * This lightweight server acts as a Hasura authentication webhook.
 * Hasura calls GET /validate on every GraphQL request, forwarding
 * the original request headers. The server validates the token
 * (either NHost JWT or atproto DPoP) and returns Hasura session
 * variables.
 *
 * It also provides POST /token which exchanges an atproto DPoP-bound
 * access token for a Hasura-compatible HS256 JWT. The client uses
 * this JWT for all subsequent Hasura requests, avoiding the need
 * for a Hasura authHook (which breaks the NHost auth service).
 *
 * Environment variables:
 *   PORT              - Server port (default: 4180)
 *   NHOST_SUBDOMAIN   - NHost project subdomain
 *   NHOST_REGION      - NHost project region
 *   NHOST_JWT_SECRET  - HS256 shared secret for NHost JWT validation
 *   HASURA_ENDPOINT   - Hasura GraphQL endpoint URL
 *   HASURA_ADMIN_SECRET - Hasura admin secret for user management
 */

import * as jose from "jose";
import { validateAtprotoToken } from "./atproto.ts";
import { handleRequestVerification, handleVerifyEmail } from "./email.ts";
import { validateNhostJwt } from "./nhost.ts";
import { createUser, findUserByProvider, linkAtprotoToUser } from "./users.ts";
import { handleValidate } from "./validate.ts";

const HASURA_ENDPOINT = Deno.env.get("HASURA_ENDPOINT");

const PORT = Number(Deno.env.get("PORT") ?? "4180");

/**
 * Shallow health check — confirms the server process is running.
 */
function handleHealthz(request?: Request): Response {
	return new Response(JSON.stringify({ status: "ok" }), {
		status: 200,
		headers: { "Content-Type": "application/json", ...corsHeaders(request) },
	});
}

/**
 * Deep health / readiness check — verifies that upstream dependencies
 * (NHost JWKS, Hasura endpoint) are reachable. Use this to gate the
 * Hasura auth-hook switchover: don't change HASURA_GRAPHQL_AUTH_HOOK
 * until `GET /healthz/ready` returns 200.
 *
 * Checks performed:
 *  1. Required environment variables are set
 *  2. NHost JWKS endpoint is reachable and returns valid JSON with keys
 *  3. Hasura endpoint is reachable (unauthenticated introspection probe)
 */
async function handleReady(): Promise<Response> {
	const checks: Record<string, { ok: boolean; detail?: string }> = {};

	// 1. Environment variables
	const requiredEnv = [
		"NHOST_JWT_SECRET",
		"HASURA_ENDPOINT",
		"HASURA_ADMIN_SECRET",
	];
	const missingEnv = requiredEnv.filter((k) => !Deno.env.get(k));
	checks.env =
		missingEnv.length === 0
			? { ok: true }
			: { ok: false, detail: `missing: ${missingEnv.join(", ")}` };

	// 2. NHost JWT secret configured
	const jwtSecret = Deno.env.get("NHOST_JWT_SECRET");
	if (jwtSecret && jwtSecret.length > 0) {
		checks.nhostJwt = { ok: true, detail: "HS256 secret configured" };
	} else {
		checks.nhostJwt = { ok: false, detail: "NHOST_JWT_SECRET not set" };
	}

	// 3. Hasura endpoint reachability
	const hasuraEndpoint = Deno.env.get("HASURA_ENDPOINT");
	if (hasuraEndpoint) {
		try {
			const res = await fetch(hasuraEndpoint, {
				method: "POST",
				headers: { "Content-Type": "application/json" },
				body: JSON.stringify({ query: "{ __typename }" }),
				signal: AbortSignal.timeout(5000),
			});
			if (res.ok) {
				checks.hasura = { ok: true };
			} else {
				checks.hasura = {
					ok: false,
					detail: `HTTP ${res.status}`,
				};
			}
		} catch (err) {
			checks.hasura = {
				ok: false,
				detail: err instanceof Error ? err.message : String(err),
			};
		}
	} else {
		checks.hasura = { ok: false, detail: "HASURA_ENDPOINT not set" };
	}

	const allOk = Object.values(checks).every((c) => c.ok);
	const status = allOk ? 200 : 503;

	return new Response(
		JSON.stringify({ status: allOk ? "ready" : "not ready", checks }, null, 2),
		{ status, headers: { "Content-Type": "application/json" } },
	);
}

/**
 * Extract the authenticated user ID from an incoming request.
 *
 * This reuses the same token validation logic as /validate but returns
 * just the user ID string (or null) for use by non-Hasura endpoints
 * like the email verification flow.
 */
async function extractUserId(request: Request): Promise<string | null> {
	const authHeader = request.headers.get("authorization");
	const dpopHeader = request.headers.get("dpop");

	const token = authHeader?.match(/^Bearer\s+(.+)$/i)?.[1] ?? null;
	if (!token) return null;

	// Path 1: atproto DPoP token
	if (dpopHeader) {
		const hasuraEndpoint = Deno.env.get("HASURA_ENDPOINT");
		if (!hasuraEndpoint) return null;

		const result = await validateAtprotoToken(
			token,
			dpopHeader,
			"POST",
			hasuraEndpoint,
		);
		if (!result) return null;

		return await findUserByProvider("atproto", result.did);
	}

	// Path 2: NHost JWT
	const result = await validateNhostJwt(token);
	if (!result) return null;

	return result.userId;
}

/**
 * Add CORS headers for requests from the wiki frontend.
 */
function corsHeaders(request?: Request): Record<string, string> {
	const wikiUrl = Deno.env.get("WIKI_URL") ?? "https://radikal.wiki";
	const origin = request?.headers.get("Origin") ?? "";
	const allowedOrigin =
		origin === wikiUrl ||
		/^https?:\/\/(localhost|127\.0\.0\.1)(:\d+)?$/.test(origin)
			? origin
			: wikiUrl;
	return {
		"Access-Control-Allow-Origin": allowedOrigin,
		"Access-Control-Allow-Methods": "GET, POST, OPTIONS",
		"Access-Control-Allow-Headers":
			"Content-Type, Authorization, DPoP, X-NHost-Authorization",
		"Access-Control-Max-Age": "86400",
	};
}

function handleRequest(request: Request): Promise<Response> | Response {
	const url = new URL(request.url);

	// CORS preflight
	if (request.method === "OPTIONS") {
		return new Response(null, { status: 204, headers: corsHeaders(request) });
	}

	// Shallow health check — is the process alive?
	if (url.pathname === "/healthz" || url.pathname === "/health") {
		return handleHealthz(request);
	}

	// Deep readiness check — are upstream deps reachable?
	if (url.pathname === "/healthz/ready" || url.pathname === "/ready") {
		return handleReady();
	}

	// Hasura auth webhook endpoint
	if (url.pathname === "/validate" && request.method === "GET") {
		return handleValidate(request);
	}

	// Link atproto account to existing NHost user
	if (url.pathname === "/link-atproto" && request.method === "POST") {
		return handleLinkAtproto(request);
	}

	// Register a new user via atproto (explicit sign-up)
	if (url.pathname === "/register-atproto" && request.method === "POST") {
		return handleRegisterAtproto(request);
	}

	// Exchange atproto DPoP token for a Hasura-compatible JWT
	if (url.pathname === "/token" && request.method === "POST") {
		return handleToken(request);
	}

	// Email verification: request a verification email (authenticated)
	if (
		url.pathname === "/email/request-verification" &&
		request.method === "POST"
	) {
		return handleEmailRequest(request);
	}

	// Email verification: verify token from email link (unauthenticated)
	if (url.pathname === "/email/verify" && request.method === "GET") {
		const token = url.searchParams.get("token");
		return handleVerifyEmail(token);
	}

	// Not found
	return new Response(JSON.stringify({ error: "not found" }), {
		status: 404,
		headers: { "Content-Type": "application/json" },
	});
}

/**
 * Handle POST /token.
 *
 * Exchanges an atproto DPoP-bound access token for a Hasura-compatible
 * HS256 JWT. This avoids the need for a Hasura authHook (which breaks
 * the NHost auth service due to circular dependencies).
 *
 * The client authenticates with atproto DPoP headers, and receives back
 * a short-lived JWT signed with the same HS256 key that NHost uses.
 * Hasura validates this JWT natively — no webhook call required.
 *
 * Request headers:
 *   Authorization: DPoP <access_token>
 *   DPoP: <proof JWT>
 *
 * Response (200):
 *   { "accessToken": "<HS256 JWT>", "expiresIn": 900 }
 *
 * Response (401):
 *   { "error": "..." }
 */
async function handleToken(request: Request): Promise<Response> {
	const headers = {
		"Content-Type": "application/json",
		...corsHeaders(request),
	};

	const authHeader = request.headers.get("authorization");
	const dpopHeader = request.headers.get("dpop");
	const accessToken = authHeader?.match(/^DPoP\s+(.+)$/i)?.[1] ?? null;

	if (!accessToken || !dpopHeader) {
		return new Response(
			JSON.stringify({ error: "missing DPoP authorization" }),
			{ status: 401, headers },
		);
	}

	// Validate the atproto DPoP token
	const hasuraEndpoint = Deno.env.get("HASURA_ENDPOINT");
	if (!hasuraEndpoint) {
		return new Response(
			JSON.stringify({ error: "server configuration error" }),
			{ status: 500, headers },
		);
	}

	const publicUrl = Deno.env.get("PUBLIC_URL") ?? "https://auth.radikal.wiki";
	const origin = new URL(publicUrl).origin;
	const tokenUrl = `${origin}/token`;

	const atprotoResult = await validateAtprotoToken(
		accessToken,
		dpopHeader,
		"POST",
		tokenUrl,
	);

	if (!atprotoResult) {
		return new Response(JSON.stringify({ error: "invalid atproto token" }), {
			status: 401,
			headers,
		});
	}

	// Look up the user linked to this DID
	const userId = await findUserByProvider("atproto", atprotoResult.did);
	if (!userId) {
		return new Response(
			JSON.stringify({ error: "atproto_not_linked", did: atprotoResult.did }),
			{ status: 401, headers },
		);
	}

	// Sign a Hasura-compatible JWT with the NHost HS256 key
	const jwtSecret = Deno.env.get("NHOST_JWT_SECRET");
	if (!jwtSecret) {
		console.error("NHOST_JWT_SECRET not set — cannot issue JWT");
		return new Response(
			JSON.stringify({ error: "server configuration error" }),
			{ status: 500, headers },
		);
	}

	const expiresIn = 900; // 15 minutes, same as NHost default
	const secretKey = new TextEncoder().encode(jwtSecret);

	const jwt = await new jose.SignJWT({
		"https://hasura.io/jwt/claims": {
			"x-hasura-user-id": userId,
			"x-hasura-default-role": "user",
			"x-hasura-allowed-roles": ["user", "me"],
		},
		sub: userId,
		iss: "wiki-auth",
		iat: Math.floor(Date.now() / 1000),
	})
		.setProtectedHeader({ alg: "HS256", typ: "JWT" })
		.setIssuedAt()
		.setExpirationTime(`${expiresIn}s`)
		.sign(secretKey);

	return new Response(
		JSON.stringify({
			accessToken: jwt,
			expiresIn,
			did: atprotoResult.did,
			handle: atprotoResult.handle ?? null,
			userId,
		}),
		{ status: 200, headers },
	);
}

/**
 * Handle POST /link-atproto.
 *
 * Links an atproto DID to an existing NHost-authenticated user.
 * This avoids the race condition where the /validate webhook would
 * create a new user before the client-side linking mutation runs.
 *
 * The request is made using the atproto session's DPoP-bound fetch
 * (which automatically attaches `Authorization: DPoP <token>` and
 * `DPoP: <proof>` headers). The existing NHost user is identified
 * via a separate `X-NHost-Authorization: Bearer <jwt>` header.
 *
 * Headers:
 *   Authorization          — DPoP-bound atproto access token (set by dpopFetch)
 *   DPoP                   — DPoP proof JWT (set by dpopFetch)
 *   X-NHost-Authorization  — Bearer <NHost JWT> (proves ownership of existing account)
 */
async function handleLinkAtproto(request: Request): Promise<Response> {
	const headers = {
		"Content-Type": "application/json",
		...corsHeaders(request),
	};

	// Step 1: Authenticate the existing user via NHost JWT from custom header
	const nhostAuthHeader = request.headers.get("x-nhost-authorization");
	const nhostToken = nhostAuthHeader?.match(/^Bearer\s+(.+)$/i)?.[1] ?? null;
	if (!nhostToken) {
		return new Response(
			JSON.stringify({ error: "missing X-NHost-Authorization header" }),
			{ status: 401, headers },
		);
	}

	const nhostResult = await validateNhostJwt(nhostToken);
	if (!nhostResult) {
		return new Response(
			JSON.stringify({ error: "invalid or expired NHost token" }),
			{ status: 401, headers },
		);
	}

	const userId = nhostResult.userId;

	// Step 2: Validate atproto DPoP token from standard headers
	// The client's dpopFetch sets Authorization: DPoP <token> and DPoP: <proof>
	const authHeader = request.headers.get("authorization");
	const dpopHeader = request.headers.get("dpop");

	const atprotoToken = authHeader?.match(/^DPoP\s+(.+)$/i)?.[1] ?? null;
	if (!atprotoToken || !dpopHeader) {
		return new Response(
			JSON.stringify({
				error: "missing atproto DPoP Authorization/DPoP headers",
			}),
			{ status: 401, headers },
		);
	}

	if (!HASURA_ENDPOINT) {
		return new Response(
			JSON.stringify({ error: "server configuration error" }),
			{ status: 500, headers },
		);
	}

	// The DPoP proof is bound to the URL the client actually fetched,
	// which is the public URL of this endpoint (e.g.
	// https://wiki-auth.overby.me/link-atproto). Behind a reverse proxy
	// request.url shows the internal origin, so prefer PUBLIC_URL.
	const publicUrl = Deno.env.get("PUBLIC_URL");
	const origin = publicUrl?.replace(/\/+$/, "") ?? new URL(request.url).origin;
	const linkUrl = `${origin}/link-atproto`;

	const atprotoResult = await validateAtprotoToken(
		atprotoToken,
		dpopHeader,
		"POST",
		linkUrl,
	);

	if (!atprotoResult) {
		return new Response(JSON.stringify({ error: "invalid atproto token" }), {
			status: 401,
			headers,
		});
	}

	// Step 3: Check if this DID is already linked to a different user
	const existingUserId = await findUserByProvider("atproto", atprotoResult.did);
	if (existingUserId && existingUserId !== userId) {
		return new Response(
			JSON.stringify({
				error: "This Bluesky account is already linked to a different user",
			}),
			{ status: 409, headers },
		);
	}

	if (existingUserId === userId) {
		// Already linked to this user — idempotent success
		return new Response(
			JSON.stringify({
				ok: true,
				did: atprotoResult.did,
				handle: atprotoResult.handle ?? null,
				alreadyLinked: true,
			}),
			{ status: 200, headers },
		);
	}

	// Step 4: Link the atproto DID to the existing user
	try {
		await linkAtprotoToUser(userId, atprotoResult.did, atprotoResult.handle);
	} catch (err) {
		console.error("Failed to link atproto to user:", err);
		return new Response(JSON.stringify({ error: "failed to link account" }), {
			status: 500,
			headers,
		});
	}

	console.log(
		`Linked atproto DID ${atprotoResult.did} to NHost user ${userId} via /link-atproto`,
	);

	return new Response(
		JSON.stringify({
			ok: true,
			did: atprotoResult.did,
			handle: atprotoResult.handle ?? null,
			alreadyLinked: false,
		}),
		{ status: 200, headers },
	);
}

/**
 * Handle POST /register-atproto.
 *
 * Explicitly creates a new wiki user for an atproto DID.
 * This is the only path that creates users for Bluesky sign-ups —
 * the /validate webhook deliberately refuses unlinked DIDs instead
 * of silently creating ghost accounts.
 *
 * The request is made using the atproto session's DPoP-bound fetch
 * (which automatically attaches `Authorization: DPoP <token>` and
 * `DPoP: <proof>` headers).
 *
 * Headers:
 *   Authorization  — DPoP-bound atproto access token (set by dpopFetch)
 *   DPoP           — DPoP proof JWT (set by dpopFetch)
 */
async function handleRegisterAtproto(request: Request): Promise<Response> {
	const headers = {
		"Content-Type": "application/json",
		...corsHeaders(request),
	};

	// Step 1: Validate atproto DPoP token
	const authHeader = request.headers.get("authorization");
	const dpopHeader = request.headers.get("dpop");

	const atprotoToken = authHeader?.match(/^DPoP\s+(.+)$/i)?.[1] ?? null;
	if (!atprotoToken || !dpopHeader) {
		return new Response(
			JSON.stringify({
				error: "missing atproto DPoP Authorization/DPoP headers",
			}),
			{ status: 401, headers },
		);
	}

	if (!HASURA_ENDPOINT) {
		return new Response(
			JSON.stringify({ error: "server configuration error" }),
			{ status: 500, headers },
		);
	}

	const publicUrl = Deno.env.get("PUBLIC_URL");
	const origin = publicUrl?.replace(/\/+$/, "") ?? new URL(request.url).origin;
	const registerUrl = `${origin}/register-atproto`;

	const atprotoResult = await validateAtprotoToken(
		atprotoToken,
		dpopHeader,
		"POST",
		registerUrl,
	);

	if (!atprotoResult) {
		return new Response(JSON.stringify({ error: "invalid atproto token" }), {
			status: 401,
			headers,
		});
	}

	// Step 2: Check if this DID is already linked to a user
	const existingUserId = await findUserByProvider("atproto", atprotoResult.did);
	if (existingUserId) {
		// Already registered — idempotent success
		return new Response(
			JSON.stringify({
				ok: true,
				userId: existingUserId,
				did: atprotoResult.did,
				handle: atprotoResult.handle ?? null,
				alreadyRegistered: true,
			}),
			{ status: 200, headers },
		);
	}

	// Step 3: Fetch display name from Bluesky profile (best-effort)
	let displayName: string | undefined;
	try {
		const profileRes = await fetch(
			`https://public.api.bsky.app/xrpc/app.bsky.actor.getProfile?actor=${encodeURIComponent(atprotoResult.did)}`,
			{
				headers: { Accept: "application/json" },
				signal: AbortSignal.timeout(5000),
			},
		);
		if (profileRes.ok) {
			const profile = (await profileRes.json()) as {
				displayName?: string;
			};
			displayName = profile.displayName || undefined;
		}
	} catch {
		// Non-fatal — proceed without display name
	}

	// Step 4: Create the user
	try {
		const userId = await createUser(
			"atproto",
			atprotoResult.did,
			displayName,
			atprotoResult.handle,
		);

		console.log(
			`Registered new user ${userId} for atproto DID ${atprotoResult.did} via /register-atproto`,
		);

		return new Response(
			JSON.stringify({
				ok: true,
				userId,
				did: atprotoResult.did,
				handle: atprotoResult.handle ?? null,
				alreadyRegistered: false,
			}),
			{ status: 201, headers },
		);
	} catch (err) {
		console.error("Failed to register atproto user:", err);
		return new Response(JSON.stringify({ error: "failed to create user" }), {
			status: 500,
			headers,
		});
	}
}

/**
 * Handle POST /email/request-verification.
 * Authenticates the request, parses the body, then delegates to the email module.
 */
async function handleEmailRequest(request: Request): Promise<Response> {
	const userId = await extractUserId(request);
	if (!userId) {
		return new Response(JSON.stringify({ error: "unauthorized" }), {
			status: 401,
			headers: { "Content-Type": "application/json", ...corsHeaders(request) },
		});
	}

	let body: { email?: string };
	try {
		body = await request.json();
	} catch {
		return new Response(JSON.stringify({ error: "invalid JSON body" }), {
			status: 400,
			headers: { "Content-Type": "application/json", ...corsHeaders(request) },
		});
	}

	const response = await handleRequestVerification(userId, body);

	// Add CORS headers to the response from the email module
	const headers = new Headers(response.headers);
	for (const [k, v] of Object.entries(corsHeaders(request))) {
		headers.set(k, v);
	}

	return new Response(response.body, {
		status: response.status,
		headers,
	});
}

Deno.serve(
	{
		port: PORT,
		onListen({ hostname, port }) {
			const host = hostname === "0.0.0.0" ? "localhost" : hostname;
			console.log(`Wiki auth webhook listening on http://${host}:${port}`);
			console.log(`  GET  /validate                    — Hasura auth webhook`);
			console.log(
				`  POST /register-atproto            — Register new account via Bluesky`,
			);
			console.log(
				`  POST /link-atproto                — Link Bluesky to existing account`,
			);
			console.log(
				`  POST /email/request-verification  — Request email verification`,
			);
			console.log(
				`  GET  /email/verify?token=...      — Verify email from link`,
			);
			console.log(`  GET  /healthz                     — Liveness check`);
			console.log(`  GET  /healthz/ready               — Readiness check`);
		},
	},
	handleRequest,
);
