import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import {
  authenticateDirectHotOuterDeploymentsV3,
  authenticateDirectCapabilitySealV1,
  decodeDirectDescriptorV4,
  decodeDirectProgramSetV2,
  decodeDirectRootSelectionV1,
  decodeSelectedDirectManifestEntryV1,
  validateProductBasisV3,
  validateDirectSignedRequestProfileV2,
} from './directHotChain';
import { hex, sha256 } from './bytes';
import * as Abi from './generated/directInlineV3';
import {
  LOADER_V3_PROGRAMDATA_OFFSET,
  UPGRADEABLE_LOADER_ID,
  type ArtifactReleaseV1,
} from './releaseRegistry';
import { type RpcAccount } from './rpc';

function identity(seed: number): Uint8Array {
  return new Uint8Array(32).fill(seed);
}

function putU16(bytes: Uint8Array, offset: number, value: number): void {
  new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength).setUint16(offset, value, true);
}

function putU32(bytes: Uint8Array, offset: number, value: number): void {
  new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength).setUint32(offset, value, true);
}

function putU64(bytes: Uint8Array, offset: number, value: bigint): void {
  new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength).setBigUint64(offset, value, true);
}

function concat(...parts: ReadonlyArray<Uint8Array>): Uint8Array {
  const output = new Uint8Array(parts.reduce((total, part) => total + part.length, 0));
  let offset = 0;
  for (const part of parts) { output.set(part, offset); offset += part.length; }
  return output;
}

function rpcAccount(owner: string, executable: boolean, data: Uint8Array): RpcAccount {
  return Object.freeze({ owner, executable, data, lamports: '1', space: data.length });
}

async function capabilitySealFixture(): Promise<Readonly<{
  client: Parameters<typeof authenticateDirectCapabilitySealV1>[0];
  accounts: Map<string, RpcAccount | null>;
  fixed: Array<Readonly<{ address: string; isSigner: boolean; isWritable: boolean; executable: boolean }>>;
  trading: string;
  tradingRelease: string;
  registry: string;
  descriptorSchema: Uint8Array;
  descriptorDigest: Uint8Array;
  records: Parameters<typeof authenticateDirectCapabilitySealV1>[8];
  sealAddress: string;
}>> {
  const trading = new PublicKey(identity(80)).toBase58();
  const registry = new PublicKey(identity(81)).toBase58();
  const tradingRelease = '52'.repeat(32);
  const descriptorSchema = identity(12);
  const descriptorDigest = identity(13);
  const action = new Uint8Array([1, 0, 0, 0]);
  const [seal, sealBump] = PublicKey.findProgramAddressSync([
    new TextEncoder().encode('dclutch:capability-seal:v1'), descriptorSchema, descriptorDigest, action,
    identity(0x52), new PublicKey(registry).toBytes(),
  ], new PublicKey(trading));
  const fixed = Array.from({ length: 39 }, (_, index) => Object.freeze({
    address: new PublicKey(identity(index + 100)).toBase58(), isSigner: false, isWritable: index === 1, executable: false,
  }));
  fixed[38] = Object.freeze({ address: seal.toBase58(), isSigner: false, isWritable: false, executable: false });
  const pairs = [[6, 7], [18, 19], [10, 11], [12, 13], [14, 15], [16, 17]] as const;
  const records = pairs.map(([rawIndex, stagingIndex], ordinal) => {
    const data = new Uint8Array(64 + ordinal).fill(ordinal + 1);
    const schema = ordinal === 0 ? descriptorSchema : identity(20 + ordinal);
    const digest = ordinal === 0 ? descriptorDigest : identity(30 + ordinal);
    return Object.freeze({ schema, digest, raw: rpcAccount(registry, false, data), rawIndex, stagingIndex });
  });
  const body = new Uint8Array(968);
  body.set(new TextEncoder().encode('DCLTCSL1'));
  putU16(body, 8, 1); putU16(body, 10, 1); putU16(body, 12, 6); putU16(body, 14, 0x00ff); putU32(body, 16, 1);
  body[20] = sealBump;
  body.set(descriptorSchema, 24); body.set(descriptorDigest, 56); body.set(identity(0x52), 88); body.set(new PublicKey(registry).toBytes(), 120);
  records.forEach((record, ordinal) => {
    const row = 152 + ordinal * 136;
    putU16(body, row, ordinal); putU32(body, row + 4, record.raw.data.length);
    body.set(record.schema, row + 8); body.set(record.digest, row + 40);
    body.set(new PublicKey(fixed[record.rawIndex]!.address).toBytes(), row + 72);
    body.set(new PublicKey(fixed[record.stagingIndex]!.address).toBytes(), row + 104);
  });
  const accounts = new Map<string, RpcAccount | null>([[seal.toBase58(), Object.freeze({ owner: trading, executable: false, data: body, lamports: '100', space: body.length })]]);
  return Object.freeze({
    client: { minimumBalanceForRentExemption: async (dataLength: number) => Object.freeze({ dataLength, lamports: '100' }) },
    accounts, fixed, trading, tradingRelease, registry, descriptorSchema, descriptorDigest, records, sealAddress: seal.toBase58(),
  });
}

