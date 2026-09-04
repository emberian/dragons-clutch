//! The relayed graduation market's run-spec input, compiled at run time.
//!
//! One author, two consumers: the relayed-vertical campaign feeds this the
//! TWIN's synthetic-of-real venue facts (its `input.rs` shim), and the
//! external driver's `graduation-market` subcommand feeds it venue facts READ
//! OFF REAL MAINNET plus the operated relayer's disclosed attestation key.
//! The venue facts are a parameter precisely so those two cannot drift in
//! graph shape - only in whose facts they pin.
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
    decode::RelayedObservableV1,
    identity::LOADER_V3_PROGRAM_ID,
    release::{
        AccountSetEntryV1, RelayedAdapterConfigV1, RelayerKeySetV1, account_set_id_preimage_len_v1,
        encode_account_set_id_preimage_v1,
    },
};
use dclutch_release_set_contract::ProgramIdentityV1;
use dclutch_source_contract::{
    BONDING_CURVE_FLOOR_DERIVATION_ID_V1, BONDING_CURVE_GRADUATION_FLOOR_LAMPORTS_V1,
    CHAIN_STATE_DEFAULT_KAPPA_DENOMINATOR_V1, CHAIN_STATE_DEFAULT_KAPPA_NUMERATOR_V1,
    CapacityEnvelope, ContentId as SourceContentId, ManipulationFloorBasis, ManipulationFloorV1,
    PRINCIPAL_CAPACITY_LIFTING_PLAN_ID_V1, ProviderReleaseV1,
    RELAYED_PROVIDER_EXTENSION_RELEASE_ID_V1, RoundingBoundary,
    SOURCE_FAILURE_POLICY_RELEASE_ID_V2, SourceAccessProfile, SourceCapacityProfileV1,
    SourceMaterialV3, SourceSpecV1, StatisticKind, StatisticSpecV1, WindowKind, WindowSpecV1,
    admit_founding_principal,
};
use solana_sdk_ids::sysvar;

use crate::direct_market::{DirectMarketCompilerInputV1, attach_direct_market_capability_v1};
use crate::market::{
    compile_linked_basis_v3, demo_id, record_identity, semantic_basis_identity_v3,
};
use crate::model::{FoundingRouteV1, MarketRunInput};
use crate::plan::hex;
use crate::{Error, Result};

/// The bounty the funded deadline walk pays whoever walks it. Disclosed at
/// founding through the capability manifest's own quote; asserted to the
/// lamport by the failure sibling.
pub(crate) const WALK_BOUNTY_LAMPORTS: u64 = 250_000;

/// Ceiling on any one outcome's share of a relayed market's ex-ante mass.
///
/// The same 9,000 the price markets state. A proposition the author believed
/// at 95%, or at 5%, is as much a foregone conclusion as a price partition
/// whose middle cell takes everything, and refuses through the same gate.
pub(crate) const RELAYED_MAX_OUTCOME_SHARE_BPS_V1: u32 = 9_000;

/// Everything about a relayed market that is the OBSERVABLE ROW's to say.
///
/// One arm per row of the decoding-rules table. Adding row 2 is adding an arm
/// here; `relayed_market_input` reads this struct and names no venue.
struct RelayedRowFactsV1 {
    /// The venue program's semantic release identity.
    venue_semantic_release: &'static str,
    /// The Product's stable semantic identity, seeded by the account set.
    product: &'static str,
    /// The coordinate domain: this row's own discriminant.
    coordinate_domain: &'static str,
    /// The result unit the discriminant is quoted in.
    result_unit: &'static str,
    /// This fixture's author's stated ex-ante probability that the row's
    /// proposition is proved inside the window, in basis points.
    ///
    /// A DECLARATION, not a derivation, exactly as a price fixture's
    /// volatility is. The shortfall from 10,000 is the disclosed failure
    /// outcome's own stated share, which is why a zero-cut market is a
    /// question at all: its single ordinary cell does not take the market.
    prior_bps: u32,
}

