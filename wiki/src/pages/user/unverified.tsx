import { MarkEmailRead } from "@mui/icons-material";
import { Avatar, CardContent, Container, Typography } from "@mui/material";
import { HeaderCard } from "comps";
import { useAuthenticationStatus } from "hooks";
import { startTransition, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";

const Unverified = () => {
	const { t } = useTranslation();
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
				title={t("auth.verifyEmail")}
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
					<Typography>{t("auth.verificationEmailSent")}</Typography>
					<Typography>{t("auth.useToActivate")}</Typography>
					<Typography>{t("auth.checkSpam")}</Typography>
				</CardContent>
			</HeaderCard>
		</Container>
	);
};

export default Unverified;
