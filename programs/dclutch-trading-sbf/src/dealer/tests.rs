use std::{boxed::Box, vec, vec::Vec};

use dclutch_capability_program_contract::{
    CAPABILITY_PROGRAM_ACCOUNT_PROFILE_OFFSET, CAPABILITY_PROGRAM_CAPACITY_PROFILE_OFFSET,
    CAPABILITY_PROGRAM_CONFIG_SCHEMA_OFFSET, CAPABILITY_PROGRAM_DERIVATION_POLICY_OFFSET,
    CAPABILITY_PROGRAM_EFFECT_SCHEMA_OFFSET, CAPABILITY_PROGRAM_HEADER_BYTES_V1,
    CAPABILITY_PROGRAM_KIND_OFFSET, CAPABILITY_PROGRAM_MAGIC_V1, CAPABILITY_PROGRAM_PROFILE_OFFSET,
    CAPABILITY_PROGRAM_PROFILE_V2, CAPABILITY_PROGRAM_REQUEST_SCHEMA_OFFSET,
    CAPABILITY_PROGRAM_ROOT_SCHEMA_OFFSET, CAPABILITY_PROGRAM_ROOT_STATE_BYTES_OFFSET,
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1, initialize_root_account_v1,
};
use dclutch_dealer_codec::{
    Action, CANDIDATE_BYTES, CandidateInput, CurveBand, CurveInput, Phase, Side, encode_candidate,
};
use dclutch_economic_slice_kernel::{POSITION_HEADER_BYTES, SCALAR_BYTES, initialize_position};
use dclutch_registry_svm::AuthenticatedRoleReceiptV1;
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, CapabilityExecutionSelectionV1, ExecutionRoleV1, ProgramIdentityV1,
};
use dclutch_token_svm::COption;

use super::*;

fn put(output: &mut [u8], offset: usize, source: &[u8]) {
    output
        .get_mut(offset..offset + source.len())
        .expect("fixture write is in bounds")
        .copy_from_slice(source);
}

fn policy(market: [u8; 32], release: [u8; 32]) -> Policy {
    Policy {
        market_id: market,
        release_set_id: release,
        dealer_id: [3; 32],
        fee_recipient_id: [4; 32],
        unwind_recipient_id: [5; 32],
        outcome_count: 2,
        quote_scale: 100,
        fee_numerator: 1,
        fee_denominator: 10,
        minimum_work_funding: 20,
        replacement_delay: 5,
    }
}

fn descriptor_bytes() -> Vec<u8> {
    let supported = supported_content_v2().expect("supported profile");
    let mut transition = vec![0_u8; 40];
    put(&mut transition, 0, b"DCTV");
    *transition.get_mut(4).expect("transition version byte") = 2;
    put(&mut transition, 6, &1_u16.to_le_bytes());
    put(&mut transition, 8, &1_u16.to_le_bytes());
    let mut output = vec![0_u8; CAPABILITY_PROGRAM_HEADER_BYTES_V1 + transition.len()];
    put(&mut output, 0, &CAPABILITY_PROGRAM_MAGIC_V1);
    put(&mut output, 8, &1_u16.to_le_bytes());
    put(
        &mut output,
        CAPABILITY_PROGRAM_PROFILE_OFFSET,
        &CAPABILITY_PROGRAM_PROFILE_V2.to_le_bytes(),
    );
    for (offset, value) in [
        (
            CAPABILITY_PROGRAM_KIND_OFFSET,
            dealer_kind_v2().expect("kind"),
        ),
        (
            CAPABILITY_PROGRAM_CONFIG_SCHEMA_OFFSET,
            supported.config_schema,
        ),
        (
            CAPABILITY_PROGRAM_REQUEST_SCHEMA_OFFSET,
            supported.request_schema,
        ),
        (CAPABILITY_PROGRAM_ROOT_SCHEMA_OFFSET, supported.root_schema),
        (
            CAPABILITY_PROGRAM_ACCOUNT_PROFILE_OFFSET,
            supported.account_profile,
        ),
        (
            CAPABILITY_PROGRAM_DERIVATION_POLICY_OFFSET,
            supported.derivation_policy,
        ),
        (
            CAPABILITY_PROGRAM_EFFECT_SCHEMA_OFFSET,
            supported.effect_schema,
        ),
    ] {
        put(&mut output, offset, value.as_bytes());
    }
    put(
        &mut output,
        CAPABILITY_PROGRAM_CAPACITY_PROFILE_OFFSET,
        &[9; 32],
    );
    put(
        &mut output,
        CAPABILITY_PROGRAM_ROOT_STATE_BYTES_OFFSET,
        &u32::try_from(ROOT_TAIL_BYTES)
            .expect("tail width")
            .to_le_bytes(),
    );
    put(&mut output, CAPABILITY_PROGRAM_HEADER_BYTES_V1, &transition);
    output
}

