//! Conservation for claim-check compaction and redemption.
//!
//! Three ledgers must close and each is stated as a plan whose `new()` refuses
//! to exist unless the movement balances, plus a `validate_post` against
//! observed post-balances. The program supplies observations and checks the
//! postcondition; arithmetic never appears inline in a route.
//!
//! **Every input is an observation, never a caller declaration.** A route that
//! compared a live PDA's balance against a number the caller supplied would be
//! blockable by anyone willing to send it one lamport in the slot before the
//! crank lands -- the same defect that makes ordinary position close a
//! retirement hostage today. Here dust is absorbed: an extra lamport at the
//! position simply enlarges the swept total and flows to the rent credit, and
//! an extra lamport already sitting at the vacant claim-check address reduces
//! the top-up the sweep owes it. Nothing compares for equality against a
//! prediction.
//!
//! The same discipline governs collateral. The vault credit is what the vault
//! was *observed* to gain, never what the transfer intended, because a
//! Token-2022 mint may carry a transfer-fee extension. Recording the
//! observation means the holder is promised exactly what is there to pay them.

use crate::claim_check_v1::ClaimCheckErrorV1;

/// Stable conservation refusal for a compaction or claim-check redemption.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClaimCheckConservationErrorV1 {
    /// A coordinate that must be nonzero was zero.
    ZeroCoordinate,
    /// Accounts required to be distinct named the same address.
    IdentityAlias,
    /// A movement did not balance against what the accounts held.
    Conservation,
    /// The lamports available could not fund a mandatory obligation.
    Uncapitalized,
    /// A claim-check was funded for an entitlement of zero atoms.
    EmptyClaimCheck,
    /// Checked arithmetic overflowed or underflowed.
    ArithmeticOverflow,
    /// Observed post-balances did not match the admitted plan.
    PostconditionMismatch,
}

/// Result alias for claim-check conservation.
pub type ClaimCheckConservationResultV1<T> = core::result::Result<T, ClaimCheckConservationErrorV1>;

impl From<ClaimCheckErrorV1> for ClaimCheckConservationErrorV1 {
    fn from(value: ClaimCheckErrorV1) -> Self {
        match value {
            ClaimCheckErrorV1::Arithmetic => Self::ArithmeticOverflow,
            ClaimCheckErrorV1::InvalidEntitlement => Self::EmptyClaimCheck,
            ClaimCheckErrorV1::InvalidIdentity => Self::IdentityAlias,
            _ => Self::Conservation,
        }
    }
}

/// One observed account: what it is, and what it currently holds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimCheckAccountObservationV1 {
    /// The account's address.
    pub identity: [u8; 32],
    /// Its lamports as read from the runtime, dust included.
    pub lamports: u64,
}

/// Everything one compaction observed, spanning the collateral transfer it has
/// already performed and preceding the closes it is about to perform.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimCheckCompactionObservationV1 {
    /// Atoms the terminal payout derivation instructed be moved.
    pub payout_atoms: u64,
    /// Hoard balance before the transfer.
    pub hoard_before: u64,
    /// Hoard balance after the transfer.
    pub hoard_after: u64,
    /// Escrow vault balance before the transfer.
    pub vault_before: u64,
    /// Escrow vault balance after the transfer.
    pub vault_after: u64,
    /// The position being compacted, and its live lamports.
    pub position: ClaimCheckAccountObservationV1,
    /// Its admission record, and its live lamports.
    pub admission: ClaimCheckAccountObservationV1,
    /// The claim-check address, and any lamports already sitting there.
    pub claim_check: ClaimCheckAccountObservationV1,
    /// The caller turning the crank.
    pub cranker: ClaimCheckAccountObservationV1,
    /// The party who opened the escrow, still owed their outlay.
    pub opener: ClaimCheckAccountObservationV1,
    /// The market's RentCredit, the residual beneficiary.
    pub rent_credit: ClaimCheckAccountObservationV1,
    /// Rent-exempt minimum for the claim-check width, read from the sysvar.
    ///
    /// Zero exactly when no record is minted. Chain-derived, never a caller's
    /// number: a bounty floor a caller may invent is a bounty a caller may
    /// inflate.
    pub claim_check_rent: u64,
    /// The opener's unrepaid outlay, as carried by the escrow record.
    pub opener_debt: u64,
    /// Ceiling on this crank's reward.
    pub crank_reward_cap: u64,
}

/// The sole admitted compaction movement: atoms and lamports, both closed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimCheckCompactionPlanV1 {
    entitlement_atoms: u64,
    hoard_after: u64,
    vault_after: u64,
    swept_lamports: u64,
    claim_check_top_up: u64,
    crank_reward: u64,
    opener_repayment: u64,
    rent_credit_residue: u64,
    opener_debt_after: u64,
    claim_check_after: u64,
    cranker_after: u64,
    opener_after: u64,
    rent_credit_after: u64,
}

