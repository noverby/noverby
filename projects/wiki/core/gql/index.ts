import { createReactClient } from "@gqty/react";
import type { QueryFetcher } from "gqty";
import { Cache, createClient } from "gqty";
import { createClient as createSubscriptionsClient } from "graphql-ws";
import { nhost } from "nhost";
import type { GeneratedSchema } from "./schema.generated";
import { generatedSchema, scalarsEnumsHash } from "./schema.generated";

const getHeaders = (): Record<string, string> =>
	process.env.HASURA_GRAPHQL_ADMIN_SECRET
		? {
				"Content-Type": "application/json",
				"x-hasura-admin-secret": process.env.HASURA_GRAPHQL_ADMIN_SECRET,
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

const url = `https://${process.env.NEXT_PUBLIC_NHOST_SUBDOMAIN}.hasura.${process.env.NEXT_PUBLIC_NHOST_REGION}.nhost.run/v1/graphql`;

const queryFetcher: QueryFetcher = async (
	{ query, variables, operationName },
	fetchOptions,
) => {
	const headers = getHeaders();
	const response = await fetch(url, {
		method: "POST",
		headers,
		body: JSON.stringify({
			query,
			variables,
			operationName,
		}),
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
	connectionParams: () => ({
		headers: getHeaders(),
	}),
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
