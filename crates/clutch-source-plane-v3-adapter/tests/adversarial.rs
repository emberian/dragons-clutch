use clutch_source_plane_v3::*;
use clutch_source_plane_v3_adapter::Error as AdapterError;
use clutch_source_plane_v3_adapter::*;
use clutch_source_profile_v1::spec_v2::{
    SourceSpecFieldsV2, SourceSpecV2, GRID_ORIGIN_UNIX_SECONDS_V1, ORIENTATION_QUOTE_PER_BASE,
};
use clutch_terminal_identity_v1::{Id, TerminalIdentityV1};
use sha2::{Digest, Sha256};

fn id(byte: u8) -> ContentId {
    ContentId::from_bytes([byte; 32])
}

fn source_plane() -> SourcePlaneProgramV3 {
    SourcePlaneProgramV3 {
        release_id: id(1),
        source_plane_version: 3,
        raw_page_codec_version: 1,
        window_codec_version: 1,
        statistic_result_codec_version: 1,
        capabilities: CAP_SOURCE_ONLY_HEAD
            | CAP_REUSABLE_RAW_PAGES
            | CAP_REALM_NEUTRAL_FEED
            | CAP_STATISTIC_RESULTS,
    }
}

fn summary() -> SummaryProgramV3 {
    SummaryProgramV3 {
        evaluator_release_id: id(3),
        evaluator_version: 1,
        feature_mask: FEATURE_TERMINAL_INTERVAL | FEATURE_DRAWDOWN_INTERVAL,
    }
}

fn v2_spec() -> SourceSpecV2 {
    SourceSpecV2::new(SourceSpecFieldsV2 {
        source_adapter_id: [0xa1; 32],
        source_adapter_version: 3,
        parser_id: 6,
        parser_version: 5,
        receiver_program: [0xb2; 32],
        receiver_programdata: [0xb3; 32],
        receiver_config: [0xc3; 32],
        config_digest: [0xd4; 32],
        provider_feed_id: [0xe5; 32],
        programdata_deployment_slot: 7,
        base_asset_id: [0x11; 32],
        quote_asset_id: [0x22; 32],
        orientation: ORIENTATION_QUOTE_PER_BASE,
        normalized_decimals: 8,
        grid_family_id: 4,
        grid_version: 9,
        grid_origin_unix_seconds: GRID_ORIGIN_UNIX_SECONDS_V1,
        bucket_seconds: 300,
        boundary_grace_seconds: 12,
        max_staleness_slots: 400,
        max_staleness_seconds: 90,
        max_future_seconds: 3,
        max_confidence_atoms: 1_000_000,
        max_confidence_bps: 200,
        confidence_multiplier: 2,
        selection_rule: 2,
    })
    .unwrap()
}

fn v2_account() -> ([u8; SOURCE_SPEC_ACCOUNT_V2_BYTES], V2SourceSpecBinding) {
    let spec = v2_spec();
    let body = spec.encode_canonical();
    let mut hasher = Sha256::new();
    hasher.update(b"dragons-clutch/feed/v2");
    hasher.update(body);
    let feed: [u8; 32] = hasher.finalize().into();
    let mut account = [0; SOURCE_SPEC_ACCOUNT_V2_BYTES];
    account[0] = 0x73;
    account[1] = 1;
    account[2..34].copy_from_slice(&feed);
    account[34..402].copy_from_slice(&body);
    account[402] = 217;
    let binding = project_v2_source_spec_fixture(
        [0x90; 32],
        [0x91; 32],
        217,
        V2AccountView {
            key: [0x91; 32],
            owner: [0x90; 32],
            executable: false,
            data: &account,
        },
    )
    .unwrap();
    (account, binding)
}

fn terminal() -> (TerminalIdentityV1, Id) {
    (
        TerminalIdentityV1 {
            payer: Id::from_bytes([0x71; 32]),
            payer_principal: 500,
            donation_floor: 9,
            generation: 1,
        },
        Id::from_bytes([0x72; 32]),
    )
}

fn account_header(family: AccountFamilyV3, principal: u64) -> AccountHeaderV3 {
    AccountHeaderV3 {
        family,
        bump: 201,
        terminal: TerminalIdentityV1 {
            payer: Id::from_bytes([0x71; 32]),
            payer_principal: principal,
            donation_floor: 9,
            generation: 1,
        },
    }
}

fn neutral_sink() -> Id {
    Id::from_bytes([0x72; 32])
}

