import { Keypair, PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { sha256 } from './bytes';
import {
  CAPABILITY_ENTRY_BYTES_V1,
  CAPABILITY_ENTRY_QUOTE_OFFSET_V1,
  CAPABILITY_MANIFEST_HEADER_BYTES_V1,
} from './capabilityManifest';
import {
  DIRECT_PACKET_WALL_V1,
  DIRECT_PRESTATE_WALL_V1,
  inspectDirectTradeSpineV1,
} from './directTradeSpine';
import * as Abi from './generated/directInlineV3';
import {
  CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
  CORE_PHASE_OPEN_TAG,
  CORE_PHASE_TERMINAL_TAG,
  CORE_READINESS_CONSUMED_TAG,
  CORE_STATE_BYTES,
  CORE_STATE_CAPABILITY_MANIFEST_OFFSET,
  CORE_STATE_GENERATION_OFFSET,
  CORE_STATE_IDENTITY_REALM_OFFSET,
  CORE_STATE_MAGIC,
  CORE_STATE_MARKET_ID_OFFSET,
  CORE_STATE_PHASE_OFFSET,
  CORE_STATE_PRODUCT_ID_OFFSET,
  CORE_STATE_PRODUCT_RECORD_OFFSET,
  CORE_STATE_READINESS_OFFSET,
  CORE_STATE_REGISTRY_PROGRAM_OFFSET,
  CORE_STATE_RENT_BENEFICIARY_OFFSET,
  CORE_STATE_RESOLUTION_POLICY_OFFSET,
  CORE_STATE_SELECTED_RELEASE_SET_OFFSET,
  CORE_STATE_TERMINAL_RECEIPT_OFFSET,
  CORE_STATE_TERMINAL_WINNER_OFFSET,
  CORE_STATE_VERSION_OFFSET,
  CORE_VERSION,
  LIABILITY_BASIS_MARKET_CLAIM_COUNT_OFFSET,
  LIABILITY_BASIS_MARKET_CUSTODY_CONTEXT_OFFSET,
  LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
  LIABILITY_BASIS_MARKET_LOGICAL_ID_OFFSET,
  LIABILITY_BASIS_MARKET_MAGIC_V2,
  LIABILITY_BASIS_MARKET_REALM_OFFSET,
  LIABILITY_BASIS_MARKET_REGISTRY_OFFSET,
  LIABILITY_BASIS_MARKET_RELEASE_SET_OFFSET,
  LIABILITY_BASIS_STATE_VERSION_V2,
} from './generated/coreFound';
import { deriveClaimsAggregateAddressV2 } from './marketCoreV2';
import { deriveFinalizedRecordAddressesV1 } from './releaseRegistry';
import { type RpcAccount } from './rpc';

const CORE = Keypair.fromSeed(new Uint8Array(32).fill(51)).publicKey.toBase58();
const REGISTRY = Keypair.fromSeed(new Uint8Array(32).fill(52)).publicKey.toBase58();
const TRADING = Keypair.fromSeed(new Uint8Array(32).fill(53)).publicKey.toBase58();
const CLAIMS = Keypair.fromSeed(new Uint8Array(32).fill(54)).publicKey.toBase58();
const MARKET = Keypair.fromSeed(new Uint8Array(32).fill(55)).publicKey.toBase58();
const OWNER = Keypair.fromSeed(new Uint8Array(32).fill(56)).publicKey.toBase58();

function putU16(bytes: Uint8Array, offset: number, value: number): void {
  new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength).setUint16(offset, value, true);
}

function putU32(bytes: Uint8Array, offset: number, value: number): void {
  new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength).setUint32(offset, value, true);
}

function putU64(bytes: Uint8Array, offset: number, value: bigint): void {
  new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength).setBigUint64(offset, value, true);
}

function identity(seed: number): Uint8Array {
  return new Uint8Array(32).fill(seed);
}

function configFixture(): Uint8Array {
  const bytes = new Uint8Array(Abi.DIRECT_EXECUTION_CONFIG_BYTES_V1);
  bytes.set(Abi.DIRECT_CONFIG_MAGIC_V1, Abi.DIRECT_CONFIG_MAGIC_OFFSET_V1);
  putU16(bytes, Abi.DIRECT_CONFIG_VERSION_OFFSET_V1, 1);
  bytes.set(identity(9), Abi.DIRECT_CONFIG_FEE_RECIPIENT_OFFSET_V1);
  putU64(bytes, Abi.DIRECT_CONFIG_PRICE_SCALE_OFFSET_V1, 1_000_000n);
  putU16(bytes, Abi.DIRECT_CONFIG_FEE_BPS_OFFSET_V1, 25);
  return bytes;
}

