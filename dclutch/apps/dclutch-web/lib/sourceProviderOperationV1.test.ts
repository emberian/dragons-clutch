import {
  Keypair,
  PublicKey,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
} from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { hex, sha256 } from './bytes';
import {
  writeUnsignedClientOperationJournalV1,
  type ClientOperationJournalStorageV1,
  type ClientOperationJournalV1,
} from './clientOperationJournal';
import { SOURCE_PROVIDER_PLAN_FORMAT_V1 } from './generated/sourceProviderWasmV1';
import {
  restoreSourceProviderJournalV1,
  sourceProviderJournalInputV1,
} from './sourceProviderOperationV1';
import { parseSourceProviderReclaimPlanV1 } from './sourceProviderV1';

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
  const resolver = Keypair.generate();
  const lifecycle = address(4);
  const program = address(5);
  const transaction = new VersionedTransaction(new TransactionMessage({
    payerKey: payer.publicKey,
    recentBlockhash: address(6),
    instructions: [new TransactionInstruction({
      programId: new PublicKey(program),
      keys: [{ pubkey: resolver.publicKey, isSigner: true, isWritable: false }],
      data: Buffer.of(1, 2, 3),
    })],
  }).compileToLegacyMessage());
  const completion = [address(10), address(11), address(12), address(13)];
  const planJson = JSON.stringify({
    format: SOURCE_PROVIDER_PLAN_FORMAT_V1,
    route: 'reclaim',
    observedSlot: '90',
    instruction: {
      program,
      accounts: Array.from({ length: 18 }, (_, index) => ({
        address: address(20 + index),
        isSigner: index === 0,
        isWritable: index < 4,
      })),
      dataBase64: 'AQID',
    },
    unsignedMessageBase64: base64(transaction.message.serialize()),
    requiredSigners: [payer.publicKey.toBase58(), resolver.publicKey.toBase58()],
    wireBytes: transaction.serialize().length,
    loadedAddresses: 0,
    lookupTables: [],
    lifecycle,
    updateAuthority: completion[2],
    completion,
    expectedPoststates: completion.map((entry, index) => ({
      address: entry,
      owner: PublicKey.default.toBase58(),
      lamports: index === 3 ? '44' : '0',
      executable: false,
      dataBase64: '',
    })),
  });
  const plan = parseSourceProviderReclaimPlanV1(planJson);
  const market = address(7);
  return {
    scope: { clusterGenesis: address(8), market, owner: payer.publicKey.toBase58() },
    acquisition: {
      plan,
      planJson,
      inputJson: '{}',
      transaction,
      resolver,
      payer: payer.publicKey.toBase58(),
      market,
      lastValidBlockHeight: '100',
      observationAddresses: Object.freeze([]),
    },
  };
}

async function operationDigest(intent: string, plan: string): Promise<string> {
  return hex(await sha256(new TextEncoder().encode(JSON.stringify({
    intent: JSON.parse(intent),
    plan: JSON.parse(plan),
  }))));
}

describe('Source provider crash journal', () => {
  it('round-trips the exact Rust message and two-signer authority boundary', async () => {
    const value = fixture();
    const input = await sourceProviderJournalInputV1(value.scope, value.acquisition);
    const journal = await writeUnsignedClientOperationJournalV1(new MemoryStorage(), input);
    const restored = await restoreSourceProviderJournalV1(journal);
    expect(restored.intent.resolver).toBe(value.acquisition.resolver.publicKey.toBase58());
    expect(restored.transaction.serialize()).toEqual(value.acquisition.transaction.serialize());
    expect(restored.rustPlan.expectedPoststates).toHaveLength(4);
  });

  it('refuses an authority substitution even when the hostile record recomputes its digest', async () => {
    const value = fixture();
    const input = await sourceProviderJournalInputV1(value.scope, value.acquisition);
    const journal = await writeUnsignedClientOperationJournalV1(new MemoryStorage(), input);
    const outer = JSON.parse(journal.plan) as Record<string, unknown>;
    const rustPlan = JSON.parse(outer.rustPlan as string) as Record<string, unknown>;
    rustPlan.requiredSigners = [value.scope.owner, address(99)];
    outer.rustPlan = JSON.stringify(rustPlan);
    const plan = JSON.stringify(outer);
    const hostile: ClientOperationJournalV1 = {
      ...journal,
      plan,
      operationDigest: await operationDigest(journal.intent, plan),
    };
    await expect(restoreSourceProviderJournalV1(hostile)).rejects.toThrow(/substituted its route, account, or authority/);
  });

  it('refuses unknown plan fields and an unsigned packet with a changed blockhash', async () => {
    const value = fixture();
    const input = await sourceProviderJournalInputV1(value.scope, value.acquisition);
    const journal = await writeUnsignedClientOperationJournalV1(new MemoryStorage(), input);
    const outer = JSON.parse(journal.plan) as Record<string, unknown>;
    const rustPlan = JSON.parse(outer.rustPlan as string) as Record<string, unknown>;
    rustPlan.extra = true;
    outer.rustPlan = JSON.stringify(rustPlan);
    const plan = JSON.stringify(outer);
    await expect(restoreSourceProviderJournalV1({
      ...journal,
      plan,
      operationDigest: await operationDigest(journal.intent, plan),
    })).rejects.toThrow(/unknown fields/);

    const changed = VersionedTransaction.deserialize(value.acquisition.transaction.serialize());
    changed.message.recentBlockhash = address(98);
    const changedOuter = JSON.parse(journal.plan) as Record<string, unknown>;
    changedOuter.unsignedWireBase64 = base64(changed.serialize());
    const changedPlan = JSON.stringify(changedOuter);
    await expect(restoreSourceProviderJournalV1({
      ...journal,
      plan: changedPlan,
      operationDigest: await operationDigest(journal.intent, changedPlan),
    })).rejects.toThrow(/differs from the exact unsigned Rust message/);
  });
});
