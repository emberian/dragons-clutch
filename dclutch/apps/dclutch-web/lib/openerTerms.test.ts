import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

import {
  COMPACTION_CRANK_REWARD_LAMPORTS_V1,
  OPENER_ACCOUNT_WIDTHS_V1,
  lamportsAsSolV1,
  openerFirstCrankV1,
} from './openerTerms';

/**
 * A SOURCE GATE, in the shape `reservationVocabulary.test.ts` established.
 *
 * `openerTerms.ts` restates four account widths, one reward cap and one
 * ORDERING that live in Rust. Restating is how a browser gets to compute
 * anything at all, and it is also how a browser starts telling a stranger
 * something the protocol stopped doing. So this file reads the Rust and pins
 * each of them: change a width, change the cap, or pay the opener before the
 * cranker, and the browser goes red here rather than drifting.
 *
 * The ordering pin is the one that matters. Every total in `openerFirstCrankV1`
 * still adds up under the reversed order -- conservation is not what
 * distinguishes them -- and the reversed order is the one the DESIGN originally
 * stated, so it is a live alternative someone could reasonably re-adopt. What
 * separates them is who is left short, which is exactly what the page tells a
 * founder.
 */

const repoRoot = join(fileURLToPath(new URL('.', import.meta.url)), '..', '..', '..');
const read = (path: string) => readFileSync(join(repoRoot, path), 'utf8');

const CLAIM_CHECK_V1 = 'crates/dclutch-claims/src/claim_check_v1.rs';
const CONSERVATION_V1 = 'crates/dclutch-claims/src/claim_check_conservation_v1.rs';

const constant = (source: string, name: string) => {
  const match = source.match(new RegExp(`pub const ${name}: u(?:64|size) = ([0-9_]+);`));
  if (match === null) throw new Error(`${name} is no longer a literal constant in the Rust`);
  return BigInt(match[1].replaceAll('_', ''));
};