async function mutableDeployment(seed: number, slot = 81n): Promise<Readonly<{
  artifact: ArtifactReleaseV1;
  programAddress: string;
  program: RpcAccount;
  programDataAddress: string;
  programData: RpcAccount;
}>> {
  const programKey = new PublicKey(identity(seed));
  const loader = new PublicKey(UPGRADEABLE_LOADER_ID);
  const programDataKey = PublicKey.findProgramAddressSync([programKey.toBytes()], loader)[0];
  const authority = new PublicKey(identity(seed + 40)).toBase58();
  const programBytes = new Uint8Array(36);
  putU32(programBytes, 0, 2);
  programBytes.set(programDataKey.toBytes(), 4);
  const programDataBytes = new Uint8Array(LOADER_V3_PROGRAMDATA_OFFSET + 64);
  putU32(programDataBytes, 0, 3);
  putU64(programDataBytes, 4, slot);
  programDataBytes[12] = 1;
  programDataBytes.set(new PublicKey(authority).toBytes(), 13);
  programDataBytes.fill(seed, LOADER_V3_PROGRAMDATA_OFFSET);
  const elfDigest = Array.from(await sha256(programDataBytes.slice(LOADER_V3_PROGRAMDATA_OFFSET)),
    (byte) => byte.toString(16).padStart(2, '0')).join('');
  const artifact: ArtifactReleaseV1 = Object.freeze({
    bytes: new Uint8Array(),
    program: programKey.toBase58(),
    loader: UPGRADEABLE_LOADER_ID,
    programData: programDataKey.toBase58(),
    semanticReleaseId: '11'.repeat(32),
    elfDigest,
    deploymentSlot: 81n,
    upgradePolicy: 'exact-authority',
    upgradeAuthority: authority,
  });
  return Object.freeze({
    artifact,
    programAddress: programKey.toBase58(),
    program: rpcAccount(UPGRADEABLE_LOADER_ID, true, programBytes),
    programDataAddress: programDataKey.toBase58(),
    programData: rpcAccount(UPGRADEABLE_LOADER_ID, false, programDataBytes),
  });
}

async function categoricalBasisFixture(): Promise<Readonly<{ basis: Uint8Array; domain: Uint8Array }>> {
  const basis = new Uint8Array(Abi.BASIS_HEADER_BYTES_V3);
  basis.set(Abi.BASIS_MAGIC_V3);
  putU16(basis, 8, Abi.BASIS_SCHEMA_V3);
  putU16(basis, 10, Abi.BASIS_HEADER_BYTES_V3);
  putU32(basis, 12, basis.length);
  basis[16] = 1;
  basis[17] = Abi.EXACT_CATEGORICAL_BOUNDARY_V3;
  putU32(basis, Abi.BASIS_WIDTH_OFFSET_V3, 3);
  basis.set(identity(1), 32);
  basis.set(identity(2), 64);
  basis.set(identity(3), 96);
  basis.set(identity(4), 128);
  putU64(basis, 160, 1n);
  putU64(basis, 168, 1n);
  basis.set(identity(5), 176);
  const domain = new Uint8Array(160);
  domain.set(identity(3), 64);
  domain.set(identity(4), 96);
  domain.set(await sha256(concat(
    new TextEncoder().encode('dclutch/product-basis/semantic/v3'),
    basis.slice(0, 32),
    basis.slice(96),
  )), 128);
  return Object.freeze({ basis, domain });
}

