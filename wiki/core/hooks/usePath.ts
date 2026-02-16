import { useLocation } from "react-router-dom";

const usePath = () => {
	const location = useLocation();
	return decodeURI(location.pathname.slice(1).split("?").slice(0, 1).join(""));
};

export default usePath;
