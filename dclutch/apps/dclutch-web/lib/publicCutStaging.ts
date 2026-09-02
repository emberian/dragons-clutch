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

/**
 * One sealing driver's own output, ingested verbatim.
 *
 * The cut's `checkedReleases` rows are NOT typed by a person. A hand-copied
 * 64-hex triple is the mirror this whole surface exists to delete: nothing
 * downstream can tell a mistyped digest from a real one, and the row would be
 * exactly as authoritative either way. So the sealing driver emits this
 * fragment and the staging tool ingests it, unchanged.
 */
export type CheckedReleaseFragmentV1 = Readonly<{
  schema: 'dclutch-public-cut-checked-releases-fragment-v1';
  /** Keyed exactly as the cut's own rows are, so ingestion is a copy. */
  checkedReleases: Readonly<Record<string, PublicCutCheckedReleaseV1>>;
}>;

const FRAGMENT_SCHEMA = 'dclutch-public-cut-checked-releases-fragment-v1';

/** Parse a sealing driver's fragment, refusing any shape it does not know. */
export function parseCheckedReleaseFragmentV1(value: unknown): CheckedReleaseFragmentV1 {
  const root = object(value, 'checked release fragment');
  exactKeys(root, ['schema', 'checkedReleases'], 'checked release fragment');
  if (root.schema !== FRAGMENT_SCHEMA) throw new Error('checked release fragment has another schema');
  const rows = object(root.checkedReleases, 'checked release fragment checkedReleases');
  return Object.freeze({
    schema: FRAGMENT_SCHEMA,
    checkedReleases: Object.freeze(Object.fromEntries(Object.entries(rows).map(([releaseSetId, entry]) => {
      const body = object(entry, `checked release fragment ${releaseSetId}`);
      exactKeys(body, ['gateDigest', 'sealedSet'], `checked release fragment ${releaseSetId}`);
      return [
        digest(releaseSetId, 'checked release fragment key'),
        Object.freeze({
          gateDigest: digest(body.gateDigest, `checked release fragment ${releaseSetId} gateDigest`),
          sealedSet: digest(body.sealedSet, `checked release fragment ${releaseSetId} sealedSet`),
        }),
      ];
    }))),
  });
}

/**
 * Stage one fragment into a cut, or refuse it by name.
 *
 * THE REFUSAL IS THE POINT. A fragment is about one execution release set, and
 * the only set that means anything to this cut is the one its own Market
 * selects -- read off the chain, which is why it arrives as an argument rather
 * than being taken on the fragment's word. A fragment for any other set is a
 * fragment for another deployment, and staging it would put a row in this
 * site's deployment record that turns the trade spine's `release` wall off for
 * a market the release was never checked against. That is the exact shape of
 * the failure the wall exists to prevent, arriving through the door meant to
 * fix it, so it refuses and names both sets.
 *
 * Returns a NEW cut. Nothing here writes a file: the caller re-serializes and
 * replaces the fixture atomically, as every other generator in this tree does.
 */
export function stageCheckedReleaseV1(
  cut: PublicDevnetCutV1,
  fragment: CheckedReleaseFragmentV1,
  marketReleaseSetId: string,
): PublicDevnetCutV1 {
  if (cut.market === null) {
    throw new Error('a pending public cut names no Market, so no checked release can be about it');
  }
  const selected = digest(marketReleaseSetId, 'Market execution release set');
  const named = Object.keys(fragment.checkedReleases);
  const row = fragment.checkedReleases[selected] ?? null;
  if (row === null) {
    // An unsealed plan emits an EMPTY map rather than omitting the key, which
    // is the producer stating "nothing is sealed" instead of staying silent --
    // and it must not read as "stage nothing, quietly". Both arms name what
    // was found so the operator sees which deployment the fragment is about.
    throw new Error(named.length === 0
      ? `this checked release fragment seals nothing, and the cut's Market ${cut.market} selects execution release set ${selected}`
      : `this checked release fragment is for execution release set${named.length === 1 ? '' : 's'} ${named.join(', ')}, and the cut's Market ${cut.market} selects ${selected}`);
  }
  const existing = cut.checkedReleases[selected] ?? null;
  if (existing !== null && (existing.gateDigest !== row.gateDigest || existing.sealedSet !== row.sealedSet)) {
    throw new Error(`this cut already names a different checked release for execution release set ${selected}`);
  }
  return Object.freeze({
    ...cut,
    checkedReleases: Object.freeze({ ...cut.checkedReleases, [selected]: row }),
  });
}
