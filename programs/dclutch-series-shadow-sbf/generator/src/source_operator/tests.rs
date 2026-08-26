use dclutch_account_profile_contract::lifecycle_v3::{
    CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5, HEADER_BYTES as LIFECYCLE_BYTES_V5,
    encode::encode_lifecycle_policy_v5_atomic,
};
use dclutch_capability_program_contract::{
    set_v2::{
        CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2, CapabilityDescriptorReferenceV2,
        CapabilityProgramSetEntryV2, SelectorWidthV2, encode_program_set_v2,
        encoded_program_set_bytes_v2,
    },
    v4::SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V4,
};
use dclutch_claims_svm::founding_v5::{ClaimsFoundingRequestInputV5, ClaimsFoundingRequestV5};
use dclutch_core_contract::ContentId;
use dclutch_custody_contract::{
    CompartmentV1, ProjectedCallerRoleV1, ProjectedCustodyOperationV1, ProjectedCustodyRequestV1,
};
use dclutch_product_runtime_v2::{
    ContentId as ProductContentId, PortfolioInputV2, ResultDomainInputV2, compile_portfolio_v2,
    compile_result_domain_v2, portfolio_record_bytes, result_domain_record_bytes,
};
use dclutch_product_runtime_v2_admission::{
    PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_BYTES_V2, PRODUCT_RECORD_SCHEMA_ID_V2, ProductRecordV2,
    RESULT_DOMAIN_SCHEMA_ID_V2,
};
use dclutch_record_contract::{
    AccountId, AddressDerivationObligationV1, ContentDigest, PageEnvelopeV1,
    RawRecordValidationObligationV1, RecordAdapterV1, RecordKeyV1, SchemaReleaseId,
    StagingLivenessPolicyV1, authenticate_finalized_raw_record_v1,
};
use dclutch_series_v3_kernel::{
    AccountKeyV3, AuthenticatedProductProjectionV2, OccurrenceV3, TemplateV3,
    generated::{
        SERIES_EXAMPLE_OCCURRENCE_V3, SERIES_EXAMPLE_TEMPLATE_V3, SERIES_EXAMPLE_TICKET_V3,
        SERIES_OCCURRENCE_PRODUCT_RECORD_OFFSET_V3, SERIES_PROJECTION_NODE_DOMAIN_V3,
        SERIES_TEMPLATE_PROJECTION_ROOT_OFFSET_V3, SERIES_TICKET_MARKET_OFFSET_V3,
        SERIES_TICKET_OCCURRENCE_ID_OFFSET_V3, SERIES_TICKET_TEMPLATE_OFFSET_V3,
    },
    occurrence_content_id,
    request::{SeriesActionV3, admit_series_action_v3, encode_series_action_header_v3},
    series_core_consume_request, template_content_id, ticket_content_id,
};
use dclutch_trading_sbf::series::consume_artifacts_v4::SeriesConsumeChildRequestsV4;
use sha2::{Digest, Sha256};

use super::*;
use crate::{
    SERIES_SHADOW_FIXED_ACCOUNT_COUNT_V4, SeriesShadowBundleSourceV4,
    SeriesShadowDescriptorSemanticsV4, SeriesShadowReleaseSourcesV4,
    compile_series_shadow_bundle_v4,
};

const SEMANTIC_SOURCE: &[u8] =
    include_bytes!("../../../../dclutch-trading-sbf/src/series/consume_artifacts_v4.rs");
const EPHEMERAL_COMPILER_SOURCE: &[u8] =
    b"test-only:series-shadow-source-operator-v1;not release evidence";
const EPHEMERAL_TOOLCHAIN: &[u8] = b"test-only:rustc-1.89.0;not release evidence";
const HASH_SEPARATOR: [u8; 1] = [0];

struct AcceptingRecordAdapter;

impl RecordAdapterV1 for AcceptingRecordAdapter {
    fn validate_page_envelope(&self, _: &PageEnvelopeV1) -> bool {
        true
    }

    fn validate_staging_liveness_policy(&self, _: &StagingLivenessPolicyV1) -> bool {
        true
    }

    fn validate_canonical_addresses(&self, _: &AddressDerivationObligationV1) -> bool {
        true
    }

