use alloc::boxed::Box;

use dclutch_market::execution_strategy::v2::AcceleratorRequestV2;

use super::*;
use dclutch_market::capability_program::hot_v3::SEALED_EXECUTION_FIXED_ALIASES_V3;

use dclutch_vm::account_profile::lifecycle_v3::{
    AuthenticateStatePlanV3, CloseStatePlanV3, CreateStatePlanV3,
};
use dclutch_vm::account_profile::v2::{
    AUTHENTICATED_ROUTE_ALIAS_HEADER_BYTES, AccountPrestateV2, DYNAMIC_FIXED_SPAN_HEADER_BYTES,
    HEADER_BYTES as ACCOUNT_PROFILE_HEADER_BYTES,
    OPERATION_BYTES as ACCOUNT_PROFILE_OPERATION_BYTES, RULE_BYTES as ACCOUNT_PROFILE_RULE_BYTES,
    TrustedBuiltinIdentityV2, TrustedEnvironmentV2, TrustedIdentityEnvironmentV2,
    encode::{
        AccountAliasInputV2, AccountCoordinateV2, AccountEffectPermissionsV2,
        AccountOperationInputV2, AccountPrivilegesV2, AccountProfileArtifactV2, AccountRuleInputV2,
        AccountRuleWithPrestateInputV2, DynamicFixedSpanInputV2, RegisterGeometryV2,
        ScalarCoordinateV2, encode_account_profile_v2_atomic,
        encode_account_profile_with_authenticated_route_alias_v2_atomic,
        encode_account_profile_with_dynamic_fixed_span_v2_atomic,
    },
};
use dclutch_vm::v3::{
    HEADER_BYTES as TRANSITION_HEADER_BYTES_V3,
    INSTRUCTION_BYTES as TRANSITION_INSTRUCTION_BYTES_V3, InstructionV3, ProgramGeometryV3,
    ScalarRegisterV3, encode_program_atomic,
};

use dclutch_market::{
    Identity, MarketIdentity, Phase as CorePhase, Readiness as CoreReadiness, StateBumpsV1,
};

/// A Market state carrying exactly the recorded bumps the test wants.
fn market_state_with_bumps(bumps: StateBumpsV1) -> CoreState {
    CoreState {
        phase: CorePhase::Open,
        readiness: CoreReadiness::Consumed,
        terminal_winner: 0,
        identity: MarketIdentity {
            market_id: Identity::new([1; 32]).expect("market"),
            realm_id: Identity::new([2; 32]).expect("realm"),
            product_record: Identity::new([3; 32]).expect("product record"),
            product_id: Identity::new([4; 32]).expect("product"),
            resolution_policy: Identity::new([5; 32]).expect("policy"),
            capability_manifest: Identity::new([6; 32]).expect("manifest"),
            selected_release_set: Identity::new([7; 32]).expect("release set"),
            registry_program: Identity::new([8; 32]).expect("registry"),
            generation: 7,
        },
        outstanding_capabilities: 1,
        principal_cap_sets: 100,
        rent_beneficiary: Identity::new([9; 32]).expect("beneficiary"),
        terminal_receipt: None,
        bumps,
    }
}

#[test]
fn ordinary_logical_market_is_byte_exact_persisted_authority() {
    let live = market_state_with_bumps(StateBumpsV1::UNRECORDED);
    let logical = AuthenticatedLogicalMarketV3::from_live(&live);
    assert_eq!(logical.identity, live.identity);
    assert_eq!(logical.rent_beneficiary, live.rent_beneficiary);
}

/// The Market address is reproduced from whichever carrier holds the bump,
/// and a wrong byte in either one names a DIFFERENT address.
///
/// This is the whole safety argument for accepting a caller-mined byte: the
/// caller of `market_core_state_address_v2` compares what comes back
/// against the account it was handed, so a hint that is not this Market's
/// canonical bump can only produce an address that comparison refuses.
#[test]
fn a_wrong_market_bump_hint_reproduces_another_address_and_refuses() {
    let core_program = Pubkey::new_from_array([0x33; 32]);
    let unrecorded = market_state_with_bumps(StateBumpsV1::UNRECORDED);
    let seeds = MarketCoreStateSeedsV2::new(unrecorded.identity);
    let (canonical_address, canonical) =
        Pubkey::find_program_address(&seeds.as_slices(), &core_program);

    // No carrier at all: the search this always did, unchanged.
    assert_eq!(
        market_core_state_address_v2(unrecorded, &core_program, 0).expect("searched"),
        canonical_address,
    );
    // The caller mined it: reproduced, and no search was run.
    assert_eq!(
        market_core_state_address_v2(unrecorded, &core_program, canonical).expect("hinted"),
        canonical_address,
    );
    // Every other hint yields something that is not this Market, either by
    // refusing to be a program address at all or by being another address.
    let mut refused = 0_u32;
    for hint in 1..=u8::MAX {
        if hint == canonical {
            continue;
        }
        match market_core_state_address_v2(unrecorded, &core_program, hint) {
            Ok(address) => assert_ne!(address, canonical_address, "hint {hint}"),
            Err(_) => {}
        }
        refused = refused.saturating_add(1);
    }
    assert_eq!(refused, 254);

    // A state that RECORDED its bump ignores the hint entirely: the
    // creator's own assertion outranks a byte off the wire, and a caller
    // cannot steer a Market that already knows its own address.
    let recorded = market_state_with_bumps(StateBumpsV1 {
        market: Some(canonical),
        ..StateBumpsV1::UNRECORDED
    });
    for hint in [0, canonical, canonical.wrapping_sub(1), u8::MAX] {
        assert_eq!(
            market_core_state_address_v2(recorded, &core_program, hint).expect("recorded"),
            canonical_address,
            "hint {hint} steered a recorded Market",
        );
    }
}

fn readonly_info(key: Pubkey) -> AccountInfo<'static> {
    AccountInfo::new(
        Box::leak(Box::new(key)),
        false,
        false,
        Box::leak(Box::new(0_u64)),
        Box::leak(Vec::new().into_boxed_slice()),
        Box::leak(Box::new(Pubkey::new_unique())),
        false,
    )
}

#[test]
fn accelerator_caller_token_binds_request_context_and_immutable_deployment() {
    let trading_program = Pubkey::new_unique();
    let root = Pubkey::new_unique();
    let market = Pubkey::new_unique();
    let accelerator_program = Pubkey::new_unique();
    let accelerator_programdata = Pubkey::new_unique();
    let loader_program = Pubkey::new_unique();
    let release_set = [0x31; 32];
    let request = b"complete admitted accelerator request";
    let envelope = HotExecutionEnvelopeV3::new(
        u32::try_from(request.len()).expect("request width"),
        release_set,
        market.to_bytes(),
        7,
        [0x32; 32],
    )
    .expect("envelope");
    let family_request = b"the exact DCLTHOT3 family request the caller signed";
    let parent_request_digest =
        family_request_digest_v3(family_request).expect("family request digest");
    let seeds_for = |parent: ContentId, chunk: u32, market: Pubkey, release: [u8; 32]| {
        CallerAuthoritySeedsV1::new(
            ContentId::new(release).expect("release set"),
            market.to_bytes(),
            ExecutionRoleV1::Trading,
            root.to_bytes(),
            accelerator_caller_authority_digest_v1(
                AcceleratorCallerKindV1::Admitted,
                parent,
                chunk,
            )
            .expect("role request digest")
            .to_bytes(),
        )
        .expect("caller seeds")
    };
    let caller_seeds = seeds_for(parent_request_digest, 0, market, release_set);
    let caller_key = Pubkey::find_program_address(&caller_seeds.as_slices(), &trading_program).0;
    let mut caller = readonly_info(caller_key);
    caller.is_signer = true;
    let program = readonly_info(accelerator_program);
    let programdata = readonly_info(accelerator_programdata);
    let deployment_slot = 19_u64;
    let mut metadata_bytes = vec![0_u8; 45];
    metadata_bytes
        .get_mut(..4)
        .expect("variant")
        .copy_from_slice(&3_u32.to_le_bytes());
    metadata_bytes
        .get_mut(4..12)
        .expect("slot")
        .copy_from_slice(&deployment_slot.to_le_bytes());
    let metadata = ProgramDataMetadataV3View::parse(&metadata_bytes).expect("metadata");
    let release = ArtifactReleaseV1::new(
        dclutch_registry::release_set::ProgramIdentityV1::new(accelerator_program.to_bytes())
            .expect("program"),
        dclutch_registry::release_set::ProgramIdentityV1::new(loader_program.to_bytes())
            .expect("loader"),
        accelerator_programdata.to_bytes(),
        ContentId::new([0x33; 32]).expect("semantic release"),
        [0x34; 32],
        deployment_slot,
        ArtifactUpgradePolicyV1::Immutable,
        None,
    )
    .expect("artifact release");
    let artifact_release_digest = hash(&release.to_bytes()).to_bytes();
    let artifact_release =
        dclutch_registry::release_set::ArtifactReleaseIdV1::decode(&artifact_release_digest)
            .expect("artifact release identity");
    let token = authenticate_accelerator_caller_authority_v4(
        &trading_program,
        &caller,
        envelope,
        &root,
        parent_request_digest,
        0,
        artifact_release_digest,
        &accelerator_program,
        &program,
        &programdata,
        metadata,
    )
    .expect("caller token");
    assert!(token.binds_context_parts(release_set, market.to_bytes(), root.to_bytes()));
    assert!(token.binds_immutable_deployment(artifact_release, release, &program, &programdata,));

    let mut nonsigner = caller.clone();
    nonsigner.is_signer = false;
    assert_eq!(
        authenticate_accelerator_caller_authority_v4(
            &trading_program,
            &nonsigner,
            envelope,
            &root,
            parent_request_digest,
            0,
            artifact_release_digest,
            &accelerator_program,
            &program,
            &programdata,
            metadata,
        ),
        Err(TradingSbfError::Release.into())
    );
    // A DIFFERENT SIGNED FAMILY REQUEST, which is the binding that
    // survived the seed change and the only content binding left.
    assert_eq!(
        authenticate_accelerator_caller_authority_v4(
            &trading_program,
            &caller,
            envelope,
            &root,
            family_request_digest_v3(b"a different family request").expect("other family"),
            0,
            artifact_release_digest,
            &accelerator_program,
            &program,
            &programdata,
            metadata,
        ),
        Err(TradingSbfError::Release.into())
    );
    // ANOTHER CHUNK OF THE SAME EXECUTION. The ordinal is what separates
    // the span, and an authority admitted for chunk 0 must not sign for 1.
    assert_eq!(
        authenticate_accelerator_caller_authority_v4(
            &trading_program,
            &caller,
            envelope,
            &root,
            parent_request_digest,
            1,
            artifact_release_digest,
            &accelerator_program,
            &program,
            &programdata,
            metadata,
        ),
        Err(TradingSbfError::Release.into())
    );
    assert_eq!(
        authenticate_accelerator_caller_authority_v4(
            &trading_program,
            &caller,
            envelope,
            &root,
            parent_request_digest,
            0,
            artifact_release_digest,
            &Pubkey::new_unique(),
            &program,
            &programdata,
            metadata,
        ),
        Err(TradingSbfError::Release.into())
    );
    // THE OLD SLOT-BOUND DERIVATION REFUSES BY NAME. `sha256(accelerator
    // request)` was the seed until 2026-09-03, and a producer that still
    // computes it -- or a stale off-chain builder -- states an account this
    // conjunct must reject rather than silently admit. `request` here is
    // that request's bytes; the address it yields is not this one.
    let stale_seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(release_set).expect("release set"),
        market.to_bytes(),
        ExecutionRoleV1::Trading,
        root.to_bytes(),
        hash(request).to_bytes(),
    )
    .expect("stale seeds");
    let mut stale =
        readonly_info(Pubkey::find_program_address(&stale_seeds.as_slices(), &trading_program).0);
    stale.is_signer = true;
    assert_ne!(stale.key, caller.key);
    assert_eq!(
        authenticate_accelerator_caller_authority_v4(
            &trading_program,
            &stale,
            envelope,
            &root,
            parent_request_digest,
            0,
            artifact_release_digest,
            &accelerator_program,
            &program,
            &programdata,
            metadata,
        ),
        Err(TradingSbfError::Release.into())
    );
    // ANOTHER MARKET AND ANOTHER RELEASE SET, each alone.
    for hostile_seeds in [
        seeds_for(parent_request_digest, 0, Pubkey::new_unique(), release_set),
        seeds_for(parent_request_digest, 0, market, [0x71; 32]),
    ] {
        let mut hostile = readonly_info(
            Pubkey::find_program_address(&hostile_seeds.as_slices(), &trading_program).0,
        );
        hostile.is_signer = true;
        assert_ne!(hostile.key, caller.key);
        assert_eq!(
            authenticate_accelerator_caller_authority_v4(
                &trading_program,
                &hostile,
                envelope,
                &root,
                parent_request_digest,
                0,
                artifact_release_digest,
                &accelerator_program,
                &program,
                &programdata,
                metadata,
            ),
            Err(TradingSbfError::Release.into())
        );
    }

    for hostile in [
        AuthenticatedAcceleratorCallerV4 {
            artifact_release: [0x51; 32],
            ..token
        },
        AuthenticatedAcceleratorCallerV4 {
            accelerator_program: Pubkey::new_unique().to_bytes(),
            ..token
        },
        AuthenticatedAcceleratorCallerV4 {
            accelerator_programdata: Pubkey::new_unique().to_bytes(),
            ..token
        },
        AuthenticatedAcceleratorCallerV4 {
            deployment_slot: deployment_slot.saturating_add(1),
            ..token
        },
        AuthenticatedAcceleratorCallerV4 {
            upgrade_authority: Some(Pubkey::new_unique().to_bytes()),
            ..token
        },
    ] {
        assert!(!hostile.binds_immutable_deployment(
            artifact_release,
            release,
            &program,
            &programdata,
        ));
    }
    for hostile in [
        AuthenticatedAcceleratorCallerV4 {
            release_set: [0x41; 32],
            ..token
        },
        AuthenticatedAcceleratorCallerV4 {
            market: Pubkey::new_unique().to_bytes(),
            ..token
        },
        AuthenticatedAcceleratorCallerV4 {
            root: Pubkey::new_unique().to_bytes(),
            ..token
        },
        AuthenticatedAcceleratorCallerV4 {
            role_request_digest: [0; 32],
            ..token
        },
    ] {
        assert!(!hostile.binds_context_parts(release_set, market.to_bytes(), root.to_bytes()));
    }
}

fn distinct_fixed_infos() -> Vec<AccountInfo<'static>> {
    (0..HOT_FIXED_ACCOUNT_COUNT_V3)
        .map(|_| readonly_info(Pubkey::new_unique()))
        .collect()
}

