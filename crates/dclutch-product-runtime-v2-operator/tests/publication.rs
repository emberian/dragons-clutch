//! Generic Registry publication and Product graph join tests.

use dclutch_product_runtime_v2::{ContentId, portfolio_record_bytes, result_domain_record_bytes};
use dclutch_product_runtime_v2_admission::PRODUCT_RECORD_BYTES_V2;
use dclutch_product_runtime_v2_operator::{
    AccountObservationV2, ProductCompilationInputV2, compile_product_records_v2,
    publication::{
        ProductPublicationMemberV2, ProductPublicationStateV2, PublicationErrorV1,
        RecordPublicationActionV1, RecordPublicationContentV1, RecordPublicationStateV1,
        build_product_publication_step_v2, build_record_publication_step_v1,
        derive_record_addresses_v1, product_publication_content_v2,
    },
};
use dclutch_record_contract::{
    AccountId, AppendPageV1, BeginRecordV1, CANONICAL_RECORD_DEPLOYMENT_PROFILE_V1,
    RecordAdapterV1, STAGING_CURSOR_BYTES_V1, StagingCursorV1, StagingLivenessPolicyV1,
    prepare_append_page_v1, prepare_begin_v1,
};
use solana_program::{
    account_info::AccountInfo, clock::Clock, pubkey::Pubkey, rent::Rent, sysvar::SysvarSerialize,
};
use solana_sdk_ids::{native_loader, system_program, sysvar};

const SLOT: u64 = 730;
const CLOCK_SLOT: u64 = 19_000;
const REGISTRY: Pubkey = Pubkey::new_from_array([0x71; 32]);
const SPONSOR: Pubkey = Pubkey::new_from_array([0x72; 32]);

fn account<'a>(
    key: Pubkey,
    owner: Pubkey,
    lamports: u64,
    executable: bool,
    data: &'a [u8],
) -> AccountObservationV2<'a> {
    AccountObservationV2 {
        slot: SLOT,
        key,
        owner,
        lamports,
        executable,
        data,
    }
}

fn rent_data() -> Vec<u8> {
    let rent = Rent::default();
    let mut lamports = 1;
    let mut data = vec![0; Rent::size_of()];
    let key = sysvar::rent::ID;
    let owner = sysvar::ID;
    let mut info = AccountInfo::new(&key, false, false, &mut lamports, &mut data, &owner, false);
    rent.to_account_info(&mut info).expect("serialize Rent");
    data
}

fn clock_data() -> Vec<u8> {
    let clock = Clock {
        slot: CLOCK_SLOT,
        ..Clock::default()
    };
    let mut lamports = 1;
    let mut data = vec![0; Clock::size_of()];
    let key = sysvar::clock::ID;
    let owner = sysvar::ID;
    let mut info = AccountInfo::new(&key, false, false, &mut lamports, &mut data, &owner, false);
    clock.to_account_info(&mut info).expect("serialize Clock");
    data
}

#[derive(Clone, Copy)]
struct ObservedState<'a> {
    key: Pubkey,
    owner: Pubkey,
    lamports: u64,
    data: &'a [u8],
}

fn observed<'a>(key: Pubkey, owner: Pubkey, lamports: u64, data: &'a [u8]) -> ObservedState<'a> {
    ObservedState {
        key,
        owner,
        lamports,
        data,
    }
}

fn state<'a>(
    raw: ObservedState<'a>,
    cursor: ObservedState<'a>,
    rent: &'a [u8],
    clock: &'a [u8],
) -> RecordPublicationStateV1<'a> {
    RecordPublicationStateV1 {
        sponsor: account(SPONSOR, system_program::ID, 10_000_000_000, false, &[]),
        raw_record: account(raw.key, raw.owner, raw.lamports, false, raw.data),
        staging_cursor: account(
            cursor.key,
            cursor.owner,
            cursor.lamports,
            false,
            cursor.data,
        ),
        system_program: account(system_program::ID, native_loader::ID, 1, true, &[]),
        rent: account(sysvar::rent::ID, sysvar::ID, 1, false, rent),
        clock: account(sysvar::clock::ID, sysvar::ID, 1, false, clock),
    }
}

