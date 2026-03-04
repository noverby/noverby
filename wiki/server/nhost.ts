/**
 * NHost JWT validation using HS256 shared secret.
 *
 * Validates incoming NHost JWTs against the shared HMAC secret configured
 * in the NHost project (the same key listed under `hasura.jwtSecrets` in
 * `nhost.toml`).  This avoids a circular dependency: the previous JWKS
 * approach fetched keys from the NHost auth service, but when Hasura is
 * configured with an authHook the auth service itself needs Hasura to be
 * reachable — creating a chicken-and-egg problem during startup.
 *
 * The HS256 key is supplied via the `NHOST_JWT_SECRET` environment variable
 * (or falls back to the `HASURA_ADMIN_SECRET` environment file if not set
 * separately).  It must match the `key` value in `[[hasura.jwtSecrets]]`.
 */

import * as jose from "jose";

const NHOST_JWT_SECRET = Deno.env.get("NHOST_JWT_SECRET");

if (!NHOST_JWT_SECRET) {
	console.warn(
		"NHOST_JWT_SECRET not set — NHost JWT validation will fail. " +
			"Set it to the HS256 key from [[hasura.jwtSecrets]] in nhost.toml.",
	);
}

/**
 * Build the HS256 secret key for jose verification.
 * Cached so we only encode once.
 */
let secretKey: Uint8Array | null = null;

function getSecretKey(): Uint8Array {
	if (!secretKey) {
		if (!NHOST_JWT_SECRET) {
			throw new Error("NHOST_JWT_SECRET is not configured");
		}
		secretKey = new TextEncoder().encode(NHOST_JWT_SECRET);
	}
	return secretKey;
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
		const { payload } = await jose.jwtVerify(token, getSecretKey(), {
			algorithms: ["HS256"],
		});

		const hasuraClaims = payload["https://hasura.io/jwt/claims"] as
			| HasuraClaims
			| undefined;

		if (!hasuraClaims) {
			console.debug("NHost JWT missing Hasura claims namespace");
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
		} else if (err instanceof jose.errors.JWSSignatureVerificationFailed) {
			console.debug(
				"NHost JWT signature verification failed — token may not be NHost-issued",
			);
		} else {
			console.error("NHost JWT validation error:", err);
		}
		return null;
	}
}
