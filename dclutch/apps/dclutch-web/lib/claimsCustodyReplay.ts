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
  CUSTODY_REPLAY_CONTEXT_OFFSET_V1,
  CUSTODY_REPLAY_GENERATION_OFFSET_V1,
  CUSTODY_REPLAY_MAGIC_V1,
  CUSTODY_REPLAY_MARKET_OFFSET_V1,
  CUSTODY_REPLAY_NEXT_REVISION_OFFSET_V1,
  CUSTODY_REPLAY_PDA_DOMAIN_V1,
  CUSTODY_REPLAY_RELEASE_SET_OFFSET_V1,
  CUSTODY_REPLAY_RENT_REFUND_OFFSET_V1,
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
  REPLAY_ACCOUNT_REALM_STAGING_V1,
  REPLAY_ACCOUNT_REALM_V1,
  REPLAY_ACCOUNT_REGISTRY_PROGRAM_V1,
  REPLAY_ACCOUNT_RENT_SYSVAR_V1,
  REPLAY_ACCOUNT_SYSTEM_PROGRAM_V1,
} from './generated/claimsCustodyReplayV1';
import { REALM_SCHEMA_RELEASE_ID_V1 } from './generated/coreFound';
import {
  decodeClaimsAggregateV2,
  deriveClaimsAggregateAddressV2,
  type ClaimsAggregateV2,
} from './marketCoreV2';
import { SYSTEM_PROGRAM_ID, UPGRADEABLE_LOADER_ID, deriveFinalizedRecordAddressesV1 } from './releaseRegistry';
import { type SolanaRpcClient } from './rpc';

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
  output.set(input.payer, CUSTODY_REQUEST_RENT_REFUND_OFFSET_V1);
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
  client: Pick<SolanaRpcClient, 'finalizedSlot' | 'multipleAccounts' | 'minimumBalanceForRentExemption' | 'latestBlockhash'>,
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
    const observation = await client.multipleAccounts([aggregateAddress], floor);
    const aggregateAccount = observation.accounts[0]?.account ?? null;
    if (aggregateAccount === null) {
      return Object.freeze({ status: 'refused', reason: `no Claims aggregate exists at ${aggregateAddress}, the address this Market derives under the selected Claims program — without the aggregate there is no persisted Custody namespace to open a replay in` });
    }
    if (aggregateAccount.owner !== claimsProgram.toBase58() || aggregateAccount.executable) {
      return Object.freeze({ status: 'refused', reason: `the derived aggregate address holds an account the selected Claims program does not own (owner ${aggregateAccount.owner})` });
    }
    const aggregate = decodeClaimsAggregateV2(aggregateAddress, aggregateAccount.data);
    if (aggregate.logicalMarket !== marketAddress) {
      return Object.freeze({ status: 'refused', reason: `the aggregate at the derived address names logical Market ${aggregate.logicalMarket}, not ${marketAddress}` });
    }

    const releaseSet = fromHex(aggregate.selectedReleaseSetId, 'aggregate release set');
    const context = fromHex(aggregate.custodyContext, 'aggregate custody context');
    const realmId = fromHex(aggregate.realmId, 'aggregate Realm identity');
    if (isZero(context)) {
      return Object.freeze({ status: 'refused', reason: 'the aggregate persists a zero Custody namespace; no replay can be derived from it' });
    }

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
    if (replayAccount !== null && replayAccount.data.length > 0) {
      if (replayAccount.owner !== custodyProgram.toBase58()
        || replayAccount.data.length !== CUSTODY_REPLAY_BYTES_V1
        || !same(replayAccount.data.slice(0, 8), CUSTODY_REPLAY_MAGIC_V1)
        || u16At(replayAccount.data, CUSTODY_REPLAY_VERSION_OFFSET_V1) !== CUSTODY_ABI_VERSION_V1
        || replayAccount.data[CUSTODY_REPLAY_CALLER_ROLE_OFFSET_V1] !== EXECUTION_ROLE_CLAIMS_V1
        || !same(replayAccount.data.slice(CUSTODY_REPLAY_MARKET_OFFSET_V1, CUSTODY_REPLAY_MARKET_OFFSET_V1 + 32), marketBytes)
        || !same(replayAccount.data.slice(CUSTODY_REPLAY_RELEASE_SET_OFFSET_V1, CUSTODY_REPLAY_RELEASE_SET_OFFSET_V1 + 32), releaseSet)
        || !same(replayAccount.data.slice(CUSTODY_REPLAY_CONTEXT_OFFSET_V1, CUSTODY_REPLAY_CONTEXT_OFFSET_V1 + 32), context)) {
        return Object.freeze({ status: 'refused', reason: `an account exists at the derived Claims-role replay address ${replayAddress} but does not decode as this Market's Claims-role Custody replay` });
      }
      return Object.freeze({
        status: 'exists',
        replayAddress,
        nextRevision: u64At(replayAccount.data, CUSTODY_REPLAY_NEXT_REVISION_OFFSET_V1).toString(),
        generation: u64At(replayAccount.data, CUSTODY_REPLAY_GENERATION_OFFSET_V1).toString(),
        rentRefund: new PublicKey(replayAccount.data.slice(CUSTODY_REPLAY_RENT_REFUND_OFFSET_V1, CUSTODY_REPLAY_RENT_REFUND_OFFSET_V1 + 32)).toBase58(),
        note: 'The Claims-role Custody replay already exists, so a redemption plan replays against it directly; no creation is owed.',
      });
    }

    const rent = await client.minimumBalanceForRentExemption(CUSTODY_REPLAY_BYTES_V1);
    const custodyRequestBytes = await encodeExpectedCustodyRequestV1({
      releaseSet,
      market: marketBytes,
      realm: realmId,
      context,
      claimsProgram: claimsProgram.toBytes(),
      payer: payer.toBytes(),
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
    keys[REPLAY_ACCOUNT_CUSTODY_PROGRAM_V1] = { pubkey: custodyProgram, isSigner: false, isWritable: false };
    keys[REPLAY_ACCOUNT_AGGREGATE_V1] = { pubkey: new PublicKey(aggregateAddress), isSigner: false, isWritable: false };

    const instructionData = encodeClaimsCustodyReplayRequestV1(marketAddress);
    const instruction = new TransactionInstruction({ programId: claimsProgram, keys, data: instructionData as Buffer });
    const budget = ComputeBudgetProgram.setComputeUnitLimit({ units: CLAIMS_CUSTODY_REPLAY_COMPUTE_UNIT_LIMIT_V1 });
    const latest = await client.latestBlockhash(floor);
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
      note: `No Claims-role replay exists at ${replayAddress}. One ${wireBytes.length}-byte legacy transaction creates it from prepaid rent (${rent.lamports} lamports), payable and refundable to the connected wallet.`,
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

/**
 * Why the payout leg itself is not yet wallet-executable, stated as the named
 * chain facts rather than as hedging. `claims/terminal_settlement_v3::process`
 * decodes caller_role Core(0) or Trading(2) only, and no campaign drives it
 * (docs/reference/routes.md; ADR-0008 §7.6), so a plain LiabilityBasisV2
 * Position has no wallet-direct payout route. The Rational representation's
 * RedeemTerminal is on-chain-proven, but it redeems Token-2022 shard
 * representations, which a plain Position does not hold.
 */
export const PLAIN_POSITION_PAYOUT_BLOCK_V1 = 'Your winning balance and its exact payout are real, on-chain numbers. What is missing is the instruction that pays a plain position out to a wallet: the chain only accepts that move from two of its own internal programs today (ADR-0008 \u00a77.6). The cursor this flow creates is step one, done and waiting; the payout instruction is the missing piece, and shipping it is protocol work \u2014 not something your wallet or this page can route around.';
