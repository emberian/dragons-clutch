import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import SupplyShareStrip from './SupplyShareStrip';

/**
 * The issuance split strip: exact shares, self-explaining evenness, a
 * zero cell that stays visible, and the built-in exact-value table — with
 * the caller's what-this-is-not caption rendered, never optional.
 */
describe('SupplyShareStrip', () => {
  const CAPTION = 'Shares of issued claims — where the claims sit, not a forecast.';

  it('draws the flagship’s even founding split and says why it is even', () => {
    const html = renderToStaticMarkup(<SupplyShareStrip
      supplies={['500000000', '500000000', '500000000', '500000000']}
      outcomes={['Below the range', 'Inside the range', 'Above the range', 'The source failed to report']}
      caption={CAPTION}
    />);
    expect(html).toContain('25.00%');
    // Renegotiated 2026-08-31: the readout used to append "evenly split:
    // issuance has not leaned toward any outcome yet" to an already-labelled
    // set of equal bars, and the table behind it was called "Exact issued
    // supply behind every share". Both deleted; the percentages say it.
    expect(html).not.toContain('has not leaned');
    expect(html).toContain('Below the range');
    expect(html).toContain('Exact numbers');
    expect(html).toContain('<td>total</td><td>100.00%</td><td>2000000000</td>');
    expect(html).toContain(CAPTION);
  });

  it('keeps a zero cell visible and exact, and drops the evenness reading', () => {
    const html = renderToStaticMarkup(<SupplyShareStrip
      supplies={['0', '300', '100']}
      caption={CAPTION}
    />);
    expect(html).toContain('0.00%');
    expect(html).toContain('75.00%');
    expect(html).toContain('25.00%');
    expect(html).not.toContain('evenly split');
    // The zero cell still occupies markup: three hit targets, three bars.
    expect(html.split('viz-hit').length - 1).toBe(3);
  });

  it('renders the stated empty reason instead of an invented uniform split', () => {
    const html = renderToStaticMarkup(<SupplyShareStrip
      supplies={['0', '0']}
      caption={CAPTION}
      emptyReason="Nothing issued yet."
    />);
    expect(html).toContain('Nothing issued yet.');
    expect(html).not.toContain('viz-hit');
    expect(html).not.toContain('%');
  });

  it('names cells by index alone when no editorial outcomes are supplied', () => {
    const html = renderToStaticMarkup(<SupplyShareStrip supplies={['1', '3']} caption={CAPTION} />);
    expect(html).toContain('outcome 1');
    expect(html).toContain('75.00%');
    expect(html).not.toContain('· undefined');
  });
});
