/** @type {import('next').NextConfig} */
const nextConfig = {
  swcMinify: true,
  redirects: async () => [
    {
      source: "/.well-known/matrix/server",
      destination: "https://matrix.overby.me/.well-known/matrix/server",
      permanent: true,
    },
    {
      source: "/.well-known/matrix/client",
      destination: "https://matrix.overby.me/.well-known/matrix/client",
      permanent: true,
    },
    {
      source: "/.well-known/matrix/support",
      destination: "https://matrix.overby.me/.well-known/matrix/support",
      permanent: true,
    },
  ],
};

module.exports = nextConfig;
