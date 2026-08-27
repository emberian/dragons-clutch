//! Release-bound preflight for the direct Market creation critical path.
//!
//! This module compiles canonical contract values supplied by a UI or other
//! host client. It does not claim those values are on chain. The executable
//! Found builder still requires one finalized [`super::FoundMarketState`].

use dclutch_capability_contract::{
    CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, CapabilityFundingDerivationV1, CapabilityManifestV1,
    ContentId as CapabilityContentId, RequiredFoundingEntryV1,
};
use dclutch_core_contract::{MarketIdentity, MarketRoot};
use dclutch_product_contract::{
    ContentId as ProductContentId,
    capacity::{CAPACITY_PROFILE_SCHEMA_RELEASE_ID_V1, CapacityProfileId, CapacityProfileV1},
    claim::{CATEGORICAL_CLAIM_SCHEMA_RELEASE_ID_V1, CategoricalUnitV1, CategoricalUnitV1Input},
    product::{InstanceV1, InstanceV1Input, PRODUCT_INSTANCE_SCHEMA_RELEASE_ID_V1},
    result_domain::{FINITE_RESULT_DOMAIN_CONTENT_DOMAIN_V1, FiniteResultDomainV1},
};
use dclutch_pyth_contract::funding::{FUNDING_BYTES, construct_required_resolution_funding};
use dclutch_realm_contract::{REALM_SCHEMA_RELEASE_ID_V1, RealmV1};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_rent_contract::RENT_CREDIT_PDA_DOMAIN_V1;
use dclutch_source_contract::{
    ContentId as SourceContentId, ProviderReleaseV1, PythAdapterConfigV1, ResolutionPolicyV1,
    RoundingBoundary, SOURCE_MATERIAL_BYTES, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1,
    SourceAccessProfile, SourceCapacityProfileV1, SourceMaterialInputV1, SourceMaterialViewV1,
    SourceSpecV1, StatisticKind, StatisticSpecV1, WindowKind, WindowSpecV1,
    encode_source_material_into_v1,
};
use solana_program::{
    hash::{hash, hashv},
    pubkey::Pubkey,
    rent::Rent,
};

use super::{
    FOUNDATION_GENERATION, FoundationDebitReport, FoundationError, MARKET_SEED,
    resolution_native_funding, validate_market_space,
};

/// One canonical immutable record needed by direct Found admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreationRecordKindV1 {
    /// Immutable Realm selecting collateral.
    Realm,
    /// Occurrence-specific canonical Product Instance.
    ProductInstance,
    /// Exhaustive categorical claim basis.
    ClaimBasis,
    /// Product capacity profile.
    ProductCapacityProfile,
    /// Provider-neutral Product-bound SourceMaterial authority.
    SourceMaterial,
    /// Capability and exact founding-funding manifest.
    CapabilityManifest,
}

/// Exact content-addressed publication obligation for one immutable record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreationRecordObligationV1 {
    /// Semantic role of the canonical bytes.
    pub kind: CreationRecordKindV1,
    /// Exact SBF-admitted schema release identity.
    pub schema_release_id: [u8; 32],
    /// SHA-256 identity of the canonical content bytes.
    pub content_id: [u8; 32],
    /// Exact canonical semantic bytes to publish.
    pub content: Vec<u8>,
    /// Content-addressed permanent raw-record PDA.
    pub raw_record: Pubkey,
    /// Temporary staging PDA which must be vacant after finalization.
    pub staging_cursor: Pubkey,
}

/// A known missing exact operator builder on the creation route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreationBuilderGapV1 {
    /// Begin/append/finalize record instructions are executable on chain but
    /// have no exact chain-derived operator builder in this crate yet.
    ImmutableRecordPublication,
    /// The SBF CreateSeries action exists, but this operator has no exact
    /// finalized-state CreateSeries transaction builder yet.
    SeriesCreate,
    /// The SBF atomic ConsumeTicket+Found action exists, but this operator has
    /// no exact transaction builder for its composed frame yet.
    SeriesConsumeAndFound,
}

