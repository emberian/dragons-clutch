//! The Fractional consumer of the capability-neutral selection seam — built
//! to the exact wall that stops it, with the wall named rather than worked
//! around.
//!
//! `fractional_selected_closure_v1` really compiles a complete Fractional
//! selected-capability closure — program set, selected descriptor, config,
//! publication — through `fractional_selected_release_v4`, exactly as a
//! founding driver would consume it. It is correct, and it is exactly as
//! callable as the shipped contracts allow: only for a Market that already
//! exists.
//!
//! THE WALL (board finding 2026-08-29, every link chain-enforced): a founded
//! Market can never select this closure, because
//!
//!   1. the Market PDA derives from `MarketIdentity` seeds that include the
//!      capability-manifest digest (`derive_founding_targets_inner`,
//!      `market.rs`);
//!   2. the manifest entry must name the selection config —
//!      `require_entry_identity`, `dclutch-trading-sbf/src/dispatch.rs:448`,
//!      reached from the real activation route (`outer.rs:328-403`);
//!   3. Fractional's config IS the exposure terms — the descriptor pins
//!      `config_schema = FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2`
//!      (`artifacts_v4.rs:241`), and Claims pins
//!      `header.selection().config() == request.input().terms` on chain
//!      (`fractional_atomic_v3.rs:224`);
//!   4. the terms bind the Market PDA — `bind_terms` refuses
//!      `input.market != terms.market()` (`request_v2.rs`), Claims pins
//!      `request.market == LBV2.logical_market` (`fractional_atomic_v3.rs:651`),
//!      and Claims founding pins `logical_market == core.identity.market_id`
//!      (`founding_v5.rs:775,929`).
//!
//! So manifest ⊃ config_id = SHA-256(terms) ⊃ terms.market =
//! PDA(seeds ⊇ SHA-256(manifest)): a SHA-256 fixed point no author can
//! construct. `a_fractional_selection_cannot_precede_the_market_it_binds`
//! below is that statement as an executable test rather than prose. Founding
//! wiring lands only with the config-split ruling (a market-free
//! `FractionalSelectionConfigV1` as the manifest-named config, the terms
//! joined to it at runtime).

use sha2::{Digest as _, Sha256};

use dclutch_fractional_claim_kernel::{
    FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2, FractionalExposureTermsAdmissionV2,
    FractionalExposureTermsV2,
};
use dclutch_fractional_claim_operator::{
    FractionalFrameWidthsV4, FractionalSelectedReleaseInputV4, fractional_selected_release_v4,
};

use crate::{Error, Result};

/// The named refusal any founding wiring must surface until the config split
/// lands. One author for the sentence, so it cannot drift between callers.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) const FRACTIONAL_FOUNDING_SELECTION_WALL_V1: &str =
    "a Fractional capability cannot be selected by a founded Market: its config is the exposure \
     terms, the terms bind the Market PDA, and the Market PDA derives from the manifest that \
     must name the config — a SHA-256 fixed point no author can construct. Founding with \
     Fractional selected waits on the config-split ruling (market-free selection config in the \
     manifest; terms joined to it at runtime).";

/// One compiled Fractional closure in the byte shape the neutral seam and the
/// record publisher consume.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct FractionalSelectedClosureBytesV1 {
    /// Exact four-entry `CapabilityProgramSetV2` bytes.
    pub(crate) program_set: Vec<u8>,
    /// The Wrap-action descriptor; all four bundles agree on every
    /// entry-authored coordinate, so any member may stand for the set.
    pub(crate) selected_descriptor: Vec<u8>,
    /// The exact terms bytes — the shipped contract's config record.
    pub(crate) config: Vec<u8>,
    /// The 480-byte canonical Market-bindable publication.
    pub(crate) publication: Vec<u8>,
    /// SHA-256 of the publication bytes.
    pub(crate) publication_id: [u8; 32],
}

