import { Public, Save } from "@mui/icons-material";
import { Avatar, Box, Fab, SpeedDial, SpeedDialAction } from "@mui/material";
import type { Node } from "hooks";
import { useState } from "react";

const EditorDial = ({
	node,
	handleSave,
}: {
	node: Node;
	handleSave: (isPublic: boolean) => void;
}) => {
	const [open, setOpen] = useState(false);
	const query = node.useQuery();

	const editable = query?.mutable && query?.isOwner;
	return (
		<Box display="flex" justifyContent="flex-end">
			{editable ? (
				<SpeedDial
					ariaLabel="Gem eller indsend"
					sx={{
						bottom: (t) => t.spacing(10),
						position: "absolute",
					}}
					icon={<Save />}
					onOpen={() => setOpen(true)}
					onClose={() => setOpen(false)}
					open={open}
				>
					<SpeedDialAction
						icon={<Avatar sx={{ bgcolor: "primary.main" }}>{<Save />}</Avatar>}
						tooltipTitle="Gem"
						tooltipOpen
						onClick={() => handleSave(false)}
					/>
					<SpeedDialAction
						icon={
							<Avatar sx={{ bgcolor: "primary.main" }}>{<Public />}</Avatar>
						}
						tooltipTitle="Indsend"
						tooltipOpen
						onClick={() => handleSave(true)}
					/>
				</SpeedDial>
			) : (
				<Fab
					sx={{
						bottom: (t) => t.spacing(10),
						position: "absolute",
						bgcolor: "primary.main",
						color: (t) => t.palette.primary.contrastText,
					}}
					onClick={() => handleSave(true)}
				>
					<Save />
				</Fab>
			)}
		</Box>
	);
};

export default EditorDial;
