//! Refusals and route admission for fractional claim-checks.
//!
//! [`crate::claim_check_compaction_v1`] compacts a sleeping *person's* Position
//! into a record only that person can open, and refuses every owner kind that
//! cannot sign for its own payout. That refusal is correct and is not relaxed
//! here. What this module adds is the second destination the refusal implies:
//! a Position whose owner cannot sign, but whose claimants hold an instrument,
//! goes to a record addressed by the instrument instead of by a payee.
//!
//! So the two gates are complementary rather than competing, and the point of
//! stating them next to each other is that every owner kind now has to answer
//! *which* claim-check it gets, rather than only whether it gets the native one.
//! A fourth owner kind is a compile error in [`claim_check_route_for`] and not a
//! silent inheritance of whichever arm it was written beside -- C0's lesson from
//! the compaction design's §15.5, applied a third time.
//!
//! # The refusal a shard burn cannot get around
//!
//! Worth stating where the codes live, because it is the fact that shapes the
//! whole fractional route and it is not visible from the design.
//!
//! A shard Mint carries Token-2022's `PermissionedBurn` extension, pinned by
//! `Token2022BehaviorProfileV2::read_mint` to the Mint's controller. In the
//! Fractional family that controller is the capability root: a **Trading**-
//! derived PDA. Token-2022 refuses a *standard* burn outright while that
//! extension is present, and its permissioned burn demands the configured
//! authority as a second signer. So no signature a shard holder can produce will
//! ever burn a shard on its own, and Claims cannot produce the other one.
//!
//! Compaction therefore has to re-point the Mint's burn authority to the escrow
//! -- a PDA Claims *can* sign -- while the root is still alive to authorize it.
//! That is why `Authority` and `ShardMint` are separate codes below: a route
//! that reached the burn without having done the re-point fails for a reason
//! that is worth telling apart from a frame error.

use dclutch_claims_svm::protocol_position_v2::ProtocolPositionOwnerKindV2;
use solana_program::program_error::ProgramError;

/// Stable fractional claim-check compaction refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum FractionalClaimCheckCompactionSbfErrorV1 {
    /// The fixed account frame, ownership, or writability refused.
    Accounts = 0x5640,
    /// A signer the route does not admit was present, or a required one absent.
    ///
    /// The Fractional capability root is a required signer here and only here:
    /// re-pointing the shard Mint's burn authority needs the authority it is
    /// being moved away from, and after this instruction nobody can produce it.
    Authority = 0x5641,
    /// Coordinates did not derive the passed account, or aliased, or were zero.
    Identity = 0x5642,
    /// The compaction deadline had not elapsed at the observed slot.
    Deadline = 0x5643,
    /// The Core phase, or the absence of a terminal receipt, refused.
    Phase = 0x5644,
    /// The fractional claim-check address was already occupied.
    AlreadyCompacted = 0x5645,
    /// A plan's atoms, shards or lamports did not balance.
    Conservation = 0x5646,
    /// The terminal payout derivation refused.
    Economic = 0x5647,
    /// Observed post-balances did not match the admitted plan.
    Receipt = 0x5648,
    /// The escrow was absent, or its mint or token program did not match.
    Escrow = 0x5649,
    /// A position kind this route does not fractionally compact.
    Scope = 0x564A,
    /// The finalized exposure terms, or the coordinate they declare, refused.
    Terms = 0x564B,
    /// The shard Mint's profile, supply, or burn-authority hand-off refused.
    ShardMint = 0x564C,
}

