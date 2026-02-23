import { CssBaseline } from "@mui/material";
import { daDK as muiDaDK, enUS as muiEnUS } from "@mui/material/locale";
import { createTheme, ThemeProvider } from "@mui/material/styles";
import { deepmerge } from "@mui/utils";
import { daDK as gridDaDK, enUS as gridEnUS } from "@mui/x-data-grid/locales";
import type React from "react";
import { type FC, useContext, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { getDesignTokens, getThemedComponents } from "./M3Theme";
import { ThemeModeContext } from "./ThemeModeContext";
import { ThemeSchemeContext } from "./ThemeSchemeContext";

type M3ThemeProps = {
	children: React.ReactNode;
};

const M3ThemeProvider: FC<M3ThemeProps> = ({ children }) => {
	const { themeMode } = useContext(ThemeModeContext);
	const { themeScheme } = useContext(ThemeSchemeContext);
	const { i18n } = useTranslation();

	const muiLocale = i18n.language === "da" ? muiDaDK : muiEnUS;
	const gridLocale = i18n.language === "da" ? gridDaDK : gridEnUS;

	const m3Theme = useMemo(() => {
		const designTokens = getDesignTokens(
			themeMode,
			themeScheme[themeMode],
			themeScheme.tones,
		);
		const newM3Theme = createTheme(designTokens, muiLocale, gridLocale);
		const newM3ThemeMerged = deepmerge(
			newM3Theme,
			getThemedComponents(newM3Theme),
		);

		if (typeof window !== "undefined")
			document
				.querySelector('meta[name="theme-color"]')
				?.setAttribute("content", themeScheme[themeMode].surface);

		return newM3ThemeMerged;
	}, [themeMode, themeScheme, muiLocale, gridLocale]);

	return (
		<ThemeProvider theme={m3Theme}>
			<CssBaseline enableColorScheme />
			{children}
		</ThemeProvider>
	);
};

export default M3ThemeProvider;
