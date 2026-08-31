//! Conservation for fractional compaction and shard-burn redemption.
//!
//! Two plans, each of which refuses to exist unless its movement balances, plus
//! a `validate_post` against observed post-balances. The program supplies
//! observations and checks the postcondition; arithmetic never appears inline in
//! a route. This is [`crate::claim_check_conservation_v1`]'s discipline, applied
//! to a claim whose payee is an instrument.
//!
//! Two things are genuinely different here, and both are the fractional shape
//! rather than a preference.
//!
//! **The lamport half has one author, and it is the native plan.** A fractional
//! compaction closes the same two accounts a native one does -- a Position and
//! its admission record -- funds one record out of the sweep, and pays the same
//! three sinks in the same amended order. So it does not restate that split; it
//! *calls* [`ClaimCheckCompactionPlanV1`] and adds its own conjunct. A second
//! author for the rent ordering is how the two halves of one feature drift into
//! paying different people.
//!
//! **The collateral half cannot absorb a transfer fee, and the native one can.**
//! A native record promises a *total*, so a vault credited less than was sent
//! can simply promise the smaller number and the one holder is paid what is
//! there. A fractional record promises a *rate* -- `payout_per_claim`, applied
//! independently by every holder who ever returns -- and a shortfall cannot be
//! distributed across claimants who are unknown at compaction and arrive one at
//! a time. Reducing the rate would underpay the early holders; keeping it would
//! leave the last holder unpayable. So a short credit is a refusal
//! (`RateNotCovered`), not a smaller promise. The observation is still made
//! rather than assumed -- the discipline is unchanged -- only its tolerance is
//! zero.

use crate::claim_check_conservation_v1::{
    ClaimCheckCompactionObservationV1, ClaimCheckCompactionPlanV1, ClaimCheckCompactionPostV1,
    ClaimCheckConservationErrorV1,
};
use crate::claim_check_v1::ClaimCheckErrorV1;
use crate::fractional_claim_check_v1::FractionalClaimCheckV1;

/// Stable conservation refusal for a fractional compaction or redemption.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FractionalClaimCheckConservationErrorV1 {
    /// The shared lamport or collateral ledger refused.
    Shared(ClaimCheckConservationErrorV1),
    /// A denominator or per-claim payout no exposure terms could produce.
    NonFractionalTerms,
    /// The vault was credited less than the promised rate requires.
    ///
    /// A fractional record promises a rate every holder applies independently,
    /// so a shortfall has nobody to charge. See this module's header.
    RateNotCovered,
    /// The shard balance presented forms no whole Claims coordinate.
    NoWholeClaim,
    /// Shard supply or a holder balance did not move by exactly what was burned.
    ShardConservation,
    /// The record's escrowed balance could not cover this redemption.
    Overdrawn,
    /// Checked arithmetic overflowed or underflowed.
    ArithmeticOverflow,
    /// Observed post-balances did not match the admitted plan.
    PostconditionMismatch,
}

/// Result alias for fractional claim-check conservation.
pub type FractionalClaimCheckConservationResultV1<T> =
    core::result::Result<T, FractionalClaimCheckConservationErrorV1>;

impl From<ClaimCheckConservationErrorV1> for FractionalClaimCheckConservationErrorV1 {
    fn from(value: ClaimCheckConservationErrorV1) -> Self {
        Self::Shared(value)
    }
}

impl From<ClaimCheckErrorV1> for FractionalClaimCheckConservationErrorV1 {
    fn from(value: ClaimCheckErrorV1) -> Self {
        match value {
            ClaimCheckErrorV1::Arithmetic => Self::ArithmeticOverflow,
            ClaimCheckErrorV1::InvalidEntitlement => Self::NonFractionalTerms,
            other => Self::Shared(ClaimCheckConservationErrorV1::from(other)),
        }
    }
}

/// Everything one fractional compaction observed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalClaimCheckCompactionObservationV1 {
    /// The lamport and collateral movement this shares with a native
    /// compaction, observed exactly as that plan requires.
    pub shared: ClaimCheckCompactionObservationV1,
    /// Exact shard atoms per whole Claims coordinate, from the finalized terms.
    pub denominator: u64,
    /// Exact collateral atoms per whole Claims coordinate, from the evaluator.
    pub payout_per_claim: u64,
    /// Shard Mint supply observed at compaction: every claim still outstanding.
    pub shard_supply: u64,
}

/// The sole admitted fractional compaction movement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalClaimCheckCompactionPlanV1 {
    shared: ClaimCheckCompactionPlanV1,
    whole_claims: u64,
    escrowed_atoms: u64,
}