fn archive_record(bucket: u64, value: u128, sequence: u64, slot: u64) -> V2ArchiveRecord {
    V2ArchiveRecord {
        bucket,
        low: value,
        high: value,
        sequence,
        publish_slot: slot,
        publish_time: sequence,
    }
}

#[test]
fn model_v2_fixture_projection_is_exact_and_refuses_future_versions() {
    let (account, binding) = v2_account();
    assert_eq!(binding.account_key(), [0x91; 32]);
    assert_eq!(binding.owner(), [0x90; 32]);
    assert_eq!(binding.stored_bump(), 217);
    assert_eq!(binding.spec(), v2_spec());
    assert_eq!(binding.feed_id().bytes(), account[2..34]);

    let verify = |bytes: &[u8], bump| {
        project_v2_source_spec_fixture(
            [0x90; 32],
            [0x91; 32],
            bump,
            V2AccountView {
                key: [0x91; 32],
                owner: [0x90; 32],
                executable: false,
                data: bytes,
            },
        )
    };
    let mut hostile = account;
    hostile[1] = 2;
    assert_eq!(
        verify(&hostile, 217),
        Err(V2SourceSpecRefusal::WrongVersion)
    );
    hostile = account;
    hostile[8] = 3; // schema is body offset 34 + 8, not here.
    assert_eq!(
        verify(&hostile, 217),
        Err(V2SourceSpecRefusal::DigestMismatch)
    );
    hostile = account;
    hostile[42] = 3;
    assert!(matches!(
        verify(&hostile, 217),
        Err(V2SourceSpecRefusal::Body(_))
    ));
    hostile = account;
    hostile[403] = 1;
    assert_eq!(
        verify(&hostile, 217),
        Err(V2SourceSpecRefusal::NonCanonicalPadding)
    );
    assert_eq!(verify(&account, 216), Err(V2SourceSpecRefusal::WrongBump));
}

#[test]
fn fixed_account_wrapper_has_one_core_owner_and_hostile_decode_is_exact() {
    let head = SourceHeadV3::new(id(2), 100, 4).unwrap();
    let (terminal, sink) = terminal();
    let header = AccountHeaderV3 {
        family: AccountFamilyV3::SourceHead,
        bump: 201,
        terminal,
    };
    let mut bytes = vec![0xa5; ACCOUNT_HEADER_BYTES + SOURCE_HEAD_BYTES];
    encode_account(header, &head, sink, &mut bytes).unwrap();
    assert_eq!(
        decode_account::<SourceHeadV3>(&bytes, sink).unwrap(),
        (header, head)
    );
    assert!(
        !canonical_account_state_digest::<SOURCE_HEAD_BYTES, _>(header, &head, sink)
            .unwrap()
            .is_zero()
    );

    let mut hostile = bytes.clone();
    hostile[8..10].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(
        decode_account::<SourceHeadV3>(&hostile, sink),
        Err(AdapterError::BadVersion)
    );
    hostile = bytes.clone();
    hostile[10..12].copy_from_slice(&(AccountFamilyV3::WindowWork as u16).to_le_bytes());
    assert_eq!(
        decode_account::<SourceHeadV3>(&hostile, sink),
        Err(AdapterError::WrongAccountFamily)
    );
    hostile = bytes.clone();
    hostile[15] = 1;
    assert_eq!(
        decode_account::<SourceHeadV3>(&hostile, sink),
        Err(AdapterError::NonCanonicalPadding)
    );
    assert_eq!(
        decode_account::<SourceHeadV3>(&bytes[..bytes.len() - 1], sink),
        Err(AdapterError::WrongLength)
    );

    let open_header = account_header(AccountFamilyV3::OpenRawPage, 500);
    let projected = project_open_raw_page(&source_plane(), &head, open_header, sink).unwrap();
    let mut open_image = vec![0; ACCOUNT_HEADER_BYTES + OPEN_RAW_PAGE_BYTES];
    encode_account(open_header, &projected.output, sink, &mut open_image).unwrap();
    assert_eq!(
        decode_account::<OpenRawPageV3>(&open_image, sink).unwrap(),
        (open_header, projected.output)
    );
    assert_eq!(
        projected.plan.creation(0).unwrap().state.state_digest(),
        canonical_account_state_digest::<OPEN_RAW_PAGE_BYTES, _>(
            open_header,
            &projected.output,
            sink,
        )
        .unwrap()
    );

    let mut different_bump = open_header;
    different_bump.bump = 202;
    let rebound = project_open_raw_page(&source_plane(), &head, different_bump, sink).unwrap();
    assert_ne!(
        projected.plan.creation(0).unwrap().state.state_digest(),
        rebound.plan.creation(0).unwrap().state.state_digest()
    );

    let mut reopen = open_header;
    reopen.terminal.generation = 2;
    assert_eq!(
        project_open_raw_page(&source_plane(), &head, reopen, sink),
        Err(AdapterError::ReopenGenerationUnavailable)
    );

    let observed = AccountMutationV3::observe(open_header, sink, 517, 500).unwrap();
    assert_eq!(observed.before_header().terminal.donation_floor, 9);
    assert_eq!(observed.after_header().terminal.donation_floor, 17);
    let close = AccountCloseV3::close(open_header, sink, 517).unwrap();
    assert_eq!(close.neutral_sink(), sink);
    assert_eq!(close.neutral_surplus_lamports(), 17);
    assert!(AccountCloseV3::close(open_header, sink, 508).is_err());
}

