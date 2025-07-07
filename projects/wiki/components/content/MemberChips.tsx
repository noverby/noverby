import { Face } from "@mui/icons-material";
import { Chip, Collapse, Grid2, Tooltip, Typography } from "@mui/material";
import { type Node, useScreen } from "hooks";
import { IconId } from "mime";

const MemberChips = ({ node, child }: { node: Node; child?: boolean }) => {
	const screen = useScreen();
	const members = node.useQuery()?.members();
	const chips =
		members?.map(({ id, name, node, user }) => (
			<Grid2 key={id ?? 0}>
				<Tooltip title="Forfatter">
					<Chip
						icon={node?.mimeId ? <IconId mimeId={node?.mimeId} /> : <Face />}
						size={screen ? "medium" : "small"}
						color="secondary"
						variant="outlined"
						label={
							<Typography variant={screen ? "h5" : undefined}>
								{name ?? user?.displayName}
							</Typography>
						}
					/>
				</Tooltip>
			</Grid2>
		)) ?? [];
	return (
		<Collapse in={!!members?.[0]?.id && members?.length !== 0}>
			<Grid2 container spacing={0.5} sx={{ ml: child ? 0 : 1 }}>
				{chips}
			</Grid2>
		</Collapse>
	);
};

export default MemberChips;
