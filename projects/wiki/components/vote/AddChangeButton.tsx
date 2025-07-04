import { PlusOne } from "@mui/icons-material";
import { useUserDisplayName } from "@nhost/nextjs";
import { AddContentDialog, AutoButton } from "comps";
import type { Node } from "hooks";
import { useState } from "react";

const AddChangeButton = ({ node }: { node: Node }) => {
	const displayName = useUserDisplayName();
	const [open, setOpen] = useState(false);
	const query = node.useQuery();

	const name = query?.mimeId === "vote/position" ? displayName : "";

	const handleSubmit = () => {
		setOpen(true);
	};

	if (!query?.inserts()?.some((mime) => mime.id === "vote/change")) return null;

	return (
		<>
			<AutoButton
				text="Nyt Ændringsforslag"
				icon={<PlusOne />}
				onClick={handleSubmit}
			/>
			<AddContentDialog
				initTitel={name}
				node={node}
				mimes={["vote/change"]}
				open={open}
				setOpen={setOpen}
				redirect
				app="editor"
			/>
		</>
	);
};

export default AddChangeButton;
