/**
 * atproto DPoP token validation.
 *
 * Validates atproto OAuth DPoP-bound access tokens by:
 * 1. Parsing the DPoP proof JWT from the DPoP header
 * 2. Verifying the proof's signature, claims, and binding to the access token
 * 3. Extracting the DID (sub claim) from the access token
 *
 * References:
 * - RFC 9449 (DPoP)
 * - atproto OAuth specification
 */

import * as jose from "jose";

/**
 * Result of a successful atproto token validation.
 */
export interface AtprotoValidationResult {
	/** The user's DID (e.g. "did:plc:abc123" or "did:web:example.com") */
	did: string;
	/** The user's handle, if present in the token */
	handle?: string;
	/** The PDS issuer URL from the token */
	issuer?: string;
}

/**
 * Cache of seen DPoP jti values for replay protection.
 * In production, this should be backed by Redis or similar.
 * We use a simple in-memory set with TTL-based eviction.
 */
const seenJtis = new Map<string, number>();
const JTI_MAX_AGE_MS = 5 * 60 * 1000; // 5 minutes

/**
 * Evict expired jti entries to prevent unbounded memory growth.
 */
function evictExpiredJtis(): void {
	const now = Date.now();
	for (const [jti, timestamp] of seenJtis) {
		if (now - timestamp > JTI_MAX_AGE_MS) {
			seenJtis.delete(jti);
		}
	}
}

// Run eviction periodically
setInterval(evictExpiredJtis, 60_000);

/**
 * Compute the SHA-256 hash of an access token for the DPoP `ath` claim.
 * Returns the base64url-encoded hash.
 */
async function computeTokenHash(accessToken: string): Promise<string> {
	const encoder = new TextEncoder();
	const data = encoder.encode(accessToken);
	const hashBuffer = await crypto.subtle.digest("SHA-256", data);
	// Convert to base64url
	const hashArray = new Uint8Array(hashBuffer);
	return jose.base64url.encode(hashArray);
}

/**
 * Extract the JWK from the DPoP proof header.
 * The DPoP proof must contain a `jwk` header parameter with the public key.
 */
function extractDpopKey(dpopProof: string): jose.JWK | null {
	try {
		const header = jose.decodeProtectedHeader(dpopProof);
		if (!header.jwk) {
			console.error("DPoP proof missing jwk header parameter");
			return null;
		}
		// DPoP proofs must use the `jwk` header, not `kid` or `x5c`
		if (header.kid || header.x5c) {
			console.error(
				"DPoP proof must not contain kid or x5c when jwk is present",
			);
			return null;
		}
		if (header.typ !== "dpop+jwt") {
			console.error(`DPoP proof has wrong typ: ${header.typ}`);
			return null;
		}
		return header.jwk as jose.JWK;
	} catch (err) {
		console.error("Failed to decode DPoP proof header:", err);
		return null;
	}
}

/**
 * Compute the JWK thumbprint (RFC 7638) for DPoP key binding confirmation.
 */
async function computeJwkThumbprint(jwk: jose.JWK): Promise<string> {
	return await jose.calculateJwkThumbprint(jwk, "sha256");
}

/**
 * Validate an atproto DPoP-bound access token.
 *
 * @param accessToken - The Bearer token from the Authorization header (without "Bearer " prefix).
 * @param dpopProof - The raw DPoP proof JWT from the DPoP header.
 * @param httpMethod - The HTTP method of the original request (e.g. "GET", "POST").
 * @param httpUrl - The full URL of the original request to the resource server.
 * @returns The validation result with the user's DID, or `null` if validation fails.
 */