fn fixture() -> (
    DealerProfileV2,
    CapabilityProgramV1<'static>,
    [u8; dclutch_dealer_codec::POLICY_BYTES],
    Pubkey,
) {
    let program_id = Pubkey::new_from_array([10; 32]);
    let market = [11; 32];
    let release = ContentId::new([12; 32]).expect("release");
    let config = policy(market, release.to_bytes())
        .to_bytes()
        .expect("policy");
    let descriptor_bytes = Box::leak(descriptor_bytes().into_boxed_slice());
    let descriptor = CapabilityProgramV1::decode(descriptor_bytes).expect("descriptor");
    let selection = CapabilityExecutionSelectionV1::new(
        0,
        ContentId::new([13; 32]).expect("manifest"),
        dealer_kind_v2().expect("kind"),
        ContentId::new(hash(descriptor_bytes).to_bytes()).expect("descriptor digest"),
        ContentId::new(hash(&config).to_bytes()).expect("config digest"),
    )
    .expect("selection");
    let header = CapabilityRootHeaderV1::new(release, market, 3, selection).expect("header");
    let child_root = Pubkey::find_program_address(&header.seeds().as_slices(), &program_id).0;
    let receipt = AuthenticatedRoleReceiptV1::new(
        ExecutionRoleV1::Trading,
        release,
        ProgramIdentityV1::new(program_id.to_bytes()).expect("program"),
        ArtifactReleaseIdV1::new([14; 32]).expect("artifact"),
        ContentId::new([15; 32]).expect("semantic"),
    );
    let context = TradingFamilyContextV1::authenticate_activation(
        &program_id,
        &child_root,
        header,
        CAPABILITY_ROOT_HEADER_BYTES_V1 + ROOT_TAIL_BYTES,
        receipt,
    )
    .expect("context");
    let profile = DealerProfileV2::authenticate_after_common_dispatch(context, descriptor, &config)
        .expect("profile");
    (profile, descriptor, config, child_root)
}

fn candidate_bytes() -> [u8; CANDIDATE_BYTES] {
    let bids = [CurveBand {
        capacity: 100,
        price_numerator: 40,
    }];
    let asks = [CurveBand {
        capacity: 100,
        price_numerator: 60,
    }];
    let curves = [
        CurveInput {
            bids: &bids,
            asks: &asks,
        },
        CurveInput {
            bids: &bids,
            asks: &asks,
        },
    ];
    let mut output = [0_u8; CANDIDATE_BYTES];
    encode_candidate(
        &mut output,
        CandidateInput {
            candidate_id: [16; 32],
            revision: 7,
            valid_from: 10,
            expires_at: 100,
            quote_reserve_floor: 50,
            work_funding: 20,
            work_reward: 2,
            minimum_inventory: &[0, 0],
            maximum_inventory: &[100, 100],
            curves: &curves,
        },
    )
    .expect("candidate");
    output
}

fn token(owner: Pubkey, mint: Pubkey, amount: u64) -> TokenAccount {
    TokenAccount {
        mint: mint.to_bytes(),
        owner: owner.to_bytes(),
        amount,
        delegate: COption::None,
        state: AccountState::Initialized,
        native_reserve: COption::None,
        delegated_amount: 0,
        close_authority: COption::None,
    }
}

