import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import ProductV2Studio from './ProductV2Studio';

describe('Product V2 Studio presentation', () => {
  it('starts empty and exposes exact authoring, finalized authority, and an external signing boundary', () => {
    const html = renderToStaticMarkup(<ProductV2Studio />);
    expect(html).toContain('Author the payoff as data.');
    expect(html).toContain('signed knot numerators');
    expect(html).toContain('sole rounding boundary');
    expect(html).toContain('No Product has been authored or compiled.');
    expect(html).toContain('No private keys · no signing · no submission');
    expect(html).not.toContain('illustrative');
    expect(html).not.toContain('sample state');
    expect(html).not.toContain('value="1"');
    expect(html).not.toContain('Unsigned atomic v0 transaction');
  });
});
