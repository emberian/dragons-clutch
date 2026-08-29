//! The General consumer of the capability-neutral selection seam.
//!
//! This is the seam's capability-neutrality test made real: a second family
//! becomes selectable through the SAME neutral entry derivation, manifest
//! merge, and validation that Direct uses — a closure compiler plus the
//! family's publication, no new driver logic. The release itself is
//! GEN-REL's `general_selected_release_v1` (dclutch-operator), whose
//! publication is the single author of every capability fact; this module
//! only re-shapes its output into the byte closure the seam consumes and
//! cross-checks the seam's derived entry against the publication.
//!
//! Unlike Fractional (see `fractional_market.rs`), General's config —
//! `GeneralConfigV3` — is market-free: capacity, claim basis, program-set
//! identity, generation, policy windows. Nothing in it consumes the Market
//! PDA, so the manifest entry is derivable before the Market exists and a
//! General-selected Market is foundable under the shipped contracts. The
//! test below pins exactly that, as the counterpart of the Fractional
//! fixed-point pin.

use dclutch_operator::general_selected_release_v1::{
    GeneralSelectedReleaseInputV1, general_selected_release_v1,
};
use sha2::{Digest as _, Sha256};

use crate::{Error, Result};

/// One record the Registry must finalize for a selected General release,
/// owned so a driver can stage it beside the market's other publications.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct GeneralSelectedRecordV1 {
    /// Operator-facing name of the record's role.
    pub(crate) label: &'static str,
    /// Schema/release identity the record finalizes under — read off the
    /// release's own artifacts by `publication_records`, never restated here.
    pub(crate) schema: [u8; 32],
    /// Exact semantic bytes.
    pub(crate) body: Vec<u8>,
}

/// One compiled General closure in the byte shape the neutral seam and the
/// record publisher consume.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct GeneralSelectedClosureBytesV1 {
    /// Exact seven-entry `CapabilityProgramSetV2` bytes.
    pub(crate) program_set: Vec<u8>,
    /// The first bundle's descriptor; all seven agree on every entry-authored
    /// coordinate, so any member may stand for the set.
    pub(crate) selected_descriptor: Vec<u8>,
    /// Exact immutable `GeneralConfigV3` bytes — market-free by construction.
    pub(crate) config: Vec<u8>,
    /// The canonical Market-bindable publication bytes.
    pub(crate) publication: Vec<u8>,
    /// SHA-256 of the publication bytes.
    pub(crate) publication_id: [u8; 32],
    /// Every record the Registry must hold for this release.
    pub(crate) records: Vec<GeneralSelectedRecordV1>,
}

/// Compile one complete General selected-capability closure.
///
/// Everything is derived by the family's own release compiler (which runs
/// `authenticate_general_release_v3` before returning) and re-shaped here
/// without restatement.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn general_selected_closure_v1(
    input: GeneralSelectedReleaseInputV1,
) -> Result<GeneralSelectedClosureBytesV1> {
    let release = general_selected_release_v1(input)
        .map_err(|error| Error::new(format!("General selected release: {error:?}")))?;
    let records = release
        .publication_records()
        .map_err(|error| Error::new(format!("General publication records: {error:?}")))?
        .into_iter()
        .map(|record| GeneralSelectedRecordV1 {
            label: record.label,
            schema: record.schema,
            body: record.body.to_vec(),
        })
        .collect();
    let selected_descriptor = release
        .bundles
        .first()
        .ok_or_else(|| Error::new("General release carried no bundles"))?
        .descriptor
        .clone();
    let publication = release.publication.to_bytes().to_vec();
    Ok(GeneralSelectedClosureBytesV1 {
        program_set: release.program_set,
        selected_descriptor,
        config: release.config,
        publication_id: Sha256::digest(&publication).into(),
        publication,
        records,
    })
}

