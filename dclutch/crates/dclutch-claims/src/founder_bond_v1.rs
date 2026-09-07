//! The founder bond: posted at founding, returned on an honest terminal, walked
//! pro rata to the ordinary claims on an exhausted one.
//!
//! Decision 0033 makes the bond MANDATORY at the size rule. Decision 0025 seats
//! a refunding Market's failure coordinate in an escrow Position nobody holds a
//! key to; the bond is the lamport side of the fact that account already owns
//! -- the lamports the escrow Position holds above its own rent. Nothing here
//! is a typed number: the size is rent arithmetic over widths the tree already
//! owns, the bond is OBSERVED off the escrow's balance and the rent it recorded
//! at founding (decision 0030), and the draw is a function of what still
//! stands.
//!
//! This module is the Rust twin of
//! `formal/dclutch-semantics/DClutchSemantics/FounderBondV1.lean`: `bond_size_v1`
//! is `bondSize`, [`FounderBondExitV1`] is `Exit`, [`FounderBondDrawPlanV1`] is
//! `draw` and one `Walk.step`, and [`FounderBondClosureV1`] is the exhausted
//! close's admission. The Lean owns the theorems and the widths are its
//! PARAMETERS; this file owns the widths by naming the tree's own constants,
//! and its tests pin the Lean's decided witnesses to the lamport.
//!
//! In the claim-check idiom every movement is a plan whose `new()` refuses to
//! exist unless the movement balances, plus a `validate_post` against observed
//! post-balances. Arithmetic never appears inline in a route.

use crate::claim_check_v1::{
    CLAIM_CHECK_BYTES_V1, CLAIM_CHECK_ESCROW_BYTES_V1, COMPACTION_CRANK_REWARD_LAMPORTS_V1,
};
use crate::liability_basis_state_v2::{
    LIABILITY_BASIS_POSITION_HEADER_BYTES_V2, LiabilityBasisMarketViewV2,
};
use crate::protocol_position_v2::PROTOCOL_POSITION_ADMISSION_BYTES_V2;
use dclutch_market::capability_manifest::funding::funded_rent_minimum_v2;

/// The certificate seat the settle allocates: `settle_certificate_bytes`.
///
/// `RESOLUTION_CERTIFICATE_BYTES_V2` is Lean-emitted into `dclutch-source`;
/// it is named here through that crate rather than re-typed.
pub(crate) const FOUNDER_BOND_CERTIFICATE_SEAT_BYTES_V1: usize =
    dclutch_source::resolution::RESOLUTION_CERTIFICATE_BYTES_V2;

/// A Token-2022 base account, the vault a compaction opener funds beside the
/// escrow record.
pub(crate) const FOUNDER_BOND_TOKEN_ACCOUNT_BYTES_V1: usize = dclutch_custody::token_svm::state::ACCOUNT_BYTES;

/// Bytes one claim coordinate occupies in a `LiabilityBasisV2` Position.
///
/// The Position is its header plus eight bytes per outcome;
/// `liability_basis_vector_width_v2` is the author of that arithmetic and the
/// test `the_position_width_is_the_trees_own` holds this constant to it.
pub(crate) const FOUNDER_BOND_POSITION_BYTES_PER_OUTCOME_V1: usize = 8;

/// Stable refusal from the bond's size rule, draw or closure admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FounderBondErrorV1 {
    /// The recorded rent rate was zero or a rent product overflowed.
    Rent,
    /// Checked arithmetic overflowed.
    ArithmeticOverflow,
    /// The runtime width seats no failure coordinate, so there is no bond.
    Width,
    /// A redemption named more ordinary claims than are outstanding.
    ExceedsOutstanding,
    /// The exhausted-arm close ran while ordinary claims still stood: the walk
    /// that pays the bond has not finished.
    OrdinaryClaimsOutstanding,
    /// The aggregate's supply vector did not decode at the stated width.
    MarketState,
    /// Observed post-balances did not match the admitted plan.
    PostconditionMismatch,
}

/// Result alias for the founder bond.
pub type FounderBondResultV1<T> = core::result::Result<T, FounderBondErrorV1>;

/// The account widths the size rule reads. Parameters in the Lean; the tree's
/// own constants here, and never a second author of any of them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FounderBondWidthsV1 {
    /// The certificate seat the settle allocates.
    pub certificate_seat: usize,
    /// `CLAIM_CHECK_ESCROW_BYTES_V1`, the escrow record a compaction opener creates.
    pub claim_check_escrow: usize,
    /// A Token-2022 account, the vault the opener funds beside the escrow.
    pub token_account: usize,
    /// The admission record the first crank sweeps.
    pub admission: usize,
    /// `CLAIM_CHECK_BYTES_V1`, the record the first crank mints.
    pub claim_check: usize,
    /// Position header bytes; a Position is header plus `position_per_outcome`
    /// per outcome.
    pub position_header: usize,
    /// Bytes per outcome in a Position's balance vector.
    pub position_per_outcome: usize,
}

