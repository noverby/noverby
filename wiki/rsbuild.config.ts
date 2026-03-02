import { defineConfig, type RsbuildPlugin } from "@rsbuild/core";
import { pluginNodePolyfill } from "@rsbuild/plugin-node-polyfill";
import { pluginReact } from "@rsbuild/plugin-react";
import { pluginSvgr } from "@rsbuild/plugin-svgr";

/**
 * Generate `client-metadata.json` for atproto OAuth at build time.
 *
 * The AT Protocol authorization server fetches `client_id` (a URL) to verify
 * the OAuth client's identity. The metadata must match the origin where the
 * app is served, so a static file hardcoding one domain breaks on other
 * origins (e.g. rebuild.radikal.wiki vs radikal.wiki).
 *
 * This plugin emits the file during compilation (both dev and build) using
 * `PUBLIC_SITE_URL` to set the correct origin. In dev mode the origin is
 * `http://localhost`, which atproto treats as a loopback client (no server
 * fetch needed), so the file is informational only.
 */
function pluginClientMetadata(): RsbuildPlugin {
	return {
		name: "plugin-client-metadata",
		setup(api) {
			api.processAssets({ stage: "additional" }, ({ compilation, sources }) => {
				const origin = process.env.PUBLIC_SITE_URL ?? "https://radikal.wiki";
				const metadata = {
					client_id: `${origin}/client-metadata.json`,
					client_name: "RadikalWiki",
					client_uri: origin,
					redirect_uris: [`${origin}/auth/callback`],
					scope: "atproto transition:generic",
					grant_types: ["authorization_code", "refresh_token"],
					response_types: ["code"],
					token_endpoint_auth_method: "none",
					application_type: "web",
					dpop_bound_access_tokens: true,
				};
				compilation.emitAsset(
					"client-metadata.json",
					new sources.RawSource(JSON.stringify(metadata, null, "\t")),
				);
			});
		},
	};
}

export default defineConfig({
	plugins: [
		pluginReact(),
		pluginNodePolyfill(),
		pluginSvgr(),
		pluginClientMetadata(),
	],
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
