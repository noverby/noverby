import { Button, IconButton, Tooltip, useMediaQuery } from "@mui/material";
import type React from "react";
import type { MouseEventHandler } from "react";

const AutoButton = ({
	text,
	icon,
	onClick,
}: {
	text: string;
	icon: React.ReactElement;
	onClick: MouseEventHandler<HTMLButtonElement>;
}) => {
	const largeScreen = useMediaQuery("(min-width:1200px)");

	return largeScreen ? (
		<Button color="primary" variant="outlined" endIcon={icon} onClick={onClick}>
			{text}
		</Button>
	) : (
		<IconButton
			aria-label={text}
			color="primary"
			onClick={onClick}
			size="large"
		>
			<Tooltip title={text}>{icon}</Tooltip>
		</IconButton>
	);
};

export default AutoButton;
