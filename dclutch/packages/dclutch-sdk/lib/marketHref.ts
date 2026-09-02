export type MarketAddressQueryV1 =
  | Readonly<{ kind: 'resolving' }>
  | Readonly<{ kind: 'missing'; reason: string }>
  | Readonly<{ kind: 'refused'; reason: string }>
  | Readonly<{ kind: 'ready'; address: string }>;

import { MARKET_REGISTRY_V1 } from './marketRegistry';

/**
 * A static-host-safe Market permalink.
 *
 * A market the shipped registry names has a real exported page at
 * /markets/<address> — with its own title, description, and share card, so
 * THAT is the link people should copy and paste. Every other address gets
 * the query-parameter route, which the static export serves as one document
 * for any address; the served (worker/local) build renders both.
 */
export function marketDetailHrefV1(address: string): string {
  const canonical = address.trim();
  if (canonical === '') throw new Error('Market address must not be empty');
  if (MARKET_REGISTRY_V1.markets[canonical] !== undefined) return `/markets/${encodeURIComponent(canonical)}`;
  return `/market?address=${encodeURIComponent(canonical)}`;
}

/** Interpret the query carried by the static `/market` document. */
export function marketAddressQueryV1(search: string | null): MarketAddressQueryV1 {
  if (search === null) return Object.freeze({ kind: 'resolving' });
  const values = new URLSearchParams(search).getAll('address');
  if (values.length === 0) {
    return Object.freeze({ kind: 'missing', reason: 'No Market address was supplied in this link.' });
  }
  if (values.length !== 1) {
    return Object.freeze({ kind: 'refused', reason: 'This link supplies more than one Market address.' });
  }
  const address = values[0].trim();
  if (address === '') {
    return Object.freeze({ kind: 'refused', reason: 'The Market address in this link is empty.' });
  }
  return Object.freeze({ kind: 'ready', address });
}
