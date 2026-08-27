//! The relayed graduation market's run-spec input, compiled at run time.
//!
//! This is the §12.8 nine-record set as one coherent input: every identity the
//! `SourceMaterialV2` names IS the SHA-256 of a body this input carries, so
//! the producer's own validation proves the graph realizable before anything
//! is founded. The shape is §12.6's: a `ResultDomainV2` with **zero cuts** —
//! one ordinary region ("graduated by the deadline") plus the explicit failure
//! region — and **no recovery policy**, so the silent-relayer sibling walks
//! the funded `Primary → Exhausted → FailureCommitted` path.
//!
//! **The disclosed conflation, stated in the market's own source material.**
//! For this v1 graduation market, "the relayer went silent", "the venue was
//! upgraded" and "it never graduated" all land on the same pre-disclosed
//! failure outcome. That sentence is carried verbatim in the transcript and
//! in the campaign evidence; the Product's byte layout has no prose field,
//! and hiding the conflation in a doc nobody reads at founding would be the
//! discovery-at-resolution §12.6 forbids.

use sha2::{Digest, Sha256};
use solana_sdk::pubkey::Pubkey;

use dclutch_capability_contract::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CapabilityManifestV1,
    CompartmentFundingV1, ContentId as CapabilityContentId, FundingAmountsV1, FundingQuoteV1,
    MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
};
use dclutch_product_runtime_v2::{portfolio_record_bytes, result_domain_record_bytes};
use dclutch_product_runtime_v2_admission::PRODUCT_RECORD_BYTES_V2;
use dclutch_product_runtime_v2_operator::ProductCompilationInputV2;
use dclutch_product_runtime_v2_operator::compile_product_records_v2;
use dclutch_registry_contract::{ArtifactReleaseV1, ArtifactUpgradePolicyV1};
use dclutch_relay_contract::{
    RELAYED_FAMILY_RELEASE_ID_V1, RELAYED_RECORD_TRANSPORT_PROFILE_ID_V1,
    SOLANA_MAINNET_GENESIS_HASH_V1,
    identity::LOADER_V3_PROGRAM_ID,
    release::{
        AccountSetEntryV1, RelayedAdapterConfigV1, RelayerKeySetV1, account_set_id_preimage_len_v1,
        encode_account_set_id_preimage_v1,
    },
};
use dclutch_release_set_contract::ProgramIdentityV1;
use dclutch_resolution_codec::RESOLUTION_CONTROLLER_RELEASE_ID_V4;
use dclutch_source_contract::{
    CapacityEnvelope, ContentId as SourceContentId, ProviderReleaseV1,
    RELAYED_PROVIDER_EXTENSION_RELEASE_ID_V1, RoundingBoundary,
    SOURCE_FAILURE_POLICY_RELEASE_ID_V2, SourceAccessProfile, SourceCapacityProfileV1,
    SourceMaterialV2, SourceSpecV1, StatisticKind, StatisticSpecV1, WindowKind, WindowSpecV1,
};
use solana_sdk_ids::sysvar;

use crate::market::{
    compile_linked_basis_v3, demo_id, record_identity, semantic_basis_identity_v3,
};
use crate::model::MarketRunInput;
use crate::plan::hex;
use crate::twin;
use crate::{Error, Result};

/// The bounty the funded deadline walk pays whoever walks it. Disclosed at
/// founding through the capability manifest's own quote; asserted to the
/// lamport by the failure sibling.
pub(crate) const WALK_BOUNTY_LAMPORTS: u64 = 250_000;

/// §12.6's conflation, disclosed where the founding input is compiled.
pub(crate) const DISCLOSED_FAILURE_CONFLATION: &str = "For this v1 relayed graduation market, \
    'the relayer went silent', 'the venue was upgraded' and 'it never graduated' all land on the \
    same pre-disclosed failure outcome. Holders buy that conflation knowingly; it is the stated \
    cost of a 1-of-1 proof-of-authority relayer, not a surprise a resolution reveals.";