impl ClaimCheckCompactionPlanV1 {
    /// Build the sole admitted compaction movement, or refuse to exist.
    ///
    /// The lamport order is deliberate and departs from the design's stated
    /// order, which paid the opener before the cranker. That order does not
    /// close arithmetically: for a binary market the position and admission
    /// together release about 6.35M lamports, the claim-check's own rent takes
    /// about 2.90M of it, and the opener advanced about 4.71M for the escrow
    /// record and the vault -- so the first crank would pay itself exactly
    /// nothing. An unfunded crank is an unturned crank, and that is the
    /// sleeping-holder deadlock coming back through the funding door, which is
    /// the failure the design names in the very same section.
    ///
    /// So the crank is paid first, the opener is repaid from what remains, and
    /// the debt carries in the escrow record until it is discharged. The opener
    /// is not disadvantaged: they are the party who wants to crank, they earn
    /// on every crank they turn, and the escrow's own close returns the escrow
    /// and vault rent -- exactly what they advanced -- to whoever closes it.
    pub fn new(
        observation: ClaimCheckCompactionObservationV1,
    ) -> ClaimCheckConservationResultV1<Self> {
        let entitlement_atoms = Self::settle_atoms(observation)?;

        // A record is funded exactly when there is a claim to record. Every
        // holder of a losing outcome resolves to zero atoms, so a route that
        // minted one anyway would pin the escrow's outstanding count above zero
        // forever with nobody holding any reason to redeem it.
        if (observation.claim_check_rent == 0) != (entitlement_atoms == 0) {
            return Err(ClaimCheckConservationErrorV1::EmptyClaimCheck);
        }

        Self::require_distinct_subjects(observation)?;

        let swept_lamports = observation
            .position
            .lamports
            .checked_add(observation.admission.lamports)
            .ok_or(ClaimCheckConservationErrorV1::ArithmeticOverflow)?;
        if swept_lamports == 0 {
            return Err(ClaimCheckConservationErrorV1::ZeroCoordinate);
        }

        // Dust already at the vacant address is not a refusal and not a
        // windfall: it counts against what the sweep owes the new record.
        let claim_check_top_up = observation
            .claim_check_rent
            .saturating_sub(observation.claim_check.lamports);
        let after_rent = swept_lamports
            .checked_sub(claim_check_top_up)
            .ok_or(ClaimCheckConservationErrorV1::Uncapitalized)?;

        // A cap, never a demand: a thin position yields a small reward rather
        // than a refusal.
        let crank_reward = observation.crank_reward_cap.min(after_rent);
        let after_reward = after_rent
            .checked_sub(crank_reward)
            .ok_or(ClaimCheckConservationErrorV1::ArithmeticOverflow)?;

        let opener_repayment = observation.opener_debt.min(after_reward);
        let rent_credit_residue = after_reward
            .checked_sub(opener_repayment)
            .ok_or(ClaimCheckConservationErrorV1::ArithmeticOverflow)?;

        // The conservation conjunct is the point: everything the two closing
        // accounts held is accounted for by exactly four credits, so the close
        // can neither strand a lamport in an account it is about to leave at
        // zero length nor pay out more than it held.
        let disbursed = claim_check_top_up
            .checked_add(crank_reward)
            .and_then(|value| value.checked_add(opener_repayment))
            .and_then(|value| value.checked_add(rent_credit_residue))
            .ok_or(ClaimCheckConservationErrorV1::ArithmeticOverflow)?;
        if disbursed != swept_lamports {
            return Err(ClaimCheckConservationErrorV1::Conservation);
        }

        let opener_debt_after = observation
            .opener_debt
            .checked_sub(opener_repayment)
            .ok_or(ClaimCheckConservationErrorV1::ArithmeticOverflow)?;

        // The three lamport sinks may legitimately be the same wallet -- the
        // opener is usually the cranker, which is precisely what makes opening
        // a funded position. Credits are therefore folded by identity, so an
        // aliased sink is expected to receive the sum rather than each credit
        // separately.
        let credits = [
            (observation.cranker.identity, crank_reward),
            (observation.opener.identity, opener_repayment),
            (observation.rent_credit.identity, rent_credit_residue),
        ];

        Ok(Self {
            entitlement_atoms,
            hoard_after: observation.hoard_after,
            vault_after: observation.vault_after,
            swept_lamports,
            claim_check_top_up,
            crank_reward,
            opener_repayment,
            rent_credit_residue,
            opener_debt_after,
            claim_check_after: observation
                .claim_check
                .lamports
                .checked_add(claim_check_top_up)
                .ok_or(ClaimCheckConservationErrorV1::ArithmeticOverflow)?,
            cranker_after: fold_credit(observation.cranker, &credits)?,
            opener_after: fold_credit(observation.opener, &credits)?,
            rent_credit_after: fold_credit(observation.rent_credit, &credits)?,
        })
    }

