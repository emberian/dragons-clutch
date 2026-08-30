import manifest from '@/fixtures/building-pulse.json';

/**
 * The `/building` page's sole input.
 *
 * The page is a synthesis — what is actively in development, written for a
 * reader who has never seen this project — and syntheses date. So the content
 * lives in one fixture, `fixtures/building-pulse.json`, and updating the page
 * IS updating that file: headline, numbers, the in-flight list, the wall
 * ledger. No component edit, no copy hunt across JSX.
 *
 * The parser refuses two ways beyond shape errors, both deliberate:
 *
 * - **Internal vocabulary is refused structurally.** The fixture is written
 *   for strangers; project shorthand (working-group names, internal ledger
 *   terms) reads as noise to them and leaks process the reader never asked
 *   about. A future edit that pastes an internal status line in verbatim gets
 *   an error naming the word, not a quiet publish.
 * - **An undated pulse is refused.** The page's honesty rests entirely on the
 *   reader knowing when it was written; `updatedDate` must be a real
 *   YYYY-MM-DD string, because "recently" is not a date.
 */

const SCHEMA = 'dclutch-building-pulse-v1';

/**
 * Words that mean nothing outside the project's own process — each with the
 * public phrasing the fixture should use instead. Checked case-insensitively,
 * on word boundaries, across every string in the fixture.
 */
const INTERNAL_VOCABULARY: ReadonlyArray<readonly [RegExp, string]> = [
  [/\bcohorts?\b/i, 'say "coordinated release" instead of cohort'],
  [/\blanes?\b/i, 'name the work, not the lane doing it'],
  [/\bswarms?\b/i, 'name the work, not the swarm doing it'],
  [/\bseams?\b/i, 'describe the boundary in plain words instead of "seam"'],
  [/\bWAVE(\.md)?\b/, 'the planning ledger is internal; cite what happened instead'],
];

export type BuildingStatV1 = Readonly<{ value: string; label: string; detail: string }>;
export type BuildingItemV1 = Readonly<{ title: string; detail: string }>;
export type BuildingWallV1 = Readonly<{ name: string; epitaph: string }>;
export type BuildingLinkV1 = Readonly<{ href: string; label: string }>;

export type BuildingPulseV1 = Readonly<{
  schema: typeof SCHEMA;
  updatedDate: string;
  updatedTime: string;
  eyebrow: string;
  headline: string;
  lede: string;
  stats: ReadonlyArray<BuildingStatV1>;
  statsProvenance: string;
  now: ReadonlyArray<BuildingItemV1>;
  recent: ReadonlyArray<BuildingItemV1>;
  walls: Readonly<{ intro: string; entries: ReadonlyArray<BuildingWallV1> }>;
  closing: string;
  links: ReadonlyArray<BuildingLinkV1>;
}>;

function object(value: unknown, field: string): Record<string, unknown> {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) throw new Error(`${field} must be one object`);
  return value as Record<string, unknown>;
}

function exactKeys(value: Record<string, unknown>, keys: ReadonlyArray<string>, field: string): void {
  const actual = Object.keys(value).sort(); const expected = [...keys].sort();
  if (actual.length !== expected.length || actual.some((key, index) => key !== expected[index])) throw new Error(`${field} has missing or unknown fields`);
}

function prose(value: unknown, field: string): string {
  if (typeof value !== 'string' || value.trim() === '') throw new Error(`${field} must be a non-empty string`);
  for (const [pattern, advice] of INTERNAL_VOCABULARY) {
    const hit = value.match(pattern);
    if (hit !== null) throw new Error(`${field} uses internal vocabulary ("${hit[0]}") — ${advice}`);
  }
  return value;
}

function items(value: unknown, field: string): Array<Record<string, unknown>> {
  if (!Array.isArray(value) || value.length === 0) throw new Error(`${field} must be a non-empty list`);
  return value.map((entry, index) => object(entry, `${field}[${index}]`));
}

/** Parse the sole static input behind `/building`. */
export function parseBuildingPulseV1(value: unknown): BuildingPulseV1 {
  const root = object(value, 'building pulse');
  exactKeys(root, ['schema', 'updatedDate', 'updatedTime', 'eyebrow', 'headline', 'lede', 'stats', 'statsProvenance', 'now', 'recent', 'walls', 'closing', 'links'], 'building pulse');
  if (root.schema !== SCHEMA) throw new Error('building pulse has another schema');
  if (typeof root.updatedDate !== 'string' || !/^\d{4}-\d{2}-\d{2}$/.test(root.updatedDate)) throw new Error('building pulse updatedDate must be one YYYY-MM-DD date');
  const stats = items(root.stats, 'building pulse stats').map((stat, index) => {
    exactKeys(stat, ['value', 'label', 'detail'], `building pulse stats[${index}]`);
    return Object.freeze({ value: prose(stat.value, `stats[${index}].value`), label: prose(stat.label, `stats[${index}].label`), detail: prose(stat.detail, `stats[${index}].detail`) });
  });
  const list = (value: unknown, field: string): ReadonlyArray<BuildingItemV1> => Object.freeze(items(value, field).map((entry, index) => {
    exactKeys(entry, ['title', 'detail'], `${field}[${index}]`);
    return Object.freeze({ title: prose(entry.title, `${field}[${index}].title`), detail: prose(entry.detail, `${field}[${index}].detail`) });
  }));
  const wallsRaw = object(root.walls, 'building pulse walls');
  exactKeys(wallsRaw, ['intro', 'entries'], 'building pulse walls');
  const walls = Object.freeze({
    intro: prose(wallsRaw.intro, 'walls.intro'),
    entries: Object.freeze(items(wallsRaw.entries, 'building pulse walls.entries').map((entry, index) => {
      exactKeys(entry, ['name', 'epitaph'], `walls.entries[${index}]`);
      return Object.freeze({ name: prose(entry.name, `walls.entries[${index}].name`), epitaph: prose(entry.epitaph, `walls.entries[${index}].epitaph`) });
    })),
  });
  const links = Object.freeze(items(root.links, 'building pulse links').map((entry, index) => {
    exactKeys(entry, ['href', 'label'], `building pulse links[${index}]`);
    const href = prose(entry.href, `links[${index}].href`);
    if (!href.startsWith('/')) throw new Error(`links[${index}].href must be a site-relative route`);
    return Object.freeze({ href, label: prose(entry.label, `links[${index}].label`) });
  }));
  return Object.freeze({
    schema: SCHEMA,
    updatedDate: root.updatedDate,
    updatedTime: prose(root.updatedTime, 'building pulse updatedTime'),
    eyebrow: prose(root.eyebrow, 'building pulse eyebrow'),
    headline: prose(root.headline, 'building pulse headline'),
    lede: prose(root.lede, 'building pulse lede'),
    stats: Object.freeze(stats),
    statsProvenance: prose(root.statsProvenance, 'building pulse statsProvenance'),
    now: list(root.now, 'building pulse now'),
    recent: list(root.recent, 'building pulse recent'),
    walls,
    closing: prose(root.closing, 'building pulse closing'),
    links,
  });
}

/** Update only fixtures/building-pulse.json when the state of work changes. */
export const BUILDING_PULSE_V1 = parseBuildingPulseV1(manifest);
