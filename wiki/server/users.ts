/**
 * User lookup and creation via the Hasura admin API.
 *
 * Maps atproto DIDs to Hasura user UUIDs using the `user_providers` table.
 * Creates new users automatically on first atproto login.
 */

const HASURA_ENDPOINT = Deno.env.get("HASURA_ENDPOINT");
const HASURA_ADMIN_SECRET = Deno.env.get("HASURA_ADMIN_SECRET");

if (!HASURA_ENDPOINT || !HASURA_ADMIN_SECRET) {
	console.warn(
		"HASURA_ENDPOINT or HASURA_ADMIN_SECRET not set — user operations will fail",
	);
}

/**
 * Execute a GraphQL query against the Hasura admin API.
 */
async function hasuraAdmin<T = unknown>(
	query: string,
	variables: Record<string, unknown> = {},
): Promise<T> {
	const response = await fetch(HASURA_ENDPOINT!, {
		method: "POST",
		headers: {
			"Content-Type": "application/json",
			"x-hasura-admin-secret": HASURA_ADMIN_SECRET!,
		},
		body: JSON.stringify({ query, variables }),
	});

	if (!response.ok) {
		const text = await response.text();
		throw new Error(
			`Hasura admin request failed (${response.status}): ${text}`,
		);
	}

	const json = (await response.json()) as {
		data?: T;
		errors?: { message: string }[];
	};

	if (json.errors?.length) {
		throw new Error(
			`Hasura GraphQL errors: ${json.errors.map((e) => e.message).join(", ")}`,
		);
	}

	return json.data as T;
}

/**
 * Look up a user by their provider and provider ID (e.g. atproto DID).
 * Returns the mapped Hasura user UUID, or `null` if not found.
 */
export async function findUserByProvider(
	provider: string,
	providerId: string,
): Promise<string | null> {
	const query = `
		query FindUserByProvider($provider: String!, $providerId: String!) {
			user_providers(
				where: {
					provider: { _eq: $provider }
					provider_id: { _eq: $providerId }
				}
				limit: 1
			) {
				user_id
			}
		}
	`;

	const data = await hasuraAdmin<{
		user_providers: { user_id: string }[];
	}>(query, { provider, providerId });

	if (data.user_providers.length > 0) {
		return data.user_providers[0].user_id;
	}

	return null;
}

/**
 * Create a new user in auth.users and link them via user_providers.
 *
 * This is used for first-time atproto logins where no existing user
 * mapping exists. The user gets a fresh UUID in auth.users and a
 * corresponding row in user_providers.
 *
 * @param provider - The auth provider name (e.g. "atproto").
 * @param providerId - The provider-specific user ID (e.g. a DID).
 * @param displayName - The user's display name (e.g. from Bluesky profile).
 * @param handle - The user's handle (e.g. "@alice.bsky.social").
 * @returns The newly created Hasura user UUID.
 */
export async function createUser(
	provider: string,
	providerId: string,
	displayName?: string,
	handle?: string,
): Promise<string> {
	// Step 1: Create a user in auth.users.
	// NHost's auth.users table has specific columns; we insert with
	// minimal data. The user starts with no email (they can provide one later).
	const insertUserQuery = `
		mutation CreateUser($displayName: String, $metadata: jsonb) {
			insert_auth_users_one(object: {
				display_name: $displayName
				default_role: "user"
				roles: {
					data: [{ role: "user" }]
				}
				is_anonymous: false
				metadata: $metadata
			}) {
				id
			}
		}
	`;

	// If the above mutation doesn't work with NHost's auth schema
	// (which may have constraints), fall back to a simpler insert.
	// We try the full insert first.
	let userId: string;

	try {
		const userData = await hasuraAdmin<{
			insert_auth_users_one: { id: string };
		}>(insertUserQuery, {
			displayName: displayName ?? handle ?? providerId,
			metadata: {
				provider,
				providerId,
				handle: handle ?? null,
			},
		});
		userId = userData.insert_auth_users_one.id;
	} catch (err) {
		// Fallback: try a simpler insert without the roles relation
		console.warn("Full user insert failed, trying simpler insert:", err);
		const simpleInsertQuery = `
			mutation CreateUserSimple($displayName: String, $metadata: jsonb) {
				insert_auth_users_one(object: {
					display_name: $displayName
					default_role: "user"
					is_anonymous: false
					metadata: $metadata
				}) {
					id
				}
			}
		`;

		const userData = await hasuraAdmin<{
			insert_auth_users_one: { id: string };
		}>(simpleInsertQuery, {
			displayName: displayName ?? handle ?? providerId,
			metadata: {
				provider,
				providerId,
				handle: handle ?? null,
			},
		});
		userId = userData.insert_auth_users_one.id;
	}

	// Step 2: Create the user_providers link
	const insertProviderQuery = `
		mutation LinkProvider($userId: uuid!, $provider: String!, $providerId: String!, $handle: String) {
			insert_user_providers_one(object: {
				user_id: $userId
				provider: $provider
				provider_id: $providerId
				handle: $handle
			}) {
				id
			}
		}
	`;

	await hasuraAdmin(insertProviderQuery, {
		userId,
		provider,
		providerId,
		handle: handle ?? null,
	});

	console.log(
		`Created new user ${userId} for ${provider}:${providerId}${handle ? ` (${handle})` : ""}`,
	);

	return userId;
}

/**
 * Find or create a user for a given atproto DID.
 *
 * This is the main entry point used by the auth webhook when an atproto
 * token is validated. It looks up the DID in user_providers and creates
 * a new user if one doesn't exist.
 *
 * @param did - The atproto DID (e.g. "did:plc:abc123").
 * @param displayName - Optional display name from the Bluesky profile.
 * @param handle - Optional handle from the atproto token.
 * @returns The Hasura user UUID.
 */
export async function findOrCreateAtprotoUser(
	did: string,
	displayName?: string,
	handle?: string,
): Promise<string> {
	// Try to find an existing user
	const existingUserId = await findUserByProvider("atproto", did);
	if (existingUserId) {
		return existingUserId;
	}

	// Create a new user
	return await createUser("atproto", did, displayName, handle);
}

/**
 * Link an atproto DID to an existing user (for account linking).
 *
 * Used when an already-authenticated NHost user wants to link their
 * Bluesky account to their existing wiki account.
 *
 * @param userId - The existing Hasura user UUID.
 * @param did - The atproto DID to link.
 * @param handle - Optional Bluesky handle.
 */
export async function linkAtprotoToUser(
	userId: string,
	did: string,
	handle?: string,
): Promise<void> {
	const query = `
		mutation LinkAtproto($userId: uuid!, $did: String!, $handle: String) {
			insert_user_providers_one(object: {
				user_id: $userId
				provider: "atproto"
				provider_id: $did
				handle: $handle
			}) {
				id
			}
		}
	`;

	await hasuraAdmin(query, { userId, did, handle: handle ?? null });

	console.log(`Linked atproto DID ${did} to existing user ${userId}`);
}
