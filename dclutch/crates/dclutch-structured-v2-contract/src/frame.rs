//! The canonical Structured V2 physical account frame.
//!
//! One author for two readers: the host operator that BUILDS the instruction
//! and the onchain adapter that PARSES it.  Neither carries a per-action
//! account table of its own, so the two cannot drift.
//!
//! The frame is a fixed base followed by one triple per BACKED representation
//! coordinate — the same coordinates, in the same strictly ascending Mint
//! order, that `StructuredHotCandidateV2::prepare` requires of the Token effect
//! sequence.  Zero-coefficient coordinates contribute no accounts at all: a row
//! that moves nothing has nothing to name, and admitting one would let a caller
//! pad the frame with accounts no effect ever touches.

use crate::StructuredActionV2;

/// Release-pinned caller authority; signer for every action.
pub const STRUCTURED_ACCOUNT_CALLER_AUTHORITY_V2: usize = 0;
/// Caller execution-role Program.
pub const STRUCTURED_ACCOUNT_CALLER_PROGRAM_V2: usize = 1;
/// Caller execution-role ProgramData.
pub const STRUCTURED_ACCOUNT_CALLER_PROGRAMDATA_V2: usize = 2;
/// Actor owning the shard sources and the receipt account; signer when active.
pub const STRUCTURED_ACCOUNT_ACTOR_V2: usize = 3;
/// Structured root: replay record, receipt Mint authority, custody owner.
pub const STRUCTURED_ACCOUNT_ROOT_V2: usize = 4;
/// Finalized Record holding the immutable Structured V2 terms bytes.
pub const STRUCTURED_ACCOUNT_TERMS_RAW_V2: usize = 5;
/// Vacant staging cursor proving the terms Record is finalized.
pub const STRUCTURED_ACCOUNT_TERMS_STAGING_V2: usize = 6;
/// Finalized Record holding the immutable exact claim-shard terms bytes.
pub const STRUCTURED_ACCOUNT_SHARD_TERMS_RAW_V2: usize = 7;
/// Vacant staging cursor proving the shard-terms Record is finalized.
pub const STRUCTURED_ACCOUNT_SHARD_TERMS_STAGING_V2: usize = 8;
/// Logical Core Market state.
pub const STRUCTURED_ACCOUNT_CORE_MARKET_V2: usize = 9;
/// Core execution-role Program.
pub const STRUCTURED_ACCOUNT_CORE_PROGRAM_V2: usize = 10;
/// Core execution-role ProgramData.
pub const STRUCTURED_ACCOUNT_CORE_PROGRAMDATA_V2: usize = 11;
/// Claims execution-role Program (the executing role's own identity).
pub const STRUCTURED_ACCOUNT_CLAIMS_PROGRAM_V2: usize = 12;
/// Claims execution-role ProgramData.
pub const STRUCTURED_ACCOUNT_CLAIMS_PROGRAMDATA_V2: usize = 13;
/// Registry Program owning the activation cache and the Record PDAs.
pub const STRUCTURED_ACCOUNT_REGISTRY_PROGRAM_V2: usize = 14;
/// Registry-owned per-Market activation cache.
pub const STRUCTURED_ACCOUNT_ACTIVATION_CACHE_V2: usize = 15;
/// Terms-selected Token program.
pub const STRUCTURED_ACCOUNT_TOKEN_PROGRAM_V2: usize = 16;
/// Structured receipt Mint.
pub const STRUCTURED_ACCOUNT_RECEIPT_MINT_V2: usize = 17;
/// Actor-side receipt Token account: destination on issue, source on burn.
pub const STRUCTURED_ACCOUNT_RECEIPT_TOKEN_V2: usize = 18;
/// Root-bound lifecycle RentCredit; active only for zero-supply retirement.
pub const STRUCTURED_ACCOUNT_RENT_CREDIT_V2: usize = 19;
/// Rent execution-role Program owning the lifecycle RentCredit.
pub const STRUCTURED_ACCOUNT_RENT_PROGRAM_V2: usize = 20;
/// Rent sysvar, for the root's own exemption floor.
pub const STRUCTURED_ACCOUNT_RENT_SYSVAR_V2: usize = 21;
/// System Program, for root allocation.
pub const STRUCTURED_ACCOUNT_SYSTEM_PROGRAM_V2: usize = 22;