fn alias_fixed_slot(accounts: &mut [AccountInfo<'static>], raw: usize, staging: usize) {
    let raw = accounts.get(raw).expect("raw fixed slot").clone();
    *accounts.get_mut(staging).expect("staging fixed slot") = raw;
}

fn fractional_wrap_request() -> Vec<u8> {
    use dclutch_claims::fractional::{
        FractionalExposureActionV2, FractionalExposureRequestInputV2, FractionalExposureRequestV2,
    };

    FractionalExposureRequestV2::new(
        FractionalExposureActionV2::Wrap,
        FractionalExposureRequestInputV2 {
            release_set: [1; 32],
            market: [2; 32],
            product_record: [3; 32],
            result_domain: [4; 32],
            terms: [5; 32],
            token_behavior: [6; 32],
            exposure: [7; 32],
            owner: [8; 32],
            source_token_account: [0; 32],
            destination_token_account: [9; 32],
            terminal_digest: [0; 32],
            expected_revision: 11,
            quantity: 13,
            representation_coordinate: 0,
        },
    )
    .expect("canonical Fractional Wrap")
    .to_bytes()
    .expect("Fractional bytes")
    .to_vec()
}

fn fractional_wrap_invocation(request_len: usize) -> dclutch_vm::effect::v3::ResolvedInvocationV3 {
    dclutch_vm::effect::v3::ResolvedInvocationV3 {
        role: FixedRole::Claims,
        kind: dclutch_vm::effect::v3::RouteKindV3::Once,
        item: None,
        fixed_account_start: 5,
        fixed_account_count: u16::try_from(
            dclutch_claims::fractional::FRACTIONAL_ATOMIC_ACCOUNT_COUNT_V3,
        )
        .expect("Fractional account count"),
        item_account_start: 0,
        item_account_count: 0,
        item_account_stride: 0,
        repeated_item_count: 0,
        request_offset: 0,
        request_len,
        borrowed_witness: None,
        receipt_dependencies: dclutch_vm::effect::v3::ResolvedReceiptDependenciesV3::empty(),
        receipt_dependency: None,
    }
}

#[test]
fn only_exact_fractional_root_alias_may_cross_local_child_boundary() {
    let request = fractional_wrap_request();
    let invocation = fractional_wrap_invocation(request.len());
    let logical_count =
        usize::from(invocation.fixed_account_start) + usize::from(invocation.fixed_account_count);
    let mut aliases = (0..logical_count).collect::<Vec<_>>();
    let root = usize::from(invocation.fixed_account_start)
        + dclutch_claims::fractional::FRACTIONAL_ATOMIC_ROOT_V3;
    aliases[root] = 0;
    let mut participation = vec![CoordinateParticipationV3::default(); logical_count];
    participation[0].mark_local_mutation();

    assert_eq!(
        fractional_local_root_overlap_v3(invocation, &request, &request, &aliases)
            .expect("exact overlap"),
        Some(0)
    );
    record_child_reach_and_require_disjoint_from_local(
        invocation,
        &aliases,
        &mut participation,
        AllowedLocalOverlapV3::FractionalRoot(0),
    )
    .expect("sole root overlap");

    let mut foreign = request.clone();
    foreign[0] ^= 1;
    assert_eq!(
        fractional_local_root_overlap_v3(invocation, &foreign, &request, &aliases)
            .expect("foreign bank"),
        None
    );
    assert!(
        record_child_reach_and_require_disjoint_from_local(
            invocation,
            &aliases,
            &mut participation,
            AllowedLocalOverlapV3::None,
        )
        .is_err()
    );

    participation[6].mark_local_mutation();
    assert!(
        record_child_reach_and_require_disjoint_from_local(
            invocation,
            &aliases,
            &mut participation,
            AllowedLocalOverlapV3::FractionalRoot(0),
        )
        .is_err(),
        "a second local/child overlap must not ride the root exception"
    );
}

#[test]
fn fractional_child_must_leave_sole_root_prestate_unchanged() {
    let request = fractional_wrap_request();
    let before = [17_u8; 128];
    let digest = dclutch_sha256_adapter::digest(&before);
    require_fractional_root_prestate_v3(&request, &before, digest)
        .expect("unchanged Fractional root");

    let mut after = before;
    after[112] ^= 1;
    assert!(require_fractional_root_prestate_v3(&request, &after, digest).is_err());
    require_fractional_root_prestate_v3(b"another-family", &after, digest)
        .expect("unrelated family is not widened");
}

#[test]
fn series_expiry_overlap_is_exactly_root_and_ticket_never_a_subset_or_superset() {
    let invocation = dclutch_vm::effect::v3::ResolvedInvocationV3 {
        role: FixedRole::Core,
        kind: dclutch_vm::effect::v3::RouteKindV3::Once,
        item: None,
        fixed_account_start: u16::try_from(series_expiry::SERIES_EXPIRE_CORE_ROUTE_START_V1)
            .expect("route start"),
        fixed_account_count: u16::try_from(series_expiry::SERIES_EXPIRE_CORE_ROUTE_COUNT_V1)
            .expect("route count"),
        item_account_start: 0,
        item_account_count: 0,
        item_account_stride: 0,
        repeated_item_count: 0,
        request_offset: 0,
        request_len: 976,
        borrowed_witness: None,
        receipt_dependencies: dclutch_vm::effect::v3::ResolvedReceiptDependenciesV3::empty(),
        receipt_dependency: None,
    };
    let mut aliases = (0..series_expiry::SERIES_EXPIRE_LOGICAL_ACCOUNTS_V1).collect::<Vec<_>>();
    aliases[series_expiry::SERIES_EXPIRE_CORE_ROUTE_START_V1 + 14] = 0;
    aliases[series_expiry::SERIES_EXPIRE_CORE_ROUTE_START_V1 + 15] =
        series_expiry::SERIES_EXPIRE_TICKET_STATE_ACCOUNT_V1;
    let mut participation = vec![
        CoordinateParticipationV3::default();
        series_expiry::SERIES_EXPIRE_LOGICAL_ACCOUNTS_V1
    ];
    participation[0].mark_local_mutation();
    participation[series_expiry::SERIES_EXPIRE_TICKET_STATE_ACCOUNT_V1].mark_local_mutation();
    let exact = AllowedLocalOverlapV3::SeriesExpiryReplay { root: 0, ticket: 5 };

    record_child_reach_and_require_disjoint_from_local(
        invocation,
        &aliases,
        &mut participation,
        exact,
    )
    .expect("exact replay pair");
    assert!(
        record_child_reach_and_require_disjoint_from_local(
            invocation,
            &aliases,
            &mut participation,
            AllowedLocalOverlapV3::FractionalRoot(0),
        )
        .is_err(),
        "root-only overlap cannot omit Ticket",
    );
    assert!(
        record_child_reach_and_require_disjoint_from_local(
            invocation,
            &aliases,
            &mut participation,
            AllowedLocalOverlapV3::FractionalRoot(5),
        )
        .is_err(),
        "Ticket-only overlap cannot omit root",
    );

    aliases[series_expiry::SERIES_EXPIRE_CORE_ROUTE_START_V1] = 6;
    participation[6].mark_local_mutation();
    assert!(
        record_child_reach_and_require_disjoint_from_local(
            invocation,
            &aliases,
            &mut participation,
            exact
        )
        .is_err(),
        "a third representative cannot ride the fixed pair",
    );
}

#[test]
fn series_expiry_post_child_proof_binds_both_data_and_lamports() {
    let root_key = Pubkey::new_unique();
    let ticket_key = Pubkey::new_unique();
    let owner = Pubkey::new_unique();
    let mut root_lamports = 41_u64;
    let mut ticket_lamports = 43_u64;
    let mut root_data = [0x51_u8; 96];
    let mut ticket_data = [0x52_u8; 80];
    let root = AccountInfo::new(
        &root_key,
        false,
        true,
        &mut root_lamports,
        &mut root_data,
        &owner,
        false,
    );
    let ticket = AccountInfo::new(
        &ticket_key,
        false,
        true,
        &mut ticket_lamports,
        &mut ticket_data,
        &owner,
        false,
    );
    let expected = SeriesExpiryReplayPrestateV1::authenticated(
        &root,
        hash(&root.try_borrow_data().expect("root data")).to_bytes(),
        &ticket,
        hash(&ticket.try_borrow_data().expect("Ticket data")).to_bytes(),
    )
    .expect("authenticated prestates");
    require_series_expiry_replay_prestate_v1(&root, &ticket, expected)
        .expect("exact post-child state");

    root.try_borrow_mut_data().expect("root mutation")[0] ^= 1;
    assert!(require_series_expiry_replay_prestate_v1(&root, &ticket, expected).is_err());
    root.try_borrow_mut_data().expect("root restore")[0] ^= 1;
    **ticket
        .try_borrow_mut_lamports()
        .expect("Ticket lamport mutation") += 1;
    assert!(require_series_expiry_replay_prestate_v1(&root, &ticket, expected).is_err());
}

#[test]
fn series_expiry_future_market_is_route_local_distinct_and_system_vacant() {
    let key = Pubkey::new_unique();
    let other = Pubkey::new_unique();
    let controller = Pubkey::new_unique();
    let core = Pubkey::new_unique();
    let mut lamports = 0_u64;
    let mut empty = [];
    let exact = AccountInfo::new(
        &key,
        false,
        false,
        &mut lamports,
        &mut empty,
        &system_program::ID,
        false,
    );
    require_series_expiry_future_market_vacancy_v1(&exact, key, &controller)
        .expect("exact vacant future Market");
    assert!(
        require_series_expiry_future_market_vacancy_v1(&exact, other, &controller).is_err(),
        "a different canonical PDA must refuse",
    );
    assert!(
        require_series_expiry_future_market_vacancy_v1(&exact, key, &key).is_err(),
        "the live controller Market may never alias the future Market",
    );

    let mut writable_lamports = 0_u64;
    let mut writable_empty = [];
    let writable = AccountInfo::new(
        &key,
        false,
        true,
        &mut writable_lamports,
        &mut writable_empty,
        &system_program::ID,
        false,
    );
    assert!(
        require_series_expiry_future_market_vacancy_v1(&writable, key, &controller).is_err(),
        "the route-local future Market is always readonly",
    );

    let mut owned_lamports = 0_u64;
    let mut owned_empty = [];
    let owned = AccountInfo::new(
        &key,
        false,
        false,
        &mut owned_lamports,
        &mut owned_empty,
        &core,
        false,
    );
    assert!(
        require_series_expiry_future_market_vacancy_v1(&owned, key, &controller).is_err(),
        "a live or stranger-owned Market is not the pre-Market state",
    );

    let mut data_lamports = 0_u64;
    let mut data = [1_u8];
    let initialized = AccountInfo::new(
        &key,
        false,
        false,
        &mut data_lamports,
        &mut data,
        &system_program::ID,
        false,
    );
    assert!(
        require_series_expiry_future_market_vacancy_v1(&initialized, key, &controller,).is_err(),
        "System ownership alone is not vacancy",
    );

    let second = Pubkey::new_unique();
    let mut second_lamports = 0_u64;
    let mut second_empty = [];
    let second_future = AccountInfo::new(
        &second,
        false,
        false,
        &mut second_lamports,
        &mut second_empty,
        &system_program::ID,
        false,
    );
    require_series_expiry_future_market_vacancy_v1(&exact, key, &controller)
        .expect("first occurrence under persistent controller root");
    require_series_expiry_future_market_vacancy_v1(&second_future, second, &controller)
        .expect("second occurrence under the same persistent controller root");
}

#[test]
fn series_expiry_permit_requires_exact_prefunded_writable_system_vacancy() {
    use dclutch_market::SERIES_FOUNDING_PERMIT_BYTES_V1;

    let rent = Rent::default();
    let key = Pubkey::new_unique();
    let other = Pubkey::new_unique();
    let owner = Pubkey::new_unique();
    let minimum = rent.minimum_balance(SERIES_FOUNDING_PERMIT_BYTES_V1);
    let mut lamports = minimum;
    let mut empty = [];
    let exact = AccountInfo::new(
        &key,
        false,
        true,
        &mut lamports,
        &mut empty,
        &system_program::ID,
        false,
    );
    require_series_expiry_vacant_permit_v1(&exact, key).expect("exact unallocated permit");
    assert!(require_series_expiry_vacant_permit_v1(&exact, other).is_err());

    let mut readonly_lamports = minimum;
    let mut readonly_empty = [];
    let readonly = AccountInfo::new(
        &key,
        false,
        false,
        &mut readonly_lamports,
        &mut readonly_empty,
        &system_program::ID,
        false,
    );
    assert!(require_series_expiry_vacant_permit_v1(&readonly, key).is_err());

    let mut signer_lamports = minimum;
    let mut signer_empty = [];
    let signer = AccountInfo::new(
        &key,
        true,
        true,
        &mut signer_lamports,
        &mut signer_empty,
        &system_program::ID,
        false,
    );
    assert!(require_series_expiry_vacant_permit_v1(&signer, key).is_err());

    let mut owned_lamports = minimum;
    let mut owned_empty = [];
    let owned = AccountInfo::new(
        &key,
        false,
        true,
        &mut owned_lamports,
        &mut owned_empty,
        &owner,
        false,
    );
    assert!(require_series_expiry_vacant_permit_v1(&owned, key).is_err());

    // A SLOT A RISEN RATE STRANDED IS STILL THE SLOT THE FOUNDING PREPAID.
    // This route REFUNDS a permit Core never allocated: the prepayment was
    // made by an earlier transaction at that transaction's rate, and a
    // floor at the rate of the moment refuses the refund the instant the
    // cluster charges more -- permanently, because nobody owns a vacant
    // permit to top up. One lamport below today's minimum used to refuse
    // here.
    let mut stranded_lamports = minimum.saturating_sub(1);
    let mut stranded_empty = [];
    let stranded = AccountInfo::new(
        &key,
        false,
        true,
        &mut stranded_lamports,
        &mut stranded_empty,
        &system_program::ID,
        false,
    );
    require_series_expiry_vacant_permit_v1(&stranded, key)
        .expect("a prepaid slot the cluster repriced is still prepaid");

    // THE HOSTILE is a DRAINED slot: no prepayment left to hand back, and
    // its vacancy is residue the runtime reaps rather than a prepayment.
    let mut drained_lamports = 0;
    let mut drained_empty = [];
    let drained = AccountInfo::new(
        &key,
        false,
        true,
        &mut drained_lamports,
        &mut drained_empty,
        &system_program::ID,
        false,
    );
    assert_eq!(
        require_series_expiry_vacant_permit_v1(&drained, key),
        Err(TradingSbfError::Content.into())
    );
}

#[test]
fn sealed_execution_fixed_alias_set_is_all_or_nothing_and_exact() {
    let distinct = distinct_fixed_infos();
    assert!(!validate_hot_fixed_alias_shape_v3(&distinct).expect("distinct fixed frame"));

    let mut sealed = distinct_fixed_infos();
    for (raw, staging) in SEALED_EXECUTION_FIXED_ALIASES_V3 {
        alias_fixed_slot(&mut sealed, raw, staging);
    }
    assert!(validate_hot_fixed_alias_shape_v3(&sealed).expect("exact six sealed aliases"));

    let mut partial = distinct_fixed_infos();
    alias_fixed_slot(
        &mut partial,
        HOT_DESCRIPTOR_RAW_ACCOUNT_V3,
        HOT_DESCRIPTOR_STAGING_ACCOUNT_V3,
    );
    assert!(validate_hot_fixed_alias_shape_v3(&partial).is_err());

    let mut wrong_pair = sealed.clone();
    let wrong = wrong_pair
        .get(HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3)
        .expect("account-profile raw fixed slot")
        .clone();
    *wrong_pair
        .get_mut(HOT_DESCRIPTOR_STAGING_ACCOUNT_V3)
        .expect("descriptor staging fixed slot") = wrong;
    assert!(validate_hot_fixed_alias_shape_v3(&wrong_pair).is_err());

    let mut seventh = sealed;
    alias_fixed_slot(
        &mut seventh,
        HOT_CONFIG_RAW_ACCOUNT_V3,
        HOT_CONFIG_STAGING_ACCOUNT_V3,
    );
    assert!(validate_hot_fixed_alias_shape_v3(&seventh).is_err());
}

#[test]
fn lifecycle_rent_credit_v2_binds_expected_key_market_release_and_generation() {
    use dclutch_market::rent::{
        RefundAuthority,
        lifecycle_v2::{
            LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2, LifecycleAccountIdV2, LifecycleRentCreditV2,
        },
    };

    let rent_program = Pubkey::new_unique();
    let refund = Pubkey::new_unique();
    let market = Pubkey::new_unique();
    let release = Pubkey::new_unique();
    let generation = 9_u64;
    let generation_seed = generation.to_le_bytes();
    let (credit_key, bump) = Pubkey::find_program_address(
        &[
            LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2,
            market.as_ref(),
            &generation_seed,
        ],
        &rent_program,
    );
    let state = LifecycleRentCreditV2::new(
        RefundAuthority::new(refund.to_bytes()).expect("refund"),
        LifecycleAccountIdV2::new(market.to_bytes()).expect("market"),
        LifecycleAccountIdV2::new(release.to_bytes()).expect("release"),
        generation,
        bump,
    )
    .expect("state");
    let rent = Rent::default();
    let floor = rent.minimum_balance(LIFECYCLE_RENT_CREDIT_BYTES_V2);
    let credit = AccountInfo::new(
        Box::leak(Box::new(credit_key)),
        false,
        true,
        Box::leak(Box::new(floor)),
        Box::leak(state.to_bytes().to_vec().into_boxed_slice()),
        Box::leak(Box::new(rent_program)),
        false,
    );
    let owner = AccountInfo::new(
        Box::leak(Box::new(rent_program)),
        false,
        false,
        Box::leak(Box::new(1_u64)),
        Box::leak(Vec::new().into_boxed_slice()),
        Box::leak(Box::new(Pubkey::new_unique())),
        true,
    );
    // The fixed authenticated Registry is the single owner-program fact;
    // lifecycle runtime contains only the credit coordinate.
    let accounts = [&credit];
    let authenticated = authenticate_lifecycle_credit_v3(
        &accounts,
        &owner,
        0,
        floor,
        market.to_bytes(),
        release.to_bytes(),
        generation,
        credit_key.to_bytes(),
    )
    .expect("exact lifecycle credit");
    assert_eq!(authenticated.beneficiary, refund.to_bytes());

    let mut non_executable_owner = owner.clone();
    non_executable_owner.executable = false;
    let mut writable_owner = owner.clone();
    writable_owner.is_writable = true;
    let mut signer_owner = owner.clone();
    signer_owner.is_signer = true;
    let unrelated_owner_key = Pubkey::new_unique();
    let unrelated_executable_owner = AccountInfo::new(
        Box::leak(Box::new(unrelated_owner_key)),
        false,
        false,
        Box::leak(Box::new(1_u64)),
        Box::leak(Vec::new().into_boxed_slice()),
        Box::leak(Box::new(Pubkey::new_unique())),
        true,
    );
    // `owner_program` is a PRIVILEGE fact and never an identity one, and
    // this is the arm that says so out loud. It is `frame.registry` at every
    // call site -- an already authenticated fixed coordinate, not something
    // a caller chooses -- and the credit's real owner binding is the
    // `create_program_address` at the end of the function, under
    // `account.owner`. `686bf2e5` did pin the two together and it was
    // measured unsatisfiable on every honest transaction, because lifecycle
    // credits are owned by the RENT program and `frame.registry` is the
    // Registry; see the note on `authenticate_lifecycle_credit_v3`.
    //
    // So an unrelated executable readonly account IS admitted here, and this
    // case asserts that rather than pretending otherwise. What must not be
    // admitted is a substituted CREDIT owner, which is the attack this
    // parameter used to be mistaken for a defence against, and which the
    // next assertion drives.
    assert!(
        authenticate_lifecycle_credit_v3(
            &accounts,
            &unrelated_executable_owner,
            0,
            floor,
            market.to_bytes(),
            release.to_bytes(),
            generation,
            credit_key.to_bytes()
        )
        .is_ok(),
        "the owner-program argument carries privileges, not identity",
    );

    // THE SUBSTITUTION THAT MATTERS, and nothing tested it. The credit is a
    // frame coordinate, so its owner is whatever the caller staged; the
    // lamport census found `LifecycleRentCreditV2` to be the one root all
    // four families compare against, and an attacker's program owning a
    // well-formed credit body is the whole attack. The derivation refuses it
    // because `create_program_address` under a foreign owner cannot
    // reproduce this credit's address from this credit's own seeds.
    let foreign_owned_credit = AccountInfo::new(
        Box::leak(Box::new(credit_key)),
        false,
        true,
        Box::leak(Box::new(floor)),
        Box::leak(state.to_bytes().to_vec().into_boxed_slice()),
        Box::leak(Box::new(unrelated_owner_key)),
        false,
    );
    assert_eq!(
        authenticate_lifecycle_credit_v3(
            &[&foreign_owned_credit],
            &owner,
            0,
            floor,
            market.to_bytes(),
            release.to_bytes(),
            generation,
            credit_key.to_bytes()
        ),
        Err(TradingSbfError::Content.into()),
        "a credit under a substituted owner must never authenticate",
    );

    for (name, hostile_owner) in [
        ("non-executable", &non_executable_owner),
        ("writable", &writable_owner),
        ("signer", &signer_owner),
    ] {
        assert_eq!(
            authenticate_lifecycle_credit_v3(
                &accounts,
                hostile_owner,
                0,
                floor,
                market.to_bytes(),
                release.to_bytes(),
                generation,
                credit_key.to_bytes()
            ),
            Err(TradingSbfError::Content.into()),
            "{name} owner substitution or privilege widening must refuse",
        );
    }

    let unrelated_runtime = [&credit, &unrelated_executable_owner];
    assert!(
        authenticate_lifecycle_credit_v3(
            &unrelated_runtime,
            &owner,
            0,
            floor,
            market.to_bytes(),
            release.to_bytes(),
            generation,
            credit_key.to_bytes()
        )
        .is_ok(),
        "runtime contents neither supply nor replace the fixed owner fact",
    );
    assert_eq!(
        authenticate_lifecycle_credit_v3(
            &accounts,
            &owner,
            0,
            floor,
            market.to_bytes(),
            release.to_bytes(),
            generation,
            Pubkey::new_unique().to_bytes()
        ),
        Err(TradingSbfError::Content.into()),
    );
    assert_eq!(
        authenticate_lifecycle_credit_v3(
            &accounts,
            &owner,
            0,
            floor,
            Pubkey::new_unique().to_bytes(),
            release.to_bytes(),
            generation,
            credit_key.to_bytes()
        ),
        Err(TradingSbfError::Content.into()),
    );
    assert_eq!(
        authenticate_lifecycle_credit_v3(
            &accounts,
            &owner,
            0,
            floor,
            market.to_bytes(),
            Pubkey::new_unique().to_bytes(),
            generation,
            credit_key.to_bytes()
        ),
        Err(TradingSbfError::Content.into()),
    );
    assert_eq!(
        authenticate_lifecycle_credit_v3(
            &accounts,
            &owner,
            0,
            floor,
            market.to_bytes(),
            release.to_bytes(),
            generation + 1,
            credit_key.to_bytes()
        ),
        Err(TradingSbfError::Content.into()),
    );

    let stale_v1 = AccountInfo::new(
        Box::leak(Box::new(credit_key)),
        false,
        true,
        Box::leak(Box::new(rent.minimum_balance(48))),
        Box::leak(vec![0_u8; 48].into_boxed_slice()),
        Box::leak(Box::new(rent_program)),
        false,
    );
    let stale_accounts = [&stale_v1];
    assert_eq!(
        authenticate_lifecycle_credit_v3(
            &stale_accounts,
            &owner,
            0,
            stale_v1.lamports(),
            market.to_bytes(),
            release.to_bytes(),
            generation,
            credit_key.to_bytes()
        ),
        Err(TradingSbfError::Content.into()),
    );
}

#[test]
fn effect_v4_schema_and_zero_extension_envelope_are_exact() {
    use dclutch_vm::effect::{
        v3::{
            HEADER_BYTES,
            encode::{EffectGeometryV3, encode_effect_program_v3_atomic},
        },
        v4::{BorrowedRangePolicyV4, HEADER_BYTES_V4, encode_program_v4_atomic},
    };
    let mut base_scratch = [0_u8; HEADER_BYTES];
    let mut base = [0_u8; HEADER_BYTES];
    encode_effect_program_v3_atomic(
        EffectGeometryV3 {
            fixed_accounts: 1,
            item_account_stride: 0,
            common_scalars: 1,
            item_scalar_stride: 0,
            common_identities: 0,
            item_identity_stride: 0,
        },
        &[],
        &[],
        &[],
        &mut base_scratch,
        &mut base,
    )
    .expect("fixed base");
    let mut scratch = vec![0_u8; HEADER_BYTES_V4 + HEADER_BYTES];
    let mut output = vec![0_u8; HEADER_BYTES_V4 + HEADER_BYTES];
    encode_program_v4_atomic(
        &base,
        BorrowedRangePolicyV4::DisjointExactCoverage,
        1,
        &[],
        &[],
        &mut scratch,
        &mut output,
    )
    .expect("zero-extension successor");
    let selected =
        decode_selected_effect_v4(EFFECT_SCHEMA_ID_V4, &output).expect("selected V4 effect");
    assert_eq!(selected.successor.span_count(), 0);
    assert_eq!(selected.successor.range_count(), 0);
    assert!(decode_selected_effect_v4([7; 32], &output).is_err());

    let mut hostile = output;
    *hostile.first_mut().expect("effect output is non-empty") ^= 1;
    assert!(decode_selected_effect_v4(EFFECT_SCHEMA_ID_V4, &hostile).is_err());
}

fn borrowed_range_effect_v4(
    policy: dclutch_vm::effect::v4::BorrowedRangePolicyV4,
    ranges: &[dclutch_vm::effect::v4::BorrowedRangeV4],
) -> Vec<u8> {
    use dclutch_vm::effect::{
        v2::FixedRole,
        v3::{
            HEADER_BYTES, ROUTE_BYTES, RouteKindV3,
            encode::{EffectGeometryV3, RouteInputV3, encode_effect_program_v3_atomic},
        },
        v4::{BORROWED_RANGE_BYTES_V4, HEADER_BYTES_V4, encode_program_v4_atomic},
    };

    const BASE_REQUEST: &[u8] = b"CORE_REQ";
    let route = RouteInputV3 {
        role: FixedRole::Core,
        kind: RouteKindV3::Once,
        enable_common_scalar: None,
        witness_range_common_scalar: None,
        receipt_dependency: None,
        fixed_account_start: 0,
        fixed_account_count: 1,
        item_account_start: 0,
        item_account_count: 0,
        fixed_request: BASE_REQUEST,
        item_request: &[],
    };
    let base_bytes = HEADER_BYTES + ROUTE_BYTES + BASE_REQUEST.len();
    let mut base_scratch = vec![0_u8; base_bytes];
    let mut base = vec![0_u8; base_bytes];
    encode_effect_program_v3_atomic(
        EffectGeometryV3 {
            fixed_accounts: 1,
            item_account_stride: 0,
            common_scalars: 1,
            item_scalar_stride: 0,
            common_identities: 0,
            item_identity_stride: 0,
        },
        &[route],
        &[],
        &[],
        &mut base_scratch,
        &mut base,
    )
    .expect("one-route base");
    let successor_bytes = HEADER_BYTES_V4 + ranges.len() * BORROWED_RANGE_BYTES_V4 + base.len();
    let mut scratch = vec![0_u8; successor_bytes];
    let mut output = vec![0_u8; successor_bytes];
    encode_program_v4_atomic(&base, policy, 4, &[], ranges, &mut scratch, &mut output)
        .expect("range successor");
    output
}

#[test]
fn child_request_digest_helpers_pin_zero_range_and_range_topology() {
    let base = b"CORE_REQ";
    let legacy = b"legacy-proof";
    let expected_v4 = hashv(&[
        CHILD_REQUEST_DIGEST_DOMAIN_V4,
        &[0, 1],
        &(base.len() as u32).to_le_bytes(),
        &(legacy.len() as u32).to_le_bytes(),
        base,
        legacy,
    ])
    .to_bytes();
    assert_eq!(
        child_request_digest_v4(base, Some(legacy)).expect("legacy digest"),
        expected_v4
    );

    let proof = b"proof";
    let mut framed = Vec::new();
    framed.extend_from_slice(CHILD_REQUEST_DIGEST_DOMAIN_V5);
    framed.extend_from_slice(&1_u16.to_le_bytes());
    framed.extend_from_slice(&(base.len() as u32).to_le_bytes());
    framed.extend_from_slice(&(proof.len() as u32).to_le_bytes());
    framed.extend_from_slice(base);
    framed.extend_from_slice(proof);
    assert_eq!(
        child_request_digest_v5(base, 1, |ordinal| (ordinal == 0)
            .then_some(proof.as_slice()))
        .expect("one authenticated range"),
        hash(&framed).to_bytes()
    );
    assert_eq!(
        child_request_digest_v5(base, 0, |_| None),
        Err(TradingSbfError::Content.into())
    );
    assert_eq!(
        child_request_digest_v5(base, 2, |ordinal| (ordinal == 0)
            .then_some(proof.as_slice())),
        Err(TradingSbfError::Content.into())
    );

    let substituted = b"proog";
    assert_ne!(
        child_request_digest_v5(base, 1, |_| Some(proof.as_slice())).expect("proof"),
        child_request_digest_v5(base, 1, |_| Some(substituted.as_slice())).expect("substitution")
    );
    let first = b"pr";
    let second = b"oof";
    assert_ne!(
        child_request_digest_v5(base, 1, |_| Some(proof.as_slice())).expect("one range"),
        child_request_digest_v5(base, 2, |ordinal| match ordinal {
            0 => Some(first.as_slice()),
            1 => Some(second.as_slice()),
            _ => None,
        })
        .expect("same concatenation, different topology")
    );
}

#[test]
fn borrowed_ranges_append_exactly_and_refuse_overlap_oob_without_mutation() {
    use dclutch_vm::effect::v4::{
        BorrowedRangePolicyV4, BorrowedRangeV4, ProgramV4, RequestCoordinateV4,
    };

    let exact = [BorrowedRangeV4::new(
        0,
        RequestCoordinateV4::Fixed(4),
        RequestCoordinateV4::Fixed(4),
    )];
    let exact_bytes =
        borrowed_range_effect_v4(BorrowedRangePolicyV4::DisjointExactCoverage, &exact);
    let exact_program = ProgramV4::decode(&exact_bytes).expect("exact range program");
    let family_request = b"HEADPROO";
    assert_eq!(
        exact_program.validate_request_coverage(family_request.len(), 0, &[0], &[]),
        Ok(())
    );
    let ranges = BorrowedRouteRangesV4::new(exact_program, 0, 0, &[0], family_request);
    let mut output = b"CORE_REQ".to_vec();
    ranges.append_to(&mut output).expect("exact append");
    assert_eq!(output, b"CORE_REQPROO");

    let overlapping = [
        BorrowedRangeV4::new(
            0,
            RequestCoordinateV4::Fixed(4),
            RequestCoordinateV4::Fixed(3),
        ),
        BorrowedRangeV4::new(
            0,
            RequestCoordinateV4::Fixed(6),
            RequestCoordinateV4::Fixed(2),
        ),
    ];
    let overlap_bytes =
        borrowed_range_effect_v4(BorrowedRangePolicyV4::DisjointExactCoverage, &overlapping);
    let overlap_program = ProgramV4::decode(&overlap_bytes).expect("shape-decodable overlap");
    assert_eq!(
        overlap_program.validate_request_coverage(family_request.len(), 0, &[0], &[]),
        Err(dclutch_vm::effect::v4::ErrorV4::RequestCoverage)
    );

    let oob = [BorrowedRangeV4::new(
        0,
        RequestCoordinateV4::Fixed(4),
        RequestCoordinateV4::Fixed(8),
    )];
    let oob_bytes = borrowed_range_effect_v4(BorrowedRangePolicyV4::DisjointExactCoverage, &oob);
    let oob_program = ProgramV4::decode(&oob_bytes).expect("shape-decodable oob");
    assert_eq!(
        oob_program.validate_request_coverage(family_request.len(), 0, &[0], &[]),
        Err(dclutch_vm::effect::v4::ErrorV4::RequestCoverage)
    );
    let hostile = BorrowedRouteRangesV4::new(oob_program, 0, 0, &[0], family_request);
    let mut unchanged = b"sentinel".to_vec();
    let before = unchanged.clone();
    assert_eq!(
        hostile.append_to(&mut unchanged),
        Err(TradingSbfError::Content.into())
    );
    assert_eq!(unchanged, before, "refusal mutated the child wire");
}

#[test]
fn authenticated_accelerator_inline_bank_is_exact_and_untruncated() {
    let mut bank = Vec::new();
    bank.extend_from_slice(&11_u64.to_le_bytes());
    bank.extend_from_slice(&u64::MAX.to_le_bytes());
    bank.extend_from_slice(&[0x5a; 32]);
    let content = |byte| ContentId::new([byte; 32]).expect("nonzero content");
    let request = AcceleratorRequestV2::new(
        RequestTransportV2::Inline,
        content(1),
        content(2),
        content(3),
        content(4),
        ContentId::new(hash(&bank).to_bytes()).expect("bank digest"),
        7,
        2,
        1,
        0,
        &bank,
    )
    .map(AdmittedAcceleratorRequestV2::ChunkedBankV2)
    .expect("inline request");
    assert_eq!(
        authenticate_accelerator_input_bank_v4(request, &[], &Pubkey::new_unique())
            .expect("authenticated inline bank"),
        bank
    );
    let (scalars, identities) =
        decode_accelerator_register_bank_v4(request, &bank).expect("register decode");
    assert_eq!(scalars, [11, u64::MAX]);
    assert_eq!(identities, [[0x5a; 32]]);

    let wrong_digest = AcceleratorRequestV2::new(
        RequestTransportV2::Inline,
        content(1),
        content(2),
        content(3),
        content(4),
        content(9),
        7,
        2,
        1,
        0,
        &bank,
    )
    .map(AdmittedAcceleratorRequestV2::ChunkedBankV2)
    .expect("hostile request shape");
    assert!(
        authenticate_accelerator_input_bank_v4(wrong_digest, &[], &Pubkey::new_unique()).is_err()
    );
    assert!(
        decode_accelerator_register_bank_v4(
            request,
            bank.get(..40).expect("bank holds at least 40 bytes")
        )
        .is_err()
    );
}

/// The accelerator's read-only frame is bound to the top-level instruction
/// ENTIRE -- every fixed slot, the capability seal included.
///
/// The loop this exercises is a `zip`, and `zip` truncates to the shorter
/// side without a diagnostic. Its array held thirty-eight entries against
/// thirty-nine metas, so `HOT_CAPABILITY_SEAL_ACCOUNT_V3` was silently
/// never compared: an accelerator could be handed a frame whose seal slot
/// was an account the authorized instruction never named, and this
/// authenticator ACCEPTED it. The seal is the account that survived that
/// gap, but it is not what this test asserts -- it walks EVERY index, so
/// the fortieth account is covered the day the frame grows one, which is
/// the drift that produced the hole in the first place (see
/// `ADMITTED_ACCELERATOR_HOT_FIXED_COUNT_V4`).
///
/// A positive baseline is load-bearing here. Three of this path's four
/// known defects survived precisely because the only test asserting them
/// asserted REFUSAL, and a function that can only refuse passes such a test
/// perfectly. So the substitutions below are differentials against an
/// invocation that genuinely succeeds.
#[test]
fn the_accelerator_top_level_frame_binds_every_fixed_meta_including_the_seal() {
    use solana_instructions_sysvar::construct_instructions_data;
    use solana_program::sysvar::instructions::{BorrowedAccountMeta, BorrowedInstruction};

    fn info(key: Pubkey, owner: Pubkey, executable: bool, data: Vec<u8>) -> AccountInfo<'static> {
        // The accelerator frame is read-only by construction:
        // `parse_accelerator_readonly` refuses any signer or writable slot.
        AccountInfo::new(
            Box::leak(Box::new(key)),
            false,
            false,
            Box::leak(Box::new(0_u64)),
            Box::leak(data.into_boxed_slice()),
            Box::leak(Box::new(owner)),
            executable,
        )
    }

    let trading_program = Pubkey::new_unique();
    let owner = Pubkey::new_unique();
    let mut keys = (0..HOT_FIXED_ACCOUNT_COUNT_V3)
        .map(|_| Pubkey::new_unique())
        .collect::<Vec<_>>();
    let set = |keys: &mut Vec<Pubkey>, slot: usize, key: Pubkey| {
        *keys.get_mut(slot).expect("Hot fixed slot inside the frame") = key;
    };
    set(&mut keys, HOT_TRADING_PROGRAM_ACCOUNT_V3, trading_program);
    set(&mut keys, HOT_RENT_SYSVAR_ACCOUNT_V3, sysvar::rent::ID);
    set(
        &mut keys,
        HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3,
        sysvar::instructions::ID,
    );
    let key_at =
        |keys: &[Pubkey], slot: usize| *keys.get(slot).expect("Hot fixed slot inside the frame");

    let envelope = HotExecutionEnvelopeV3::new(
        1,
        [0x31; 32],
        key_at(&keys, HOT_MARKET_ACCOUNT_V3).to_bytes(),
        7,
        [0x32; 32],
    )
    .expect("envelope");
    let mut hot_bytes = envelope.to_bytes().to_vec();
    hot_bytes.push(9);

    // Two scalars and one identity is a 48-byte bank: one inline chunk, so
    // exactly one caller authority, which is what the frame supplies below.
    let bank = vec![0x5a_u8; 48];
    let content = |byte| ContentId::new([byte; 32]).expect("nonzero content");
    let request = AcceleratorRequestV2::new(
        RequestTransportV2::Inline,
        content(1),
        content(2),
        content(3),
        content(4),
        ContentId::new(hash(&bank).to_bytes()).expect("bank digest"),
        7,
        2,
        1,
        0,
        &bank,
    )
    .expect("inline request");
    assert_eq!(request.chunk_count(), 1);
    let request = AdmittedAcceleratorRequestV2::ChunkedBankV2(request);

    let strategy_keys = (0..ADMITTED_ACCELERATOR_STRATEGY_EVIDENCE_COUNT_V4)
        .map(|_| Pubkey::new_unique())
        .collect::<Vec<_>>();
    let caller_key = Pubkey::new_unique();

    // The canonical meta vector: the thirty-nine fixed slots, the eight
    // strategy evidence accounts, then this chunk's caller authority. Only
    // the root is writable on the ordinary (non-Registry) top level.
    let sysvar_bytes = |meta_keys: &[Pubkey]| {
        let mut metas = meta_keys
            .iter()
            .enumerate()
            .map(|(index, key)| BorrowedAccountMeta {
                pubkey: key,
                is_signer: false,
                is_writable: index == HOT_ROOT_ACCOUNT_V3,
            })
            .collect::<Vec<_>>();
        metas.extend(
            strategy_keys
                .iter()
                .chain(core::iter::once(&caller_key))
                .map(|key| BorrowedAccountMeta {
                    pubkey: key,
                    is_signer: false,
                    is_writable: false,
                }),
        );
        let borrowed = [BorrowedInstruction {
            program_id: &trading_program,
            accounts: metas,
            data: &hot_bytes,
        }];
        let mut data = construct_instructions_data(&borrowed);
        let end = data.len();
        data.get_mut(end - 2..)
            .expect("current instruction")
            .copy_from_slice(&0_u16.to_le_bytes());
        data
    };

    let canonical_sysvar = sysvar_bytes(&keys);
    let fixed = keys
        .iter()
        .enumerate()
        .map(|(index, key)| {
            let executable = matches!(
                index,
                HOT_CORE_PROGRAM_ACCOUNT_V3
                    | HOT_TRADING_PROGRAM_ACCOUNT_V3
                    | HOT_REGISTRY_PROGRAM_ACCOUNT_V3
            );
            let data = if index == HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3 {
                canonical_sysvar.clone()
            } else {
                Vec::new()
            };
            info(*key, owner, executable, data)
        })
        .collect::<Vec<_>>();
    let strategy_evidence = strategy_keys
        .iter()
        .map(|key| info(*key, owner, false, Vec::new()))
        .collect::<Vec<_>>();
    let caller_authority = info(caller_key, owner, false, Vec::new());

    let frame = HotFrameV3::parse_accelerator_readonly(&trading_program, &fixed)
        .expect("read-only accelerator frame");

    // BASELINE, and it must be a success: everything below is a
    // differential against it.
    assert_eq!(
        authenticate_accelerator_top_level_v4(
            frame,
            &strategy_evidence,
            &caller_authority,
            None,
            request,
        )
        .expect("canonical accelerator top level"),
        hot_bytes
    );

    // Substituting ANY fixed meta must refuse. Before the array carried its
    // thirty-ninth entry, index 38 -- the capability seal -- passed.
    let unrelated = Pubkey::new_unique();
    for index in 0..HOT_FIXED_ACCOUNT_COUNT_V3 {
        let mut substituted = keys.clone();
        set(&mut substituted, index, unrelated);
        let bytes = sysvar_bytes(&substituted);
        assert_eq!(bytes.len(), canonical_sysvar.len());
        fixed
            .get(HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3)
            .expect("instructions sysvar slot inside the frame")
            .try_borrow_mut_data()
            .expect("instructions data")
            .copy_from_slice(&bytes);
        assert!(
            authenticate_accelerator_top_level_v4(
                frame,
                &strategy_evidence,
                &caller_authority,
                None,
                request,
            )
            .is_err(),
            "fixed meta {index} is not bound to the top-level instruction"
        );
        fixed
            .get(HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3)
            .expect("instructions sysvar slot inside the frame")
            .try_borrow_mut_data()
            .expect("instructions restore")
            .copy_from_slice(&canonical_sysvar);
    }

    // The evidence vector is length-checked rather than zipped short: a
    // caller handing over seven evidence accounts must refuse, not silently
    // authenticate the seven it can see.
    assert!(
        authenticate_accelerator_top_level_v4(
            frame,
            strategy_evidence
                .get(..ADMITTED_ACCELERATOR_STRATEGY_EVIDENCE_COUNT_V4 - 1)
                .expect("short evidence vector"),
            &caller_authority,
            None,
            request,
        )
        .is_err()
    );
}

