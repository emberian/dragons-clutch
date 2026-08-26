import { describe, expect, it } from 'vitest';

import {
  decodeDirectDescriptorV3,
  decodeDirectRootSelectionV1,
  decodeSelectedDirectManifestEntryV1,
  validateDirectSignedRequestProfileV2,
} from './directHotChain';
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
  const bytes = new Uint8Array(Abi.CAPABILITY_PROGRAM_V3_BYTES);
  bytes.set(Abi.CAPABILITY_PROGRAM_V3_MAGIC);
  putU16(bytes, Abi.CAPABILITY_PROGRAM_V3_SCHEMA_VERSION_OFFSET, Abi.CAPABILITY_PROGRAM_V3_SCHEMA_VERSION);
  putU16(bytes, Abi.CAPABILITY_PROGRAM_V3_ARTIFACT_PROFILE_OFFSET, Abi.CAPABILITY_PROGRAM_V3_ARTIFACT_PROFILE);
  putU16(bytes, Abi.CAPABILITY_PROGRAM_V3_TRANSITION_SCHEMA_VERSION_OFFSET, Abi.CAPABILITY_PROGRAM_V3_TRANSITION_SCHEMA_VERSION);
  putU16(bytes, Abi.CAPABILITY_PROGRAM_V3_REQUEST_PROFILE_SCHEMA_VERSION_OFFSET, Abi.CAPABILITY_PROGRAM_V3_REQUEST_PROFILE_SCHEMA_VERSION);
  bytes.set(Abi.DIRECT_SUCCESSOR_KIND_ID_V3, Abi.CAPABILITY_PROGRAM_V3_KIND_OFFSET);
  bytes.set(Abi.DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1, Abi.CAPABILITY_PROGRAM_V3_CONFIG_SCHEMA_OFFSET);
  bytes.set(Abi.DIRECT_EXECUTION_REQUEST_SCHEMA_ID_V3, Abi.CAPABILITY_PROGRAM_V3_REQUEST_SCHEMA_OFFSET);
  bytes.set(Abi.DIRECT_ROOT_SCHEMA_ID_V1, Abi.CAPABILITY_PROGRAM_V3_ROOT_SCHEMA_OFFSET);
  bytes.set(identity(6), Abi.CAPABILITY_PROGRAM_V3_ACCOUNT_PROFILE_OFFSET);
  bytes.set(identity(5), Abi.CAPABILITY_PROGRAM_V3_DERIVATION_POLICY_OFFSET);
  bytes.set(identity(4), Abi.CAPABILITY_PROGRAM_V3_CAPACITY_PROFILE_OFFSET);
  bytes.set(identity(7), Abi.CAPABILITY_PROGRAM_V3_EFFECT_PROGRAM_OFFSET);
  bytes.set(Abi.REQUEST_PROFILE_V2_SCHEMA_RELEASE_ID, Abi.CAPABILITY_PROGRAM_V3_REQUEST_PROFILE_SCHEMA_OFFSET);
  bytes.set(identity(8), Abi.CAPABILITY_PROGRAM_V3_REQUEST_PROFILE_PROGRAM_OFFSET);
  bytes.set(Abi.EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2, Abi.CAPABILITY_PROGRAM_V3_TRANSITION_SCHEMA_OFFSET);
  bytes.set(identity(9), Abi.CAPABILITY_PROGRAM_V3_TRANSITION_PROGRAM_OFFSET);
  putU32(bytes, Abi.CAPABILITY_PROGRAM_V3_ROOT_STATE_BYTES_OFFSET, Abi.DIRECT_ROOT_STATE_BYTES_V1);
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
  putU16(bytes, embedded + Abi.REQUEST_PROFILE_COMMON_IDENTITIES_OFFSET, 2);
  const operation = embedded + Abi.REQUEST_PROFILE_HEADER_BYTES_V1;
  bytes[operation + Abi.REQUEST_OPERATION_OPCODE_OFFSET] = 2;
  putU32(bytes, operation + Abi.REQUEST_OPERATION_REQUEST_OFFSET_OFFSET, 12);
  putU64(bytes, operation + Abi.REQUEST_OPERATION_IMMEDIATE_OFFSET, 1n);
  const requirements = embedded + embeddedBytes;
  putU16(bytes, requirements, 192);
  putU16(bytes, requirements + 2, 172);
  putU32(bytes, requirements + 4, 0);
  putU16(bytes, requirements + Abi.NATIVE_SIGNATURE_REQUIREMENT_BYTES_V1, 396);
  putU16(bytes, requirements + Abi.NATIVE_SIGNATURE_REQUIREMENT_BYTES_V1 + 2, 172);
  putU32(bytes, requirements + Abi.NATIVE_SIGNATURE_REQUIREMENT_BYTES_V1 + 4, 1);
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

  it('hostile-decodes the successor descriptor without a legacy transition shortcut', () => {
    const decoded = decodeDirectDescriptorV3(descriptorFixture());
    expect(decoded.lifecycle).toEqual(identity(5));
    expect(decoded.strategyProgram).toEqual(identity(9));

    const width = descriptorFixture();
    putU32(width, Abi.CAPABILITY_PROGRAM_V3_ROOT_STATE_BYTES_OFFSET, Abi.DIRECT_ROOT_STATE_BYTES_V1 + 1);
    expect(() => decodeDirectDescriptorV3(width)).toThrow(/root-tail width/);
    const strategy = descriptorFixture();
    strategy.set(identity(11), Abi.CAPABILITY_PROGRAM_V3_TRANSITION_SCHEMA_OFFSET);
    expect(() => decodeDirectDescriptorV3(strategy)).toThrow(/Strategy V2/);
  });

  it('validates the embedded request interpreter and both distinct native-signature destinations', () => {
    expect(() => validateDirectSignedRequestProfileV2(signedRequestProfileFixture())).not.toThrow();
    const offset = signedRequestProfileFixture();
    const embeddedBytes = Abi.REQUEST_PROFILE_HEADER_BYTES_V1 + Abi.REQUEST_PROFILE_OPERATION_BYTES_V1;
    const requirements = Abi.REQUEST_PROFILE_V2_HEADER_BYTES + embeddedBytes;
    putU16(offset, requirements, 191);
    expect(() => validateDirectSignedRequestProfileV2(offset)).toThrow(/current-instruction message/);

    const alias = signedRequestProfileFixture();
    putU32(alias, requirements + Abi.NATIVE_SIGNATURE_REQUIREMENT_BYTES_V1 + 4, 0);
    expect(() => validateDirectSignedRequestProfileV2(alias)).toThrow(/invalid or aliased/);

    const reserved = signedRequestProfileFixture();
    reserved[Abi.REQUEST_PROFILE_V2_HEADER_BYTES + Abi.REQUEST_PROFILE_HEADER_BYTES_V1
      + Abi.REQUEST_OPERATION_RESERVED_BYTE_OFFSET] = 1;
    expect(() => validateDirectSignedRequestProfileV2(reserved)).toThrow(/reserved/);
  });
});
