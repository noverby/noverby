"use client";
import { LocalizationProvider } from "@mui/x-date-pickers";
import { AdapterDateFns } from "@mui/x-date-pickers/AdapterDateFns";
import { NhostProvider } from "@nhost/nextjs";
import { Layout, SessionProvider, SnackbarProvider } from "comps";
import { initBugfender } from "core/bugfender";
import M3ThemeProvider from "core/theme/M3ThemeProvider";
import ThemeModeProvider from "core/theme/ThemeModeContext";
import ThemeSchemeProvider from "core/theme/ThemeSchemeContext";
import type { AppProps } from "next/app";
import Head from "next/head";
import { nhost } from "nhost";
import { useEffect } from "react";
import { ErrorBoundary } from "react-error-boundary";

const fallbackRender = ({ error }: { error: { stack: string } }) => {
	return (
		<div role="alert">
			<h3>Noget gik galt! 😔</h3>
			<p>
				Send venligst følgende besked til{" "}
				<a href="mailto:niclas@overby.me">niclas@overby.me</a>:
			</p>
			<pre style={{ background: "#EDEDED", padding: "20px" }}>
				{error.stack}
			</pre>
		</div>
	);
};

const App = ({ Component, pageProps }: AppProps) => {
	useEffect(() => {
		// Remove the server-side injected CSS.
		const jssStyles = document.querySelector("#jss-server-side");
		jssStyles?.parentElement?.removeChild(jssStyles);
	}, []);
	useEffect(() => {
		initBugfender();
	}, []);

	return (
		<>
			<Head>
				<title>RadikalWiki</title>
				<meta
					name="viewport"
					content="minimum-scale=1, initial-scale=1, width=device-width"
				/>
			</Head>

			<ErrorBoundary fallbackRender={fallbackRender}>
				<NhostProvider nhost={nhost}>
					<LocalizationProvider dateAdapter={AdapterDateFns}>
						<SessionProvider>
							<ThemeModeProvider>
								<ThemeSchemeProvider>
									<M3ThemeProvider>
										<SnackbarProvider>
											<Layout>
												<Component {...pageProps} />
											</Layout>
										</SnackbarProvider>
									</M3ThemeProvider>
								</ThemeSchemeProvider>
							</ThemeModeProvider>
						</SessionProvider>
					</LocalizationProvider>
				</NhostProvider>
			</ErrorBoundary>
		</>
	);
};

export default App;
