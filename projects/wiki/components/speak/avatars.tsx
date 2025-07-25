import {
	Announcement,
	EmojiPeople,
	LiveHelp,
	PanTool,
	RecordVoiceOver,
} from "@mui/icons-material";
import { Avatar } from "@mui/material";
import { green, orange, red, yellow } from "@mui/material/colors";

const color = "#fff";

type AvatarType = {
	name: string;
	priority: number;
	avatar: React.ReactNode;
};

const avatars: { [id: string]: AvatarType } = {
	0: {
		name: "Tal",
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
		name: "Spørgsmål",
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
		name: "Opklar",
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
		name: "Misforstået",
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
		name: "Procedure",
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

export default avatars;