impl FractionalClaimCheckCompactionPlanV1 {
    /// Build the sole admitted fractional compaction movement, or refuse.
    pub fn new(
        observation: FractionalClaimCheckCompactionObservationV1,
    ) -> FractionalClaimCheckConservationResultV1<Self> {
        // The exposure terms refuse a denominator of one or zero at decode
        // (`NonFractionalDenominator`), so a compaction claiming one is
        // compacting something the terms could not have produced.
        if observation.denominator <= 1 {
            return Err(FractionalClaimCheckConservationErrorV1::NonFractionalTerms);
        }

        // CALLED, never restated: the lamport split, its amended order, the
        // aliased-sink folding, the dust tolerances and the
        // record-funded-exactly-when-there-is-a-claim rule all belong to the
        // native plan and have one author there.
        let shared = ClaimCheckCompactionPlanV1::new(observation.shared)?;

        // The only division, and it floors, exactly as `divide_exposure_shards_v2`
        // does. Supply below the denominator forms no claim, so there is nothing
        // to escrow and nothing to record.
        let whole_claims = observation.shard_supply / observation.denominator;
        let escrowed_atoms = whole_claims
            .checked_mul(observation.payout_per_claim)
            .ok_or(FractionalClaimCheckConservationErrorV1::ArithmeticOverflow)?;

        // The binding conjunct, and the reason this plan exists at all: what the
        // terminal derivation actually moved into the vault must be exactly what
        // the rate this record persists will later pay out. Exact in both
        // directions -- a vault credited less leaves the last holder unpayable,
        // and a vault credited more is holding somebody else's collateral.
        if shared.entitlement_atoms() != escrowed_atoms {
            return Err(FractionalClaimCheckConservationErrorV1::RateNotCovered);
        }

        // Funding and promising stay welded, in both directions, exactly as the
        // native plan welds them: a record is minted when and only when there is
        // a claim to record.
        if shared.mints_claim_check() != (escrowed_atoms != 0) {
            return Err(FractionalClaimCheckConservationErrorV1::Shared(
                ClaimCheckConservationErrorV1::EmptyClaimCheck,
            ));
        }
        // There is deliberately no separate refusal for a zero rate. A
        // coordinate paying nothing per claim escrows nothing whatever its
        // supply, so the conjunct above already declines to mint, and a second
        // test of the same fact would be a check that can never fire.
        Ok(Self {
            shared,
            whole_claims,
            escrowed_atoms,
        })
    }

    /// Verify observed post-balances against this exact plan.
    pub fn validate_post(
        self,
        post: ClaimCheckCompactionPostV1,
    ) -> FractionalClaimCheckConservationResultV1<()> {
        self.shared.validate_post(post)?;
        Ok(())
    }

    /// The shared lamport and collateral plan, for the route's own accessors.
    #[must_use]
    pub const fn shared(self) -> ClaimCheckCompactionPlanV1 {
        self.shared
    }

    /// Whole Claims coordinates the outstanding supply could form.
    #[must_use]
    pub const fn whole_claims(self) -> u64 {
        self.whole_claims
    }

    /// Collateral atoms this record opens escrowed, and pays down.
    #[must_use]
    pub const fn escrowed_atoms(self) -> u64 {
        self.escrowed_atoms
    }

    /// Whether this compaction mints a fractional claim-check at all.
    #[must_use]
    pub const fn mints_claim_check(self) -> bool {
        self.escrowed_atoms != 0
    }
}

/// Everything one shard-burn redemption observed before it moved anything.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalClaimCheckRedemptionObservationV1 {
    /// The live record, supplying the denominator, rate and escrowed balance.
    pub record: FractionalClaimCheckV1,
    /// Shard atoms the holder presented.
    pub shard_atoms: u64,
    /// The holder's shard balance before the burn.
    pub holder_shards_before: u64,
    /// The shard Mint's supply before the burn.
    pub shard_supply_before: u64,
    /// Escrow vault collateral before the payout.
    pub vault_before: u64,
    /// The holder's collateral token balance before the payout.
    pub holder_collateral_before: u64,
    /// Live lamports at the record, dust included.
    pub record_lamports: u64,
    /// The holder's wallet lamports before any close.
    pub holder_lamports_before: u64,
}

/// The sole admitted shard-burn redemption movement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalClaimCheckRedemptionPlanV1 {
    whole_claims: u64,
    consumed_shards: u64,
    change_shards: u64,
    collateral_atoms: u64,
    holder_shards_after: u64,
    shard_supply_after: u64,
    vault_after: u64,
    holder_collateral_before: u64,
    holder_collateral_ceiling: u64,
    escrowed_after: u64,
    settles: bool,
    record_lamports_after: u64,
    holder_lamports_after: u64,
}

