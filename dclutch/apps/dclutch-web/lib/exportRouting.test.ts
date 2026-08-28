import { describe, expect, it } from 'vitest';

import { resolveExportedPathnameV1 } from './exportRouting';

describe('resolveExportedPathnameV1', () => {
  it('routes a Market permalink to the detail surface', () => {
    expect(
      resolveExportedPathnameV1('/markets/So11111111111111111111111111111111111111112'),
    ).toEqual({
      kind: 'market-detail',
      address: 'So11111111111111111111111111111111111111112',
    });
  });

  it('tolerates a trailing slash, which some hosts add', () => {
    expect(resolveExportedPathnameV1('/markets/AbCd/')).toEqual({
      kind: 'market-detail',
      address: 'AbCd',
    });
  });

  it('percent-decodes the address segment exactly once', () => {
    expect(resolveExportedPathnameV1('/markets/%41%42')).toEqual({
      kind: 'market-detail',
      address: 'AB',
    });
  });

  it('routes structurally: a non-address still reaches the surface that refuses it', () => {
    // The detail surface names why those bytes are not a Market. A validator
    // here could only say "no", less precisely and one page earlier.
    expect(resolveExportedPathnameV1('/markets/not-an-address')).toEqual({
      kind: 'market-detail',
      address: 'not-an-address',
    });
  });

  it('does not claim the Market list itself', () => {
    // /markets IS prerendered; the 404 document should never be asked for it,
    // and must not answer for it if it is.
    expect(resolveExportedPathnameV1('/markets')).toEqual({
      kind: 'not-found',
      pathname: '/markets',
    });
  });

  it('does not claim a deeper path under a Market', () => {
    expect(resolveExportedPathnameV1('/markets/AbCd/trade')).toEqual({
      kind: 'not-found',
      pathname: '/markets/AbCd/trade',
    });
  });

  it('does not claim an empty address', () => {
    expect(resolveExportedPathnameV1('/markets/')).toEqual({
      kind: 'not-found',
      pathname: '/markets/',
    });
  });

  it('reports a genuinely unknown path as not found', () => {
    expect(resolveExportedPathnameV1('/no-such-page')).toEqual({
      kind: 'not-found',
      pathname: '/no-such-page',
    });
    expect(resolveExportedPathnameV1('/')).toEqual({ kind: 'not-found', pathname: '/' });
  });

  it('refuses a malformed percent-escape rather than throwing', () => {
    expect(resolveExportedPathnameV1('/markets/%E0%A4%A')).toEqual({
      kind: 'not-found',
      pathname: '/markets/%E0%A4%A',
    });
  });
});
