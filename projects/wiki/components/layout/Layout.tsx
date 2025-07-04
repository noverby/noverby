import { Box, Container, useMediaQuery } from "@mui/material";
import {
	useAuthenticationStatus,
	useUserDisplayName,
	useUserEmail,
	useUserId,
} from "@nhost/nextjs";
import {
	AppDrawer,
	BottomBar,
	Drawer,
	MobileMenu,
	OldBrowser,
	Scroll,
} from "comps";
import { checkVersion } from "core/util";
import { usePath, useSession } from "hooks";
import { useRouter } from "next/router";
import type React from "react";
import { Suspense, startTransition, useEffect, useState } from "react";

const Layout = ({ children }: { children: React.ReactElement }) => {
	const [outdated, setOutdated] = useState(false);
	const [showing, setShowing] = useState(false);
	const [openDrawer, setOpenDrawer] = useState(false);
	const [session, setSession] = useSession();
	const router = useRouter();
	const path = usePath();
	const { isLoading } = useAuthenticationStatus();
	const userEmail = useUserEmail();
	const userName = useUserDisplayName();
	const userId = useUserId();
	const largeScreen = useMediaQuery("(min-width:1200px)");

	useEffect(() => {
		startTransition(() => {
			setOutdated(typeof window !== "undefined" && !checkVersion());
			setShowing(true);
		});
	}, []);

	useEffect(() => {
		const registerUser = async () => {
			const { Bugfender } = await import("@bugfender/sdk");
			Bugfender.setDeviceKey("user.id", userId ?? "");
			Bugfender.setDeviceKey("user.email", userEmail ?? "");
			Bugfender.setDeviceKey("user.name", userName ?? "");
		};
		registerUser();
	}, [userId, userEmail, userName]);

	useEffect(() => {
		// if (session !== null && session?.timeDiff === undefined) {
		//   setSession({ timeDiff: 0 });
		//   fetch('/api/time').then((res) =>
		//     res.json().then(({ time }) => {
		//       setSession({
		//         timeDiff: new Date().getTime() - new Date(time).getTime(),
		//       });
		//     })
		//   );
		// }
	}, [session, setSession]);

	useEffect(() => {
		// const checkVersion = () => {
		//   fetch('/api/version').then((res) => {
		//     res.json().then(({ commit }) => {
		//       if (version == undefined) {
		//         setVersion(commit);
		//       } else if (version != commit) {
		//         enqueueSnackbar('Ny version tilgængelig', {
		//           variant: 'info',
		//           autoHideDuration: null,
		//           action: () => {
		//             return (
		//               <IconButton onClick={() => startTransition(() => router.reload() )}>
		//                 <Refresh />
		//               </IconButton>
		//             );
		//           },
		//         });
		//       }
		//     });
		//   });
		// };
		// checkVersion();
		globalThis.addEventListener("focus", checkVersion);
		return () => globalThis.removeEventListener("focus", checkVersion);
	}, []);

	if (outdated) return <OldBrowser />;
	if (!showing || isLoading || !router.isReady) return null;

	if (router.query.app === "screen" || path.startsWith("user")) return children;

	return (
		<Box sx={{ display: "flex" }}>
			<Scroll>
				{largeScreen && <Box sx={{ p: 4 }} />}
				{typeof window !== "undefined" && (
					<Container sx={{ pl: 1, pr: 1, pt: 1 }} disableGutters>
						{children}
					</Container>
				)}
				<BottomBar setOpenDrawer={setOpenDrawer} />
			</Scroll>
			{largeScreen && <AppDrawer />}

			<Drawer
				open={openDrawer}
				setOpen={() => startTransition(() => setOpenDrawer(false))}
			/>

			{!largeScreen && (
				<Suspense>
					<MobileMenu />
				</Suspense>
			)}
		</Box>
	);
};

export default Layout;
