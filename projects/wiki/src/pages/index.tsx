import { Loader } from "comps";
import { startTransition, useEffect } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";

const Index = () => {
	const navigate = useNavigate();
	const [searchParams] = useSearchParams();

	useEffect(() => {
		if (searchParams.get("type") === "passwordReset") {
			startTransition(() => {
				navigate("/user/set-password");
			});
		}
	}, [searchParams, navigate]);

	return <Loader app="home" />;
};

export { Index };
