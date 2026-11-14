const config = {
  output: "export",
  trailingSlash: true,
  images: { unoptimized: true },
  poweredByHeader: false,
  generateBuildId: () => "lengua",
  typescript: { ignoreBuildErrors: true },
  turbopack: { root: import.meta.dirname },
  experimental: { turbopackModuleIds: "named" },
};

export default config;
