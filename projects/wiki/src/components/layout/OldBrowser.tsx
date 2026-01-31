import { BrowserNotSupported } from "@mui/icons-material";
import { Avatar, CardContent, Typography } from "@mui/material";
import { Container } from "@mui/system";
import { HeaderCard } from "comps";
import platform from "platform";

const OldBrowser = () => (
	<Container>
		<HeaderCard
			title="Forældet Browser"
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
					{`Din browser "${platform}" er ikke understøttet af RadikalWiki.`}
				</Typography>
				<Typography sx={{ mb: 1 }}>
					Opdater venligst din browser, eller installer en understøttet browser.
				</Typography>
				<Typography sx={{ mb: 1, fontWeight: "bold" }}>
					OBS: På iOS er det kun muligt at bruge Safari. (Opdater Safari!)
				</Typography>
				<Typography sx={{ fontStyle: "italic", mb: 1 }}>
					Chrome og Firefox er blot et skin over Safari på iOS
				</Typography>
			</CardContent>
		</HeaderCard>
	</Container>
);

export default OldBrowser;
