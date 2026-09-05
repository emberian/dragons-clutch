import {
  AddressLookupTableAccount,
  PublicKey,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
} from '@solana/web3.js';

import { boundedInstructionsV1 } from './founding/computeBudget';
import { routableAddressesV1 } from './founding/lookupTable';
import { SOLANA_PACKET_BYTES_V1 } from './solanaLimits';
import {
  CAPABILITY_ENTRY_ACTIVATION_DEADLINE_OFFSET_V1,
  CAPABILITY_ENTRY_ACTIVATION_POLICY_OFFSET_V1,
  CAPABILITY_ENTRY_BYTES_V1,
  CAPABILITY_ENTRY_CAPACITY_PROFILE_ID_OFFSET_V1,
  CAPABILITY_ENTRY_CHILD_DERIVATION_ID_OFFSET_V1,
  CAPABILITY_ENTRY_CHILD_SCHEMA_ID_OFFSET_V1,
  CAPABILITY_ENTRY_CONFIG_ID_OFFSET_V1,
  CAPABILITY_ENTRY_DEPENDENCIES_OFFSET_V1,
  CAPABILITY_ENTRY_DEPENDENCY_COUNT_OFFSET_V1,
  CAPABILITY_ENTRY_KIND_ID_OFFSET_V1,
  CAPABILITY_ENTRY_QUOTE_OFFSET_V1,
  CAPABILITY_ENTRY_RELEASE_ID_OFFSET_V1,
  CAPABILITY_ENTRY_RESERVED_BYTES_V1,
  CAPABILITY_ENTRY_RESERVED_OFFSET_V1,
  CAPABILITY_FUNDING_ALLOCATION_AMOUNT_OFFSET_V1,
  CAPABILITY_FUNDING_ALLOCATION_CLASS_OFFSET_V1,
  CAPABILITY_FUNDING_ALLOCATION_RESERVED_BYTES_V1,
  CAPABILITY_FUNDING_ALLOCATION_RESERVED_OFFSET_V1,
  CAPABILITY_FUNDING_AMOUNTS_NATIVE_TOTAL_OFFSET_V1,
  CAPABILITY_FUNDING_AMOUNTS_REALM_TOTAL_OFFSET_V1,
  CAPABILITY_FUNDING_BINDING_BENEFICIARY_OFFSET_V1,
  CAPABILITY_FUNDING_BINDING_BYTES_V1,
  CAPABILITY_FUNDING_BINDING_MINT_OFFSET_V1,
  CAPABILITY_FUNDING_BINDING_REALM_ID_OFFSET_V1,
  CAPABILITY_FUNDING_BINDING_RELEASE_ID_OFFSET_V1,
  CAPABILITY_FUNDING_BINDING_TOKEN_PROGRAM_OFFSET_V1,
  CAPABILITY_FUNDING_QUOTE_AMOUNTS_OFFSET_V1,
  CAPABILITY_FUNDING_QUOTE_BINDING_OFFSET_V1,
  CAPABILITY_FUNDING_QUOTE_BYTES_V1,
  CAPABILITY_FUNDING_QUOTE_COLLATERAL_KIND_OFFSET_V1,
  CAPABILITY_FUNDING_QUOTE_MAGIC_OFFSET_V1,
  CAPABILITY_FUNDING_QUOTE_MAGIC_V1,
  CAPABILITY_FUNDING_QUOTE_RESERVED_BYTES_V1,
  CAPABILITY_FUNDING_QUOTE_RESERVED_OFFSET_V1,
  CAPABILITY_FUNDING_QUOTE_SCHEMA_OFFSET_V1,
  CAPABILITY_FUNDING_QUOTE_SCHEMA_VERSION_V1,
  CAPABILITY_MANIFEST_ARTIFACT_PROFILE_V1,
  CAPABILITY_MANIFEST_COUNT_OFFSET_V1,
  CAPABILITY_MANIFEST_HEADER_BYTES_V1,
  CAPABILITY_MANIFEST_HEADER_RESERVED_BYTES_V1,
  CAPABILITY_MANIFEST_MAGIC_OFFSET_V1,
  CAPABILITY_MANIFEST_MAGIC_V1,
  CAPABILITY_MANIFEST_PROFILE_OFFSET_V1,
  CAPABILITY_MANIFEST_RESERVED_OFFSET_V1,
  CAPABILITY_MANIFEST_SCHEMA_OFFSET_V1,
  CAPABILITY_MANIFEST_SCHEMA_VERSION_V1,
  FUNDING_COMPARTMENTS_V1,
  MAX_CAPABILITIES_V1,
  MAX_DEPENDENCIES_PER_CAPABILITY_V1,
} from './generated/capabilityManifestV1';
import {
  REALM_ADAPTER_RELEASE_ID_OFFSET_V1,
  REALM_BYTES_V1,
  REALM_COLLATERAL_MINT_OFFSET_V1,
  REALM_FREEZE_AUTHORITY_POLICY_OFFSET_V1,
  REALM_MAGIC_OFFSET_V1,
  REALM_MAGIC_V1,
  REALM_MINT_AUTHORITY_POLICY_OFFSET_V1,
  REALM_RESERVED_BYTES_V1,
  REALM_RESERVED_OFFSET_V1,
  REALM_SCHEMA_VERSION_OFFSET_V1,
  REALM_SCHEMA_VERSION_V1,
  REALM_TOKEN_PROGRAM_OFFSET_V1,
} from './generated/realmPositionV1';
import {
  CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
  ARTIFACT_RELEASE_SCHEMA_ID_V1,
  GRADED_BASIS_RECORD_SCHEMA_ID_V3,
  MANIPULATION_FLOOR_SCHEMA_RELEASE_ID_V1,
  SOURCE_CAPACITY_PROFILE_SCHEMA_ID_V1,
  SOURCE_SPEC_SCHEMA_ID_V1,
  WINDOW_SPEC_SCHEMA_ID_V1,
  CREATE_LIFECYCLE_RENT_CREDIT_BYTES_V2,
  CORE_ACTION_FOUND_TAG,
  CORE_FOUND_ACCOUNT_COUNT_V3,
  CORE_FOUND_ACCOUNT_ROLES_V3,
  CORE_FOUND_PRICE_GATE_ACCOUNT_COUNT_V3,
  CORE_FOUND_PRICE_GATE_ACCOUNT_ROLES_V3,
  CORE_REQUEST_BYTES,
  CORE_REQUEST_MAGIC,
  CORE_STATE_BYTES,
  CORE_VERSION,
  LIFECYCLE_RENT_ACTION_CREATE_V2,
  LIFECYCLE_RENT_CREDIT_BYTES_V2,
  LIFECYCLE_RENT_CREDIT_MAGIC_OFFSET_V2,
  LIFECYCLE_RENT_CREDIT_MAGIC_V2,
  LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2,
  LIFECYCLE_RENT_INSTRUCTION_ACTION_OFFSET_V2,
  LIFECYCLE_RENT_INSTRUCTION_MAGIC_V2,
  LIFECYCLE_RENT_SCHEMA_VERSION_V2,
  MARKET_CORE_STATE_PDA_DOMAIN_V2,
  PORTFOLIO_SCHEMA_ID_V2,
  PRICE_GATE_RECORD_SCHEMA_ID_V1,
  PRODUCT_RECORD_DOMAIN_DIGEST_OFFSET_V2,
  PRODUCT_RECORD_PORTFOLIO_DIGEST_OFFSET_V2,
  PRODUCT_RECORD_SCHEMA_ID_V2,
  REALM_SCHEMA_RELEASE_ID_V1,
  RESULT_DOMAIN_SCHEMA_ID_V2,
  SOURCE_MATERIAL_MANIPULATION_FLOOR_OFFSET_V3,
  SOURCE_MATERIAL_WINDOW_SPEC_OFFSET_V3,
  SOURCE_MATERIAL_PRIMARY_SOURCE_SPEC_OFFSET_V3,
  SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
} from './generated/coreFound';
import { inspectProtocolInfrastructureV1, type ProtocolInfrastructureInspectionV1 } from './infrastructure';
import {
  NATIVE_LOADER_ID,
  RENT_SYSVAR_ID,
  SYSTEM_PROGRAM_ID,
  SYSVAR_OWNER_ID,
  deriveFinalizedRecordAddressesV1,
} from './releaseRegistry';
import { MAX_RPC_RESPONSE_BYTES, type RpcAccount, type SolanaRpcClient } from './rpc';
import { ascii, hex, isZero, requireNonzero, requireZero, sha256, slice, u16, u64 } from './bytes';
import {
  PRODUCT_RUNTIME_DOMAIN_MAGIC_V2,
  PRODUCT_RUNTIME_PORTFOLIO_MAGIC_V2,
} from './generated/protocolConstantsV1';
import {
  PRODUCT_RECORD_MAGIC_V2,
} from './generated/productRuntimeV2Admission';

const PRODUCT_RECORD_BYTES = 112;
const DOMAIN_HEADER_BYTES = 240;
const PORTFOLIO_HEADER_BYTES = 208;
const SOURCE_MATERIAL_BYTES = 240;
const MAX_U32 = 0xffff_ffff;

