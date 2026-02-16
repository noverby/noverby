import { useLocation } from "react-router-dom";

const usePathList = () => {
	const { pathname } = useLocation();
	const path = decodeURIComponent(pathname);
	return path ? path.split("/").filter(Boolean) : [];
};

export default usePathList;
