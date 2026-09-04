import { describe, expect, it } from 'vitest';

import {
  INSTRUCTION_MAGICS,
  PREDICATE_SELECTED_ROUTES,
  UNRESOLVED_PREDICATE_ARMS_V1,
} from './generated/routeCensus';
import {
  LEADING_BYTE_SELECTED_ROUTES_V1,
  censusRouteIdsForInstructionsV1,
  censusRoutesForInstructionV1,
  censusRoutesForMagicV1,
  instructionMagicV1,
  magicIsAmbiguousV1,
  programsWithALeadingByteSelectorV1,
} from './routeSelector';

/** One instruction whose data is exactly this magic and nothing else. */
function headed(program: string, magic: string, tail = 0): { program: string; data: Uint8Array } {
  const data = new Uint8Array(8 + tail);
  data.set(new TextEncoder().encode(magic), 0);
  return { program, data };
}

describe('the leading-byte derivation is the census’s own table', () => {
  it('names a route for every magic arm the census carries', () => {
    // A table with rows and a lookup that finds none of them would pass every
    // negative case below, so the round trip is asserted first and in bulk.
    expect(INSTRUCTION_MAGICS.length).toBeGreaterThan(40);
    for (const entry of INSTRUCTION_MAGICS) {
      const selected = censusRoutesForInstructionV1(headed(entry.program, entry.magic));
      expect(selected.map((one) => one.routeId), entry.routeId).toContain(entry.routeId);
    }
  });

  it('names a route for every predicate arm the census resolved', () => {
    expect(PREDICATE_SELECTED_ROUTES.length).toBeGreaterThan(20);
    for (const entry of PREDICATE_SELECTED_ROUTES) {
      const selected = censusRoutesForInstructionV1(headed(entry.program, entry.magic));
      expect(selected.map((one) => one.routeId), entry.routeId).toContain(entry.routeId);
    }
  });

  it('names the two routes this browser builds and could not name before', () => {
    // The case that made the predicate resolution worth having: `DCLTHOT3` and
    // `DCLTPUA1` are Trading arms and Trading dispatches on predicates, so
    // before `a44696974` the browser could name the route behind a redemption
    // and not the route behind the fill it is redeeming.
    //
    // This line used to assert that `INSTRUCTION_MAGICS` carries NO Trading
    // row, which was the shape of the defect rather than a property worth
    // keeping. `4b2519c3a` reads each predicate's magic out of its own body,
    // so every Trading arm now reaches the magic table too, and the reference
    // regeneration that surfaced it is `95544a853`. Asserting the old premise
    // would have held the defect open inside a green test.
    expect(censusRouteIdsForInstructionsV1([headed('trading', 'DCLTHOT3')]))
      .toEqual(['trading/hot_v3::process_hot_execution_v3']);
    expect(censusRouteIdsForInstructionsV1([headed('trading', 'DCLTPUA1')]))
      .toEqual(['trading/user_position_admission_v1::process_user_position_admission_v1']);
  });
});

describe('what the derivation refuses to answer', () => {
  it('reads Core’s request magic as the whole Action candidate set', () => {
    // `DCLTCRQ2` is the magic every Core `Action` instruction starts with, and
    // it used to select NOTHING: the check lived only inside `Request::decode`,
    // which the census's dispatch walk treats as terminal, so the honest answer
    // was the empty set and a consumer had to be told not to read that as "this
    // instruction reaches no route". The check is now also in the dispatch
    // guard, so the magic resolves — to eleven routes at once, because Core
    // separates them by a decoded `Action` variant this derivation has no
    // offset for. Ambiguous is a different answer from absent, and this is the
    // test that keeps the two apart.
    expect(magicIsAmbiguousV1('core', 'DCLTCRQ2')).toBe(true);
    expect(censusRouteIdsForInstructionsV1([headed('core', 'DCLTCRQ2', 64)])).toContain(
      'core/found::process#Found',
    );
    expect(LEADING_BYTE_SELECTED_ROUTES_V1.some((entry) => entry.magic === 'DCLTCRQ2')).toBe(true);
  });

  it('returns the whole candidate set when one magic selects several routes', () => {
    // Rent's three lifecycle arms share `DCLRNCI2` and are separated by a
    // decoded variant. Narrowing that by guessing would be the route-magic
    // mistake in another costume.
    expect(magicIsAmbiguousV1('rent', 'DCLRNCI2')).toBe(true);
    expect(censusRouteIdsForInstructionsV1([headed('rent', 'DCLRNCI2', 120)])).toEqual([
      'rent/process_close_v2#Close',
      'rent/process_create_v2#Create',
      'rent/process_sweep_v2#Sweep',
    ]);
  });

  it('reads non-ASCII leading bytes as no magic, not as an unmatched one', () => {
    expect(instructionMagicV1(Uint8Array.from([0, 1, 2, 3, 4, 5, 6, 7]))).toBeNull();
    expect(instructionMagicV1(Uint8Array.from([1, 2, 3]))).toBeNull();
    expect(instructionMagicV1(new TextEncoder().encode('DCLTHOT3'))).toBe('DCLTHOT3');
  });

  it('carries the arms that compare no magic at all, with the census’s reason', () => {
    expect(UNRESOLVED_PREDICATE_ARMS_V1.length).toBeGreaterThan(0);
    for (const arm of UNRESOLVED_PREDICATE_ARMS_V1) {
      expect(arm.reason.length).toBeGreaterThan(20);
      // Every one of them is a real route the census enumerates, so none may
      // be silently absent from the leading-byte table without a reason.
      expect(LEADING_BYTE_SELECTED_ROUTES_V1.some((entry) => entry.routeId === arm.routeId)).toBe(false);
    }
  });

  it('names the programs it can speak about, and Core is in it only partly', () => {
    const programs = programsWithALeadingByteSelectorV1();
    expect(programs).toContain('trading');
    expect(programs).toContain('claims');
    expect(programs).toContain('resolution');
    // Core IS here — `DCLTCSR1`, `DCLTGFQ1` and the rest are magic arms. What
    // is missing is the `Action` family, which is the part an act drives.
    expect(programs).toContain('core');
    expect(censusRoutesForMagicV1('core', 'DCLTGFQ1').map((one) => one.routeId))
      .toEqual(['core/generic_founding_v1::process']);
  });
});
