import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { hex, sha256 } from './bytes';
import {
  KNOWN_ABI_RELEASES_V1,
  authenticateReleaseCurrencyV1,
  discoverCurrentActivationCacheV1,
  openReleaseBoundSessionV1,
  readExecutionReleaseIdentityV1,
  selectAbiReleaseV1,
  type AbiReleaseTableV1,
} from './releaseIdentity';
import {
  ACTIVATION_CACHE_BYTES,
  ARTIFACT_RELEASE_BYTES,
  REGISTRY_ROLES,
  UPGRADEABLE_LOADER_ID,
  type RegistryRole,
} from './releaseRegistry';
import { type RpcAccount } from './rpc';

const ACTIVATION_DOMAIN = new TextEncoder().encode('dclutch:release-activation:v1');
const REGISTRY_PROGRAM = new PublicKey(Uint8Array.from({ length: 32 }, () => 0xd1)).toBase58();

function account(owner: string, executable: boolean, data: Uint8Array): RpcAccount {
  return Object.freeze({ owner, executable, data: new Uint8Array(data), lamports: '1', space: data.length });
}

/**
 * One role's activated identity, with its three axes independently settable.
 *
 * `programSeed` fixes WHICH program, `semanticSeed` fixes WHAT SEMANTICS it
 * speaks, and `slot` fixes WHEN it was deployed. The protocol lets these move
 * independently — a rebuild moves the slot and the ELF while the semantics
 * stand still — so a fixture that could not separate them could not test the
 * one thing this module gets right.
 */
type RoleSpec = Readonly<{ programSeed: number; semanticSeed: number; slot: number }>;

async function roleBytes(spec: RoleSpec): Promise<Readonly<{
  bytes: Uint8Array;
  program: string;
  programData: string;
  programDataAccount: RpcAccount;
}>> {
  const loader = new PublicKey(UPGRADEABLE_LOADER_ID);
  const program = new PublicKey(Uint8Array.from({ length: 32 }, () => spec.programSeed));
  const programData = PublicKey.findProgramAddressSync([program.toBytes()], loader)[0];
  const elf = Uint8Array.from({ length: 64 }, (_, index) => (spec.slot + index) & 0xff);
  const bytes = new Uint8Array(ARTIFACT_RELEASE_BYTES);
  bytes.set(new TextEncoder().encode('DCLTARF1'));
  const view = new DataView(bytes.buffer);
  view.setUint16(8, 1, true);
  view.setUint16(10, 1, true);
  bytes.set(program.toBytes(), 16);
  bytes.set(loader.toBytes(), 48);
  bytes.set(programData.toBytes(), 80);
  bytes.fill(spec.semanticSeed, 112, 144);
  bytes.set(await sha256(elf), 144);
  view.setBigUint64(176, BigInt(spec.slot), true);
  const programDataBytes = new Uint8Array(45 + elf.length);
  new DataView(programDataBytes.buffer).setUint32(0, 3, true);
  new DataView(programDataBytes.buffer).setBigUint64(4, BigInt(spec.slot), true);
  programDataBytes.set(elf, 45);
  return Object.freeze({
    bytes,
    program: program.toBase58(),
    programData: programData.toBase58(),
    programDataAccount: account(UPGRADEABLE_LOADER_ID, false, programDataBytes),
  });
}

type CacheFixture = Readonly<{
  registryProgram: string;
  activationCache: string;
  cacheAccount: RpcAccount;
  releaseSetId: string;
  semanticReleaseIds: Readonly<Record<RegistryRole, string>>;
  programData: Readonly<Record<RegistryRole, string>>;
  programDataAccounts: Readonly<Record<RegistryRole, RpcAccount>>;
}>;

