import { Route, BrowserRouter as Router, Routes } from "react-router-dom";
import { Index } from "./pages";

const App = () => {
	return (
		<Router>
			<Routes>
				<Route path="/" element={<Index />} />
			</Routes>
		</Router>
	);
};

export default App;
