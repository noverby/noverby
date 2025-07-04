import { compare } from "compare-versions";
import platform from "platform";

const checkVersion = () => {
	switch (platform.layout) {
		case "Gecko":
			if (compare(platform.version ?? "0", "94", "<=")) return false;
			return true;
		case "Blink":
			if (
				compare(platform.version ?? "0", "98", "<=") &&
				platform.name !== "Opera"
			)
				return false;
			return true;
		case "WebKit":
			if (compare(platform.version ?? "0", "15.4", "<=")) return false;
			return true;
		default:
			return true;
	}
};

export { checkVersion };