impl RelayedRowFactsV1 {
    const fn for_observable(observable: RelayedObservableV1) -> Self {
        match observable {
            RelayedObservableV1::DbcMigrationProgressV1 => Self {
                venue_semantic_release: "relayed/venue-semantic-release/meteora-dbc",
                product: "relayed/product/dbc-graduation",
                coordinate_domain: "relayed/coordinate-domain/dbc-migration-progress",
                result_unit: "relayed/result-unit/migration-progress-discriminant",
                prior_bps: 3_500,
            },
            RelayedObservableV1::Token2022MintAuthorityRenouncedV1 => Self {
                venue_semantic_release: "relayed/venue-semantic-release/spl-token-2022",
                product: "relayed/product/mint-authority-renounced",
                coordinate_domain: "relayed/coordinate-domain/mint-authority-state",
                result_unit: "relayed/result-unit/mint-authority-discriminant",
                // A launch that has published a renunciation intention but not
                // executed it. Stated, not derived.
                prior_bps: 4_000,
            },
        }
    }
}

/// §12.6's conflation, disclosed where the founding input is compiled.
pub(crate) const DISCLOSED_FAILURE_CONFLATION: &str = "For this v1 relayed graduation market, \
    'the relayer went silent', 'the venue was upgraded' and 'it never graduated' all land on the \
    same pre-disclosed failure outcome. Holders buy that conflation knowingly; it is the stated \
    cost of a 1-of-1 proof-of-authority relayer, not a surprise a resolution reveals.";

fn source_content(bytes: [u8; 32]) -> Result<SourceContentId> {
    SourceContentId::new(bytes).map_err(|error| Error::new(format!("source identity: {error:?}")))
}

/// The pinned ordered account set the daemon derives and the adapter compares.
/// The venue deployment one relayed market pins: WHOSE facts these are is the
/// caller's statement (the twin's synthetic-of-real set, or a real mainnet
/// read), and everything downstream inherits that label.
#[derive(Clone, Copy)]
pub(crate) struct RelayedVenueFactsV1 {
    /// The venue program address (Meteora DBC's real mainnet address in both
    /// known callers).
    pub(crate) program: [u8; 32],
    pub(crate) programdata: [u8; 32],
    /// The watched `VirtualPool` account.
    pub(crate) pool: [u8; 32],
    /// SHA-256 of the venue ProgramData's ELF tail (bytes 45..).
    pub(crate) elf_digest: [u8; 32],
    pub(crate) deployment_slot: u64,
    pub(crate) upgrade_authority: [u8; 32],
}

