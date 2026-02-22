import { Login, QuestionMark } from "@mui/icons-material";
import { Avatar, Button, CardContent, Grid, Typography } from "@mui/material";
import { useAuthenticationStatus } from "@nhost/react";
import { HeaderCard } from "comps";
import { useNavigate } from "react-router-dom";

const UnknownApp = () => {
	const navigate = useNavigate();
	const { isAuthenticated } = useAuthenticationStatus();

	return (
		<Grid container spacing={1}>
			<Grid size={{ xs: 12 }}>
				<HeaderCard
					title="Dokumentet er ikke tilgængelig"
					avatar={
						<Avatar
							sx={{
								bgcolor: "secondary.main",
							}}
						>
							<QuestionMark />
						</Avatar>
					}
				>
					<CardContent>
						<Typography sx={{ mb: 1 }}>
							Dokumentet er ikke tilgængelig.
						</Typography>
						<Typography sx={{ mb: isAuthenticated ? 0 : 1 }}>
							Det kan skyldes, at dokumentet ikke findes, eller at du ikke har
							adgang til det.
						</Typography>
						{!isAuthenticated && (
							<>
								<Typography>
									Du kan måske få adgang til dokumentet ved at logge ind:
								</Typography>
								<Button
									startIcon={<Login />}
									sx={{ mt: 1 }}
									variant="outlined"
									onClick={() => navigate("/user/login")}
								>
									Log ind
								</Button>
							</>
						)}
					</CardContent>
				</HeaderCard>
			</Grid>
		</Grid>
	);
};

export default UnknownApp;
