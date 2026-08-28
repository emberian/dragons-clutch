/**
 * Hostile consumer and resumable signer handoff for Rust-authored wallet
 * payout lookup-table plans.
 *
 * Payout tables deliberately preserve first-use account order. The generic
 * founding table helper sorts, so it cannot be reused here. Rust owns the
 * address sequence and exact official instructions; this module independently
 * reconstructs those instructions before it will ask the Position owner to
 * sign anything.
 */
import { createHash } from 'node:crypto';

import {
  AddressLookupTableAccount,
  AddressLookupTableProgram,
  PublicKey,
  Transaction,
  TransactionInstruction,
  type Keypair,
} from '@solana/web3.js';

import type { RpcAccount } from '@dclutch/sdk/rpc';

const FORMAT = 'dclutch-wallet-terminal-payout-alt-plan-v1';
const PAGE = 20;
const MAX_U64 = 0xffff_ffff_ffff_ffffn;
const MAX_PLAN_BYTES = 131_072;
const RECENT_SLOT_HASH_WINDOW = 512n;

type JsonObject = Record<string, unknown>;

export type InstructionManifestV1 = Readonly<{
  programId: string;
  accounts: ReadonlyArray<Readonly<{ address: string; signer: boolean; writable: boolean }>>;
  dataBase64: string;
}>;

export type WalletTerminalPayoutAltPlanV1 = Readonly<{
  format: typeof FORMAT;
  sourceInputSha256: string;
  observationSlot: string;
  payer: string;
  authority: string;
  lookupTable: string;
  addresses: ReadonlyArray<string>;
  create: InstructionManifestV1;
  extensions: ReadonlyArray<InstructionManifestV1>;
  payoutInput: Readonly<JsonObject>;
}>;

export type WalletTerminalPayoutAltObservationV1 = Readonly<{
  slot: string;
  owner: string | null;
  executable: boolean;
  authority: string | null;
  deactivationSlot: string;
  lastExtendedSlot: string;
  addresses: ReadonlyArray<string>;
}>;

export type WalletTerminalPayoutAltActionV1 =
  | Readonly<{ kind: 'create' }>
  | Readonly<{ kind: 'extend'; page: number }>
  | Readonly<{ kind: 'wait'; minimumSlot: string }>
  | Readonly<{ kind: 'ready'; finalizedSlot: string }>;