/// The demo General-selected market: the same lab market graph as
/// `demo_market_input`, with the General capability selected instead of
/// Direct through the neutral seam.
///
/// Derived facts come from the base graph itself — capacity profile from the
/// carried source-capacity body, claim basis from the carried linked-basis
/// record, outcome width from the cuts, price scale from the collateral
/// decimals, generation from the founding lane — and the deadline and root
/// Rent quote from the same finalized loopback snapshot Direct quotes.
/// LAB FACTS, labeled as lab facts exactly like the demo market's synthetic
/// Pyth release: the policy windows are the executed accelerator campaign's,
/// the external widths are the ones that campaign really ran against, and the
/// four deployment identities (accelerator artifact release, compiler,
/// toolchain, translation validation) are domain-separated projections of the
/// plan's own release-set identity, because no local accelerator deployment
/// exists to observe. A devnet General market replaces exactly those inputs;
/// nothing else here is a free parameter.
pub(crate) fn demo_general_market_input(
    plan_path: &std::path::Path,
    rpc_url: &str,
    registry: solana_sdk::pubkey::Pubkey,
    quote_surplus_beneficiary: solana_sdk::pubkey::Pubkey,
) -> Result<crate::model::MarketRunInput> {
    use dclutch_operator::general_selected_release_v1::{
        GeneralConfigWindowsV1, GeneralDeploymentFactsV1,
    };

    let (plan, observation) =
        crate::direct_market::observe_local_market_policy_v1(plan_path, rpc_url, registry)?;
    let resolution_release = crate::direct_market::authenticated_resolution_release_v1(&plan)?;
    let mut input = crate::market::demo_market_input_base(registry, resolution_release)?;

    let capacity_profile: [u8; 32] =
        Sha256::digest(crate::runtime::decode_hex(&input.source_capacity_profile_hex)?).into();
    let claim_basis = crate::market::semantic_basis_identity_v3(&crate::runtime::decode_hex(
        &input.linked_basis_hex,
    )?)?;
    let outcome_count = input
        .cuts
        .len()
        .checked_add(2)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| Error::new("General market outcome width overflow"))?;
    let price_scale = 10_u64
        .checked_pow(u32::from(input.collateral_display_decimals))
        .ok_or_else(|| Error::new("Market collateral decimals overflow General price scale"))?;
    // The generic founding commits the Open Market at generation + 1, and the
    // config binds the Market it is selected by, so the selection names the
    // Open Market's generation, not the Found37 staging generation.
    let generation = input
        .generation
        .checked_add(1)
        .ok_or_else(|| Error::new("General market generation overflow"))?;
    let lab = |label: &str| -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(b"dclutch:lab:general-selection:v1");
        hasher.update([0]);
        hasher.update(plan.release_set_id.as_bytes());
        hasher.update([0]);
        hasher.update(label.as_bytes());
        hasher.finalize().into()
    };
    let release_input = dclutch_operator::general_selected_release_v1::GeneralSelectedReleaseInputV1 {
        capacity_profile,
        claim_basis,
        selection_policy: lab("selection-policy"),
        quote_surplus_beneficiary: quote_surplus_beneficiary.to_bytes(),
        generation,
        price_scale,
        // The executed accelerator campaign's policy windows.
        windows: GeneralConfigWindowsV1 {
            collection_slots: 16,
            selection_slots: 16,
            settlement_slots: 64,
            max_orders_per_candidate: 32,
            max_pages_per_candidate: 32,
            continuation_reward_lamports: 1,
        },
        outcome_count,
        // The widths the accelerator campaign really executed against at this
        // exact outcome width. Reconciling them against the live local
        // deployment is the General-hot follow-up; founding does not read them.
        external_widths: dclutch_general_adapter_contract::account_rules_v3::GeneralExternalAccountWidthsV3 {
            linked_basis_prefix: 256,
            result_domain: 192,
            rent_sysvar: 17,
            core_market: 320,
            activation_cache: 160,
            upgradeable_program: 36,
            trading_programdata_prefix: 45,
            claims_programdata_prefix: 45,
            core_programdata_prefix: 45,
            realm_record: 112,
            rent_credit: 48,
        },
        token_account_bytes: 165,
        deployment: GeneralDeploymentFactsV1 {
            accelerator_artifact_release: lab("accelerator-artifact-release"),
            compiler_release: lab("compiler-release"),
            toolchain: lab("toolchain"),
            translation_validation: lab("translation-validation"),
        },
    };
    let closure = general_selected_closure_v1(release_input)?;
    let root_bytes = general_root_bytes_v1(&closure)?;
    let payload = general_selected_payload_v1(
        &closure,
        observation.activation_deadline_slot_v1()?,
        observation.root_rent_minimum_for_width_v1(root_bytes)?,
    );
    crate::selected_capability::attach_selected_capability_v1(&mut input, payload)?;
    crate::market::validate_market_input(&input)?;
    Ok(input)
}

/// The complete capability-root width the closure's own descriptor names.
fn general_root_bytes_v1(closure: &GeneralSelectedClosureBytesV1) -> Result<usize> {
    let descriptor = dclutch_capability_program_contract::v4::CapabilityProgramV4::decode(
        &closure.selected_descriptor,
    )
    .map_err(|error| Error::new(format!("General selected descriptor: {error:?}")))?;
    dclutch_capability_program_contract::CAPABILITY_ROOT_HEADER_BYTES_V1
        .checked_add(
            usize::try_from(descriptor.root_state_bytes())
                .map_err(|_| Error::new("General root state width overflow"))?,
        )
        .ok_or_else(|| Error::new("General root width overflow"))
}