/// Exact fixed account count preceding the per-coordinate triples.
pub const STRUCTURED_BASE_ACCOUNT_COUNT_V2: usize = 23;

/// Shard Mint of one backed coordinate, relative to its triple base.
pub const STRUCTURED_ASSET_SHARD_MINT_V2: usize = 0;
/// Actor-side shard Token account of one backed coordinate.
pub const STRUCTURED_ASSET_ACTOR_SHARD_V2: usize = 1;
/// Structured shard custody Token account of one backed coordinate.
pub const STRUCTURED_ASSET_CUSTODY_SHARD_V2: usize = 2;

/// Exact account count contributed by one backed representation coordinate.
pub const STRUCTURED_ASSET_ACCOUNT_COUNT_V2: usize = 3;

/// Stable Structured V2 frame refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StructuredFrameErrorV2 {
    /// The backed-coordinate count was zero, over the capacity profile, or
    /// disagreed with the supplied account count.
    InvalidWidth,
    /// A coordinate index was outside the declared backed width.
    InvalidCoordinate,
    /// Checked account arithmetic overflowed.
    Arithmetic,
}

/// Result alias for Structured V2 frame arithmetic.
pub type Result<T> = core::result::Result<T, StructuredFrameErrorV2>;

/// The exact account frame one Structured V2 action expands to.
///
/// Constructed from the BACKED coordinate count — the number of coordinates
/// whose coefficient is nonzero — which is also the Token effect count minus
/// the single receipt effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredFrameSpecV2 {
    backed_coordinates: usize,
}

impl StructuredFrameSpecV2 {
    /// Admit one frame for a nonzero backed width inside the capacity profile.
    ///
    /// The bound is `STRUCTURED_HOT_MAX_TOKEN_EFFECTS_V2 - 1`, restated from
    /// the effect bound rather than written again, so widening the capacity
    /// profile cannot leave a second stale limit behind.
    pub fn new(backed_coordinates: usize) -> Result<Self> {
        if backed_coordinates == 0
            || backed_coordinates > crate::STRUCTURED_HOT_MAX_TOKEN_EFFECTS_V2 - 1
        {
            return Err(StructuredFrameErrorV2::InvalidWidth);
        }
        Ok(Self { backed_coordinates })
    }

    /// Admit one frame by joining a declared backed width to an observed count.
    pub fn from_account_count(backed_coordinates: usize, accounts: usize) -> Result<Self> {
        let spec = Self::new(backed_coordinates)?;
        if spec.account_count()? != accounts {
            return Err(StructuredFrameErrorV2::InvalidWidth);
        }
        Ok(spec)
    }

    /// Exact number of backed representation coordinates.
    pub const fn backed_coordinates(self) -> usize {
        self.backed_coordinates
    }

    /// Exact total account count for this frame.
    pub fn account_count(self) -> Result<usize> {
        self.backed_coordinates
            .checked_mul(STRUCTURED_ASSET_ACCOUNT_COUNT_V2)
            .and_then(|assets| assets.checked_add(STRUCTURED_BASE_ACCOUNT_COUNT_V2))
            .ok_or(StructuredFrameErrorV2::Arithmetic)
    }

    /// First account index of one backed coordinate's triple.
    pub fn asset_base(self, backed_index: usize) -> Result<usize> {
        if backed_index >= self.backed_coordinates {
            return Err(StructuredFrameErrorV2::InvalidCoordinate);
        }
        backed_index
            .checked_mul(STRUCTURED_ASSET_ACCOUNT_COUNT_V2)
            .and_then(|offset| offset.checked_add(STRUCTURED_BASE_ACCOUNT_COUNT_V2))
            .ok_or(StructuredFrameErrorV2::Arithmetic)
    }

    /// Absolute account index of one backed coordinate's shard Mint.
    pub fn shard_mint(self, backed_index: usize) -> Result<usize> {
        self.asset_slot(backed_index, STRUCTURED_ASSET_SHARD_MINT_V2)
    }