/// The widths as the tree owns them. Every field names the constant its owner
/// exports; nothing here is a number typed for this module.
pub(crate) const FOUNDER_BOND_WIDTHS_V1: FounderBondWidthsV1 = FounderBondWidthsV1 {
    certificate_seat: FOUNDER_BOND_CERTIFICATE_SEAT_BYTES_V1,
    claim_check_escrow: CLAIM_CHECK_ESCROW_BYTES_V1,
    token_account: FOUNDER_BOND_TOKEN_ACCOUNT_BYTES_V1,
    admission: PROTOCOL_POSITION_ADMISSION_BYTES_V2,
    claim_check: CLAIM_CHECK_BYTES_V1,
    position_header: LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
    position_per_outcome: FOUNDER_BOND_POSITION_BYTES_PER_OUTCOME_V1,
};

/// Rent-exempt minimum for `bytes` of account data at the exemption-scaled
/// `rate` (decision 0030's persisted fact): `(128 + bytes) * rate`.
///
/// `funded_rent_minimum_v2` is the author; this is its name inside the bond.
pub(crate) fn rent_for_v1(rate: u32, bytes: usize) -> FounderBondResultV1<u64> {
    funded_rent_minimum_v2(rate, bytes).map_err(|_| FounderBondErrorV1::Rent)
}

/// Every input the size rule reads. All four are the founding's own: the rate
/// the founding is creating accounts at, the Market's outcome count, the
/// crank reward cap, and the ladder's rung quotes summed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FounderBondSizeInputV1 {
    /// Exemption-scaled lamports per byte the founding funds accounts at. A
    /// CREATING site reads it off the sysvar and records what it paid; every
    /// later reader recovers it from the escrow admission's recorded principal
    /// (`funded_rent_rate_from_minimum_v1`), never from the sysvar of the
    /// moment.
    pub rate: u32,
    /// The Market's outcome count, ordinary and failure together.
    pub outcomes: u32,
    /// `COMPACTION_CRANK_REWARD_LAMPORTS_V1`.
    pub crank_reward_cap: u64,
    /// The ladder's funding: each rung's Bounty quote, summed. A Market with no
    /// recovery policy has none. See `BUILD_founder-bond.md` for who supplies
    /// it and the provisional floor the chain enforces without it.
    pub ladder_funding: u64,
}

/// The size rule's three terms and their sum.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FounderBondSizeV1 {
    /// `S`: the certificate seat's rent.
    pub seat_prepay: u64,
    /// `F`: the compaction opener's advance less what the first crank repays.
    pub first_crank_shortfall: u64,
    /// `Λ`: the ladder's funding.
    pub ladder_funding: u64,
    /// `B = S + F + Λ`.
    pub bond: u64,
}

/// The seat prepay: the certificate seat's rent, a founding-time caller
/// obligation the terminal consumes and nothing reimburses.
pub(crate) fn seat_prepay_v1(widths: FounderBondWidthsV1, rate: u32) -> FounderBondResultV1<u64> {
    rent_for_v1(rate, widths.certificate_seat)
}

/// What a compaction opener advances: the escrow record and its vault.
pub(crate) fn opener_advance_v1(widths: FounderBondWidthsV1, rate: u32) -> FounderBondResultV1<u64> {
    rent_for_v1(rate, widths.claim_check_escrow)?
        .checked_add(rent_for_v1(rate, widths.token_account)?)
        .ok_or(FounderBondErrorV1::ArithmeticOverflow)
}

/// What the first crank sweeps: the Position and the admission record.
pub(crate) fn first_crank_swept_v1(
    widths: FounderBondWidthsV1,
    rate: u32,
    outcomes: u32,
) -> FounderBondResultV1<u64> {
    let per_outcome = widths
        .position_per_outcome
        .checked_mul(usize::try_from(outcomes).map_err(|_| FounderBondErrorV1::ArithmeticOverflow)?)
        .ok_or(FounderBondErrorV1::ArithmeticOverflow)?;
    let position = widths
        .position_header
        .checked_add(per_outcome)
        .ok_or(FounderBondErrorV1::ArithmeticOverflow)?;
    rent_for_v1(rate, position)?
        .checked_add(rent_for_v1(rate, widths.admission)?)
        .ok_or(FounderBondErrorV1::ArithmeticOverflow)
}

