import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { decodeGeneralBatchV2 } from '@dclutch/sdk/generalClearingV1';
import ClearingPriceHistory, { type ClearingPriceHistoryEntryV1 } from './ClearingPriceHistory';

/**
 * The page reads this component real decoded batches; these fixtures are
 * exactly the bytes `generalClearingV1.test.ts` builds for the same witness
 * (`ClearingPriceV1Abi.zeroPricedTail`: prices `[60, 40, 0]`, residual
 * `[0, 0, 8]`, a ten-set mint), so a reader can compare the two directly.
 */

function id32(value: number): Uint8Array { return new Uint8Array(32).fill(value); }
function putU16(out: Uint8Array, offset: number, value: number): void { new DataView(out.buffer, out.byteOffset + offset, 2).setUint16(0, value, true); }
function putU32(out: Uint8Array, offset: number, value: number): void { new DataView(out.buffer, out.byteOffset + offset, 4).setUint32(0, value, true); }
function putU64(out: Uint8Array, offset: number, value: bigint): void { new DataView(out.buffer, out.byteOffset + offset, 8).setBigUint64(0, value, true); }

function batchBytes(status: 'collecting' | 'closed' | 'cleared', sequence: bigint): Uint8Array {
  const outcomeCount = 3;
  const out = new Uint8Array(296 + 16 * outcomeCount);
  out.set(new TextEncoder().encode('DCGBAT02'), 0);
  putU16(out, 8, 2);
  out[10] = 20;
  putU32(out, 12, outcomeCount);
  putU64(out, 16, sequence);
  putU64(out, 24, 7n); // generation
  out.set(id32(1), 32); // market
  out.set(id32(2), 64); // productId
  out.set(id32(3), 96); // configId
  putU64(out, 128, 100n); // priceScale
  putU64(out, 136, 80n); // collectionCloseSlot
  putU32(out, 144, 4); // maxOrders
  putU64(out, 152, 100n); // settlementCloseSlot
  out[160] = status === 'collecting' ? 1 : status === 'closed' ? 2 : 3;
  putU32(out, 164, 3); // orderCount
  putU64(out, 168, 9n); // openedRootRevision
  putU64(out, 176, status === 'collecting' ? 0n : 10n); // closedRootRevision
  putU64(out, 184, 10n); // committedQuoteReserve
  putU32(out, 192, 0); // cancelledCount
  if (status === 'cleared') {
    out.set(id32(0x51), 224); // clearedCandidateId
    putU64(out, 256, 1_000n); // clearedSlot
    out[264] = 1; // mint
    putU64(out, 272, 10n); // setsQuantity
    putU64(out, 280, 22n); // filledLots
    putU32(out, 288, 3); // liveOrderCount
    const prices = [60n, 40n, 0n];
    const residual = [0n, 0n, 8n];
    for (let outcome = 0; outcome < outcomeCount; outcome += 1) {
      putU64(out, 296 + 8 * outcome, prices[outcome]);
      putU64(out, 296 + 8 * outcomeCount + 8 * outcome, residual[outcome]);
    }
  }
  return out;
}

function entry(address: string, status: 'collecting' | 'closed' | 'cleared', sequence: bigint): ClearingPriceHistoryEntryV1 {
  return { address, decoded: decodeGeneralBatchV2(batchBytes(status, sequence)) };
}

function render(batches: ReadonlyArray<ClearingPriceHistoryEntryV1>): string {
  return renderToStaticMarkup(<ClearingPriceHistory batches={batches} />);
}

describe('ClearingPriceHistory', () => {
  it('renders nothing to show when no batch has opened', () => {
    expect(render([])).toContain('No batch has opened');
  });

  it('lists an uncleared batch by its status alone', () => {
    const markup = render([entry('BatchOne', 'collecting', 1n), entry('BatchTwo', 'closed', 2n)]);
    expect(markup).toContain('collecting');
    expect(markup).toContain('closed, awaiting a clearing');
    expect(markup).toContain('No batch here has cleared yet');
  });

  it('renders a cleared batch’s sequence, cleared slot, per-outcome prices, move, and stranded residual', () => {
    const markup = render([entry('BatchThree', 'cleared', 3n)]);
    expect(markup).toContain('>3<'); // sequence
    expect(markup).toContain('1000'); // cleared slot
    expect(markup).toContain('60%');
    expect(markup).toContain('40%');
    expect(markup).toContain('mint');
    expect(markup).toContain('10 sets');
    expect(markup).toContain('22'); // filled lots
    expect(markup).toContain('8 stranded');
    expect(markup).toContain('Prices sum to one by construction');
  });

  it('orders rows by sequence regardless of input order', () => {
    const markup = render([entry('Later', 'closed', 5n), entry('Earlier', 'collecting', 1n)]);
    const firstSequenceCell = markup.indexOf('>1<');
    const secondSequenceCell = markup.indexOf('>5<');
    expect(firstSequenceCell).toBeGreaterThan(-1);
    expect(secondSequenceCell).toBeGreaterThan(firstSequenceCell);
  });

  it('presents no market-data metric — raw facts only', () => {
    const markup = render([entry('BatchFour', 'cleared', 4n)]);
    for (const forbidden of ['volume', 'Volume', 'odds', 'probability', 'Probability', 'TVL', 'liquidity', 'Liquidity', 'APR', 'APY', 'yield']) {
      expect(markup).not.toContain(forbidden);
    }
  });
});
