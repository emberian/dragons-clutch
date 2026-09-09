//! Generic Registry publication and Product graph join tests.

use dclutch_product::admission::PRODUCT_RECORD_BYTES_V2;
use dclutch_product::{ContentId, portfolio_record_bytes, result_domain_record_bytes};
use dclutch_product_runtime_v2_operator::{
    AccountObservationV2, ProductCompilationInputV2, compile_product_records_v2,
    publication::{
        ProductPublicationMemberV2, ProductPublicationStateV2, PublicationErrorV1,
        RecordPublicationActionV1, RecordPublicationContentV1, RecordPublicationStateV1,
        build_product_publication_step_v2, build_record_publication_step_v1,
        derive_record_addresses_v1, product_publication_content_v2,
    },
};
use dclutch_registry::record::{
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
// Agave 4.0.2 exposes this exact immutable NativeLoader metadata on the
// built-in System Program account. It is not executable program state owned
// by the caller and must not be confused with a vacant System-owned account.
const AGAVE_4_0_2_SYSTEM_PROGRAM_DATA: &[u8; 14] = b"system_program";

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
        system_program: account(
            system_program::ID,
            native_loader::ID,
            1,
            true,
            AGAVE_4_0_2_SYSTEM_PROGRAM_DATA,
        ),
        rent: account(sysvar::rent::ID, sysvar::ID, 1, false, rent),
        clock: account(sysvar::clock::ID, sysvar::ID, 1, false, clock),
    }
}

#[test]
fn publication_accepts_real_agave_system_metadata_and_refuses_authority_substitution() {
    let bytes = [0x30; 17];
    let content = RecordPublicationContentV1 {
        schema_release_id: [0x43; 32],
        content: &bytes,
    };
    let (raw, cursor, _) = derive_record_addresses_v1(REGISTRY, content).expect("addresses");
    let rent_bytes = rent_data();
    let clock_bytes = clock_data();
    let canonical = state(
        observed(raw, system_program::ID, 0, &[]),
        observed(cursor, system_program::ID, 0, &[]),
        &rent_bytes,
        &clock_bytes,
    );
    assert_eq!(
        build_record_publication_step_v1(REGISTRY, content, canonical)
            .expect("real System Program metadata")
            .action,
        RecordPublicationActionV1::Begin
    );

    let mut wrong_key = canonical;
    wrong_key.system_program.key = Pubkey::new_unique();
    assert_eq!(
        build_record_publication_step_v1(REGISTRY, content, wrong_key),
        Err(PublicationErrorV1::AccountAuthority)
    );
    let mut wrong_owner = canonical;
    wrong_owner.system_program.owner = system_program::ID;
    assert_eq!(
        build_record_publication_step_v1(REGISTRY, content, wrong_owner),
        Err(PublicationErrorV1::AccountAuthority)
    );
    let mut not_executable = canonical;
    not_executable.system_program.executable = false;
    assert_eq!(
        build_record_publication_step_v1(REGISTRY, content, not_executable),
        Err(PublicationErrorV1::AccountAuthority)
    );
}

struct AcceptAdapter;

impl RecordAdapterV1 for AcceptAdapter {
    fn validate_page_envelope(&self, envelope: &dclutch_registry::record::PageEnvelopeV1) -> bool {
        CANONICAL_RECORD_DEPLOYMENT_PROFILE_V1.validates_page_envelope(*envelope)
    }

    fn validate_staging_liveness_policy(&self, _: &StagingLivenessPolicyV1) -> bool {
        true
    }

    fn validate_canonical_addresses(
        &self,
        _: &dclutch_registry::record::AddressDerivationObligationV1,
    ) -> bool {
        true
    }