/// What the first crank repays the opener in the crank-first order decision
/// 0024 item 3 keeps: the sweep, less the claim check's own rent, less the
/// cranker's capped reward. Truncated subtraction is the kernel's `min`.
pub(crate) fn first_crank_repayment_v1(
    widths: FounderBondWidthsV1,
    rate: u32,
    outcomes: u32,
    crank_reward_cap: u64,
) -> FounderBondResultV1<u64> {
    Ok(first_crank_swept_v1(widths, rate, outcomes)?
        .saturating_sub(rent_for_v1(rate, widths.claim_check)?)
        .saturating_sub(crank_reward_cap))
}

/// The opener's shortfall on a single-crank market: 1,244,945 lamports on the
/// cohorts' rate, the figure the economics note measures.
pub(crate) fn first_crank_shortfall_v1(
    widths: FounderBondWidthsV1,
    rate: u32,
    outcomes: u32,
    crank_reward_cap: u64,
) -> FounderBondResultV1<u64> {
    Ok(opener_advance_v1(widths, rate)?
        .saturating_sub(first_crank_repayment_v1(widths, rate, outcomes, crank_reward_cap)?))
}

/// **The size rule.** `B = S + F + Λ`: the seat prepay plus the first crank's
/// shortfall plus the ladder's funding -- the cost of the terminal the
/// founder's oracle, if it goes quiet, makes the holders walk.
pub(crate) fn bond_size_v1(
    widths: FounderBondWidthsV1,
    input: FounderBondSizeInputV1,
) -> FounderBondResultV1<FounderBondSizeV1> {
    let seat_prepay = seat_prepay_v1(widths, input.rate)?;
    let first_crank_shortfall =
        first_crank_shortfall_v1(widths, input.rate, input.outcomes, input.crank_reward_cap)?;
    let bond = seat_prepay
        .checked_add(first_crank_shortfall)
        .and_then(|value| value.checked_add(input.ladder_funding))
        .ok_or(FounderBondErrorV1::ArithmeticOverflow)?;
    Ok(FounderBondSizeV1 {
        seat_prepay,
        first_crank_shortfall,
        ladder_funding: input.ladder_funding,
        bond,
    })
}

/// The size rule at the tree's own widths and crank cap, for a founding that
/// knows only its rate, its width and its ladder.
pub fn founding_bond_size_v1(
    rate: u32,
    outcomes: u32,
    ladder_funding: u64,
) -> FounderBondResultV1<FounderBondSizeV1> {
    bond_size_v1(
        FOUNDER_BOND_WIDTHS_V1,
        FounderBondSizeInputV1 {
            rate,
            outcomes,
            crank_reward_cap: COMPACTION_CRANK_REWARD_LAMPORTS_V1,
            ladder_funding,
        },
    )
}

/// What every later route reads as the bond: the escrow account's lamports
/// above the rent it recorded at founding. An observation, never a caller's
/// number; a lamport somebody donates enlarges it rather than stranding.
pub const fn observed_bond_v1(escrow_lamports: u64, recorded_rent: u64) -> u64 {
    escrow_lamports.saturating_sub(recorded_rent)
}

/// The founding conjunct: the escrow account holds its rent and the bond.
pub const fn founded_v1(escrow_lamports: u64, recorded_rent: u64, bond: u64) -> bool {
    match recorded_rent.checked_add(bond) {
        Some(required) => required <= escrow_lamports,
        None => false,
    }
}

/// Which exit the bond leaves by. The certificate is written once and its kind
/// decides: an ordinary winner returns the bond, the failure selector walks it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FounderBondExitV1 {
    /// The certificate names an ordinary winner: returned to the founder's
    /// refund source, in full, at closure.
    Honest,
    /// The certificate names the failure selector: walked pro rata to the
    /// ordinary claims in the redemptions that pay the escrow refund.
    Exhausted,
}

/// The exit a terminal enables, or `None` on a Market that posted no bond.
///
/// Only a refunding Market seats an escrow and therefore holds a bond; the
/// exit is a function of the winner alone (`exit?` in the Lean).
pub const fn exit_v1(
    refunds_on_failure: bool,
    terminal_winner: u32,
    failure_selector: u32,
) -> Option<FounderBondExitV1> {
    if !refunds_on_failure {
        return None;
    }
    if terminal_winner == failure_selector {
        Some(FounderBondExitV1::Exhausted)
    } else {
        Some(FounderBondExitV1::Honest)
    }
}

/// The ordinary claims still outstanding on the aggregate: the sum of every
/// supply coordinate but the failure selector's. An observation of the
/// aggregate, never a caller's number.
pub fn ordinary_outstanding_v1(
    market: LiabilityBasisMarketViewV2,
    market_bytes: &[u8],
    failure_selector: u32,
) -> FounderBondResultV1<u64> {
    if failure_selector >= market.claim_count {
        return Err(FounderBondErrorV1::Width);
    }
    let mut outstanding = 0_u64;
    let mut index = 0_u32;
    while index < market.claim_count {
        if index != failure_selector {
            let supply = market
                .supply(market_bytes, index)
                .map_err(|_| FounderBondErrorV1::MarketState)?;
            outstanding = outstanding
                .checked_add(supply)
                .ok_or(FounderBondErrorV1::ArithmeticOverflow)?;
        }
        index = index
            .checked_add(1)
            .ok_or(FounderBondErrorV1::ArithmeticOverflow)?;
    }
    Ok(outstanding)
}