/** Build one activation cache that survives the contract's hostile decode. */
async function cacheFixture(specs: Readonly<Record<RegistryRole, RoleSpec>>): Promise<CacheFixture> {
  const roles = await Promise.all(REGISTRY_ROLES.map((role) => roleBytes(specs[role])));
  const artifactIds = await Promise.all(roles.map((role) => sha256(role.bytes)));

  const releaseSet = new Uint8Array(336);
  releaseSet.set(new TextEncoder().encode('DCLTRLS1'));
  const setView = new DataView(releaseSet.buffer);
  setView.setUint16(8, 1, true);
  setView.setUint16(10, 1, true);
  REGISTRY_ROLES.forEach((_, index) => {
    releaseSet.set(new PublicKey(roles[index].program).toBytes(), 16 + index * 64);
    releaseSet.set(artifactIds[index], 48 + index * 64);
  });
  const releaseSetId = await sha256(releaseSet);

  const cache = new Uint8Array(ACTIVATION_CACHE_BYTES);
  cache.set(new TextEncoder().encode('DCLTACT1'));
  const cacheView = new DataView(cache.buffer);
  cacheView.setUint16(8, 1, true);
  cacheView.setUint16(10, 1, true);
  cache.set(releaseSetId, 16);
  REGISTRY_ROLES.forEach((_, index) => {
    const offset = 48 + index * (32 + ARTIFACT_RELEASE_BYTES);
    cache.set(artifactIds[index], offset);
    cache.set(roles[index].bytes, offset + 32);
  });
  const activationCache = PublicKey.findProgramAddressSync(
    [ACTIVATION_DOMAIN, releaseSetId],
    new PublicKey(REGISTRY_PROGRAM),
  )[0].toBase58();

  const byRole = <T>(pick: (index: number) => T): Readonly<Record<RegistryRole, T>> => Object.freeze(
    Object.fromEntries(REGISTRY_ROLES.map((role, index) => [role, pick(index)])) as Record<RegistryRole, T>,
  );
  return Object.freeze({
    registryProgram: REGISTRY_PROGRAM,
    activationCache,
    cacheAccount: account(REGISTRY_PROGRAM, false, cache),
    releaseSetId: hex(releaseSetId),
    semanticReleaseIds: byRole((index) => hex(roles[index].bytes.slice(112, 144))),
    programData: byRole((index) => roles[index].programData),
    programDataAccounts: byRole((index) => roles[index].programDataAccount),
  });
}

function specs(programBase: number, semanticBase: number, slotBase: number): Readonly<Record<RegistryRole, RoleSpec>> {
  return Object.freeze(Object.fromEntries(REGISTRY_ROLES.map((role, index) => [role, Object.freeze({
    programSeed: programBase + index,
    semanticSeed: semanticBase + index,
    slot: slotBase + index,
  })])) as Record<RegistryRole, RoleSpec>);
}

function reader(fixture: CacheFixture, override?: RpcAccount | null) {
  return {
    accountInfo: async (address: string) => Object.freeze({
      slot: '4242',
      account: address === fixture.activationCache
        ? (override === undefined ? fixture.cacheAccount : override)
        : null,
    }),
  };
}

function slicer(accounts: ReadonlyMap<string, RpcAccount | null>) {
  return {
    multipleAccountDataSlices: async (addresses: ReadonlyArray<string>, offset: number, length: number) => Object.freeze({
      slot: '4242',
      accounts: Object.freeze(addresses.map((address) => {
        const found = accounts.get(address) ?? null;
        return Object.freeze({
          address,
          account: found === null ? null : account(found.owner, found.executable, found.data.slice(offset, offset + length)),
        });
      })),
    }),
  };
}

/** A Registry that owns the given caches, as a width-filtered scan would see. */
function scanner(fixtures: ReadonlyArray<CacheFixture>, extra: ReadonlyArray<Readonly<{ address: string; account: RpcAccount }>> = []) {
  return {
    programAccountsOfExactWidth: async (programId: string, dataLength: number) => {
      expect(dataLength).toBe(ACTIVATION_CACHE_BYTES);
      expect(programId).toBe(REGISTRY_PROGRAM);
      return Object.freeze({
        slot: '4242',
        accounts: Object.freeze([
          ...fixtures.map((fixture) => Object.freeze({ address: fixture.activationCache, account: fixture.cacheAccount })),
          ...extra,
        ]),
      });
    },
  };
}