fn position(profile: DealerProfileV2, claims_program: &Pubkey) -> (Pubkey, Vec<u8>) {
    let holder = profile.context().child_root_key();
    let seeds =
        ClaimsPositionSeedsV1::new(profile.context().market(), holder).expect("position seeds");
    let key = Pubkey::find_program_address(&seeds.as_slices(), claims_program).0;
    let mut bytes = vec![0_u8; POSITION_HEADER_BYTES + 2 * SCALAR_BYTES * 2];
    initialize_position(&mut bytes, profile.context().market(), holder, 2).expect("position");
    (key, bytes)
}

fn vaults(
    profile: DealerProfileV2,
    custody_program: &Pubkey,
    token_program: Pubkey,
    mint: Pubkey,
) -> [VaultAccountObservationV2; 3] {
    let market = profile.context().market();
    let release = profile.context().release_set().to_bytes();
    let authority = Pubkey::find_program_address(
        &[CUSTODY_AUTHORITY_PDA_DOMAIN_V1, &market, &release],
        custody_program,
    )
    .0;
    let mut observations = [VaultAccountObservationV2 {
        key: Pubkey::default(),
        owner_program: token_program,
        token: token(authority, mint, 0),
    }; 3];
    for ((observation, compartment), amount) in observations
        .iter_mut()
        .zip([
            CompartmentV1::TradingPrincipal,
            CompartmentV1::FeeVault,
            CompartmentV1::LivenessVault,
        ])
        .zip([50, 0, 20])
    {
        let seeds = CustodyVaultSeedsV1::new(
            market,
            release,
            profile.context().child_root_key(),
            compartment,
        );
        observation.key = Pubkey::find_program_address(&seeds.as_slices(), custody_program).0;
        observation.token.amount = amount;
    }
    observations
}

#[test]
fn descriptor_config_root_candidate_position_and_vaults_join_exactly() {
    let (profile, descriptor, _config, child_root) = fixture();
    let candidate_bytes = candidate_bytes();
    let candidate_view = CandidateView::decode(&candidate_bytes).expect("candidate");
    let trading_program = Pubkey::new_from_array(profile.context().program_id());
    let candidate_key = Pubkey::find_program_address(
        &[
            DEALER_CANDIDATE_PDA_DOMAIN_V2,
            &child_root.to_bytes(),
            &candidate_view.candidate_id,
        ],
        &trading_program,
    )
    .0;
    let active = profile
        .candidate(&candidate_key, &trading_program, &candidate_bytes)
        .expect("candidate join");
    let tail = RootTail::initialize(active);
    let header = CapabilityRootHeaderV1::new(
        profile.context().release_set(),
        profile.context().market(),
        profile.context().generation(),
        profile.context().selection(),
    )
    .expect("header");
    let mut root = vec![0_u8; profile.context().root_account_bytes()];
    initialize_root_account_v1(
        &mut root,
        header,
        descriptor,
        &tail.to_bytes().expect("tail"),
    )
    .expect("root");
    assert_eq!(profile.root_tail(descriptor, &root), Ok(tail));

    let claims_program = Pubkey::new_from_array([17; 32]);
    let (position_key, position_bytes) = position(profile, &claims_program);
    let projected = DealerPositionProjectionV2::authenticate(
        profile,
        &claims_program,
        &position_key,
        &claims_program,
        &position_bytes,
        0,
    )
    .expect("position projection");
    assert_eq!(projected.native(), &[0, 0]);

    let custody_program = Pubkey::new_from_array([18; 32]);
    let token_program = Pubkey::new_from_array([19; 32]);
    let mint = Pubkey::new_from_array([20; 32]);
    let vaults = DealerVaultProjectionV2::authenticate(
        profile,
        &custody_program,
        &token_program,
        &mint,
        vaults(profile, &custody_program, token_program, mint),
    )
    .expect("vaults");
    let state = profile
        .materialize_state(tail, active, None, projected, vaults, None)
        .expect("semantic projection");
    assert_eq!(state.inventory[..2], [0, 0]);
    assert_eq!(state.quote_custody, 50);
    assert_eq!(state.liveness_custody, 20);
    assert_eq!(state.phase, Phase::Open);
}

