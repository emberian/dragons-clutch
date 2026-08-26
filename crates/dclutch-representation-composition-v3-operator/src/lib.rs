#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Chain-derived Product-to-representation composition and unsigned workflows.
//!
//! The operator authenticates finalized Registry records from one finalized
//! observation, recomputes every content digest and PDA, and then delegates
//! all semantic parsing to the Product, composition, and Claims contracts.
//! `K` is always the composition/Claims width. `N` is always the independently
//! authenticated Product basis/result width. Neither width is caller authority.
//!
//! This crate performs no RPC, signing, submission, or account mutation.

/// Canonical composition-admitted compact Trading Hot construction.
pub mod hot_v3;

use dclutch_product_payoff_v2_codec::{
    registry_v3::GRADED_BASIS_RECORD_SCHEMA_ID_V3,
    runtime_v3::{ProductBasisV3, SEMANTIC_BASIS_CONTENT_DOMAIN_V3, semantic_basis_preimage_v3},
};
use dclutch_product_runtime_v2::ContentId;
use dclutch_product_runtime_v2_admission::{
    AdmissionProjectionV2, AdmissionReceiptV2, FinalizedRecordCoordinateV2, PORTFOLIO_SCHEMA_ID_V2,
    PRODUCT_RECORD_SCHEMA_ID_V2, RESULT_DOMAIN_SCHEMA_ID_V2, admit_authenticated_records_v2,
};
use dclutch_rational_representation_v2_kernel::{
    DescriptorAdmissionV2, RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2,
    REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3, RepresentationDescriptorV2,
};
use dclutch_rational_representation_v2_lifecycle_contract::{
    LIFECYCLE_COMMON_ACCOUNT_COUNT_V2, LIFECYCLE_COORDINATE_ACCOUNT_COUNT_V2,
    LIFECYCLE_COORDINATE_BYTES_V2, LIFECYCLE_HEADER_BYTES_V2, LIFECYCLE_VACANCY_ACCOUNT_COUNT_V2,
    LifecycleActionV2, LifecycleCoordinateV2, LifecycleHeaderV2, LifecycleRequestV2, prepare,
};
use dclutch_record_contract::{
    APPEND_PAGE_HEADER_BYTES_V1, AppendPageV1, BeginRecordV1,
    CANONICAL_RECORD_DEPLOYMENT_PROFILE_V1, ContentDigest, FinalizeRecordV1, RecordKeyV1,
    SchemaReleaseId,
};
use dclutch_representation_composition_v3_kernel::{
    COMPOSITION_DESCRIPTOR_SCHEMA_ID_V3, COMPOSITION_EXPOSURE_SCHEMA_ID_V3,
    COMPOSITION_GRAPH_SCHEMA_ID_V3, COMPOSITION_TRANSLATION_SCHEMA_ID_V3, CompositionBundleV3,
    CompositionDescriptorV3, CompositionExposureBundleV3, CompositionExposureExpectedV3,
    RecordAdmissionV3, decode_composition_bundle_v3,
};
use dclutch_versioned_message_operator::{
    Finality, Observation, ObservedAccount, VersionedMessagePlanV0,
    compile_v0_message_with_optional_tables,
};
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_hash::Hash;
use solana_program::{
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::{system_program, sysvar};

/// Stable refusal from observation, semantic join, or unsigned planning.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Accounts did not share one nonzero finalized observation.
    Observation,
    /// The selected Registry program was absent or not executable.
    Registry,
    /// A finalized record schema, owner, PDA, rent floor, digest, or cursor refused.
    FinalizedRecord,
    /// Product Runtime records did not form one exact graph.
    Product,
    /// ProductBasisV3 hostile decoding or semantic identity reconstruction refused.
    ProductBasis,
    /// Composition descriptor, DAG, translation, or exposure hostile decoding refused.
    Composition,
    /// Product, DAG, exposure, Market, release, basis, K, or N identities diverged.
    CrossRecord,
    /// Canonical immutable-record publication construction refused.
    Publication,
    /// Canonical Claims lifecycle construction refused.
    ClaimsLifecycle,
    /// Unsigned v0 packet compilation or exact packet limit refused.
    Packet,
    /// Composition admission did not join the selected compact Trading/Claims path.
    HotAdapter,
    /// Checked size, offset, slot, or count arithmetic overflowed.
    Arithmetic,
}

/// Result alias for operator construction.
pub type Result<T> = core::result::Result<T, Error>;

