import type { NextConfig } from 'next';

// DCLUTCH_PAGES_EXPORT=1 switches the build to a fully static export for the
// GitHub Pages artifact (dragons-clutch .github/workflows/pages.yml). The
// default build stays the Cloudflare worker target; nothing about the app's
// runtime behavior differs between the two — every route is client-rendered
// against whatever chain the viewer configures.
//
// The export is built for a DOMAIN ROOT, and the Pages artifact mounts it
// there (tools/genref/render-site.mjs). Both of vinext 1.0.0-beta.3's
// relocation knobs were measured against output:'export' on 2026-08-27:
//
//   - basePath is broken. Its export prerenderer requests un-prefixed paths
//     and skips every route, so the export comes out empty. DCLUTCH_PAGES_BASE_PATH
//     below is kept only so a future vinext can be retested through it; setting
//     it today produces no site.
//   - assetPrefix works, but is not enough. It correctly rewrites every asset
//     URL and relocates the on-disk directory to match (`assetPrefix: '/app'`
//     emits `/app/_next/…` and writes dist/client/app/_next/), and it does not
//     touch route URLs — so under a subpath the page would load its CSS and
//     hydrate, and then every link in it would 404 at the domain root.
//
// One prefix that moves assets and routes together does not exist here, so the
// app is served where its URLs already point.
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
