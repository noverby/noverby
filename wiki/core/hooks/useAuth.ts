/**
 * Unified Auth Facade — Dual Auth Hooks.
 *
 * This module provides drop-in replacements for the `@nhost/react` hooks
 * used throughout the codebase. During the NHost → atproto migration,
 * both auth providers are active simultaneously. These hooks check the
 * atproto session first (preferred) and fall back to NHost.
 *
 * This is the key abstraction for the transition period. Components swap
 * their imports from `@nhost/react` to `core/hooks/useAuth` and everything
 * keeps working — regardless of which provider the user authenticated with.
 *
 * Exported hooks (matching @nhost/react API):
 *   useAuthenticationStatus()  → { isAuthenticated, isLoading }
 *   useAuthenticated()         → boolean
 *   useUserId()                → string | null   (always a Hasura UUID)
 *   useUserEmail()             → string | null
 *   useUserDisplayName()       → string | null
 *   useSignOut()               → () => Promise<void>
 */

import {
	useAuthenticationStatus as useNhostAuthenticationStatus,
	useUserDisplayName as useNhostUserDisplayName,
	useUserEmail as useNhostUserEmail,
	useUserId as useNhostUserId,
} from "@nhost/react";
import { nhost } from "nhost";
import {
	useAtprotoAuth,
	useAtprotoProfile,
	useAtprotoSignOut,
} from "./useAtproto";

// ---------------------------------------------------------------------------
// useAuthenticationStatus
// ---------------------------------------------------------------------------

/**
 * Returns the combined authentication status across both providers.
 *
 * - `isLoading` is `true` while either provider is still initializing.
 * - `isAuthenticated` is `true` if either provider has an active session
 *   (atproto takes precedence when both are active).
 */
export function useAuthenticationStatus(): {
	isAuthenticated: boolean;
	isLoading: boolean;
} {
	const nhost = useNhostAuthenticationStatus();
	const atproto = useAtprotoAuth();

	// We're loading if either provider is still initializing
	const isLoading = nhost.isLoading || atproto.isLoading;

	// Authenticated if either has a session
	const isAuthenticated = atproto.isAuthenticated || nhost.isAuthenticated;

	return { isAuthenticated, isLoading };
}

// ---------------------------------------------------------------------------
// useAuthenticated
// ---------------------------------------------------------------------------

/**
 * Simple boolean: is the user authenticated with any provider?
 */
export function useAuthenticated(): boolean {
	const { isAuthenticated } = useAuthenticationStatus();
	return isAuthenticated;
}

// ---------------------------------------------------------------------------
// useUserId
// ---------------------------------------------------------------------------

/**
 * Returns the Hasura user UUID for the authenticated user, or `undefined`.
 *
 * - For NHost users: the UUID comes directly from the NHost JWT claims.
 * - For atproto users: the auth webhook maps the DID → UUID via the
 *   `user_providers` table. On session activation the
 *   `AtprotoAuthProvider` makes a DPoP-authenticated GraphQL query
 *   (`{ users { id } }`) to fetch the resolved UUID and stores it as
 *   `hasuraUserId`. This hook returns that UUID so components can pass
 *   it into GraphQL queries that expect a `uuid!` variable.
 *
 *   Returns `undefined` while the UUID is still being resolved (briefly
 *   after login) or if the resolution fails.
 */
export function useUserId(): string | undefined {
	const atproto = useAtprotoAuth();
	const nhostUserId = useNhostUserId();

	if (atproto.isAuthenticated) {
		// The auth webhook maps the atproto DID → a Hasura UUID via the
		// `user_providers` table. After session activation the provider
		// fetches this UUID and stores it as `hasuraUserId`. We return
		// that so GraphQL queries that pass the user ID as a `uuid!`
		// variable get a real UUID, not a DID string.
		return atproto.hasuraUserId ?? undefined;
	}

	return nhostUserId ?? undefined;
}

// ---------------------------------------------------------------------------
// useUserEmail
// ---------------------------------------------------------------------------

/**
 * Returns the user's email address, or `null`.
 *
 * - NHost users: email comes from the NHost session.
 * - atproto users: email is `null` until the user provides it via the
 *   email collection dialog (Phase 3.3). Once set, it's stored in the
 *   `users.email` column and could be fetched from a GraphQL query,
 *   but for now we return `null` for atproto users on the client side.
 */
export function useUserEmail(): string | undefined {
	const atproto = useAtprotoAuth();
	const nhostEmail = useNhostUserEmail();

	if (atproto.isAuthenticated) {
		// atproto doesn't provide email — it must be collected separately.
		// A future enhancement could query the users table for the stored email.
		return undefined;
	}

	return nhostEmail ?? undefined;
}

// ---------------------------------------------------------------------------
// useUserDisplayName
// ---------------------------------------------------------------------------

/**
 * Returns the user's display name, or `undefined`.
 *
 * - NHost users: display name from the NHost session.
 * - atproto users: display name from the Bluesky profile, falling back
 *   to the handle.
 */
export function useUserDisplayName(): string | undefined {
	const atproto = useAtprotoAuth();
	const atprotoProfile = useAtprotoProfile();
	const nhostDisplayName = useNhostUserDisplayName();

	if (atproto.isAuthenticated) {
		return (
			atprotoProfile.displayName ??
			atprotoProfile.handle ??
			atproto.handle ??
			atproto.did ??
			undefined
		);
	}

	return nhostDisplayName ?? undefined;
}

// ---------------------------------------------------------------------------
// useSignOut
// ---------------------------------------------------------------------------

/**
 * Returns a function that signs out of whichever provider is active.
 * If both are active (e.g. during account linking), signs out of both.
 */
export function useSignOut(): () => Promise<void> {
	const atproto = useAtprotoAuth();
	const atprotoSignOut = useAtprotoSignOut();

	return async () => {
		const promises: Promise<void>[] = [];

		if (atproto.isAuthenticated) {
			promises.push(atprotoSignOut());
		}

		// Always attempt NHost sign-out to clear any lingering session.
		// nhost.auth.signOut() is safe to call even if not authenticated.
		promises.push(nhost.auth.signOut().then(() => undefined));

		await Promise.allSettled(promises);
	};
}