#[test]
fn pda_registry_preserves_v2_seed_and_binds_v3_pages_to_exact_release() {
    let (_, binding) = v2_account();
    let legacy = PdaRecipeV3::v2_source_spec(binding.feed_id()).unwrap();
    assert_eq!(legacy.seed_count(), 2);
    assert_eq!(legacy.seed(0).unwrap(), b"source-spec-v1");
    assert_eq!(legacy.seed(1).unwrap(), &binding.feed_id().bytes());

    let plane_a = source_plane().id().unwrap();
    let plane_b = SourcePlaneProgramV3 {
        release_id: id(90),
        ..source_plane()
    }
    .id()
    .unwrap();
    assert_ne!(
        PdaRecipeV3::source_head(plane_a, binding.feed_id(), 1)
            .unwrap()
            .id()
            .unwrap(),
        PdaRecipeV3::source_head(plane_b, binding.feed_id(), 1)
            .unwrap()
            .id()
            .unwrap()
    );
    assert_ne!(
        PdaRecipeV3::raw_page(plane_a, id(44))
            .unwrap()
            .id()
            .unwrap(),
        PdaRecipeV3::raw_page(plane_b, id(44))
            .unwrap()
            .id()
            .unwrap()
    );
    assert_ne!(
        PdaRecipeV3::statistic_result(id(31)).unwrap().id().unwrap(),
        PdaRecipeV3::statistic_result(id(32)).unwrap().id().unwrap()
    );
    assert_ne!(
        PdaRecipeV3::window_spec(id(31)).unwrap().id().unwrap(),
        PdaRecipeV3::statistic_key(id(31)).unwrap().id().unwrap()
    );
    assert_ne!(
        PdaRecipeV3::summary_program(id(31)).unwrap().id().unwrap(),
        PdaRecipeV3::statistic_key(id(31)).unwrap().id().unwrap()
    );
    assert_eq!(
        PdaRecipeV3::window_spec(ContentId::ZERO),
        Err(AdapterError::ZeroIdentity)
    );
}

#[test]
fn v2_is_transcoded_not_reinterpreted_and_stricter_v3_slot_rule_refuses() {
    let (_, binding) = v2_account();
    let head = SourceHeadV3::new(binding.feed_id(), 10, 0).unwrap();
    let mut open = head.open_page().unwrap();
    open = open
        .append_observation(
            archive_record(10, 100, 112, 1_000)
                .project_v3_candidate(10)
                .unwrap(),
        )
        .unwrap();
    let page = open.seal().unwrap();
    let head = head.commit_page(&page).unwrap();
    let open = head.open_page().unwrap();

    // V2 admits increasing sequence with decreasing posted_slot. V3 refuses.
    let slot_regression = archive_record(11, 101, 125, 999)
        .project_v3_candidate(11)
        .unwrap();
    assert_eq!(
        open.append_observation(slot_regression),
        Err(clutch_source_plane_v3::Error::DiscontinuousPage)
    );

    let too_large = archive_record(11, MAX_SOURCE_VALUE + 1, 125, 1_001);
    assert_eq!(
        too_large.project_v3_candidate(11),
        Err(AdapterError::V2ProjectionUnavailable)
    );
    assert_eq!(
        V2ArchiveRecord::decode(&[0; ARCHIVE_RECORD_V2_BYTES - 1]),
        Err(AdapterError::WrongLength)
    );
    let inverted = V2ArchiveRecord {
        low: 102,
        high: 101,
        ..archive_record(11, 101, 125, 1_001)
    };
    assert_eq!(
        inverted.project_v3_candidate(11),
        Err(AdapterError::V2ProjectionUnavailable)
    );
    let mismatched_time = V2ArchiveRecord {
        publish_time: 126,
        ..archive_record(11, 101, 125, 1_001)
    };
    assert_eq!(
        mismatched_time.project_v3_candidate(11),
        Err(AdapterError::V2ProjectionUnavailable)
    );

    let v2_bytes = archive_record(10, 100, 112, 1_000).encode();
    let mut page_bytes = vec![0; RAW_PAGE_BYTES];
    page.encode_into(&mut page_bytes).unwrap();
    assert_ne!(&v2_bytes[..8], &page_bytes[104..112]);
    assert_eq!(page_bytes[104], RawRecordKindV3::Observation as u8);
    assert!(page_bytes[105..112].iter().all(|byte| *byte == 0));
}