    /// Verify observed post-balances against this exact plan.
    ///
    /// The position's and admission's post-balances are not parameters: both
    /// are required to be zero, because a closed account keeps nothing and a
    /// residual there would be a fifth party to a four-way movement.
    pub fn validate_post(
        self,
        post: ClaimCheckCompactionPostV1,
    ) -> ClaimCheckConservationResultV1<()> {
        if post.position_lamports != 0
            || post.admission_lamports != 0
            || post.claim_check_lamports != self.claim_check_after
            || post.cranker_lamports != self.cranker_after
            || post.opener_lamports != self.opener_after
            || post.rent_credit_lamports != self.rent_credit_after
            || post.hoard_lamports_of_collateral != self.hoard_after
            || post.vault_lamports_of_collateral != self.vault_after
        {
            return Err(ClaimCheckConservationErrorV1::PostconditionMismatch);
        }
        Ok(())
    }

    /// Collateral atoms this claim-check may promise: the observed credit.
    #[must_use]
    pub const fn entitlement_atoms(self) -> u64 {
        self.entitlement_atoms
    }

    /// Total lamports released by the position and its admission record.
    #[must_use]
    pub const fn swept_lamports(self) -> u64 {
        self.swept_lamports
    }

    /// Lamports the sweep must add to reach the claim-check's rent floor.
    #[must_use]
    pub const fn claim_check_top_up(self) -> u64 {
        self.claim_check_top_up
    }

    /// Lamports this crank pays its caller.
    #[must_use]
    pub const fn crank_reward(self) -> u64 {
        self.crank_reward
    }

    /// Lamports this crank returns to the escrow's opener.
    #[must_use]
    pub const fn opener_repayment(self) -> u64 {
        self.opener_repayment
    }

    /// The opener's outlay still outstanding after this crank.
    #[must_use]
    pub const fn opener_debt_after(self) -> u64 {
        self.opener_debt_after
    }

    /// Lamports flowing to the market's RentCredit.
    #[must_use]
    pub const fn rent_credit_residue(self) -> u64 {
        self.rent_credit_residue
    }

    /// Whether this compaction mints a claim-check at all.
    #[must_use]
    pub const fn mints_claim_check(self) -> bool {
        self.entitlement_atoms != 0
    }

    fn settle_atoms(
        observation: ClaimCheckCompactionObservationV1,
    ) -> ClaimCheckConservationResultV1<u64> {
        // The Hoard is debited by exactly what the derivation instructed. This
        // is the one exact equality in the collateral ledger, and it is safe
        // because it is stated over two observations rather than against a
        // caller's prediction.
        let expected_hoard = observation
            .hoard_before
            .checked_sub(observation.payout_atoms)
            .ok_or(ClaimCheckConservationErrorV1::Uncapitalized)?;
        if observation.hoard_after != expected_hoard {
            return Err(ClaimCheckConservationErrorV1::Conservation);
        }

        // The vault's credit is observed, never assumed: a Token-2022 mint may
        // levy a transfer fee, in which case the vault gains less than was
        // sent, and the holder must be promised what is there rather than what
        // was intended.
        let credit = observation
            .vault_after
            .checked_sub(observation.vault_before)
            .ok_or(ClaimCheckConservationErrorV1::Conservation)?;
        // A vault that gained more than was sent it is holding somebody else's
        // collateral, and paying this claim-check out of it would be theft
        // from another holder.
        if credit > observation.payout_atoms {
            return Err(ClaimCheckConservationErrorV1::Conservation);
        }
        Ok(credit)
    }

    fn require_distinct_subjects(
        observation: ClaimCheckCompactionObservationV1,
    ) -> ClaimCheckConservationResultV1<()> {
        let subjects = [
            observation.position.identity,
            observation.admission.identity,
            observation.claim_check.identity,
        ];
        let sinks = [
            observation.cranker.identity,
            observation.opener.identity,
            observation.rent_credit.identity,
        ];
        for (index, subject) in subjects.iter().enumerate() {
            if subject.iter().all(|byte| *byte == 0) {
                return Err(ClaimCheckConservationErrorV1::ZeroCoordinate);
            }
            let rest = index
                .checked_add(1)
                .ok_or(ClaimCheckConservationErrorV1::ArithmeticOverflow)?;
            if subjects.iter().skip(rest).any(|other| other == subject) {
                return Err(ClaimCheckConservationErrorV1::IdentityAlias);
            }
            // A sink that is also an account being closed or created would be
            // credited and zeroed in the same movement, and the ledger would
            // close while a lamport went missing.
            if sinks.iter().any(|sink| sink == subject) {
                return Err(ClaimCheckConservationErrorV1::IdentityAlias);
            }
        }
        for sink in sinks {
            if sink.iter().all(|byte| *byte == 0) {
                return Err(ClaimCheckConservationErrorV1::ZeroCoordinate);
            }
        }
        Ok(())
    }
}

