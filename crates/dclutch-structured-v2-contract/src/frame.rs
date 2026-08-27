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

    /// Exact Token effect count for this frame: one receipt plus one per
    /// backed coordinate.
    ///
    /// Restated from the backed width rather than passed in, so the effect
    /// sequence and the account frame cannot be sized by two different facts.
    pub fn effect_count(self) -> Result<usize> {
        self.backed_coordinates
            .checked_add(1)
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

/// Exact frame coordinates of the five accounts one Token effect can name.
///
/// This is the join that makes the frame load-bearing rather than descriptive.
/// `StructuredHotCandidateV2::prepare` checks that an effect's accounts do not
/// alias and that its authority is exactly the root or the actor; it cannot
/// check that they sit where the frame says, because the coordinates reach it
/// already chosen.  Deriving them here means the operator that BUILDS the
/// instruction and the adapter that PARSES it read the same assignment, so the
/// only way to disagree with the frame is to stop calling this function.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredFrameEffectSlotsV2 {
    /// Terms-selected Token program.
    pub token_program: u16,
    /// Receipt Mint, or the shard Mint of this effect's backed coordinate.
    pub mint: u16,
    /// Source account, absent exactly when the effect has none.
    pub source: Option<u16>,
    /// Destination account, absent exactly when the effect has none.
    pub destination: Option<u16>,
    /// Signing authority: the root, or the actor for a lock transfer.
    pub authority: u16,
}

/// Exact frame coordinates for one effect of one action's canonical sequence.
///
/// `effect_index` walks the same sequence `StructuredHotCandidateV2::prepare`
/// requires: the receipt effect FIRST for the three supply-changing actions and
/// LAST for retirement, so the custody-closure sweep runs before the Mint
/// closes; the backed coordinates strictly ascending in between.
pub fn structured_frame_effect_slots_v2(
    spec: StructuredFrameSpecV2,
    action: StructuredActionV2,
    effect_index: usize,
) -> Result<StructuredFrameEffectSlotsV2> {
    let retiring = matches!(action, StructuredActionV2::ZeroSupplyRetire);
    if effect_index >= spec.effect_count()? {
        return Err(StructuredFrameErrorV2::InvalidCoordinate);
    }
    // The receipt slot is the only index that is not a backed coordinate, so
    // the shard index is the walk position with the receipt slot removed.
    let receipt_slot = if retiring {
        spec.backed_coordinates()
    } else {
        0
    };
    let token_program = narrow(STRUCTURED_ACCOUNT_TOKEN_PROGRAM_V2)?;
    if effect_index == receipt_slot {
        let (source, destination, authority) = match action {
            StructuredActionV2::Issue => (
                None,
                Some(narrow(STRUCTURED_ACCOUNT_RECEIPT_TOKEN_V2)?),
                narrow(STRUCTURED_ACCOUNT_ROOT_V2)?,
            ),
            StructuredActionV2::Unwrap | StructuredActionV2::TerminalRedeem => (
                Some(narrow(STRUCTURED_ACCOUNT_RECEIPT_TOKEN_V2)?),
                None,
                narrow(STRUCTURED_ACCOUNT_ROOT_V2)?,
            ),
            // Retirement pays the Mint's recovered rent to the lifecycle
            // RentCredit, which is the only action that admits it at all.
            StructuredActionV2::ZeroSupplyRetire => (
                None,
                Some(narrow(STRUCTURED_ACCOUNT_RENT_CREDIT_V2)?),
                narrow(STRUCTURED_ACCOUNT_ROOT_V2)?,
            ),
        };
        return Ok(StructuredFrameEffectSlotsV2 {
            token_program,
            mint: narrow(STRUCTURED_ACCOUNT_RECEIPT_MINT_V2)?,
            source,
            destination,
            authority,
        });
    }
    let backed_index = if retiring {
        effect_index
    } else {
        effect_index
            .checked_sub(1)
            .ok_or(StructuredFrameErrorV2::Arithmetic)?
    };
    let actor_shard = narrow(spec.actor_shard(backed_index)?)?;
    let custody_shard = narrow(spec.custody_shard(backed_index)?)?;
    let (source, destination, authority) = match action {
        // The actor signs the lock: the basket leaves an account the root has
        // no authority over, so the transfer cannot be root-authorized.
        StructuredActionV2::Issue => (
            Some(actor_shard),
            Some(custody_shard),
            narrow(STRUCTURED_ACCOUNT_ACTOR_V2)?,
        ),
        StructuredActionV2::Unwrap | StructuredActionV2::TerminalRedeem => (
            Some(custody_shard),
            Some(actor_shard),
            narrow(STRUCTURED_ACCOUNT_ROOT_V2)?,
        ),
        StructuredActionV2::ZeroSupplyRetire => (
            Some(custody_shard),
            Some(narrow(STRUCTURED_ACCOUNT_RENT_CREDIT_V2)?),
            narrow(STRUCTURED_ACCOUNT_ROOT_V2)?,
        ),
    };
    Ok(StructuredFrameEffectSlotsV2 {
        token_program,
        mint: narrow(spec.shard_mint(backed_index)?)?,
        source,
        destination,
        authority,
    })
}

