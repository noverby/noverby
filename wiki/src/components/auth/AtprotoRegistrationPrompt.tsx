/**
 * Atproto Registration Prompt.
 *
 * Shown when a user successfully authenticates via Bluesky OAuth but
 * their atproto DID is not linked to any existing Hasura user.  The
 * `/validate` webhook intentionally returns 401 for unlinked DIDs
 * instead of silently creating ghost accounts.
 *
 * The prompt gives the user two clear choices:
 *
 * 1. **Register** — create a brand-new wiki account linked to their
 *    Bluesky DID (calls `POST /register-atproto` on the auth server).
 *
 * 2. **Link existing account** — sign in with their existing NHost
 *    email/password first, then link the Bluesky DID to that account
 *    (opens the AccountLinkDialog flow).
 *
 * If the user dismisses the prompt or wants to start over they can
 * sign out of the atproto session entirely.
 */

import { HowToReg, Link as LinkIcon, Logout } from "@mui/icons-material";
import {
	Alert,
	Avatar,
	Box,
	Button,
	CircularProgress,
	Container,
	Divider,
	Stack,
	Typography,
} from "@mui/material";
import { getAtprotoSession, isAtprotoAuthenticated } from "core/atproto";
import { useAtprotoAuth, useAtprotoProfile, useAtprotoSignOut } from "hooks";
import { nhost } from "nhost";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";

const AUTH_SERVER_URL =
	process.env.PUBLIC_AUTH_SERVER_URL ?? "https://wiki-auth.overby.me";

/**
 * Bluesky butterfly SVG icon.
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

/**
 * Get the atproto DPoP-bound fetch function from the current session.
 */
function getAtprotoDpopFetch(): typeof fetch | null {
	if (!isAtprotoAuthenticated()) return null;
	const session = getAtprotoSession();
	const dpopFetch: typeof fetch | undefined =
		session?.fetchHandler ?? session?.dpopFetch ?? session?.fetch;
	return typeof dpopFetch === "function" ? dpopFetch : null;
}

/**
 * Call the server-side `/register-atproto` endpoint to create a new
 * wiki user linked to the current atproto DID.
 */
async function registerViaServer(): Promise<{
	ok: boolean;
	userId?: string;
	did?: string;
	handle?: string | null;
	alreadyRegistered?: boolean;
	error?: string;
}> {
	const dpopFetch = getAtprotoDpopFetch();
	if (!dpopFetch) {
		throw new Error("No atproto session available");
	}

	const response = await dpopFetch(`${AUTH_SERVER_URL}/register-atproto`, {
		method: "POST",
		headers: { "Content-Type": "application/json" },
		body: JSON.stringify({}),
	});

	const json = await response.json();

	if (!response.ok) {
		throw new Error(json.error ?? `Server returned ${response.status}`);
	}

	return json as {
		ok: boolean;
		userId?: string;
		did?: string;
		handle?: string | null;
		alreadyRegistered?: boolean;
	};
}

/**
 * Call the server-side `/link-atproto` endpoint to link the current
 * atproto DID to an existing NHost-authenticated user.
 */