    /// Absolute account index of one backed coordinate's actor shard account.
    pub fn actor_shard(self, backed_index: usize) -> Result<usize> {
        self.asset_slot(backed_index, STRUCTURED_ASSET_ACTOR_SHARD_V2)
    }

    /// Absolute account index of one backed coordinate's custody account.
    pub fn custody_shard(self, backed_index: usize) -> Result<usize> {
        self.asset_slot(backed_index, STRUCTURED_ASSET_CUSTODY_SHARD_V2)
    }

    fn asset_slot(self, backed_index: usize, slot: usize) -> Result<usize> {
        self.asset_base(backed_index)?
            .checked_add(slot)
            .ok_or(StructuredFrameErrorV2::Arithmetic)
    }
}

/// Whether one base coordinate is ACTIVE for one action.
///
/// An inactive coordinate is still present — the frame width is a function of
/// the backed width alone, never of the action — but the adapter requires it to
/// be the canonical inert account rather than a live one, so a caller cannot
/// smuggle a writable RentCredit into an issue or a receipt source into a
/// retirement.
pub const fn structured_account_is_active_v2(action: StructuredActionV2, index: usize) -> bool {
    match index {
        STRUCTURED_ACCOUNT_ACTOR_V2 => !matches!(action, StructuredActionV2::ZeroSupplyRetire),
        STRUCTURED_ACCOUNT_RECEIPT_TOKEN_V2 => {
            !matches!(action, StructuredActionV2::ZeroSupplyRetire)
        }
        STRUCTURED_ACCOUNT_RENT_CREDIT_V2 | STRUCTURED_ACCOUNT_RENT_PROGRAM_V2 => {
            matches!(action, StructuredActionV2::ZeroSupplyRetire)
        }
        _ => true,
    }
}