#[test]
fn stale_substituted_position_candidate_and_vault_refuse() {
    let (profile, _descriptor, _config, child_root) = fixture();
    let claims_program = Pubkey::new_from_array([17; 32]);
    let (position_key, position_bytes) = position(profile, &claims_program);
    assert_eq!(
        DealerPositionProjectionV2::authenticate(
            profile,
            &claims_program,
            &position_key,
            &claims_program,
            &position_bytes,
            1,
        ),
        Err(DealerProfileError::Position)
    );
    assert_eq!(
        DealerPositionProjectionV2::authenticate(
            profile,
            &claims_program,
            &Pubkey::new_from_array([99; 32]),
            &claims_program,
            &position_bytes,
            0,
        ),
        Err(DealerProfileError::Position)
    );

    let candidate_bytes = candidate_bytes();
    let trading_program = Pubkey::new_from_array(profile.context().program_id());
    assert_eq!(
        profile.candidate(
            &Pubkey::new_from_array([98; 32]),
            &trading_program,
            &candidate_bytes,
        ),
        Err(DealerProfileError::Candidate)
    );
    let candidate = CandidateView::decode(&candidate_bytes).expect("candidate");
    let expected_candidate = Pubkey::find_program_address(
        &[
            DEALER_CANDIDATE_PDA_DOMAIN_V2,
            &child_root.to_bytes(),
            &candidate.candidate_id,
        ],
        &trading_program,
    )
    .0;
    assert!(
        profile
            .candidate(
                &expected_candidate,
                &Pubkey::new_from_array([97; 32]),
                &candidate_bytes,
            )
            .is_err()
    );

    let custody_program = Pubkey::new_from_array([18; 32]);
    let token_program = Pubkey::new_from_array([19; 32]);
    let mint = Pubkey::new_from_array([20; 32]);
    let mut observations = vaults(profile, &custody_program, token_program, mint);
    observations[2].key = observations[1].key;
    assert_eq!(
        DealerVaultProjectionV2::authenticate(
            profile,
            &custody_program,
            &token_program,
            &mint,
            observations,
        ),
        Err(DealerProfileError::Vault)
    );
}

#[test]
fn request_rejoins_the_exact_position_revision() {
    let (profile, _descriptor, _config, _child_root) = fixture();
    let claims_program = Pubkey::new_from_array([17; 32]);
    let (key, bytes) = position(profile, &claims_program);
    let projected = DealerPositionProjectionV2::authenticate(
        profile,
        &claims_program,
        &key,
        &claims_program,
        &bytes,
        0,
    )
    .expect("position");
    let request = TradingRequest {
        action: Action::Fill,
        side: Side::TakerBuys,
        outcome: 0,
        expected_state_revision: 1,
        expected_position_revision: 0,
        now: 11,
        quantity: 1,
        expected_candidate_id: [16; 32],
        actor_id: [0; 32],
        replacement_candidate_id: [0; 32],
        expected_candidate_revision: 7,
    };
    let request_bytes = request.to_bytes().expect("request");
    assert_eq!(
        authenticate_request_v2(&request_bytes, projected),
        Ok(request)
    );
    let mut stale = request;
    stale.expected_position_revision = 1;
    assert_eq!(
        authenticate_request_v2(&stale.to_bytes().expect("stale bytes"), projected),
        Err(DealerProfileError::Position)
    );
}

