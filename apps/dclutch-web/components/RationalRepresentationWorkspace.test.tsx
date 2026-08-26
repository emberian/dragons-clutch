import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import RationalRepresentationWorkspace from './RationalRepresentationWorkspace';

describe('Rational representation successor workbench', () => {
  it('exposes only the transaction-complete transfer and keeps unfinished Hot actions fail-closed', () => {
    const html = renderToStaticMarkup(<RationalRepresentationWorkspace />);
    expect(html).toContain('Decimals are a label.');
    expect(html).toContain('raw-u64 economics');
    expect(html).toContain('Bearer transfer');
    expect(html).toContain('transaction-complete');
    expect(html).toContain('Build exact unsigned v0 + ALT packet');
    expect(html).toContain('zero payout is valid');
    expect(html).toContain('closure only; not a payout route');
    expect(html).not.toContain('Submit');
    expect(html).not.toContain('sample token balance');
    expect(html).not.toContain('Convert to atoms');
  });
});
