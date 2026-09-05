import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';

// The launch page's third call to action is "Read activity". Before this, a
// reader who followed it arrived at a form asking for "Market addresses, one
// per line" -- at exactly the moment we most want them to succeed, and with no
// way to know the address. The cut already names it.
//
// A link's own market list still wins; this is only the fallback.

const MARKET = 'GtmpRvSL9y6RpqMth73VSdb9h1XRe7zqQZkhJkfgxKrA';
const FOUND = '3K6ik9Ah7xzBtYgvm6ZuaNs7C3GCNnPiwP5XX1b9gDG1EyjbU9AEN7ei8kYk4umPt3dXCXqiFwLEecBjunFVKtwF';

vi.mock('@dclutch/sdk/publicCutStaging', async () => {
  const actual = await vi.importActual<typeof import('@dclutch/sdk/publicCutStaging')>('@/lib/publicCutStaging');
  const cut = actual.parsePublicDevnetCutV1({
    schema: 'dclutch-public-cut-v1',
    cluster: 'devnet',
    market: MARKET,
    activity: { found: FOUND, trade: null, resolve: null, redeem: null },
    checkedReleases: {},
  });
  return { ...actual, PUBLIC_DEVNET_CUT_V1: cut };
});

const { default: ActivityWorkspace } = await import('./ActivityWorkspace');

describe('activity, once a market is open', () => {
  const html = renderToStaticMarkup(<ActivityWorkspace />);

  it('fills the market field from the public cut so the reader is not asked for an address they cannot know', () => {
    expect(html).toContain(MARKET);
  });

  it('still asks for the owner rather than inventing one', () => {
    // The market is public; whose history to read is not, and this page must
    // never guess at an identity.
    expect(html).toContain('Owner address');
    expect(html).toContain('No signature history has been read.');
  });
});
