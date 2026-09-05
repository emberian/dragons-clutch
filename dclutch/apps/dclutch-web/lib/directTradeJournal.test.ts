import { describe, expect, it } from 'vitest';

import {
  describeClaimChangeV1,
  directInlineJournalInputV1,
  directTradeBalanceChangesV1,
  directTradeFinalizedCompletionV1,
} from '@/lib/directTradeJournal';
import { type SignatureStatusObservation } from '@dclutch/sdk/rpc';

const SCOPE = Object.freeze({
  clusterGenesis: 'EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG',
  market: '57i7c6zwEEzySrt7a94FAbY6AWnEdK4jDZEYkTej4PrP',
  owner: '5oGySWQAKZ3fLmAwUbG6WifP7dCF6FRtriawtgxoCZXf',
});

const PLAN = Object.freeze({
  payer: SCOPE.owner,
  lookupTable: 'GcE6LWbduoATDgK8jsGyj2i8ywV37fcAYABKCmKgttDz',
  routeObservedSlot: '100',
  blockhashObservedSlot: '101',
  lastValidBlockHeight: '250',
  messageBase64: 'AAEC',
});

function status(overrides: Partial<SignatureStatusObservation>): SignatureStatusObservation {
  return Object.freeze({
    signature: '1'.repeat(64),
    known: true,
    slot: '102',
    confirmationStatus: 'finalized',
    succeeded: true,
    errorText: null,
    ...overrides,
  });
}

describe('the Direct trade journal seam', () => {
  it('binds the journal to the exact message bytes and the signed ticket', async () => {
    const message = Uint8Array.from([0, 1, 2]);
    const input = await directInlineJournalInputV1(SCOPE, '{"ticket":"signed"}', PLAN, message);
    expect(input.operation).toBe('direct-inline-v3');
    expect(input.operationDigest).toMatch(/^[0-9a-f]{64}$/);
    expect(input.intent).toBe('{"ticket":"signed"}');
    expect(JSON.parse(input.plan)).toMatchObject({ schema: 'dclutch-direct-inline-journal-plan-v1', lastValidBlockHeight: '250' });
    // A different packet is a different operation identity.
    const other = await directInlineJournalInputV1(SCOPE, '{"ticket":"signed"}', PLAN, Uint8Array.from([9, 9, 9]));
    expect(other.operationDigest).not.toBe(input.operationDigest);
  });

  it('refuses an empty ticket or empty message bytes', async () => {
    await expect(directInlineJournalInputV1(SCOPE, '  ', PLAN, Uint8Array.from([1]))).rejects.toThrow('signed taker ticket');
    await expect(directInlineJournalInputV1(SCOPE, '{"t":1}', PLAN, new Uint8Array())).rejects.toThrow('exact prepared message bytes');
  });

  it('claims completion only at finalized success', () => {
    expect(directTradeFinalizedCompletionV1(status({}))).toBe(true);
    expect(directTradeFinalizedCompletionV1(undefined)).toBe(false);
    expect(directTradeFinalizedCompletionV1(status({ known: false }))).toBe(false);
    expect(directTradeFinalizedCompletionV1(status({ succeeded: false }))).toBe(false);
    expect(directTradeFinalizedCompletionV1(status({ succeeded: null }))).toBe(false);
    expect(directTradeFinalizedCompletionV1(status({ confirmationStatus: 'confirmed' }))).toBe(false);
  });

  it('reports exact per-claim movement and refuses a width change', () => {
    const changes = directTradeBalanceChangesV1(
      { positionBalances: [100n, 50n], spendableCollateralAtoms: 1_000n },
      { positionBalances: [130n, 50n], spendableCollateralAtoms: 700n },
    );
    expect(changes.moved).toBe(true);
    expect(changes.claims).toHaveLength(2);
    expect(describeClaimChangeV1(changes.claims[0]!)).toBe('claim 0: gained 30 atoms (100 → 130)');
    expect(describeClaimChangeV1(changes.claims[1]!)).toBe('claim 1: unchanged at 50 atoms');
    expect(changes.spendableBefore).toBe(1_000n);
    expect(changes.spendableAfter).toBe(700n);
    expect(() => directTradeBalanceChangesV1(
      { positionBalances: [1n], spendableCollateralAtoms: 0n },
      { positionBalances: [1n, 2n], spendableCollateralAtoms: 0n },
    )).toThrow('changed width');
  });

  it('says so when a finalized crossing moved nothing', () => {
    const changes = directTradeBalanceChangesV1(
      { positionBalances: [5n], spendableCollateralAtoms: 9n },
      { positionBalances: [5n], spendableCollateralAtoms: 9n },
    );
    expect(changes.moved).toBe(false);
  });
});