function descriptorFixture(): Uint8Array {
  const bytes = new Uint8Array(Abi.CAPABILITY_PROGRAM_V4_BYTES);
  bytes.set(Abi.CAPABILITY_PROGRAM_V4_MAGIC);
  putU16(bytes, Abi.CAPABILITY_PROGRAM_V4_SCHEMA_VERSION_OFFSET, Abi.CAPABILITY_PROGRAM_V4_SCHEMA_VERSION);
  putU16(bytes, Abi.CAPABILITY_PROGRAM_V4_ARTIFACT_PROFILE_OFFSET, Abi.CAPABILITY_PROGRAM_V4_ARTIFACT_PROFILE);
  bytes.set(Abi.DIRECT_SUCCESSOR_KIND_ID_V3, Abi.CAPABILITY_PROGRAM_V4_KIND_OFFSET);
  bytes.set(Abi.DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1, Abi.CAPABILITY_PROGRAM_V4_CONFIG_SCHEMA_OFFSET);
  bytes.set(Abi.DIRECT_EXECUTION_REQUEST_SCHEMA_ID_V3, Abi.CAPABILITY_PROGRAM_V4_REQUEST_SCHEMA_OFFSET);
  bytes.set(Abi.DIRECT_ROOT_SCHEMA_ID_V1, Abi.CAPABILITY_PROGRAM_V4_ROOT_SCHEMA_OFFSET);
  bytes.set(Abi.DIRECT_INLINE_ORDINARY_LIFECYCLE_ID_V5, Abi.CAPABILITY_PROGRAM_V4_DERIVATION_POLICY_OFFSET);
  bytes.set(identity(4), Abi.CAPABILITY_PROGRAM_V4_CAPACITY_PROFILE_OFFSET);
  bytes.set(Abi.ACCOUNT_SCHEMA_RELEASE_ID, Abi.CAPABILITY_PROGRAM_V4_ACCOUNT_PROFILE_SCHEMA_OFFSET);
  bytes.set(Abi.DIRECT_INLINE_ORDINARY_ACCOUNT_PROFILE_ID_V3, Abi.CAPABILITY_PROGRAM_V4_ACCOUNT_PROFILE_PROGRAM_OFFSET);
  bytes.set(Abi.REQUEST_PROFILE_V2_SCHEMA_RELEASE_ID, Abi.CAPABILITY_PROGRAM_V4_REQUEST_PROFILE_SCHEMA_OFFSET);
  bytes.set(Abi.DIRECT_INLINE_ORDINARY_REQUEST_PROFILE_ID_V3, Abi.CAPABILITY_PROGRAM_V4_REQUEST_PROFILE_PROGRAM_OFFSET);
  bytes.set(Abi.SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5, Abi.CAPABILITY_PROGRAM_V4_LIFECYCLE_SCHEMA_OFFSET);
  bytes.set(Abi.DIRECT_INLINE_ORDINARY_LIFECYCLE_ID_V5, Abi.CAPABILITY_PROGRAM_V4_LIFECYCLE_PROGRAM_OFFSET);
  bytes.set(Abi.EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2, Abi.CAPABILITY_PROGRAM_V4_STRATEGY_SCHEMA_OFFSET);
  bytes.set(Abi.DIRECT_INLINE_ORDINARY_STRATEGY_ID_V3, Abi.CAPABILITY_PROGRAM_V4_STRATEGY_PROGRAM_OFFSET);
  bytes.set(Abi.TRANSITION_SCHEMA_RELEASE_ID, Abi.CAPABILITY_PROGRAM_V4_TRANSITION_SCHEMA_OFFSET);
  bytes.set(Abi.DIRECT_INLINE_ORDINARY_TRANSITION_ID_V3, Abi.CAPABILITY_PROGRAM_V4_TRANSITION_PROGRAM_OFFSET);
  bytes.set(Abi.EFFECT_SCHEMA_RELEASE_ID, Abi.CAPABILITY_PROGRAM_V4_EFFECT_SCHEMA_OFFSET);
  bytes.set(Abi.DIRECT_INLINE_ORDINARY_EFFECT_ID_V4, Abi.CAPABILITY_PROGRAM_V4_EFFECT_PROGRAM_OFFSET);
  putU32(bytes, Abi.CAPABILITY_PROGRAM_V4_ROOT_STATE_BYTES_OFFSET, Abi.DIRECT_ROOT_STATE_BYTES_V1);
  return bytes;
}

