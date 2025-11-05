import { Stack } from "@mui/material";
import { ContentApp, FolderApp } from "comps";
import type { Node } from "hooks";

const EventApp = ({ node }: { node: Node }) => (
	<Stack spacing={1}>
		<ContentApp node={node} hideMembers />
		<FolderApp node={node} child />
	</Stack>
);

export default EventApp;