fn narrow(index: usize) -> Result<u16> {
    u16::try_from(index).map_err(|_| StructuredFrameErrorV2::Arithmetic)
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

    const EVERY_ACTION: [StructuredActionV2; 4] = [
        StructuredActionV2::Issue,
        StructuredActionV2::Unwrap,
        StructuredActionV2::TerminalRedeem,
        StructuredActionV2::ZeroSupplyRetire,
    ];

    fn slots(
        action: StructuredActionV2,
        backed: usize,
        index: usize,
    ) -> StructuredFrameEffectSlotsV2 {
        structured_frame_effect_slots_v2(
            StructuredFrameSpecV2::new(backed).expect("spec"),
            action,
            index,
        )
        .expect("slots")
    }

    #[test]
    fn the_receipt_effect_is_first_except_for_retirement_where_it_is_last() {
        let receipt_mint = u16::try_from(STRUCTURED_ACCOUNT_RECEIPT_MINT_V2).expect("mint");
        for action in EVERY_ACTION {
            let retiring = matches!(action, StructuredActionV2::ZeroSupplyRetire);
            let receipt_slot = if retiring { 3 } else { 0 };
            for index in 0..4 {
                let slot = slots(action, 3, index);
                assert_eq!(
                    slot.mint == receipt_mint,
                    index == receipt_slot,
                    "{action:?} effect {index} named the wrong Mint kind"
                );
            }
        }
    }

    #[test]
    fn the_shard_walk_ascends_the_triples_in_lockstep_with_the_effect_sequence() {
        for action in EVERY_ACTION {
            let retiring = matches!(action, StructuredActionV2::ZeroSupplyRetire);
            let spec = StructuredFrameSpecV2::new(4).expect("spec");
            let mut previous: Option<u16> = None;
            for index in 0..spec.effect_count().expect("count") {
                if index == if retiring { 4 } else { 0 } {
                    continue;
                }
                let slot = structured_frame_effect_slots_v2(spec, action, index).expect("slots");
                // Strictly ascending Mint coordinates is exactly what
                // `StructuredHotCandidateV2::prepare` requires of the sweep.
                if let Some(previous) = previous {
                    assert!(
                        slot.mint > previous,
                        "{action:?} effect {index} did not ascend"
                    );
                }
                previous = Some(slot.mint);
                let backed_index = if retiring { index } else { index - 1 };
                assert_eq!(
                    slot.mint,
                    u16::try_from(spec.shard_mint(backed_index).expect("mint")).expect("narrow")
                );
            }
            assert!(previous.is_some());
        }
    }

    #[test]
    fn no_effect_ever_names_one_account_twice() {
        // The alias check inside `StructuredHotCandidateV2::prepare` refuses a
        // repeated coordinate.  The frame must never PRODUCE one, or the two
        // authors would disagree only at runtime.
        for action in EVERY_ACTION {
            for backed in 1..6 {
                for index in 0..=backed {
                    let slot = slots(action, backed, index);
                    let named = [
                        Some(slot.token_program),
                        Some(slot.mint),
                        slot.source,
                        slot.destination,
                        Some(slot.authority),
                    ];
                    for (left, value) in named.iter().enumerate() {
                        let Some(value) = value else { continue };
                        for other in named.iter().skip(left + 1).flatten() {
                            assert_ne!(
                                value, other,
                                "{action:?} backed={backed} effect {index} aliased a coordinate"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn only_a_lock_is_actor_authorized_and_only_retirement_pays_rent_credit() {
        let actor = u16::try_from(STRUCTURED_ACCOUNT_ACTOR_V2).expect("actor");
        let root = u16::try_from(STRUCTURED_ACCOUNT_ROOT_V2).expect("root");
        let rent_credit = u16::try_from(STRUCTURED_ACCOUNT_RENT_CREDIT_V2).expect("rent");
        for action in EVERY_ACTION {
            for index in 0..3 {
                let slot = slots(action, 2, index);
                let locking = matches!(action, StructuredActionV2::Issue) && index != 0;
                assert_eq!(slot.authority, if locking { actor } else { root });
                // A RentCredit destination is admissible for exactly the one
                // action whose frame declares that coordinate active.
                if slot.destination == Some(rent_credit) {
                    assert!(structured_account_is_active_v2(
                        action,
                        STRUCTURED_ACCOUNT_RENT_CREDIT_V2
                    ));
                }
            }
        }
    }

    #[test]
    fn retirement_closes_every_custody_account_and_the_mint_last() {
        let rent_credit = u16::try_from(STRUCTURED_ACCOUNT_RENT_CREDIT_V2).expect("rent");
        let spec = StructuredFrameSpecV2::new(3).expect("spec");
        for index in 0..3 {
            let slot =
                structured_frame_effect_slots_v2(spec, StructuredActionV2::ZeroSupplyRetire, index)
                    .expect("slots");
            assert_eq!(
                slot.source,
                Some(u16::try_from(spec.custody_shard(index).expect("custody")).expect("narrow"))
            );
            assert_eq!(slot.destination, Some(rent_credit));
        }
        let mint_close =
            structured_frame_effect_slots_v2(spec, StructuredActionV2::ZeroSupplyRetire, 3)
                .expect("slots");
        assert_eq!(mint_close.source, None);
        assert_eq!(mint_close.destination, Some(rent_credit));
    }

    #[test]
    fn an_effect_index_past_the_sequence_refuses() {
        for action in EVERY_ACTION {
            let spec = StructuredFrameSpecV2::new(2).expect("spec");
            assert_eq!(spec.effect_count(), Ok(3));
            assert!(structured_frame_effect_slots_v2(spec, action, 2).is_ok());
            assert_eq!(
                structured_frame_effect_slots_v2(spec, action, 3),
                Err(StructuredFrameErrorV2::InvalidCoordinate)
            );
            assert_eq!(
                structured_frame_effect_slots_v2(spec, action, usize::MAX),
                Err(StructuredFrameErrorV2::InvalidCoordinate)
            );
        }
    }

    #[test]
    fn issue_moves_the_basket_toward_custody_and_unwrap_moves_it_back() {
        let spec = StructuredFrameSpecV2::new(2).expect("spec");
        for index in 1..3 {
            let backed = index - 1;
            let actor = u16::try_from(spec.actor_shard(backed).expect("actor")).expect("narrow");
            let custody =
                u16::try_from(spec.custody_shard(backed).expect("custody")).expect("narrow");
            let issue = structured_frame_effect_slots_v2(spec, StructuredActionV2::Issue, index)
                .expect("issue");
            assert_eq!(
                (issue.source, issue.destination),
                (Some(actor), Some(custody))
            );
            for release in [
                StructuredActionV2::Unwrap,
                StructuredActionV2::TerminalRedeem,
            ] {
                let slot = structured_frame_effect_slots_v2(spec, release, index).expect("release");
                assert_eq!(
                    (slot.source, slot.destination),
                    (Some(custody), Some(actor))
                );
            }
        }
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
