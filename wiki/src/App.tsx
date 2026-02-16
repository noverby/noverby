import { LocalizationProvider } from "@mui/x-date-pickers";
import { AdapterDateFns } from "@mui/x-date-pickers/AdapterDateFns";
import { NhostProvider } from "@nhost/react";
import { Layout, SessionProvider, SnackbarProvider } from "comps";
import { initBugfender } from "core/bugfender";
import M3ThemeProvider from "core/theme/M3ThemeProvider";
import ThemeModeProvider from "core/theme/ThemeModeContext";
import ThemeSchemeProvider from "core/theme/ThemeSchemeContext";
import { nhost } from "nhost";
import { useEffect } from "react";
import { ErrorBoundary } from "react-error-boundary";
import { Route, BrowserRouter as Router, Routes } from "react-router-dom";
import { Index } from "./pages";
import Path from "./pages/[...path]";
import Login from "./pages/user/login";
import Register from "./pages/user/register";
import ResetPassword from "./pages/user/reset-password";
import SetPassword from "./pages/user/set-password";
import Unverified from "./pages/user/unverified";

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

const App = () => {
	useEffect(() => {
		// Remove the server-side injected CSS.
		const jssStyles = document.querySelector("#jss-server-side");
		jssStyles?.parentElement?.removeChild(jssStyles);
	}, []);
	useEffect(() => {
		initBugfender();
	}, []);

	return (
		<ErrorBoundary fallbackRender={fallbackRender}>
			<NhostProvider nhost={nhost}>
				<LocalizationProvider dateAdapter={AdapterDateFns}>
					<SessionProvider>
						<ThemeModeProvider>
							<ThemeSchemeProvider>
								<M3ThemeProvider>
									<SnackbarProvider>
										<Router>
											<Layout>
												<Routes>
													<Route path="/" element={<Index />} />
													<Route path="/user/login" element={<Login />} />
													<Route path="/user/register" element={<Register />} />
													<Route
														path="/user/reset-password"
														element={<ResetPassword />}
													/>
													<Route
														path="/user/set-password"
														element={<SetPassword />}
													/>
													<Route
														path="/user/unverified"
														element={<Unverified />}
													/>
													<Route path="/*" element={<Path />} />
												</Routes>
											</Layout>
										</Router>
									</SnackbarProvider>
								</M3ThemeProvider>
							</ThemeSchemeProvider>
						</ThemeModeProvider>
					</SessionProvider>
				</LocalizationProvider>
			</NhostProvider>
		</ErrorBoundary>
	);
};

export default App;
