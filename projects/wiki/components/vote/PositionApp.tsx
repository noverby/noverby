import { Stack } from "@mui/material";
import {
	CandidateList,
	ContentApp,
	FolderDial,
	PollList,
	QuestionList,
} from "comps";
import type { Node } from "hooks";

const PositionApp = ({ node }: { node: Node }) => (
	<Stack spacing={1}>
		<ContentApp node={node} add />
		<CandidateList node={node} />
		<QuestionList node={node} />
		<PollList node={node} />
		<FolderDial node={node} />
	</Stack>
);

export default PositionApp;