fn page_and_window() -> (WindowSpecV3, RawPageV3) {
    let plane = source_plane();
    let head = SourceHeadV3::new(id(2), 100, 0).unwrap();
    let mut open = head.open_page().unwrap();
    for (value, sequence) in [
        (100, 10),
        (120, 11),
        (90, 12),
        (110, 13),
        (70, 14),
        (80, 15),
    ] {
        open = open
            .append_observation(RawRecordV3::observation(
                value,
                value,
                sequence,
                sequence + 10,
                sequence + 20,
            ))
            .unwrap();
    }
    let page = open.seal().unwrap();
    let window = WindowSpecV3 {
        source_spec_id: id(2),
        source_plane_program_id: plane.id().unwrap(),
        start_bucket: 100,
        end_bucket_exclusive: 104,
        maturity_bucket_exclusive: 106,
        repair_generation: 0,
        coverage_policy_id: 1,
        coverage_policy_parameter: 0,
    };
    (window, page)
}

#[test]
fn promoted_seal_projection_binds_exact_images_and_prefund_close() {
    let plane = source_plane();
    let head_before = SourceHeadV3::new(id(2), 100, 0).unwrap();
    let open_before = head_before
        .open_page()
        .unwrap()
        .append_observation(RawRecordV3::observation(100, 101, 7, 8, 9))
        .unwrap();
    let sealed_page = open_before.seal().unwrap();
    let head_after = head_before.commit_page(&sealed_page).unwrap();
    let head_runtime = RuntimeMutationProjectionV1 {
        account_data_before_id: id(20),
        account_data_after_id: id(21),
        generation: 3,
    };
    let open_runtime = RuntimeCloseProjectionV1 {
        account_data_id: id(22),
        generation: 4,
        principal_recipient: ContentId::ZERO,
        payer_principal_lamports: 0,
        neutral_sink: id(23),
        neutral_surplus_lamports: 800,
    };
    let page_runtime = RuntimeCreationProjectionV1 {
        account_data_id: id(24),
        generation: 1,
        payer: ContentId::ZERO,
        rent_principal_lamports: 0,
    };
    let plan = project_runtime_seal_raw_page(
        &plane,
        &head_before,
        &open_before,
        &head_after,
        &sealed_page,
        head_runtime,
        open_runtime,
        page_runtime,
        id(25),
    )
    .unwrap();
    assert_eq!(plan.action(), TransitionActionV3::SealRawPage);
    assert_eq!((plan.mutation_count(), plan.creation_count(), plan.close_count()), (1, 1, 1));
    assert_eq!(plan.close(0).unwrap().principal_recipient, ContentId::ZERO);
    assert_eq!(plan.close(0).unwrap().payer_principal_lamports, 0);
    assert_eq!(plan.creation(0).unwrap().payer, ContentId::ZERO);
    assert_eq!(plan.creation(0).unwrap().rent_principal_lamports, 0);

    let rebound = project_runtime_seal_raw_page(
        &plane,
        &head_before,
        &open_before,
        &head_after,
        &sealed_page,
        RuntimeMutationProjectionV1 {
            account_data_after_id: id(26),
            ..head_runtime
        },
        open_runtime,
        page_runtime,
        id(25),
    )
    .unwrap();
    assert_ne!(plan.id().unwrap(), rebound.id().unwrap());
    let different_receipt = project_runtime_seal_raw_page(
        &plane,
        &head_before,
        &open_before,
        &head_after,
        &sealed_page,
        head_runtime,
        open_runtime,
        page_runtime,
        id(27),
    )
    .unwrap();
    assert_ne!(plan.id().unwrap(), different_receipt.id().unwrap());

    assert_eq!(
        project_runtime_seal_raw_page(
            &plane,
            &head_before,
            &open_before,
            &head_before,
            &sealed_page,
            head_runtime,
            open_runtime,
            page_runtime,
            id(25),
        ),
        Err(AdapterError::InvalidParameter)
    );
    assert_eq!(
        project_runtime_seal_raw_page(
            &plane,
            &head_before,
            &open_before,
            &head_after,
            &sealed_page,
            head_runtime,
            RuntimeCloseProjectionV1 {
                principal_recipient: id(28),
                ..open_runtime
            },
            page_runtime,
            id(25),
        ),
        Err(AdapterError::InvalidParameter)
    );
    assert_eq!(
        project_runtime_seal_raw_page(
            &plane,
            &head_before,
            &open_before,
            &head_after,
            &sealed_page,
            head_runtime,
            open_runtime,
            page_runtime,
            ContentId::ZERO,
        ),
        Err(AdapterError::InvalidParameter)
    );
}