    fn validate_raw_record(
        &self,
        _: &dclutch_registry::record::RawRecordValidationObligationV1<'_>,
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
    assert_eq!(finalize.cursor_refund, Some(cursor_rent * 2));

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

    let mut stale_portfolio = portfolio_state;
    stale_portfolio.clock.slot += 1;
    assert_eq!(
        build_product_publication_step_v2(
            REGISTRY,
            content,
            ProductPublicationStateV2 {
                product: product_state,
                result_domain: domain_state,
                portfolio: stale_portfolio,
            },
        ),
        Err(PublicationErrorV1::ObservationMismatch)
    );
}

#[test]
fn artifact_publication_resumes_native_code_cursor_and_never_claims_partial_refund() {
    use dclutch_registry::artifact_code_commitment_v2::{
        CODE_COMMITMENT_CHUNK_BYTES_V2, CODE_COMMITMENT_PROGRESS_BYTES_V2,
        CodeCommitmentProgressV2, code_commitment_v2,
    };
    use dclutch_registry::record::staging_cursor_bytes_v1;
    use dclutch_registry::release_set::ProgramIdentityV1;
    use dclutch_registry::{
        ARTIFACT_RELEASE_SCHEMA_ID_V2, ArtifactReleaseV2, ArtifactUpgradePolicyV1,
    };
    let elf = vec![0x5a; CODE_COMMITMENT_CHUNK_BYTES_V2 + 17];
    let program = Pubkey::new_from_array([0x81; 32]);
    let programdata = Pubkey::new_from_array([0x82; 32]);
    let release = ArtifactReleaseV2::new(
        ProgramIdentityV1::new(program.to_bytes()).expect("program identity"),
        ProgramIdentityV1::new(solana_sdk_ids::bpf_loader_upgradeable::ID.to_bytes())
            .expect("loader identity"),
        programdata.to_bytes(),
        dclutch_core_contract::ContentId::new([0x83; 32]).expect("semantic identity"),
        code_commitment_v2(&elf).expect("exact ELF commitment"),
        7,
        ArtifactUpgradePolicyV1::Immutable,
        None,
    )
    .expect("artifact release");
    let body = release.to_bytes();
    let content = RecordPublicationContentV1 {
        schema_release_id: ARTIFACT_RELEASE_SCHEMA_ID_V2,
        content: &body,
    };
    let (raw, cursor, _) =
        derive_record_addresses_v1(REGISTRY, content).expect("artifact addresses");
    let rent_bytes = rent_data();
    let clock_bytes = clock_data();
    let vacant = state(
        observed(raw, system_program::ID, 0, &[]),
        observed(cursor, system_program::ID, 0, &[]),
        &rent_bytes,
        &clock_bytes,
    );
    let begin =
        build_record_publication_step_v1(REGISTRY, content, vacant).expect("artifact Begin");
    let begin_wire = BeginRecordV1::decode(&begin.instruction.expect("Begin instruction").data)
        .expect("Begin wire");
    let width = staging_cursor_bytes_v1(begin_wire.key());
    assert_eq!(
        width,
        STAGING_CURSOR_BYTES_V1 + CODE_COMMITMENT_PROGRESS_BYTES_V2
    );
    let cursor_rent = Rent::default().minimum_balance(width);
    assert_eq!(
        begin.sponsor_debit,
        Rent::default().minimum_balance(body.len()) + 2 * cursor_rent
    );
    let prefix = cursor_from_begin(begin_wire, raw, cursor, cursor_rent);
    let appended = prepare_append_page_v1(
        prefix,
        AccountId::new(raw.to_bytes()).expect("raw identity"),
        AccountId::new(cursor.to_bytes()).expect("cursor identity"),
        u64::try_from(body.len()).expect("body length"),
        AppendPageV1::new(0, 0, &body).expect("one artifact page"),
    )
    .expect("artifact append")
    .next_cursor();
    let mut unstarted = appended.to_bytes().to_vec();
    unstarted.resize(width, 0);
    let raw_lamports = Rent::default().minimum_balance(body.len());
    let active = |bytes: &[u8]| {
        build_record_publication_step_v1(
            REGISTRY,
            content,
            state(
                observed(raw, REGISTRY, raw_lamports, &body),
                observed(cursor, REGISTRY, 2 * cursor_rent, bytes),
                &rent_bytes,
                &clock_bytes,
            ),
        )
    };
    let first = active(&unstarted).expect("first bounded Finalize");
    assert_eq!(first.action, RecordPublicationActionV1::Finalize);
    assert_eq!(first.cursor_refund, None);
    let instruction = first.instruction.expect("Finalize instruction");
    assert_eq!(instruction.accounts.len(), 5);
    assert_eq!(instruction.accounts[3].pubkey, program);
    assert_eq!(instruction.accounts[4].pubkey, programdata);
    let progress = CodeCommitmentProgressV2::new(u64::try_from(elf.len()).expect("ELF length"))
        .expect("initial progress")
        .advance(0, &elf[..CODE_COMMITMENT_CHUNK_BYTES_V2])
        .expect("first code chunk");
    let mut partial = unstarted.clone();
    partial[STAGING_CURSOR_BYTES_V1..].copy_from_slice(&progress.to_bytes());
    let resumed = active(&partial).expect("resume authenticated native progress");
    assert_eq!(resumed.action, RecordPublicationActionV1::Finalize);
    assert_eq!(resumed.cursor_refund, None);
    assert_eq!(
        active(&partial[..STAGING_CURSOR_BYTES_V1]),
        Err(PublicationErrorV1::AccountAuthority)
    );
    let finalized = state(
        observed(raw, REGISTRY, raw_lamports, &body),
        observed(cursor, system_program::ID, 0, &[]),
        &rent_bytes,
        &clock_bytes,
    );
    assert_eq!(
        build_record_publication_step_v1(REGISTRY, content, finalized)
            .expect("absent cursor closes publication")
            .action,
        RecordPublicationActionV1::Complete
    );
}
