import { PublicKey } from '@solana/web3.js';

import { publicFirstAdmissionBindingV1, publicMarketBindingsCohortV1 } from '@/lib/publicMarketBindings';

/** The bounded simulator's public observation, published beside the static app. */
export const AQUARIUM_STATUS_SCHEMA_V1 = 'dclutch-aquarium-status-v1';
export const AQUARIUM_STATUS_URL_V1 = '/aquarium-status-v1.json';

export type AquariumStateV1 = 'preflight' | 'running' | 'stopping' | 'stopped' | 'halted' | 'stale';

export type AquariumStatusV1 = Readonly<{
  schema: typeof AQUARIUM_STATUS_SCHEMA_V1;
  cohort: Readonly<{
    number: number; manifestSha256: string; deploymentCommit: string; checkedAt: string;
    releaseGateSha256?: string;
    generalAccelerator?: Readonly<{ programId: string; deploymentSlot: number; elfSha256: string; semanticReleaseId: string }>;
  }>;
  state: AquariumStateV1;
  run: Readonly<{
    startedAt: string;
    updatedAt: string;
    expectedNextUpdateBy: string | null;
    plannedMarketCount: number;
    activeMarketTarget: number;
    joinedWalletTarget: number;
    maxWallets: number;
    maxLamportsSpent: number;
    lamportsSpentObserved: number;
  }>;
  activity: Readonly<{
    lastEventAt: string | null;
    lastEventKind: string | null;
    counts: Readonly<Record<'found' | 'admitted' | 'fill' | 'resolved' | 'deadline_failure' | 'redeemed' | 'retired' | 'census' | 'refused' | 'unattempted' | 'blocked', number>>;
    activeMarkets: ReadonlyArray<Readonly<{
      marketId: string;
      address: string | null;
      state: string;
      joinOpen: boolean;
      observedAt: string | null;
      epochsCompleted: number;
      epochsPrecommitted: number;
    }>>;
    joinNote: string;
  }>;
  limits: Readonly<{ maxActiveMarkets: number }>;
  artifacts: Readonly<{ driver: string; epochJournalSchema: string; childJournalOwner: string }>;
  failure: Readonly<{ kind: 'driver-exit'; at: string; detail: string }> | null;
}>;

function object(value: unknown, field: string): Record<string, unknown> {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) throw new Error(`${field} must be one object`);
  return value as Record<string, unknown>;
}

function exactKeys(value: Record<string, unknown>, keys: ReadonlyArray<string>, field: string): void {
  const actual = Object.keys(value).sort();
  const expected = [...keys].sort();
  if (actual.length !== expected.length || actual.some((key, index) => key !== expected[index])) throw new Error(`${field} has missing or unknown fields`);
}

/** Permit an additive public identity fact, but never silently accept an unknown field. */
function requiredAndKnownKeys(
  value: Record<string, unknown>, required: ReadonlyArray<string>, optional: ReadonlyArray<string>, field: string,
): void {
  const allowed = new Set([...required, ...optional]);
  for (const key of Object.keys(value)) if (!allowed.has(key)) throw new Error(`${field} has missing or unknown fields`);
  for (const key of required) if (!(key in value)) throw new Error(`${field} has missing or unknown fields`);
}

function text(value: unknown, field: string): string {
  if (typeof value !== 'string' || value.length === 0) throw new Error(`${field} must be one non-empty string`);
  return value;
}

function count(value: unknown, field: string): number {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0) throw new Error(`${field} must be one exact non-negative integer`);
  return value;
}

/** Producer config may serialize a slot as JSON number or canonical decimal text. */
function slot(value: unknown, field: string): number {
  if (typeof value === 'number') return count(value, field);
  if (typeof value !== 'string' || !/^(?:0|[1-9][0-9]*)$/.test(value)) throw new Error(`${field} must be one exact non-negative integer`);
  return count(Number(value), field);
}

function instant(value: unknown, field: string): string {
  const raw = text(value, field);
  if (!/^\d{4}-\d{2}-\d{2}T.*Z$/.test(raw) || Number.isNaN(Date.parse(raw))) throw new Error(`${field} must be one parseable UTC timestamp`);
  return raw;
}

