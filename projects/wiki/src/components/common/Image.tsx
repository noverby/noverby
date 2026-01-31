import { BrokenImage } from "@mui/icons-material";
import { CircularProgress } from "@mui/material";
import { grey } from "@mui/material/colors";
import { Box } from "@mui/system";
import { type ReactEventHandler, useRef, useState } from "react";

const Image = ({
	src,
	alt,
	onLoad,
	onError,
	animationDuration = 3000,
	aspectRatio = 1,
}: {
	src: string;
	alt: string;
	layout?: string;
	onLoad?: (event: React.SyntheticEvent<HTMLImageElement>) => void;
	onError?: (event: React.SyntheticEvent<HTMLImageElement>) => void;
	animationDuration?: number;
	aspectRatio?: number;
}) => {
	const [loaded, setLoaded] = useState(false);
	const [error, setError] = useState(false);
	const imageRef = useRef<HTMLImageElement>(null);

	const handleLoadImage: ReactEventHandler<HTMLImageElement> = (e) => {
		setLoaded(true);
		setError(false);
		if (onLoad) {
			onLoad(e);
		}
	};

	const handleImageError: ReactEventHandler<HTMLImageElement> = (e) => {
		if (src) {
			setError(true);
			if (onError) {
				onError(e);
			}
		}
	};

	const imageTransition = {
		opacity: loaded ? 1 : 0,
		filter: `brightness(${loaded ? 100 : 0}%) saturate(${loaded ? 100 : 20}%)`,
		transition: `
			filter ${animationDuration * 0.75}ms cubic-bezier(0.4, 0.0, 0.2, 1),
			opacity ${animationDuration / 2}ms cubic-bezier(0.4, 0.0, 0.2, 1)`,
	};

	return (
		<Box
			sx={{
				paddingTop: `calc(1 / ${aspectRatio} * 100%)`,
				position: "relative",
			}}
		>
			{src && (
				<img
					src={src}
					alt={alt}
					ref={imageRef}
					style={{
						borderRadius: "20px",
						width: "100%",
						height: "100%",
						position: "absolute",
						objectFit: "cover",
						cursor: "inherit",
						top: 0,
						left: 0,
						...imageTransition,
					}}
					onLoad={handleLoadImage}
					onError={handleImageError}
				/>
			)}
			<Box
				style={{
					width: "100%",
					height: "100%",
					position: "absolute",
					cursor: "inherit",
					top: 0,
					left: 0,
					display: "flex",
					alignItems: "center",
					justifyContent: "center",
					pointerEvents: "none",
				}}
			>
				{!loaded && !error && <CircularProgress size={48} />}
				{error && (
					<BrokenImage sx={{ width: 48, height: 48, color: grey[300] }} />
				)}
			</Box>
		</Box>
	);
};

export default Image;
