import million from "million/compiler";

/** @type {import('next').NextConfig} */
const nextConfig = {
  // ...
  webpack: (config) => {
    // eslint-disable-next-line functional/immutable-data
    config.resolve.fallback = {
      ...config.resolve.fallback,
      fs: false,
    };
    config.module.rules.push({
      test: /\.svg$/,
      use: ['@svgr/webpack'],
    });

    return config;
  },
  images: {
    domains: ['pgvhpsenoifywhuxnybq.storage.eu-central-1.nhost.run'],
  },
};

const millionConfig = {
  auto: true,
};

export default million.next(nextConfig, millionConfig);
