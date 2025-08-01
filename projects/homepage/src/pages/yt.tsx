import { useSearchParams } from "react-router-dom";

const Yt = () => {
	const [params] = useSearchParams();
	const regex = /.*v=([a-zA-Z0-9_-]{11}).*/;
	const match = params.get("url")?.match(regex)?.[1];
	if (!match) return null;

	return (
		<div
			style={{
				position: "relative",
				overflow: "hidden",
				width: "100%",
				height: "100vh",
			}}
		>
			<iframe
				title="YouTube iframe"
				width="100%"
				height="100%"
				src={`https://youtube.com/embed/${match}?enablejsapi=1`}
				frameBorder="0"
				allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture"
				allowFullScreen
			/>
		</div>
	);
};

export { Yt };
