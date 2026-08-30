import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';

// The front door's closed face, pinned against a mock pending cut now that
// the published fixture names a Market. Its companion,
// SiteLanding.opened.test.tsx, renders the same page with a market named.

vi.mock('@/lib/publicCutStaging', async () => {
  const actual = await vi.importActual<typeof import('@/lib/publicCutStaging')>('@/lib/publicCutStaging');
  const cut = actual.parsePublicDevnetCutV1({
    schema: 'dclutch-public-cut-v1',
    cluster: 'devnet',
    market: null,
    activity: { found: null, trade: null, resolve: null, redeem: null },
  });
  return { ...actual, PUBLIC_DEVNET_CUT_V1: cut };
});

const { default: SiteLanding } = await import('./SiteLanding');

describe('the front door', () => {
  const html = renderToStaticMarkup(<SiteLanding />);

  it('says plainly where this stands before it says anything else', () => {
    expect(html).toContain('On devnet — nothing for sale');
    expect(html).toContain('the first markets are being set up');
    expect(html).toContain('no value at risk anywhere');
  });

  it('does not promise the reader a view of activity that is not there', () => {
    // It used to say "you can watch it all happen live below", above a strip
    // of three numbers.
    expect(html).not.toContain('watch it all happen live');
    expect(html).toContain('read live from the chain every time you open this page');
  });

  it('describes what needs an open market without pretending there is one', () => {
    expect(html).toContain('The seven programs are deployed');
    expect(html).toContain('will tell you plainly that there is not one yet');
  });

  it('offers the faucet to a reader who wants to try it, right beside the nothing-for-sale fact', () => {
    expect(html).toContain('https://faucet.solana.com');
    expect(html).toContain('devnet SOL is free from the');
  });

  it('carries the key art with a described image and an honest caption', () => {
    expect(html).toContain('/art/dragons-clutch-key-art-v1-1672w.webp');
    expect(html).toContain('claw cradling a glowing, faceted gem');
    expect(html).toContain('holds like treasure');
    // Lazy: the art must never delay the numbers the page exists to show.
    expect(html).toContain('loading="lazy"');
  });
});
