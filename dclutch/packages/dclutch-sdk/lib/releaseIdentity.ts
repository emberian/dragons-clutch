/**
 * Frame/ABI selection by ON-CHAIN release identity.
 *
 * THE PROBLEM THIS DISSOLVES. A dClutch program is upgraded in place: the
 * permanent-ID ladder keeps all seven program addresses across every cohort,
 * on purpose. So a program address tells a client NOTHING about which code it
 * is talking to, and a client built at some commit has no way, from its own
 * bundle, to know whether the chain still speaks the frames it was generated
 * against. On 2026-08-29 that gap cost most of a day: a client whose founding
 * frame was one account wide of the live program refused as `0x4001`
 * (`TradingSbfError::Release`) forty accounts deep, after four legs of a
 * composed founding had already executed, and the diagnosis took three lanes
 * and a dozen burned market mints. Every attempted remedy was a guess about
 * WHICH BUILD the chain was running — composite clients assembled from
 * cherry-picks, era windows measured in minutes of git history.
 *
 * None of that archaeology was ever necessary. The chain SAYS which release is
 * live. The Registry owns an activation cache (`DCLTACT1`) whose body is an
 * exact projection of the five finalized `ArtifactReleaseV1` records that were
 * activated, and each of those records carries a `semantic_release_id`. Clients
 * simply never asked.
 *
 * So: identity is what you assert; state is what you read.
 *
 * WHY THE KEY IS THE SEMANTIC RELEASE ID, NOT THE RELEASE SET ID. A release set
 * id is the hash of the whole activated set, so it moves whenever ANY role is
 * redeployed — including a rebuild that changes no wire at all. Keying ABI
 * tables on it would make a client refuse on every cohort bump. The semantic
 * release id is derived from the role's SOURCE (DEPLOY_1.md §3 records the
 * preimage as `SHA-256("dclutch/deploy-1/semantic-release/v1\nrole=<role>\n
 * commit=<commit>")`), which is exactly what a generated ABI table describes.
 * This is observable, not theoretical: read live on devnet 2026-08-29, Trading
 * and Resolution held the SAME semantic release id across cohorts 2, 3, 4 and 5
 * while their ELF digests and deployment slots moved every time. A table keyed
 * on semantics survives a rebuild and refuses a real wire change, which is the
 * discrimination a client actually needs.
 *
 * The protocol already does this to itself on chain, so this module is the
 * protocol's own idiom moved one RPC round earlier:
 * `dclutch-resolution-core-v3-operator`'s `authenticate_role_semantic_release`
 * refuses when Resolution's activated semantic release is not the one it was
 * built for. This module is that refusal, client-side, before a signature.
 *
 * WHAT THIS GUARANTEES, EXACTLY — stated narrowly on purpose. It guarantees a
 * client REFUSES TO ACT against a release whose identity it was not built and
 * pinned for, and names both identities when it does. It does NOT verify that
 * a pinned table's frames are correct for the release it claims: that is the
 * separate job of the `abi:*:verify` generator gates, which check the generated
 * modules against the Rust authorities. The two together are the whole story;
 * neither substitutes for the other.
 */
import { PublicKey } from '@solana/web3.js';

import { decodeActivationCacheV1, type ActivatedProjectionV1 } from './infrastructure';
import { ACTIVATION_CACHE_BYTES, REGISTRY_ROLES, type RegistryRole } from './releaseRegistry';
import { type SolanaRpcClient } from './rpc';

import { CORE_FOUND_ACCOUNT_COUNT_V3 } from './generated/coreFound';
import { CLAIMS_CUSTODY_REPLAY_ACCOUNT_COUNT_V1 } from './generated/claimsCustodyReplayV1';
import { HOT_FIXED_ACCOUNT_COUNT_V3 } from './generated/directInlineV3';
import {
  CLAIMS_FOUNDING_ACCOUNT_COUNT_V5,
  CORE_FOUND_TRADING_PROGRAM_INDEX_V1,
  GENERIC_FOUNDING_FOUND_FIXED_ACCOUNT_COUNT_V1,
  GENERIC_FOUNDING_OPEN_ACCOUNT_COUNT_V1,
  PROJECTED_FOUND_ACCOUNT_COUNT_V2,
} from './generated/genericFoundingV1';
import { TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3 } from './generated/walletTerminalPayoutV3';