pub(crate) fn account_set_entries(
    observable: RelayedObservableV1,
    venue: &RelayedVenueFactsV1,
) -> [AccountSetEntryV1; 4] {
    // The state position's pinned width comes off the observable's own row
    // rather than out of this file: 424 for a DBC `VirtualPool`, 82 for a
    // base SPL Token-2022 `Mint`, and the decoding rules prove each equals the
    // only length that row admits. Typing it here would be a second author.
    [
        AccountSetEntryV1 {
            key: venue.program,
            expected_owner: LOADER_V3_PROGRAM_ID,
            inline_len: 36,
        },
        AccountSetEntryV1 {
            key: venue.programdata,
            expected_owner: LOADER_V3_PROGRAM_ID,
            inline_len: 45,
        },
        AccountSetEntryV1 {
            key: venue.pool,
            expected_owner: venue.program,
            inline_len: observable.state_inline_bytes(),
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
pub(crate) fn account_set_id(
    observable: RelayedObservableV1,
    venue: &RelayedVenueFactsV1,
) -> Result<[u8; 32]> {
    let entries = account_set_entries(observable, venue);
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
    /// The statistic the consumption frame now names. It was already founded
    /// and already named by the material; nothing could read it.
    pub(crate) statistic_digest: [u8; 32],
    pub(crate) material_digest: [u8; 32],
    pub(crate) product_record_digest: [u8; 32],
    pub(crate) result_domain_bytes: Vec<u8>,
    pub(crate) result_domain_digest: [u8; 32],
    pub(crate) portfolio_digest: [u8; 32],
    pub(crate) manifest_bytes: Vec<u8>,
    pub(crate) max_observation_age_seconds: u64,
    pub(crate) manipulation_floor_bytes: Vec<u8>,
    pub(crate) manipulation_floor_digest: [u8; 32],
    pub(crate) admitted_principal_atoms: u128,
    pub(crate) admitted_principal_cap_atoms: u128,
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
///
/// Called by the relayed-vertical rehearsal only — the external driver's
/// graduation market states its window explicitly, so this is dead code in
/// the successor binary by design.
#[allow(dead_code)]
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

pub(crate) const MAX_CLUSTER_SKEW_SECONDS: u64 = 120;

/// Compile the whole relayed market input from the run-time facts.
///
/// `observable` names the row of the decoding-rules table this market resolves
/// under. Every place the row's shape shows through -- the state position's
/// pinned inline width, the adapter config's selector and declared exponent,
/// the venue's semantic release, the Product's coordinate-domain and
/// result-unit identities, and the author's stated prior -- reads it, so a
/// third row is another arm of `RelayedRowFactsV1::for_observable` and no edit
/// to this function.
pub(crate) fn relayed_market_input(
    registry: Pubkey,
    relayer_pubkey: [u8; 32],
    window_choice: &WindowChoiceV1,
    observable: RelayedObservableV1,
    venue: &RelayedVenueFactsV1,
    direct: DirectMarketCompilerInputV1<'_>,
) -> Result<RelayedMarketFactsV1> {
    let row = RelayedRowFactsV1::for_observable(observable);
    let set_id = account_set_id(observable, venue)?;

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
        observable.selector(),
        observable.raw_exponent(),
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

    // 1. The venue's pinned deployment (P-B), from the caller's venue facts —
    //    the twin's synthetic-of-real set for the rehearsal, or a real mainnet
    //    read for the operated market.
    let venue_release = ArtifactReleaseV1::new(
        ProgramIdentityV1::new(venue.program)
            .map_err(|error| Error::new(format!("venue program: {error:?}")))?,
        ProgramIdentityV1::new(LOADER_V3_PROGRAM_ID)
            .map_err(|error| Error::new(format!("loader: {error:?}")))?,
        venue.programdata,
        dclutch_core_contract::ContentId::new(demo_id(row.venue_semantic_release, &[]))
            .map_err(|error| Error::new(format!("venue semantic release: {error:?}")))?,
        venue.elf_digest,
        venue.deployment_slot,
        ArtifactUpgradePolicyV1::ExactAuthority,
        Some(venue.upgrade_authority),
    )
    .map_err(|error| Error::new(format!("venue artifact release: {error:?}")))?;
    let venue_release_bytes = venue_release.to_bytes().to_vec();
    let venue_release_digest = record_identity(&venue_release_bytes);

    // The Product identities. The coordinate domain is the observed venue's
    // own discriminant, at exponent zero (§12.6). Distinct per row, so a
    // Product carving one observable cannot be resolved by the other.
    let product_identity = demo_id(row.product, &[&set_id]);
    let coordinate_domain = demo_id(row.coordinate_domain, &[]);
    let result_unit = demo_id(row.result_unit, &[]);
    let claim_basis = demo_id("claim-basis/unit-complete-set", &[]);
    let representation = demo_id("representation/categorical-fixed-width", &[]);
    let mapping = demo_id("mapping/scaled-integer-cut", &[&coordinate_domain]);

    // §6.5's κ, stated on the profile (KAPPA's founding guard). κ is
    // PROVISIONAL and its lifting plan IS the profile's own envelope_basis_id,
    // so the AGENTS.md obligation is discharged with no parallel mechanism.
    let capacity = SourceCapacityProfileV1::new(
        CapacityEnvelope::Provisional,
        1,
        0,
        source_content(demo_id("relayed/capacity/terminal-verifier", &[]))?,
        source_content(PRINCIPAL_CAPACITY_LIFTING_PLAN_ID_V1)?,
        512,
        4,
    )
    .map_err(|error| Error::new(format!("relayed source capacity: {error:?}")))?
    .bounding_principal(
        CHAIN_STATE_DEFAULT_KAPPA_NUMERATOR_V1,
        CHAIN_STATE_DEFAULT_KAPPA_DENOMINATOR_V1,
    )
    .map_err(|error| Error::new(format!("relayed source kappa: {error:?}")))?;
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

    // The venue's manipulation floor (§5.4, curve-derived: a curve floor does
    // not fall as the coin's liquidity thins). Its identity is the digest of
    // its own body, like every other record in this graph, and its venue
    // binding is the adapter_config_id the Source itself names — for the
    // relayed family, the venue's ArtifactReleaseV1 (§12.4). The collateral
    // unit is the rehearsal Realm's native-lamport stand-in: one collateral
    // atom represents one lamport, which is the unit the curve floor is
    // denominated in.
    let collateral_unit_id = source_content(demo_id(
        "relayed/collateral-unit/realm-native-lamports",
        &[],
    ))?;
    let manipulation_floor = ManipulationFloorV1::new(
        ManipulationFloorBasis::CurveDerived,
        source_content(source_spec_digest)?,
        source_content(venue_release_digest)?,
        collateral_unit_id,
        source_content(BONDING_CURVE_FLOOR_DERIVATION_ID_V1)?,
        BONDING_CURVE_GRADUATION_FLOOR_LAMPORTS_V1,
    );
    let manipulation_floor_bytes = manipulation_floor.to_bytes().to_vec();
    let manipulation_floor_digest = record_identity(&manipulation_floor_bytes);

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
    // One unit identity on both sides: this family's observations are already
    // counted in the unit its cuts are authored in, so it declares no
    // conversion and the only admissible factor is the identity. The record
    // now says so with a number rather than by the reader inferring it, and
    // `StatisticSpecV1::validate_scale` refuses any other value here.
    let statistic = StatisticSpecV1::new(
        source_content(result_unit)?,
        source_content(result_unit)?,
        0,
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

    // 8. SourceMaterialV3 with a bounded floor and no recovery policy: the §12.7 shape whose
    //    silent-provider path is the funded deadline walk.
    let material = SourceMaterialV3::bounded_by_floor(
        source_content(product_record_digest)?,
        source_content(source_spec_digest)?,
        source_content(window_digest)?,
        source_content(statistic_digest)?,
        None,
        source_content(SOURCE_FAILURE_POLICY_RELEASE_ID_V2)?,
        source_content(manipulation_floor_digest)?,
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
    // The checked Direct compiler is the semantic owner for the activated
    // Resolution role. Reuse that exact release identity so the relayed
    // controller ledger cannot drift behind the selected executable.
    let release = capability_content(direct.resolution_release)?;
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

    let mut input = MarketRunInput {
        // This market has NO CUTS -- one ordinary cell over the whole domain
        // plus the failure outcome -- and it DOES have a belief. It just is
        // not a random walk around a positive spot: its coordinate is the
        // discriminant of a four-state enum, and "volatility in basis points
        // of spot" denotes nothing about it.
        //
        // The belief is `P(this pool graduates inside the window) = 35%`, and
        // the other 6,500 bps are the disclosed failure outcome's own stated
        // share -- the conflation DISCLOSED_FAILURE_CONFLATION names, priced
        // ex ante instead of merely admitted. This is a DECLARATION and not a
        // derivation, exactly as the SOL/USD fixture's volatility is: it is
        // what this fixture's author believes, written down where the compiler
        // can hold them to it.
        //
        // The earlier `None` here read "it declares no belief rather than
        // fabricating one it would never be measured against". Right about the
        // ethics, wrong about the mechanism: there was a belief to declare and
        // no kind of belief to declare it in.
        founding_band: Some(crate::model::FoundingBandInputV1::proposition(
            vec![row.prior_bps],
            RELAYED_MAX_OUTCOME_SHARE_BPS_V1,
        )),
        generation: 1,
        collateral_display_decimals: 6,
        local_participant_fixture_liquidity_atoms: 0,
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
        source_capacity_profile_hex: hex(&capacity.to_bytes()),
        manipulation_floor_hex: hex(&manipulation_floor_bytes),
        window_spec_hex: hex(&window_bytes),
        statistic_spec_hex: hex(&statistic_bytes),
        provider_release_hex: hex(&provider_release_bytes),
        // For the relayed family the spec's adapter_config_id names the
        // VENUE's ArtifactReleaseV1 (§12.4); the producer publishes it under
        // the artifact-release schema by reading the provider release's own
        // extension.
        pyth_adapter_config_hex: hex(&venue_release_bytes),
        pyth_sponsored_push_release_hex: String::new(),
        recovery_policy_hex: String::new(),
        capability_manifest_hex: hex(&manifest),
        direct_capability: None,
        selected_capability: None,
        linked_basis_hex: hex(&linked_basis),
        price_gate_hex: String::new(),
        founding_route: FoundingRouteV1::Atomic,
    };
    attach_direct_market_capability_v1(&mut input, direct)?;
    crate::market::validate_market_input(&input)?;

    // The market founds UNDER the capacity predicate, host-side (the on-chain
    // closure is queued behind W1b's founding-root ADR; until then the
    // campaign that founds is the enforcement site). Hoard principal is the
    // complete-set quantity times the basis scale — the producer founds
    // quantity = initial_collateral_atoms / 2 at categorical scale 1
    // (market.rs, "The complete-set quantity times the basis scale is the
    // Hoard principal").
    let total_principal_atoms = u128::from(input.initial_collateral_atoms / 2);
    let capacity_kappa = capacity
        .principal_capacity()
        .map_err(|error| Error::new(format!("relayed kappa read-back: {error:?}")))?;
    admit_founding_principal(
        capacity_kappa,
        manipulation_floor,
        source_content(source_spec_digest)?,
        source_spec,
        collateral_unit_id,
        total_principal_atoms,
    )
    .map_err(|error| {
        Error::new(format!(
            "the founding principal ({total_principal_atoms} atoms) is refused by kappa = \
             {CHAIN_STATE_DEFAULT_KAPPA_NUMERATOR_V1}/{CHAIN_STATE_DEFAULT_KAPPA_DENOMINATOR_V1} \
             against the {BONDING_CURVE_GRADUATION_FLOOR_LAMPORTS_V1}-lamport curve floor: \
             {error:?}"
        ))
    })?;
    // The bound is exercised at its exact boundary, not just under it: the
    // largest admitted principal at kappa = 1/4 against the 18.618074 SOL
    // floor is 4,654,518,500 lamports, and one more atom refuses.
    let admitted_principal_cap_atoms = u128::from(CHAIN_STATE_DEFAULT_KAPPA_NUMERATOR_V1)
        * u128::from(BONDING_CURVE_GRADUATION_FLOOR_LAMPORTS_V1)
        / u128::from(CHAIN_STATE_DEFAULT_KAPPA_DENOMINATOR_V1);
    if capacity_kappa
        .admit(
            manipulation_floor.floor_atoms(),
            admitted_principal_cap_atoms,
        )
        .is_err()
        || capacity_kappa
            .admit(
                manipulation_floor.floor_atoms(),
                admitted_principal_cap_atoms + 1,
            )
            .is_ok()
    {
        return Err(Error::new(
            "the kappa admission boundary is not where the predicate says it is",
        ));
    }

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
        statistic_digest,
        material_digest,
        product_record_digest,
        result_domain_bytes: domain,
        result_domain_digest: domain_digest,
        portfolio_digest,
        manifest_bytes: manifest,
        max_observation_age_seconds,
        manipulation_floor_bytes,
        manipulation_floor_digest,
        admitted_principal_atoms: total_principal_atoms,
        admitted_principal_cap_atoms,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::direct_market::{DirectDeploymentWidthsV1, DirectMarketCompilerOwnedV1};

    #[test]
    fn relayed_manifest_uses_the_authenticated_resolution_release() {
        let registry = Pubkey::new_from_array([0x41; 32]);
        let direct = DirectMarketCompilerOwnedV1::for_test(
            registry,
            DirectDeploymentWidthsV1::new(1_141_117, 971_053, 934_037)
                .expect("test deployment widths"),
        );
        let compiler = direct.compiler();
        let facts = relayed_market_input(
            registry,
            [0x42; 32],
            &WindowChoiceV1 {
                start_unix_seconds: 1_800_000_000,
                end_unix_seconds: 1_800_003_600,
                max_age_seconds: 900,
            },
            RelayedObservableV1::DbcMigrationProgressV1,
            &RelayedVenueFactsV1 {
                program: [0x51; 32],
                programdata: [0x52; 32],
                pool: [0x53; 32],
                elf_digest: [0x54; 32],
                deployment_slot: 99,
                upgrade_authority: [0x55; 32],
            },
            compiler,
        )
        .expect("relayed market input");
        assert!(facts.input.pyth_sponsored_push_release_hex.is_empty());
        assert!(
            serde_json::to_value(&facts.input)
                .expect("relayed JSON")
                .get("pyth_sponsored_push_release_hex")
                .is_none(),
            "the optional sponsored field must not change legacy relayed inputs"
        );
        let manifest = CapabilityManifestV1::decode(&facts.manifest_bytes)
            .expect("relayed capability manifest");
        assert_eq!(manifest.entry_count(), 3);
        for entry_index in 0..manifest.entry_count() {
            assert_eq!(
                manifest
                    .entry(entry_index)
                    .expect("controller entry")
                    .release_id()
                    .to_bytes(),
                compiler.resolution_release,
            );
            assert_ne!(
                manifest
                    .entry(entry_index)
                    .expect("controller entry")
                    .release_id()
                    .to_bytes(),
                dclutch_resolution_codec::RESOLUTION_CONTROLLER_RELEASE_ID_V5,
            );
        }
    }
}

#[cfg(test)]
mod the_founding_path {
    use super::*;
    use crate::direct_market::{DirectDeploymentWidthsV1, DirectMarketCompilerOwnedV1};

    fn row_input(observable: RelayedObservableV1) -> (Pubkey, MarketRunInput) {
        let registry = Pubkey::new_from_array([0x41; 32]);
        let direct = DirectMarketCompilerOwnedV1::for_test(
            registry,
            DirectDeploymentWidthsV1::new(1_141_117, 971_053, 934_037).expect("widths"),
        );
        let facts = relayed_market_input(
            registry,
            [0x42; 32],
            &WindowChoiceV1 {
                start_unix_seconds: 1_800_000_000,
                end_unix_seconds: 1_800_003_600,
                max_age_seconds: 900,
            },
            observable,
            &RelayedVenueFactsV1 {
                program: [0x51; 32],
                programdata: [0x52; 32],
                pool: [0x53; 32],
                elf_digest: [0x54; 32],
                deployment_slot: 99,
                upgrade_authority: [0x55; 32],
            },
            direct.compiler(),
        )
        .expect("relayed market input");
        (registry, facts.input)
    }

    fn graduation_input() -> (Pubkey, MarketRunInput) {
        row_input(RelayedObservableV1::DbcMigrationProgressV1)
    }

    /// THE CONTROL THIS UNIT WAS BUILT AROUND. `compile_market_bodies` is the
    /// first act of `publish_market_records`, before any RPC, so this reaches
    /// the exact site the founding campaign reaches. Before the belief became
    /// a family this refused with "founding_band is required to compile this
    /// market's partition ... There is no default".
    #[test]
    fn the_graduation_market_compiles_through_the_gated_entrance() {
        let (registry, input) = graduation_input();
        let declared = input
            .founding_band
            .as_ref()
            .expect("the graduation market states a belief");
        assert_eq!(
            declared.cell_probability_bps.as_deref(),
            Some(
                [
                    RelayedRowFactsV1::for_observable(RelayedObservableV1::DbcMigrationProgressV1,)
                        .prior_bps
                ]
                .as_slice()
            ),
            "and the belief it states is propositional"
        );
        crate::market::native_composition_bodies_for_test(registry, &input)
            .expect("the gated founding path must compile the one non-price market");
    }

    /// The gate was not weakened to let this market through: an absent belief
    /// still refuses by name, and so does a foregone one.
    #[test]
    fn an_absent_or_foregone_belief_still_refuses_on_the_same_path() {
        let (registry, base) = graduation_input();

        let mut absent = base.clone();
        absent.founding_band = None;
        let refusal = crate::market::native_composition_bodies_for_test(registry, &absent)
            .expect_err("an absent belief must still refuse");
        assert!(
            format!("{refusal}").contains("founding_band is required"),
            "the refusal must still name the field: {refusal}"
        );

        for foregone in [9_500_u32, 400] {
            let mut certain = base.clone();
            certain.founding_band = Some(crate::model::FoundingBandInputV1::proposition(
                vec![foregone],
                RELAYED_MAX_OUTCOME_SHARE_BPS_V1,
            ));
            let refusal = crate::market::native_composition_bodies_for_test(registry, &certain)
                .expect_err("a foregone proposition must refuse");
            assert!(
                format!("{refusal}").contains("DegenerateOutcomePartition"),
                "a {foregone} bp prior must refuse as degenerate: {refusal}"
            );
        }

        // A belief of the WRONG KIND for this market: a spot band over a
        // four-state discriminant. Its one ordinary cell takes the whole
        // plausible band under every possible band, which is the arithmetic
        // certainty that bricked this path in the first place.
        let mut spot = base.clone();
        spot.founding_band = Some(crate::model::FoundingBandInputV1::spot_band(
            3, 200, 10_000, 3, 9_000,
        ));
        let refusal = crate::market::native_composition_bodies_for_test(registry, &spot)
            .expect_err("a zero-cut market under a spot band is still degenerate");
        assert!(
            format!("{refusal}").contains("DegenerateOutcomePartition"),
            "a spot band over a proposition must refuse as degenerate: {refusal}"
        );
    }

    /// OBSERVABLE #2, THROUGH THE GATED ENTRANCE. The renunciation market is a
    /// second zero-cut proposition, founded with its own declared prior, on
    /// the same call the graduation market takes.
    #[test]
    fn the_mint_renunciation_market_founds_beside_the_graduation_market() {
        let (registry, mint) = row_input(RelayedObservableV1::Token2022MintAuthorityRenouncedV1);
        assert!(mint.cuts.is_empty(), "a proposition places no cuts");
        assert_eq!(mint.coefficients, vec![1, 0]);
        let declared = mint
            .founding_band
            .as_ref()
            .expect("row 1 states a belief too");
        assert_eq!(
            declared.cell_probability_bps.as_deref(),
            Some(
                [RelayedRowFactsV1::for_observable(
                    RelayedObservableV1::Token2022MintAuthorityRenouncedV1,
                )
                .prior_bps]
                .as_slice()
            )
        );
        crate::market::native_composition_bodies_for_test(registry, &mint)
            .expect("observable #2 founds through the gated entrance");

        // And the two rows are DIFFERENT markets: distinct Product semantic
        // identity, distinct coordinate domain, distinct result unit. A
        // Product carving one observable cannot be resolved by the other.
        let (_, graduation) = graduation_input();
        assert_ne!(mint.product_id, graduation.product_id);
        assert_ne!(mint.coordinate_domain_id, graduation.coordinate_domain_id);
        assert_ne!(mint.result_unit_id, graduation.result_unit_id);
        // The pinned account set differs too, because the state position's
        // inline width comes off the row: 82 against 424.
        assert_ne!(mint.source_spec_hex, graduation.source_spec_hex);
    }

    /// A partial or double declaration refuses naming the field AND the kind --
    /// strictly more than serde's requiredness said before.
    #[test]
    fn a_partial_or_double_declaration_refuses_by_field_and_kind() {
        let (registry, base) = graduation_input();
        let cases = [
            (
                crate::model::FoundingBandInputV1 {
                    anchor: Some(15_000),
                    volatility_bps: Some(200),
                    window_slots: None,
                    plausible_half_widths: Some(3),
                    cell_probability_bps: None,
                    max_cell_share_bps: 9_000,
                },
                "founding_band/window_slots",
            ),
            (
                crate::model::FoundingBandInputV1 {
                    anchor: Some(15_000),
                    volatility_bps: Some(200),
                    window_slots: Some(10_000),
                    plausible_half_widths: Some(3),
                    cell_probability_bps: Some(vec![3_500]),
                    max_cell_share_bps: 9_000,
                },
                "states BOTH",
            ),
        ];
        for (band, expected) in cases {
            let mut input = base.clone();
            input.founding_band = Some(band);
            let refusal = crate::market::native_composition_bodies_for_test(registry, &input)
                .expect_err("a malformed declaration must refuse");
            assert!(
                format!("{refusal}").contains(expected),
                "expected {expected} in: {refusal}"
            );
        }
    }
}
