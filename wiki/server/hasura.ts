/**
 * Hasura session variable builders.
 *
 * Constructs the JSON objects that the Hasura auth webhook expects
 * as 200 responses (session variables) or 401 rejections.
 */

export interface HasuraSessionVariables {
	"X-Hasura-Role": string;
	"X-Hasura-User-Id": string;
	[key: string]: string;
}

/**
 * Build a successful session response for an authenticated user.
 */
export function userSession(
	userId: string,
	role = "user",
): HasuraSessionVariables {
	return {
		"X-Hasura-Role": role,
		"X-Hasura-User-Id": userId,
	};
}

/**
 * Build session variables for the public (unauthenticated) role.
 * Hasura uses this when the webhook returns 200 without a user id,
 * but typically we just return 401 for unauthenticated requests
 * and let Hasura fall back to HASURA_GRAPHQL_UNAUTHORIZED_ROLE.
 */
export function publicSession(): { "X-Hasura-Role": string } {
	return {
		"X-Hasura-Role": "public",
	};
}

/**
 * Build a 401 unauthorized response body.
 */
export function unauthorizedResponse(message = "unauthorized"): {
	error: string;
} {
	return { error: message };
}