// Registered refusal band (`docs/decisions/0007-namespaced-refusal-codes.md`).
// The discriminants stay literal so a code seen in a validator log is greppable;
// these assertions are what stops them drifting out of the allocated band.
const _: () = assert!(
    FractionalClaimCheckCompactionSbfErrorV1::Accounts as u32
        == dclutch_refusal_registry::CLAIMS_REFUSAL_BASE + 0x640,
    "FractionalClaimCheckCompactionSbfErrorV1 must start at its registered refusal band base"
);
const _: () = assert!(
    (FractionalClaimCheckCompactionSbfErrorV1::ShardMint as u32)
        < dclutch_refusal_registry::CLAIMS_REFUSAL_BASE + dclutch_refusal_registry::BAND_SPAN,
    "FractionalClaimCheckCompactionSbfErrorV1 must not run past its registered refusal band"
);
// Four sub-bands now: native compaction 0x600, native redemption 0x620,
// fractional compaction 0x640, fractional redemption 0x660. Each is
// independently versioned, so none may grow into the next. These assertions are
// what would catch it.
const _: () = assert!(
    (crate::claim_check_redemption_v1::ClaimCheckRedemptionSbfErrorV1::Vault as u32)
        < FractionalClaimCheckCompactionSbfErrorV1::Accounts as u32,
    "the native redemption sub-band must not run into fractional compaction"
);

impl From<FractionalClaimCheckCompactionSbfErrorV1> for ProgramError {
    fn from(value: FractionalClaimCheckCompactionSbfErrorV1) -> Self {
        Self::Custom(value as u32)
    }
}

/// Stable fractional claim-check redemption refusal.
///
/// Short, and the shortness is the feature: what a shard holder touches when
/// they finally come back is a market that no longer exists.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum FractionalClaimCheckRedemptionSbfErrorV1 {
    /// The fixed account frame, ownership, or writability refused.
    Accounts = 0x5660,
    /// The signer was not the presented shard account's own owner.
    ///
    /// Not "was not the record's holder": this record names no holder. The only
    /// authority a redemption proves is over the shards being burned.
    Authority = 0x5661,
    /// The record was not at its derived address, or a mint did not match.
    Identity = 0x5662,
    /// The vault debit, the shard burn, or the pay-down did not balance.
    Conservation = 0x5663,
    /// Observed post-balances did not match the admitted plan.
    Receipt = 0x5664,
    /// The shard balance presented forms no whole Claims coordinate.
    ///
    /// Its own code, and not folded into `Conservation`, because it is the one
    /// refusal an honest holder can hit: sub-denominator dust was never a claim
    /// on collateral and does not become one here. A holder seeing this needs to
    /// be told to consolidate, not to suspect the escrow.
    NoWholeClaim = 0x5665,
    /// An escrow close was attempted while fractional claim-checks were live.
    Vault = 0x5666,
}

const _: () = assert!(
    FractionalClaimCheckRedemptionSbfErrorV1::Accounts as u32
        == dclutch_refusal_registry::CLAIMS_REFUSAL_BASE + 0x660,
    "FractionalClaimCheckRedemptionSbfErrorV1 must start at its registered refusal band base"
);
const _: () = assert!(
    (FractionalClaimCheckRedemptionSbfErrorV1::Vault as u32)
        < dclutch_refusal_registry::CLAIMS_REFUSAL_BASE + dclutch_refusal_registry::BAND_SPAN,
    "FractionalClaimCheckRedemptionSbfErrorV1 must not run past its registered refusal band"
);
const _: () = assert!(
    (FractionalClaimCheckCompactionSbfErrorV1::ShardMint as u32)
        < FractionalClaimCheckRedemptionSbfErrorV1::Accounts as u32,
    "the fractional compaction sub-band must not run into fractional redemption"
);

impl From<FractionalClaimCheckRedemptionSbfErrorV1> for ProgramError {
    fn from(value: FractionalClaimCheckRedemptionSbfErrorV1) -> Self {
        Self::Custom(value as u32)
    }
}

/// Which claim-check, if any, a Position's owner kind is entitled to.
///
/// The question the native weld only half asked. `owner_kind_can_open_a_claim_check`
/// answers "can this address sign for its own payout", which is exactly right
/// and is why the Fractional reserve is refused there. But "cannot sign" is not
/// the same as "has no claimants", and conflating the two is what turned a delay
/// into a total loss the first time. So the routing is stated once, exhaustively,
/// and both gates read it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClaimCheckRouteV1 {
    /// A wallet: it signs, so a record may name it as the payee.
    Native,
    /// A program-derived owner whose claimants hold a Fractional shard Mint.
    Fractional,
    /// Neither route can represent this owner's claimants today.
    Neither,
}

