import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import RationalRepresentationWorkspace from './RationalRepresentationWorkspace';

describe('Rational representation successor workbench', () => {
  it('shows available transfer, opening, retirement, and payout actions', () => {
    const html = renderToStaticMarkup(<RationalRepresentationWorkspace />);
    expect(html).toContain('Claims &amp;');
    expect(html).toContain('Bearer transfer');
    expect(html).toContain('Sign and send');
    expect(html).toContain('Build unsigned transfer');
    expect(html).toContain('Open native shards or a Structured receipt');
    expect(html).toContain('Build unsigned opening transaction');
    expect(html).toContain('Denominate, reconstitute, issue, or unwrap');
    expect(html).toContain('Preview terminal payout');
    expect(html).toContain('A losing claim pays zero.');
    expect(html).toContain('Redemption signing unavailable');
    expect(html).toContain('Close a zero-supply receipt');
    expect(html).toContain('Wallet signing unavailable');
    expect(html).toContain('20 + 4S');
    expect(html).toContain('Sign and send the transfer');
    expect(html).toContain('Submit fully signed transfer');
    expect(html).toContain('Discard unsigned saved plan');
    expect(html).toContain('Saved transfers resume from their transaction signature');
    expect(html).toContain('Submit fully signed retirement');
    expect(html).not.toContain('sample token balance');
    expect(html).not.toContain('Convert to atoms');
  });
});
