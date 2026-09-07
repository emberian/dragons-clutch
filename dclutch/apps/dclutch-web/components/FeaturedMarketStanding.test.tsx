import { describe, expect, it } from 'vitest';

import { frontDoorPhraseV1 } from './FeaturedMarketStanding';

describe('the featured-market standing', () => {
  it('names only a phase authenticated by the selected deployment Core', () => {
    expect(frontDoorPhraseV1({ kind: 'read', phase: 'Open' })).toBe('open');
    expect(frontDoorPhraseV1({ kind: 'read', phase: 'Retired' })).toBe('finished');
  });

  it('never turns a stale feature or an unread account into an active-market claim', () => {
    expect(frontDoorPhraseV1({ kind: 'other-cohort' })).toBeNull();
    expect(frontDoorPhraseV1({ kind: 'unread' })).toBeNull();
  });
});
