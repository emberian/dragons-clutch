import { PublicKey } from '@solana/web3.js';

import { ascii, hex, requireNonzero, requireZero, sha256, slice, u16 } from './bytes';
import { classifyHeader } from './decoders';
import {
  DEVNET_DEPLOYMENT_V1,
  DEVNET_PROGRAM_EVIDENCE_V1,
  type DeploymentV1,
  type ProgramEvidenceV1,
} from './deployments';
import {
  ACTIVATION_CACHE_BYTES,
  ARTIFACT_RELEASE_BYTES,
  EXECUTION_RELEASE_SET_BYTES,
  LOADER_V3_PROGRAM_BYTES,
  LOADER_V3_PROGRAMDATA_OFFSET,
  REGISTRY_ACTIVATED_ROLE_BYTES,
  REGISTRY_ACTIVATION_CACHE_ROLES_OFFSET,
  REGISTRY_ROLES,
  UPGRADEABLE_LOADER_ID,
  decodeArtifactReleaseV1,
  decodeExecutionReleaseSetV1,
  requireSlotPinnedReleaseV1,
  REGISTRY_ACTIVATION_PDA_SEED_V1,
  type ArtifactReleaseV1,
  type RegistryRole,
} from './releaseRegistry';
import { SOLANA_DEVNET_GENESIS_HASH_V1 } from './rpc';
import { type RpcAccount, type SolanaRpcClient } from './rpc';

export const OPERATOR_ROLES = ['registry', 'core', 'trading', 'claims', 'custody', 'resolution'] as const;
export type OperatorRole = (typeof OPERATOR_ROLES)[number];

export type OperatorCoordinatesV1 = Readonly<Record<OperatorRole, string>> & Readonly<{
  market?: string;
  realm?: string;
}>;

export type OperatorDeploymentPresetV1 = Readonly<{
  schema: 'dclutch-operator-deployment-preset-v1';
  label: 'Checked live devnet';
  endpoint: string;
  genesisHash: string;
  coordinates: Readonly<Record<OperatorRole, string>>;
  activationCache: string;
  evidence: Readonly<Record<OperatorRole, ProgramEvidenceV1>>;
  provenance: string;
}>;

export type OperatorRoleObservationV1 = Readonly<{
  role: OperatorRole;
  address: string;
  owner: string;
  executable: boolean;
  dataBytes: number;
}>;

export type RouteSpecificReleaseAdmissionV1 = Readonly<{
  kind: 'unproven';
  reason: string;
}>;

/**
 * A deployment match is intentionally not a Market admission.
 *
 * The six Loader pairs and the Registry activation cache answer which program
 * generation is running. They do not name a Realm, Market, capability, or the
 * route-specific releases those accounts select. Keep that missing join in the
 * result instead of letting a caller accidentally promote "programs match" to
 * "this route is admitted".
 */
export const ROUTE_SPECIFIC_RELEASE_ADMISSION_UNPROVEN_V1: RouteSpecificReleaseAdmissionV1 = Object.freeze({
  kind: 'unproven',
  reason: 'The live preset authenticates the six deployed program generations and their activation cache only; no Realm, Market, or route-specific release admission was proved.',
});

export type OperatorSurfaceSnapshotV1 = Readonly<{
  observedSlot: string;
  roles: ReadonlyArray<OperatorRoleObservationV1>;
  deploymentPreset: null | Readonly<{
    label: OperatorDeploymentPresetV1['label'];
    genesisHash: string;
    activationCache: string;
    executionReleaseSetId: string;
    /** Live deployment slots, read from each ProgramData header this run. */
    deploymentSlots: Readonly<Record<OperatorRole, string>>;
    upgradeAuthorities: Readonly<Record<OperatorRole, string>>;
    /**
     * Roles whose live slot is past the one the shipped manifest recorded.
     *
     * Upgrading in place at a permanent address is ordinary, so this reports
     * the manifest's age rather than a fault. The upgrade AUTHORITY is still
     * asserted exactly against the activated release above, which is where
     * this preset's authenticity actually lives.
     */
    upgradedSinceRecord: ReadonlyArray<OperatorRole>;
    routeSpecificReleaseAdmission: RouteSpecificReleaseAdmissionV1;
  }>;
  realm: null | Readonly<{
    address: string;
    owner: string;
    dataBytes: number;
    header: string | null;
  }>;
  market: null | Readonly<{
    address: string;
    owner: string;
    dataBytes: number;
    header: string | null;
  }>;
}>;

