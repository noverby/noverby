import { PlusOne } from "@mui/icons-material";
import { AddContentDialog, AutoButton } from "comps";
import type { Node } from "hooks";
import { useState } from "react";

const AddQuestionButton = ({ node }: { node: Node }) => {
	const [open, setOpen] = useState(false);
	const query = node.useQuery();

	const handleSubmit = async () => {
		setOpen(true);
	};

	if (!query?.inserts()?.some((mime) => mime.id === "vote/comment"))
		return null;

	return (
		<>
			<AutoButton
				text="Ny Kommentar"
				icon={<PlusOne />}
				onClick={handleSubmit}
			/>
			<AddContentDialog
				mutable={false}
				node={node}
				mimes={["vote/comment"]}
				open={open}
				setOpen={setOpen}
			/>
		</>
	);
};

export default AddQuestionButton;
