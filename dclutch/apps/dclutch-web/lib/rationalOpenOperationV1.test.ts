import {
  AddressLookupTableAccount,
  PublicKey,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
} from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import {
  writeUnsignedClientOperationJournalV1,
  type ClientOperationJournalStorageV1,
} from './clientOperationJournal';
import {
  rationalOpenJournalInputV1,
  restoreRationalOpenJournalV1,
} from './rationalOpenOperationV1';
import {
  type RationalOpenCandidateV4,
  type RationalOpenChainInspectionV4,
} from './rationalOpenChainV4';

const bytes = (value: number) => new Uint8Array(32).fill(value);
const address = (value: number) => new PublicKey(bytes(value)).toBase58();

class MemoryStorage implements ClientOperationJournalStorageV1 {
  readonly values = new Map<string, string>();
  get length() { return this.values.size; }
  key(index: number) { return [...this.values.keys()][index] ?? null; }
  getItem(key: string) { return this.values.get(key) ?? null; }
  setItem(key: string, value: string) { this.values.set(key, value); }
  removeItem(key: string) { this.values.delete(key); }
}

function fixture() {
  const owner = address(1);
  const market = address(2);
  const loaded = new PublicKey(address(3));
  const table = new AddressLookupTableAccount({
    key: new PublicKey(address(4)),
    state: {
      deactivationSlot: 18_446_744_073_709_551_615n,
      lastExtendedSlot: 1,
      lastExtendedSlotStartIndex: 0,
      authority: undefined,
      addresses: [loaded],
    },
  });
  const outerBytes = Uint8Array.of(1, 2, 3, 4);
  const instruction = new TransactionInstruction({
    programId: new PublicKey(address(5)),
    keys: [{ pubkey: loaded, isSigner: false, isWritable: true }],
    data: outerBytes as Buffer,
  });
  const transaction = new VersionedTransaction(new TransactionMessage({
    payerKey: new PublicKey(owner),
    recentBlockhash: address(6),
    instructions: [instruction],
  }).compileToV0Message([table]));
  const wireBytes = transaction.serialize();
  const poststate = Object.freeze({
    context: Object.freeze({
      claimsProgram: address(10), descriptorId: bytes(11), actor: owner,
      representationAuthority: address(12), aggregate: address(13), market,
      releaseSet: bytes(14), registry: address(15), product: bytes(16), realm: bytes(17),
      generation: 9n, outcomes: 3, basis: bytes(18), custodyContext: bytes(19),
    }),
    replay: Object.freeze({ address: address(20), revision: 4n }),
    aggregate: Object.freeze({ address: address(13), revision: 7n, balances: Object.freeze([5n, 6n, 7n]) }),
    positions: Object.freeze([
      Object.freeze({ address: address(21), owner, revision: 8n, balances: Object.freeze([5n, 4n, 7n]) }),
      Object.freeze({ address: address(22), owner: address(23), revision: 9n, balances: Object.freeze([1n, 2n, 3n]) }),
    ]),
    receipt: null,
    assets: Object.freeze([Object.freeze({
      mint: address(24), mintSupply: 120n, actorAccount: address(25), actorAmount: 80n,
      structuredAccount: address(26), structuredAmount: 40n,
    })]),
  });
  const inspection = Object.freeze({
    observedSlot: '90', action: 'denominate' as const, payer: owner, actor: owner, market,
    generation: 9n, representationWidth: 3, resultOutcomeCount: 5, selectedOutcome: 1,
    rawQuantity: 2n, displayDecimals: 0, descriptorId: bytes(11), tokenBehaviorDigest: bytes(27),
    capabilityDigest: bytes(28), rootDigest: bytes(29),
    family: Object.freeze({
      action: 'denominate' as const, familyBytes: Uint8Array.of(7), familyDigest: bytes(30),
      childRequest: Uint8Array.of(8), childDigest: bytes(31), assetCount: 1, claimsAccountCount: 36,
      rawQuantity: 2n, rawReceiptDelta: 0n, rawShardDeltas: Object.freeze([20n]),
    }),
    fixedAccounts: Object.freeze([]), physicalClaimsAccounts: Object.freeze([]), lookupTable: table,
    poststate, executionStatus: 'blocked' as const, refusal: 'physical release pending',
  }) satisfies RationalOpenChainInspectionV4;
  const candidate = Object.freeze({
    transaction, instruction, outerBytes, wireBytes, requiredSigners: Object.freeze([owner]), loadedAddresses: 1,
    logicalClaimsAccounts: 36, physicalClaimsAccounts: 31,
    executionStatus: 'blocked' as const, refusal: inspection.refusal,
  }) satisfies RationalOpenCandidateV4;
  return { scope: { clusterGenesis: address(32), market, owner }, inspection, candidate };
}

describe('Rational open crash journal', () => {
  it('round-trips the exact unsigned v0 message and finalized atom ledger', async () => {
    const value = fixture();
    const input = await rationalOpenJournalInputV1(value.scope, value.inspection, value.candidate, '120');
    const journal = await writeUnsignedClientOperationJournalV1(new MemoryStorage(), input);
    const restored = await restoreRationalOpenJournalV1(journal);
    expect(restored.transaction.serialize()).toEqual(value.candidate.wireBytes);
    expect(restored.intent.action).toBe('denominate');
    expect(restored.poststate.positions[0]?.balances).toEqual([5n, 4n, 7n]);
    expect(restored.poststate.assets[0]).toMatchObject({ mintSupply: 120n, actorAmount: 80n });
  });

  it('refuses packet, poststate, signer, and unknown-field substitutions', async () => {
    const value = fixture();
    const input = await rationalOpenJournalInputV1(value.scope, value.inspection, value.candidate, '120');
    const journal = await writeUnsignedClientOperationJournalV1(new MemoryStorage(), input);
    const plan = JSON.parse(journal.plan) as Record<string, unknown>;
    const poststate = plan.poststate as Record<string, unknown>;
    const assets = poststate.assets as Array<Record<string, unknown>>;
    assets[0]!.actorAmount = '81';
    await expect(restoreRationalOpenJournalV1({ ...journal, plan: JSON.stringify(plan) }))
      .rejects.toThrow('operation digest changed');

    const unknown = JSON.parse(journal.plan) as Record<string, unknown>;
    (unknown.poststate as Record<string, unknown>).invented = true;
    await expect(restoreRationalOpenJournalV1({ ...journal, plan: JSON.stringify(unknown) }))
      .rejects.toThrow('missing or unknown fields');

    const another = fixture();
    await expect(rationalOpenJournalInputV1(
      { ...another.scope, owner: address(99) }, another.inspection, another.candidate, '120',
    )).rejects.toThrow('signer disagree');
  });
});
