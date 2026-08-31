import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import {
  BUNDLE_MINT_B_V1,
  BUNDLE_TERMS_TWO_V1,
  bundleEntryV1,
  bundlePortfolioV1,
} from '../fixtures/bundlePortfolio';
import { bundleExposureV1 } from '../lib/bundleExposure';
import BundleExposurePanel from './BundleExposurePanel';

/**
 * The panel is where the arithmetic becomes a sentence, so these tests read it
 * as a reader would: are the exact atoms on the page, is the netting answer
 * stated rather than implied, and does the vocabulary stay inside the same
 * refusal the rest of this surface keeps.
 */

function render(entries: Parameters<typeof bundlePortfolioV1>[0]): string {
  return renderToStaticMarkup(<BundleExposurePanel exposure={bundleExposureV1(bundlePortfolioV1(entries))} />);
}

describe('the bundle panel', () => {
  const unrelated = render([
    bundleEntryV1('MarketOne', ['10', '40', '25']),
    bundleEntryV1('MarketTwo', ['5', '5', '100'], { terms: BUNDLE_TERMS_TWO_V1 }),
  ]);

  it('puts the exact bound on the page, not a rounded one', () => {
    expect(unrelated).toContain('140');
    expect(unrelated).toContain('15');
    expect(unrelated).toContain('125');
    expect(unrelated).toContain('Arrives whatever happens');
    expect(unrelated).toContain('The most it can pay');
    expect(unrelated).toContain('Decided by the outcomes');
  });

  it('states the refusal that is the whole feature', () => {
    expect(unrelated).toContain('settle against different things');
    expect(unrelated).toContain('That sum is the true maximum, not a cautious one');
    expect(unrelated).toContain('will not put one into your arithmetic');
    expect(unrelated).not.toContain('Cannot both be paid');
  });

  it('names the records it did not read instead of estimating past them', () => {
    expect(unrelated).toContain('the payoff basis records themselves, the knots and the degree');
    expect(unrelated).toContain('It states no number it cannot derive from bytes it read');
  });

  it('shows a locked group as a conditional refinement, never as the headline', () => {
    const locked = render([
      bundleEntryV1('MarketOne', ['10', '40', '25']),
      bundleEntryV1('MarketTwo', ['30', '5', '5']),
    ]);
    expect(locked).toContain('Cannot both be paid');
    expect(locked).toContain('Locked to each other');
    expect(locked).toContain('walked to its own failure outcome on its own deadline');
    expect(locked).toContain('the figures above the fold stay the sum');
    // The headline tiles keep the sum; the narrower pair is stated beside them.
    expect(locked).toContain('>70<');
    expect(locked).toContain('45');
  });

  it('keeps two collateral mints as two bundles and says why', () => {
    const mixed = render([
      bundleEntryV1('MarketOne', ['10', '40']),
      bundleEntryV1('MarketTwo', ['1', '1000'], { mint: BUNDLE_MINT_B_V1, terms: BUNDLE_TERMS_TWO_V1 }),
    ]);
    expect(mixed).toContain('atoms of different mints are different units and are never added');
    expect(mixed).toContain('one collateral mint');
  });

  it('leaves a Market it could not place out of every bundle, by name', () => {
    const excluded = render([
      bundleEntryV1('MarketOne', ['10', '40']),
      bundleEntryV1('MarketTwo', ['1', '1000'], { marketRefused: true }),
    ]);
    expect(excluded).toContain('left out of every bundle');
    expect(excluded).toContain('did not decode at this finalized floor');
  });

  it('presents raw atoms and never a market-data metric', () => {
    for (const forbidden of ['volume', 'Volume', 'odds', 'probability', 'Probability', 'TVL', 'liquidity', 'Liquidity', '24h', 'APR', 'APY', 'yield', 'Total value locked', '$', 'price', 'Price', 'portfolio value', 'P&L', 'Sign', 'Submit']) {
      expect(unrelated).not.toContain(forbidden);
    }
  });
});