    fn validate_raw_record(&self, _: &RawRecordValidationObligationV1<'_>) -> bool {
        true
    }
}

struct Fixture {
    observation: ContentId,
    template: [u8; dclutch_series_v3_kernel::generated::SERIES_TEMPLATE_BYTES_V3],
    occurrence: [u8; dclutch_series_v3_kernel::generated::SERIES_OCCURRENCE_BYTES_V3],
    ticket: [u8; dclutch_series_v3_kernel::generated::SERIES_TICKET_BYTES_V3],
    product: [u8; PRODUCT_RECORD_BYTES_V2],
    domain: Vec<u8>,
    portfolio: Vec<u8>,
    program_set: Vec<u8>,
    descriptor: Vec<u8>,
    lifecycle: [u8; LIFECYCLE_BYTES_V5],
    family_request: Vec<u8>,
    lock: [u8; dclutch_custody_contract::PROJECTED_CUSTODY_REQUEST_BYTES_V1],
    core: [u8; dclutch_market_core_codec::SERIES_CORE_REQUEST_BYTES_V1],
    realize: [u8; dclutch_custody_contract::PROJECTED_CUSTODY_REQUEST_BYTES_V1],
    claims: [u8; dclutch_claims_svm::founding_v5::CLAIMS_FOUNDING_REQUEST_BYTES_V5],
    widths: [u32; SERIES_SHADOW_FIXED_ACCOUNT_COUNT_V4],
    checked_release: CheckedSeriesShadowReleaseV1,
    replay: SeriesShadowReplaySourceV1,
}