function tableFor(fixture: CacheFixture, label: string): AbiReleaseTableV1 {
  return Object.freeze({
    label,
    provenance: `synthetic fixture for ${label}`,
    semanticReleaseIds: fixture.semanticReleaseIds,
    abi: KNOWN_ABI_RELEASES_V1[0].abi,
  });
}

describe('reading which release the chain says is live', () => {
  it('decodes the release-set identity and every role semantic id from one account', async () => {
    const fixture = await cacheFixture(specs(0x11, 0x21, 700));
    const identity = await readExecutionReleaseIdentityV1(reader(fixture), {
      registryProgram: fixture.registryProgram,
      activationCache: fixture.activationCache,
    });
    expect(identity.executionReleaseSetId).toBe(fixture.releaseSetId);
    expect(identity.activationCache).toBe(fixture.activationCache);
    expect(identity.observedSlot).toBe('4242');
    for (const role of REGISTRY_ROLES) {
      expect(identity.roles[role].semanticReleaseId, role).toBe(fixture.semanticReleaseIds[role]);
      expect(identity.roles[role].programData, role).toBe(fixture.programData[role]);
    }
    expect(identity.roles.core.deploymentSlot).toBe('700');
  });

  it('refuses by name when the named cache holds no account', async () => {
    const fixture = await cacheFixture(specs(0x11, 0x21, 700));
    await expect(readExecutionReleaseIdentityV1(reader(fixture, null), {
      registryProgram: fixture.registryProgram,
      activationCache: fixture.activationCache,
    })).rejects.toThrow(/no account exists there/);
  });

  it('refuses when the account at the cache address is not Registry-owned', async () => {
    const fixture = await cacheFixture(specs(0x11, 0x21, 700));
    const foreign = account(UPGRADEABLE_LOADER_ID, false, fixture.cacheAccount.data);
    await expect(readExecutionReleaseIdentityV1(reader(fixture, foreign), {
      registryProgram: fixture.registryProgram,
      activationCache: fixture.activationCache,
    })).rejects.toThrow(/not the Registry program/);
  });
});

