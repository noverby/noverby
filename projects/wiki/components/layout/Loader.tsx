import { AppLoader, HomeApp, MimeLoader, UnknownApp } from "comps";
import { useRouter } from "next/router";

const Loader = ({ app, id }: { app?: string; id?: string }) => {
	const router = useRouter();

	if (!router.query.app && app === "home") {
		return <HomeApp />;
	} else if (router.query.app || app) {
		return (
			(id && <AppLoader app={(router.query.app as string) ?? app} id={id} />) ||
			null
		);
	} else {
		return id ? <MimeLoader id={id} /> : <UnknownApp />;
	}
};

export default Loader;