#[test]
fn promoted_window_work_projection_requires_product_window_authentication() {
    let plane = source_plane();
    let (window, _) = page_and_window();
    let work = WindowWorkV3::new(&window).unwrap();
    let runtime = RuntimeCreationProjectionV1 {
        account_data_id: id(31),
        generation: 2,
        payer: id(32),
        rent_principal_lamports: 700,
    };
    let plan = project_runtime_initialize_window_work(&plane, &window, &work, runtime, id(33))
        .unwrap();
    assert_eq!(plan.action(), TransitionActionV3::CreateWindowWork);
    assert_eq!((plan.mutation_count(), plan.creation_count(), plan.close_count()), (0, 1, 0));
    assert_eq!(
        plan.creation(0).unwrap().state.binding_id(),
        PdaRecipeV3::window_work(window.id().unwrap())
            .unwrap()
            .id()
            .unwrap()
    );
    assert_eq!(
        project_runtime_initialize_window_work(
            &plane,
            &window,
            &work,
            runtime,
            ContentId::ZERO,
        ),
        Err(AdapterError::InvalidParameter)
    );
    assert_eq!(
        project_runtime_initialize_window_work(
            &plane,
            &window,
            &work,
            RuntimeCreationProjectionV1 {
                payer: ContentId::ZERO,
                ..runtime
            },
            id(33),
        ),
        Err(AdapterError::InvalidParameter)
    );
}

#[test]
fn promoted_window_fold_projection_binds_exact_page_and_work_cas() {
    let plane = source_plane();
    let (window, page) = page_and_window();
    let before = WindowWorkV3::new(&window).unwrap();
    let after = before.push_page(&window, &page).unwrap();
    let runtime = RuntimeMutationProjectionV1 {
        account_data_before_id: id(34),
        account_data_after_id: id(35),
        generation: 2,
    };
    let plan = project_runtime_fold_window_page(
        &plane,
        &window,
        &before,
        &page,
        &after,
        runtime,
        id(36),
    )
    .unwrap();
    assert_eq!(plan.action(), TransitionActionV3::FoldWindowPage);
    assert_eq!((plan.mutation_count(), plan.creation_count(), plan.close_count()), (1, 0, 0));

    let rebound = project_runtime_fold_window_page(
        &plane,
        &window,
        &before,
        &page,
        &after,
        RuntimeMutationProjectionV1 {
            generation: 3,
            ..runtime
        },
        id(36),
    )
    .unwrap();
    assert_ne!(plan.id().unwrap(), rebound.id().unwrap());
    assert_eq!(
        project_runtime_fold_window_page(
            &plane,
            &window,
            &before,
            &page,
            &before,
            runtime,
            id(36),
        ),
        Err(AdapterError::InvalidParameter)
    );
}