/// Route one Position owner kind to the claim-check that can represent it.
///
/// Exhaustive on purpose, and the arms carry their reasons because the reasons
/// are the whole content of the function.
#[must_use]
pub const fn claim_check_route_for(kind: ProtocolPositionOwnerKindV2) -> ClaimCheckRouteV1 {
    match kind {
        // A wallet. It signs, so the native record can name it and be opened.
        ProtocolPositionOwnerKindV2::User => ClaimCheckRouteV1::Native,
        // Trading-owned resting inventory, and the Fractional reserve. The
        // address cannot sign, so it can never be a payee -- but the collateral
        // it holds backs every outstanding shard of one coordinate, and those
        // shards have holders who can sign for themselves. The record is
        // addressed by the Mint, so no payee is named and none has to sign.
        ProtocolPositionOwnerKindV2::TradingRecord => ClaimCheckRouteV1::Fractional,
        // A Claims capability PDA. Structurally the same shape -- its claimants
        // hold a Mint too -- but a rational representation's Mint, whose terms
        // live in a different family. The fractional record's two numbers,
        // `denominator` and `payout_per_claim`, come from the Fractional
        // exposure terms specifically, and nothing authors them for a rational
        // representation. Debt, named rather than absolved: this owner kind
        // still has no compaction route, and a market carrying one still cannot
        // retire past a sleeping holder.
        ProtocolPositionOwnerKindV2::ClaimsCapability => ClaimCheckRouteV1::Neither,
    }
}

