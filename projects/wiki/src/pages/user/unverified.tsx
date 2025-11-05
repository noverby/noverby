import { MarkEmailRead } from "@mui/icons-material";
import { Avatar, CardContent, Container, Typography } from "@mui/material";
import { useAuthenticationStatus } from "@nhost/nextjs";
import { HeaderCard } from "comps";
import { useRouter } from "next/router";
import { startTransition, useEffect } from "react";

const Unverified = () => {
	const router = useRouter();
	const { isAuthenticated } = useAuthenticationStatus();

	useEffect(() => {
		if (isAuthenticated) {
			startTransition(() => {
				router.push("/");
			});
		}
	}, [isAuthenticated, router.push]);

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