/** Offset of the deployment slot inside a Loader V3 ProgramData account. */
const PROGRAMDATA_DEPLOYMENT_SLOT_OFFSET = 4;
const PROGRAMDATA_HEADER_BYTES = 45;

/** One activated role's identity, exactly as the Registry cache states it. */
export type ExecutionRoleIdentityV1 = Readonly<{
  /** Content id of the finalized `ArtifactReleaseV1` record. */
  artifactReleaseId: string;
  /** The role's SOURCE-derived semantic identity — the ABI selection key. */
  semanticReleaseId: string;
  program: string;
  programData: string;
  elfDigest: string;
  /** Deployment slot the activation PINNED. State, not identity. */
  deploymentSlot: string;
}>;

/** What the chain says about which release is live. */
export type ExecutionReleaseIdentityV1 = Readonly<{
  /** Finalized slot the identity was observed at. */
  observedSlot: string;
  registryProgram: string;
  activationCache: string;
  /** Hash of the whole activated set — moves on any redeploy. */
  executionReleaseSetId: string;
  roles: Readonly<Record<RegistryRole, ExecutionRoleIdentityV1>>;
}>;

/**
 * The frame widths a client builds. These are the coordinates whose drift
 * presents on chain as a `0x4001`-family refusal deep inside a composed
 * instruction, so they are what a table pins. Sourced from the generated
 * modules for the current entry, which is why the current entry cannot
 * silently disagree with what the client actually builds.
 */
export type AbiFrameFactsV1 = Readonly<{
  coreFoundAccountCount: number;
  coreFoundTradingProgramIndex: number;
  genericFoundingFoundFixedAccountCount: number;
  genericFoundingOpenAccountCount: number;
  projectedFoundAccountCount: number;
  claimsFoundingAccountCount: number;
  claimsCustodyReplayAccountCount: number;
  directHotFixedAccountCount: number;
  terminalSettlementAccountCount: number;
}>;

/**
 * One ABI table, and the on-chain release identity it describes.
 *
 * A cohort ships => append ONE entry. See the SDK README, "Shipping a new
 * cohort". Historical entries carry pinned literals; the CURRENT entry reads
 * its frame facts out of the live generated modules.
 */
export type AbiReleaseTableV1 = Readonly<{
  label: string;
  /** Where the pinned identity was OBSERVED. Never a guess. */
  provenance: string;
  semanticReleaseIds: Readonly<Record<RegistryRole, string>>;
  abi: AbiFrameFactsV1;
}>;

/**
 * The frame facts of the modules this build actually generated.
 *
 * Read from the generated singletons rather than transcribed, so that a
 * regeneration moves this in lockstep and a stale literal cannot survive here
 * the way `CORE_FOUND_TRADING_PROGRAM_INDEX_V1 = 25` survived in the browser
 * behind a fail-closed generator on 2026-08-29.
 */
export const CURRENT_ABI_FRAME_FACTS_V1: AbiFrameFactsV1 = Object.freeze({
  coreFoundAccountCount: CORE_FOUND_ACCOUNT_COUNT_V3,
  coreFoundTradingProgramIndex: CORE_FOUND_TRADING_PROGRAM_INDEX_V1,
  genericFoundingFoundFixedAccountCount: GENERIC_FOUNDING_FOUND_FIXED_ACCOUNT_COUNT_V1,
  genericFoundingOpenAccountCount: GENERIC_FOUNDING_OPEN_ACCOUNT_COUNT_V1,
  projectedFoundAccountCount: PROJECTED_FOUND_ACCOUNT_COUNT_V2,
  claimsFoundingAccountCount: CLAIMS_FOUNDING_ACCOUNT_COUNT_V5,
  claimsCustodyReplayAccountCount: CLAIMS_CUSTODY_REPLAY_ACCOUNT_COUNT_V1,
  directHotFixedAccountCount: HOT_FIXED_ACCOUNT_COUNT_V3,
  terminalSettlementAccountCount: TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3,
});

