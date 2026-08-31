import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import MarketGateCard from './MarketGateCard';
import { marketGateV1, type MarketGateV1 } from '@/lib/tradeFlowSteps';

const closed = (walls: ReadonlyArray<Readonly<{ name: string; detail: string }>>) => {
  const gate = marketGateV1(walls);
  return renderToStaticMarkup(<MarketGateCard gate={gate as Extract<MarketGateV1, { kind: 'closed' }>} />);
};

describe('the gate that stands instead of the stepper', () => {
  it('says the market is not open, in the wall’s own words, and points somewhere useful', () => {
    const html = closed([{ name: 'phase', detail: 'this Market is Retired — trading is only open while a Market is Open' }]);
    expect(html).toContain('This market is not open for trading.');
    expect(html).toContain('this Market is Retired — trading is only open while a Market is Open');
    expect(html).toContain('href="/markets"');
  });

  /**
   * The activation wall's last clause is the remedy: it tells a reader that
   * the thing they were about to go looking for does not exist, and that the
   * wait is not theirs to end. Splitting or trimming it would leave a refusal
   * a reader could only respond to by trying again.
   */
  it('keeps the activation wall’s operator clause, whole and in one element', () => {
    const detail = 'this Market founded a Direct trading capability but never switched it on — no activation root exists at Root111. Activation is the operator’s move, not yours.';
    const html = closed([{ name: 'activation', detail }]);
    expect(html).toContain('This market’s Direct trading was founded, but never switched on.');
    expect(html).toContain(`<p>${detail}</p>`);
  });

  it('never offers a control for a wall no reader can move', () => {
    const html = closed([{ name: 'phase', detail: 'this Market is Founding — trading is only open while a Market is Open' }]);
    expect(html).not.toContain('<button');
    expect(html).not.toContain('greyed-out');
    // And no stepper vocabulary leaks into a surface that has no steps.
    expect(html).not.toContain('flow-rail');
    expect(html).not.toContain('Sign');
  });
});