fn source_content(bytes: [u8; 32]) -> Result<SourceContentId> {
    SourceContentId::new(bytes).map_err(|error| Error::new(format!("source identity: {error:?}")))
}

/// The pinned ordered account set the daemon derives and the adapter compares.
pub(crate) fn account_set_entries() -> [AccountSetEntryV1; 4] {
    [
        AccountSetEntryV1 {
            key: twin::DBC_PROGRAM,
            expected_owner: LOADER_V3_PROGRAM_ID,
            inline_len: 36,
        },
        AccountSetEntryV1 {
            key: twin::DBC_PROGRAMDATA,
            expected_owner: LOADER_V3_PROGRAM_ID,
            inline_len: 45,
        },
        AccountSetEntryV1 {
            key: twin::pool_address(),
            expected_owner: twin::DBC_PROGRAM,
            inline_len: 424,
        },
        AccountSetEntryV1 {
            key: sysvar::clock::ID.to_bytes(),
            expected_owner: sysvar::ID.to_bytes(),
            inline_len: 40,
        },
    ]
}

/// The founding-time pinned set identity, exactly as the adapter re-derives
/// it: bound to the cluster the attestations CLAIM (mainnet), not to the twin.
pub(crate) fn account_set_id() -> Result<[u8; 32]> {
    let entries = account_set_entries();
    let width = account_set_id_preimage_len_v1(entries.len())
        .map_err(|error| Error::new(format!("account-set preimage width: {error:?}")))?;
    let mut preimage = vec![0u8; width];
    encode_account_set_id_preimage_v1(
        &mut preimage,
        SOLANA_MAINNET_GENESIS_HASH_V1,
        RELAYED_FAMILY_RELEASE_ID_V1,
        &entries,
    )
    .map_err(|error| Error::new(format!("account-set preimage: {error:?}")))?;
    Ok(Sha256::digest(&preimage).into())
}

/// Everything the campaign derives while compiling the input, kept so the
/// later stages authenticate against the same facts the founding pinned.
#[allow(dead_code)]
pub(crate) struct RelayedMarketFactsV1 {
    pub(crate) input: MarketRunInput,
    pub(crate) account_set_id: [u8; 32],
    pub(crate) relayer_key_set_bytes: Vec<u8>,
    pub(crate) relayer_key_set_digest: [u8; 32],
    pub(crate) relayed_adapter_config_bytes: Vec<u8>,
    pub(crate) relayed_adapter_config_digest: [u8; 32],
    pub(crate) venue_release_bytes: Vec<u8>,
    pub(crate) venue_release_digest: [u8; 32],
    pub(crate) source_spec_digest: [u8; 32],
    pub(crate) window: WindowSpecV1,
    pub(crate) window_digest: [u8; 32],
    pub(crate) material_digest: [u8; 32],
    pub(crate) product_record_digest: [u8; 32],
    pub(crate) result_domain_bytes: Vec<u8>,
    pub(crate) result_domain_digest: [u8; 32],
    pub(crate) portfolio_digest: [u8; 32],
    pub(crate) manifest_bytes: Vec<u8>,
    pub(crate) max_observation_age_seconds: u64,
}

/// The wall-clock terminal window, chosen per walk.
pub(crate) struct WindowChoiceV1 {
    pub(crate) start_unix_seconds: i64,
    pub(crate) end_unix_seconds: i64,
    pub(crate) max_age_seconds: u32,
}

/// The §12.3 discipline, applied to a rehearsal's clocks: the success walk
/// needs a window the observation lands inside with real freshness margin;
/// the failure walk needs a deadline the campaign can honestly outwait.
pub(crate) fn window_choice(now_unix_seconds: i64, success: bool) -> WindowChoiceV1 {
    if success {
        WindowChoiceV1 {
            start_unix_seconds: now_unix_seconds - 300,
            end_unix_seconds: now_unix_seconds + 3_600,
            max_age_seconds: 900,
        }
    } else {
        // end + max_age is the primary deadline; the campaign waits it out in
        // real time, so the whole budget is ~verification-speed rather than a
        // market-realistic width. The disclosed failure outcome is reached
        // because the deadline passed with no sealed record — exactly §12.7.
        WindowChoiceV1 {
            start_unix_seconds: now_unix_seconds - 300,
            end_unix_seconds: now_unix_seconds + 240,
            max_age_seconds: 150,
        }
    }
}

