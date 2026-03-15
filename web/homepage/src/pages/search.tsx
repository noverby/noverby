import { useEffect } from "react";
import { useSearchParams } from "react-router-dom";

const Search = () => {
	const [params] = useSearchParams();
	const match = params.get("url")?.match(/.*q=([^&]*)/)?.[1];

	const goto = (url: string) => {
		window.location.href = url;
	};

	useEffect(() => {
		goto(`https://startpage.com${match ? `/search?q=${match}` : ""}`);
	}, [match]);
	return null;
};

export { Search };
