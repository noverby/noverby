import {
	Announcement,
	EmojiPeople,
	LiveHelp,
	PanTool,
	RecordVoiceOver,
} from "@mui/icons-material";
import { Avatar } from "@mui/material";
import { green, orange, red, yellow } from "@mui/material/colors";
import i18n from "i18n";

const color = "#fff";

type AvatarType = {
	nameKey: string;
	priority: number;
	avatar: React.ReactNode;
};

const avatars: { [id: string]: AvatarType } = {
	0: {
		nameKey: "speak.talk",
		priority: 0,
		avatar: (
			<Avatar
				sx={{
					color,
					backgroundColor: "#303f9f",
				}}
			>
				<RecordVoiceOver />
			</Avatar>
		),
	},
	1: {
		nameKey: "speak.question",
		priority: 1,
		avatar: (
			<Avatar
				sx={{
					color,
					backgroundColor: yellow[700],
				}}
			>
				<LiveHelp />
			</Avatar>
		),
	},
	2: {
		nameKey: "speak.clarify",
		priority: 2,
		avatar: (
			<Avatar
				sx={{
					color,
					backgroundColor: green[700],
				}}
			>
				<EmojiPeople />
			</Avatar>
		),
	},
	3: {
		nameKey: "speak.misunderstood",
		priority: 3,
		avatar: (
			<Avatar
				sx={{
					color,
					backgroundColor: orange[700],
				}}
			>
				<Announcement />
			</Avatar>
		),
	},
	4: {
		nameKey: "speak.procedure",
		priority: 4,
		avatar: (
			<Avatar
				sx={{
					color,
					backgroundColor: red[700],
				}}
			>
				<PanTool />
			</Avatar>
		),
	},
};

const getAvatarName = (id: string): string =>
	i18n.t(avatars[id]?.nameKey ?? "");

export { getAvatarName };
export default avatars;