export type CoreFoundInputV2 = Readonly<{
  payer: string;
  registryProgram: string;
  activationCache: string;
  refundWallet: string;
  realmRecord: string;
  productRecord: string;
  resultDomainRecord: string;
  portfolioRecord: string;
  linkedBasisRecord: string;
  sourceMaterialRecord: string;
  sourceSpecRecord: string;
  capacityProfileRecord: string;
  manipulationFloorRecord: string;
  capabilityManifestRecord: string;
  /**
   * Finalized `DCLTPGT1` certificate emitted alongside a curved ProductBasisV3.
   *
   * The SDK does not reconstruct spline or no-arbitrage semantics. A caller
   * supplies this coordinate from the Rust compiler report; Registry
   * authentication below then extends the canonical Found37 frame to Found39.
   */
  priceGateRecord?: string;
  generation: bigint;
  /**
   * The finalized routing table the selected Found37/Found39 frame rides,
   * read back off the chain.
   *
   * Absent, `prepareCoreFoundV2` still derives everything and still reports
   * `routableAddresses`, but compiling refuses: with its required ComputeBudget
   * declaration the inline frame is ten bytes over the packet bound. That
   * refusal is the correct answer, and it is how a caller learns it owes a
   * table rather than discovering it at submission.
   */
  lookupTable?: AddressLookupTableAccount;
}>;

export type CoreFoundPlanV2 = Readonly<{
  observedSlot: string;
  market: string;
  rentCredit: string;
  coreProgram: string;
  registryProgram: string;
  rentProgram: string;
  productRecordDigest: string;
  priceGateRecordDigest: string | null;
  productId: string;
  outcomeCount: number;
  executionReleaseSetId: string;
  infrastructureProfile: string;
  infrastructureRecognition: ProtocolInfrastructureInspectionV1['recognition'];
  marketRentTopUp: string;
  rentCreditRentDebit: string;
  lastValidBlockHeight: string;
  accountAddresses: ReadonlyArray<string>;
  /** Every non-signer key a routing table may carry, in first-seen order. */
  routableAddresses: ReadonlyArray<string>;
  requiredSigners: ReadonlyArray<string>;
  requestBytes: Uint8Array;
  /** `vacant` when this plan must create the credit, `created` when it exists. */
  rentCreditState: 'vacant' | 'created';
  rentCreateRequestBytes: Uint8Array | null;
  rentCreateTransaction: VersionedTransaction | null;
  rentCreateWireBytes: Uint8Array | null;
  /**
   * The compiled Found37/Found39 packet, or null when it could not be compiled.
   *
   * Null is not a failure of the derivation: everything above it -- the Market
   * address, the selected exact account frame, the exact rent debit -- is derived and
   * correct. It means only that this frame does not fit a packet without a
   * routing table, which is a fact about the frame and not about the caller.
   * `foundRefusal` says so in words.
   */
  transaction: VersionedTransaction | null;
  wireBytes: Uint8Array | null;
  foundRefusal: string | null;
}>;

export type CompiledCoreFoundTransactionV2 = Readonly<{
  requestBytes: Uint8Array;
  transaction: VersionedTransaction;
  wireBytes: Uint8Array;
  requiredSigners: ReadonlyArray<string>;
}>;

export type CompiledLifecycleRentCreateTransactionV2 = Readonly<{
  rentCredit: string;
  requestBytes: Uint8Array;
  transaction: VersionedTransaction;
  wireBytes: Uint8Array;
  requiredSigners: ReadonlyArray<string>;
}>;

type RecordAuthority = Readonly<{
  raw: string;
  staging: string;
  schema: Uint8Array;
  digest: Uint8Array;
  bytes: Uint8Array;
  rentMinimum: bigint;
}>;

type ProductGraph = Readonly<{ productId: Uint8Array; outcomeCount: number }>;

function key(value: string, field: string): PublicKey {
  const parsed = new PublicKey(value);
  if (parsed.toBase58() !== value) throw new Error(`${field} must be canonical base58 text`);
  return parsed;
}

