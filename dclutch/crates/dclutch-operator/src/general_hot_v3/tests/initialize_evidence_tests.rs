//! InitializeSettlement consumes the exact accounts its preceding actions write.
use super::*;

fn observed_initialize_state() -> (GeneralHotStateV3, GeneralConfigV3, [u8; 32]) {
    let fixture = verify_request_fixture();
    let mut submission = fixture.submission;
    let cursor_len = candidate_verifier_len_v1(submission).expect("cursor width");
    let certificate_len = candidate_certificate_len_v1(submission).expect("certificate width");
    let mut cursor = Vec::new();
    let mut certificate = vec![0; certificate_len];
    for row in 0..2_u32 {
        let view = CandidateVerifyRowViewV1 {
            batch: fixture.batch,
            submission,
            candidate: &fixture.candidate,
            page: &fixture.page,
            order: &fixture.orders[usize::try_from(row).expect("row")],
            cursor_before: &cursor,
            verified_before: &[],
            expected_page_index: 0,
            expected_row_index: row,
            expected_revision: u64::from(row),
        };
        let count = candidate_verify_manifest_orders_v1(&view).expect("manifest count");
        let manifest_len = settlement_manifest_len_v2(3, count).expect("manifest width");
        let mut next = vec![0; cursor_len];
        let result = verify_candidate_row_v1(
            view,
            CandidateVerifyRowBuffersV1 {
                cursor_scratch: &mut vec![0; cursor_len],
                cursor_output: &mut next,
                verified_scratch: &mut vec![0; certificate_len],
                verified_output: &mut certificate,
                manifest_scratch: &mut vec![0; manifest_len],
                manifest_output: &mut vec![0; manifest_len],
            },
        )
        .expect("actual protocol row output");
        submission = result.submission;
        cursor = next;
    }
    let config = submit_request_config();
    let policy = SelectionPolicyV1 {
        policy_id: config.selection_policy_id(),
        ..selection_policy()
    };
    let open = open_selection(3, policy, &certificate);
    let open_local = GeneralLocalStateV3::decode(&open).expect("selection envelope");
    let mut frozen = vec![0; RUNTIME_SELECTION_CURSOR_BYTES_V2];
    freeze_selection_v2(
        open_local.body(),
        1,
        &mut vec![0; RUNTIME_SELECTION_CURSOR_BYTES_V2],
        &mut frozen,
    )
    .expect("actual Freeze output");
    let trading = key(0xe1);
    let observed = |data| GeneralObservedAccountMetaV3 {
        account: ObservedAccount {
            observation: observation(),
            key: key(0xe2),
            owner: trading,
            lamports: 1_000_000,
            executable: false,
            data,
        },
        is_signer: false,
        is_writable: false,
    };
    let mut fixed_accounts = vec![observed(Vec::new()); HOT_FIXED_ACCOUNT_COUNT_V3];
    fixed_accounts[HOT_TRADING_PROGRAM_ACCOUNT_V3].account.key = trading;
    fixed_accounts[HOT_TRADING_PROGRAM_ACCOUNT_V3]
        .account
        .executable = true;
    let mut suffix = vec![observed(Vec::new()); 16];
    for (kind, bytes) in [
        (
            GeneralReadonlyEvidenceKindV3::FrozenSelection,
            local_state(GeneralLocalStateKindV3::Selection, &frozen, 3),
        ),
        (
            GeneralReadonlyEvidenceKindV3::RuntimeVerifier,
            local_state(GeneralLocalStateKindV3::Verifier, &cursor, 3),
        ),
        (
            GeneralReadonlyEvidenceKindV3::SelectedVerifiedCandidate,
            certificate,
        ),
    ] {
        let coordinate = initialize_coordinate(kind);
        suffix[coordinate] = observed(bytes);
    }
    (
        GeneralHotStateV3 {
            fixed_accounts,
            strategy_accounts: Vec::new(),
            runtime_suffix_accounts: suffix,
            release_set: [1; 32],
            generation: 7,
            minimum_finalized_slot: observation().slot,
            checked_release: None,
        },
        config,
        fixture.batch.opening().product_id,
    )
}

fn initialize_coordinate(kind: GeneralReadonlyEvidenceKindV3) -> usize {
    (0..general_readonly_evidence_count_v3(Action::InitializeSettlement))
        .map(|index| {
            general_readonly_evidence_v3(Action::InitializeSettlement, index).expect("evidence")
        })
        .find(|evidence| evidence.kind == kind)
        .map(|evidence| usize::from(evidence.coordinate) - HOT_RUNTIME_LOGICAL_PREFIX_V3)
        .expect("Initialize evidence kind")
}

