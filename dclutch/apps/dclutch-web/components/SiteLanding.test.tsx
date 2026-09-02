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
    checkedReleases: {},
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
    // Renegotiated 2026-08-31: the strip used to be introduced by a paragraph
    // saying the numbers are read live, never estimated, never remembered,
    // that a dash means unread and a zero means zero. Three labelled numbers
    // do not need a legend. Deleted; what is pinned is that no zero is
    // invented in place of an unread value.
    expect(html).not.toContain('>0</strong>');
  });

  it('describes what needs an open market without pretending there is one', () => {
    // Renegotiated 2026-08-31: the section used to promise that anything
    // needing an open market "will tell you plainly that there is not one yet,
    // instead of failing quietly". Deleted -- the pages do it, they no longer
    // announce that they will.
    expect(html).toContain('Seven programs, deployed on devnet');
    expect(html).not.toContain('failing quietly');
  });

  it('offers the faucet to a reader who wants to try it, right beside the nothing-for-sale fact', () => {
    expect(html).toContain('https://faucet.solana.com');
    expect(html).toContain('devnet SOL is free from the');
  });

  /**
   * The field notes were written on 25 August 2026, committed the same day to
   * a separate posters repository, and then linked from nowhere — the one
   * long-form piece about how this was built, unreachable by anyone who did
   * not already know it existed. The link is pinned here so it cannot go
   * quiet again, and the path is pinned because the artifact's link check
   * resolves this exact string against a directory index.
   */
  it('gives a reader the field notes on how this was built, and says what they are', () => {
    expect(html).toContain('/notes/plan-to-compost-at-least-three/');
    expect(html).toContain('Plan to compost at least three');
    expect(html).toContain('How this was built');
    expect(html).toContain('Two earlier builds, thrown away on purpose');
    // Renegotiated 2026-08-31: "and they are honest about what is proved and
    // what is still only tested" is the field notes vouching for themselves.
    // Deleted; the notes are linked and can speak for themselves.
    expect(html).toContain('what survived each time');
  });

  it('carries the key art with a described image', () => {
    expect(html).toContain('/art/dragons-clutch-key-art-v1-1672w.webp');
    // Renegotiated 2026-08-31: the figcaption ("every claim fully backed by
    // collateral the market holds like treasure") restated the hero in
    // metaphor under a picture. Deleted; the alt text still describes it.
    expect(html).toContain('claw cradling a glowing, faceted gem');
    expect(html).not.toContain('<figcaption>');
    // Lazy: the art must never delay the numbers the page exists to show.
    expect(html).toContain('loading="lazy"');
  });
});
