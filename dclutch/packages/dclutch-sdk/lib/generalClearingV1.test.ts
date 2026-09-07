import { describe, expect, it } from 'vitest';

import {
  clearingPricesV1,
  decodeGeneralBatchV2,
  decodeGeneralOrderV2,
  generalOrderIdV2,
} from './generalClearingV1';

/**
 * Byte fixtures for the joint clearing's two records. The cleared-batch
 * witness (prices `[60, 40, 0]`, residual `[0, 0, 8]`, a ten-set mint) is
 * `ClearingPriceV1Abi.zeroPricedTail` in
 * `formal/dclutch-semantics/DClutchSemantics/ClearingPriceV1Abi.lean`, kept
 * byte-identical here so the two authorities can be read side by side.
 */

function id32(value: number): Uint8Array { return new Uint8Array(32).fill(value); }
function putU16(out: Uint8Array, offset: number, value: number): void { new DataView(out.buffer, out.byteOffset + offset, 2).setUint16(0, value, true); }
function putU32(out: Uint8Array, offset: number, value: number): void { new DataView(out.buffer, out.byteOffset + offset, 4).setUint32(0, value, true); }
function putU64(out: Uint8Array, offset: number, value: bigint): void { new DataView(out.buffer, out.byteOffset + offset, 8).setBigUint64(0, value, true); }

type BatchStatusV2 = 'collecting' | 'closed' | 'cleared';

type BatchOptions = Readonly<{
  prices?: ReadonlyArray<bigint>;
  residual?: ReadonlyArray<bigint>;
  setsMove?: number;
  setsQuantity?: bigint;
  filledLots?: bigint;
  liveOrderCount?: number;
}>;

/** A canonically shaped `296 + 16N` batch record, N=3, price scale 100. */
function batchBytes(status: BatchStatusV2, options: BatchOptions = {}): Uint8Array {
  const outcomeCount = 3;
  const out = new Uint8Array(296 + 16 * outcomeCount);
  out.set(new TextEncoder().encode('DCGBTCH2'), 0);
  putU16(out, 8, 2);
  out[10] = 20;
  putU32(out, 12, outcomeCount);
  putU64(out, 16, 2n); // sequence
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
    out[264] = options.setsMove ?? 1; // mint
    putU64(out, 272, options.setsQuantity ?? 10n);
    putU64(out, 280, options.filledLots ?? 22n);
    putU32(out, 288, options.liveOrderCount ?? 3);
    const prices = options.prices ?? [60n, 40n, 0n];
    const residual = options.residual ?? [0n, 0n, 8n];
    for (let outcome = 0; outcome < outcomeCount; outcome += 1) {
      putU64(out, 296 + 8 * outcome, prices[outcome]);
      putU64(out, 296 + 8 * outcomeCount + 8 * outcome, residual[outcome]);
    }
  }
  return out;
}

type OrderOptions = Readonly<{
  side?: number;
  outcomeCount?: number;
  outcomeLo?: number;
  outcomeHi?: number;
  claimsPerLot?: bigint;
  phase?: number;
  rowOverride?: Readonly<{ outcome: number; receive: bigint; deliver: bigint }>;
}>;

/** A canonically shaped `216 + 16N` placed order record, side buy on [1,1]. */
function orderBytes(options: OrderOptions = {}): Uint8Array {
  const outcomeCount = options.outcomeCount ?? 3;
  const side = options.side ?? 1;
  const outcomeLo = options.outcomeLo ?? 1;
  const outcomeHi = options.outcomeHi ?? 1;
  const claimsPerLot = options.claimsPerLot ?? 1_000n;
  const out = new Uint8Array(216 + 16 * outcomeCount);
  out.set(new TextEncoder().encode('DCGSORD2'), 0);
  putU16(out, 8, 2);
  out[10] = 21;
  putU32(out, 12, outcomeCount);
  putU64(out, 16, 2n); // nonce
  putU64(out, 24, 0n); // minQuoteCreditPerLot
  out.set(id32(3), 32); // ownerId
  out.set(id32(1), 64); // market
  out.set(id32(4), 96); // batchId
  putU64(out, 128, 7n); // generation
  putU64(out, 136, 2n); // maxLots
  putU64(out, 144, 5n); // maxQuoteDebitPerLot
  putU64(out, 152, 100n); // validUntilSlot
  out[160] = side;
  putU32(out, 164, outcomeLo);
  putU32(out, 168, outcomeHi);
  putU64(out, 176, claimsPerLot);
  out[184] = options.phase ?? 1; // placed
  putU64(out, 192, 50n); // admittedSlot
  putU64(out, 200, 0n); // releasedSlot
  for (let outcome = 0; outcome < outcomeCount; outcome += 1) {
    const covers = outcome >= outcomeLo && outcome <= outcomeHi;
    let receive = covers && side === 1 ? claimsPerLot : 0n;
    let deliver = covers && side === 2 ? claimsPerLot : 0n;
    if (options.rowOverride && options.rowOverride.outcome === outcome) {
      receive = options.rowOverride.receive;
      deliver = options.rowOverride.deliver;
    }
    putU64(out, 216 + 16 * outcome, receive);
    putU64(out, 216 + 16 * outcome + 8, deliver);
  }
  return out;
}

