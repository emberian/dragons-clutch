//! Refusals for permissionless claim-check compaction.
//!
//! Compaction is the crank that lets a terminal market retire past a holder
//! who never returns. After a release-fixed deadline anyone may resolve one
//! sleeping position's payout into a per-market escrow only that holder can
//! open, retire its supply through redemption's own signed-delta executor, and
//! close the position and its admission record -- paying the caller out of rent
//! that was already leaving those accounts.
//!
//! Two properties shape this table and are worth stating where the codes live,
//! because a later edit could quietly drop either.
//!
//! **The wrong-holder hostile has no code, by construction.** The route takes
//! `(aggregate, owner)` as coordinates and re-derives both the position and the
//! claim-check from them; a caller naming the wrong owner derives an address
//! that is not the account they passed. `Identity` is what that derivation
//! mismatch surfaces as -- it is not a holder-field comparison, because there
//! is no holder field.
//!
//! **`Deadline` is the only time refusal, and it can only ever be generous.**
//! The clock origin is the slot the escrow was opened, which is at or after the
//! market went terminal, so stamping it there lengthens the wait rather than
//! shortening it. The deadline itself is compiled into this ELF, so the only
//! tamper surface is a release re-point, which does not exist today and must
//! refuse a shortening when it does.

use solana_program::program_error::ProgramError;

/// Stable claim-check compaction refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ClaimCheckCompactionSbfErrorV1 {
    /// The fixed account frame, ownership, or writability refused.
    Accounts = 0x5600,
    /// A signer the route does not admit was present.
    Authority = 0x5601,
    /// Coordinates did not derive the passed account, or aliased, or were zero.
    Identity = 0x5602,
    /// The compaction deadline had not elapsed at the observed slot.
    Deadline = 0x5603,
    /// The Core phase, or the absence of a terminal receipt, refused.
    Phase = 0x5604,
    /// The claim-check address was already occupied.
    AlreadyCompacted = 0x5605,
    /// A plan's atoms or lamports did not balance.
    Conservation = 0x5606,
    /// The terminal payout derivation refused.
    Economic = 0x5607,
    /// Observed post-balances did not match the admitted plan.
    Receipt = 0x5608,
    /// The escrow was absent, or its mint or token program did not match.
    Escrow = 0x5609,
    /// A position kind this version does not compact.
    Scope = 0x560A,
}

// Registered refusal band (`docs/decisions/0007-namespaced-refusal-codes.md`).
// The discriminants stay literal so a code seen in a validator log is greppable;
// these assertions are what stops them drifting out of the allocated band.
const _: () = assert!(
    ClaimCheckCompactionSbfErrorV1::Accounts as u32
        == dclutch_refusal_registry::CLAIMS_REFUSAL_BASE + 0x600,
    "ClaimCheckCompactionSbfErrorV1 must start at its registered refusal band base"
);
const _: () = assert!(
    (ClaimCheckCompactionSbfErrorV1::Scope as u32)
        < dclutch_refusal_registry::CLAIMS_REFUSAL_BASE + dclutch_refusal_registry::BAND_SPAN,
    "ClaimCheckCompactionSbfErrorV1 must not run past its registered refusal band"
);
// Compaction and claim-check redemption are independently versioned request
// families and hold separate round sub-bands. This assertion is what keeps the
// two from ever being interleaved by a later addition to either table.
const _: () = assert!(
    (ClaimCheckCompactionSbfErrorV1::Scope as u32)
        < crate::claim_check_redemption_v1::ClaimCheckRedemptionSbfErrorV1::Accounts as u32,
    "the compaction sub-band must not run into the claim-check redemption sub-band"
);

impl From<ClaimCheckCompactionSbfErrorV1> for ProgramError {
    fn from(value: ClaimCheckCompactionSbfErrorV1) -> Self {
        Self::Custom(value as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE: [ClaimCheckCompactionSbfErrorV1; 11] = [
        ClaimCheckCompactionSbfErrorV1::Accounts,
        ClaimCheckCompactionSbfErrorV1::Authority,
        ClaimCheckCompactionSbfErrorV1::Identity,
        ClaimCheckCompactionSbfErrorV1::Deadline,
        ClaimCheckCompactionSbfErrorV1::Phase,
        ClaimCheckCompactionSbfErrorV1::AlreadyCompacted,
        ClaimCheckCompactionSbfErrorV1::Conservation,
        ClaimCheckCompactionSbfErrorV1::Economic,
        ClaimCheckCompactionSbfErrorV1::Receipt,
        ClaimCheckCompactionSbfErrorV1::Escrow,
        ClaimCheckCompactionSbfErrorV1::Scope,
    ];

    #[test]
    fn every_code_is_contiguous_and_unique_within_the_sub_band() {
        for (index, code) in TABLE.iter().enumerate() {
            let expected = dclutch_refusal_registry::CLAIMS_REFUSAL_BASE + 0x600 + index as u32;
            assert_eq!(*code as u32, expected);
            let rest = index + 1;
            assert!(!TABLE.iter().skip(rest).any(|other| other == code));
        }
    }

    #[test]
    fn the_sub_band_does_not_collide_with_any_occupied_claims_sub_band() {
        // Occupied in claims-sbf before this lane: 0x000, 0x100, 0x140, 0x160,
        // 0x180, 0x200, 0x210, 0x260, 0x500. A later addition to any of those
        // that ran 0x100 wide would reach this block, and this is the assertion
        // that would catch it.
        for occupied in [
            0x000_u32, 0x100, 0x140, 0x160, 0x180, 0x200, 0x210, 0x260, 0x500,
        ] {
            let base = dclutch_refusal_registry::CLAIMS_REFUSAL_BASE + occupied;
            for code in TABLE {
                assert_ne!(code as u32, base);
                assert!(code as u32 > base);
            }
        }
    }

    #[test]
    fn a_refusal_reaches_the_runtime_as_the_literal_a_log_line_shows() {
        assert_eq!(
            ProgramError::from(ClaimCheckCompactionSbfErrorV1::Deadline),
            ProgramError::Custom(0x5603)
        );
        assert_eq!(
            ProgramError::from(ClaimCheckCompactionSbfErrorV1::Scope),
            ProgramError::Custom(0x560A)
        );
    }
}
