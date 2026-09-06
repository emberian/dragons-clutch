//! The host's one author for a Market's refunding failure escrow.
//!
//! Decision 0025 seats a refunding Market's failure coordinate in a Position
//! nobody controls, and the addresses are derived rather than declared. Two
//! host consumers need them — the journey census, which joins the escrow into
//! L3, and the `BeginRetiring` preflight, which has to tell a seated residue
//! apart from an unpaid holder — and a third spelling would eventually
//! disagree with the other two. `programs/dclutch-claims-sbf`'s
//! `FailureEscrowIdentityV1::derive` is this function's on-chain twin; it
//! derives the OWNER only, because a program with the escrow in its frame is
//! told the account and re-derives the identity, while a host has to find the
//! account first.
//!
//! Every input is read off the Claims aggregate: the program is its `owner`,
//! the logical Market and the runtime width are its own header fields. So a
//! caller cannot point this at another Market's escrow, and a reader that has
//! the aggregate at all needs to be told nothing further.

use dclutch_claims::protocol_position_v2::{
    ProtocolPositionAdmissionSeedsV2, ProtocolPositionClaimsCapabilitySeedsV2,
    ProtocolPositionSeedsV2,
};
use solana_program::pubkey::Pubkey;

/// One Market's derived failure escrow.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureEscrowV1 {
    /// Runtime index of the failure coordinate.
    pub failure_selector: u32,
    /// `ClaimsCapability` owner PDA at `(logical market, failure selector)`.
    pub owner: Pubkey,
    /// The escrow's `LiabilityBasisV2` Position under that owner.
    pub position: Pubkey,
    /// The escrow's protocol-Position admission record under that owner.
    pub admission: Pubkey,
}

/// Why a Market has no derivable failure escrow.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureEscrowErrorV1 {
    /// The runtime width seats no escrow: a refunding set needs one ordinary
    /// coordinate and one failure coordinate.
    Width,
    /// A derivation input did not fit its seed.
    Seeds,
}

impl core::fmt::Display for FailureEscrowErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Width => formatter.write_str(
                "the Claims aggregate's runtime width seats no failure escrow: a refunding \
                 complete set needs one ordinary coordinate and one failure coordinate",
            ),
            Self::Seeds => {
                formatter.write_str("the failure escrow's derivation inputs did not fit its seeds")
            }
        }
    }
}

/// Derive one Market's failure escrow from coordinates the chain itself holds.
///
/// The failure selector comes from the economic kernel's
/// `refunding_failure_index`, the sole author of which coordinate a refunding
/// complete set seats where. Nothing here re-spells "the last one".
pub fn failure_escrow_v1(
    claims_program: Pubkey,
    logical_market: [u8; 32],
    aggregate: Pubkey,
    claim_count: u32,
) -> Result<FailureEscrowV1, FailureEscrowErrorV1> {
    let failure = dclutch_product::economic_slice::refunding_failure_index(claim_count)
        .map_err(|_| FailureEscrowErrorV1::Width)?;
    let failure_selector = u32::try_from(failure).map_err(|_| FailureEscrowErrorV1::Width)?;
    let owner = Pubkey::find_program_address(
        &ProtocolPositionClaimsCapabilitySeedsV2::new(logical_market, failure_selector)
            .map_err(|_| FailureEscrowErrorV1::Seeds)?
            .as_slices(),
        &claims_program,
    )
    .0;
    let position = Pubkey::find_program_address(
        &ProtocolPositionSeedsV2::new(aggregate.to_bytes(), owner.to_bytes())
            .map_err(|_| FailureEscrowErrorV1::Seeds)?
            .as_slices(),
        &claims_program,
    )
    .0;
    let admission = Pubkey::find_program_address(
        &ProtocolPositionAdmissionSeedsV2::new(aggregate.to_bytes(), owner.to_bytes())
            .map_err(|_| FailureEscrowErrorV1::Seeds)?
            .as_slices(),
        &claims_program,
    )
    .0;
    Ok(FailureEscrowV1 {
        failure_selector,
        owner,
        position,
        admission,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cohort-16.1's real devnet coordinates, so the derivation is pinned to a
    /// chain rather than to itself. `7FQCfc4Rrrs…` is a live 160-byte
    /// `DCLLBP02` Position holding `[0, 0, 0, 166666667]` at revision 1, and it
    /// is the residue that forecloses this Market's retirement. A derivation
    /// that drifts stops reproducing a real address rather than stopping
    /// agreeing with itself.
    #[test]
    fn the_derived_escrow_is_the_account_cohort_16_1_founded() {
        let claims: Pubkey = "8JfHfBBGaoUP1yV6VzXcvWwhQSZNV8eQmDAiYmCpNQJk"
            .parse()
            .expect("Claims program");
        let market: Pubkey = "3xoSXBVsAXENB1RPq4sqS8euCksT1qsnnz83eWQPEtgY"
            .parse()
            .expect("logical Market");
        let aggregate: Pubkey = "CBzv1hhtToxpCaExaA7QqES4bMu5UjxiAiBW9bMUrCdg"
            .parse()
            .expect("Claims aggregate");
        let escrow = failure_escrow_v1(claims, market.to_bytes(), aggregate, 4).expect("escrow");
        assert_eq!(escrow.failure_selector, 3);
        assert_eq!(
            escrow.owner.to_string(),
            "Hq6sF5pv3i8CBkH46dsyN9fnzJi1jooS2gj6USCQmke3",
        );
        assert_eq!(
            escrow.position.to_string(),
            "7FQCfc4RrrsATEe969eNVYoLjDukmBVKMAxM1yg7AzcQ",
        );
        assert_eq!(
            escrow.admission.to_string(),
            "4WUZ2qZKz7nkgGnnNejP8cLNHhjKCFCpHwNVDikE3T9b",
        );
    }

    /// A width that can seat no escrow is not an escrow this function invents.
    #[test]
    fn a_width_below_the_structural_floor_refuses_by_name() {
        let claims = Pubkey::new_unique();
        let aggregate = Pubkey::new_unique();
        for width in [0, 1] {
            assert_eq!(
                failure_escrow_v1(claims, aggregate.to_bytes(), aggregate, width),
                Err(FailureEscrowErrorV1::Width)
            );
        }
        assert!(failure_escrow_v1(claims, aggregate.to_bytes(), aggregate, 2).is_ok());
    }
}
