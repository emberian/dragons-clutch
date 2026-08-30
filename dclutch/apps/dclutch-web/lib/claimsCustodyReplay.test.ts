import { Keypair, PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import {
  CLAIMS_CUSTODY_REPLAY_COMPUTE_UNIT_LIMIT_V1,
  encodeClaimsCustodyReplayRequestV1,
  encodeExpectedCustodyRequestV1,
  inspectClaimsCustodyReplayV1,
} from './claimsCustodyReplay';
import {
  CLAIMS_CUSTODY_REPLAY_ACCOUNT_COUNT_V1,
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
  CUSTODY_REQUEST_CALLER_ROLE_OFFSET_V1,
  CUSTODY_REQUEST_MARKET_OFFSET_V1,
  CUSTODY_REQUEST_OPERATION_OFFSET_V1,
  CUSTODY_REQUEST_PARENT_REQUEST_DIGEST_OFFSET_V1,
  CUSTODY_REQUEST_PAYER_OFFSET_V1,
  CUSTODY_REQUEST_RENT_LAMPORTS_OFFSET_V1,
  CUSTODY_REQUEST_RENT_REFUND_OFFSET_V1,
  CUSTODY_REQUEST_RESERVED_OFFSET_V1,
  EXECUTION_ROLE_CLAIMS_V1,
  EXECUTION_ROLE_TRADING_V1,
  REPLAY_ACCOUNT_AGGREGATE_V1,
  REPLAY_ACCOUNT_CUSTODY_REPLAY_V1,
  REPLAY_ACCOUNT_PAYER_V1,
  REPLAY_ACCOUNT_RENT_REFUND_V1,
} from './generated/claimsCustodyReplayV1';
import {
  LIABILITY_BASIS_MARKET_CLAIM_COUNT_OFFSET,
  LIABILITY_BASIS_MARKET_CUSTODY_CONTEXT_OFFSET,
  LIABILITY_BASIS_MARKET_GENERATION_OFFSET,
  LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
  LIABILITY_BASIS_MARKET_LOGICAL_ID_OFFSET,
  LIABILITY_BASIS_MARKET_MAGIC_V2,
  LIABILITY_BASIS_MARKET_BASIS_OFFSET,
  LIABILITY_BASIS_MARKET_PRODUCT_OFFSET,
  LIABILITY_BASIS_MARKET_REALM_OFFSET,
  LIABILITY_BASIS_MARKET_REVISION_OFFSET,
  LIABILITY_BASIS_MARKET_REGISTRY_OFFSET,
  LIABILITY_BASIS_MARKET_RELEASE_SET_OFFSET,
  LIABILITY_BASIS_STATE_VERSION_V2,
  CORE_STATE_GENERATION_OFFSET,
  CORE_STATE_IDENTITY_REALM_OFFSET,
  CORE_STATE_MARKET_ID_OFFSET,
  CORE_STATE_REGISTRY_PROGRAM_OFFSET,
  CORE_STATE_RENT_BENEFICIARY_OFFSET,
  CORE_STATE_SELECTED_RELEASE_SET_OFFSET,
} from './generated/coreFound';
import { deriveClaimsAggregateAddressV2 } from './marketCoreV2';
import { SYSTEM_PROGRAM_ID } from './releaseRegistry';
import { type RpcAccount } from './rpc';
import { currentCoreMarketV3 } from '../fixtures/liveOpenMarket';

const CLAIMS = Keypair.fromSeed(new Uint8Array(32).fill(21)).publicKey.toBase58();
const CUSTODY = Keypair.fromSeed(new Uint8Array(32).fill(22)).publicKey.toBase58();
const REGISTRY = Keypair.fromSeed(new Uint8Array(32).fill(23)).publicKey.toBase58();
const MARKET = Keypair.fromSeed(new Uint8Array(32).fill(24)).publicKey.toBase58();
const PAYER = Keypair.fromSeed(new Uint8Array(32).fill(25)).publicKey.toBase58();
const RELEASE_SET = new Uint8Array(32).fill(31);
const CONTEXT = new Uint8Array(32).fill(32);
const REALM_ID = new Uint8Array(32).fill(33);
const PRODUCT_ID = new Uint8Array(32).fill(34);
const BASIS_ID = new Uint8Array(32).fill(35);
const RENT_REFUND = Keypair.fromSeed(new Uint8Array(32).fill(36)).publicKey.toBase58();
const BLOCKHASH = Keypair.fromSeed(new Uint8Array(32).fill(26)).publicKey.toBase58();
const CUSTODY_REPLAY_LAST_REQUEST_DIGEST_OFFSET_V1 = CUSTODY_REPLAY_GENERATION_OFFSET_V1 + 8;
const CUSTODY_REPLAY_LAST_POSTSTATE_COMMITMENT_OFFSET_V1 = CUSTODY_REPLAY_LAST_REQUEST_DIGEST_OFFSET_V1 + 32;

function aggregateBytes(overrides?: Partial<Readonly<{ logicalMarket: string }>>): Uint8Array {
  const claimCount = 4;
  const bytes = new Uint8Array(LIABILITY_BASIS_MARKET_HEADER_BYTES_V2 + claimCount * 8);
  bytes.set(LIABILITY_BASIS_MARKET_MAGIC_V2, 0);
  new DataView(bytes.buffer).setUint16(8, LIABILITY_BASIS_STATE_VERSION_V2, true);
  new DataView(bytes.buffer).setUint32(LIABILITY_BASIS_MARKET_CLAIM_COUNT_OFFSET, claimCount, true);
  new DataView(bytes.buffer).setBigUint64(LIABILITY_BASIS_MARKET_REVISION_OFFSET, 7n, true);
  bytes.set(new PublicKey(overrides?.logicalMarket ?? MARKET).toBytes(), LIABILITY_BASIS_MARKET_LOGICAL_ID_OFFSET);
  bytes.set(new PublicKey(REGISTRY).toBytes(), LIABILITY_BASIS_MARKET_REGISTRY_OFFSET);
  bytes.set(RELEASE_SET, LIABILITY_BASIS_MARKET_RELEASE_SET_OFFSET);
  bytes.set(PRODUCT_ID, LIABILITY_BASIS_MARKET_PRODUCT_OFFSET);
  bytes.set(BASIS_ID, LIABILITY_BASIS_MARKET_BASIS_OFFSET);
  bytes.set(REALM_ID, LIABILITY_BASIS_MARKET_REALM_OFFSET);
  bytes.set(CONTEXT, LIABILITY_BASIS_MARKET_CUSTODY_CONTEXT_OFFSET);
  new DataView(bytes.buffer).setBigUint64(LIABILITY_BASIS_MARKET_GENERATION_OFFSET, 2n, true);
  for (let index = 0; index < claimCount; index += 1) {
    new DataView(bytes.buffer).setBigUint64(LIABILITY_BASIS_MARKET_HEADER_BYTES_V2 + index * 8, 500n, true);
  }
  return bytes;
}

function coreMarketBytes(): Uint8Array {
  const bytes = currentCoreMarketV3();
  bytes.set(new PublicKey(MARKET).toBytes(), CORE_STATE_MARKET_ID_OFFSET);
  bytes.set(RELEASE_SET, CORE_STATE_SELECTED_RELEASE_SET_OFFSET);
  bytes.set(new PublicKey(REGISTRY).toBytes(), CORE_STATE_REGISTRY_PROGRAM_OFFSET);
  bytes.set(REALM_ID, CORE_STATE_IDENTITY_REALM_OFFSET);
  new DataView(bytes.buffer).setBigUint64(CORE_STATE_GENERATION_OFFSET, 2n, true);
  bytes.set(new PublicKey(RENT_REFUND).toBytes(), CORE_STATE_RENT_BENEFICIARY_OFFSET);
  return bytes;
}

function account(owner: string, data: Uint8Array, overrides: Partial<RpcAccount> = {}): RpcAccount {
  return Object.freeze({ data, executable: false, lamports: '2895840', owner, space: data.length, ...overrides });
}

/** Exact output shape of CustodyReplayV1::initialize for this aggregate. */
function canonicalReplayBytes(): Uint8Array {
  const replay = new Uint8Array(CUSTODY_REPLAY_BYTES_V1);
  replay.set(CUSTODY_REPLAY_MAGIC_V1, 0);
  new DataView(replay.buffer).setUint16(CUSTODY_REPLAY_VERSION_OFFSET_V1, 1, true);
  replay[CUSTODY_REPLAY_STATUS_OFFSET_V1] = 1;
  replay[CUSTODY_REPLAY_CALLER_ROLE_OFFSET_V1] = EXECUTION_ROLE_CLAIMS_V1;
  new DataView(replay.buffer).setUint32(CUSTODY_REPLAY_OPEN_VAULT_COUNT_OFFSET_V1, 0, true);
  replay.set(RELEASE_SET, CUSTODY_REPLAY_RELEASE_SET_OFFSET_V1);
  replay.set(new PublicKey(MARKET).toBytes(), CUSTODY_REPLAY_MARKET_OFFSET_V1);
  replay.set(REALM_ID, CUSTODY_REPLAY_REALM_OFFSET_V1);
  replay.set(CONTEXT, CUSTODY_REPLAY_CONTEXT_OFFSET_V1);
  replay.set(new PublicKey(CLAIMS).toBytes(), CUSTODY_REPLAY_CALLER_PROGRAM_OFFSET_V1);
  replay.set(new PublicKey(PAYER).toBytes(), CUSTODY_REPLAY_RENT_REFUND_OFFSET_V1);
  new DataView(replay.buffer).setBigUint64(CUSTODY_REPLAY_NEXT_REVISION_OFFSET_V1, 1n, true);
  new DataView(replay.buffer).setBigUint64(CUSTODY_REPLAY_GENERATION_OFFSET_V1, 2n, true);
  replay.fill(0xa1, CUSTODY_REPLAY_LAST_REQUEST_DIGEST_OFFSET_V1, CUSTODY_REPLAY_LAST_POSTSTATE_COMMITMENT_OFFSET_V1);
  replay.fill(0xa2, CUSTODY_REPLAY_LAST_POSTSTATE_COMMITMENT_OFFSET_V1);
  return replay;
}

function replayAddress(): string {
  return PublicKey.findProgramAddressSync([
    CUSTODY_REPLAY_PDA_DOMAIN_V1, new PublicKey(MARKET).toBytes(), RELEASE_SET, Uint8Array.of(EXECUTION_ROLE_CLAIMS_V1), CONTEXT,
  ], new PublicKey(CUSTODY))[0].toBase58();
}

async function inspectWithReplay(replay: RpcAccount) {
  const aggregateAddress = deriveClaimsAggregateAddressV2(CLAIMS, MARKET);
  return inspectClaimsCustodyReplayV1(fakeClient({
    [aggregateAddress]: account(CLAIMS, aggregateBytes()),
    [replayAddress()]: replay,
  }), { marketAddress: MARKET, claimsProgramId: CLAIMS, custodyProgramId: CUSTODY, registryProgramId: REGISTRY, payer: PAYER });
}

function fakeClient(accounts: Record<string, RpcAccount>) {
  return {
    async finalizedSlot() { return '90'; },
    async multipleAccounts(addresses: ReadonlyArray<string>) {
      return Object.freeze({
        slot: '90',
        accounts: Object.freeze(addresses.map((address) => Object.freeze({
          address,
          account: accounts[address] ?? (address === MARKET ? account(REGISTRY, coreMarketBytes()) : null),
        }))),
      });
    },
    async minimumBalanceForRentExemption(dataLength: number) {
      return Object.freeze({ dataLength, lamports: '2895840' });
    },
    async latestMutationBlockhash() {
      return Object.freeze({ slot: '90', blockhash: BLOCKHASH, lastValidBlockHeight: '120' });
    },
  };
}

describe('Claims-role Custody replay creation (the wallet-side redemption precondition)', () => {
  it('encodes the exact 48-byte DCLCCR01 request: magic, version, zero reserve, Market at 16', () => {
    const bytes = encodeClaimsCustodyReplayRequestV1(MARKET);
    expect(bytes).toHaveLength(48);
    expect(new TextDecoder().decode(bytes.slice(0, 8))).toBe('DCLCCR01');
    expect(new DataView(bytes.buffer).getUint16(8, true)).toBe(1);
    expect(Array.from(bytes.slice(10, 16))).toEqual([0, 0, 0, 0, 0, 0]);
    expect(new PublicKey(bytes.slice(16, 48)).toBase58()).toBe(MARKET);
  });

  it('mirrors expected_request_v1: InitializeReplay under the Claims role with the immutable Core Market RentCredit as refund', async () => {
    const request = await encodeExpectedCustodyRequestV1({
      releaseSet: RELEASE_SET,
      market: new PublicKey(MARKET).toBytes(),
      realm: REALM_ID,
      context: CONTEXT,
      claimsProgram: new PublicKey(CLAIMS).toBytes(),
      payer: new PublicKey(PAYER).toBytes(),
      rentRefund: new PublicKey(RENT_REFUND).toBytes(),
      generation: 2n,
      rentLamports: 2895840n,
    });
    expect(request).toHaveLength(CUSTODY_REQUEST_BYTES_V1);
    expect(new TextDecoder().decode(request.slice(0, 8))).toBe('DCLCUSR1');
    expect(request[CUSTODY_REQUEST_OPERATION_OFFSET_V1]).toBe(0);
    expect(request[CUSTODY_REQUEST_CALLER_ROLE_OFFSET_V1]).toBe(EXECUTION_ROLE_CLAIMS_V1);
    expect(new PublicKey(request.slice(CUSTODY_REQUEST_MARKET_OFFSET_V1, CUSTODY_REQUEST_MARKET_OFFSET_V1 + 32)).toBase58()).toBe(MARKET);
    expect(new PublicKey(request.slice(CUSTODY_REQUEST_RENT_REFUND_OFFSET_V1, CUSTODY_REQUEST_RENT_REFUND_OFFSET_V1 + 32)).toBase58())
      .toBe(RENT_REFUND);
    expect(new DataView(request.buffer).getBigUint64(CUSTODY_REQUEST_RENT_LAMPORTS_OFFSET_V1, true)).toBe(2895840n);
    // The synthetic parent digest is nonzero: validate() requires it.
    expect(request.slice(CUSTODY_REQUEST_PARENT_REQUEST_DIGEST_OFFSET_V1, CUSTODY_REQUEST_PARENT_REQUEST_DIGEST_OFFSET_V1 + 32).some((byte) => byte !== 0)).toBe(true);
    expect(Array.from(request.slice(CUSTODY_REQUEST_RESERVED_OFFSET_V1))).toEqual(new Array(24).fill(0));
  });

  it('refuses zero rent the way the on-chain validate refuses it', async () => {
    await expect(encodeExpectedCustodyRequestV1({
      releaseSet: RELEASE_SET,
      market: new PublicKey(MARKET).toBytes(),
      realm: REALM_ID,
      context: CONTEXT,
      claimsProgram: new PublicKey(CLAIMS).toBytes(),
      payer: new PublicKey(PAYER).toBytes(),
      rentRefund: new PublicKey(RENT_REFUND).toBytes(),
      generation: 2n,
      rentLamports: 0n,
    })).rejects.toThrow('rent');
  });

  it('plans one legacy packet-bound transaction with the exact 15-account frame', async () => {
    const aggregateAddress = deriveClaimsAggregateAddressV2(CLAIMS, MARKET);
    const state = await inspectClaimsCustodyReplayV1(fakeClient({
      [aggregateAddress]: account(CLAIMS, aggregateBytes()),
    }), { marketAddress: MARKET, claimsProgramId: CLAIMS, custodyProgramId: CUSTODY, registryProgramId: REGISTRY, payer: PAYER });
    expect(state.status).toBe('creatable');
    if (state.status !== 'creatable') return;
    const plan = state.plan;
    expect(plan.wireBytes.length).toBeLessThanOrEqual(1232);
    expect(plan.requiredSigners).toEqual([PAYER]);
    expect(plan.instructionData).toHaveLength(48);
    // The message is legacy on purpose: no address-table lookups exist.
    expect('addressTableLookups' in plan.transaction.message ? plan.transaction.message.addressTableLookups : []).toEqual([]);
    // Two instructions: an explicit compute ceiling, then the route.
    expect(plan.transaction.message.compiledInstructions).toHaveLength(2);
    const route = plan.transaction.message.compiledInstructions[1];
    expect(route.accountKeyIndexes).toHaveLength(CLAIMS_CUSTODY_REPLAY_ACCOUNT_COUNT_V1);
    const staticKeys = plan.transaction.message.staticAccountKeys.map((key) => key.toBase58());
    const frame = Array.from(route.accountKeyIndexes, (index) => staticKeys[index]);
    expect(frame[REPLAY_ACCOUNT_PAYER_V1]).toBe(PAYER);
    expect(frame[REPLAY_ACCOUNT_CUSTODY_REPLAY_V1]).toBe(plan.replayAddress);
    expect(frame[REPLAY_ACCOUNT_AGGREGATE_V1]).toBe(plan.aggregateAddress);
    expect(frame[REPLAY_ACCOUNT_RENT_REFUND_V1]).toBe(RENT_REFUND);
    expect(plan.rentRefundAddress).toBe(RENT_REFUND);
    expect(state.note).toContain('legacy transaction');
    expect(CLAIMS_CUSTODY_REPLAY_COMPUTE_UNIT_LIMIT_V1).toBeGreaterThan(160_000);
  });

  it('derives a Claims-role replay address the Trading role can never alias', () => {
    const marketBytes = new PublicKey(MARKET).toBytes();
    const claims = PublicKey.findProgramAddressSync([
      CUSTODY_REPLAY_PDA_DOMAIN_V1, marketBytes, RELEASE_SET, Uint8Array.of(EXECUTION_ROLE_CLAIMS_V1), CONTEXT,
    ], new PublicKey(CUSTODY))[0].toBase58();
    const trading = PublicKey.findProgramAddressSync([
      CUSTODY_REPLAY_PDA_DOMAIN_V1, marketBytes, RELEASE_SET, Uint8Array.of(EXECUTION_ROLE_TRADING_V1), CONTEXT,
    ], new PublicKey(CUSTODY))[0].toBase58();
    expect(claims).not.toBe(trading);
  });

  it('reports an existing well-formed replay as exists with its live revision cursor', async () => {
    const aggregateAddress = deriveClaimsAggregateAddressV2(CLAIMS, MARKET);
    const replay = canonicalReplayBytes();
    const state = await inspectClaimsCustodyReplayV1(fakeClient({
      [aggregateAddress]: account(CLAIMS, aggregateBytes()),
      [replayAddress()]: account(CUSTODY, replay),
    }), { marketAddress: MARKET, claimsProgramId: CLAIMS, custodyProgramId: CUSTODY, registryProgramId: REGISTRY, payer: PAYER });
    expect(state).toMatchObject({ status: 'exists', replayAddress: replayAddress(), nextRevision: '1', generation: '2', rentRefund: PAYER });
  });

  it.each([
    ['magic', (bytes: Uint8Array) => { bytes[0] ^= 1; }],
    ['version', (bytes: Uint8Array) => { new DataView(bytes.buffer).setUint16(CUSTODY_REPLAY_VERSION_OFFSET_V1, 2, true); }],
    ['initialized status', (bytes: Uint8Array) => { bytes[CUSTODY_REPLAY_STATUS_OFFSET_V1] = 0; }],
    ['Claims caller role', (bytes: Uint8Array) => { bytes[CUSTODY_REPLAY_CALLER_ROLE_OFFSET_V1] = EXECUTION_ROLE_TRADING_V1; }],
    ['release set', (bytes: Uint8Array) => { bytes[CUSTODY_REPLAY_RELEASE_SET_OFFSET_V1] ^= 1; }],
    ['Market', (bytes: Uint8Array) => { bytes[CUSTODY_REPLAY_MARKET_OFFSET_V1] ^= 1; }],
    ['Realm', (bytes: Uint8Array) => { bytes[CUSTODY_REPLAY_REALM_OFFSET_V1] ^= 1; }],
    ['context', (bytes: Uint8Array) => { bytes[CUSTODY_REPLAY_CONTEXT_OFFSET_V1] ^= 1; }],
    ['Claims caller program', (bytes: Uint8Array) => { bytes[CUSTODY_REPLAY_CALLER_PROGRAM_OFFSET_V1] ^= 1; }],
    ['nonzero rent refund', (bytes: Uint8Array) => { bytes.fill(0, CUSTODY_REPLAY_RENT_REFUND_OFFSET_V1, CUSTODY_REPLAY_RENT_REFUND_OFFSET_V1 + 32); }],
    ['nonzero next revision', (bytes: Uint8Array) => { new DataView(bytes.buffer).setBigUint64(CUSTODY_REPLAY_NEXT_REVISION_OFFSET_V1, 0n, true); }],
    ['aggregate generation join', (bytes: Uint8Array) => { new DataView(bytes.buffer).setBigUint64(CUSTODY_REPLAY_GENERATION_OFFSET_V1, 3n, true); }],
    ['nonzero last-request digest', (bytes: Uint8Array) => { bytes.fill(0, CUSTODY_REPLAY_LAST_REQUEST_DIGEST_OFFSET_V1, CUSTODY_REPLAY_LAST_POSTSTATE_COMMITMENT_OFFSET_V1); }],
    ['nonzero last-poststate commitment', (bytes: Uint8Array) => { bytes.fill(0, CUSTODY_REPLAY_LAST_POSTSTATE_COMMITMENT_OFFSET_V1); }],
  ])('refuses an existing replay when only its %s differs from Rust', async (_field, mutate) => {
    const replay = canonicalReplayBytes();
    mutate(replay);
    expect((await inspectWithReplay(account(CUSTODY, replay))).status).toBe('refused');
  });

  it.each([
    ['owner', account(CLAIMS, canonicalReplayBytes())],
    ['executable bit', account(CUSTODY, canonicalReplayBytes(), { executable: true })],
    ['lamports', account(CUSTODY, canonicalReplayBytes(), { lamports: '0' })],
    ['RPC space', account(CUSTODY, canonicalReplayBytes(), { space: CUSTODY_REPLAY_BYTES_V1 - 1 })],
  ])('refuses an existing replay with invalid account %s semantics', async (_field, replay) => {
    expect((await inspectWithReplay(replay)).status).toBe('refused');
  });

  it('accepts only an exact present System vacancy as creatable', async () => {
    const aggregateAddress = deriveClaimsAggregateAddressV2(CLAIMS, MARKET);
    const base = { marketAddress: MARKET, claimsProgramId: CLAIMS, custodyProgramId: CUSTODY, registryProgramId: REGISTRY, payer: PAYER };
    const vacant = account(SYSTEM_PROGRAM_ID, new Uint8Array(), { lamports: '0' });
    const state = await inspectClaimsCustodyReplayV1(fakeClient({
      [aggregateAddress]: account(CLAIMS, aggregateBytes()),
      [replayAddress()]: vacant,
    }), base);
    expect(state.status).toBe('creatable');

    for (const occupied of [
      account(CUSTODY, new Uint8Array(), { lamports: '0' }),
      account(SYSTEM_PROGRAM_ID, new Uint8Array(), { lamports: '1' }),
      account(SYSTEM_PROGRAM_ID, new Uint8Array(), { lamports: '0', executable: true }),
      account(SYSTEM_PROGRAM_ID, new Uint8Array(), { lamports: '0', space: 1 }),
    ]) {
      expect((await inspectWithReplay(occupied)).status).toBe('refused');
    }
  });

  it('refuses a nonzero aggregate reserved byte exactly as LiabilityBasisMarketViewV2::decode does', async () => {
    const aggregateAddress = deriveClaimsAggregateAddressV2(CLAIMS, MARKET);
    const aggregate = aggregateBytes();
    aggregate[10] = 1;
    const state = await inspectClaimsCustodyReplayV1(fakeClient({
      [aggregateAddress]: account(CLAIMS, aggregate),
    }), { marketAddress: MARKET, claimsProgramId: CLAIMS, custodyProgramId: CUSTODY, registryProgramId: REGISTRY, payer: PAYER });
    expect(state.status).toBe('refused');
    if (state.status === 'refused') expect(state.reason).toContain('reserved bytes');
  });

  it('refuses a missing aggregate instead of planning against a namespace nobody persisted', async () => {
    const state = await inspectClaimsCustodyReplayV1(fakeClient({}), {
      marketAddress: MARKET, claimsProgramId: CLAIMS, custodyProgramId: CUSTODY, registryProgramId: REGISTRY, payer: PAYER,
    });
    expect(state.status).toBe('refused');
    if (state.status === 'refused') expect(state.reason).toContain('no Claims aggregate exists');
  });

  it('refuses an aggregate naming another logical Market', async () => {
    const aggregateAddress = deriveClaimsAggregateAddressV2(CLAIMS, MARKET);
    const other = Keypair.fromSeed(new Uint8Array(32).fill(44)).publicKey.toBase58();
    const state = await inspectClaimsCustodyReplayV1(fakeClient({
      [aggregateAddress]: account(CLAIMS, aggregateBytes({ logicalMarket: other })),
    }), { marketAddress: MARKET, claimsProgramId: CLAIMS, custodyProgramId: CUSTODY, registryProgramId: REGISTRY, payer: PAYER });
    expect(state.status).toBe('refused');
    if (state.status === 'refused') expect(state.reason).toContain('names logical Market');
  });

  it('refuses an account at the replay address that is not this replay, rather than calling it created', async () => {
    const aggregateAddress = deriveClaimsAggregateAddressV2(CLAIMS, MARKET);
    const state = await inspectClaimsCustodyReplayV1(fakeClient({
      [aggregateAddress]: account(CLAIMS, aggregateBytes()),
      [replayAddress()]: account(CUSTODY, new Uint8Array(64).fill(7)),
    }), { marketAddress: MARKET, claimsProgramId: CLAIMS, custodyProgramId: CUSTODY, registryProgramId: REGISTRY, payer: PAYER });
    expect(state.status).toBe('refused');
    if (state.status === 'refused') expect(state.reason).toContain('does not decode');
  });
});
