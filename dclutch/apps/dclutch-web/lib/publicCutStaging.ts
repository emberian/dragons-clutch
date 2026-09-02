import { PublicKey } from '@solana/web3.js';

import manifest from '@/fixtures/public-cut.devnet.json';
import { marketDetailHrefV1 } from './marketHref';

const SCHEMA = 'dclutch-public-cut-v1';
const ACTIVITY_STEPS = ['found', 'trade', 'resolve', 'redeem'] as const;

export type PublicCutActivityStepV1 = (typeof ACTIVITY_STEPS)[number];

export type PublicDevnetCutV1 = Readonly<{
  schema: typeof SCHEMA;
  cluster: 'devnet';
  market: string | null;
  activity: Readonly<Record<PublicCutActivityStepV1, string | null>>;
}>;

function object(value: unknown, field: string): Record<string, unknown> {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) throw new Error(`${field} must be one object`);
  return value as Record<string, unknown>;
}

function exactKeys(value: Record<string, unknown>, keys: ReadonlyArray<string>, field: string): void {
  const actual = Object.keys(value).sort(); const expected = [...keys].sort();
  if (actual.length !== expected.length || actual.some((key, index) => key !== expected[index])) throw new Error(`${field} has missing or unknown fields`);
}

function address(value: unknown, field: string): string {
  if (typeof value !== 'string') throw new Error(`${field} must be one canonical Solana address`);
  let parsed: PublicKey;
  try { parsed = new PublicKey(value); } catch { throw new Error(`${field} must be one canonical Solana address`); }
  if (parsed.toBase58() !== value) throw new Error(`${field} must be one canonical Solana address`);
  return value;
}

function signature(value: unknown, field: string): string | null {
  if (value === null) return null;
  if (typeof value !== 'string' || !/^[1-9A-HJ-NP-Za-km-z]{64,88}$/.test(value)) throw new Error(`${field} must be one canonical transaction signature or null`);
  return value;
}

/** Parse the sole static input that opens the public devnet cut. */
export function parsePublicDevnetCutV1(value: unknown): PublicDevnetCutV1 {
  const root = object(value, 'public cut'); exactKeys(root, ['schema', 'cluster', 'market', 'activity'], 'public cut');
  if (root.schema !== SCHEMA || root.cluster !== 'devnet') throw new Error('public cut has another schema or cluster');
  const market = root.market === null ? null : address(root.market, 'public cut Market');
  const activityRaw = object(root.activity, 'public cut activity'); exactKeys(activityRaw, ACTIVITY_STEPS, 'public cut activity');
  const activity = Object.freeze(Object.fromEntries(ACTIVITY_STEPS.map((step) => [step, signature(activityRaw[step], `public cut ${step} signature`)]))) as Readonly<Record<PublicCutActivityStepV1, string | null>>;
  if (market === null && ACTIVITY_STEPS.some((step) => activity[step] !== null)) throw new Error('a pending public cut cannot name lifecycle activity');
  // A live cut used to be REQUIRED to name its founding transaction, and that
  // rule is gone because it was not satisfiable and was therefore about to be
  // satisfied by a guess. Cohort-12's Found rides an address lookup table, so
  // the Market is not in that transaction's static keys and
  // `getSignaturesForAddress` on the Market does not return it (checked
  // 2026-09-02 against the Market and its Claims aggregate: thirteen and five
  // signatures respectively, and no Core `Found` action tag among them).
  // Naming a market whose founding signature this cut cannot verify is honest;
  // naming a plausible neighbouring signature would not be, and a rule whose
  // only effect is to force the second is worse than no rule.
  return Object.freeze({ schema: SCHEMA, cluster: 'devnet', market, activity });
}

/** Update only fixtures/public-cut.devnet.json after a checked public opening. */
export const PUBLIC_DEVNET_CUT_V1 = parsePublicDevnetCutV1(manifest);

export function publicCutMarketHrefV1(cut = PUBLIC_DEVNET_CUT_V1): string {
  return cut.market === null ? '/markets' : marketDetailHrefV1(cut.market);
}

export function publicCutExplorerHrefV1(cut = PUBLIC_DEVNET_CUT_V1): string {
  return cut.market === null ? '/explorer' : `/explorer?view=market&q=${encodeURIComponent(cut.market)}`;
}

export function publicCutTransactionHrefV1(step: PublicCutActivityStepV1, cut = PUBLIC_DEVNET_CUT_V1): string | null {
  const value = cut.activity[step];
  return value === null ? null : `/explorer?view=transaction&q=${encodeURIComponent(value)}`;
}
