import { Event, Group, Subject } from "@mui/icons-material";
import {
	Avatar,
	List,
	ListItem,
	ListItemAvatar,
	ListItemButton,
	ListItemText,
} from "@mui/material";
import { HeaderCard, Link } from "comps";
import { useQuery } from "gql";
import type { Node } from "hooks";
import { IconId } from "mime";
import { useTranslation } from "react-i18next";

const UserApp = ({ node }: { node: Node }) => {
	const { t } = useTranslation();
	const query = useQuery();
	const nodes = query.nodes({
		where: {
			_or: [{ ownerId: { _eq: node?.id } }],
		},
	});
	return (
		<>
			<HeaderCard title={t("layout.memberships")} avatar={<Group />}>
				<List>
					{query
						?.members({
							where: {
								_and: [
									{ nodeId: { _eq: node?.id } },
									{ parent: { mimeId: { _eq: "wiki/group" } } },
								],
							},
						})
						.map(({ parent, node }) => (
							<ListItemButton
								key={node?.id ?? 0}
								component={Link}
								href={parent?.id ?? ""}
							>
								<ListItemText primary={parent?.name} />
							</ListItemButton>
						)) ?? (
						<ListItem>
							<ListItemText primary={t("layout.noMemberships")} />
						</ListItem>
					)}
				</List>
			</HeaderCard>
			<HeaderCard title={t("layout.events")} avatar={<Event />}>
				<List>
					{query
						?.members({
							where: {
								_and: [
									{ nodeId: { _eq: node?.id } },
									{ parent: { mimeId: { _eq: "wiki/event" } } },
								],
							},
						})
						.map(({ parent, node }) => (
							<ListItemButton
								key={node?.id ?? 0}
								component={Link}
								href={parent?.id ?? ""}
							>
								<ListItemText primary={parent?.name} />
							</ListItemButton>
						)) ?? (
						<ListItem>
							<ListItemText primary={t("layout.noMemberships")} />
						</ListItem>
					)}
				</List>
			</HeaderCard>
			<HeaderCard title={t("layout.content")} avatar={<Subject />}>
				<List>
					{nodes?.map(({ id, name, mimeId, parent }) => (
						<ListItemButton key={id} component={Link} href={id ?? ""}>
							<ListItemAvatar>
								<Avatar
									sx={{
										bgcolor: "secondary.main",
									}}
								>
									<IconId mimeId={mimeId} />
								</Avatar>
							</ListItemAvatar>
							<ListItemText primary={name} secondary={parent?.name} />
						</ListItemButton>
					)) ?? (
						<ListItem>
							<ListItemText primary={t("common.noContent")} />
						</ListItem>
					)}
				</List>
			</HeaderCard>
		</>
	);
};

export default UserApp;
