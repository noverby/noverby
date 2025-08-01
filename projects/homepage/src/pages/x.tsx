import { useEffect } from "react";
import { useSearchParams } from "react-router-dom";

const X = () => {
	const [params] = useSearchParams();
	const match = params.get("url")?.match(/.*(x|twitter)\.com(.*)/)?.[2];

	const goto = (url: string) => {
		window.location.href = url;
	};

	useEffect(() => {
		if (!match) return;
		goto(`https://xcancel.com${match}`);
	}, [match]);
	return null;
};

export { X };
