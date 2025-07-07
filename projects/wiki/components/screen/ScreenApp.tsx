import { Grid2 } from "@mui/material";
import { Box } from "@mui/system";
import { MimeLoader, SpeakApp } from "comps";
import type { Node } from "hooks";

const ScreenApp = ({ node }: { node: Node }) => {
	const get = node.useSubsGet();
	const content = get("active");
	const id = content?.id;
	const mimeId = content?.mimeId;
	return (
		<Box sx={{ height: "100%", m: 1 }}>
			<Grid2
				container
				alignItems="stretch"
				justifyContent="space-evenly"
				spacing={1}
			>
				<Grid2>{id && <MimeLoader id={id} mimeId={mimeId!} />}</Grid2>
				<Grid2 size={{ xs: 3 }}>
					<SpeakApp node={node} />
				</Grid2>
			</Grid2>
		</Box>
	);
};

export default ScreenApp;
