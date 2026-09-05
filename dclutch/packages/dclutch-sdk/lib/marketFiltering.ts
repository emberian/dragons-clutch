import { type MarketDiscoveryCardV1 } from './marketDiscovery';
import { marketEditorialV1 } from './marketRegistry';

/**
 * Narrowing a listing without deciding anything for the reader.
 *
 * Two markets fit on a screen. The load simulator's whole purpose is to make
 * that stop being true, and a listing nobody can search is a listing nobody
 * reads past the fold. So this is search and sort — and nothing else, because
 * the other things a listing usually grows (a "trending" tab, a default sort
 * by activity, a hidden relevance score) are editorial claims about which
 * market matters, made silently.
 *
 * THE RULES THIS FILE KEEPS.
 *   - A search matches only what the reader can already SEE or copy: the
 *     market's name and question, its address, its phase and generation. It
 *     never matches a field the page does not show, so a card can never
 *     appear for a reason its own text does not explain.
 *   - Filtering removes cards; it never reorders them and never changes what
 *     exists. The surface states both numbers.
 *   - Every ordering is named in the reader's words and is total: ties fall
 *     back to the incoming order, so the same query gives the same list
 *     instead of reshuffling on each read.
 *   - A card whose figures could not be read is never ranked as if it held a
 *     zero. It sorts last, and it still says why it refused.
 */

export type MarketSortOrderV1 = 'enumerated' | 'name' | 'issued';

export type MarketSortChoiceV1 = Readonly<{
  order: MarketSortOrderV1;
  /** What the reader picks, in their words. */
  label: string;
  /** One sentence: what this ordering means, and what it does not claim. */
  meaning: string;
}>;

export const MARKET_SORT_CHOICES_V1: ReadonlyArray<MarketSortChoiceV1> = Object.freeze([
  Object.freeze({
    order: 'enumerated' as const,
    label: 'As the chain lists them',
    meaning: 'Chain order',
  }),
  Object.freeze({
    order: 'name' as const,
    label: 'By name, A to Z',
    meaning: 'Alphabetical; unnamed markets last',
  }),
  Object.freeze({
    order: 'issued' as const,
    label: 'Most claims issued first',
    meaning: 'Most claims issued first',
  }),
]);

export const MARKET_SEARCH_MEANING_V1 =
  'Name, question, address, or phase';

/** Everything about a card a reader can see, lowercased for matching. */
function haystack(card: MarketDiscoveryCardV1): string {
  const editorial = marketEditorialV1(card.address);
  const parts: Array<string> = [card.address];
  if (card.status !== 'refused') parts.push(card.phase, card.generation);
  if (editorial !== null) {
    // Every editorial field is optional now, and a row that names only a
    // coordinate is the common case: `SOL/USD` is exactly what a reader types
    // into this box, so it is searchable text like any other.
    if (editorial.title !== null) parts.push(editorial.title);
    if (editorial.question !== null) parts.push(editorial.question);
    if (editorial.coordinate !== null) parts.push(editorial.coordinate.label);
    if (editorial.outcomes !== null) parts.push(...editorial.outcomes);
  }
  return parts.join(' ').toLowerCase();
}

/**
 * Total issued claim atoms, or null when this card's claims were not read.
 *
 * Null is not zero and is never treated as zero: a market whose Claims
 * aggregate refused to decode has an unknown issuance, and ranking it beneath
 * a market that genuinely issued nothing would invent a comparison.
 */
export function totalIssuedAtomsV1(card: MarketDiscoveryCardV1): bigint | null {
  if (card.status === 'refused' || card.liability.status !== 'bound') return null;
  return card.liability.supplyAtoms.reduce((sum, atoms) => sum + BigInt(atoms), 0n);
}

/**
 * The cards whose visible text contains every whitespace-separated term.
 *
 * Every term must match, so adding a word always narrows. An empty or
 * whitespace-only query returns the list unchanged rather than nothing.
 */
export function filterMarketCardsV1(
  cards: ReadonlyArray<MarketDiscoveryCardV1>,
  query: string,
): ReadonlyArray<MarketDiscoveryCardV1> {
  const terms = query.toLowerCase().split(/\s+/).filter((term) => term.length > 0);
  if (terms.length === 0) return cards;
  return Object.freeze(cards.filter((card) => {
    const text = haystack(card);
    return terms.every((term) => text.includes(term));
  }));
}

/**
 * The cards in the reader's chosen order. The incoming order is the authority
 * for ties, so this is stable against the chain's enumeration and against
 * whatever curation already ran.
 */
/**
 * Reorder one group of cards. GENERIC because a sort is not a widening.
 *
 * Written against the whole `MarketDiscoveryCardV1` union, this handed back
 * the union whatever it was given -- so running it over `MarketListingV1`'s
 * four DECODED groups produced four groups that might contain a refused card,
 * and `RestOfTheRecord`, which correctly asks for the decoded rows, stopped
 * typechecking. The function never reads a decoded-only field and never builds
 * a card; it permutes the array it was handed. Saying so keeps the caller's
 * narrower element type all the way through.
 */
export function sortMarketCardsV1<CardV1 extends MarketDiscoveryCardV1>(
  cards: ReadonlyArray<CardV1>,
  order: MarketSortOrderV1,
): ReadonlyArray<CardV1> {
  if (order === 'enumerated') return cards;
  const indexed = cards.map((card, index) => ({ card, index }));
  if (order === 'name') {
    indexed.sort((left, right) => {
      const leftName = marketEditorialV1(left.card.address)?.title ?? null;
      const rightName = marketEditorialV1(right.card.address)?.title ?? null;
      if (leftName === null && rightName === null) return left.card.address.localeCompare(right.card.address);
      if (leftName === null) return 1;
      if (rightName === null) return -1;
      const byName = leftName.localeCompare(rightName);
      return byName === 0 ? left.index - right.index : byName;
    });
    return Object.freeze(indexed.map((entry) => entry.card));
  }
  indexed.sort((left, right) => {
    const leftAtoms = totalIssuedAtomsV1(left.card);
    const rightAtoms = totalIssuedAtomsV1(right.card);
    if (leftAtoms === null && rightAtoms === null) return left.index - right.index;
    if (leftAtoms === null) return 1;
    if (rightAtoms === null) return -1;
    if (leftAtoms === rightAtoms) return left.index - right.index;
    return leftAtoms > rightAtoms ? -1 : 1;
  });
  return Object.freeze(indexed.map((entry) => entry.card));
}

/** What a surface says when a search removed every card in a group. */
export function noMatchSentenceV1(query: string, total: number): string {
  return `Nothing here matches “${query}”. All ${total} of these markets are still listed — clear the search to see them.`;
}