impl Fixture {
    fn new() -> Self {
        let observation = identity(240);
        let product_id = product_identity(40);
        let liability_basis = SERIES_EXAMPLE_OCCURRENCE_V3
            .get(
                dclutch_series_v3_kernel::generated::SERIES_OCCURRENCE_LIABILITY_BASIS_OFFSET_V3
                    ..dclutch_series_v3_kernel::generated::SERIES_OCCURRENCE_LIABILITY_BASIS_OFFSET_V3
                        + 32,
            )
            .expect("fixed liability-basis field");
        let liability_basis: [u8; 32] = liability_basis.try_into().expect("exact identity");
        let representation = product_identity(41);
        let domain_length = result_domain_record_bytes(2).expect("domain length");
        let mut domain = vec![0; domain_length];
        compile_result_domain_v2(
            ResultDomainInputV2 {
                product_id,
                coordinate_domain_id: product_identity(42),
                result_unit_id: product_identity(43),
                liability_basis_id: ProductContentId::new(liability_basis)
                    .expect("liability basis"),
                representation_release_id: representation,
                mapping_release_id: product_identity(44),
                cut_denominator: 10,
                cuts: &[1, 2],
            },
            &mut domain,
        )
        .expect("domain encodes");
        let domain_id = digest(&domain);
        let portfolio_length = portfolio_record_bytes(4).expect("portfolio length");
        let mut portfolio = vec![0; portfolio_length];
        compile_portfolio_v2(
            PortfolioInputV2 {
                product_id,
                result_domain_id: ProductContentId::new(domain_id.to_bytes())
                    .expect("domain identity"),
                claim_basis_id: product_identity(45),
                liability_basis_id: ProductContentId::new(liability_basis)
                    .expect("liability basis"),
                representation_release_id: representation,
                denominator: 1,
                coefficients: &[1, 0, 0, 0],
            },
            &mut portfolio,
        )
        .expect("portfolio encodes");
        let portfolio_id = digest(&portfolio);
        let mut product = [0; PRODUCT_RECORD_BYTES_V2];
        ProductRecordV2::new(
            product_id,
            ProductContentId::new(domain_id.to_bytes()).expect("domain identity"),
            ProductContentId::new(portfolio_id.to_bytes()).expect("portfolio identity"),
        )
        .encode_into(&mut product)
        .expect("Product record encodes");
        let product_record = digest(&product);

        let mut occurrence = SERIES_EXAMPLE_OCCURRENCE_V3;
        put(
            &mut occurrence,
            SERIES_OCCURRENCE_PRODUCT_RECORD_OFFSET_V3,
            &product_record.to_bytes(),
        );
        let occurrence_value = OccurrenceV3::decode(&occurrence).expect("Occurrence decodes");
        let siblings = [[90; 32], [91; 32]];
        let occurrence_id = occurrence_content_id(&occurrence).expect("Occurrence identity");
        let mut template = SERIES_EXAMPLE_TEMPLATE_V3;
        put(
            &mut template,
            SERIES_TEMPLATE_PROJECTION_ROOT_OFFSET_V3,
            &projection_root(occurrence_id, occurrence_value.occurrence(), &siblings),
        );
        let template_id = template_content_id(&template).expect("Template identity");
        let mut ticket = SERIES_EXAMPLE_TICKET_V3;
        put(
            &mut ticket,
            SERIES_TICKET_TEMPLATE_OFFSET_V3,
            &template_id.to_bytes(),
        );
        put(
            &mut ticket,
            SERIES_TICKET_OCCURRENCE_ID_OFFSET_V3,
            &occurrence_id.to_bytes(),
        );
        put(
            &mut ticket,
            SERIES_TICKET_MARKET_OFFSET_V3,
            &occurrence_value.market().to_bytes(),
        );
        let ticket_id = ticket_content_id(&ticket).expect("Ticket identity");
        let header = encode_series_action_header_v3(
            SeriesActionV3::Consume,
            template_id,
            Some(occurrence_id),
            Some(ticket_id),
            4,
            0,
            2,
        )
        .expect("Consume header");
        let mut family_request = Vec::from(header);
        family_request.extend_from_slice(&siblings[0]);
        family_request.extend_from_slice(&siblings[1]);
        let admitted =
            admit_series_action_v3(&family_request, &template, Some(&occurrence), Some(&ticket))
                .expect("Series admission");
        let projection = AuthenticatedProductProjectionV2::new(
            product_record,
            ContentId::new(product_id.to_bytes()).expect("stable Product"),
            domain_id,
        );
        let replay = SeriesShadowReplaySourceV1 {
            observation,
            ticket_state_account: AccountKeyV3::new(tag(46)).expect("Ticket state"),
            expected_series_revision: 4,
            expected_ticket_revision: 0,
        };
        let core = series_core_consume_request(
            admitted
                .required_occurrence()
                .expect("Occurrence admission"),
            admitted.required_ticket().expect("Ticket admission"),
            projection,
            replay.ticket_state_account,
            replay.expected_series_revision,
            replay.expected_ticket_revision,
        )
        .expect("Core semantic request")
        .encode()
        .expect("Core bytes");
        let template_value = TemplateV3::decode(&template).expect("Template decodes");
        let refund_owner = admitted
            .required_ticket()
            .expect("Ticket admission")
            .ticket()
            .refund_owner()
            .to_bytes();
        let lock_request = projected_request(
            ProjectedCustodyOperationV1::LockHoardAndCloseSource,
            template_value,
            occurrence_value,
            ticket_id,
            product_id.to_bytes(),
            refund_owner,
            (2, 3),
        );
        let realize_request = projected_request(
            ProjectedCustodyOperationV1::RealizeAndClose,
            template_value,
            occurrence_value,
            ticket_id,
            product_id.to_bytes(),
            refund_owner,
            (3, 4),
        );
        let lock = lock_request.encode().expect("Lock request encodes");
        let realize = realize_request.encode().expect("Realize request encodes");
        let amount = occurrence_value.funds().hoard_principal();
        let claims = ClaimsFoundingRequestV5::new(ClaimsFoundingRequestInputV5 {
            release_set: template_value.release_set().to_bytes(),
            market: occurrence_value.market().to_bytes(),
            product_record_digest: product_record.to_bytes(),
            product_instance_id: product_id.to_bytes(),
            linked_basis_record_digest: tag(47),
            semantic_basis_id: occurrence_value.liability_basis().to_bytes(),
            founder: admitted
                .required_ticket()
                .expect("Ticket admission")
                .ticket()
                .founder()
                .to_bytes(),
            founding_intent_digest: tag(48),
            aggregate: tag(49),
            position: tag(50),
            admission: tag(51),
            hoard: lock_request.hoard_vault,
            rent_credit: tag(52),
            rent_program: tag(53),
            claims_program: tag(54),
            trading_program: lock_request.caller_program,
            funding_source: lock_request.funding_source_vault,
            custody_replay: tag(55),
            custody_request_digest: Sha256::digest(lock).into(),
            custody_receipt_digest: tag(56),
            generation: lock_request.generation,
            claim_count: 4,
            quantity: amount,
            basis_scale: 1,
            pre_source_amount: amount,
            post_source_amount: 0,
            pre_hoard_amount: 0,
            post_hoard_amount: amount,
            pre_custody_revision: 2,
            post_custody_revision: 3,
            aggregate_rent_principal: 1,
            position_rent_principal: 1,
            admission_rent_principal: 1,
            observed_aggregate_lamports: 1,
            observed_position_lamports: 1,
            observed_admission_lamports: 1,
            pre_aggregate_revision: 0,
            post_aggregate_revision: 1,
            pre_position_revision: 0,
            post_position_revision: 1,
        })
        .expect("Claims request")
        .to_bytes();

        let mut lifecycle_scratch = [0; LIFECYCLE_BYTES_V5];
        let mut lifecycle = [0; LIFECYCLE_BYTES_V5];
        encode_lifecycle_policy_v5_atomic(
            &[],
            &[],
            &[],
            &[],
            &[],
            &[],
            &mut lifecycle_scratch,
            &mut lifecycle,
        )
        .expect("canonical empty LifecycleV5");
        let widths = [64; SERIES_SHADOW_FIXED_ACCOUNT_COUNT_V4];
        let descriptor_semantics = SeriesShadowDescriptorSemanticsV4 {
            kind: identity(60),
            config_schema: identity(61),
            request_schema: identity(62),
            root_schema: identity(63),
            derivation_policy: identity(64),
            capacity_profile: identity(65),
            root_state_bytes: 64,
        };
        let certificate = identity(66);
        let compiled = compile_series_shadow_bundle_v4(SeriesShadowBundleSourceV4 {
            descriptor: descriptor_semantics,
            release_sources: SeriesShadowReleaseSourcesV4 {
                semantic_source: SEMANTIC_SOURCE,
                compiler_source: EPHEMERAL_COMPILER_SOURCE,
                toolchain_manifest: EPHEMERAL_TOOLCHAIN,
                certificate,
            },
            lifecycle: &lifecycle,
            fixed_data_lengths: &widths,
            child_requests: SeriesConsumeChildRequestsV4 {
                lock: &lock,
                core: &core,
                realize: &realize,
                claims: &claims,
            },
        })
        .expect("exact generated bundle");
        let descriptor = Vec::from(compiled.capability_program);
        let descriptor_schema = identity_from_bytes(CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V4);
        let descriptor_program = digest(&descriptor);
        let mut program_set = vec![0; encoded_program_set_bytes_v2(1).expect("Set width")];
        encode_program_set_v2(
            12,
            SelectorWidthV2::U8,
            &[CapabilityProgramSetEntryV2::new(
                u32::from(SeriesActionV3::Consume as u8),
                CapabilityDescriptorReferenceV2::new(descriptor_schema, descriptor_program),
            )],
            &mut program_set,
        )
        .expect("ProgramSet encodes");
        let checked_release = CheckedSeriesShadowReleaseV1 {
            observation,
            program_set: digest(&program_set),
            descriptor_schema,
            descriptor_program,
            lifecycle: digest(&lifecycle),
            semantic_source: digest(SEMANTIC_SOURCE),
            compiler_source: digest(EPHEMERAL_COMPILER_SOURCE),
            toolchain: digest(EPHEMERAL_TOOLCHAIN),
            certificate,
        };
        Self {
            observation,
            template,
            occurrence,
            ticket,
            product,
            domain,
            portfolio,
            program_set,
            descriptor,
            lifecycle,
            family_request,
            lock,
            core,
            realize,
            claims,
            widths,
            checked_release,
            replay,
        }
    }