/**
 * The one release identity this build is pinned to.
 *
 * OBSERVED, not asserted: these five semantic release ids were decoded from
 * the live Registry activation cache
 * `77PrN82TY4rrQwUjyKBM14A1n3qxktHrN8vd2RcacovK` (release set
 * `094336271db1146f09f6ff419488af2d3174da762d3b2b468fac635754aa862d`) on
 * public devnet at finalized slot 490054895, 2026-08-29, and the five pinned
 * deployment slots in that cache were confirmed equal to the five live
 * ProgramData accounts' deployment slots in the same reading.
 *
 * There is exactly ONE entry and no historical ones. Tables for releases this
 * repository never observed would be fabrications, and a fabricated table is
 * worse than a refusal: it would select silently and be wrong.
 */
export const DEVNET_COHORT_5_ABI_RELEASE_V1: AbiReleaseTableV1 = Object.freeze({
  label: 'devnet cohort-5',
  provenance: 'Decoded from the live Registry activation cache 77PrN82TY4rrQwUjyKBM14A1n3qxktHrN8vd2RcacovK on public devnet at finalized slot 490054895 (2026-08-29); its five pinned deployment slots matched the five live ProgramData accounts in the same reading.',
  semanticReleaseIds: Object.freeze({
    core: '4079ba3e948078500e49821823a50847614c243d2f7093badb59f83515a4c6a6',
    claims: '73a9212256d7e8257f1b4f303a8d1d75ae5559e9fb158f7fe42d9b9c42fa3c3e',
    trading: '79fad2f04f8d9ce07d76c809fe116db8ef9374adbeb15e62f603235c3a2b96b9',
    resolution: '6e4b9a545277cf68731108fe1729ff047affe72e16d79c3930acadc8016f554a',
    custody: '141880f45a4195a1e2a6381c17a75714448ea61efefe5c59227537650b6680b0',
  }),
  abi: CURRENT_ABI_FRAME_FACTS_V1,
});

/**
 * The release identity public devnet is running now.
 *
 * The five ids are OBSERVED, by the same call this module exports: they were
 * decoded from the live Registry activation cache
 * `3hFTU9ka7fryKrVY7s8Lm5ZMCHsnq5bxEGcgSCd6TSiu` (release set
 * `9895faee8f7f6a1926df18302f1b003afcf4b6c56518ba7bba2614c86eea8a22`) at
 * finalized slot 492,954,022 on 2026-09-04, and the five deployment slots that
 * cache pins were equal to the five the deployment manifest carries in the same
 * reading. Trading and Resolution are BYTE-IDENTICAL to cohort-5's: those two
 * roles' semantics have not moved in ten cohorts, which is the useful thing a
 * table of observations says and a table of guesses cannot.
 *
 * AND THE FRAME FACTS ARE NOT A GUESS EITHER, which is the conjunct that makes
 * this an observation rather than the fabrication the note above forbids.
 * `CURRENT_ABI_FRAME_FACTS_V1` is whatever THIS build generated, and cohort-15
 * was deployed from `1cae26fd6`; every one of the nine counts is byte-identical
 * between that commit's generated modules and this build's, and the one
 * generated file that moved at all (`generated/coreFound.ts`) moved by ADDING
 * StatisticSpecV1 offsets and changed no count. So the frame this table offers
 * is the frame those programs were built with.
 *
 * WHAT THIS UNBLOCKS. `selectAbiReleaseV1` refused every cohort from 6 through
 * 15 — correctly, fail-closed, "refusing to build a frame against semantics it
 * was not generated for" — which meant no browser could open a release-bound
 * session against any live deployment. The refusal was never the defect; the
 * missing row was, and a row is one observation per cohort.
 */
