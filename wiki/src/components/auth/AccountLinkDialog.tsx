/**
 * Account Link Dialog.
 *
 * Allows an existing NHost-authenticated user to link their Bluesky
 * account to their wiki account. Once linked, the user can log in
 * with either NHost email/password or Bluesky OAuth in the future,
 * and both will resolve to the same Hasura user UUID.
 *
 * Flow:
 * 1. User is logged in via NHost (existing account)
 * 2. They open this dialog and enter their Bluesky handle
 * 3. The dialog initiates the atproto OAuth flow
 * 4. On successful auth, the client calls the server-side `/link-atproto`
 *    endpoint which creates a `user_providers` row linking the atproto
 *    DID to their existing NHost user UUID
 * 5. Future atproto logins resolve to the same user
 *
 * The linking is done server-side (via the wiki-auth `/link-atproto`
 * endpoint) rather than via a direct Hasura mutation to avoid a race
 * condition: if the client made an atproto-authenticated Hasura request
 * first, the `/validate` webhook would call `findOrCreateAtprotoUser`
 * and create a *new* duplicate user before the linking mutation could
 * run. The server endpoint validates the NHost JWT (proving ownership
 * of the existing account) and the atproto DPoP token (proving
 * ownership of the Bluesky account) together, then links them
 * atomically.
 *
 * If the wiki-auth server is not running the dialog detects this and
 * shows a clear "server unavailable" message instead of letting the
 * user hit cryptic network errors.
 *
 * Phase 6.3 of the atproto auth migration plan.
 */

import { Link as LinkIcon, LinkOff } from "@mui/icons-material";
import {
	Alert,
	Button,
	CircularProgress,
	Dialog,
	DialogActions,
	DialogContent,
	DialogContentText,
	DialogTitle,
	InputAdornment,
	List,
	ListItem,
	ListItemIcon,
	ListItemText,
	TextField,
	Typography,
} from "@mui/material";

import { getAtprotoSession, isAtprotoAuthenticated } from "core/atproto";
import {
	useAtprotoAuth,
	useAtprotoSignIn,
	useAuthenticationStatus,
	useUserId,
} from "hooks";
import { nhost } from "nhost";
import {
	type ChangeEventHandler,
	type FormEvent,
	useCallback,
	useEffect,
	useRef,
	useState,
} from "react";
import { useTranslation } from "react-i18next";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

interface LinkedProvider {
	id: string;
	provider: string;
	provider_id: string;
	handle: string | null;
	created_at: string;
}

const HASURA_URL = `https://${process.env.PUBLIC_NHOST_SUBDOMAIN}.hasura.${process.env.PUBLIC_NHOST_REGION}.nhost.run/v1/graphql`;

const AUTH_SERVER_URL =
	process.env.PUBLIC_AUTH_SERVER_URL ?? "https://auth.radikal.wiki";

/**
 * Check whether a fetch error is a network-level failure (server
 * unreachable, DNS error, CORS block, timeout) as opposed to an
 * HTTP-level error (4xx/5xx with a JSON body).
 */
function isNetworkError(err: unknown): boolean {
	if (err instanceof TypeError) return true; // fetch throws TypeError on network failure
	if (err instanceof DOMException && err.name === "AbortError") return true;
	return false;
}

/**
 * Get the atproto DPoP-bound fetch function from the current session.
 * Returns `null` if no atproto session is active or no fetch method
 * is available.
 */
function getAtprotoDpopFetch(): typeof fetch | null {
	if (!isAtprotoAuthenticated()) return null;
	const session = getAtprotoSession();
	const dpopFetch: typeof fetch | undefined =
		session?.fetchHandler ?? session?.dpopFetch ?? session?.fetch;
	return typeof dpopFetch === "function" ? dpopFetch : null;
}

/**
 * Execute an authenticated GraphQL query against Hasura.
 * Uses the atproto DPoP fetch if available, otherwise falls back to
 * NHost JWT auth.
 */
