import { AppList } from "comps";
import { Suspense } from "react";

const AppDrawer = () => {
	return (
		<Suspense>
			<AppList />
		</Suspense>
	);
};

export default AppDrawer;
