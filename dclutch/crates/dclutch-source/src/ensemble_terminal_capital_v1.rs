//! Exact native capital for one Ensemble's deferred terminal accounts.
//!
//! The source-material funding row owns this capital. Its `Rent` compartment
//! prepays the member seats and its `Creation` compartment becomes excess
//! lamports on the existing Resolution Source state. The state then pays each
//! terminal certificate and fold receipt when its exclusive branch executes.

use crate::SourceMaterialV3;

/// Refusal from the exact Ensemble terminal-capital calculation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnsembleTerminalCapitalErrorV1 {
    /// A single-source material has no Ensemble terminal-capital row.
    SingleSource,
    /// A required positive chain rent quote was zero.
    ZeroRentQuote,
    /// Exact native-lamport arithmetic overflowed.
    ArithmeticOverflow,
    /// A funding quote did not equal the material-derived requirement.
    FundingMismatch,
    /// The source-material funding row held less native capital than it quoted.
    Underfunded,
}

/// Material-derived native capital for every account one Ensemble can create.
///
/// Member seats can all exist before either terminal branch, so all `k` seats
/// are additive. The fold and failure paths are mutually exclusive under the
/// Source phase machine, so the Source reserve is the larger of those paths:
/// one success certificate plus one fold receipt, or every certificate the
/// bounded failure ladder can leave behind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EnsembleTerminalCapitalPlanV1 {
    member_seat_count: u8,
    maximum_failure_certificate_count: u8,
    member_seat_rent_lamports: u64,
    source_creation_reserve_lamports: u64,
}

impl EnsembleTerminalCapitalPlanV1 {
    /// Derive the exact plan from one authenticated Source material and the
    /// chain's current rent minima for the three fixed account widths.
    pub fn for_material(
        material: SourceMaterialV3,
        member_seat_rent_lamports: u64,
        terminal_certificate_rent_lamports: u64,
        fold_receipt_rent_lamports: u64,
    ) -> Result<Self, EnsembleTerminalCapitalErrorV1> {
        let ensemble = material.ensemble();
        if ensemble.is_single() {
            return Err(EnsembleTerminalCapitalErrorV1::SingleSource);
        }
        if member_seat_rent_lamports == 0
            || terminal_certificate_rent_lamports == 0
            || fold_receipt_rent_lamports == 0
        {
            return Err(EnsembleTerminalCapitalErrorV1::ZeroRentQuote);
        }

        let member_seat_count = ensemble.members();
        let member_seat_rent_lamports = member_seat_rent_lamports
            .checked_mul(u64::from(member_seat_count))
            .ok_or(EnsembleTerminalCapitalErrorV1::ArithmeticOverflow)?;
        let maximum_failure_certificate_count = if material.ensemble_rungs() == 0 {
            1
        } else {
            material
                .ensemble_rungs()
                .checked_add(2)
                .ok_or(EnsembleTerminalCapitalErrorV1::ArithmeticOverflow)?
        };
        let failure_path_lamports = terminal_certificate_rent_lamports
            .checked_mul(u64::from(maximum_failure_certificate_count))
            .ok_or(EnsembleTerminalCapitalErrorV1::ArithmeticOverflow)?;
        let fold_path_lamports = terminal_certificate_rent_lamports
            .checked_add(fold_receipt_rent_lamports)
            .ok_or(EnsembleTerminalCapitalErrorV1::ArithmeticOverflow)?;

        Ok(Self {
            member_seat_count,
            maximum_failure_certificate_count,
            member_seat_rent_lamports,
            source_creation_reserve_lamports: core::cmp::max(
                failure_path_lamports,
                fold_path_lamports,
            ),
        })
    }

    /// Number of member seats prepaid before the Market becomes ready.
    pub const fn member_seat_count(self) -> u8 {
        self.member_seat_count
    }

    /// Most certificates left allocated by the bounded failure branch.
    pub const fn maximum_failure_certificate_count(self) -> u8 {
        self.maximum_failure_certificate_count
    }

    /// Exact `Rent` compartment: all member seats.
    pub const fn member_seat_rent_lamports(self) -> u64 {
        self.member_seat_rent_lamports
    }

    /// Exact `Creation` compartment retained on the Source state.
    pub const fn source_creation_reserve_lamports(self) -> u64 {
        self.source_creation_reserve_lamports
    }

    /// Exact native debit from the source-material funding row at activation.
    pub fn activation_debit_lamports(self) -> Result<u64, EnsembleTerminalCapitalErrorV1> {
        self.member_seat_rent_lamports
            .checked_add(self.source_creation_reserve_lamports)
            .ok_or(EnsembleTerminalCapitalErrorV1::ArithmeticOverflow)
    }

