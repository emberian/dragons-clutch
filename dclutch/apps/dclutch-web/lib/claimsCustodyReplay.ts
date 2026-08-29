import {
  ComputeBudgetProgram,
  PublicKey,
  SYSVAR_RENT_PUBKEY,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
} from '@solana/web3.js';

import { fromHex, hex, isZero, sha256 } from './bytes';
import {
  CALLER_AUTHORITY_PDA_DOMAIN_V1,
  CLAIMS_CUSTODY_REPLAY_ACCOUNT_COUNT_V1,
  CLAIMS_CUSTODY_REPLAY_MARKET_OFFSET_V1,
  CLAIMS_CUSTODY_REPLAY_PARENT_DOMAIN_V1,
  CLAIMS_CUSTODY_REPLAY_REQUEST_BYTES_V1,
  CLAIMS_CUSTODY_REPLAY_REQUEST_MAGIC_V1,
  CLAIMS_CUSTODY_REPLAY_VERSION_OFFSET_V1,
  CLAIMS_CUSTODY_REPLAY_VERSION_V1,
  CUSTODY_ABI_VERSION_V1,
  CUSTODY_COMPARTMENT_NONE_V1,
  CUSTODY_OPERATION_INITIALIZE_REPLAY_V1,
  CUSTODY_REPLAY_BYTES_V1,
  CUSTODY_REPLAY_CALLER_ROLE_OFFSET_V1,
  CUSTODY_REPLAY_CALLER_PROGRAM_OFFSET_V1,
  CUSTODY_REPLAY_CONTEXT_OFFSET_V1,
  CUSTODY_REPLAY_GENERATION_OFFSET_V1,
  CUSTODY_REPLAY_MAGIC_V1,
  CUSTODY_REPLAY_MARKET_OFFSET_V1,
  CUSTODY_REPLAY_NEXT_REVISION_OFFSET_V1,
  CUSTODY_REPLAY_OPEN_VAULT_COUNT_OFFSET_V1,
  CUSTODY_REPLAY_PDA_DOMAIN_V1,
  CUSTODY_REPLAY_REALM_OFFSET_V1,
  CUSTODY_REPLAY_RELEASE_SET_OFFSET_V1,
  CUSTODY_REPLAY_RENT_REFUND_OFFSET_V1,
  CUSTODY_REPLAY_STATUS_OFFSET_V1,
  CUSTODY_REPLAY_VERSION_OFFSET_V1,
  CUSTODY_REQUEST_BYTES_V1,
  CUSTODY_REQUEST_CALLER_PROGRAM_OFFSET_V1,
  CUSTODY_REQUEST_CALLER_ROLE_OFFSET_V1,
  CUSTODY_REQUEST_CONTEXT_OFFSET_V1,
  CUSTODY_REQUEST_DESTINATION_COMPARTMENT_OFFSET_V1,
  CUSTODY_REQUEST_EXPECTED_REVISION_OFFSET_V1,
  CUSTODY_REQUEST_GENERATION_OFFSET_V1,
  CUSTODY_REQUEST_MAGIC_V1,
  CUSTODY_REQUEST_MARKET_OFFSET_V1,
  CUSTODY_REQUEST_OPERATION_OFFSET_V1,
  CUSTODY_REQUEST_PARENT_REQUEST_DIGEST_OFFSET_V1,
  CUSTODY_REQUEST_PAYER_OFFSET_V1,
  CUSTODY_REQUEST_REALM_OFFSET_V1,
  CUSTODY_REQUEST_RELEASE_SET_OFFSET_V1,
  CUSTODY_REQUEST_RENT_LAMPORTS_OFFSET_V1,
  CUSTODY_REQUEST_RENT_REFUND_OFFSET_V1,
  CUSTODY_REQUEST_RESULTING_REVISION_OFFSET_V1,
  CUSTODY_REQUEST_SOURCE_COMPARTMENT_OFFSET_V1,
  CUSTODY_REQUEST_VERSION_OFFSET_V1,
  EXECUTION_ROLE_CLAIMS_V1,
  REGISTRY_ACTIVATION_PDA_DOMAIN_V1,
  REPLAY_ACCOUNT_ACTIVATION_CACHE_V1,
  REPLAY_ACCOUNT_AGGREGATE_V1,
  REPLAY_ACCOUNT_CLAIMS_PROGRAMDATA_V1,
  REPLAY_ACCOUNT_CLAIMS_PROGRAM_V1,
  REPLAY_ACCOUNT_CORE_MARKET_V1,
  REPLAY_ACCOUNT_CUSTODY_CALLER_AUTHORITY_V1,
  REPLAY_ACCOUNT_CUSTODY_PROGRAM_V1,
  REPLAY_ACCOUNT_CUSTODY_REPLAY_V1,
  REPLAY_ACCOUNT_PAYER_V1,
  REPLAY_ACCOUNT_RENT_REFUND_V1,
  REPLAY_ACCOUNT_REALM_STAGING_V1,
  REPLAY_ACCOUNT_REALM_V1,
  REPLAY_ACCOUNT_REGISTRY_PROGRAM_V1,
  REPLAY_ACCOUNT_RENT_SYSVAR_V1,
  REPLAY_ACCOUNT_SYSTEM_PROGRAM_V1,
} from './generated/claimsCustodyReplayV1';
import { REALM_SCHEMA_RELEASE_ID_V1 } from './generated/coreFound';
import {
  decodeMarketCoreStateV2,
  decodeClaimsAggregateV2,
  deriveClaimsAggregateAddressV2,
  type ClaimsAggregateV2,
} from './marketCoreV2';
import { SYSTEM_PROGRAM_ID, UPGRADEABLE_LOADER_ID, deriveFinalizedRecordAddressesV1 } from './releaseRegistry';
import { type RpcAccount, type SolanaRpcClient } from './rpc';

