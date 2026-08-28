import { describe, expect, it } from 'vitest';

import { marketAddressQueryV1, marketDetailHrefV1 } from './marketHref';

describe('static Market permalinks', () => {
  it('encodes one address into the exported 200-status route', () => {
    expect(marketDetailHrefV1(' Ab/Cd+ ')).toBe('/market?address=Ab%2FCd%2B');
  });

  it('distinguishes prerender, missing, malformed, and ready queries', () => {
    expect(marketAddressQueryV1(null)).toEqual({ kind: 'resolving' });
    expect(marketAddressQueryV1('')).toEqual({ kind: 'missing', reason: 'No Market address was supplied in this link.' });
    expect(marketAddressQueryV1('?address=')).toEqual({ kind: 'refused', reason: 'The Market address in this link is empty.' });
    expect(marketAddressQueryV1('?address=one&address=two')).toEqual({ kind: 'refused', reason: 'This link supplies more than one Market address.' });
    expect(marketAddressQueryV1('?address=Ab%2FCd')).toEqual({ kind: 'ready', address: 'Ab/Cd' });
  });

  it('refuses to build an empty link', () => {
    expect(() => marketDetailHrefV1('   ')).toThrow('Market address must not be empty');
  });
});
