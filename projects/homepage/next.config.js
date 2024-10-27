/** @type {import('next').NextConfig} */
const nextConfig = {
  swcMinify: true,
  redirects: async () => [
    {
      source: "/.well-known/matrix/server",
      destination: "https://matrix.overby.me/.well-known/matrix/server",
      statusCode: 301
    },
    {
      source: "/.well-known/matrix/client",
      destination: "https://matrix.overby.me/.well-known/matrix/client",
      statusCode: 301
    },
  ]
}

module.exports = nextConfig
