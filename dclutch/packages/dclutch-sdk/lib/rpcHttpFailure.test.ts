import { describe, expect, it } from 'vitest';

import { httpFailureReasonV1 } from './rpc';

/**
 * A reader of the public site meets these. The landing reads the chain on
 * every visit, the public devnet endpoint throttles bulk reads, and until now
 * the whole explanation a visitor got was the number 429.
 */
describe('what a person reads when the node says no', () => {
  it('explains a throttle as a throttle, and says what to do about it', () => {
    const reason = httpFailureReasonV1(429);
    expect(reason).toContain('rate-limiting this browser');
    expect(reason).toContain('wait a few seconds');
    // The status stays in the sentence: someone debugging still needs it.
    expect(reason).toContain('429');
  });

  it('separates an endpoint being down from the request being wrong', () => {
    for (const status of [502, 503, 504]) {
      const reason = httpFailureReasonV1(status);
      expect(reason).toContain('unavailable right now');
      expect(reason).toContain('Nothing is wrong with the chain');
      expect(reason).toContain(String(status));
    }
  });

  it('does not dress an unknown failure up as an understood one', () => {
    // A status we have no account of gets its number and no story.
    expect(httpFailureReasonV1(418)).toBe('RPC HTTP status 418');
    expect(httpFailureReasonV1(400)).toBe('RPC HTTP status 400');
  });
});
