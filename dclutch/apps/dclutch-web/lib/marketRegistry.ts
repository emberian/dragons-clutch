import { PublicKey } from '@solana/web3.js';

import registry from '@/fixtures/market-registry.devnet.json';
import { type MarketCorePhaseV2 } from './marketCoreV2';
import { shortAddressV1 } from './marketDiscovery';

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
 * a refusal — those always come from the finalized reads. An address missing
 * from this file renders exactly as before: address first, no invented name.
 */

const SCHEMA = 'dclutch-market-registry-v1';

export type MarketEditorialEntryV1 = Readonly<{
  /** Short display name. Editorial, never chain-read. */
  title: string;
  /** The market's question, as this site words it. */
  question: string;
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
    if (!['title', 'question', 'outcomes', 'resolution', 'story'].includes(key)) throw new Error(`${field} has an unknown field: ${key}`);
  }
  const title = prose(root.title, `${field} title`);
  const question = prose(root.question, `${field} question`);
  let outcomes: ReadonlyArray<string> | null = null;
  if (root.outcomes !== undefined && root.outcomes !== null) {
    if (!Array.isArray(root.outcomes) || root.outcomes.length === 0) throw new Error(`${field} outcomes must be a non-empty array or null`);
    outcomes = Object.freeze(root.outcomes.map((label, index) => prose(label, `${field} outcome ${index}`)));
  }
  const resolution = root.resolution === undefined || root.resolution === null ? null : prose(root.resolution, `${field} resolution`);
  const story = root.story === undefined || root.story === null ? null : prose(root.story, `${field} story`);
  return Object.freeze({ title, question, outcomes, resolution, story });
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