function instantOrNull(value: unknown, field: string): string | null {
  return value === null ? null : instant(value, field);
}

function addressOrNull(value: unknown, field: string): string | null {
  if (value === null) return null;
  const raw = text(value, field);
  let parsed: PublicKey;
  try { parsed = new PublicKey(raw); } catch { throw new Error(`${field} must be one canonical Solana address or null`); }
  if (parsed.toBase58() !== raw) throw new Error(`${field} must be one canonical Solana address or null`);
  return raw;
}

const ACTIVITY_COUNT_KEYS = ['found', 'admitted', 'fill', 'resolved', 'deadline_failure', 'redeemed', 'retired', 'census', 'refused', 'unattempted', 'blocked'] as const;
const STATES = ['preflight', 'running', 'stopping', 'stopped', 'halted', 'stale'] as const;

export type AquariumJoinBindingsV1 = Readonly<{
  cohort: () => Readonly<{ number: number; manifestSha256: string | null }>;
  bindingFor: (market: string) => unknown;
}>;

const PUBLIC_JOIN_BINDINGS_V1: AquariumJoinBindingsV1 = Object.freeze({
  cohort: publicMarketBindingsCohortV1,
  bindingFor: publicFirstAdmissionBindingV1,
});

/** Decode the whole public record or render none of it. */
export function parseAquariumStatusV1(
  value: unknown,
  joinBindings: AquariumJoinBindingsV1 = PUBLIC_JOIN_BINDINGS_V1,
): AquariumStatusV1 {
  const root = object(value, 'aquarium status');
  exactKeys(root, ['schema', 'cohort', 'state', 'run', 'activity', 'limits', 'artifacts', 'failure'], 'aquarium status');
  if (root.schema !== AQUARIUM_STATUS_SCHEMA_V1) throw new Error('aquarium status has another schema');
  if (!STATES.includes(root.state as AquariumStateV1)) throw new Error('aquarium state is unknown');

  const cohort = object(root.cohort, 'cohort');
  requiredAndKnownKeys(
    cohort,
    ['number', 'manifest_sha256', 'deployment_commit', 'checked_at'],
    ['release_gate_sha256', 'general_accelerator'],
    'cohort',
  );
  const manifestSha256 = text(cohort.manifest_sha256, 'cohort manifest_sha256');
  if (!/^[0-9a-f]{64}$/.test(manifestSha256)) throw new Error('cohort manifest_sha256 must be 64 lowercase hex characters');
  const deploymentCommit = text(cohort.deployment_commit, 'cohort deployment_commit');
  if (!/^[0-9a-f]{7,64}$/.test(deploymentCommit)) throw new Error('cohort deployment_commit must be one lowercase git digest');
  const releaseGateSha256 = cohort.release_gate_sha256 === undefined ? undefined : text(cohort.release_gate_sha256, 'cohort release_gate_sha256');
  if (releaseGateSha256 !== undefined && !/^[0-9a-f]{64}$/.test(releaseGateSha256)) throw new Error('cohort release_gate_sha256 must be 64 lowercase hex characters');
  let generalAccelerator: AquariumStatusV1['cohort']['generalAccelerator'];
  if (cohort.general_accelerator !== undefined) {
    const accelerator = object(cohort.general_accelerator, 'cohort general_accelerator');
    exactKeys(accelerator, ['program_id', 'deployment_slot', 'elf_sha256', 'semantic_release_id'], 'cohort general_accelerator');
    const programId = addressOrNull(accelerator.program_id, 'cohort general_accelerator program_id');
    if (programId === null) throw new Error('cohort general_accelerator program_id must be one canonical Solana address');
    const elfSha256 = text(accelerator.elf_sha256, 'cohort general_accelerator elf_sha256');
    const semanticReleaseId = text(accelerator.semantic_release_id, 'cohort general_accelerator semantic_release_id');
    if (!/^[0-9a-f]{64}$/.test(elfSha256) || !/^[0-9a-f]{64}$/.test(semanticReleaseId)) throw new Error('cohort general_accelerator ELF and semantic release digests must be 64 lowercase hex characters');
    generalAccelerator = Object.freeze({
      programId,
      deploymentSlot: slot(accelerator.deployment_slot, 'cohort general_accelerator deployment_slot'),
      elfSha256,
      semanticReleaseId,
    });
  }

  const run = object(root.run, 'run');
  exactKeys(run, ['started_at', 'updated_at', 'expected_next_update_by', 'planned_market_count', 'active_market_target', 'joined_wallet_target', 'max_wallets', 'max_lamports_spent', 'lamports_spent_observed'], 'run');
  const limits = object(root.limits, 'limits');
  exactKeys(limits, ['max_active_markets'], 'limits');
  const maxActiveMarkets = count(limits.max_active_markets, 'limits max_active_markets');
  if (maxActiveMarkets > 32) throw new Error('limits max_active_markets exceeds the public bound of 32');

  const activity = object(root.activity, 'activity');
  exactKeys(activity, ['synthetic_actors', 'last_event_at', 'last_event_kind', 'counts', 'active_markets', 'join_note'], 'activity');
  if (activity.synthetic_actors !== true) throw new Error('activity must declare synthetic actors');
  const counts = object(activity.counts, 'activity counts');
  exactKeys(counts, ACTIVITY_COUNT_KEYS, 'activity counts');
  if (!Array.isArray(activity.active_markets)) throw new Error('activity active_markets must be one list');
  if (activity.active_markets.length > maxActiveMarkets) throw new Error('activity active_markets exceeds its published limit');
  const marketIds = new Set<string>();
  const addresses = new Set<string>();
  const activeMarkets = Object.freeze(activity.active_markets.map((entry, index) => {
    const market = object(entry, `active market ${index}`);
    exactKeys(market, ['market_id', 'address', 'state', 'join_open', 'observed_at', 'epochs_completed', 'epochs_precommitted'], `active market ${index}`);
    const marketId = text(market.market_id, `active market ${index} market_id`);
    const address = addressOrNull(market.address, `active market ${index} address`);
    if (marketIds.has(marketId)) throw new Error(`active market ${index} repeats market_id ${marketId}`);
    marketIds.add(marketId);
    if (address !== null) {
      if (addresses.has(address)) throw new Error(`active market ${index} repeats address ${address}`);
      addresses.add(address);
    }
    if (typeof market.join_open !== 'boolean') throw new Error(`active market ${index} join_open must be boolean`);
    if (market.join_open) {
      if (address === null) throw new Error(`active market ${index} cannot open joining without one market address`);
      const bindingCohort = joinBindings.cohort();
      if (bindingCohort.manifestSha256 === null
          || bindingCohort.number !== count(cohort.number, 'cohort number')
          || bindingCohort.manifestSha256 !== manifestSha256) {
        throw new Error(`active market ${index} public join binding belongs to another cohort`);
      }
      if (joinBindings.bindingFor(address) === undefined) {
        throw new Error(`active market ${index} has no checked public first-admission binding`);
      }
    }
    return Object.freeze({
      marketId,
      address,
      state: text(market.state, `active market ${index} state`),
      joinOpen: market.join_open,
      observedAt: instantOrNull(market.observed_at, `active market ${index} observed_at`),
      epochsCompleted: count(market.epochs_completed, `active market ${index} epochs_completed`),
      epochsPrecommitted: count(market.epochs_precommitted, `active market ${index} epochs_precommitted`),
    });
  }));

  const artifacts = object(root.artifacts, 'artifacts');
  exactKeys(artifacts, ['driver', 'epoch_journal_schema', 'child_journal_owner'], 'artifacts');
  let failure: AquariumStatusV1['failure'] = null;
  if (root.failure !== null) {
    const rawFailure = object(root.failure, 'failure');
    exactKeys(rawFailure, ['kind', 'at', 'detail'], 'failure');
    if (rawFailure.kind !== 'driver-exit') throw new Error('failure kind is unknown');
    failure = Object.freeze({ kind: 'driver-exit' as const, at: instant(rawFailure.at, 'failure at'), detail: text(rawFailure.detail, 'failure detail') });
  }

  return Object.freeze({
    schema: AQUARIUM_STATUS_SCHEMA_V1,
    cohort: Object.freeze({
      number: count(cohort.number, 'cohort number'), manifestSha256, deploymentCommit, checkedAt: instant(cohort.checked_at, 'cohort checked_at'),
      ...(releaseGateSha256 === undefined ? {} : { releaseGateSha256 }),
      ...(generalAccelerator === undefined ? {} : { generalAccelerator }),
    }),
    state: root.state as AquariumStateV1,
    run: Object.freeze({
      startedAt: instant(run.started_at, 'run started_at'), updatedAt: instant(run.updated_at, 'run updated_at'), expectedNextUpdateBy: instantOrNull(run.expected_next_update_by, 'run expected_next_update_by'),
      plannedMarketCount: count(run.planned_market_count, 'run planned_market_count'), activeMarketTarget: count(run.active_market_target, 'run active_market_target'), joinedWalletTarget: count(run.joined_wallet_target, 'run joined_wallet_target'), maxWallets: count(run.max_wallets, 'run max_wallets'), maxLamportsSpent: count(run.max_lamports_spent, 'run max_lamports_spent'), lamportsSpentObserved: count(run.lamports_spent_observed, 'run lamports_spent_observed'),
    }),
    activity: Object.freeze({
      lastEventAt: instantOrNull(activity.last_event_at, 'activity last_event_at'), lastEventKind: activity.last_event_kind === null ? null : text(activity.last_event_kind, 'activity last_event_kind'),
      counts: Object.freeze(Object.fromEntries(ACTIVITY_COUNT_KEYS.map((key) => [key, count(counts[key], `activity counts ${key}`)]))) as AquariumStatusV1['activity']['counts'],
      activeMarkets, joinNote: text(activity.join_note, 'activity join_note'),
    }),
    limits: Object.freeze({ maxActiveMarkets }),
    artifacts: Object.freeze({ driver: text(artifacts.driver, 'artifacts driver'), epochJournalSchema: text(artifacts.epoch_journal_schema, 'artifacts epoch_journal_schema'), childJournalOwner: text(artifacts.child_journal_owner, 'artifacts child_journal_owner') }),
    failure,
  });
}

