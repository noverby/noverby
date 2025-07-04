import type { NextConfig } from "next";

const nextConfig: NextConfig = {
	output: "export",
	productionBrowserSourceMaps: true,
	webpack: (config) => {
		// eslint-disable-next-line functional/immutable-data
		config.resolve.fallback = {
			...config.resolve.fallback,
			fs: false,
		};
		config.module.rules.push({
			test: /\.svg$/,
			use: ["@svgr/webpack"],
		});

		return config;
	},
	images: {
		domains: ["pgvhpsenoifywhuxnybq.storage.eu-central-1.nhost.run"],
		unoptimized: true,
	},
};

export default nextConfig;
