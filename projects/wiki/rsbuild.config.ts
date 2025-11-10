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
		define: {
			"process.env.NHOST_SUBDOMAIN": JSON.stringify(
				process.env.NHOST_SUBDOMAIN,
			),
			"process.env.NHOST_REGION": JSON.stringify(process.env.NHOST_REGION),
		},
	},
	server: {
		historyApiFallback: true,
	},
	output: {
		assetPrefix: "/",
	},
});
