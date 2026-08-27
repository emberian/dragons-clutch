import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { sha256 } from './bytes';
import {
  ACTIVATION_CACHE_BYTES,
  ARTIFACT_RELEASE_BYTES,
  ARTIFACT_RELEASE_SCHEMA_ID_V1,
  CHECKED_MULTIPROGRAM_BYTES,
  EXECUTION_RELEASE_SET_BYTES,
  EXECUTION_RELEASE_SET_SCHEMA_ID_V1,
  REGISTRY_ACTIVATE_ROLE_ACCOUNT_COUNT,
  REGISTRY_ACTIVATED_ROLE_BYTES,
  REGISTRY_ACTIVATION_CACHE_ROLES_OFFSET,
  REGISTRY_REAUTH_ACCOUNT_COUNT,
  REGISTRY_ROLES,
  RENT_SYSVAR_ID,
  SYSTEM_PROGRAM_ID,
  SYSVAR_OWNER_ID,
  UPGRADEABLE_LOADER_ID,
  activationCacheProgressV1,
  compileRegistryReauthenticationTransaction,
  compileRegistryRoleActivationTransaction,
  decodeCheckedMultiprogramV1,
  decodeCheckedReleaseV1,
  deriveFinalizedRecordAddressesV1,
  prepareRegistryActivation,
  prepareRegistryReauthentication,
  type RegistryRole,
} from './releaseRegistry';
import { type RpcAccount } from './rpc';

const NATIVE_LOADER_ID = 'NativeLoader1111111111111111111111111111111';
const ACTIVATION_PACKET_BYTES = 525;
const REAUTH_PACKET_BYTES = 326;
const ACTIVATION_DOMAIN = new TextEncoder().encode('dclutch:release-activation:v1');

// Seven DISTINCT programs, exactly as an honest dClutch release set carries
// them. Registry and Rent are not execution roles; the five roles are.
const REGISTRY_PROGRAM_SEED = 11;
const RENT_PROGRAM_SEED = 12;
const ROLE_PROGRAM_SEEDS: Readonly<Record<RegistryRole, number>> = Object.freeze({
  core: 21, claims: 22, trading: 23, resolution: 24, custody: 25,
});

function publicKey(seed: number): string { return new PublicKey(Uint8Array.from({ length: 32 }, () => seed)).toBase58(); }
function putU64(output: Uint8Array, offset: number, value: bigint): void { new DataView(output.buffer).setBigUint64(offset, value, true); }
function text(value: string): Uint8Array { const bytes = new TextEncoder().encode(value); const output = new Uint8Array(2 + bytes.length); new DataView(output.buffer).setUint16(0, bytes.length, true); output.set(bytes, 2); return output; }
function account(owner: string, executable: boolean, data: Uint8Array, lamports = '1'): RpcAccount {
  return Object.freeze({ owner, executable, data: new Uint8Array(data), lamports, space: data.length });
}

type RoleFixture = Readonly<{
  program: string;
  programData: string;
  checked: Uint8Array;
  artifact: Uint8Array;
  artifactId: Uint8Array;
  checkedId: Uint8Array;
  programAccount: RpcAccount;
  programDataAccount: RpcAccount;
}>;

