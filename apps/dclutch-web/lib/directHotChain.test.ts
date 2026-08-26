import { describe, expect, it } from 'vitest';

import {
  decodeDirectDescriptorV4,
  decodeDirectProgramSetV2,
  decodeDirectRootSelectionV1,
  decodeSelectedDirectManifestEntryV1,
  validateProductBasisV3,
  validateDirectSignedRequestProfileV2,
} from './directHotChain';
import { sha256 } from './bytes';
import * as Abi from './generated/directInlineV3';

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
  bytes.set(Abi.EFFECT_SCHEMA_RELEASE_ID, Abi.CAPABILITY_PROGRAM_V4_EFFECT_SCHEMA_OFFSET);
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
  const operation = embedded + Abi.REQUEST_PROFILE_HEADER_BYTES_V1;
  bytes[operation + Abi.REQUEST_OPERATION_OPCODE_OFFSET] = 2;
  putU32(bytes, operation + Abi.REQUEST_OPERATION_REQUEST_OFFSET_OFFSET, 12);
  putU64(bytes, operation + Abi.REQUEST_OPERATION_IMMEDIATE_OFFSET, 1n);
  const requirements = embedded + embeddedBytes;
  putU16(bytes, requirements, 192);
  putU16(bytes, requirements + 2, 172);
  putU32(bytes, requirements + 4, Abi.IDENTITY_SELLER_NATIVE_SIGNER_V3);
  putU16(bytes, requirements + Abi.NATIVE_SIGNATURE_REQUIREMENT_BYTES_V1, 396);
  putU16(bytes, requirements + Abi.NATIVE_SIGNATURE_REQUIREMENT_BYTES_V1 + 2, 172);
  putU32(bytes, requirements + Abi.NATIVE_SIGNATURE_REQUIREMENT_BYTES_V1 + 4, Abi.IDENTITY_BUYER_NATIVE_SIGNER_V3);
  return bytes;
}

describe('Direct V3 chain-selected artifacts', () => {
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
    expect(decoded.lifecycle.program).toEqual(Abi.DIRECT_INLINE_ORDINARY_LIFECYCLE_ID_V5);
    expect(decoded.strategy.program).toEqual(Abi.DIRECT_INLINE_ORDINARY_STRATEGY_ID_V3);
    expect(decoded.transition.program).toEqual(Abi.DIRECT_INLINE_ORDINARY_TRANSITION_ID_V3);

    const width = descriptorFixture();
    putU32(width, Abi.CAPABILITY_PROGRAM_V4_ROOT_STATE_BYTES_OFFSET, Abi.DIRECT_ROOT_STATE_BYTES_V1 + 1);
    expect(() => decodeDirectDescriptorV4(width)).toThrow(/root-tail width/);
    const strategy = descriptorFixture();
    strategy.set(identity(11), Abi.CAPABILITY_PROGRAM_V4_STRATEGY_SCHEMA_OFFSET);
    expect(() => decodeDirectDescriptorV4(strategy)).toThrow(/schema-bound/);
    const parallelLifecycle = descriptorFixture();
    parallelLifecycle.set(identity(11), Abi.CAPABILITY_PROGRAM_V4_DERIVATION_POLICY_OFFSET);
    expect(() => decodeDirectDescriptorV4(parallelLifecycle)).toThrow(/schema-bound/);
  });

  it('validates the embedded request interpreter and both distinct native-signature destinations', () => {
    expect(() => validateDirectSignedRequestProfileV2(signedRequestProfileFixture())).not.toThrow();
    const offset = signedRequestProfileFixture();
    const embeddedBytes = Abi.REQUEST_PROFILE_HEADER_BYTES_V1 + Abi.REQUEST_PROFILE_OPERATION_BYTES_V1;
    const requirements = Abi.REQUEST_PROFILE_V2_HEADER_BYTES + embeddedBytes;
    putU16(offset, requirements, 191);
    expect(() => validateDirectSignedRequestProfileV2(offset)).toThrow(/current-instruction message/);

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
    await expect(validateProductBasisV3(fixture.basis, identity(1), identity(2), fixture.domain)).resolves.toBe(3);

    const substituted = fixture.basis.slice();
    substituted.set(identity(9), 32);
    await expect(validateProductBasisV3(substituted, identity(1), identity(2), fixture.domain)).rejects.toThrow(/does not join/);

    const noncanonical = fixture.basis.slice();
    noncanonical[208] = 1;
    await expect(validateProductBasisV3(noncanonical, identity(1), identity(2), fixture.domain)).rejects.toThrow(/reserved/);
  });
});
