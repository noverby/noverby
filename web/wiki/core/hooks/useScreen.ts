import { useSearchParams } from "react-router-dom";

const useScreen = () => {
	const [searchParams] = useSearchParams();
	return searchParams.get("app") === "screen";
};

export default useScreen;