function plain(value: unknown): value is JsonObject {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function exactObject(value: unknown, fields: ReadonlyArray<string>, label: string): JsonObject {
  if (!plain(value)) throw new Error(`${label} must be one JSON object`);
  const keys = Object.keys(value).sort();
  const expected = [...fields].sort();
  if (keys.length !== expected.length || keys.some((field, index) => field !== expected[index])) {
    throw new Error(`${label} has missing or unknown fields`);
  }
  return value;
}

function text(value: unknown, label: string, maximum = 256): string {
  if (typeof value !== 'string' || value.length === 0 || value.length > maximum || value.trim() !== value) {
    throw new Error(`${label} must be bounded canonical text`);
  }
  return value;
}

function canonicalKey(value: unknown, label: string): string {
  const encoded = text(value, label, 64);
  let parsed: PublicKey;
  try { parsed = new PublicKey(encoded); } catch { throw new Error(`${label} must be one public key`); }
  if (parsed.toBase58() !== encoded) throw new Error(`${label} must be canonical base58 text`);
  return encoded;
}

function key(value: unknown, label: string): string {
  const encoded = canonicalKey(value, label);
  if (new PublicKey(encoded).equals(PublicKey.default)) throw new Error(`${label} must be nonzero`);
  return encoded;
}

function u64(value: unknown, label: string): string {
  const encoded = text(value, label, 20);
  if (!/^(0|[1-9][0-9]*)$/.test(encoded) || BigInt(encoded) > MAX_U64) {
    throw new Error(`${label} must be canonical u64 text`);
  }
  return encoded;
}

function hex32(value: unknown, label: string): string {
  const encoded = text(value, label, 64);
  if (!/^[0-9a-f]{64}$/.test(encoded)) throw new Error(`${label} must be lowercase 32-byte hex`);
  return encoded;
}

function base64(value: unknown, label: string): string {
  const encoded = text(value, label, 16_384);
  const bytes = Buffer.from(encoded, 'base64');
  if (bytes.length === 0 || bytes.toString('base64') !== encoded) throw new Error(`${label} must be canonical nonempty base64`);
  return encoded;
}

function instruction(value: unknown, label: string): InstructionManifestV1 {
  const object = exactObject(value, ['programId', 'accounts', 'dataBase64'], label);
  if (!Array.isArray(object.accounts) || object.accounts.length === 0 || object.accounts.length > 16) {
    throw new Error(`${label}.accounts must hold 1..16 account metas`);
  }
  const accounts = object.accounts.map((entry, index) => {
    const meta = exactObject(entry, ['address', 'signer', 'writable'], `${label}.accounts[${index}]`);
    if (typeof meta.signer !== 'boolean' || typeof meta.writable !== 'boolean') {
      throw new Error(`${label}.accounts[${index}] flags must be booleans`);
    }
    return Object.freeze({
      address: canonicalKey(meta.address, `${label}.accounts[${index}].address`),
      signer: meta.signer,
      writable: meta.writable,
    });
  });
  return Object.freeze({
    programId: key(object.programId, `${label}.programId`),
    accounts: Object.freeze(accounts),
    dataBase64: base64(object.dataBase64, `${label}.dataBase64`),
  });
}

function instructionManifest(value: TransactionInstruction): InstructionManifestV1 {
  return Object.freeze({
    programId: value.programId.toBase58(),
    accounts: Object.freeze(value.keys.map((meta) => Object.freeze({
      address: meta.pubkey.toBase58(),
      signer: meta.isSigner,
      writable: meta.isWritable,
    }))),
    dataBase64: Buffer.from(value.data).toString('base64'),
  });
}

function equalJson(left: unknown, right: unknown): boolean {
  if (left === right) return true;
  if (Array.isArray(left) || Array.isArray(right)) {
    return Array.isArray(left) && Array.isArray(right)
      && left.length === right.length
      && left.every((entry, index) => equalJson(entry, right[index]));
  }
  if (!plain(left) || !plain(right)) return false;
  const leftKeys = Object.keys(left).sort();
  const rightKeys = Object.keys(right).sort();
  return leftKeys.length === rightKeys.length
    && leftKeys.every((field, index) => field === rightKeys[index] && equalJson(left[field], right[field]));
}

function exactInstruction(observed: InstructionManifestV1, expected: TransactionInstruction, label: string): void {
  if (!equalJson(observed, instructionManifest(expected))) throw new Error(`${label} is not the exact official ALT instruction`);
}

/** Parse and independently reconstruct every instruction before signing is possible. */
export function parseWalletTerminalPayoutAltPlanV1(
  encoded: string,
  sourceInputBytes: Uint8Array,
): WalletTerminalPayoutAltPlanV1 {
  if (encoded.length === 0 || encoded.length > MAX_PLAN_BYTES) throw new Error('wallet payout ALT plan is outside its byte bound');
  let value: unknown;
  try { value = JSON.parse(encoded); } catch { throw new Error('wallet payout ALT plan is not JSON'); }
  const object = exactObject(value, [
    'format', 'sourceInputSha256', 'observationSlot', 'payer', 'authority', 'lookupTable',
    'addresses', 'create', 'extensions', 'payoutInput',
  ], 'wallet payout ALT plan');
  if (object.format !== FORMAT) throw new Error(`wallet payout ALT plan format must be ${FORMAT}`);
  const sourceInputSha256 = hex32(object.sourceInputSha256, 'sourceInputSha256');
  const expectedDigest = createHash('sha256').update(sourceInputBytes).digest('hex');
  if (sourceInputSha256 !== expectedDigest) throw new Error('wallet payout ALT plan names another source input');
  const observationSlot = u64(object.observationSlot, 'observationSlot');
  if (BigInt(observationSlot) > BigInt(Number.MAX_SAFE_INTEGER)) throw new Error('observationSlot exceeds the JS exact-integer boundary');
  const payer = key(object.payer, 'payer');
  const authority = key(object.authority, 'authority');
  if (authority !== payer) throw new Error('wallet payout ALT authority must equal its payer');
  const lookupTable = key(object.lookupTable, 'lookupTable');
  if (!Array.isArray(object.addresses) || object.addresses.length === 0 || object.addresses.length > 256) {
    throw new Error('wallet payout ALT addresses must hold 1..256 entries');
  }
  const addresses = Object.freeze(object.addresses.map((entry, index) => key(entry, `addresses[${index}]`)));
  if (new Set(addresses).size !== addresses.length) throw new Error('wallet payout ALT addresses must be duplicate-free');
  const create = instruction(object.create, 'create');
  if (!Array.isArray(object.extensions) || object.extensions.length !== Math.ceil(addresses.length / PAGE)) {
    throw new Error('wallet payout ALT extension count does not cover its exact address sequence');
  }
  const extensions = Object.freeze(object.extensions.map((entry, index) => instruction(entry, `extensions[${index}]`)));
  const payoutInput = exactObject(object.payoutInput, [
    'format', 'market', 'owner', 'recipientOwner', 'recipient', 'collateralMint', 'tokenProgram',
    'quantity', 'claimIndex', 'transferIndex', 'parentContext', 'custodyContext', 'releaseSet',
    'terminalCertificate', 'lookupTable', 'programs', 'records',
  ], 'payoutInput');
  if (key(payoutInput.owner, 'payoutInput.owner') !== payer
      || key(payoutInput.recipientOwner, 'payoutInput.recipientOwner') !== payer
      || key(payoutInput.lookupTable, 'payoutInput.lookupTable') !== lookupTable) {
    throw new Error('wallet payout ALT plan does not join its embedded payout input');
  }
  let sourceInput: unknown;
  try { sourceInput = JSON.parse(Buffer.from(sourceInputBytes).toString('utf8')); } catch { throw new Error('source payout input is not JSON'); }
  if (!plain(sourceInput) || Object.hasOwn(sourceInput, 'lookupTable')) {
    throw new Error('source payout input must omit lookupTable during ALT preparation');
  }
  const withoutLookup = { ...payoutInput };
  delete withoutLookup.lookupTable;
  if (!equalJson(sourceInput, withoutLookup)) throw new Error('wallet payout ALT plan substituted its embedded payout input');

  const [expectedCreate, expectedTable] = AddressLookupTableProgram.createLookupTable({
    authority: new PublicKey(authority),
    payer: new PublicKey(payer),
    recentSlot: Number(observationSlot),
  });
  if (expectedTable.toBase58() !== lookupTable) throw new Error('wallet payout ALT address is not derived from owner and observation slot');
  exactInstruction(create, expectedCreate, 'create');
  for (let offset = 0; offset < addresses.length; offset += PAGE) {
    const page = offset / PAGE;
    const expected = AddressLookupTableProgram.extendLookupTable({
      lookupTable: expectedTable,
      authority: new PublicKey(authority),
      payer: new PublicKey(payer),
      addresses: addresses.slice(offset, offset + PAGE).map((address) => new PublicKey(address)),
    });
    const observed = extensions[page];
    if (observed === undefined) throw new Error(`wallet payout ALT extension ${page} is absent`);
    exactInstruction(observed, expected, `extensions[${page}]`);
  }
  return Object.freeze({
    format: FORMAT,
    sourceInputSha256,
    observationSlot,
    payer,
    authority,
    lookupTable,
    addresses,
    create,
    extensions,
    payoutInput: Object.freeze(payoutInput),
  });
}

/** Convert a checked Rust instruction manifest into a web3 instruction. */
export function payoutAltInstructionV1(value: InstructionManifestV1): TransactionInstruction {
  return new TransactionInstruction({
    programId: new PublicKey(value.programId),
    keys: value.accounts.map((meta) => ({
      pubkey: new PublicKey(meta.address),
      isSigner: meta.signer,
      isWritable: meta.writable,
    })),
    data: Buffer.from(value.dataBase64, 'base64'),
  });
}

/** Decode only finalized RPC account bytes; no plan data is trusted as readback. */
export function observeWalletTerminalPayoutAltV1(
  slot: string,
  account: RpcAccount | null,
  lookupTable: string,
): WalletTerminalPayoutAltObservationV1 {
  u64(slot, 'lookup table observation slot');
  if (account === null) return Object.freeze({
    slot, owner: null, executable: false, authority: null,
    deactivationSlot: MAX_U64.toString(), lastExtendedSlot: '0', addresses: Object.freeze([]),
  });
  let table: ReturnType<typeof AddressLookupTableAccount.deserialize>;
  try { table = AddressLookupTableAccount.deserialize(account.data); } catch { throw new Error(`lookup table ${lookupTable} has malformed finalized bytes`); }
  return Object.freeze({
    slot,
    owner: account.owner,
    executable: account.executable,
    authority: table.authority?.toBase58() ?? null,
    deactivationSlot: BigInt(table.deactivationSlot).toString(),
    lastExtendedSlot: String(table.lastExtendedSlot),
    addresses: Object.freeze(table.addresses.map((address) => address.toBase58())),
  });
}

/** Decide exactly one idempotent next action from finalized table state. */
export function nextWalletTerminalPayoutAltActionV1(
  plan: WalletTerminalPayoutAltPlanV1,
  observed: WalletTerminalPayoutAltObservationV1,
): WalletTerminalPayoutAltActionV1 {
  const slot = BigInt(u64(observed.slot, 'lookup table observation slot'));
  const planSlot = BigInt(plan.observationSlot);
  if (slot < planSlot) throw new Error('lookup table observation predates its Rust-authored plan');
  if (observed.owner === null) {
    if (observed.addresses.length !== 0 || observed.authority !== null) throw new Error('absent lookup table observation carried state');
    if (slot - planSlot >= RECENT_SLOT_HASH_WINDOW) {
      throw new Error('absent payout ALT plan expired from SlotHashes; do not submit or discard it until any prior create signature is reconciled');
    }
    return Object.freeze({ kind: 'create' });
  }
  if (observed.owner !== AddressLookupTableProgram.programId.toBase58() || observed.executable) {
    throw new Error('lookup table has another owner or executable bit');
  }
  if (observed.authority !== plan.authority || BigInt(u64(observed.deactivationSlot, 'deactivationSlot')) !== MAX_U64) {
    throw new Error('lookup table has another authority or is deactivating');
  }
  const lastExtendedSlot = BigInt(u64(observed.lastExtendedSlot, 'lastExtendedSlot'));
  if (lastExtendedSlot > slot) throw new Error('lookup table last extension is ahead of its finalized observation');
  if (observed.addresses.length > plan.addresses.length
      || observed.addresses.some((address, index) => address !== plan.addresses[index])) {
    throw new Error('lookup table is not an exact prefix of the Rust-authored address sequence');
  }
  if (observed.addresses.length === plan.addresses.length) {
    return slot <= lastExtendedSlot
      ? Object.freeze({ kind: 'wait', minimumSlot: (lastExtendedSlot + 1n).toString() })
      : Object.freeze({ kind: 'ready', finalizedSlot: slot.toString() });
  }
  if (observed.addresses.length % PAGE !== 0) {
    throw new Error('lookup table partial width is not one complete planned extension page');
  }
  return Object.freeze({ kind: 'extend', page: observed.addresses.length / PAGE });
}

export type PayoutAltProvisionDependenciesV1 = Readonly<{
  observe: () => Promise<WalletTerminalPayoutAltObservationV1>;
  latestMutationBlockhash: (minimumContextSlot: string) => Promise<Readonly<{ blockhash: string }>>;
  submit: (wire: Uint8Array) => Promise<boolean>;
  wait: () => Promise<void>;
}>;

/** Resume create/extend pages until a strictly later finalized readback is ready. */
export async function provisionWalletTerminalPayoutAltV1(
  plan: WalletTerminalPayoutAltPlanV1,
  signer: Keypair,
  dependencies: PayoutAltProvisionDependenciesV1,
): Promise<Readonly<{ transactions: number; finalizedSlot: string }>> {
  if (signer.publicKey.toBase58() !== plan.payer || plan.authority !== plan.payer) {
    throw new Error('the explicit signer is not this payout ALT payer and authority');
  }
  let transactions = 0;
  let pending: Extract<WalletTerminalPayoutAltActionV1, { kind: 'create' | 'extend' }> | null = null;
  const maximumTurns = plan.extensions.length * 64 + 128;
  for (let turn = 0; turn < maximumTurns; turn += 1) {
    const observed = await dependencies.observe();
    const action = nextWalletTerminalPayoutAltActionV1(plan, observed);
    if (pending !== null) {
      const unchanged = (pending.kind === 'create' && action.kind === 'create')
        || (pending.kind === 'extend' && action.kind === 'extend' && action.page === pending.page);
      if (unchanged) {
        await dependencies.wait();
        continue;
      }
      if (pending.kind === 'extend'
          && (action.kind === 'create' || (action.kind === 'extend' && action.page < pending.page))) {
        throw new Error('wallet payout ALT finalized readback moved backward after submission');
      }
      pending = null;
    }
    if (action.kind === 'ready') return Object.freeze({ transactions, finalizedSlot: action.finalizedSlot });
    if (action.kind === 'wait') {
      await dependencies.wait();
      continue;
    }
    const instruction = action.kind === 'create' ? plan.create : plan.extensions[action.page];
    if (instruction === undefined) throw new Error('wallet payout ALT resume selected an absent extension page');
    const recent = await dependencies.latestMutationBlockhash(observed.slot);
    const transaction = new Transaction({
      feePayer: signer.publicKey,
      recentBlockhash: recent.blockhash,
    }).add(payoutAltInstructionV1(instruction));
    transaction.sign(signer);
    const wire = Uint8Array.from(transaction.serialize());
    if (wire.length > 1_232) throw new Error(`wallet payout ALT transaction is ${wire.length}/1232 bytes`);
    if (!await dependencies.submit(wire)) throw new Error('wallet payout ALT transaction was refused');
    transactions += 1;
    pending = action;
    await dependencies.wait();
  }
  throw new Error('wallet payout ALT did not reach an active finalized readback within the bounded poll budget');
}
