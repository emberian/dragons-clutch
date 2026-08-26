import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { sha256 } from './bytes';
import {
  ARTIFACT_RELEASE_BYTES,
  CHECKED_MULTIPROGRAM_BYTES,
  REGISTRY_ROLES,
  UPGRADEABLE_LOADER_ID,
  compileRegistryActivationTransaction,
  compileRegistryReauthenticationTransaction,
  decodeCheckedMultiprogramV1,
  decodeCheckedReleaseV1,
  type RegistryRole,
  type RegistryRoleAddressesV1,
} from './releaseRegistry';

function publicKey(seed: number): string { return new PublicKey(Uint8Array.from({ length: 32 }, () => seed)).toBase58(); }
function putU64(output: Uint8Array, offset: number, value: bigint): void { new DataView(output.buffer).setBigUint64(offset, value, true); }
function text(value: string): Uint8Array { const bytes = new TextEncoder().encode(value); const output = new Uint8Array(2 + bytes.length); new DataView(output.buffer).setUint16(0, bytes.length, true); output.set(bytes, 2); return output; }

async function checkedFixture(program: string): Promise<Readonly<{ checked: Uint8Array; artifact: Uint8Array; programData: string }>> {
  const loader = new PublicKey(UPGRADEABLE_LOADER_ID); const programKey = new PublicKey(program); const programData = PublicKey.findProgramAddressSync([programKey.toBytes()], loader)[0];
  const programBytes = new Uint8Array(36); new DataView(programBytes.buffer).setUint32(0, 2, true); programBytes.set(programData.toBytes(), 4);
  const programDataBytes = new Uint8Array(109); new DataView(programDataBytes.buffer).setUint32(0, 3, true); putU64(programDataBytes, 4, 81n); programDataBytes[12] = 1; programDataBytes.set(new PublicKey(publicKey(93)).toBytes(), 13); programDataBytes.fill(0x5a, 45);
  const metadata = ['revision-1', 'rustc 1', 'solana 1', 'cargo-build-sbf 1', 'sbf-solana-solana', 'cargo build-sbf', 'offline-check'].map(text); const manifestLength = 388 + metadata.reduce((sum, value) => sum + value.length, 0);
  const checked = new Uint8Array(manifestLength); checked.set(new TextEncoder().encode('DCLTREL1')); const view = new DataView(checked.buffer); view.setUint16(8, 1, true); checked[10] = 0; checked[11] = 1; checked[12] = 1; checked[13] = 1; view.setUint32(16, manifestLength, true);
  putU64(checked, 20, 16n); putU64(checked, 28, 64n); putU64(checked, 36, 36n); putU64(checked, 44, 109n); putU64(checked, 52, 81n); putU64(checked, 60, 45n);
  checked.set(await sha256(programDataBytes.slice(45)), 68); checked.fill(7, 100, 132); checked.set(await sha256(programBytes), 132); checked.set(await sha256(programDataBytes), 164); checked.set(programKey.toBytes(), 196); checked.set(programData.toBytes(), 228); checked.set(loader.toBytes(), 260); checked.set(new PublicKey(publicKey(93)).toBytes(), 292); checked.fill(8, 324, 356); checked.fill(9, 356, 388);
  let offset = 388; for (const value of metadata) { checked.set(value, offset); offset += value.length; }
  const decoded = await decodeCheckedReleaseV1(checked); return Object.freeze({ checked, artifact: decoded.artifact.bytes, programData: programData.toBase58() });
}

