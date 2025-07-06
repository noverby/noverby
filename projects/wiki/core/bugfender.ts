export let isInitialized = false;

export const initBugfender = async () => {
	if (
		typeof window !== "undefined" &&
		!isInitialized &&
		process.env.NEXT_PUBLIC_BUGFENDER_APP_KEY
	) {
		const { Bugfender } = await import("@bugfender/sdk");
		Bugfender.init({
			appKey: process.env.NEXT_PUBLIC_BUGFENDER_APP_KEY,
			// Override console methods to capture all console.log, console.error, etc.
			overrideConsoleMethods: true,
			// Print logs to browser console as well (useful for development)
			printToConsole: process.env.NODE_ENV === "development",
			// Automatically register global error handlers for unhandled errors
			registerErrorHandler: true,
			// Log browser events (page loads, navigation, etc.)
			logBrowserEvents: true,
			// Log UI events (clicks, form submissions, etc.)
			logUIEvents: true,
			build: process.env.NEXT_PUBLIC_GIT_COMMIT_SHA ?? "dev",
		});

		isInitialized = true;
	}
};
