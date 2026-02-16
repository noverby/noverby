import { useEffect, useState } from "react";
import Graph from "../components/Graph";
import { Search } from "./search";
import { X } from "./x";
import { Yt } from "./yt";

const Index = () => {
	const [showing, setShowing] = useState(false);
	useEffect(() => {
		setShowing(true);
	}, []);

	console.log("App component loaded");
	return (
		<>
			<a rel="me" href="https://mas.to/@niclasoverby" />
			{showing && <Graph />}
		</>
	);
};
export { Index, Search, X, Yt };