struct AcceptAdapter;

impl RecordAdapterV1 for AcceptAdapter {
    fn validate_page_envelope(&self, envelope: &dclutch_record_contract::PageEnvelopeV1) -> bool {
        CANONICAL_RECORD_DEPLOYMENT_PROFILE_V1.validates_page_envelope(*envelope)
    }

    fn validate_staging_liveness_policy(&self, _: &StagingLivenessPolicyV1) -> bool {
        true
    }

    fn validate_canonical_addresses(
        &self,
        _: &dclutch_record_contract::AddressDerivationObligationV1,
    ) -> bool {
        true
    }

    fn validate_raw_record(
        &self,
        _: &dclutch_record_contract::RawRecordValidationObligationV1<'_>,
    ) -> bool {
        true
    }
}

fn cursor_from_begin(
    begin: BeginRecordV1,
    raw: Pubkey,
    cursor: Pubkey,
    cursor_rent: u64,
) -> StagingCursorV1 {
    let policy = CANONICAL_RECORD_DEPLOYMENT_PROFILE_V1
        .staging_liveness_policy(cursor_rent)
        .expect("canonical liveness");
    prepare_begin_v1(
        &AcceptAdapter,
        begin,
        policy,
        CLOCK_SLOT,
        AccountId::new(raw.to_bytes()).expect("raw identity"),
        AccountId::new(cursor.to_bytes()).expect("cursor identity"),
        AccountId::new(SPONSOR.to_bytes()).expect("sponsor identity"),
    )
    .expect("valid Begin")
    .cursor()
}