describe('selecting an ABI table by on-chain identity', () => {
  it('chooses the table whose semantics the chain is running, out of two', async () => {
    const first = await cacheFixture(specs(0x11, 0x21, 700));
    const second = await cacheFixture(specs(0x41, 0x51, 900));
    expect(first.releaseSetId).not.toBe(second.releaseSetId);
    const tables = [tableFor(first, 'cohort-alpha'), tableFor(second, 'cohort-beta')];

    for (const [fixture, expected] of [[first, 'cohort-alpha'], [second, 'cohort-beta']] as const) {
      const identity = await readExecutionReleaseIdentityV1(reader(fixture), {
        registryProgram: fixture.registryProgram,
        activationCache: fixture.activationCache,
      });
      expect(selectAbiReleaseV1(identity, tables).label).toBe(expected);
    }
  });

  it('refuses an unknown identity by NAMING both what the chain runs and what it carries', async () => {
    const known = await cacheFixture(specs(0x11, 0x21, 700));
    const live = await cacheFixture(specs(0x41, 0x51, 900));
    const identity = await readExecutionReleaseIdentityV1(reader(live), {
      registryProgram: live.registryProgram,
      activationCache: live.activationCache,
    });

    let message = '';
    try {
      selectAbiReleaseV1(identity, [tableFor(known, 'cohort-alpha')]);
      throw new Error('selection should have refused');
    } catch (error) {
      message = (error as Error).message;
    }
    // The refusal is the deliverable: it must be answerable without a debugger.
    expect(message).toContain('no ABI table for the release the chain is running');
    expect(message).toContain(live.releaseSetId.slice(0, 16));
    expect(message).toContain(live.semanticReleaseIds.trading.slice(0, 16));
    expect(message).toContain('cohort-alpha');
    expect(message).toContain(live.activationCache);
    for (const role of REGISTRY_ROLES) expect(message, role).toContain(role);
  });

  /**
   * The design decision, made executable.
   *
   * A release set id is the hash of the whole activated set, so an ordinary
   * rebuild — same source, new ELF, new deployment slot — mints a DIFFERENT
   * release set id. Keying ABI tables on it would make every client refuse on
   * every cohort bump, which is not upgrade-proofing, it is breakage on a
   * schedule. Semantics are what a frame actually depends on, so that is the
   * key. Observed on live devnet 2026-08-29: Trading and Resolution held one
   * semantic release id across four consecutive cohorts.
   */
  it('keeps selecting across a pure rebuild: new slots and set id, unchanged semantics', async () => {
    const before = await cacheFixture(specs(0x11, 0x21, 700));
    const after = await cacheFixture(specs(0x11, 0x21, 900));
    expect(after.releaseSetId).not.toBe(before.releaseSetId);
    expect(after.semanticReleaseIds).toEqual(before.semanticReleaseIds);

    const identity = await readExecutionReleaseIdentityV1(reader(after), {
      registryProgram: after.registryProgram,
      activationCache: after.activationCache,
    });
    expect(selectAbiReleaseV1(identity, [tableFor(before, 'cohort-alpha')]).label).toBe('cohort-alpha');
  });

  it('refuses when a single role changes semantics, and names that role', async () => {
    const before = await cacheFixture(specs(0x11, 0x21, 700));
    const moved = { ...specs(0x11, 0x21, 700) } as Record<RegistryRole, RoleSpec>;
    moved.trading = Object.freeze({ programSeed: moved.trading.programSeed, semanticSeed: 0x99, slot: moved.trading.slot });
    const after = await cacheFixture(Object.freeze(moved));

    const identity = await readExecutionReleaseIdentityV1(reader(after), {
      registryProgram: after.registryProgram,
      activationCache: after.activationCache,
    });
    expect(() => selectAbiReleaseV1(identity, [tableFor(before, 'cohort-alpha')]))
      .toThrow(/differs on trading/);
  });
});

describe('confirming the named cache is the CURRENT one', () => {
  it('accepts a cache whose pinned slots equal the live ProgramData slots', async () => {
    const fixture = await cacheFixture(specs(0x11, 0x21, 700));
    const identity = await readExecutionReleaseIdentityV1(reader(fixture), {
      registryProgram: fixture.registryProgram,
      activationCache: fixture.activationCache,
    });
    const live = new Map(REGISTRY_ROLES.map((role) => [fixture.programData[role], fixture.programDataAccounts[role]]));
    await expect(authenticateReleaseCurrencyV1(slicer(live), identity)).resolves.toBeUndefined();
  });

  /**
   * The defect this exists to catch, reproduced.
   *
   * A superseded activation cache keeps its Registry owner, its `DCLTACT1`
   * magic and its exact width forever, so every cheap health check on it
   * passes. On 2026-08-29 the shipped devnet manifest named a cache four
   * cohorts stale that had passed exactly such a check that morning.
   */
  it('refuses a SUPERSEDED cache, naming the pinned and the live slot', async () => {
    const shipped = await cacheFixture(specs(0x11, 0x21, 700));
    const current = await cacheFixture(specs(0x11, 0x21, 900));
    const identity = await readExecutionReleaseIdentityV1(reader(shipped), {
      registryProgram: shipped.registryProgram,
      activationCache: shipped.activationCache,
    });
    // The cache still decodes perfectly. Only the chain moved past it.
    expect(identity.executionReleaseSetId).toBe(shipped.releaseSetId);
    const live = new Map(REGISTRY_ROLES.map((role) => [shipped.programData[role], current.programDataAccounts[role]]));

    let message = '';
    try {
      await authenticateReleaseCurrencyV1(slicer(live), identity);
      throw new Error('currency check should have refused');
    } catch (error) {
      message = (error as Error).message;
    }
    expect(message).toContain('SUPERSEDED');
    expect(message).toContain('core pinned slot 700, live slot 900');
    expect(message).toContain(shipped.activationCache);
  });
});