impl FractionalClaimCheckRedemptionPlanV1 {
    /// Build the sole admitted redemption movement, or refuse to exist.
    pub fn new(
        observation: FractionalClaimCheckRedemptionObservationV1,
    ) -> FractionalClaimCheckConservationResultV1<Self> {
        let record = observation.record.new()?;

        // A holder cannot present shards they do not hold, and a burn of nothing
        // is not a redemption.
        if observation.shard_atoms == 0
            || observation.shard_atoms > observation.holder_shards_before
        {
            return Err(FractionalClaimCheckConservationErrorV1::ShardConservation);
        }
        // The shard supply must be able to contain the holder's own balance;
        // anything else means one of the two observations is not of this Mint.
        if observation.holder_shards_before > observation.shard_supply_before {
            return Err(FractionalClaimCheckConservationErrorV1::ShardConservation);
        }

        // The only division, and it floors. Sub-denominator dust forms no claim
        // -- the refusal `divide_exposure_shards_v2` names `NoWholeClaim` --
        // and it is not a claim on collateral here either.
        let whole_claims = record.whole_claims_for(observation.shard_atoms);
        if whole_claims == 0 {
            return Err(FractionalClaimCheckConservationErrorV1::NoWholeClaim);
        }
        let consumed_shards = record.consumed_shards(whole_claims)?;
        let change_shards = observation
            .shard_atoms
            .checked_sub(consumed_shards)
            .ok_or(FractionalClaimCheckConservationErrorV1::ArithmeticOverflow)?;

        // Two exact multiplications, no second rounding boundary. This is the
        // whole claim of the record: the same two numbers and the same two
        // operations on-time redemption would have used.
        let collateral_atoms = record.claim_payout(whole_claims)?;
        if collateral_atoms == 0 {
            return Err(FractionalClaimCheckConservationErrorV1::NonFractionalTerms);
        }

        let holder_shards_after = observation
            .holder_shards_before
            .checked_sub(consumed_shards)
            .ok_or(FractionalClaimCheckConservationErrorV1::ShardConservation)?;
        let shard_supply_after = observation
            .shard_supply_before
            .checked_sub(consumed_shards)
            .ok_or(FractionalClaimCheckConservationErrorV1::ShardConservation)?;

        // The vault's debit is exact, because it is the side every other
        // holder's claim depends on.
        let vault_after = observation
            .vault_before
            .checked_sub(collateral_atoms)
            .ok_or(FractionalClaimCheckConservationErrorV1::Overdrawn)?;
        // The holder's credit is bounded rather than fixed: a transfer-fee mint
        // can deliver less than the vault gave up, and that loss is the holder's
        // own rather than another claimant's.
        let holder_collateral_ceiling = observation
            .holder_collateral_before
            .checked_add(collateral_atoms)
            .ok_or(FractionalClaimCheckConservationErrorV1::ArithmeticOverflow)?;

        let paid_down = record.pay_down(collateral_atoms)?;
        let settles = paid_down.is_settled();
        let escrowed_after = paid_down.remaining().map_or(0, |next| next.escrowed_atoms);

        // Lamports move on the settling redemption and on no other. An
        // unsettled redemption leaves the record funded, because the record
        // still has to exist.
        let (record_lamports_after, holder_lamports_after) = if settles {
            (
                0,
                observation
                    .holder_lamports_before
                    .checked_add(observation.record_lamports)
                    .ok_or(FractionalClaimCheckConservationErrorV1::ArithmeticOverflow)?,
            )
        } else {
            (
                observation.record_lamports,
                observation.holder_lamports_before,
            )
        };

        Ok(Self {
            whole_claims,
            consumed_shards,
            change_shards,
            collateral_atoms,
            holder_shards_after,
            shard_supply_after,
            vault_after,
            holder_collateral_before: observation.holder_collateral_before,
            holder_collateral_ceiling,
            escrowed_after,
            settles,
            record_lamports_after,
            holder_lamports_after,
        })
    }

    /// Verify observed post-balances against this exact plan.
    pub fn validate_post(
        self,
        post: FractionalClaimCheckRedemptionPostV1,
    ) -> FractionalClaimCheckConservationResultV1<()> {
        // The record's lamports are zero exactly when it settled, and are the
        // untouched pre-balance otherwise -- stated as a number the plan carries
        // rather than as a comparison of the observation with itself, so a route
        // that closed a live record, or left a settled one funded, is refused
        // here rather than discovered by whoever comes back next.
        if post.shard_supply != self.shard_supply_after
            || post.holder_shards != self.holder_shards_after
            || post.vault_atoms != self.vault_after
            || post.holder_collateral < self.holder_collateral_before
            || post.holder_collateral > self.holder_collateral_ceiling
            || post.holder_lamports != self.holder_lamports_after
            || post.record_lamports != self.record_lamports_after
            || post.record_escrowed_atoms != self.escrowed_after
        {
            return Err(FractionalClaimCheckConservationErrorV1::PostconditionMismatch);
        }
        if self.settles && (post.record_lamports != 0 || post.record_escrowed_atoms != 0) {
            return Err(FractionalClaimCheckConservationErrorV1::PostconditionMismatch);
        }
        Ok(())
    }

    /// Whole Claims coordinates this redemption paid for.
    #[must_use]
    pub const fn whole_claims(self) -> u64 {
        self.whole_claims
    }

    /// Shard atoms this redemption burns. Never more than the holder presented.
    #[must_use]
    pub const fn consumed_shards(self) -> u64 {
        self.consumed_shards
    }

    /// Shard atoms that stay in the holder's account, unburned.
    #[must_use]
    pub const fn change_shards(self) -> u64 {
        self.change_shards
    }

