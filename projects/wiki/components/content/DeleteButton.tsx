import { Delete } from "@mui/icons-material";
import { Button, Dialog, DialogActions, DialogTitle } from "@mui/material";
import { AutoButton } from "comps";
import { type Node, useLink } from "hooks";
import { useState } from "react";

const DeleteButton = ({ node }: { node: Node }) => {
	const [open, setOpen] = useState(false);
	const link = useLink();
	const $delete = node.useDelete();
	const members = node.useMembers();

	const handleDelete = async () => {
		await members.delete();
		await $delete();
		link.pop();
	};

	return (
		<>
			<AutoButton
				key="delete"
				text="Slet"
				icon={<Delete />}
				onClick={() => setOpen(true)}
			/>
			<Dialog open={open} onClose={() => setOpen(false)}>
				<DialogTitle>Bekræft Sletning</DialogTitle>
				<DialogActions>
					<Button
						endIcon={<Delete />}
						variant="contained"
						color="primary"
						onClick={handleDelete}
					>
						Slet
					</Button>
				</DialogActions>
			</Dialog>
		</>
	);
};

export default DeleteButton;