/// Compile one complete Fractional selected-capability closure from exact
/// canonical terms bytes.
///
/// Frames, descriptors, program set, and publication are all derived by the
/// family's own release compiler and hostile-revalidated there; this function
/// restates nothing. It works only for terms that already name their Market —
/// which is the wall documented above, not a limitation of this function.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn fractional_selected_closure_v1(
    terms_bytes: &[u8],
    capacity_profile: [u8; 32],
    widths: FractionalFrameWidthsV4,
) -> Result<FractionalSelectedClosureBytesV1> {
    let terms_digest: [u8; 32] = Sha256::digest(terms_bytes).into();
    let terms = FractionalExposureTermsV2::decode(
        terms_bytes,
        FractionalExposureTermsAdmissionV2 {
            selected_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
            finalized_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
            selected_terms_id: terms_digest,
            finalized_terms_id: terms_digest,
            recomputed_terms_digest: terms_digest,
            finalized_terms_digest: terms_digest,
            record_authenticated: true,
        },
    )
    .map_err(|error| Error::new(format!("canonical Fractional terms: {error:?}")))?;
    let release = fractional_selected_release_v4(FractionalSelectedReleaseInputV4 {
        terms,
        capacity_profile,
        widths,
    })
    .map_err(|error| Error::new(format!("Fractional selected release: {error:?}")))?;
    let selected_descriptor = release
        .bundles
        .first()
        .ok_or_else(|| Error::new("Fractional release carried no bundles"))?
        .descriptor
        .to_vec();
    Ok(FractionalSelectedClosureBytesV1 {
        program_set: release.program_set,
        selected_descriptor,
        config: terms_bytes.to_vec(),
        publication: release.publication.to_bytes().to_vec(),
        publication_id: release.publication.publication_id(),
    })
}

#[cfg(test)]
mod tests {
    use dclutch_fractional_claim_kernel::{
        FractionalExposureTermsInputV2, encode_fractional_exposure_terms_v2,
        fractional_exposure_terms_bytes_v2,
    };
    use dclutch_market_core_codec::{Identity, MarketCoreStateSeedsV2, MarketIdentity};
    use solana_sdk::pubkey::Pubkey;

    use super::*;
    use crate::selected_capability::{
        SelectedCapabilityClosureV1, merge_selected_manifest_v1, selected_manifest_entry_v1,
    };

    const REPRESENTATION_WIDTH: usize = 4;
    const CORE_PROGRAM: Pubkey = Pubkey::new_from_array([0xC0; 32]);

    fn terms_bytes_for(market: [u8; 32]) -> Vec<u8> {
        let mut shard_mints = [[0_u8; 32]; REPRESENTATION_WIDTH];
        for (index, mint) in shard_mints.iter_mut().enumerate() {
            *mint = [0x60 + index as u8; 32];
        }
        let width = fractional_exposure_terms_bytes_v2(shard_mints.len()).expect("terms width");
        let mut scratch = vec![0_u8; width];
        let mut bytes = vec![0_u8; width];
        encode_fractional_exposure_terms_v2(
            FractionalExposureTermsInputV2 {
                market,
                product_record: [0x31; 32],
                result_domain: [0x32; 32],
                release_set: [0x33; 32],
                token_program: [0x34; 32],
                token_behavior: [0x35; 32],
                exposure_id: [0x36; 32],
                product_basis: [0x37; 32],
                representation_basis: [0x38; 32],
                graph_id: [0x39; 32],
                product_width: REPRESENTATION_WIDTH as u32,
                denominator: 10,
                shard_mints: &shard_mints,
            },
            &mut scratch,
            &mut bytes,
        )
        .expect("exact Fractional exposure terms");
        bytes
    }

    fn widths() -> FractionalFrameWidthsV4 {
        FractionalFrameWidthsV4 {
            linked_basis_record: 200,
            product_record: 300,
            result_domain_record: 120,
            portfolio_record: 160,
            selected_config: fractional_exposure_terms_bytes_v2(REPRESENTATION_WIDTH)
                .expect("terms width") as u32,
            core_market: 512,
            activation_cache: 256,
            rent_credit: 128,
        }
    }

    fn identity(bytes: [u8; 32]) -> Identity {
        Identity::new(bytes).expect("identity")
    }

    /// The Market PDA a manifest determines, derived exactly as
    /// `derive_founding_targets_inner` derives it: the manifest digest is one
    /// of the nine identity seeds, `market_id` is not a seed.
    fn market_for_manifest(manifest: &[u8]) -> [u8; 32] {
        let manifest_digest: [u8; 32] = Sha256::digest(manifest).into();
        let template = MarketIdentity {
            market_id: identity([0xFF; 32]),
            realm_id: identity([0x41; 32]),
            product_record: identity([0x42; 32]),
            product_id: identity([0x43; 32]),
            resolution_policy: identity([0x44; 32]),
            capability_manifest: identity(manifest_digest),
            selected_release_set: identity([0x33; 32]),
            registry_program: identity([0x45; 32]),
            generation: 1,
        };
        Pubkey::find_program_address(
            &MarketCoreStateSeedsV2::new(template).as_slices(),
            &CORE_PROGRAM,
        )
        .0
        .to_bytes()
    }

