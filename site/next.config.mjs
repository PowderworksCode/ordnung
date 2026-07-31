import { createMDX } from "fumadocs-mdx/next";

const withMDX = createMDX();

/** @type {import('next').NextConfig} */
const config = {
  output: "export",
  reactStrictMode: true,
  transpilePackages: ["@thepowderworks/fumadocs"],
  turbopack: { root: import.meta.dirname },
  // The shared package is a local file dependency during development. Preserve
  // its node_modules location so peer dependencies resolve to this app's single
  // React/Fumadocs instances. Published packages do this naturally.
  webpack(config) {
    config.resolve.symlinks = false;
    return config;
  },
};

export default withMDX(config);