#[test]
fn selector_is_exact_and_does_not_shadow_activation() {
    assert!(is_hot_execution_v3(b"DCLTHOT3"));
    assert!(!is_hot_execution_v3(b"DCLTHOT2"));
    assert!(!is_hot_execution_v3(b"DCLTHOT"));
}

#[test]
fn registry_continuation_authenticates_admission_and_market_union() {
    use dclutch_registry::svm::continuation_v1::RegistryContinuationAdmissionSeedsV1;
    use solana_instructions_sysvar::construct_instructions_data;
    use solana_program::sysvar::instructions::{BorrowedAccountMeta, BorrowedInstruction};

    fn info(
        key: Pubkey,
        signer: bool,
        writable: bool,
        owner: Pubkey,
        executable: bool,
        data: Vec<u8>,
    ) -> AccountInfo<'static> {
        AccountInfo::new(
            Box::leak(Box::new(key)),
            signer,
            writable,
            Box::leak(Box::new(0_u64)),
            Box::leak(data.into_boxed_slice()),
            Box::leak(Box::new(owner)),
            executable,
        )
    }

    let program_id = Pubkey::new_unique();
    let registry = Pubkey::new_unique();
    let owner = Pubkey::new_unique();
    let mut keys = (0..=HOT_FIXED_ACCOUNT_COUNT_V3)
        .map(|_| Pubkey::new_unique())
        .collect::<Vec<_>>();
    let key_at =
        |keys: &[Pubkey], slot: usize| *keys.get(slot).expect("Hot fixed slot inside the frame");
    *keys
        .get_mut(HOT_TRADING_PROGRAM_ACCOUNT_V3)
        .expect("Hot fixed slot inside the frame") = program_id;
    *keys
        .get_mut(HOT_REGISTRY_PROGRAM_ACCOUNT_V3)
        .expect("Hot fixed slot inside the frame") = registry;
    *keys
        .get_mut(HOT_RENT_SYSVAR_ACCOUNT_V3)
        .expect("Hot fixed slot inside the frame") = sysvar::rent::ID;
    *keys
        .get_mut(HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3)
        .expect("Hot fixed slot inside the frame") = sysvar::instructions::ID;
    let activation_bytes = vec![0xa7; 64];
    let release = ContentId::new([0x31; 32]).expect("release");
    let envelope = HotExecutionEnvelopeV3::new(
        1,
        release.to_bytes(),
        key_at(&keys, HOT_MARKET_ACCOUNT_V3).to_bytes(),
        7,
        [0x32; 32],
    )
    .expect("envelope");
    let mut hot_bytes = envelope.to_bytes().to_vec();
    hot_bytes.push(9);
    let activation_digest =
        ContentId::new(hash(&activation_bytes).to_bytes()).expect("activation digest");
    let hot_digest = ContentId::new(hash(&hot_bytes).to_bytes()).expect("Hot digest");
    let continuation = RegistryContinuationRequestV1::new_core_trading_hot(
        release,
        activation_digest,
        hot_digest,
        u32::try_from(hot_bytes.len()).expect("Hot width"),
    )
    .expect("continuation");
    let batch = continuation.role_batch_request().expect("batch");
    let batch_digest = ContentId::new(hash(&batch.to_bytes()).to_bytes()).expect("batch digest");
    let seeds = RegistryContinuationAdmissionSeedsV1::new(
        continuation,
        key_at(&keys, HOT_ACTIVATION_CACHE_ACCOUNT_V3).to_bytes(),
        batch_digest,
    )
    .expect("admission seeds");
    let release_seed = seeds.release_set();
    let cache_seed = seeds.activation_cache();
    let batch_seed = seeds.batch_request_digest();
    let mask_seed = seeds.role_mask();
    let role_seed = seeds.continuation_role();
    let digest_seed = seeds.continuation_digest();
    *keys
        .get_mut(HOT_FIXED_ACCOUNT_COUNT_V3)
        .expect("admission slot inside the frame") = Pubkey::find_program_address(
        &[
            seeds.domain(),
            release_seed.as_slice(),
            cache_seed.as_slice(),
            batch_seed.as_slice(),
            mask_seed.as_slice(),
            role_seed.as_slice(),
            digest_seed.as_slice(),
        ],
        &registry,
    )
    .0;

    let top_data = hot_bytes.clone();
    let outer_keys = [
        key_at(&keys, HOT_ACTIVATION_CACHE_ACCOUNT_V3),
        key_at(&keys, HOT_CORE_PROGRAM_ACCOUNT_V3),
        key_at(&keys, HOT_CORE_PROGRAMDATA_ACCOUNT_V3),
        key_at(&keys, HOT_TRADING_PROGRAM_ACCOUNT_V3),
        key_at(&keys, HOT_TRADING_PROGRAMDATA_ACCOUNT_V3),
        key_at(&keys, HOT_FIXED_ACCOUNT_COUNT_V3),
    ];
    let build_metas = || {
        let mut metas = outer_keys
            .iter()
            .map(|key| BorrowedAccountMeta {
                pubkey: key,
                is_signer: false,
                is_writable: false,
            })
            .collect::<Vec<_>>();
        metas.extend(
            keys.iter()
                .enumerate()
                .map(|(index, key)| BorrowedAccountMeta {
                    pubkey: key,
                    is_signer: false,
                    is_writable: index == HOT_MARKET_ACCOUNT_V3 || index == HOT_ROOT_ACCOUNT_V3,
                }),
        );
        metas
    };
    let borrowed = [BorrowedInstruction {
        program_id: &registry,
        accounts: build_metas(),
        data: &top_data,
    }];
    let mut instructions_data = construct_instructions_data(&borrowed);
    let instructions_end = instructions_data.len();
    instructions_data
        .get_mut(instructions_end - 2..)
        .expect("current instruction")
        .copy_from_slice(&0_u16.to_le_bytes());

    let mut accounts = keys
        .iter()
        .enumerate()
        .map(|(index, key)| {
            let executable = matches!(
                index,
                HOT_CORE_PROGRAM_ACCOUNT_V3
                    | HOT_TRADING_PROGRAM_ACCOUNT_V3
                    | HOT_REGISTRY_PROGRAM_ACCOUNT_V3
            );
            let signer = index == HOT_FIXED_ACCOUNT_COUNT_V3;
            let writable = index == HOT_MARKET_ACCOUNT_V3 || index == HOT_ROOT_ACCOUNT_V3;
            let account_owner = if index == HOT_FIXED_ACCOUNT_COUNT_V3 {
                system_program::ID
            } else {
                owner
            };
            let data = if index == HOT_ACTIVATION_CACHE_ACCOUNT_V3 {
                activation_bytes.clone()
            } else if index == HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3 {
                instructions_data.clone()
            } else {
                Vec::new()
            };
            info(*key, signer, writable, account_owner, executable, data)
        })
        .collect::<Vec<_>>();

    let authenticated =
        authenticate_hot_invocation_v3(&program_id, &accounts, &hot_bytes, envelope)
            .expect("Registry continuation");
    assert_eq!(
        authenticated.strategy_extras_start,
        HOT_FIXED_ACCOUNT_COUNT_V3 + 1
    );
    assert_eq!(authenticated.native_message_offset_bias, 0);
    assert!(authenticated.permits_fixed_market_union);
    assert_eq!(
        authenticated.role_authentication,
        HotRoleAuthenticationV3::AuthenticatedContinuation
    );
    assert!(HotFrameV3::parse(&program_id, &accounts, false).is_err());
    assert!(HotFrameV3::parse(&program_id, &accounts, true).is_ok());

    let mut substituted_data = top_data.clone();
    *substituted_data.last_mut().expect("family byte") ^= 1;
    let substituted = [BorrowedInstruction {
        program_id: &registry,
        accounts: build_metas(),
        data: &substituted_data,
    }];
    let mut substituted_instructions = construct_instructions_data(&substituted);
    let substituted_end = substituted_instructions.len();
    substituted_instructions
        .get_mut(substituted_end - 2..)
        .expect("substituted current instruction")
        .copy_from_slice(&0_u16.to_le_bytes());
    accounts
        .get(HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3)
        .expect("instructions sysvar slot inside the frame")
        .try_borrow_mut_data()
        .expect("instructions data")
        .copy_from_slice(&substituted_instructions);
    assert!(authenticate_hot_invocation_v3(&program_id, &accounts, &hot_bytes, envelope).is_err());
    accounts
        .get(HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3)
        .expect("instructions sysvar slot inside the frame")
        .try_borrow_mut_data()
        .expect("instructions restore")
        .copy_from_slice(&instructions_data);

    accounts
        .get_mut(HOT_FIXED_ACCOUNT_COUNT_V3)
        .expect("admission slot inside the frame")
        .is_signer = false;
    assert!(authenticate_hot_invocation_v3(&program_id, &accounts, &hot_bytes, envelope).is_err());
}