#[test]
fn generic_publication_selects_begin_append_finalize_and_complete() {
    let content_bytes = vec![0x5a; 1_031];
    let content = RecordPublicationContentV1 {
        schema_release_id: [0x44; 32],
        content: &content_bytes,
    };
    let (raw, cursor, digest) =
        derive_record_addresses_v1(REGISTRY, content).expect("canonical addresses");
    let rent_bytes = rent_data();
    let clock_bytes = clock_data();
    let vacant = state(
        observed(raw, system_program::ID, 17, &[]),
        observed(cursor, system_program::ID, 23, &[]),
        &rent_bytes,
        &clock_bytes,
    );
    let begin = build_record_publication_step_v1(REGISTRY, content, vacant).expect("Begin");
    assert_eq!(begin.action, RecordPublicationActionV1::Begin);
    assert_eq!(begin.content_digest, digest);
    assert!(begin.sponsor_debit > 0);
    let begin_wire =
        BeginRecordV1::decode(&begin.instruction.as_ref().expect("Begin instruction").data)
            .expect("canonical Begin wire");
    assert_eq!(
        begin_wire.expiry_slot(),
        CLOCK_SLOT + CANONICAL_RECORD_DEPLOYMENT_PROFILE_V1.maximum_staging_lifetime_slots()
    );

    let cursor_rent = Rent::default().minimum_balance(STAGING_CURSOR_BYTES_V1);
    let mut cursor_value = cursor_from_begin(begin_wire, raw, cursor, cursor_rent);
    let mut raw_bytes = vec![0; content_bytes.len()];
    let live_cursor = cursor_value.to_bytes();
    let live = state(
        observed(
            raw,
            REGISTRY,
            Rent::default().minimum_balance(raw_bytes.len()),
            &raw_bytes,
        ),
        observed(cursor, REGISTRY, cursor_rent * 2, &live_cursor),
        &rent_bytes,
        &clock_bytes,
    );
    let append = build_record_publication_step_v1(REGISTRY, content, live).expect("Append");
    assert_eq!(append.action, RecordPublicationActionV1::Append);
    let append_wire = AppendPageV1::decode(
        &append
            .instruction
            .as_ref()
            .expect("Append instruction")
            .data,
    )
    .expect("canonical Append wire");
    assert_eq!(append_wire.page_index(), 0);
    assert_eq!(
        append_wire.page().len(),
        usize::try_from(CANONICAL_RECORD_DEPLOYMENT_PROFILE_V1.page_bytes()).expect("page width")
    );

    for (page_index, page) in content_bytes
        .chunks(
            usize::try_from(CANONICAL_RECORD_DEPLOYMENT_PROFILE_V1.page_bytes())
                .expect("page width"),
        )
        .enumerate()
    {
        let offset = u64::try_from(
            page_index
                * usize::try_from(CANONICAL_RECORD_DEPLOYMENT_PROFILE_V1.page_bytes())
                    .expect("page width"),
        )
        .expect("offset");
        let request = AppendPageV1::new(u64::try_from(page_index).expect("page"), offset, page)
            .expect("Append request");
        let transition = prepare_append_page_v1(
            cursor_value,
            AccountId::new(raw.to_bytes()).expect("raw identity"),
            AccountId::new(cursor.to_bytes()).expect("cursor identity"),
            u64::try_from(content_bytes.len()).expect("record length"),
            request,
        )
        .expect("append transition");
        let start = usize::try_from(transition.write().offset()).expect("start");
        let end = start + transition.write().page().len();
        raw_bytes
            .get_mut(start..end)
            .expect("checked raw page")
            .copy_from_slice(transition.write().page());
        cursor_value = transition.next_cursor();
    }
    assert!(cursor_value.is_complete());
    let complete_cursor = cursor_value.to_bytes();
    let staged_complete = state(
        observed(
            raw,
            REGISTRY,
            Rent::default().minimum_balance(raw_bytes.len()),
            &raw_bytes,
        ),
        observed(cursor, REGISTRY, cursor_rent * 2, &complete_cursor),
        &rent_bytes,
        &clock_bytes,
    );
    let finalize =
        build_record_publication_step_v1(REGISTRY, content, staged_complete).expect("Finalize");
    assert_eq!(finalize.action, RecordPublicationActionV1::Finalize);
    assert_eq!(finalize.cursor_refund, cursor_rent * 2);

    let finalized = state(
        observed(
            raw,
            REGISTRY,
            Rent::default().minimum_balance(raw_bytes.len()),
            &raw_bytes,
        ),
        observed(cursor, system_program::ID, 0, &[]),
        &rent_bytes,
        &clock_bytes,
    );
    let complete =
        build_record_publication_step_v1(REGISTRY, content, finalized).expect("Complete");
    assert_eq!(complete.action, RecordPublicationActionV1::Complete);
    assert!(complete.instruction.is_none());
}

#[test]
fn publication_refuses_substitution_and_late_content_mismatch() {
    let bytes = vec![0x31; 9];
    let content = RecordPublicationContentV1 {
        schema_release_id: [0x45; 32],
        content: &bytes,
    };
    let (raw, cursor, _) = derive_record_addresses_v1(REGISTRY, content).expect("addresses");
    let rent_bytes = rent_data();
    let clock_bytes = clock_data();
    let wrong = state(
        observed(Pubkey::new_unique(), system_program::ID, 0, &[]),
        observed(cursor, system_program::ID, 0, &[]),
        &rent_bytes,
        &clock_bytes,
    );
    assert_eq!(
        build_record_publication_step_v1(REGISTRY, content, wrong),
        Err(PublicationErrorV1::AddressMismatch)
    );

    let mut hostile = bytes.clone();
    *hostile.get_mut(8).expect("hostile byte") ^= 1;
    let finalized = state(
        observed(
            raw,
            REGISTRY,
            Rent::default().minimum_balance(hostile.len()),
            &hostile,
        ),
        observed(cursor, system_program::ID, 0, &[]),
        &rent_bytes,
        &clock_bytes,
    );
    assert_eq!(
        build_record_publication_step_v1(REGISTRY, content, finalized),
        Err(PublicationErrorV1::Record)
    );

    let mut stale = state(
        observed(raw, system_program::ID, 0, &[]),
        observed(cursor, system_program::ID, 0, &[]),
        &rent_bytes,
        &clock_bytes,
    );
    stale.clock.slot += 1;
    assert_eq!(
        build_record_publication_step_v1(REGISTRY, content, stale),
        Err(PublicationErrorV1::ObservationMismatch)
    );
}

