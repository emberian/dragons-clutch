import { PublicKey } from '@solana/web3.js';

/**
 * The load simulator's status artifact, decoded for the /pulse surface.
 *
 * tools/load-simulator/simcore.py rewrites `<work>/status.json` atomically
 * every cycle (schema `dclutch-load-simulator-status-v1`, StatusWriter). The
 * publish pipeline may copy that file to the site root as
 * `/simulator-status.json`; most of the time it will not be there, and the
 * page must say so instead of inventing a pulse. Three states are therefore
 * distinguished on purpose:
 *
 *   absent  — nothing was published, or the host answered with its fallback
 *             page. Not an error: the honest reading is "no simulator ran".
 *   loaded  — a document with the pinned schema decoded completely.
 *   refused — a real JSON artifact arrived and did NOT decode. That is a
 *             defect worth showing, and it is never folded into "absent".
 *
 * The writer appends free extra keys (StatusWriter.write's `extra`), so this
 * decoder pins the fields it renders and tolerates keys it does not know —
 * unlike the public cut, whose input is exact by construction. Reconciliation
 * bodies vary by run mode (`skipped`, `resumed`, `output`), so only `ok` and
 * `checked_at` are demanded there.
 */

export const SIMULATOR_STATUS_SCHEMA_V1 = 'dclutch-load-simulator-status-v1';

/** The one URL the surface reads. Pinned by test: the site's link checker
 * cannot see a runtime fetch, so the string itself is the contract. */
export const SIMULATOR_STATUS_URL_V1 = '/simulator-status.json';

/** One plain sentence for the shipped default state. */
export const NO_SIMULATOR_SENTENCE_V1 =
  'No simulator is publishing here right now. Nothing has been read, and nothing below is a zero.';

/** A pulse older than this renders as stale rather than running: the
 * simulator writes every cycle, and fifteen minutes of silence is a stopped
 * writer whatever the file says. */
export const STALE_AFTER_MS_V1 = 15 * 60_000;

export type SimulatorWalletV1 = Readonly<{
  address: string;
  role: string;
  /** Exact lamports, or null when the balance read did not answer. */
  solLamports: number | null;
  source: string;
}>;

export type SimulatorReconciliationV1 = Readonly<{
  ok: boolean;
  checkedAt: string;
  /** One short phrase carrying the run-mode detail the writer attached. */
  detail: string | null;
}>;

export type SimulatorStatusV1 = Readonly<{
  schema: typeof SIMULATOR_STATUS_SCHEMA_V1;
  clusterLabel: 'local' | 'devnet';
  rpcUrl: string;
  market: string | null;
  mode: 'finite' | 'sustain';
  startedAt: string;
  updatedAt: string;
  cyclesRun: number;
  cyclesTarget: number | null;
  tradesLanded: number;
  signatures: ReadonlyArray<string>;
  wallets: ReadonlyArray<SimulatorWalletV1>;
  lastReconciliation: SimulatorReconciliationV1 | null;
  halted: boolean;
  haltReason: string | null;
  stopping: boolean;
}>;

function object(value: unknown, field: string): Record<string, unknown> {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) throw new Error(`${field} must be one object`);
  return value as Record<string, unknown>;
}

function text(value: unknown, field: string): string {
  if (typeof value !== 'string' || value.length === 0) throw new Error(`${field} must be one non-empty string`);
  return value;
}

function textOrNull(value: unknown, field: string): string | null {
  return value === null || value === undefined ? null : text(value, field);
}

function flag(value: unknown, field: string): boolean {
  if (typeof value !== 'boolean') throw new Error(`${field} must be true or false`);
  return value;
}

function count(value: unknown, field: string): number {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0) throw new Error(`${field} must be one exact non-negative integer`);
  return value;
}

function countOrNull(value: unknown, field: string): number | null {
  return value === null || value === undefined ? null : count(value, field);
}

function instant(value: unknown, field: string): string {
  const raw = text(value, field);
  if (Number.isNaN(Date.parse(raw))) throw new Error(`${field} must be one parseable timestamp`);
  return raw;
}

function address(value: unknown, field: string): string {
  const raw = text(value, field);
  let parsed: PublicKey;
  try { parsed = new PublicKey(raw); } catch { throw new Error(`${field} must be one canonical Solana address`); }
  if (parsed.toBase58() !== raw) throw new Error(`${field} must be one canonical Solana address`);
  return raw;
}

function reconciliation(value: unknown): SimulatorReconciliationV1 | null {
  if (value === null || value === undefined) return null;
  const body = object(value, 'last_reconciliation');
  // `output` is the writer's own absolute path inside its work directory. That
  // is the right thing for the simulator to record and the wrong thing for a
  // public page to print: it is an operator's local filesystem layout, and it
  // tells a reader nothing. The file's name does tell them something — which
  // cycle's census this verdict came out of — so that is what survives here.
  const detail = typeof body.skipped === 'string'
    ? `skipped: ${body.skipped}`
    : body.resumed === true
      ? 'resumed from an earlier run'
      : typeof body.output === 'string'
        ? `from census file ${body.output.split('/').pop()}`
        : null;
  return Object.freeze({
    ok: flag(body.ok, 'last_reconciliation ok'),
    checkedAt: instant(body.checked_at, 'last_reconciliation checked_at'),
    detail,
  });
}

