//! Refusals for holder-signed claim-check redemption and escrow close.
//!
//! This is the route that outlives the market. The holder signs; nobody else
//! can call it and nobody else needs to. What makes it durable is what is
//! *absent* from its frame -- no Claims aggregate, no Core state, no linked
//! basis record, no composition graph record, no Hoard, no Realm, no Custody
//! authority, and above all no Custody replay cursor. Every one of those has
//! been closed by the retirement the claim-check permitted.
//!
//! That absence is why this table is short, and the shortness is the feature.
//!
//! **Anti-replay is the account's own existence, so it has no code.** The
//! record is created once -- a non-vacant address refuses a second compaction
//! -- and closed on redemption. A closed account cannot be redeemed, and
//! re-creating one would need a compaction crank, which refuses because the
//! position it would have to read is gone. There is no cursor, no revision and
//! no counter to get wrong.
//!
//! **Draining another holder is likewise refused by construction.** The vault
//! is debited only by a redemption closing exactly one record for exactly its
//! own entitlement, and `Conservation` is what an attempt to move any other
//! quantity surfaces as.

use solana_program::program_error::ProgramError;

/// Stable claim-check redemption refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ClaimCheckRedemptionSbfErrorV1 {
    /// The fixed account frame, ownership, or writability refused.
    Accounts = 0x5620,
    /// The signer was not the record's sole entitled holder.
    Authority = 0x5621,
    /// The record was not at its derived address, or the vault did not match.
    Identity = 0x5622,
    /// The vault debit did not equal the record's entitlement.
    Conservation = 0x5623,
    /// Observed post-balances did not match the admitted plan.
    Receipt = 0x5624,
    /// An escrow close was attempted while claim-checks were still live.
    Vault = 0x5625,
}

// Registered refusal band (`docs/decisions/0007-namespaced-refusal-codes.md`).
// The discriminants stay literal so a code seen in a validator log is greppable;
// these assertions are what stops them drifting out of the allocated band.
const _: () = assert!(
    ClaimCheckRedemptionSbfErrorV1::Accounts as u32
        == dclutch_refusal_registry::CLAIMS_REFUSAL_BASE + 0x620,
    "ClaimCheckRedemptionSbfErrorV1 must start at its registered refusal band base"
);
const _: () = assert!(
    (ClaimCheckRedemptionSbfErrorV1::Vault as u32)
        < dclutch_refusal_registry::CLAIMS_REFUSAL_BASE + dclutch_refusal_registry::BAND_SPAN,
    "ClaimCheckRedemptionSbfErrorV1 must not run past its registered refusal band"
);

impl From<ClaimCheckRedemptionSbfErrorV1> for ProgramError {
    fn from(value: ClaimCheckRedemptionSbfErrorV1) -> Self {
        Self::Custom(value as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE: [ClaimCheckRedemptionSbfErrorV1; 6] = [
        ClaimCheckRedemptionSbfErrorV1::Accounts,
        ClaimCheckRedemptionSbfErrorV1::Authority,
        ClaimCheckRedemptionSbfErrorV1::Identity,
        ClaimCheckRedemptionSbfErrorV1::Conservation,
        ClaimCheckRedemptionSbfErrorV1::Receipt,
        ClaimCheckRedemptionSbfErrorV1::Vault,
    ];

    #[test]
    fn every_code_is_contiguous_and_unique_within_the_sub_band() {
        for (index, code) in TABLE.iter().enumerate() {
            let expected = dclutch_refusal_registry::CLAIMS_REFUSAL_BASE + 0x620 + index as u32;
            assert_eq!(*code as u32, expected);
            let rest = index + 1;
            assert!(!TABLE.iter().skip(rest).any(|other| other == code));
        }
    }

    #[test]
    fn the_two_claim_check_sub_bands_never_overlap() {
        // Compaction runs 0x5600..=0x560A. Redemption starts at 0x5620, and the
        // gap is deliberate room for compaction to grow without either family
        // renumbering, which would break every hostile test naming a literal.
        use crate::claim_check_compaction_v1::ClaimCheckCompactionSbfErrorV1;
        assert!(
            (ClaimCheckCompactionSbfErrorV1::Scope as u32)
                < ClaimCheckRedemptionSbfErrorV1::Accounts as u32
        );
        for code in TABLE {
            assert!(code as u32 > ClaimCheckCompactionSbfErrorV1::Scope as u32);
        }
    }

    #[test]
    fn a_refusal_reaches_the_runtime_as_the_literal_a_log_line_shows() {
        assert_eq!(
            ProgramError::from(ClaimCheckRedemptionSbfErrorV1::Authority),
            ProgramError::Custom(0x5621)
        );
        assert_eq!(
            ProgramError::from(ClaimCheckRedemptionSbfErrorV1::Vault),
            ProgramError::Custom(0x5625)
        );
    }
}
