import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';

// `/` is the front door, and its "Where this stands" aside is the one thing on
// it that dates. It read "the first markets are being set up" as a hard-coded
// fact, so the day a market opened the front door would have gone on saying
// they were still being set up until somebody noticed.
//
// It now reads the same public cut the launch page does. This renders the face
// nobody sees until that happens.

const MARKET = 'GtmpRvSL9y6RpqMth73VSdb9h1XRe7zqQZkhJkfgxKrA';
const FOUND = '3K6ik9Ah7xzBtYgvm6ZuaNs7C3GCNnPiwP5XX1b9gDG1EyjbU9AEN7ei8kYk4umPt3dXCXqiFwLEecBjunFVKtwF';

vi.mock('@/lib/publicCutStaging', async () => {
  const actual = await vi.importActual<typeof import('@/lib/publicCutStaging')>('@/lib/publicCutStaging');
  const cut = actual.parsePublicDevnetCutV1({
    schema: 'dclutch-public-cut-v1',
    cluster: 'devnet',
    market: MARKET,
    activity: { found: FOUND, trade: null, resolve: null, redeem: null },
  });
  return { ...actual, PUBLIC_DEVNET_CUT_V1: cut };
});

const { default: SiteLanding } = await import('./SiteLanding');

describe('the front door, once a market is open', () => {
  const html = renderToStaticMarkup(<SiteLanding />);

  it('stops saying the first markets are being set up, and links the one that is', () => {
    expect(html).not.toContain('the first markets are being set up');
    expect(html).toContain('the first market is');
    expect(html).toContain(`/market?address=${MARKET}`);
  });

  it('keeps the part that is still true on devnet', () => {
    // An open market does not make devnet tokens worth anything. This is the
    // sentence that must survive the market opening, not be swept out with it.
    expect(html).toContain('On devnet — nothing for sale');
    expect(html).toContain('worthless by construction');
    expect(html).toContain('no value at risk anywhere');
  });
});
