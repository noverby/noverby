import { LockOpen } from "@mui/icons-material";
import { Badge, Avatar as MuiAvatar, Skeleton, Tooltip } from "@mui/material";
import type { Maybe } from "gql";
import { withSuspense } from "hoc";
import { type Node, useNode } from "hooks";
import { IconId } from "mime";

const MimeAvatar = ({
	mimeId,
	index,
	name,
	child,
}: {
	mimeId: Maybe<string | undefined>;
	index?: number;
	name?: string;
	child?: boolean;
}) => {
	return (
		<MuiAvatar
			sx={{
				bgcolor: child ? "secondary.main" : "primary.main",
			}}
		>
			<IconId name={name} mimeId={mimeId} index={index} avatar child={child} />
		</MuiAvatar>
	);
};

const Avatar = ({ node }: { node: Node }) => {
	const query = node?.useQuery();
	const type = query?.data?.({ path: "type" });
	const mimeId = query?.mimeId;
	const id = type ?? mimeId;
	const name = query?.name;
	const index = query?.getIndex;

	if (id === undefined) {
		return (
			<MuiAvatar
				sx={{
					bgcolor: "primary.main",
				}}
			>
				{" "}
			</MuiAvatar>
		);
	}

	const avatar = (
		<MuiAvatar
			sx={{
				bgcolor: "primary.main",
			}}
		>
			<IconId
				name={name}
				mimeId={id}
				index={index ? index - 1 : undefined}
				avatar
			/>
		</MuiAvatar>
	);
	return query?.mutable ? (
		<Badge
			overlap="circular"
			anchorOrigin={{
				vertical: "bottom",
				horizontal: "right",
			}}
			badgeContent={
				<Tooltip title="Ikke indsendt">
					<MuiAvatar
						sx={{
							width: 18,
							height: 18,
							bgcolor: "primary.main",
						}}
					>
						<LockOpen
							sx={{
								width: 14,
								height: 14,
								color: "#fff",
							}}
						/>
					</MuiAvatar>
				</Tooltip>
			}
		>
			{avatar}
		</Badge>
	) : (
		avatar
	);
};

const MimeAvatarNode = withSuspense(
	Avatar,
	<Skeleton variant="circular" width={24} height={24} />,
);
const MimeAvatarId = withSuspense(
	({ id, ...props }: { id: string }) => {
		const node = useNode({ id });
		return <Avatar node={node} {...props} />;
	},
	<Skeleton variant="circular" width={24} height={24} />,
);

export { MimeAvatar, MimeAvatarNode, MimeAvatarId };
