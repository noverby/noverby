import { createReactClient } from "@gqty/react";
import { isAtprotoAuthenticated } from "core/atproto";
import { getHasuraJwt } from "core/hooks/useAtproto";
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
 *   2. atproto session → use Hasura JWT obtained from wiki-auth /token exchange
 *   3. NHost JWT (legacy user auth)
 *   4. Public / unauthenticated
 */
const getHeaders = (): Record<string, string> => {
	if (process.env.HASURA_GRAPHQL_ADMIN_SECRET) {
		return {
			"Content-Type": "application/json",
			"x-hasura-admin-secret": process.env.HASURA_GRAPHQL_ADMIN_SECRET,
		};
	}

	// atproto users get a Hasura-compatible JWT from the /token endpoint
	const hasuraJwt = isAtprotoAuthenticated() ? getHasuraJwt() : null;
	if (hasuraJwt) {
		return {
			"Content-Type": "application/json",
			authorization: `Bearer ${hasuraJwt}`,
		};
	}

	if (nhost.auth.isAuthenticated()) {
		return {
			"Content-Type": "application/json",
			authorization: `Bearer ${nhost.auth.getAccessToken()}`,
		};
	}

	return {
		"Content-Type": "application/json",
		"x-hasura-role": "public",
	};
};

const url = `https://${process.env.PUBLIC_NHOST_SUBDOMAIN}.hasura.${process.env.PUBLIC_NHOST_REGION}.nhost.run/v1/graphql`;

/**
 * GQty query fetcher.
 *
 * All auth paths (admin secret, atproto JWT, NHost JWT, public) are
 * handled by `getHeaders()`. The atproto path uses a standard Bearer
 * JWT obtained from the wiki-auth /token endpoint — no DPoP headers
 * are sent to Hasura directly.
 */
const queryFetcher: QueryFetcher = async (
	{ query, variables, operationName },
	fetchOptions,
) => {
	const body = JSON.stringify({ query, variables, operationName });
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
		// For WebSocket subscriptions, use the Hasura JWT from /token exchange.
		if (isAtprotoAuthenticated()) {
			const jwt = getHasuraJwt();
			if (jwt) {
				return {
					headers: {
						authorization: `Bearer ${jwt}`,
					},
				};
			}
		}

		// Legacy: NHost JWT for subscriptions
		if (nhost.auth.isAuthenticated()) {
			const accessToken = nhost.auth.getAccessToken();
			if (accessToken) {
				return {
					headers: {
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
