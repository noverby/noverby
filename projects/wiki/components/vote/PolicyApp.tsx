import { Stack } from "@mui/material";
import {
	ChangeList,
	CommentList,
	ContentApp,
	FolderDial,
	PollList,
} from "comps";
import type { Node } from "hooks";
import { Suspense } from "react";

const PolicyApp = ({ node }: { node: Node }) => (
	<Stack spacing={1}>
		<ContentApp node={node} />
		<Suspense fallback={null}>
			<CommentList node={node} />
		</Suspense>
		<Suspense fallback={null}>
			<ChangeList node={node} />
		</Suspense>
		<PollList node={node} />
		<Suspense fallback={null}>
			<FolderDial node={node} />
		</Suspense>
	</Stack>
);

export default PolicyApp;
