import { MarkEmailRead } from "@mui/icons-material";
import { Avatar, CardContent, Typography } from "@mui/material";
import { Container, Stack } from "@mui/system";
import { useAuthenticationStatus } from "@nhost/react";
import { AuthForm, HeaderCard } from "comps";
import { useTranslation } from "react-i18next";

const Reset = () => {
	const { t } = useTranslation();
	const { isAuthenticated } = useAuthenticationStatus();
	if (!isAuthenticated) {
		return (
			<Container>
				<HeaderCard
					title={t("auth.checkEmail")}
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
						<Stack spacing={1.5}>
							<Typography>{t("auth.passwordResetSent")}</Typography>
							<Typography>{t("auth.useToResetPassword")}</Typography>
							<Typography>{t("auth.checkSpam")}</Typography>
						</Stack>
					</CardContent>
				</HeaderCard>
			</Container>
		);
	}

	return <AuthForm mode="set-password" />;
};

export default Reset;