function rootFixture(): Uint8Array {
  const bytes = new Uint8Array(Abi.CAPABILITY_ROOT_HEADER_BYTES_V1 + Abi.DIRECT_ROOT_STATE_BYTES_V1);
  bytes.set(Abi.CAPABILITY_ROOT_MAGIC_V1, Abi.CAPABILITY_ROOT_MAGIC_OFFSET);
  putU16(bytes, Abi.CAPABILITY_ROOT_SCHEMA_VERSION_OFFSET, Abi.CAPABILITY_ROOT_SCHEMA_VERSION_V1);
  putU16(bytes, Abi.CAPABILITY_ROOT_PROFILE_OFFSET, Abi.CAPABILITY_ROOT_PROFILE_V1);
  const selection = Abi.CAPABILITY_ROOT_SELECTION_OFFSET;
  bytes.set(Abi.CAPABILITY_EXECUTION_SELECTION_MAGIC_V1, selection + Abi.CAPABILITY_EXECUTION_SELECTION_MAGIC_OFFSET);
  putU16(bytes, selection + Abi.CAPABILITY_EXECUTION_SELECTION_SCHEMA_VERSION_OFFSET, Abi.CAPABILITY_EXECUTION_SELECTION_SCHEMA_VERSION_V1);
  putU16(bytes, selection + Abi.CAPABILITY_EXECUTION_SELECTION_PROFILE_OFFSET, Abi.CAPABILITY_EXECUTION_SELECTION_PROFILE_V1);
  putU16(bytes, selection + Abi.CAPABILITY_EXECUTION_SELECTION_ENTRY_INDEX_OFFSET, 0);
  bytes.set(identity(1), selection + Abi.CAPABILITY_EXECUTION_SELECTION_MANIFEST_OFFSET);
  bytes.set(Abi.DIRECT_SUCCESSOR_KIND_ID_V3, selection + Abi.CAPABILITY_EXECUTION_SELECTION_KIND_OFFSET);
  bytes.set(identity(2), selection + Abi.CAPABILITY_EXECUTION_SELECTION_RELEASE_OFFSET);
  bytes.set(identity(3), selection + Abi.CAPABILITY_EXECUTION_SELECTION_CONFIG_OFFSET);
  return bytes;
}

function manifestFixture(): Uint8Array {
  const bytes = new Uint8Array(Abi.MANIFEST_HEADER_BYTES + Abi.CAPABILITY_ENTRY_BYTES);
  bytes.set(Abi.MANIFEST_MAGIC);
  putU16(bytes, Abi.MANIFEST_SCHEMA_OFFSET, 1);
  putU16(bytes, Abi.MANIFEST_PROFILE_OFFSET, 1);
  putU16(bytes, Abi.MANIFEST_COUNT_OFFSET, 1);
  const entry = Abi.MANIFEST_HEADER_BYTES;
  bytes.set(Abi.DIRECT_SUCCESSOR_KIND_ID_V3, entry + Abi.KIND_ID_OFFSET);
  bytes.set(identity(2), entry + Abi.RELEASE_ID_OFFSET);
  bytes.set(identity(3), entry + Abi.CONFIG_ID_OFFSET);
  bytes.set(identity(4), entry + Abi.CAPACITY_PROFILE_ID_OFFSET);
  bytes.set(Abi.DIRECT_ROOT_SCHEMA_ID_V1, entry + Abi.CHILD_SCHEMA_ID_OFFSET);
  bytes.set(identity(5), entry + Abi.CHILD_DERIVATION_ID_OFFSET);
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
  bytes.set(Abi.EFFECT_SCHEMA_RELEASE_ID_V4, Abi.CAPABILITY_PROGRAM_V4_EFFECT_SCHEMA_OFFSET);
  bytes.set(Abi.DIRECT_INLINE_ORDINARY_EFFECT_ID_V4, Abi.CAPABILITY_PROGRAM_V4_EFFECT_PROGRAM_OFFSET);
  putU32(bytes, Abi.CAPABILITY_PROGRAM_V4_ROOT_STATE_BYTES_OFFSET, Abi.DIRECT_ROOT_STATE_BYTES_V1);
  return bytes;
}

