import { Cancel, HowToVote } from "@mui/icons-material";
import {
	Avatar,
	Badge,
	IconButton,
	List,
	ListItemAvatar,
	ListItemButton,
	ListItemSecondaryAction,
	ListItemText,
	Tooltip,
} from "@mui/material";
import { HeaderCard } from "comps";
import { type Node, useLink, useScreen } from "hooks";
import { IconId } from "mime";
import { Fragment, Suspense } from "react";

const PollListSuspense = ({ node }: { node: Node }) => {
	const link = useLink();
	const query = node.useQuery();
	const $delete = node.useDelete();
	const polls = query?.children({
		where: { mimeId: { _eq: "vote/poll" } },
	});
	const handleDeletePoll = (id?: string) => () => {
		if (!id) return;
		$delete({ id });
	};

	const owner = query?.isContextOwner;

	const card = (
		<HeaderCard
			avatar={
				<Avatar
					sx={{
						bgcolor: "secondary.main",
					}}
				>
					<IconId mimeId="vote/poll" />
				</Avatar>
			}
			title="Afstemninger"
		>
			<List>
				{polls?.map(({ id, key, children_aggregate, createdAt }) => (
					<Fragment key={id ?? 0}>
						<ListItemButton onClick={() => link.push([key!])}>
							<Tooltip title="Antal stemmer">
								<ListItemAvatar>
									<Badge
										color="primary"
										max={1000}
										overlap="circular"
										badgeContent={
											children_aggregate().aggregate?.count() || "0"
										}
									>
										<Avatar
											sx={{
												bgcolor: "secondary.main",
											}}
										>
											<HowToVote />
										</Avatar>
									</Badge>
								</ListItemAvatar>
							</Tooltip>
							<ListItemText
								primary={`${new Date(createdAt!).toLocaleString("da-DK")}`}
							/>
							{owner && (
								<ListItemSecondaryAction>
									<IconButton
										onClick={handleDeletePoll(id)}
										color="primary"
										edge="end"
										size="large"
									>
										<Cancel />
									</IconButton>
								</ListItemSecondaryAction>
							)}
						</ListItemButton>
					</Fragment>
				))}
			</List>
		</HeaderCard>
	);

	return polls?.[0]?.id ? card : null;
};

const PollList = ({ node }: { node: Node }) => {
	const screen = useScreen();

	if (screen) return null;
	return (
		<Suspense fallback={null}>
			<PollListSuspense node={node} />
		</Suspense>
	);
};

export default PollList;