#[test]
fn promoted_window_seal_projection_binds_evidence_and_close_split() {
    let plane = source_plane();
    let (window, page) = page_and_window();
    let work = WindowWorkV3::new(&window)
        .unwrap()
        .push_page(&window, &page)
        .unwrap();
    let closure = WindowClosureReceiptV3::from_page(&plane, &window, &page).unwrap();
    let seal = work.finish(&window, &closure).unwrap();
    let close = RuntimeCloseProjectionV1 {
        account_data_id: id(37),
        generation: 2,
        principal_recipient: ContentId::ZERO,
        payer_principal_lamports: 0,
        neutral_sink: id(38),
        neutral_surplus_lamports: 900,
    };
    let creation = RuntimeCreationProjectionV1 {
        account_data_id: id(39),
        generation: 1,
        payer: id(40),
        rent_principal_lamports: 800,
    };
    let plan = project_runtime_seal_window(
        &plane,
        &window,
        &work,
        &page,
        &closure,
        &seal,
        close,
        creation,
        id(41),
    )
    .unwrap();
    assert_eq!(plan.action(), TransitionActionV3::SealWindow);
    assert_eq!((plan.mutation_count(), plan.creation_count(), plan.close_count()), (0, 1, 1));
    assert_eq!(plan.close(0).unwrap().principal_recipient, ContentId::ZERO);
    assert_eq!(
        project_runtime_seal_window(
            &plane,
            &window,
            &work,
            &page,
            &closure,
            &seal,
            RuntimeCloseProjectionV1 {
                neutral_sink: ContentId::ZERO,
                ..close
            },
            creation,
            id(41),
        ),
        Err(AdapterError::InvalidParameter)
    );
}

#[test]
fn promoted_evaluation_projection_binds_result_semantics_and_exact_postimage() {
    let plane = source_plane();
    let summary = summary();
    let (window, page) = page_and_window();
    let work = WindowWorkV3::new(&window)
        .unwrap()
        .push_page(&window, &page)
        .unwrap();
    let closure = WindowClosureReceiptV3::from_page(&plane, &window, &page).unwrap();
    let seal = work.finish(&window, &closure).unwrap();
    let key = StatisticKeyV3 {
        window_id: window.id().unwrap(),
        summary_program_id: summary.id().unwrap(),
        statistic: StatisticKindV3::TerminalInterval,
    };
    let result = StatisticResultV3::terminal(&key, &summary, &seal, &window, 70, 120).unwrap();
    let runtime = RuntimeCreationProjectionV1 {
        account_data_id: id(42),
        generation: 1,
        payer: id(43),
        rent_principal_lamports: 900,
    };
    let plan = project_runtime_evaluate_statistic(
        &plane,
        &window,
        &key,
        &summary,
        &seal,
        &result,
        runtime,
        id(44),
    )
    .unwrap();
    assert_eq!(plan.action(), TransitionActionV3::WriteTerminalResult);
    assert_eq!((plan.mutation_count(), plan.creation_count(), plan.close_count()), (0, 1, 0));
    assert_eq!(
        plan.creation(0).unwrap().state.binding_id(),
        PdaRecipeV3::statistic_result(key.id().unwrap())
            .unwrap()
            .id()
            .unwrap()
    );
    assert_eq!(
        project_runtime_evaluate_statistic(
            &plane,
            &window,
            &key,
            &summary,
            &seal,
            &result,
            runtime,
            ContentId::ZERO,
        ),
        Err(AdapterError::InvalidParameter)
    );
    assert_eq!(
        project_runtime_evaluate_statistic(
            &plane,
            &window,
            &key,
            &summary,
            &seal,
            &result,
            RuntimeCreationProjectionV1 {
                payer: ContentId::ZERO,
                ..runtime
            },
            id(44),
        ),
        Err(AdapterError::InvalidParameter)
    );
}

#[test]
fn window_core_requires_v3_maturity_while_adapter_reads_remain_opaque() {
    let plane = source_plane();
    let (window, page) = page_and_window();

    // Current V2 considers its end cursor sufficient after per-boundary time
    // grace. The corresponding four-record page cannot satisfy V3's explicit
    // end+2 maturity page requirement.
    let head = SourceHeadV3::new(id(2), 100, 0).unwrap();
    let mut short_open = head.open_page().unwrap();
    for sequence in 10..14 {
        short_open = short_open
            .append_observation(RawRecordV3::observation(
                100,
                100,
                sequence,
                sequence + 10,
                sequence + 20,
            ))
            .unwrap();
    }
    let v2_end_page = short_open.seal().unwrap();
    assert_eq!(v2_end_page.end_bucket_exclusive().unwrap(), 104);
    assert!(WindowClosureReceiptV3::from_page(&plane, &window, &v2_end_page).is_err());
    let work = WindowWorkV3::new(&window)
        .unwrap()
        .push_page(&window, &page)
        .unwrap();
    let closure = WindowClosureReceiptV3::from_page(&plane, &window, &page).unwrap();
    work.finish(&window, &closure).unwrap();

    let premature = WindowSpecV3 {
        maturity_bucket_exclusive: 107,
        ..window
    };
    let work = WindowWorkV3::new(&premature)
        .unwrap()
        .push_page(&premature, &page)
        .unwrap();
    assert!(WindowClosureReceiptV3::from_page(&plane, &premature, &page).is_err());
    let _ = work;
}