/// One Registry-owned finalized raw record and vacant staging cursor.
#[derive(Clone, Copy, Debug)]
pub struct FinalizedRecordObservationV3<'a> {
    /// Exact schema selected by the semantic owner.
    pub schema_id: [u8; 32],
    /// Registry-owned exact semantic bytes.
    pub raw: &'a ObservedAccount,
    /// System-owned data-empty finalization cursor.
    pub staging: &'a ObservedAccount,
    /// Rent minimum observed for the exact raw byte width at the same slot.
    pub raw_rent_minimum: u64,
}

/// Product Runtime records bound to one ProductBasisV3 record.
#[derive(Clone, Copy, Debug)]
pub struct ProductCompositionObservationV3<'a> {
    /// Runtime Product graph-root record.
    pub product: FinalizedRecordObservationV3<'a>,
    /// Product-selected result-domain record.
    pub result_domain: FinalizedRecordObservationV3<'a>,
    /// Product-selected exact portfolio record.
    pub portfolio: FinalizedRecordObservationV3<'a>,
    /// Product-owned runtime payout basis record.
    pub product_basis: FinalizedRecordObservationV3<'a>,
}

/// Representation DAG, canonical translation, and Product exposure records.
#[derive(Clone, Copy, Debug)]
pub struct RepresentationCompositionObservationV3<'a> {
    /// Rational execution descriptor selecting the finalized exposure record.
    pub execution_descriptor: FinalizedRecordObservationV3<'a>,
    /// Composition descriptor owning Market, release, basis, and `K`.
    pub descriptor: FinalizedRecordObservationV3<'a>,
    /// Exact acyclic representation DAG.
    pub graph: FinalizedRecordObservationV3<'a>,
    /// Byte-identical canonical root translation.
    pub translation: FinalizedRecordObservationV3<'a>,
    /// Exact ordered `K x N` sparse Product exposure.
    pub exposure: FinalizedRecordObservationV3<'a>,
}

/// One finalized chain observation sufficient for complete admission.
#[derive(Clone, Copy, Debug)]
pub struct CompositionChainObservationV3<'a> {
    /// Current executable Registry/record program.
    pub registry_program: &'a ObservedAccount,
    /// Current executable Claims program used only for canonical PDA derivation.
    pub claims_program: &'a ObservedAccount,
    /// Product records and ProductBasisV3.
    pub product: ProductCompositionObservationV3<'a>,
    /// Representation composition and exposure records.
    pub representation: RepresentationCompositionObservationV3<'a>,
}

/// One exact finalized record coordinate projected for operator consumers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FinalizedCoordinateV3 {
    /// Schema identity.
    pub schema_id: [u8; 32],
    /// SHA-256 of the complete exact semantic bytes.
    pub content_digest: [u8; 32],
    /// Canonical Registry-owned raw PDA.
    pub raw_account: Pubkey,
    /// Canonical vacant staging PDA.
    pub staging_account: Pubkey,
}

#[derive(Clone, Copy)]
struct AuthenticatedRecordV3<'a> {
    bytes: &'a [u8],
    coordinate: FinalizedCoordinateV3,
}

/// Fully admitted Product-to-Claims composition from one chain observation.
#[derive(Clone, Copy)]
pub struct AdmittedCompositionV3<'a> {
    observation: Observation,
    product: AdmissionProjectionV2,
    product_basis: ProductBasisV3<'a>,
    composition: CompositionBundleV3<'a>,
    exposure: CompositionExposureBundleV3<'a>,
    execution_descriptor: RepresentationDescriptorV2<'a>,
    execution_descriptor_record: FinalizedCoordinateV3,
    descriptor_record: FinalizedCoordinateV3,
    exposure_record: FinalizedCoordinateV3,
    claims_program: Pubkey,
}

/// Stateless admission plan over exact finalized semantic-owner records.
///
/// There is deliberately no synthetic persisted admission account here. The
/// plan is the checked borrowed composition itself; every physical consumer
/// must reauthenticate the same finalized coordinates at its own boundary.
#[derive(Clone, Copy)]
pub struct CompositionAdmissionPlanV3<'a> {
    admitted: AdmittedCompositionV3<'a>,
}

