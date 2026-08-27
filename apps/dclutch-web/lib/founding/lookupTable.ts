/**
 * Address lookup tables for the founding routes, built in the browser.
 *
 * Found31 is a 31-account frame. Inline, with the ComputeBudget declaration it
 * cannot execute without, its v0 message is 1,242 bytes against a 1,232-byte
 * packet — ten bytes over, exactly as the Rust client measured before it moved
 * the route onto a table. So a lookup table is not an optimization here; it is
 * the difference between a transaction that exists and one that does not.
 *
 * THE HAZARD THIS MODULE IS SHAPED AROUND. A table's contents are stored in
 * canonical sorted order, and a client that compiles lookup indexes against the
 * list it *built the plan from* rather than against the table's own contents
 * hands the program a permuted account frame. That cost the vertical lane three
 * validator runs and a byte-exact host reconstruction that validated clean, and
 * it surfaced only as a program refusal three layers away. The rule the board
 * drew from it: hand on `plan.addresses` or the observed account, never the
 * vector you built the plan from. `planLookupTableV1` therefore returns the
 * canonical order it will actually write, and `lookupTableAccountV1` reads the
 * contents back off the chain rather than trusting the plan at all.
 *
 * ROUTING IS TRANSPORT, NEVER AUTHORITY. Every account a table names is still
 * authenticated by the program that reads it. A table is how a frame fits in a
 * packet; it decides nothing about what the frame means.
 */

import {
  AddressLookupTableAccount,
  AddressLookupTableProgram,
  PublicKey,
  TransactionInstruction,
} from '@solana/web3.js';

import { type SolanaRpcClient } from '../rpc';

/**
 * Addresses per extend transaction.
 *
 * Mirrors `EXTEND_ADDRESSES_PER_TRANSACTION_V1` in
 * `crates/dclutch-versioned-message-operator`. Kept here as a named constant
 * rather than emitted only because it bounds a transaction this module builds
 * itself; `lookupTable.test.ts` pins it against the Rust value.
 */
export const EXTEND_ADDRESSES_PER_TRANSACTION_V1 = 20;

/** Agave's per-table ceiling. */
export const LOOKUP_TABLE_MAX_ADDRESSES_V1 = 256;

export type LookupTablePlanV1 = Readonly<{
  lookupTable: string;
  /** The canonical sorted, deduplicated order the table will hold. */
  addresses: ReadonlyArray<string>;
  recentSlot: string;
  create: TransactionInstruction;
  extensions: ReadonlyArray<TransactionInstruction>;
}>;

/**
 * Sort and deduplicate exactly as the operator does.
 *
 * The sort is over raw 32-byte keys, not over base58 text: base58 is not
 * order-preserving, so sorting the strings would produce a different table and
 * a different set of indexes.
 */
export function canonicalLookupAddressesV1(addresses: ReadonlyArray<string>): ReadonlyArray<string> {
  if (addresses.length === 0) throw new Error('a lookup table needs at least one address');
  if (addresses.length > LOOKUP_TABLE_MAX_ADDRESSES_V1) throw new Error(`a lookup table holds at most ${LOOKUP_TABLE_MAX_ADDRESSES_V1} addresses`);
  const keys = addresses.map((address) => {
    const parsed = new PublicKey(address);
    if (parsed.toBase58() !== address) throw new Error(`${address} is not canonical base58 text`);
    return parsed;
  });
  const sorted = [...keys].sort((left, right) => {
    const a = left.toBytes();
    const b = right.toBytes();
    for (let index = 0; index < 32; index += 1) {
      if (a[index] !== b[index]) return a[index] < b[index] ? -1 : 1;
    }
    return 0;
  });
  for (let index = 1; index < sorted.length; index += 1) {
    if (sorted[index - 1].equals(sorted[index])) throw new Error(`lookup table names ${sorted[index].toBase58()} twice`);
  }
  return Object.freeze(sorted.map((key) => key.toBase58()));
}

/**
 * Plan one table: its derived address, its create, and its extend pages.
 *
 * `recentSlot` is part of the table's own PDA derivation, so it is not a
 * freshness hint — a different slot is a different table.
 */
export function planLookupTableV1(input: Readonly<{
  authority: string;
  payer: string;
  recentSlot: bigint;
  addresses: ReadonlyArray<string>;
}>): LookupTablePlanV1 {
  const authority = new PublicKey(input.authority);
  const payer = new PublicKey(input.payer);
  const addresses = canonicalLookupAddressesV1(input.addresses);
  if (input.recentSlot < 0n || input.recentSlot > 0xffff_ffff_ffff_ffffn) throw new Error('recent slot is outside u64');
  const [create, lookupTable] = AddressLookupTableProgram.createLookupTable({
    authority,
    payer,
    recentSlot: Number(input.recentSlot),
  });
  const extensions: TransactionInstruction[] = [];
  for (let offset = 0; offset < addresses.length; offset += EXTEND_ADDRESSES_PER_TRANSACTION_V1) {
    extensions.push(AddressLookupTableProgram.extendLookupTable({
      lookupTable,
      authority,
      payer,
      addresses: addresses.slice(offset, offset + EXTEND_ADDRESSES_PER_TRANSACTION_V1).map((address) => new PublicKey(address)),
    }));
  }
  return Object.freeze({
    lookupTable: lookupTable.toBase58(),
    addresses,
    recentSlot: input.recentSlot.toString(),
    create,
    extensions: Object.freeze(extensions),
  });
}

/**
 * Read a table's contents back off the chain, at finalized commitment.
 *
 * This — and never the plan — is what a v0 message is compiled against. A table
 * is usable only strictly after the slot that last extended it, so a caller
 * that has just extended must wait for finality before this answers usefully;
 * the refusal below is what a caller who did not wait sees.
 */
export async function lookupTableAccountV1(
  client: Pick<SolanaRpcClient, 'accountInfo'>,
  address: string,
  expected: ReadonlyArray<string>,
): Promise<AddressLookupTableAccount> {
  const observed = await client.accountInfo(address);
  if (observed.account === null) throw new Error(`lookup table ${address} is absent at finalized commitment`);
  const account = new AddressLookupTableAccount({
    key: new PublicKey(address),
    state: AddressLookupTableAccount.deserialize(observed.account.data),
  });
  const contents = account.state.addresses.map((key) => key.toBase58());
  if (contents.length !== expected.length || contents.some((entry, index) => entry !== expected[index])) {
    throw new Error(`lookup table ${address} holds ${contents.length} of ${expected.length} planned addresses, or holds them in another order`);
  }
  return account;
}

/**
 * Every address a v0 message may route, given the instructions it carries.
 *
 * The fee payer and every signer stay in the static key list — a lookup table
 * cannot carry a signer — so they are excluded here. Program ids are included:
 * they are ordinary readonly accounts to the message compiler.
 */
export function routableAddressesV1(
  instructions: ReadonlyArray<TransactionInstruction>,
  payer: string,
): ReadonlyArray<string> {
  const seen = new Set<string>();
  const routable: string[] = [];
  for (const instruction of instructions) {
    for (const address of [instruction.programId.toBase58(), ...instruction.keys.filter((meta) => !meta.isSigner).map((meta) => meta.pubkey.toBase58())]) {
      if (address === payer || seen.has(address)) continue;
      seen.add(address);
      routable.push(address);
    }
  }
  return Object.freeze(routable);
}