#[test]
fn lifecycle_v5_quotes_are_derived_only_from_current_rent() {
    use dclutch_vm::account_profile::lifecycle_v3::{
        ACTION_PLAN_BYTES, CURRENT_RENT_QUOTE_BYTES_V5, HEADER_BYTES, PROTECTED_OUTPUT_BYTES,
        RECIPE_BYTES, SEED_BYTES,
        encode::{
            LifecycleAccountCoordinateV3, LifecycleCurrentRentQuoteInputV5, LifecycleGuardInputV3,
            LifecycleOperationInputV3, LifecyclePlanInputV3, LifecycleRecipeInputV3,
            LifecycleRefundSourceInputV3, LifecycleSeedInputV3, encode_lifecycle_policy_v5_atomic,
        },
    };

    const WIDTH: usize = HEADER_BYTES
        + RECIPE_BYTES
        + 2 * SEED_BYTES
        + ACTION_PLAN_BYTES
        + PROTECTED_OUTPUT_BYTES
        + CURRENT_RENT_QUOTE_BYTES_V5;
    let recipes = [LifecycleRecipeInputV3 {
        state: LifecycleAccountCoordinateV3::fixed(0),
        seed_start: 0,
        seed_count: 2,
        bump_offset: 1,
        data_base: 8,
        data_stride: 0,
    }];
    let seeds = [
        LifecycleSeedInputV3::Literal(b"hot-rent-quote-v5"),
        LifecycleSeedInputV3::CanonicalBump,
    ];
    let plans = [LifecyclePlanInputV3 {
        action: 1,
        operation: LifecycleOperationInputV3::Authenticate,
        recipe: 0,
        payer: None,
        rent_credit: None,
        principal: None,
        beneficiary: None,
        refund_source: LifecycleRefundSourceInputV3::Credit,
        guard: LifecycleGuardInputV3::Always,
    }];
    let mut scratch = [0_u8; WIDTH];
    let mut bytes = [0_u8; WIDTH];
    encode_lifecycle_policy_v5_atomic(
        &recipes,
        &seeds,
        &plans,
        &[None],
        &[],
        &[LifecycleCurrentRentQuoteInputV5 {
            exact_data_len: 152,
            scalar_destination: 64,
            action: None,
        }],
        &mut scratch,
        &mut bytes,
    )
    .expect("lifecycle V5 with current-Rent declaration");
    let policy = StateLifecyclePolicyV5::decode_selected([1; 32], [1; 32], &bytes)
        .expect("selected lifecycle V5");
    let rent = Rent::default();
    let quotes = authenticate_current_rent_quotes_v5(policy, &rent, 0)
        .expect("authenticated current Rent quote");
    assert_eq!(quotes.len(), 1);
    let quote = quotes.first().expect("exactly one quote");
    assert_eq!(quote.exact_data_len, 152);
    assert_eq!(quote.scalar_destination, 64);
    assert_eq!(quote.current_minimum, rent.minimum_balance(152));
}

#[test]
fn profile13_zero_spans_expand_aliases_and_downgrade_child_privileges() {
    const READONLY: AccountPrivilegesV2 = AccountPrivilegesV2::new(false, false, false);
    const WRITABLE: AccountPrivilegesV2 = AccountPrivilegesV2::new(false, true, false);
    const NO_EFFECTS: AccountEffectPermissionsV2 =
        AccountEffectPermissionsV2::new(false, false, false);

    let exact = |privileges| AccountRuleWithPrestateInputV2 {
        rule: AccountRuleInputV2 {
            privileges,
            effect_permissions: NO_EFFECTS,
            alias: AccountAliasInputV2::SelfCoordinate,
            data_length: 0,
            data_item_stride: 0,
        },
        prestate: AccountPrestateV2::Exact,
    };
    let alias = AccountRuleWithPrestateInputV2 {
        rule: AccountRuleInputV2 {
            privileges: READONLY,
            effect_permissions: NO_EFFECTS,
            alias: AccountAliasInputV2::Fixed(4),
            data_length: 0,
            data_item_stride: 0,
        },
        prestate: AccountPrestateV2::AuthenticatedRouteAlias,
    };
    let rules = [
        exact(READONLY),
        exact(READONLY),
        exact(READONLY),
        exact(READONLY),
        exact(WRITABLE),
        exact(READONLY),
        alias,
    ];
    let width = DYNAMIC_FIXED_SPAN_HEADER_BYTES
        .checked_add(
            rules
                .len()
                .checked_mul(ACCOUNT_PROFILE_RULE_BYTES)
                .expect("rules"),
        )
        .expect("width");
    let mut scratch = vec![0_u8; width];
    let mut bytes = vec![0_u8; width];
    encode_account_profile_with_dynamic_fixed_span_v2_atomic(
        TrustedEnvironmentV2::None,
        TrustedIdentityEnvironmentV2::None,
        TrustedBuiltinIdentityV2::SystemProgram { destination: 0 },
        &[],
        &rules,
        &[],
        &[],
        RegisterGeometryV2 {
            common_scalars: 0,
            item_scalar_stride: 0,
            common_identities: 1,
            item_identity_stride: 0,
        },
        &mut scratch,
        &mut bytes,
    )
    .expect("profile13 zero spans");
    let profile = AccountProfileV2::decode(&bytes).expect("decode profile13");
    assert_eq!(
        profile.artifact_profile(),
        dclutch_vm::account_profile::v2::DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE
    );
    assert_eq!(profile.dynamic_fixed_span_count(), 0);

    let make_account = |writable| {
        let key = Box::leak(Box::new(Pubkey::new_unique()));
        let owner = Box::leak(Box::new(Pubkey::new_unique()));
        let lamports = Box::leak(Box::new(0_u64));
        let data = Box::leak(Vec::new().into_boxed_slice());
        AccountInfo::new(key, false, writable, lamports, data, owner, false)
    };
    let physical = [
        make_account(false),
        make_account(false),
        make_account(false),
        make_account(false),
        make_account(true),
        make_account(false),
    ];
    let logical = expand_runtime_accounts_v3(
        profile,
        0,
        &[],
        [
            &physical[0],
            &physical[1],
            &physical[2],
            &physical[3],
            &physical[4],
        ],
        &physical[5..],
    )
    .expect("expand physical representatives");
    assert_eq!(logical.len(), 7);
    assert_eq!(
        logical
            .get(4)
            .expect("representative coordinate inside the expanded frame")
            .key,
        logical
            .get(6)
            .expect("alias coordinate inside the expanded frame")
            .key
    );

    let declared =
        child_route_privileges_v3(profile, 0, &[], &logical).expect("declared privileges");
    let child = downgraded_effect_accounts_v3(&logical, &declared).expect("downgrade route views");
    // Coordinate 6 is an authenticated route alias of the writable
    // representative 4. An alias is emitted privilege-free, so it states
    // nothing at all, and a child CPI meta built from the alias would hand
    // the child program a readonly view of an account its own authenticated
    // declaration states as writable.
    assert!(child.view(4).expect("child view").is_writable);
    assert!(child.view(6).expect("child view").is_writable);
    assert_eq!(
        child.view(4).expect("child view").key,
        child.view(6).expect("child view").key
    );

    // A declaration never becomes a writable meta for an account the
    // transaction did not include as writable.
    let withheld = [
        make_account(false),
        make_account(false),
        make_account(false),
        make_account(false),
        make_account(false),
        make_account(false),
    ];
    let withheld_logical = expand_runtime_accounts_v3(
        profile,
        0,
        &[],
        [
            &withheld[0],
            &withheld[1],
            &withheld[2],
            &withheld[3],
            &withheld[4],
        ],
        &withheld[5..],
    )
    .expect("expand physical representatives");
    assert!(child_route_privileges_v3(profile, 0, &[], &withheld_logical).is_err());
    assert!(
        expand_runtime_accounts_v3(
            profile,
            0,
            &[],
            [
                &physical[0],
                &physical[1],
                &physical[2],
                &physical[3],
                &physical[4],
            ],
            &physical[4..],
        )
        .is_err()
    );
}

#[test]
fn fixed_route_alias_of_an_executable_representative_survives_the_child_downgrade() {
    const READONLY: AccountPrivilegesV2 = AccountPrivilegesV2::new(false, false, false);
    const EXECUTABLE: AccountPrivilegesV2 = AccountPrivilegesV2::new(false, false, true);
    const NO_EFFECTS: AccountEffectPermissionsV2 =
        AccountEffectPermissionsV2::new(false, false, false);

    let exact = |privileges| AccountRuleWithPrestateInputV2 {
        rule: AccountRuleInputV2 {
            privileges,
            effect_permissions: NO_EFFECTS,
            alias: AccountAliasInputV2::SelfCoordinate,
            data_length: 0,
            data_item_stride: 0,
        },
        prestate: AccountPrestateV2::Exact,
    };
    // Post-`cc228cd` an authenticated route alias is privilege-free: the
    // representative owns the physical executable fact.
    let alias = AccountRuleWithPrestateInputV2 {
        rule: AccountRuleInputV2 {
            privileges: READONLY,
            effect_permissions: NO_EFFECTS,
            alias: AccountAliasInputV2::Fixed(1),
            data_length: 0,
            data_item_stride: 0,
        },
        prestate: AccountPrestateV2::AuthenticatedRouteAlias,
    };
    let rules = [exact(READONLY), exact(EXECUTABLE), alias];
    let width = AUTHENTICATED_ROUTE_ALIAS_HEADER_BYTES
        .checked_add(
            rules
                .len()
                .checked_mul(ACCOUNT_PROFILE_RULE_BYTES)
                .expect("rules"),
        )
        .expect("width");
    let mut scratch = vec![0_u8; width];
    let mut bytes = vec![0_u8; width];
    encode_account_profile_with_authenticated_route_alias_v2_atomic(
        TrustedEnvironmentV2::None,
        TrustedIdentityEnvironmentV2::None,
        TrustedBuiltinIdentityV2::SystemProgram { destination: 0 },
        &rules,
        &[],
        &[],
        &[],
        RegisterGeometryV2 {
            common_scalars: 0,
            item_scalar_stride: 0,
            common_identities: 1,
            item_identity_stride: 0,
        },
        &mut scratch,
        &mut bytes,
    )
    .expect("authenticated route alias profile");
    let profile = AccountProfileV2::decode(&bytes).expect("decode route alias profile");
    assert!(!profile.uses_dynamic_fixed_spans());

    let account = |executable| {
        let key = Box::leak(Box::new(Pubkey::new_unique()));
        let owner = Box::leak(Box::new(Pubkey::new_unique()));
        let lamports = Box::leak(Box::new(0_u64));
        let data = Box::leak(Vec::new().into_boxed_slice());
        AccountInfo::new(key, false, false, lamports, data, owner, executable)
    };
    let plain = account(false);
    let program = account(true);
    let logical = [&plain, &program, &program];

    let declared =
        child_route_privileges_v3(profile, 0, &[], &logical).expect("declared privileges");
    let child = downgraded_effect_accounts_v3(&logical, &declared)
        .expect("alias of an executable representative downgrades");
    assert!(child.view(1).expect("child view").executable);
    assert!(
        child.view(2).expect("child view").executable,
        "alias lost its physical executability"
    );
    assert_eq!(
        child.view(1).expect("child view").key,
        child.view(2).expect("child view").key
    );
    assert!(
        !child.view(2).expect("child view").is_signer
            && !child.view(2).expect("child view").is_writable
    );

    // The representative's own executable bit is still checked against the
    // physical account, in both directions.
    let hostile = [&plain, &plain, &plain];
    assert!(child_route_privileges_v3(profile, 0, &[], &hostile).is_err());
    let inverted = [&program, &program, &program];
    assert!(child_route_privileges_v3(profile, 0, &[], &inverted).is_err());
}

/// A role callee is resolved by PHYSICAL account, not by coordinate count.
///
/// `d5aed77` pins the precondition this depends on against real emitted
/// profile bytes: for each role program the carrier set has exactly one
/// representative, that representative is a readonly executable, and every
/// other carrier is an alias emitted privilege-free. A layout that split a
/// role's program across two physical accounts would break this resolution
/// as surely as it repairs the aliased one, so both directions are pinned.
#[cfg(any(
    feature = "families",
    feature = "series-family",
    feature = "dealer-family"
))]
#[test]
fn a_role_callee_is_one_physical_account_however_many_frames_name_it() {
    let account = |signer, writable, executable| {
        let key = Box::leak(Box::new(Pubkey::new_unique()));
        let owner = Box::leak(Box::new(Pubkey::new_unique()));
        let lamports = Box::leak(Box::new(0_u64));
        let data = Box::leak(Vec::new().into_boxed_slice());
        AccountInfo::new(key, signer, writable, lamports, data, owner, executable)
    };
    let carrier = account(false, false, true);
    let expected = carrier.key.to_bytes();
    let other = account(false, false, false);
    // Drives the dedup rule directly over hand-built child views: it is the
    // rule this pins, not the profile decoding that produces the views.
    let resolve = |frame: &[AccountInfo<'static>], aliases: &[usize]| {
        resolve_carrier_by_representative_v3(frame.len(), aliases, expected, |index| {
            frame
                .get(index)
                .cloned()
                .ok_or_else(|| ProgramError::from(TradingSbfError::Content))
        })
    };

    // The Series consume shape: three logical coordinates, one physical
    // account, all readonly executable. Coordinates 1 and 3 are aliases of
    // 0, so the representative table maps all three to 0. This refused
    // before the dedup, and it is a layout a frame cannot avoid -- three
    // different child programs each need the callee in their own list.
    let series = [
        carrier.clone(),
        carrier.clone(),
        other.clone(),
        carrier.clone(),
    ];
    let aliases = [0, 0, 2, 0];
    assert_eq!(
        resolve(&series, &aliases)
            .expect("one account named three times resolves")
            .key,
        carrier.key
    );

    // Two DISTINCT physical accounts carrying the role's key: still
    // refused. Same key, but self-representatives at two coordinates, so
    // nothing says which one the CPI is made through.
    let ambiguous = [carrier.clone(), other.clone(), carrier.clone()];
    assert!(resolve(&ambiguous, &[0, 1, 2]).is_err());

    // An aliased carrier that arrived writable or signing is refused even
    // though its representative is clean: the privilege check is per
    // account and runs before the dedup, not after.
    for hostile in [account(false, true, true), account(true, false, true)] {
        let mut copy = hostile.clone();
        copy.key = carrier.key;
        let frame = [carrier.clone(), copy];
        assert!(resolve(&frame, &[0, 0]).is_err());
    }

    // A non-executable carrier is refused, and a role nothing carries has
    // no answer at all.
    let inert = {
        let mut value = other.clone();
        value.key = carrier.key;
        value
    };
    assert!(resolve(&[inert], &[0]).is_err());
    assert!(resolve(core::slice::from_ref(&other), &[0]).is_err());

    // The alias table has to be the one this vector was downgraded at.
    assert!(resolve(&series, &[0, 0, 2]).is_err());
}