/// Observed balances after one compaction has closed its accounts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimCheckCompactionPostV1 {
    /// Position lamports; must be zero.
    pub position_lamports: u64,
    /// Admission-record lamports; must be zero.
    pub admission_lamports: u64,
    /// Claim-check lamports after the top-up.
    pub claim_check_lamports: u64,
    /// Cranker lamports after its credit.
    pub cranker_lamports: u64,
    /// Opener lamports after its repayment.
    pub opener_lamports: u64,
    /// RentCredit lamports after the residue.
    pub rent_credit_lamports: u64,
    /// Hoard collateral atoms after the transfer.
    pub hoard_lamports_of_collateral: u64,
    /// Escrow vault collateral atoms after the transfer.
    pub vault_lamports_of_collateral: u64,
}

/// Everything one claim-check redemption observed before it moved anything.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimCheckRedemptionObservationV1 {
    /// Atoms the record promises its holder.
    pub entitlement_atoms: u64,
    /// Escrow vault balance before the payout.
    pub vault_before: u64,
    /// The holder's token account balance before the payout.
    pub holder_tokens_before: u64,
    /// Live lamports at the claim-check record, dust included.
    pub record_lamports: u64,
    /// The holder's wallet lamports before the record closes into it.
    pub holder_lamports_before: u64,
}

/// The sole admitted claim-check redemption movement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimCheckRedemptionPlanV1 {
    entitlement_atoms: u64,
    vault_after: u64,
    holder_tokens_before: u64,
    holder_tokens_ceiling: u64,
    holder_lamports_after: u64,
}

impl ClaimCheckRedemptionPlanV1 {
    /// Build the sole admitted redemption movement, or refuse to exist.
    pub fn new(
        observation: ClaimCheckRedemptionObservationV1,
    ) -> ClaimCheckConservationResultV1<Self> {
        if observation.entitlement_atoms == 0 {
            return Err(ClaimCheckConservationErrorV1::EmptyClaimCheck);
        }
        if observation.record_lamports == 0 {
            return Err(ClaimCheckConservationErrorV1::ZeroCoordinate);
        }

        // The vault must hold what this record promises. This is the design's
        // running invariant -- the sum of live entitlements equals the vault
        // balance -- evaluated at the one instant it is about to be spent.
        let vault_after = observation
            .vault_before
            .checked_sub(observation.entitlement_atoms)
            .ok_or(ClaimCheckConservationErrorV1::Uncapitalized)?;

        // The holder's credit is bounded, not fixed: a transfer-fee mint can
        // deliver less than the vault gave up. What is exact is the vault's
        // debit, which is the side another holder's entitlement depends on.
        let holder_tokens_ceiling = observation
            .holder_tokens_before
            .checked_add(observation.entitlement_atoms)
            .ok_or(ClaimCheckConservationErrorV1::ArithmeticOverflow)?;

        Ok(Self {
            entitlement_atoms: observation.entitlement_atoms,
            vault_after,
            holder_tokens_before: observation.holder_tokens_before,
            holder_tokens_ceiling,
            holder_lamports_after: observation
                .holder_lamports_before
                .checked_add(observation.record_lamports)
                .ok_or(ClaimCheckConservationErrorV1::ArithmeticOverflow)?,
        })
    }

    /// Verify observed post-balances against this exact plan.
    pub fn validate_post(
        self,
        post: ClaimCheckRedemptionPostV1,
    ) -> ClaimCheckConservationResultV1<()> {
        if post.vault_atoms != self.vault_after
            || post.record_lamports != 0
            || post.holder_lamports != self.holder_lamports_after
            || post.holder_tokens < self.holder_tokens_before
            || post.holder_tokens > self.holder_tokens_ceiling
        {
            return Err(ClaimCheckConservationErrorV1::PostconditionMismatch);
        }
        Ok(())
    }

    /// Atoms the vault must give up.
    #[must_use]
    pub const fn entitlement_atoms(self) -> u64 {
        self.entitlement_atoms
    }

    /// Vault balance this redemption must leave behind.
    #[must_use]
    pub const fn vault_after(self) -> u64 {
        self.vault_after
    }

    /// Holder wallet lamports after the record closes into it.
    #[must_use]
    pub const fn holder_lamports_after(self) -> u64 {
        self.holder_lamports_after
    }
}

/// Observed balances after one claim-check redemption.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimCheckRedemptionPostV1 {
    /// Escrow vault atoms after the payout.
    pub vault_atoms: u64,
    /// The holder's token account atoms after the payout.
    pub holder_tokens: u64,
    /// Claim-check record lamports; must be zero.
    pub record_lamports: u64,
    /// The holder's wallet lamports after the record closed into it.
    pub holder_lamports: u64,
}

