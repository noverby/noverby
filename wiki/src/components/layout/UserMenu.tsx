import {
	Brightness4,
	Brightness7,
	HowToReg,
	LockReset,
	Login,
	Logout,
	ManageAccounts,
} from "@mui/icons-material";
import {
	Avatar,
	IconButton,
	ListItemIcon,
	ListItemText,
	Menu,
	MenuItem,
	useMediaQuery,
	useTheme,
} from "@mui/material";
import { useAuthenticationStatus } from "@nhost/react";
import { ThemeModeContext } from "core/theme/ThemeModeContext";
import { client } from "gql";
import { nhost } from "nhost";
import { type MouseEventHandler, useContext, useState } from "react";
import { useNavigate } from "react-router-dom";

const UserMenu = ({ avatar }: { avatar?: boolean }) => {
	const navigate = useNavigate();
	const [anchorEl, setAnchorEl] = useState<
		HTMLButtonElement | HTMLDivElement | null
	>(null);
	const { isAuthenticated } = useAuthenticationStatus();
	const { toggleThemeMode } = useContext(ThemeModeContext);
	const { palette } = useTheme();
	const largeScreen = useMediaQuery("(min-width:1200px)");

	const handleClick: MouseEventHandler<HTMLButtonElement | HTMLDivElement> = (
		event,
	) => {
		setAnchorEl(event.currentTarget);
	};

	const handleClose = () => {
		setAnchorEl(null);
	};

	const handleLogout = async () => {
		await nhost.auth.signOut();
		// Delete cache
		// eslint-disable-next-line functional/immutable-data
		client.cache.clear();
		setAnchorEl(null);
	};

	const handleUser = (mode: "login" | "register" | "set-password") => () => {
		setAnchorEl(null);
		navigate(`/user/${mode}`);
	};

	return (
		<>
			{avatar ? (
				<IconButton onClick={handleClick}>
					<Avatar sx={{ bgcolor: "primary.main" }}>
						<ManageAccounts />
					</Avatar>
				</IconButton>
			) : (
				<IconButton onClick={handleClick}>
					<ManageAccounts />
				</IconButton>
			)}
			<Menu
				anchorOrigin={{
					vertical: largeScreen ? "bottom" : "top",
					horizontal: "center",
				}}
				transformOrigin={{
					vertical: largeScreen ? "top" : "bottom",
					horizontal: "center",
				}}
				anchorEl={anchorEl}
				open={Boolean(anchorEl)}
				onClose={handleClose}
			>
				{(isAuthenticated && [
					//<MenuItem
					//  key="profile"
					//  component={Link}
					//  href={session?.user?.id!}
					//  onClick={handleClose}
					//>
					//  <ListItemIcon>
					//    <Face />
					//  </ListItemIcon>
					//  <ListItemText>Profil</ListItemText>
					//</MenuItem>,
					<MenuItem key="reset" onClick={handleUser("set-password")}>
						<ListItemIcon>
							<LockReset />
						</ListItemIcon>
						<ListItemText>Sæt Kodeord</ListItemText>
					</MenuItem>,
					<MenuItem key="logout" onClick={handleLogout}>
						<ListItemIcon>
							<Logout />
						</ListItemIcon>
						<ListItemText>Log ud</ListItemText>
					</MenuItem>,
				]) || [
					<MenuItem key="login" onClick={handleUser("login")}>
						<ListItemIcon>
							<Login />
						</ListItemIcon>
						<ListItemText>Log ind</ListItemText>
					</MenuItem>,
					<MenuItem key="register" onClick={handleUser("register")}>
						<ListItemIcon>
							<HowToReg />
						</ListItemIcon>
						<ListItemText>Registrer</ListItemText>
					</MenuItem>,
				]}
				<MenuItem key="theme" onClick={toggleThemeMode}>
					<ListItemIcon>
						{palette.mode === "light" ? <Brightness4 /> : <Brightness7 />}
					</ListItemIcon>
					<ListItemText>
						{palette.mode === "light" ? "Mørk" : "Lys"}
					</ListItemText>
				</MenuItem>
			</Menu>
		</>
	);
};

export default UserMenu;