/// Honest availability of one creation workflow stage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreationStageStatusV1 {
    /// Canonical bytes or arithmetic have been completed locally.
    Complete,
    /// An exact builder exists but requires a later finalized chain snapshot.
    FinalizedObservationRequired,
    /// No safe exact builder exists at this operator release.
    BuilderUnavailable(CreationBuilderGapV1),
}

/// One ordered stage in the direct creation route or its Series alternative.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreationStageV1 {
    /// Compile and cross-check Product, Source, Realm, and capability content.
    CompileCanonicalArtifacts,
    /// Create the immutable collateral Realm when it does not already exist.
    CreateRealm,
    /// Create the sponsor-bound permanent rent-refund credit.
    CreateRentCredit,
    /// Publish and finalize every content-addressed immutable record.
    PublishImmutableRecords,
    /// Atomically create the Market and its exact prepaid resolution Fund.
    FoundMarketAndFund,
    /// Create the finite prefunded Series root and escrow.
    CreateSeries,
    /// Release one exact scheduled occurrence into a one-use ticket.
    InstantiateSeriesOccurrence,
    /// Consume the ticket and compose its principal with atomic Found.
    ConsumeSeriesTicketAndFound,
    /// Create collateral custody and open the funded Market.
    OpenCollateralVault,
}

/// Availability report for one ordered workflow stage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreationStageReportV1 {
    /// Workflow stage.
    pub stage: CreationStageV1,
    /// Current exact-builder status.
    pub status: CreationStageStatusV1,
}

/// Canonical user-selected material for one direct initial-generation Market.
///
/// Every field is a protocol contract value, not a second operator DTO for
/// its semantics. `rent` and `current_slot` are planning assumptions which
/// must later equal the finalized snapshot consumed by the Found builder.
#[derive(Clone, Debug, PartialEq)]
pub struct ReleaseBoundCreationInputV1 {
    /// SBF program release to derive every destination under.
    pub program_id: Pubkey,
    /// Intended System sponsor and immutable rent-refund beneficiary.
    pub sponsor: Pubkey,
    /// Canonical Realm content.
    pub realm: RealmV1,
    /// Canonical Product capacity profile.
    pub product_capacity_profile: CapacityProfileV1,
    /// Canonical exhaustive categorical claim basis.
    pub claim_basis: CategoricalUnitV1,
    /// Canonical occurrence-specific Product Instance.
    pub product_instance: InstanceV1,
    /// Exact canonical provider-neutral SourceMaterial bytes.
    ///
    /// The borrowed [`SourceMaterialViewV1`] remains the sole decoded runtime
    /// representation; this host input merely owns its canonical preimage.
    pub source_material: Vec<u8>,
    /// Exact canonical capability-manifest bytes.
    pub capability_manifest: Vec<u8>,
    /// Planning Rent schedule; Found must later re-authenticate it on chain.
    pub rent: Rent,
    /// Planning slot used only for the expiring funding state.
    pub current_slot: u64,
}