/// Whether one base coordinate must arrive WRITABLE for one action.
pub const fn structured_account_is_writable_v2(action: StructuredActionV2, index: usize) -> bool {
    match index {
        STRUCTURED_ACCOUNT_ROOT_V2 | STRUCTURED_ACCOUNT_RECEIPT_MINT_V2 => true,
        STRUCTURED_ACCOUNT_RECEIPT_TOKEN_V2 => {
            !matches!(action, StructuredActionV2::ZeroSupplyRetire)
        }
        STRUCTURED_ACCOUNT_RENT_CREDIT_V2 => matches!(action, StructuredActionV2::ZeroSupplyRetire),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_frame_width_is_a_function_of_the_backed_width_alone() {
        let spec = StructuredFrameSpecV2::new(3).expect("spec");
        assert_eq!(
            spec.account_count(),
            Ok(STRUCTURED_BASE_ACCOUNT_COUNT_V2 + 9)
        );
        assert_eq!(spec.backed_coordinates(), 3);
        assert_eq!(
            StructuredFrameSpecV2::new(1).expect("one").account_count(),
            Ok(STRUCTURED_BASE_ACCOUNT_COUNT_V2 + 3)
        );
    }

    #[test]
    fn the_triples_are_contiguous_ascending_and_never_alias_the_base() {
        let spec = StructuredFrameSpecV2::new(4).expect("spec");
        let mut previous = STRUCTURED_BASE_ACCOUNT_COUNT_V2;
        for index in 0..4 {
            let mint = spec.shard_mint(index).expect("mint");
            let actor = spec.actor_shard(index).expect("actor");
            let custody = spec.custody_shard(index).expect("custody");
            assert_eq!(mint, previous);
            assert_eq!(actor, mint + 1);
            assert_eq!(custody, mint + 2);
            assert!(mint >= STRUCTURED_BASE_ACCOUNT_COUNT_V2);
            previous = custody + 1;
        }
        assert_eq!(previous, spec.account_count().expect("count"));
    }

    #[test]
    fn a_coordinate_past_the_backed_width_refuses() {
        let spec = StructuredFrameSpecV2::new(2).expect("spec");
        assert_eq!(
            spec.shard_mint(2),
            Err(StructuredFrameErrorV2::InvalidCoordinate)
        );
        assert_eq!(
            spec.asset_base(usize::MAX),
            Err(StructuredFrameErrorV2::InvalidCoordinate)
        );
    }

    #[test]
    fn zero_width_and_over_capacity_widths_refuse() {
        assert_eq!(
            StructuredFrameSpecV2::new(0),
            Err(StructuredFrameErrorV2::InvalidWidth)
        );
        let capacity = crate::STRUCTURED_HOT_MAX_TOKEN_EFFECTS_V2 - 1;
        assert!(StructuredFrameSpecV2::new(capacity).is_ok());
        assert_eq!(
            StructuredFrameSpecV2::new(capacity + 1),
            Err(StructuredFrameErrorV2::InvalidWidth)
        );
    }

    #[test]
    fn a_padded_or_truncated_account_list_refuses_the_join() {
        let exact = StructuredFrameSpecV2::new(2)
            .expect("spec")
            .account_count()
            .expect("count");
        assert!(StructuredFrameSpecV2::from_account_count(2, exact).is_ok());
        assert_eq!(
            StructuredFrameSpecV2::from_account_count(2, exact + 1),
            Err(StructuredFrameErrorV2::InvalidWidth)
        );
        assert_eq!(
            StructuredFrameSpecV2::from_account_count(2, exact - 1),
            Err(StructuredFrameErrorV2::InvalidWidth)
        );
        // A declared width that would produce the same count as another width
        // does not exist -- the stride is three -- but a substituted width is
        // still refused because the count is derived, never trusted.
        assert_eq!(
            StructuredFrameSpecV2::from_account_count(3, exact),
            Err(StructuredFrameErrorV2::InvalidWidth)
        );
    }

    #[test]
    fn activity_and_writability_split_retirement_from_the_supply_actions() {
        for action in [
            StructuredActionV2::Issue,
            StructuredActionV2::Unwrap,
            StructuredActionV2::TerminalRedeem,
        ] {
            assert!(structured_account_is_active_v2(
                action,
                STRUCTURED_ACCOUNT_RECEIPT_TOKEN_V2
            ));
            assert!(structured_account_is_writable_v2(
                action,
                STRUCTURED_ACCOUNT_RECEIPT_TOKEN_V2
            ));
            assert!(!structured_account_is_active_v2(
                action,
                STRUCTURED_ACCOUNT_RENT_CREDIT_V2
            ));
            assert!(!structured_account_is_writable_v2(
                action,
                STRUCTURED_ACCOUNT_RENT_CREDIT_V2
            ));
            assert!(structured_account_is_active_v2(
                action,
                STRUCTURED_ACCOUNT_ACTOR_V2
            ));
        }
        let retire = StructuredActionV2::ZeroSupplyRetire;
        assert!(!structured_account_is_active_v2(
            retire,
            STRUCTURED_ACCOUNT_RECEIPT_TOKEN_V2
        ));
        assert!(!structured_account_is_active_v2(
            retire,
            STRUCTURED_ACCOUNT_ACTOR_V2
        ));
        assert!(structured_account_is_active_v2(
            retire,
            STRUCTURED_ACCOUNT_RENT_CREDIT_V2
        ));
        assert!(structured_account_is_writable_v2(
            retire,
            STRUCTURED_ACCOUNT_RENT_CREDIT_V2
        ));
    }

    #[test]
    fn the_root_and_the_receipt_mint_are_writable_for_every_action() {
        for action in [
            StructuredActionV2::Issue,
            StructuredActionV2::Unwrap,
            StructuredActionV2::TerminalRedeem,
            StructuredActionV2::ZeroSupplyRetire,
        ] {
            assert!(structured_account_is_writable_v2(
                action,
                STRUCTURED_ACCOUNT_ROOT_V2
            ));
            assert!(structured_account_is_writable_v2(
                action,
                STRUCTURED_ACCOUNT_RECEIPT_MINT_V2
            ));
            for readonly in [
                STRUCTURED_ACCOUNT_TERMS_RAW_V2,
                STRUCTURED_ACCOUNT_SHARD_TERMS_RAW_V2,
                STRUCTURED_ACCOUNT_CORE_MARKET_V2,
                STRUCTURED_ACCOUNT_TOKEN_PROGRAM_V2,
                STRUCTURED_ACCOUNT_ACTIVATION_CACHE_V2,
            ] {
                assert!(!structured_account_is_writable_v2(action, readonly));
            }
        }
    }
}