fn id(byte: u8) -> ContentId {
    ContentId::new([byte; 32]).expect("identity")
}

#[test]
fn compiled_product_graph_owns_schemas_digests_and_publication_order() {
    let cuts = [-1_i128, 0, 1];
    let coefficients = [1_u64; 5];
    let mut product = [0; PRODUCT_RECORD_BYTES_V2];
    let mut domain = vec![0; result_domain_record_bytes(cuts.len()).expect("domain width")];
    let mut portfolio =
        vec![0; portfolio_record_bytes(coefficients.len()).expect("portfolio width")];
    let compiled = compile_product_records_v2(
        REGISTRY,
        ProductCompilationInputV2 {
            product_id: id(1),
            coordinate_domain_id: id(2),
            result_unit_id: id(3),
            claim_basis_id: id(4),
            liability_basis_id: id(5),
            representation_release_id: id(6),
            mapping_release_id: id(7),
            cut_denominator: 1,
            cuts: &cuts,
            portfolio_denominator: 5,
            coefficients: &coefficients,
        },
        &mut product,
        &mut domain,
        &mut portfolio,
    )
    .expect("compiled Product graph");
    let content = product_publication_content_v2(REGISTRY, compiled, &product, &domain, &portfolio)
        .expect("joined Product graph");
    let mut hostile_portfolio = portfolio.clone();
    let hostile_index = hostile_portfolio.len() - 1;
    *hostile_portfolio
        .get_mut(hostile_index)
        .expect("hostile portfolio byte") ^= 1;
    assert_eq!(
        product_publication_content_v2(REGISTRY, compiled, &product, &domain, &hostile_portfolio,),
        Err(PublicationErrorV1::ProductGraphMismatch)
    );

    let rent_bytes = rent_data();
    let clock_bytes = clock_data();
    let (product_raw, product_cursor, _) =
        derive_record_addresses_v1(REGISTRY, content.product).expect("Product addresses");
    let (domain_raw, domain_cursor, _) =
        derive_record_addresses_v1(REGISTRY, content.result_domain).expect("domain addresses");
    let (portfolio_raw, portfolio_cursor, _) =
        derive_record_addresses_v1(REGISTRY, content.portfolio).expect("portfolio addresses");
    let product_state = state(
        observed(
            product_raw,
            REGISTRY,
            Rent::default().minimum_balance(product.len()),
            &product,
        ),
        observed(product_cursor, system_program::ID, 0, &[]),
        &rent_bytes,
        &clock_bytes,
    );
    let domain_state = state(
        observed(domain_raw, system_program::ID, 0, &[]),
        observed(domain_cursor, system_program::ID, 0, &[]),
        &rent_bytes,
        &clock_bytes,
    );
    let portfolio_state = state(
        observed(portfolio_raw, system_program::ID, 0, &[]),
        observed(portfolio_cursor, system_program::ID, 0, &[]),
        &rent_bytes,
        &clock_bytes,
    );
    let next = build_product_publication_step_v2(
        REGISTRY,
        content,
        ProductPublicationStateV2 {
            product: product_state,
            result_domain: domain_state,
            portfolio: portfolio_state,
        },
    )
    .expect("next graph publication");
    assert_eq!(next.member, ProductPublicationMemberV2::ResultDomain);
    assert_eq!(next.record.action, RecordPublicationActionV1::Begin);
}
