import { describe, expect, it } from 'vitest';

import { capabilityAccessSentenceV1, capabilityRouteAccessV1 } from '@dclutch/sdk/capabilityAccess';
import { BROWSER_CAPABILITY_STANDINGS_V1 } from './capabilitySurface';

/**
 * The user-inaccessible count, recomputed with declarations that were checked.
 *
 * `docs/evidence/C16_REHEARSAL_2026_09_03.md` section 6.2 published **65 of 78
 * strict** and named the thirteen reachable magics by hand. This is the same
 * question asked of instruments the tree regenerates, and the answer moves in
 * both directions at once, which is why the delta is named per capability
 * below rather than summarised.
 */

const census = capabilityRouteAccessV1(BROWSER_CAPABILITY_STANDINGS_V1);

/**
 * The thirteen the rehearsal scored reachable, as it wrote them.
 *
 * Its rule was "a client encoder or WASM codec actually constructs the bytes",
 * over any module in the three client trees. Quoted here as a claim to compare
 * against, never as an input to the count.
 */
const REHEARSAL_REACHABLE_MAGICS_V1 = [
  'DCFRRQ03', 'DCLTHOT3', 'DCLTGMF3', 'DCLTGFQ1', 'DCLTPCB2', 'DCLTPUA1', 'DCLTSQ03',
  'DCRRPRQ2', 'DCRRLC02', 'DCLCCR01', 'DCLSDP03', 'DCLRNCI2', 'DCLCUSR1',
] as const;

describe('what a reader of this client can actually reach', () => {
  it('counts the routes a leading-byte view can name at all', () => {
    // A census with an empty denominator would make every ratio below vacuous.
    expect(census.selectable).toBe(75);
    expect(census.rows).toHaveLength(census.selectable);
    expect(census.reachable + census.inaccessible).toBe(census.selectable);
  });

  it('names the six routes an act offers, and the sentence carries no typed number', () => {
    expect(census.rows.filter((row) => row.reachableActs.length > 0).map((row) => row.routeId)).toEqual([
      'claims/custody_replay_v1::process',
      'claims/terminal_settlement_v3::process',
      'registry/record_v1::dispatch',
      'resolution/core_effect::process_direct_funding_close_v1',
      'trading/hot_v3::process_hot_execution_v3',
      'trading/user_position_admission_v1::process_user_position_admission_v1',
    ]);
    expect(census.reachable).toBe(6);
    expect(census.inaccessible).toBe(69);
    expect(capabilityAccessSentenceV1(census)).toBe(
      '6 of 75 routes a program selects from an instruction’s first eight bytes are reachable from an act on this page; 69 are not reachable from any client at all.',
    );
  });

  it('names what no leading-byte count can hold, instead of dropping it', () => {
    // Four of these six are the routes carrying the only Market phase gates an
    // act reads. A count that quietly excluded them would understate the
    // catalogue and overstate the instrument in the same breath.
    expect(census.declaredOutsideTheDenominator).toEqual([
      'core/execute_provider_v3::process#ExecuteProvider',
      'core/found::process#Found',
      'core/resolution::process#AdmitTerminal',
      'core/resolution::process#CreateFund',
      'core/resolution::process#VerifyFundReady',
      'trading/user_position_admission_v1::process_user_position_admission_v1#Admit',
    ]);
  });
});

describe('the delta against the rehearsal’s hand count, per capability', () => {
  const magicsHere = new Set(census.rows.flatMap((row) => row.magics));
  const reachableMagics = new Set(census.rows.filter((row) => row.reachableActs.length > 0).flatMap((row) => row.magics));

  it('two arms the rehearsal scored unreachable are reachable, and both are new declarations', () => {
    // `DCLTRIX1` is the Registry instruction `/release` has been compiling
    // since it shipped, and `DCLRFCQ1` is the fund closure `/resolution`
    // plans. The rehearsal's search could not see either, because neither act
    // declared a route for a search to confirm.
    const gained = [...reachableMagics].filter((magic) => !REHEARSAL_REACHABLE_MAGICS_V1.includes(magic as never)).sort();
    expect(gained).toEqual(['DCLRFCQ1', 'DCLTRIX1']);
  });

  it('names the arms a client builds that no act on this board offers', () => {
    // The rehearsal's numerator was "some client module encodes these bytes".
    // This one is "an act a reader can find declares it". The difference is
    // not a disagreement: it is the list of routes this browser can build and
    // does not publish, and three of them the rehearsal itself flagged as
    // child or suffix payloads rather than standalone submissions
    // (`DCLSDP03`, `DCLCUSR1`, `DCLRNCI2`).
    const builtButUnoffered = REHEARSAL_REACHABLE_MAGICS_V1
      .filter((magic) => magicsHere.has(magic) && !reachableMagics.has(magic))
      .sort();
    expect(builtButUnoffered).toEqual([
      'DCFRRQ03', 'DCLRNCI2', 'DCLSDP03', 'DCLTGFQ1', 'DCLTPCB2', 'DCRRLC02', 'DCRRPRQ2',
    ]);
  });

  it('names the arms the rehearsal counted that this denominator cannot hold', () => {
    // `DCLTGMF3`'s predicate decodes a whole struct rather than comparing a
    // magic, and `DCLCUSR1` is a CPI-level Custody request no top-level arm
    // selects — so neither is a route an instruction's first eight bytes can
    // name, whoever builds the bytes.
    const outsideHere = REHEARSAL_REACHABLE_MAGICS_V1.filter((magic) => !magicsHere.has(magic)).sort();
    expect(outsideHere).toEqual(['DCLCUSR1', 'DCLTGMF3']);
  });
});