impl<'a> CompositionAdmissionPlanV3<'a> {
    /// Return the exact checked borrowed composition.
    pub const fn admitted(self) -> AdmittedCompositionV3<'a> {
        self.admitted
    }

    /// Runtime Claims/representation width `K`.
    pub const fn representation_width(self) -> u32 {
        self.admitted.representation_width()
    }

    /// Runtime Product terminal width `N`.
    pub const fn product_width(self) -> u32 {
        self.admitted.product_width()
    }
}

impl<'a> AdmittedCompositionV3<'a> {
    /// One finalized observation shared by every authenticated account.
    pub const fn observation(self) -> Observation {
        self.observation
    }

    /// Independently admitted Product Runtime graph.
    pub const fn product(self) -> AdmissionProjectionV2 {
        self.product
    }

    /// Independently hostile-decoded ProductBasisV3.
    pub const fn product_basis(self) -> ProductBasisV3<'a> {
        self.product_basis
    }

    /// Completely joined descriptor, acyclic graph, and canonical translation.
    pub const fn composition(self) -> CompositionBundleV3<'a> {
        self.composition
    }

    /// Exact admitted sparse Product-to-Claims exposure.
    pub const fn exposure(self) -> CompositionExposureBundleV3<'a> {
        self.exposure
    }

    /// Rational execution descriptor selecting the exact exposure record.
    pub const fn execution_descriptor(self) -> RepresentationDescriptorV2<'a> {
        self.execution_descriptor
    }

    /// Finalized rational execution-descriptor coordinate.
    pub const fn execution_descriptor_record(self) -> FinalizedCoordinateV3 {
        self.execution_descriptor_record
    }

    /// Runtime Claims/representation width `K`.
    pub const fn representation_width(self) -> u32 {
        self.composition.descriptor().outcome_count()
    }

    /// Runtime Product terminal width `N`.
    pub const fn product_width(self) -> u32 {
        self.product_basis.basis_width()
    }

    /// Finalized composition descriptor coordinate.
    pub const fn descriptor_record(self) -> FinalizedCoordinateV3 {
        self.descriptor_record
    }

    /// Finalized exposure coordinate.
    pub const fn exposure_record(self) -> FinalizedCoordinateV3 {
        self.exposure_record
    }

    /// Same-finalized executable Claims program used for PDA derivation.
    pub const fn claims_program(self) -> Pubkey {
        self.claims_program
    }
}