export const DEVNET_COHORT_15_ABI_RELEASE_V1: AbiReleaseTableV1 = Object.freeze({
  label: 'devnet cohort-15',
  provenance: 'Decoded from the live Registry activation cache 3hFTU9ka7fryKrVY7s8Lm5ZMCHsnq5bxEGcgSCd6TSiu on public devnet at finalized slot 492954022 (2026-09-04) by discoverCurrentActivationCacheV1; its five pinned deployment slots equalled the five the deployment manifest records for this cohort in the same reading. The frame facts are this build\u2019s own generated modules, whose nine counts are byte-identical to those at 1cae26fd6, the commit cohort-15 was deployed from.',
  semanticReleaseIds: Object.freeze({
    core: 'f069d9ef9ed59cace372746436245ec4f766baccd64b04a4cf023c5d51f0b89a',
    claims: '4d43ae7308d64002d52def66c6c889c9af70f9d7583ebd3ae4a23ac63f93f196',
    trading: '79fad2f04f8d9ce07d76c809fe116db8ef9374adbeb15e62f603235c3a2b96b9',
    resolution: '6e4b9a545277cf68731108fe1729ff047affe72e16d79c3930acadc8016f554a',
    custody: '9bc89cbb7e30eec5ebc98a83658f56a2eb525d70f575b48997f3b610d1913721',
  }),
  abi: CURRENT_ABI_FRAME_FACTS_V1,
});

/** Every release identity this build carries an ABI table for. */
export const KNOWN_ABI_RELEASES_V1: ReadonlyArray<AbiReleaseTableV1> = Object.freeze([
  DEVNET_COHORT_5_ABI_RELEASE_V1,
  DEVNET_COHORT_15_ABI_RELEASE_V1,
]);

function shortId(value: string): string {
  return value.length <= 16 ? value : `${value.slice(0, 16)}…`;
}

/**
 * Read which release the chain says is live. ONE RPC round.
 *
 * The decode is the Registry contract's own hostile projection
 * (`decodeActivationCacheV1`): exact width, `DCLTACT1` magic, schema, profile,
 * reserved-zero, the cache PDA re-derived from the release-set identity it
 * claims, every artifact record hashed to its own content id, and the five-role
 * projection hashed to the release-set id. A counterfeit cache cannot answer
 * this call.
 */
export async function readExecutionReleaseIdentityV1(
  client: Pick<SolanaRpcClient, 'accountInfo'>,
  input: Readonly<{ registryProgram: string; activationCache: string }>,
): Promise<ExecutionReleaseIdentityV1> {
  const registryProgram = new PublicKey(input.registryProgram).toBase58();
  const activationCache = new PublicKey(input.activationCache).toBase58();
  const observation = await client.accountInfo(activationCache);
  const account = observation.account;
  if (account === null) {
    throw new Error(
      `the deployment names activation cache ${activationCache}, and no account exists there — this client cannot learn which release is live, so it will not build a frame`,
    );
  }
  if (account.owner !== registryProgram) {
    throw new Error(
      `activation cache ${activationCache} is owned by ${account.owner}, not the Registry program ${registryProgram} — it is not an activation cache`,
    );
  }
  const projection = await decodeActivationCacheV1(account.data, registryProgram, activationCache);
  return identityFromProjection(projection, registryProgram, activationCache, observation.slot);
}

/** Project one decoded cache into the identity a client selects on. */
function identityFromProjection(
  projection: ActivatedProjectionV1,
  registryProgram: string,
  activationCache: string,
  observedSlot: string,
): ExecutionReleaseIdentityV1 {
  const roles = Object.fromEntries(REGISTRY_ROLES.map((role) => {
    const release = projection.artifacts[role];
    return [role, Object.freeze({
      artifactReleaseId: projection.artifactIds[role],
      semanticReleaseId: release.semanticReleaseId,
      program: release.program,
      programData: release.programData,
      elfDigest: release.elfDigest,
      deploymentSlot: release.deploymentSlot.toString(),
    })];
  })) as Record<RegistryRole, ExecutionRoleIdentityV1>;
  return Object.freeze({
    observedSlot,
    registryProgram,
    activationCache,
    executionReleaseSetId: projection.releaseSetId,
    roles: Object.freeze(roles),
  });
}

