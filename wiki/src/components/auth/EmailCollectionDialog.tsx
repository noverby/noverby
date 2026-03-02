/**
 * Email Collection Dialog.
 *
 * Shown to atproto users after their first login when they have no email
 * on record. The email is used for wiki invitations and notifications.
 *
 * Security: The `auth.users.email` column is used as an identity key for
 * the invite/membership system. Allowing users to self-set their email
 * without verification would let them claim other users' pending invites.
 * Instead of a direct Hasura mutation, this dialog sends the email to the
 * auth webhook server, which sends a verification link. The email is only
 * written to the database after the user clicks the link.
 *
 * Flow:
 *  1. User enters their email address in the dialog
 *  2. Frontend POSTs to /email/request-verification on the auth webhook
 *  3. Server sends a verification email with a signed token link
 *  4. Dialog shows "check your inbox" confirmation
 *  5. User clicks the link → server verifies token → writes email to DB
 *
 * The dialog is skippable — the user can dismiss it and provide their
 * email later from settings. It stores a flag in localStorage so it
 * doesn't re-prompt on every page load (only once per session until
 * the user provides an email or explicitly skips).
 *
 * Phase 3.3 / 9.3 of the atproto auth migration plan.
 */

import {
	Alert,
	Button,
	Dialog,
	DialogActions,
	DialogContent,
	DialogContentText,
	DialogTitle,
	TextField,
} from "@mui/material";
import { getAtprotoSession, isAtprotoAuthenticated } from "core/atproto";
import { useAtprotoAuth, useUserId } from "hooks";
import { nhost } from "nhost";
import {
	type ChangeEventHandler,
	type FormEvent,
	useEffect,
	useState,
} from "react";
import { useTranslation } from "react-i18next";

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

/**
 * Base URL of the auth webhook server.
 * In production this is the deployed webhook; in dev it can be overridden.
 */
const AUTH_SERVER_URL =
	process.env.PUBLIC_AUTH_SERVER_URL ?? "https://wiki-auth.overby.me";

const HASURA_URL = `https://${process.env.PUBLIC_NHOST_SUBDOMAIN}.hasura.${process.env.PUBLIC_NHOST_REGION}.nhost.run/v1/graphql`;

const SKIP_KEY = "atproto-email-collection-skipped";

// ---------------------------------------------------------------------------
// Auth-aware fetch helpers
// ---------------------------------------------------------------------------

/**
 * Build authorization headers for the current session.
 * Uses atproto DPoP fetch if available, otherwise NHost JWT.
 */
function getAuthHeaders(): {
	headers: Record<string, string>;
	fetchFn: typeof fetch;
} {
	const headers: Record<string, string> = {
		"Content-Type": "application/json",
	};

	let fetchFn: typeof fetch = fetch;

	if (isAtprotoAuthenticated()) {
		const session = getAtprotoSession();
		const dpopFetch: typeof fetch | undefined =
			session?.fetchHandler ?? session?.dpopFetch ?? session?.fetch;
		if (typeof dpopFetch === "function") {
			fetchFn = dpopFetch;
		}
	} else if (nhost.auth.isAuthenticated()) {
		headers.authorization = `Bearer ${nhost.auth.getAccessToken()}`;
	}

	return { headers, fetchFn };
}

/**
 * Execute an authenticated GraphQL query against Hasura.
 * Used only for reading the current user's email (not writing).
 */
