import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';

// The launch page has two faces and only one of them is ever on screen. Until a
// market opens, every `opened ? A : B` renders B, so the whole A side ships
// unread -- and it shipped for weeks promising resolution and redemption,
// because nothing rendered it. This file renders A.
//
// It matters most on the day the public cut names a market: that is a fixture
// edit, no code review, and the page silently switches to copy nobody looked
// at. These assertions are what makes that switch safe.

const MARKET = 'GtmpRvSL9y6RpqMth73VSdb9h1XRe7zqQZkhJkfgxKrA';
const FOUND = '3K6ik9Ah7xzBtYgvm6ZuaNs7C3GCNnPiwP5XX1b9gDG1EyjbU9AEN7ei8kYk4umPt3dXCXqiFwLEecBjunFVKtwF';

vi.mock('@dclutch/sdk/publicCutStaging', async () => {
  const actual = await vi.importActual<typeof import('@dclutch/sdk/publicCutStaging')>('@dclutch/sdk/publicCutStaging');
  // Built through the REAL parser, so this test also proves the shape the
  // public cut will actually carry is one the parser accepts.
  const cut = actual.parsePublicDevnetCutV1({
    schema: 'dclutch-public-cut-v1',
    cluster: 'devnet',
    market: MARKET,
    activity: { found: FOUND, trade: null, resolve: null, redeem: null },
    checkedReleases: {},
  });
  return { ...actual, PUBLIC_DEVNET_CUT_V1: cut };
});

const { default: LaunchStory } = await import('./LaunchStory');

describe('launch story, once a market is open', () => {
  const html = renderToStaticMarkup(<LaunchStory />);

  it('links the market it names', () => {
    // The permalink is the static-host-safe /market?address= form, not a
    // /markets/<address> path: the export has no such prerendered document.
    expect(html).toContain(`/market?address=${MARKET}`);
    expect(html).toContain('View featured market');
    expect(html).toContain(`q=${MARKET}`);
    expect(html).toContain('Open found transaction →');
    expect(html).toContain('<strong>LINKED</strong><span>featured market</span>');
    expect(html).toContain('href="/campaign"');
    expect(html).toContain('href="/population"');
  });

  it('offers no transaction link for a step that has no signature', () => {
    // A market can be founded long before it is traded. The rail must not grow
    // a dead "Open trade transaction" link just because a market exists.
    expect(html).not.toContain('Open trade transaction');
    expect(html).not.toContain('Open resolve transaction');
    expect(html).not.toContain('Open redeem transaction');
  });
});
