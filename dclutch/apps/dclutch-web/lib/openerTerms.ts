/**
 * What opening a market costs the party who opens it, in lamports, derived.
 *
 * # The fact this file exists to state honestly
 *
 * A claim-check escrow's opener advances rent for the escrow record and its
 * token vault out of pocket. The first permissionless compaction crank sweeps
 * two closing accounts, pays the claim check's own rent out of the proceeds,
 * pays the CRANKER, and only then repays the opener from what is left. On a
 * binary or four-outcome market the sweep does not cover both, so a
 * single-crank market never repays its opener in full.
 *
 * That order is deliberate and the kernel argues for it in its own words: the
 * design's stated order paid the opener first, which does not close
 * arithmetically -- the first crank would pay itself exactly nothing, and an
 * unfunded crank is an unturned crank. `crates/dclutch-claims/src/
 * claim_check_conservation_v1.rs`, `ClaimCheckCompactionPlanV1::new`.
 *
 * # Why the numbers are computed rather than quoted
 *
 * Rent is a cluster parameter and it MOVES: devnet went from 6,333 to 5,080
 * lamports a byte at the epoch-1141 boundary during cohort-15
 * (`docs/evidence/COHORT15_DEPLOYED_SEALED_FOUNDED_CAPTURED_2026_09_04.md`),
 * which changes this figure by a fifth. A number typed into a page is a number
 * that goes quietly wrong. So the caller supplies `rentFor`, which reads
 * `getMinimumBalanceForRentExemption` off the cluster the page is pointed at,
 * and everything below is arithmetic over what the chain said.
 *
 * The widths and the reward cap are pinned against the Rust by
 * `openerTerms.test.ts`, which reads the kernel source: if a width changes or
 * the crank stops being paid first, this file goes red rather than drifting.
 */

/** Account widths this crank moves, from `crates/dclutch-claims`. */
export const OPENER_ACCOUNT_WIDTHS_V1 = Object.freeze({
  /** `CLAIM_CHECK_BYTES_V1` -- the record the crank mints. */
  claimCheck: 288,
  /** `CLAIM_CHECK_ESCROW_BYTES_V1` -- the escrow the opener creates. */
  claimCheckEscrow: 256,
  /** A Token-2022 account: the vault the opener funds alongside the escrow. */
  tokenAccount: 165,
  /** The admission record the crank sweeps. */
  admission: 512,
  /** `position_bytes(outcomes) = positionHeader + positionPerOutcome * n`. */
  positionHeader: 128,
  positionPerOutcome: 8,
});

/**
 * `COMPACTION_CRANK_REWARD_LAMPORTS_V1`, the cap on one crank's reward.
 *
 * A cap on a residual, never a demand: a thin position yields a thin reward
 * rather than a refusal, because a compaction that could refuse for lack of
 * funds would reintroduce the sleeping-holder deadlock through the funding
 * door. It is also the one lamport magnitude in this family still written as a
 * source literal rather than derived from Rent, which
 * `docs/design/FUNDED_CRANK_V1.md` section 3 rules against; the governed record
 * `dclutch-market::protocol_parameters` is where it moves to.
 */
export const COMPACTION_CRANK_REWARD_LAMPORTS_V1 = 200_000n;

/** What the first crank does to the opener's advance, lamport by lamport. */
export type OpenerFirstCrankV1 = Readonly<{
  /** What the opener advanced: the escrow record plus its token vault. */
  openerOutlay: bigint;
  /** What the crank sweeps: the Position and the admission record. */
  released: bigint;
  /** What the new claim-check record's own rent takes off the top. */
  claimCheckTopUp: bigint;
  /** What the cranker is paid, first. */
  crankReward: bigint;
  /** What reaches the opener, second. */
  openerRepayment: bigint;
  /** What the opener is still owed after this crank. Zero on a fat sweep. */
  openerStillOwed: bigint;
  /** What reaches the market's RentCredit, last. Zero until the debt clears. */
  rentCreditResidue: bigint;
}>;

const min = (left: bigint, right: bigint) => (left < right ? left : right);

/**
 * Run the kernel's exact order over rents the caller read off a cluster.
 *
 * The order is the whole content: claim-check rent, then the cranker, then the
 * opener, then the residue. Reordering these lines states something false about
 * the protocol even when every total still adds up.
 */
export function openerFirstCrankV1(input: Readonly<{
  outcomeCount: number;
  /** Rent-exempt minimum for a width, read from the cluster. */
  rentFor: (bytes: number) => bigint;
  crankRewardCapLamports?: bigint;
}>): OpenerFirstCrankV1 {
  const widths = OPENER_ACCOUNT_WIDTHS_V1;
  const rent = input.rentFor;
  const positionBytes = widths.positionHeader + widths.positionPerOutcome * input.outcomeCount;

  const openerOutlay = rent(widths.claimCheckEscrow) + rent(widths.tokenAccount);
  const released = rent(positionBytes) + rent(widths.admission);
  const claimCheckTopUp = rent(widths.claimCheck);
  const afterRent = released > claimCheckTopUp ? released - claimCheckTopUp : 0n;

  const crankReward = min(input.crankRewardCapLamports ?? COMPACTION_CRANK_REWARD_LAMPORTS_V1, afterRent);
  const afterReward = afterRent - crankReward;
  const openerRepayment = min(openerOutlay, afterReward);

  return Object.freeze({
    openerOutlay,
    released,
    claimCheckTopUp,
    crankReward,
    openerRepayment,
    openerStillOwed: openerOutlay - openerRepayment,
    rentCreditResidue: afterReward - openerRepayment,
  });
}

/** Lamports as SOL, exactly, with no float anywhere in the path. */
export function lamportsAsSolV1(lamports: bigint): string {
  const whole = lamports / 1_000_000_000n;
  const fraction = (lamports % 1_000_000_000n).toString().padStart(9, '0');
  return `${whole}.${fraction}`;
}