#[test]
fn profile13_trailing_transport_span_selects_exact_scratch_pages() {
    const READONLY: AccountPrivilegesV2 = AccountPrivilegesV2::new(false, false, false);
    const NO_EFFECTS: AccountEffectPermissionsV2 =
        AccountEffectPermissionsV2::new(false, false, false);
    let rule = AccountRuleWithPrestateInputV2 {
        rule: AccountRuleInputV2 {
            privileges: READONLY,
            effect_permissions: NO_EFFECTS,
            alias: AccountAliasInputV2::SelfCoordinate,
            data_length: 0,
            data_item_stride: 0,
        },
        prestate: AccountPrestateV2::AuthenticatedOpaqueReadonlyData,
    };
    let fixed_rules = [rule; 5];
    let spans = [DynamicFixedSpanInputV2 {
        insertion_coordinate: 5,
        count_scalar: 0,
        rule_start: 0,
        rule_stride: 1,
        minimum: 1,
        maximum: 4,
        step: 1,
    }];
    let width = DYNAMIC_FIXED_SPAN_HEADER_BYTES
        + ACCOUNT_PROFILE_RULE_BYTES * (fixed_rules.len() + 1)
        + dclutch_vm::account_profile::v2::DYNAMIC_FIXED_SPAN_ENTRY_BYTES;
    let mut scratch = vec![0_u8; width];
    let mut bytes = vec![0_u8; width];
    encode_account_profile_with_dynamic_fixed_span_v2_atomic(
        TrustedEnvironmentV2::None,
        TrustedIdentityEnvironmentV2::None,
        TrustedBuiltinIdentityV2::SystemProgram { destination: 0 },
        &spans,
        &fixed_rules,
        &[rule],
        &[],
        RegisterGeometryV2 {
            common_scalars: 1,
            item_scalar_stride: 0,
            common_identities: 1,
            item_identity_stride: 0,
        },
        &mut scratch,
        &mut bytes,
    )
    .expect("trailing scratch span");
    let profile = AccountProfileV2::decode(&bytes).expect("decode profile13");
    let make_account = || {
        AccountInfo::new(
            Box::leak(Box::new(Pubkey::new_unique())),
            false,
            false,
            Box::leak(Box::new(0_u64)),
            Box::leak(Vec::new().into_boxed_slice()),
            Box::leak(Box::new(Pubkey::new_unique())),
            false,
        )
    };
    let accounts = (0..7).map(|_| make_account()).collect::<Vec<_>>();
    let logical = accounts.iter().collect::<Vec<_>>();
    let pages = authenticated_input_scratch_pages_v3(profile, &[2], Some(0), &logical)
        .expect("exact trailing pages");
    assert_eq!(pages.len(), 2);
    assert_eq!(
        pages.first().expect("first trailing page").key,
        accounts.get(5).expect("page slot inside the frame").key
    );
    assert_eq!(
        pages.get(1).expect("second trailing page").key,
        accounts.get(6).expect("page slot inside the frame").key
    );
    assert!(authenticated_input_scratch_pages_v3(profile, &[2], Some(1), &logical).is_err());
    assert!(authenticated_input_scratch_pages_v3(profile, &[4], Some(0), &logical).is_err());
    assert_eq!(
        lifecycle_semantic_prefix_width_v3(profile, 0, &[2], logical.len())
            .expect("lifecycle receives the exact fixed semantic prefix"),
        fixed_rules.len()
    );
    assert!(lifecycle_semantic_prefix_width_v3(profile, 0, &[2], logical.len() + 1).is_err());
}

#[cfg(any(
    feature = "families",
    feature = "series-family",
    feature = "dealer-family"
))]
#[test]
fn selected_family_profile_links_real_claims_and_custody_routes() {
    let _claims_route = execute_claims_route_v3;
    let _custody_preflight = preflight_custody_route_v3;
    let _custody_route = execute_custody_route_v3;

    assert_ne!(core::mem::size_of::<ClaimsRouteReceiptV3>(), 0);
    assert_ne!(core::mem::size_of::<CustodyCompositionParentV3>(), 0);
}

#[test]
fn only_the_shadow_disposition_refuses_a_slot_declaring_account_profile() {
    // THE WHOLE MATRIX, because a conjunct written as one `match` arm is
    // one typo away from refusing the disposition that ships. General's
    // `OpenBatch` IS a slot-declaring profile on `AdmittedAot`, executes on
    // this path in `general-hot`, and must pass through untouched.
    let slot = TrustedEnvironmentV2::CurrentSlot { destination: 90 };
    for disposition in [
        StrategyDispositionV2::Interpreted,
        StrategyDispositionV2::AdmittedAot,
    ] {
        require_shadow_declares_no_trusted_slot_v1(disposition, slot)
            .expect("only ShadowAot is gated");
        require_shadow_declares_no_trusted_slot_v1(disposition, TrustedEnvironmentV2::None)
            .expect("a profile declaring nothing is admitted everywhere");
    }
    require_shadow_declares_no_trusted_slot_v1(
        StrategyDispositionV2::ShadowAot,
        TrustedEnvironmentV2::None,
    )
    .expect("Series' own pairing, which is what ships");

    // NAMED, not `is_err()`. The sibling gate two hundred lines up refuses
    // a `ShadowAot` naming the wrong transport with `UnsupportedContent`,
    // and a hostile that accepted either code would pass on a defect in the
    // other one.
    assert_eq!(
        require_shadow_declares_no_trusted_slot_v1(StrategyDispositionV2::ShadowAot, slot),
        Err(TradingSbfError::ShadowTrustedEnvironment.into())
    );
    // The destination coordinate is not part of the accusation: ANY
    // declaration refuses, so a family cannot move the slot to a scalar
    // this conjunct happens not to name. That is the exact shape the note
    // called "positional and accidental" about the child-request seeds.
    assert_eq!(
        require_shadow_declares_no_trusted_slot_v1(
            StrategyDispositionV2::ShadowAot,
            TrustedEnvironmentV2::CurrentSlot { destination: 0 },
        ),
        Err(TradingSbfError::ShadowTrustedEnvironment.into())
    );
}

#[test]
fn trusted_current_slot_survives_projection_boundary_and_reaches_transition() {
    let observation = TrustedEnvironmentObservationV3 {
        current_slot: Some((1, 42)),
        current_executing_program: Some((2, [0x91; 32])),
        system_program: Some((0, system_program::ID.to_bytes())),
    };
    let mut projected = [0_u64; 3];
    let mut projected_identities = [[0_u8; 32]; 3];
    seed_trusted_environment_v3(observation, &mut projected, &mut projected_identities)
        .expect("trusted seed");
    require_trusted_environment_v3(observation, &projected, &projected_identities)
        .expect("seed preserved");

    let mut hostile = projected;
    hostile[1] = 41;
    assert_eq!(
        require_trusted_environment_v3(observation, &hostile, &projected_identities),
        Err(TradingSbfError::Content.into())
    );
    let mut hostile_identities = projected_identities;
    hostile_identities[2] = [0x92; 32];
    assert_eq!(
        require_trusted_environment_v3(observation, &projected, &hostile_identities),
        Err(TradingSbfError::Content.into())
    );
    let mut hostile_builtin = projected_identities;
    hostile_builtin[0] = [0x93; 32];
    assert_eq!(
        require_trusted_environment_v3(observation, &projected, &hostile_builtin),
        Err(TradingSbfError::Content.into())
    );

    let width = TRANSITION_HEADER_BYTES_V3 + TRANSITION_INSTRUCTION_BYTES_V3;
    let mut program_scratch = vec![0_u8; width];
    let mut program_bytes = vec![0_u8; width];
    encode_program_atomic(
        ProgramGeometryV3 {
            common_scalars: 3,
            item_scalar_stride: 0,
            common_identities: 0,
            item_identity_stride: 0,
        },
        &[InstructionV3::copy_scalar(
            ScalarRegisterV3::common(1),
            ScalarRegisterV3::common(2),
        )],
        &[],
        &[],
        &mut program_scratch,
        &mut program_bytes,
    )
    .expect("transition program");
    let program = TransitionProgramV3::decode(&program_bytes).expect("transition decode");
    let mut transition_scratch = [0_u64; 3];
    let mut transition_output = [0_u64; 3];
    execute_fold_atomic(
        program,
        0,
        RegisterInput {
            scalars: &projected,
            identities: &[],
        },
        RegisterOutput {
            scalars: &mut transition_scratch,
            identities: &mut [],
        },
        RegisterOutput {
            scalars: &mut transition_output,
            identities: &mut [],
        },
    )
    .expect("transition sees trusted slot");
    assert_eq!(transition_output, [0, 42, 42]);
}

#[test]
fn root_header_and_alias_projection_cannot_be_written() {
    let root_header = ResolvedEffectV3::WriteScalar {
        account: 0,
        offset: u32::try_from(CAPABILITY_ROOT_HEADER_BYTES_V1 - 8).expect("offset"),
        value: 9,
    };
    assert!(require_root_write_is_state_only(root_header, &[0, 1]).is_err());

    let first_state_byte = ResolvedEffectV3::WriteScalar {
        account: 0,
        offset: u32::try_from(CAPABILITY_ROOT_HEADER_BYTES_V1).expect("offset"),
        value: 9,
    };
    assert_eq!(
        require_root_write_is_state_only(first_state_byte, &[0, 1]),
        Ok(())
    );

    let aliased_header = ResolvedEffectV3::WriteIdentity {
        account: 1,
        offset: 0,
        value: [7; 32],
    };
    assert!(require_root_write_is_state_only(aliased_header, &[0, 0]).is_err());

    let ordinary_account = ResolvedEffectV3::WriteIdentity {
        account: 1,
        offset: 0,
        value: [7; 32],
    };
    assert_eq!(
        require_root_write_is_state_only(ordinary_account, &[0, 1]),
        Ok(())
    );

    let narrow_root_header = ResolvedEffectV3::WriteU8 {
        account: 0,
        offset: u32::try_from(CAPABILITY_ROOT_HEADER_BYTES_V1 - 1).expect("offset"),
        value: 1,
    };
    assert!(require_root_write_is_state_only(narrow_root_header, &[0, 1]).is_err());
}

#[test]
fn typed_writes_initialize_zeroed_lifecycle_state_exactly() {
    assert_eq!(
        commit_data_effect(ResolvedEffectV3::Noop, &[], &[], false),
        Ok(false),
        "disabled conditional write reached an account or commit pass"
    );
    let root_key = Box::leak(Box::new(Pubkey::new_unique()));
    let state_key = Box::leak(Box::new(Pubkey::new_unique()));
    let owner = Box::leak(Box::new(Pubkey::new_unique()));
    let root_lamports = Box::leak(Box::new(1_u64));
    let state_lamports = Box::leak(Box::new(1_u64));
    let root_data = Box::leak(Vec::new().into_boxed_slice());
    let state_data = Box::leak(vec![0_u8; 16].into_boxed_slice());
    let root = AccountInfo::new(
        root_key,
        false,
        true,
        root_lamports,
        root_data,
        owner,
        false,
    );
    let state = AccountInfo::new(
        state_key,
        false,
        true,
        state_lamports,
        state_data,
        owner,
        false,
    );
    let accounts = [&root, &state];
    let aliases = [0, 1];
    for effect in [
        ResolvedEffectV3::WriteU8 {
            account: 1,
            offset: 0,
            value: 0xa1,
        },
        ResolvedEffectV3::WriteU16 {
            account: 1,
            offset: 1,
            value: 0xb2c3,
        },
        ResolvedEffectV3::WriteU32 {
            account: 1,
            offset: 3,
            value: 0xd4e5_f607,
        },
    ] {
        assert!(!commit_data_effect(effect, &accounts, &aliases, false).expect("typed write"));
    }
    let data = state.try_borrow_data().expect("state data");
    assert_eq!(data.first(), Some(&0xa1));
    assert_eq!(data.get(1..3), Some(0xb2c3_u16.to_le_bytes().as_slice()));
    assert_eq!(
        data.get(3..7),
        Some(0xd4e5_f607_u32.to_le_bytes().as_slice())
    );
    assert!(data.get(7..).expect("tail").iter().all(|byte| *byte == 0));
}

/// One authored Effect artifact whose writes straddle the commit-last
/// boundary: two fixed writes (one non-root, one root) and one per-item
/// write whose first item aliases onto the root coordinate.
///
/// The whole point of building a real artifact rather than hand-made
/// [`ResolvedEffectV3`] values is that the commit passes RESOLVE — their
/// ordinal walk, not just `commit_data_effect`, is what the two share and
/// what the commit-last ordering depends on.
struct CommitLastFixtureV1 {
    artifact: Vec<u8>,
    scalars: [u64; 5],
    root_data_offset: u32,
    item_data_offset: u32,
    tail_count: u32,
}

fn commit_last_fixture_v1() -> CommitLastFixtureV1 {
    use dclutch_vm::effect::{
        v3::{
            HEADER_BYTES, OPERATION_BYTES,
            encode::{
                AccountCoordinateV3, EffectGeometryV3, EffectInstructionV3, ScalarCoordinateV3,
                encode_effect_program_v3_atomic,
            },
        },
        v4::{BorrowedRangePolicyV4, HEADER_BYTES_V4, encode_program_v4_atomic},
    };
    const ROOT_DATA_OFFSET: u32 = 40;
    const ITEM_DATA_OFFSET: u32 = 32;
    let fixed = [
        // ordinal 0: non-root, committed by the first pass.
        EffectInstructionV3::write_u64(
            AccountCoordinateV3::fixed(1),
            0,
            ScalarCoordinateV3::common(0),
        ),
        // ordinal 1: root, committed by the second pass only.
        EffectInstructionV3::write_u64(
            AccountCoordinateV3::fixed(0),
            ROOT_DATA_OFFSET,
            ScalarCoordinateV3::common(1),
        ),
    ];
    // ordinals 2..5: one per item. Item 0's account aliases onto the root,
    // so exactly one ITEM ordinal also belongs to the second pass.
    let item = [EffectInstructionV3::write_u64(
        AccountCoordinateV3::item(0),
        ITEM_DATA_OFFSET,
        ScalarCoordinateV3::item(0),
    )];
    let base_bytes = HEADER_BYTES + (fixed.len() + item.len()) * OPERATION_BYTES;
    let mut base_scratch = vec![0_u8; base_bytes];
    let mut base = vec![0_u8; base_bytes];
    encode_effect_program_v3_atomic(
        EffectGeometryV3 {
            fixed_accounts: 2,
            item_account_stride: 1,
            common_scalars: 2,
            item_scalar_stride: 1,
            common_identities: 0,
            item_identity_stride: 0,
        },
        &[],
        &fixed,
        &item,
        &mut base_scratch,
        &mut base,
    )
    .expect("commit-last base program");
    let mut scratch = vec![0_u8; HEADER_BYTES_V4 + base_bytes];
    let mut artifact = vec![0_u8; HEADER_BYTES_V4 + base_bytes];
    encode_program_v4_atomic(
        &base,
        BorrowedRangePolicyV4::DisjointExactCoverage,
        1,
        &[],
        &[],
        &mut scratch,
        &mut artifact,
    )
    .expect("commit-last successor");
    CommitLastFixtureV1 {
        artifact,
        scalars: [0xa1a1_a1a1, 0xb2b2_b2b2, 0xc001, 0xc002, 0xc003],
        root_data_offset: ROOT_DATA_OFFSET,
        item_data_offset: ITEM_DATA_OFFSET,
        tail_count: 3,
    }
}

