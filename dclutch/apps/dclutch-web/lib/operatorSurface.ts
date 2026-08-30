import { PublicKey } from '@solana/web3.js';

import { classifyHeader } from './decoders';
import {
  DEVNET_DEPLOYMENT_V1,
  DEVNET_PROGRAM_EVIDENCE_V1,
  type DeploymentV1,
  type ProgramEvidenceV1,
} from './deployments';
import {
  ACTIVATION_CACHE_BYTES,
  LOADER_V3_PROGRAM_BYTES,
  LOADER_V3_PROGRAMDATA_OFFSET,
  UPGRADEABLE_LOADER_ID,
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

export type OperatorSurfaceSnapshotV1 = Readonly<{
  observedSlot: string;
  roles: ReadonlyArray<OperatorRoleObservationV1>;
  deploymentPreset: null | Readonly<{
    label: OperatorDeploymentPresetV1['label'];
    genesisHash: string;
    activationCache: string;
    /** Live deployment slots, read from each ProgramData header this run. */
    deploymentSlots: Readonly<Record<OperatorRole, string>>;
    /**
     * Roles whose live slot is past the one the shipped manifest recorded.
     *
     * An upgrade in place at a permanent address is ordinary and expected, so
     * this is a statement about the manifest's age, not a fault. It is
     * surfaced rather than thrown because the operator is the one who knows
     * whether an upgrade was supposed to have happened.
     */
    upgradedSinceRecord: ReadonlyArray<OperatorRole>;
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

export const LIVE_DEVNET_OPERATOR_PRESET_V1 = checkedLiveDevnetOperatorPresetV1(
  DEVNET_DEPLOYMENT_V1 satisfies DeploymentV1,
  DEVNET_PROGRAM_EVIDENCE_V1,
);

function requireAccount(account: RpcAccount | null, field: string): RpcAccount {
  if (account === null) throw new Error(`${field} is absent at the finalized observation floor`);
  return account;
}

function littleU64(bytes: Uint8Array, offset: number, field: string): string {
  if (bytes.length < offset + 8) throw new Error(`${field} is truncated`);
  return new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength).getBigUint64(offset, true).toString();
}

function exactLoaderPair(
  role: OperatorRole,
  programAddress: string,
  program: RpcAccount,
  programDataAddress: string,
  programData: RpcAccount,
  recordedSlot: string,
): string {
  if (program.owner !== UPGRADEABLE_LOADER_ID || !program.executable || program.data.length !== LOADER_V3_PROGRAM_BYTES) {
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
  if (programDataView.getUint32(0, true) !== 3 || programData.data[12] > 1) throw new Error(`${role} ProgramData has an invalid Loader-v3 header`);
  const observedSlot = littleU64(programData.data, 4, `${role} ProgramData deployment slot`);
  // The deployment slot is STATE, not identity. Every check above this line is
  // identity -- Loader ownership, the executable flag, the exact 36-byte
  // Program body, its link to this ProgramData, the canonical PDA derivation,
  // the ProgramData header tag -- and none of them move when a program is
  // upgraded in place at a permanent address. The slot does, by design, on
  // every upgrade.
  //
  // Requiring it to equal a slot baked into the shipped manifest made an
  // ordinary upgrade indistinguishable from an attack, and the manifest cannot
  // win that race: it is a build-time constant and the chain moves after the
  // build. Five of the seven devnet roles were upgraded on 2026-08-29 and the
  // whole /operate live-devnet preset had been refusing ever since -- not
  // because anything was wrong, but because the constant had aged. A check
  // that fires on correct behaviour is not protecting anyone.
  //
  // Backwards is still refused, and that one is real: the genesis hash already
  // pinned the cluster, so an observation OLDER than the recorded deployment
  // cannot be a later state of the same program. Forward is reported by the
  // caller instead, which is what an operator actually needs to know.
  if (BigInt(observedSlot) < BigInt(recordedSlot)) {
    throw new Error(`${role} DeploymentSlotMismatch: this is a stale or wrong-generation observation; the preset records slot ${recordedSlot}, and finalized chain state reports the earlier ${observedSlot}`);
  }
  return observedSlot;
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
    checkedObservedSlot = loaderHeaders.slot;
    const deploymentSlots = {} as Record<OperatorRole, string>;
    for (const [index, role] of OPERATOR_ROLES.entries()) {
      deploymentSlots[role] = exactLoaderPair(
        role,
        roleAddresses[index],
        requireAccount(observation.accounts[index].account, `${role} program`),
        programDataAddresses[index],
        requireAccount(loaderHeaders.accounts[index].account, `${role} ProgramData`),
        deploymentPreset.evidence[role].deploymentSlot,
      );
    }
    const activationIndex = roleAddresses.length;
    const activation = requireAccount(observation.accounts[activationIndex].account, 'release activation cache');
    if (activation.owner !== coordinates.registry || activation.executable || activation.data.length !== ACTIVATION_CACHE_BYTES) {
      throw new Error('release activation cache is absent, partial, executable, or not owned by the preset Registry program');
    }
    presetObservation = Object.freeze({
      label: deploymentPreset.label,
      genesisHash: deploymentPreset.genesisHash,
      activationCache: deploymentPreset.activationCache,
      deploymentSlots: Object.freeze(deploymentSlots),
      upgradedSinceRecord: Object.freeze(OPERATOR_ROLES.filter(
        (role) => BigInt(deploymentSlots[role]) > BigInt(deploymentPreset.evidence[role].deploymentSlot),
      )),
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
