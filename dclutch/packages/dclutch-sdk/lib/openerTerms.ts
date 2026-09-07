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
 * door.
 *
 * Its one author is now the governed record's genesis,
 * `PROTOCOL_GENESIS_CRANK_REWARD_CAP_LAMPORTS_V1` in `crates/dclutch-market/
 * src/protocol_parameters/generated.rs`; the Rust constant this restates
 * projects that, and `openerTerms.test.ts` pins the chain rather than the
 * number. It is still a source literal rather than a value read off the record
 * at run time, which `docs/design/FUNDED_CRANK_V1.md` section 3 argues against:
 * the compaction frame does not carry the record, and widening it is its own
 * cohort's work. What changed is that there is one place to edit, not four.
 */
export const COMPACTION_CRANK_REWARD_LAMPORTS_V1 = 200_000n;

/**
 * Solana's fixed per-account storage overhead, in bytes.
 *
 * `ACCOUNT_STORAGE_OVERHEAD_BYTES` in `crates/dclutch-market/src/
 * capability_manifest/generated_abi.rs`, whose one author is
 * `formal/dclutch-semantics/DClutchSemantics/CapabilityManifestV1Abi.lean`.
 */
export const ACCOUNT_STORAGE_OVERHEAD_BYTES_V1 = 128n;

/**
 * What an account of `accountBytes` costs at a founding's RECORDED rate.
 *
 * `funded_rent_minimum_v2` in `crates/dclutch-market/src/capability_manifest/
 * funding.rs`, restated. `Rent::minimum_balance` is affine in the length, so
 * ONE rate prices every account a founding created, at every width -- which is
 * why the persisted fact is the rate and not any one length's minimum.
 *
 * This is the honest pricing for a market that already exists: its accounts
 * hold what its own founding funded them with, at the rate its founding fixed,
 * and the cluster's rate today prices some other market.
 */
export function fundedRentMinimumV1(fundedRentRate: bigint, accountBytes: number): bigint {
  if (fundedRentRate <= 0n) throw new Error('a funded-rent rate is a nonzero lamports-per-byte figure');
  if (!Number.isInteger(accountBytes) || accountBytes < 0) throw new Error('an account width is a non-negative integer');
  return (ACCOUNT_STORAGE_OVERHEAD_BYTES_V1 + BigInt(accountBytes)) * fundedRentRate;
}

/**
 * Recover the rate a founding recorded from one account's funded minimum.
 *
 * `funded_rent_rate_from_minimum_v1` in the same Rust module, including its
 * refusal: a minimum this cannot reproduce exactly is a minimum that was not
 * priced at any single rate, and it is refused rather than rounded. The
 * founding surface uses it against the cluster's own zero-length reading,
 * which is exactly `derive_funded_rent_rate_v2`'s first step.
 */
export function fundedRentRateFromMinimumV1(minimum: bigint, accountBytes: number): bigint {
  const span = ACCOUNT_STORAGE_OVERHEAD_BYTES_V1 + BigInt(accountBytes);
  const rate = minimum / span;
  if (rate <= 0n || fundedRentMinimumV1(rate, accountBytes) !== minimum) {
    throw new Error(`no single lamports-per-byte rate prices ${minimum} lamports for ${accountBytes} bytes`);
  }
  return rate;
}

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

/**
 * The same plan, priced the way the protocol prices: at ONE recorded rate.
 *
 * The difference from [`openerFirstCrankV1`] is not arithmetic, it is
 * provenance. That function takes whatever the caller read for each width,
 * which is right for a market that does not exist yet only if the cluster's
 * rent is affine and stable across the four reads. This one takes the single
 * `u32` a founding persists in its capability funding ledger
 * (`CAPABILITY_FUNDING_LEDGER_FUNDED_RENT_RATE_OFFSET_V2`) and prices every
 * width through it, so the figure a terms surface states about an existing
 * market is the figure that market's own accounts actually hold -- devnet moved
 * 6,333 to 5,080 lamports a byte inside cohort-15, and a page pricing a
 * cohort-15 market at today's rate is a fifth wrong about a number a founder is
 * being asked to accept.
 */
export function openerFirstCrankAtFundedRateV1(input: Readonly<{
  outcomeCount: number;
  /** The `u32` a founding recorded, in lamports per byte. */
  fundedRentRate: bigint;
  crankRewardCapLamports?: bigint;
}>): OpenerFirstCrankV1 {
  return openerFirstCrankV1({
    outcomeCount: input.outcomeCount,
    rentFor: (bytes: number) => fundedRentMinimumV1(input.fundedRentRate, bytes),
    crankRewardCapLamports: input.crankRewardCapLamports,
  });
}

/** Lamports as SOL, exactly, with no float anywhere in the path. */
export function lamportsAsSolV1(lamports: bigint): string {
  const whole = lamports / 1_000_000_000n;
  const fraction = (lamports % 1_000_000_000n).toString().padStart(9, '0');
  return `${whole}.${fraction}`;
}
