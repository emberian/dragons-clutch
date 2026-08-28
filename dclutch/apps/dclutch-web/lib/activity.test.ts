import { Keypair, PublicKey, SystemProgram, TransactionMessage, VersionedTransaction } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import {
  ACTIVITY_MAX_TRANSACTIONS,
  inspectActivityV1,
  lamportDeltaV1,
} from './activity';
import { deriveClaimsAggregateAddressV2, deriveClaimsPositionAddressV2 } from './marketCoreV2';
import { type SignatureRecordObservation, type TransactionMetaObservation } from './rpc';

const OWNER = Keypair.fromSeed(new Uint8Array(32).fill(7)).publicKey.toBase58();
const CLAIMS = Keypair.fromSeed(new Uint8Array(32).fill(9)).publicKey.toBase58();
const MARKET = Keypair.fromSeed(new Uint8Array(32).fill(11)).publicKey.toBase58();
const OTHER = Keypair.fromSeed(new Uint8Array(32).fill(13)).publicKey.toBase58();

function signatureText(fill: number): string {
  // 88 characters of base58 text, the canonical width of one Ed25519 signature.
  return '3'.repeat(87) + String(fill % 9 + 1);
}

function record(overrides: Partial<SignatureRecordObservation> & Readonly<{ signature: string; slot: string }>): SignatureRecordObservation {
  return Object.freeze({
    succeeded: true,
    errorText: null,
    blockTime: null,
    memo: null,
    ...overrides,
  });
}

function transferTransactionBytes(fromText: string, toText: string): Uint8Array {
  const from = new PublicKey(fromText);
  const transaction = new VersionedTransaction(new TransactionMessage({
    payerKey: from,
    recentBlockhash: '11111111111111111111111111111111',
    instructions: [SystemProgram.transfer({ fromPubkey: from, toPubkey: new PublicKey(toText), lamports: 5 })],
  }).compileToV0Message());
  return transaction.serialize();
}

function meta(signature: string, overrides: Partial<TransactionMetaObservation>): TransactionMetaObservation {
  const transactionBytes = transferTransactionBytes(OWNER, OTHER);
  const decoded = VersionedTransaction.deserialize(transactionBytes);
  const addresses = decoded.message.staticAccountKeys.map((key) => key.toBase58());
  return Object.freeze({
    signature,
    slot: '90',
    blockTime: '1790000000',
    succeeded: true,
    errorText: null,
    error: null,
    feeLamports: '5000',
    computeUnits: null,
    accountAddresses: Object.freeze(addresses),
    preBalances: Object.freeze(addresses.map(() => '1000000')),
    postBalances: Object.freeze(addresses.map((_, index) => (index === 0 ? '994995' : '1000005'))),
    logMessages: Object.freeze([]),
    innerInstructions: Object.freeze([]),
    returnData: null,
    transactionBytes,
    ...overrides,
  });
}

