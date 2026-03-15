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
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";

const AddContentFabSuspense = () => {
	const node = useNode();
	return <AddContentFab node={node} />;
};

const HomeApp = () => {
	const { t } = useTranslation();
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
			<Grid size={{ xs: 12, lg: isAuthenticated ? 8 : 12 }}>
				{isAuthenticated && !largeScreen ? (
					<Card>
						<HomeList />
					</Card>
				) : (
					<HeaderCard
						title={t("layout.welcomeTitle")}
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
									<Typography>{t("layout.loginOrRegister")}</Typography>
									<Typography>{t("layout.rememberEmail")}</Typography>
									<Stack direction="row">
										<Button
											startIcon={<Login />}
											sx={{ mt: 1 }}
											variant="outlined"
											onClick={() => navigate("/user/login")}
										>
											{t("common.logIn")}
										</Button>
										<Box sx={{ p: 1 }} />
										<Button
											startIcon={<HowToReg />}
											sx={{ mt: 1 }}
											variant="outlined"
											onClick={() => navigate("/user/register")}
										>
											{t("auth.register")}
										</Button>
									</Stack>
								</>
							)) || (
								<>
									<Typography sx={{ mb: 1 }}>
										{t("layout.greeting", { name: displayName })}
									</Typography>
									<Typography sx={{ mb: 1 }}>
										{t("layout.acceptInvitations")}
									</Typography>
									<Typography>{t("layout.noInvitationsHint")}</Typography>
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
