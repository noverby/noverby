import { Grid } from "@mui/material";
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
			<Grid
				container
				alignItems="stretch"
				justifyContent="space-evenly"
				spacing={1}
			>
				<Grid>{id && <MimeLoader id={id} mimeId={mimeId!} />}</Grid>
				<Grid size={{ xs: 3 }}>
					<SpeakApp node={node} />
				</Grid>
			</Grid>
		</Box>
	);
};

export default ScreenApp;
