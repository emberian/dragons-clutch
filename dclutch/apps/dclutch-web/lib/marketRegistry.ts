import { PublicKey } from '@solana/web3.js';

import registry from '@/fixtures/market-registry.devnet.json';
import { type MarketCorePhaseV2 } from './marketCoreV2';
import { shortAddressV1 } from './marketDiscovery';
import {
  derivedOutcomeLabelsV1,
  derivedQuestionV1,
  derivedTitleV1,
  type CoordinateNamingV1,
  type MarketQuestionV1,
} from './marketQuestion';

/**
 * The editorial half of a market page.
 *
 * A Core Market account stores no title, no question, and no story — only
 * identities, phase, and numbers. Prediction markets are DEFINED by their
 * question, so this site ships one file of editorial entries keyed by market
 * address, and says plainly on every surface that renders one that the words
 * are the site's and everything beside them is read from the chain.
 *
 * The registry is deliberately narrow: it may NAME things, and it may TELL a
 * market's story, but nothing in it can move a number, a phase, an address, or
 * a refusal — those always come from the finalized reads.
 *
 * IT IS NO LONGER THE ONLY AUTHOR, and that is the repair this file needed.
 * Keyed by market address, it went stale at every redeploy: on 2026-09-02 it
 * named six markets on a Core program closed the day before and did not name
 * the one open market at all, so the live market rendered as
 * `Unnamed · EQnY…mGs1` with no question and no outcome names, while its own
 * records carried the cuts, the denominator and the window. `marketQuestion.ts`
 * reads those records; `marketNarrativeV1` below is where the two meet, and the
 * order is editorial first, chain second, address last. An address missing from
 * this file now renders its real boundaries and its real settlement time,
 * missing only the coordinate's common name — which is the one thing the chain
 * genuinely does not carry.
 */

const SCHEMA = 'dclutch-market-registry-v1';

export type MarketEditorialEntryV1 = Readonly<{
  /** Short display name, or null to take the derived one. Never chain-read. */
  title: string | null;
  /** The market's question as this site words it, or null to derive it. */
  question: string | null;
  /**
   * What this market's cuts measure, where the chain has no word for it.
   *
   * The one editorial field that does NOT go stale at a redeploy: a coordinate
   * is `SOL/USD` on every cohort that ever measures it, while a title, a
   * question and an outcome list restate boundaries the records already carry
   * and so drift the moment a market is re-founded on different cuts. Naming
   * the coordinate and deriving everything else is the whole shape of the fix.
   */
  coordinate: CoordinateNamingV1 | null;
  /** Editorial names for the outcome cells, index-ordered, or null. */
  outcomes: ReadonlyArray<string> | null;
  /** How the question settles, in words — the design, not a promise. */
  resolution: string | null;
  /** What happened to this market, told once, kindly, and honestly. */
  story: string | null;
}>;

export type MarketRegistryV1 = Readonly<{
  schema: typeof SCHEMA;
  cluster: 'devnet';
  provenance: string;
  markets: Readonly<Record<string, MarketEditorialEntryV1>>;
}>;

function object(value: unknown, field: string): Record<string, unknown> {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) throw new Error(`${field} must be one object`);
  return value as Record<string, unknown>;
}

function prose(value: unknown, field: string): string {
  if (typeof value !== 'string' || value.trim() === '' || value.trim() !== value) {
    throw new Error(`${field} must be one trimmed, non-empty string`);
  }
  return value;
}

function address(value: string, field: string): string {
  let parsed: PublicKey;
  try { parsed = new PublicKey(value); } catch { throw new Error(`${field} must be one canonical Solana address`); }
  if (parsed.toBase58() !== value) throw new Error(`${field} must be one canonical Solana address`);
  return value;
}

function entry(value: unknown, field: string): MarketEditorialEntryV1 {
  const root = object(value, field);
  const keys = Object.keys(root);
  for (const key of keys) {
    if (!['title', 'question', 'coordinate', 'outcomes', 'resolution', 'story'].includes(key)) throw new Error(`${field} has an unknown field: ${key}`);
  }
  const title = root.title === undefined || root.title === null ? null : prose(root.title, `${field} title`);
  const question = root.question === undefined || root.question === null ? null : prose(root.question, `${field} question`);
  let coordinate: CoordinateNamingV1 | null = null;
  if (root.coordinate !== undefined && root.coordinate !== null) {
    const naming = object(root.coordinate, `${field} coordinate`);
    for (const key of Object.keys(naming)) {
      if (!['label', 'unitPrefix'].includes(key)) throw new Error(`${field} coordinate has an unknown field: ${key}`);
    }
    coordinate = Object.freeze({
      label: prose(naming.label, `${field} coordinate label`),
      unitPrefix: naming.unitPrefix === undefined || naming.unitPrefix === null ? null : prose(naming.unitPrefix, `${field} coordinate unit prefix`),
    });
  }
  let outcomes: ReadonlyArray<string> | null = null;
  if (root.outcomes !== undefined && root.outcomes !== null) {
    if (!Array.isArray(root.outcomes) || root.outcomes.length === 0) throw new Error(`${field} outcomes must be a non-empty array or null`);
    outcomes = Object.freeze(root.outcomes.map((label, index) => prose(label, `${field} outcome ${index}`)));
  }
  const resolution = root.resolution === undefined || root.resolution === null ? null : prose(root.resolution, `${field} resolution`);
  const story = root.story === undefined || root.story === null ? null : prose(root.story, `${field} story`);
  if (title === null && question === null && coordinate === null && outcomes === null && resolution === null && story === null) {
    throw new Error(`${field} says nothing; delete the row instead of shipping an empty one`);
  }
  return Object.freeze({ title, question, coordinate, outcomes, resolution, story });
}