async function gqlFetch<T = unknown>(
	query: string,
	variables: Record<string, unknown> = {},
): Promise<T> {
	const headers: Record<string, string> = {
		"Content-Type": "application/json",
	};

	let fetchFn: typeof fetch = fetch;

	const dpopFetch = getAtprotoDpopFetch();
	if (dpopFetch) {
		fetchFn = dpopFetch;
	} else if (nhost.auth.isAuthenticated()) {
		headers.authorization = `Bearer ${nhost.auth.getAccessToken()}`;
	}

	const response = await fetchFn(HASURA_URL, {
		method: "POST",
		headers,
		body: JSON.stringify({ query, variables }),
	});

	const json = (await response.json()) as {
		data?: T;
		errors?: { message: string }[];
	};

	if (json.errors?.length) {
		throw new Error(json.errors.map((e) => e.message).join(", "));
	}

	return json.data as T;
}

async function fetchUserProviders(userId: string): Promise<LinkedProvider[]> {
	const data = await gqlFetch<{
		user_providers: LinkedProvider[];
	}>(
		`query GetUserProviders($userId: uuid!) {
			user_providers(
				where: { user_id: { _eq: $userId } }
				order_by: { created_at: asc }
			) {
				id
				provider
				provider_id
				handle
				created_at
			}
		}`,
		{ userId },
	);
	return data.user_providers ?? [];
}

/**
 * Link an atproto DID to the current NHost user via the server-side
 * `/link-atproto` endpoint.
 *
 * The request is made using the atproto session's DPoP-bound fetch
 * (which attaches `Authorization: DPoP <token>` and `DPoP: <proof>`
 * headers automatically). The existing NHost user is identified via
 * a separate `X-NHost-Authorization` header carrying the NHost JWT.
 *
 * The server validates both credentials and creates the
 * `user_providers` row atomically, avoiding the race condition where
 * the `/validate` webhook would create a duplicate user.
 */
async function linkAtprotoViaServer(): Promise<{
	ok: boolean;
	did?: string;
	handle?: string | null;
	alreadyLinked?: boolean;
	error?: string;
}> {
	const dpopFetch = getAtprotoDpopFetch();
	if (!dpopFetch) {
		throw new Error("No atproto session available for linking");
	}

	const nhostToken = nhost.auth.getAccessToken();
	if (!nhostToken) {
		throw new Error("No NHost session available for linking");
	}

	const response = await dpopFetch(`${AUTH_SERVER_URL}/link-atproto`, {
		method: "POST",
		headers: {
			"Content-Type": "application/json",
			"X-NHost-Authorization": `Bearer ${nhostToken}`,
		},
		// Empty body — credentials are entirely in headers
		body: JSON.stringify({}),
	});

	const json = await response.json();

	if (!response.ok) {
		throw new Error(json.error ?? `Server returned ${response.status}`);
	}

	return json as {
		ok: boolean;
		did?: string;
		handle?: string | null;
		alreadyLinked?: boolean;
	};
}