function programSetFixture(descriptorDigest: Uint8Array): Uint8Array {
  const bytes = new Uint8Array(Abi.CAPABILITY_PROGRAM_SET_HEADER_BYTES_V2 + Abi.CAPABILITY_PROGRAM_SET_ENTRY_BYTES_V2);
  bytes.set(Abi.CAPABILITY_PROGRAM_SET_MAGIC_V2);
  putU16(bytes, 8, Abi.CAPABILITY_PROGRAM_SET_SCHEMA_VERSION_V2);
  putU16(bytes, 10, Abi.CAPABILITY_PROGRAM_SET_ARTIFACT_PROFILE_V2);
  putU32(bytes, Abi.CAPABILITY_PROGRAM_SET_SELECTOR_OFFSET_OFFSET_V2, Abi.DIRECT_EXECUTION_REQUEST_SELECTOR_OFFSET_V3);
  bytes[Abi.CAPABILITY_PROGRAM_SET_SELECTOR_WIDTH_OFFSET_V2] = 4;
  bytes[Abi.CAPABILITY_PROGRAM_SET_SELECTOR_ENDIAN_OFFSET_V2] = Abi.CAPABILITY_PROGRAM_SET_CANONICAL_ENDIAN_V2;
  putU16(bytes, Abi.CAPABILITY_PROGRAM_SET_ENTRY_COUNT_OFFSET_V2, 1);
  const entry = Abi.CAPABILITY_PROGRAM_SET_HEADER_BYTES_V2;
  putU32(bytes, entry + Abi.CAPABILITY_PROGRAM_SET_ENTRY_SELECTOR_OFFSET_V2, Abi.DIRECT_INLINE_ORDINARY_ACTION_V3);
  bytes.set(Abi.CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID, entry + Abi.CAPABILITY_PROGRAM_SET_ENTRY_DESCRIPTOR_SCHEMA_OFFSET_V2);
  bytes.set(descriptorDigest, entry + Abi.CAPABILITY_PROGRAM_SET_ENTRY_DESCRIPTOR_PROGRAM_OFFSET_V2);
  return bytes;
}

function manifestFixture(programSetDigest: Uint8Array, configDigest: Uint8Array): Uint8Array {
  const bytes = new Uint8Array(CAPABILITY_MANIFEST_HEADER_BYTES_V1 + CAPABILITY_ENTRY_BYTES_V1);
  bytes.set(new TextEncoder().encode('DCLTCAP1'));
  putU16(bytes, 8, 1);
  putU16(bytes, 10, 1);
  putU16(bytes, 12, 1);
  const offset = CAPABILITY_MANIFEST_HEADER_BYTES_V1;
  bytes.set(Abi.DIRECT_SUCCESSOR_KIND_ID_V3, offset);
  bytes.set(programSetDigest, offset + 32);
  bytes.set(configDigest, offset + 64);
  bytes.set(identity(41), offset + 96);
  bytes.set(identity(42), offset + 128);
  bytes.set(identity(43), offset + 160);
  const quote = offset + CAPABILITY_ENTRY_QUOTE_OFFSET_V1;
  bytes.set(new TextEncoder().encode('DCLTFQ01'), quote);
  putU16(bytes, quote + 8, 1);
  return bytes;
}

