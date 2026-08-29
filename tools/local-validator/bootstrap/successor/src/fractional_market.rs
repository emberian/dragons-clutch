//! The Fractional consumer of the capability-neutral selection seam.
//!
//! `fractional_selected_closure_v1` compiles a complete Fractional
//! selected-capability closure — program set, selected descriptor, the
//! market-free selection config, the execution terms, and the publication —
//! through `fractional_selected_release_v4`, exactly as a founding driver
//! consumes it.
//!
//! THE WALL THIS MODULE WAS BUILT AGAINST, AND HOW IT CAME DOWN
//!
//! Until the config split this file documented an unsatisfiable-by-
//! construction founding (board finding 2026-08-29): the Market PDA derives
//! from `MarketIdentity` seeds including the capability-manifest digest; the
//! manifest entry must name the selection config; Fractional's config WAS the
//! exposure terms; and the terms bind the Market PDA. So
//! `manifest ⊃ config_id = SHA-256(terms) ⊃ terms.market =
//! PDA(seeds ⊇ SHA-256(manifest))` — a SHA-256 fixed point no author can
//! construct.
//!
//! The split (ORCH ruling 13:15, amended by the second-hop finding) removes
//! the third link and only the third link: the manifest-named config is now a
//! market-free `FractionalSelectionConfigV1` (denominator, both widths, Token
//! program, graph), and the terms stay the execution record, joined to the
//! config at runtime by `join_fractional_selection_config_v1` and to the
//! Market by the binds that already existed.
//!
//! `a_fractional_selection_now_precedes_the_market_it_binds` below is the
//! INVERSION of the old divergence test, kept deliberately in the same shape
//! so the two can be read against each other: the old one proved the
//! iteration diverges, this one proves the manifest does not move at all.

use sha2::{Digest as _, Sha256};

use dclutch_fractional_claim_kernel::{
    FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2, FRACTIONAL_SELECTION_CONFIG_SCHEMA_ID_V1,
    FractionalExposureTermsAdmissionV2, FractionalExposureTermsV2,
};
use dclutch_fractional_claim_operator::{
    FractionalFrameWidthsV4, FractionalSelectedReleaseInputV4, fractional_selected_release_v4,
};

use crate::{Error, Result};

/// The two record schemas one Fractional closure publishes, named once.
///
/// The ORDER matters to a reader and not to the machine: the selection config
/// is the record the MANIFEST names, and the terms are the record EXECUTION
/// uses. Conflating them is the defect the config split removed.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) const FRACTIONAL_SELECTION_CONFIG_RECORD_LABEL_V1: &str =
    "fractional_selection_config_record";
/// Label for the execution terms record.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) const FRACTIONAL_TERMS_RECORD_LABEL_V1: &str = "fractional_terms_record";

/// One compiled Fractional closure in the byte shape the neutral seam and the
/// record publisher consume.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct FractionalSelectedClosureBytesV1 {
    /// Exact four-entry `CapabilityProgramSetV2` bytes.
    pub(crate) program_set: Vec<u8>,
    /// The Wrap-action descriptor; all four bundles agree on every
    /// entry-authored coordinate, so any member may stand for the set.
    pub(crate) selected_descriptor: Vec<u8>,
    /// The market-free selection config — the record the MANIFEST names.
    ///
    /// This is what the seam hashes into `entry.config_id`, and it is
    /// derivable before the Market address exists.
    pub(crate) config: Vec<u8>,
    /// Schema the selection config is finalized under.
    pub(crate) config_schema: [u8; 32],
    /// The exact exposure terms — the EXECUTION record, joined to the config
    /// at runtime. It binds the Market and is deliberately not the config.
    pub(crate) terms: Vec<u8>,
    /// Schema the terms record is finalized under.
    pub(crate) terms_schema: [u8; 32],
    /// The canonical Market-bindable publication.
    pub(crate) publication: Vec<u8>,
    /// SHA-256 of the publication bytes.
    pub(crate) publication_id: [u8; 32],
}