    fn input(&self) -> SeriesShadowObservedSourceV1<'_> {
        SeriesShadowObservedSourceV1 {
            records: SeriesShadowFinalizedRecordsV1 {
                template: record(
                    self.observation,
                    dclutch_series_v3_kernel::generated::SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
                    &self.template,
                    1,
                ),
                occurrence: record(
                    self.observation,
                    dclutch_series_v3_kernel::generated::SERIES_OCCURRENCE_SCHEMA_RELEASE_ID_V3,
                    &self.occurrence,
                    3,
                ),
                ticket: record(
                    self.observation,
                    dclutch_series_v3_kernel::generated::SERIES_TICKET_SCHEMA_RELEASE_ID_V3,
                    &self.ticket,
                    5,
                ),
                product: record(
                    self.observation,
                    PRODUCT_RECORD_SCHEMA_ID_V2,
                    &self.product,
                    7,
                ),
                result_domain: record(
                    self.observation,
                    RESULT_DOMAIN_SCHEMA_ID_V2,
                    &self.domain,
                    9,
                ),
                portfolio: record(
                    self.observation,
                    PORTFOLIO_SCHEMA_ID_V2,
                    &self.portfolio,
                    11,
                ),
                program_set: record(
                    self.observation,
                    CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
                    &self.program_set,
                    13,
                ),
                descriptor: record(
                    self.observation,
                    CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V4,
                    &self.descriptor,
                    15,
                ),
                lifecycle: record(
                    self.observation,
                    CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5,
                    &self.lifecycle,
                    17,
                ),
            },
            checked_release: self.checked_release,
            family_request: &self.family_request,
            replay: self.replay,
            account_widths: SeriesShadowAccountWidthsV1 {
                observation: self.observation,
                fixed_data_lengths: &self.widths,
            },
            child_requests: SeriesConsumeChildRequestsV4 {
                lock: &self.lock,
                core: &self.core,
                realize: &self.realize,
                claims: &self.claims,
            },
            semantic_source: SEMANTIC_SOURCE,
            compiler_source_manifest: EPHEMERAL_COMPILER_SOURCE,
            toolchain_manifest: EPHEMERAL_TOOLCHAIN,
        }
    }
}

