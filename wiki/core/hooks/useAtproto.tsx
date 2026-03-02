/**
 * atproto Auth Provider + Hooks.
 *
 * Wraps the atproto BrowserOAuthClient in a React context and exposes
 * hooks with a shape similar to @nhost/react, making it easy to swap
 * imports during the migration.
 *
 * The provider:
 * - Calls `atprotoClient.init()` on mount to restore sessions / handle callbacks
 * - Listens for session events (login, logout, token refresh)
 * - Fetches the Bluesky profile (displayName, avatar) on session start
 * - Stores everything in context state
 *
 * Exported hooks:
 *   useAtprotoAuth()        → { isAuthenticated, isLoading, did, handle, session }
 *   useAtprotoSignIn()      → (handle: string) => Promise<void>
 *   useAtprotoSignOut()     → () => Promise<void>
 *   useAtprotoProfile()     → { displayName, avatarUrl, handle }
 */

import { atprotoClient } from "core/atproto";
import type { ReactNode } from "react";
import {
	createContext,
	useCallback,
	useContext,
	useEffect,
	useMemo,
	useRef,
	useState,
} from "react";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/** The session object exposed by BrowserOAuthClient after a successful auth. */
interface AtprotoSession {
	did: string;
	handle?: string;
	/** The underlying session from the OAuth client (contains dpopFetch, etc.) */
	// biome-ignore lint/suspicious/noExplicitAny: atproto OAuth session type is opaque
	raw: any;
}

interface AtprotoProfile {
	displayName: string | null;
	avatarUrl: string | null;
	handle: string | null;
}

interface AtprotoAuthState {
	isAuthenticated: boolean;
	isLoading: boolean;
	did: string | null;
	handle: string | null;
	session: AtprotoSession | null;
	profile: AtprotoProfile;
}

interface AtprotoAuthContextValue extends AtprotoAuthState {
	signIn: (handle: string) => Promise<void>;
	signOut: () => Promise<void>;
}

// ---------------------------------------------------------------------------
// Context
// ---------------------------------------------------------------------------

const AtprotoAuthContext = createContext<AtprotoAuthContextValue | null>(null);

// ---------------------------------------------------------------------------
// Bluesky profile fetcher
// ---------------------------------------------------------------------------

async function fetchBlueskyProfile(
	did: string,
): Promise<{ displayName?: string; avatar?: string; handle?: string }> {
	try {
		const res = await fetch(
			`https://public.api.bsky.app/xrpc/app.bsky.actor.getProfile?actor=${encodeURIComponent(did)}`,
			{
				headers: { Accept: "application/json" },
				signal: AbortSignal.timeout(5000),
			},
		);
		if (!res.ok) return {};
		return (await res.json()) as {
			displayName?: string;
			avatar?: string;
			handle?: string;
		};
	} catch {
		return {};
	}
}

// ---------------------------------------------------------------------------
// Provider
// ---------------------------------------------------------------------------

