import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import PositionBars from './PositionBars';

describe('PositionBars', () => {
  it('draws the merge floor at the smallest balance while the Market is Open', () => {
    const html = renderToStaticMarkup(<PositionBars
      balances={['12', '7', '30']}
      claim={{ kind: 'mergeable', completeSetsAtoms: '7' }}
      caption="Owned claim atoms per claim."
    />);
    expect(html).toContain('merge floor · 7 complete sets');
    // The set count is scale-free and is stated. What a set is WORTH is not,
    // and this figure reads no record that carries it, so it says so rather
    // than drawing the scale-1 assumption as a fact.
    expect(html).toContain('what each set is worth in collateral is this Market\u2019s basis scale, which this figure has not read');
    expect(html).not.toContain('one collateral atom');
    expect(html).toContain('claim 2 · 30 atoms');
    expect(html).toContain('var(--viz-law)');
  });

  it('states the collateral exactly when the caller has authenticated the basis scale', () => {
    const html = renderToStaticMarkup(<PositionBars
      balances={['12', '7', '30']}
      claim={{ kind: 'mergeable', completeSetsAtoms: '7', mergeableCollateralAtoms: '7000000' }}
      caption="Owned claim atoms per claim."
    />);
    expect(html).toContain('these sets merge back into 7000000 collateral atoms');
    expect(html).not.toContain('has not read');
  });

  it('says plainly when a zero balance means no complete set exists', () => {
    const html = renderToStaticMarkup(<PositionBars
      balances={['12', '0']}
      claim={{ kind: 'mergeable', completeSetsAtoms: '0' }}
      caption="One empty claim."
    />);
    expect(html).toContain('merge floor · 0 complete sets');
    expect(html).toContain('one claim balance is zero, so no complete set exists to merge');
  });

  it('flips to emphasis once redeemable: the winner in the accent, losers named as paying zero', () => {
    const html = renderToStaticMarkup(<PositionBars
      balances={['5', '9', '4']}
      claim={{ kind: 'redeemable', winningClaim: 1, redeemableAtoms: '9' }}
      caption="Settled position."
    />);
    expect(html).toContain('var(--viz-accent)');
    expect(html).toContain('var(--viz-deemph)');
    expect(html).toContain('redeemable · 9 atoms');
    expect(html).toContain('winning claim 1; every losing claim pays zero');
    expect(html).toContain('losing · pays zero');
  });

  it('renders balances plainly when the phase admits no transition', () => {
    const html = renderToStaticMarkup(<PositionBars
      balances={['3', '3']}
      claim={{ kind: 'unavailable' }}
      caption="Phase admits neither merge nor redemption right now."
    />);
    expect(html).toContain('claim 0 · 3 atoms');
    expect(html).not.toContain('merge floor');
    expect(html).not.toContain('redeemable ·');
  });

  it('says why in one plain sentence when there are no balances', () => {
    const html = renderToStaticMarkup(<PositionBars
      balances={[]}
      claim={{ kind: 'unavailable' }}
      caption="unused"
      emptyReason="The Position decoded zero claims wide, which the aggregate refuses upstream."
    />);
    expect(html).toContain('The Position decoded zero claims wide, which the aggregate refuses upstream.');
    expect(html).not.toContain('<svg');
  });
});
