import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import RationalRetireReceiptPanel from './RationalRetireReceiptPanel';

describe('compact Rational RetireReceipt workbench', () => {
  it('keeps signing behind the exact checked compact route', () => {
    const html = renderToStaticMarkup(<RationalRetireReceiptPanel />);
    expect(html).toContain('Retire a zero-supply Structured receipt');
    expect(html).toContain('fixed 400');
    expect(html).toContain('20 + 4S');
    expect(html).toContain('S is ordered nonzero support within representation K');
    expect(html).toContain('wallet never supplies N, K, outcomes, coefficients');
    expect(html).toContain('Build exact unsigned v0 + ALT candidate');
    expect(html).toContain('checked release required');
    expect(html).toContain('Sign retirement transaction');
    expect(html).toContain('Submit fully signed retirement');
    expect(html).toContain('available only after this page authenticates the compact V4 capability');
    expect(html).toContain('Download unsigned candidate');
    expect(html).not.toContain('Support count input');
  });
});
