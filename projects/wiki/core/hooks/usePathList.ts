import { useParams } from "react-router-dom";

const usePathList = () => {
	const params = useParams();
	const path = params["*"];
	return path ? path.split("/").filter(Boolean) : [];
};

export default usePathList;
