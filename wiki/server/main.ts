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

function handleRequest(request: Request): Promise<Response> | Response {
	const url = new URL(request.url);

	// Health check
	if (url.pathname === "/healthz" || url.pathname === "/health") {
		return new Response(JSON.stringify({ status: "ok" }), {
			status: 200,
			headers: { "Content-Type": "application/json" },
		});
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
			console.log(`  POST /validate  — Hasura auth webhook`);
			console.log(`  GET  /healthz   — Health check`);
		},
	},
	handleRequest,
);
