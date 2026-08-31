import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import PreviewReceipt from './PreviewReceipt';
import { type DirectParticipantCrossingAdmissionV1 } from '@/lib/directParticipant';
import { type DenominationV1 } from '@/lib/quantity';
import { type DirectCrossingPlanV1 } from '@dclutch/sdk/directTicket';

const SIX_DECIMALS_V1: DenominationV1 = Object.freeze({ decimals: 6, unit: 'USDC', mint: 'mint' });

/** A buy of 500 claims at 35¢, fee-free: 175 USDC in, 500 claims out. */
function planV1(takerSide: 'buy' | 'sell'): DirectCrossingPlanV1 {
  return Object.freeze({
    takerSide,
    fill: 500_000_000n,
    executionPrice: 350_000n,
    taker: Object.freeze({ outcome: 1 }),
    note: 'Buying 500000000 claim atoms of outcome 1 at the maker’s signed price 350000',
    preview: Object.freeze({
      fill: 500_000_000n,
      executionPrice: 350_000n,
      grossCollateral: 175_000_000n,
      sellerFee: 0n,
      buyerFee: 0n,
      sellerNetCollateralCredit: 175_000_000n,
      buyerCollateralDebit: 175_000_000n,
      totalFeeTransfer: 0n,
    }),
  }) as unknown as DirectCrossingPlanV1;
}

const ADMISSION_V1 = Object.freeze({
  requiredAtoms: 175_000_000n,
  availableAtoms: 240_000_000n,
  resource: 'spendable collateral',
}) as unknown as DirectParticipantCrossingAdmissionV1;

const render = (takerSide: 'buy' | 'sell' = 'buy') => renderToStaticMarkup(<PreviewReceipt
  plan={planV1(takerSide)}
  admission={ADMISSION_V1}
  replaySlot="490712003"
  denomination={SIX_DECIMALS_V1}
  priceScale={1_000_000n}
  feeBasisPoints={0}
  outcomeLabel={(index) => (index === 1 ? 'Above one eighty' : `claim ${index}`)}
/>);

describe('the preview receipt', () => {
  const html = render();

  it('reads as a sentence, in display units, not four equal tiles', () => {
    expect(html).toContain('You buy');
    expect(html).toContain('500');
    expect(html).toContain('Above one eighty');
    expect(html).toContain('35¢');
    expect(html).toContain('175 USDC');
  });

  /**
   * The arithmetic is the protocol's. `buyerCollateralDebit` already includes
   * the fee, so the receipt names that field rather than adding numbers up
   * itself and hoping the sum matches what the chain will do.
   */
  it('shows the settled amount beside its principal and fee', () => {
    expect(html).toContain('principal');
    expect(html).toContain('0 fee (0 bps)');
    expect(render('sell')).toContain('You receive');
    expect(render('sell')).toContain('less');
  });

  it('says what the reader ends up holding, and what it pays if this outcome wins', () => {
    expect(html).toContain('You will hold');
    expect(html).toContain('If this outcome wins, they pay');
  });

  /**
   * The evidence line is not a decision, it is the proof the decision was
   * checked -- so it stays exact, and stays visible rather than moving into
   * the drawer with the rest of the atoms.
   */
  it('keeps the asset check exact and on the face of the receipt', () => {
    expect(html).toContain('Checked against your assets: 175000000 required / 240000000 available');
    expect(html).toContain('finalized through slot 490712003');
  });

  it('keeps the standing unsigned note verbatim, in one element', () => {
    expect(html).toContain('<p class="direct-status">Unsigned preview. Nothing is signed until you continue below.</p>');
  });

  /**
   * The four tiles the receipt replaced are not deleted, they are one click
   * down, in raw atoms. This is the mechanism by which humanizing costs
   * nothing -- and the drawer is really in the markup, so a guard over it is
   * not passing vacuously.
   */
  it('keeps the four exact tiles as the twin, one click away', () => {
    expect(html).toContain('<summary>Exact numbers, in raw atoms</summary>');
    expect(html).not.toContain('trade-v3-bytes" open');
    expect(html).toContain('500000000 claim atoms');
    expect(html).toContain('175000000 collateral atoms');
    expect(html).toContain('price scale 1000000');
    expect(html).toContain('Gross collateral');
    expect(html).toContain('Your fee');
    expect(html).toContain('Asset check');
  });

  it('explains the price scale exactly once, where a price first appears', () => {
    expect(html).toContain('Each claim pays 1 USDC if this outcome wins, nothing if it does not.');
    expect(html).toContain('about 35% likely');
    expect(html.split('Each claim pays 1 USDC').length - 1).toBe(1);
  });

  it('never invents a market-data metric', () => {
    for (const forbidden of ['volume', 'Volume', 'odds', 'probability', 'TVL', 'APR', 'APY', '$', '24h', 'P&L']) {
      expect(html).not.toContain(forbidden);
    }
  });
});