/// One redemption's draw: the bond still standing, times the share of the
/// claims still outstanding that this redemption retires, floored. The floor
/// is the one named rounding boundary and it is bounded at one lamport per
/// redemption (`draw_within_one_lamport_of_the_exact_share`).
pub(crate) fn draw_v1(remaining: u64, outstanding: u64, quantity: u64) -> FounderBondResultV1<u64> {
    if quantity == 0 {
        return Ok(0);
    }
    if quantity > outstanding {
        return Err(FounderBondErrorV1::ExceedsOutstanding);
    }
    let product = u128::from(remaining)
        .checked_mul(u128::from(quantity))
        .ok_or(FounderBondErrorV1::ArithmeticOverflow)?;
    let share = product
        .checked_div(u128::from(outstanding))
        .ok_or(FounderBondErrorV1::ArithmeticOverflow)?;
    u64::try_from(share).map_err(|_| FounderBondErrorV1::ArithmeticOverflow)
}

/// Everything one terminal redemption observed about the bond before it moves
/// a lamport.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FounderBondDrawObservationV1 {
    /// The exit the certificate enables.
    pub exit: FounderBondExitV1,
    /// The escrow Position's live lamports, dust and donations included.
    pub escrow_lamports: u64,
    /// The rent the escrow Position recorded at founding, read off its
    /// admission record.
    pub recorded_rent: u64,
    /// The ordinary claims outstanding on the aggregate BEFORE this
    /// redemption's debit.
    pub ordinary_outstanding: u64,
    /// The coordinate this redemption retires.
    pub claim_index: u32,
    /// The failure selector at this width.
    pub failure_selector: u32,
    /// The claims this redemption retires at `claim_index`.
    pub quantity: u64,
    /// The bond recipient's live lamports before the draw.
    pub recipient_lamports: u64,
}

/// The sole admitted bond movement for one redemption: `Walk.step`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FounderBondDrawPlanV1 {
    remaining_before: u64,
    draw: u64,
    escrow_after: u64,
    recipient_after: u64,
}

impl FounderBondDrawPlanV1 {
    /// Build the sole admitted draw, or refuse to exist.
    ///
    /// The honest exit draws nothing (`no_redemption_draws_the_bond_on_an_honest_terminal`);
    /// the failure coordinate draws nothing (`the_failure_coordinate_draws_nothing`);
    /// an ordinary redemption under the exhausted exit draws its pro-rata share
    /// (`an_ordinary_redemption_draws_its_share`), and the redemption that
    /// retires the last ordinary claim draws everything left
    /// (`the_last_redemption_draws_everything`).
    pub fn new(observation: FounderBondDrawObservationV1) -> FounderBondResultV1<Self> {
        let remaining_before =
            observed_bond_v1(observation.escrow_lamports, observation.recorded_rent);
        let ordinary_quantity = if observation.claim_index == observation.failure_selector {
            0
        } else {
            observation.quantity
        };
        let draw = match observation.exit {
            FounderBondExitV1::Honest => 0,
            FounderBondExitV1::Exhausted => draw_v1(
                remaining_before,
                observation.ordinary_outstanding,
                ordinary_quantity,
            )?,
        };
        // `draw <= remaining <= escrow_lamports`, so neither subtraction can
        // fail; they are checked anyway because a plan that could not balance
        // must not exist.
        let escrow_after = observation
            .escrow_lamports
            .checked_sub(draw)
            .ok_or(FounderBondErrorV1::ArithmeticOverflow)?;
        let recipient_after = observation
            .recipient_lamports
            .checked_add(draw)
            .ok_or(FounderBondErrorV1::ArithmeticOverflow)?;
        Ok(Self {
            remaining_before,
            draw,
            escrow_after,
            recipient_after,
        })
    }

    /// The bond standing before this redemption.
    pub const fn remaining_before(self) -> u64 {
        self.remaining_before
    }

    /// Lamports this redemption moves from the escrow to the recipient.
    pub const fn draw(self) -> u64 {
        self.draw
    }

    /// The bond standing after this redemption.
    pub const fn remaining_after(self) -> u64 {
        self.remaining_before.saturating_sub(self.draw)
    }

    /// The escrow Position's lamports after the draw.
    pub const fn escrow_after(self) -> u64 {
        self.escrow_after
    }

    /// The recipient's lamports after the draw.
    pub const fn recipient_after(self) -> u64 {
        self.recipient_after
    }

