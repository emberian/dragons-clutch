export type MarketAddressQueryV1 =
  | Readonly<{ kind: 'resolving' }>
  | Readonly<{ kind: 'missing'; reason: string }>
  | Readonly<{ kind: 'refused'; reason: string }>
  | Readonly<{ kind: 'ready'; address: string }>;

/** A static-host-safe Market permalink. */
export function marketDetailHrefV1(address: string): string {
  const canonical = address.trim();
  if (canonical === '') throw new Error('Market address must not be empty');
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