/// Serialize one General closure into the family-neutral payload the driver
/// consumes — the single author for the label scheme and byte fields, shared
/// by the compiler and its tests.
fn general_selected_payload_v1(
    closure: &GeneralSelectedClosureBytesV1,
    activation_deadline_slot: u64,
    root_rent_minimum_lamports: u64,
) -> crate::model::SelectedCapabilityV1 {
    crate::model::SelectedCapabilityV1 {
        family: "general".into(),
        program_set_hex: crate::plan::hex(&closure.program_set),
        selected_descriptor_hex: crate::plan::hex(&closure.selected_descriptor),
        config_hex: crate::plan::hex(&closure.config),
        publication_hex: crate::plan::hex(&closure.publication),
        records: closure
            .records
            .iter()
            .enumerate()
            .map(|(index, record)| crate::model::SelectedCapabilityRecordV1 {
                // Positional prefix + `_record` suffix: unique, deterministic,
                // and covered by the founding checkpoint's record-graph census.
                label: format!(
                    "general_{index:02}_{}_record",
                    record.label.replace('-', "_")
                ),
                schema_hex: crate::plan::hex(&record.schema),
                body_hex: crate::plan::hex(&record.body),
            })
            .collect(),
        activation_deadline_slot,
        root_rent_minimum_lamports,
        selected_manifest_entry_index: 0,
    }
}

#[cfg(test)]
mod tests {
    use dclutch_market_core_codec::{Identity, MarketCoreStateSeedsV2, MarketIdentity};
    use dclutch_operator::general_selected_release_v1::{
        GeneralConfigWindowsV1, GeneralDeploymentFactsV1,
    };
    use solana_sdk::pubkey::Pubkey;

    use super::*;
    use crate::selected_capability::{
        SelectedCapabilityClosureV1, merge_selected_manifest_v1, selected_manifest_entry_v1,
        validate_selected_manifest_v1,
    };

    fn release_input() -> GeneralSelectedReleaseInputV1 {
        use dclutch_general_adapter_contract::account_rules_v3::GeneralExternalAccountWidthsV3;
        GeneralSelectedReleaseInputV1 {
            capacity_profile: [0x41; 32],
            claim_basis: [0x42; 32],
            selection_policy: [0x43; 32],
            quote_surplus_beneficiary: [0x44; 32],
            generation: 2,
            price_scale: 1_000_000,
            windows: GeneralConfigWindowsV1 {
                collection_slots: 16,
                selection_slots: 16,
                settlement_slots: 64,
                max_orders_per_candidate: 32,
                max_pages_per_candidate: 32,
                continuation_reward_lamports: 1,
            },
            outcome_count: 4,
            // The widths the accelerator campaign really executed against.
            external_widths: GeneralExternalAccountWidthsV3 {
                linked_basis_prefix: 256,
                result_domain: 192,
                rent_sysvar: 17,
                core_market: 320,
                activation_cache: 160,
                upgradeable_program: 36,
                trading_programdata_prefix: 45,
                claims_programdata_prefix: 45,
                core_programdata_prefix: 45,
                realm_record: 112,
                rent_credit: 48,
            },
            token_account_bytes: 165,
            deployment: GeneralDeploymentFactsV1 {
                accelerator_artifact_release: [0x51; 32],
                compiler_release: [0x52; 32],
                toolchain: [0x53; 32],
                translation_validation: [0x54; 32],
            },
        }
    }