describe('the widths and the cap this file restates are the Rust\'s', () => {
  const source = read(CLAIM_CHECK_V1);

  it('pins the two claim-check widths', () => {
    expect(constant(source, 'CLAIM_CHECK_BYTES_V1')).toBe(BigInt(OPENER_ACCOUNT_WIDTHS_V1.claimCheck));
    expect(constant(source, 'CLAIM_CHECK_ESCROW_BYTES_V1')).toBe(BigInt(OPENER_ACCOUNT_WIDTHS_V1.claimCheckEscrow));
  });

  it('pins the crank reward cap', () => {
    expect(constant(source, 'COMPACTION_CRANK_REWARD_LAMPORTS_V1')).toBe(COMPACTION_CRANK_REWARD_LAMPORTS_V1);
  });

  it('pins the Position width formula', () => {
    // `fn position_bytes(outcomes: u64) -> u64 { 128 + 8 * outcomes }`, stated
    // in the conservation module's own fixtures.
    const conservation = read(CONSERVATION_V1);
    const formula = conservation.match(/fn position_bytes\(outcomes: u64\) -> u64 \{\s*(\d+) \+ (\d+) \* outcomes/);
    expect(formula).not.toBeNull();
    expect(Number(formula![1])).toBe(OPENER_ACCOUNT_WIDTHS_V1.positionHeader);
    expect(Number(formula![2])).toBe(OPENER_ACCOUNT_WIDTHS_V1.positionPerOutcome);
  });
});

describe('the crank is still paid before the opener', () => {
  const source = read(CONSERVATION_V1);

  it('binds crank_reward strictly before opener_repayment', () => {
    const crank = source.indexOf('let crank_reward = observation.crank_reward_cap.min(after_rent);');
    const opener = source.indexOf('let opener_repayment = observation.opener_debt.min(after_reward);');
    expect(crank).toBeGreaterThan(-1);
    expect(opener).toBeGreaterThan(-1);
    expect(crank).toBeLessThan(opener);
  });

  it('still carries the argument for that order in its own words', () => {
    // If the kernel ever stops arguing for the inversion, the browser must stop
    // asserting it. The sentence is load-bearing, not decoration.
    expect(source).toContain('An unfunded crank is an unturned crank');
    expect(source).toContain('the crank is paid first, the opener is repaid from what remains');
  });
});

describe('the arithmetic reproduces the cohorts\' own numbers', () => {
  // `rent_exempt_reference_v1` and the cluster both compute
  // `(128 + bytes) * lamports_per_byte`, so a rate is all the harness needs.
  const at = (lamportsPerByte: bigint) => (bytes: number) =>
    (BigInt(bytes) + 128n) * lamportsPerByte;

  it('reproduces cohort-9\'s recorded shortfall to within a rounding of its own', () => {
    // Cohort-9 recorded 1,348,376 lamports short for a four-outcome market
    // (`WAVE.md`, FRACCHECK-7 ruling 3). Re-derived at HEAD's widths and the
    // kernel's reference rate it is 1,348,400: the ruling's number reproduces.
    const plan = openerFirstCrankV1({ outcomeCount: 4, rentFor: at(6_960n) });
    expect(plan.openerStillOwed).toBe(1_348_400n);
    expect(plan.crankReward).toBe(COMPACTION_CRANK_REWARD_LAMPORTS_V1);
    expect(plan.rentCreditResidue).toBe(0n);
  });

  it('reproduces the sweep cohort-14 measured on chain', () => {
    // `COHORT14:1698-1702` reads 2 x 5,877,024 lamports of PDA rent, and
    // `COHORT15:2147-2152` gives the founding rate as 6,333 a byte with the
    // Position at 288 bytes / 1,823,904 lamports. A four-outcome Position plus
    // the admission record is exactly that 5,877,024.
    const plan = openerFirstCrankV1({ outcomeCount: 4, rentFor: at(6_333n) });
    expect(plan.released).toBe(5_877_024n);
    expect(plan.openerStillOwed).toBe(1_244_945n);
    expect(lamportsAsSolV1(plan.openerStillOwed)).toBe('0.001244945');
  });

  it('shrinks with the cluster\'s rent rate, which is why nothing here is typed', () => {
    // Devnet moved 6,333 -> 5,080 a byte at the epoch-1141 boundary during
    // cohort-15. A page quoting the first figure would have been a fifth wrong
    // within a day.
    const cheaper = openerFirstCrankV1({ outcomeCount: 4, rentFor: at(5_080n) });
    expect(cheaper.openerStillOwed).toBe(1_038_200n);
    expect(cheaper.openerStillOwed).toBeLessThan(1_244_945n);
  });

  it('conserves every lamport it sweeps, at every rate and width', () => {
    for (const rate of [5_080n, 6_333n, 6_960n]) {
      for (const outcomeCount of [2, 3, 4, 8, 64]) {
        const plan = openerFirstCrankV1({ outcomeCount, rentFor: at(rate) });
        expect(plan.claimCheckTopUp + plan.crankReward + plan.openerRepayment + plan.rentCreditResidue)
          .toBe(plan.released);
      }
    }
  });

  it('pays the crank even when that leaves the opener short, and never refuses', () => {
    const thin = openerFirstCrankV1({ outcomeCount: 2, rentFor: at(6_333n) });
    expect(thin.crankReward).toBe(COMPACTION_CRANK_REWARD_LAMPORTS_V1);
    expect(thin.openerStillOwed).toBeGreaterThan(0n);
    // And a sweep too thin to pay the full cap pays what there is rather than
    // refusing -- the property the kernel calls load-bearing.
    const starved = openerFirstCrankV1({ outcomeCount: 2, rentFor: at(6_333n), crankRewardCapLamports: 10n ** 12n });
    expect(starved.crankReward).toBe(starved.released - starved.claimCheckTopUp);
    expect(starved.openerRepayment).toBe(0n);
  });
});