export type AquariumReadV1 = Readonly<{ kind: 'absent' }> | Readonly<{ kind: 'loaded'; status: AquariumStatusV1 }> | Readonly<{ kind: 'refused'; reason: string }>;

/** Missing static assets and host fallback HTML are absence; malformed JSON is a refusal. */
export async function readAquariumStatusV1(
  fetchLike: (url: string) => Promise<{ ok: boolean; text(): Promise<string> }>,
): Promise<AquariumReadV1> {
  let body: string;
  try {
    const response = await fetchLike(AQUARIUM_STATUS_URL_V1);
    if (!response.ok) return Object.freeze({ kind: 'absent' as const });
    body = await response.text();
  } catch { return Object.freeze({ kind: 'absent' as const }); }
  let raw: unknown;
  try { raw = JSON.parse(body); } catch { return Object.freeze({ kind: 'absent' as const }); }
  try { return Object.freeze({ kind: 'loaded' as const, status: parseAquariumStatusV1(raw) }); }
  catch (error) { return Object.freeze({ kind: 'refused' as const, reason: error instanceof Error ? error.message : 'the artifact did not decode for a usable reason' }); }
}

export type AquariumBeatV1 = Readonly<{ state: AquariumStateV1; sentence: string }>;

/** The producer's own deadline overrides a stale-looking state left by a dead writer. */
export function aquariumBeatV1(status: AquariumStatusV1, nowMs: number): AquariumBeatV1 {
  if (status.failure !== null) return Object.freeze({ state: 'halted', sentence: `Aquarium driver exited: ${status.failure.detail}` });
  if (status.run.expectedNextUpdateBy !== null && nowMs > Date.parse(status.run.expectedNextUpdateBy)) {
    return Object.freeze({ state: 'stale', sentence: 'Aquarium update is overdue.' });
  }
  const sentence: Record<AquariumStateV1, string> = {
    preflight: 'Aquarium is preparing.', running: 'Aquarium is running.', stopping: 'Aquarium is finishing the current epoch.', stopped: 'Aquarium stopped.', halted: 'Aquarium halted.', stale: 'Aquarium data is stale.',
  };
  return Object.freeze({ state: status.state, sentence: sentence[status.state] });
}
