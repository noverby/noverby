import { Face } from "@mui/icons-material";
import { Chip, Collapse, Grid, Tooltip, Typography } from "@mui/material";
import { type Node, useScreen } from "hooks";
import { IconId } from "mime";
import { useTranslation } from "react-i18next";

const MemberChips = ({ node, child }: { node: Node; child?: boolean }) => {
	const { t } = useTranslation();
	const screen = useScreen();
	const members = node.useQuery()?.members();
	const chips =
		members?.map(({ id, name, node, user }) => (
			<Grid key={id ?? 0}>
				<Tooltip title={t("member.author")}>
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
			</Grid>
		)) ?? [];
	return (
		<Collapse in={!!members?.[0]?.id && members?.length !== 0}>
			<Grid container spacing={0.5} sx={{ ml: child ? 0 : 1 }}>
				{chips}
			</Grid>
		</Collapse>
	);
};

export default MemberChips;