function programSetFixture(): Uint8Array {
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
  bytes.set(identity(12), entry + Abi.CAPABILITY_PROGRAM_SET_ENTRY_DESCRIPTOR_PROGRAM_OFFSET_V2);
  return bytes;
}

function signedRequestProfileFixture(): Uint8Array {
  const embeddedBytes = Abi.REQUEST_PROFILE_HEADER_BYTES_V1 + Abi.REQUEST_PROFILE_OPERATION_BYTES_V1;
  const bytes = new Uint8Array(Abi.REQUEST_PROFILE_V2_HEADER_BYTES + embeddedBytes
    + 2 * Abi.NATIVE_SIGNATURE_REQUIREMENT_BYTES_V1);
  bytes.set(Abi.REQUEST_PROFILE_V2_MAGIC);
  putU16(bytes, 8, Abi.REQUEST_PROFILE_V2_SCHEMA_VERSION);
  putU16(bytes, 10, Abi.REQUEST_PROFILE_V2_ARTIFACT_PROFILE);
  putU32(bytes, Abi.EMBEDDED_V1_BYTES_OFFSET, embeddedBytes);
  putU32(bytes, Abi.REQUIREMENT_COUNT_OFFSET, 2);
  const embedded = Abi.REQUEST_PROFILE_V2_HEADER_BYTES;
  bytes.set(Abi.REQUEST_PROFILE_MAGIC_V1, embedded);
  putU16(bytes, embedded + Abi.REQUEST_PROFILE_VERSION_OFFSET, Abi.REQUEST_PROFILE_SCHEMA_VERSION_V1);
  putU16(bytes, embedded + Abi.REQUEST_PROFILE_ARTIFACT_OFFSET, Abi.REQUEST_PROFILE_ARTIFACT_PROFILE_V1);
  putU32(bytes, embedded + Abi.REQUEST_PROFILE_FIXED_REQUEST_BYTES_OFFSET, Abi.DIRECT_INLINE_ORDINARY_REQUEST_BYTES_V3);
  putU16(bytes, embedded + Abi.REQUEST_PROFILE_FIXED_OPERATIONS_OFFSET, 1);
  putU16(bytes, embedded + Abi.REQUEST_PROFILE_COMMON_SCALARS_OFFSET, Abi.DIRECT_ORDINARY_COMMON_SCALARS_V3);
  putU16(bytes, embedded + Abi.REQUEST_PROFILE_COMMON_IDENTITIES_OFFSET, Abi.DIRECT_ORDINARY_COMMON_IDENTITIES_V3);
  // The register file is affine in the outcome count, so the emitter writes
  // both strides from named constants. Leaving them implicitly zero built a
  // profile no Market publishes.
  putU16(bytes, embedded + Abi.REQUEST_PROFILE_ITEM_SCALAR_STRIDE_OFFSET, Abi.DIRECT_ORDINARY_ITEM_SCALAR_STRIDE_V3);
  putU16(bytes, embedded + Abi.REQUEST_PROFILE_ITEM_IDENTITY_STRIDE_OFFSET, Abi.DIRECT_ORDINARY_ITEM_IDENTITY_STRIDE_V3);
  const operation = embedded + Abi.REQUEST_PROFILE_HEADER_BYTES_V1;
  bytes[operation + Abi.REQUEST_OPERATION_OPCODE_OFFSET] = 2;
  putU32(bytes, operation + Abi.REQUEST_OPERATION_REQUEST_OFFSET_OFFSET, 12);
  putU64(bytes, operation + Abi.REQUEST_OPERATION_IMMEDIATE_OFFSET, 1n);
  const requirements = embedded + embeddedBytes;
  putU16(bytes, requirements, Abi.DIRECT_NATIVE_EVIDENCE_SELLER_MESSAGE_OFFSET_V3);
  putU16(bytes, requirements + 2, Abi.COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V2);
  putU32(bytes, requirements + 4, Abi.IDENTITY_SELLER_NATIVE_SIGNER_V3);
  putU16(bytes, requirements + Abi.NATIVE_SIGNATURE_REQUIREMENT_BYTES_V1, Abi.DIRECT_NATIVE_EVIDENCE_BUYER_MESSAGE_OFFSET_V3);
  putU16(bytes, requirements + Abi.NATIVE_SIGNATURE_REQUIREMENT_BYTES_V1 + 2, Abi.COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V2);
  putU32(bytes, requirements + Abi.NATIVE_SIGNATURE_REQUIREMENT_BYTES_V1 + 4, Abi.IDENTITY_BUYER_NATIVE_SIGNER_V3);
  return bytes;
}

