import { Stack } from "@mui/material";
import { PollAdmin } from "comps";
import { type Node, useScreen } from "hooks";

const PollApp = ({ node }: { node: Node }) => {
	const screen = useScreen();

	return (
		<Stack spacing={1}>
			{!screen && <PollAdmin node={node} />}
			{/* <PollChartSub node={node} /> */}
		</Stack>
	);
};

export default PollApp;