/// Authenticate one full production chain observation.
///
/// RuntimeV2 currently has an explicit failure outcome, so the full Product
/// graph admits `N >= 2`. The exposure-only candidate validator remains able
/// to exercise the ProductBasisV3 capacity witness `N = 1` without pretending
/// that a RuntimeV2 ResultDomain with that width exists.
pub fn authenticate_composition_v3(
    observed: CompositionChainObservationV3<'_>,
) -> Result<AdmittedCompositionV3<'_>> {
    let observation = common_observation(observed)?;
    let registry = observed.registry_program.key;
    let product_record = authenticate_record(
        registry,
        observed.product.product,
        PRODUCT_RECORD_SCHEMA_ID_V2,
    )?;
    let result_domain_record = authenticate_record(
        registry,
        observed.product.result_domain,
        RESULT_DOMAIN_SCHEMA_ID_V2,
    )?;
    let portfolio_record =
        authenticate_record(registry, observed.product.portfolio, PORTFOLIO_SCHEMA_ID_V2)?;
    let product_basis_record = authenticate_record(
        registry,
        observed.product.product_basis,
        GRADED_BASIS_RECORD_SCHEMA_ID_V3,
    )?;
    let product = admit_authenticated_records_v2(
        AdmissionReceiptV2 {
            product: product_coordinate(product_record.coordinate)?,
            result_domain: product_coordinate(result_domain_record.coordinate)?,
            portfolio: product_coordinate(portfolio_record.coordinate)?,
        },
        product_record.bytes,
        result_domain_record.bytes,
        portfolio_record.bytes,
    )
    .map_err(|_| Error::Product)?;
    let product_basis =
        ProductBasisV3::decode(product_basis_record.bytes).map_err(|_| Error::ProductBasis)?;
    if product_basis.product_id() != product.join.product_id.to_bytes()
        || product_basis.result_domain_id() != result_domain_record.coordinate.content_digest
        || product_basis.basis_width() != product.join.outcome_count
    {
        return Err(Error::CrossRecord);
    }

    let descriptor_record = authenticate_record(
        registry,
        observed.representation.descriptor,
        COMPOSITION_DESCRIPTOR_SCHEMA_ID_V3,
    )?;
    let descriptor_admission = admission(
        descriptor_record.coordinate.content_digest,
        descriptor_record.coordinate.content_digest,
    );
    let descriptor = CompositionDescriptorV3::decode(descriptor_record.bytes, descriptor_admission)
        .map_err(|_| Error::Composition)?;
    let graph_record = authenticate_record(
        registry,
        observed.representation.graph,
        COMPOSITION_GRAPH_SCHEMA_ID_V3,
    )?;
    let translation_record = authenticate_record(
        registry,
        observed.representation.translation,
        COMPOSITION_TRANSLATION_SCHEMA_ID_V3,
    )?;
    let composition = decode_composition_bundle_v3(
        descriptor_record.bytes,
        descriptor_admission,
        graph_record.bytes,
        admission(
            descriptor.graph_id(),
            graph_record.coordinate.content_digest,
        ),
        translation_record.bytes,
        admission(
            descriptor.translation_id(),
            translation_record.coordinate.content_digest,
        ),
    )
    .map_err(|_| Error::Composition)?;
    let exposure_record = authenticate_record(
        registry,
        observed.representation.exposure,
        COMPOSITION_EXPOSURE_SCHEMA_ID_V3,
    )?;
    let semantic_basis = semantic_basis_id(product_basis_record.bytes)?;
    if semantic_basis != product.join.liability_basis_id.to_bytes()
        || descriptor.result_domain() != result_domain_record.coordinate.content_digest
        || descriptor.native_basis() != semantic_basis
    {
        return Err(Error::CrossRecord);
    }
    let exposure = decode_and_join_exposure(
        exposure_record,
        product_basis_record.coordinate.content_digest,
        product_basis,
        composition,
        semantic_basis,
    )?;
    let execution_descriptor_record = authenticate_record(
        registry,
        observed.representation.execution_descriptor,
        REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3,
    )?;
    let representation_authority = Pubkey::find_program_address(
        &[
            RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2,
            &execution_descriptor_record.coordinate.content_digest,
        ],
        &observed.claims_program.key,
    )
    .0;
    let execution_descriptor = RepresentationDescriptorV2::decode(
        execution_descriptor_record.bytes,
        DescriptorAdmissionV2 {
            selected_descriptor_id: execution_descriptor_record.coordinate.content_digest,
            finalized_descriptor_id: execution_descriptor_record.coordinate.content_digest,
            recomputed_descriptor_digest: execution_descriptor_record.coordinate.content_digest,
            finalized_descriptor_digest: execution_descriptor_record.coordinate.content_digest,
            record_authenticated: true,
            derived_representation_authority: representation_authority.to_bytes(),
            authority_derivation_authenticated: true,
        },
    )
    .map_err(|_| Error::Composition)?;
    execution_descriptor
        .authenticate_exposure(exposure)
        .map_err(|_| Error::CrossRecord)?;
    if execution_descriptor.market_id() != descriptor.market()
        || execution_descriptor.release_set_id() != descriptor.release_set()
        || execution_descriptor.outcome_count() != descriptor.outcome_count()
    {
        return Err(Error::CrossRecord);
    }
    Ok(AdmittedCompositionV3 {
        observation,
        product,
        product_basis,
        composition,
        exposure,
        execution_descriptor,
        execution_descriptor_record: execution_descriptor_record.coordinate,
        descriptor_record: descriptor_record.coordinate,
        exposure_record: exposure_record.coordinate,
        claims_program: observed.claims_program.key,
    })
}

/// Build the stateless composition admission plan from one chain observation.
pub fn build_composition_admission_plan_v3(
    observed: CompositionChainObservationV3<'_>,
) -> Result<CompositionAdmissionPlanV3<'_>> {
    authenticate_composition_v3(observed).map(|admitted| CompositionAdmissionPlanV3 { admitted })
}