/// Whether a Position's owner kind may be compacted into a fractional record.
///
/// The mirror of `claim_check_compaction_v1::owner_kind_can_open_a_claim_check`,
/// reading the same routing so the two can never both admit one kind.
#[must_use]
pub const fn owner_kind_may_open_a_fractional_claim_check(
    kind: ProtocolPositionOwnerKindV2,
) -> bool {
    matches!(claim_check_route_for(kind), ClaimCheckRouteV1::Fractional)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claim_check_compaction_v1::ClaimCheckCompactionSbfErrorV1;
    use crate::claim_check_redemption_v1::ClaimCheckRedemptionSbfErrorV1;
    use dclutch_claims_svm::claim_check_v1::{
        CLAIM_CHECK_ESCROW_SEED_V1, CLAIM_CHECK_SEED_V1, CLAIM_CHECK_VAULT_SEED_V1,
    };
    use dclutch_claims_svm::fractional_claim_check_v1::{
        FRACTIONAL_CLAIM_CHECK_SEED_V1, FractionalClaimCheckSeedsV1, MAX_PDA_SEED_BYTES_V1,
    };
    use solana_program::pubkey::{Pubkey, PubkeyError};

    const COMPACTION_TABLE: [FractionalClaimCheckCompactionSbfErrorV1; 13] = [
        FractionalClaimCheckCompactionSbfErrorV1::Accounts,
        FractionalClaimCheckCompactionSbfErrorV1::Authority,
        FractionalClaimCheckCompactionSbfErrorV1::Identity,
        FractionalClaimCheckCompactionSbfErrorV1::Deadline,
        FractionalClaimCheckCompactionSbfErrorV1::Phase,
        FractionalClaimCheckCompactionSbfErrorV1::AlreadyCompacted,
        FractionalClaimCheckCompactionSbfErrorV1::Conservation,
        FractionalClaimCheckCompactionSbfErrorV1::Economic,
        FractionalClaimCheckCompactionSbfErrorV1::Receipt,
        FractionalClaimCheckCompactionSbfErrorV1::Escrow,
        FractionalClaimCheckCompactionSbfErrorV1::Scope,
        FractionalClaimCheckCompactionSbfErrorV1::Terms,
        FractionalClaimCheckCompactionSbfErrorV1::ShardMint,
    ];

    const REDEMPTION_TABLE: [FractionalClaimCheckRedemptionSbfErrorV1; 7] = [
        FractionalClaimCheckRedemptionSbfErrorV1::Accounts,
        FractionalClaimCheckRedemptionSbfErrorV1::Authority,
        FractionalClaimCheckRedemptionSbfErrorV1::Identity,
        FractionalClaimCheckRedemptionSbfErrorV1::Conservation,
        FractionalClaimCheckRedemptionSbfErrorV1::Receipt,
        FractionalClaimCheckRedemptionSbfErrorV1::NoWholeClaim,
        FractionalClaimCheckRedemptionSbfErrorV1::Vault,
    ];

    const KINDS: [ProtocolPositionOwnerKindV2; 3] = [
        ProtocolPositionOwnerKindV2::TradingRecord,
        ProtocolPositionOwnerKindV2::User,
        ProtocolPositionOwnerKindV2::ClaimsCapability,
    ];

    #[test]
    fn every_code_is_contiguous_and_unique_within_its_sub_band() {
        for (index, code) in COMPACTION_TABLE.iter().enumerate() {
            let expected = dclutch_refusal_registry::CLAIMS_REFUSAL_BASE + 0x640 + index as u32;
            assert_eq!(*code as u32, expected);
            assert!(
                !COMPACTION_TABLE
                    .iter()
                    .skip(index + 1)
                    .any(|other| other == code)
            );
        }
        for (index, code) in REDEMPTION_TABLE.iter().enumerate() {
            let expected = dclutch_refusal_registry::CLAIMS_REFUSAL_BASE + 0x660 + index as u32;
            assert_eq!(*code as u32, expected);
            assert!(
                !REDEMPTION_TABLE
                    .iter()
                    .skip(index + 1)
                    .any(|other| other == code)
            );
        }
    }

    #[test]
    fn the_four_claim_check_sub_bands_are_ordered_and_never_overlap() {
        // Native compaction 0x600, native redemption 0x620, fractional
        // compaction 0x640, fractional redemption 0x660. The gaps are deliberate
        // room for any one family to grow without the others renumbering, which
        // would break every hostile test naming a literal.
        let boundaries = [
            ClaimCheckCompactionSbfErrorV1::Accounts as u32,
            ClaimCheckRedemptionSbfErrorV1::Accounts as u32,
            FractionalClaimCheckCompactionSbfErrorV1::Accounts as u32,
            FractionalClaimCheckRedemptionSbfErrorV1::Accounts as u32,
        ];
        for (index, base) in boundaries.iter().enumerate() {
            if let Some(next) = boundaries.get(index + 1) {
                assert!(base < next, "sub-band bases must ascend");
            }
        }
        assert!(
            (ClaimCheckCompactionSbfErrorV1::Scope as u32)
                < ClaimCheckRedemptionSbfErrorV1::Accounts as u32
        );
        assert!(
            (ClaimCheckRedemptionSbfErrorV1::Vault as u32)
                < FractionalClaimCheckCompactionSbfErrorV1::Accounts as u32
        );
        assert!(
            (FractionalClaimCheckCompactionSbfErrorV1::ShardMint as u32)
                < FractionalClaimCheckRedemptionSbfErrorV1::Accounts as u32
        );
        // And every occupied Claims sub-band predating this lane still sits
        // below all four, so an addition to one of those that ran 0x100 wide
        // would be caught here.
        for occupied in [
            0x000_u32, 0x100, 0x140, 0x160, 0x180, 0x200, 0x210, 0x260, 0x500,
        ] {
            let base = dclutch_refusal_registry::CLAIMS_REFUSAL_BASE + occupied;
            for code in COMPACTION_TABLE {
                assert!(code as u32 > base);
            }
            for code in REDEMPTION_TABLE {
                assert!(code as u32 > base);
            }
        }
    }

    #[test]
    fn a_refusal_reaches_the_runtime_as_the_literal_a_log_line_shows() {
        assert_eq!(
            ProgramError::from(FractionalClaimCheckCompactionSbfErrorV1::Scope),
            ProgramError::Custom(0x564A)
        );
        assert_eq!(
            ProgramError::from(FractionalClaimCheckCompactionSbfErrorV1::ShardMint),
            ProgramError::Custom(0x564C)
        );
        assert_eq!(
            ProgramError::from(FractionalClaimCheckRedemptionSbfErrorV1::NoWholeClaim),
            ProgramError::Custom(0x5665)
        );
    }

    #[test]
    fn no_owner_kind_is_admitted_by_both_gates() {
        // The weld's own property, extended. Refusing `TradingRecord` at the
        // native gate was the fix; admitting it at the fractional gate is the
        // completion. What must never happen is both, because a Position
        // compacted twice would resolve one pot of collateral into two records.
        for kind in KINDS {
            let native = crate::claim_check_compaction_v1::owner_kind_can_open_a_claim_check(kind);
            let fractional = owner_kind_may_open_a_fractional_claim_check(kind);
            assert!(
                !(native && fractional),
                "{kind:?} may not be entitled to two claim-checks for one Position"
            );
        }
    }

    #[test]
    fn the_native_gate_is_not_relaxed_by_this_lane() {
        // Stated against the shipped function rather than restated, so a future
        // edit to the weld has to argue with this test. `TradingRecord` is the
        // Fractional reserve: giving it a native record would mint collateral to
        // an address that can never sign, which is the exposure the weld closed.
        assert!(
            crate::claim_check_compaction_v1::owner_kind_can_open_a_claim_check(
                ProtocolPositionOwnerKindV2::User
            )
        );
        assert!(
            !crate::claim_check_compaction_v1::owner_kind_can_open_a_claim_check(
                ProtocolPositionOwnerKindV2::TradingRecord
            )
        );
        assert!(
            !crate::claim_check_compaction_v1::owner_kind_can_open_a_claim_check(
                ProtocolPositionOwnerKindV2::ClaimsCapability
            )
        );
    }

    #[test]
    fn the_pda_claimant_design_is_what_admits_the_kind_the_weld_refuses() {
        // The whole argument of this lane, as an assertion. The native gate asks
        // "can this address sign for its own payout" and `TradingRecord` answers
        // no, permanently. The fractional record never asks: it names no payee,
        // because its address IS the instrument. So the one kind the weld had to
        // refuse is exactly the one this route admits, and neither gate had to
        // move.
        assert_eq!(
            claim_check_route_for(ProtocolPositionOwnerKindV2::TradingRecord),
            ClaimCheckRouteV1::Fractional
        );
        assert!(owner_kind_may_open_a_fractional_claim_check(
            ProtocolPositionOwnerKindV2::TradingRecord
        ));
        assert!(
            !crate::claim_check_compaction_v1::owner_kind_can_open_a_claim_check(
                ProtocolPositionOwnerKindV2::TradingRecord
            )
        );
    }

    #[test]
    fn every_owner_kind_is_routed_and_exactly_one_is_still_stranded() {
        // The routing is total, and the coverage is stated as a count so the
        // named debt cannot quietly grow or quietly vanish.
        let native = KINDS
            .into_iter()
            .filter(|kind| matches!(claim_check_route_for(*kind), ClaimCheckRouteV1::Native))
            .count();
        let fractional = KINDS
            .into_iter()
            .filter(|kind| matches!(claim_check_route_for(*kind), ClaimCheckRouteV1::Fractional))
            .count();
        let neither = KINDS
            .into_iter()
            .filter(|kind| matches!(claim_check_route_for(*kind), ClaimCheckRouteV1::Neither))
            .count();
        assert_eq!(native, 1);
        assert_eq!(fractional, 1);
        // `ClaimsCapability`. Its claimants hold a rational representation's
        // Mint, whose terms author neither of this record's two numbers. Still
        // open, still named in the census.
        assert_eq!(neither, 1);
        assert_eq!(native + fractional + neither, KINDS.len());
        assert_eq!(
            claim_check_route_for(ProtocolPositionOwnerKindV2::ClaimsCapability),
            ClaimCheckRouteV1::Neither
        );
    }

    #[test]
    fn the_fractional_gate_agrees_with_the_routing_and_reads_no_second_table() {
        for kind in KINDS {
            assert_eq!(
                owner_kind_may_open_a_fractional_claim_check(kind),
                matches!(claim_check_route_for(kind), ClaimCheckRouteV1::Fractional),
                "{kind:?} must be admitted by the routing and by nothing else"
            );
        }
    }

    /// The record's address derives, which it could not until 2026-08-31.
    ///
    /// `FRACTIONAL_CLAIM_CHECK_SEED_V1` shipped at 33 bytes — one over Solana's
    /// per-seed maximum — so `create_program_address` refused
    /// `MaxSeedLengthExceeded` for all 255 bumps, `find_program_address` found
    /// none, and every route naming this record would have **panicked** rather
    /// than refused. Nothing caught it because nothing had ever derived it:
    /// `dclutch-claims-svm` is `no_std` with no SDK dependency, so its own tests
    /// cannot call this. This crate can, and does.
    ///
    /// **The test discriminates**, which is the part that matters: the same call
    /// on the exact 33-byte string it used to hold returns `None`. A derivation
    /// test that could not tell the old value from the new one would pass either
    /// way and prove nothing about the fix.
    #[test]
    fn the_record_address_derives_and_the_old_domain_still_could_not() {
        let seeds = FractionalClaimCheckSeedsV1::new([3; 32], [4; 32]).expect("coordinates");
        let program = Pubkey::new_from_array([5; 32]);

        let (address, bump) = Pubkey::try_find_program_address(&seeds.as_slices(), &program)
            .expect("the fractional claim-check address must derive");
        assert_ne!(address, Pubkey::default());
        // And the bump round-trips: the address the record persists a bump for
        // is the one `create_program_address` reproduces from that bump.
        let with_bump: [&[u8]; 4] = [
            FRACTIONAL_CLAIM_CHECK_SEED_V1,
            &seeds.aggregate(),
            &seeds.shard_mint(),
            core::slice::from_ref(&bump),
        ];
        assert_eq!(
            Pubkey::create_program_address(&with_bump, &program).expect("canonical bump"),
            address
        );

        // The exact string this domain held until it was shortened. Restated as
        // a literal rather than reconstructed, so this stays a statement about
        // the value that actually shipped.
        const SHIPPED_UNDERIVABLE: &[u8] = b"dclutch:fractional-claim-check:v1";
        assert_eq!(SHIPPED_UNDERIVABLE.len(), 33);
        assert_eq!(
            Pubkey::try_find_program_address(
                &[SHIPPED_UNDERIVABLE, &seeds.aggregate(), &seeds.shard_mint()],
                &program,
            ),
            None,
            "the 33-byte domain has no derivable address for any bump"
        );
        assert_eq!(
            Pubkey::create_program_address(
                &[
                    SHIPPED_UNDERIVABLE,
                    &seeds.aggregate(),
                    &seeds.shard_mint(),
                    &[255]
                ],
                &program,
            ),
            Err(PubkeyError::MaxSeedLengthExceeded),
            "and the reason is the seed length, not an unlucky bump"
        );

        // Every domain this family derives with is inside the maximum, so the
        // fix is the family's and not one constant's.
        for domain in [
            FRACTIONAL_CLAIM_CHECK_SEED_V1,
            CLAIM_CHECK_SEED_V1,
            CLAIM_CHECK_ESCROW_SEED_V1,
            CLAIM_CHECK_VAULT_SEED_V1,
        ] {
            assert!(domain.len() <= MAX_PDA_SEED_BYTES_V1);
        }
    }
}
