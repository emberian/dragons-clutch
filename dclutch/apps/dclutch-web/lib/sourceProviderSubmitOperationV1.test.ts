import {
  AddressLookupTableAccount,
  Keypair,
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
import { SOURCE_PROVIDER_SUBMIT_PLAN_FORMAT_V1 } from '@dclutch/sdk/generated/sourceProviderWasmV1';
import {
  restoreSourceProviderSubmitJournalV1,
  sourceProviderSubmitJournalInputV1,
} from './sourceProviderSubmitOperationV1';
import { parseSourceProviderSubmitPlanV1 } from './sourceProviderV1';

const address = (byte: number) => new PublicKey(new Uint8Array(32).fill(byte)).toBase58();
const base64 = (bytes: Uint8Array) => Buffer.from(bytes).toString('base64');

class MemoryStorage implements ClientOperationJournalStorageV1 {
  readonly values = new Map<string, string>();
  get length() { return this.values.size; }
  key(index: number) { return [...this.values.keys()][index] ?? null; }
  getItem(key: string) { return this.values.get(key) ?? null; }
  setItem(key: string, value: string) { this.values.set(key, value); }
  removeItem(key: string) { this.values.delete(key); }
}

function fixture() {
  const payer = Keypair.generate();
  const update = Keypair.generate();
  const tableAddress = new PublicKey(address(10));
  const loaded = new PublicKey(address(11));
  const table = new AddressLookupTableAccount({
    key: tableAddress,
    state: {
      deactivationSlot: 18_446_744_073_709_551_615n,
      lastExtendedSlot: 1,
      lastExtendedSlotStartIndex: 0,
      authority: undefined,
      addresses: [loaded],
    },
  });
  const program = new PublicKey(address(12));
  const transaction = new VersionedTransaction(new TransactionMessage({
    payerKey: payer.publicKey,
    recentBlockhash: address(13),
    instructions: [new TransactionInstruction({
      programId: program,
      keys: [
        { pubkey: update.publicKey, isSigner: true, isWritable: true },
        { pubkey: loaded, isSigner: false, isWritable: false },
      ],
      data: Buffer.of(1, 2, 3),
    })],
  }).compileToV0Message([table]));
  const lifecycle = address(14);
  const planJson = JSON.stringify({
    format: SOURCE_PROVIDER_SUBMIT_PLAN_FORMAT_V1,
    route: 'submit',
    observedSlot: '90',
    instruction: {
      program: program.toBase58(),
      accounts: Array.from({ length: 38 }, (_, index) => ({
        address: address(30 + index), isSigner: index < 2, isWritable: index < 3,
      })),
      dataBase64: 'AQID',
    },
    unsignedMessageBase64: base64(transaction.message.serialize()),
    requiredSigners: [payer.publicKey.toBase58(), update.publicKey.toBase58()],
    wireBytes: transaction.serialize().length,
    loadedAddresses: 1,
    lookupTables: [tableAddress.toBase58()],
    lifecycleTopUpLamports: '1000',
    completion: [lifecycle, update.publicKey.toBase58()],
    poststate: {
      lifecycle,
      updateAccount: update.publicKey.toBase58(),
      updateAuthority: address(15),
      resolutionProgram: program.toBase58(),
      receiverProgram: address(16),
      submitRequestBase64: 'AQ==',
    },
  });
  const market = address(17);
  return {
    scope: { clusterGenesis: address(18), market, owner: payer.publicKey.toBase58() },
    acquisition: {
      plan: parseSourceProviderSubmitPlanV1(planJson),
      planJson,
      inputJson: '{}',
      transaction,
      update,
      payer: payer.publicKey.toBase58(),
      market,
      lastValidBlockHeight: '100',
      observationAddresses: Object.freeze([]),
    },
  };
}

describe('Source provider submit crash journal', () => {
  it('round-trips one exact table-backed wallet/update message', async () => {
    const value = fixture();
    const input = await sourceProviderSubmitJournalInputV1(value.scope, value.acquisition);
    const journal = await writeUnsignedClientOperationJournalV1(new MemoryStorage(), input);
    const restored = await restoreSourceProviderSubmitJournalV1(journal);
    expect(restored.intent.update).toBe(value.acquisition.update.publicKey.toBase58());
    expect(restored.rustPlan.lookupTables).toEqual(value.acquisition.plan.lookupTables);
    expect(restored.transaction.serialize()).toEqual(value.acquisition.transaction.serialize());
  });

  it('refuses a substituted update authority and changed table', async () => {
    const value = fixture();
    const input = await sourceProviderSubmitJournalInputV1(value.scope, value.acquisition);
    const journal = await writeUnsignedClientOperationJournalV1(new MemoryStorage(), input);
    const intent = JSON.parse(journal.intent) as Record<string, unknown>;
    intent.update = address(99);
    await expect(restoreSourceProviderSubmitJournalV1({ ...journal, intent: JSON.stringify(intent) }))
      .rejects.toThrow(/substituted its route, account, or authority|operation digest changed/);

    const outer = JSON.parse(journal.plan) as Record<string, unknown>;
    const plan = JSON.parse(outer.rustPlan as string) as Record<string, unknown>;
    plan.lookupTables = [address(98)];
    outer.rustPlan = JSON.stringify(plan);
    await expect(restoreSourceProviderSubmitJournalV1({ ...journal, plan: JSON.stringify(outer) }))
      .rejects.toThrow(/exact unsigned Rust message|operation digest changed/);
  });
});