#[test]
fn initialize_accepts_observed_selection_and_verifier_envelopes() {
    let (state, config, product) = observed_initialize_state();
    let request = derive_initialize_request_v5(&state, config, 3, product)
        .expect("Initialize must consume the actual enveloped Freeze/Verify poststates");
    assert_eq!(request.action, Action::InitializeSettlement);
    assert_eq!(request.expected_revision, 0);
    let writable_verifier = general_account_profile_rule_v3(
        Action::VerifyCandidateRow,
        dclutch_trading::general::state_artifacts_v3::GENERAL_VERIFY_VERIFIER_STATE_ACCOUNT_V3,
        packet_neutral_widths(),
    )
    .expect("writable verifier profile");
    assert_eq!(
        u64::from(writable_verifier.rule.data_length)
            + 3 * u64::from(writable_verifier.rule.data_item_stride),
        u64::try_from(
            state.runtime_suffix_accounts
                [initialize_coordinate(GeneralReadonlyEvidenceKindV3::RuntimeVerifier)]
            .account
            .data
            .len(),
        )
        .expect("observed verifier width"),
        "Verify authenticates the same complete account later read by Initialize"
    );
    for kind in [
        GeneralReadonlyEvidenceKindV3::FrozenSelection,
        GeneralReadonlyEvidenceKindV3::RuntimeVerifier,
    ] {
        let index = initialize_coordinate(kind);
        let coordinate = u16::try_from(index + HOT_RUNTIME_LOGICAL_PREFIX_V3).expect("coordinate");
        let rule = general_account_profile_rule_v3(
            Action::InitializeSettlement,
            coordinate,
            packet_neutral_widths(),
        )
        .expect("published evidence profile");
        assert_eq!(
            u64::from(rule.rule.data_length) + 3 * u64::from(rule.rule.data_item_stride),
            u64::try_from(state.runtime_suffix_accounts[index].account.data.len())
                .expect("account width"),
            "the profile authenticates the complete {kind:?} account before body projection"
        );
    }
}

#[test]
fn initialize_refuses_substituted_readonly_lifecycle_accounts_by_exact_cause() {
    use dclutch_trading::general::local_state_v3::{
        GENERAL_LOCAL_STATE_HEADER_BYTES_V3, GENERAL_LOCAL_STATE_VERSION_V3,
        GeneralLocalStateErrorV3 as LocalError,
    };
    let (state, config, product) = observed_initialize_state();
    for kind in [
        GeneralReadonlyEvidenceKindV3::FrozenSelection,
        GeneralReadonlyEvidenceKindV3::RuntimeVerifier,
    ] {
        let index = initialize_coordinate(kind);
        let assert_refused = |changed: GeneralHotStateV3, cause| {
            assert_eq!(
                derive_initialize_request_v5(&changed, config, 3, product),
                Err(GeneralHotOperatorErrorV3::GeneralLocalState(cause)),
                "{kind:?}"
            );
        };
        let mut changed = state.clone();
        changed.runtime_suffix_accounts[index].account.owner = key(0xff);
        assert_refused(changed, LocalError::InvalidOwner);

        let mut changed = state.clone();
        changed.runtime_suffix_accounts[index].account.data[8..10]
            .copy_from_slice(&(GENERAL_LOCAL_STATE_VERSION_V3 + 1).to_le_bytes());
        assert_refused(changed, LocalError::InvalidEncoding);

        let mut changed = state.clone();
        changed.runtime_suffix_accounts[index].account.data[12] = 1;
        assert_refused(changed, LocalError::InvalidEncoding);

        let mut changed = state.clone();
        changed.runtime_suffix_accounts[index].account.data[10] = u8::MAX;
        assert_refused(changed, LocalError::InvalidKind);

        let mut changed = state.clone();
        changed.runtime_suffix_accounts[index].account.data[16..24].fill(0);
        assert_refused(changed, LocalError::InvalidLifecycle);

        let mut changed = state.clone();
        changed.runtime_suffix_accounts[index].account.data.pop();
        assert_refused(changed, LocalError::InvalidLength);

        let mut changed = state.clone();
        changed.runtime_suffix_accounts[index].account.data.push(0);
        assert_refused(changed, LocalError::InvalidLength);

        // The superseded raw-body publication is refused, even with the
        // correct owner. The account itself is the evidence, not a copy of it.
        let mut changed = state.clone();
        changed.runtime_suffix_accounts[index].account.data =
            changed.runtime_suffix_accounts[index].account.data
                [GENERAL_LOCAL_STATE_HEADER_BYTES_V3..]
                .to_vec();
        assert_refused(changed, LocalError::InvalidLength);

        let mut changed = state.clone();
        changed.runtime_suffix_accounts[index].account.data[GENERAL_LOCAL_STATE_HEADER_BYTES_V3] ^=
            1;
        assert_refused(changed, LocalError::InvalidBody);

        let mut changed = state.clone();
        changed.runtime_suffix_accounts[index].is_writable = true;
        assert_eq!(
            derive_initialize_request_v5(&changed, config, 3, product),
            Err(GeneralHotOperatorErrorV3::ChainState)
        );
    }
    assert_eq!(
        derive_initialize_request_v5(&state, config, 3, product)
            .expect("unsubstituted control")
            .action,
        Action::InitializeSettlement
    );
}
