/**
 * What the static export's 404 document should render for a path the export
 * never wrote a file for.
 *
 * The export prerenders one document per *static* route. `/markets/:address`
 * is dynamic — a Market address is chain data no build can enumerate — so the
 * export ships no page for it and a static host answers a deep link with
 * `404.html`. That is the classic static-export gap, and the classic fix is to
 * make `404.html` the app shell: it hydrates, reads `location.pathname`, and
 * renders the route the path names.
 *
 * Nothing is lost by resolving late. The Market detail surface reads every byte
 * it shows from an RPC endpoint the viewer chooses, in the browser, after
 * mount — it never had server-rendered content to give up.
 *
 * This module is the route table for that fallback, kept separate from the
 * component so it can be tested as the pure function it is. It is the mirror of
 * `DYNAMIC_PREFIXES` in `tools/genref/render-site.mjs`, which tells the
 * artifact's link checker the same thing: these prefixes have no prerendered
 * file, and that is expected. A new dynamic route belongs in both.
 *
 * Routing is **structural**, not semantic: `/markets/<anything>` resolves to
 * the Market detail surface, which then refuses by name if those bytes are not
 * a Market. Validating an address here would move a refusal that the detail
 * surface already states precisely into a page that could only say "no".
 */

export type ExportedRouteV1 =
  /** `/markets/:address` — the Market detail surface, for this address. */
  | Readonly<{ kind: 'market-detail'; address: string }>
  /** No route claims this path. The 404 is real; say so. */
  | Readonly<{ kind: 'not-found'; pathname: string }>;

/**
 * Resolve one pathname against the routes the export could not prerender.
 *
 * @param pathname `location.pathname` — already percent-decoded per segment by
 *   this function, never by the caller.
 */
export function resolveExportedPathnameV1(pathname: string): ExportedRouteV1 {
  const trimmed = pathname.replace(/\/+$/, '');
  const segments = trimmed.split('/').filter((segment) => segment !== '');
  if (segments.length === 2 && segments[0] === 'markets') {
    const address = decodeSegmentV1(segments[1]);
    if (address !== null && address !== '') {
      return Object.freeze({ kind: 'market-detail' as const, address });
    }
  }
  return Object.freeze({ kind: 'not-found' as const, pathname });
}

/** A percent-escape a browser would accept in a URL but not in a path segment. */
function decodeSegmentV1(segment: string): string | null {
  try {
    return decodeURIComponent(segment);
  } catch {
    return null;
  }
}
