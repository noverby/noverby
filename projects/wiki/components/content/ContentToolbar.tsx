import { Edit, People } from "@mui/icons-material";
import { ButtonGroup, Stack } from "@mui/material";
import {
	AddContentButton,
	AutoButton,
	DeleteButton,
	DownloadButton,
	PublishButton,
} from "comps";
import { type Node, useLink } from "hooks";

const ContentToolbar = ({
	node,
	child,
	add,
}: {
	node: Node;
	child?: boolean;
	add?: boolean;
}) => {
	const query = node.useQuery();
	const link = useLink();

	return (
		<Stack spacing={1} direction="row">
			{!child &&
				["wiki/event", "wiki/group"].includes(query?.mimeId ?? "") &&
				query?.isContextOwner && (
					<AutoButton
						key="member"
						text="Medlemmer"
						icon={<People />}
						onClick={() => link.push([], "member")}
					/>
				)}
			{!child && <DownloadButton node={node} />}
			{!child &&
				((query?.mutable && query?.isOwner) || query?.isContextOwner) && (
					<ButtonGroup>
						<DeleteButton node={node} />
						<AutoButton
							text="Rediger"
							icon={<Edit />}
							onClick={() => link.push([], "editor")}
						/>
						{query?.mutable && <PublishButton node={node} />}
					</ButtonGroup>
				)}
			{add && <AddContentButton node={node} />}
		</Stack>
	);
};

export default ContentToolbar;
