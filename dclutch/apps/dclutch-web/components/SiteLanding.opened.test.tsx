import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';

// `/` is the front door, and its "Try dClutch" aside is the one thing on
// it that dates. It read "the first markets are being set up" as a hard-coded
// fact, so the day a market opened the front door would have gone on saying
// they were still being set up until somebody noticed.
//
// It now reads the same public cut the launch page does. This renders the face
// nobody sees until that happens.

const MARKET = 'GtmpRvSL9y6RpqMth73VSdb9h1XRe7zqQZkhJkfgxKrA';
const FOUND = '3K6ik9Ah7xzBtYgvm6ZuaNs7C3GCNnPiwP5XX1b9gDG1EyjbU9AEN7ei8kYk4umPt3dXCXqiFwLEecBjunFVKtwF';

vi.mock('@dclutch/sdk/publicCutStaging', async () => {
  const actual = await vi.importActual<typeof import('@dclutch/sdk/publicCutStaging')>('@dclutch/sdk/publicCutStaging');
  const cut = actual.parsePublicDevnetCutV1({
    schema: 'dclutch-public-cut-v1',
    cluster: 'devnet',
    market: MARKET,
    activity: { found: FOUND, trade: null, resolve: null, redeem: null },
    checkedReleases: {},
  });
  return { ...actual, PUBLIC_DEVNET_CUT_V1: cut };
});

const { default: SiteLanding } = await import('./SiteLanding');

describe('the front door, once a market is open', () => {
  const html = renderToStaticMarkup(<SiteLanding />);

  it('does not call an unverified feature current, and links the record it names', () => {
    expect(html).not.toContain('No featured market has been staged');
    expect(html).toContain('Loading');
    expect(html).toContain(`/market?address=${MARKET}`);
  });

  it('does not TYPE the phase it has not read', () => {
    // The aside said the first market was "open" as a written word, which is a
    // promise about a chain fact nobody was checking -- and a resolution moves
    // that fact the same afternoon a fill lands on it. The phase now comes off
    // the market's own Core account, so the SERVER-rendered face, which has
    // read nothing yet, must carry no phase word at all: it says where the
    // answer is read instead of guessing it.
    expect(html).toContain('Loading');
    for (const phase of ['open', 'resolved', 'winding down', 'finished', 'still being set up']) {
      expect(html.slice(html.indexOf('Try dClutch'), html.indexOf('</aside>'))).not.toContain(phase);
    }
  });

  it('stops saying the app will tell you there is no open market, once there is one', () => {
    // The aside was not the only dated sentence on this page. "Anything that
    // still needs an open market will tell you plainly that there is not one
    // yet" was written before there was one, and would have gone on saying it.
    // Renegotiated 2026-08-31 with the sibling test: both arms of the old
    // conditional blurb are deleted, so neither can go stale. What this file
    // still pins is the OTHER dated sentence -- the hero aside -- switching.
    expect(html).toContain('Devnet markets.');
    expect(html).not.toContain('will tell you plainly that there is not one yet');
  });

  it('keeps the part that is still true on devnet', () => {
    // An open market does not make devnet tokens worth anything. This is the
    // sentence that must survive the market opening, not be swept out with it.
    expect(html).toContain('Devnet preview · test tokens');
    expect(html).toContain('Use test tokens to try a market');
    expect(html).not.toContain('no value at risk anywhere');
  });
});
