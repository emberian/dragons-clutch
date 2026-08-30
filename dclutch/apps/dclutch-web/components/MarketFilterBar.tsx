'use client';

import {
  MARKET_SEARCH_MEANING_V1,
  MARKET_SORT_CHOICES_V1,
  type MarketSortOrderV1,
} from '@/lib/marketFiltering';

/**
 * The one control this page has ever grown, and the rule that let it.
 *
 * /markets is built on an inversion: you land on CONTENT. The deployment
 * manifest supplies the endpoint and the Core authority, the list loads by
 * itself, and the page never asks a visitor to go find a piece of
 * infrastructure and type it in. That inversion is what the discovery tests
 * protect, and for a long time they protected it with a blunt instrument —
 * the page was asserted to contain no `<input>` at all.
 *
 * A search box is an `<input>` and breaks nothing the inversion cares about.
 * The distinction that actually matters is not whether a control exists but
 * WHAT IT ASKS FOR:
 *
 *   FORBIDDEN, still and always — asking the visitor for infrastructure. An
 *   RPC endpoint, a program address, a Market address to paste, a keypair, a
 *   registry ID: anything a reader must leave the page and go find before the
 *   page will show them what it already knows. That was the pattern, that is
 *   what the ban was for, and none of it is relaxed here. The one place
 *   "bring your own infrastructure" lives is the cluster picker in the nav.
 *
 *   ALLOWED — narrowing or reordering what is already on the page. It asks
 *   for nothing the reader does not have, and it can be ignored entirely: the
 *   page is complete before it is touched.
 *
 * So this control is typed as a search, is labelled for what it searches, and
 * says what it reads. The tests were rewritten to check that rule instead of
 * counting tags.
 */

export type MarketFilterBarPropsV1 = Readonly<{
  query: string;
  onQuery: (next: string) => void;
  order: MarketSortOrderV1;
  onOrder: (next: MarketSortOrderV1) => void;
  /** Cards showing after the search, and how many exist in total. */
  shown: number;
  total: number;
}>;

export default function MarketFilterBar({ query, onQuery, order, onOrder, shown, total }: MarketFilterBarPropsV1) {
  const chosen = MARKET_SORT_CHOICES_V1.find((choice) => choice.order === order) ?? MARKET_SORT_CHOICES_V1[0];
  return <div className="market-filter-bar">
    <label className="market-filter-search">
      <span>Search these markets</span>
      <input
        type="search"
        value={query}
        placeholder="name, question, address, or phase"
        autoComplete="off"
        spellCheck={false}
        onChange={(event) => onQuery(event.target.value)}
      />
    </label>
    <label className="market-filter-order">
      <span>Order</span>
      <select value={order} onChange={(event) => onOrder(event.target.value as MarketSortOrderV1)}>
        {MARKET_SORT_CHOICES_V1.map((choice) => (
          <option key={choice.order} value={choice.order}>{choice.label}</option>
        ))}
      </select>
    </label>
    <p className="market-filter-note">
      {query.trim().length === 0
        ? `${total} market${total === 1 ? '' : 's'} on this deployment.`
        : `${shown} of ${total} market${total === 1 ? '' : 's'} match. Searching hides cards; it never changes what exists.`}
      {' '}{MARKET_SEARCH_MEANING_V1} {chosen.meaning}
    </p>
  </div>;
}
