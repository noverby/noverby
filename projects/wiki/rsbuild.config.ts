import { defineConfig } from "@rsbuild/core";
import { pluginNodePolyfill } from "@rsbuild/plugin-node-polyfill";
import { pluginReact } from "@rsbuild/plugin-react";

export default defineConfig({
	plugins: [pluginReact(), pluginNodePolyfill()],
	html: {
		title: "RadikalWiki",
	},
	source: {
		define: {
			"process.env.NHOST_SUBDOMAIN": JSON.stringify(
				process.env.NHOST_SUBDOMAIN,
			),
			"process.env.NHOST_REGION": JSON.stringify(process.env.NHOST_REGION),
		},
	},
});
