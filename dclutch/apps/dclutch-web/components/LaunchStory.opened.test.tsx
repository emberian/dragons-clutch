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

vi.mock('@/lib/publicCutStaging', async () => {
  const actual = await vi.importActual<typeof import('@/lib/publicCutStaging')>('@/lib/publicCutStaging');
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
    expect(html).toContain('Enter the live market');
    expect(html).toContain(`q=${MARKET}`);
    expect(html).toContain('Open found transaction →');
    expect(html).toContain('<strong>YES</strong><span>market open</span>');
    expect(html).toContain('href="/campaign"');
    expect(html).toContain('href="/population"');
  });

  it('still does not promise resolution or redemption', () => {
    // The opened branch of the "What changed" card used to read "Resolution can
    // use the sponsored SOL/USD Pyth account ... Redemption returns collateral
    // through the same public market" -- it would have gone live saying so the
    // instant the fixture named a market.
    expect(html).not.toContain('Redemption returns collateral');
    expect(html).not.toContain('RESOLVE');
    expect(html).not.toContain('REDEEM');
    expect(html).toContain('<code>FOUND → JOIN → TRADE</code>');
    // Renegotiated 2026-09-02. The card used to say resolution and redemption
    // "are not open yet", and redemption HAS been open in this browser since
    // it shipped -- with no file, no CLI and no operator. What is still true
    // is a fact about the markets rather than about the code, so the page says
    // that and this case pins the distinction rather than the old sentence.
    expect(html).not.toContain('are not open yet');
    expect(html).toContain('no market has reached an answer yet');
    expect(html).toContain('<strong>Resolve</strong><p>Not yet.');
    expect(html).toContain('<strong>Redeem</strong><p>Not yet — no market has an answer.');
  });

  it('offers no transaction link for a step that has no signature', () => {
    // A market can be founded long before it is traded. The rail must not grow
    // a dead "Open trade transaction" link just because a market exists.
    expect(html).not.toContain('Open trade transaction');
    expect(html).not.toContain('Open resolve transaction');
    expect(html).not.toContain('Open redeem transaction');
  });
});