    /// The manifest a founding would have to publish for terms bound to one
    /// candidate market: closure → entry → merged four-entry manifest.
    fn manifest_for_market(market: [u8; 32]) -> Vec<u8> {
        let closure = fractional_selected_closure_v1(&terms_bytes_for(market), [0x50; 32], widths())
            .expect("Fractional closure compiles for a named market");
        let entry = selected_manifest_entry_v1(SelectedCapabilityClosureV1 {
            program_set: &closure.program_set,
            selected_descriptor: &closure.selected_descriptor,
            config: &closure.config,
            activation_deadline_slot: u64::MAX,
            root_rent_minimum_lamports: 1_000_000,
        })
        .expect("seam entry");
        let base = {
            use dclutch_capability_contract::{
                ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CapabilityManifestV1,
                CompartmentFundingV1, ContentId, FundingAmountsV1, FundingQuoteV1,
                MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
            };
            let none = CompartmentFundingV1::not_applicable();
            let amounts = FundingAmountsV1::new(
                CompartmentFundingV1::native_lamports(1_000_000).expect("native"),
                none,
                none,
                none,
                none,
                none,
                none,
            )
            .expect("amounts");
            let companion = |kind: u8, config: u8| {
                CapabilityEntryV1::new(
                    ContentId::new([kind; 32]).expect("kind"),
                    ContentId::new([0x52; 32]).expect("release"),
                    ContentId::new([config; 32]).expect("config"),
                    ContentId::new([0x54; 32]).expect("capacity"),
                    ContentId::new([0x55; 32]).expect("root schema"),
                    ContentId::new([0x56; 32]).expect("derivation"),
                    ActivationPolicy::PrepaidLazy,
                    u64::MAX,
                    0,
                    [0; MAX_DEPENDENCIES_PER_CAPABILITY],
                    FundingQuoteV1::new(amounts, None).expect("quote"),
                )
                .expect("companion entry")
            };
            let entries = [
                companion(0x01, 0x21),
                companion(0x02, 0x22),
                companion(0x03, 0x23),
            ];
            let mut bytes =
                vec![0_u8; MANIFEST_HEADER_BYTES + entries.len() * CAPABILITY_ENTRY_BYTES];
            CapabilityManifestV1::encode_into(&entries, &mut bytes).expect("base manifest");
            bytes
        };
        merge_selected_manifest_v1(&base, entry).expect("merged manifest").0
    }

    /// The fixed point, executed: for any terms an author binds to a candidate
    /// market, the manifest that names them derives a DIFFERENT market, and
    /// re-authoring the terms for that market moves the market again. The
    /// iteration diverges instead of closing — a founded Market can never
    /// satisfy `entry.config_id == SHA-256(terms(its own PDA))`.
    #[test]
    fn a_fractional_selection_cannot_precede_the_market_it_binds() {
        let candidate = [0x77; 32];
        let manifest_0 = manifest_for_market(candidate);
        let market_1 = market_for_manifest(&manifest_0);
        assert_ne!(
            market_1, candidate,
            "the manifest naming terms for the candidate derives another market"
        );

        // Re-author honestly for the market the first manifest derives…
        let manifest_1 = manifest_for_market(market_1);
        assert_ne!(
            manifest_1, manifest_0,
            "moving the terms market moves the config identity and the manifest"
        );
        // …and the manifest carrying THOSE terms derives yet another market.
        let market_2 = market_for_manifest(&manifest_1);
        assert_ne!(
            market_2, market_1,
            "the iteration diverges: no manifest can name terms bound to the market it derives"
        );

        // The wall has one named sentence for any wiring that reaches it.
        assert!(FRACTIONAL_FOUNDING_SELECTION_WALL_V1.contains("fixed point"));
    }

    /// Control: for a market that already exists (any named identity), the
    /// closure compiles whole — the wall is founding-specific, not a defect in
    /// the release compiler or the seam.
    #[test]
    fn the_closure_compiles_whole_for_a_market_that_already_exists() {
        let closure = fractional_selected_closure_v1(
            &terms_bytes_for([0x77; 32]),
            [0x50; 32],
            widths(),
        )
        .expect("closure");
        assert_eq!(closure.publication.len(), 480);
        assert_eq!(
            closure.publication_id,
            <[u8; 32]>::from(Sha256::digest(&closure.publication))
        );
        // The seam consumes it without a Fractional-shaped special case.
        selected_manifest_entry_v1(SelectedCapabilityClosureV1 {
            program_set: &closure.program_set,
            selected_descriptor: &closure.selected_descriptor,
            config: &closure.config,
            activation_deadline_slot: u64::MAX,
            root_rent_minimum_lamports: 1_000_000,
        })
        .expect("neutral entry");
    }
}