async function linkViaServer(): Promise<{
	ok: boolean;
	did?: string;
	handle?: string | null;
	alreadyLinked?: boolean;
	error?: string;
}> {
	const dpopFetch = getAtprotoDpopFetch();
	if (!dpopFetch) {
		throw new Error("No atproto session available");
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

type Step = "choose" | "link-login" | "link-ready";

export default function AtprotoRegistrationPrompt() {
	const { t } = useTranslation();
	const navigate = useNavigate();
	const atproto = useAtprotoAuth();
	const profile = useAtprotoProfile();
	const atprotoSignOut = useAtprotoSignOut();

	const [step, setStep] = useState<Step>("choose");
	const [loading, setLoading] = useState(false);
	const [error, setError] = useState("");
	const [serverAvailable, setServerAvailable] = useState<boolean | null>(null);

	// Probe the wiki-auth server on mount so we can show a clear
	// "server not available" message instead of letting the user click
	// buttons that will fail with cryptic network errors.
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

	// NHost login state (for the "link existing" flow)
	const [email, setEmail] = useState("");
	const [password, setPassword] = useState("");
	const [loginError, setLoginError] = useState("");

	if (!atproto.isAuthenticated || !atproto.needsRegistration) {
		return null;
	}

	const displayName =
		profile.displayName ?? atproto.handle ?? atproto.did ?? "Bluesky user";

	// ----- Register as new user -----

	const handleRegister = async () => {
		setLoading(true);
		setError("");

		try {
			const result = await registerViaServer();
			if (result.ok) {
				// Reload the page so the atproto provider re-initialises and
				// fetchHasuraUserId succeeds now that the user exists.
				window.location.reload();
			} else {
				setError(
					result.error ?? t("auth.registrationFailed", "Registration failed"),
				);
			}
		} catch (err) {
			console.error("Atproto registration failed:", err);
			if (isNetworkError(err)) {
				setServerAvailable(false);
			} else {
				const message = err instanceof Error ? err.message : String(err);
				setError(
					t("auth.registrationError", "Could not register: {{message}}", {
						message,
					}),
				);
			}
		} finally {
			setLoading(false);
		}
	};

	// ----- Link to existing NHost account -----

	const handleNhostLogin = async () => {
		if (!email || !password) {
			setLoginError(
				t("auth.missingCredentials", "Please enter email and password"),
			);
			return;
		}

		setLoading(true);
		setLoginError("");
		setError("");

		try {
			const { error: nhostError } = await nhost.auth.signIn({
				email: email.toLowerCase(),
				password,
			});

			if (nhostError) {
				if (nhostError.error === "unverified-user") {
					setLoginError(t("auth.emailNotVerified", "Email not verified"));
				} else {
					setLoginError(t("auth.wrongCredentials", "Wrong email or password"));
				}
				setLoading(false);
				return;
			}

			// NHost session is now active — proceed to link
			setStep("link-ready");
			await performLink();
		} catch (err) {
			const message = err instanceof Error ? err.message : String(err);
			console.error("NHost login for linking failed:", err);
			setLoginError(message);
			setLoading(false);
		}
	};

	const performLink = async () => {
		setLoading(true);
		setError("");

		try {
			const result = await linkViaServer();
			if (result.ok) {
				// Sign out of NHost (we only needed it for the linking JWT)
				// then reload so atproto session picks up the linked user.
				await nhost.auth.signOut();
				window.location.reload();
			} else {
				setError(result.error ?? t("auth.linkFailed", "Linking failed"));
			}
		} catch (err) {
			console.error("Atproto account link failed:", err);
			if (isNetworkError(err)) {
				setServerAvailable(false);
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
	};

	// ----- Sign out -----

	const handleSignOut = async () => {
		await atprotoSignOut();
		navigate("/user/login");
	};

	return (
		<Container sx={{ padding: 3 }} maxWidth="xs">
			<Stack spacing={2} alignItems="center">
				<Avatar sx={{ bgcolor: "#0085ff", width: 56, height: 56 }}>
					<BlueskyIcon size={28} />
				</Avatar>

				<Typography variant="h5" textAlign="center">
					{t("auth.welcomeBluesky", "Velkommen, {{name}}!", {
						name: displayName,
					})}
				</Typography>

				<Typography variant="body2" color="text.secondary" textAlign="center">
					{t(
						"auth.blueskyNotLinked",
						"Din Bluesky-konto er ikke forbundet med en wiki-konto endnu. Opret en ny konto eller forbind til en eksisterende.",
					)}
				</Typography>

				{atproto.handle && (
					<Typography variant="body2" color="text.secondary">
						@{atproto.handle}
					</Typography>
				)}

				{error && (
					<Alert severity="error" sx={{ width: "100%" }}>
						{error}
					</Alert>
				)}

				{serverAvailable === false && (
					<Alert severity="warning" sx={{ width: "100%" }}>
						{t(
							"auth.authServerUnavailable",
							"Wiki-auth-serveren er ikke tilgængelig. Bluesky-login kræver at wiki-auth er sat op og kører. Kontakt en administrator, eller log ind med e-mail i stedet.",
						)}
					</Alert>
				)}

				{step === "choose" && serverAvailable !== false && (
					<>
						<Button
							fullWidth
							variant="contained"
							startIcon={
								loading ? (
									<CircularProgress size={18} color="inherit" />
								) : (
									<HowToReg />
								)
							}
							disabled={loading}
							onClick={handleRegister}
							sx={{
								bgcolor: "#0085ff",
								"&:hover": { bgcolor: "#0070dd" },
								textTransform: "none",
								fontWeight: 600,
								py: 1.2,
							}}
						>
							{t("auth.registerWithBluesky", "Opret ny konto med Bluesky")}
						</Button>

						<Divider sx={{ width: "100%" }}>{t("auth.or", "eller")}</Divider>

						<Button
							fullWidth
							variant="outlined"
							startIcon={<LinkIcon />}
							disabled={loading}
							onClick={() => setStep("link-login")}
							sx={{ textTransform: "none", fontWeight: 600, py: 1.2 }}
						>
							{t("auth.linkExistingAccount", "Forbind til eksisterende konto")}
						</Button>
					</>
				)}

				{step === "link-login" && (
					<Box sx={{ width: "100%" }}>
						<Stack spacing={2}>
							<Typography variant="subtitle2">
								{t(
									"auth.linkLoginPrompt",
									"Log ind med din eksisterende e-mail-konto for at forbinde den med Bluesky:",
								)}
							</Typography>

							<input
								type="email"
								placeholder={t("auth.email", "E-mail")}
								value={email}
								onChange={(e) => {
									setEmail(e.target.value);
									if (loginError) setLoginError("");
								}}
								disabled={loading}
								style={{
									width: "100%",
									padding: "12px 14px",
									fontSize: "1rem",
									borderRadius: 4,
									border: loginError
										? "1px solid #d32f2f"
										: "1px solid rgba(0,0,0,0.23)",
									boxSizing: "border-box",
								}}
							/>

							<input
								type="password"
								placeholder={t("auth.password", "Adgangskode")}
								value={password}
								onChange={(e) => {
									setPassword(e.target.value);
									if (loginError) setLoginError("");
								}}
								disabled={loading}
								onKeyDown={(e) => {
									if (e.key === "Enter") handleNhostLogin();
								}}
								style={{
									width: "100%",
									padding: "12px 14px",
									fontSize: "1rem",
									borderRadius: 4,
									border: loginError
										? "1px solid #d32f2f"
										: "1px solid rgba(0,0,0,0.23)",
									boxSizing: "border-box",
								}}
							/>

							{loginError && <Alert severity="error">{loginError}</Alert>}

							<Button
								fullWidth
								variant="contained"
								startIcon={
									loading ? (
										<CircularProgress size={18} color="inherit" />
									) : (
										<LinkIcon />
									)
								}
								disabled={loading || !email || !password}
								onClick={handleNhostLogin}
								sx={{ textTransform: "none", fontWeight: 600 }}
							>
								{t("auth.loginAndLink", "Log ind og forbind")}
							</Button>

							<Button
								size="small"
								disabled={loading}
								onClick={() => {
									setStep("choose");
									setLoginError("");
								}}
								sx={{ textTransform: "none" }}
							>
								{t("common.back", "Tilbage")}
							</Button>
						</Stack>
					</Box>
				)}

				{step === "link-ready" && (
					<Box sx={{ width: "100%", textAlign: "center" }}>
						<CircularProgress size={32} sx={{ my: 2 }} />
						<Typography variant="body2" color="text.secondary">
							{t("auth.linking", "Forbinder kontoer...")}
						</Typography>
					</Box>
				)}

				<Divider sx={{ width: "100%" }} />

				<Button
					size="small"
					color="inherit"
					startIcon={<Logout fontSize="small" />}
					onClick={handleSignOut}
					disabled={loading}
					sx={{ textTransform: "none", color: "text.secondary" }}
				>
					{t("auth.signOutBluesky", "Log ud af Bluesky")}
				</Button>
			</Stack>
		</Container>
	);
}