/** Decode one status document. Throws with the field named; never returns a
 * half-status. */
export function parseSimulatorStatusV1(value: unknown): SimulatorStatusV1 {
  const root = object(value, 'simulator status');
  if (root.schema !== SIMULATOR_STATUS_SCHEMA_V1) throw new Error('simulator status has another schema');
  const cluster = object(root.cluster, 'cluster');
  const clusterLabel = cluster.label;
  if (clusterLabel !== 'local' && clusterLabel !== 'devnet') throw new Error('cluster label must be local or devnet');
  const mode = root.mode;
  if (mode !== 'finite' && mode !== 'sustain') throw new Error('mode must be finite or sustain');
  const market = object(root.market, 'market');
  const cycles = object(root.cycles, 'cycles');
  const trades = object(root.trades, 'trades');
  if (!Array.isArray(trades.signatures)) throw new Error('trades signatures must be one list');
  const signatures = Object.freeze(trades.signatures.map((entry, index) => text(entry, `trades signature ${index}`)));
  if (!Array.isArray(root.wallets)) throw new Error('wallets must be one list');
  const wallets = Object.freeze(root.wallets.map((entry, index) => {
    const body = object(entry, `wallet ${index}`);
    return Object.freeze({
      address: address(body.address, `wallet ${index} address`),
      role: text(body.role, `wallet ${index} role`),
      solLamports: countOrNull(body.sol_lamports, `wallet ${index} sol_lamports`),
      source: text(body.source, `wallet ${index} source`),
    });
  }));
  return Object.freeze({
    schema: SIMULATOR_STATUS_SCHEMA_V1,
    clusterLabel,
    rpcUrl: text(cluster.rpc_url, 'cluster rpc_url'),
    market: market.address === null || market.address === undefined ? null : address(market.address, 'market address'),
    mode,
    startedAt: instant(root.started_at, 'started_at'),
    updatedAt: instant(root.updated_at, 'updated_at'),
    cyclesRun: count(cycles.run, 'cycles run'),
    cyclesTarget: countOrNull(cycles.target, 'cycles target'),
    tradesLanded: count(trades.landed, 'trades landed'),
    signatures,
    wallets,
    lastReconciliation: reconciliation(root.last_reconciliation),
    halted: flag(root.halted, 'halted'),
    haltReason: textOrNull(root.halt_reason, 'halt_reason'),
    stopping: flag(root.stopping, 'stopping'),
  });
}

export type SimulatorBeatV1 = Readonly<{
  state: 'running' | 'stopping' | 'halted' | 'stale';
  sentence: string;
}>;

/** What the heartbeat dot says about a loaded status, judged against now. */
export function simulatorBeatV1(status: SimulatorStatusV1, nowMs: number): SimulatorBeatV1 {
  if (status.halted) {
    return Object.freeze({
      state: 'halted' as const,
      sentence: status.haltReason === null
        ? 'The simulator halted itself and recorded no reason; the work directory holds the details.'
        : `The simulator halted itself: ${status.haltReason}`,
    });
  }
  const age = nowMs - Date.parse(status.updatedAt);
  if (age > STALE_AFTER_MS_V1) {
    return Object.freeze({
      state: 'stale' as const,
      sentence: `The last write is older than ${Math.floor(STALE_AFTER_MS_V1 / 60_000)} minutes, so this pulse is a record, not a heartbeat.`,
    });
  }
  if (status.stopping) {
    return Object.freeze({ state: 'stopping' as const, sentence: 'The simulator is finishing its current cycle and sealing its journals.' });
  }
  return Object.freeze({ state: 'running' as const, sentence: 'The simulator wrote this within the last few minutes.' });
}

export type SimulatorReadV1 =
  | Readonly<{ kind: 'absent' }>
  | Readonly<{ kind: 'loaded'; status: SimulatorStatusV1 }>
  | Readonly<{ kind: 'refused'; reason: string }>;

/**
 * Read the published artifact, guarded for a static host. A missing path
 * answers with the host's fallback page — an HTML body, sometimes under a
 * 200 — so a non-OK answer and an unparseable body both read as absent, and
 * only a real JSON document that fails the decoder reads as refused.
 */
export async function readSimulatorStatusV1(
  fetchLike: (url: string) => Promise<{ ok: boolean; text(): Promise<string> }>,
): Promise<SimulatorReadV1> {
  let body: string;
  try {
    const response = await fetchLike(SIMULATOR_STATUS_URL_V1);
    if (!response.ok) return Object.freeze({ kind: 'absent' as const });
    body = await response.text();
  } catch {
    return Object.freeze({ kind: 'absent' as const });
  }
  let raw: unknown;
  try { raw = JSON.parse(body); } catch { return Object.freeze({ kind: 'absent' as const }); }
  try {
    return Object.freeze({ kind: 'loaded' as const, status: parseSimulatorStatusV1(raw) });
  } catch (error) {
    const reason = error instanceof Error ? error.message : 'the artifact did not decode for a usable reason';
    return Object.freeze({ kind: 'refused' as const, reason });
  }
}