fn commit_last_account_v1(key: &'static Pubkey, lamports: u64) -> AccountInfo<'static> {
    AccountInfo::new(
        key,
        false,
        true,
        Box::leak(Box::new(lamports)),
        Box::leak(vec![0_u8; 64].into_boxed_slice()),
        Box::leak(Box::new(Pubkey::new_unique())),
        false,
    )
}

/// The frame's carving and the transport's chunking are TWO authors.
///
/// Until `require_admitted_bank_matches_frame_v3` the only guard on this
/// path was `execute_admitted_aot_v3`'s `scalar_count !=
/// context.scalar_count`, and both sides of it are `view.scalars.len()` --
/// the context field is assigned from it twenty lines before the comparison.
/// It could not fail for any input, which is exactly why it read as the
/// transport check the path did not have.
#[test]
fn an_admitted_bank_must_be_the_width_its_account_frame_was_carved_for() {
    let transport = || ProgramError::from(TradingSbfError::AdmittedTransport);
    require_admitted_bank_matches_frame_v3((12, 5), (12, 5)).expect("exact agreement");
    // Zero-width banks agree with a zero-width carving and nothing else.
    require_admitted_bank_matches_frame_v3((0, 0), (0, 0)).expect("empty agreement");
    for (bank, frame, why) in [
        ((13, 5), (12, 5), "a scalar bank wider than its carving"),
        ((11, 5), (12, 5), "a scalar bank narrower than its carving"),
        ((12, 6), (12, 5), "an identity bank wider than its carving"),
        (
            (12, 4),
            (12, 5),
            "an identity bank narrower than its carving",
        ),
        ((0, 0), (12, 5), "an empty bank against a carved frame"),
        ((12, 5), (0, 0), "a carved bank against an empty frame"),
        // The transposition, which a comparison of TOTALS would admit and
        // which carves a different number of caller authorities.
        ((5, 12), (12, 5), "a transposed pair"),
    ] {
        assert_eq!(
            require_admitted_bank_matches_frame_v3(bank, frame),
            Err(transport()),
            "{why} must refuse",
        );
    }
}

/// Wall A still stands for every registered action without a planner.
///
/// The wall was never a gate: `prepare_direct_hot_crosscheck_v3` refuses an
/// action it has no second opinion about, because a crosscheck that cannot
/// check an action must refuse it. Two actions acquired one on 2026-09-01
/// and the other eleven did not, so the list is CLOSED and this drives it.
///
/// It replaces a chain campaign that paid a whole Direct execution to
/// observe one refusal and could only ever name the single action it
/// submitted. This names all thirteen.
#[test]
fn wall_a_still_stands_for_every_registered_action_without_a_planner() {
    use dclutch_trading::execution_v3::DirectExecutionActionV3 as Action;

    for action in [
        Action::InlineOrdinary,
        Action::RegisterSell,
        Action::RegisterBuy,
    ] {
        assert!(
            direct_action_crosschecks_against_config_v3(action as u32),
            "{action:?} has a planner and must reach it",
        );
    }
    // Every other Direct action. A planner arriving for one of these must
    // DELETE its line here, which is the edit that makes the new coverage
    // visible instead of letting a list quietly go stale.
    for action in [
        Action::FillRegisteredOrdinary,
        Action::SplitRegistered,
        Action::MergeRegistered,
        Action::CancelRegistered,
        Action::ExpireRegistered,
        Action::CloseInvalidated,
        Action::CancelThrough,
        Action::CloseMakerReplay,
        Action::CloseDirectRoot,
        Action::SplitInline,
        Action::MergeInline,
    ] {
        assert!(
            !direct_action_crosschecks_against_config_v3(action as u32),
            "{action:?} would reach a crosscheck that has no opinion about it",
        );
    }
}

/// The four arms of the commit's lamport authority, driven directly.
#[test]
fn the_commit_owns_a_planned_lamport_and_never_a_child_deposited_one() {
    let none = CoordinateParticipationV3::default();
    let mut local = CoordinateParticipationV3::default();
    local.mark_local_mutation();
    let mut child = CoordinateParticipationV3::default();
    child.mark_child_reach();
    let mut both = CoordinateParticipationV3::default();
    both.mark_local_mutation();
    both.mark_child_reach();

    // The plan applies where the Effect declared the movement, and the
    // declaration is what decides it -- not whether the value differs.
    assert_eq!(
        committed_lamports_v3(500, 100, local),
        CommittedLamportsV3::Apply
    );
    assert_eq!(
        committed_lamports_v3(500, 500, local),
        CommittedLamportsV3::Apply
    );
    // A permitted local/child overlap keeps exactly the authority it had.
    assert_eq!(
        committed_lamports_v3(500, 100, both),
        CommittedLamportsV3::Apply
    );
    // Undeclared and unmoved: nothing to do, and this is the overwhelming
    // majority of every frame.
    assert_eq!(
        committed_lamports_v3(100, 100, none),
        CommittedLamportsV3::Settled
    );
    assert_eq!(
        committed_lamports_v3(100, 100, child),
        CommittedLamportsV3::Settled
    );
    // THE REPAIR. 2,895,360 is the exact rent a Custody replay needs; the
    // plan says the zero it observed before the child ran.
    assert_eq!(
        committed_lamports_v3(0, 2_895_360, child),
        CommittedLamportsV3::ChildPoststate
    );
    // And the guard that used to be an unbalanced-instruction abort with no
    // name on it. No local plan, no declared child: nothing explains this.
    assert_eq!(
        committed_lamports_v3(0, 2_895_360, none),
        CommittedLamportsV3::Unexplained
    );
    assert_eq!(
        committed_lamports_v3(2_895_360, 0, none),
        CommittedLamportsV3::Unexplained
    );
}

/// The child-reach mark is written by the walk that proves disjointness.
///
/// Not a separate pass over the same windows: one walk, so the fact the
/// commit relies on and the fact that makes relying on it sound cannot be
/// looking at different coordinates.
#[test]
fn the_disjointness_walk_records_every_coordinate_the_child_reaches() {
    let request = fractional_wrap_request();
    let invocation = fractional_wrap_invocation(request.len());
    let start = usize::from(invocation.fixed_account_start);
    let count = usize::from(invocation.fixed_account_count);
    let logical_count = start + count;
    let aliases = (0..logical_count).collect::<Vec<_>>();
    let mut participation = vec![CoordinateParticipationV3::default(); logical_count];

    record_child_reach_and_require_disjoint_from_local(
        invocation,
        &aliases,
        &mut participation,
        AllowedLocalOverlapV3::None,
    )
    .expect("no local mutation to collide with");

    for (coordinate, slot) in participation.iter().enumerate() {
        let inside = (start..logical_count).contains(&coordinate);
        assert_eq!(
            slot.child_reached(),
            inside,
            "coordinate {coordinate} child-reach mark disagrees with the invocation window",
        );
        assert!(!slot.locally_mutated(), "the walk must mark nothing else");
    }
    // A coordinate OUTSIDE every child window stays unmarked, which is what
    // makes `Unexplained` reachable rather than decorative.
    assert!(start > 0 && !participation[0].child_reached());
}

/// The commit-last split is a REAL ordering, and the second pass really
/// writes.
///
/// `commit_prepared_post_children_v3` commits every non-root coordinate and
/// then the root, so a Market's own state lands only after every other
/// account has been committed. Until this test nothing in the tree executed
/// the second pass at all: the only path that reaches it is a complete Hot
/// execution past three child CPIs. Anything that changes how the second
/// pass selects its work — a recorded plan, a narrower sweep — is refutable
/// here instead of landing silently green.
#[test]
fn commit_last_writes_the_root_only_after_every_other_coordinate() {
    let fixture = commit_last_fixture_v1();
    let effect = decode_selected_effect_v4(EFFECT_SCHEMA_ID_V4, &fixture.artifact)
        .expect("selected commit-last effect");
    assert_eq!(effect.fixed_operation_count(), 2);
    assert_eq!(effect.item_operation_count(), 1);

    let rent = Rent::default();
    let exempt = rent.minimum_balance(64);
    let root = commit_last_account_v1(Box::leak(Box::new(Pubkey::new_unique())), exempt);
    let state = commit_last_account_v1(Box::leak(Box::new(Pubkey::new_unique())), exempt);
    let item_one = commit_last_account_v1(Box::leak(Box::new(Pubkey::new_unique())), exempt);
    let item_two = commit_last_account_v1(Box::leak(Box::new(Pubkey::new_unique())), exempt);
    let aliased = commit_last_account_v1(Box::leak(Box::new(Pubkey::new_unique())), exempt);
    // Coordinate 2 (item 0's account) aliases onto the root coordinate.
    let accounts = [&root, &state, &aliased, &item_one, &item_two];
    let aliases = [0_usize, 1, 0, 3, 4];
    let output_lamports = [exempt + 1_000, exempt + 500, exempt, exempt, exempt];

    let plan = commit_non_root_effects_v3(
        effect,
        fixture.tail_count,
        &fixture.scalars,
        &[],
        &accounts,
        &aliases,
        &output_lamports,
        None,
    )
    .expect("non-root commit pass");
    // Two of the five ordinals belong to the commit-last pass: the root's
    // own fixed write, and item 0's write through the aliased coordinate.
    assert_eq!(plan.ordinals, 5);
    assert_eq!(plan.bits, [0b0000_0110]);

    let root_offset = usize::try_from(fixture.root_data_offset).expect("root offset");
    let item_offset = usize::try_from(fixture.item_data_offset).expect("item offset");
    assert!(
        root.try_borrow_data()
            .expect("root data")
            .iter()
            .all(|byte| *byte == 0),
        "the first pass must not write the root coordinate"
    );
    assert_eq!(root.lamports(), exempt, "root lamports commit last too");
    assert_eq!(
        state.try_borrow_data().expect("state data").get(..8),
        Some(fixture.scalars[0].to_le_bytes().as_slice())
    );
    assert_eq!(state.lamports(), exempt + 500);
    for (account, scalar) in [
        (&item_one, fixture.scalars[3]),
        (&item_two, fixture.scalars[4]),
    ] {
        assert_eq!(
            account
                .try_borrow_data()
                .expect("item data")
                .get(item_offset..item_offset + 8),
            Some(scalar.to_le_bytes().as_slice())
        );
    }

    commit_root_effects_v3(
        effect,
        fixture.tail_count,
        &fixture.scalars,
        &[],
        &accounts,
        &aliases,
        &output_lamports,
        None,
        &plan,
    )
    .expect("root commit pass");
    {
        let data = root.try_borrow_data().expect("root data");
        assert_eq!(
            data.get(root_offset..root_offset + 8),
            Some(fixture.scalars[1].to_le_bytes().as_slice()),
            "the second pass writes the root's fixed effect"
        );
        assert_eq!(
            data.get(item_offset..item_offset + 8),
            Some(fixture.scalars[2].to_le_bytes().as_slice()),
            "the second pass writes the item effect that aliases onto the root"
        );
    }
    assert_eq!(root.lamports(), exempt + 1_000);
    // Nothing the first pass committed moves when the second pass runs.
    assert_eq!(state.lamports(), exempt + 500);
    assert_eq!(
        state.try_borrow_data().expect("state data").get(..8),
        Some(fixture.scalars[0].to_le_bytes().as_slice())
    );
}

/// A root coordinate left below its rent floor is refused by the pass that
/// owns it, not by the one that ran first.
#[test]
fn commit_last_refuses_a_root_left_below_the_rent_floor() {
    let fixture = commit_last_fixture_v1();
    let effect = decode_selected_effect_v4(EFFECT_SCHEMA_ID_V4, &fixture.artifact)
        .expect("selected commit-last effect");
    let rent = Rent::default();
    let exempt = rent.minimum_balance(64);
    let root = commit_last_account_v1(Box::leak(Box::new(Pubkey::new_unique())), exempt);
    let state = commit_last_account_v1(Box::leak(Box::new(Pubkey::new_unique())), exempt);
    let aliased = commit_last_account_v1(Box::leak(Box::new(Pubkey::new_unique())), exempt);
    let item_one = commit_last_account_v1(Box::leak(Box::new(Pubkey::new_unique())), exempt);
    let item_two = commit_last_account_v1(Box::leak(Box::new(Pubkey::new_unique())), exempt);
    let accounts = [&root, &state, &aliased, &item_one, &item_two];
    let aliases = [0_usize, 1, 0, 3, 4];
    // A ROOT ONE LAMPORT UNDER TODAY'S MINIMUM IS NOT WHAT THIS REFUSES.
    // `require_committed_accounts_persist_v3` stopped pricing a committed
    // account at the rate of the moment (`a4b2cbb17`): the runtime already
    // refuses any transaction that leaves a writable account partially
    // rented, so the postcondition this commit is answerable for is that it
    // drained nothing it wrote. A root funded when a byte cost less is a
    // root the runtime grandfathers, and it commits.
    let stranded = [exempt - 1, exempt, exempt, exempt, exempt];
    let stranded_plan = commit_non_root_effects_v3(
        effect,
        fixture.tail_count,
        &fixture.scalars,
        &[],
        &accounts,
        &aliases,
        &stranded,
        None,
    )
    .expect("non-root commit pass over a repriced root");
    commit_root_effects_v3(
        effect,
        fixture.tail_count,
        &fixture.scalars,
        &[],
        &accounts,
        &aliases,
        &stranded,
        None,
        &stranded_plan,
    )
    .expect("a root the cluster funded at a cheaper rate still commits");

    // THE HOSTILE is a root this commit DRAINED: residue the runtime reaps,
    // and the one case it has not already decided.
    let output_lamports = [0, exempt, exempt, exempt, exempt];
    let plan = commit_non_root_effects_v3(
        effect,
        fixture.tail_count,
        &fixture.scalars,
        &[],
        &accounts,
        &aliases,
        &output_lamports,
        None,
    )
    .expect("non-root commit pass");
    assert_eq!(
        commit_root_effects_v3(
            effect,
            fixture.tail_count,
            &fixture.scalars,
            &[],
            &accounts,
            &aliases,
            &output_lamports,
            None,
            &plan,
        ),
        Err(TradingSbfError::Commit.into())
    );
}

/// A plan recorded against a different geometry is refused, not replayed.
///
/// The ordinal space is a function of `tail_count`, so a plan is only
/// meaningful for the execution that recorded it. Without this the second
/// pass would silently decode ordinals into the wrong item indices.
#[test]
fn commit_last_refuses_a_plan_recorded_for_another_geometry() {
    let fixture = commit_last_fixture_v1();
    let effect = decode_selected_effect_v4(EFFECT_SCHEMA_ID_V4, &fixture.artifact)
        .expect("selected commit-last effect");
    let rent = Rent::default();
    let exempt = rent.minimum_balance(64);
    let root = commit_last_account_v1(Box::leak(Box::new(Pubkey::new_unique())), exempt);
    let state = commit_last_account_v1(Box::leak(Box::new(Pubkey::new_unique())), exempt);
    let aliased = commit_last_account_v1(Box::leak(Box::new(Pubkey::new_unique())), exempt);
    let accounts = [&root, &state, &aliased];
    let aliases = [0_usize, 1, 0];
    let output_lamports = [exempt, exempt, exempt];
    let plan = commit_non_root_effects_v3(
        effect,
        1,
        &fixture.scalars[..3],
        &[],
        &accounts,
        &aliases,
        &output_lamports,
        None,
    )
    .expect("non-root commit pass at tail one");
    assert_eq!(plan.ordinals, 3);
    assert_eq!(
        commit_root_effects_v3(
            effect,
            fixture.tail_count,
            &fixture.scalars,
            &[],
            &accounts,
            &aliases,
            &output_lamports,
            None,
            &plan,
        ),
        Err(TradingSbfError::Commit.into())
    );
}

#[test]
fn lifecycle_candidate_updates_every_alias_and_reserves_one_state_once() {
    let plan = PreparedLifecycleInvocationV3 {
        plan: StateLifecyclePlanV3::Create(CreateStatePlanV3 {
            state: [1; 32],
            payer: [2; 32],
            rent_credit: [3; 32],
            beneficiary: [4; 32],
            refund_source: LifecycleRefundSourceV3::Credit,
            target_data_bytes: 144,
            historical_rent_principal: 30,
            state_before: 5,
            state_after: 30,
            payer_debit: 25,
            payer_after: 75,
            bump: 9,
        }),
        state: 1,
        payer: Some(2),
        rent_credit: Some(4),
        seeds: Vec::new(),
        immutable_identity_bindings: Vec::new(),
    };
    let aliases = [0, 1, 2, 1, 4];
    let mut accounts = vec![
        AccountInput {
            lamports: 1,
            data_len: 8,
        };
        aliases.len()
    ];
    apply_lifecycle_candidates_v3(&[plan], &aliases, &mut accounts).expect("candidate applies");
    let state_after = accounts.get(1).expect("state candidate");
    assert_eq!(state_after.lamports, 30);
    assert_eq!(state_after.data_len, 144);
    assert_eq!(accounts.get(3), Some(state_after));
    assert_eq!(accounts.get(2).map(|account| account.lamports), Some(75));

    let mut used = [false; 3];
    assert_eq!(reserve_lifecycle_state_v3(0, &mut used), Ok(()));
    assert_eq!(
        reserve_lifecycle_state_v3(0, &mut used),
        Err(TradingSbfError::Content.into())
    );
    assert_eq!(reserve_lifecycle_state_v3(1, &mut used), Ok(()));
    assert_eq!(
        reserve_lifecycle_state_v3(1, &mut used),
        Err(TradingSbfError::Content.into())
    );
}

fn root_close_plan_v3() -> PreparedLifecycleInvocationV3 {
    PreparedLifecycleInvocationV3 {
        plan: StateLifecyclePlanV3::Close(CloseStatePlanV3 {
            state: [1; 32],
            rent_credit: [2; 32],
            beneficiary: [3; 32],
            refund_source: LifecycleRefundSourceV3::Credit,
            source_data_bytes: 144,
            historical_rent_principal: 30,
            source_before: 37,
            source_after: 0,
            rent_credit_before: 100,
            rent_credit_after: 137,
            bump: 9,
        }),
        state: 0,
        payer: None,
        rent_credit: Some(2),
        seeds: Vec::new(),
        immutable_identity_bindings: Vec::new(),
    }
}

#[test]
fn root_lifecycle_close_is_the_only_root_plan_and_projects_vacancy() {
    assert_eq!(
        selected_root_lifecycle_close_v3(&[root_close_plan_v3()]),
        Ok(true)
    );

    let aliases = [0, 0, 2];
    let mut accounts = vec![
        AccountInput {
            lamports: 37,
            data_len: 144,
        },
        AccountInput {
            lamports: 37,
            data_len: 144,
        },
        AccountInput {
            lamports: 100,
            data_len: 96,
        },
    ];
    apply_lifecycle_candidates_v3(&[root_close_plan_v3()], &aliases, &mut accounts)
        .expect("root close candidate");
    for coordinate in [0_usize, 1] {
        assert_eq!(
            accounts.get(coordinate),
            Some(&AccountInput {
                lamports: 0,
                data_len: 0,
            })
        );
    }
    assert_eq!(accounts.get(2).map(|account| account.lamports), Some(137));

    let mut create = root_close_plan_v3();
    create.plan = StateLifecyclePlanV3::Create(CreateStatePlanV3 {
        state: [1; 32],
        payer: [4; 32],
        rent_credit: [2; 32],
        beneficiary: [3; 32],
        refund_source: LifecycleRefundSourceV3::Credit,
        target_data_bytes: 144,
        historical_rent_principal: 30,
        state_before: 0,
        state_after: 30,
        payer_debit: 30,
        payer_after: 70,
        bump: 9,
    });
    assert_eq!(
        selected_root_lifecycle_close_v3(&[create]),
        Err(TradingSbfError::Transition.into())
    );
    assert_eq!(
        selected_root_lifecycle_close_v3(&[root_close_plan_v3(), root_close_plan_v3()]),
        Err(TradingSbfError::Transition.into())
    );
}

#[test]
fn root_lifecycle_close_refuses_effect_and_child_alias_overlap() {
    let aliases = [0_usize, 1, 0, 3];
    assert_eq!(
        require_no_root_local_mutation_v3(
            ResolvedEffectV3::WriteU8 {
                account: 2,
                offset: u32::try_from(CAPABILITY_ROOT_HEADER_BYTES_V1).expect("offset"),
                value: 1,
            },
            &aliases,
        ),
        Err(TradingSbfError::Transition.into())
    );
    assert_eq!(
        require_no_root_local_mutation_v3(
            ResolvedEffectV3::TransferLamports {
                source: 1,
                destination: 0,
                amount: 1,
            },
            &aliases,
        ),
        Err(TradingSbfError::Transition.into())
    );
    assert_eq!(
        require_no_root_local_mutation_v3(
            ResolvedEffectV3::RequireLamportsEq {
                account: 0,
                value: 37,
            },
            &aliases,
        ),
        Ok(())
    );
    assert_eq!(
        require_window_excludes_root_v3(1, 3, &aliases),
        Err(TradingSbfError::Transition.into())
    );
    assert_eq!(require_window_excludes_root_v3(3, 4, &aliases), Ok(()));
}

#[test]
fn vacant_root_digest_binds_address_owner_balance_and_width() {
    let root = Pubkey::new_unique();
    let digest = vacant_root_poststate_digest_v3(&root);
    let zero = 0_u64.to_le_bytes();
    assert_eq!(
        digest,
        hashv(&[
            VACANT_ROOT_POSTSTATE_DOMAIN_V3,
            root.as_ref(),
            system_program::ID.as_ref(),
            &zero,
            &zero,
        ])
        .to_bytes()
    );
    assert_ne!(
        digest,
        vacant_root_poststate_digest_v3(&Pubkey::new_unique())
    );
    assert_ne!(digest, hash(&[]).to_bytes());
}

#[test]
fn lifecycle_immutable_binding_requires_one_exact_typed_write() {
    let binding = PreparedImmutableIdentityBindingV4 {
        data_offset: 16,
        canonical: [0x63; 32],
    };
    let aliases = [0, 1, 1];
    assert_eq!(
        inspect_lifecycle_binding_effect_v4(
            1,
            &binding,
            ResolvedEffectV3::WriteIdentity {
                account: 2,
                offset: 16,
                value: binding.canonical,
            },
            &aliases,
        ),
        Ok(true)
    );
    assert_eq!(
        inspect_lifecycle_binding_effect_v4(
            1,
            &binding,
            ResolvedEffectV3::WriteIdentity {
                account: 1,
                offset: 16,
                value: [0x64; 32],
            },
            &aliases,
        ),
        Err(TradingSbfError::Transition.into())
    );
    assert_eq!(
        inspect_lifecycle_binding_effect_v4(
            1,
            &binding,
            ResolvedEffectV3::WriteU32 {
                account: 1,
                offset: 44,
                value: 0,
            },
            &aliases,
        ),
        Err(TradingSbfError::Transition.into())
    );
    assert_eq!(
        inspect_lifecycle_binding_effect_v4(
            1,
            &binding,
            ResolvedEffectV3::WriteIdentity {
                account: 0,
                offset: 16,
                value: binding.canonical,
            },
            &aliases,
        ),
        Ok(false)
    );
}

fn preplanned_invocation(state: usize, seed: u8, canonical: u8) -> PreparedLifecycleInvocationV3 {
    PreparedLifecycleInvocationV3 {
        plan: StateLifecyclePlanV3::Authenticate(AuthenticateStatePlanV3 {
            state: [seed; 32],
            data_bytes: 64,
            lamports: 1,
            bump: 254,
        }),
        state,
        payer: None,
        rent_credit: None,
        seeds: alloc::vec![alloc::vec![seed, seed], alloc::vec![254]],
        immutable_identity_bindings: alloc::vec![PreparedImmutableIdentityBindingV4 {
            data_offset: 16,
            canonical: [canonical; 32],
        }],
    }
}

/// A replan that materializes a different seed byte refuses at that seed,
/// before it can reach a derivation it is no longer entitled to reuse.
#[test]
fn a_replan_seed_that_differs_from_the_preplan_refuses() {
    let prior = preplanned_invocation(1, 0x11, 0x63);
    let mut seeds = LifecycleSeedsV4::new(Some(prior.seeds.as_slice()), 2).expect("verify");
    assert!(seeds.push(&[0x11, 0x11]).is_ok());
    let mut diverged = LifecycleSeedsV4::new(Some(prior.seeds.as_slice()), 2).expect("verify");
    assert!(diverged.push(&[0x11, 0x12]).is_err());
    // A seed of the right bytes but the wrong width is not the same seed.
    let mut short = LifecycleSeedsV4::new(Some(prior.seeds.as_slice()), 2).expect("verify");
    assert!(short.push(&[0x11]).is_err());
    // And a table of a different seed width never opens at all.
    assert!(LifecycleSeedsV4::new(Some(prior.seeds.as_slice()), 3).is_err());
}

/// The replan reuses the preplan's bump rather than deriving it, and takes
/// it from the preplan's own final seed.
#[test]
fn the_replan_reuses_the_preplan_bump_and_refuses_a_malformed_one() {
    let prior = preplanned_invocation(1, 0x11, 0x63);
    let mut seeds = LifecycleSeedsV4::new(Some(prior.seeds.as_slice()), 2).expect("verify");
    seeds.push(&[0x11, 0x11]).expect("first seed agrees");
    let program = Pubkey::new_from_array([0x77; 32]);
    assert!(matches!(
        seeds.pending_bump(&program, 0).expect("reused bump"),
        LifecycleCanonicalBumpV4::Reused { bump: 254 }
    ));
    // A preplan whose final seed is not a single bump byte is not a bump.
    let malformed = PreparedLifecycleInvocationV3 {
        seeds: alloc::vec![alloc::vec![0x11, 0x11], alloc::vec![254, 254]],
        ..preplanned_invocation(1, 0x11, 0x63)
    };
    let mut seeds = LifecycleSeedsV4::new(Some(malformed.seeds.as_slice()), 2).expect("verify");
    seeds.push(&[0x11, 0x11]).expect("first seed agrees");
    assert!(seeds.pending_bump(&program, 0).is_err());
}

/// The preplan reproduces the caller's mined bump instead of searching, and
/// a wrong one names an account this execution was not handed.
///
/// A lifecycle-created state is the hardest address on the route to store a
/// bump for -- it does not exist yet -- and the easiest to mine off chain,
/// because its seeds come from registers the caller already computed. The
/// refusal is `prepare_lifecycle_v4`'s equality against the state
/// coordinate; here the property that equality rests on is what is pinned:
/// a wrong hint never lands on the canonical address.
#[test]
fn a_wrong_lifecycle_bump_hint_reproduces_another_address_and_refuses() {
    let program = Pubkey::new_from_array([0x77; 32]);
    let collect = || {
        let mut seeds = LifecycleSeedsV4::new(None, 2).expect("collect");
        seeds.push(&[0x11, 0x11]).expect("collected");
        seeds
    };
    let LifecycleCanonicalBumpV4::Derived {
        address: canonical_address,
        bump: canonical,
    } = collect().pending_bump(&program, 0).expect("searched")
    else {
        panic!("the collecting pass derives");
    };
    // Mined by the caller: the same address, with no search behind it.
    assert!(matches!(
        collect().pending_bump(&program, canonical).expect("hinted"),
        LifecycleCanonicalBumpV4::Derived { address, bump }
            if address == canonical_address && bump == canonical
    ));
    let mut refused = 0_u32;
    for hint in 1..=u8::MAX {
        if hint == canonical {
            continue;
        }
        match collect().pending_bump(&program, hint) {
            Ok(LifecycleCanonicalBumpV4::Derived { address, .. }) => {
                assert_ne!(address, canonical_address, "hint {hint}");
            }
            Ok(LifecycleCanonicalBumpV4::Reused { .. }) => {
                panic!("a collecting pass never reuses");
            }
            Err(_) => {}
        }
        refused = refused.saturating_add(1);
    }
    assert_eq!(refused, 254);

    // The REPLAN still ignores the hint outright: it reuses the preplan's
    // own final seed, so a hint cannot make the two passes disagree.
    let prior = preplanned_invocation(1, 0x11, 0x63);
    for hint in [0, 1, canonical, u8::MAX] {
        let mut seeds = LifecycleSeedsV4::new(Some(prior.seeds.as_slice()), 2).expect("verify");
        seeds.push(&[0x11, 0x11]).expect("first seed agrees");
        assert!(matches!(
            seeds.pending_bump(&program, hint).expect("reused"),
            LifecycleCanonicalBumpV4::Reused { bump: 254 }
        ));
    }
}

/// The preplan derives the bump for real; the two modes are not
/// interchangeable in either direction.
#[test]
fn the_preplan_derives_and_never_borrows_a_verified_answer() {
    let mut seeds = LifecycleSeedsV4::new(None, 2).expect("collect");
    seeds.push(&[0x11, 0x11]).expect("collected");
    let program = Pubkey::new_from_array([0x77; 32]);
    assert!(matches!(
        seeds.pending_bump(&program, 0).expect("derived"),
        LifecycleCanonicalBumpV4::Derived { .. }
    ));
    // A collecting cursor is never "exhausted", and a verifying one never
    // yields a collected vector: the two modes cannot be confused silently.
    assert!(seeds.exhausted().is_err());
    let prior = preplanned_invocation(1, 0x11, 0x63);
    let verifying = LifecycleSeedsV4::new(Some(prior.seeds.as_slice()), 2).expect("verify");
    assert!(verifying.collected().is_err());
}

/// Every difference the replan can produce in one invocation refuses, and
/// the plan table it agrees with is never a duplicate it allocated.
#[test]
fn a_replan_invocation_that_differs_anywhere_refuses() {
    let expected = alloc::vec![preplanned_invocation(1, 0x11, 0x63)];
    let prior = expected.first().expect("one preplanned invocation");
    let program = Pubkey::new_from_array([0x77; 32]);
    let agreeing = |state: usize, canonical: u8| {
        let sink = LifecycleBatchSinkV4::new(Some(expected.as_slice()), 1).expect("verify");
        let mut seeds = LifecycleSeedsV4::new(Some(prior.seeds.as_slice()), 2).expect("verify");
        seeds.push(&[0x11, 0x11]).expect("first seed agrees");
        let LifecycleCanonicalBumpV4::Reused { bump } =
            seeds.pending_bump(&program, 0).expect("reused")
        else {
            return Err(TradingSbfError::Content.into());
        };
        seeds.push(&[bump]).expect("bump agrees");
        let mut bindings =
            LifecycleBindingsV4::new(Some(prior.immutable_identity_bindings.as_slice()), 1)
                .expect("verify");
        bindings
            .push(PreparedImmutableIdentityBindingV4 {
                data_offset: 16,
                canonical: [canonical; 32],
            })
            .map(|()| (sink, seeds, bindings, state))
    };
    // The faithful replan is admitted and hands back no second table.
    let (mut sink, seeds, bindings, state) = agreeing(1, 0x63).expect("faithful bindings");
    sink.admit(prior.plan, state, None, None, seeds, bindings)
        .expect("faithful replan agrees");
    assert!(sink.finish(1).expect("verified").is_empty());
    // A different state coordinate refuses.
    let (mut sink, seeds, bindings, _) = agreeing(1, 0x63).expect("faithful bindings");
    assert!(
        sink.admit(prior.plan, 2, None, None, seeds, bindings)
            .is_err()
    );
    // A different payer coordinate refuses.
    let (mut sink, seeds, bindings, state) = agreeing(1, 0x63).expect("faithful bindings");
    assert!(
        sink.admit(prior.plan, state, Some(4), None, seeds, bindings)
            .is_err()
    );
    // A different plan refuses.
    let (mut sink, seeds, bindings, state) = agreeing(1, 0x63).expect("faithful bindings");
    assert!(
        sink.admit(
            StateLifecyclePlanV3::Authenticate(AuthenticateStatePlanV3 {
                state: [0x11; 32],
                data_bytes: 64,
                lamports: 2,
                bump: 254,
            }),
            state,
            None,
            None,
            seeds,
            bindings,
        )
        .is_err()
    );
    // A different immutable identity binding refuses at the binding.
    assert!(agreeing(1, 0x64).is_err());
}

/// A replan table of the wrong width, or one that stops early, refuses.
#[test]
fn a_replan_table_of_the_wrong_width_refuses() {
    let expected = alloc::vec![
        preplanned_invocation(1, 0x11, 0x63),
        preplanned_invocation(3, 0x21, 0x64),
    ];
    assert!(LifecycleBatchSinkV4::new(Some(expected.as_slice()), 1).is_err());
    assert!(LifecycleBatchSinkV4::new(Some(expected.as_slice()), 3).is_err());
    // Two invocations were declared; admitting none is not agreement.
    let sink = LifecycleBatchSinkV4::new(Some(expected.as_slice()), 2).expect("verify");
    assert!(sink.finish(2).is_err());
    // Nor is a preplan that collected fewer rows than it declared.
    let sink = LifecycleBatchSinkV4::new(None, 2).expect("collect");
    assert!(sink.finish(2).is_err());
}

/// Seeds and bindings the replan never reached are not agreement either:
/// a short walk must refuse rather than pass by silence.
#[test]
fn a_replan_that_skips_a_seed_or_a_binding_refuses() {
    let expected = alloc::vec![preplanned_invocation(1, 0x11, 0x63)];
    let prior = expected.first().expect("one preplanned invocation");
    let mut sink = LifecycleBatchSinkV4::new(Some(expected.as_slice()), 1).expect("verify");
    let mut seeds = LifecycleSeedsV4::new(Some(prior.seeds.as_slice()), 2).expect("verify");
    seeds.push(&[0x11, 0x11]).expect("first seed agrees");
    // The bump seed was never pushed.
    let mut bindings =
        LifecycleBindingsV4::new(Some(prior.immutable_identity_bindings.as_slice()), 1)
            .expect("verify");
    bindings
        .push(PreparedImmutableIdentityBindingV4 {
            data_offset: 16,
            canonical: [0x63; 32],
        })
        .expect("binding agrees");
    assert!(
        sink.admit(prior.plan, 1, None, None, seeds, bindings)
            .is_err()
    );
    // And the mirror: every seed reached, no binding reached.
    let mut sink = LifecycleBatchSinkV4::new(Some(expected.as_slice()), 1).expect("verify");
    let mut seeds = LifecycleSeedsV4::new(Some(prior.seeds.as_slice()), 2).expect("verify");
    seeds.push(&[0x11, 0x11]).expect("first seed agrees");
    seeds.push(&[254]).expect("bump agrees");
    let bindings = LifecycleBindingsV4::new(Some(prior.immutable_identity_bindings.as_slice()), 1)
        .expect("verify");
    assert!(
        sink.admit(prior.plan, 1, None, None, seeds, bindings)
            .is_err()
    );
}

/// Folding one resolved write across every planned binding must mark
/// exactly the bindings that write names, and must still refuse an
/// overlapping write that is not the exact binding.
#[test]
fn one_resolved_write_marks_only_the_binding_it_names() {
    let plans = alloc::vec![
        PreparedLifecycleInvocationV3 {
            plan: StateLifecyclePlanV3::Create(CreateStatePlanV3 {
                state: [0x11; 32],
                payer: [0x12; 32],
                rent_credit: [0x13; 32],
                beneficiary: [0x14; 32],
                refund_source: LifecycleRefundSourceV3::Credit,
                target_data_bytes: 64,
                historical_rent_principal: 1,
                state_before: 0,
                state_after: 1,
                payer_debit: 1,
                payer_after: 0,
                bump: 255,
            }),
            state: 1,
            payer: None,
            rent_credit: None,
            seeds: alloc::vec::Vec::new(),
            immutable_identity_bindings: alloc::vec![PreparedImmutableIdentityBindingV4 {
                data_offset: 16,
                canonical: [0x63; 32],
            }],
        },
        PreparedLifecycleInvocationV3 {
            plan: StateLifecyclePlanV3::Authenticate(AuthenticateStatePlanV3 {
                state: [0x21; 32],
                data_bytes: 64,
                lamports: 1,
                bump: 254,
            }),
            state: 3,
            payer: None,
            rent_credit: None,
            seeds: alloc::vec::Vec::new(),
            immutable_identity_bindings: alloc::vec![PreparedImmutableIdentityBindingV4 {
                data_offset: 16,
                canonical: [0x64; 32],
            }],
        },
    ];
    let aliases = [0_usize, 1, 1, 3];
    let mut written = alloc::vec![false; 2];
    // A write naming the second plan's state and value marks only it.
    assert_eq!(
        inspect_lifecycle_binding_effects_v4(
            &plans,
            ResolvedEffectV3::WriteIdentity {
                account: 3,
                offset: 16,
                value: [0x64; 32],
            },
            &aliases,
            &mut written,
        ),
        Ok(())
    );
    assert_eq!(written, alloc::vec![false, true]);
    // A write through coordinate 1's alias marks the first.
    assert_eq!(
        inspect_lifecycle_binding_effects_v4(
            &plans,
            ResolvedEffectV3::WriteIdentity {
                account: 2,
                offset: 16,
                value: [0x63; 32],
            },
            &aliases,
            &mut written,
        ),
        Ok(())
    );
    assert_eq!(written, alloc::vec![true, true]);
    // An overlapping write that is not the binding still refuses, and the
    // fold reaches it wherever the binding sits in the batch.
    assert_eq!(
        inspect_lifecycle_binding_effects_v4(
            &plans,
            ResolvedEffectV3::WriteU32 {
                account: 3,
                offset: 44,
                value: 0,
            },
            &aliases,
            &mut written,
        ),
        Err(TradingSbfError::Transition.into())
    );
}

#[test]
fn lifecycle_pda_requires_canonical_bump_not_merely_valid_bump() {
    let program_id = Pubkey::new_from_array([0x71; 32]);
    let identity = [0x42; 32];
    let prefix = [b"general-state".as_slice(), identity.as_slice()];
    let (canonical_key, canonical_bump) = Pubkey::find_program_address(&prefix, &program_id);
    let canonical_seed = [canonical_bump];
    let canonical = [prefix[0], prefix[1], canonical_seed.as_slice()];
    assert_eq!(
        require_canonical_lifecycle_pda_v3(&program_id, &canonical),
        Ok(canonical_key)
    );

    let alternate = (0_u8..=u8::MAX)
        .find(|bump| {
            if *bump == canonical_bump {
                return false;
            }
            let bump_seed = [*bump];
            Pubkey::create_program_address(
                &[prefix[0], prefix[1], bump_seed.as_slice()],
                &program_id,
            )
            .is_ok()
        })
        .expect("at least one noncanonical valid bump");
    let alternate_seed = [alternate];
    let hostile = [prefix[0], prefix[1], alternate_seed.as_slice()];
    assert!(require_canonical_lifecycle_pda_v3(&program_id, &hostile).is_err());
}

#[test]
fn common_projection_bindings_and_child_reservations_are_exact() {
    let id = |tag: u8| [tag; 32];
    let physical = Pubkey::new_from_array(id(1));
    let projected = LogicalProjectionKeysV3 {
        selected_config: id(2),
        product_root: id(3),
        portfolio: id(4),
        linked_basis: id(5),
    };
    for (coordinate, expected) in [
        (0_usize, id(1)),
        (1, id(2)),
        (2, id(3)),
        (3, id(4)),
        (4, id(5)),
        (5, id(1)),
    ] {
        assert_eq!(
            *logical_projection_key_v3(coordinate, &physical, &projected),
            expected
        );
    }
    assert_ne!(*logical_projection_key_v3(1, &physical, &projected), id(1));
    let canonical = CommonProjectionBindingsV3 {
        selected_config: id(1),
        selected_product_record: id(2),
        authenticated_product_record: id(2),
        market_product: id(3),
        runtime_product: id(3),
        product_semantic_basis: id(4),
        authenticated_semantic_basis: id(4),
        authenticated_linked_basis: id(5),
    };
    assert_eq!(require_common_projection_bindings_v3(canonical), Ok(()));
    for hostile in [
        // The selected config's binding to an authenticated finalized
        // record is owned by `borrow_finalized_record`, which refuses
        // before this predicate is reached; what stays here is the refusal
        // of an unset selection.
        CommonProjectionBindingsV3 {
            selected_config: [0; 32],
            ..canonical
        },
        CommonProjectionBindingsV3 {
            selected_product_record: id(6),
            ..canonical
        },
        CommonProjectionBindingsV3 {
            market_product: id(6),
            ..canonical
        },
        CommonProjectionBindingsV3 {
            product_semantic_basis: id(6),
            ..canonical
        },
        CommonProjectionBindingsV3 {
            authenticated_linked_basis: [0; 32],
            ..canonical
        },
    ] {
        assert_eq!(
            require_common_projection_bindings_v3(hostile),
            Err(TradingSbfError::Content.into())
        );
    }

    let invocation = dclutch_vm::effect::v3::ResolvedInvocationV3 {
        role: FixedRole::Custody,
        kind: dclutch_vm::effect::v3::RouteKindV3::Once,
        item: None,
        fixed_account_start: 1,
        fixed_account_count: 1,
        item_account_start: 0,
        item_account_count: 0,
        item_account_stride: 0,
        repeated_item_count: 0,
        request_offset: 0,
        request_len: 1,
        borrowed_witness: None,
        receipt_dependencies: dclutch_vm::effect::v3::ResolvedReceiptDependenciesV3::empty(),
        receipt_dependency: None,
    };
    assert_eq!(
        require_no_common_projection_child_accounts_v3(invocation),
        Err(TradingSbfError::Content.into())
    );
    assert_eq!(
        require_no_common_projection_child_accounts_v3(
            dclutch_vm::effect::v3::ResolvedInvocationV3 {
                fixed_account_start: 5,
                ..invocation
            }
        ),
        Ok(())
    );
    assert_eq!(require_tail_count_agreement_v3(7, Some(7)), Ok(()));
    assert_eq!(
        require_tail_count_agreement_v3(7, Some(6)),
        Err(TradingSbfError::Content.into())
    );
    // A profile that projects NO tail count is not a profile projecting
    // zero. This is the arm that was unsatisfiable: every fixed-topology
    // profile in the protocol arrived here as `(outcomes, 0)` and refused.
    assert_eq!(require_tail_count_agreement_v3(3, None), Ok(()));
    assert_eq!(require_tail_count_agreement_v3(2, None), Ok(()));
    // It is still held to the only honest value: a projection of zero from
    // a profile that declares one is a width nobody wrote.
    assert_eq!(
        require_tail_count_agreement_v3(3, Some(0)),
        Err(TradingSbfError::Content.into())
    );
    // And the market floor applies either way.
    for projected in [None, Some(0), Some(1)] {
        assert_eq!(
            require_tail_count_agreement_v3(1, projected),
            Err(TradingSbfError::Content.into()),
            "a one-outcome market is not a market",
        );
    }
    let mut permissions = [AccountPermission::read_only(); 5];
    permissions[0] = AccountPermission::program_owned_mutable();
    assert_eq!(
        require_common_projection_permissions_v3(&permissions),
        Ok(())
    );
    permissions[2] = AccountPermission::program_owned_mutable();
    assert_eq!(
        require_common_projection_permissions_v3(&permissions),
        Err(TradingSbfError::Content.into())
    );
}

#[test]
fn projected_observation_marker_is_owned_only_by_variable_prestate() {
    assert!(prestate_uses_variable_marker_v3(
        AccountPrestateV2::AdapterAuthenticatedVariableData
    ));
    for substituted in [
        AccountPrestateV2::Exact,
        AccountPrestateV2::LifecycleBound,
        AccountPrestateV2::AdapterAuthenticatedVariableDataAlias,
        AccountPrestateV2::AuthenticatedRouteAlias,
        AccountPrestateV2::AuthenticatedOpaqueReadonlyData,
    ] {
        assert!(!prestate_uses_variable_marker_v3(substituted));
    }
}

#[test]
fn projected_product_tail_count_is_rechecked_after_atomic_account_projection() {
    let rules = [AccountRuleInputV2 {
        privileges: AccountPrivilegesV2::new(false, false, false),
        effect_permissions: AccountEffectPermissionsV2::new(false, false, false),
        alias: AccountAliasInputV2::SelfCoordinate,
        data_length: 4,
        data_item_stride: 0,
    }];
    let operations = [AccountOperationInputV2::ProjectTailCountU32 {
        account: AccountCoordinateV2::fixed(0),
        destination: ScalarCoordinateV2::common(0),
        data_offset: 0,
    }];
    let bytes =
        ACCOUNT_PROFILE_HEADER_BYTES + ACCOUNT_PROFILE_RULE_BYTES + ACCOUNT_PROFILE_OPERATION_BYTES;
    let mut scratch = vec![0_u8; bytes];
    let mut encoded = vec![0_u8; bytes];
    encode_account_profile_v2_atomic(
        AccountProfileArtifactV2::TypedScalar,
        &rules,
        &[],
        &operations,
        &[],
        RegisterGeometryV2 {
            common_scalars: 1,
            item_scalar_stride: 0,
            common_identities: 0,
            item_identity_stride: 0,
        },
        &mut scratch,
        &mut encoded,
    )
    .expect("tail-count profile");
    let profile = AccountProfileV2::decode(&encoded).expect("decode profile");
    assert_eq!(
        require_projected_tail_count_agreement_v3(profile, 7, &[7]),
        Ok(())
    );
    assert_eq!(
        require_projected_tail_count_agreement_v3(profile, 7, &[6]),
        Err(TradingSbfError::Content.into())
    );
}

#[test]
fn loader_state_contributes_its_identity_to_a_transcript_and_not_its_bytes() {
    let programdata = |bytes: u8| {
        vec![
            leaked_account_with_facts([0x11; 32], [0x21; 32], 1, vec![0x01; 8], false),
            leaked_account_with_facts(
                [0x12; 32],
                solana_sdk_ids::bpf_loader_upgradeable::ID.to_bytes(),
                2,
                vec![bytes; 4096],
                false,
            ),
        ]
    };
    // The ELF the loader owns is not prestate: two deployments of different
    // bytes at the same address digest the same, which is what takes
    // 9.5 MB out of the Dealer equity transcript.
    assert_eq!(
        test_runtime_transcript(&programdata(0x5a), &[]),
        test_runtime_transcript(&programdata(0xa5), &[]),
    );
    // Its IDENTITY still is. A substituted deployment arrives at a
    // different address, and the transcript refuses to agree.
    let substituted = vec![
        leaked_account_with_facts([0x11; 32], [0x21; 32], 1, vec![0x01; 8], false),
        leaked_account_with_facts(
            [0x13; 32],
            solana_sdk_ids::bpf_loader_upgradeable::ID.to_bytes(),
            2,
            vec![0x5a; 4096],
            false,
        ),
    ];
    assert_ne!(
        test_runtime_transcript(&programdata(0x5a), &[]),
        test_runtime_transcript(&substituted, &[]),
    );
    // And an ordinary state account is untouched by the rule: its bytes are
    // exactly what a candidate is evaluated against.
    let state = |bytes: u8| {
        vec![leaked_account_with_facts(
            [0x11; 32],
            [0x21; 32],
            1,
            vec![bytes; 8],
            false,
        )]
    };
    assert_ne!(
        test_runtime_transcript(&state(0x01), &[]),
        test_runtime_transcript(&state(0x02), &[]),
    );
}

/// One authority per invocation, plus the page when the profile has one.
///
/// The chunked rows are unchanged, which is the load-bearing half: adding a
/// transport must not move the frame of the transport already on chain.
#[test]
fn admitted_runtime_follows_the_authority_vector_and_the_output_page() {
    use AcceleratorTransportProfileV2::{ChunkedBankV2, OutputPageV3};

    assert_eq!(HOT_ADMITTED_CALLER_AUTHORITIES_START_V3, 47);
    assert_eq!(
        hot_admitted_runtime_accounts_start_v3(ChunkedBankV2, 1, 1),
        Ok(48)
    );
    assert_eq!(
        hot_admitted_runtime_accounts_start_v3(ChunkedBankV2, 120, 2),
        Ok(49)
    );
    assert!(hot_admitted_runtime_accounts_start_v3(ChunkedBankV2, 0, 0).is_err());

    // One authority and one page, whatever the bank costs: the Dealer
    // equity Add at two chunks and Remove at three both carve 49.
    for (scalars, identities) in [(1_u32, 1_u32), (26, 37), (35, 53)] {
        assert_eq!(
            hot_admitted_runtime_accounts_start_v3(OutputPageV3, scalars, identities),
            Ok(49)
        );
    }
    assert!(hot_admitted_runtime_accounts_start_v3(OutputPageV3, 0, 0).is_err());
}

fn leaked_readonly_account(key: [u8; 32], owner: [u8; 32], data: Vec<u8>) -> AccountInfo<'static> {
    leaked_account_with_facts(key, owner, 1_000, data, false)
}