    /// Require the observed post-balances to be exactly the admitted ones.
    pub fn validate_post(
        self,
        escrow_lamports: u64,
        recipient_lamports: u64,
    ) -> FounderBondResultV1<()> {
        if escrow_lamports != self.escrow_after || recipient_lamports != self.recipient_after {
            return Err(FounderBondErrorV1::PostconditionMismatch);
        }
        Ok(())
    }
}

/// What the closure observed about the bond before it disposes of the escrow.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FounderBondClosureObservationV1 {
    /// The exit the certificate enables.
    pub exit: FounderBondExitV1,
    /// The escrow Position's live lamports.
    pub escrow_lamports: u64,
    /// The rent the escrow Position recorded at founding.
    pub recorded_rent: u64,
    /// The ordinary claims still outstanding on the aggregate.
    pub ordinary_outstanding: u64,
}

/// The closure's disposition of the bond: exactly one of the two exits, never
/// both, never partially (`the_bond_leaves_by_exactly_one_exit`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FounderBondClosureV1 {
    exit: FounderBondExitV1,
    rent: u64,
    returned_to_founder: u64,
    surplus_to_refund_wallet: u64,
}

impl FounderBondClosureV1 {
    /// Admit the closure's bond arm, or refuse to exist.
    ///
    /// On the honest exit everything above rent returns to the founder's refund
    /// source with the rent. On the exhausted exit the close refuses while
    /// ordinary claims stand -- the walk that pays the bond has not finished --
    /// and once none stand, whatever remains above rent is a donation that
    /// arrived after the last redemption: surplus to the refund wallet, named.
    pub fn new(observation: FounderBondClosureObservationV1) -> FounderBondResultV1<Self> {
        let remaining = observed_bond_v1(observation.escrow_lamports, observation.recorded_rent);
        let rent = observation.escrow_lamports.saturating_sub(remaining);
        match observation.exit {
            FounderBondExitV1::Honest => Ok(Self {
                exit: observation.exit,
                rent,
                returned_to_founder: remaining,
                surplus_to_refund_wallet: 0,
            }),
            FounderBondExitV1::Exhausted => {
                if observation.ordinary_outstanding != 0 {
                    return Err(FounderBondErrorV1::OrdinaryClaimsOutstanding);
                }
                Ok(Self {
                    exit: observation.exit,
                    rent,
                    returned_to_founder: 0,
                    surplus_to_refund_wallet: remaining,
                })
            }
        }
    }

    /// The exit this closure runs under.
    pub const fn exit(self) -> FounderBondExitV1 {
        self.exit
    }

    /// The escrow's own rent, returned on both exits.
    pub const fn rent(self) -> u64 {
        self.rent
    }

    /// The bond returned to the founder's refund source: the whole of it on the
    /// honest exit, nothing on the exhausted one.
    pub const fn returned_to_founder(self) -> u64 {
        self.returned_to_founder
    }

    /// Lamports above rent that reach the refund wallet on the exhausted exit:
    /// zero unless somebody donated after the walk finished.
    pub const fn surplus_to_refund_wallet(self) -> u64 {
        self.surplus_to_refund_wallet
    }

