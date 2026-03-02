/**
 * NHost JWT validation using JWKS.
 *
 * Fetches the JSON Web Key Set from the NHost auth service and verifies
 * incoming JWTs against it. Extracts Hasura claims from the token payload.
 */

import * as jose from "jose";

const NHOST_SUBDOMAIN = Deno.env.get("NHOST_SUBDOMAIN");
const NHOST_REGION = Deno.env.get("NHOST_REGION");

if (!NHOST_SUBDOMAIN || !NHOST_REGION) {
	console.warn(
		"NHOST_SUBDOMAIN or NHOST_REGION not set — NHost JWT validation will fail",
	);
}

function getJwksUrl(): string {
	return `https://${NHOST_SUBDOMAIN}.auth.${NHOST_REGION}.nhost.run/v1/.well-known/jwks.json`;
}

// Cache the JWKS fetcher so we don't re-create it on every request.
// jose.createRemoteJWKSet handles caching and rotation internally.
let jwks: ReturnType<typeof jose.createRemoteJWKSet> | null = null;

function getJwks(): ReturnType<typeof jose.createRemoteJWKSet> {
	if (!jwks) {
		jwks = jose.createRemoteJWKSet(new URL(getJwksUrl()));
	}
	return jwks;
}

/**
 * Hasura claims embedded in the NHost JWT under the
 * `https://hasura.io/jwt/claims` namespace.
 */
export interface HasuraClaims {
	"x-hasura-user-id": string;
	"x-hasura-default-role": string;
	"x-hasura-allowed-roles": string[];
	[key: string]: unknown;
}

export interface NhostValidationResult {
	userId: string;
	role: string;
	claims: HasuraClaims;
}

/**
 * Validate an NHost JWT and extract the Hasura session claims.
 *
 * @param token - The raw Bearer token (without the "Bearer " prefix).
 * @returns The extracted user ID and role, or `null` if validation fails.
 */
export async function validateNhostJwt(
	token: string,
): Promise<NhostValidationResult | null> {
	try {
		const { payload } = await jose.jwtVerify(token, getJwks(), {
			// NHost issues JWTs with RS256 by default
			algorithms: ["RS256"],
		});

		const hasuraClaims = payload["https://hasura.io/jwt/claims"] as
			| HasuraClaims
			| undefined;

		if (!hasuraClaims) {
			console.error("NHost JWT missing Hasura claims namespace");
			return null;
		}

		const userId = hasuraClaims["x-hasura-user-id"];
		const role = hasuraClaims["x-hasura-default-role"];

		if (!userId) {
			console.error("NHost JWT missing x-hasura-user-id claim");
			return null;
		}

		return {
			userId,
			role: role ?? "user",
			claims: hasuraClaims,
		};
	} catch (err) {
		if (err instanceof jose.errors.JWTExpired) {
			console.debug("NHost JWT expired");
		} else if (err instanceof jose.errors.JWKSNoMatchingKey) {
			console.debug(
				"NHost JWT key not found in JWKS — token may not be NHost-issued",
			);
		} else {
			console.error("NHost JWT validation error:", err);
		}
		return null;
	}
}
