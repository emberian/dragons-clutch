import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import RationalOpenPanel from './RationalOpenPanel';

describe('Rational CapabilityV4 open workbench', () => {
  it('exposes all four chain-derived raw-atom actions without claiming execution', () => {
    const html = renderToStaticMarkup(<RationalOpenPanel />);
    expect(html).toContain('Open native shards or a Structured receipt');
    expect(html).toContain('Denominate native claim');
    expect(html).toContain('Reconstitute native claim');
    expect(html).toContain('Issue Structured receipt');
    expect(html).toContain('Unwrap Structured receipt');
    expect(html).toContain('32 + 4K');
    expect(html).toContain('one representation coordinate');
    expect(html).toContain('zero coefficients remain zero-delta rows');
    expect(html).toContain('Build bounded unsigned v0 + ALT candidate');
    expect(html).toContain('Wallet signing blocked by checked-release gate');
    expect(html).not.toContain('Convert to atoms');
    expect(html).not.toContain('Submit');
  });
});
