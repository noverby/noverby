import { Publish } from "@mui/icons-material";
import {
	Button,
	Dialog,
	DialogActions,
	DialogContent,
	DialogTitle,
} from "@mui/material";
import { AutoButton } from "comps";
import type { Node } from "hooks";
import { useState } from "react";

const PublishButton = ({
	node,
	handlePublish,
}: {
	node: Node;
	handlePublish?: () => Promise<void>;
}) => {
	const [open, setOpen] = useState(false);
	const update = node.useUpdate();
	const query = node.useQuery();

	const handler =
		handlePublish ??
		(() => {
			update({ set: { mutable: false } });
		});

	if (!query?.mutable) return null;

	return (
		<>
			<AutoButton
				key="sent"
				text="Indsend"
				icon={<Publish />}
				onClick={() => setOpen(true)}
			/>
			<Dialog open={open} onClose={() => setOpen(false)}>
				<DialogTitle>Bekræft Indsendelse</DialogTitle>
				<DialogContent>
					Når du har indsendt, så er det ikke muligt at redigere mere.
				</DialogContent>
				<DialogActions>
					<Button
						endIcon={<Publish />}
						variant="contained"
						color="primary"
						onClick={async () => {
							await handler();
							setOpen(false);
						}}
					>
						Indsend
					</Button>
				</DialogActions>
			</Dialog>
		</>
	);
};

export default PublishButton;
