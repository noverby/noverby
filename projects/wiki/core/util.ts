import { compare } from "compare-versions";
import platform from "platform";

const checkVersion = () => {
	switch (platform.layout) {
		case "Gecko":
			if (compare(platform.version ?? "0", "94", "<=")) return false;
			break;
		case "Blink":
			if (
				compare(platform.version ?? "0", "98", "<=") &&
				platform.name !== "Opera"
			)
				return false;
			break;
		case "WebKit":
			if (compare(platform.version ?? "0", "15.4", "<=")) return false;
			break;
		default:
			return true;
	}
};

export { checkVersion };