/// User-selected inputs for the executable terminal-Pyth Source profile.
///
/// The caller supplies real immutable provider deployment, decoding, transport,
/// feed, semantic-domain, and release identities through the typed contract
/// values. No synthetic feed or provider identity is inserted by this builder.
#[derive(Clone, Debug, PartialEq)]
pub struct TerminalPythCreationInputV1 {
    /// SBF program release used for every PDA.
    pub program_id: Pubkey,
    /// Intended System sponsor and permanent rent beneficiary.
    pub sponsor: Pubkey,
    /// Canonical immutable Realm content.
    pub realm: RealmV1,
    /// Product capacity selected for the claim and Instance.
    pub product_capacity_profile: CapacityProfileV1,
    /// Existing reusable Product Terms content identity.
    pub terms_id: ProductContentId,
    /// Existing Product Occurrence content identity.
    pub occurrence_id: ProductContentId,
    /// User-selected exhaustive, disjoint, ordered finite result domain.
    pub result_domain: FiniteResultDomainV1,
    /// Source capacity profile bounding the terminal observation.
    pub source_capacity_profile: SourceCapacityProfileV1,
    /// Real provider adapter/deployment/decoding/transport release tuple.
    pub provider_release: ProviderReleaseV1,
    /// Exact real Pyth feed and integer-normalization configuration.
    pub pyth_adapter_config: PythAdapterConfigV1,
    /// First second of the closed period this market sells an answer about.
    pub window_open_unix_seconds: i64,
    /// Last second of that period; the first admissible observation in
    /// `[open, close]` resolves the market and a later one refuses.
    pub window_close_unix_seconds: i64,
    /// Maximum admitted age of the provider observation.
    pub max_age_seconds: u32,
    /// Maximum admitted future skew of the provider observation.
    pub max_future_skew_seconds: u32,
    /// Immutable terminal schedule release identity.
    pub schedule_id: SourceContentId,
    /// Immutable terminal statistic evaluator release identity.
    pub evaluator_release_id: SourceContentId,
    /// Exact canonical capability manifest funding this SourceMaterial.
    pub capability_manifest: Vec<u8>,
    /// Planning Rent schedule, re-authenticated by finalized Found.
    pub rent: Rent,
    /// Planning slot for the expiring resolution Fund.
    pub current_slot: u64,
}

/// Canonical Product and terminal-Source inputs which do not depend on a
/// realized capability manifest.
///
/// This is the first phase of creation. It breaks the otherwise impossible
/// host ordering in which a manifest entry must select `hash(SourceMaterial)`
/// before the SourceMaterial bytes are available. Every field remains a
/// canonical contract value; this bundle introduces no parallel semantic ID.
#[derive(Clone, Debug, PartialEq)]
pub struct TerminalPythArtifactInputV1 {
    /// Product capacity selected for the claim and Instance.
    pub product_capacity_profile: CapacityProfileV1,
    /// Existing reusable Product Terms content identity.
    pub terms_id: ProductContentId,
    /// Existing Product Occurrence content identity.
    pub occurrence_id: ProductContentId,
    /// User-selected exhaustive, disjoint, ordered finite result domain.
    pub result_domain: FiniteResultDomainV1,
    /// Source capacity profile bounding the terminal observation.
    pub source_capacity_profile: SourceCapacityProfileV1,
    /// Real provider adapter/deployment/decoding/transport release tuple.
    pub provider_release: ProviderReleaseV1,
    /// Exact real Pyth feed and integer-normalization configuration.
    pub pyth_adapter_config: PythAdapterConfigV1,
    /// First second of the closed period this market sells an answer about.
    pub window_open_unix_seconds: i64,
    /// Last second of that period; the first admissible observation in
    /// `[open, close]` resolves the market and a later one refuses.
    pub window_close_unix_seconds: i64,
    /// Maximum admitted age of the provider observation.
    pub max_age_seconds: u32,
    /// Maximum admitted future skew of the provider observation.
    pub max_future_skew_seconds: u32,
    /// Immutable terminal schedule release identity.
    pub schedule_id: SourceContentId,
    /// Immutable terminal statistic evaluator release identity.
    pub evaluator_release_id: SourceContentId,
}

/// Manifest-independent canonical artifacts from terminal-Pyth semantics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalPythArtifactsV1 {
    /// Derived exhaustive categorical claim basis.
    pub claim_basis: CategoricalUnitV1,
    /// Derived occurrence-specific Product Instance.
    pub product_instance: InstanceV1,
    /// Exact canonical provider-neutral SourceMaterial bytes.
    pub source_material: Vec<u8>,
}

