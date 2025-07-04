import { Button } from "@mui/material";

const Crash = () => {
	return (
		<Button
			onClick={() => {
				// eslint-disable-next-line functional/no-throw-statements
				throw Error("Triggered Crash");
			}}
		>
			Trigger Crash
		</Button>
	);
};

export default Crash;
