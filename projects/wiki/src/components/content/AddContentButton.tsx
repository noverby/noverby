import { Add } from "@mui/icons-material";
import { AddContentDialog, AutoButton } from "comps";
import { type Node, useScreen } from "hooks";
import { useState } from "react";

const AddContentButton = ({ node }: { node: Node }) => {
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

	if (screen || (!query?.isContextOwner && !query?.attachable) || !mimes?.[0])
		return null;

	return (
		<>
			<AutoButton
				key="add"
				text="Tilføj"
				icon={<Add />}
				onClick={() => setOpen(true)}
			/>
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

export default AddContentButton;