/**
 * The wallet-side redemption precondition, built exactly once.
 *
 * ADR-0008 §7.3: "a terminal-settlement plan opens with the replay-creation
 * instruction when the Claims-role replay is absent, and the same is true of
 * any redemption builder, including the browser's." This module is that
 * builder. It mirrors `expected_request_v1` — the SINGLE Rust author of every
 * Custody coordinate this route forwards — byte for byte, because the Claims
 * caller-authority PDA is seeded by the SHA-256 of that exact 672-byte request:
 * a builder that derived even one field differently would derive a different
 * authority address and the route would refuse the frame.
 *
 * Everything here is derived, never asked for: the namespace from the
 * aggregate's persisted `custody_context`, the role from the route itself,
 * the rent from the live Rent minimum, the payer from the wallet that signs.
 * The 48-byte instruction carries the Market and nothing else. The
 * transaction is deliberately LEGACY: creating this cursor stands ahead of a
 * redemption and must never depend on a published address-lookup table.
 */

const SOLANA_PACKET_BYTES = 1_232;
const U64_MAX = 0xffff_ffff_ffff_ffffn;

// These two fields close the 288-byte Rust replay layout. They are derived
// from the generated preceding coordinate so this hand-written consumer does
// not invent a second offset authority.
const CUSTODY_REPLAY_LAST_REQUEST_DIGEST_OFFSET_V1 = CUSTODY_REPLAY_GENERATION_OFFSET_V1 + 8;
const CUSTODY_REPLAY_LAST_POSTSTATE_COMMITMENT_OFFSET_V1 = CUSTODY_REPLAY_LAST_REQUEST_DIGEST_OFFSET_V1 + 32;

/**
 * An explicit ceiling far above the measured cost of the whole Custody-CPI
 * family (~130k–160k CU per `docs/reference/budgets.md`), stated because the
 * runtime's default per-instruction budget is not a number this route pinned.
 */
export const CLAIMS_CUSTODY_REPLAY_COMPUTE_UNIT_LIMIT_V1 = 500_000;

function exactKey(value: string, field: string): PublicKey {
  const parsed = new PublicKey(value);
  if (parsed.toBase58() !== value) throw new Error(`${field} must be canonical base58 text`);
  return parsed;
}

