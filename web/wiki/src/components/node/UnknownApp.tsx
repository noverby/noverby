import { Login, QuestionMark } from "@mui/icons-material";
import { Avatar, Button, CardContent, Grid, Typography } from "@mui/material";
import { useAuthenticationStatus } from "@nhost/react";
import { HeaderCard } from "comps";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";

const UnknownApp = () => {
	const { t } = useTranslation();
	const navigate = useNavigate();
	const { isAuthenticated } = useAuthenticationStatus();

	return (
		<Grid container spacing={1}>
			<Grid size={{ xs: 12 }}>
				<HeaderCard
					title={t("node.documentUnavailable")}
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
							{t("node.documentUnavailable")}
						</Typography>
						<Typography sx={{ mb: isAuthenticated ? 0 : 1 }}>
							{t("node.notFoundOrNoAccess")}
						</Typography>
						{!isAuthenticated && (
							<>
								<Typography>{t("node.maybeLoginForAccess")}</Typography>
								<Button
									startIcon={<Login />}
									sx={{ mt: 1 }}
									variant="outlined"
									onClick={() => navigate("/user/login")}
								>
									{t("common.logIn")}
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
