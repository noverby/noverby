import { defineConfig } from "@rsbuild/core";
import { pluginNodePolyfill } from "@rsbuild/plugin-node-polyfill";
import { pluginReact } from "@rsbuild/plugin-react";

export default defineConfig({
	plugins: [pluginReact(), pluginNodePolyfill()],
	html: {
		template: "./public/index.html",
	},
	source: {
		entry: {
			index: "./src/index.tsx",
		},
	},
	server: {
		historyApiFallback: true,
	},
	output: {
		assetPrefix: "/",
	},
});
