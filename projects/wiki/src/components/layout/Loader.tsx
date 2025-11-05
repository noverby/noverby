import { AppLoader, HomeApp, MimeLoader, UnknownApp } from "comps";
import { useSearchParams } from "react-router-dom";

const Loader = ({ app, id }: { app?: string; id?: string }) => {
	const [searchParams] = useSearchParams();
	const queryApp = searchParams.get("app");

	if (!queryApp && app === "home") {
		return <HomeApp />;
	} else if (queryApp || app) {
		return (id && <AppLoader app={queryApp ?? app} id={id} />) || null;
	} else {
		return id ? <MimeLoader id={id} /> : <UnknownApp />;
	}
};

export default Loader;
