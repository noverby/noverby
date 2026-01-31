import { MarkEmailRead } from "@mui/icons-material";
import { Avatar, CardContent, Container, Typography } from "@mui/material";
import { useAuthenticationStatus } from "@nhost/react";
import { HeaderCard } from "comps";
import { startTransition, useEffect } from "react";
import { useNavigate } from "react-router-dom";

const Unverified = () => {
	const navigate = useNavigate();
	const { isAuthenticated } = useAuthenticationStatus();

	useEffect(() => {
		if (isAuthenticated) {
			startTransition(() => {
				navigate("/");
			});
		}
	}, [isAuthenticated, navigate]);

	return (
		<Container>
			<HeaderCard
				title="Verificer din email"
				avatar={
					<Avatar
						sx={{
							bgcolor: "secondary.main",
						}}
					>
						<MarkEmailRead />
					</Avatar>
				}
			>
				<CardContent>
					<Typography>
						Du skulle gerne have modtaget en verifications email.
					</Typography>
					<Typography>Brug den til at aktivere din bruger.</Typography>
					<Typography>Tjek eventuelt om emailen er endt i spam.</Typography>
				</CardContent>
			</HeaderCard>
		</Container>
	);
};

export default Unverified;