function canonicalKey(value: unknown, field: string): string {
  if (typeof value !== 'string' || value.length === 0) throw new Error(`${field} is required`);
  const key = new PublicKey(value);
  if (key.toBase58() !== value) throw new Error(`${field} must be canonical base58 text`);
  return value;
}

function object(value: unknown, field: string): Record<string, unknown> {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) throw new Error(`${field} is absent or partial`);
  return value as Record<string, unknown>;
}

function exactDeploymentSlot(value: unknown, field: string): string {
  if (typeof value !== 'string' || !/^[1-9][0-9]*$/.test(value)) throw new Error(`${field} is not one positive canonical decimal slot`);
  return value;
}

/**
 * Turn the app's deployment authority into the one preset `/operate` may name.
 *
 * This validates a static projection; it deliberately does not call it chain
 * truth. `acquireOperatorSurfaceV1` must still reacquire devnet's identity and
 * every Loader Program/ProgramData pair at finalized commitment before the UI
 * describes the preset as matched.
 */
export function checkedLiveDevnetOperatorPresetV1(
  deployment: unknown,
  programEvidence: unknown,
): OperatorDeploymentPresetV1 {
  const source = object(deployment, 'deployment projection');
  if (source.cluster !== 'devnet') throw new Error('operator live preset refuses every non-devnet deployment');
  if (source.genesisHash !== SOLANA_DEVNET_GENESIS_HASH_V1) throw new Error('operator live preset refuses a deployment that does not pin Solana devnet genesis');
  if (typeof source.endpoint !== 'string') throw new Error('operator live preset endpoint is absent');
  const endpoint = new URL(source.endpoint);
  if (endpoint.protocol !== 'http:' && endpoint.protocol !== 'https:') throw new Error('operator live preset endpoint must use http or https');
  const programs = object(source.programs, 'deployment program table');
  const evidenceTable = object(programEvidence, 'deployment ProgramData evidence');
  const coordinates = {} as Record<OperatorRole, string>;
  const evidence = {} as Record<OperatorRole, ProgramEvidenceV1>;
  const loader = new PublicKey(UPGRADEABLE_LOADER_ID);
  for (const role of OPERATOR_ROLES) {
    const program = canonicalKey(programs[role] as string, `${role} preset program`);
    const row = object(evidenceTable[role], `${role} ProgramData evidence`);
    const programData = canonicalKey(row.programData as string, `${role} preset ProgramData`);
    const derived = PublicKey.findProgramAddressSync([new PublicKey(program).toBytes()], loader)[0].toBase58();
    if (programData !== derived) throw new Error(`${role} preset ProgramData is not the canonical Loader-v3 coordinate for its Program`);
    coordinates[role] = program;
    evidence[role] = Object.freeze({
      programData,
      deploymentSlot: exactDeploymentSlot(row.deploymentSlot, `${role} recorded deployment slot`),
    });
  }
  const allLoaderCoordinates = [
    ...Object.values(coordinates),
    ...Object.values(evidence).map((row) => row.programData),
  ];
  if (new Set(allLoaderCoordinates).size !== allLoaderCoordinates.length) throw new Error('operator live preset aliases Program or ProgramData coordinates');
  const activationCache = canonicalKey(source.activationCache as string, 'operator live preset activation cache');
  if (allLoaderCoordinates.includes(activationCache)) throw new Error('operator live preset activation cache aliases a Loader coordinate');
  return Object.freeze({
    schema: 'dclutch-operator-deployment-preset-v1',
    label: 'Checked live devnet',
    endpoint: source.endpoint,
    genesisHash: source.genesisHash,
    coordinates: Object.freeze(coordinates),
    activationCache,
    evidence: Object.freeze(evidence),
    provenance: 'The checked DEPLOY-1 record projected by the app deployment manifest; finalized chain reacquisition is still required.',
  });
}