fn payouts() -> PayoutTableV3 {
    let mut vectors = [PayoutVectorV3::ZERO; MAX_PAYOUTS];
    for outcome in 0..4 {
        let mut weights = [0; MAX_OUTCOMES];
        weights[outcome] = 1_000;
        vectors[outcome] = PayoutVectorV3 {
            denominator: 1_000,
            weights,
        };
    }
    let mut uniform = [0; MAX_OUTCOMES];
    uniform[..4].fill(250);
    vectors[4] = PayoutVectorV3 {
        denominator: 1_000,
        weights: uniform,
    };
    PayoutTableV3 {
        outcome_count: 4,
        payout_count: 5,
        failure_payout_index: 4,
        payouts: vectors,
    }
}

fn template() -> ProductTemplateV3 {
    ProductTemplateV3 {
        source_plane_program_id: source_plane().id().unwrap(),
        source_spec_id: id(2),
        summary_program_id: summary().id().unwrap(),
        partition_id: id(4),
        payout_table_id: payouts().id().unwrap(),
        settlement_policy_id: id(5),
        compiler_version: 1,
        statistic: StatisticKindV3::TerminalInterval,
        coverage_policy_id: 1,
        failure_policy_id: FAILURE_UNIFORM_REFUND_01,
        repair_policy_id: 1,
        window_span_buckets: 4,
        maturity_grace_buckets: 2,
        repair_generation: 0,
        coverage_policy_parameter: 0,
    }
}

fn work() -> WorkEnvelopeV3 {
    WorkEnvelopeV3 {
        version: 1,
        creation_lamports: 10,
        liveness_lamports: 20,
    }
}

fn liquidity() -> LiquidityEnvelopeV3 {
    LiquidityEnvelopeV3 {
        liquidity_policy_id: id(6),
        version: 1,
        collateral_per_instance: 100,
    }
}

fn series(first: u64) -> SeriesPlanV3 {
    SeriesPlanV3 {
        template_id: template().id().unwrap(),
        realm_id: id(7),
        profile_id: id(8),
        price_grid_id: id(9),
        fee_policy_id: id(10),
        work_envelope_id: work().id().unwrap(),
        liquidity_envelope_id: liquidity().id().unwrap(),
        first_start_bucket: first,
        stride_buckets: 10,
        instance_count: 3,
        creation_lead_buckets: 5,
        market_collateral_cap: 200,
    }
}

#[allow(clippy::too_many_arguments)]
fn bindings<'a>(
    plane: &'a SourcePlaneProgramV3,
    summary_program: &'a SummaryProgramV3,
    payout_table: &'a PayoutTableV3,
    partition: &'a PartitionViewV3,
    product: &'a ProductTemplateV3,
    work_quote: &'a WorkEnvelopeV3,
    liquidity_quote: &'a LiquidityEnvelopeV3,
    schedule: &'a SeriesPlanV3,
) -> SeriesBindingsV3<'a> {
    SeriesBindingsV3 {
        source_plane: plane,
        summary: summary_program,
        payouts: payout_table,
        partition,
        template: product,
        work: work_quote,
        liquidity: liquidity_quote,
        series: schedule,
    }
}