/**
 * The live deployment slot of each named ProgramData, or null where absent.
 *
 * Reads the 45-byte Loader V3 header only. A ProgramData account carries a
 * whole ELF, and nothing here needs one.
 */
async function readDeploymentSlotsV1(
  client: Pick<SolanaRpcClient, 'multipleAccountDataSlices'>,
  addresses: ReadonlyArray<string>,
): Promise<ReadonlyArray<string | null>> {
  const observation = await client.multipleAccountDataSlices(addresses, 0, PROGRAMDATA_HEADER_BYTES);
  return addresses.map((_, index) => {
    const account = observation.accounts[index]?.account;
    if (account === null || account === undefined) return null;
    const data = account.data;
    if (data.length < PROGRAMDATA_DEPLOYMENT_SLOT_OFFSET + 8) return null;
    return new DataView(data.buffer, data.byteOffset, data.byteLength)
      .getBigUint64(PROGRAMDATA_DEPLOYMENT_SLOT_OFFSET, true)
      .toString();
  });
}

/**
 * Confirm the named cache is the CURRENT one. ONE further RPC round.
 *
 * A superseded activation cache is not deleted and does not decay: it keeps its
 * Registry owner, its `DCLTACT1` magic and its exact width forever, so every
 * cheap health check on it passes. Only its CONTENT ages. This is not
 * hypothetical — on 2026-08-29 the shipped devnet manifest named a cache four
 * cohorts stale, and it had passed an existence-owner-magic audit that morning.
 *
 * The activation pins each role's deployment slot, and
 * `ArtifactReleaseV1::authenticate_deployment` re-checks it on chain, so a
 * stale cache is not merely wrong metadata: every route that re-authenticates
 * a role against it MUST refuse. This turns that refusal into a named answer.
 */
export async function authenticateReleaseCurrencyV1(
  client: Pick<SolanaRpcClient, 'multipleAccountDataSlices'>,
  identity: ExecutionReleaseIdentityV1,
): Promise<void> {
  const addresses = REGISTRY_ROLES.map((role) => identity.roles[role].programData);
  const live = await readDeploymentSlotsV1(client, addresses);
  const superseded: string[] = [];
  REGISTRY_ROLES.forEach((role, index) => {
    const observed = live[index];
    if (observed === null) {
      superseded.push(`${role}: ProgramData ${identity.roles[role].programData} is absent or too short to carry a deployment slot`);
      return;
    }
    const pinned = identity.roles[role].deploymentSlot;
    if (observed !== pinned) superseded.push(`${role} pinned slot ${pinned}, live slot ${observed}`);
  });
  if (superseded.length > 0) {
    throw new Error(
      `activation cache ${identity.activationCache} (release set ${shortId(identity.executionReleaseSetId)}) has been SUPERSEDED — it is not the current cache, and every route that re-authenticates a role against it will refuse on chain: ${superseded.join('; ')}. Find the current cache by reading the Registry program's accounts of width 1288 and taking the one whose pinned slots equal the live ProgramData slots, then update the deployment manifest's activationCache.`,
    );
  }
}

/**
 * Choose the ABI table for an observed release identity, or refuse by name.
 *
 * The refusal is the deliverable. It replaces the failure mode this module
 * exists to kill: a frame mismatch presenting as an opaque `0x4001` deep inside
 * a composed on-chain instruction, diagnosable only by archaeology across
 * client and program git history.
 */
