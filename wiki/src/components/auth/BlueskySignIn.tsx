/**
 * Bluesky Sign-In Component.
 *
 * Renders a handle input field and a "Sign in with Bluesky" button.
 * When submitted, it triggers the atproto OAuth flow via the
 * AtprotoAuthProvider, which redirects the user to their Bluesky
 * authorization server.
 *
 * Used inside AuthForm.tsx as the primary sign-in option above the
 * legacy NHost email/password form.
 *
 * The component probes the wiki-auth server (`/healthz`) on mount.
 * If the server is unreachable the entire Bluesky section is hidden,
 * since without wiki-auth Hasura cannot validate DPoP tokens and
 * the register/link endpoints do not exist.
 */

import {
	Alert,
	Avatar,
	Box,
	Button,
	CircularProgress,
	Divider,
	InputAdornment,
	Stack,
	TextField,
} from "@mui/material";
import { useAtprotoSignIn } from "hooks";
import {
	type ChangeEventHandler,
	type FormEvent,
	useEffect,
	useState,
} from "react";
import { useTranslation } from "react-i18next";

const AUTH_SERVER_URL =
	process.env.PUBLIC_AUTH_SERVER_URL ?? "https://auth.radikal.wiki";

/**
 * Bluesky butterfly SVG icon used as the button icon and avatar.
 * Simplified version of the Bluesky logo.
 */
function BlueskyIcon({ size = 24 }: { size?: number }) {
	return (
		<svg
			width={size}
			height={size}
			viewBox="0 0 568 501"
			fill="currentColor"
			xmlns="http://www.w3.org/2000/svg"
			role="img"
			aria-label="Bluesky"
		>
			<title>Bluesky</title>
			<path d="M123.121 33.6637C188.241 82.5526 258.281 181.681 284 234.873C309.719 181.681 379.759 82.5526 444.879 33.6637C491.866 -1.61183 568 -28.9064 568 57.9464C568 75.2916 558.055 203.659 552.222 224.501C531.947 296.954 458.067 315.434 392.347 304.249C507.222 323.8 536.444 388.56 473.333 453.32C353.473 576.312 301.061 422.461 287.631 383.039C285.169 374.577 284.043 370.529 284 372.799C283.957 370.529 282.831 374.577 280.369 383.039C266.939 422.461 214.527 576.312 94.6667 453.32C31.5556 388.56 60.7778 323.8 175.653 304.249C109.933 315.434 36.0535 296.954 15.7778 224.501C9.94525 203.659 0 75.2916 0 57.9464C0 -28.9064 76.1338 -1.61183 123.121 33.6637Z" />
		</svg>
	);
}

export default function BlueskySignIn() {
	const { t } = useTranslation();
	const signIn = useAtprotoSignIn();

	const [handle, setHandle] = useState("");
	const [error, setError] = useState("");
	const [loading, setLoading] = useState(false);
	const [serverAvailable, setServerAvailable] = useState<boolean | null>(null);

	// Probe the wiki-auth server on mount. If it is unreachable the
	// entire Bluesky sign-in section is hidden — without wiki-auth,
	// Hasura cannot validate DPoP tokens and the register/link
	// endpoints do not exist, so the atproto flow is a dead end.
	useEffect(() => {
		let cancelled = false;
		(async () => {
			try {
				const res = await fetch(`${AUTH_SERVER_URL}/healthz`, {
					signal: AbortSignal.timeout(5000),
				});
				if (!cancelled) setServerAvailable(res.ok);
			} catch {
				if (!cancelled) setServerAvailable(false);
			}
		})();
		return () => {
			cancelled = true;
		};
	}, []);

	// While probing or if the server is down, render nothing —
	// including the divider that separates Bluesky from the email form.
	if (serverAvailable !== true) return null;

	const onHandleChange: ChangeEventHandler<HTMLInputElement> = (e) => {
		const value = e.target.value.trim();
		setHandle(value);
		if (error) setError("");
	};

	/**
	 * Validate the handle format before initiating the OAuth flow.
	 * Accepts formats like:
	 *   - alice.bsky.social
	 *   - @alice.bsky.social
	 *   - did:plc:abc123
	 */
	function validateHandle(input: string): string | null {
		const trimmed = input.trim().replace(/^@/, "");
		if (!trimmed) return null;

		// Accept DIDs directly
		if (trimmed.startsWith("did:")) return trimmed;

		// Must look like a domain-style handle (at least one dot)
		if (trimmed.includes(".") && trimmed.length >= 3) return trimmed;

		return null;
	}

	const handleSubmit = async (e: FormEvent) => {
		e.preventDefault();

		const validHandle = validateHandle(handle);
		if (!validHandle) {
			setError(
				t(
					"auth.invalidHandle",
					"Ugyldigt Bluesky-handle (f.eks. alice.bsky.social)",
				),
			);
			return;
		}

		setLoading(true);
		setError("");

		try {
			await signIn(validHandle);
			// signIn triggers a redirect — we won't reach here normally
		} catch (err) {
			const message = err instanceof Error ? err.message : String(err);
			console.error("Bluesky sign-in error:", err);
			setError(
				t(
					"auth.blueskySignInError",
					"Kunne ikke logge ind med Bluesky: {{message}}",
					{
						message,
					},
				),
			);
			setLoading(false);
		}
	};

	return (
		<Stack spacing={2} alignItems="center" sx={{ mb: 3 }}>
			<Box component="form" onSubmit={handleSubmit} sx={{ width: "100%" }}>
				<Box
					sx={{
						display: "flex",
						flexDirection: "column",
						alignItems: "center",
						gap: 2,
					}}
				>
					<Avatar sx={{ bgcolor: "#0085ff" }}>
						<BlueskyIcon size={22} />
					</Avatar>

					<TextField
						fullWidth
						label={t("auth.blueskyHandle", "Bluesky-handle")}
						placeholder="alice.bsky.social"
						value={handle}
						onChange={onHandleChange}
						error={!!error}
						helperText={error || undefined}
						disabled={loading}
						slotProps={{
							input: {
								startAdornment: (
									<InputAdornment position="start">@</InputAdornment>
								),
							},
						}}
					/>

					<Box sx={{ position: "relative", width: "100%" }}>
						<Button
							fullWidth
							type="submit"
							variant="contained"
							disabled={loading || !handle.trim()}
							startIcon={<BlueskyIcon size={18} />}
							sx={{
								bgcolor: "#0085ff",
								"&:hover": { bgcolor: "#0070dd" },
								textTransform: "none",
								fontWeight: 600,
								py: 1.2,
							}}
						>
							{t("auth.signInWithBluesky", "Log ind med Bluesky")}
						</Button>

						{loading && (
							<CircularProgress
								size={24}
								sx={{
									position: "absolute",
									top: "50%",
									left: "50%",
									marginTop: "-12px",
									marginLeft: "-12px",
								}}
							/>
						)}
					</Box>

					{error && (
						<Alert severity="error" sx={{ width: "100%" }}>
							{error}
						</Alert>
					)}
				</Box>
			</Box>
			<Divider sx={{ width: "100%" }}>
				{t("auth.orSignInWith", "eller log ind med e-mail")}
			</Divider>
		</Stack>
	);
}