/// Compile one complete Fractional selected-capability closure from exact
/// canonical terms bytes.
///
/// Frames, descriptors, program set, and publication are all derived by the
/// family's own release compiler and hostile-revalidated there; this function
/// restates nothing. The terms still name a Market — they are the execution
/// record — but nothing the MANIFEST names does, which is what makes the
/// founding reachable.
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
        config: release.selection_config,
        config_schema: FRACTIONAL_SELECTION_CONFIG_SCHEMA_ID_V1,
        terms: terms_bytes.to_vec(),
        terms_schema: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
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

    /// THE INVERSION. The old form of this test proved the iteration
    /// diverges; this one proves there is no iteration to run.
    ///
    /// It is deliberately the same experiment: author terms for one candidate
    /// Market, build the manifest a founding would publish, derive the Market
    /// that manifest determines. Before the split the derived Market differed
    /// from the candidate, re-authoring moved the manifest, and the sequence
    /// diverged forever. After the split the manifest is IDENTICAL for two
    /// unrelated candidate Markets — so it determines exactly one Market, and
    /// terms re-authored for that Market leave the manifest untouched. The
    /// fixed point closes on the first step instead of running away.
    ///
    /// This is the property a founding needs, stated as the thing that used to
    /// be impossible.
    #[test]
    fn a_fractional_selection_now_precedes_the_market_it_binds() {
        let candidate = [0x77; 32];
        let unrelated = [0x19; 32];
        assert_ne!(candidate, unrelated);

        // Terms for two different Markets really are different bytes...
        assert_ne!(terms_bytes_for(candidate), terms_bytes_for(unrelated));

        // ...and yet they publish the SAME manifest. That is the whole split.
        let manifest_0 = manifest_for_market(candidate);
        let manifest_unrelated = manifest_for_market(unrelated);
        assert_eq!(
            manifest_0, manifest_unrelated,
            "the manifest must not move when only the terms' Market moves"
        );

        // So the manifest determines one Market...
        let market_1 = market_for_manifest(&manifest_0);
        // ...and authoring the terms honestly FOR that Market reproduces the
        // very same manifest. The iteration has a fixed point and this is it.
        let manifest_1 = manifest_for_market(market_1);
        assert_eq!(
            manifest_1, manifest_0,
            "terms authored for the derived Market must reproduce the manifest"
        );
        assert_eq!(
            market_for_manifest(&manifest_1),
            market_1,
            "the fixed point is closed: this manifest and this Market agree"
        );
    }

    /// The manifest-named config is the selection config, and the terms are
    /// NOT it — the two halves of the split, checked at the seam boundary the
    /// founding actually uses.
    #[test]
    fn the_manifest_entry_names_the_selection_config_and_never_the_terms() {
        let closure =
            fractional_selected_closure_v1(&terms_bytes_for([0x77; 32]), [0x50; 32], widths())
                .expect("closure");
        assert_eq!(closure.config_schema, FRACTIONAL_SELECTION_CONFIG_SCHEMA_ID_V1);
        assert_eq!(closure.terms_schema, FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2);
        assert_ne!(closure.config, closure.terms);

        let entry = selected_manifest_entry_v1(SelectedCapabilityClosureV1 {
            program_set: &closure.program_set,
            selected_descriptor: &closure.selected_descriptor,
            config: &closure.config,
            activation_deadline_slot: u64::MAX,
            root_rent_minimum_lamports: 1_000_000,
        })
        .expect("seam entry");

        let config_digest: [u8; 32] = Sha256::digest(&closure.config).into();
        let terms_digest: [u8; 32] = Sha256::digest(&closure.terms).into();
        assert_eq!(entry.config_id().to_bytes(), config_digest);
        assert_ne!(entry.config_id().to_bytes(), terms_digest);
    }

    /// Control: the closure still compiles whole and the seam still consumes
    /// it with no Fractional-shaped special case.
    #[test]
    fn the_closure_compiles_whole_and_the_seam_consumes_it() {
        let closure =
            fractional_selected_closure_v1(&terms_bytes_for([0x77; 32]), [0x50; 32], widths())
                .expect("closure");
        assert_eq!(closure.publication.len(), 512);
        assert_eq!(
            closure.publication_id,
            <[u8; 32]>::from(Sha256::digest(&closure.publication))
        );
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
