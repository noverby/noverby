import i18n from "i18next";
import LanguageDetector from "i18next-browser-languagedetector";
import { initReactI18next } from "react-i18next";

import da from "./locales/da/translation.json";
import en from "./locales/en/translation.json";

i18n
	.use(LanguageDetector)
	.use(initReactI18next)
	.init({
		resources: {
			en: { translation: en },
			da: { translation: da },
		},
		fallbackLng: "da",
		interpolation: {
			escapeValue: false,
		},
		detection: {
			order: ["navigator", "htmlTag", "localStorage", "cookie"],
			caches: ["localStorage"],
		},
	});

export default i18n;
