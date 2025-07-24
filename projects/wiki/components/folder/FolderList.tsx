import {
	CheckBox,
	CheckBoxOutlineBlankOutlined,
	DoNotDisturb,
} from "@mui/icons-material";
import {
	Avatar,
	Collapse,
	ListItemAvatar,
	ListItemButton,
	ListItemText,
	Tooltip,
	Typography,
} from "@mui/material";
import { useUserId } from "@nhost/nextjs";
import { MimeAvatar } from "comps";
import { order_by } from "gql";
import { type Node, useLink, useSession } from "hooks";
import type { MouseEventHandler } from "react";
import { TransitionGroup } from "react-transition-group";

const FolderList = ({ node }: { node: Node }) => {
	const [session, setSession] = useSession();
	const isOwner = node.useQuery()?.isContextOwner;
	const userId = useUserId();
	const link = useLink();
	const query = node.useQuery();

	const children =
		query?.children({
			order_by: [{ index: order_by.asc }, { createdAt: order_by.asc }],
			where: {
				_and: [
					{
						_or: [
							{ mutable: { _eq: false } },
							{ ownerId: { _eq: userId } },
							{ members: { nodeId: { _eq: userId } } },
						],
					},
					{
						mime: {
							hidden: { _eq: false },
						},
					},
				],
			},
		}) ?? [];

	const handleOnClick = (key?: string) => () => {
		link.push([key!]);
	};

	const number = children.filter((child) => child.mime?.icon === "number");
	const letter = children.filter((child) => child.mime?.icon === "letter");
	const findIndex = (id: string) => {
		const numberIndex = number.findIndex((elem) => elem.id === id);
		if (numberIndex !== -1) return numberIndex;
		const letterIndex = letter.findIndex((elem) => elem.id === id);
		if (letterIndex !== -1) return letterIndex;
		return undefined;
	};

	const handleSelect: (id: string) => MouseEventHandler<HTMLDivElement> =
		(id) => (e) => {
			e.stopPropagation();
			if (session?.selected?.includes(id)) {
				setSession({ selected: session?.selected.filter((cid) => cid !== id) });
			} else {
				setSession({ selected: (session?.selected ?? []).concat([id]) });
			}
		};

	return (
		<TransitionGroup>
			{children.map((child) => {
				const { id, mimeId, name, key } = child;
				const index = findIndex(id!);
				const typeMimeId = child.data({ path: "type" }) ?? mimeId;
				return !id ? null : (
					<Collapse key={id ?? 0}>
						<ListItemButton
							selected={session?.selected?.includes(child.id!)}
							onClick={handleOnClick(key)}
						>
							<ListItemAvatar>
								<MimeAvatar
									mimeId={typeMimeId}
									index={index}
									name={name}
									child
								/>
							</ListItemAvatar>
							<ListItemText primary={<Typography>{name}</Typography>} />
							{isOwner ? (
								<Tooltip title="Kopier" onClick={handleSelect(id)}>
									{session?.selected?.includes(child.id!) ? (
										<CheckBox />
									) : (
										<CheckBoxOutlineBlankOutlined />
									)}
								</Tooltip>
							) : null}
						</ListItemButton>
					</Collapse>
				);
			})}
			{children.length === 0 && (
				<Collapse key={-1}>
					<ListItemButton>
						<ListItemAvatar>
							<Avatar
								sx={{
									bgcolor: "secondary.main",
								}}
							>
								<DoNotDisturb />
							</Avatar>
						</ListItemAvatar>
						<ListItemText primary="Intet indhold" />
					</ListItemButton>
				</Collapse>
			)}
		</TransitionGroup>
	);
};

export default FolderList;