describe('decodeGeneralBatchV2', () => {
  it('decodes a vacant closed batch with no clearing', () => {
    const decoded = decodeGeneralBatchV2(batchBytes('closed'));
    expect(decoded.status).toBe('closed');
    expect(decoded.clearing).toBeNull();
    expect(decoded.liveOrderCount).toBe(3);
  });

  it('decodes a cleared batch with the design note witness — prices [60,40,0], residual [0,0,8]', () => {
    const decoded = decodeGeneralBatchV2(batchBytes('cleared'));
    expect(decoded.status).toBe('cleared');
    expect(decoded.clearing).not.toBeNull();
    expect(decoded.clearing?.prices).toEqual([60n, 40n, 0n]);
    expect(decoded.clearing?.residual).toEqual([0n, 0n, 8n]);
    expect(decoded.clearing?.setsMove).toBe('mint');
    expect(decoded.clearing?.setsQuantity).toBe(10n);
    expect(decoded.clearing?.filledLots).toBe(22n);
    expect(decoded.clearing?.liveOrderCount).toBe(3);

    const view = clearingPricesV1(decoded);
    expect(view).not.toBeNull();
    expect(view?.byConstruction).toBe(true);
    expect(view?.fractions).toEqual([0.6, 0.4, 0]);
  });

  it('returns null from clearingPricesV1 for an uncleared batch', () => {
    expect(clearingPricesV1(decodeGeneralBatchV2(batchBytes('collecting')))).toBeNull();
  });

  it('refuses a cleared status over a vacant tail', () => {
    const wire = batchBytes('closed');
    wire[160] = 3; // flip to cleared without filling the tail
    expect(() => decodeGeneralBatchV2(wire)).toThrow(/clearing/);
  });

  it('refuses a closed status over a nonvacant tail', () => {
    const wire = batchBytes('cleared');
    wire[160] = 2; // flip to closed while the tail stays filled
    expect(() => decodeGeneralBatchV2(wire)).toThrow(/not vacant/);
  });

  it('refuses prices off the simplex', () => {
    const wire = batchBytes('cleared', { prices: [60n, 41n, 0n], residual: [0n, 0n, 8n] });
    expect(() => decodeGeneralBatchV2(wire)).toThrow(/sum to the price scale/);
  });

  it('refuses a residual on a priced outcome', () => {
    const wire = batchBytes('cleared', { prices: [60n, 40n, 0n], residual: [0n, 5n, 8n] });
    expect(() => decodeGeneralBatchV2(wire)).toThrow(/strands a residual behind a priced outcome/);
  });
});

describe('decodeGeneralOrderV2', () => {
  it('decodes a placed single-outcome buy', () => {
    const decoded = decodeGeneralOrderV2(orderBytes());
    expect(decoded.side).toBe('buy');
    expect(decoded.phase).toBe('placed');
    expect(decoded.rows).toEqual([{ receive: 0n, deliver: 0n }, { receive: 1_000n, deliver: 0n }, { receive: 0n, deliver: 0n }]);
  });

  it('decodes a placed sell across a wider interval', () => {
    const decoded = decodeGeneralOrderV2(orderBytes({ side: 2, outcomeLo: 0, outcomeHi: 2, claimsPerLot: 7n }));
    expect(decoded.side).toBe('sell');
    expect(decoded.rows).toEqual([{ receive: 0n, deliver: 7n }, { receive: 0n, deliver: 7n }, { receive: 0n, deliver: 7n }]);
  });

  it('refuses an order whose rows disagree with its shape', () => {
    const wire = orderBytes({ rowOverride: { outcome: 1, receive: 1_000n, deliver: 5n } });
    expect(() => decodeGeneralOrderV2(wire)).toThrow(/rows disagree with the shape/);
  });

  it('refuses an empty interval', () => {
    const wire = orderBytes({ outcomeCount: 6, outcomeLo: 4, outcomeHi: 3, claimsPerLot: 1n });
    expect(() => decodeGeneralOrderV2(wire)).toThrow(/nonempty interval/);
  });

  it('refuses an unknown side', () => {
    const wire = orderBytes({ side: 3 });
    expect(() => decodeGeneralOrderV2(wire)).toThrow(/side tag is unknown/);
  });

  it('derives the order identity as the sha256 of its 184-byte header alone', async () => {
    const wire = orderBytes();
    const id = await generalOrderIdV2(wire);
    expect(id).toMatch(/^[0-9a-f]{64}$/);
    const mutatedStateOnly = orderBytes();
    putU64(mutatedStateOnly, 192, 999n); // admittedSlot lives in the state window, not the header
    expect(await generalOrderIdV2(mutatedStateOnly)).toBe(id);
    const mutatedHeader = orderBytes();
    putU64(mutatedHeader, 16, 3n); // nonce lives in the header
    expect(await generalOrderIdV2(mutatedHeader)).not.toBe(id);
  });
});