const MAX_CLUSTER_SKEW_SECONDS: u64 = 120;

/// Compile the whole relayed market input from the run-time facts.
pub(crate) fn relayed_market_input(
    registry: Pubkey,
    relayer_pubkey: [u8; 32],
    window_choice: &WindowChoiceV1,
) -> Result<RelayedMarketFactsV1> {
    let set_id = account_set_id()?;

    // 2. RelayerKeySetV1, n = 1, m = 1, over the daemon's disclosed key.
    let key_set = RelayerKeySetV1::new(&[relayer_pubkey], 1)
        .map_err(|error| Error::new(format!("relayer key set: {error:?}")))?;
    let key_set_bytes = key_set
        .to_bytes()
        .map_err(|error| Error::new(format!("relayer key set bytes: {error:?}")))?
        .to_vec();
    let key_set_digest = record_identity(&key_set_bytes);

    // 3. RelayedAdapterConfigV1 over the pinned set. The staleness bound must
    //    dominate the cluster-skew bound (§10.6's record-creation precondition
    //    reads the WINDOW's max_age against the config's skew).
    let max_observation_age_seconds = u64::from(window_choice.max_age_seconds);
    if max_observation_age_seconds <= MAX_CLUSTER_SKEW_SECONDS {
        return Err(Error::new(
            "the window's max_age must strictly dominate the cluster-skew bound",
        ));
    }
    let adapter_config = RelayedAdapterConfigV1::new(
        set_id,
        0,
        0,
        max_observation_age_seconds,
        MAX_CLUSTER_SKEW_SECONDS,
    )
    .map_err(|error| Error::new(format!("relayed adapter config: {error:?}")))?;
    let adapter_config_bytes = adapter_config
        .to_bytes()
        .map_err(|error| Error::new(format!("relayed adapter config bytes: {error:?}")))?
        .to_vec();
    let adapter_config_digest = record_identity(&adapter_config_bytes);

    // 4. ProviderReleaseV1: family, extension, key set, decoding rules,
    //    record transport.
    let provider_release = ProviderReleaseV1::new(
        source_content(RELAYED_FAMILY_RELEASE_ID_V1)?,
        source_content(RELAYED_PROVIDER_EXTENSION_RELEASE_ID_V1)?,
        source_content(key_set_digest)?,
        source_content(adapter_config_digest)?,
        source_content(RELAYED_RECORD_TRANSPORT_PROFILE_ID_V1)?,
    );
    let provider_release_bytes = provider_release.to_bytes();
    let provider_release_digest = record_identity(&provider_release_bytes);

    // 1. The venue's pinned deployment (P-B). Program and ProgramData
    //    addresses are the real mainnet ones; slot, digest and authority are
    //    the twin's synthetic-of-real facts — the dossier never read the real
    //    values, and this lane makes no public reads.
    let venue_release = ArtifactReleaseV1::new(
        ProgramIdentityV1::new(twin::DBC_PROGRAM)
            .map_err(|error| Error::new(format!("venue program: {error:?}")))?,
        ProgramIdentityV1::new(LOADER_V3_PROGRAM_ID)
            .map_err(|error| Error::new(format!("loader: {error:?}")))?,
        twin::DBC_PROGRAMDATA,
        dclutch_core_contract::ContentId::new(demo_id(
            "relayed/venue-semantic-release/meteora-dbc",
            &[],
        ))
        .map_err(|error| Error::new(format!("venue semantic release: {error:?}")))?,
        twin::synthetic_elf_digest(),
        twin::SYNTHETIC_DEPLOYMENT_SLOT,
        ArtifactUpgradePolicyV1::ExactAuthority,
        Some(twin::upgrade_authority()),
    )
    .map_err(|error| Error::new(format!("venue artifact release: {error:?}")))?;
    let venue_release_bytes = venue_release.to_bytes().to_vec();
    let venue_release_digest = record_identity(&venue_release_bytes);

    // The Product identities. The coordinate domain is the venue's
    // MigrationProgress discriminant itself, at exponent zero (§12.6).
    let product_identity = demo_id("relayed/product/dbc-graduation", &[&set_id]);
    let coordinate_domain = demo_id("relayed/coordinate-domain/dbc-migration-progress", &[]);
    let result_unit = demo_id("relayed/result-unit/migration-progress-discriminant", &[]);
    let claim_basis = demo_id("claim-basis/unit-complete-set", &[]);
    let representation = demo_id("representation/categorical-fixed-width", &[]);
    let mapping = demo_id("mapping/scaled-integer-cut", &[&coordinate_domain]);

    let capacity = SourceCapacityProfileV1::new(
        CapacityEnvelope::Measured,
        1,
        0,
        source_content(demo_id("relayed/capacity/terminal-verifier", &[]))?,
        source_content(demo_id("relayed/capacity/envelope-basis", &[]))?,
        512,
        4,
    )
    .map_err(|error| Error::new(format!("relayed source capacity: {error:?}")))?;
    let capacity_id = source_content(record_identity(&capacity.to_bytes()))?;

    // 5. SourceSpecV1: coordinate/unit equal to the Product domain's, relayed
    //    access profile, and the venue release as the pinned deployment.
    let source_spec = SourceSpecV1::new(
        source_content(coordinate_domain)?,
        source_content(result_unit)?,
        source_content(provider_release_digest)?,
        SourceAccessProfile::RelayedObservationRecord,
        source_content(venue_release_digest)?,
        capacity_id,
    );
    let source_spec_bytes = source_spec.to_bytes();
    let source_spec_digest = record_identity(&source_spec_bytes);

    // 6. WindowSpecV1, Terminal, with real width (§12.3, TWIN's finding).
    let window = WindowSpecV1::new(
        source_content(source_spec_digest)?,
        WindowKind::Terminal,
        window_choice.start_unix_seconds,
        window_choice.end_unix_seconds,
        window_choice.max_age_seconds,
        1,
        source_content(demo_id("window-schedule/terminal-single-sample", &[]))?,
    )
    .map_err(|error| Error::new(format!("relayed terminal window: {error:?}")))?;
    let window_bytes = window.to_bytes();
    let window_digest = record_identity(&window_bytes);

    // 7. StatisticSpecV1, TerminalSample, ExactRational.
    let statistic = StatisticSpecV1::new(
        source_content(result_unit)?,
        source_content(result_unit)?,
        StatisticKind::TerminalSample,
        RoundingBoundary::ExactRational,
        1,
        0,
        capacity_id,
        source_content(demo_id("statistic-evaluator/terminal-sample", &[]))?,
        capacity,
    )
    .map_err(|error| Error::new(format!("relayed terminal statistic: {error:?}")))?;
    let statistic_bytes = statistic.to_bytes();
    let statistic_digest = record_identity(&statistic_bytes);

    // 9. Product Runtime V2: zero cuts — one ordinary region plus the
    //    explicit failure region. A cut at CreatedPool would mint an ordinary
    //    cell nothing could ever select (§12.6).
    let cut_denominator = 1_u64;
    let cuts: Vec<i128> = vec![];
    let coefficients: Vec<u64> = vec![1, 0];
    let outcome_count = coefficients.len();
    let evaluator_release = demo_id("liability-basis/categorical-unit-evaluator", &[]);
    let liability_basis = semantic_basis_identity_v3(&compile_linked_basis_v3(
        product_identity,
        product_identity,
        coordinate_domain,
        result_unit,
        evaluator_release,
        outcome_count,
    )?)?;

    let product = ProductCompilationInputV2 {
        product_id: product_content(product_identity)?,
        coordinate_domain_id: product_content(coordinate_domain)?,
        result_unit_id: product_content(result_unit)?,
        claim_basis_id: product_content(claim_basis)?,
        liability_basis_id: product_content(liability_basis)?,
        representation_release_id: product_content(representation)?,
        mapping_release_id: product_content(mapping)?,
        cut_denominator,
        cuts: &cuts,
        portfolio_denominator: 1,
        coefficients: &coefficients,
    };
    let mut product_bytes = [0_u8; PRODUCT_RECORD_BYTES_V2];
    let mut domain = vec![
        0_u8;
        result_domain_record_bytes(cuts.len()).map_err(|error| Error::new(
            format!("relayed domain width: {error:?}")
        ))?
    ];
    let mut portfolio = vec![
        0_u8;
        portfolio_record_bytes(outcome_count).map_err(|error| Error::new(
            format!("relayed portfolio width: {error:?}")
        ))?
    ];
    compile_product_records_v2(
        registry,
        product,
        &mut product_bytes,
        &mut domain,
        &mut portfolio,
    )
    .map_err(|error| Error::new(format!("relayed Product compiler: {error:?}")))?;
    let product_record_digest: [u8; 32] = Sha256::digest(product_bytes).into();
    let domain_digest: [u8; 32] = Sha256::digest(&domain).into();
    let portfolio_digest: [u8; 32] = Sha256::digest(&portfolio).into();
    let linked_basis = compile_linked_basis_v3(
        product_identity,
        domain_digest,
        coordinate_domain,
        result_unit,
        evaluator_release,
        outcome_count,
    )?;
    if semantic_basis_identity_v3(&linked_basis)? != liability_basis {
        return Err(Error::new(
            "linking the relayed liability basis to its Product changed its semantic identity",
        ));
    }

    // 8. SourceMaterialV2 with NO recovery policy: the §12.7 shape whose
    //    silent-provider path is the funded deadline walk.
    let material = SourceMaterialV2::new(
        source_content(product_record_digest)?,
        source_content(source_spec_digest)?,
        source_content(window_digest)?,
        source_content(statistic_digest)?,
        None,
        source_content(SOURCE_FAILURE_POLICY_RELEASE_ID_V2)?,
    );
    let material_digest: [u8; 32] = Sha256::digest(material.to_bytes()).into();

    // The manifest: three Resolution-controller entries. The failure entry is
    // configured by this market's own Source material; the two others are the
    // structural companions the no-recovery admission requires — prepaid,
    // never walked, refunded at close.
    let rent_compartment = CompartmentFundingV1::native_lamports(1)
        .map_err(|error| Error::new(format!("funding compartment: {error:?}")))?;
    let bounty_compartment = CompartmentFundingV1::native_lamports(WALK_BOUNTY_LAMPORTS)
        .map_err(|error| Error::new(format!("bounty compartment: {error:?}")))?;
    let none = CompartmentFundingV1::not_applicable();
    let amounts = FundingAmountsV1::new(
        rent_compartment,
        rent_compartment,
        none,
        none,
        bounty_compartment,
        none,
        none,
    )
    .map_err(|error| Error::new(format!("funding amounts: {error:?}")))?;
    let quote = FundingQuoteV1::new(amounts, None)
        .map_err(|error| Error::new(format!("funding quote: {error:?}")))?;
    let release = capability_content(RESOLUTION_CONTROLLER_RELEASE_ID_V4)?;
    let mut entries_input: Vec<([u8; 32], [u8; 32])> = vec![
        (
            demo_id("relayed/capability/recovery-companion", &[&set_id]),
            demo_id("relayed/companion-config/recovery", &[&set_id]),
        ),
        (
            demo_id("relayed/capability/exhaustion-companion", &[&set_id]),
            demo_id("relayed/companion-config/exhaustion", &[&set_id]),
        ),
        (
            demo_id("relayed/capability/source-material", &[&set_id]),
            material_digest,
        ),
    ];
    entries_input.sort_by_key(|entry| entry.0);
    let mut entries = Vec::new();
    for (index, (kind, config)) in entries_input.into_iter().enumerate() {
        let entry_index =
            u16::try_from(index).map_err(|_| Error::new("capability index overflow"))?;
        entries.push(
            CapabilityEntryV1::new(
                capability_content(kind)?,
                release,
                capability_content(config)?,
                capability_content(demo_id("capability/capacity", &[&[entry_index as u8]]))?,
                capability_content(demo_id("capability/schema", &[]))?,
                capability_content(demo_id("capability/derivation", &[]))?,
                ActivationPolicy::RequiredAtFounding,
                0,
                0,
                [0; MAX_DEPENDENCIES_PER_CAPABILITY],
                quote,
            )
            .map_err(|error| Error::new(format!("capability entry: {error:?}")))?,
        );
    }
    let mut manifest = vec![0_u8; MANIFEST_HEADER_BYTES + entries.len() * CAPABILITY_ENTRY_BYTES];
    CapabilityManifestV1::encode_into(&entries, &mut manifest)
        .map_err(|error| Error::new(format!("capability manifest: {error:?}")))?;

    let input = MarketRunInput {
        generation: 1,
        collateral_display_decimals: 6,
        initial_collateral_atoms: 1_000_000_000,
        product_id: hex(&product_identity),
        coordinate_domain_id: hex(&coordinate_domain),
        result_unit_id: hex(&result_unit),
        claim_basis_id: hex(&claim_basis),
        liability_basis_id: hex(&liability_basis),
        representation_release_id: hex(&representation),
        mapping_release_id: hex(&mapping),
        cut_denominator,
        cuts: Vec::new(),
        portfolio_denominator: 1,
        coefficients,
        primary_source_spec_id: hex(&source_spec_digest),
        window_spec_id: hex(&window_digest),
        statistic_spec_id: hex(&statistic_digest),
        failure_policy_release_id: hex(&SOURCE_FAILURE_POLICY_RELEASE_ID_V2),
        source_spec_hex: hex(&source_spec_bytes),
        window_spec_hex: hex(&window_bytes),
        statistic_spec_hex: hex(&statistic_bytes),
        provider_release_hex: hex(&provider_release_bytes),
        // For the relayed family the spec's adapter_config_id names the
        // VENUE's ArtifactReleaseV1 (§12.4); the producer publishes it under
        // the artifact-release schema by reading the provider release's own
        // extension.
        pyth_adapter_config_hex: hex(&venue_release_bytes),
        recovery_policy_hex: String::new(),
        capability_manifest_hex: hex(&manifest),
        linked_basis_hex: hex(&linked_basis),
    };
    crate::market::validate_market_input(&input)?;

    Ok(RelayedMarketFactsV1 {
        input,
        account_set_id: set_id,
        relayer_key_set_bytes: key_set_bytes,
        relayer_key_set_digest: key_set_digest,
        relayed_adapter_config_bytes: adapter_config_bytes,
        relayed_adapter_config_digest: adapter_config_digest,
        venue_release_bytes,
        venue_release_digest,
        source_spec_digest,
        window,
        window_digest,
        material_digest,
        product_record_digest,
        result_domain_bytes: domain,
        result_domain_digest: domain_digest,
        portfolio_digest,
        manifest_bytes: manifest,
        max_observation_age_seconds,
    })
}

fn product_content(bytes: [u8; 32]) -> Result<dclutch_product_runtime_v2::ContentId> {
    dclutch_product_runtime_v2::ContentId::new(bytes)
        .map_err(|error| Error::new(format!("product identity: {error:?}")))
}

fn capability_content(bytes: [u8; 32]) -> Result<CapabilityContentId> {
    CapabilityContentId::new(bytes)
        .map_err(|error| Error::new(format!("capability identity: {error:?}")))
}