#[test]
fn total_machine_emits_child_plan_but_only_inventory_free_tail_is_persistable() {
    let (profile, _descriptor, _config, child_root) = fixture();
    let candidate_bytes = candidate_bytes();
    let candidate_view = CandidateView::decode(&candidate_bytes).expect("candidate");
    let trading_program = Pubkey::new_from_array(profile.context().program_id());
    let candidate_key = Pubkey::find_program_address(
        &[
            DEALER_CANDIDATE_PDA_DOMAIN_V2,
            &child_root.to_bytes(),
            &candidate_view.candidate_id,
        ],
        &trading_program,
    )
    .0;
    let active = profile
        .candidate(&candidate_key, &trading_program, &candidate_bytes)
        .expect("candidate");
    let tail = RootTail::initialize(active);

    let claims_program = Pubkey::new_from_array([17; 32]);
    let (position_key, position_bytes) = position(profile, &claims_program);
    let position = DealerPositionProjectionV2::authenticate(
        profile,
        &claims_program,
        &position_key,
        &claims_program,
        &position_bytes,
        0,
    )
    .expect("position");
    let custody_program = Pubkey::new_from_array([18; 32]);
    let token_program = Pubkey::new_from_array([19; 32]);
    let mint = Pubkey::new_from_array([20; 32]);
    let mut vault_observations = vaults(profile, &custody_program, token_program, mint);
    vault_observations[0].token.amount = 60;
    let vaults = DealerVaultProjectionV2::authenticate(
        profile,
        &custody_program,
        &token_program,
        &mint,
        vault_observations,
    )
    .expect("vaults");
    let request = TradingRequest {
        action: Action::Fill,
        side: Side::TakerSells,
        outcome: 0,
        expected_state_revision: 1,
        expected_position_revision: 0,
        now: 11,
        quantity: 10,
        expected_candidate_id: [16; 32],
        actor_id: [0; 32],
        replacement_candidate_id: [0; 32],
        expected_candidate_revision: 7,
    };
    let transition = interpret_projected_v2(
        profile,
        DealerInterpretationInputV2 {
            tail,
            active,
            pending: None,
            proposed: None,
            position,
            vaults,
            terminal_winner: None,
            request,
        },
    )
    .expect("projected transition");
    assert_eq!(transition.post_inventory(), &[10, 0]);
    assert_eq!(transition.post_tail().state_revision, 2);
    assert_eq!(transition.post_tail().sell_used[0], 10);
    assert_eq!(transition.post_tail().to_bytes().expect("tail").len(), 384);
}

#[test]
fn activation_requires_present_candidate_safe_claims_principal_and_work_funding() {
    let (profile, _descriptor, _config, child_root) = fixture();
    let candidate_bytes = candidate_bytes();
    let candidate_view = CandidateView::decode(&candidate_bytes).expect("candidate");
    let trading_program = Pubkey::new_from_array(profile.context().program_id());
    let candidate_key = Pubkey::find_program_address(
        &[
            DEALER_CANDIDATE_PDA_DOMAIN_V2,
            &child_root.to_bytes(),
            &candidate_view.candidate_id,
        ],
        &trading_program,
    )
    .0;
    let active = profile
        .candidate(&candidate_key, &trading_program, &candidate_bytes)
        .expect("candidate");
    let claims_program = Pubkey::new_from_array([17; 32]);
    let (position_key, position_bytes) = position(profile, &claims_program);
    let position = DealerPositionProjectionV2::authenticate(
        profile,
        &claims_program,
        &position_key,
        &claims_program,
        &position_bytes,
        0,
    )
    .expect("position");
    let custody_program = Pubkey::new_from_array([18; 32]);
    let token_program = Pubkey::new_from_array([19; 32]);
    let mint = Pubkey::new_from_array([20; 32]);
    let observations = vaults(profile, &custody_program, token_program, mint);
    let projected = DealerVaultProjectionV2::authenticate(
        profile,
        &custody_program,
        &token_program,
        &mint,
        observations,
    )
    .expect("vaults");
    assert_eq!(
        initialize_projected_v2(profile, active, position, projected),
        Ok(RootTail::initialize(active))
    );
    let mut underfunded = observations;
    underfunded[2].token.amount -= 1;
    let underfunded = DealerVaultProjectionV2::authenticate(
        profile,
        &custody_program,
        &token_program,
        &mint,
        underfunded,
    )
    .expect("shape still authentic");
    assert_eq!(
        initialize_projected_v2(profile, active, position, underfunded),
        Err(DealerProfileError::Tail)
    );
}
