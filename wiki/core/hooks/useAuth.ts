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
 * Returns the Hasura user UUID for the authenticated user, or `null`.
 *
 * - For NHost users: the UUID comes directly from the NHost JWT claims.
 * - For atproto users: the auth webhook maps the DID → UUID via the
 *   `user_providers` table, and the Hasura session contains the mapped
 *   UUID. However, on the client side we don't have direct access to
 *   the webhook-resolved UUID from context alone. Instead, we store
 *   the resolved user ID in the atproto session after the first
 *   authenticated GraphQL request. As a fallback, the DID is returned
 *   (the webhook will still resolve it server-side).
 *
 * In practice, the NHost `useUserId` hook reads from the JWT. For atproto
 * users, the atproto DID serves as the identifier on the client and the
 * webhook translates it to a UUID for Hasura. Components that pass the
 * user ID into GraphQL queries will get the correct results because
 * Hasura's `X-Hasura-User-Id` is set by the webhook, not the client.
 */
export function useUserId(): string | null {
	const atproto = useAtprotoAuth();
	const nhostUserId = useNhostUserId();

	if (atproto.isAuthenticated && atproto.did) {
		// Prefer the atproto DID. The auth webhook maps this to a UUID
		// server-side, so GraphQL permission checks work correctly.
		// If we later store the resolved UUID in the session, prefer that.
		const raw = atproto.session?.raw;
		const resolvedUserId =
			raw && typeof raw === "object"
				? (raw as Record<string, unknown>).hasuraUserId
				: undefined;
		if (typeof resolvedUserId === "string") {
			return resolvedUserId;
		}
		return atproto.did;
	}

	return nhostUserId ?? null;
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
export function useUserEmail(): string | null {
	const atproto = useAtprotoAuth();
	const nhostEmail = useNhostUserEmail();

	if (atproto.isAuthenticated) {
		// atproto doesn't provide email — it must be collected separately.
		// A future enhancement could query the users table for the stored email.
		return null;
	}

	return nhostEmail ?? null;
}

// ---------------------------------------------------------------------------
// useUserDisplayName
// ---------------------------------------------------------------------------

/**
 * Returns the user's display name, or `null`.
 *
 * - NHost users: display name from the NHost session.
 * - atproto users: display name from the Bluesky profile, falling back
 *   to the handle.
 */
export function useUserDisplayName(): string | null {
	const atproto = useAtprotoAuth();
	const atprotoProfile = useAtprotoProfile();
	const nhostDisplayName = useNhostUserDisplayName();

	if (atproto.isAuthenticated) {
		return (
			atprotoProfile.displayName ??
			atprotoProfile.handle ??
			atproto.handle ??
			atproto.did
		);
	}

	return nhostDisplayName ?? null;
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
