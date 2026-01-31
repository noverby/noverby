import { useLocation } from 'react-router-dom';

const usePathList = () => {
	const path = useLocation().pathname;
	return path ? path.split("/").filter(Boolean) : [];
};

export default usePathList;