/// Canonical artifacts and direct Found admission compiled from terminal-Pyth inputs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalPythCreationPlanV1 {
    /// Derived exhaustive categorical claim basis.
    pub claim_basis: CategoricalUnitV1,
    /// Derived occurrence-specific Product Instance.
    pub product_instance: InstanceV1,
    /// Derived canonical Product-bound SourceMaterial bytes.
    pub source_material: Vec<u8>,
    /// Complete release-bound direct Found preparation.
    pub found: ReleaseBoundCreationPlanV1,
}

/// A complete locally compiled direct Found admission, prior to chain reads.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleaseBoundCreationPlanV1 {
    /// Canonical initial Market identity.
    pub identity: MarketIdentity,
    /// Canonical Market PDA.
    pub market_address: Pubkey,
    /// Permanent sponsor-bound RentCredit required by Found and retirement.
    pub rent_credit_address: Pubkey,
    /// Canonical resolution Fund PDA.
    pub fund_address: Pubkey,
    /// Exhaustive native outcome width, including failure.
    pub outcome_count: u8,
    /// Unique manifest entry authorizing exact founding funding.
    pub resolution_funding: RequiredFoundingEntryV1,
    /// Exact Found-only planning debit, subject to finalized Rent/slot reauthentication.
    ///
    /// Realm, RentCredit, and immutable-record publication capitalization is
    /// intentionally excluded until those stages have exact builders and
    /// finalized rent observations.
    pub debit: FoundationDebitReport,
    /// Exact record publication obligations in Found18 account order.
    pub records: Vec<CreationRecordObligationV1>,
    /// Ordered direct one-Market route.
    pub direct_stages: Vec<CreationStageReportV1>,
    /// Ordered finite-Series alternative, with current gaps explicit.
    pub series_stages: Vec<CreationStageReportV1>,
}

/// Compile user-selected terminal-Pyth semantics into Product and Source authority.
///
/// Outcome count and failure selector come only from `result_domain`; the
/// builder creates no parallel labels or selector ordering. The sole rounding
/// boundary is exact-rational statistic output into that Product-owned map.
pub fn compile_terminal_pyth_creation_v1(
    input: &TerminalPythCreationInputV1,
) -> Result<TerminalPythCreationPlanV1, FoundationError> {
    let artifacts = compile_terminal_pyth_artifacts_v1(&TerminalPythArtifactInputV1 {
        product_capacity_profile: input.product_capacity_profile,
        terms_id: input.terms_id,
        occurrence_id: input.occurrence_id,
        result_domain: input.result_domain,
        source_capacity_profile: input.source_capacity_profile,
        provider_release: input.provider_release,
        pyth_adapter_config: input.pyth_adapter_config,
        window_open_unix_seconds: input.window_open_unix_seconds,
        window_close_unix_seconds: input.window_close_unix_seconds,
        max_age_seconds: input.max_age_seconds,
        max_future_skew_seconds: input.max_future_skew_seconds,
        schedule_id: input.schedule_id,
        evaluator_release_id: input.evaluator_release_id,
    })?;
    let found = compile_release_bound_creation_v1(&ReleaseBoundCreationInputV1 {
        program_id: input.program_id,
        sponsor: input.sponsor,
        realm: input.realm,
        product_capacity_profile: input.product_capacity_profile,
        claim_basis: artifacts.claim_basis,
        product_instance: artifacts.product_instance,
        source_material: artifacts.source_material.clone(),
        capability_manifest: input.capability_manifest.clone(),
        rent: input.rent.clone(),
        current_slot: input.current_slot,
    })?;
    Ok(TerminalPythCreationPlanV1 {
        claim_basis: artifacts.claim_basis,
        product_instance: artifacts.product_instance,
        source_material: artifacts.source_material,
        found,
    })
}

