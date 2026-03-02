import { createReactClient } from "@gqty/react";
import { getAtprotoSession, isAtprotoAuthenticated } from "core/atproto";
import type { QueryFetcher } from "gqty";
import { Cache, createClient } from "gqty";
import { createClient as createSubscriptionsClient } from "graphql-ws";
import { nhost } from "nhost";
import type { GeneratedSchema } from "./schema.generated";
import { generatedSchema, scalarsEnumsHash } from "./schema.generated";

/**
 * Build HTTP headers for Hasura GraphQL requests.
 *
 * Priority:
 *   1. Admin secret (dev / server-side)
 *   2. atproto DPoP session (preferred user auth)
 *   3. NHost JWT (legacy user auth)
 *   4. Public / unauthenticated
 *
 * When an atproto session is active we still return a minimal header set
 * here — the actual Authorization + DPoP headers are attached by the
 * session's `fetchHandler` in `queryFetcher`.  We include the Content-Type
 * so the caller can always spread these headers safely.
 */
const getHeaders = (): Record<string, string> =>
	process.env.HASURA_GRAPHQL_ADMIN_SECRET
		? {
				"Content-Type": "application/json",
				"x-hasura-admin-secret": process.env.HASURA_GRAPHQL_ADMIN_SECRET,
			}
		: isAtprotoAuthenticated()
			? {
					// The atproto DPoP fetch path handles Authorization + DPoP
					// headers itself — see queryFetcher below.
					"Content-Type": "application/json",
				}
			: nhost.auth.isAuthenticated()
				? {
						"Content-Type": "application/json",
						authorization: `Bearer ${nhost.auth.getAccessToken()}`,
					}
				: {
						"Content-Type": "application/json",
						"x-hasura-role": "public",
					};

const url = `https://${process.env.PUBLIC_NHOST_SUBDOMAIN}.hasura.${process.env.PUBLIC_NHOST_REGION}.nhost.run/v1/graphql`;

/**
 * GQty query fetcher with dual-auth support.
 *
 * When an atproto session is active, we use the session's `fetchHandler`
 * (exposed as `session.fetchHandler` or via the agent's `fetch`) which
 * automatically attaches the DPoP proof and Authorization header to every
 * outgoing request.  This is the recommended approach from the
 * @atproto/oauth-client-browser documentation — it handles token refresh,
 * DPoP nonce rotation, and proof generation transparently.
 *
 * For NHost / admin / public requests we fall back to plain `fetch` with
 * the headers built by `getHeaders()`.
 */
const queryFetcher: QueryFetcher = async (
	{ query, variables, operationName },
	fetchOptions,
) => {
	const body = JSON.stringify({ query, variables, operationName });

	// --- atproto DPoP path ---------------------------------------------------
	if (isAtprotoAuthenticated()) {
		const session = getAtprotoSession();

		// The session object from BrowserOAuthClient exposes a `fetchHandler`
		// that wraps the global fetch and adds Authorization + DPoP headers.
		// Some versions of the library also expose it as `dpopFetch`.
		// We must call it *on* the session object to preserve `this` (the
		// method internally accesses `this.getTokenSet()` etc.).
		const method: string | undefined = [
			"fetchHandler",
			"dpopFetch",
			"fetch",
		].find((m) => typeof session?.[m] === "function");

		if (method) {
			try {
				const response: Response = await session[method](url, {
					method: "POST",
					headers: { "Content-Type": "application/json" },
					body,
					mode: "cors" as RequestMode,
					...fetchOptions,
				});
				const json = await response.json();

				// If Hasura returned GraphQL-level auth errors (e.g. the auth
				// webhook isn't deployed yet and Hasura rejects the DPoP
				// Authorization header), fall through to public fetch instead
				// of surfacing a hard error.
				const gqlErrors: { message: string }[] | undefined = json.errors;
				const isAuthError = gqlErrors?.some(
					(e) =>
						/malformed authorization/i.test(e.message) ||
						/unauthorized/i.test(e.message),
				);
				if (!isAuthError) return json;

				console.warn(
					"atproto DPoP auth rejected by Hasura — falling back to public access. " +
						"Ensure the auth webhook is deployed and Hasura is configured with " +
						"HASURA_GRAPHQL_AUTH_HOOK.",
					gqlErrors,
				);
			} catch (err) {
				console.warn(
					"atproto DPoP fetch failed — falling back to public access:",
					err,
				);
			}
			// Fall through to unauthenticated fetch below.
		} else {
			console.warn(
				"atproto session active but no fetchHandler found — falling back to plain fetch",
			);
		}
	}

	// --- NHost JWT / admin / public path -------------------------------------
	const headers = getHeaders();
	const response = await fetch(url, {
		method: "POST",
		headers,
		body,
		mode: "cors",
		...fetchOptions,
	});

	const json = await response.json();
	return json;
};

const cache = new Cache(undefined, {
	staleWhileRevalidate: 5 * 60 * 1000,
	normalization: true,
});

const subscriptionsClient = createSubscriptionsClient({
	connectionParams: () => {
		// For WebSocket subscriptions, DPoP proofs aren't applicable (no
		// per-request HTTP headers).  atproto tokens are still sent as a
		// Bearer token in connectionParams.  The auth webhook on the server
		// side will validate whichever token type it receives.
		if (isAtprotoAuthenticated()) {
			const session = getAtprotoSession();
			// The access token may be exposed directly on the session object.
			const accessToken: string | undefined =
				session?.accessToken ??
				session?.tokenSet?.access_token ??
				session?.credentials?.accessToken;
			if (accessToken) {
				return {
					headers: {
						"Content-Type": "application/json",
						authorization: `Bearer ${accessToken}`,
					},
				};
			}
		}

		return {
			headers: getHeaders(),
		};
	},
	shouldRetry: (_errOrCloseEvent) => true,
	on: {
		error: (error) =>
			console.error(`GraphQL Subscription error: '${JSON.stringify(error)}'`),
	},
	url: () => {
		const urlClass = new URL(url);
		// eslint-disable-next-line functional/immutable-data
		urlClass.protocol = urlClass.protocol.replace("http", "ws");
		return urlClass.href;
	},
});

export const client = createClient<GeneratedSchema>({
	aliasLength: 10,
	schema: generatedSchema,
	scalars: scalarsEnumsHash,
	cache,
	fetchOptions: {
		fetcher: queryFetcher,
		subscriber: subscriptionsClient,
	},
});

// Core functions
export const { resolve, subscribe, schema } = client;

export const {
	graphql,
	useQuery,
	usePaginatedQuery,
	useTransactionQuery,
	useLazyQuery,
	useRefetch,
	useMutation,
	useMetaState,
	prepareReactRender,
	useHydrateCache,
	prepareQuery,
	useSubscription,
} = createReactClient<GeneratedSchema>(client, {
	defaults: {
		suspense: true,
		mutationSuspense: true,
		transactionQuerySuspense: true,
		staleWhileRevalidate: true,
	},
});

export * from "./schema.generated";

if (process.env.NODE_ENV === "development" && typeof window !== "undefined") {
	import("@gqty/logger").then(({ createLogger }) => {
		const logger = createLogger(client);
		logger.start();
	});
}
