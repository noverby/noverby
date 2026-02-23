import { BrowserNotSupported } from "@mui/icons-material";
import { Avatar, CardContent, Typography } from "@mui/material";
import { Container } from "@mui/system";
import { HeaderCard } from "comps";
import platform from "platform";
import { useTranslation } from "react-i18next";

const OldBrowser = () => {
	const { t } = useTranslation();
	return (
		<Container>
			<HeaderCard
				title={t("browser.outdatedTitle")}
				avatar={
					<Avatar
						sx={{
							bgcolor: "primary.main",
						}}
					>
						<BrowserNotSupported />
					</Avatar>
				}
			>
				<CardContent>
					<Typography sx={{ mb: 1 }}>
						{t("browser.notSupported", { platform })}
					</Typography>
					<Typography sx={{ mb: 1 }}>{t("browser.pleaseUpdate")}</Typography>
					<Typography sx={{ mb: 1, fontWeight: "bold" }}>
						{t("browser.iosNote")}
					</Typography>
					<Typography sx={{ fontStyle: "italic", mb: 1 }}>
						{t("browser.iosSkinNote")}
					</Typography>
				</CardContent>
			</HeaderCard>
		</Container>
	);
};

export default OldBrowser;
