import { PublicKey } from '@solana/web3.js';

import manifest from '@/fixtures/public-cut.devnet.json';
import { marketDetailHrefV1 } from './marketHref';

const SCHEMA = 'dclutch-public-cut-v1';
const ACTIVITY_STEPS = ['found', 'trade', 'resolve', 'redeem'] as const;

export type PublicCutActivityStepV1 = (typeof ACTIVITY_STEPS)[number];

/**
 * One execution release set that has a checked release on file.
 *
 * A Direct fill is admitted at the route boundary only against a CHECKED
 * execution release, and that artifact is produced offline by
 * `devnet-checked-execution-release-v1` -- there is no account on chain a
 * browser can read to learn whether one exists. So the browser's own
 * authenticated deployment record carries it: the seal lane's output writes a
 * row here, keyed by the execution release set the Market selects, and the
 * absence of a row is exactly the fact a trader needs before they start.
 *
 * Both digests are the seal's, not this file's arithmetic: `gateDigest` is the
 * gate the release was checked under and `sealedSet` the set it sealed, so a
 * reader who has the artifact can compare it against what this site claims.
 */
export type PublicCutCheckedReleaseV1 = Readonly<{ gateDigest: string; sealedSet: string }>;

export type PublicDevnetCutV1 = Readonly<{
  schema: typeof SCHEMA;
  cluster: 'devnet';
  market: string | null;
  activity: Readonly<Record<PublicCutActivityStepV1, string | null>>;
  /** Keyed by execution release set identity, 64 lowercase hex. May be empty. */
  checkedReleases: Readonly<Record<string, PublicCutCheckedReleaseV1>>;
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

function digest(value: unknown, field: string): string {
  if (typeof value !== 'string' || !/^[0-9a-f]{64}$/.test(value)) throw new Error(`${field} must be 64 lowercase hex characters`);
  return value;
}

function signature(value: unknown, field: string): string | null {
  if (value === null) return null;
  if (typeof value !== 'string' || !/^[1-9A-HJ-NP-Za-km-z]{64,88}$/.test(value)) throw new Error(`${field} must be one canonical transaction signature or null`);
  return value;
}

/** Parse the sole static input that opens the public devnet cut. */
export function parsePublicDevnetCutV1(value: unknown): PublicDevnetCutV1 {
  const root = object(value, 'public cut'); exactKeys(root, ['schema', 'cluster', 'market', 'activity', 'checkedReleases'], 'public cut');
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
  const releasesRaw = object(root.checkedReleases, 'public cut checkedReleases');
  const checkedReleases = Object.freeze(Object.fromEntries(Object.entries(releasesRaw).map(([releaseSetId, entry]) => {
    const body = object(entry, `public cut checked release ${releaseSetId}`);
    exactKeys(body, ['gateDigest', 'sealedSet'], `public cut checked release ${releaseSetId}`);
    return [
      digest(releaseSetId, 'public cut checked release key'),
      Object.freeze({
        gateDigest: digest(body.gateDigest, `public cut checked release ${releaseSetId} gateDigest`),
        sealedSet: digest(body.sealedSet, `public cut checked release ${releaseSetId} sealedSet`),
      }),
    ];
  })));
  return Object.freeze({ schema: SCHEMA, cluster: 'devnet', market, activity, checkedReleases });
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

/**
 * The execution release sets this cut says have a checked release, or null.
 *
 * NULL IS NOT AN EMPTY LIST and the difference is the whole point: an empty
 * list is "this deployment record was consulted and names none", which is a
 * reason to tell a trader the fill will refuse; null would be "nobody asked",
 * which is not. A cut with no Market has not been staged at all, so it answers
 * null rather than claiming knowledge of a deployment it does not describe.
 */
export function checkedReleaseSetIdsV1(cut = PUBLIC_DEVNET_CUT_V1): ReadonlyArray<string> | null {
  return cut.market === null ? null : Object.freeze(Object.keys(cut.checkedReleases));
}