/// One role's complete checked release plus the exact Loader-v3 accounts it
/// claims, all derived from one seed so no two roles share a program, a
/// ProgramData, or an ELF digest.
async function roleFixture(seed: number): Promise<RoleFixture> {
  const loader = new PublicKey(UPGRADEABLE_LOADER_ID); const programKey = new PublicKey(publicKey(seed));
  const programData = PublicKey.findProgramAddressSync([programKey.toBytes()], loader)[0];
  const programBytes = new Uint8Array(36); new DataView(programBytes.buffer).setUint32(0, 2, true); programBytes.set(programData.toBytes(), 4);
  const programDataBytes = new Uint8Array(109); new DataView(programDataBytes.buffer).setUint32(0, 3, true); putU64(programDataBytes, 4, 81n);
  programDataBytes[12] = 1; programDataBytes.set(new PublicKey(publicKey(93)).toBytes(), 13); programDataBytes.fill(seed, 45);
  const elf = programDataBytes.slice(45);
  const metadata = ['revision-1', 'rustc 1', 'solana 1', 'cargo-build-sbf 1', 'sbf-solana-solana', 'cargo build-sbf', 'offline-check'].map(text);
  const manifestLength = 388 + metadata.reduce((sum, value) => sum + value.length, 0);
  const checked = new Uint8Array(manifestLength); checked.set(new TextEncoder().encode('DCLTREL1')); const view = new DataView(checked.buffer);
  view.setUint16(8, 1, true); checked[10] = 0; checked[11] = 1; checked[12] = 1; checked[13] = 1; view.setUint32(16, manifestLength, true);
  putU64(checked, 20, 16n); putU64(checked, 28, 64n); putU64(checked, 36, 36n); putU64(checked, 44, 109n); putU64(checked, 52, 81n); putU64(checked, 60, 45n);
  checked.set(await sha256(elf), 68); checked.fill(seed + 100, 100, 132); checked.set(await sha256(programBytes), 132); checked.set(await sha256(programDataBytes), 164);
  checked.set(programKey.toBytes(), 196); checked.set(programData.toBytes(), 228); checked.set(loader.toBytes(), 260);
  checked.set(new PublicKey(publicKey(93)).toBytes(), 292); checked.fill(8, 324, 356); checked.fill(9, 356, 388);
  let offset = 388; for (const value of metadata) { checked.set(value, offset); offset += value.length; }
  const decoded = await decodeCheckedReleaseV1(checked);
  return Object.freeze({
    program: programKey.toBase58(), programData: programData.toBase58(), checked, artifact: decoded.artifact.bytes,
    artifactId: await sha256(decoded.artifact.bytes), checkedId: await sha256(checked),
    programAccount: account(UPGRADEABLE_LOADER_ID, true, programBytes), programDataAccount: account(UPGRADEABLE_LOADER_ID, false, programDataBytes),
  });
}

type Fixture = Readonly<{
  registry: string;
  rentProgram: string;
  payer: string;
  roles: Readonly<Record<RegistryRole, RoleFixture>>;
  multiprogram: Uint8Array;
  checked: Readonly<Record<RegistryRole, Uint8Array>>;
  releaseSet: Uint8Array;
  releaseSetId: Uint8Array;
  cache: string;
  expectedCache: Uint8Array;
  accounts: Map<string, RpcAccount>;
}>;

/// The activation cache layout, restated independently of the library so the
/// test is not a mirror of the code it checks.
function expectedCacheBytes(releaseSetId: Uint8Array, roles: Readonly<Record<RegistryRole, RoleFixture>>): Uint8Array {
  const output = new Uint8Array(ACTIVATION_CACHE_BYTES); output.set(new TextEncoder().encode('DCLTACT1'));
  const view = new DataView(output.buffer); view.setUint16(8, 1, true); view.setUint16(10, 1, true); output.set(releaseSetId, 16);
  REGISTRY_ROLES.forEach((role, index) => {
    const offset = REGISTRY_ACTIVATION_CACHE_ROLES_OFFSET + index * REGISTRY_ACTIVATED_ROLE_BYTES;
    output.set(roles[role].artifactId, offset); output.set(roles[role].artifact, offset + 32);
  });
  return output;
}

