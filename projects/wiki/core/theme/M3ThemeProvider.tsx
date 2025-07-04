import { CssBaseline } from "@mui/material";

import { createTheme, ThemeProvider } from "@mui/material/styles";
import { deepmerge } from "@mui/utils";
import type React from "react";
import { type FC, useContext, useMemo } from "react";
import { getDesignTokens, getThemedComponents } from "./M3Theme";
import { ThemeModeContext } from "./ThemeModeContext";
import { ThemeSchemeContext } from "./ThemeSchemeContext";

type M3ThemeProps = {
	children: React.ReactNode;
};

const M3ThemeProvider: FC<M3ThemeProps> = ({ children }) => {
	const { themeMode } = useContext(ThemeModeContext);
	const { themeScheme } = useContext(ThemeSchemeContext);

	const m3Theme = useMemo(() => {
		const designTokens = getDesignTokens(
			themeMode,
			themeScheme[themeMode],
			themeScheme.tones,
		);
		const newM3Theme = createTheme(designTokens);
		const newM3ThemeMerged = deepmerge(
			newM3Theme,
			getThemedComponents(newM3Theme),
		);

		if (typeof window !== "undefined")
			document
				.querySelector('meta[name="theme-color"]')
				?.setAttribute("content", themeScheme[themeMode].surface);

		return newM3ThemeMerged;
	}, [themeMode, themeScheme]);

	return (
		<ThemeProvider theme={m3Theme}>
			<CssBaseline enableColorScheme />
			{children}
		</ThemeProvider>
	);
};

export default M3ThemeProvider;