fn fold_credit(
    sink: ClaimCheckAccountObservationV1,
    credits: &[([u8; 32], u64)],
) -> ClaimCheckConservationResultV1<u64> {
    let mut total = sink.lamports;
    for (identity, amount) in credits {
        if *identity == sink.identity {
            total = total
                .checked_add(*amount)
                .ok_or(ClaimCheckConservationErrorV1::ArithmeticOverflow)?;
        }
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rent-exempt minimum for a data account of `bytes`, as the runtime
    /// computes it, so a test can state the design's funding argument as
    /// arithmetic rather than as a claim.
    ///
    /// Deliberately test-only. A route must read the Rent sysvar: a second
    /// author for the rent figure is how a program that funds the wrong amount
    /// gets built and passes its own tests.
    const fn rent_exempt_reference_v1(bytes: u64) -> u64 {
        // (ACCOUNT_STORAGE_OVERHEAD + bytes) * LAMPORTS_PER_BYTE_YEAR * EXEMPTION.
        (128 + bytes) * 3480 * 2
    }
    use crate::claim_check_v1::{
        CLAIM_CHECK_BYTES_V1, CLAIM_CHECK_ESCROW_BYTES_V1, COMPACTION_CRANK_REWARD_LAMPORTS_V1,
    };

    const POSITION: [u8; 32] = [11; 32];
    const ADMISSION: [u8; 32] = [12; 32];
    const CLAIM_CHECK: [u8; 32] = [13; 32];
    const CRANKER: [u8; 32] = [21; 32];
    const OPENER: [u8; 32] = [22; 32];
    const RENT_CREDIT: [u8; 32] = [23; 32];

    const TOKEN_ACCOUNT_BYTES: u64 = 165;

    fn position_bytes(outcomes: u64) -> u64 {
        128 + 8 * outcomes
    }

    fn account(identity: [u8; 32], lamports: u64) -> ClaimCheckAccountObservationV1 {
        ClaimCheckAccountObservationV1 { identity, lamports }
    }

    fn observation(outcomes: u64, payout: u64) -> ClaimCheckCompactionObservationV1 {
        let claim_check_rent = if payout == 0 {
            0
        } else {
            rent_exempt_reference_v1(CLAIM_CHECK_BYTES_V1 as u64)
        };
        ClaimCheckCompactionObservationV1 {
            payout_atoms: payout,
            hoard_before: 10_000_000,
            hoard_after: 10_000_000 - payout,
            vault_before: 4_000,
            vault_after: 4_000 + payout,
            position: account(POSITION, rent_exempt_reference_v1(position_bytes(outcomes))),
            admission: account(ADMISSION, rent_exempt_reference_v1(512)),
            claim_check: account(CLAIM_CHECK, 0),
            cranker: account(CRANKER, 1_000_000),
            opener: account(OPENER, 500_000),
            rent_credit: account(RENT_CREDIT, 7_000_000),
            claim_check_rent,
            opener_debt: opener_outlay(),
            crank_reward_cap: COMPACTION_CRANK_REWARD_LAMPORTS_V1,
        }
    }

    fn opener_outlay() -> u64 {
        rent_exempt_reference_v1(CLAIM_CHECK_ESCROW_BYTES_V1 as u64)
            + rent_exempt_reference_v1(TOKEN_ACCOUNT_BYTES)
    }

    fn post_of(
        plan: ClaimCheckCompactionPlanV1,
        observation: ClaimCheckCompactionObservationV1,
    ) -> ClaimCheckCompactionPostV1 {
        ClaimCheckCompactionPostV1 {
            position_lamports: 0,
            admission_lamports: 0,
            claim_check_lamports: observation.claim_check.lamports + plan.claim_check_top_up(),
            cranker_lamports: observation.cranker.lamports + plan.crank_reward(),
            opener_lamports: observation.opener.lamports + plan.opener_repayment(),
            rent_credit_lamports: observation.rent_credit.lamports + plan.rent_credit_residue(),
            hoard_lamports_of_collateral: observation.hoard_after,
            vault_lamports_of_collateral: observation.vault_after,
        }
    }

    #[test]
    fn the_reference_rent_matches_the_runtimes_known_token_account_figure() {
        // Anchors every funding argument below to a number anyone can check.
        assert_eq!(rent_exempt_reference_v1(TOKEN_ACCOUNT_BYTES), 2_039_280);
    }

    #[test]
    fn a_compaction_closes_both_ledgers() {
        let observed = observation(2, 750_000);
        let plan = ClaimCheckCompactionPlanV1::new(observed).expect("plan");
        assert_eq!(plan.entitlement_atoms(), 750_000);
        assert_eq!(
            plan.swept_lamports(),
            plan.claim_check_top_up()
                + plan.crank_reward()
                + plan.opener_repayment()
                + plan.rent_credit_residue()
        );
        assert_eq!(plan.validate_post(post_of(plan, observed)), Ok(()));
    }

    #[test]
    fn the_designs_stated_split_could_not_have_closed_for_a_binary_market() {
        // The amendment, stated as arithmetic rather than as an opinion. Paying
        // the opener before the cranker leaves the first crank with exactly
        // nothing, and an unfunded crank is an unturned crank.
        let observed = observation(2, 750_000);
        let released = observed.position.lamports + observed.admission.lamports;
        let claim_check_rent = observed.claim_check_rent;
        assert!(released - claim_check_rent < opener_outlay());

        // Under the order actually implemented, the crank is funded on its
        // first turn and the opener's debt still shrinks.
        let plan = ClaimCheckCompactionPlanV1::new(observed).expect("plan");
        assert_eq!(plan.crank_reward(), COMPACTION_CRANK_REWARD_LAMPORTS_V1);
        assert!(plan.opener_repayment() > 0);
        assert!(plan.opener_debt_after() < observed.opener_debt);
        assert_eq!(plan.rent_credit_residue(), 0);
    }

    #[test]
    fn the_openers_debt_discharges_and_the_residue_then_reaches_rent_credit() {
        let first = observation(2, 750_000);
        let first_plan = ClaimCheckCompactionPlanV1::new(first).expect("first");
        let second = ClaimCheckCompactionObservationV1 {
            opener_debt: first_plan.opener_debt_after(),
            ..first
        };
        let second_plan = ClaimCheckCompactionPlanV1::new(second).expect("second");
        assert_eq!(second_plan.opener_debt_after(), 0);
        assert!(second_plan.rent_credit_residue() > 0);
        assert_eq!(
            first_plan.opener_repayment() + second_plan.opener_repayment(),
            opener_outlay()
        );
    }

    #[test]
    fn a_claim_checks_rent_is_covered_by_the_positions_own_rent_at_every_width() {
        // The fixed-width record is strictly cheaper than the runtime-width
        // position plus its 512-byte admission at any outcome count, so the
        // mandatory obligation can never exceed what the sweep releases. A
        // compaction that could refuse for lack of funds would reintroduce the
        // deadlock through the funding door.
        for outcomes in [1_u64, 2, 8, 64, 256, 4_096] {
            let released =
                rent_exempt_reference_v1(position_bytes(outcomes)) + rent_exempt_reference_v1(512);
            assert!(released > rent_exempt_reference_v1(CLAIM_CHECK_BYTES_V1 as u64));

            let observed = observation(outcomes, 1);
            let plan = ClaimCheckCompactionPlanV1::new(observed).expect("plan");
            assert_eq!(plan.claim_check_top_up(), observed.claim_check_rent);
            assert_eq!(plan.validate_post(post_of(plan, observed)), Ok(()));
        }
    }

    #[test]
    fn the_thinnest_admissible_position_is_paid_a_small_reward_never_refused() {
        // Underfunding must degrade the reward, not produce an error.
        let mut observed = observation(1, 1);
        observed.position.lamports = observed.claim_check_rent + 3;
        observed.admission.lamports = 0;
        let plan = ClaimCheckCompactionPlanV1::new(observed).expect("thin plan");
        assert_eq!(plan.crank_reward(), 3);
        assert_eq!(plan.opener_repayment(), 0);
        assert_eq!(plan.rent_credit_residue(), 0);
        assert_eq!(plan.validate_post(post_of(plan, observed)), Ok(()));

        // Even a position holding exactly the record's rent and not one lamport
        // more is compacted, for a reward of zero.
        let mut exact = observation(1, 1);
        exact.position.lamports = exact.claim_check_rent;
        exact.admission.lamports = 0;
        let exact_plan = ClaimCheckCompactionPlanV1::new(exact).expect("exact plan");
        assert_eq!(exact_plan.crank_reward(), 0);
        assert_eq!(exact_plan.validate_post(post_of(exact_plan, exact)), Ok(()));
    }

    #[test]
    fn dust_at_the_vacant_claim_check_address_is_absorbed_not_refused() {
        // The 1-lamport griefer, at the one address a compaction is about to
        // create. Nothing here compares a live balance to a declared one.
        for dust in [1_u64, 999, 2_895_359, 2_895_360, 9_000_000] {
            let mut observed = observation(2, 750_000);
            observed.claim_check.lamports = dust;
            let plan = ClaimCheckCompactionPlanV1::new(observed).expect("dusted plan");
            assert_eq!(
                plan.claim_check_top_up(),
                observed.claim_check_rent.saturating_sub(dust)
            );
            assert_eq!(plan.validate_post(post_of(plan, observed)), Ok(()));
            // The griefer's lamports fund the record and free the sweep to pay
            // the crank more, which is the opposite of a block.
            assert!(plan.crank_reward() >= COMPACTION_CRANK_REWARD_LAMPORTS_V1.min(dust));
        }
    }

    #[test]
    fn dust_at_the_position_or_admission_enlarges_the_sweep_and_blocks_nothing() {
        for dust in [1_u64, 5_000_000] {
            let base = observation(2, 750_000);
            let mut observed = base;
            observed.position.lamports += dust;
            observed.admission.lamports += dust;
            let plan = ClaimCheckCompactionPlanV1::new(observed).expect("dusted plan");
            assert_eq!(
                plan.swept_lamports(),
                base.position.lamports + base.admission.lamports + 2 * dust
            );
            assert_eq!(plan.validate_post(post_of(plan, observed)), Ok(()));
        }
    }

    #[test]
    fn an_opener_who_is_also_the_cranker_receives_the_sum_of_both_credits() {
        // The expected case, and the one a naive plan gets wrong twice.
        let mut observed = observation(2, 750_000);
        observed.opener = account(CRANKER, observed.cranker.lamports);
        let plan = ClaimCheckCompactionPlanV1::new(observed).expect("aliased plan");
        let expected = observed.cranker.lamports + plan.crank_reward() + plan.opener_repayment();
        assert_eq!(
            plan.validate_post(ClaimCheckCompactionPostV1 {
                position_lamports: 0,
                admission_lamports: 0,
                claim_check_lamports: plan.claim_check_top_up(),
                cranker_lamports: expected,
                opener_lamports: expected,
                rent_credit_lamports: observed.rent_credit.lamports + plan.rent_credit_residue(),
                hoard_lamports_of_collateral: observed.hoard_after,
                vault_lamports_of_collateral: observed.vault_after,
            }),
            Ok(())
        );
    }

    #[test]
    fn a_sink_that_is_also_a_closing_account_is_refused() {
        for alias in [POSITION, ADMISSION, CLAIM_CHECK] {
            let mut observed = observation(2, 750_000);
            observed.rent_credit = account(alias, 7_000_000);
            assert_eq!(
                ClaimCheckCompactionPlanV1::new(observed),
                Err(ClaimCheckConservationErrorV1::IdentityAlias)
            );
        }
        let mut aliased = observation(2, 750_000);
        aliased.admission = account(POSITION, aliased.admission.lamports);
        assert_eq!(
            ClaimCheckCompactionPlanV1::new(aliased),
            Err(ClaimCheckConservationErrorV1::IdentityAlias)
        );
    }

    #[test]
    fn a_zero_payout_position_compacts_without_minting_a_record() {
        // Every holder of a losing outcome. Supply still retires and the
        // accounts still close; only the useless record is skipped.
        let observed = observation(2, 0);
        let plan = ClaimCheckCompactionPlanV1::new(observed).expect("empty plan");
        assert!(!plan.mints_claim_check());
        assert_eq!(plan.entitlement_atoms(), 0);
        assert_eq!(plan.claim_check_top_up(), 0);
        assert_eq!(plan.crank_reward(), COMPACTION_CRANK_REWARD_LAMPORTS_V1);
        assert_eq!(plan.validate_post(post_of(plan, observed)), Ok(()));

        // And funding a record for nothing is refused, in both directions.
        let mut funded = observation(2, 0);
        funded.claim_check_rent = rent_exempt_reference_v1(CLAIM_CHECK_BYTES_V1 as u64);
        assert_eq!(
            ClaimCheckCompactionPlanV1::new(funded),
            Err(ClaimCheckConservationErrorV1::EmptyClaimCheck)
        );
        let mut unfunded = observation(2, 750_000);
        unfunded.claim_check_rent = 0;
        assert_eq!(
            ClaimCheckCompactionPlanV1::new(unfunded),
            Err(ClaimCheckConservationErrorV1::EmptyClaimCheck)
        );
    }

    #[test]
    fn a_transfer_fee_makes_the_entitlement_the_observed_credit() {
        let mut observed = observation(2, 750_000);
        observed.vault_after = observed.vault_before + 748_500;
        let plan = ClaimCheckCompactionPlanV1::new(observed).expect("fee plan");
        assert_eq!(plan.entitlement_atoms(), 748_500);
        assert_eq!(plan.validate_post(post_of(plan, observed)), Ok(()));
    }

    #[test]
    fn a_vault_that_gained_more_than_was_sent_is_refused() {
        // Somebody else's collateral. Paying this claim-check out of it would
        // be theft from another holder.
        let mut observed = observation(2, 750_000);
        observed.vault_after = observed.vault_before + 750_001;
        assert_eq!(
            ClaimCheckCompactionPlanV1::new(observed),
            Err(ClaimCheckConservationErrorV1::Conservation)
        );

        let mut shrunk = observation(2, 750_000);
        shrunk.vault_after = shrunk.vault_before - 1;
        assert_eq!(
            ClaimCheckCompactionPlanV1::new(shrunk),
            Err(ClaimCheckConservationErrorV1::Conservation)
        );
    }

    #[test]
    fn a_hoard_that_did_not_move_by_the_derived_payout_is_refused() {
        for hoard_after in [10_000_000_u64, 9_250_001, 9_249_999] {
            let mut observed = observation(2, 750_000);
            observed.hoard_after = hoard_after;
            assert_eq!(
                ClaimCheckCompactionPlanV1::new(observed),
                Err(ClaimCheckConservationErrorV1::Conservation)
            );
        }
        let mut overdrawn = observation(2, 750_000);
        overdrawn.hoard_before = 1;
        assert_eq!(
            ClaimCheckCompactionPlanV1::new(overdrawn),
            Err(ClaimCheckConservationErrorV1::Uncapitalized)
        );
    }

    #[test]
    fn a_post_state_that_strands_a_lamport_is_refused() {
        let observed = observation(2, 750_000);
        let plan = ClaimCheckCompactionPlanV1::new(observed).expect("plan");
        let good = post_of(plan, observed);
        for mutate in [
            |post: &mut ClaimCheckCompactionPostV1| post.position_lamports = 1,
            |post: &mut ClaimCheckCompactionPostV1| post.admission_lamports = 1,
            |post: &mut ClaimCheckCompactionPostV1| post.claim_check_lamports -= 1,
            |post: &mut ClaimCheckCompactionPostV1| post.cranker_lamports += 1,
            |post: &mut ClaimCheckCompactionPostV1| post.opener_lamports -= 1,
            |post: &mut ClaimCheckCompactionPostV1| post.rent_credit_lamports += 1,
            |post: &mut ClaimCheckCompactionPostV1| post.hoard_lamports_of_collateral += 1,
            |post: &mut ClaimCheckCompactionPostV1| post.vault_lamports_of_collateral -= 1,
        ] {
            let mut post = good;
            mutate(&mut post);
            assert_eq!(
                plan.validate_post(post),
                Err(ClaimCheckConservationErrorV1::PostconditionMismatch)
            );
        }
    }

    fn redemption(entitlement: u64) -> ClaimCheckRedemptionObservationV1 {
        ClaimCheckRedemptionObservationV1 {
            entitlement_atoms: entitlement,
            vault_before: 1_500_000,
            holder_tokens_before: 42,
            record_lamports: rent_exempt_reference_v1(CLAIM_CHECK_BYTES_V1 as u64),
            holder_lamports_before: 9_000,
        }
    }

    #[test]
    fn a_redemption_debits_the_vault_by_exactly_what_the_record_promised() {
        let observed = redemption(750_000);
        let plan = ClaimCheckRedemptionPlanV1::new(observed).expect("plan");
        assert_eq!(plan.vault_after(), 750_000);
        assert_eq!(
            plan.validate_post(ClaimCheckRedemptionPostV1 {
                vault_atoms: 750_000,
                holder_tokens: 750_042,
                record_lamports: 0,
                holder_lamports: observed.holder_lamports_before + observed.record_lamports,
            }),
            Ok(())
        );
    }

    #[test]
    fn a_redemption_tolerates_a_transfer_fee_on_the_holders_side_only() {
        let observed = redemption(750_000);
        let plan = ClaimCheckRedemptionPlanV1::new(observed).expect("plan");
        let holder_lamports = observed.holder_lamports_before + observed.record_lamports;
        // Less than promised is a fee, and admitted.
        assert_eq!(
            plan.validate_post(ClaimCheckRedemptionPostV1 {
                vault_atoms: 750_000,
                holder_tokens: 749_000,
                record_lamports: 0,
                holder_lamports,
            }),
            Ok(())
        );
        // More than promised is another holder's collateral, and refused.
        assert_eq!(
            plan.validate_post(ClaimCheckRedemptionPostV1 {
                vault_atoms: 750_000,
                holder_tokens: 750_043,
                record_lamports: 0,
                holder_lamports,
            }),
            Err(ClaimCheckConservationErrorV1::PostconditionMismatch)
        );
        // And the vault's own debit stays exact in every case.
        assert_eq!(
            plan.validate_post(ClaimCheckRedemptionPostV1 {
                vault_atoms: 750_001,
                holder_tokens: 750_042,
                record_lamports: 0,
                holder_lamports,
            }),
            Err(ClaimCheckConservationErrorV1::PostconditionMismatch)
        );
    }

    #[test]
    fn a_vault_that_cannot_cover_the_record_refuses_rather_than_underpays() {
        let mut observed = redemption(750_000);
        observed.vault_before = 749_999;
        assert_eq!(
            ClaimCheckRedemptionPlanV1::new(observed),
            Err(ClaimCheckConservationErrorV1::Uncapitalized)
        );
    }

    #[test]
    fn a_redemption_leaves_no_lamport_in_the_closed_record() {
        let observed = redemption(750_000);
        let plan = ClaimCheckRedemptionPlanV1::new(observed).expect("plan");
        assert_eq!(
            plan.validate_post(ClaimCheckRedemptionPostV1 {
                vault_atoms: 750_000,
                holder_tokens: 750_042,
                record_lamports: 1,
                holder_lamports: plan.holder_lamports_after(),
            }),
            Err(ClaimCheckConservationErrorV1::PostconditionMismatch)
        );
    }

    #[test]
    fn dust_at_the_record_reaches_the_holder_rather_than_stranding() {
        let mut observed = redemption(750_000);
        observed.record_lamports += 4_242;
        let plan = ClaimCheckRedemptionPlanV1::new(observed).expect("plan");
        assert_eq!(
            plan.holder_lamports_after(),
            observed.holder_lamports_before + observed.record_lamports
        );
    }

    #[test]
    fn a_record_promising_nothing_cannot_be_redeemed() {
        assert_eq!(
            ClaimCheckRedemptionPlanV1::new(redemption(0)),
            Err(ClaimCheckConservationErrorV1::EmptyClaimCheck)
        );
    }
}