/// Compile Product claim/Instance and exact SourceMaterial before a manifest exists.
///
/// The caller can hash the returned `source_material`, build the canonical
/// manifest whose unique founding entry selects that digest, and only then
/// invoke [`compile_terminal_pyth_creation_v1`] for full release/funding
/// validation. No provisional identity or callback-authored manifest is used.
pub fn compile_terminal_pyth_artifacts_v1(
    input: &TerminalPythArtifactInputV1,
) -> Result<TerminalPythArtifactsV1, FoundationError> {
    input
        .result_domain
        .validate()
        .map_err(|_| FoundationError::InvalidRecord)?;
    let outcome_count = input.result_domain.outcome_count();
    if !(2..=16).contains(&outcome_count) {
        return Err(FoundationError::InvalidOutcomeCount);
    }
    let product_capacity_bytes = input.product_capacity_profile.to_bytes();
    let product_capacity_id = ProductContentId::new(hash(&product_capacity_bytes).to_bytes())
        .map_err(|_| FoundationError::ContentLinkMismatch)?;
    let claim_basis = CategoricalUnitV1::new(
        CategoricalUnitV1Input {
            capacity_profile_id: CapacityProfileId::new(product_capacity_id),
            outcome_count: u32::from(outcome_count),
        },
        input.product_capacity_profile,
    )
    .map_err(|_| FoundationError::InvalidOutcomeCount)?;
    let claim_id = ProductContentId::new(hash(&claim_basis.to_bytes()).to_bytes())
        .map_err(|_| FoundationError::ContentLinkMismatch)?;
    let domain_bytes = input.result_domain.to_bytes();
    let domain_id_bytes = hashv(&[
        FINITE_RESULT_DOMAIN_CONTENT_DOMAIN_V1,
        &[0],
        domain_bytes.as_slice(),
    ])
    .to_bytes();
    let product_instance = InstanceV1::new(InstanceV1Input {
        terms_id: input.terms_id,
        occurrence_id: input.occurrence_id,
        claim_basis_id: claim_id,
        result_domain_id: ProductContentId::new(domain_id_bytes)
            .map_err(|_| FoundationError::ContentLinkMismatch)?,
        capacity_profile_id: CapacityProfileId::new(product_capacity_id),
        partition_cell_count: u32::from(outcome_count),
    })
    .map_err(|_| FoundationError::InvalidRecord)?;
    let product_instance_id = source_id(hash(&product_instance.to_bytes()).to_bytes())?;
    let source_capacity_id = source_id(hash(&input.source_capacity_profile.to_bytes()).to_bytes())?;
    let provider_id = source_id(hash(&input.provider_release.to_bytes()).to_bytes())?;
    let adapter_config_id = source_id(hash(&input.pyth_adapter_config.to_bytes()).to_bytes())?;
    let primary_source = SourceSpecV1::new(
        source_id(input.result_domain.coordinate_domain_id().to_bytes())?,
        source_id(input.result_domain.result_unit_id().to_bytes())?,
        provider_id,
        SourceAccessProfile::PythTerminalOneTransaction,
        adapter_config_id,
        source_capacity_id,
    );
    let primary_source_id = source_id(hash(&primary_source.to_bytes()).to_bytes())?;
    let window = WindowSpecV1::new(
        primary_source_id,
        WindowKind::Terminal,
        input.window_open_unix_seconds,
        input.window_close_unix_seconds,
        input.max_age_seconds,
        input.max_future_skew_seconds,
        input.schedule_id,
    )
    .map_err(|_| FoundationError::InvalidRecord)?;
    let window_id = source_id(hash(&window.to_bytes()).to_bytes())?;
    let result_unit = source_id(input.result_domain.result_unit_id().to_bytes())?;
    let statistic = StatisticSpecV1::new(
        result_unit,
        result_unit,
        StatisticKind::TerminalSample,
        RoundingBoundary::ExactRational,
        1,
        0,
        source_capacity_id,
        input.evaluator_release_id,
        input.source_capacity_profile,
    )
    .map_err(|_| FoundationError::InvalidRecord)?;
    let statistic_id = source_id(hash(&statistic.to_bytes()).to_bytes())?;
    let policy = ResolutionPolicyV1::new(
        source_capacity_id,
        product_instance_id,
        primary_source_id,
        window_id,
        statistic_id,
        source_id(domain_id_bytes)?,
        None,
    );
    let mut source_material = vec![0; SOURCE_MATERIAL_BYTES];
    encode_source_material_into_v1(
        &mut source_material,
        SourceMaterialInputV1 {
            policy: &policy,
            capacity_profile_id: source_capacity_id,
            capacity_profile: &input.source_capacity_profile,
            primary_source_id,
            primary_source: &primary_source,
            primary_provider_release_id: provider_id,
            primary_provider_release: &input.provider_release,
            primary_adapter_config: &input.pyth_adapter_config,
            window_id,
            window: &window,
            statistic_id,
            statistic: &statistic,
            product_instance_id,
            product_instance: &product_instance,
            result_domain: &input.result_domain,
            recovery: None,
        },
    )
    .map_err(|_| FoundationError::ContentLinkMismatch)?;
    Ok(TerminalPythArtifactsV1 {
        claim_basis,
        product_instance,
        source_material,
    })
}