/**
 * The one preset `/operate` may name, derived on first use.
 *
 * IT USED TO BE A MODULE-SCOPE `const`, and that was a latent bundle bug.
 * `checkedLiveDevnetOperatorPresetV1` calls `PublicKey.findProgramAddressSync`
 * once per role, and that function SEARCHES: it walks 256 nonces and throws
 * `Unable to find a viable program address nonce` when none lands off the
 * curve. A module whose top level can throw takes down everything that
 * imports it, however incidentally — and it did. Past the eighteenth
 * component import in one module graph this threw during collection, while
 * the same module imported alone evaluated fine; bisected to that exact
 * boundary. A page that happened to import one more component would have
 * shipped broken, with a stack naming this file rather than whatever pushed
 * the graph over.
 *
 * Deriving on demand changes nothing about the checks — every one of them
 * runs, unchanged, on the first call — and moves the failure to a caller that
 * can see it. The result is memoized, so the seven derivations still happen
 * exactly once per process.
 */
let livePreset: OperatorDeploymentPresetV1 | null = null;

export function liveDevnetOperatorPresetV1(): OperatorDeploymentPresetV1 {
  livePreset ??= checkedLiveDevnetOperatorPresetV1(
    DEVNET_DEPLOYMENT_V1 satisfies DeploymentV1,
    DEVNET_PROGRAM_EVIDENCE_V1,
  );
  return livePreset;
}

function requireAccount(account: RpcAccount | null, field: string): RpcAccount {
  if (account === null) throw new Error(`${field} is absent at the finalized observation floor`);
  return account;
}

function littleU64(bytes: Uint8Array, offset: number, field: string): string {
  if (bytes.length < offset + 8) throw new Error(`${field} is truncated`);
  return new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength).getBigUint64(offset, true).toString();
}