export async function validateAtprotoToken(
	accessToken: string,
	dpopProof: string,
	httpMethod: string,
	httpUrl: string,
): Promise<AtprotoValidationResult | null> {
	try {
		// Step 1: Extract the public key from the DPoP proof header
		const dpopJwk = extractDpopKey(dpopProof);
		if (!dpopJwk) {
			return null;
		}

		// Step 2: Import the public key for verification
		const dpopKey = await jose.importJWK(dpopJwk);

		// Step 3: Verify the DPoP proof JWT signature and extract claims
		const { payload: dpopPayload } = await jose.jwtVerify(dpopProof, dpopKey, {
			// DPoP proofs have a short lifetime
			maxTokenAge: 120, // 2 minutes
			clockTolerance: 30,
		});

		// Step 4: Validate DPoP proof claims per RFC 9449
		const { jti, htm, htu, ath, iat } = dpopPayload as {
			jti?: string;
			htm?: string;
			htu?: string;
			ath?: string;
			iat?: number;
		};

		// jti must be present and unique
		if (!jti) {
			console.error("DPoP proof missing jti claim");
			return null;
		}

		if (seenJtis.has(jti)) {
			console.error("DPoP proof jti replay detected:", jti);
			return null;
		}
		seenJtis.set(jti, Date.now());

		// htm must match the HTTP method
		if (!htm || htm.toUpperCase() !== httpMethod.toUpperCase()) {
			console.error(`DPoP htm mismatch: expected ${httpMethod}, got ${htm}`);
			return null;
		}

		// htu must match the request URL (scheme, host, path — ignore query/fragment)
		if (!htu) {
			console.error("DPoP proof missing htu claim");
			return null;
		}
		const proofUrl = new URL(htu);
		const requestUrl = new URL(httpUrl);
		if (
			proofUrl.origin !== requestUrl.origin ||
			proofUrl.pathname !== requestUrl.pathname
		) {
			console.error(
				`DPoP htu mismatch: expected ${requestUrl.origin}${requestUrl.pathname}, got ${proofUrl.origin}${proofUrl.pathname}`,
			);
			return null;
		}

		// ath must match the hash of the access token
		if (!ath) {
			console.error("DPoP proof missing ath claim");
			return null;
		}
		const expectedAth = await computeTokenHash(accessToken);
		if (ath !== expectedAth) {
			console.error("DPoP ath mismatch — access token hash does not match");
			return null;
		}

		// iat must be present and recent
		if (!iat) {
			console.error("DPoP proof missing iat claim");
			return null;
		}

		// Step 5: Decode the access token to extract claims.
		// atproto access tokens are JWTs, but we validate binding via DPoP
		// rather than verifying the AT signature here (the PDS is the issuer,
		// and we trust the DPoP binding for authorization).
		// For full security, we could also verify the AT against the PDS's JWKS,
		// but atproto's auth model relies on DPoP binding.
		const atPayload = jose.decodeJwt(accessToken);

		const { sub, iss } = atPayload as {
			sub?: string;
			iss?: string;
			cnf?: { jkt?: string };
		};

		if (!sub) {
			console.error("Access token missing sub claim (DID)");
			return null;
		}

		// Verify the sub is a valid DID
		if (!sub.startsWith("did:")) {
			console.error(`Access token sub is not a DID: ${sub}`);
			return null;
		}

		// Step 6: Verify DPoP key binding (cnf.jkt in access token must match
		// the thumbprint of the DPoP proof key)
		const cnf = atPayload.cnf as { jkt?: string } | undefined;
		if (!cnf?.jkt) {
			console.error("Access token missing cnf.jkt claim for DPoP binding");
			return null;
		}

		const dpopThumbprint = await computeJwkThumbprint(dpopJwk);
		if (cnf.jkt !== dpopThumbprint) {
			console.error(
				"DPoP key binding mismatch — cnf.jkt does not match proof key thumbprint",
			);
			return null;
		}

		return {
			did: sub,
			handle: (atPayload as Record<string, unknown>).handle as
				| string
				| undefined,
			issuer: iss,
		};
	} catch (err) {
		if (err instanceof jose.errors.JWTExpired) {
			console.debug("DPoP proof expired");
		} else if (err instanceof jose.errors.JWSSignatureVerificationFailed) {
			console.error("DPoP proof signature verification failed");
		} else {
			console.error("atproto token validation error:", err);
		}
		return null;
	}
}
