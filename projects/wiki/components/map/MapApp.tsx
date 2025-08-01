import Maplibre, { type MapStyle } from "react-map-gl/maplibre";
import "maplibre-gl/dist/maplibre-gl.css";
import { Card } from "@mui/material";
import { ContentHeader } from "components/content";
import type { Node } from "hooks";

const STYLE: MapStyle = {
	version: 8,
	sources: {
		"raster-tiles": {
			type: "raster",
			tiles: [
				"https://a.tile.openstreetmap.org/{z}/{x}/{y}.png",
				"https://b.tile.openstreetmap.org/{z}/{x}/{y}.png",
				"https://c.tile.openstreetmap.org/{z}/{x}/{y}.png",
			],
			tileSize: 256,
		},
	},
	layers: [
		{
			id: "osm-tiles",
			type: "raster",
			source: "raster-tiles",
			minzoom: 0,
			maxzoom: 19,
		},
	],
};

const MapApp = ({ node }: { node: Node }) => {
	return (
		<Card sx={{ m: 0 }}>
			<ContentHeader hideMembers node={node} />
			<Maplibre
				initialViewState={{
					longitude: 10.2,
					latitude: 56.2,
					zoom: 6.5,
				}}
				style={{ width: "100%", height: "85vh" }}
				mapStyle={STYLE}
			/>
		</Card>
	);
};
export default MapApp;
