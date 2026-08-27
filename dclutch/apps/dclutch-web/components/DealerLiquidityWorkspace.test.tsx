import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import DealerLiquidityWorkspace from './DealerLiquidityWorkspace';

describe('Dealer V3 liquidity workbench', () => {
  it('exposes only executable chain-derived equity routes and the explicit wallet boundary', () => {
    const html = renderToStaticMarkup(<DealerLiquidityWorkspace />);
    expect(html).toContain('Liquidity is a residual.');
    expect(html).toContain('six executable shapes');
    expect(html).toContain('Canonical Dealer equity request');
    expect(html).toContain('Hot38 + admitted-AOT + runtime route manifest');
    expect(html).toContain('Build exact unsigned v0 transaction');
    expect(html).toContain('Sign as transaction payer');
    expect(html).toContain('Download exact packet');
    expect(html).toContain('LP open/close and scenario trading remain hidden');
    expect(html).not.toContain('Submit signed transaction');
    expect(html).not.toContain('mock balance');
    expect(html).not.toContain('sample liquidity');
  });
});