function marketFixture(manifestDigest: Uint8Array, phase: number): Uint8Array {
  const bytes = new Uint8Array(CORE_STATE_BYTES);
  bytes.set(CORE_STATE_MAGIC, 0);
  putU16(bytes, CORE_STATE_VERSION_OFFSET, CORE_VERSION);
  bytes[CORE_STATE_PHASE_OFFSET] = phase;
  bytes[CORE_STATE_READINESS_OFFSET] = CORE_READINESS_CONSUMED_TAG;
  bytes.set(new PublicKey(MARKET).toBytes(), CORE_STATE_MARKET_ID_OFFSET);
  bytes.set(identity(61), CORE_STATE_IDENTITY_REALM_OFFSET);
  bytes.set(identity(62), CORE_STATE_PRODUCT_RECORD_OFFSET);
  bytes.set(identity(63), CORE_STATE_PRODUCT_ID_OFFSET);
  bytes.set(identity(64), CORE_STATE_RESOLUTION_POLICY_OFFSET);
  bytes.set(manifestDigest, CORE_STATE_CAPABILITY_MANIFEST_OFFSET);
  bytes.set(identity(65), CORE_STATE_SELECTED_RELEASE_SET_OFFSET);
  bytes.set(new PublicKey(REGISTRY).toBytes(), CORE_STATE_REGISTRY_PROGRAM_OFFSET);
  putU64(bytes, CORE_STATE_GENERATION_OFFSET, 2n);
  bytes.set(new PublicKey(Keypair.fromSeed(new Uint8Array(32).fill(66)).publicKey.toBytes()).toBytes(), CORE_STATE_RENT_BENEFICIARY_OFFSET);
  if (phase === CORE_PHASE_TERMINAL_TAG) {
    putU32(bytes, CORE_STATE_TERMINAL_WINNER_OFFSET, 1);
    bytes.set(identity(67), CORE_STATE_TERMINAL_RECEIPT_OFFSET);
  }
  return bytes;
}

function aggregateFixture(): Uint8Array {
  const claimCount = 4;
  const bytes = new Uint8Array(LIABILITY_BASIS_MARKET_HEADER_BYTES_V2 + claimCount * 8);
  bytes.set(LIABILITY_BASIS_MARKET_MAGIC_V2, 0);
  putU16(bytes, 8, LIABILITY_BASIS_STATE_VERSION_V2);
  putU32(bytes, LIABILITY_BASIS_MARKET_CLAIM_COUNT_OFFSET, claimCount);
  bytes.set(new PublicKey(MARKET).toBytes(), LIABILITY_BASIS_MARKET_LOGICAL_ID_OFFSET);
  bytes.set(new PublicKey(REGISTRY).toBytes(), LIABILITY_BASIS_MARKET_REGISTRY_OFFSET);
  bytes.set(identity(65), LIABILITY_BASIS_MARKET_RELEASE_SET_OFFSET);
  bytes.set(identity(61), LIABILITY_BASIS_MARKET_REALM_OFFSET);
  bytes.set(identity(71), LIABILITY_BASIS_MARKET_CUSTODY_CONTEXT_OFFSET);
  return bytes;
}

function account(owner: string, data: Uint8Array): RpcAccount {
  return Object.freeze({ data, executable: false, lamports: '1000000', owner, space: data.length });
}

async function chainFixture(phase = CORE_PHASE_OPEN_TAG): Promise<Record<string, RpcAccount>> {
  const config = configFixture();
  const configDigest = await sha256(config);
  const descriptor = descriptorFixture();
  const descriptorDigest = await sha256(descriptor);
  const programSet = programSetFixture(descriptorDigest);
  const programSetDigest = await sha256(programSet);
  const manifest = manifestFixture(programSetDigest, configDigest);
  const manifestDigest = await sha256(manifest);
  const market = marketFixture(manifestDigest, phase);
  const accounts: Record<string, RpcAccount> = {
    [MARKET]: account(CORE, market),
    [deriveFinalizedRecordAddressesV1(REGISTRY, CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, manifestDigest).record]: account(REGISTRY, manifest),
    [deriveFinalizedRecordAddressesV1(REGISTRY, Abi.CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2, programSetDigest).record]: account(REGISTRY, programSet),
    [deriveFinalizedRecordAddressesV1(REGISTRY, Abi.CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID, descriptorDigest).record]: account(REGISTRY, descriptor),
    [deriveFinalizedRecordAddressesV1(REGISTRY, Abi.DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1, configDigest).record]: account(REGISTRY, config),
    [deriveClaimsAggregateAddressV2(CLAIMS, MARKET)]: account(CLAIMS, aggregateFixture()),
  };
  return accounts;
}

function client(accounts: Record<string, RpcAccount>) {
  return {
    async finalizedSlot() { return '90'; },
    async multipleAccounts(addresses: ReadonlyArray<string>) {
      return Object.freeze({
        slot: '90',
        accounts: Object.freeze(addresses.map((address) => Object.freeze({ address, account: accounts[address] ?? null }))),
      });
    },
  };
}

