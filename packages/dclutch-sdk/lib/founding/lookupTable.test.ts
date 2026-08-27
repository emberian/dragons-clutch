import { readFileSync } from 'node:fs';

import { AddressLookupTableAccount, Keypair, PublicKey, TransactionInstruction } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import {
  EXTEND_ADDRESSES_PER_TRANSACTION_V1,
  LOOKUP_TABLE_MAX_ADDRESSES_V1,
  canonicalLookupAddressesV1,
  lookupTableAccountV1,
  planLookupTableV1,
  routableAddressesV1,
} from './lookupTable';

const operator = readFileSync(new URL('../../../../crates/dclutch-versioned-message-operator/src/lib.rs', import.meta.url), 'utf8');

function address(): string {
  return Keypair.generate().publicKey.toBase58();
}

describe('the routing table agrees with the Rust operator', () => {
  it('pages at the width the operator pages at', () => {
    // A different page width is a different set of transactions, and this is
    // the one number this module states rather than imports.
    const declared = operator.match(/pub const EXTEND_ADDRESSES_PER_TRANSACTION_V1: usize = ([0-9_]+);/);
    expect(declared).not.toBeNull();
    expect(Number(declared![1].replaceAll('_', ''))).toBe(EXTEND_ADDRESSES_PER_TRANSACTION_V1);
  });

  it('sorts over raw key bytes, not over base58 text', () => {
    // Base58 is not order-preserving, so sorting the strings gives a different
    // table and therefore a different set of indexes. This is the exact slip
    // that produces a permuted account frame.
    const keys = Array.from({ length: 40 }, () => Keypair.generate().publicKey);
    const canonical = canonicalLookupAddressesV1(keys.map((key) => key.toBase58()));
    const byBytes = [...keys].sort((left, right) => Buffer.compare(Buffer.from(left.toBytes()), Buffer.from(right.toBytes()))).map((key) => key.toBase58());
    expect(canonical).toEqual(byBytes);

    const byText = [...keys].map((key) => key.toBase58()).sort();
    // With forty random keys the two orders essentially never coincide; if they
    // did, this test would be vacuous rather than wrong, so it asserts the
    // disagreement exists before relying on it.
    expect(byText).not.toEqual(byBytes);
  });

  it('refuses an empty table, a duplicate, and one past the ceiling', () => {
    expect(() => canonicalLookupAddressesV1([])).toThrow(/at least one address/);
    const duplicate = address();
    expect(() => canonicalLookupAddressesV1([duplicate, address(), duplicate])).toThrow(/twice/);
    expect(() => canonicalLookupAddressesV1(Array.from({ length: LOOKUP_TABLE_MAX_ADDRESSES_V1 + 1 }, address))).toThrow(/at most 256/);
    expect(() => canonicalLookupAddressesV1(['not-base58'])).toThrow();
  });
});

describe('planning one table', () => {
  it('emits one create and one extend per twenty addresses', () => {
    for (const [count, pages] of [[1, 1], [20, 1], [21, 2], [30, 2], [41, 3]] as const) {
      const plan = planLookupTableV1({
        authority: address(),
        payer: address(),
        recentSlot: 1_234n,
        addresses: Array.from({ length: count }, address),
      });
      expect(plan.extensions.length, `${count} addresses`).toBe(pages);
      expect(plan.addresses.length).toBe(count);
    }
  });

  it('extends in exactly the canonical order it reports', () => {
    const plan = planLookupTableV1({
      authority: address(),
      payer: address(),
      recentSlot: 7n,
      addresses: Array.from({ length: 30 }, address),
    });
    // Every extend page, concatenated, must be `plan.addresses` verbatim --
    // that is what makes `plan.addresses` safe to compile indexes against.
    const extended = plan.extensions.flatMap((instruction) => {
      const data = Uint8Array.from(instruction.data);
      const count = Number(new DataView(data.buffer, data.byteOffset + 4, 8).getBigUint64(0, true));
      return Array.from({ length: count }, (_, index) => new PublicKey(data.slice(12 + index * 32, 44 + index * 32)).toBase58());
    });
    expect(extended).toEqual([...plan.addresses]);
  });

  it('derives a different table for a different slot', () => {
    const shared = { authority: address(), payer: address(), addresses: Array.from({ length: 4 }, address) };
    const first = planLookupTableV1({ ...shared, recentSlot: 100n });
    const second = planLookupTableV1({ ...shared, recentSlot: 101n });
    // The slot is part of the table's own derivation, so it is not a freshness
    // hint: a different slot is a different table.
    expect(first.lookupTable).not.toBe(second.lookupTable);
  });

  it('refuses a slot outside u64', () => {
    expect(() => planLookupTableV1({ authority: address(), payer: address(), recentSlot: -1n, addresses: [address()] })).toThrow(/outside u64/);
  });
});

