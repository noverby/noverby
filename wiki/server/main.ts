/**
 * Deno HTTP server entrypoint for the RadikalWiki auth webhook.
 *
 * This lightweight server acts as a Hasura authentication webhook.
 * Hasura calls GET /validate on every GraphQL request, forwarding
 * the original request headers. The server validates the token
 * (either NHost JWT or atproto DPoP) and returns Hasura session
 * variables.
 *
 * Environment variables:
 *   PORT              - Server port (default: 4180)
 *   NHOST_SUBDOMAIN   - NHost project subdomain
 *   NHOST_REGION      - NHost project region
 *   HASURA_ENDPOINT   - Hasura GraphQL endpoint URL
 *   HASURA_ADMIN_SECRET - Hasura admin secret for user management
 */

import { handleValidate } from "./validate.ts";

const PORT = Number(Deno.env.get("PORT") ?? "4180");
const NHOST_SUBDOMAIN = Deno.env.get("NHOST_SUBDOMAIN");
const NHOST_REGION = Deno.env.get("NHOST_REGION");

function getJwksUrl(): string {
	return `https://${NHOST_SUBDOMAIN}.auth.${NHOST_REGION}.nhost.run/v1/.well-known/jwks.json`;
}

/**
 * Shallow health check — confirms the server process is running.
 */
function handleHealthz(): Response {
	return new Response(JSON.stringify({ status: "ok" }), {
		status: 200,
		headers: { "Content-Type": "application/json" },
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
		"NHOST_SUBDOMAIN",
		"NHOST_REGION",
		"HASURA_ENDPOINT",
		"HASURA_ADMIN_SECRET",
	];
	const missingEnv = requiredEnv.filter((k) => !Deno.env.get(k));
	checks.env =
		missingEnv.length === 0
			? { ok: true }
			: { ok: false, detail: `missing: ${missingEnv.join(", ")}` };

	// 2. NHost JWKS reachability
	if (NHOST_SUBDOMAIN && NHOST_REGION) {
		try {
			const jwksUrl = getJwksUrl();
			const res = await fetch(jwksUrl, {
				signal: AbortSignal.timeout(5000),
			});
			if (!res.ok) {
				checks.jwks = {
					ok: false,
					detail: `HTTP ${res.status} from ${jwksUrl}`,
				};
			} else {
				const body = (await res.json()) as { keys?: unknown[] };
				if (Array.isArray(body.keys) && body.keys.length > 0) {
					checks.jwks = {
						ok: true,
						detail: `${body.keys.length} key(s) loaded`,
					};
				} else {
					checks.jwks = { ok: false, detail: "JWKS response has no keys" };
				}
			}
		} catch (err) {
			checks.jwks = {
				ok: false,
				detail: err instanceof Error ? err.message : String(err),
			};
		}
	} else {
		checks.jwks = { ok: false, detail: "NHOST_SUBDOMAIN/NHOST_REGION not set" };
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

function handleRequest(request: Request): Promise<Response> | Response {
	const url = new URL(request.url);

	// Shallow health check — is the process alive?
	if (url.pathname === "/healthz" || url.pathname === "/health") {
		return handleHealthz();
	}

	// Deep readiness check — are upstream deps reachable?
	if (url.pathname === "/healthz/ready" || url.pathname === "/ready") {
		return handleReady();
	}

	// Hasura auth webhook endpoint
	if (url.pathname === "/validate" && request.method === "GET") {
		return handleValidate(request);
	}

	// Not found
	return new Response(JSON.stringify({ error: "not found" }), {
		status: 404,
		headers: { "Content-Type": "application/json" },
	});
}

Deno.serve(
	{
		port: PORT,
		onListen({ hostname, port }) {
			const host = hostname === "0.0.0.0" ? "localhost" : hostname;
			console.log(`Wiki auth webhook listening on http://${host}:${port}`);
			console.log(`  GET /validate      — Hasura auth webhook`);
			console.log(`  GET /healthz       — Liveness check`);
			console.log(`  GET /healthz/ready — Readiness check (JWKS + Hasura)`);
		},
	},
	handleRequest,
);