fn leaked_account_with_facts(
    key: [u8; 32],
    owner: [u8; 32],
    lamports: u64,
    data: Vec<u8>,
    executable: bool,
) -> AccountInfo<'static> {
    AccountInfo::new(
        Box::leak(Box::new(Pubkey::new_from_array(key))),
        false,
        false,
        Box::leak(Box::new(lamports)),
        Box::leak(data.into_boxed_slice()),
        Box::leak(Box::new(Pubkey::new_from_array(owner))),
        executable,
    )
}

fn test_runtime_transcript(
    accounts: &[AccountInfo<'static>],
    canonical_scratch_coordinates: &[usize],
) -> ContentId {
    let data = accounts
        .iter()
        .map(|account| account.try_borrow_data().expect("readable account"))
        .collect::<Vec<_>>();
    let observations = accounts
        .iter()
        .zip(&data)
        .map(|(account, bytes)| {
            AccountObservationV1::new(
                account.key.as_array(),
                account.owner.as_array(),
                account.lamports(),
                bytes.as_ref(),
                false,
                false,
                account.executable,
            )
        })
        .collect::<Vec<_>>();
    let borrowed = accounts.iter().collect::<Vec<_>>();
    let canonical = canonical_scratch_coordinates
        .iter()
        .map(|coordinate| {
            borrowed
                .get(*coordinate)
                .copied()
                .expect("canonical coordinate")
        })
        .collect::<Vec<_>>();
    runtime_transcript_digest_v3(&observations, &borrowed, &canonical).expect("runtime transcript")
}

#[test]
fn authenticated_scratch_data_is_the_only_runtime_observation_fact_canonicalized() {
    let accounts =
        |scratch_key, scratch_owner, scratch_lamports, scratch_data, executable, other_data| {
            vec![
                leaked_account_with_facts(
                    scratch_key,
                    scratch_owner,
                    scratch_lamports,
                    scratch_data,
                    executable,
                ),
                leaked_readonly_account([0x82; 32], [0x83; 32], other_data),
            ]
        };
    let baseline = test_runtime_transcript(
        &accounts([0x80; 32], [0x81; 32], 7_000, vec![1, 2, 3], false, vec![4]),
        &[0],
    );
    assert_eq!(
        baseline,
        test_runtime_transcript(
            &accounts(
                [0x80; 32],
                [0x81; 32],
                7_000,
                vec![9, 8, 7, 6],
                false,
                vec![4],
            ),
            &[0],
        ),
        "the authenticated input-bank digest, not this transcript, commits page bytes"
    );
    for substituted in [
        accounts([0x84; 32], [0x81; 32], 7_000, vec![1], false, vec![4]),
        accounts([0x80; 32], [0x85; 32], 7_000, vec![1], false, vec![4]),
        accounts([0x80; 32], [0x81; 32], 7_001, vec![1], false, vec![4]),
        accounts([0x80; 32], [0x81; 32], 7_000, vec![1], true, vec![4]),
        accounts([0x80; 32], [0x81; 32], 7_000, vec![1], false, vec![5]),
    ] {
        assert_ne!(baseline, test_runtime_transcript(&substituted, &[0]));
    }
    let reversed = {
        let mut value = accounts([0x80; 32], [0x81; 32], 7_000, vec![1], false, vec![4]);
        value.reverse();
        value
    };
    assert_ne!(
        baseline,
        test_runtime_transcript(&reversed, &[1]),
        "account order remains committed"
    );
    let mut expanded = accounts([0x80; 32], [0x81; 32], 7_000, vec![1], false, vec![4]);
    expanded.push(leaked_readonly_account([0x86; 32], [0x87; 32], vec![6]));
    assert_ne!(
        baseline,
        test_runtime_transcript(&expanded, &[0]),
        "account geometry remains committed"
    );
}

/// Both admitted-AOT observation walks over one bank, compared directly.
///
/// Trading and the accelerator each own a copy of this walk and each
/// commits its result to the same `AdmittedInvocationContextV3` field, so
/// the only thing that makes the lane executable is that the two produce
/// the same bytes. The accelerator's copy used to key by the raw
/// `enumerate()` index while Trading keys by the coordinate's
/// REPRESENTATIVE; the second half of this test is the substitution that
/// difference amounts to, and it is not subtle — an aliased coordinate is
/// keyed by a record's content digest on one side and by an account
/// address on the other.
#[test]
fn both_admitted_observation_walks_key_an_alias_by_its_representative() {
    let projected = LogicalProjectionKeysV3 {
        selected_config: [0x11; 32],
        product_root: [0x22; 32],
        portfolio: [0x33; 32],
        linked_basis: [0x44; 32],
    };
    // Six logical coordinates over five physical accounts: coordinate 5
    // route-aliases onto the Product root at coordinate 2, which is the
    // shape the shipped Dealer scenario profile uses three times, so the
    // two coordinates share one physical account.
    let representatives = [0_usize, 1, 2, 3, 4, 2];
    let accounts = (0..representatives.len())
        .map(|coordinate| {
            let representative = *representatives
                .get(coordinate)
                .expect("representative per coordinate");
            let tag = u8::try_from(representative).expect("small coordinate");
            leaked_readonly_account([0xa0 | tag; 32], [0x5c; 32], vec![tag; 8])
        })
        .collect::<Vec<_>>();
    let borrowed = accounts.iter().collect::<Vec<_>>();
    let inline_bank = [0_u8; 8];
    let content = |tag| ContentId::new([tag; 32]).expect("nonzero content");
    let request = AcceleratorRequestV2::new(
        RequestTransportV2::Inline,
        content(0x71),
        content(0x72),
        content(0x73),
        content(0x74),
        content(0x75),
        1,
        1,
        0,
        0,
        &inline_bank,
    )
    .map(AdmittedAcceleratorRequestV2::ChunkedBankV2)
    .expect("inline request");
    let trading_program = Pubkey::new_from_array([0x76; 32]);

    let accelerator = accelerator_runtime_observations_digest_v4(
        &accounts,
        &representatives,
        request,
        &trading_program,
        projected.selected_config,
        projected.product_root,
        projected.portfolio,
        projected.linked_basis,
    )
    .expect("accelerator transcript");

    let trading_transcript = |walk: &[usize]| {
        let data = accounts
            .iter()
            .map(|account| account.try_borrow_data().expect("readable account"))
            .collect::<Vec<_>>();
        let observations = accounts
            .iter()
            .zip(&data)
            .enumerate()
            .map(|(coordinate, (account, bytes))| {
                AccountObservationV1::new(
                    logical_projection_key_v3(
                        *walk.get(coordinate).unwrap_or(&coordinate),
                        account.key,
                        &projected,
                    ),
                    account.owner.as_array(),
                    account.lamports(),
                    bytes.as_ref(),
                    false,
                    false,
                    account.executable,
                )
            })
            .collect::<Vec<_>>();
        runtime_transcript_digest_v3(&observations, &borrowed, &[]).expect("Trading transcript")
    };

    assert_eq!(
        accelerator,
        trading_transcript(&representatives),
        "the accelerator must reproduce Trading's transcript exactly"
    );
    // The pre-fix accelerator walk. Coordinate 5's key becomes its physical
    // address instead of the Product root's content digest, so no
    // well-formed invocation of a route-aliasing family could ever match.
    let raw_index_walk = (0..representatives.len()).collect::<Vec<_>>();
    assert_ne!(
        accelerator,
        trading_transcript(&raw_index_walk),
        "keying by the raw coordinate must not accidentally agree"
    );
}

/// One synthetic borrowed-witness family, in either spelling.
///
/// Sixteen bytes of profiled prefix and an eight-byte witness. The V3
/// spelling resolves it from `witness_range_common_scalar` 0, whose pair of
/// registers hold `[16, 8]`; the V4 spelling declares the same bytes as a
/// range on the same route. `both` is the shape neither authority admits.
#[cfg(any(
    feature = "families",
    feature = "series-family",
    feature = "dealer-family"
))]
fn witness_coverage_effect_v4(
    role: dclutch_vm::effect::v2::FixedRole,
    legacy_bit: bool,
    semantic_prefix_bytes: u32,
    ranges: &[dclutch_vm::effect::v4::BorrowedRangeV4],
) -> Vec<u8> {
    use dclutch_vm::effect::{
        v3::{
            HEADER_BYTES, ROUTE_BYTES,
            encode::{EffectGeometryV3, RouteInputV3, encode_effect_program_v3_atomic},
        },
        v4::{
            BORROWED_RANGE_BYTES_V4, BorrowedRangePolicyV4, HEADER_BYTES_V4,
            encode_program_v4_atomic,
        },
    };

    let route = RouteInputV3 {
        role,
        kind: dclutch_vm::effect::v3::RouteKindV3::Once,
        enable_common_scalar: None,
        witness_range_common_scalar: legacy_bit.then_some(0),
        receipt_dependency: None,
        fixed_account_start: 0,
        fixed_account_count: 1,
        item_account_start: 0,
        item_account_count: 0,
        fixed_request: &[],
        item_request: &[],
    };
    let base_bytes = HEADER_BYTES + ROUTE_BYTES;
    let mut base_scratch = vec![0_u8; base_bytes];
    let mut base = vec![0_u8; base_bytes];
    encode_effect_program_v3_atomic(
        EffectGeometryV3 {
            fixed_accounts: 1,
            item_account_stride: 0,
            common_scalars: 2,
            item_scalar_stride: 0,
            common_identities: 0,
            item_identity_stride: 0,
        },
        &[route],
        &[],
        &[],
        &mut base_scratch,
        &mut base,
    )
    .expect("one-route borrowed base");
    let successor_bytes = HEADER_BYTES_V4 + ranges.len() * BORROWED_RANGE_BYTES_V4 + base.len();
    let mut scratch = vec![0_u8; successor_bytes];
    let mut output = vec![0_u8; successor_bytes];
    encode_program_v4_atomic(
        &base,
        BorrowedRangePolicyV4::DisjointExactCoverage,
        semantic_prefix_bytes,
        &[],
        ranges,
        &mut scratch,
        &mut output,
    )
    .expect("borrowed successor");
    output
}

