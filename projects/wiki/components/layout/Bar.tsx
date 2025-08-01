import { alpha, Box, type BoxProps } from "@mui/material";
import { type ForwardedRef, forwardRef } from "react";

const BarRef = (props: BoxProps, ref?: ForwardedRef<HTMLDivElement>) => (
	<Box
		{...props}
		ref={ref}
		sx={{
			position: "relative",
			borderRadius: "20px",
			backgroundColor: (t) => alpha(t.palette.secondary.main, 0.9),
			marginLeft: {
				sm: 1,
				md: 0,
			},
			width: "100%",
		}}
	/>
);

const Bar = forwardRef(BarRef);

export default Bar;