async function deleteUserProvider(providerId: string): Promise<void> {
	await gqlFetch(
		`mutation UnlinkProvider($id: uuid!) {
			delete_user_providers_by_pk(id: $id) {
				id
			}
		}`,
		{ id: providerId },
	);
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

/**
 * Validate a Bluesky handle or DID.
 */
function validateHandle(input: string): string | null {
	const trimmed = input.trim().replace(/^@/, "");
	if (!trimmed) return null;
	if (trimmed.startsWith("did:")) return trimmed;
	if (trimmed.includes(".") && trimmed.length >= 3) return trimmed;
	return null;
}

export default function AccountLinkDialog({
	open,
	onClose,
}: {
	open: boolean;
	onClose: () => void;
}) {
	const { t } = useTranslation();
	const { isAuthenticated } = useAuthenticationStatus();
	const userId = useUserId();
	const atproto = useAtprotoAuth();
	const atprotoSignIn = useAtprotoSignIn();

	const [handle, setHandle] = useState("");
	const [error, setError] = useState("");
	const [loading, setLoading] = useState(false);
	const [linkedProviders, setLinkedProviders] = useState<LinkedProvider[]>([]);
	const [loadingProviders, setLoadingProviders] = useState(false);
	const [linkSuccess, setLinkSuccess] = useState(false);
	const [serverAvailable, setServerAvailable] = useState<boolean | null>(null);

	// Probe the wiki-auth server when the dialog opens so we can show
	// a clear message instead of letting the user hit cryptic errors.
	const healthChecked = useRef(false);
	useEffect(() => {
		if (!open || healthChecked.current) return;
		healthChecked.current = true;
		(async () => {
			try {
				const res = await fetch(`${AUTH_SERVER_URL}/healthz`, {
					signal: AbortSignal.timeout(5000),
				});
				setServerAvailable(res.ok);
			} catch {
				setServerAvailable(false);
			}
		})();
	}, [open]);

	/**
	 * Fetch the current user's linked providers from the user_providers table.
	 */
	const fetchLinkedProviders = useCallback(async () => {
		if (!userId) return;
		setLoadingProviders(true);
		try {
			const providers = await fetchUserProviders(userId);
			setLinkedProviders(providers);
		} catch (err) {
			console.error("Failed to fetch linked providers:", err);
		} finally {
			setLoadingProviders(false);
		}
	}, [userId]);

	// Fetch linked providers when the dialog opens
	useEffect(() => {
		if (open && userId) {
			fetchLinkedProviders();
		}
	}, [open, userId, fetchLinkedProviders]);

	// If an atproto session becomes active while the dialog is open,
	// that means the user just completed the OAuth flow for linking.
	// Call the server-side /link-atproto endpoint to create the link
	// atomically (avoiding the race condition with /validate).
	useEffect(() => {
		if (!open || !atproto.isAuthenticated || !atproto.did || !userId) return;

		// Must also have an active NHost session for the server to verify
		if (!nhost.auth.isAuthenticated()) return;

		// Check if this DID is already linked
		const alreadyLinked = linkedProviders.some(
			(p) => p.provider === "atproto" && p.provider_id === atproto.did,
		);
		if (alreadyLinked) return;

		// Link via the server-side endpoint
		(async () => {
			setLoading(true);
			try {
				const result = await linkAtprotoViaServer();
				if (result.ok) {
					setLinkSuccess(true);
					await fetchLinkedProviders();
				} else {
					setError(
						t("auth.linkError", "Could not link Bluesky account: {{message}}", {
							message: result.error ?? "unknown error",
						}),
					);
				}
			} catch (err) {
				console.error("Failed to link atproto account:", err);
				if (isNetworkError(err)) {
					setServerAvailable(false);
					setError(
						t(
							"auth.authServerUnavailable",
							"Wiki-auth-serveren er ikke tilgængelig. Bluesky-kontoforbindelse kræver at wiki-auth er sat op og kører.",
						),
					);
				} else {
					const message = err instanceof Error ? err.message : String(err);
					setError(
						t("auth.linkError", "Could not link Bluesky account: {{message}}", {
							message,
						}),
					);
				}
			} finally {
				setLoading(false);
			}
		})();
	}, [
		open,
		atproto.isAuthenticated,
		atproto.did,
		atproto.handle,
		userId,
		linkedProviders,
		fetchLinkedProviders,
		t,
	]);

	const onHandleChange: ChangeEventHandler<HTMLInputElement> = (e) => {
		setHandle(e.target.value.trim());
		if (error) setError("");
		if (linkSuccess) setLinkSuccess(false);
	};

	const handleLink = async (e?: FormEvent) => {
		e?.preventDefault();

		const validHandle = validateHandle(handle);
		if (!validHandle) {
			setError(
				t(
					"auth.invalidHandle",
					"Invalid Bluesky handle (e.g. alice.bsky.social)",
				),
			);
			return;
		}

		setLoading(true);
		setError("");

		try {
			// Initiate the atproto OAuth flow. This will redirect the user
			// away to the Bluesky authorization server. When they return
			// to the callback page and the session is restored, the
			// useEffect above will call linkAtprotoViaServer() to complete
			// the linking server-side.
			await atprotoSignIn(validHandle);
			// The page will redirect — we won't reach here normally
		} catch (err) {
			const message = err instanceof Error ? err.message : String(err);
			console.error("Bluesky link sign-in error:", err);
			setError(
				t(
					"auth.blueskySignInError",
					"Could not sign in with Bluesky: {{message}}",
					{ message },
				),
			);
			setLoading(false);
		}
	};

	const handleUnlink = async (providerId: string) => {
		try {
			await deleteUserProvider(providerId);
			await fetchLinkedProviders();
		} catch (err) {
			console.error("Failed to unlink provider:", err);
			setError(t("auth.unlinkError", "Could not unlink account"));
		}
	};

	const handleClose = () => {
		setHandle("");
		setError("");
		setLinkSuccess(false);
		onClose();
	};

	const hasAtprotoLink = linkedProviders.some((p) => p.provider === "atproto");

	if (!isAuthenticated) return null;

	return (
		<Dialog open={open} onClose={handleClose} maxWidth="sm" fullWidth>
			<DialogTitle>{t("auth.accountLinkTitle", "Link accounts")}</DialogTitle>
			<DialogContent>
				<DialogContentText sx={{ mb: 2 }}>
					{t(
						"auth.accountLinkDescription",
						"Link your Bluesky account so you can log in with either method.",
					)}
				</DialogContentText>

				{/* Current linked providers */}
				{loadingProviders ? (
					<CircularProgress
						size={24}
						sx={{ display: "block", mx: "auto", my: 2 }}
					/>
				) : linkedProviders.length > 0 ? (
					<>
						<Typography variant="subtitle2" sx={{ mt: 1, mb: 0.5 }}>
							{t("auth.linkedAccounts", "Linked accounts")}
						</Typography>
						<List dense>
							{linkedProviders.map((p) => (
								<ListItem
									key={p.id}
									secondaryAction={
										p.provider === "atproto" ? (
											<Button
												size="small"
												color="error"
												startIcon={<LinkOff />}
												onClick={() => handleUnlink(p.id)}
											>
												{t("auth.unlink", "Unlink")}
											</Button>
										) : null
									}
								>
									<ListItemIcon>
										<LinkIcon />
									</ListItemIcon>
									<ListItemText
										primary={
											p.provider === "atproto"
												? `Bluesky${p.handle ? ` (@${p.handle})` : ""}`
												: t("auth.emailPasswordProvider", "Email / password")
										}
										secondary={p.provider === "atproto" ? p.provider_id : null}
									/>
								</ListItem>
							))}
						</List>
					</>
				) : null}

				{/* Auth server unavailable warning */}
				{serverAvailable === false && (
					<Alert severity="warning" sx={{ mt: 2 }}>
						{t(
							"auth.authServerUnavailable",
							"Wiki-auth-serveren er ikke tilgængelig. Bluesky-kontoforbindelse kræver at wiki-auth er sat op og kører.",
						)}
					</Alert>
				)}

				{/* Link Bluesky form — only show if not already linked and server is available */}
				{!hasAtprotoLink && serverAvailable !== false && (
					<form onSubmit={handleLink}>
						<TextField
							fullWidth
							label={t("auth.blueskyHandle", "Bluesky handle")}
							placeholder="alice.bsky.social"
							value={handle}
							onChange={onHandleChange}
							error={!!error}
							helperText={error || undefined}
							disabled={loading}
							sx={{ mt: 2 }}
							slotProps={{
								input: {
									startAdornment: (
										<InputAdornment position="start">@</InputAdornment>
									),
								},
							}}
						/>

						<Button
							fullWidth
							type="submit"
							variant="contained"
							disabled={loading || !handle.trim()}
							sx={{
								mt: 2,
								bgcolor: "#0085ff",
								"&:hover": { bgcolor: "#0070dd" },
								textTransform: "none",
								fontWeight: 600,
							}}
						>
							{loading ? (
								<CircularProgress size={24} color="inherit" />
							) : (
								t("auth.linkBluesky", "Link Bluesky account")
							)}
						</Button>
					</form>
				)}

				{linkSuccess && (
					<Alert severity="success" sx={{ mt: 2 }}>
						{t(
							"auth.linkSuccess",
							"Bluesky account linked! You can now log in with either method.",
						)}
					</Alert>
				)}

				{error && !handle && (
					<Alert severity="error" sx={{ mt: 2 }}>
						{error}
					</Alert>
				)}
			</DialogContent>
			<DialogActions>
				<Button onClick={handleClose}>{t("common.close", "Close")}</Button>
			</DialogActions>
		</Dialog>
	);
}