/// Validate canonical publication candidates without claiming finalized-chain admission.
///
/// This pure pre-publication path is where the `K3/N1` capacity witness lives.
/// Full production admission still calls [`authenticate_composition_v3`] and
/// independently checks the Product Runtime ResultDomain width.
pub fn validate_publication_candidates_v3<'a>(
    product_basis_bytes: &'a [u8],
    descriptor_bytes: &'a [u8],
    graph_bytes: &'a [u8],
    translation_bytes: &'a [u8],
    exposure_bytes: &'a [u8],
) -> Result<(CompositionBundleV3<'a>, CompositionExposureBundleV3<'a>)> {
    let basis_digest = hash(product_basis_bytes).to_bytes();
    let descriptor_digest = hash(descriptor_bytes).to_bytes();
    let graph_digest = hash(graph_bytes).to_bytes();
    let translation_digest = hash(translation_bytes).to_bytes();
    let exposure_digest = hash(exposure_bytes).to_bytes();
    let product_basis =
        ProductBasisV3::decode(product_basis_bytes).map_err(|_| Error::ProductBasis)?;
    let descriptor_admission = admission(descriptor_digest, descriptor_digest);
    let descriptor = CompositionDescriptorV3::decode(descriptor_bytes, descriptor_admission)
        .map_err(|_| Error::Composition)?;
    let composition = decode_composition_bundle_v3(
        descriptor_bytes,
        descriptor_admission,
        graph_bytes,
        admission(descriptor.graph_id(), graph_digest),
        translation_bytes,
        admission(descriptor.translation_id(), translation_digest),
    )
    .map_err(|_| Error::Composition)?;
    let semantic_basis = semantic_basis_id(product_basis_bytes)?;
    if descriptor.result_domain() != product_basis.result_domain_id()
        || descriptor.native_basis() != semantic_basis
    {
        return Err(Error::CrossRecord);
    }
    let exposure_record = AuthenticatedRecordV3 {
        bytes: exposure_bytes,
        coordinate: FinalizedCoordinateV3 {
            schema_id: COMPOSITION_EXPOSURE_SCHEMA_ID_V3,
            content_digest: exposure_digest,
            raw_account: Pubkey::default(),
            staging_account: Pubkey::default(),
        },
    };
    let exposure = decode_and_join_exposure(
        exposure_record,
        basis_digest,
        product_basis,
        composition,
        semantic_basis,
    )?;
    Ok((composition, exposure))
}

fn decode_and_join_exposure<'a>(
    exposure_record: AuthenticatedRecordV3<'a>,
    product_basis_digest: [u8; 32],
    product_basis: ProductBasisV3<'_>,
    composition: CompositionBundleV3<'_>,
    semantic_basis: [u8; 32],
) -> Result<CompositionExposureBundleV3<'a>> {
    let descriptor = composition.descriptor();
    CompositionExposureBundleV3::decode(
        exposure_record.bytes,
        admission(
            exposure_record.coordinate.content_digest,
            exposure_record.coordinate.content_digest,
        ),
    )
    .and_then(|exposure| {
        exposure.verify_for(CompositionExposureExpectedV3 {
            market: descriptor.market(),
            result_domain: product_basis.result_domain_id(),
            release_set: descriptor.release_set(),
            product_basis: product_basis_digest,
            representation_basis: semantic_basis,
            graph_id: descriptor.graph_id(),
            product_width: product_basis.basis_width(),
            representation_width: descriptor.outcome_count(),
        })
    })
    .and_then(|exposure| exposure.verify_composition_graph(composition.graph()))
    .map_err(|_| Error::Composition)
}

/// Immutable bytes selected for canonical Record publication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PublicationTargetV3<'a> {
    /// Exact semantic schema identity.
    pub schema_id: [u8; 32],
    /// Exact canonical semantic bytes.
    pub bytes: &'a [u8],
}

/// Chain accounts and liveness inputs for a publication sequence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PublicationContextV3 {
    /// Current Registry/record program.
    pub record_program: Pubkey,
    /// Signing publication sponsor.
    pub sponsor: Pubkey,
    /// Permanent RentCredit receiving the finalized cursor balance.
    pub rent_credit: Pubkey,
    /// Current finalized slot used only to derive exact expiry.
    pub current_slot: u64,
    /// Trusted current staging-cursor Rent floor and cleanup bounty.
    pub cursor_rent_principal: u64,
}

/// Complete unsigned canonical publication sequence for one immutable record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicationPlanV3 {
    /// Exact content digest.
    pub content_digest: [u8; 32],
    /// Canonical raw PDA.
    pub raw_record: Pubkey,
    /// Canonical staging PDA.
    pub staging_cursor: Pubkey,
    /// Begin, ordered Append pages, then Finalize.
    pub instructions: Vec<Instruction>,
}