export function AtprotoAuthProvider({ children }: { children: ReactNode }) {
	const [state, setState] = useState<AtprotoAuthState>({
		isAuthenticated: false,
		isLoading: true,
		did: null,
		handle: null,
		session: null,
		profile: { displayName: null, avatarUrl: null, handle: null },
	});

	// Guard against double-init in React StrictMode
	const initRef = useRef(false);

	/**
	 * Given a raw session result from the OAuth client, populate state
	 * and kick off a profile fetch.
	 */
	const activateSession = useCallback(
		// biome-ignore lint/suspicious/noExplicitAny: atproto OAuth session type is opaque
		async (rawSession: any) => {
			const did: string = rawSession?.did ?? rawSession?.sub;
			const handle: string | undefined = rawSession?.handle;

			if (!did) {
				console.error("atproto session has no DID");
				setState((s) => ({ ...s, isLoading: false }));
				return;
			}

			const session: AtprotoSession = { did, handle, raw: rawSession };

			setState((s) => ({
				...s,
				isAuthenticated: true,
				isLoading: false,
				did,
				handle: handle ?? null,
				session,
			}));

			// Fetch profile asynchronously — don't block auth
			const profile = await fetchBlueskyProfile(did);
			setState((s) => ({
				...s,
				profile: {
					displayName: profile.displayName ?? null,
					avatarUrl: profile.avatar ?? null,
					handle: profile.handle ?? handle ?? null,
				},
				// Also update the top-level handle if the profile resolved one
				handle: profile.handle ?? s.handle,
			}));
		},
		[],
	);

	/**
	 * Initialize: restore an existing session or process an OAuth callback.
	 */
	useEffect(() => {
		if (initRef.current) return;
		initRef.current = true;

		(async () => {
			try {
				const result = await atprotoClient.init();

				if (result?.session) {
					await activateSession(result.session);
				} else {
					setState((s) => ({ ...s, isLoading: false }));
				}
			} catch (err) {
				console.error("atproto init failed:", err);
				setState((s) => ({ ...s, isLoading: false }));
			}
		})();
	}, [activateSession]);

	// -----------------------------------------------------------------------
	// Actions
	// -----------------------------------------------------------------------

	const signIn = useCallback(async (handle: string) => {
		setState((s) => ({ ...s, isLoading: true }));
		try {
			// This triggers a redirect to the Bluesky authorization server.
			// The page will navigate away, so we don't need to update state here.
			await atprotoClient.signIn(handle);
		} catch (err) {
			console.error("atproto signIn failed:", err);
			setState((s) => ({ ...s, isLoading: false }));
			throw err;
		}
	}, []);

	const signOut = useCallback(async () => {
		try {
			// The BrowserOAuthClient may expose a signOut / revoke method.
			// If not, clearing local state is sufficient since tokens are
			// stored in the client's internal storage and init() won't
			// restore them after revocation.
			// biome-ignore lint/suspicious/noExplicitAny: accessing optional methods on opaque client type
			const client = atprotoClient as any;
			if (typeof client.signOut === "function") {
				await client.signOut();
			} else if (typeof client.revoke === "function") {
				await client.revoke(state.did ?? undefined);
			}
		} catch (err) {
			console.warn("atproto signOut/revoke error (non-fatal):", err);
		}

		setState({
			isAuthenticated: false,
			isLoading: false,
			did: null,
			handle: null,
			session: null,
			profile: { displayName: null, avatarUrl: null, handle: null },
		});
	}, [state.did]);

	// -----------------------------------------------------------------------
	// Memoised context value
	// -----------------------------------------------------------------------

	const value = useMemo<AtprotoAuthContextValue>(
		() => ({
			...state,
			signIn,
			signOut,
		}),
		[state, signIn, signOut],
	);

	return (
		<AtprotoAuthContext.Provider value={value}>
			{children}
		</AtprotoAuthContext.Provider>
	);
}

// ---------------------------------------------------------------------------
// Hooks
// ---------------------------------------------------------------------------

function useAtprotoContext(): AtprotoAuthContextValue {
	const ctx = useContext(AtprotoAuthContext);
	if (!ctx) {
		throw new Error(
			"useAtproto* hooks must be used within <AtprotoAuthProvider>",
		);
	}
	return ctx;
}

/**
 * Core atproto auth state.
 */
export function useAtprotoAuth() {
	const { isAuthenticated, isLoading, did, handle, session } =
		useAtprotoContext();
	return { isAuthenticated, isLoading, did, handle, session };
}

/**
 * Trigger the atproto OAuth sign-in flow.
 * Returns a function that accepts a Bluesky handle and initiates the redirect.
 */
export function useAtprotoSignIn() {
	const { signIn } = useAtprotoContext();
	return signIn;
}

/**
 * Sign out of atproto, clearing the local session.
 */
export function useAtprotoSignOut() {
	const { signOut } = useAtprotoContext();
	return signOut;
}

/**
 * The authenticated user's Bluesky profile information.
 */
export function useAtprotoProfile() {
	const { profile } = useAtprotoContext();
	return profile;
}