async function gqlFetch<T = unknown>(
	query: string,
	variables: Record<string, unknown> = {},
): Promise<T> {
	const { headers, fetchFn } = getAuthHeaders();

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

// ---------------------------------------------------------------------------
// API calls
// ---------------------------------------------------------------------------

/**
 * Check whether the current user already has an email address stored.
 */
async function fetchUserEmail(userId: string): Promise<string | null> {
	const data = await gqlFetch<{
		user: { email: string | null } | null;
	}>(
		`query GetUserEmail($userId: uuid!) {
			user(id: $userId) {
				email
			}
		}`,
		{ userId },
	);
	return data.user?.email ?? null;
}

/**
 * Request a verification email from the auth webhook server.
 *
 * The server validates the auth token, creates a signed verification JWT,
 * and sends an email with a confirmation link. The email is only written
 * to auth.users.email after the user clicks the link.
 */
async function requestEmailVerification(
	email: string,
): Promise<{ ok: boolean; error?: string }> {
	const { headers, fetchFn } = getAuthHeaders();

	const response = await fetchFn(
		`${AUTH_SERVER_URL}/email/request-verification`,
		{
			method: "POST",
			headers,
			body: JSON.stringify({ email }),
		},
	);

	const json = (await response.json()) as { ok?: boolean; error?: string };

	if (!response.ok) {
		return { ok: false, error: json.error ?? `HTTP ${response.status}` };
	}

	return { ok: true };
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

/**
 * Check whether the email collection dialog should be shown.
 * Returns `true` for authenticated atproto users who haven't
 * already provided an email or explicitly skipped the prompt.
 */
function useShowEmailCollection(): boolean {
	const { isAuthenticated, did } = useAtprotoAuth();
	const userId = useUserId();
	const [shouldShow, setShouldShow] = useState(false);

	useEffect(() => {
		if (!isAuthenticated || !did || !userId) {
			setShouldShow(false);
			return;
		}

		// Don't show if the user previously skipped
		const skipped = localStorage.getItem(SKIP_KEY);
		if (skipped === did) {
			setShouldShow(false);
			return;
		}

		// Check if the user already has an email stored.
		(async () => {
			try {
				const email = await fetchUserEmail(userId);
				if (email) {
					// User already has an email — no need to prompt
					setShouldShow(false);
				} else {
					setShouldShow(true);
				}
			} catch {
				// If the query fails, don't block the user — just skip
				setShouldShow(false);
			}
		})();
	}, [isAuthenticated, did, userId]);

	return shouldShow;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Validates a basic email format.
 */
function isValidEmail(value: string): boolean {
	return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(value);
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

type DialogState = "input" | "sending" | "sent" | "error";

export default function EmailCollectionDialog() {
	const { t } = useTranslation();
	const { did } = useAtprotoAuth();
	const shouldShow = useShowEmailCollection();

	const [open, setOpen] = useState(false);
	const [email, setEmail] = useState("");
	const [error, setError] = useState("");
	const [state, setState] = useState<DialogState>("input");

	// Open when the hook says we should show
	useEffect(() => {
		if (shouldShow) {
			setOpen(true);
		}
	}, [shouldShow]);

	const onEmailChange: ChangeEventHandler<HTMLInputElement> = (e) => {
		setEmail(e.target.value.trim());
		if (error) setError("");
		if (state === "error") setState("input");
	};

	const handleSkip = () => {
		if (did) {
			localStorage.setItem(SKIP_KEY, did);
		}
		setOpen(false);
	};

	const handleSend = async (e?: FormEvent) => {
		e?.preventDefault();

		if (!email) {
			setError(t("auth.missingEmail", "Email required"));
			return;
		}

		if (!isValidEmail(email)) {
			setError(t("auth.invalidEmail", "Invalid email"));
			return;
		}

		setState("sending");
		setError("");

		try {
			const result = await requestEmailVerification(email.toLowerCase());

			if (result.ok) {
				setState("sent");
			} else {
				setState("error");
				setError(
					result.error ===
						"this email address is already in use by another account"
						? t(
								"auth.emailTaken",
								"This email address is already in use by another account",
							)
						: t(
								"auth.emailSendError",
								"Could not send verification email: {{message}}",
								{
									message: result.error,
								},
							),
				);
			}
		} catch (err) {
			const message = err instanceof Error ? err.message : String(err);
			console.error("Failed to request email verification:", err);
			setState("error");
			setError(
				t(
					"auth.emailSendError",
					"Could not send verification email: {{message}}",
					{
						message,
					},
				),
			);
		}
	};

	const handleClose = () => {
		if (did && state !== "sent") {
			// If they close without completing, treat as skip
			localStorage.setItem(SKIP_KEY, did);
		}
		setOpen(false);
	};

	if (!shouldShow && !open) return null;

	// "Check your inbox" state after successful send
	if (state === "sent") {
		return (
			<Dialog open={open} onClose={handleClose} maxWidth="sm" fullWidth>
				<DialogTitle>
					{t("auth.emailSentTitle", "Check your inbox")}
				</DialogTitle>
				<DialogContent>
					<Alert severity="success" sx={{ mb: 2 }}>
						{t(
							"auth.emailSentMessage",
							"We have sent a verification email to {{email}}. Click the link in the email to confirm your email address.",
							{ email },
						)}
					</Alert>
					<DialogContentText>
						{t(
							"auth.emailSentHint",
							"The link expires in 1 hour. Check your spam filter if you cannot find the email.",
						)}
					</DialogContentText>
				</DialogContent>
				<DialogActions>
					<Button onClick={handleClose} variant="contained">
						{t("common.ok", "OK")}
					</Button>
				</DialogActions>
			</Dialog>
		);
	}

	// Input / error state
	return (
		<Dialog
			open={open}
			onClose={handleClose}
			maxWidth="sm"
			fullWidth
			PaperProps={{
				component: "form",
				onSubmit: handleSend,
			}}
		>
			<DialogTitle>
				{t("auth.emailCollectionTitle", "Add email address")}
			</DialogTitle>
			<DialogContent>
				<DialogContentText sx={{ mb: 2 }}>
					{t(
						"auth.emailCollectionDescription",
						"Your email address is used for invitations and notifications in the wiki. We will send a verification email — your address is only saved once you click the link. You can always add it later.",
					)}
				</DialogContentText>

				<TextField
					autoFocus
					fullWidth
					type="email"
					label={t("auth.email", "Email")}
					placeholder="you@email.com"
					value={email}
					onChange={onEmailChange}
					error={!!error}
					helperText={error || undefined}
					disabled={state === "sending"}
				/>
			</DialogContent>
			<DialogActions>
				<Button onClick={handleSkip} disabled={state === "sending"}>
					{t("common.skip", "Skip")}
				</Button>
				<Button
					type="submit"
					variant="contained"
					disabled={state === "sending" || !email}
				>
					{state === "sending"
						? t("common.sending", "Sending...")
						: t("auth.sendVerification", "Send verification email")}
				</Button>
			</DialogActions>
		</Dialog>
	);
}