#[test]
fn recurring_lapse_projection_is_permissionless_and_preserves_compartments() {
    let plane = source_plane();
    let summary_program = summary();
    let payout_table = payouts();
    let partition = PartitionViewV3 {
        partition_id: id(4),
        outcome_count: 4,
    };
    let product = template();
    let work_quote = work();
    let liquidity_quote = liquidity();
    let schedule = series(100);
    let bound = bindings(
        &plane,
        &summary_program,
        &payout_table,
        &partition,
        &product,
        &work_quote,
        &liquidity_quote,
        &schedule,
    );
    let sink = neutral_sink();
    let funding_header = account_header(AccountFamilyV3::SeriesFunding, 500);
    let funding = SeriesFundingV3::activate(
        &schedule,
        &product,
        &work_quote,
        &liquidity_quote,
        30,
        60,
        300,
    )
    .unwrap();
    let mutation = AccountMutationV3::observe(funding_header, sink, 599, 590).unwrap();
    let lapsed = project_lapse_next_instance(bound, funding, 100, mutation).unwrap();
    assert_eq!(lapsed.output.next_ordinal(), 1);
    assert_eq!(lapsed.output.creation_lamports(), 30);
    assert_eq!(lapsed.output.liveness_lamports(), 60);
    assert_eq!(lapsed.output.liquidity_collateral(), 300);
    assert_eq!(
        project_refund_series_funding(bound, lapsed.output),
        Err(AdapterError::SeriesTerminalRefundUnavailable)
    );

    let wrong_accounted = AccountMutationV3::observe(funding_header, sink, 598, 589).unwrap();
    assert_eq!(
        project_lapse_next_instance(bound, funding, 100, wrong_accounted),
        Err(AdapterError::MismatchedState)
    );

    let intent = IntentPreimageV3::new(lapsed.plan, id(69), id(0x99), 50_000).unwrap();
    let encoded = intent.encode().unwrap();
    assert_eq!(IntentPreimageV3::decode(&encoded).unwrap(), intent);
    IntentPreimageV3::decode(&encoded)
        .unwrap()
        .validate_for_program(id(69), lapsed.plan)
        .unwrap();
    let mut hostile = encoded;
    hostile[10..12]
        .copy_from_slice(&(TransitionActionV3::CreateSeriesInstance as u16).to_le_bytes());
    assert_eq!(
        IntentPreimageV3::decode(&hostile)
            .unwrap()
            .validate_for_program(id(69), lapsed.plan),
        Err(AdapterError::MismatchedState)
    );
    hostile = encoded;
    hostile[108..116].copy_from_slice(&96_u64.to_le_bytes());
    assert_eq!(
        IntentPreimageV3::decode(&hostile)
            .unwrap()
            .validate_for_program(id(69), lapsed.plan),
        Err(AdapterError::MismatchedState)
    );
    hostile = encoded;
    hostile[8..10].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(
        IntentPreimageV3::decode(&hostile),
        Err(AdapterError::BadVersion)
    );
    hostile = encoded;
    hostile[159] = 1;
    assert_eq!(
        IntentPreimageV3::decode(&hostile),
        Err(AdapterError::NonCanonicalPadding)
    );
    assert_eq!(
        intent.validate_for_program(id(68), lapsed.plan),
        Err(AdapterError::MismatchedState)
    );
}

#[test]
fn frozen_adapter_vectors_match() {
    fn hex(bytes: &[u8]) -> String {
        let mut output = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            use core::fmt::Write;
            write!(&mut output, "{byte:02x}").unwrap();
        }
        output
    }
    let vectors: serde_json::Value =
        serde_json::from_str(include_str!("../vectors/source-plane-v3-adapter.json")).unwrap();
    let (_, binding) = v2_account();
    let plane = source_plane();
    let summary_program = summary();
    let payout_table = payouts();
    let partition = PartitionViewV3 {
        partition_id: id(4),
        outcome_count: 4,
    };
    let product = template();
    let work_quote = work();
    let liquidity_quote = liquidity();
    let schedule = series(100);
    let bound = bindings(
        &plane,
        &summary_program,
        &payout_table,
        &partition,
        &product,
        &work_quote,
        &liquidity_quote,
        &schedule,
    );
    let funding = SeriesFundingV3::activate(
        &schedule,
        &product,
        &work_quote,
        &liquidity_quote,
        30,
        60,
        300,
    )
    .unwrap();
    let funding_header = account_header(AccountFamilyV3::SeriesFunding, 500);
    let mutation = AccountMutationV3::observe(funding_header, neutral_sink(), 599, 590).unwrap();
    let lapse = project_lapse_next_instance(bound, funding, 100, mutation).unwrap();
    let intent = IntentPreimageV3::new(lapse.plan, id(69), id(0x99), 50_000).unwrap();
    assert_eq!(vectors["v2_feed_id"], hex(&binding.feed_id().bytes()));
    assert_eq!(
        vectors["source_head_pda_recipe_id"],
        hex(
            &PdaRecipeV3::source_head(source_plane().id().unwrap(), binding.feed_id(), 0)
                .unwrap()
                .id()
                .unwrap()
                .bytes()
        )
    );
    assert_eq!(
        vectors["series_lapse_transition_id"],
        hex(&lapse.plan.id().unwrap().bytes())
    );
    assert_eq!(
        vectors["series_lapse_intent_id"],
        hex(&intent.id().unwrap().bytes())
    );
    assert_eq!(
        vectors["series_lapse_intent_hex"],
        hex(&intent.encode().unwrap())
    );
}
