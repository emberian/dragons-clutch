import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import RationalRetireReceiptPanel from './RationalRetireReceiptPanel';

describe('compact Rational RetireReceipt workbench', () => {
  it('shows exact derived geometry, packet export, and the honest release gate', () => {
    const html = renderToStaticMarkup(<RationalRetireReceiptPanel />);
    expect(html).toContain('Retire a zero-supply Structured receipt');
    expect(html).toContain('fixed 400');
    expect(html).toContain('20 + 4S');
    expect(html).toContain('S is ordered nonzero support within representation K');
    expect(html).toContain('wallet never supplies N, K, outcomes, coefficients');
    expect(html).toContain('Build exact unsigned v0 + ALT candidate');
    expect(html).toContain('Wallet signing blocked by checked-release gate');
    expect(html).toContain('Download unsigned candidate');
    expect(html).not.toContain('Submit');
    expect(html).not.toContain('Support count input');
  });
});
