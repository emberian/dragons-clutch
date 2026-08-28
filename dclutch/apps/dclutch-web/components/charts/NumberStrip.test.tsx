import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import NumberStrip from './NumberStrip';

describe('NumberStrip', () => {
  it('shows a read zero as zero and an unread value as no value, never the other way', () => {
    const html = renderToStaticMarkup(<NumberStrip
      stats={[
        { label: 'Markets founded', value: '0', detail: 'finalized Core accounts' },
        { label: 'Collateral locked', value: null, detail: 'Hoard principal, raw atoms' },
        { label: 'Resolutions run', value: '0', detail: 'terminal receipts written' },
      ]}
      provenance="Read finalized off devnet at floor 401882211; the collateral read refused and says so above."
    />);
    expect(html).toContain('Markets founded');
    expect(html).toContain('>0</strong>');
    expect(html).toContain('>—</strong>');
    expect(html).toContain('Read finalized off devnet at floor 401882211');
  });

  it('carries the no-deployment state as one plain sentence', () => {
    const html = renderToStaticMarkup(<NumberStrip
      stats={[
        { label: 'Markets founded', value: null, detail: 'finalized Core accounts' },
        { label: 'Collateral locked', value: null, detail: 'Hoard principal, raw atoms' },
        { label: 'Resolutions run', value: null, detail: 'terminal receipts written' },
      ]}
      provenance="No chain has been asked yet: this page reads only when a deployment is named."
    />);
    expect(html).toContain('No chain has been asked yet');
    expect(html.split('>—</strong>').length - 1).toBe(3);
  });
});
