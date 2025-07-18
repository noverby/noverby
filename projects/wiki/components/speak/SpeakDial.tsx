import { SpeedDial, SpeedDialAction, SpeedDialIcon } from "@mui/material";
import { useUserDisplayName } from "@nhost/nextjs";
import { avatars } from "comps";
import { type Node, useSession } from "hooks";
import { useState } from "react";

const SpeakDial = ({ node }: { node: Node }) => {
	const [session] = useSession();
	const displayName = useUserDisplayName();
	const [open, setOpen] = useState(false);

	const get = node.useSubsGet();
	const speakerlist = get("speakerlist");
	const insert = node.useInsert({ refetch: false });
	const id = speakerlist?.id;

	const handleAddSpeak = (type: string) => () => {
		setOpen(false);
		const time = new Date(
			Date.now() + (session?.timeDiff ?? 0),
		).toLocaleString();
		insert({
			name: displayName,
			parentId: id,
			key: `${displayName?.toLocaleLowerCase()}-${time}`,
			mimeId: "speak/speak",
			data: type,
		});
	};

	return (
		<SpeedDial
			ariaLabel="Kom på talerlisten"
			sx={{
				position: "fixed",
				bottom: (t) => t.spacing(9),
				right: (t) => t.spacing(3),
			}}
			icon={<SpeedDialIcon />}
			onOpen={() => setOpen(true)}
			onClose={() => setOpen(false)}
			open={open}
		>
			{Object.entries(avatars).map(
				([key, action]) =>
					(speakerlist?.mutable || key !== "0") && (
						<SpeedDialAction
							key={key}
							icon={action.avatar}
							tooltipTitle={action.name}
							tooltipOpen
							onClick={handleAddSpeak(key)}
						/>
					),
			)}
		</SpeedDial>
	);
};

export default SpeakDial;