export function selectAbiReleaseV1(
  identity: ExecutionReleaseIdentityV1,
  releases: ReadonlyArray<AbiReleaseTableV1> = KNOWN_ABI_RELEASES_V1,
): AbiReleaseTableV1 {
  const selected = releases.find((release) => REGISTRY_ROLES.every(
    (role) => release.semanticReleaseIds[role] === identity.roles[role].semanticReleaseId,
  ));
  if (selected !== undefined) return selected;
  const observed = REGISTRY_ROLES
    .map((role) => `${role}=${shortId(identity.roles[role].semanticReleaseId)}`)
    .join(' ');
  const known = releases.length === 0
    ? 'none'
    : releases.map((release) => {
      const differing = REGISTRY_ROLES.filter(
        (role) => release.semanticReleaseIds[role] !== identity.roles[role].semanticReleaseId,
      );
      return `"${release.label}" (differs on ${differing.join(', ')})`;
    }).join(', ');
  throw new Error(
    `this client has no ABI table for the release the chain is running. Chain says: release set ${shortId(identity.executionReleaseSetId)}, semantics ${observed} (activation cache ${identity.activationCache}, observed at slot ${identity.observedSlot}). This client carries: ${known}. Refusing to build a frame against semantics it was not generated for — regenerate the ABI modules against the live release and append its table, rather than letting the frame mismatch surface on chain.`,
  );
}

/**
 * Find the current activation cache by reading the chain, not a constant.
 *
 * A new cohort activates a new release set, which mints a new cache at a new
 * PDA. Superseded caches are never deleted, so the Registry accumulates one
 * 1288-byte cache per cohort and a shipped address ages the moment a cohort
 * lands. Rather than requiring a human to update a constant — the exact
 * failure this module exists to abolish — derive the answer:
 *
 *   1. every 1288-byte account the Registry owns (one server-filtered call),
 *   2. the live deployment slot of each role's ProgramData (one call — the
 *      program addresses are permanent, so all cohorts name the same five),
 *   3. the cache whose five pinned slots equal those five live slots.
 *
 * Two RPC rounds regardless of cohort count. Exactly one cache can satisfy
 * step 3, because the on-chain reauthentication that the protocol performs
 * accepts exactly that one; finding none is a real alarm, not a fallback.
 */
export async function discoverCurrentActivationCacheV1(
  client: Pick<SolanaRpcClient, 'programAccountsOfExactWidth' | 'multipleAccountDataSlices'>,
  registryProgram: string,
): Promise<ExecutionReleaseIdentityV1> {
  const registry = new PublicKey(registryProgram).toBase58();
  const observation = await client.programAccountsOfExactWidth(registry, ACTIVATION_CACHE_BYTES);
  const candidates: ExecutionReleaseIdentityV1[] = [];
  const undecodable: string[] = [];
  for (const entry of observation.accounts) {
    try {
      const projection = await decodeActivationCacheV1(entry.account.data, registry, entry.address);
      candidates.push(identityFromProjection(projection, registry, entry.address, observation.slot));
    } catch {
      // A partially activated cache legitimately fails to decode: activation
      // admits one role per transaction, so between transactions the cache
      // holds a strict subset. Such a cache is inert for every reader.
      undecodable.push(entry.address);
    }
  }
  if (candidates.length === 0) {
    throw new Error(
      `the Registry program ${registry} owns no decodable ${ACTIVATION_CACHE_BYTES}-byte activation cache${undecodable.length > 0 ? ` (${undecodable.length} account(s) present but not fully activated: ${undecodable.join(', ')})` : ''} — this chain has no activated release for a client to bind to`,
    );
  }

  const programData = REGISTRY_ROLES.map((role) => candidates[0].roles[role].programData);
  const live = await readDeploymentSlotsV1(client, programData);
  const current = candidates.filter((candidate) => REGISTRY_ROLES.every(
    (role, index) => live[index] !== null && live[index] === candidate.roles[role].deploymentSlot,
  ));
  if (current.length === 1) return current[0];

  const inventory = candidates
    .map((candidate) => `${candidate.activationCache} (set ${shortId(candidate.executionReleaseSetId)}, core slot ${candidate.roles.core.deploymentSlot})`)
    .join(', ');
  const liveSlots = REGISTRY_ROLES.map((role, index) => `${role}=${live[index] ?? 'absent'}`).join(' ');
  if (current.length === 0) {
    throw new Error(
      `no activation cache on this chain describes the programs that are actually running. Live deployment slots: ${liveSlots}. Caches the Registry owns: ${inventory}. Either the deployment was upgraded without re-activating a release set, or these roles do not share one deployment — a client cannot bind a frame to a release the chain is not running.`,
    );
  }
  throw new Error(
    `${current.length} activation caches claim the same live deployment slots (${liveSlots}), so the current release is ambiguous: ${inventory}. Refusing to guess which one a frame should be built against.`,
  );
}

