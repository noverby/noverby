import { useRouter } from "next/router";
import { useEffect } from "react";

const P404 = () => {
	const router = useRouter();

	useEffect(() => {
		router.push(router.asPath);
	}, [router.asPath]);

	return <></>;
};

export default P404;