/** Parse the shipped editorial registry, refusing shapes it does not know. */
export function parseMarketRegistryV1(value: unknown): MarketRegistryV1 {
  const root = object(value, 'market registry');
  const keys = Object.keys(root).sort();
  if (keys.join(',') !== 'cluster,markets,provenance,schema') throw new Error('market registry has missing or unknown fields');
  if (root.schema !== SCHEMA || root.cluster !== 'devnet') throw new Error('market registry has another schema or cluster');
  const provenance = prose(root.provenance, 'market registry provenance');
  const marketsRaw = object(root.markets, 'market registry markets');
  const markets = Object.freeze(Object.fromEntries(
    Object.entries(marketsRaw).map(([key, value]) => [address(key, 'market registry key'), entry(value, `market registry entry ${key}`)]),
  ));
  return Object.freeze({ schema: SCHEMA, cluster: 'devnet', provenance, markets });
}

/** The shipped devnet registry. Update only fixtures/market-registry.devnet.json. */
export const MARKET_REGISTRY_V1 = parseMarketRegistryV1(registry);

/** One rendered sentence saying whose words the editorial fields are. */
export const MARKET_EDITORIAL_NOTE_V1 =
  'The name, question, and story here are this site’s editorial — the chain stores no names. Every number, phase, address, and refusal is read from the chain.';

/**
 * The editorial entry for one market address, or null.
 *
 * Keyed by address alone, and that is cluster-safe by construction: a Market
 * address is derived from its full identity, including the program ids of the
 * deployment it lives on, so an address in this devnet-scoped file cannot
 * denote a different market on another cluster.
 */
export function marketEditorialV1(marketAddress: string): MarketEditorialEntryV1 | null {
  return MARKET_REGISTRY_V1.markets[marketAddress] ?? null;
}

/**
 * A display title for a market with no registry entry. Phase-aware and
 * plainly generated: a founding that never finished is labelled as build-out
 * debris, anything else says outright that no name is on file.
 */
export function fallbackMarketTitleV1(phase: MarketCorePhaseV2 | null, marketAddress: string): string {
  const short = shortAddressV1(marketAddress, 4);
  if (phase === 'Founding') return `Unfinished · ${short}`;
  return `Unnamed · ${short}`;
}

/**
 * What the page actually says about one market, and who said it.
 *
 * Three authors, in order. The registry may name a market and tell its story;
 * the market's own records say how many outcomes it has, where their
 * boundaries fall and when the window closes; the address is the last resort
 * and only ever a last resort. Nothing here can move a number: every quantity
 * in the derived arm came out of `inspectMarketQuestionV1`, and the registry
 * cannot overwrite one — it can only supply words the chain does not carry.
 *
 * `source` is on the record because a reader is owed it. "SOL/USD — which side
 * of the week" and "$98 – $102" are not the same kind of claim, and the page
 * says which is editorial and which was read.
 */
export type MarketNarrativeV1 = Readonly<{
  title: string;
  question: string | null;
  outcomes: ReadonlyArray<string> | null;
  resolution: string | null;
  story: string | null;
  /** Where the title and question came from. */
  titleSource: 'registry' | 'chain' | 'address';
  questionSource: 'registry' | 'chain' | 'none';
  outcomeSource: 'registry' | 'chain' | 'none';
}>;

export function marketNarrativeV1(
  marketAddress: string,
  phase: MarketCorePhaseV2 | null,
  editorial: MarketEditorialEntryV1 | null,
  derived: MarketQuestionV1 | null,
): MarketNarrativeV1 {
  const naming = editorial?.coordinate ?? null;
  const derivedTitle = derived === null ? null : derivedTitleV1(derived, naming);
  const derivedQuestion = derived === null ? null : derivedQuestionV1(derived, naming);
  const derivedOutcomes = derived === null ? null : derivedOutcomeLabelsV1(derived, naming);
  return Object.freeze({
    title: editorial?.title ?? derivedTitle ?? fallbackMarketTitleV1(phase, marketAddress),
    question: editorial?.question ?? derivedQuestion,
    outcomes: editorial?.outcomes ?? derivedOutcomes,
    resolution: editorial?.resolution ?? null,
    story: editorial?.story ?? null,
    titleSource: editorial?.title != null ? 'registry' : derivedTitle !== null ? 'chain' : 'address',
    questionSource: editorial?.question != null ? 'registry' : derivedQuestion !== null ? 'chain' : 'none',
    outcomeSource: editorial?.outcomes != null ? 'registry' : derivedOutcomes !== null ? 'chain' : 'none',
  });
}
