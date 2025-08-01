import { Add, ContactMail, DoNotDisturb } from "@mui/icons-material";
import {
	Avatar,
	IconButton,
	List,
	ListItem,
	ListItemAvatar,
	ListItemText,
	Tooltip,
} from "@mui/material";
import { useUserEmail, useUserId } from "@nhost/nextjs";
import { HeaderCard, MimeAvatarId } from "comps";
import {
	client,
	type members_set_input,
	order_by,
	resolve,
	useMutation,
	useSubscription,
} from "gql";
import { startTransition } from "react";

const ListSuspense = () => {
	const sub = useSubscription();
	const userId = useUserId();
	const email = useUserEmail();
	const invites = sub
		.members({
			where: {
				_and: [
					{ accepted: { _eq: false } },
					{
						_or: [{ nodeId: { _eq: userId } }, { email: { _eq: email } }],
					},
					{ parent: { mimeId: { _in: ["wiki/group", "wiki/event"] } } },
				],
			},
		})
		.filter((invite) => invite.parent?.id);
	const events = !userId
		? []
		: sub.nodes({
				order_by: [{ createdAt: order_by.desc }],
				where: {
					_and: [
						{ mimeId: { _eq: "wiki/event" } },
						{
							_or: [
								{ ownerId: { _eq: userId } },
								{
									members: {
										_and: [
											{ accepted: { _eq: true } },
											{ nodeId: { _eq: userId } },
										],
									},
								},
							],
						},
					],
				},
			});
	const groups = !userId
		? []
		: sub.nodes({
				order_by: [{ createdAt: order_by.desc }],
				where: {
					_and: [
						{ mimeId: { _eq: "wiki/group" } },
						{
							_or: [
								{ ownerId: { _eq: userId } },
								{
									members: {
										_and: [
											{ accepted: { _eq: true } },
											{ nodeId: { _eq: userId } },
										],
									},
								},
							],
						},
					],
				},
			});

	const [updateMember] = useMutation(
		(mutation, args: { id?: string; set: members_set_input }) => {
			if (!args.id) return;
			mutation.updateMember({
				pk_columns: { id: args.id },
				_set: args.set,
			})?.id;
		},
		{
			refetchQueries: [invites, events, groups],
			awaitRefetchQueries: true,
		},
	);

	const [deleteMember] = useMutation(
		(mutation, args: { id?: string }) => {
			if (args.id === undefined) return;
			mutation.deleteMember({ id: args.id })?.id;
		},
		{
			refetchQueries: [invites, events, groups],
			awaitRefetchQueries: true,
		},
	);

	const handleAcceptInvite = (id?: string) => () => {
		startTransition(async () => {
			try {
				await updateMember({
					args: { id, set: { accepted: true, nodeId: userId } },
				});
			} catch (_) {
				await deleteMember({ args: { id } });
			}

			// Delete cache
			// eslint-disable-next-line functional/immutable-data
			client.cache.clear();
			await resolve(
				({ query }) =>
					query
						.membersAggregate({
							where: {
								_and: [
									{ accepted: { _eq: false } },
									{
										_or: [
											{ nodeId: { _eq: userId } },
											{ email: { _eq: email } },
										],
									},
								],
							},
						})
						.aggregate?.count(),
				{ cachePolicy: "no-cache" },
			);
		});
	};

	return (
		<List>
			{invites.map(({ id, parent }) => {
				const item = (
					<ListItem key={id ?? 0}>
						{parent?.id && (
							<ListItemAvatar>
								<MimeAvatarId id={parent?.id} />
							</ListItemAvatar>
						)}
						<ListItemText primary={parent?.name} />
						<Tooltip title={`Accepter invitation til ${parent?.name}`}>
							<IconButton onClick={handleAcceptInvite(id)}>
								<Add />
							</IconButton>
						</Tooltip>
					</ListItem>
				);

				return id ? item : null;
			})}
			{!invites?.[0]?.id && (
				<ListItem>
					<ListItemAvatar>
						<Avatar
							sx={{
								bgcolor: "secondary.main",
							}}
						>
							<DoNotDisturb />
						</Avatar>
					</ListItemAvatar>
					<ListItemText primary="Ingen invitationer" />
				</ListItem>
			)}
		</List>
	);
};

const InvitesUserList = () => (
	<HeaderCard
		avatar={
			<Avatar
				sx={{
					bgcolor: "primary.main",
				}}
			>
				<ContactMail />
			</Avatar>
		}
		title="Invitationer"
	>
		<ListSuspense />
	</HeaderCard>
);

export default InvitesUserList;