    /// Everything the escrow Position surrenders to the aggregate at closure.
    pub const fn surrendered(self) -> u64 {
        self.rent + self.returned_to_founder + self.surplus_to_refund_wallet
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::liability_basis_state_v2::{
        LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LiabilityBasisMarketInputV2,
        encode_liability_basis_market_into_v2, liability_basis_vector_width_v2,
    };
    use std::vec;
    use std::vec::Vec;

    /// Cohort-15's founding rate, devnet after epoch 1141, and the kernel's
    /// reference: the three rates the economics note prices everything at.
    const COHORT_FIFTEEN_RATE: u32 = 6_333;
    const EPOCH_1141_RATE: u32 = 5_080;
    const KERNEL_REFERENCE_RATE: u32 = 6_960;
    /// Cohort-15's market: three ordinary outcomes and the failure selector.
    const COHORT_FIFTEEN_OUTCOMES: u32 = 4;
    /// Its bond, decided in Lean as `cohort_fifteen_bond`.
    const COHORT_FIFTEEN_BOND: u64 = 4_031_465;
    /// Its ordinary claims: three coordinates at 500,000,000.
    const COHORT_FIFTEEN_OUTSTANDING: u64 = 3 * 500_000_000;

    fn cohort_fifteen(rate: u32) -> FounderBondSizeV1 {
        founding_bond_size_v1(rate, COHORT_FIFTEEN_OUTCOMES, 0).expect("size rule")
    }

    /// The widths the Lean's `cohortWidths` instantiates are the tree's.
    #[test]
    fn the_widths_are_the_trees_own() {
        assert_eq!(
            FOUNDER_BOND_WIDTHS_V1,
            FounderBondWidthsV1 {
                certificate_seat: 312,
                claim_check_escrow: 256,
                token_account: 165,
                admission: 512,
                claim_check: 288,
                position_header: 128,
                position_per_outcome: 8,
            }
        );
        assert_eq!(COMPACTION_CRANK_REWARD_LAMPORTS_V1, 200_000);
    }

    /// `position_header + 8n` is what `liability_basis_vector_width_v2` says a
    /// Position is; this module does not author it twice.
    #[test]
    fn the_position_width_is_the_trees_own() {
        for outcomes in [2_u32, 3, 4, 7, 16] {
            let width = liability_basis_vector_width_v2(
                LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
                outcomes,
            )
            .expect("position width");
            assert_eq!(
                width,
                FOUNDER_BOND_WIDTHS_V1.position_header
                    + FOUNDER_BOND_WIDTHS_V1.position_per_outcome * outcomes as usize
            );
        }
    }

    /// The Lean's decided witnesses, to the lamport.
    #[test]
    fn the_size_rule_reproduces_the_lean_witnesses() {
        let widths = FOUNDER_BOND_WIDTHS_V1;
        assert_eq!(seat_prepay_v1(widths, COHORT_FIFTEEN_RATE), Ok(2_786_520));
        assert_eq!(opener_advance_v1(widths, COHORT_FIFTEEN_RATE), Ok(4_287_441));
        assert_eq!(
            first_crank_repayment_v1(
                widths,
                COHORT_FIFTEEN_RATE,
                COHORT_FIFTEEN_OUTCOMES,
                COMPACTION_CRANK_REWARD_LAMPORTS_V1
            ),
            Ok(3_042_496)
        );
        assert_eq!(
            first_crank_shortfall_v1(
                widths,
                COHORT_FIFTEEN_RATE,
                COHORT_FIFTEEN_OUTCOMES,
                COMPACTION_CRANK_REWARD_LAMPORTS_V1
            ),
            Ok(1_244_945)
        );
        assert_eq!(cohort_fifteen(COHORT_FIFTEEN_RATE).bond, COHORT_FIFTEEN_BOND);
        assert_eq!(cohort_fifteen(EPOCH_1141_RATE).bond, 3_273_400);
        assert_eq!(cohort_fifteen(KERNEL_REFERENCE_RATE).bond, 4_410_800);
    }

    /// `a_rung_never_lowers_the_bond`, and the one-rung figure decision 0033
    /// section 6 names: the 0024 floor on a 376-byte receipt at 6,333.
    #[test]
    fn a_rung_never_lowers_the_bond() {
        let rung = rent_for_v1(COHORT_FIFTEEN_RATE, 376).expect("rung floor");
        assert_eq!(rung, 3_191_832);
        let one = founding_bond_size_v1(COHORT_FIFTEEN_RATE, COHORT_FIFTEEN_OUTCOMES, rung)
            .expect("one rung");
        let two = founding_bond_size_v1(COHORT_FIFTEEN_RATE, COHORT_FIFTEEN_OUTCOMES, 2 * rung)
            .expect("two rungs");
        assert_eq!(one.bond, 7_223_297);
        assert_eq!(two.bond, 10_415_129);
        assert!(cohort_fifteen(COHORT_FIFTEEN_RATE).bond <= one.bond && one.bond <= two.bond);
    }

    /// `a_founding_one_lamport_short_refuses`.
    #[test]
    fn a_founding_one_lamport_short_refuses() {
        let rent = rent_for_v1(COHORT_FIFTEEN_RATE, 160).expect("rent");
        assert!(founded_v1(rent + COHORT_FIFTEEN_BOND, rent, COHORT_FIFTEEN_BOND));
        assert!(!founded_v1(rent + COHORT_FIFTEEN_BOND - 1, rent, COHORT_FIFTEEN_BOND));
        assert!(!founded_v1(u64::MAX, u64::MAX, 1));
        assert_eq!(observed_bond_v1(rent + COHORT_FIFTEEN_BOND, rent), COHORT_FIFTEEN_BOND);
        assert_eq!(observed_bond_v1(rent - 1, rent), 0);
    }

    #[test]
    fn a_zero_rate_refuses_by_name() {
        assert_eq!(
            founding_bond_size_v1(0, COHORT_FIFTEEN_OUTCOMES, 0),
            Err(FounderBondErrorV1::Rent)
        );
    }

    /// The exit is the winner's alone, and a Market without an escrow has none.
    #[test]
    fn the_exit_is_a_function_of_the_winner() {
        assert_eq!(exit_v1(true, 3, 3), Some(FounderBondExitV1::Exhausted));
        assert_eq!(exit_v1(true, 1, 3), Some(FounderBondExitV1::Honest));
        assert_eq!(exit_v1(false, 3, 3), None);
        assert_eq!(exit_v1(false, 0, 3), None);
    }

    fn walk(exit: FounderBondExitV1, redemptions: &[u64]) -> (Vec<u64>, u64) {
        let rent = 1_000_000;
        let mut escrow = rent + COHORT_FIFTEEN_BOND;
        let mut outstanding = COHORT_FIFTEEN_OUTSTANDING;
        let mut draws = Vec::new();
        for quantity in redemptions {
            let plan = FounderBondDrawPlanV1::new(FounderBondDrawObservationV1 {
                exit,
                escrow_lamports: escrow,
                recorded_rent: rent,
                ordinary_outstanding: outstanding,
                claim_index: 0,
                failure_selector: 3,
                quantity: *quantity,
                recipient_lamports: 7,
            })
            .expect("feasible redemption");
            plan.validate_post(plan.escrow_after(), plan.recipient_after())
                .expect("the plan's own post-state");
            assert_eq!(plan.recipient_after(), 7 + plan.draw());
            draws.push(plan.draw());
            escrow = plan.escrow_after();
            outstanding -= quantity;
        }
        (draws, escrow - rent)
    }

    /// Cohort-13's own measured table redeemed in both orders on cohort-15's
    /// bond: the Lean's `cohortFifteenWalk` witnesses.
    #[test]
    fn the_cohort_thirteen_table_walks_exactly_in_both_orders() {
        let (draws, remaining) = walk(FounderBondExitV1::Exhausted, &[200, 1_499_999_800]);
        assert_eq!(draws, vec![0, COHORT_FIFTEEN_BOND]);
        assert_eq!(remaining, 0);
        let (draws, remaining) = walk(FounderBondExitV1::Exhausted, &[1_499_999_800, 200]);
        assert_eq!(draws, vec![4_031_464, 1]);
        assert_eq!(remaining, 0);
    }

    /// `an_exhausting_walk_pays_the_bond_exactly`: any partition, any order,
    /// no remainder.
    #[test]
    fn any_partition_of_the_outstanding_claims_pays_the_bond_exactly() {
        let partitions: [&[u64]; 5] = [
            &[COHORT_FIFTEEN_OUTSTANDING],
            &[500_000_000, 500_000_000, 500_000_000],
            &[1, 1, 1, COHORT_FIFTEEN_OUTSTANDING - 3],
            &[123_456_789, 987_654_321, 388_888_890],
            &[750_000_000, 750_000_000],
        ];
        for partition in partitions {
            assert_eq!(partition.iter().sum::<u64>(), COHORT_FIFTEEN_OUTSTANDING);
            let (draws, remaining) = walk(FounderBondExitV1::Exhausted, partition);
            assert_eq!(draws.iter().sum::<u64>(), COHORT_FIFTEEN_BOND, "{partition:?}");
            assert_eq!(remaining, 0, "{partition:?}");
        }
        // Half the claims draw half the bond, to the lamport the floor allows.
        assert_eq!(
            draw_v1(COHORT_FIFTEEN_BOND, 1_500_000_000, 750_000_000),
            Ok(2_015_732)
        );
    }

    /// Hostile: a bond paid on an honest resolution. Every redemption draws zero.
    #[test]
    fn no_redemption_draws_the_bond_on_an_honest_terminal() {
        let (draws, remaining) = walk(FounderBondExitV1::Honest, &[1_499_999_800, 200]);
        assert_eq!(draws, vec![0, 0]);
        assert_eq!(remaining, COHORT_FIFTEEN_BOND);
    }

    /// Hostile: the escrow's own claims drawing on the failure arm.
    #[test]
    fn the_failure_coordinate_draws_nothing() {
        let plan = FounderBondDrawPlanV1::new(FounderBondDrawObservationV1 {
            exit: FounderBondExitV1::Exhausted,
            escrow_lamports: 1_000_000 + COHORT_FIFTEEN_BOND,
            recorded_rent: 1_000_000,
            ordinary_outstanding: COHORT_FIFTEEN_OUTSTANDING,
            claim_index: 3,
            failure_selector: 3,
            quantity: 500_000_000,
            recipient_lamports: 0,
        })
        .expect("the failure coordinate's redemption is admitted and draws nothing");
        assert_eq!(plan.draw(), 0);
        assert_eq!(plan.remaining_after(), COHORT_FIFTEEN_BOND);
    }

    /// Hostile: a payout exceeding the bond. A quantity the aggregate could
    /// not have admitted is refused by name rather than overdrawn.
    #[test]
    fn a_redemption_beyond_the_outstanding_claims_refuses() {
        assert_eq!(
            draw_v1(COHORT_FIFTEEN_BOND, 100, 101),
            Err(FounderBondErrorV1::ExceedsOutstanding)
        );
        assert_eq!(draw_v1(COHORT_FIFTEEN_BOND, 0, 1), Err(FounderBondErrorV1::ExceedsOutstanding));
        assert_eq!(draw_v1(COHORT_FIFTEEN_BOND, 0, 0), Ok(0));
        // The draw is bounded by what remains for every admitted quantity.
        for quantity in [1_u64, 200, 500_000_000, COHORT_FIFTEEN_OUTSTANDING] {
            assert!(
                draw_v1(COHORT_FIFTEEN_BOND, COHORT_FIFTEEN_OUTSTANDING, quantity).expect("draw")
                    <= COHORT_FIFTEEN_BOND
            );
        }
    }

    #[test]
    fn the_postcondition_refuses_a_lamport_astray() {
        let plan = FounderBondDrawPlanV1::new(FounderBondDrawObservationV1 {
            exit: FounderBondExitV1::Exhausted,
            escrow_lamports: 1_000_000 + COHORT_FIFTEEN_BOND,
            recorded_rent: 1_000_000,
            ordinary_outstanding: COHORT_FIFTEEN_OUTSTANDING,
            claim_index: 1,
            failure_selector: 3,
            quantity: 500_000_000,
            recipient_lamports: 0,
        })
        .expect("plan");
        assert_eq!(plan.draw(), 1_343_821);
        assert_eq!(
            plan.validate_post(plan.escrow_after() + 1, plan.recipient_after()),
            Err(FounderBondErrorV1::PostconditionMismatch)
        );
        assert_eq!(
            plan.validate_post(plan.escrow_after(), plan.recipient_after() - 1),
            Err(FounderBondErrorV1::PostconditionMismatch)
        );
    }

    /// The closure: the honest exit returns everything above rent; the
    /// exhausted exit refuses while claims stand and names a late donation as
    /// surplus once they do not.
    #[test]
    fn the_closure_disposes_of_the_bond_by_exactly_one_exit() {
        let honest = FounderBondClosureV1::new(FounderBondClosureObservationV1 {
            exit: FounderBondExitV1::Honest,
            escrow_lamports: 1_000_000 + COHORT_FIFTEEN_BOND,
            recorded_rent: 1_000_000,
            ordinary_outstanding: COHORT_FIFTEEN_OUTSTANDING,
        })
        .expect("honest close");
        assert_eq!(honest.returned_to_founder(), COHORT_FIFTEEN_BOND);
        assert_eq!(honest.surplus_to_refund_wallet(), 0);
        assert_eq!(honest.rent(), 1_000_000);
        assert_eq!(honest.surrendered(), 1_000_000 + COHORT_FIFTEEN_BOND);

        assert_eq!(
            FounderBondClosureV1::new(FounderBondClosureObservationV1 {
                exit: FounderBondExitV1::Exhausted,
                escrow_lamports: 1_000_000 + COHORT_FIFTEEN_BOND,
                recorded_rent: 1_000_000,
                ordinary_outstanding: 1,
            }),
            Err(FounderBondErrorV1::OrdinaryClaimsOutstanding)
        );
        let exhausted = FounderBondClosureV1::new(FounderBondClosureObservationV1 {
            exit: FounderBondExitV1::Exhausted,
            escrow_lamports: 1_000_000 + 5,
            recorded_rent: 1_000_000,
            ordinary_outstanding: 0,
        })
        .expect("exhausted close after the walk");
        assert_eq!(exhausted.returned_to_founder(), 0);
        assert_eq!(exhausted.surplus_to_refund_wallet(), 5);
        assert_eq!(exhausted.surrendered(), 1_000_005);
    }

    /// `ordinary_outstanding_v1` sums every coordinate but the failure
    /// selector's, off the aggregate's own bytes.
    #[test]
    fn the_outstanding_claims_are_read_off_the_aggregate() {
        let width = liability_basis_vector_width_v2(LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, 4)
            .expect("aggregate width");
        let mut bytes = vec![0_u8; width];
        encode_liability_basis_market_into_v2(
            LiabilityBasisMarketInputV2 {
                revision: 1,
                logical_market: [1; 32],
                release_set: [2; 32],
                registry_program: [3; 32],
                product_instance_id: [4; 32],
                basis_id: [5; 32],
                realm_id: [6; 32],
                custody_context: [7; 32],
                generation: 1,
            },
            &[500_000_000, 400_000_000, 300_000_000, 166_666_667],
            &mut bytes,
        )
        .expect("aggregate");
        let view = LiabilityBasisMarketViewV2::decode(&bytes).expect("view");
        assert_eq!(ordinary_outstanding_v1(view, &bytes, 3), Ok(1_200_000_000));
        assert_eq!(
            ordinary_outstanding_v1(view, &bytes, 4),
            Err(FounderBondErrorV1::Width)
        );
    }
}
