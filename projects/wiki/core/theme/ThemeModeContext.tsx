import { createContext, type FC, useEffect, useState } from "react";
import type { M3ThemeMode } from "./M3Theme";

export type ThemeModeContextType = {
	themeMode: M3ThemeMode;
	toggleThemeMode: () => void;
	resetThemeMode: () => void;
};

const DEFAULT_MODE: M3ThemeMode = "light";
const THEME_MODE_KEY = "ThemeMode";

export const ThemeModeContext = createContext<ThemeModeContextType>({
	themeMode: DEFAULT_MODE,
	toggleThemeMode: () => {},
	resetThemeMode: () => {},
});

const ThemeModeProvider: FC<{ children: React.ReactNode }> = ({ children }) => {
	const [themeMode, setThemeMode] = useState<M3ThemeMode>(DEFAULT_MODE);

	useEffect(() => {
		if (localStorage.getItem(THEME_MODE_KEY)) {
			const localMode = JSON.parse(
				localStorage.getItem(THEME_MODE_KEY) || "{}",
			);
			setThemeMode(localMode);
		}
	}, []);

	const toggleThemeMode = () => {
		const value = themeMode === "light" ? "dark" : "light";
		setThemeMode(value);
		localStorage.setItem(THEME_MODE_KEY, JSON.stringify(value));
	};

	const resetThemeMode = () => {
		setThemeMode("light");
		localStorage.setItem(THEME_MODE_KEY, JSON.stringify(DEFAULT_MODE));
	};

	return (
		<ThemeModeContext.Provider
			value={{ themeMode, toggleThemeMode, resetThemeMode }}
		>
			{children}
		</ThemeModeContext.Provider>
	);
};

export default ThemeModeProvider;