function same(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

type ActivatedExecutionV1 = Readonly<{
  releaseSetId: string;
  artifacts: Readonly<Record<RegistryRole, ArtifactReleaseV1>>;
}>;

/** Hostile-decode the complete cache without reading a single ELF byte. */
async function exactActivationCache(
  account: RpcAccount,
  registryProgram: string,
  cacheAddress: string,
): Promise<ActivatedExecutionV1> {
  if (account.owner !== registryProgram || account.executable
      || account.data.length !== ACTIVATION_CACHE_BYTES || account.space !== ACTIVATION_CACHE_BYTES) {
    throw new Error('release activation cache is absent, partial, executable, or not the exact Registry-owned account');
  }
  const bytes = account.data;
  if (ascii(bytes, 0, 8) !== 'DCLTACT1' || u16(bytes, 8) !== 1 || u16(bytes, 10) !== 1) {
    throw new Error('release activation cache has the wrong exact magic, schema, or profile');
  }
  requireZero(bytes, 12, 4, 'release activation cache header');
  const releaseIdentity = slice(bytes, 16, 32);
  requireNonzero(releaseIdentity, 'release activation identity');
  const expectedCache = PublicKey.findProgramAddressSync(
    [REGISTRY_ACTIVATION_PDA_SEED_V1, releaseIdentity],
    new PublicKey(registryProgram),
  )[0].toBase58();
  if (expectedCache !== cacheAddress) throw new Error('release activation cache is not the release-derived Registry PDA');

  const releaseBytes = new Uint8Array(EXECUTION_RELEASE_SET_BYTES);
  releaseBytes.set(new TextEncoder().encode('DCLTRLS1'));
  const releaseView = new DataView(releaseBytes.buffer);
  releaseView.setUint16(8, 1, true);
  releaseView.setUint16(10, 1, true);
  const artifacts = {} as Record<RegistryRole, ArtifactReleaseV1>;
  for (const [index, role] of REGISTRY_ROLES.entries()) {
    const offset = REGISTRY_ACTIVATION_CACHE_ROLES_OFFSET + index * REGISTRY_ACTIVATED_ROLE_BYTES;
    const artifactId = slice(bytes, offset, 32);
    requireNonzero(artifactId, `${role} activated artifact identity`);
    const artifact = decodeArtifactReleaseV1(slice(bytes, offset + 32, ARTIFACT_RELEASE_BYTES));
    if (!same(await sha256(artifact.bytes), artifactId)) throw new Error(`${role} activated artifact bytes do not hash to their identity`);
    if (artifact.loader !== UPGRADEABLE_LOADER_ID) throw new Error(`${role} activated artifact does not bind Loader v3`);
    requireSlotPinnedReleaseV1(artifact, `${role} activated artifact release`);
    releaseBytes.set(new PublicKey(artifact.program).toBytes(), 16 + index * 64);
    releaseBytes.set(artifactId, 48 + index * 64);
    artifacts[role] = artifact;
  }
  const releaseSet = await decodeExecutionReleaseSetV1(releaseBytes);
  if (releaseSet.id !== hex(releaseIdentity)) throw new Error('release activation cache does not rebuild its selected release-set identity');
  for (const role of REGISTRY_ROLES) {
    if (releaseSet.roles[role].program !== artifacts[role].program) throw new Error(`${role} activated artifact does not implement its release-set binding`);
  }
  return Object.freeze({ releaseSetId: releaseSet.id, artifacts: Object.freeze(artifacts) });
}

function exactLoaderPair(
  role: OperatorRole,
  programAddress: string,
  program: RpcAccount,
  programDataAddress: string,
  programData: RpcAccount,
  recordedSlot: string,
  expectedAuthority: string,
): Readonly<{ deploymentSlot: string; upgradeAuthority: string }> {
  if (program.owner !== UPGRADEABLE_LOADER_ID || !program.executable
      || program.data.length !== LOADER_V3_PROGRAM_BYTES || program.space !== LOADER_V3_PROGRAM_BYTES) {
    throw new Error(`${role} program is not an exact Loader-v3 executable Program account`);
  }
  const programView = new DataView(program.data.buffer, program.data.byteOffset, program.data.byteLength);
  if (programView.getUint32(0, true) !== 2 || new PublicKey(program.data.slice(4)).toBase58() !== programDataAddress) {
    throw new Error(`${role} Program does not link to the preset ProgramData account`);
  }
  const derived = PublicKey.findProgramAddressSync(
    [new PublicKey(programAddress).toBytes()],
    new PublicKey(UPGRADEABLE_LOADER_ID),
  )[0].toBase58();
  if (derived !== programDataAddress) throw new Error(`${role} ProgramData is not the canonical Loader-v3 coordinate`);
  if (programData.owner !== UPGRADEABLE_LOADER_ID || programData.executable
      || programData.data.length !== LOADER_V3_PROGRAMDATA_OFFSET || programData.space <= LOADER_V3_PROGRAMDATA_OFFSET) {
    throw new Error(`${role} ProgramData is absent or not a Loader-v3 ProgramData account`);
  }
  const programDataView = new DataView(programData.data.buffer, programData.data.byteOffset, programData.data.byteLength);
  if (programDataView.getUint32(0, true) !== 3 || programData.data[12] !== 1) {
    throw new Error(`${role} ProgramData is not the exact mutable Loader-v3 header pinned by this devnet preset`);
  }
  const observedAuthority = new PublicKey(programData.data.slice(13, 45)).toBase58();
  if (observedAuthority !== expectedAuthority) throw new Error(`${role} ProgramData upgrade authority differs from the activated exact-authority release`);
  const observedSlot = littleU64(programData.data, 4, `${role} ProgramData deployment slot`);
  // The deployment slot is STATE, not identity. Every check above -- Loader
  // ownership, the executable flag, the 36-byte Program body, its link to this
  // ProgramData, the canonical PDA, the mutable header, and the exact upgrade
  // AUTHORITY -- survives an upgrade in place. The slot does not, by design.
  //
  // Asserting it against a slot baked into the shipped manifest made an
  // ordinary upgrade indistinguishable from an attack, and the manifest loses
  // that race by construction: it is fixed at build time and the chain moves
  // afterwards. Five of seven devnet roles moved on 2026-08-29 and every
  // caller of this preset refused from then on, for no fault at all.
  //
  // Backwards is still refused. The genesis hash pinned the cluster, so a
  // deployment slot EARLIER than the recorded one cannot be a later state of
  // this program; it is a stale or wrong-generation observation.
  if (BigInt(observedSlot) < BigInt(recordedSlot)) {
    throw new Error(`${role} DeploymentSlotMismatch: this is a stale or wrong-generation observation; the preset records slot ${recordedSlot}, and finalized chain state reports the earlier ${observedSlot}`);
  }
  return Object.freeze({ deploymentSlot: observedSlot, upgradeAuthority: observedAuthority });
}

export async function acquireOperatorSurfaceV1(
  client: SolanaRpcClient,
  coordinates: OperatorCoordinatesV1,
  deploymentPreset: OperatorDeploymentPresetV1 | null = null,
): Promise<OperatorSurfaceSnapshotV1> {
  const roleAddresses = OPERATOR_ROLES.map((role) => canonicalKey(coordinates[role], `${role} program`));
  if (new Set(roleAddresses).size !== roleAddresses.length) throw new Error('multiprogram roles must have distinct executable program identities');
  const market = coordinates.market === undefined || coordinates.market === ''
    ? null
    : canonicalKey(coordinates.market, 'Market');
  const realm = coordinates.realm === undefined || coordinates.realm === ''
    ? null
    : canonicalKey(coordinates.realm, 'Realm');
  const stateAddresses = [realm, market].filter((address): address is string => address !== null);
  if (stateAddresses.some((address) => roleAddresses.includes(address))) throw new Error('Realm or Market aliases an executable program role');
  if (new Set(stateAddresses).size !== stateAddresses.length) throw new Error('Realm and Market must have distinct state identities');
  if (deploymentPreset !== null) {
    for (const role of OPERATOR_ROLES) {
      if (coordinates[role] !== deploymentPreset.coordinates[role]) throw new Error(`${role} program differs from the loaded live-devnet preset`);
    }
    const facts = await client.probe();
    if (facts.genesisHash !== deploymentPreset.genesisHash) {
      throw new Error(`live-devnet preset refused: the endpoint reports ${facts.genesisHash}, not Solana devnet genesis`);
    }
  }
  const floor = await client.finalizedSlot();
  const programDataAddresses = deploymentPreset === null
    ? []
    : OPERATOR_ROLES.map((role) => deploymentPreset.evidence[role].programData);
  const presetAddresses = deploymentPreset === null ? [] : [deploymentPreset.activationCache];
  const addresses = [...roleAddresses, ...presetAddresses, ...stateAddresses];
  const observation = await client.multipleAccounts(addresses, floor);
  if (BigInt(observation.slot) < BigInt(floor)) throw new Error('program/cache observation predates its finalized floor');
  let checkedObservedSlot = observation.slot;
  const roles = OPERATOR_ROLES.map((role, index) => {
    const account = requireAccount(observation.accounts[index].account, `${role} program`);
    if (!account.executable) throw new Error(`${role} program is not executable`);
    return Object.freeze({
      role,
      address: roleAddresses[index],
      owner: account.owner,
      executable: account.executable,
      dataBytes: account.data.length,
    });
  });
  let presetObservation: OperatorSurfaceSnapshotV1['deploymentPreset'] = null;
  if (deploymentPreset !== null) {
    const activationIndex = roleAddresses.length;
    const activation = await exactActivationCache(
      requireAccount(observation.accounts[activationIndex].account, 'release activation cache'),
      coordinates.registry,
      deploymentPreset.activationCache,
    );
    const activatedAuthorities = new Set<string>();
    for (const role of REGISTRY_ROLES) {
      const artifact = activation.artifacts[role];
      if (artifact.program !== deploymentPreset.coordinates[role]
          || artifact.programData !== deploymentPreset.evidence[role].programData
          || artifact.deploymentSlot.toString() !== deploymentPreset.evidence[role].deploymentSlot) {
        throw new Error(`${role} activation-cache release does not join the preset Program, ProgramData, and deployment slot`);
      }
      if (artifact.upgradeAuthority === null) throw new Error(`${role} activation-cache release is not the exact-authority devnet generation`);
      activatedAuthorities.add(artifact.upgradeAuthority);
    }
    if (activatedAuthorities.size !== 1) throw new Error('the checked live-devnet generation does not bind one shared retained upgrade authority');
    const sharedUpgradeAuthority = [...activatedAuthorities][0];

    // ProgramData contains the whole program ELF. Six bodies exceed the
    // browser's bounded RPC response, while the exact Loader identity and slot
    // live entirely in the first 45 bytes. Read that header only, after the
    // Program/cache observation, at no older finalized context.
    const loaderHeaders = await client.multipleAccountDataSlices(
      programDataAddresses,
      0,
      LOADER_V3_PROGRAMDATA_OFFSET,
      observation.slot,
    );
    if (BigInt(loaderHeaders.slot) < BigInt(observation.slot)) throw new Error('ProgramData header observation predates the Program/cache observation');
    checkedObservedSlot = loaderHeaders.slot;
    const deploymentSlots = {} as Record<OperatorRole, string>;
    const upgradeAuthorities = {} as Record<OperatorRole, string>;
    for (const [index, role] of OPERATOR_ROLES.entries()) {
      const expectedAuthority = role === 'registry'
        ? sharedUpgradeAuthority
        : activation.artifacts[role].upgradeAuthority;
      if (expectedAuthority === null) throw new Error(`${role} activated release unexpectedly lacks an upgrade authority`);
      const loader = exactLoaderPair(
        role,
        roleAddresses[index],
        requireAccount(observation.accounts[index].account, `${role} program`),
        programDataAddresses[index],
        requireAccount(loaderHeaders.accounts[index].account, `${role} ProgramData`),
        deploymentPreset.evidence[role].deploymentSlot,
        expectedAuthority,
      );
      deploymentSlots[role] = loader.deploymentSlot;
      upgradeAuthorities[role] = loader.upgradeAuthority;
    }
    presetObservation = Object.freeze({
      label: deploymentPreset.label,
      genesisHash: deploymentPreset.genesisHash,
      activationCache: deploymentPreset.activationCache,
      executionReleaseSetId: activation.releaseSetId,
      deploymentSlots: Object.freeze(deploymentSlots),
      upgradeAuthorities: Object.freeze(upgradeAuthorities),
      upgradedSinceRecord: Object.freeze(OPERATOR_ROLES.filter(
        (role) => BigInt(deploymentSlots[role]) > BigInt(deploymentPreset.evidence[role].deploymentSlot),
      )),
      routeSpecificReleaseAdmission: ROUTE_SPECIFIC_RELEASE_ADMISSION_UNPROVEN_V1,
    });
  }
  let nextStateIndex = roleAddresses.length + presetAddresses.length;
  let realmObservation: OperatorSurfaceSnapshotV1['realm'] = null;
  if (realm !== null) {
    const account = requireAccount(observation.accounts[nextStateIndex].account, 'Realm');
    nextStateIndex += 1;
    if (account.executable) throw new Error('Realm is executable');
    if (account.owner !== coordinates.core) throw new Error('Realm is not owned by the selected Core program');
    realmObservation = Object.freeze({
      address: realm,
      owner: account.owner,
      dataBytes: account.data.length,
      header: classifyHeader(account.data),
    });
  }
  let marketObservation: OperatorSurfaceSnapshotV1['market'] = null;
  if (market !== null) {
    const account = requireAccount(observation.accounts[nextStateIndex].account, 'Market');
    if (account.executable) throw new Error('Market is executable');
    if (account.owner !== coordinates.core) throw new Error('Market is not owned by the selected Core program');
    marketObservation = Object.freeze({
      address: market,
      owner: account.owner,
      dataBytes: account.data.length,
      header: classifyHeader(account.data),
    });
  }
  return Object.freeze({ observedSlot: checkedObservedSlot, roles: Object.freeze(roles), deploymentPreset: presetObservation, realm: realmObservation, market: marketObservation });
}