    fn seam_closure(
        closure: &GeneralSelectedClosureBytesV1,
    ) -> SelectedCapabilityClosureV1<'_> {
        SelectedCapabilityClosureV1 {
            program_set: &closure.program_set,
            selected_descriptor: &closure.selected_descriptor,
            config: &closure.config,
            activation_deadline_slot: u64::MAX,
            root_rent_minimum_lamports: 1_000_000,
        }
    }

    fn base_manifest() -> Vec<u8> {
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
        let mut bytes = vec![0_u8; MANIFEST_HEADER_BYTES + entries.len() * CAPABILITY_ENTRY_BYTES];
        CapabilityManifestV1::encode_into(&entries, &mut bytes).expect("base manifest");
        bytes
    }

    fn market_for_manifest(manifest: &[u8]) -> [u8; 32] {
        let identity = |bytes: [u8; 32]| Identity::new(bytes).expect("identity");
        let manifest_digest: [u8; 32] = Sha256::digest(manifest).into();
        let template = MarketIdentity {
            market_id: identity([0xFF; 32]),
            realm_id: identity([0x61; 32]),
            product_record: identity([0x62; 32]),
            product_id: identity([0x63; 32]),
            resolution_policy: identity([0x64; 32]),
            capability_manifest: identity(manifest_digest),
            selected_release_set: identity([0x65; 32]),
            registry_program: identity([0x66; 32]),
            generation: 2,
        };
        Pubkey::find_program_address(
            &MarketCoreStateSeedsV2::new(template).as_slices(),
            &Pubkey::new_from_array([0xC0; 32]),
        )
        .0
        .to_bytes()
    }

    /// The seam consumes a second family with no family-shaped special case,
    /// and the entry it derives says exactly what the PUBLICATION says — the
    /// publication is the single author, checked across two independent
    /// derivations rather than asserted.
    #[test]
    fn the_neutral_seam_derives_the_entry_the_general_publication_authors() {
        let closure = general_selected_closure_v1(release_input()).expect("General closure");
        // The typed publication from an independent compile of the same
        // input; the compiler is deterministic and the assertion below on the
        // closure's own publication bytes proves the two runs agree.
        let publication = general_selected_release_v1(release_input())
            .expect("General release")
            .publication;
        assert_eq!(closure.publication, publication.to_bytes().to_vec());
        let entry = selected_manifest_entry_v1(seam_closure(&closure)).expect("neutral entry");

        assert_eq!(entry.kind_id().to_bytes(), publication.kind_id);
        assert_eq!(entry.release_id().to_bytes(), publication.program_set_id);
        assert_eq!(entry.config_id().to_bytes(), publication.config_id);
        assert_eq!(
            entry.capacity_profile_id().to_bytes(),
            publication.capacity_profile
        );

        let (manifest, index) =
            merge_selected_manifest_v1(&base_manifest(), entry).expect("merged manifest");
        validate_selected_manifest_v1(&manifest, entry, index).expect("validated manifest");

        // Every record the Registry must hold is enumerated with its schema
        // read off the release's own artifacts: 2 shared + 9 per action x 7.
        assert_eq!(closure.records.len(), 2 + 9 * 7);
    }

    /// The founding driver's own static gate accepts a General-selected
    /// market input end to end: the demo market graph with the General
    /// closure attached through the seam passes `validate_market_input` — the
    /// same validation every founding campaign runs before it signs anything.
    /// Direct appears nowhere in the input.
    #[test]
    fn the_founding_validator_accepts_a_general_selected_market_input() {
        let registry = Pubkey::new_from_array([0x77; 32]);
        let mut input = crate::market::demo_market_input_base(
            registry,
            dclutch_resolution_codec::RESOLUTION_CONTROLLER_RELEASE_ID_V7,
        )
        .expect("demo market base");

        // The same derivations the compiler makes from the base graph.
        let capacity_profile: [u8; 32] = Sha256::digest(
            crate::runtime::decode_hex(&input.source_capacity_profile_hex).expect("capacity"),
        )
        .into();
        let claim_basis = crate::market::semantic_basis_identity_v3(
            &crate::runtime::decode_hex(&input.linked_basis_hex).expect("basis"),
        )
        .expect("semantic basis identity");
        let outcome_count = u32::try_from(input.cuts.len() + 2).expect("width");
        let generation = input.generation + 1;
        let release_input = GeneralSelectedReleaseInputV1 {
            capacity_profile,
            claim_basis,
            generation,
            outcome_count,
            price_scale: 1_000_000,
            ..release_input()
        };
        let closure = general_selected_closure_v1(release_input).expect("General closure");
        let root_bytes = general_root_bytes_v1(&closure).expect("root width");
        assert!(root_bytes > dclutch_capability_program_contract::CAPABILITY_ROOT_HEADER_BYTES_V1);
        let payload = general_selected_payload_v1(&closure, u64::MAX, 1_000_000);

        crate::selected_capability::attach_selected_capability_v1(&mut input, payload)
            .expect("attach through the neutral seam");
        assert!(input.direct_capability.is_none());
        crate::market::validate_market_input(&input)
            .expect("the founding validator accepts the General-selected market input");
    }

    /// The counterpart of the Fractional fixed-point pin: General's config is
    /// market-free, so the selection PRECEDES the Market it will bind — the
    /// closure compiles with no market input (a type-level fact), the manifest
    /// it determines is stable, and the Market PDA that manifest derives is
    /// therefore well-defined. This is what makes a General-selected Market
    /// foundable where a Fractional-selected one is a SHA-256 fixed point.
    #[test]
    fn a_general_selection_precedes_the_market_it_will_bind() {
        let manifest_of = || {
            let closure = general_selected_closure_v1(release_input()).expect("General closure");
            let entry = selected_manifest_entry_v1(seam_closure(&closure)).expect("neutral entry");
            merge_selected_manifest_v1(&base_manifest(), entry)
                .expect("merged manifest")
                .0
        };
        let manifest_0 = manifest_of();
        let manifest_1 = manifest_of();
        assert_eq!(
            manifest_0, manifest_1,
            "the selection derives one manifest, byte-stable across compilations"
        );
        let market = market_for_manifest(&manifest_0);
        assert_eq!(
            market,
            market_for_manifest(&manifest_1),
            "and therefore one well-defined Market PDA before the Market exists"
        );
    }
}