/** How a session came to be bound to the release it is bound to. */
export type ReleaseBindingSourceV1 =
  | Readonly<{ kind: 'manifest'; activationCache: string }>
  | Readonly<{ kind: 'discovered'; activationCache: string; supersededManifestCache: string | null; note: string }>;

/** A session bound to the release the chain says is live. */
export type ReleaseBoundSessionV1 = Readonly<{
  identity: ExecutionReleaseIdentityV1;
  release: AbiReleaseTableV1;
  abi: AbiFrameFactsV1;
  /** Whether the manifest's hint was current, or discovery had to follow. */
  source: ReleaseBindingSourceV1;
}>;

/**
 * Open a session against a deployment: read the identity, confirm the cache is
 * current, and select the ABI table — or refuse, naming what the chain runs and
 * what this client carries.
 *
 * Two RPC rounds, constant in the size of the deployment, once per session.
 * Call this before building the first frame.
 */
export async function openReleaseBoundSessionV1(
  client: Pick<SolanaRpcClient, 'accountInfo' | 'multipleAccountDataSlices' | 'programAccountsOfExactWidth'>,
  deployment: Readonly<{ registryProgram: string; activationCache: string | null }>,
  options: Readonly<{
    releases?: ReadonlyArray<AbiReleaseTableV1>;
    /**
     * Refuse instead of following when the manifest's hint has aged out.
     * The default FOLLOWS: a stale constant is not a reason to stop working.
     */
    followCurrent?: boolean;
  }> = {},
): Promise<ReleaseBoundSessionV1> {
  const hint = deployment.activationCache;
  if (hint !== null) {
    const identity = await readExecutionReleaseIdentityV1(client, {
      registryProgram: deployment.registryProgram,
      activationCache: hint,
    }).catch((error: unknown) => (options.followCurrent === false ? Promise.reject(error) : null));
    if (identity !== null) {
      const superseded = await authenticateReleaseCurrencyV1(client, identity).then(() => null, (error: unknown) => error);
      if (superseded === null) {
        const release = selectAbiReleaseV1(identity, options.releases);
        return Object.freeze({
          identity,
          release,
          abi: release.abi,
          source: Object.freeze({ kind: 'manifest', activationCache: hint } as const),
        });
      }
      if (options.followCurrent === false) throw superseded;
    }
  }

  // The manifest's constant is a HINT, not the answer. Follow the chain.
  const identity = await discoverCurrentActivationCacheV1(client, deployment.registryProgram);
  const release = selectAbiReleaseV1(identity, options.releases);
  const note = hint === null
    ? `the deployment manifest names no activation cache; discovery bound this session to ${identity.activationCache} (release set ${shortId(identity.executionReleaseSetId)}), whose pinned deployment slots match the live programs`
    : `the deployment manifest's activation cache ${hint} is not the current one; discovery followed the chain to ${identity.activationCache} (release set ${shortId(identity.executionReleaseSetId)}, core deployment slot ${identity.roles.core.deploymentSlot}), whose pinned deployment slots match the live programs`;
  return Object.freeze({
    identity,
    release,
    abi: release.abi,
    source: Object.freeze({
      kind: 'discovered',
      activationCache: identity.activationCache,
      supersededManifestCache: hint,
      note,
    } as const),
  });
}
