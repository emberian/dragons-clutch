//! The host's one author for a Market's refunding failure escrow.
//!
//! The derivation itself lives in `dclutch_claims::protocol_position_v2`,
//! beside the three seed types it composes, because it now has three host
//! consumers in two crates: the journey census and the `BeginRetiring`
//! preflight here, and the checkpointed retirement builder in
//! `dclutch-market-retirement-v1-operator`, which has to put the escrow into
//! two programs' account frames. A derivation re-spelled per consumer
//! eventually disagrees with itself, and this one already reproduces a real
//! devnet address rather than agreeing with itself -- see the pin below.
//!
//! This module is the host-side name for it and the home of that pin.

pub use dclutch_claims::protocol_position_v2::{
    FailureEscrowErrorV1, FailureEscrowV1, failure_escrow_v1,
};

#[cfg(test)]
mod tests {
    use super::*;
    use solana_program::pubkey::Pubkey;

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