#[cfg(any(
    feature = "families",
    feature = "series-family",
    feature = "dealer-family"
))]
fn borrowed_witness_profile_bytes(minimum: u32, maximum: u32) -> Vec<u8> {
    use dclutch_vm::request_profile::{
        encode::{
            RequestCoordinateV1, RequestGeometryV1, RequestInstructionV1, ScalarRegisterV1,
            encode_request_profile_v1_atomic,
        },
        v3::{REQUEST_PROFILE_V3_HEADER_BYTES, encode_request_profile_v3_atomic},
    };

    let instructions = [
        RequestInstructionV1::require_u64(
            RequestCoordinateV1::fixed(0),
            u64::from_le_bytes(*b"PREFIX03"),
        ),
        RequestInstructionV1::project_u64(
            RequestCoordinateV1::fixed(8),
            ScalarRegisterV1::common(0),
        ),
    ];
    let embedded_bytes = dclutch_vm::request_profile::HEADER_BYTES
        + 2 * dclutch_vm::request_profile::OPERATION_BYTES;
    let mut embedded_scratch = vec![0_u8; embedded_bytes];
    let mut embedded = vec![0_u8; embedded_bytes];
    encode_request_profile_v1_atomic(
        RequestGeometryV1::new(16, 0, 2, 0, 0, 0),
        &instructions,
        &[],
        &mut embedded_scratch,
        &mut embedded,
    )
    .expect("embedded prefix projector");
    let width = REQUEST_PROFILE_V3_HEADER_BYTES + embedded.len();
    let mut scratch = vec![0_u8; width];
    let mut output = vec![0_u8; width];
    encode_request_profile_v3_atomic(
        &embedded,
        BorrowedWitnessPolicyV3 {
            minimum_bytes: minimum,
            maximum_bytes: maximum,
            consumer_role: BorrowedWitnessRoleV3::Claims,
            child_request_magic: *b"CHILD003",
            child_receipt_magic: *b"RECPT003",
            child_receipt_bytes: 376,
        },
        &mut scratch,
        &mut output,
    )
    .expect("borrowed-witness wrapper");
    output
}

/// The borrow has ONE spelling, and every other shape refuses by name.
///
/// The post-trade partial equity Remove was the first action ever to reach
/// this rule with a V4 successor, and it refused `BorrowedWitnessRoute` at
/// site 3 -- borrower count 0 -- because the rule counted only the V3
/// route bit while `encode_dealer_equity_effect_base_for_v4` had moved the
/// fact to the Effect's own borrowed-range table. Each conjunct is named
/// here rather than left to an integration bundle: a hostile that must
/// first get past thirty other gates cannot say which gate answered it.
///
/// The V3 bit is pinned from the KERNEL's side as well as this rule's,
/// because the two are the same law read at two distances:
/// `EffectProgramV4::decode` refuses any base route carrying it, and the
/// shipped Hot path reaches this walk through `from_sealed`, which does
/// not re-run that sweep.
#[test]
#[cfg(any(
    feature = "families",
    feature = "series-family",
    feature = "dealer-family"
))]
fn a_borrowed_witness_has_one_spelling_and_every_other_shape_refuses_by_name() {
    use dclutch_vm::effect::{
        v2::FixedRole,
        v4::{BorrowedRangeV4, RequestCoordinateV4},
    };
    use dclutch_vm::request_profile::v3::RequestProfileV3;

    const FAMILY: &[u8] = b"PREFIX03\x00\x00\x00\x00\x00\x00\x00\x00CHILD003";
    let scalars = [16_u64, 8];
    let suffix = [BorrowedRangeV4::new(
        0,
        RequestCoordinateV4::Fixed(16),
        RequestCoordinateV4::CommonScalar(1),
    )];
    let profile_bytes = borrowed_witness_profile_bytes(8, 16);
    let profile = RequestProfileV3::decode(&profile_bytes).expect("borrowed profile");
    let request_profile = RequestProfileKindV3::Borrowed(profile);
    assert_eq!(
        profile.split_request(0, FAMILY),
        Ok((&FAMILY[..16], &FAMILY[16..]))
    );

    let check = |bytes: &[u8]| -> Result<(), ProgramError> {
        let successor = EffectProgramV4::decode(bytes).expect("successor");
        let effect = SelectedEffectProgramV4 {
            base: EffectProgramV3::decode(successor.base().bytes()).expect("base"),
            successor,
            funding: None,
        };
        require_borrowed_witness_coverage_v3(request_profile, effect, 0, &scalars, &[], FAMILY)
    };

    // The V4 successor: no route bit, one range over the declared witness.
    let v4 = witness_coverage_effect_v4(FixedRole::Claims, false, 16, &suffix);
    assert_eq!(check(&v4), Ok(()));

    // NEITHER -- a V4 release whose Claims route carries no range at all,
    // which is the exact shape the equity Remove refused with. Site 3.
    let neither = witness_coverage_effect_v4(FixedRole::Claims, false, 16, &[]);
    assert_eq!(
        check(&neither),
        Err(TradingSbfError::BorrowedWitnessRoute.into())
    );

    // A V4 range on a route whose role is not the one the policy admits.
    // Site 1, conjunct mask bit 0.
    let wrong_role = witness_coverage_effect_v4(FixedRole::Custody, false, 16, &suffix);
    assert_eq!(
        check(&wrong_role),
        Err(TradingSbfError::BorrowedWitnessRoute.into())
    );

    // A V4 range that is not the witness the profile declared. The range
    // table still covers the request exactly, so `SuccessorCoverage` is
    // satisfied and the bytes are this function's own accusation.
    let wide = [BorrowedRangeV4::new(
        0,
        RequestCoordinateV4::Fixed(8),
        RequestCoordinateV4::Fixed(16),
    )];
    let wrong_bytes = witness_coverage_effect_v4(FixedRole::Claims, false, 8, &wide);
    assert_eq!(
        check(&wrong_bytes),
        Err(TradingSbfError::BorrowedWitnessBytes.into())
    );

    // The V3 bit under a V4 successor, refused at BOTH distances. The
    // encoder cannot even emit one -- its own closing decode runs the
    // table sweep -- so the artifact is built by flipping the bit in the
    // encoded base, which is exactly what a hostile sealed record is.
    let mut legacy_bit = witness_coverage_effect_v4(FixedRole::Claims, false, 16, &suffix);
    let honest = legacy_bit.clone();
    let base_offset = legacy_bit.len() - honest_base_len(&honest);
    let route_offset = base_offset + dclutch_vm::effect::v3::HEADER_BYTES;
    // `RouteInputV3`'s witness flag is byte 3 of the route record, the same
    // coordinate `borrowed_witness_is_an_exact_authenticated_suffix` sets.
    legacy_bit[route_offset + 3] = 1;
    assert_ne!(legacy_bit, honest, "the hostile flipped exactly one bit");
    assert_eq!(
        EffectProgramV4::decode(&legacy_bit).err(),
        Some(dclutch_vm::effect::v4::ErrorV4::RangeTable),
        "no Effect V4 may carry the retired V3 route bit"
    );
}

/// Exact encoded width of the one-route V3 base inside a test successor.
#[cfg(any(
    feature = "families",
    feature = "series-family",
    feature = "dealer-family"
))]
fn honest_base_len(successor: &[u8]) -> usize {
    EffectProgramV4::decode(successor)
        .expect("honest successor")
        .base()
        .bytes()
        .len()
}