async function sevenProgramFixture(): Promise<Fixture> {
  const registry = publicKey(REGISTRY_PROGRAM_SEED); const rentProgram = publicKey(RENT_PROGRAM_SEED); const payer = publicKey(31);
  const built = await Promise.all(REGISTRY_ROLES.map((role) => roleFixture(ROLE_PROGRAM_SEEDS[role])));
  const roles = Object.freeze(Object.fromEntries(REGISTRY_ROLES.map((role, index) => [role, built[index]])) as Record<RegistryRole, RoleFixture>);

  const releaseSet = new Uint8Array(EXECUTION_RELEASE_SET_BYTES); releaseSet.set(new TextEncoder().encode('DCLTRLS1'));
  const releaseView = new DataView(releaseSet.buffer); releaseView.setUint16(8, 1, true); releaseView.setUint16(10, 1, true);
  REGISTRY_ROLES.forEach((role, index) => { releaseSet.set(new PublicKey(roles[role].program).toBytes(), 16 + index * 64); releaseSet.set(roles[role].artifactId, 48 + index * 64); });
  const releaseSetId = await sha256(releaseSet);

  const multiprogram = new Uint8Array(CHECKED_MULTIPROGRAM_BYTES); multiprogram.set(new TextEncoder().encode('DCLTMPR1'));
  const manifestView = new DataView(multiprogram.buffer); manifestView.setUint16(8, 1, true); manifestView.setUint16(10, 5, true); multiprogram.set(releaseSet, 16);
  REGISTRY_ROLES.forEach((role, index) => {
    const offset = 352 + index * (ARTIFACT_RELEASE_BYTES + 32);
    multiprogram.set(roles[role].artifact, offset); multiprogram.set(roles[role].checkedId, offset + ARTIFACT_RELEASE_BYTES);
  });

  const registryProgramBytes = new Uint8Array(36); new DataView(registryProgramBytes.buffer).setUint32(0, 2, true);
  registryProgramBytes.set(PublicKey.findProgramAddressSync([new PublicKey(registry).toBytes()], new PublicKey(UPGRADEABLE_LOADER_ID))[0].toBytes(), 4);
  const rentSysvar = new Uint8Array(17);

  const accounts = new Map<string, RpcAccount>([
    [payer, account(SYSTEM_PROGRAM_ID, false, new Uint8Array(0), '1000000000')],
    [registry, account(UPGRADEABLE_LOADER_ID, true, registryProgramBytes)],
    [SYSTEM_PROGRAM_ID, account(NATIVE_LOADER_ID, true, new Uint8Array(0))],
    [RENT_SYSVAR_ID, account(SYSVAR_OWNER_ID, false, rentSysvar)],
  ]);
  const releasePdas = deriveFinalizedRecordAddressesV1(registry, EXECUTION_RELEASE_SET_SCHEMA_ID_V1, releaseSetId);
  accounts.set(releasePdas.record, account(registry, false, releaseSet));
  for (const role of REGISTRY_ROLES) {
    const pdas = deriveFinalizedRecordAddressesV1(registry, ARTIFACT_RELEASE_SCHEMA_ID_V1, roles[role].artifactId);
    accounts.set(pdas.record, account(registry, false, roles[role].artifact));
    accounts.set(roles[role].program, roles[role].programAccount);
    accounts.set(roles[role].programData, roles[role].programDataAccount);
  }
  const cache = PublicKey.findProgramAddressSync([ACTIVATION_DOMAIN, releaseSetId], new PublicKey(registry))[0].toBase58();
  return Object.freeze({
    registry, rentProgram, payer, roles, multiprogram,
    checked: Object.freeze(Object.fromEntries(REGISTRY_ROLES.map((role) => [role, roles[role].checked])) as Record<RegistryRole, Uint8Array>),
    releaseSet, releaseSetId, cache, expectedCache: expectedCacheBytes(releaseSetId, roles), accounts,
  });
}

function client(accounts: ReadonlyMap<string, RpcAccount>) {
  return {
    finalizedSlot: async () => '900',
    multipleAccounts: async (addresses: ReadonlyArray<string>) => Object.freeze({
      slot: '900',
      accounts: Object.freeze(addresses.map((address) => Object.freeze({ address, account: accounts.get(address) ?? null }))),
    }),
    minimumBalanceForRentExemption: async (dataLength: number) => Object.freeze({ dataLength, lamports: '1' }),
    latestBlockhash: async () => Object.freeze({ slot: '900', blockhash: publicKey(32), lastValidBlockHeight: '1000' }),
  };
}