describe('indexer-free activity', () => {
  it('signs the lamport delta exactly', () => {
    expect(lamportDeltaV1('100', '250')).toBe('+150');
    expect(lamportDeltaV1('250', '100')).toBe('-150');
    expect(lamportDeltaV1('7', '7')).toBe('0');
  });

  it('watches the owner plus the derived Claims Position of every named Market', async () => {
    const aggregate = deriveClaimsAggregateAddressV2(CLAIMS, MARKET);
    const position = deriveClaimsPositionAddressV2(CLAIMS, aggregate, OWNER);
    const asked: string[] = [];
    const activity = await inspectActivityV1({
      async signaturesForAddress(address) {
        asked.push(address);
        return Object.freeze([]);
      },
      async transaction() { throw new Error('no transaction should be read for an empty history'); },
    }, { owner: OWNER, claimsProgramId: CLAIMS, marketAddresses: [MARKET] });
    expect(asked).toEqual([OWNER, position]);
    expect(activity.watched.map((entry) => entry.address)).toEqual([OWNER, position]);
    expect(activity.entries).toHaveLength(0);
    expect(activity.reason).toContain("this node's answer");
  });

  it('merges duplicate signatures across watched addresses and orders newest first', async () => {
    const shared = signatureText(1);
    const older = signatureText(2);
    const aggregate = deriveClaimsAggregateAddressV2(CLAIMS, MARKET);
    const position = deriveClaimsPositionAddressV2(CLAIMS, aggregate, OWNER);
    const activity = await inspectActivityV1({
      async signaturesForAddress(address) {
        if (address === OWNER) {
          return Object.freeze([record({ signature: shared, slot: '90' }), record({ signature: older, slot: '40' })]);
        }
        return Object.freeze([record({ signature: shared, slot: '90' })]);
      },
      async transaction(signature) {
        return meta(signature, signature === older ? { slot: '40' } : {});
      },
    }, { owner: OWNER, claimsProgramId: CLAIMS, marketAddresses: [MARKET] });
    expect(activity.entries).toHaveLength(2);
    expect(activity.entries[0].signature).toBe(shared);
    expect(activity.entries[0].watchedAddresses.map((entry) => entry.address)).toEqual([OWNER, position]);
    expect(activity.entries[1].signature).toBe(older);
    expect(activity.entries[1].watchedAddresses.map((entry) => entry.address)).toEqual([OWNER]);
  });

  it('decodes program touches and the exact owner lamport delta from the finalized bytes', async () => {
    const signature = signatureText(3);
    const activity = await inspectActivityV1({
      async signaturesForAddress(address) {
        return address === OWNER ? Object.freeze([record({ signature, slot: '90' })]) : Object.freeze([]);
      },
      async transaction(candidate) { return meta(candidate, {}); },
    }, { owner: OWNER });
    expect(activity.entries).toHaveLength(1);
    const entry = activity.entries[0];
    expect(entry.detail.status).toBe('decoded');
    expect(entry.programs).toEqual([{ address: '11111111111111111111111111111111', label: 'System Program' }]);
    expect(entry.ownerLamportDelta).toBe('-5005');
    expect(entry.feeLamports).toBe('5000');
  });

  it('labels programs from the caller-selected role map, never by guessing', async () => {
    const signature = signatureText(4);
    const activity = await inspectActivityV1({
      async signaturesForAddress(address) {
        return address === OWNER ? Object.freeze([record({ signature, slot: '90' })]) : Object.freeze([]);
      },
      async transaction(candidate) { return meta(candidate, {}); },
    }, { owner: OWNER, programLabels: { '11111111111111111111111111111111': 'System Program (selected)' } });
    expect(activity.entries[0].programs[0]).toEqual({ address: '11111111111111111111111111111111', label: 'System Program (selected)' });
  });

  it('reports a listed-but-unserved transaction as the node refusing, not as empty', async () => {
    const signature = signatureText(5);
    const activity = await inspectActivityV1({
      async signaturesForAddress(address) {
        return address === OWNER ? Object.freeze([record({ signature, slot: '90', succeeded: false, errorText: '"InstructionError"' })]) : Object.freeze([]);
      },
      async transaction() { return null; },
    }, { owner: OWNER });
    const entry = activity.entries[0];
    expect(entry.detail).toEqual({ status: 'refused', reason: 'the node lists this signature but no longer serves its transaction' });
    expect(entry.succeeded).toBe(false);
    expect(entry.errorText).toBe('"InstructionError"');
    expect(entry.ownerLamportDelta).toBeNull();
  });

  it('bounds the transaction reads and says the listing was truncated', async () => {
    const signatures = Array.from({ length: 30 }, (_, index) => `${'2'.repeat(80)}${String(index + 10)}${'2'.repeat(6)}`);
    const activity = await inspectActivityV1({
      async signaturesForAddress(address) {
        if (address !== OWNER) return Object.freeze([]);
        return Object.freeze(signatures.map((signature, index) => record({ signature, slot: String(1000 - index) })));
      },
      async transaction(candidate) { return meta(candidate, {}); },
    }, { owner: OWNER });
    expect(activity.entries).toHaveLength(ACTIVITY_MAX_TRANSACTIONS);
    expect(activity.truncated).toBe(true);
  });

  it('refuses a noncanonical owner instead of deriving from it', async () => {
    await expect(inspectActivityV1({
      async signaturesForAddress() { return Object.freeze([]); },
      async transaction() { return null; },
    }, { owner: 'not-an-address' })).rejects.toThrow('canonical');
  });
});
