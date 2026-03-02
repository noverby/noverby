/**
 * OAuth callback page for atproto authentication.
 *
 * This page is the redirect target after the user authorizes the app
 * on their Bluesky/AT Protocol authorization server. The URL will contain
 * OAuth callback parameters (code, state, etc.).
 *
 * The actual token exchange is handled by `atprotoClient.init()` inside
 * the `AtprotoAuthProvider`, which runs on every page load and detects
 * when the current URL is a callback URL. This component simply shows
 * a loading state while that processing happens, then redirects to `/`.
 *
 * Route: /auth/callback
 */

import { CircularProgress, Stack, Typography } from "@mui/material";
import { useAtprotoAuth } from "hooks";
import { useEffect } from "react";
import { useNavigate } from "react-router-dom";

export default function AuthCallback() {
	const navigate = useNavigate();
	const { isAuthenticated, isLoading } = useAtprotoAuth();

	useEffect(() => {
		if (!isLoading) {
			if (isAuthenticated) {
				// Successfully authenticated — go to home
				navigate("/", { replace: true });
			} else {
				// Authentication failed or was cancelled — back to login
				navigate("/user/login", { replace: true });
			}
		}
	}, [isAuthenticated, isLoading, navigate]);

	return (
		<Stack
			alignItems="center"
			justifyContent="center"
			spacing={2}
			sx={{ minHeight: "60vh" }}
		>
			<CircularProgress />
			<Typography variant="body1" color="text.secondary">
				Logger ind med Bluesky...
			</Typography>
		</Stack>
	);
}