describe('the Direct trade spine', () => {
  it('derives the whole Direct capability spine from the Market alone', async () => {
    const spine = await inspectDirectTradeSpineV1(client(await chainFixture()), {
      marketAddress: MARKET, coreProgramId: CORE, registryProgramId: REGISTRY,
      tradingProgramId: TRADING, claimsProgramId: CLAIMS, owner: OWNER,
    });
    expect(spine.status).toBe('inspected');
    if (spine.status !== 'inspected') return;
    expect(spine.phase).toBe('Open');
    expect(spine.priceScale).toBe(1_000_000n);
    expect(spine.feeBasisPoints).toBe(25);
    expect(spine.outcomeCount).toBe(4);
    expect(spine.entryIndex).toBe(0);
    // No root was planted, so the activation wall is named with its address.
    expect(spine.rootExists).toBe(false);
    expect(spine.tradable).toBe(false);
    const wallNames = spine.walls.map((wall) => wall.name);
    expect(wallNames).toContain('activation');
    expect(wallNames).toContain('prestate');
    expect(wallNames).toContain('packet');
  });

  it('reports an activated root as standing, leaving only the walls that remain', async () => {
    const accounts = await chainFixture();
    const spineBefore = await inspectDirectTradeSpineV1(client(accounts), {
      marketAddress: MARKET, coreProgramId: CORE, registryProgramId: REGISTRY,
      tradingProgramId: TRADING, claimsProgramId: CLAIMS,
    });
    if (spineBefore.status !== 'inspected' || spineBefore.rootAddress === null) throw new Error('spine must derive a root address');
    accounts[spineBefore.rootAddress] = account(TRADING, new Uint8Array(64).fill(1));
    const spine = await inspectDirectTradeSpineV1(client(accounts), {
      marketAddress: MARKET, coreProgramId: CORE, registryProgramId: REGISTRY,
      tradingProgramId: TRADING, claimsProgramId: CLAIMS,
    });
    expect(spine.status).toBe('inspected');
    if (spine.status !== 'inspected') return;
    expect(spine.rootExists).toBe(true);
    expect(spine.tradable).toBe(true);
    expect(spine.walls.map((wall) => wall.name)).toEqual(['packet']);
  });

  it('names a non-Open phase as the Market speaking, not an outage', async () => {
    const spine = await inspectDirectTradeSpineV1(client(await chainFixture(CORE_PHASE_TERMINAL_TAG)), {
      marketAddress: MARKET, coreProgramId: CORE, registryProgramId: REGISTRY,
    });
    expect(spine.status).toBe('inspected');
    if (spine.status !== 'inspected') return;
    expect(spine.phase).toBe('Terminal');
    expect(spine.tradable).toBe(false);
    expect(spine.walls.find((wall) => wall.name === 'phase')?.detail).toContain('trading is only open while a Market is Open');
  });

  it('refuses a manifest without a Direct entry by naming the Market choice', async () => {
    const accounts = await chainFixture();
    // Rebuild the manifest with a different kind so no Direct entry exists.
    const config = configFixture();
    const manifest = manifestFixture(await sha256(config), await sha256(config));
    manifest.set(new Uint8Array(32).fill(200), CAPABILITY_MANIFEST_HEADER_BYTES_V1);
    const manifestDigest = await sha256(manifest);
    accounts[MARKET] = account(CORE, marketFixture(manifestDigest, CORE_PHASE_OPEN_TAG));
    accounts[deriveFinalizedRecordAddressesV1(REGISTRY, CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, manifestDigest).record] = account(REGISTRY, manifest);
    const spine = await inspectDirectTradeSpineV1(client(accounts), {
      marketAddress: MARKET, coreProgramId: CORE, registryProgramId: REGISTRY,
    });
    expect(spine.status).toBe('refused');
    if (spine.status === 'refused') expect(spine.reason).toContain('none is the Direct successor kind');
  });

  it('refuses a Market the selected Core program does not own', async () => {
    const accounts = await chainFixture();
    accounts[MARKET] = account(TRADING, accounts[MARKET].data);
    const spine = await inspectDirectTradeSpineV1(client(accounts), {
      marketAddress: MARKET, coreProgramId: CORE, registryProgramId: REGISTRY,
    });
    expect(spine.status).toBe('refused');
    if (spine.status === 'refused') expect(spine.reason).toContain('not owned by the selected Core program');
  });

  it('carries the packet and prestate walls verbatim as named protocol facts', () => {
    expect(DIRECT_PACKET_WALL_V1.detail).toContain('1,268 > 1,232');
    expect(DIRECT_PRESTATE_WALL_V1.detail).toContain('ADR-0008');
  });
});
