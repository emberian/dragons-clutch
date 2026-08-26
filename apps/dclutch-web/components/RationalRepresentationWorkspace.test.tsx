import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import RationalRepresentationWorkspace from './RationalRepresentationWorkspace';

describe('Rational representation successor workbench', () => {
  it('separates executable transfer, chain-derived open/retirement, and unfinished terminal execution', () => {
    const html = renderToStaticMarkup(<RationalRepresentationWorkspace />);
    expect(html).toContain('Decimals are a label.');
    expect(html).toContain('raw-u64 economics');
    expect(html).toContain('Bearer transfer');
    expect(html).toContain('transaction-complete');
    expect(html).toContain('Build exact unsigned v0 + ALT packet');
    expect(html).toContain('Open native shards or a Structured receipt');
    expect(html).toContain('Build bounded unsigned v0 + ALT candidate');
    expect(html).toContain('four CapabilityV4 actions');
    expect(html).toContain('zero payout is valid');
    expect(html).toContain('closure only; not a payout route');
    expect(html).toContain('Product N / support K');
    expect(html).toContain('Wallet signing blocked by checked-release gate');
    expect(html).toContain('20 + 4K');
    expect(html).not.toContain('Submit');
    expect(html).not.toContain('sample token balance');
    expect(html).not.toContain('Convert to atoms');
  });
});
