import { describe, expect, it } from 'vitest';

import {
  FOUNDING_LADDER_V1,
  browserDrivableRungsV1,
  summarizeFoundingLadderV1,
} from './ladder';

describe('the founding ladder', () => {
  it('does not claim the browser can found a Market on its own', () => {
    // The whole point of this module. If someone adds a browser builder for
    // every rung, this assertion is what tells them to go update the copy that
    // currently says otherwise.
    const summary = summarizeFoundingLadderV1();
    expect(summary.browserComplete).toBe(false);
    expect(summary.toolingOnly).toBeGreaterThan(0);
    expect(summary.browserBuilders + summary.browserFrames + summary.toolingOnly).toBe(summary.rungs);
  });

  it('gives every rung a builder and a reason, and never a placeholder one', () => {
    for (const rung of FOUNDING_LADDER_V1) {
      expect(rung.builder.length, rung.id).toBeGreaterThan(0);
      expect(rung.reason.length, rung.id).toBeGreaterThan(40);
      expect(rung.effect.length, rung.id).toBeGreaterThan(40);
      expect(rung.reason, rung.id).not.toMatch(/^(TODO|TBD|not yet)/i);
    }
  });

  it('names what would have to exist first for every tooling-only rung', () => {
    // A rung marked tooling-only without a named blocker is a shrug. Each of
    // these reasons has to point at a specific missing thing.
    for (const rung of FOUNDING_LADDER_V1.filter((entry) => entry.status === 'tooling-only')) {
      expect(rung.reason, rung.id).toMatch(/emitter|builder|encoder|derived|derivation|allocate/i);
    }
  });

  it('lists the rungs in execution order, ending at the Open-last outer', () => {
    expect(FOUNDING_LADDER_V1.map((rung) => rung.id)).toEqual([
      'collateral',
      'records',
      'rent-credit',
      'found37',
      'custody-bootstrap',
      'founding-requests',
      'prefunding',
      'routing-table',
      'dcltgmf3',
    ]);
    expect(FOUNDING_LADDER_V1.at(-1)?.id).toBe('dcltgmf3');
  });

  it('offers exactly the three rungs a browser drives today, in execution order', () => {
    // The routing table joined them once the browser could build one, and that
    // is the whole reason Found37 is drivable at all: it does not fit a packet
    // inline with the ComputeBudget limit it cannot execute without.
    expect(browserDrivableRungsV1().map((rung) => rung.id)).toEqual(['rent-credit', 'found37', 'routing-table']);
  });

  it('marks the three rungs that ride a lookup table', () => {
    expect(FOUNDING_LADDER_V1.filter((rung) => rung.lookupTable).map((rung) => rung.id))
      .toEqual(['found37', 'custody-bootstrap', 'dcltgmf3']);
  });

  it('has unique rung identifiers', () => {
    expect(new Set(FOUNDING_LADDER_V1.map((rung) => rung.id)).size).toBe(FOUNDING_LADDER_V1.length);
  });
});
