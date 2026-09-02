//! Which collateral adapter release a founding SELECTS, and which a reader ADMITS.
//!
//! These are two questions with two different answers, and the whole cost of
//! conflating them is a stranded cohort. Cohort-13's realm is on chain carrying
//! `228c14f9…`, the Token-2022 zero-extension release, and its deployed Custody
//! selects a profile by matching that stored id. Nothing this tree does later
//! may change what that identity means. But a market founded under it can never
//! pay a wallet its own associated token account, because the ATA program always
//! writes `ImmutableOwner` and that release refuses the suffix at 170 bytes.
//!
//! So a founding selects the NEWEST release (`430369ce…`, admitting exactly that
//! one extension on transfer participants), and a reader admits ANY production
//! release whose profile is Token-2022 -- because the realm it is reading was
//! founded by whichever cohort founded it, and the release id is authenticated
//! against the realm account, which is itself authenticated by its record digest.
//!
//! Before this module the founded release was spelled by hand at five sites and
//! the read side pinned exactly one of them, so founding cohort-14 under a new
//! release would have refused every admission against it with a message about
//! the Realm's "exact Token-2022 no-authority profile" -- a true sentence about
//! the wrong conjunct.

use dclutch_token_svm::{
    CollateralAdapterReleaseV1, PRODUCTION_ADAPTER_RELEASES, TOKEN_2022_PROGRAM_ID,
};
use sha2::{Digest, Sha256};

/// The collateral adapter release a NEW realm is founded under.
///
/// A cohort boundary is where a released identity is supposed to change, and
/// this is the one line that changes it. Every founding site reads it; none
/// spells a constructor.
pub(crate) const FOUNDED_COLLATERAL_ADAPTER_RELEASE_V1: CollateralAdapterReleaseV1 =
    CollateralAdapterReleaseV1::token_2022_immutable_owner_exact_transfer();

/// SHA-256 of the founded release's preimage: a realm's `collateral_adapter_release_id`.
pub(crate) fn founded_collateral_adapter_release_id_v1() -> [u8; 32] {
    Sha256::digest(FOUNDED_COLLATERAL_ADAPTER_RELEASE_V1.to_bytes()).into()
}

/// Return the production release a stored id names, if it is a Token-2022 one.
///
/// The read side's question. It is not "is this the release I would found
/// under" -- that would refuse every market an earlier cohort founded, which is
/// every market that exists. It is "is this one of the releases this tree
/// implements, and does it run against Token-2022", which is exactly what the
/// programs themselves ask: Custody's `collateral_profile`, Core's
/// `authenticate_vault_poststate` and Trading's `direct_token_setup_v1` all walk
/// `PRODUCTION_ADAPTER_RELEASES` and take the profile of whichever entry matches.
pub(crate) fn admitted_token_2022_collateral_release_v1(
    stored_id: &[u8; 32],
) -> Option<CollateralAdapterReleaseV1> {
    PRODUCTION_ADAPTER_RELEASES.into_iter().find(|release| {
        let digest: [u8; 32] = Sha256::digest(release.to_bytes()).into();
        &digest == stored_id && release.token_program() == TOKEN_2022_PROGRAM_ID
    })
}

/// Whether a stored id names an admitted Token-2022 collateral release.
pub(crate) fn is_admitted_token_2022_collateral_release_v1(stored_id: &[u8; 32]) -> bool {
    admitted_token_2022_collateral_release_v1(stored_id).is_some()
}

#[cfg(test)]
mod tests {
    use dclutch_token_svm::ExactTransferProfileV1;

    use super::*;

    /// The digest cohort-13's realm carries on chain, and which must stay admitted.
    ///
    /// Written as bytes rather than derived, on purpose: derived from the
    /// constructor it would move with the constructor and prove nothing.
    const COHORT_13_REALM_RELEASE_ID: [u8; 32] = [
        0x22, 0x8c, 0x14, 0xf9, 0xe5, 0x01, 0xf8, 0x61, 0x38, 0xd3, 0xf1, 0x9e, 0x5e, 0xa8, 0x15,
        0xaf, 0x62, 0x8c, 0x0a, 0xdf, 0x49, 0x9d, 0xc6, 0xa9, 0x3d, 0xd8, 0xcb, 0x18, 0x5c, 0x87,
        0x0e, 0x29,
    ];

    /// The release cohort-14 founds under.
    const COHORT_14_FOUNDED_RELEASE_ID: [u8; 32] = [
        0x43, 0x03, 0x69, 0xce, 0x72, 0xf5, 0xe1, 0xdc, 0xfa, 0x19, 0xdc, 0xee, 0x63, 0xd5, 0xe1,
        0x5f, 0x9f, 0xbf, 0x2d, 0x6c, 0x99, 0x50, 0xc5, 0xca, 0xab, 0x53, 0xd5, 0xc0, 0x28, 0xae,
        0x0a, 0x2d,
    ];

    #[test]
    fn founding_selects_the_immutable_owner_release_and_nothing_else() {
        assert_eq!(
            founded_collateral_adapter_release_id_v1(),
            COHORT_14_FOUNDED_RELEASE_ID
        );
        assert_ne!(
            founded_collateral_adapter_release_id_v1(),
            COHORT_13_REALM_RELEASE_ID,
            "a cohort boundary is where the identity changes; if these are equal it did not",
        );
        assert_eq!(
            FOUNDED_COLLATERAL_ADAPTER_RELEASE_V1.profile(),
            ExactTransferProfileV1::Token2022ImmutableOwnerTransferV1,
        );
    }

    /// COHORT-13 STAYS READABLE. This is the assertion the whole module is for.
    #[test]
    fn both_token_2022_releases_are_admitted_and_the_legacy_one_is_not() {
        assert!(is_admitted_token_2022_collateral_release_v1(
            &COHORT_13_REALM_RELEASE_ID
        ));
        assert!(is_admitted_token_2022_collateral_release_v1(
            &COHORT_14_FOUNDED_RELEASE_ID
        ));
        assert_eq!(
            admitted_token_2022_collateral_release_v1(&COHORT_13_REALM_RELEASE_ID)
                .expect("cohort-13's release")
                .profile(),
            ExactTransferProfileV1::Token2022ZeroExtensionExactTransferV1,
        );

        let legacy: [u8; 32] =
            Sha256::digest(CollateralAdapterReleaseV1::legacy_exact_transfer().to_bytes()).into();
        assert!(
            !is_admitted_token_2022_collateral_release_v1(&legacy),
            "the legacy SPL Token release is a real production release and still not this one",
        );
        assert!(!is_admitted_token_2022_collateral_release_v1(&[0; 32]));
        assert!(!is_admitted_token_2022_collateral_release_v1(&[0xff; 32]));
    }
}