describe('discovering the current cache instead of trusting a constant', () => {
  it('picks the one cache whose pinned slots match the live programs, out of five cohorts', async () => {
    const cohorts = await Promise.all([700, 750, 800, 850, 900].map((slot) => cacheFixture(specs(0x11, 0x21, slot))));
    const current = cohorts[4];
    const live = new Map(REGISTRY_ROLES.map((role) => [current.programData[role], current.programDataAccounts[role]]));
    const identity = await discoverCurrentActivationCacheV1(
      { ...scanner(cohorts), ...slicer(live) },
      REGISTRY_PROGRAM,
    );
    expect(identity.activationCache).toBe(current.activationCache);
    expect(identity.executionReleaseSetId).toBe(current.releaseSetId);
    // Permanent program ids: every cohort names the same five ProgramData
    // accounts, so following costs one scan and one five-account read.
    expect(cohorts[0].programData.core).toBe(current.programData.core);
  });

  it('ignores an undecodable (partially activated) cache rather than choking', async () => {
    const cohorts = await Promise.all([700, 900].map((slot) => cacheFixture(specs(0x11, 0x21, slot))));
    const partial = new Uint8Array(ACTIVATION_CACHE_BYTES);
    partial.set(new TextEncoder().encode('DCLTACT1'));
    const live = new Map(REGISTRY_ROLES.map((role) => [cohorts[1].programData[role], cohorts[1].programDataAccounts[role]]));
    const identity = await discoverCurrentActivationCacheV1(
      {
        ...scanner(cohorts, [Object.freeze({ address: 'PartiallyActivatedCache', account: account(REGISTRY_PROGRAM, false, partial) })]),
        ...slicer(live),
      },
      REGISTRY_PROGRAM,
    );
    expect(identity.activationCache).toBe(cohorts[1].activationCache);
  });

  it('refuses — a real alarm — when no cache describes the live programs', async () => {
    const cohorts = await Promise.all([700, 750].map((slot) => cacheFixture(specs(0x11, 0x21, slot))));
    const unrelated = await cacheFixture(specs(0x11, 0x21, 900));
    const live = new Map(REGISTRY_ROLES.map((role) => [cohorts[0].programData[role], unrelated.programDataAccounts[role]]));
    await expect(discoverCurrentActivationCacheV1({ ...scanner(cohorts), ...slicer(live) }, REGISTRY_PROGRAM))
      .rejects.toThrow(/no activation cache on this chain describes the programs that are actually running/);
  });

  it('refuses when the Registry owns no decodable cache at all', async () => {
    await expect(discoverCurrentActivationCacheV1({ ...scanner([]), ...slicer(new Map()) }, REGISTRY_PROGRAM))
      .rejects.toThrow(/owns no decodable 1288-byte activation cache/);
  });
});

