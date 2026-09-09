import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';

// The front door's closed face, pinned against a mock pending cut now that
// the published fixture names a Market. Its companion,
// SiteLanding.opened.test.tsx, renders the same page with a market named.

vi.mock('@dclutch/sdk/publicCutStaging', async () => {
  const actual = await vi.importActual<typeof import('@dclutch/sdk/publicCutStaging')>('@dclutch/sdk/publicCutStaging');
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

  it('explains claims and labels the devnet preview', () => {
    expect(html).toContain('Markets with');
    expect(html).toContain('fixed payouts.');
    expect(html).toContain('Devnet preview · test tokens');
    expect(html).toContain('market list');
    expect(html).toContain('each winning claim pays one collateral unit');
  });

  it('leaves unread market counts empty', () => {
    expect(html).not.toContain('>0</strong>');
  });

  it('links the available actions', () => {
    expect(html).toContain('href="/markets"');
    expect(html).toContain('href="/portfolio"');
    expect(html).toContain('href="/create"');
    expect(html).toContain('href="/console"');
  });

  it('links the devnet faucet', () => {
    expect(html).toContain('https://faucet.solana.com');
    expect(html).toContain('Get free devnet SOL');
  });

  it('links the project history', () => {
    expect(html).toContain('/notes/plan-to-compost-at-least-three/');
    expect(html).toContain('Plan to compost at least three');
    expect(html).toContain('Project history');
  });

  it('describes the existing key art', () => {
    expect(html).toContain('/art/dragons-clutch-key-art-v1-1672w.webp');
    expect(html).toContain('claw cradling a glowing, faceted gem');
    expect(html).toContain('loading="lazy"');
  });
});