#[test]
fn chain_derived_source_is_byte_identical_and_emits_exact_build_inputs() {
    let fixture = Fixture::new();
    let first = build_series_shadow_source_v1(fixture.input()).expect("first source build");
    let second = build_series_shadow_source_v1(fixture.input()).expect("second source build");
    assert_eq!(first, second);
    assert_eq!(first.build_inputs.source_manifest, digest(&first.manifest));
    assert_eq!(
        first.build_inputs.generated_include,
        digest(&first.generated_include)
    );
    assert!(
        first
            .generated_include
            .windows(b"SERIES_SHADOW_LIFECYCLE_V5".len())
            .any(|window| window == b"SERIES_SHADOW_LIFECYCLE_V5")
    );
}

#[test]
fn stale_observation_and_toolchain_substitution_refuse() {
    let fixture = Fixture::new();
    let mut stale_observation = fixture.input();
    stale_observation.replay.observation = identity(239);
    assert_eq!(
        build_series_shadow_source_v1(stale_observation),
        Err(SeriesShadowSourceOperatorErrorV1::Observation)
    );

    let mut stale_toolchain = fixture.input();
    stale_toolchain.toolchain_manifest = b"substituted toolchain";
    assert_eq!(
        build_series_shadow_source_v1(stale_toolchain),
        Err(SeriesShadowSourceOperatorErrorV1::Source)
    );
}

#[test]
fn child_request_and_selected_descriptor_substitution_refuse() {
    let fixture = Fixture::new();
    let mut hostile_core = fixture.core;
    let core_byte = hostile_core.get_mut(32).expect("Core body byte");
    *core_byte ^= 1;
    let mut child_substitution = fixture.input();
    child_substitution.child_requests.core = &hostile_core;
    assert_eq!(
        build_series_shadow_source_v1(child_substitution),
        Err(SeriesShadowSourceOperatorErrorV1::ChildRequest)
    );

    let mut descriptor_substitution = fixture.input();
    descriptor_substitution.checked_release.descriptor_program = identity(238);
    assert_eq!(
        build_series_shadow_source_v1(descriptor_substitution),
        Err(SeriesShadowSourceOperatorErrorV1::Record)
    );
}