/// Build an unsigned canonical Begin/Append/Finalize sequence.
pub fn build_publication_plan_v3(
    context: PublicationContextV3,
    target: PublicationTargetV3<'_>,
) -> Result<PublicationPlanV3> {
    if context.record_program == Pubkey::default()
        || context.sponsor == Pubkey::default()
        || context.rent_credit == Pubkey::default()
        || context.current_slot == 0
        || context.cursor_rent_principal == 0
        || target.bytes.is_empty()
    {
        return Err(Error::Publication);
    }
    let schema = SchemaReleaseId::new(target.schema_id).map_err(|_| Error::Publication)?;
    let digest = hash(target.bytes).to_bytes();
    let content = ContentDigest::new(digest).map_err(|_| Error::Publication)?;
    let key = RecordKeyV1::new(schema, content);
    let raw_seeds = key.raw_record_pda_seeds();
    let raw_schema = raw_seeds.schema_release_id();
    let raw_digest = raw_seeds.expected_digest();
    let raw_record = Pubkey::find_program_address(
        &[
            raw_seeds.domain(),
            raw_schema.as_bytes(),
            raw_digest.as_bytes(),
        ],
        &context.record_program,
    )
    .0;
    let staging_seeds = key.staging_cursor_pda_seeds();
    let staging_schema = staging_seeds.schema_release_id();
    let staging_digest = staging_seeds.expected_digest();
    let staging_cursor = Pubkey::find_program_address(
        &[
            staging_seeds.domain(),
            staging_schema.as_bytes(),
            staging_digest.as_bytes(),
        ],
        &context.record_program,
    )
    .0;
    let profile = CANONICAL_RECORD_DEPLOYMENT_PROFILE_V1;
    let expiry = context
        .current_slot
        .checked_add(profile.maximum_staging_lifetime_slots())
        .ok_or(Error::Arithmetic)?;
    let policy = profile
        .staging_liveness_policy(context.cursor_rent_principal)
        .map_err(|_| Error::Publication)?;
    let begin = BeginRecordV1::new(
        key,
        u64::try_from(target.bytes.len()).map_err(|_| Error::Arithmetic)?,
        profile.page_envelope().map_err(|_| Error::Publication)?,
        policy.policy_id(),
        expiry,
        context.cursor_rent_principal,
    )
    .map_err(|_| Error::Publication)?;
    let page_bytes = usize::try_from(profile.page_bytes()).map_err(|_| Error::Arithmetic)?;
    let page_count = target
        .bytes
        .len()
        .checked_add(page_bytes.checked_sub(1).ok_or(Error::Arithmetic)?)
        .ok_or(Error::Arithmetic)?
        / page_bytes;
    let mut instructions = Vec::with_capacity(page_count.checked_add(2).ok_or(Error::Arithmetic)?);
    instructions.push(Instruction {
        program_id: context.record_program,
        accounts: vec![
            AccountMeta::new(context.sponsor, true),
            AccountMeta::new(raw_record, false),
            AccountMeta::new(staging_cursor, false),
            AccountMeta::new_readonly(context.rent_credit, false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
            AccountMeta::new_readonly(sysvar::clock::ID, false),
        ],
        data: begin.to_bytes().to_vec(),
    });
    for (page_index, page) in target.bytes.chunks(page_bytes).enumerate() {
        let page_index = u64::try_from(page_index).map_err(|_| Error::Arithmetic)?;
        let offset = page_index
            .checked_mul(u64::try_from(page_bytes).map_err(|_| Error::Arithmetic)?)
            .ok_or(Error::Arithmetic)?;
        let append = AppendPageV1::new(page_index, offset, page).map_err(|_| Error::Publication)?;
        let mut data = vec![0; APPEND_PAGE_HEADER_BYTES_V1 + page.len()];
        append.encode(&mut data).map_err(|_| Error::Publication)?;
        instructions.push(Instruction {
            program_id: context.record_program,
            accounts: vec![
                AccountMeta::new_readonly(context.sponsor, true),
                AccountMeta::new(raw_record, false),
                AccountMeta::new(staging_cursor, false),
            ],
            data,
        });
    }
    instructions.push(Instruction {
        program_id: context.record_program,
        accounts: vec![
            AccountMeta::new_readonly(raw_record, false),
            AccountMeta::new(staging_cursor, false),
            AccountMeta::new(context.rent_credit, false),
        ],
        data: FinalizeRecordV1.to_bytes().to_vec(),
    });
    Ok(PublicationPlanV3 {
        content_digest: digest,
        raw_record,
        staging_cursor,
        instructions,
    })
}

/// Exact canonical Claims lifecycle request and frame geometry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaimsLifecyclePlanV3 {
    /// Canonical child request bytes.
    pub request: Vec<u8>,
    /// Exact child account count selected by the semantic-owner contract.
    pub account_count: usize,
    /// Product/Claims width `K` committed by the request.
    pub representation_width: u32,
}

