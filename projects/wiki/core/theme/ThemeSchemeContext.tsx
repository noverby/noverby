import {
	argbFromHex,
	hexFromArgb,
	themeFromSourceColor,
} from "@material/material-color-utilities";
import { createContext, type FC, useEffect, useState } from "react";
import { DEFAULT_M3_THEME_SCHEME, type M3ThemeScheme } from "./M3Theme";

export type ThemeSchemeContextType = {
	themeScheme: M3ThemeScheme;
	generateThemeScheme: (base: string) => void;
	resetThemeScheme: () => void;
};

export const ThemeSchemeContext = createContext<ThemeSchemeContextType>({
	themeScheme: DEFAULT_M3_THEME_SCHEME,
	generateThemeScheme: async (_base: string) => {},
	resetThemeScheme: () => {},
});

const THEME_SCHEME_KEY = "ThemeScheme";

const ThemeSchemeProvider: FC<{ children: React.ReactNode }> = ({
	children,
}) => {
	const [themeScheme, setThemeScheme] = useState<M3ThemeScheme>(
		DEFAULT_M3_THEME_SCHEME,
	);

	useEffect(() => {
		if (localStorage.getItem(THEME_SCHEME_KEY)) {
			const localThemeScheme = JSON.parse(
				localStorage.getItem(THEME_SCHEME_KEY) || "{}",
			);
			setThemeScheme(localThemeScheme);
		}
	}, []);

	const generateThemeScheme = (colorBase: string) => {
		const theme = themeFromSourceColor(argbFromHex(colorBase), [
			{
				name: "custom-1",
				value: argbFromHex("#303f9f"),
				blend: true,
			},
		]);

		const paletteTones: Record<string, Record<number, string>> = {};
		for (const [key, palette] of Object.entries(theme.palettes)) {
			const tones: Record<number, string> = {};
			for (const tone of [0, 10, 20, 30, 40, 50, 60, 70, 80, 90, 95, 99, 100]) {
				const color = hexFromArgb(palette.tone(tone));
				tones[tone] = color;
			}
			paletteTones[key] = tones;
		}

		const light: Record<string, string> = {};
		for (const [key, value] of Object.entries(theme.schemes.light.toJSON())) {
			const color = hexFromArgb(value);
			light[key] = color;
		}

		const dark: Record<string, string> = {};
		for (const [key, value] of Object.entries(theme.schemes.dark.toJSON())) {
			const color = hexFromArgb(value);
			dark[key] = color;
		}
		const scheme: M3ThemeScheme = {
			// @ts-expect-error: needs to be replaced anyway
			light,
			// @ts-expect-error: needs to be replaced anyway
			dark,
			// @ts-expect-error: needs to be replaced anyway
			tones: paletteTones,
		};
		setThemeScheme(scheme);
		localStorage.setItem(THEME_SCHEME_KEY, JSON.stringify(scheme));
	};

	const resetThemeScheme = () => {
		setThemeScheme(DEFAULT_M3_THEME_SCHEME);
		localStorage.setItem(
			THEME_SCHEME_KEY,
			JSON.stringify(DEFAULT_M3_THEME_SCHEME),
		);
	};

	return (
		<ThemeSchemeContext.Provider
			value={{ themeScheme, generateThemeScheme, resetThemeScheme }}
		>
			{children}
		</ThemeSchemeContext.Provider>
	);
};

export default ThemeSchemeProvider;
