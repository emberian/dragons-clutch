import { AddressLookupTableAccount, Keypair, PublicKey, VersionedTransaction } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { decodeTokenBehaviorSelectionV2, encodeTokenBehaviorSelectionV2, buildUnsignedBearerTransferV2,
  TOKEN_2022_PROGRAM_ID, type BearerTransferInspectionV2 } from '@dclutch/sdk/rationalTokenV2';
import { bearerTransferJournalInputV2, nextBearerTransferSignerV2, restoreBearerTransferJournalV2 } from './bearerTransferOperationV2';
import { markClientOperationSubmittedV1, transactionSignatureV1, writeUnsignedClientOperationJournalV1,
  type ClientOperationJournalStorageV1 } from './clientOperationJournal';

const address = (byte: number) => new PublicKey(new Uint8Array(32).fill(byte)).toBase58();
class MemoryStorage implements ClientOperationJournalStorageV1 {
  values = new Map<string, string>(); get length() { return this.values.size; }
  key(index: number) { return [...this.values.keys()][index] ?? null; }
  getItem(key: string) { return this.values.get(key) ?? null; }
  setItem(key: string, value: string) { this.values.set(key, value); }
  removeItem(key: string) { this.values.delete(key); }
}

describe('Bearer transfer operation journal', () => {
  it('round-trips exact payer, actor, message, and poststate arithmetic', async () => {
    const payerKey = Keypair.generate(); const authorityKey = Keypair.generate();
    const payer = payerKey.publicKey.toBase58(); const authority = authorityKey.publicKey.toBase58();
    const mint = address(3); const source = address(4); const destination = address(5);
    const realm = new Uint8Array(32).fill(6); const release = new Uint8Array(32).fill(7);
    const inspection: BearerTransferInspectionV2 = Object.freeze({
      observedSlot: '8', payer, authority, coreProgram: address(9), market: address(10), marketPhase: 'Open', generation: 1n,
      registryProgram: address(11), selectionRecord: address(12), selectionDigest: new Uint8Array(32).fill(13),
      selection: decodeTokenBehaviorSelectionV2(encodeTokenBehaviorSelectionV2(realm, release), realm, release),
      mint: Object.freeze({ mint, controller: address(14), rawSupply: 100n, displayDecimals: 6, metadata: 'absent' }),
      source: Object.freeze({ address: source, mint, owner: authority, rawAmount: 70n }),
      destination: Object.freeze({ address: destination, mint, owner: address(15), rawAmount: 2n }),
      lookupTable: new AddressLookupTableAccount({ key: new PublicKey(address(16)), state: {
        deactivationSlot: 18_446_744_073_709_551_615n, lastExtendedSlot: 1, lastExtendedSlotStartIndex: 0, authority: undefined,
        addresses: [source, mint, destination, TOKEN_2022_PROGRAM_ID].map((value) => new PublicKey(value)),
      } }),
    });
    const plan = buildUnsignedBearerTransferV2(inspection, address(17), 20n);
    expect(nextBearerTransferSignerV2(plan.transaction, payer, authority)).toEqual({
      address: authority, role: 'source transfer authority',
    });
    const partial = VersionedTransaction.deserialize(plan.transaction.serialize()); partial.sign([authorityKey]);
    expect(nextBearerTransferSignerV2(partial, payer, authority)).toEqual({ address: payer, role: 'transaction payer' });
    partial.sign([payerKey]);
    expect(nextBearerTransferSignerV2(partial, payer, authority)).toBeNull();
    const storage = new MemoryStorage();
    const journal = await writeUnsignedClientOperationJournalV1(storage, await bearerTransferJournalInputV2({
      clusterGenesis: address(18), market: inspection.market, owner: payer,
    }, plan, '200'));
    await expect(restoreBearerTransferJournalV2(journal)).resolves.toMatchObject({
      lastValidBlockHeight: '200', poststate: { payer, authority, sourceBefore: 70n, sourceAfter: 50n, destinationAfter: 22n },
    });
    const submitted = await markClientOperationSubmittedV1(storage, journal,
      transactionSignatureV1(partial.signatures[0]!), partial.serialize());
    await expect(restoreBearerTransferJournalV2(submitted)).resolves.toMatchObject({
      poststate: { payer, authority, sourceAfter: 50n, destinationAfter: 22n },
    });
    const hostile = JSON.parse(journal.intent) as Record<string, unknown>; hostile.sourceAfter = '51';
    await expect(restoreBearerTransferJournalV2({ ...journal, intent: JSON.stringify(hostile) }))
      .rejects.toThrow(/arithmetic/);
  });
});
