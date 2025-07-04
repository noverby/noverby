import { Add } from "@mui/icons-material";
import { Fab, useScrollTrigger, Zoom } from "@mui/material";
import { AddContentDialog } from "comps";
import { type Node, useScreen } from "hooks";
import { useState } from "react";

const AddContentFab = ({ node }: { node: Node }) => {
	const trigger = useScrollTrigger({
		target:
			typeof document !== "undefined"
				? document.querySelector("#scroll") || undefined
				: undefined,
		disableHysteresis: true,
		threshold: 100,
	});
	const screen = useScreen();
	const [open, setOpen] = useState(false);
	const query = node.useQuery();
	const mimes =
		query
			?.inserts({
				where: {
					_or: [{ context: { _eq: true } }, { hidden: { _eq: false } }],
				},
			})
			?.map((mime) => mime.id!) ?? [];

	if (screen || !query?.attachable || !mimes?.[0]) return null;

	return (
		<>
			<Zoom in={true}>
				<Fab
					sx={{
						position: "fixed",
						bottom: (t) => t.spacing(16),
						right: (t) => t.spacing(3),
					}}
					variant={!trigger ? "extended" : "circular"}
					color="primary"
					aria-label="Tilføj indhold"
					onClick={() => setOpen(true)}
				>
					<Add sx={{ mr: !trigger ? 1 : 0 }} />
					{!trigger && "Tilføj"}
				</Fab>
			</Zoom>
			<AddContentDialog
				node={node}
				open={open}
				setOpen={setOpen}
				mimes={mimes}
				redirect
			/>
		</>
	);
};

export default AddContentFab;
