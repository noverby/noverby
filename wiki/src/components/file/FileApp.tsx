import { Card, Collapse, Stack } from "@mui/material";
import { ChangeList, ContentHeader, FileLoader, QuestionList } from "comps";
import type { Node } from "hooks";

const FileApp = ({ node }: { node: Node }) => {
	return (
		<Stack spacing={1}>
			<Card sx={{ m: 0 }}>
				<ContentHeader node={node} />
				<Collapse in>
					<FileLoader node={node} />
				</Collapse>
			</Card>
			<ChangeList node={node} />
			<QuestionList node={node} />
		</Stack>
	);
};

export default FileApp;