/// Encode one Claims lifecycle request after joining it to admitted composition facts.
pub fn build_claims_lifecycle_plan_v3(
    admitted: AdmittedCompositionV3<'_>,
    header: LifecycleHeaderV2,
    coordinates: &[LifecycleCoordinateV2],
) -> Result<ClaimsLifecyclePlanV3> {
    let descriptor = admitted.execution_descriptor;
    if header.release_set != descriptor.release_set_id()
        || header.market != descriptor.market_id()
        || header.graph_id != descriptor.graph_id()
        || header.descriptor_id != descriptor.descriptor_id()
        || header.representation_authority != descriptor.representation_authority()
        || header.receipt_mint != descriptor.receipt_mint()
        || header.token_program != descriptor.token_program()
        || header.outcome_count != descriptor.outcome_count()
        || usize::try_from(header.coordinate_count).map_err(|_| Error::Arithmetic)?
            != coordinates.len()
    {
        return Err(Error::ClaimsLifecycle);
    }
    let coordinate_bytes = coordinates
        .len()
        .checked_mul(LIFECYCLE_COORDINATE_BYTES_V2)
        .ok_or(Error::Arithmetic)?;
    let mut rows = vec![0_u8; coordinate_bytes];
    for (index, coordinate) in coordinates.iter().copied().enumerate() {
        let start = index
            .checked_mul(LIFECYCLE_COORDINATE_BYTES_V2)
            .ok_or(Error::Arithmetic)?;
        let end = start
            .checked_add(LIFECYCLE_COORDINATE_BYTES_V2)
            .ok_or(Error::Arithmetic)?;
        coordinate
            .encode_into(rows.get_mut(start..end).ok_or(Error::Arithmetic)?)
            .map_err(|_| Error::ClaimsLifecycle)?;
    }
    let request = LifecycleRequestV2::new(header, &rows).map_err(|_| Error::ClaimsLifecycle)?;
    let mut wire = vec![0_u8; LIFECYCLE_HEADER_BYTES_V2 + rows.len()];
    request
        .encode_into(&mut wire)
        .map_err(|_| Error::ClaimsLifecycle)?;
    let decoded = LifecycleRequestV2::decode(&wire).map_err(|_| Error::ClaimsLifecycle)?;
    prepare(decoded, descriptor).map_err(|_| Error::ClaimsLifecycle)?;
    let account_count = match header.action {
        LifecycleActionV2::ActivateReceipt => LIFECYCLE_COMMON_ACCOUNT_COUNT_V2,
        LifecycleActionV2::ActivateCoordinate | LifecycleActionV2::RetireCoordinate => {
            LIFECYCLE_COORDINATE_ACCOUNT_COUNT_V2
        }
        LifecycleActionV2::RetireReceipt => LIFECYCLE_COMMON_ACCOUNT_COUNT_V2
            .checked_add(
                coordinates
                    .len()
                    .checked_mul(LIFECYCLE_VACANCY_ACCOUNT_COUNT_V2)
                    .ok_or(Error::Arithmetic)?,
            )
            .ok_or(Error::Arithmetic)?,
    };
    Ok(ClaimsLifecyclePlanV3 {
        request: wire,
        account_count,
        representation_width: admitted.representation_width(),
    })
}

/// Compile exact instructions into an unsigned packet-safe v0 message.
///
/// Both compute-budget instructions are included in packet accounting. Lookup
/// tables are finalized observations used only for routing compression.
pub fn compile_unsigned_packet_v0(
    payer: Pubkey,
    instructions: &[Instruction],
    recent_blockhash: Hash,
    observation: Observation,
    lookup_tables: &[ObservedAccount],
    compute_unit_limit: u32,
    compute_unit_price_micro_lamports: u64,
) -> Result<VersionedMessagePlanV0> {
    if payer == Pubkey::default() || compute_unit_limit == 0 {
        return Err(Error::Packet);
    }
    let mut complete =
        Vec::with_capacity(instructions.len().checked_add(2).ok_or(Error::Arithmetic)?);
    complete.push(ComputeBudgetInstruction::set_compute_unit_limit(
        compute_unit_limit,
    ));
    complete.push(ComputeBudgetInstruction::set_compute_unit_price(
        compute_unit_price_micro_lamports,
    ));
    complete.extend_from_slice(instructions);
    compile_v0_message_with_optional_tables(
        payer,
        &complete,
        recent_blockhash,
        observation,
        lookup_tables,
    )
    .map_err(|_| Error::Packet)
}

