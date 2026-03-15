import { Route, BrowserRouter as Router, Routes } from "react-router-dom";
import { Index, Search, X, Yt } from "./pages";

const App = () => {
	return (
		<Router>
			<Routes>
				<Route path="/" element={<Index />} />
				<Route path="/search" element={<Search />} />
				<Route path="/x" element={<X />} />
				<Route path="/yt" element={<Yt />} />
			</Routes>
		</Router>
	);
};

export default App;