describe('reading a table back off the chain', () => {
  function observed(addresses: ReadonlyArray<string>): { accountInfo: () => Promise<unknown> } {
    const account = new AddressLookupTableAccount({
      key: new PublicKey(address()),
      state: {
        deactivationSlot: 2n ** 64n - 1n,
        lastExtendedSlot: 9,
        lastExtendedSlotStartIndex: 0,
        authority: new PublicKey(address()),
        addresses: addresses.map((entry) => new PublicKey(entry)),
      },
    });
    // web3.js has no public serializer, so the fixture is the deserializer's
    // own input shape: a header the parser accepts, then the raw keys.
    const header = new Uint8Array(56);
    new DataView(header.buffer).setUint32(0, 1, true);
    new DataView(header.buffer).setBigUint64(4, 2n ** 64n - 1n, true);
    new DataView(header.buffer).setBigUint64(12, 9n, true);
    header[20] = 0;
    header[21] = 1;
    header.set(account.state.authority!.toBytes(), 22);
    const data = new Uint8Array(header.length + addresses.length * 32);
    data.set(header, 0);
    addresses.forEach((entry, index) => data.set(new PublicKey(entry).toBytes(), header.length + index * 32));
    return { accountInfo: async () => ({ account: { data, owner: '', lamports: '0', executable: false } }) };
  }

  it('refuses a table that is absent', async () => {
    const client = { accountInfo: async () => ({ account: null }) };
    await expect(lookupTableAccountV1(client as never, address(), [address()])).rejects.toThrow(/absent at finalized commitment/);
  });

  it('refuses a table whose contents are not the planned ones, in order', async () => {
    const planned = canonicalLookupAddressesV1(Array.from({ length: 5 }, address));
    // Same set, wrong order: the indexes a message compiles would point at the
    // wrong accounts, and the program would refuse three layers away.
    const permuted = [planned[1], planned[0], ...planned.slice(2)];
    await expect(lookupTableAccountV1(observed(permuted) as never, address(), planned)).rejects.toThrow(/another order/);
    await expect(lookupTableAccountV1(observed(planned.slice(0, 3)) as never, address(), planned)).rejects.toThrow(/3 of 5 planned/);
  });

  it('accepts a table that holds exactly the planned contents', async () => {
    const planned = canonicalLookupAddressesV1(Array.from({ length: 6 }, address));
    const account = await lookupTableAccountV1(observed(planned) as never, address(), planned);
    expect(account.state.addresses.map((key) => key.toBase58())).toEqual([...planned]);
  });
});

describe('choosing what may route', () => {
  it('excludes the payer and every signer, and includes program ids', () => {
    // A lookup table cannot carry a signer, so a signer that routed would be a
    // transaction that cannot be signed.
    const payer = address();
    const signer = address();
    const readonlyKey = address();
    const program = address();
    const routable = routableAddressesV1([new TransactionInstruction({
      programId: new PublicKey(program),
      keys: [
        { pubkey: new PublicKey(payer), isSigner: true, isWritable: true },
        { pubkey: new PublicKey(signer), isSigner: true, isWritable: false },
        { pubkey: new PublicKey(readonlyKey), isSigner: false, isWritable: false },
      ],
      data: Buffer.alloc(0),
    })], payer);
    expect(routable).toContain(program);
    expect(routable).toContain(readonlyKey);
    expect(routable).not.toContain(payer);
    expect(routable).not.toContain(signer);
  });

  it('names a repeated account once, in first-seen order', () => {
    const payer = address();
    const shared = address();
    const program = address();
    const meta = { pubkey: new PublicKey(shared), isSigner: false, isWritable: true };
    const routable = routableAddressesV1([
      new TransactionInstruction({ programId: new PublicKey(program), keys: [meta, meta], data: Buffer.alloc(0) }),
      new TransactionInstruction({ programId: new PublicKey(program), keys: [meta], data: Buffer.alloc(0) }),
    ], payer);
    expect(routable).toEqual([program, shared]);
  });
});
