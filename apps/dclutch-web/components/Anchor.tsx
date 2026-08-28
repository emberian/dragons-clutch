import type { AnchorHTMLAttributes, ReactNode } from 'react';

/**
 * A link to another route, as a plain `<a>` — a full page load.
 *
 * This exists instead of `next/link` for two reasons, one permanent and one a
 * defect we are waiting out.
 *
 * The permanent one: **this app ships as a static export.** Every route is a
 * separate prerendered document that reads whatever chain the viewer points it
 * at, and no route carries state another route needs. A full page load *is* the
 * navigation model here; client-side routing would buy nothing and costs a
 * router that has to agree with the host about what exists.
 *
 * The defect: vinext 1.0.0-beta.3's `next/link` shim is **inert in every
 * production bundle**. Its click handler awaits `import("./navigation.js")` for
 * `navigateClientSide`; in the built bundle that specifier resolves to an
 * export-less namespace, so prefetch throws — and by then the handler has
 * already called `preventDefault()`. The result is a link that swallows the
 * click and navigates nowhere. Typed URLs and plain `<a>` were unaffected
 * throughout, which is how this was isolated: measured 2026-08-27 under
 * `vinext start` and against the Pages artifact alike, so it is the shim, not
 * the host. beta.8 does not drop in — it needs `@vitejs/plugin-rsc` >= 0.5.34,
 * which then breaks `rolldown:vite-resolve`. That coordination is a WAVE lane.
 *
 * So: use `Anchor` for anything that leaves the current route. Keep `next/link`
 * only for a link that is genuinely intra-page or depends on hydrated state
 * surviving the transition — at the time of writing, the app has none, and
 * `next/link` appears nowhere in it.
 */
export default function Anchor({
  href,
  children,
  ...rest
}: Readonly<
  Omit<AnchorHTMLAttributes<HTMLAnchorElement>, 'href'> & {
    href: string;
    children?: ReactNode;
  }
>) {
  return <a href={href} {...rest}>{children}</a>;
}
