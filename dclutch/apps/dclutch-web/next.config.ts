import type { NextConfig } from 'next';

// DCLUTCH_PAGES_EXPORT=1 switches the build to a fully static export for the
// GitHub Pages artifact (dragons-clutch .github/workflows/pages.yml). The
// default build stays the Cloudflare worker target; nothing about the app's
// runtime behavior differs between the two — every route is client-rendered
// against whatever chain the viewer configures.
//
// DCLUTCH_PAGES_BASE_PATH sets the subpath a GitHub project page is served
// under (e.g. /dragons-clutch/app). Unset, assets resolve from the root.
const staticExport = process.env.DCLUTCH_PAGES_EXPORT === '1';
const basePath = process.env.DCLUTCH_PAGES_BASE_PATH;

const nextConfig: NextConfig = {
  ...(staticExport
    ? {
        output: 'export',
        ...(basePath ? { basePath } : {}),
      }
    : {}),
};

export default nextConfig;