async function evidenceFixture(): Promise<Readonly<{ multiprogram: Uint8Array; checked: Readonly<Record<RegistryRole, Uint8Array>>; registry: string; artifactId: Uint8Array; programData: string }>> {
  const registry = publicKey(11); const fixture = await checkedFixture(registry); const artifactId = await sha256(fixture.artifact);
  const release = new Uint8Array(336); release.set(new TextEncoder().encode('DCLTRLS1')); const view = new DataView(release.buffer); view.setUint16(8, 1, true); view.setUint16(10, 1, true);
  REGISTRY_ROLES.forEach((_, index) => { release.set(new PublicKey(registry).toBytes(), 16 + index * 64); release.set(artifactId, 48 + index * 64); });
  const checkedId = await sha256(fixture.checked); const multiprogram = new Uint8Array(CHECKED_MULTIPROGRAM_BYTES); multiprogram.set(new TextEncoder().encode('DCLTMPR1')); const manifestView = new DataView(multiprogram.buffer); manifestView.setUint16(8, 1, true); manifestView.setUint16(10, 5, true); multiprogram.set(release, 16);
  REGISTRY_ROLES.forEach((_, index) => { const offset = 352 + index * (ARTIFACT_RELEASE_BYTES + 32); multiprogram.set(fixture.artifact, offset); multiprogram.set(checkedId, offset + ARTIFACT_RELEASE_BYTES); });
  return Object.freeze({ multiprogram, checked: Object.freeze(Object.fromEntries(REGISTRY_ROLES.map((role) => [role, fixture.checked])) as Record<RegistryRole, Uint8Array>), registry, artifactId, programData: fixture.programData });
}

describe('checked Registry release workspace', () => {
  it('joins five full manifests to the fixed multiprogram authority', async () => {
    const fixture = await evidenceFixture(); const decoded = await decodeCheckedMultiprogramV1(fixture.multiprogram, fixture.checked);
    expect(decoded.releaseSet.roles.core.program).toBe(fixture.registry); expect(decoded.releaseSet.roles.custody.artifactReleaseId).toBe(Array.from(fixture.artifactId, (byte) => byte.toString(16).padStart(2, '0')).join(''));
    expect(decoded.artifacts.trading.programData).toBe(fixture.programData); expect(decoded.checkedReleaseIds.core).toBe(decoded.checkedReleaseIds.custody);
  });

  it('refuses checked-release substitution and noncanonical manifest bytes', async () => {
    const fixture = await evidenceFixture(); const substituted = { ...fixture.checked, trading: new Uint8Array(fixture.checked.trading) }; substituted.trading[substituted.trading.length - 1] ^= 1;
    await expect(decodeCheckedMultiprogramV1(fixture.multiprogram, substituted)).rejects.toThrow();
    const reserved = new Uint8Array(fixture.multiprogram); reserved[12] = 1; await expect(decodeCheckedMultiprogramV1(reserved, fixture.checked)).rejects.toThrow('reserved');
  });

  it('emits the committed aliased activation and reauthentication packet geometry', async () => {
    const fixture = await evidenceFixture(); const record = publicKey(21); const staging = publicKey(22); const programData = fixture.programData;
    const roleAddresses = Object.freeze(Object.fromEntries(REGISTRY_ROLES.map((role) => [role, Object.freeze({ record, staging, program: fixture.registry, programData })])) as Record<RegistryRole, RegistryRoleAddressesV1>);
    const activation = compileRegistryActivationTransaction({ payer: publicKey(31), registryProgram: fixture.registry, recentBlockhash: publicKey(32), computeUnitLimit: 400_000, cache: publicKey(33), releaseSetRecord: publicKey(34), releaseSetStaging: publicKey(35), roles: roleAddresses });
    expect(activation.wireBytes).toHaveLength(509); expect(activation.requiredSigners).toEqual([publicKey(31)]); expect(activation.transaction.message.compiledInstructions[1].accountKeyIndexes).toHaveLength(26);
    const reauthentication = compileRegistryReauthenticationTransaction({ payer: publicKey(31), registryProgram: fixture.registry, recentBlockhash: publicKey(32), computeUnitLimit: 80_000, cache: publicKey(33), role: 'trading', program: fixture.registry, programData });
    expect(reauthentication.wireBytes).toHaveLength(294); expect(reauthentication.transaction.message.compiledInstructions[1].accountKeyIndexes).toHaveLength(3); expect(Array.from(reauthentication.transaction.message.compiledInstructions[1].data.slice(10, 12))).toEqual([1, 2]);
  });
});