fn projected_request(
    operation: ProjectedCustodyOperationV1,
    template: TemplateV3,
    occurrence: OccurrenceV3,
    ticket: ContentId,
    product: [u8; 32],
    refund_owner: [u8; 32],
    revisions: (u64, u64),
) -> ProjectedCustodyRequestV1 {
    ProjectedCustodyRequestV1 {
        operation,
        caller_role: ProjectedCallerRoleV1::TradingCapability,
        market: occurrence.market().to_bytes(),
        generation: u64::from(occurrence.occurrence()) + 1,
        realm: template.realm().to_bytes(),
        product_record: occurrence.product_record().to_bytes(),
        product,
        source: occurrence.resolution_policy().to_bytes(),
        release_set: template.release_set().to_bytes(),
        projection_receipt_digest: tag(70),
        parent_capability_root: tag(71),
        context_digest: tag(72),
        caller_program: tag(73),
        payer: tag(74),
        core_program: tag(75),
        rent_program: tag(76),
        refund_owner,
        rent_credit: tag(77),
        hoard_vault: tag(78),
        funding_source_vault: tag(79),
        funding_source_context: ticket.to_bytes(),
        funding_source_compartment: CompartmentV1::SeriesEscrow,
        mint: tag(80),
        token_program: tag(81),
        collateral_release: tag(82),
        expiry_slot: template
            .retry_through(occurrence.occurrence())
            .expect("retry slot"),
        expected_revision: revisions.0,
        resulting_revision: revisions.1,
        amount: occurrence.funds().hoard_principal(),
        state_rent_lamports: 10,
        vault_rent_lamports: 11,
        funding_source_replay_revision: 1,
        funding_source_state_rent_lamports: 12,
        funding_source_vault_rent_lamports: 13,
    }
}

fn record<'a>(
    observation: ContentId,
    schema: [u8; 32],
    bytes: &'a [u8],
    account_tag: u8,
) -> ObservedSeriesShadowRecordV1<'a> {
    let key = RecordKeyV1::new(
        SchemaReleaseId::new(schema).expect("schema identity"),
        ContentDigest::new(digest(bytes).to_bytes()).expect("content digest"),
    );
    let record = authenticate_finalized_raw_record_v1(
        &AcceptingRecordAdapter,
        key,
        account(account_tag),
        account(account_tag.checked_add(1).expect("paired account")),
        bytes,
    )
    .expect("test adapter authentication");
    ObservedSeriesShadowRecordV1 {
        observation,
        record,
    }
}

fn projection_root(
    occurrence_id: ContentId,
    mut occurrence: u32,
    siblings: &[[u8; 32]],
) -> [u8; 32] {
    let mut node = occurrence_id.to_bytes();
    for sibling in siblings {
        let mut hasher = Sha256::new();
        hasher.update(SERIES_PROJECTION_NODE_DOMAIN_V3);
        hasher.update(HASH_SEPARATOR);
        if occurrence & 1 == 0 {
            hasher.update(node);
            hasher.update(sibling);
        } else {
            hasher.update(sibling);
            hasher.update(node);
        }
        node = hasher.finalize().into();
        occurrence >>= 1;
    }
    node
}

fn put<const N: usize>(bytes: &mut [u8], offset: usize, value: &[u8; N]) {
    bytes
        .get_mut(offset..offset.checked_add(N).expect("fixed offset"))
        .expect("fixed field")
        .copy_from_slice(value);
}

fn digest(bytes: &[u8]) -> ContentId {
    ContentId::new(Sha256::digest(bytes).into()).expect("nonzero content digest")
}

fn identity(tag: u8) -> ContentId {
    identity_from_bytes([tag; 32])
}

fn identity_from_bytes(bytes: [u8; 32]) -> ContentId {
    ContentId::new(bytes).expect("nonzero identity")
}

fn product_identity(tag: u8) -> ProductContentId {
    ProductContentId::new([tag; 32]).expect("nonzero Product identity")
}

fn tag(value: u8) -> [u8; 32] {
    [value; 32]
}

fn account(value: u8) -> AccountId {
    AccountId::new(tag(value)).expect("nonzero account")
}