function putU16(bytes: Uint8Array, offset: number, value: number): void {
  new DataView(bytes.buffer, bytes.byteOffset + offset, 2).setUint16(0, value, true);
}

function putU64(bytes: Uint8Array, offset: number, value: bigint): void {
  if (value < 0n || value > 0xffff_ffff_ffff_ffffn) throw new Error('u64 field is outside its exact width');
  new DataView(bytes.buffer, bytes.byteOffset + offset, 8).setBigUint64(0, value, true);
}

function u64At(bytes: Uint8Array, offset: number): bigint {
  return new DataView(bytes.buffer, bytes.byteOffset + offset, 8).getBigUint64(0, true);
}

function u16At(bytes: Uint8Array, offset: number): number {
  return new DataView(bytes.buffer, bytes.byteOffset + offset, 2).getUint16(0, true);
}

function concat(...parts: ReadonlyArray<Uint8Array>): Uint8Array {
  const output = new Uint8Array(parts.reduce((total, part) => total + part.length, 0));
  let offset = 0;
  for (const part of parts) { output.set(part, offset); offset += part.length; }
  return output;
}

function same(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function accountLamports(account: RpcAccount, field: string): bigint {
  if (!/^(0|[1-9][0-9]*)$/.test(account.lamports)) throw new Error(`${field} lamports are not canonical unsigned decimal text`);
  const value = BigInt(account.lamports);
  if (value > U64_MAX) throw new Error(`${field} lamports exceed u64`);
  return value;
}

function requireMaterialAccount(account: RpcAccount, field: string): void {
  if (account.executable) throw new Error(`${field} is executable`);
  if (account.space !== account.data.length) throw new Error(`${field} RPC space does not equal its acquired data length`);
  if (accountLamports(account, field) === 0n) throw new Error(`${field} has zero lamports`);
}

function requireCanonicalAggregateBytes(account: RpcAccount): void {
  // LiabilityBasisMarketViewV2::decode requires these two reserved header
  // bytes to be zero. decodeClaimsAggregateV2 historically only decoded them.
  if (account.data[10] !== 0 || account.data[11] !== 0) throw new Error('Claims aggregate reserved bytes are nonzero');
}

/** Encode the exact 48-byte `DCLCCR01` request naming one Market. */
export function encodeClaimsCustodyReplayRequestV1(market: string): Uint8Array {
  const marketBytes = exactKey(market, 'Market').toBytes();
  if (isZero(marketBytes)) throw new Error('the named Market identity is zero');
  const output = new Uint8Array(CLAIMS_CUSTODY_REPLAY_REQUEST_BYTES_V1);
  output.set(CLAIMS_CUSTODY_REPLAY_REQUEST_MAGIC_V1, 0);
  putU16(output, CLAIMS_CUSTODY_REPLAY_VERSION_OFFSET_V1, CLAIMS_CUSTODY_REPLAY_VERSION_V1);
  output.set(marketBytes, CLAIMS_CUSTODY_REPLAY_MARKET_OFFSET_V1);
  return output;
}

export type ClaimsCustodyRequestInputV1 = Readonly<{
  releaseSet: Uint8Array;
  market: Uint8Array;
  realm: Uint8Array;
  context: Uint8Array;
  claimsProgram: Uint8Array;
  payer: Uint8Array;
  rentRefund: Uint8Array;
  generation: bigint;
  rentLamports: bigint;
}>;

/**
 * Mirror of the Rust route's `expected_request_v1` + `CustodyRequestV1::to_bytes`.
 *
 * Every InitializeReplay shape rule the on-chain `validate` enforces is
 * enforced here first, so a plan that could never execute refuses in the
 * browser with the same meaning.
 */
export async function encodeExpectedCustodyRequestV1(input: ClaimsCustodyRequestInputV1): Promise<Uint8Array> {
  for (const [value, field] of [
    [input.releaseSet, 'release set'], [input.market, 'market'], [input.realm, 'realm'],
    [input.context, 'custody context'], [input.claimsProgram, 'Claims program'], [input.payer, 'payer'],
    [input.rentRefund, 'RentCredit refund beneficiary'],
  ] as const) {
    if (value.length !== 32 || isZero(value)) throw new Error(`Custody request ${field} must be one nonzero 32-byte identity`);
  }
  if (input.rentLamports === 0n) throw new Error('InitializeReplay rent must be the exact nonzero Rent minimum');
  const rentLe = new Uint8Array(8);
  putU64(rentLe, 0, input.rentLamports);
  const parentRequestDigest = await sha256(concat(
    CLAIMS_CUSTODY_REPLAY_PARENT_DOMAIN_V1,
    input.market,
    input.releaseSet,
    input.context,
    input.payer,
    input.rentRefund,
    rentLe,
  ));
  const output = new Uint8Array(CUSTODY_REQUEST_BYTES_V1);
  output.set(CUSTODY_REQUEST_MAGIC_V1, 0);
  putU16(output, CUSTODY_REQUEST_VERSION_OFFSET_V1, CUSTODY_ABI_VERSION_V1);
  output[CUSTODY_REQUEST_OPERATION_OFFSET_V1] = CUSTODY_OPERATION_INITIALIZE_REPLAY_V1;
  output[CUSTODY_REQUEST_CALLER_ROLE_OFFSET_V1] = EXECUTION_ROLE_CLAIMS_V1;
  output[CUSTODY_REQUEST_SOURCE_COMPARTMENT_OFFSET_V1] = CUSTODY_COMPARTMENT_NONE_V1;
  output[CUSTODY_REQUEST_DESTINATION_COMPARTMENT_OFFSET_V1] = CUSTODY_COMPARTMENT_NONE_V1;
  output.set(input.releaseSet, CUSTODY_REQUEST_RELEASE_SET_OFFSET_V1);
  output.set(input.market, CUSTODY_REQUEST_MARKET_OFFSET_V1);
  output.set(input.realm, CUSTODY_REQUEST_REALM_OFFSET_V1);
  output.set(input.context, CUSTODY_REQUEST_CONTEXT_OFFSET_V1);
  output.set(input.claimsProgram, CUSTODY_REQUEST_CALLER_PROGRAM_OFFSET_V1);
  output.set(parentRequestDigest, CUSTODY_REQUEST_PARENT_REQUEST_DIGEST_OFFSET_V1);
  output.set(input.payer, CUSTODY_REQUEST_PAYER_OFFSET_V1);
  output.set(input.rentRefund, CUSTODY_REQUEST_RENT_REFUND_OFFSET_V1);
  putU64(output, CUSTODY_REQUEST_EXPECTED_REVISION_OFFSET_V1, 0n);
  putU64(output, CUSTODY_REQUEST_RESULTING_REVISION_OFFSET_V1, 1n);
  putU64(output, CUSTODY_REQUEST_GENERATION_OFFSET_V1, input.generation);
  putU64(output, CUSTODY_REQUEST_RENT_LAMPORTS_OFFSET_V1, input.rentLamports);
  return output;
}

export type ClaimsCustodyReplayPlanV1 = Readonly<{
  marketAddress: string;
  aggregateAddress: string;
  aggregate: ClaimsAggregateV2;
  replayAddress: string;
  callerAuthorityAddress: string;
  activationCacheAddress: string;
  claimsProgramDataAddress: string;
  realmRecordAddress: string;
  realmStagingAddress: string;
  rentRefundAddress: string;
  payer: string;
  rentLamports: string;
  custodyRequestBytes: Uint8Array;
  custodyRequestDigestHex: string;
  instructionData: Uint8Array;
  transaction: VersionedTransaction;
  wireBytes: Uint8Array;
  requiredSigners: ReadonlyArray<string>;
}>;

export type ClaimsCustodyReplayStateV1 =
  | Readonly<{
    status: 'exists';
    replayAddress: string;
    nextRevision: string;
    generation: string;
    rentRefund: string;
    note: string;
  }>
  | Readonly<{ status: 'creatable'; plan: ClaimsCustodyReplayPlanV1; note: string }>
  | Readonly<{ status: 'refused'; reason: string }>;

export type ClaimsCustodyReplayRequestV1 = Readonly<{
  marketAddress: string;
  claimsProgramId: string;
  custodyProgramId: string;
  registryProgramId: string;
  payer: string;
}>;

/**
 * Inspect the Claims-role Custody replay for one Market, and when it does not
 * exist, return the complete signable creation plan.
 */
export async function inspectClaimsCustodyReplayV1(
  client: Pick<SolanaRpcClient, 'finalizedSlot' | 'multipleAccounts' | 'minimumBalanceForRentExemption' | 'latestMutationBlockhash'>,
  request: ClaimsCustodyReplayRequestV1,
): Promise<ClaimsCustodyReplayStateV1> {
  try {
    const marketAddress = exactKey(request.marketAddress, 'Market').toBase58();
    const claimsProgram = exactKey(request.claimsProgramId, 'Claims program');
    const custodyProgram = exactKey(request.custodyProgramId, 'Custody program');
    const registryProgram = exactKey(request.registryProgramId, 'Registry program');
    const payer = exactKey(request.payer, 'payer');

    const aggregateAddress = deriveClaimsAggregateAddressV2(claimsProgram.toBase58(), marketAddress);
    const floor = await client.finalizedSlot();
    const observation = await client.multipleAccounts([aggregateAddress, marketAddress], floor);
    const aggregateAccount = observation.accounts[0]?.account ?? null;
    const marketAccount = observation.accounts[1]?.account ?? null;
    if (aggregateAccount === null) {
      return Object.freeze({ status: 'refused', reason: `no Claims aggregate exists at ${aggregateAddress}, the address this Market derives under the selected Claims program — without the aggregate there is no persisted Custody namespace to open a replay in` });
    }
    if (aggregateAccount.owner !== claimsProgram.toBase58() || aggregateAccount.executable) {
      return Object.freeze({ status: 'refused', reason: `the derived aggregate address holds an account the selected Claims program does not own (owner ${aggregateAccount.owner})` });
    }
    requireMaterialAccount(aggregateAccount, 'Claims aggregate');
    requireCanonicalAggregateBytes(aggregateAccount);
    const aggregate = decodeClaimsAggregateV2(aggregateAddress, aggregateAccount.data);
    if (aggregate.logicalMarket !== marketAddress) {
      return Object.freeze({ status: 'refused', reason: `the aggregate at the derived address names logical Market ${aggregate.logicalMarket}, not ${marketAddress}` });
    }
    if (aggregate.registryProgram !== registryProgram.toBase58()) {
      return Object.freeze({ status: 'refused', reason: `the aggregate selects Registry program ${aggregate.registryProgram}, not ${registryProgram.toBase58()}` });
    }

    const releaseSet = fromHex(aggregate.selectedReleaseSetId, 'aggregate release set');
    const context = fromHex(aggregate.custodyContext, 'aggregate custody context');
    const realmId = fromHex(aggregate.realmId, 'aggregate Realm identity');
    const productInstanceId = fromHex(aggregate.productInstanceId, 'aggregate Product instance');
    const liabilityBasisId = fromHex(aggregate.liabilityBasisId, 'aggregate LiabilityBasis');
    if ([releaseSet, realmId, context, productInstanceId, liabilityBasisId].some(isZero)
      || isZero(new PublicKey(aggregate.logicalMarket).toBytes())
      || isZero(new PublicKey(aggregate.registryProgram).toBytes())) {
      return Object.freeze({ status: 'refused', reason: 'the Claims aggregate contains a zero identity that the Rust aggregate codec refuses' });
    }
    if (marketAccount === null) return Object.freeze({ status: 'refused', reason: 'the Core Market that selects the immutable RentCredit is absent' });
    requireMaterialAccount(marketAccount, 'Core Market');
    const market = decodeMarketCoreStateV2(marketAddress, marketAccount.data);
    if (market.marketId !== marketAddress
      || market.identity.selectedReleaseSetId !== aggregate.selectedReleaseSetId
      || market.identity.registryProgram !== registryProgram.toBase58()
      || market.identity.realmId !== aggregate.realmId
      || market.identity.generation !== aggregate.generation) {
      return Object.freeze({ status: 'refused', reason: 'the Core Market does not agree with the Claims aggregate on its immutable lifecycle identities' });
    }
    const rentRefund = exactKey(market.rentBeneficiary, 'Core Market RentCredit').toBytes();

    const marketBytes = new PublicKey(marketAddress).toBytes();
    const [replay] = PublicKey.findProgramAddressSync([
      CUSTODY_REPLAY_PDA_DOMAIN_V1,
      marketBytes,
      releaseSet,
      Uint8Array.of(EXECUTION_ROLE_CLAIMS_V1),
      context,
    ], custodyProgram);
    const replayAddress = replay.toBase58();

    const replayObservation = await client.multipleAccounts([replayAddress], floor);
    const replayAccount = replayObservation.accounts[0]?.account ?? null;
    if (replayAccount !== null && replayAccount.data.length === 0) {
      if (replayAccount.owner !== SYSTEM_PROGRAM_ID
        || replayAccount.executable
        || replayAccount.space !== 0
        || accountLamports(replayAccount, 'vacant replay account') !== 0n) {
        return Object.freeze({ status: 'refused', reason: `an occupied account exists at the derived Claims-role replay address ${replayAddress}; creation requires a System-owned, non-executable, zero-lamport, zero-space vacancy` });
      }
    } else if (replayAccount !== null) {
      requireMaterialAccount(replayAccount, 'Claims-role Custody replay');
      if (replayAccount.owner !== custodyProgram.toBase58()
        || replayAccount.data.length !== CUSTODY_REPLAY_BYTES_V1) {
        return Object.freeze({ status: 'refused', reason: `an account exists at the derived Claims-role replay address ${replayAddress} but does not decode as this Market's Claims-role Custody replay` });
      }
      const nextRevision = u64At(replayAccount.data, CUSTODY_REPLAY_NEXT_REVISION_OFFSET_V1);
      const generation = u64At(replayAccount.data, CUSTODY_REPLAY_GENERATION_OFFSET_V1);
      const rentRefund = replayAccount.data.slice(CUSTODY_REPLAY_RENT_REFUND_OFFSET_V1, CUSTODY_REPLAY_RENT_REFUND_OFFSET_V1 + 32);
      const lastRequestDigest = replayAccount.data.slice(CUSTODY_REPLAY_LAST_REQUEST_DIGEST_OFFSET_V1, CUSTODY_REPLAY_LAST_REQUEST_DIGEST_OFFSET_V1 + 32);
      const lastPoststateCommitment = replayAccount.data.slice(CUSTODY_REPLAY_LAST_POSTSTATE_COMMITMENT_OFFSET_V1, CUSTODY_REPLAY_BYTES_V1);
      if (!same(replayAccount.data.slice(0, 8), CUSTODY_REPLAY_MAGIC_V1)
        || u16At(replayAccount.data, CUSTODY_REPLAY_VERSION_OFFSET_V1) !== CUSTODY_ABI_VERSION_V1
        || replayAccount.data[CUSTODY_REPLAY_STATUS_OFFSET_V1] !== 1
        || replayAccount.data[CUSTODY_REPLAY_CALLER_ROLE_OFFSET_V1] !== EXECUTION_ROLE_CLAIMS_V1
        || !same(replayAccount.data.slice(CUSTODY_REPLAY_MARKET_OFFSET_V1, CUSTODY_REPLAY_MARKET_OFFSET_V1 + 32), marketBytes)
        || !same(replayAccount.data.slice(CUSTODY_REPLAY_RELEASE_SET_OFFSET_V1, CUSTODY_REPLAY_RELEASE_SET_OFFSET_V1 + 32), releaseSet)
        || !same(replayAccount.data.slice(CUSTODY_REPLAY_REALM_OFFSET_V1, CUSTODY_REPLAY_REALM_OFFSET_V1 + 32), realmId)
        || !same(replayAccount.data.slice(CUSTODY_REPLAY_CONTEXT_OFFSET_V1, CUSTODY_REPLAY_CONTEXT_OFFSET_V1 + 32), context)
        || !same(replayAccount.data.slice(CUSTODY_REPLAY_CALLER_PROGRAM_OFFSET_V1, CUSTODY_REPLAY_CALLER_PROGRAM_OFFSET_V1 + 32), claimsProgram.toBytes())
        || isZero(rentRefund)
        || nextRevision === 0n
        || generation !== BigInt(aggregate.generation)
        || isZero(lastRequestDigest)
        || isZero(lastPoststateCommitment)) {
        return Object.freeze({ status: 'refused', reason: `an account exists at the derived Claims-role replay address ${replayAddress} but does not decode as this Market's Claims-role Custody replay` });
      }
      // Reading the field is intentional even though every u32 bit pattern is
      // valid: it keeps this parser's byte coverage aligned with Rust's exact
      // replay layout instead of silently leaving the coordinate unchecked.
      new DataView(replayAccount.data.buffer, replayAccount.data.byteOffset + CUSTODY_REPLAY_OPEN_VAULT_COUNT_OFFSET_V1, 4).getUint32(0, true);
      return Object.freeze({
        status: 'exists',
        replayAddress,
        nextRevision: nextRevision.toString(),
        generation: generation.toString(),
        rentRefund: new PublicKey(rentRefund).toBase58(),
        note: 'The Claims-role Custody replay already exists, so a redemption plan replays against it directly; no creation is owed.',
      });
    }

    const rent = await client.minimumBalanceForRentExemption(CUSTODY_REPLAY_BYTES_V1);
    if (rent.dataLength !== CUSTODY_REPLAY_BYTES_V1) throw new Error('Rent response does not name the requested replay width');
    const custodyRequestBytes = await encodeExpectedCustodyRequestV1({
      releaseSet,
      market: marketBytes,
      realm: realmId,
      context,
      claimsProgram: claimsProgram.toBytes(),
      payer: payer.toBytes(),
      rentRefund,
      generation: BigInt(aggregate.generation),
      rentLamports: BigInt(rent.lamports),
    });
    const requestDigest = await sha256(custodyRequestBytes);
    const [callerAuthority] = PublicKey.findProgramAddressSync([
      CALLER_AUTHORITY_PDA_DOMAIN_V1,
      releaseSet,
      marketBytes,
      Uint8Array.of(EXECUTION_ROLE_CLAIMS_V1),
      context,
      requestDigest,
    ], claimsProgram);
    const [activationCache] = PublicKey.findProgramAddressSync([
      REGISTRY_ACTIVATION_PDA_DOMAIN_V1,
      releaseSet,
    ], registryProgram);
    const [claimsProgramData] = PublicKey.findProgramAddressSync([
      claimsProgram.toBytes(),
    ], new PublicKey(UPGRADEABLE_LOADER_ID));
    const realmRecord = deriveFinalizedRecordAddressesV1(registryProgram.toBase58(), REALM_SCHEMA_RELEASE_ID_V1, realmId);

    const keys = new Array<{ pubkey: PublicKey; isSigner: boolean; isWritable: boolean }>(CLAIMS_CUSTODY_REPLAY_ACCOUNT_COUNT_V1);
    keys[REPLAY_ACCOUNT_CUSTODY_CALLER_AUTHORITY_V1] = { pubkey: callerAuthority, isSigner: false, isWritable: false };
    keys[REPLAY_ACCOUNT_CORE_MARKET_V1] = { pubkey: new PublicKey(marketAddress), isSigner: false, isWritable: false };
    keys[REPLAY_ACCOUNT_ACTIVATION_CACHE_V1] = { pubkey: activationCache, isSigner: false, isWritable: false };
    keys[REPLAY_ACCOUNT_REGISTRY_PROGRAM_V1] = { pubkey: registryProgram, isSigner: false, isWritable: false };
    keys[REPLAY_ACCOUNT_CLAIMS_PROGRAM_V1] = { pubkey: claimsProgram, isSigner: false, isWritable: false };
    keys[REPLAY_ACCOUNT_CLAIMS_PROGRAMDATA_V1] = { pubkey: claimsProgramData, isSigner: false, isWritable: false };
    keys[REPLAY_ACCOUNT_REALM_V1] = { pubkey: new PublicKey(realmRecord.record), isSigner: false, isWritable: false };
    keys[REPLAY_ACCOUNT_REALM_STAGING_V1] = { pubkey: new PublicKey(realmRecord.staging), isSigner: false, isWritable: false };
    keys[REPLAY_ACCOUNT_CUSTODY_REPLAY_V1] = { pubkey: replay, isSigner: false, isWritable: true };
    keys[REPLAY_ACCOUNT_PAYER_V1] = { pubkey: payer, isSigner: true, isWritable: true };
    keys[REPLAY_ACCOUNT_SYSTEM_PROGRAM_V1] = { pubkey: new PublicKey(SYSTEM_PROGRAM_ID), isSigner: false, isWritable: false };
    keys[REPLAY_ACCOUNT_RENT_SYSVAR_V1] = { pubkey: SYSVAR_RENT_PUBKEY, isSigner: false, isWritable: false };
    keys[REPLAY_ACCOUNT_RENT_REFUND_V1] = { pubkey: new PublicKey(rentRefund), isSigner: false, isWritable: true };
    keys[REPLAY_ACCOUNT_CUSTODY_PROGRAM_V1] = { pubkey: custodyProgram, isSigner: false, isWritable: false };
    keys[REPLAY_ACCOUNT_AGGREGATE_V1] = { pubkey: new PublicKey(aggregateAddress), isSigner: false, isWritable: false };

    const instructionData = encodeClaimsCustodyReplayRequestV1(marketAddress);
    const instruction = new TransactionInstruction({ programId: claimsProgram, keys, data: instructionData as Buffer });
    const budget = ComputeBudgetProgram.setComputeUnitLimit({ units: CLAIMS_CUSTODY_REPLAY_COMPUTE_UNIT_LIMIT_V1 });
    const latest = await client.latestMutationBlockhash(floor);
    // Deliberately LEGACY: this instruction must never depend on a published
    // address-lookup table (ADR-0008 §7), and it fits with room to spare.
    const transaction = new VersionedTransaction(new TransactionMessage({
      payerKey: payer,
      recentBlockhash: latest.blockhash,
      instructions: [budget, instruction],
    }).compileToLegacyMessage());
    const wireBytes = transaction.serialize();
    if (wireBytes.length > SOLANA_PACKET_BYTES) {
      return Object.freeze({ status: 'refused', reason: `the replay-creation transaction is ${wireBytes.length} bytes, above the ${SOLANA_PACKET_BYTES}-byte legacy packet bound it is asserted to fit` });
    }
    const requiredSigners = Object.freeze(transaction.message.staticAccountKeys
      .slice(0, transaction.message.header.numRequiredSignatures)
      .map((signer) => signer.toBase58()));
    if (requiredSigners.length !== 1 || requiredSigners[0] !== payer.toBase58()) {
      return Object.freeze({ status: 'refused', reason: 'the replay-creation message requires an unexpected transaction signer' });
    }
    return Object.freeze({
      status: 'creatable',
      note: `No Claims-role replay exists at ${replayAddress}. One ${wireBytes.length}-byte legacy transaction creates it from prepaid rent (${rent.lamports} lamports); the immutable Core Market lifecycle RentCredit remains its refund beneficiary.`,
      plan: Object.freeze({
        marketAddress,
        aggregateAddress,
        aggregate,
        replayAddress,
        callerAuthorityAddress: callerAuthority.toBase58(),
        activationCacheAddress: activationCache.toBase58(),
        claimsProgramDataAddress: claimsProgramData.toBase58(),
        realmRecordAddress: realmRecord.record,
        realmStagingAddress: realmRecord.staging,
        rentRefundAddress: new PublicKey(rentRefund).toBase58(),
        payer: payer.toBase58(),
        rentLamports: rent.lamports,
        custodyRequestBytes,
        custodyRequestDigestHex: hex(requestDigest),
        instructionData,
        transaction,
        wireBytes,
        requiredSigners,
      }),
    });
  } catch (error) {
    return Object.freeze({ status: 'refused', reason: error instanceof Error ? error.message : 'the replay inspection refused without a usable reason' });
  }
}
