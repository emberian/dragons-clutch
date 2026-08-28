import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import CellStrip from './CellStrip';

const U64_MAX = '18446744073709551615';

describe('CellStrip', () => {
  it('renders one cell per claim at N=2 with the exact atoms reachable without hover', () => {
    const html = renderToStaticMarkup(<CellStrip
      supplies={['500000000', '500000000']}
      winner={null}
      requiredBackingAtoms="500000000"
      requiredBackingNote="largest claim supply; every claim could still be the one that pays"
      caption="Issued claim atoms per cell, from the Claims aggregate."
      notes={['pays one collateral atom per claim atom if it wins', 'pays one collateral atom per claim atom if it wins']}
    />);
    expect(html.split('viz-hit').length - 1).toBe(2);
    expect(html).toContain('claim 0 · 500000000 atoms');
    expect(html).toContain('required backing · 500000000 atoms');
    expect(html).toContain('Issued claim atoms per cell');
  });

  it('scales to N=50 cells with sparse index labels', () => {
    const supplies = Array.from({ length: 50 }, (_, index) => String((index + 1) * 3));
    const html = renderToStaticMarkup(<CellStrip
      supplies={supplies}
      winner={null}
      requiredBackingAtoms="150"
      requiredBackingNote="largest claim supply"
      caption="Fifty cells."
    />);
    expect(html.split('viz-hit').length - 1).toBe(50);
    // Many cells keep their width and scroll in their own container instead
    // of compressing below a usable slot on narrow screens.
    expect(html).toContain('viz-scroll');
    expect(html).toContain('min-width');
    // Index axis stays sparse: first, middle, last — not fifty labels.
    expect(html).toContain('>0</text>');
    expect(html).toContain('>24</text>');
    expect(html).toContain('>49</text>');
    expect(html).not.toContain('>13</text>');
  });

  it('flips to the emphasis form once terminal: winner in the accent, named in words', () => {
    const html = renderToStaticMarkup(<CellStrip
      supplies={['10', '30', '20']}
      winner={1}
      requiredBackingAtoms="30"
      requiredBackingNote="winning claim supply; only winning claims can still be paid"
      caption="Settled."
      notes={['pays zero', 'winning · pays one collateral atom per claim atom', 'pays zero']}
    />);
    expect(html).toContain('var(--viz-accent)');
    expect(html).toContain('var(--viz-deemph)');
    // The default readout is the settled winner.
    expect(html).toContain('claim 1 · 30 atoms');
    expect(html).toContain('winning · pays one collateral atom per claim atom');
  });

  it('draws an exact zero as a baseline tick and a near-ceiling u64 without precision loss', () => {
    const html = renderToStaticMarkup(<CellStrip
      supplies={['0', U64_MAX]}
      winner={null}
      requiredBackingAtoms={U64_MAX}
      requiredBackingNote="largest claim supply"
      caption="Extremes."
    />);
    expect(html).toContain(`claim 1 · ${U64_MAX} atoms`);
    expect(html).toContain('claim 0 · 0 atoms');
  });

  it('says why in one plain sentence when there are no cells', () => {
    const html = renderToStaticMarkup(<CellStrip
      supplies={[]}
      winner={null}
      requiredBackingAtoms={null}
      requiredBackingNote={null}
      caption="unused"
      emptyReason="No Claims program is selected, so no cells have been read."
    />);
    expect(html).toContain('No Claims program is selected, so no cells have been read.');
    expect(html).not.toContain('<svg');
  });

  it('speaks in atoms, never in market-data vocabulary', () => {
    const html = renderToStaticMarkup(<CellStrip
      supplies={['1', '2', '3']}
      winner={2}
      requiredBackingAtoms="3"
      requiredBackingNote="winning claim supply"
      caption="Issued claim atoms per cell."
    />);
    for (const forbidden of ['price', 'Price', 'odds', 'probability', 'Probability', 'volume', 'liquidity', '$']) {
      expect(html).not.toContain(forbidden);
    }
  });
});
