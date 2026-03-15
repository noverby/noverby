import { CardHeader, Skeleton, Tooltip, Typography } from "@mui/material";
import { Stack } from "@mui/system";
import { ContentToolbar, MemberChips, MimeAvatar, MimeAvatarNode } from "comps";
import formatDistance from "date-fns/formatDistance";
import { da, enUS } from "date-fns/locale";
import { type Node, useScreen } from "hooks";
import { Suspense } from "react";
import { useTranslation } from "react-i18next";

const Title = ({ node }: { node: Node }) => {
	const query = node.useQuery();
	const screen = useScreen();
	return query?.name ? (
		<Stack>
			<Typography variant={screen ? "h5" : "body1"} sx={{ color: "inherit" }}>
				{query?.name}
			</Typography>
		</Stack>
	) : null;
};

const Subtitle = ({ node }: { node: Node }) => {
	const { t, i18n } = useTranslation();
	const query = node.useQuery();
	const locale = i18n.language === "da" ? da : enUS;

	return (
		<Tooltip
			title={query?.createdAt && new Date(query?.createdAt).toLocaleString()}
		>
			<Typography
				component="span"
				variant="caption"
				sx={{ color: "common.black" }}
			>
				{query?.createdAt
					? t("content.createdAgo", {
							time: formatDistance(new Date(), new Date(query?.createdAt), {
								locale,
							}),
						})
					: ""}
			</Typography>
		</Tooltip>
	);
};

const ContentHeader = ({
	node,
	hideMembers,
	child,
	add,
}: {
	node: Node;
	hideMembers?: boolean;
	child?: boolean;
	add?: boolean;
}) => {
	const { t } = useTranslation();
	return (
		<>
			<CardHeader
				title={
					child ? (
						t("mime.folder")
					) : (
						<Suspense fallback={<Skeleton width={10} />}>
							<Title node={node} />
						</Suspense>
					)
				}
				subheader={
					child ? undefined : (
						<Suspense fallback={null}>
							<Subtitle node={node} />
						</Suspense>
					)
				}
				avatar={
					child ? (
						<MimeAvatar mimeId="app/folder" />
					) : (
						<MimeAvatarNode node={node} />
					)
				}
				sx={{
					borderRadius: "4px 4px 0px 0px",
				}}
				action={
					<Suspense fallback={null}>
						<ContentToolbar child={child} add={add} node={node} />
					</Suspense>
				}
			/>
			{!hideMembers && (
				<Suspense fallback={null}>
					<MemberChips node={node} />
				</Suspense>
			)}
		</>
	);
};

export default ContentHeader;