/// Compile canonical user-selected material into a release-bound direct Found plan.
///
/// This proves all Product/Source/manifest links and computes exact addresses
/// and funding. It deliberately returns publication and Series gaps as data.
/// It never emits a Found instruction without finalized records.
pub fn compile_release_bound_creation_v1(
    input: &ReleaseBoundCreationInputV1,
) -> Result<ReleaseBoundCreationPlanV1, FoundationError> {
    let realm_bytes = input.realm.to_bytes();
    let capacity_bytes = input.product_capacity_profile.to_bytes();
    let capacity_id = hash(&capacity_bytes).to_bytes();
    if input
        .claim_basis
        .capacity_profile_id()
        .content_id()
        .to_bytes()
        != capacity_id
    {
        return Err(FoundationError::ContentLinkMismatch);
    }
    input
        .claim_basis
        .validate_capacity(input.product_capacity_profile)
        .map_err(|_| FoundationError::ContentLinkMismatch)?;
    let claim_bytes = input.claim_basis.to_bytes();
    let claim_id = hash(&claim_bytes).to_bytes();
    input
        .product_instance
        .validate_claim_basis(
            dclutch_product_contract::ContentId::new(claim_id)
                .map_err(|_| FoundationError::ContentLinkMismatch)?,
            input.claim_basis,
        )
        .map_err(|_| FoundationError::ContentLinkMismatch)?;

    let outcome_count = u8::try_from(input.claim_basis.outcome_count())
        .map_err(|_| FoundationError::InvalidOutcomeCount)?;
    if !(2..=16).contains(&outcome_count)
        || input.product_instance.partition_cell_count() != u32::from(outcome_count)
    {
        return Err(FoundationError::InvalidOutcomeCount);
    }
    let instance_bytes = input.product_instance.to_bytes();
    let instance_id = hash(&instance_bytes).to_bytes();
    let source_bytes = input.source_material.as_slice();
    let source =
        SourceMaterialViewV1::decode(source_bytes).map_err(|_| FoundationError::InvalidRecord)?;
    let policy = source
        .policy()
        .map_err(|_| FoundationError::InvalidRecord)?;
    let domain = source
        .result_domain()
        .map_err(|_| FoundationError::InvalidRecord)?;
    let domain_bytes = domain.to_bytes();
    let domain_id = hashv(&[
        FINITE_RESULT_DOMAIN_CONTENT_DOMAIN_V1,
        &[0],
        domain_bytes.as_slice(),
    ])
    .to_bytes();
    if source
        .product_instance_id()
        .map_err(|_| FoundationError::InvalidRecord)?
        .to_bytes()
        != instance_id
        || policy.product_instance_id().to_bytes() != instance_id
        || policy.result_domain_id().to_bytes() != domain_id
        || input.product_instance.result_domain_id().to_bytes() != domain_id
        || domain.outcome_count() != outcome_count
    {
        return Err(FoundationError::ContentLinkMismatch);
    }
    let (_, provider) = source
        .primary_provider_release()
        .map_err(|_| FoundationError::InvalidRecord)?;
    let source_id = hash(source_bytes).to_bytes();
    let manifest = CapabilityManifestV1::decode(&input.capability_manifest)
        .map_err(|_| FoundationError::InvalidRecord)?;
    let manifest_id = hash(manifest.as_bytes()).to_bytes();
    let source_capability_id =
        CapabilityContentId::new(source_id).map_err(|_| FoundationError::ContentLinkMismatch)?;
    let resolution_funding = manifest
        .required_founding_entry_for_config(source_capability_id)
        .map_err(|_| FoundationError::InvalidFundingAuthority)?;
    if resolution_funding.entry().release_id().to_bytes()
        != provider.adapter_release_id().to_bytes()
    {
        return Err(FoundationError::ContentLinkMismatch);
    }
    let fund_rent = input.rent.minimum_balance(FUNDING_BYTES);
    let quote = resolution_funding
        .validate_one_shot_resolution_fund_quote(fund_rent)
        .map_err(|_| FoundationError::InvalidFundingAuthority)?;
    let native = resolution_native_funding(quote)?;

    let identity = MarketIdentity::new(
        super::core_id(hash(&realm_bytes).to_bytes())?,
        super::core_id(instance_id)?,
        super::core_id(claim_id)?,
        super::core_id(source_id)?,
        super::core_id(manifest_id)?,
        FOUNDATION_GENERATION,
    );
    let identity_id = hash(&identity.to_bytes()).to_bytes();
    let (market_address, _) =
        Pubkey::find_program_address(&[MARKET_SEED, &identity_id], &input.program_id);
    let (rent_credit_address, _) = Pubkey::find_program_address(
        &[RENT_CREDIT_PDA_DOMAIN_V1, input.sponsor.as_ref()],
        &input.program_id,
    );
    let funding = construct_required_resolution_funding(
        super::core_id(manifest_id)?,
        manifest,
        resolution_funding,
        fund_rent,
        input.current_slot,
    )
    .map_err(|_| FoundationError::InvalidFundingAuthority)?;
    let derivation = CapabilityFundingDerivationV1::new(
        market_address.to_bytes(),
        FOUNDATION_GENERATION,
        super::core_id(manifest_id)?,
        manifest,
        funding,
    )
    .map_err(|_| FoundationError::InvalidFundingAuthority)?;
    let (fund_address, _) =
        Pubkey::find_program_address(&derivation.seed_components(), &input.program_id);
    let mut root = MarketRoot::founding(identity, input.sponsor.to_bytes())
        .map_err(|_| FoundationError::InvalidRecord)?;
    root.register_child(FOUNDATION_GENERATION, 0)
        .map_err(|_| FoundationError::InvalidRecord)?;
    let market_rent = input
        .rent
        .minimum_balance(validate_market_space(outcome_count, root)?);
    let total_sponsor_debit = market_rent
        .checked_add(native.total_lamports)
        .ok_or(FoundationError::ArithmeticOverflow)?;

    let records = vec![
        record(
            input.program_id,
            CreationRecordKindV1::Realm,
            REALM_SCHEMA_RELEASE_ID_V1,
            realm_bytes.to_vec(),
        )?,
        record(
            input.program_id,
            CreationRecordKindV1::ProductInstance,
            PRODUCT_INSTANCE_SCHEMA_RELEASE_ID_V1,
            instance_bytes.to_vec(),
        )?,
        record(
            input.program_id,
            CreationRecordKindV1::ClaimBasis,
            CATEGORICAL_CLAIM_SCHEMA_RELEASE_ID_V1,
            claim_bytes.to_vec(),
        )?,
        record(
            input.program_id,
            CreationRecordKindV1::ProductCapacityProfile,
            CAPACITY_PROFILE_SCHEMA_RELEASE_ID_V1,
            capacity_bytes.to_vec(),
        )?,
        record(
            input.program_id,
            CreationRecordKindV1::SourceMaterial,
            SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1,
            source_bytes.to_vec(),
        )?,
        record(
            input.program_id,
            CreationRecordKindV1::CapabilityManifest,
            CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
            manifest.as_bytes().to_vec(),
        )?,
    ];
    let shared_stages = [
        CreationStageReportV1 {
            stage: CreationStageV1::CompileCanonicalArtifacts,
            status: CreationStageStatusV1::Complete,
        },
        CreationStageReportV1 {
            stage: CreationStageV1::CreateRealm,
            status: CreationStageStatusV1::FinalizedObservationRequired,
        },
        CreationStageReportV1 {
            stage: CreationStageV1::CreateRentCredit,
            status: CreationStageStatusV1::FinalizedObservationRequired,
        },
        CreationStageReportV1 {
            stage: CreationStageV1::PublishImmutableRecords,
            status: CreationStageStatusV1::FinalizedObservationRequired,
        },
    ];
    let direct_stages = vec![
        shared_stages[0],
        shared_stages[1],
        shared_stages[2],
        shared_stages[3],
        CreationStageReportV1 {
            stage: CreationStageV1::FoundMarketAndFund,
            status: CreationStageStatusV1::FinalizedObservationRequired,
        },
        CreationStageReportV1 {
            stage: CreationStageV1::OpenCollateralVault,
            status: CreationStageStatusV1::FinalizedObservationRequired,
        },
    ];
    let series_stages = vec![
        shared_stages[0],
        shared_stages[1],
        shared_stages[2],
        shared_stages[3],
        CreationStageReportV1 {
            stage: CreationStageV1::CreateSeries,
            status: CreationStageStatusV1::FinalizedObservationRequired,
        },
        CreationStageReportV1 {
            stage: CreationStageV1::InstantiateSeriesOccurrence,
            status: CreationStageStatusV1::FinalizedObservationRequired,
        },
        CreationStageReportV1 {
            stage: CreationStageV1::ConsumeSeriesTicketAndFound,
            status: CreationStageStatusV1::FinalizedObservationRequired,
        },
        CreationStageReportV1 {
            stage: CreationStageV1::OpenCollateralVault,
            status: CreationStageStatusV1::FinalizedObservationRequired,
        },
    ];

    Ok(ReleaseBoundCreationPlanV1 {
        identity,
        market_address,
        rent_credit_address,
        fund_address,
        outcome_count,
        resolution_funding,
        debit: FoundationDebitReport {
            sponsor: input.sponsor,
            realm_rent: 0,
            market_rent,
            fund_rent: native.rent_lamports,
            provider_fee_reimbursement: native.provider_lamports,
            resolution_success_bounty: native.bounty_lamports,
            total_sponsor_debit,
        },
        records,
        direct_stages,
        series_stages,
    })
}

fn record(
    program_id: Pubkey,
    kind: CreationRecordKindV1,
    schema_release_id: [u8; 32],
    content: Vec<u8>,
) -> Result<CreationRecordObligationV1, FoundationError> {
    let content_id = hash(&content).to_bytes();
    if content_id.iter().all(|byte| *byte == 0) {
        return Err(FoundationError::ContentLinkMismatch);
    }
    let (raw_record, _) = Pubkey::find_program_address(
        &[
            RAW_RECORD_PDA_SEED_V1,
            schema_release_id.as_slice(),
            content_id.as_slice(),
        ],
        &program_id,
    );
    let (staging_cursor, _) = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            schema_release_id.as_slice(),
            content_id.as_slice(),
        ],
        &program_id,
    );
    Ok(CreationRecordObligationV1 {
        kind,
        schema_release_id,
        content_id,
        content,
        raw_record,
        staging_cursor,
    })
}

fn source_id(bytes: [u8; 32]) -> Result<SourceContentId, FoundationError> {
    SourceContentId::new(bytes).map_err(|_| FoundationError::ContentLinkMismatch)
}