describe('checked Registry release workspace', () => {
  it('joins five full manifests over seven distinct programs, with Core not the Registry', async () => {
    const fixture = await sevenProgramFixture();
    const decoded = await decodeCheckedMultiprogramV1(fixture.multiprogram, fixture.checked);

    // The whole point of the fixture: seven programs, no two equal.
    const programs = [fixture.registry, fixture.rentProgram, ...REGISTRY_ROLES.map((role) => decoded.releaseSet.roles[role].program)];
    expect(new Set(programs).size).toBe(7);
    expect(decoded.releaseSet.roles.core.program).not.toBe(fixture.registry);
    expect(decoded.releaseSet.roles.core.program).toBe(fixture.roles.core.program);

    // Distinct programs must carry distinct artifact releases, or the release
    // set's partial-alias rule would have refused it.
    expect(new Set(REGISTRY_ROLES.map((role) => decoded.releaseSet.roles[role].artifactReleaseId)).size).toBe(5);
    expect(decoded.artifacts.trading.programData).toBe(fixture.roles.trading.programData);
    expect(decoded.checkedReleaseIds.core).not.toBe(decoded.checkedReleaseIds.custody);
  });

  it('refuses checked-release substitution and noncanonical manifest bytes', async () => {
    const fixture = await sevenProgramFixture();
    const substituted = { ...fixture.checked, trading: new Uint8Array(fixture.checked.trading) };
    substituted.trading[substituted.trading.length - 1] ^= 1;
    await expect(decodeCheckedMultiprogramV1(fixture.multiprogram, substituted)).rejects.toThrow();
    const reserved = new Uint8Array(fixture.multiprogram); reserved[12] = 1;
    await expect(decodeCheckedMultiprogramV1(reserved, fixture.checked)).rejects.toThrow('reserved');
    // A role's manifest moved onto another role's slot must not authenticate.
    const swapped = { ...fixture.checked, core: fixture.checked.custody };
    await expect(decodeCheckedMultiprogramV1(fixture.multiprogram, swapped)).rejects.toThrow('core');
  });

  it('plans the five-transaction activation walk over a seven-program release set', async () => {
    const fixture = await sevenProgramFixture();
    const plan = await prepareRegistryActivation(client(fixture.accounts), {
      registryProgram: fixture.registry, payer: fixture.payer, multiprogram: fixture.multiprogram,
      checkedReleases: fixture.checked, computeUnitLimit: 400_000,
    });

    expect(plan.mode).toBe('absent');
    expect(plan.activatedRoles).toEqual([]);
    expect(plan.remainingRoles).toEqual([...REGISTRY_ROLES]);
    expect(plan.cache).toBe(fixture.cache);
    expect(plan.packets).toHaveLength(REGISTRY_ROLES.length);
    expect(plan.roles.core.program).not.toBe(fixture.registry);
    expect(plan.totalElfBytesHashed).toBe(64 * REGISTRY_ROLES.length);

    // Each packet is exactly one role's ten-account instruction, and names that
    // role in the wire bytes: action 0, role index in canonical order.
    plan.packets.forEach((packet, index) => {
      expect(packet.role).toBe(REGISTRY_ROLES[index]);
      expect(packet.alreadyActivated).toBe(false);
      expect(packet.elfBytesHashed).toBe(64);
      expect(packet.addresses.program).toBe(fixture.roles[packet.role].program);
      expect(packet.requiredSigners).toEqual([fixture.payer]);
      const instruction = packet.transaction.message.compiledInstructions[1];
      expect(instruction.accountKeyIndexes).toHaveLength(REGISTRY_ACTIVATE_ROLE_ACCOUNT_COUNT);
      expect(Array.from(instruction.data.slice(10, 12))).toEqual([0, index]);
    });
    // Five distinct role programs must produce five distinct packets.
    expect(new Set(plan.packets.map((packet) => packet.wireBytes.join(','))).size).toBe(5);
  });

  it('reports mid-walk cache progress instead of refusing a partially admitted cache', async () => {
    const fixture = await sevenProgramFixture();
    expect(activationCacheProgressV1(fixture.expectedCache, fixture.expectedCache)).toEqual([...REGISTRY_ROLES]);

    const empty = new Uint8Array(fixture.expectedCache.slice(0, REGISTRY_ACTIVATION_CACHE_ROLES_OFFSET));
    const opened = new Uint8Array(ACTIVATION_CACHE_BYTES); opened.set(empty);
    expect(activationCacheProgressV1(opened, fixture.expectedCache)).toEqual([]);

    const partial = new Uint8Array(opened);
    [0, 2].forEach((index) => {
      const offset = REGISTRY_ACTIVATION_CACHE_ROLES_OFFSET + index * REGISTRY_ACTIVATED_ROLE_BYTES;
      partial.set(fixture.expectedCache.slice(offset, offset + REGISTRY_ACTIVATED_ROLE_BYTES), offset);
    });
    expect(activationCacheProgressV1(partial, fixture.expectedCache)).toEqual(['core', 'trading']);

    const walked = await prepareRegistryActivation(client(new Map(fixture.accounts).set(fixture.cache, account(fixture.registry, false, partial))), {
      registryProgram: fixture.registry, payer: fixture.payer, multiprogram: fixture.multiprogram,
      checkedReleases: fixture.checked, computeUnitLimit: 400_000,
    });
    expect(walked.mode).toBe('partial');
    expect(walked.activatedRoles).toEqual(['core', 'trading']);
    expect(walked.remainingRoles).toEqual(['claims', 'resolution', 'custody']);
    expect(walked.cacheRentDebitLamports).toBe('0');
    expect(walked.packets.filter((packet) => packet.alreadyActivated).map((packet) => packet.role)).toEqual(['core', 'trading']);

    const complete = await prepareRegistryActivation(client(new Map(fixture.accounts).set(fixture.cache, account(fixture.registry, false, fixture.expectedCache))), {
      registryProgram: fixture.registry, payer: fixture.payer, multiprogram: fixture.multiprogram,
      checkedReleases: fixture.checked, computeUnitLimit: 400_000,
    });
    expect(complete.mode).toBe('complete');
    expect(complete.remainingRoles).toEqual([]);
  });

  it('refuses a cache carrying a different release set masquerading as progress', async () => {
    const fixture = await sevenProgramFixture();
    const conflicting = new Uint8Array(fixture.expectedCache);
    conflicting[REGISTRY_ACTIVATION_CACHE_ROLES_OFFSET + 40] ^= 1;
    expect(() => activationCacheProgressV1(conflicting, fixture.expectedCache)).toThrow('conflicting core role');

    const foreignSelection = new Uint8Array(fixture.expectedCache); foreignSelection[16] ^= 1;
    expect(() => activationCacheProgressV1(foreignSelection, fixture.expectedCache)).toThrow('different release set');

    await expect(prepareRegistryActivation(client(new Map(fixture.accounts).set(fixture.cache, account(fixture.registry, false, conflicting))), {
      registryProgram: fixture.registry, payer: fixture.payer, multiprogram: fixture.multiprogram,
      checkedReleases: fixture.checked, computeUnitLimit: 400_000,
    })).rejects.toThrow('conflicting core role');
  });

  it('refuses activation when the named Registry program is not a live executable', async () => {
    const fixture = await sevenProgramFixture();
    const withoutRegistry = new Map(fixture.accounts); withoutRegistry.delete(fixture.registry);
    await expect(prepareRegistryActivation(client(withoutRegistry), {
      registryProgram: fixture.registry, payer: fixture.payer, multiprogram: fixture.multiprogram,
      checkedReleases: fixture.checked, computeUnitLimit: 400_000,
    })).rejects.toThrow('Registry Program');

    const notExecutable = new Map(fixture.accounts);
    const observed = fixture.accounts.get(fixture.registry);
    notExecutable.set(fixture.registry, account(UPGRADEABLE_LOADER_ID, false, observed?.data ?? new Uint8Array(36)));
    await expect(prepareRegistryActivation(client(notExecutable), {
      registryProgram: fixture.registry, payer: fixture.payer, multiprogram: fixture.multiprogram,
      checkedReleases: fixture.checked, computeUnitLimit: 400_000,
    })).rejects.toThrow('not current Loader-v3 executable state');
  });

  it('reauthenticates one role from a cache whose Core is not the Registry program', async () => {
    const fixture = await sevenProgramFixture();
    const accounts = new Map(fixture.accounts).set(fixture.cache, account(fixture.registry, false, fixture.expectedCache));
    const plan = await prepareRegistryReauthentication(client(accounts), {
      registryProgram: fixture.registry, payer: fixture.payer, cache: fixture.cache, role: 'trading', computeUnitLimit: 80_000,
    });
    expect(plan.artifact.program).toBe(fixture.roles.trading.program);
    expect(plan.artifact.program).not.toBe(fixture.registry);
    expect(plan.transaction.message.compiledInstructions[1].accountKeyIndexes).toHaveLength(REGISTRY_REAUTH_ACCOUNT_COUNT);
    expect(Array.from(plan.transaction.message.compiledInstructions[1].data.slice(10, 12))).toEqual([1, 2]);
  });

  it('emits the committed activation and reauthentication packet geometry', async () => {
    const fixture = await sevenProgramFixture();
    const activation = compileRegistryRoleActivationTransaction({
      payer: fixture.payer, registryProgram: fixture.registry, recentBlockhash: publicKey(32), computeUnitLimit: 400_000,
      cache: publicKey(33), releaseSetRecord: publicKey(34), releaseSetStaging: publicKey(35), role: 'custody',
      addresses: Object.freeze({ record: publicKey(21), staging: publicKey(22), program: fixture.roles.custody.program, programData: fixture.roles.custody.programData }),
    });
    expect(activation.requiredSigners).toEqual([fixture.payer]);
    expect(activation.transaction.message.compiledInstructions[1].accountKeyIndexes).toHaveLength(REGISTRY_ACTIVATE_ROLE_ACCOUNT_COUNT);
    expect(Array.from(activation.transaction.message.compiledInstructions[1].data.slice(10, 12))).toEqual([0, 4]);
    // Committed geometry. These byte counts moved when the fixture stopped
    // aliasing every account to one key: a degenerate fixture compiles a
    // message with almost no distinct static keys and cannot pin a real frame.
    expect(activation.wireBytes).toHaveLength(ACTIVATION_PACKET_BYTES);
    expect(activation.wireBytes.length).toBeLessThanOrEqual(1_232);

    const reauthentication = compileRegistryReauthenticationTransaction({
      payer: fixture.payer, registryProgram: fixture.registry, recentBlockhash: publicKey(32), computeUnitLimit: 80_000,
      cache: publicKey(33), role: 'trading', program: fixture.roles.trading.program, programData: fixture.roles.trading.programData,
    });
    expect(reauthentication.wireBytes).toHaveLength(REAUTH_PACKET_BYTES);
    expect(reauthentication.transaction.message.compiledInstructions[1].accountKeyIndexes).toHaveLength(REGISTRY_REAUTH_ACCOUNT_COUNT);
    expect(Array.from(reauthentication.transaction.message.compiledInstructions[1].data.slice(10, 12))).toEqual([1, 2]);
  });
});