fn common_observation(observed: CompositionChainObservationV3<'_>) -> Result<Observation> {
    let expected = observed.registry_program.observation;
    if expected.slot == 0
        || expected.finality != Finality::Finalized
        || !observed.registry_program.executable
        || observed.registry_program.key == Pubkey::default()
        || observed.claims_program.observation != expected
        || !observed.claims_program.executable
        || observed.claims_program.key == Pubkey::default()
    {
        return Err(Error::Registry);
    }
    for record in all_records(observed) {
        if record.raw.observation != expected || record.staging.observation != expected {
            return Err(Error::Observation);
        }
    }
    Ok(expected)
}

fn all_records(
    observed: CompositionChainObservationV3<'_>,
) -> [FinalizedRecordObservationV3<'_>; 9] {
    [
        observed.product.product,
        observed.product.result_domain,
        observed.product.portfolio,
        observed.product.product_basis,
        observed.representation.execution_descriptor,
        observed.representation.descriptor,
        observed.representation.graph,
        observed.representation.translation,
        observed.representation.exposure,
    ]
}

fn authenticate_record<'a>(
    registry: Pubkey,
    observed: FinalizedRecordObservationV3<'a>,
    expected_schema: [u8; 32],
) -> Result<AuthenticatedRecordV3<'a>> {
    if observed.schema_id != expected_schema
        || observed.raw.owner != registry
        || observed.raw.executable
        || observed.raw.data.is_empty()
        || observed.raw.lamports < observed.raw_rent_minimum
        || observed.raw_rent_minimum == 0
        || observed.staging.owner != system_program::ID
        || observed.staging.executable
        || !observed.staging.data.is_empty()
        || observed.raw.key == observed.staging.key
    {
        return Err(Error::FinalizedRecord);
    }
    let digest = hash(&observed.raw.data).to_bytes();
    let raw_account = Pubkey::find_program_address(
        &[
            dclutch_record_contract::RAW_RECORD_PDA_SEED_V1,
            &expected_schema,
            &digest,
        ],
        &registry,
    )
    .0;
    let staging_account = Pubkey::find_program_address(
        &[
            dclutch_record_contract::STAGING_CURSOR_PDA_SEED_V1,
            &expected_schema,
            &digest,
        ],
        &registry,
    )
    .0;
    if observed.raw.key != raw_account || observed.staging.key != staging_account {
        return Err(Error::FinalizedRecord);
    }
    Ok(AuthenticatedRecordV3 {
        bytes: &observed.raw.data,
        coordinate: FinalizedCoordinateV3 {
            schema_id: expected_schema,
            content_digest: digest,
            raw_account,
            staging_account,
        },
    })
}

fn product_coordinate(value: FinalizedCoordinateV3) -> Result<FinalizedRecordCoordinateV2> {
    Ok(FinalizedRecordCoordinateV2 {
        schema_id: ContentId::new(value.schema_id).map_err(|_| Error::Product)?,
        content_digest: ContentId::new(value.content_digest).map_err(|_| Error::Product)?,
        raw_account: ContentId::new(value.raw_account.to_bytes()).map_err(|_| Error::Product)?,
        staging_account: ContentId::new(value.staging_account.to_bytes())
            .map_err(|_| Error::Product)?,
    })
}

fn semantic_basis_id(bytes: &[u8]) -> Result<[u8; 32]> {
    let semantic = semantic_basis_preimage_v3(bytes).map_err(|_| Error::ProductBasis)?;
    Ok(hashv(&[
        SEMANTIC_BASIS_CONTENT_DOMAIN_V3,
        semantic.prefix(),
        semantic.suffix(),
    ])
    .to_bytes())
}

const fn admission(selected_id: [u8; 32], digest: [u8; 32]) -> RecordAdmissionV3 {
    RecordAdmissionV3 {
        selected_id,
        finalized_id: selected_id,
        recomputed_digest: digest,
        finalized_digest: digest,
        record_authenticated: true,
    }
}
