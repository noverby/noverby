import { Box, Collapse, Grid2 } from "@mui/material";
import { Image, Slate } from "comps";
import useFile from "core/hooks/useFile";
import type { Node } from "hooks";
import { startTransition, useEffect, useState } from "react";
import type { Descendant } from "slate";

const Content = ({ node, fontSize }: { node: Node; fontSize: string }) => {
	const query = node.useQuery();
	const data = query?.data();
	const image = useFile({ fileId: data?.image, image: true });
	const [content, setContent] = useState<Descendant[]>(
		structuredClone(data?.content),
	);

	useEffect(() => {
		startTransition(() => {
			setContent(structuredClone(data?.content));
		});
	}, [JSON.stringify(data?.content)]);

	return (
		<Grid2 container spacing={2}>
			<Grid2 size={{ xs: 9 }}>
				<Box sx={{ fontSize, overflowX: "auto" }}>
					<Collapse in={!!content}>
						<Slate value={content} readOnly />
					</Collapse>
				</Box>
			</Grid2>
			{image && (
				<Grid2 size={{ xs: 9, sm: 4 }}>
					<Image alt="Billede for indhold" layout="fill" src={image} />
				</Grid2>
			)}
		</Grid2>
	);
};

export default Content;
