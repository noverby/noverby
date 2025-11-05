import { Hail, HowToReg, Login } from "@mui/icons-material";
import {
	Avatar,
	Box,
	Button,
	Card,
	CardContent,
	Grid,
	Stack,
	Typography,
	useMediaQuery,
} from "@mui/material";
import { useAuthenticationStatus, useUserDisplayName } from "@nhost/react";
import { AddContentFab, HeaderCard, HomeList, InvitesUserList } from "comps";
import { useNode } from "hooks";
import { Suspense } from "react";
import { useNavigate } from "react-router-dom";

const AddContentFabSuspense = () => {
	const node = useNode();
	return <AddContentFab node={node} />;
};

const HomeApp = () => {
	const navigate = useNavigate();
	const { isAuthenticated } = useAuthenticationStatus();
	const displayName = useUserDisplayName();
	const largeScreen = useMediaQuery("(min-width:1200px)");

	return (
		<Grid direction={largeScreen ? "row-reverse" : "row"} container spacing={1}>
			{isAuthenticated && (
				<Grid size={{ xs: 12, lg: 4 }}>
					<InvitesUserList />
					<Suspense fallback={null}>
						<AddContentFabSuspense />
					</Suspense>
				</Grid>
			)}
			<Grid size={{ xs: 12, lg: 8 }}>
				{isAuthenticated && !largeScreen ? (
					<Card>
						<HomeList />
					</Card>
				) : (
					<HeaderCard
						title="Velkommen til RadikalWiki"
						avatar={
							<Avatar
								sx={{
									bgcolor: "primary.main",
								}}
							>
								<Hail />
							</Avatar>
						}
					>
						<CardContent>
							{(!isAuthenticated && (
								<>
									<Typography>Log ind eller registrer dig.</Typography>
									<Typography>
										Husk at bruge den email, som du registrerede dig med hos RU.
									</Typography>
									<Stack direction="row">
										<Button
											startIcon={<Login />}
											sx={{ mt: 1 }}
											variant="outlined"
											onClick={() => navigate("/user/login")}
										>
											Log ind
										</Button>
										<Box sx={{ p: 1 }} />
										<Button
											startIcon={<HowToReg />}
											sx={{ mt: 1 }}
											variant="outlined"
											onClick={() => navigate("/user/register")}
										>
											Registrer
										</Button>
									</Stack>
								</>
							)) || (
								<>
									<Typography sx={{ mb: 1 }}>Hej {displayName}!</Typography>
									<Typography sx={{ mb: 1 }}>
										Accepter venligst dine invitationer til grupper og
										begivenheder.
									</Typography>
									<Typography>
										Hvis der ikke forekommer nogen invitationer, så har du højst
										sandsynligt brugt en anden email, end den som er registreret
										ved Radikal Ungdom.
									</Typography>
								</>
							)}
						</CardContent>
					</HeaderCard>
				)}
			</Grid>
		</Grid>
	);
};

export default HomeApp;
