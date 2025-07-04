import { Save } from "@mui/icons-material";
import { ButtonGroup, Card, CardContent, Grid, TextField } from "@mui/material";
import { Stack } from "@mui/system";
import { DatePicker } from "@mui/x-date-pickers";
import {
	AuthorTextField,
	AutoButton,
	DeleteButton,
	FileUploader,
	Image,
	PublishButton,
	Slate,
} from "comps";
import type { CustomElement } from "core/types/slate";
import { parseISO } from "date-fns";
import { resolve } from "gql";
import { type Node, useFile, useLink } from "hooks";
import { startTransition, useEffect, useState } from "react";
import type { Descendant } from "slate";

const Editor = ({ node }: { node: Node }) => {
	const link = useLink();
	const query = node.useQuery();
	const update = node.useUpdate();
	const nodeMembers = node.useMembers();
	const data = query?.data();
	const [fileId, setFileId] = useState<string | undefined>();
	const image = useFile({ fileId: fileId ?? data?.image, image: true });

	const [name, setName] = useState("");
	const [date, setDate] = useState<Date | null>(null);
	const [members, setMembers] = useState<
		{ nodeId?: string; name?: string; email?: string }[]
	>([]);
	const [content, setContent] = useState<Descendant[]>([]);
	const [authorError, setAuthorError] = useState<string | undefined>();

	useEffect(() => {
		startTransition(() => {
			if (!["wiki/group", "wiki/event"].includes(query?.mimeId!)) {
				const fetchMembers = async (id: string) => {
					const members = await resolve(({ query }) =>
						query
							?.node({ id })
							?.members()
							.map((member) => {
								return {
									nodeId: member.nodeId!,
									name: member.name!,
									email: member.email!,
									//mimeId: member.node?.mimeId,
								};
							}),
					);
					if (members?.[0]?.nodeId || members?.[0]?.email || members?.[0]?.name)
						setMembers(members);
				};
				if (node.id) {
					fetchMembers(node.id);
				}
			}
			const fetch = async () => {
				setName(query?.name ?? "");
				setDate(parseISO(query?.createdAt ?? ""));
				setContent(structuredClone(data?.content));
			};
			fetch();
		});
	}, [query?.name, query?.createdAt, JSON.stringify(data?.content)]);

	const handleSave = (mutable?: boolean) => async () => {
		if (
			![
				"wiki/group",
				"wiki/event",
				"vote/position",
				"vote/candidate",
				"wiki/folder",
			].includes(query?.mimeId!)
		) {
			if (members.length === 0) {
				setAuthorError("Tilføj mindst 1 forfatter");
				return;
			}
			await nodeMembers.delete();
			await nodeMembers.insert({
				members: members.map((member) => ({ ...member, mimeId: undefined })),
			});
		}
		const newContent =
			content?.length >= 1 &&
			(content[0] as CustomElement).children?.[0].text === ""
				? content.slice(1)
				: content;

		await update({
			set: {
				name,
				data: { content: newContent, image: fileId },
				mutable,
				createdAt: date?.toISOString(),
			},
		});
		link.push([]);
	};

	return (
		<>
			<Card sx={{ m: 0 }}>
				<CardContent>
					<Grid container spacing={2}>
						<Grid item xs={12}>
							<Stack spacing={2} direction={"row"} alignItems="center">
								<TextField
									value={name}
									onChange={(e) => setName(e.target.value)}
									label="Titel"
									variant="outlined"
									fullWidth
									multiline
								/>
								<ButtonGroup>
									<DeleteButton node={node} />
									<AutoButton
										text="Gem"
										icon={<Save />}
										onClick={handleSave()}
									/>
									<PublishButton
										node={node}
										handlePublish={handleSave(false)}
									/>
								</ButtonGroup>
							</Stack>
						</Grid>
						<Grid item xs={12}>
							<Stack spacing={2} direction={"row"} alignItems="center">
								{![
									"wiki/group",
									"wiki/event",
									"vote/position",
									"vote/candidate",
								].includes(query?.mimeId!) && (
									<AuthorTextField
										value={members}
										onChange={setMembers}
										setAuthorError={setAuthorError}
										authorError={authorError}
									/>
								)}

								{query?.isContextOwner && (
									<DatePicker value={date} onChange={setDate} />
								)}
							</Stack>
						</Grid>
						<Grid item>
							<FileUploader
								text="Upload Billede"
								onNewFile={({ fileId }: { fileId?: string }) => {
									fileId && setFileId(fileId);
								}}
							/>
						</Grid>

						<Grid item xs={12}>
							<Grid container>
								{image && (
									<Grid item xs={3}>
										<Image alt="Billede for indhold" src={image} />
									</Grid>
								)}
							</Grid>
						</Grid>
					</Grid>
				</CardContent>
				<Slate
					value={content}
					onChange={(value) => setContent(structuredClone(value))}
					readOnly={false}
				/>
			</Card>
		</>
	);
};

export default Editor;