function same(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function accountMap(entries: Awaited<ReturnType<SolanaRpcClient['multipleAccounts']>>): ReadonlyMap<string, RpcAccount | null> {
  return new Map(entries.accounts.map((entry) => [entry.address, entry.account]));
}

/** The RPC's hard bound on how many keys one `getMultipleAccounts` may name. */
export const MAX_MULTIPLE_ACCOUNT_KEYS_V1 = 32;

/**
 * Bytes one account costs a `getMultipleAccounts` response beyond its payload.
 *
 * Measured against a devnet node rather than guessed: a single-account response
 * over a 572,317-byte ProgramData came back 269 bytes longer than the base64 of
 * its data, and a seven-account response 167 bytes per account longer. This is
 * that figure with slack, because a planner that underestimates the envelope
 * plans a chunk the node refuses.
 */
const RPC_ACCOUNT_ENVELOPE_BYTES_V1 = 256;

/** Bytes the JSON-RPC result wrapper itself costs, with the same slack. */
const RPC_RESPONSE_ENVELOPE_BYTES_V1 = 1_024;

/** What one account of this data length will cost the response, base64 included. */
function accountResponseBytesV1(space: number): number {
  return 4 * Math.ceil(space / 3) + RPC_ACCOUNT_ENVELOPE_BYTES_V1;
}

/**
 * Split one address list into chunks the node can actually answer.
 *
 * `getMultipleAccounts` has TWO bounds and only one of them is a key count.
 * Splitting at 32 keys and stopping there is correct for a frame of protocol
 * records, which are hundreds of bytes each, and wrong for any frame carrying a
 * ProgramData account, which is a whole ELF. Measured on cohort-13: the wallet
 * terminal payout frame's first 32-key chunk was 5,269,020 bytes against this
 * client's 4 MiB response bound, and the derivation could not complete in a
 * browser at all.
 *
 * So the split is by CUMULATIVE SIZE, under the key bound as well. Order is
 * preserved and no address is dropped or repeated: the concatenation of the
 * chunks is the input.
 *
 * A single account too large to be read on its own is REFUSED BY NAME rather
 * than emitted as a chunk the node will reject. That is a real bound — an
 * account above roughly 3 MiB cannot be read whole by this client — and the
 * caller that hits it needs a byte window (`multipleAccountDataSlices`), not a
 * different chunking.
 */
export function planFinalizedAccountChunksV1(
  sizes: ReadonlyArray<Readonly<{ address: string; space: number }>>,
  responseBound: number = MAX_RPC_RESPONSE_BYTES,
): ReadonlyArray<ReadonlyArray<string>> {
  const budget = responseBound - RPC_RESPONSE_ENVELOPE_BYTES_V1;
  const chunks: Array<Array<string>> = [];
  let current: Array<string> = [];
  let bytes = 0;
  for (const entry of sizes) {
    const cost = accountResponseBytesV1(entry.space);
    if (cost > budget) {
      throw new Error(
        `account ${entry.address} is ${entry.space} bytes and cannot be read whole under the ${responseBound}-byte response bound`,
      );
    }
    if (current.length === MAX_MULTIPLE_ACCOUNT_KEYS_V1 || bytes + cost > budget) {
      chunks.push(current);
      current = [];
      bytes = 0;
    }
    current.push(entry.address);
    bytes += cost;
  }
  if (current.length > 0) chunks.push(current);
  return Object.freeze(chunks.map((chunk) => Object.freeze(chunk)));
}

/**
 * Learn every address's data length without downloading one body.
 *
 * `space` is the account's FULL data length and the node reports it whether or
 * not a `dataSlice` was asked for -- confirmed against devnet, where a
 * one-byte slice over the seven cohort-13 ProgramData accounts came back in
 * 1,296 bytes total and still named 2,320,197 for the largest. A vacant address
 * costs nothing and plans as zero.
 */
async function learnFinalizedAccountSizesV1(
  client: Pick<SolanaRpcClient, 'multipleAccountDataSlices'>,
  addresses: ReadonlyArray<string>,
  minimumContextSlot: string,
): Promise<ReadonlyArray<Readonly<{ address: string; space: number }>>> {
  const sizes: Array<Readonly<{ address: string; space: number }>> = [];
  for (let offset = 0; offset < addresses.length; offset += MAX_MULTIPLE_ACCOUNT_KEYS_V1) {
    const window = addresses.slice(offset, offset + MAX_MULTIPLE_ACCOUNT_KEYS_V1);
    const observation = await client.multipleAccountDataSlices(window, 0, 1, minimumContextSlot);
    if (BigInt(observation.slot) < BigInt(minimumContextSlot)) {
      throw new Error('chunked finalized sizing regressed below its context floor');
    }
    for (const entry of observation.accounts) {
      sizes.push(Object.freeze({ address: entry.address, space: entry.account === null ? 0 : entry.account.space }));
    }
  }
  return Object.freeze(sizes);
}

/** Whether two observations of one address are the same account, byte for byte. */
function sameAccountV1(left: RpcAccount | null, right: RpcAccount | null): boolean {
  if (left === null || right === null) return left === right;
  return left.owner === right.owner
    && left.lamports === right.lamports
    && left.executable === right.executable
    && left.space === right.space
    && left.data.length === right.data.length
    && left.data.every((byte, index) => byte === right.data[index]);
}

/**
 * Acquire one finalized observation without exceeding EITHER RPC bound.
 *
 * A sizing round first, then bodies in chunks planned from those sizes. The
 * sizing round is deliberately outside the consistency proof below: a size is
 * only used to PLAN, so a stale one either still fits or is refused loudly by
 * the response bound, and holding the round that decides nothing to the same
 * bar would let it fail the acquisition.
 *
 * WHAT REPLACED "EVERY CHUNK REPORTS THE SAME SLOT", and why it had to. That
 * check read as the strongest possible statement and was in fact
 * UNSATISFIABLE: `getMultipleAccounts` answers from the node's current
 * finalized bank, and devnet's advances while a round is in flight. Measured
 * 2026-09-02 against cohort-13's payout frame -- two chunks, four attempts,
 * and the second chunk came back **two slots later every single time**. It had
 * never fired only because every round this client made until now fit in one
 * chunk, and the byte bound stopped the first round that did not before the
 * second chunk was ever requested. A check nothing can satisfy is not a strong
 * check; it is a wall in front of the feature.
 *
 * So the composite is allowed to straddle a tick, and it is PROVED to be one
 * picture instead of asserted to be: every chunk but the last is read again
 * after the last one, at the greatest slot observed, and must come back byte
 * for byte identical. If nothing an earlier chunk named changed over the whole
 * window, the accounts read at either end describe the same state.
 *
 * That costs one extra read per chunk beyond the first, and NOTHING at all for
 * a single-chunk round, which is every round but the payout frame. An account
 * that changed and changed back inside the window would pass; every account
 * these frames name is either content-addressed by the digest it was found
 * through or carries a monotonic revision, so that is not a shape this chain
 * produces.
 */
export async function acquireFinalizedAccountsInChunksV1(
  client: Pick<SolanaRpcClient, 'multipleAccounts' | 'multipleAccountDataSlices'>,
  addresses: ReadonlyArray<string>,
  minimumContextSlot: string,
): Promise<Awaited<ReturnType<SolanaRpcClient['multipleAccounts']>>> {
  if (addresses.length === 0 || new Set(addresses).size !== addresses.length) {
    throw new Error('chunked finalized acquisition requires distinct nonempty addresses');
  }
  const plan = planFinalizedAccountChunksV1(
    await learnFinalizedAccountSizesV1(client, addresses, minimumContextSlot),
  );
  const accounts: Array<{ address: string; account: RpcAccount | null }> = [];
  let observedSlot: string | null = null;
  for (const chunk of plan) {
    const observation = await client.multipleAccounts(chunk, minimumContextSlot);
    if (BigInt(observation.slot) < BigInt(minimumContextSlot)) {
      throw new Error('chunked finalized acquisition regressed below its context floor');
    }
    if (observedSlot === null || BigInt(observation.slot) > BigInt(observedSlot)) observedSlot = observation.slot;
    accounts.push(...observation.accounts);
  }
  if (observedSlot === null) throw new Error('chunked finalized acquisition returned no context');
  const first = new Map(accounts.map((entry) => [entry.address, entry.account]));
  for (const chunk of plan.slice(0, -1)) {
    const again = await client.multipleAccounts(chunk, observedSlot);
    if (BigInt(again.slot) < BigInt(observedSlot)) {
      throw new Error('chunked finalized acquisition regressed below its context floor');
    }
    for (const entry of again.accounts) {
      if (!sameAccountV1(first.get(entry.address) ?? null, entry.account)) {
        throw new Error(`chunked finalized acquisition read ${entry.address} changing between its chunks`);
      }
    }
  }
  return Object.freeze({ slot: observedSlot, accounts: Object.freeze(accounts) });
}

function required(accounts: ReadonlyMap<string, RpcAccount | null>, address: string, field: string): RpcAccount {
  const account = accounts.get(address);
  if (account === undefined || account === null) throw new Error(`${field} ${address} is absent at finalized commitment`);
  return account;
}

function vacant(account: RpcAccount | null | undefined, field: string): void {
  if (account === null || account === undefined) return;
  if (account.owner !== SYSTEM_PROGRAM_ID || account.executable || account.data.length !== 0) {
    throw new Error(`${field} is not a vacant System-owned account`);
  }
}

function u32(bytes: Uint8Array, offset: number): number {
  return new DataView(bytes.buffer, bytes.byteOffset + offset, 4).getUint32(0, true);
}

function i128(bytes: Uint8Array, offset: number): bigint {
  const view = new DataView(bytes.buffer, bytes.byteOffset + offset, 16);
  return (view.getBigInt64(8, true) << 64n) + view.getBigUint64(0, true);
}

function putU16(bytes: Uint8Array, offset: number, value: number): void {
  new DataView(bytes.buffer, bytes.byteOffset + offset, 2).setUint16(0, value, true);
}

function putU64(bytes: Uint8Array, offset: number, value: bigint): void {
  new DataView(bytes.buffer, bytes.byteOffset + offset, 8).setBigUint64(0, value, true);
}

function compareBytes(left: Uint8Array, right: Uint8Array): number {
  for (let index = 0; index < left.length; index += 1) {
    if (left[index] !== right[index]) return left[index] < right[index] ? -1 : 1;
  }
  return 0;
}

function gcd(left: bigint, right: bigint): bigint {
  let a = left;
  let b = right;
  while (b !== 0n) [a, b] = [b, a % b];
  return a;
}

async function recordAuthority(
  client: Pick<SolanaRpcClient, 'minimumBalanceForRentExemption'>,
  registry: string,
  raw: string,
  schema: Uint8Array,
  account: RpcAccount,
  field: string,
): Promise<RecordAuthority> {
  if (account.owner !== registry || account.executable) throw new Error(`${field} is not a nonexecutable Registry-owned raw record`);
  const digest = await sha256(account.data);
  const coordinates = deriveFinalizedRecordAddressesV1(registry, schema, digest);
  if (coordinates.record !== raw) throw new Error(`${field} is not the schema/content-derived Registry raw PDA`);
  const rent = await client.minimumBalanceForRentExemption(account.data.length);
  if (BigInt(account.lamports) < BigInt(rent.lamports)) throw new Error(`${field} is below its exact current rent minimum`);
  return Object.freeze({ raw, staging: coordinates.staging, schema, digest, bytes: account.data, rentMinimum: BigInt(rent.lamports) });
}

function validateRealm(bytes: Uint8Array): void {
  if (bytes.length !== REALM_BYTES_V1
      || ascii(bytes, REALM_MAGIC_OFFSET_V1, 8) !== REALM_MAGIC_V1
      || u16(bytes, REALM_SCHEMA_VERSION_OFFSET_V1) !== REALM_SCHEMA_VERSION_V1) throw new Error('Realm record has the wrong exact ABI');
  if (bytes[REALM_MINT_AUTHORITY_POLICY_OFFSET_V1] > 1 || bytes[REALM_FREEZE_AUTHORITY_POLICY_OFFSET_V1] > 1) throw new Error('Realm authority policy is undefined');
  requireZero(bytes, REALM_RESERVED_OFFSET_V1, REALM_RESERVED_BYTES_V1, 'Realm header');
  [REALM_TOKEN_PROGRAM_OFFSET_V1, REALM_COLLATERAL_MINT_OFFSET_V1, REALM_ADAPTER_RELEASE_ID_OFFSET_V1]
    .forEach((offset) => requireNonzero(slice(bytes, offset, 32), 'Realm identity'));
}

export function validateCoreFoundSourceMaterialV3(bytes: Uint8Array, productDigest: Uint8Array): void {
  if (bytes.length !== SOURCE_MATERIAL_BYTES || ascii(bytes, 0, 8) !== 'DCLTSMV3' || u16(bytes, 8) !== 3) throw new Error('SourceMaterialV3 has the wrong exact ABI');
  if (bytes[10] > 1 || (bytes[11] !== 1 && bytes[11] !== 2)) throw new Error('SourceMaterialV3 tag is undefined');
  requireZero(bytes, 12, 4, 'SourceMaterialV3 header');
  if (!same(slice(bytes, 16, 32), productDigest)) throw new Error('SourceMaterialV3 selects a different Product record digest');
  [16, 48, 80, 112, 176].forEach((offset) => requireNonzero(slice(bytes, offset, 32), 'SourceMaterialV3 identity'));
  const recovery = slice(bytes, 144, 32);
  const floor = slice(bytes, 208, 32);
  if ((bytes[10] === 0 && !isZero(recovery)) || (bytes[10] === 1 && isZero(recovery))) throw new Error('SourceMaterialV3 recovery policy is noncanonical');
  if ((bytes[11] === 1 && !isZero(floor)) || (bytes[11] === 2 && isZero(floor))) throw new Error('SourceMaterialV3 principal policy is noncanonical');
}

function validateFundingQuote(bytes: Uint8Array): Readonly<{ rent: bigint; creation: bigint }> {
  const kindOffset = CAPABILITY_FUNDING_QUOTE_COLLATERAL_KIND_OFFSET_V1;
  if (bytes.length !== CAPABILITY_FUNDING_QUOTE_BYTES_V1
      || ascii(bytes, CAPABILITY_FUNDING_QUOTE_MAGIC_OFFSET_V1, 8) !== CAPABILITY_FUNDING_QUOTE_MAGIC_V1
      || u16(bytes, CAPABILITY_FUNDING_QUOTE_SCHEMA_OFFSET_V1) !== CAPABILITY_FUNDING_QUOTE_SCHEMA_VERSION_V1
      || bytes[kindOffset] > 1) throw new Error('capability funding quote has the wrong exact ABI');
  requireZero(bytes, CAPABILITY_FUNDING_QUOTE_RESERVED_OFFSET_V1, CAPABILITY_FUNDING_QUOTE_RESERVED_BYTES_V1, 'capability funding quote header');
  const binding = slice(bytes, CAPABILITY_FUNDING_QUOTE_BINDING_OFFSET_V1, CAPABILITY_FUNDING_BINDING_BYTES_V1);
  if (bytes[kindOffset] === 0) requireZero(binding, 0, CAPABILITY_FUNDING_BINDING_BYTES_V1, 'absent Realm funding binding');
  else [
    CAPABILITY_FUNDING_BINDING_REALM_ID_OFFSET_V1,
    CAPABILITY_FUNDING_BINDING_RELEASE_ID_OFFSET_V1,
    CAPABILITY_FUNDING_BINDING_TOKEN_PROGRAM_OFFSET_V1,
    CAPABILITY_FUNDING_BINDING_MINT_OFFSET_V1,
    CAPABILITY_FUNDING_BINDING_BENEFICIARY_OFFSET_V1,
  ].forEach((offset) => requireNonzero(slice(binding, offset, 32), 'Realm funding binding'));
  let nativeTotal = 0n;
  let realmTotal = 0n;
  const amounts: bigint[] = [];
  const amountsOffset = CAPABILITY_FUNDING_QUOTE_AMOUNTS_OFFSET_V1;
  for (let index = 0; index < FUNDING_COMPARTMENTS_V1.length; index += 1) {
    const offset = amountsOffset + FUNDING_COMPARTMENTS_V1[index].offset;
    const asset = bytes[offset + CAPABILITY_FUNDING_ALLOCATION_CLASS_OFFSET_V1];
    const amount = u64(bytes, offset + CAPABILITY_FUNDING_ALLOCATION_AMOUNT_OFFSET_V1);
    requireZero(bytes, offset + CAPABILITY_FUNDING_ALLOCATION_RESERVED_OFFSET_V1, CAPABILITY_FUNDING_ALLOCATION_RESERVED_BYTES_V1, 'capability funding allocation');
    const nativeOnly = FUNDING_COMPARTMENTS_V1[index].assetPolicy === 'native-lamports-only';
    if (asset > 2 || (amount === 0n) !== (asset === 0) || (nativeOnly && asset === 2)) throw new Error('capability funding allocation has a noncanonical asset class');
    if (asset === 1) nativeTotal += amount;
    if (asset === 2) realmTotal += amount;
    if (nativeTotal > 0xffff_ffff_ffff_ffffn || realmTotal > 0xffff_ffff_ffff_ffffn) {
      throw new Error('capability funding compartment total overflows u64');
    }
    amounts.push(amount);
  }
  if (u64(bytes, amountsOffset + CAPABILITY_FUNDING_AMOUNTS_NATIVE_TOTAL_OFFSET_V1) !== nativeTotal
      || u64(bytes, amountsOffset + CAPABILITY_FUNDING_AMOUNTS_REALM_TOTAL_OFFSET_V1) !== realmTotal) throw new Error('capability funding totals do not equal their typed compartments');
  if ((realmTotal === 0n) !== (bytes[kindOffset] === 0)) throw new Error('Realm funding binding does not match Realm collateral use');
  return Object.freeze({ rent: amounts[0], creation: amounts[1] });
}

export function validateCoreFoundCapabilityManifestV1(bytes: Uint8Array): void {
  if (bytes.length < CAPABILITY_MANIFEST_HEADER_BYTES_V1
      || ascii(bytes, CAPABILITY_MANIFEST_MAGIC_OFFSET_V1, 8) !== CAPABILITY_MANIFEST_MAGIC_V1
      || u16(bytes, CAPABILITY_MANIFEST_SCHEMA_OFFSET_V1) !== CAPABILITY_MANIFEST_SCHEMA_VERSION_V1
      || u16(bytes, CAPABILITY_MANIFEST_PROFILE_OFFSET_V1) !== CAPABILITY_MANIFEST_ARTIFACT_PROFILE_V1) throw new Error('capability manifest has the wrong exact header');
  requireZero(bytes, CAPABILITY_MANIFEST_RESERVED_OFFSET_V1, CAPABILITY_MANIFEST_HEADER_RESERVED_BYTES_V1, 'capability manifest header');
  const count = u16(bytes, CAPABILITY_MANIFEST_COUNT_OFFSET_V1);
  if (count > MAX_CAPABILITIES_V1 || bytes.length !== CAPABILITY_MANIFEST_HEADER_BYTES_V1 + count * CAPABILITY_ENTRY_BYTES_V1) throw new Error('capability manifest width is not canonical');
  const dependencies: number[][] = [];
  let previous: Uint8Array | null = null;
  for (let index = 0; index < count; index += 1) {
    const offset = CAPABILITY_MANIFEST_HEADER_BYTES_V1 + index * CAPABILITY_ENTRY_BYTES_V1;
    const kind = slice(bytes, offset + CAPABILITY_ENTRY_KIND_ID_OFFSET_V1, 32);
    [
      CAPABILITY_ENTRY_KIND_ID_OFFSET_V1,
      CAPABILITY_ENTRY_RELEASE_ID_OFFSET_V1,
      CAPABILITY_ENTRY_CONFIG_ID_OFFSET_V1,
      CAPABILITY_ENTRY_CAPACITY_PROFILE_ID_OFFSET_V1,
      CAPABILITY_ENTRY_CHILD_SCHEMA_ID_OFFSET_V1,
      CAPABILITY_ENTRY_CHILD_DERIVATION_ID_OFFSET_V1,
    ].forEach((relative) => requireNonzero(slice(bytes, offset + relative, 32), 'capability entry identity'));
    if (previous !== null && compareBytes(previous, kind) >= 0) throw new Error('capability entries are not strictly ordered');
    previous = kind;
    const policy = bytes[offset + CAPABILITY_ENTRY_ACTIVATION_POLICY_OFFSET_V1];
    const dependencyCount = bytes[offset + CAPABILITY_ENTRY_DEPENDENCY_COUNT_OFFSET_V1];
    if (policy > 1 || dependencyCount > MAX_DEPENDENCIES_PER_CAPABILITY_V1) throw new Error('capability entry policy or dependency count is undefined');
    requireZero(bytes, offset + CAPABILITY_ENTRY_RESERVED_OFFSET_V1, CAPABILITY_ENTRY_RESERVED_BYTES_V1, 'capability entry header');
    const active: number[] = [];
    for (let position = 0; position < MAX_DEPENDENCIES_PER_CAPABILITY_V1; position += 1) {
      const dependency = bytes[offset + CAPABILITY_ENTRY_DEPENDENCIES_OFFSET_V1 + position];
      if (position < dependencyCount) {
        if (dependency >= count || dependency === index || (active.length > 0 && active[active.length - 1] >= dependency)) throw new Error('capability dependency is invalid or noncanonical');
        active.push(dependency);
      } else if (dependency !== 0) throw new Error('inactive capability dependency is nonzero');
    }
    const deadline = u64(bytes, offset + CAPABILITY_ENTRY_ACTIVATION_DEADLINE_OFFSET_V1);
    const quote = validateFundingQuote(slice(bytes, offset + CAPABILITY_ENTRY_QUOTE_OFFSET_V1, CAPABILITY_FUNDING_QUOTE_BYTES_V1));
    if ((policy === 0 && deadline !== 0n) || (policy === 1 && (deadline === 0n || (quote.rent === 0n && quote.creation === 0n)))) throw new Error('capability activation policy and prepaid creation facts do not join');
    dependencies.push(active);
  }
  const resolved = new Set<number>();
  while (resolved.size < count) {
    const before = resolved.size;
    dependencies.forEach((entry, index) => { if (!resolved.has(index) && entry.every((dependency) => resolved.has(dependency))) resolved.add(index); });
    if (resolved.size === before) throw new Error('capability dependency graph is cyclic');
  }
}

/**
 * The Product Runtime V2 root's own header, checked in exactly one place.
 *
 * Two callers need it — the graph decoder, which also has the domain and
 * portfolio bytes, and the dependent-record derivation, which has only this
 * record. Writing the check twice would put the magic and version coordinates
 * in the browser's own words a second time, which `abi-coverage.mjs` counts
 * and refuses; it caught exactly that when the derivation was first written.
 */
function validateProductRootHeaderV2(product: Uint8Array): void {
  if (product.length !== PRODUCT_RECORD_BYTES || ascii(product, 0, 8) !== PRODUCT_RECORD_MAGIC_V2 || u16(product, 8) !== 2) throw new Error('Product Runtime V2 root has the wrong exact ABI');
  requireZero(product, 10, 6, 'Product Runtime V2 root');
}

/** One ResultDomainV2 record: the outcome partition, as the operator wrote it. */
export type ResultDomainV2 = Readonly<{
  /** Ordinary cells: always `cuts.length + 1`. */
  regionCount: number;
  /** Ticks per whole unit of the coordinate. */
  denominator: bigint;
  /** The interior boundaries, strictly increasing, in ticks. */
  cuts: ReadonlyArray<bigint>;
}>;

/**
 * Decode the operator's own outcome partition.
 *
 * THE DEFECT THIS CLOSES. These exact bytes were already being decoded here,
 * validated for ABI, width, identity and strict increase -- and then thrown
 * away, because the graph decoder only needed to know they were well formed.
 * So the one artifact that says what a market's outcomes ARE reached the
 * browser and left no trace, while `/product-v2` rendered a parallel list of
 * "interpolation segment N" derived in TypeScript from the payoff KNOTS and
 * presented it in the place a reader looks for the partition. Knots are where
 * the payoff bends. Cuts are where the outcome changes. They are not the same
 * list, they are not the same length, and only one of them is on chain.
 *
 * C-02's closing clause asks that the same artifacts found by the operator be
 * explained and inspectable in the client. A client that re-derives its own
 * parallel description is what that clause forbids, so the cuts are returned
 * now instead of discarded.
 */
export function decodeResultDomainV2(domain: Uint8Array): ResultDomainV2 {
  if (domain.length < DOMAIN_HEADER_BYTES || ascii(domain, 0, 8) !== PRODUCT_RUNTIME_DOMAIN_MAGIC_V2 || u16(domain, 8) !== 2 || u16(domain, 10) !== DOMAIN_HEADER_BYTES || u32(domain, 12) !== domain.length) throw new Error('Product result domain has the wrong exact ABI');
  requireZero(domain, 24, 8, 'Product result-domain header');
  requireZero(domain, 232, 8, 'Product result-domain tail');
  const regionCount = u32(domain, 16);
  const cutCount = u32(domain, 20);
  if (regionCount === 0 || regionCount !== cutCount + 1 || domain.length !== DOMAIN_HEADER_BYTES + cutCount * 16) throw new Error('Product result-domain width is inconsistent');
  [32, 64, 96, 128, 160, 192].forEach((offset) => requireNonzero(slice(domain, offset, 32), 'Product result-domain identity'));
  const denominator = u64(domain, 224);
  if (denominator === 0n) throw new Error('Product result-domain identity or denominator differs');
  const cuts: bigint[] = [];
  let prior: bigint | null = null;
  for (let index = 0; index < cutCount; index += 1) {
    const cut = i128(domain, DOMAIN_HEADER_BYTES + index * 16);
    if (prior !== null && cut <= prior) throw new Error('Product result-domain cuts are not strictly increasing');
    prior = cut;
    cuts.push(cut);
  }
  return Object.freeze({ regionCount, denominator, cuts: Object.freeze(cuts) });
}

export function decodeCoreFoundProductGraphV2(product: Uint8Array, domain: Uint8Array, portfolio: Uint8Array, domainDigest: Uint8Array, portfolioDigest: Uint8Array): ProductGraph {
  validateProductRootHeaderV2(product);
  const productId = slice(product, 16, 32);
  requireNonzero(productId, 'Product identity');
  if (!same(slice(product, 48, 32), domainDigest) || !same(slice(product, 80, 32), portfolioDigest)) throw new Error('Product root does not select the supplied domain and portfolio');

  const partition = decodeResultDomainV2(domain);
  if (!same(slice(domain, 32, 32), productId)) throw new Error('Product result-domain identity or denominator differs');
  const regions = partition.regionCount;

  if (portfolio.length < PORTFOLIO_HEADER_BYTES || ascii(portfolio, 0, 8) !== PRODUCT_RUNTIME_PORTFOLIO_MAGIC_V2 || u16(portfolio, 8) !== 2 || u16(portfolio, 10) !== PORTFOLIO_HEADER_BYTES || u32(portfolio, 12) !== portfolio.length) throw new Error('Product portfolio has the wrong exact ABI');
  const coefficientCount = u32(portfolio, 16);
  if (coefficientCount === 0 || coefficientCount !== regions + 1 || portfolio.length !== PORTFOLIO_HEADER_BYTES + coefficientCount * 8 || portfolio[20] !== 1) throw new Error('Product portfolio width or rounding boundary is inconsistent');
  requireZero(portfolio, 21, 11, 'Product portfolio header');
  requireZero(portfolio, 200, 8, 'Product portfolio tail');
  [32, 64, 96, 128, 160].forEach((offset) => requireNonzero(slice(portfolio, offset, 32), 'Product portfolio identity'));
  if (!same(slice(portfolio, 32, 32), productId) || !same(slice(portfolio, 64, 32), domainDigest) || !same(slice(portfolio, 128, 32), slice(domain, 128, 32)) || !same(slice(portfolio, 160, 32), slice(domain, 160, 32))) throw new Error('Product domain and portfolio identities do not join');
  let divisor = u64(portfolio, 192);
  let nonzero = false;
  if (divisor === 0n) throw new Error('Product portfolio denominator is zero');
  for (let index = 0; index < coefficientCount; index += 1) {
    const coefficient = u64(portfolio, PORTFOLIO_HEADER_BYTES + index * 8);
    nonzero ||= coefficient !== 0n;
    divisor = gcd(divisor, coefficient);
  }
  if (!nonzero || divisor !== 1n) throw new Error('Product portfolio is empty or not gcd-normalized');
  const outcomeCount = regions + 1;
  if (outcomeCount > MAX_U32) throw new Error('Product outcome width exceeds u32');
  return Object.freeze({ productId, outcomeCount });
}

function generationBytes(generation: bigint): Uint8Array {
  if (generation <= 0n || generation > 0xffff_ffff_ffff_ffffn) throw new Error('Market generation is outside lifecycle u64');
  const bytes = new Uint8Array(8);
  putU64(bytes, 0, generation);
  return bytes;
}

function lifecycleRentCreateRequest(input: Readonly<{
  refundWallet: PublicKey;
  market: PublicKey;
  releaseSet: Uint8Array;
  generation: bigint;
  bump: number;
}>): Uint8Array {
  if (input.releaseSet.length !== 32 || isZero(input.releaseSet)) throw new Error('execution release set is not one nonzero 32-byte identity');
  if (same(input.refundWallet.toBytes(), input.market.toBytes())
      || same(input.refundWallet.toBytes(), input.releaseSet)
      || same(input.market.toBytes(), input.releaseSet)) {
    throw new Error('lifecycle Rent identities alias');
  }
  const output = new Uint8Array(CREATE_LIFECYCLE_RENT_CREDIT_BYTES_V2);
  output.set(LIFECYCLE_RENT_INSTRUCTION_MAGIC_V2, 0);
  putU16(output, 8, LIFECYCLE_RENT_SCHEMA_VERSION_V2);
  // The action discriminator, not a reserved byte. This was a hand-written `0`
  // and `LifecycleRentActionV2::Create` is 1, so every packet this builder
  // produced was refused at `LifecycleRentInstructionV2::decode` before the
  // Rent program looked at a single account. Nothing caught it because `/found`
  // downloaded the packet and never submitted one; the first submission, from
  // the create wizard against a local validator, refused in 1,041 CU.
  output[LIFECYCLE_RENT_INSTRUCTION_ACTION_OFFSET_V2] = LIFECYCLE_RENT_ACTION_CREATE_V2;
  output.set(input.refundWallet.toBytes(), 16);
  output.set(input.market.toBytes(), 48);
  output.set(input.releaseSet, 80);
  output.set(generationBytes(input.generation), 112);
  output[120] = input.bump;
  return output;
}

export function compileLifecycleRentCreateTransactionV2(input: Readonly<{
  payer: string;
  refundWallet: string;
  market: string;
  releaseSet: Uint8Array;
  generation: bigint;
  rentProgram: string;
  recentBlockhash: string;
}>): CompiledLifecycleRentCreateTransactionV2 {
  const payer = key(input.payer, 'payer');
  const refundWallet = key(input.refundWallet, 'refund wallet');
  const market = key(input.market, 'Market');
  const rentProgram = key(input.rentProgram, 'Rent program');
  const generation = generationBytes(input.generation);
  const [rentCredit, bump] = PublicKey.findProgramAddressSync(
    [LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2, market.toBytes(), generation],
    rentProgram,
  );
  const requestBytes = lifecycleRentCreateRequest({
    refundWallet,
    market,
    releaseSet: input.releaseSet,
    generation: input.generation,
    bump,
  });
  const instruction = new TransactionInstruction({
    programId: rentProgram,
    keys: [
      { pubkey: payer, isSigner: true, isWritable: true },
      { pubkey: rentCredit, isSigner: false, isWritable: true },
      { pubkey: key(SYSTEM_PROGRAM_ID, 'System program'), isSigner: false, isWritable: false },
      { pubkey: key(RENT_SYSVAR_ID, 'Rent sysvar'), isSigner: false, isWritable: false },
    ],
    data: requestBytes as Buffer,
  });
  const transaction = new VersionedTransaction(new TransactionMessage({
    payerKey: payer,
    recentBlockhash: key(input.recentBlockhash, 'recent blockhash').toBase58(),
    instructions: [instruction],
  }).compileToV0Message());
  const wireBytes = transaction.serialize();
  if (wireBytes.length > SOLANA_PACKET_BYTES_V1) throw new Error('lifecycle Rent Create exceeds the packet bound');
  const requiredSigners = Object.freeze(transaction.message.staticAccountKeys
    .slice(0, transaction.message.header.numRequiredSignatures)
    .map((value) => value.toBase58()));
  if (requiredSigners.length !== 1 || requiredSigners[0] !== input.payer) throw new Error('lifecycle Rent Create requires an unexpected signer');
  return Object.freeze({ rentCredit: rentCredit.toBase58(), requestBytes, transaction, wireBytes, requiredSigners });
}

/**
 * Where Core's `Action` tag sits in a request body.
 *
 * Core's dispatch is TWO coordinates, not one: `CORE_REQUEST_MAGIC` says the
 * instruction is a Core request, and this byte narrows the eleven routes that
 * magic selects -- narrows, not resolves, since `Action::Retire` alone reaches
 * four of them. The magic reaches the browser generated; this
 * coordinate does not, because `dclutch-market`'s
 * `REQUEST_ACTION_OFFSET` is crate-private and `generate-core-found.mjs` emits
 * no Core counterpart to `LIFECYCLE_RENT_INSTRUCTION_ACTION_OFFSET_V2`. It was
 * a bare `output[10]` in the encoder below; naming it is not the fix, it is
 * what makes the missing emission visible and lets a reader of a compiled
 * instruction find the tag the same way the encoder wrote it. THE FIX is one
 * line in that generator, and it belongs to the generator's owner.
 */
export const CORE_REQUEST_ACTION_OFFSET = 10;

function foundRequest(generation: bigint, market: PublicKey): Uint8Array {
  if (generation < 0n || generation > 0xffff_ffff_ffff_ffffn) throw new Error('Market generation is outside u64');
  const output = new Uint8Array(CORE_REQUEST_BYTES);
  output.set(CORE_REQUEST_MAGIC, 0);
  putU16(output, 8, CORE_VERSION);
  output[CORE_REQUEST_ACTION_OFFSET] = CORE_ACTION_FOUND_TAG;
  putU64(output, 32, generation);
  output.set(market.toBytes(), 40);
  return output;
}

export function compileCoreFoundTransactionV2(input: Readonly<{
  payer: string;
  coreProgram: string;
  market: string;
  generation: bigint;
  recentBlockhash: string;
  accountAddresses: ReadonlyArray<string>;
  lookupTable?: AddressLookupTableAccount;
}>): CompiledCoreFoundTransactionV2 {
  const extended = input.accountAddresses.length === CORE_FOUND_PRICE_GATE_ACCOUNT_COUNT_V3;
  if (!extended && input.accountAddresses.length !== CORE_FOUND_ACCOUNT_COUNT_V3) {
    throw new Error(`Core Found requires exactly ${CORE_FOUND_ACCOUNT_COUNT_V3} or ${CORE_FOUND_PRICE_GATE_ACCOUNT_COUNT_V3} account metas`);
  }
  if (new Set(input.accountAddresses).size !== input.accountAddresses.length) {
    throw new Error('Core Found account metas alias named roles');
  }
  if (input.accountAddresses[0] !== input.payer
      || input.accountAddresses[1] !== input.market
      || input.accountAddresses[25] !== input.coreProgram) {
    throw new Error('Core Found payer, Market, or Core program is at the wrong exact account index');
  }
  const payer = key(input.payer, 'payer');
  const market = key(input.market, 'Market');
  const requestBytes = foundRequest(input.generation, market);
  const roles = extended ? CORE_FOUND_PRICE_GATE_ACCOUNT_ROLES_V3 : CORE_FOUND_ACCOUNT_ROLES_V3;
  const metas = input.accountAddresses.map((address, index) => ({
    pubkey: key(address, `Found account ${index}`),
    isSigner: roles[index].signer,
    isWritable: roles[index].writable,
  }));
  const instruction = new TransactionInstruction({
    programId: key(input.coreProgram, 'Core program'),
    keys: metas,
    data: requestBytes as Buffer,
  });
  // The bounded-instruction owner supplies the configured limit. This builder
  // reproduces its bytes; it does not infer a compute margin from the old wire.
  // Current V2 routes require their own pass-count and 20-seed mean evidence.
  const transaction = new VersionedTransaction(new TransactionMessage({
    payerKey: payer,
    recentBlockhash: key(input.recentBlockhash, 'recent blockhash').toBase58(),
    instructions: [...boundedInstructionsV1([instruction])],
  }).compileToV0Message(input.lookupTable === undefined ? undefined : [input.lookupTable]));
  let wireBytes: Uint8Array;
  try {
    wireBytes = transaction.serialize();
  } catch (error) {
    throw new Error(`Core Found transaction exceeds the ${SOLANA_PACKET_BYTES_V1}-byte packet bound${input.lookupTable === undefined ? '; this frame requires a finalized routing table' : ''}`, { cause: error });
  }
  if (wireBytes.length > SOLANA_PACKET_BYTES_V1) {
    throw new Error(input.lookupTable === undefined
      ? `Core Found transaction is ${wireBytes.length} bytes inline, above the ${SOLANA_PACKET_BYTES_V1}-byte packet bound; this frame requires a finalized routing table`
      : `Core Found transaction is ${wireBytes.length} bytes, above the ${SOLANA_PACKET_BYTES_V1}-byte packet bound`);
  }
  const requiredSigners = Object.freeze(transaction.message.staticAccountKeys
    .slice(0, transaction.message.header.numRequiredSignatures)
    .map((value) => value.toBase58()));
  if (requiredSigners.length !== 1 || requiredSigners[0] !== input.payer) {
    throw new Error('Core Found transaction requires an unexpected signer');
  }
  return Object.freeze({ requestBytes, transaction, wireBytes, requiredSigners });
}

function infrastructureEqual(left: ProtocolInfrastructureInspectionV1, right: ProtocolInfrastructureInspectionV1): boolean {
  return left.registryProgram === right.registryProgram
    && left.activationCache === right.activationCache
    && left.executionReleaseSetId === right.executionReleaseSetId
    && left.profilePda === right.profilePda
    && left.profileDigest === right.profileDigest
    && JSON.stringify(left.core) === JSON.stringify(right.core)
    && JSON.stringify(left.registry) === JSON.stringify(right.registry)
    && JSON.stringify(left.rent) === JSON.stringify(right.rent);
}

/**
 * One finalized raw record, authenticated exactly as `prepareCoreFoundV2` does:
 * owner, executable bit, and the schema/content PDA it must hash to.
 *
 * Extracted because two callers need the identical refusal and only one of them
 * can afford a round trip per record. It takes the ACCOUNT rather than an
 * address and a client, so a caller that read fifty records in one observation
 * authenticates each of them by exactly the rule a caller that read one does.
 */
export async function authenticateFinalizedRawRecordV2(
  account: RpcAccount | null,
  address: string,
  registryProgram: string,
  schema: Uint8Array,
  field: string,
): Promise<Readonly<{ bytes: Uint8Array; digest: Uint8Array }>> {
  if (account === null) throw new Error(`${field} does not exist at ${address}`);
  if (account.owner !== registryProgram || account.executable) throw new Error(`${field} is not a nonexecutable Registry-owned raw record`);
  const digest = await sha256(account.data);
  if (deriveFinalizedRecordAddressesV1(registryProgram, schema, digest).record !== address) {
    throw new Error(`${field} is not the schema/content-derived Registry raw PDA`);
  }
  return Object.freeze({ bytes: account.data, digest });
}

/** The five dependent record addresses, and the parent each came out of. */
export type CoreFoundDerivedRecordsV2 = Readonly<{
  resultDomainRecord: string;
  portfolioRecord: string;
  sourceSpecRecord: string;
  /**
   * The manipulation floor, or NULL where the Source material has none.
   *
   * This was typed `string` and derived unconditionally, and that made the
   * whole derivation refuse -- `manipulation floor digest is the all-zero
   * identity` -- on every market founded with an `ExplicitlyUnbounded`
   * principal policy, which is what cohort-12's open market is. The policy is
   * a canonical choice the SourceMaterialV3 states, and
   * `validateCoreFoundSourceMaterialV3` has already proved that an all-zero
   * floor means exactly that choice and nothing else, so an absent floor is a
   * value here rather than a failure.
   */
  manipulationFloorRecord: string | null;
  /**
   * The window this market settles on.
   *
   * `/found` never asked for it -- a Found transaction does not carry the
   * window record -- so it was the one dependent address nobody derived, and
   * the market page consequently told every reader "no settlement time is
   * published" about a market whose window has been on chain since it was
   * founded. Same derivation as the four above, from a coordinate the
   * SourceMaterialV3 names.
   */
  windowSpecRecord: string;
  /** The finalized floor both parents were read at. */
  observedSlot: string;
  /** One sentence per derived address, naming the record and coordinate it came from. */
  provenance: Readonly<Record<'resultDomainRecord' | 'portfolioRecord' | 'sourceSpecRecord' | 'manipulationFloorRecord' | 'windowSpecRecord', string>>;
}>;

/**
 * Turn four of `/found`'s fourteen pasted addresses into read values.
 *
 * THE DEFECT THIS CLOSES. The console asks a stranger for fourteen base58
 * addresses, and five of its own field comments admit the value is one another
 * record on the same list already contains: "Derivable from that record once
 * this console reads it; today it is typed and then checked." Checked is not
 * derived. Every check in `prepareCoreFoundV2` passes for a correctly pasted
 * address and for nothing else, which means the console's remaining failure
 * mode is entirely the reader's transcription -- and a console that asks for a
 * value it could compute is asking the reader to be an oracle for its own
 * chain reads.
 *
 * A Product record carries, at named coordinates, the digests of the result
 * domain and portfolio it selects; a SourceMaterialV3 carries the digests of
 * its source spec, its manipulation floor, and the window it settles on. Digest plus schema id is the
 * whole input to the Registry's raw-record PDA, so each of those four is a
 * derivation, not a question. It runs through `deriveFinalizedRecordAddressesV1`
 * -- the Registry's own constructor, the same one `recordAuthority` checks
 * against -- and the coordinates arrive from `lib/generated/coreFound.ts`,
 * emitted from the Rust that writes them.
 *
 * WHAT DERIVING COSTS, said plainly. For a derived address the PDA equality in
 * `recordAuthority` can no longer fail: it would be comparing a value with
 * itself, and a guard whose two sides move together is not a guard. What
 * survives, and is the whole of the correctness here, is that the two PARENTS
 * are still typed and still checked against their own content PDAs, and that
 * the account found at each derived address must still be Registry-owned,
 * non-executable, rent-sufficient, and hash to the digest its parent named.
 * The children inherit their correctness from the parents' refusal; that is
 * why this function re-derives and re-checks both parents rather than trusting
 * that `prepareCoreFoundV2` will get to it later.
 *
 * The FIFTH derivable address is not here. The capacity profile a source spec
 * selects sits at a coordinate `SourceSpecV1::decode` and `to_bytes` write as
 * a bare `144` (crates/dclutch-source/src/lib.rs:911,927), with no
 * named constant for the ABI generator to emit. Deriving it would mean this
 * browser restating a wire coordinate in its own words, which is the drift
 * `abi-coverage.mjs` exists to refuse. It stays typed, and the missing Rust
 * constant is a routed finding rather than a fifth quiet mirror.
 */
export async function deriveCoreFoundRecordsV2(
  client: Pick<SolanaRpcClient, 'finalizedSlot' | 'multipleAccounts' | 'multipleAccountDataSlices'>,
  input: Readonly<{ registryProgram: string; productRecord: string; sourceMaterialRecord: string }>,
): Promise<CoreFoundDerivedRecordsV2> {
  const registry = key(input.registryProgram, 'Registry program').toBase58();
  const productRecord = key(input.productRecord, 'Product record').toBase58();
  const sourceMaterialRecord = key(input.sourceMaterialRecord, 'SourceMaterialV3 record').toBase58();
  if (productRecord === sourceMaterialRecord) throw new Error('Product and SourceMaterialV3 cannot be the same record');

  const observedSlot = await client.finalizedSlot();
  const observation = await client.multipleAccounts([productRecord, sourceMaterialRecord], observedSlot);
  const accounts = accountMap(observation);

  const product = await authenticateFinalizedRawRecordV2(accounts.get(productRecord) ?? null, productRecord, registry, PRODUCT_RECORD_SCHEMA_ID_V2, 'Product');
  const source = await authenticateFinalizedRawRecordV2(accounts.get(sourceMaterialRecord) ?? null, sourceMaterialRecord, registry, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3, 'Source material');
  // The two parents must be about each other before either can name a child.
  validateCoreFoundSourceMaterialV3(source.bytes, product.digest);
  validateProductRootHeaderV2(product.bytes);

  const at = (bytes: Uint8Array, offset: number, field: string) => {
    const digest = slice(bytes, offset, 32);
    if (isZero(digest)) throw new Error(`${field} is the all-zero identity`);
    return digest;
  };
  const optional = (bytes: Uint8Array, offset: number): Uint8Array | null => {
    const digest = slice(bytes, offset, 32);
    return isZero(digest) ? null : digest;
  };
  const derive = (schema: Uint8Array, digest: Uint8Array) => deriveFinalizedRecordAddressesV1(registry, schema, digest).record;

  return Object.freeze({
    resultDomainRecord: derive(RESULT_DOMAIN_SCHEMA_ID_V2, at(product.bytes, PRODUCT_RECORD_DOMAIN_DIGEST_OFFSET_V2, 'result domain digest')),
    portfolioRecord: derive(PORTFOLIO_SCHEMA_ID_V2, at(product.bytes, PRODUCT_RECORD_PORTFOLIO_DIGEST_OFFSET_V2, 'portfolio digest')),
    sourceSpecRecord: derive(SOURCE_SPEC_SCHEMA_ID_V1, at(source.bytes, SOURCE_MATERIAL_PRIMARY_SOURCE_SPEC_OFFSET_V3, 'source spec digest')),
    manipulationFloorRecord: optional(source.bytes, SOURCE_MATERIAL_MANIPULATION_FLOOR_OFFSET_V3) === null
      ? null
      : derive(MANIPULATION_FLOOR_SCHEMA_RELEASE_ID_V1, at(source.bytes, SOURCE_MATERIAL_MANIPULATION_FLOOR_OFFSET_V3, 'manipulation floor digest')),
    windowSpecRecord: derive(WINDOW_SPEC_SCHEMA_ID_V1, at(source.bytes, SOURCE_MATERIAL_WINDOW_SPEC_OFFSET_V3, 'window spec digest')),
    observedSlot: observation.slot,
    provenance: Object.freeze({
      resultDomainRecord: `Read from the Product record at byte ${PRODUCT_RECORD_DOMAIN_DIGEST_OFFSET_V2}, at finalized slot ${observation.slot}.`,
      portfolioRecord: `Read from the Product record at byte ${PRODUCT_RECORD_PORTFOLIO_DIGEST_OFFSET_V2}, at finalized slot ${observation.slot}.`,
      sourceSpecRecord: `Read from the SourceMaterialV3 record at byte ${SOURCE_MATERIAL_PRIMARY_SOURCE_SPEC_OFFSET_V3}, at finalized slot ${observation.slot}.`,
      manipulationFloorRecord: optional(source.bytes, SOURCE_MATERIAL_MANIPULATION_FLOOR_OFFSET_V3) === null
        ? `This Source material selects an explicitly unbounded principal policy and names no manipulation floor, read at finalized slot ${observation.slot}.`
        : `Read from the SourceMaterialV3 record at byte ${SOURCE_MATERIAL_MANIPULATION_FLOOR_OFFSET_V3}, at finalized slot ${observation.slot}.`,
      windowSpecRecord: `Read from the SourceMaterialV3 record at byte ${SOURCE_MATERIAL_WINDOW_SPEC_OFFSET_V3}, at finalized slot ${observation.slot}.`,
    }),
  });
}

export async function prepareCoreFoundV2(client: SolanaRpcClient, input: CoreFoundInputV2): Promise<CoreFoundPlanV2> {
  generationBytes(input.generation);
  key(input.payer, 'payer');
  const registry = key(input.registryProgram, 'Registry program');
  key(input.activationCache, 'activation cache');
  key(input.refundWallet, 'refund wallet');
  const rawAddresses = [input.realmRecord, input.productRecord, input.resultDomainRecord, input.portfolioRecord, input.linkedBasisRecord, input.sourceMaterialRecord, input.sourceSpecRecord, input.capacityProfileRecord, input.manipulationFloorRecord, input.capabilityManifestRecord];
  if (input.priceGateRecord !== undefined) rawAddresses.push(input.priceGateRecord);
  rawAddresses.forEach((address, index) => key(address, `finalized raw record ${index}`));
  if (new Set([input.registryProgram, input.activationCache, input.refundWallet, ...rawAddresses]).size !== 3 + rawAddresses.length) throw new Error('Found authority inputs alias named roles');

  const infrastructure = await inspectProtocolInfrastructureV1(client, { registryProgram: registry.toBase58(), activationCache: input.activationCache });
  const initial = await client.multipleAccounts(rawAddresses, infrastructure.observedSlot);
  const initialAccounts = accountMap(initial);
  const specs: Array<readonly [string, Uint8Array, string]> = [
    [input.realmRecord, REALM_SCHEMA_RELEASE_ID_V1, 'Realm'],
    [input.productRecord, PRODUCT_RECORD_SCHEMA_ID_V2, 'Product'],
    [input.resultDomainRecord, RESULT_DOMAIN_SCHEMA_ID_V2, 'result domain'],
    [input.portfolioRecord, PORTFOLIO_SCHEMA_ID_V2, 'portfolio'],
    [input.linkedBasisRecord, GRADED_BASIS_RECORD_SCHEMA_ID_V3, 'linked basis'],
    [input.sourceMaterialRecord, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3, 'Source material'],
    [input.sourceSpecRecord, SOURCE_SPEC_SCHEMA_ID_V1, 'Source spec'],
    [input.capacityProfileRecord, SOURCE_CAPACITY_PROFILE_SCHEMA_ID_V1, 'capacity profile'],
    [input.manipulationFloorRecord, MANIPULATION_FLOOR_SCHEMA_RELEASE_ID_V1, 'manipulation floor'],
    [input.capabilityManifestRecord, CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, 'capability manifest'],
  ];
  if (input.priceGateRecord !== undefined) specs.push([input.priceGateRecord, PRICE_GATE_RECORD_SCHEMA_ID_V1, 'price-gate certificate']);
  const authorities = await Promise.all(specs.map(([address, schema, field]) => recordAuthority(client, input.registryProgram, address, schema, required(initialAccounts, address, field), field)));
  const releaseSetDigest = Uint8Array.from(infrastructure.executionReleaseSetId.match(/../g) ?? [], (value) => Number.parseInt(value, 16));
  if (releaseSetDigest.length !== 32 || isZero(releaseSetDigest)) throw new Error('activation cache has an invalid release-set identity');

  const [realm, product, domain, portfolio, linkedBasis, source, sourceSpec, capacityProfile, manipulationFloor, manifest, priceGate] = authorities;
  validateRealm(realm.bytes);
  const graph = decodeCoreFoundProductGraphV2(product.bytes, domain.bytes, portfolio.bytes, domain.digest, portfolio.digest);
  validateCoreFoundSourceMaterialV3(source.bytes, product.digest);
  if (!same(slice(source.bytes, 48, 32), sourceSpec.digest)
      || !same(slice(sourceSpec.bytes, 144, 32), capacityProfile.digest)
      || !same(slice(source.bytes, 208, 32), manipulationFloor.digest)) throw new Error('SourceMaterialV3 graph identities differ from the authenticated records');
  validateCoreFoundCapabilityManifestV1(manifest.bytes);

  const market = PublicKey.findProgramAddressSync([
    MARKET_CORE_STATE_PDA_DOMAIN_V2,
    realm.digest,
    product.digest,
    graph.productId,
    source.digest,
    manifest.digest,
    releaseSetDigest,
    registry.toBytes(),
    generationBytes(input.generation),
  ], key(infrastructure.core.program, 'Core program'))[0];
  const [rentCredit] = PublicKey.findProgramAddressSync([
    LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2,
    market.toBytes(),
    generationBytes(input.generation),
  ], key(infrastructure.rent.program, 'Rent program'));
  const registryArtifact = deriveFinalizedRecordAddressesV1(input.registryProgram, ARTIFACT_RELEASE_SCHEMA_ID_V1, Uint8Array.from(infrastructure.registry.artifactReleaseId.match(/../g) ?? [], (value) => Number.parseInt(value, 16)));
  const rentArtifact = deriveFinalizedRecordAddressesV1(input.registryProgram, ARTIFACT_RELEASE_SCHEMA_ID_V1, Uint8Array.from(infrastructure.rent.artifactReleaseId.match(/../g) ?? [], (value) => Number.parseInt(value, 16)));
  const accountAddresses = [
    input.payer, market.toBase58(), rentCredit.toBase58(), infrastructure.rent.program,
    realm.raw, realm.staging, product.raw, product.staging, domain.raw, domain.staging, portfolio.raw, portfolio.staging,
    linkedBasis.raw, linkedBasis.staging, source.raw, source.staging, sourceSpec.raw, sourceSpec.staging,
    capacityProfile.raw, capacityProfile.staging, manipulationFloor.raw, manipulationFloor.staging,
    manifest.raw, manifest.staging, input.activationCache, infrastructure.core.program, infrastructure.core.programData, input.registryProgram, RENT_SYSVAR_ID, SYSTEM_PROGRAM_ID,
    infrastructure.profilePda, registryArtifact.record, registryArtifact.staging, infrastructure.registry.programData,
    rentArtifact.record, rentArtifact.staging, infrastructure.rent.programData,
  ];
  if (priceGate !== undefined) accountAddresses.push(priceGate.raw, priceGate.staging);
  const foundAccountCount = priceGate === undefined ? CORE_FOUND_ACCOUNT_COUNT_V3 : CORE_FOUND_PRICE_GATE_ACCOUNT_COUNT_V3;
  const foundRoles = priceGate === undefined ? CORE_FOUND_ACCOUNT_ROLES_V3 : CORE_FOUND_PRICE_GATE_ACCOUNT_ROLES_V3;
  if (accountAddresses.length !== foundAccountCount || new Set(accountAddresses).size !== foundAccountCount) throw new Error(`Found${foundAccountCount} account projection aliases named roles`);
  const observationAddresses = input.refundWallet === input.payer ? accountAddresses : [...accountAddresses, input.refundWallet];
  const finalObservation = await acquireFinalizedAccountsInChunksV1(client, observationAddresses, initial.slot);
  const finalAccounts = accountMap(finalObservation);
  const payerAccount = required(finalAccounts, input.payer, 'payer');
  if (payerAccount.owner !== SYSTEM_PROGRAM_ID || payerAccount.executable || payerAccount.data.length !== 0) throw new Error('payer is not a System-owned data-free wallet');
  vacant(finalAccounts.get(market.toBase58()), 'Market destination');
  const refundWallet = required(finalAccounts, input.refundWallet, 'refund wallet');
  if (refundWallet.owner !== SYSTEM_PROGRAM_ID || refundWallet.executable || refundWallet.data.length !== 0) throw new Error('refund wallet is not a System-owned data-free wallet');
  // The credit is a PRECONDITION of Found and the OUTPUT of its own
  // transaction, and those are different requirements. Demanding vacancy in
  // both places is what made a two-stage flow impossible: the moment the credit
  // landed, re-preparing Found against the same coordinates refused. So the
  // observation branches on what is actually there, and each branch states the
  // whole of its own requirement.
  const creditDestination = finalAccounts.get(rentCredit.toBase58()) ?? null;
  const creditExists = creditDestination !== null && creditDestination.owner === infrastructure.rent.program;
  if (creditExists) {
    if (creditDestination.executable
        || creditDestination.data.length !== LIFECYCLE_RENT_CREDIT_BYTES_V2
        || !same(slice(creditDestination.data, LIFECYCLE_RENT_CREDIT_MAGIC_OFFSET_V2, LIFECYCLE_RENT_CREDIT_MAGIC_V2.length), LIFECYCLE_RENT_CREDIT_MAGIC_V2)) {
      throw new Error('the existing lifecycle RentCredit is not a canonical Rent-owned credit account');
    }
  } else {
    vacant(creditDestination, 'lifecycle RentCredit destination');
    if ((creditDestination?.lamports ?? '0') !== '0') throw new Error('lifecycle RentCredit destination is prefunded');
  }
  for (const authority of authorities) {
    const raw = required(finalAccounts, authority.raw, 'finalized raw record');
    if (raw.owner !== input.registryProgram || raw.executable || BigInt(raw.lamports) < authority.rentMinimum || !same(raw.data, authority.bytes) || !same(await sha256(raw.data), authority.digest)) throw new Error('finalized raw record changed or lost Registry/rent authority');
    vacant(finalAccounts.get(authority.staging), 'finalized staging cursor');
  }
  const rentSysvar = required(finalAccounts, RENT_SYSVAR_ID, 'Rent sysvar');
  const system = required(finalAccounts, SYSTEM_PROGRAM_ID, 'System Program');
  // A real Agave observation of the System Program carries the 14-byte
  // NativeLoader metadata body `system_program`; requiring emptiness refuses
  // every read of a live cluster (measured 2026-08-27, same defect the Rust
  // operators fixed in `770610c` / `c25de27`).
  if (rentSysvar.owner !== SYSVAR_OWNER_ID || rentSysvar.executable || rentSysvar.data.length !== 17 || system.owner !== NATIVE_LOADER_ID || !system.executable) throw new Error('Rent or System runtime account is not canonical');
  const marketRent = await client.minimumBalanceForRentExemption(CORE_STATE_BYTES);
  const creditRent = await client.minimumBalanceForRentExemption(LIFECYCLE_RENT_CREDIT_BYTES_V2);
  const marketLamports = finalAccounts.get(market.toBase58())?.lamports ?? '0';
  const marketRentTopUp = BigInt(marketRent.lamports) > BigInt(marketLamports) ? BigInt(marketRent.lamports) - BigInt(marketLamports) : 0n;
  // A credit that already exists is already paid for.
  const totalRentDebit = marketRentTopUp + (creditExists ? 0n : BigInt(creditRent.lamports));
  if (BigInt(payerAccount.lamports) < totalRentDebit) throw new Error('payer cannot cover the exact current Market and lifecycle-credit rent debit');
  const confirmedInfrastructure = await inspectProtocolInfrastructureV1(client, { registryProgram: input.registryProgram, activationCache: input.activationCache });
  if (!infrastructureEqual(infrastructure, confirmedInfrastructure)) throw new Error('immutable infrastructure projection changed during Found construction');
  const blockhash = await client.latestMutationBlockhash(confirmedInfrastructure.observedSlot);
  const rentCreation = creditExists ? null : compileLifecycleRentCreateTransactionV2({
    payer: input.payer,
    refundWallet: input.refundWallet,
    market: market.toBase58(),
    releaseSet: releaseSetDigest,
    generation: input.generation,
    rentProgram: infrastructure.rent.program,
    recentBlockhash: blockhash.blockhash,
  });
  if (rentCreation !== null && rentCreation.rentCredit !== rentCredit.toBase58()) throw new Error('lifecycle RentCredit derivation changed during compilation');
  // The frame's routable set is derived whether or not it can be compiled,
  // because a caller who has just learned it needs a table needs to know what
  // to put in one. Signers never route: a lookup table cannot carry one.
  const foundInstruction = new TransactionInstruction({
    programId: key(infrastructure.core.program, 'Core program'),
    keys: accountAddresses.map((address, index) => ({
      pubkey: key(address, `Found account ${index}`),
      isSigner: foundRoles[index].signer,
      isWritable: foundRoles[index].writable,
    })),
    data: foundRequest(input.generation, market) as Buffer,
  });
  const routableAddresses = routableAddressesV1([foundInstruction], input.payer);
  let compiled: CompiledCoreFoundTransactionV2 | null = null;
  let foundRefusal: string | null = null;
  try {
    compiled = compileCoreFoundTransactionV2({
      payer: input.payer,
      coreProgram: infrastructure.core.program,
      market: market.toBase58(),
      generation: input.generation,
      recentBlockhash: blockhash.blockhash,
      accountAddresses,
      lookupTable: input.lookupTable,
    });
  } catch (error) {
    // Only a packet-bound refusal is recoverable into a plan. Anything else --
    // an aliased role, a misplaced payer, an unexpected signer -- is a defect
    // in the projection and must not be softened into a status line.
    const message = error instanceof Error ? error.message : String(error);
    if (!message.includes('packet bound')) throw error;
    foundRefusal = message;
  }
  return Object.freeze({
    observedSlot: finalObservation.slot,
    market: market.toBase58(),
    rentCredit: rentCredit.toBase58(),
    coreProgram: infrastructure.core.program,
    registryProgram: input.registryProgram,
    rentProgram: infrastructure.rent.program,
    productRecordDigest: hex(product.digest),
    priceGateRecordDigest: priceGate === undefined ? null : hex(priceGate.digest),
    productId: hex(graph.productId),
    outcomeCount: graph.outcomeCount,
    executionReleaseSetId: infrastructure.executionReleaseSetId,
    infrastructureProfile: infrastructure.profilePda,
    infrastructureRecognition: infrastructure.recognition,
    marketRentTopUp: marketRentTopUp.toString(),
    rentCreditRentDebit: creditRent.lamports,
    lastValidBlockHeight: blockhash.lastValidBlockHeight,
    accountAddresses: Object.freeze(accountAddresses),
    routableAddresses,
    requiredSigners: compiled?.requiredSigners ?? Object.freeze([input.payer]),
    requestBytes: compiled?.requestBytes ?? foundRequest(input.generation, market),
    rentCreditState: creditExists ? 'created' : 'vacant',
    rentCreateRequestBytes: rentCreation?.requestBytes ?? null,
    rentCreateTransaction: rentCreation?.transaction ?? null,
    rentCreateWireBytes: rentCreation?.wireBytes ?? null,
    transaction: compiled?.transaction ?? null,
    wireBytes: compiled?.wireBytes ?? null,
    foundRefusal,
  });
}