describe('Direct V3 chain-selected artifacts', () => {
  it('authenticates the exact seal PDA/body and refuses release, row, PDA, and rent substitutions', async () => {
    const fixture = await capabilitySealFixture();
    await expect(authenticateDirectCapabilitySealV1(
      fixture.client, fixture.accounts, fixture.fixed, fixture.trading, fixture.tradingRelease,
      fixture.registry, fixture.descriptorSchema, fixture.descriptorDigest, fixture.records,
    )).resolves.toMatch(/^[0-9a-f]{64}$/);

    const seal = fixture.accounts.get(fixture.sealAddress)!;
    const changedRow = new Uint8Array(seal!.data); changedRow[152 + 104] ^= 1;
    await expect(authenticateDirectCapabilitySealV1(
      fixture.client, new Map([[fixture.sealAddress, { ...seal!, data: changedRow }]]), fixture.fixed,
      fixture.trading, fixture.tradingRelease, fixture.registry, fixture.descriptorSchema, fixture.descriptorDigest, fixture.records,
    )).rejects.toThrow(/row 0 differs/);
    await expect(authenticateDirectCapabilitySealV1(
      fixture.client, fixture.accounts, fixture.fixed, fixture.trading, '53'.repeat(32),
      fixture.registry, fixture.descriptorSchema, fixture.descriptorDigest, fixture.records,
    )).rejects.toThrow(/another descriptor, Trading release, or Registry/);
    const wrongPda = fixture.fixed.slice(); wrongPda[38] = { ...wrongPda[38]!, address: new PublicKey(identity(250)).toBase58() };
    await expect(authenticateDirectCapabilitySealV1(
      fixture.client, new Map([[wrongPda[38]!.address, seal]]), wrongPda, fixture.trading, fixture.tradingRelease,
      fixture.registry, fixture.descriptorSchema, fixture.descriptorDigest, fixture.records,
    )).rejects.toThrow(/canonical Trading PDA/);
    await expect(authenticateDirectCapabilitySealV1(
      { minimumBalanceForRentExemption: async (dataLength: number) => Object.freeze({ dataLength, lamports: '101' }) },
      fixture.accounts, fixture.fixed, fixture.trading, fixture.tradingRelease, fixture.registry,
      fixture.descriptorSchema, fixture.descriptorDigest, fixture.records,
    )).rejects.toThrow(/below its exact rent minimum/);
  });

  it('admits decision-0012 mutable deployments at their exact pins and refuses slot drift', async () => {
    const trading = await mutableDeployment(20);
    const core = await mutableDeployment(21);
    await expect(authenticateDirectHotOuterDeploymentsV3(
      trading.programAddress,
      core.programAddress,
      trading,
      core,
    )).resolves.toBeUndefined();

    const upgraded = await mutableDeployment(20, 82n);
    await expect(authenticateDirectHotOuterDeploymentsV3(
      trading.programAddress,
      core.programAddress,
      upgraded,
      core,
    )).rejects.toThrow(/ReleaseSupersededByUpgrade.*slot 81.*slot 82/);

    await expect(authenticateDirectHotOuterDeploymentsV3(
      core.programAddress,
      trading.programAddress,
      trading,
      core,
    )).rejects.toThrow(/another Core or Trading program/);
  });

  it('joins the immutable root selection to one exact manifest entry', () => {
    const selection = decodeDirectRootSelectionV1(rootFixture());
    const selected = decodeSelectedDirectManifestEntryV1(manifestFixture(), selection);
    expect(selected.programSet).toEqual(identity(2));
    expect(selected.capacityProfile).toEqual(identity(4));

    const substituted = manifestFixture();
    substituted.set(identity(10), Abi.MANIFEST_HEADER_BYTES + Abi.RELEASE_ID_OFFSET);
    expect(() => decodeSelectedDirectManifestEntryV1(substituted, selection)).toThrow(/differs/);
  });

  it('selects one schema-bound V4 descriptor and rejects V1-shaped authority', () => {
    const selected = decodeDirectProgramSetV2(programSetFixture());
    expect(selected.schema).toEqual(Abi.CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID);
    expect(selected.program).toEqual(identity(12));
    const schema = programSetFixture();
    schema.set(identity(13), Abi.CAPABILITY_PROGRAM_SET_HEADER_BYTES_V2
      + Abi.CAPABILITY_PROGRAM_SET_ENTRY_DESCRIPTOR_SCHEMA_OFFSET_V2);
    expect(decodeDirectProgramSetV2(schema).schema).toEqual(identity(13));
    const legacyMagic = programSetFixture();
    legacyMagic[7] = 1;
    expect(() => decodeDirectProgramSetV2(legacyMagic)).toThrow(/wrong exact header/);
    const reserved = programSetFixture();
    reserved[Abi.CAPABILITY_PROGRAM_SET_HEADER_BYTES_V2 + Abi.CAPABILITY_PROGRAM_SET_ENTRY_RESERVED_OFFSET_V2] = 1;
    expect(() => decodeDirectProgramSetV2(reserved)).toThrow(/reserved/);
  });

  it('hostile-decodes the schema-bound V4 successor descriptor', () => {
    const decoded = decodeDirectDescriptorV4(descriptorFixture());
    // These assert the FIXTURE round-trips the program ids it was built from,
    // not that the authenticator pins them: Rust binds artifact programs by
    // content (`require_content`), never by equality to a publisher id.
    expect(decoded.lifecycle.program).toEqual(Abi.DIRECT_INLINE_ORDINARY_LIFECYCLE_ID_V5);
    expect(decoded.strategy.program).toEqual(Abi.DIRECT_INLINE_ORDINARY_STRATEGY_ID_V3);
    expect(decoded.transition.program).toEqual(Abi.DIRECT_INLINE_ORDINARY_TRANSITION_ID_V3);

    const width = descriptorFixture();
    putU32(width, Abi.CAPABILITY_PROGRAM_V4_ROOT_STATE_BYTES_OFFSET, Abi.DIRECT_ROOT_STATE_BYTES_V1 + 1);
    expect(() => decodeDirectDescriptorV4(width)).toThrow(/root-tail width/);

    // A substituted SCHEMA stays refused, and the refusal now localizes itself:
    // it names the one disagreeing field and both values.
    const strategy = descriptorFixture();
    strategy.set(identity(11), Abi.CAPABILITY_PROGRAM_V4_STRATEGY_SCHEMA_OFFSET);
    expect(() => decodeDirectDescriptorV4(strategy)).toThrow(/schema-bound/);
    expect(() => decodeDirectDescriptorV4(strategy)).toThrow(/Strategy schema/);
    expect(() => decodeDirectDescriptorV4(strategy)).toThrow(new RegExp(hex(identity(11))));
    expect(() => decodeDirectDescriptorV4(strategy))
      .toThrow(new RegExp(hex(Abi.EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2)));

    // A republished artifact PROGRAM id is accepted: the chain accepts any
    // content the descriptor's own digest names, and so does this client
    // (integrity lives at the fetch, where the bytes are hashed). This is the
    // fluidity proof that the pinned-mirror refusal class is retired.
    const republishedAccountProfile = descriptorFixture();
    republishedAccountProfile.set(identity(12), Abi.CAPABILITY_PROGRAM_V4_ACCOUNT_PROFILE_PROGRAM_OFFSET);
    expect(decodeDirectDescriptorV4(republishedAccountProfile).accountProfile.program)
      .toEqual(identity(12));

    const parallelLifecycle = descriptorFixture();
    parallelLifecycle.set(identity(11), Abi.CAPABILITY_PROGRAM_V4_DERIVATION_POLICY_OFFSET);
    expect(() => decodeDirectDescriptorV4(parallelLifecycle)).toThrow(/derivation policy is not its own Lifecycle program/);
  });

  it('validates the embedded request interpreter and both distinct native-signature destinations', () => {
    expect(() => validateDirectSignedRequestProfileV2(signedRequestProfileFixture())).not.toThrow();
    const offset = signedRequestProfileFixture();
    const embeddedBytes = Abi.REQUEST_PROFILE_HEADER_BYTES_V1 + Abi.REQUEST_PROFILE_OPERATION_BYTES_V1;
    const requirements = Abi.REQUEST_PROFILE_V2_HEADER_BYTES + embeddedBytes;
    putU16(offset, requirements, Abi.DIRECT_NATIVE_EVIDENCE_SELLER_MESSAGE_OFFSET_V3 - 1);
    expect(() => validateDirectSignedRequestProfileV2(offset)).toThrow(/authenticated Trading-instruction message/);

    const alias = signedRequestProfileFixture();
    putU32(alias, requirements + Abi.NATIVE_SIGNATURE_REQUIREMENT_BYTES_V1 + 4, Abi.IDENTITY_SELLER_NATIVE_SIGNER_V3);
    expect(() => validateDirectSignedRequestProfileV2(alias)).toThrow(/invalid or aliased/);

    const reserved = signedRequestProfileFixture();
    reserved[Abi.REQUEST_PROFILE_V2_HEADER_BYTES + Abi.REQUEST_PROFILE_HEADER_BYTES_V1
      + Abi.REQUEST_OPERATION_RESERVED_BYTE_OFFSET] = 1;
    expect(() => validateDirectSignedRequestProfileV2(reserved)).toThrow(/reserved/);
  });

  it('hostile-decodes the Hot38 Product-basis continuation and semantic join', async () => {
    const fixture = await categoricalBasisFixture();
    // The scale comes back now rather than being computed and dropped. A
    // categorical basis is canonical Q=1 BY REFUSAL -- the validator throws on
    // any other value -- so this pins the one case where 1 is a fact and not an
    // assumption, and every consumer that wants the number has to ask for it.
    await expect(validateProductBasisV3(fixture.basis, identity(1), identity(2), fixture.domain))
      .resolves.toEqual({ basisWidth: 3, payoutScale: 1n, kind: 1 });

    const substituted = fixture.basis.slice();
    substituted.set(identity(9), 32);
    await expect(validateProductBasisV3(substituted, identity(1), identity(2), fixture.domain)).rejects.toThrow(/does not join/);

    const noncanonical = fixture.basis.slice();
    noncanonical[208] = 1;
    await expect(validateProductBasisV3(noncanonical, identity(1), identity(2), fixture.domain)).rejects.toThrow(/reserved/);
  });
});
