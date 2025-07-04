import { SupervisorAccount } from "@mui/icons-material";
import { Avatar, Card, CardHeader, Typography } from "@mui/material";
import type React from "react";

const AdminCard = ({
	children,
	title,
}: {
	children: React.ReactNode | React.ReactNode[];
	title: string;
}) => (
	<Card sx={{ m: 0, bgcolor: "primary.main" }}>
		<CardHeader
			title={<Typography variant="h5">{title}</Typography>}
			avatar={
				<Avatar
					sx={{
						bgcolor: "secondary.main",
					}}
				>
					<SupervisorAccount />
				</Avatar>
			}
		/>
		{children}
	</Card>
);

export default AdminCard;
