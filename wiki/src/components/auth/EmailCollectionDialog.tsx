/**
 * Email Collection Dialog.
 *
 * Shown to atproto users after their first login when they have no email
 * on record. The email is used for wiki invitations and notifications.
 *
 * The dialog is skippable — the user can dismiss it and provide their
 * email later from settings. It stores a flag in localStorage so it
 * doesn't re-prompt on every page load (only once per session until
 * the user provides an email or explicitly skips).
 *
 * Note: The `auth.users` table is not directly exposed in the GQty
 * generated schema, so we use raw GraphQL queries via fetch instead
 * of the typed GQL client for user email operations.
 *
 * Phase 3.3 of the atproto auth migration plan.
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
// Raw GraphQL helpers (users table not in GQty schema)
// ---------------------------------------------------------------------------

const HASURA_URL = `https://${process.env.PUBLIC_NHOST_SUBDOMAIN}.hasura.${process.env.PUBLIC_NHOST_REGION}.nhost.run/v1/graphql`;

const SKIP_KEY = "atproto-email-collection-skipped";

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
 * Update the current user's email address.
 */
async function updateUserEmail(userId: string, email: string): Promise<void> {
	await gqlFetch(
		`mutation UpdateUserEmail($userId: uuid!, $email: citext!) {
			updateUser(pk_columns: { id: $userId }, _set: { email: $email }) {
				id
			}
		}`,
		{ userId, email },
	);
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

export default function EmailCollectionDialog() {
	const { t } = useTranslation();
	const { did } = useAtprotoAuth();
	const userId = useUserId();
	const shouldShow = useShowEmailCollection();

	const [open, setOpen] = useState(false);
	const [email, setEmail] = useState("");
	const [error, setError] = useState("");
	const [saving, setSaving] = useState(false);
	const [success, setSuccess] = useState(false);

	// Open when the hook says we should show
	useEffect(() => {
		if (shouldShow) {
			setOpen(true);
		}
	}, [shouldShow]);

	const onEmailChange: ChangeEventHandler<HTMLInputElement> = (e) => {
		setEmail(e.target.value.trim());
		if (error) setError("");
	};

	const handleSkip = () => {
		if (did) {
			localStorage.setItem(SKIP_KEY, did);
		}
		setOpen(false);
	};

	const handleSave = async (e?: FormEvent) => {
		e?.preventDefault();

		if (!email) {
			setError(t("auth.missingEmail", "Indtast venligst en e-mailadresse"));
			return;
		}

		if (!isValidEmail(email)) {
			setError(t("auth.invalidEmail", "Ugyldig e-mailadresse"));
			return;
		}

		if (!userId) {
			setError("Bruger-ID mangler");
			return;
		}

		setSaving(true);
		setError("");

		try {
			await updateUserEmail(userId, email.toLowerCase());

			setSuccess(true);
			// Close after a brief delay so the user sees the success state
			setTimeout(() => {
				setOpen(false);
			}, 1500);
		} catch (err) {
			const message = err instanceof Error ? err.message : String(err);
			console.error("Failed to save email:", err);
			setError(
				t(
					"auth.emailSaveError",
					"Kunne ikke gemme e-mailadresse: {{message}}",
					{ message },
				),
			);
		} finally {
			setSaving(false);
		}
	};

	if (!shouldShow && !open) return null;

	return (
		<Dialog
			open={open}
			onClose={handleSkip}
			maxWidth="sm"
			fullWidth
			PaperProps={{
				component: "form",
				onSubmit: handleSave,
			}}
		>
			<DialogTitle>
				{t("auth.emailCollectionTitle", "Tilføj e-mailadresse")}
			</DialogTitle>
			<DialogContent>
				<DialogContentText sx={{ mb: 2 }}>
					{t(
						"auth.emailCollectionDescription",
						"Din e-mailadresse bruges til invitationer og notifikationer i wikien. Du kan altid tilføje den senere.",
					)}
				</DialogContentText>

				<TextField
					autoFocus
					fullWidth
					type="email"
					label={t("auth.email", "E-mail")}
					placeholder="din@email.dk"
					value={email}
					onChange={onEmailChange}
					error={!!error}
					helperText={error || undefined}
					disabled={saving || success}
				/>

				{success && (
					<Alert severity="success" sx={{ mt: 2 }}>
						{t("auth.emailSaved", "E-mailadresse gemt!")}
					</Alert>
				)}
			</DialogContent>
			<DialogActions>
				<Button onClick={handleSkip} disabled={saving}>
					{t("common.skip", "Spring over")}
				</Button>
				<Button
					type="submit"
					variant="contained"
					disabled={saving || success || !email}
				>
					{saving ? t("common.saving", "Gemmer...") : t("common.save", "Gem")}
				</Button>
			</DialogActions>
		</Dialog>
	);
}