    /// Authenticate the two native compartments authored in the manifest.
    pub const fn authenticate_quote(
        self,
        rent_lamports: u64,
        creation_lamports: u64,
    ) -> Result<(), EnsembleTerminalCapitalErrorV1> {
        if rent_lamports != self.member_seat_rent_lamports
            || creation_lamports != self.source_creation_reserve_lamports
        {
            return Err(EnsembleTerminalCapitalErrorV1::FundingMismatch);
        }
        Ok(())
    }

    /// Debit the exact activation amount from the source-material row.
    ///
    /// The returned remainder plus the two credits always equals `row_before`.
    /// Physical adapters use this before touching an account, so a one-lamport
    /// shortfall cannot partially prepay seats or the Source reserve.
    pub fn debit_funded_row(self, row_before: u64) -> Result<u64, EnsembleTerminalCapitalErrorV1> {
        row_before
            .checked_sub(self.activation_debit_lamports()?)
            .ok_or(EnsembleTerminalCapitalErrorV1::Underfunded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ContentId, EnsembleSpecV1};

    fn id(byte: u8) -> ContentId {
        ContentId::new([byte; 32]).expect("nonzero content id")
    }

    fn material(members: u8, quorum: u8, rungs: u8) -> SourceMaterialV3 {
        SourceMaterialV3::explicitly_unbounded(id(1), id(2), id(3), id(4), Some(id(5)), id(6))
            .with_ensemble(
                EnsembleSpecV1::new(members, quorum).expect("ensemble"),
                rungs,
            )
            .expect("material")
    }

    #[test]
    fn four_member_no_rung_budget_is_exact_and_conserved() {
        let plan = EnsembleTerminalCapitalPlanV1::for_material(
            material(4, 3, 0),
            3_062_400,
            3_062_400,
            2_839_680,
        )
        .expect("plan");
        assert_eq!(plan.member_seat_count(), 4);
        assert_eq!(plan.maximum_failure_certificate_count(), 1);
        assert_eq!(plan.member_seat_rent_lamports(), 12_249_600);
        assert_eq!(plan.source_creation_reserve_lamports(), 5_902_080);
        assert_eq!(plan.activation_debit_lamports(), Ok(18_151_680));
        assert_eq!(plan.debit_funded_row(18_151_687), Ok(7));
        assert_eq!(
            7_u64
                .checked_add(plan.member_seat_rent_lamports())
                .and_then(|value| value.checked_add(plan.source_creation_reserve_lamports())),
            Some(18_151_687),
        );
    }

    #[test]
    fn bounded_ladder_reserves_the_larger_exclusive_path() {
        let plan = EnsembleTerminalCapitalPlanV1::for_material(material(3, 3, 2), 10, 10, 11)
            .expect("plan");
        assert_eq!(plan.maximum_failure_certificate_count(), 4);
        assert_eq!(plan.member_seat_rent_lamports(), 30);
        assert_eq!(plan.source_creation_reserve_lamports(), 40);
        assert_eq!(plan.activation_debit_lamports(), Ok(70));
    }

    #[test]
    fn quote_mismatch_and_one_lamport_shortfall_refuse() {
        let plan = EnsembleTerminalCapitalPlanV1::for_material(material(4, 3, 0), 10, 10, 11)
            .expect("plan");
        assert_eq!(plan.authenticate_quote(40, 21), Ok(()));
        assert_eq!(
            plan.authenticate_quote(39, 21),
            Err(EnsembleTerminalCapitalErrorV1::FundingMismatch),
        );
        assert_eq!(
            plan.debit_funded_row(60),
            Err(EnsembleTerminalCapitalErrorV1::Underfunded),
        );
        assert_eq!(plan.debit_funded_row(61), Ok(0));
    }

    #[test]
    fn invalid_or_unrepresentable_rent_inputs_refuse() {
        let single =
            SourceMaterialV3::explicitly_unbounded(id(1), id(2), id(3), id(4), None, id(6));
        assert_eq!(
            EnsembleTerminalCapitalPlanV1::for_material(single, 1, 1, 1),
            Err(EnsembleTerminalCapitalErrorV1::SingleSource),
        );
        assert_eq!(
            EnsembleTerminalCapitalPlanV1::for_material(material(4, 3, 0), 0, 1, 1),
            Err(EnsembleTerminalCapitalErrorV1::ZeroRentQuote),
        );
        assert_eq!(
            EnsembleTerminalCapitalPlanV1::for_material(material(4, 3, 0), u64::MAX, 1, 1,),
            Err(EnsembleTerminalCapitalErrorV1::ArithmeticOverflow),
        );
    }
}