    /// Collateral atoms the vault must give up.
    #[must_use]
    pub const fn collateral_atoms(self) -> u64 {
        self.collateral_atoms
    }

    /// Vault balance this redemption must leave behind.
    #[must_use]
    pub const fn vault_after(self) -> u64 {
        self.vault_after
    }

    /// The record's escrowed balance after this redemption; zero if it settled.
    #[must_use]
    pub const fn escrowed_after(self) -> u64 {
        self.escrowed_after
    }

    /// Whether this redemption exhausts the record and closes it.
    #[must_use]
    pub const fn settles(self) -> bool {
        self.settles
    }

    /// The record's lamports afterwards: untouched, or zero once it settled.
    #[must_use]
    pub const fn record_lamports_after(self) -> u64 {
        self.record_lamports_after
    }

    /// The holder's wallet lamports afterwards; moved only on the settling burn.
    #[must_use]
    pub const fn holder_lamports_after(self) -> u64 {
        self.holder_lamports_after
    }
}

/// Observed balances after one shard-burn redemption.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalClaimCheckRedemptionPostV1 {
    /// Shard Mint supply after the burn.
    pub shard_supply: u64,
    /// The holder's shard balance after the burn.
    pub holder_shards: u64,
    /// Escrow vault collateral after the payout.
    pub vault_atoms: u64,
    /// The holder's collateral balance after the payout.
    pub holder_collateral: u64,
    /// The record's escrowed balance afterwards; zero once it has settled.
    pub record_escrowed_atoms: u64,
    /// The record's lamports; zero once it has settled.
    pub record_lamports: u64,
    /// The holder's wallet lamports afterwards.
    pub holder_lamports: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claim_check_conservation_v1::ClaimCheckAccountObservationV1;
    use crate::claim_check_v1::{CLAIM_CHECK_ESCROW_BYTES_V1, COMPACTION_CRANK_REWARD_LAMPORTS_V1};
    use crate::fractional_claim_check_v1::FRACTIONAL_CLAIM_CHECK_BYTES_V1;

    const DENOMINATOR: u64 = 1_000;
    const PAYOUT_PER_CLAIM: u64 = 7_500;
    const SUPPLY: u64 = 12_345;
    const WHOLE_CLAIMS: u64 = 12;
    const ESCROWED: u64 = WHOLE_CLAIMS * PAYOUT_PER_CLAIM;

    const POSITION: [u8; 32] = [11; 32];
    const ADMISSION: [u8; 32] = [12; 32];
    const RECORD: [u8; 32] = [13; 32];
    const CRANKER: [u8; 32] = [21; 32];
    const OPENER: [u8; 32] = [22; 32];
    const RENT_CREDIT: [u8; 32] = [23; 32];

    /// Rent-exempt minimum as the runtime computes it. Test-only: a route reads
    /// the sysvar, because a second author for the rent figure is how a program
    /// that funds the wrong amount passes its own tests.
    const fn rent_exempt_reference_v1(bytes: u64) -> u64 {
        (128 + bytes) * 3480 * 2
    }

    fn account(identity: [u8; 32], lamports: u64) -> ClaimCheckAccountObservationV1 {
        ClaimCheckAccountObservationV1 { identity, lamports }
    }

    fn record() -> FractionalClaimCheckV1 {
        FractionalClaimCheckV1 {
            aggregate: [1; 32],
            shard_mint: [2; 32],
            market: [3; 32],
            release_set: [4; 32],
            vault: [5; 32],
            collateral_mint: [6; 32],
            position_atoms_digest: [7; 32],
            escrowed_atoms: ESCROWED,
            denominator: DENOMINATOR,
            payout_per_claim: PAYOUT_PER_CLAIM,
            compacted_shard_supply: SUPPLY,
            compacted_slot: 38_892_000,
            generation: 9,
            representation_coordinate: 3,
            bump: 251,
        }
        .new()
        .expect("record")
    }

    fn compaction(
        supply: u64,
        payout_per_claim: u64,
    ) -> FractionalClaimCheckCompactionObservationV1 {
        let escrowed = (supply / DENOMINATOR) * payout_per_claim;
        let claim_check_rent = if escrowed == 0 {
            0
        } else {
            rent_exempt_reference_v1(FRACTIONAL_CLAIM_CHECK_BYTES_V1 as u64)
        };
        FractionalClaimCheckCompactionObservationV1 {
            shared: ClaimCheckCompactionObservationV1 {
                payout_atoms: escrowed,
                hoard_before: 10_000_000,
                hoard_after: 10_000_000 - escrowed,
                vault_before: 4_000,
                vault_after: 4_000 + escrowed,
                // The Fractional reserve Position carries the whole coordinate,
                // so it is a runtime-width account like any other.
                position: account(POSITION, rent_exempt_reference_v1(128 + 8 * 2)),
                admission: account(ADMISSION, rent_exempt_reference_v1(512)),
                claim_check: account(RECORD, 0),
                cranker: account(CRANKER, 1_000_000),
                opener: account(OPENER, 500_000),
                rent_credit: account(RENT_CREDIT, 7_000_000),
                claim_check_rent,
                opener_debt: rent_exempt_reference_v1(CLAIM_CHECK_ESCROW_BYTES_V1 as u64)
                    + rent_exempt_reference_v1(165),
                crank_reward_cap: COMPACTION_CRANK_REWARD_LAMPORTS_V1,
            },
            denominator: DENOMINATOR,
            payout_per_claim,
            shard_supply: supply,
        }
    }

    fn compaction_post(
        plan: FractionalClaimCheckCompactionPlanV1,
        observation: FractionalClaimCheckCompactionObservationV1,
    ) -> ClaimCheckCompactionPostV1 {
        let shared = plan.shared();
        ClaimCheckCompactionPostV1 {
            position_lamports: 0,
            admission_lamports: 0,
            claim_check_lamports: observation.shared.claim_check.lamports
                + shared.claim_check_top_up(),
            cranker_lamports: observation.shared.cranker.lamports + shared.crank_reward(),
            opener_lamports: observation.shared.opener.lamports + shared.opener_repayment(),
            rent_credit_lamports: observation.shared.rent_credit.lamports
                + shared.rent_credit_residue(),
            hoard_lamports_of_collateral: observation.shared.hoard_after,
            vault_lamports_of_collateral: observation.shared.vault_after,
        }
    }

    fn redemption(shard_atoms: u64) -> FractionalClaimCheckRedemptionObservationV1 {
        FractionalClaimCheckRedemptionObservationV1 {
            record: record(),
            shard_atoms,
            holder_shards_before: shard_atoms,
            shard_supply_before: SUPPLY,
            vault_before: ESCROWED,
            holder_collateral_before: 42,
            record_lamports: rent_exempt_reference_v1(FRACTIONAL_CLAIM_CHECK_BYTES_V1 as u64),
            holder_lamports_before: 9_000,
        }
    }

    fn redemption_post(
        plan: FractionalClaimCheckRedemptionPlanV1,
        observation: FractionalClaimCheckRedemptionObservationV1,
    ) -> FractionalClaimCheckRedemptionPostV1 {
        FractionalClaimCheckRedemptionPostV1 {
            shard_supply: observation.shard_supply_before - plan.consumed_shards(),
            holder_shards: observation.holder_shards_before - plan.consumed_shards(),
            vault_atoms: plan.vault_after(),
            holder_collateral: observation.holder_collateral_before + plan.collateral_atoms(),
            record_escrowed_atoms: plan.escrowed_after(),
            record_lamports: plan.record_lamports_after(),
            holder_lamports: plan.holder_lamports_after(),
        }
    }

    #[test]
    fn a_compaction_escrows_every_whole_claim_the_supply_can_form_and_no_dust() {
        let observed = compaction(SUPPLY, PAYOUT_PER_CLAIM);
        let plan = FractionalClaimCheckCompactionPlanV1::new(observed).expect("plan");
        assert_eq!(plan.whole_claims(), WHOLE_CLAIMS);
        assert_eq!(plan.escrowed_atoms(), ESCROWED);
        assert!(plan.mints_claim_check());
        // The dust -- 345 shards, a third of a claim -- is escrowed for nobody,
        // because it is a claim on nothing before compaction and must not become
        // one after.
        assert_eq!(SUPPLY - WHOLE_CLAIMS * DENOMINATOR, 345);
        assert_eq!(plan.validate_post(compaction_post(plan, observed)), Ok(()));
    }

    #[test]
    fn the_lamport_split_is_the_native_plans_and_not_a_second_copy_of_it() {
        // One author for the rent ordering. The fractional plan delegates, so
        // the crank is paid first here for the same measured reason it is there,
        // and the opener's debt discharges over two cranks.
        let observed = compaction(SUPPLY, PAYOUT_PER_CLAIM);
        let plan = FractionalClaimCheckCompactionPlanV1::new(observed).expect("plan");
        let shared = plan.shared();
        assert_eq!(shared.crank_reward(), COMPACTION_CRANK_REWARD_LAMPORTS_V1);
        assert_eq!(
            shared.swept_lamports(),
            shared.claim_check_top_up()
                + shared.crank_reward()
                + shared.opener_repayment()
                + shared.rent_credit_residue()
        );
        assert!(shared.opener_debt_after() < observed.shared.opener_debt);
    }

    #[test]
    fn a_vault_credited_less_than_the_rate_requires_is_refused_not_reduced() {
        // The one place fractional conservation is STRICTER than native. A
        // native record promises a total and can promise the smaller observed
        // number; a fractional record promises a rate that every holder applies
        // independently, so a shortfall has nobody to charge.
        let mut observed = compaction(SUPPLY, PAYOUT_PER_CLAIM);
        observed.shared.vault_after = observed.shared.vault_before + ESCROWED - 1;
        assert_eq!(
            FractionalClaimCheckCompactionPlanV1::new(observed),
            Err(FractionalClaimCheckConservationErrorV1::RateNotCovered)
        );

        // A vault credited MORE is somebody else's collateral, and the shared
        // plan already refuses it before the rate is ever consulted.
        let mut over = compaction(SUPPLY, PAYOUT_PER_CLAIM);
        over.shared.vault_after = over.shared.vault_before + ESCROWED + 1;
        assert_eq!(
            FractionalClaimCheckCompactionPlanV1::new(over),
            Err(FractionalClaimCheckConservationErrorV1::Shared(
                ClaimCheckConservationErrorV1::Conservation
            ))
        );
    }

    #[test]
    fn terms_no_exposure_could_have_produced_are_refused() {
        for denominator in [0_u64, 1] {
            let mut observed = compaction(SUPPLY, PAYOUT_PER_CLAIM);
            observed.denominator = denominator;
            assert_eq!(
                FractionalClaimCheckCompactionPlanV1::new(observed),
                Err(FractionalClaimCheckConservationErrorV1::NonFractionalTerms)
            );
        }
    }

    #[test]
    fn a_coordinate_that_pays_nothing_compacts_without_minting_a_record() {
        // The losing outcome, fractional edition: supply outstanding, and every
        // claim on it worth zero. Supply still retires, the accounts still
        // close, and no record is minted -- because a record nobody would ever
        // redeem pins the escrow's outstanding count above zero forever.
        let observed = compaction(SUPPLY, 0);
        let plan = FractionalClaimCheckCompactionPlanV1::new(observed).expect("plan");
        assert!(!plan.mints_claim_check());
        assert_eq!(plan.escrowed_atoms(), 0);
        assert_eq!(plan.whole_claims(), WHOLE_CLAIMS);
        assert_eq!(plan.validate_post(compaction_post(plan, observed)), Ok(()));

        // And a supply too thin to form one claim likewise records nothing.
        let thin = compaction(DENOMINATOR - 1, PAYOUT_PER_CLAIM);
        let thin_plan = FractionalClaimCheckCompactionPlanV1::new(thin).expect("thin plan");
        assert_eq!(thin_plan.whole_claims(), 0);
        assert!(!thin_plan.mints_claim_check());
    }

    #[test]
    fn funding_a_record_for_nothing_is_refused_in_both_directions() {
        let mut funded = compaction(SUPPLY, 0);
        funded.shared.claim_check_rent =
            rent_exempt_reference_v1(FRACTIONAL_CLAIM_CHECK_BYTES_V1 as u64);
        assert_eq!(
            FractionalClaimCheckCompactionPlanV1::new(funded),
            Err(FractionalClaimCheckConservationErrorV1::Shared(
                ClaimCheckConservationErrorV1::EmptyClaimCheck
            ))
        );

        let mut unfunded = compaction(SUPPLY, PAYOUT_PER_CLAIM);
        unfunded.shared.claim_check_rent = 0;
        assert_eq!(
            FractionalClaimCheckCompactionPlanV1::new(unfunded),
            Err(FractionalClaimCheckConservationErrorV1::Shared(
                ClaimCheckConservationErrorV1::EmptyClaimCheck
            ))
        );
    }

    #[test]
    fn one_burn_pays_exactly_what_on_time_redemption_would_have_paid() {
        // The sentence the census called impossible, as arithmetic. A holder
        // presenting 3_400 shards at a denominator of 1_000 forms three whole
        // claims, burns 3_000 shards, keeps 400, and is paid 3 * 7_500 -- the
        // same number `divide_exposure_shards_v2` and the terminal evaluator
        // would have produced together while the market still existed.
        let observed = redemption(3_400);
        let plan = FractionalClaimCheckRedemptionPlanV1::new(observed).expect("plan");
        assert_eq!(plan.whole_claims(), 3);
        assert_eq!(plan.consumed_shards(), 3_000);
        assert_eq!(plan.change_shards(), 400);
        assert_eq!(plan.collateral_atoms(), 3 * PAYOUT_PER_CLAIM);
        assert_eq!(plan.vault_after(), ESCROWED - 3 * PAYOUT_PER_CLAIM);
        assert!(!plan.settles());
        assert_eq!(plan.validate_post(redemption_post(plan, observed)), Ok(()));
    }

    #[test]
    fn conservation_holds_to_the_atom_across_the_whole_pay_down() {
        // Four holders arriving one at a time, in the wrong order, one of them
        // twice, with dust left behind by two of them. Every burn conserves, and
        // the sum of every payout is exactly the opening escrow.
        let opening = record();
        let mut live = opening;
        let mut vault = ESCROWED;
        let mut supply = SUPPLY;
        let mut paid = 0_u64;
        let mut burned = 0_u64;

        for (presented, expected_claims) in [(5_400_u64, 5_u64), (999, 0), (4_100, 4), (3_000, 3)] {
            let observed = FractionalClaimCheckRedemptionObservationV1 {
                record: live,
                shard_atoms: presented,
                holder_shards_before: presented,
                shard_supply_before: supply,
                vault_before: vault,
                holder_collateral_before: 0,
                record_lamports: 2_000_000,
                holder_lamports_before: 5_000,
            };
            if expected_claims == 0 {
                // Sub-denominator dust is refused, exactly as `NoWholeClaim`
                // refuses it while the market still exists. It is not a zero
                // payout and it does not touch the record.
                assert_eq!(
                    FractionalClaimCheckRedemptionPlanV1::new(observed),
                    Err(FractionalClaimCheckConservationErrorV1::NoWholeClaim)
                );
                continue;
            }
            let plan = FractionalClaimCheckRedemptionPlanV1::new(observed).expect("plan");
            assert_eq!(plan.whole_claims(), expected_claims);
            // Per burn: shards out and collateral out agree through the rate.
            assert_eq!(
                plan.collateral_atoms() * DENOMINATOR,
                plan.consumed_shards() * PAYOUT_PER_CLAIM
            );
            assert_eq!(
                plan.consumed_shards() + plan.change_shards(),
                presented,
                "every atom the holder presented is either burned or returned"
            );
            assert_eq!(plan.validate_post(redemption_post(plan, observed)), Ok(()));

            paid += plan.collateral_atoms();
            burned += plan.consumed_shards();
            vault = plan.vault_after();
            supply -= plan.consumed_shards();

            if plan.settles() {
                assert_eq!(plan.escrowed_after(), 0);
                assert_eq!(vault, 0);
            } else {
                live = FractionalClaimCheckV1 {
                    escrowed_atoms: plan.escrowed_after(),
                    ..live
                };
            }
        }

        // Across the full pay-down: every escrowed atom reached a holder, every
        // burned shard was paid for, and the two agree through the rate.
        assert_eq!(paid, ESCROWED);
        assert_eq!(burned, WHOLE_CLAIMS * DENOMINATOR);
        assert_eq!(paid * DENOMINATOR, burned * PAYOUT_PER_CLAIM);
        assert_eq!(vault, 0);
        // The dust the compaction never escrowed is exactly the dust still
        // outstanding as unredeemable shards.
        assert_eq!(supply, SUPPLY - burned);
        assert_eq!(supply, 345);
        assert!(supply < DENOMINATOR);
    }

    #[test]
    fn the_settling_burn_closes_the_record_and_no_other_burn_moves_a_lamport() {
        let live = FractionalClaimCheckV1 {
            escrowed_atoms: 2 * PAYOUT_PER_CLAIM,
            ..record()
        }
        .new()
        .expect("live");

        let partial = FractionalClaimCheckRedemptionObservationV1 {
            record: live,
            shard_atoms: DENOMINATOR,
            holder_shards_before: DENOMINATOR,
            shard_supply_before: 2 * DENOMINATOR,
            vault_before: 2 * PAYOUT_PER_CLAIM,
            holder_collateral_before: 0,
            record_lamports: 2_000_000,
            holder_lamports_before: 5_000,
        };
        let partial_plan = FractionalClaimCheckRedemptionPlanV1::new(partial).expect("partial");
        assert!(!partial_plan.settles());
        // Not one lamport moves while the record still has to exist.
        assert_eq!(partial_plan.holder_lamports_after(), 5_000);
        assert_eq!(partial_plan.escrowed_after(), PAYOUT_PER_CLAIM);
        assert_eq!(
            partial_plan.validate_post(redemption_post(partial_plan, partial)),
            Ok(())
        );
        // A route that closed a live record is refused by the postcondition.
        let mut closed_early = redemption_post(partial_plan, partial);
        closed_early.record_lamports = 0;
        assert_eq!(
            partial_plan.validate_post(closed_early),
            Err(FractionalClaimCheckConservationErrorV1::PostconditionMismatch)
        );

        let settling = FractionalClaimCheckRedemptionObservationV1 {
            record: FractionalClaimCheckV1 {
                escrowed_atoms: PAYOUT_PER_CLAIM,
                ..live
            },
            vault_before: PAYOUT_PER_CLAIM,
            shard_supply_before: DENOMINATOR,
            ..partial
        };
        let settling_plan = FractionalClaimCheckRedemptionPlanV1::new(settling).expect("settling");
        assert!(settling_plan.settles());
        assert_eq!(settling_plan.escrowed_after(), 0);
        // The rent goes home with whoever finished it.
        assert_eq!(settling_plan.holder_lamports_after(), 5_000 + 2_000_000);
        assert_eq!(
            settling_plan.validate_post(redemption_post(settling_plan, settling)),
            Ok(())
        );
        // A route that left a settled record funded is refused too.
        let mut stranded = redemption_post(settling_plan, settling);
        stranded.record_lamports = 1;
        assert_eq!(
            settling_plan.validate_post(stranded),
            Err(FractionalClaimCheckConservationErrorV1::PostconditionMismatch)
        );
    }

    #[test]
    fn a_burn_that_would_overdraw_the_vault_or_the_record_is_refused() {
        // The escrowed balance is what every other holder is paid out of.
        let mut thin_vault = redemption(3_000);
        thin_vault.vault_before = 3 * PAYOUT_PER_CLAIM - 1;
        assert_eq!(
            FractionalClaimCheckRedemptionPlanV1::new(thin_vault),
            Err(FractionalClaimCheckConservationErrorV1::Overdrawn)
        );

        // A record already paid down below what this holder is claiming.
        let mut thin_record = redemption(3_000);
        thin_record.record = FractionalClaimCheckV1 {
            escrowed_atoms: 2 * PAYOUT_PER_CLAIM,
            ..record()
        };
        thin_record.vault_before = 3 * PAYOUT_PER_CLAIM;
        assert_eq!(
            FractionalClaimCheckRedemptionPlanV1::new(thin_record),
            Err(FractionalClaimCheckConservationErrorV1::ArithmeticOverflow)
        );
    }

    #[test]
    fn shards_the_holder_does_not_hold_are_refused_before_any_arithmetic() {
        let mut overclaim = redemption(3_000);
        overclaim.holder_shards_before = 2_999;
        assert_eq!(
            FractionalClaimCheckRedemptionPlanV1::new(overclaim),
            Err(FractionalClaimCheckConservationErrorV1::ShardConservation)
        );

        let mut empty = redemption(3_000);
        empty.shard_atoms = 0;
        assert_eq!(
            FractionalClaimCheckRedemptionPlanV1::new(empty),
            Err(FractionalClaimCheckConservationErrorV1::ShardConservation)
        );

        // A holder balance larger than the Mint's whole supply means one of the
        // two observations is not of this Mint.
        let mut impossible = redemption(3_000);
        impossible.shard_supply_before = 2_999;
        assert_eq!(
            FractionalClaimCheckRedemptionPlanV1::new(impossible),
            Err(FractionalClaimCheckConservationErrorV1::ShardConservation)
        );
    }

    #[test]
    fn sub_denominator_dust_is_refused_rather_than_paid_zero() {
        // `NoWholeClaim` is a refusal, not a zero payout, before compaction. It
        // must stay one after, or a holder could burn dust for nothing and the
        // route would report success.
        for presented in [1_u64, DENOMINATOR - 1] {
            assert_eq!(
                FractionalClaimCheckRedemptionPlanV1::new(redemption(presented)),
                Err(FractionalClaimCheckConservationErrorV1::NoWholeClaim)
            );
        }
    }

    #[test]
    fn a_transfer_fee_is_tolerated_on_the_holders_side_only() {
        let observed = redemption(3_000);
        let plan = FractionalClaimCheckRedemptionPlanV1::new(observed).expect("plan");
        let good = redemption_post(plan, observed);

        // Less than promised is a fee, and it is the holder's own loss.
        let mut fee = good;
        fee.holder_collateral = observed.holder_collateral_before + plan.collateral_atoms() - 1;
        assert_eq!(plan.validate_post(fee), Ok(()));

        // More than promised is another claimant's collateral.
        let mut windfall = good;
        windfall.holder_collateral =
            observed.holder_collateral_before + plan.collateral_atoms() + 1;
        assert_eq!(
            plan.validate_post(windfall),
            Err(FractionalClaimCheckConservationErrorV1::PostconditionMismatch)
        );

        // The vault's own debit stays exact whatever the holder received.
        for vault_atoms in [plan.vault_after() - 1, plan.vault_after() + 1] {
            let mut moved = good;
            moved.vault_atoms = vault_atoms;
            assert_eq!(
                plan.validate_post(moved),
                Err(FractionalClaimCheckConservationErrorV1::PostconditionMismatch)
            );
        }
    }

    #[test]
    fn a_post_state_that_burned_the_wrong_number_of_shards_is_refused() {
        let observed = redemption(3_400);
        let plan = FractionalClaimCheckRedemptionPlanV1::new(observed).expect("plan");
        let good = redemption_post(plan, observed);
        for mutate in [
            // The change must stay with the holder, unburned.
            |post: &mut FractionalClaimCheckRedemptionPostV1| post.holder_shards = 0,
            // Burning more supply than the holder's own shards would be burning
            // somebody else's.
            |post: &mut FractionalClaimCheckRedemptionPostV1| post.shard_supply -= 1,
            |post: &mut FractionalClaimCheckRedemptionPostV1| post.shard_supply += 1,
            |post: &mut FractionalClaimCheckRedemptionPostV1| post.holder_shards += 1,
            |post: &mut FractionalClaimCheckRedemptionPostV1| post.record_escrowed_atoms += 1,
            |post: &mut FractionalClaimCheckRedemptionPostV1| post.holder_lamports += 1,
        ] {
            let mut post = good;
            mutate(&mut post);
            assert_eq!(
                plan.validate_post(post),
                Err(FractionalClaimCheckConservationErrorV1::PostconditionMismatch)
            );
        }
    }
}