describe('opening a release-bound session', () => {
  it('uses the manifest hint directly when it is still current', async () => {
    const fixture = await cacheFixture(specs(0x11, 0x21, 700));
    const live = new Map(REGISTRY_ROLES.map((role) => [fixture.programData[role], fixture.programDataAccounts[role]]));
    const client = { ...reader(fixture), ...slicer(live), ...scanner([fixture]) };
    const session = await openReleaseBoundSessionV1(client, {
      registryProgram: fixture.registryProgram,
      activationCache: fixture.activationCache,
    }, { releases: [tableFor(fixture, 'cohort-alpha')] });
    expect(session.release.label).toBe('cohort-alpha');
    expect(session.identity.executionReleaseSetId).toBe(fixture.releaseSetId);
    expect(session.source.kind).toBe('manifest');
    expect(session.abi.coreFoundAccountCount).toBe(KNOWN_ABI_RELEASES_V1[0].abi.coreFoundAccountCount);
  });

  /**
   * The upgrade that makes this lane's thesis real.
   *
   * A cohort lands, the manifest constant ages out the same minute, and no
   * human updates anything. The session FOLLOWS the chain to the cache whose
   * pinned slots match the live programs, and says which one it found versus
   * what the manifest shipped. A stale constant is not a reason to stop
   * working — it is a reason to stop trusting the constant.
   */
  it('FOLLOWS the chain when the manifest hint has aged out, and names both caches', async () => {
    const shipped = await cacheFixture(specs(0x11, 0x21, 700));
    const current = await cacheFixture(specs(0x11, 0x21, 900));
    const live = new Map(REGISTRY_ROLES.map((role) => [current.programData[role], current.programDataAccounts[role]]));
    const client = {
      accountInfo: reader(shipped).accountInfo,
      ...slicer(live),
      ...scanner([shipped, current]),
    };
    const session = await openReleaseBoundSessionV1(client, {
      registryProgram: shipped.registryProgram,
      activationCache: shipped.activationCache,
    }, { releases: [tableFor(current, 'cohort-current')] });

    expect(session.identity.activationCache).toBe(current.activationCache);
    expect(session.release.label).toBe('cohort-current');
    expect(session.source.kind).toBe('discovered');
    if (session.source.kind !== 'discovered') throw new Error('unreachable');
    expect(session.source.supersededManifestCache).toBe(shipped.activationCache);
    expect(session.source.note).toContain(shipped.activationCache);
    expect(session.source.note).toContain(current.activationCache);
  });

  it('discovers even when the manifest names no activation cache at all', async () => {
    const current = await cacheFixture(specs(0x11, 0x21, 900));
    const live = new Map(REGISTRY_ROLES.map((role) => [current.programData[role], current.programDataAccounts[role]]));
    const session = await openReleaseBoundSessionV1(
      { ...reader(current), ...slicer(live), ...scanner([current]) },
      { registryProgram: current.registryProgram, activationCache: null },
      { releases: [tableFor(current, 'cohort-current')] },
    );
    expect(session.identity.activationCache).toBe(current.activationCache);
    expect(session.source.kind).toBe('discovered');
  });

  it('refuses instead of following when the caller asks it to', async () => {
    const shipped = await cacheFixture(specs(0x11, 0x21, 700));
    const current = await cacheFixture(specs(0x11, 0x21, 900));
    const live = new Map(REGISTRY_ROLES.map((role) => [shipped.programData[role], current.programDataAccounts[role]]));
    await expect(openReleaseBoundSessionV1(
      { ...reader(shipped), ...slicer(live), ...scanner([shipped, current]) },
      { registryProgram: shipped.registryProgram, activationCache: shipped.activationCache },
      { releases: [tableFor(shipped, 'cohort-alpha')], followCurrent: false },
    )).rejects.toThrow(/SUPERSEDED/);
  });
});

describe('the shipped table', () => {
  it('carries exactly one release, with observed provenance', () => {
    expect(KNOWN_ABI_RELEASES_V1).toHaveLength(1);
    const only = KNOWN_ABI_RELEASES_V1[0];
    expect(only.provenance).toMatch(/decoded from the live registry activation cache/i);
    for (const role of REGISTRY_ROLES) {
      expect(only.semanticReleaseIds[role], role).toMatch(/^[0-9a-f]{64}$/);
    }
    // Distinct semantics per role: a table that repeated one id would select
    // on a coincidence rather than on the five facts the chain states.
    expect(new Set(REGISTRY_ROLES.map((role) => only.semanticReleaseIds[role])).size).toBe(REGISTRY_ROLES.length);
  });

  it('reads its frame facts from the generated modules, not from transcribed literals', async () => {
    const generated = await import('./generated/genericFoundingV1');
    expect(KNOWN_ABI_RELEASES_V1[0].abi.coreFoundTradingProgramIndex)
      .toBe(generated.CORE_FOUND_TRADING_PROGRAM_INDEX_V1);
  });
});
