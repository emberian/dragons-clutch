//! The builder's first nonzero dynamic fixed span, against General's real
//! emitted artifacts.
//!
//! Every family reproduced through this builder so far runs at zero declared
//! spans, so the span-width path had never been executed. General is the first
//! family that declares one, and this suite is what makes the width a
//! *derivation* rather than a number a campaign types in.
//!
//! It also corrects the boundary `src/general.rs` sketched. The sole General
//! span is **not** request-owned: its selector is
//! `scalar::INPUT_SCRATCH_PAGE_COUNT`, which no General RequestProfile writes,
//! so the width comes from the canonical register-bank geometry and the span is
//! admissible under exactly one strategy disposition. Both facts are executed
//! below rather than asserted in prose.

use dclutch_account_profile_contract::v2::AccountProfileV2;
use dclutch_chain_bundle_builder::{
    BuilderError, WaistFactsV1,
    general::{
        GeneralRequestEvidenceV1, GeneralRequestInputV1, GeneralRequestV1,
        derive_general_request_v1,
    },
    profile_ops,
    registers::{SpanWidthInputV1, derive_dynamic_span_widths},
};
use dclutch_effect_kernel::v4::ProgramV4 as EffectProgramV4;
use dclutch_execution_strategy_contract::shadow_v3::{
    SHADOW_ACK_SCHEMA_ID_V3, SHADOW_REQUEST_SCHEMA_ID_V3,
};
use dclutch_execution_strategy_contract::v2::{
    ACCELERATOR_ACK_SCHEMA_ID_V2, ACCELERATOR_REQUEST_SCHEMA_ID_V2, BankTransportV2,
    EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2, EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2,
    ExecutionStrategyProgramV2, StrategyDispositionV2, classify_bank_transport_v2,
};
use dclutch_general_adapter_contract::{
    account_rules_v3::{
        GeneralExternalAccountWidthsV3, encode_general_account_profile_v3_atomic,
        general_account_profile_bytes_v3, general_account_profile_fixed_count_v3,
    },
    artifacts_v3::decode_general_request_v3,
    candidate_v1::{
        CandidateVerifyRowBuffersV1, CandidateVerifyRowViewV1, GeneralCandidateOpeningV1,
        GeneralCandidateV1, general_candidate_identity_v1, verify_candidate_row_v1,
    },
    collection_v1::{
        GeneralBatchOpeningV1, GeneralBatchV1, GeneralOrderHeaderV1, GeneralOrderPhaseV1,
        GeneralOrderStateV1, GeneralOrderV1, MakerFundingV1, general_order_len_v1,
        general_signed_order_terms_len_v1,
    },
    effect_artifacts_v3::{
        GENERAL_EFFECT_INSTRUCTION_PLACEHOLDER_V3, encode_general_effect_program_v4_atomic,
        general_effect_instruction_count_v3, general_effect_program_bytes_v3,
        general_effect_program_bytes_v4, general_effect_template_bytes_v3,
    },
    hot_candidate_v3::{GENERAL_HOT_COMMON_IDENTITIES_V3, general_hot_scalar_count_v3, scalar},
    local_state_v3::{
        GeneralLocalStateHeaderV3, GeneralLocalStateKindV3, encode_general_local_state_v3_atomic,
        general_local_state_len_v3,
    },
    release_v3::{GENERAL_ACTIONS_V3, GENERAL_ACTIONS_V5},
    runtime_manifest::settlement_manifest_len_v2,
    runtime_selection::{RUNTIME_SELECTION_CURSOR_BYTES_V2, consider_verified_candidate_v2},
    runtime_settlement::initialize_runtime_settlement_in_place_v2,
    runtime_verify::runtime_verifier_len_v2,
    runtime_width::{
        CandidateHeaderV2, CandidateV2, ExecutionHeaderV2, ExecutionV2, PageHeaderV2, PageV2,
        candidate_len, execution_len, page_len, settlement_cursor_len, verified_candidate_len,
    },
    specialization::{general_request_profile_bytes_v1, general_request_profile_v1},
    state_seeds_v3::GeneralStateAddressSeedsV3,
};
use dclutch_general_codec::{
    Action, MAX_SELECTION_CRITERIA, SelectionCriterion, SelectionPolicyV1,
    successor_request_v2::ControllerRequestV2, successor_request_v3::ControllerRequestV3,
};
use dclutch_general_config_contract::{
    GeneralRootV2,
    v3::{GeneralConfigV3, GeneralConfigV3Input},
};
use dclutch_request_profile_contract::{
    ProjectionRegisterKindV1, ProjectionRegisterSpaceV1, ProjectionTargetV1, RequestProfileV1,
    SCHEMA_RELEASE_ID as REQUEST_PROFILE_SCHEMA_ID_V1, validate_request,
};
use solana_program::pubkey::Pubkey;

/// The Product width the accelerator campaign measures at (`N` outcomes).
const OUTCOME_COUNT: u32 = 4;
/// A candidate identity the request may name.
const CANDIDATE_ID: [u8; 32] = [0x81; 32];
/// Poisoned bump witnesses: the span derivation never reads them.
const PLACEHOLDER_BUMP: u8 = 0xEE;

/// The external account widths the published profile encoder is generated at.
const WIDTHS: GeneralExternalAccountWidthsV3 = GeneralExternalAccountWidthsV3 {
    linked_basis_prefix: 256,
    result_domain: 192,
    rent_sysvar: 17,
    core_market: 320,
    activation_cache: 160,
    upgradeable_program: 36,
    trading_programdata_prefix: 45,
    claims_programdata_prefix: 45,
    core_programdata_prefix: 45,
    realm_record: 112,
    rent_credit: 48,
};

fn waist() -> WaistFactsV1 {
    WaistFactsV1 {
        registry_program: Pubkey::new_from_array([0xc1; 32]),
        trading_program: Pubkey::new_from_array([0xc3; 32]),
        core_program: Pubkey::new_from_array([0xc2; 32]),
        claims_program: Pubkey::new_from_array([0xc5; 32]),
        custody_program: Pubkey::new_from_array([0xc6; 32]),
        release_set: [0x31; 32],
        activation_cache: Pubkey::new_from_array([0xac; 32]),
        trading_semantic_release: [0x32; 32],
    }
}

fn account_profile(action: Action) -> Vec<u8> {
    let bytes = general_account_profile_bytes_v3(action).expect("profile width");
    let mut scratch = vec![0_u8; bytes];
    let mut output = vec![0_u8; bytes];
    encode_general_account_profile_v3_atomic(action, WIDTHS, &mut scratch, &mut output)
        .expect("General AccountProfile");
    output
}

/// General's current emitted EffectProgram V4 envelope.
fn effect_v4(action: Action) -> Vec<u8> {
    let (fixed, item) = general_effect_instruction_count_v3(action);
    let count = fixed.checked_add(item).expect("effect instruction count");
    let mut instructions = vec![GENERAL_EFFECT_INSTRUCTION_PLACEHOLDER_V3; count];
    let mut templates = vec![0_u8; general_effect_template_bytes_v3(action)];
    let base_bytes = general_effect_program_bytes_v3(action).expect("base effect width");
    let mut base_scratch = vec![0_u8; base_bytes];
    let mut base_output = vec![0_u8; base_bytes];
    let bytes = general_effect_program_bytes_v4(action).expect("effect width");
    let mut scratch = vec![0_u8; bytes];
    let mut output = vec![0_u8; bytes];
    encode_general_effect_program_v4_atomic(
        action,
        &mut instructions,
        &mut templates,
        &mut base_scratch,
        &mut base_output,
        &mut scratch,
        &mut output,
    )
    .expect("General EffectProgram");
    output
}

/// A real `ExecutionStrategyProgramV2` at one disposition.
///
/// The certificate and admission digests are opaque to the span rule; the
/// disposition is the only field it reads, and the encoder still enforces the
/// full presence grammar, so an Interpreted strategy really is one.
fn strategy(disposition: StrategyDispositionV2) -> Vec<u8> {
    let content = |value: [u8; 32]| dclutch_core_contract::ContentId::new(value).expect("content");
    // The encoder's own presence and transport grammar, not a choice of this
    // test: ShadowAot carries a certificate and the shadow transcript schemas,
    // AdmittedAot carries both records over the chunked bank, Interpreted
    // carries neither.
    let (request_schema, ack_schema) = match disposition {
        StrategyDispositionV2::ShadowAot => (SHADOW_REQUEST_SCHEMA_ID_V3, SHADOW_ACK_SCHEMA_ID_V3),
        StrategyDispositionV2::Interpreted | StrategyDispositionV2::AdmittedAot => (
            ACCELERATOR_REQUEST_SCHEMA_ID_V2,
            ACCELERATOR_ACK_SCHEMA_ID_V2,
        ),
    };
    ExecutionStrategyProgramV2::new(
        disposition,
        content(dclutch_transition_vm::v3::SCHEMA_RELEASE_ID),
        content([0x41; 32]),
        content(EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2),
        (disposition != StrategyDispositionV2::Interpreted).then(|| content([0x42; 32])),
        content(EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2),
        (disposition == StrategyDispositionV2::AdmittedAot).then(|| content([0x43; 32])),
        content(request_schema),
        content(ack_schema),
    )
    .expect("strategy grammar")
    .to_bytes()
    .to_vec()
}

/// The canonical request for one action.
///
/// The codec's own grammar decides which coordinates an action may carry, so
/// these are not free choices: `Freeze` names no candidate, only `Close` may
/// witness a terminal bump, and only the two row actions may name a manifest
/// row. Anything else the encoder refuses outright.
fn request(action: Action) -> Vec<u8> {
    ControllerRequestV2 {
        action,
        expected_revision: 0,
        candidate_id: (action != Action::Freeze).then_some(CANDIDATE_ID),
        page_index: 0,
        execution_index: 0,
        manifest_order_index: 0,
        state_bump: PLACEHOLDER_BUMP,
        terminal_record_bump: if action == Action::Close {
            PLACEHOLDER_BUMP
        } else {
            0
        },
    }
    .to_bytes()
    .expect("General controller request")
    .to_vec()
}

/// The page count the bank transport selects for this Product width — the
/// number the campaign table records as "scratch pages".
fn expected_pages(action: Action, outcome_count: u32) -> u32 {
    let scalars = general_hot_scalar_count_v3(action, outcome_count).expect("General scalar count");
    match classify_bank_transport_v2(scalars, GENERAL_HOT_COMMON_IDENTITIES_V3)
        .expect("General bank transport")
    {
        BankTransportV2::AuthenticatedScratchPages { page_count, .. } => page_count,
        BankTransportV2::InlineReturnData { .. } => 1,
    }
}

struct Artifacts {
    account_profile: Vec<u8>,
    request_profile: Vec<u8>,
    effect: Vec<u8>,
    request: Vec<u8>,
}

fn artifacts(action: Action) -> Artifacts {
    Artifacts {
        account_profile: account_profile(action),
        request_profile: general_request_profile_bytes_v1(action).to_vec(),
        effect: effect_v4(action),
        request: request(action),
    }
}

fn derive(
    action: Action,
    set: &Artifacts,
    strategy_bytes: &[u8],
    outcome_count: u32,
) -> Result<Vec<u32>, BuilderError> {
    let profile = AccountProfileV2::decode(&set.account_profile).expect("profile decode");
    let _ = action;
    derive_dynamic_span_widths(&SpanWidthInputV1 {
        profile,
        request_profile_bytes: &set.request_profile,
        request_profile_schema: REQUEST_PROFILE_SCHEMA_ID_V1,
        effect_bytes: &set.effect,
        effect_schema: dclutch_effect_kernel::v4::SCHEMA_RELEASE_ID_V4,
        strategy_bytes,
        waist: waist(),
        tail_count: outcome_count,
        family_request: &set.request,
        clock_slot: 9_001,
    })
}

/// The published General profile declares NO span, and its former selector is
/// a register nothing writes.
///
/// The span was the input scratch-page transport. Its width could never come
/// from a request projection -- nothing in the request reaches that register --
/// and the transport it selected has no producer a chain transaction can be, so
/// the bank rides inline in the CPI instruction data and the span is gone.
/// Coordinate 86 survived the span's removal so the 151 common scalars would
/// not renumber, and on 2026-09-04 it stopped being a hole and became
/// `scalar::ORDER_MIN_QUOTE_CREDIT_PER_LOT`, the seller's floor. What this
/// asserts is UNCHANGED and now load-bearing for a different reason: no
/// AccountProfile operation and no RequestProfile coordinate writes it. The
/// floor reaches it from the accelerator's own projection of the verifier
/// cursor, so a profile that wrote it would be a second author for a term the
/// signed order record owns.
#[test]
fn general_declares_no_span_and_nothing_writes_its_former_selector() {
    let target = ProjectionTargetV1 {
        kind: ProjectionRegisterKindV1::Scalar,
        space: ProjectionRegisterSpaceV1::Common,
        index: u16::try_from(scalar::ORDER_MIN_QUOTE_CREDIT_PER_LOT).expect("selector index"),
    };
    for action in GENERAL_ACTIONS_V3 {
        let bytes = account_profile(action);
        let profile = AccountProfileV2::decode(&bytes).expect("profile decode");
        assert!(
            profile.uses_dynamic_fixed_spans(),
            "{action:?} is profile 13"
        );
        assert_eq!(profile.dynamic_fixed_span_count(), 0, "{action:?}");
        assert_eq!(profile.item_account_stride(), 0, "{action:?}");
        let request_profile = RequestProfileV1::decode(general_request_profile_bytes_v1(action))
            .expect("request profile");
        assert!(
            !request_profile
                .writes_register(target)
                .expect("writes_register"),
            "{action:?} request must not state the scratch-page count"
        );
    }
}

/// The derived width vector is empty, for every action and at every Product
/// width, because there is no span to give a width to.
#[test]
fn the_derived_span_width_vector_is_empty_at_every_width() {
    let admitted = strategy(StrategyDispositionV2::AdmittedAot);
    for outcome_count in [1_u32, OUTCOME_COUNT, 258] {
        for action in GENERAL_ACTIONS_V3 {
            let set = artifacts(action);
            let widths =
                derive(action, &set, &admitted, outcome_count).expect("General span widths");
            assert!(
                widths.is_empty(),
                "{action:?} at N={outcome_count} declares a span it should not"
            );
        }
    }
}

/// The logical frame is exactly the fixed count, at every Product width, and
/// the query still refuses a width vector that is not the profile's shape.
#[test]
fn the_frame_is_the_fixed_count_and_a_stated_width_refuses() {
    let admitted = strategy(StrategyDispositionV2::AdmittedAot);
    for action in GENERAL_ACTIONS_V3 {
        let set = artifacts(action);
        let profile = AccountProfileV2::decode(&set.account_profile).expect("profile decode");
        let widths = derive(action, &set, &admitted, OUTCOME_COUNT).expect("span widths");
        let fixed = usize::from(general_account_profile_fixed_count_v3(action).expect("fixed"));
        let logical = profile_ops::logical_count(profile, OUTCOME_COUNT, &widths)
            .expect("logical count with spans");
        assert_eq!(logical, fixed, "{action:?}");
        // A width for a span this profile does not declare is refused rather
        // than silently ignored, which is what keeps a caller from re-inventing
        // the page span by stating one.
        assert!(matches!(
            profile_ops::logical_count(profile, OUTCOME_COUNT, &[4]),
            Err(BuilderError::Profile(_))
        ));
        assert!(matches!(
            profile_ops::physical_count(profile, OUTCOME_COUNT, &[4]),
            Err(BuilderError::Profile(_))
        ));
    }
}

fn pages_as_u32(value: usize) -> u32 {
    u32::try_from(value).expect("page count")
}

/// A span-free profile derives the same empty width vector under every
/// disposition.
///
/// It used to refuse all but `AdmittedAot`, and that refusal was the span's:
/// a profile-only selector is admissible only when the strategy's canonical
/// bank geometry requires scratch pages. With no span there is no selector to
/// own, so the derivation stops caring which disposition asked. What still
/// forces General onto the accelerated route is the Strategy record itself,
/// not the account profile.
#[test]
fn a_span_free_profile_derives_no_widths_under_every_disposition() {
    for action in GENERAL_ACTIONS_V3 {
        let set = artifacts(action);
        for disposition in [
            StrategyDispositionV2::Interpreted,
            StrategyDispositionV2::ShadowAot,
            StrategyDispositionV2::AdmittedAot,
        ] {
            assert_eq!(
                derive(action, &set, &strategy(disposition), OUTCOME_COUNT),
                Ok(Vec::new()),
                "{action:?} under {disposition:?}"
            );
        }
    }
}

/// A zero-span profile accepts the empty width vector and refuses every other,
/// which is the Direct reproduction's path and now General's too.
#[test]
fn a_zero_span_profile_derives_no_widths() {
    let set = artifacts(Action::Freeze);
    let profile = AccountProfileV2::decode(&set.account_profile).expect("profile decode");
    assert!(profile_ops::physical_count(profile, OUTCOME_COUNT, &[]).is_ok());
    assert!(matches!(
        profile_ops::physical_count(profile, OUTCOME_COUNT, &[1]),
        Err(BuilderError::Profile(_))
    ));
}

/// Current General releases emit the V4 envelope the Hot executor selects.
#[test]
fn the_general_effect_artifact_is_current_v4_and_drives_span_geometry() {
    for action in GENERAL_ACTIONS_V3 {
        let emitted = effect_v4(action);
        EffectProgramV4::decode(&emitted).expect("General emits a valid EffectV4");
        let set = artifacts(action);
        assert_eq!(
            derive(
                action,
                &set,
                &strategy(StrategyDispositionV2::AdmittedAot),
                OUTCOME_COUNT
            )
            .expect("current effect drives span geometry"),
            Vec::new(),
            "{action:?}"
        );
    }
}

fn open_config(generation: u64) -> Vec<u8> {
    GeneralConfigV3::new(GeneralConfigV3Input {
        capacity_profile_id: [0x41; 32],
        claim_basis_id: [0x42; 32],
        program_set_id: [0x43; 32],
        generation,
        price_scale: 1_000_000,
        collection_slots: 16,
        selection_slots: 16,
        settlement_slots: 64,
        max_orders_per_candidate: 32,
        max_pages_per_candidate: 32,
        continuation_reward_lamports: 1,
        selection_policy_id: [0x44; 32],
        quote_surplus_beneficiary: [0x45; 32],
    })
    .expect("General config")
    .to_bytes()
    .to_vec()
}

#[test]
fn open_batch_request_derives_occurrence_and_lifecycle_bump_at_runtime_widths() {
    let config = open_config(9);
    let config_id = solana_program::hash::hash(&config).to_bytes();
    let market = [0x61; 32];
    let root_address = Pubkey::new_from_array([0x62; 32]);
    let root = GeneralRootV2::active(market, config_id, 9).expect("active root");
    for outcome_count in [1_u32, 258] {
        let derived = derive_general_request_v1(GeneralRequestInputV1 {
            action: Action::OpenBatch,
            root,
            root_address,
            config: &config,
            outcome_count,
            product_id: [0x63; 32],
            trading_program: waist().trading_program,
            primary_state_account: None,
            evidence: GeneralRequestEvidenceV1::default(),
        })
        .expect("chain-derived OpenBatch request");
        let decoded = ControllerRequestV3::decode(&derived.request).expect("canonical V3 request");
        assert_eq!(decoded.action.legacy(), Some(Action::OpenBatch));
        assert_eq!(derived.action, Action::OpenBatch);
        assert_eq!(decoded.subject_id, derived.subject_id);
        assert_eq!(decoded.expected_revision, root.revision());
        assert_eq!(decoded.primary_state_bump, derived.primary_state_bump);
        assert_ne!(derived.primary_state, root_address);
    }
}

#[test]
fn open_batch_request_refuses_substituted_config_generation_and_zero_coordinates() {
    let config = open_config(9);
    let root = GeneralRootV2::active(
        [0x61; 32],
        solana_program::hash::hash(&config).to_bytes(),
        9,
    )
    .expect("active root");
    let base = GeneralRequestInputV1 {
        action: Action::OpenBatch,
        root,
        root_address: Pubkey::new_from_array([0x62; 32]),
        config: &config,
        outcome_count: 1,
        product_id: [0x63; 32],
        trading_program: waist().trading_program,
        primary_state_account: None,
        evidence: GeneralRequestEvidenceV1::default(),
    };
    let foreign_config = open_config(10);
    assert!(matches!(
        derive_general_request_v1(GeneralRequestInputV1 {
            config: &foreign_config,
            ..base
        }),
        Err(BuilderError::Binding(_))
    ));
    assert!(matches!(
        derive_general_request_v1(GeneralRequestInputV1 {
            product_id: [0; 32],
            ..base
        }),
        Err(BuilderError::Binding(_))
    ));
    assert!(matches!(
        derive_general_request_v1(GeneralRequestInputV1 {
            outcome_count: 0,
            ..base
        }),
        Err(BuilderError::Binding(_))
    ));
}

/// One live Batch envelope exactly as the chain holds it after an `OpenBatch`.
///
/// Built through the semantic owners -- `GeneralBatchV1::open` consumes the
/// root's revision and sequence, `encode_general_local_state_v3_atomic` wraps
/// the record in its physical lifecycle -- rather than by spelling 224 bytes
/// here, so a record layout that moved would move this fixture with it.
fn live_batch_account(
    root: &mut GeneralRootV2,
    config: &[u8],
    root_address: Pubkey,
    trading_program: Pubkey,
    outcome_count: u32,
    product_id: [u8; 32],
    current_slot: u64,
) -> (Vec<u8>, [u8; 32], u64) {
    let decoded = GeneralConfigV3::decode(config).expect("General config");
    let opening = GeneralBatchOpeningV1 {
        outcome_count,
        sequence: root.next_batch_sequence(),
        generation: root.generation(),
        market: root.market(),
        product_id,
        config_id: root.config_id(),
        price_scale: decoded.price_scale(),
        collection_close_slot: current_slot + decoded.collection_slots(),
        settlement_close_slot: current_slot
            + decoded.collection_slots()
            + decoded.settlement_slots(),
        max_orders: decoded.max_orders_per_candidate(),
    };
    let expected_revision = root.revision();
    let batch =
        GeneralBatchV1::open(root, opening, expected_revision, current_slot).expect("open batch");
    let batch_id = batch.batch_id();
    let seeds =
        GeneralStateAddressSeedsV3::batch(root_address.to_bytes(), batch_id).expect("Batch seeds");
    let bump = Pubkey::find_program_address(
        seeds.as_slices().expect("Batch seed slices").as_slice(),
        &trading_program,
    )
    .1;
    let body = batch.to_bytes();
    let bytes = general_local_state_len_v3(GeneralLocalStateKindV3::Batch, outcome_count)
        .expect("Batch envelope width");
    let mut scratch = vec![0_u8; bytes];
    let mut account = vec![0_u8; bytes];
    encode_general_local_state_v3_atomic(
        GeneralLocalStateHeaderV3 {
            kind: GeneralLocalStateKindV3::Batch,
            bump,
            rent_principal: 2_282_880,
            beneficiary: [0x64; 32],
        },
        &body,
        &mut scratch,
        &mut account,
    )
    .expect("Batch envelope");
    (account, batch_id, opening.collection_close_slot)
}

/// THE SECOND ACTION DERIVES ITS SUBJECT FROM THE CHAIN, NOT FROM A PREDICTION.
///
/// `OpenBatch` computes the occurrence identity it is about to create; every
/// action after it must READ the identity the chain already holds, because a
/// host that recomputed the opening would be a second author for a record the
/// Batch already owns -- and it would agree right up until one config window or
/// one close slot differed, at which point it would name an address that does
/// not exist and the refusal would be about a missing account rather than about
/// the substitution that caused it.
#[test]
fn close_batch_request_reads_its_subject_off_the_live_batch_at_runtime_widths() {
    let config = open_config(9);
    let config_id = solana_program::hash::hash(&config).to_bytes();
    let market = [0x61; 32];
    let root_address = Pubkey::new_from_array([0x62; 32]);
    let product_id = [0x63; 32];
    let trading_program = waist().trading_program;
    for outcome_count in [1_u32, 258] {
        let mut root = GeneralRootV2::active(market, config_id, 9).expect("active root");
        let opened = derive_general_request_v1(GeneralRequestInputV1 {
            action: Action::OpenBatch,
            root,
            root_address,
            config: &config,
            outcome_count,
            product_id,
            trading_program,
            primary_state_account: None,
            evidence: GeneralRequestEvidenceV1::default(),
        })
        .expect("chain-derived OpenBatch request");
        let (account, batch_id, _) = live_batch_account(
            &mut root,
            &config,
            root_address,
            trading_program,
            outcome_count,
            product_id,
            1_000,
        );
        // The occurrence the open PREDICTED and the identity the batch CARRIES
        // are the same, and that is a measurement rather than an assumption:
        // `GeneralBatchOccurrenceTermsV1::new` zeroes both close slots before
        // hashing, so the identity is slot-independent and precomputable.
        assert_eq!(opened.subject_id, Some(batch_id));
        let derived = derive_general_request_v1(GeneralRequestInputV1 {
            action: Action::CloseBatch,
            root,
            root_address,
            config: &config,
            outcome_count,
            product_id,
            trading_program,
            primary_state_account: Some(&account),
            evidence: GeneralRequestEvidenceV1::default(),
        })
        .expect("chain-derived CloseBatch request");
        let decoded = ControllerRequestV3::decode(&derived.request).expect("canonical V3 request");
        assert_eq!(decoded.action.legacy(), Some(Action::CloseBatch));
        assert_eq!(derived.subject_id, Some(batch_id));
        assert_eq!(derived.primary_state, opened.primary_state);
        assert_eq!(derived.primary_state_bump, opened.primary_state_bump);
        // The open consumed revision 1; the close must ask for what the root
        // now holds, or the batch's own replay guard refuses it.
        assert_eq!(decoded.expected_revision, root.revision());
        assert_ne!(
            decoded.expected_revision,
            ControllerRequestV3::decode(&opened.request)
                .expect("canonical V3 request")
                .expected_revision,
        );
    }
}

#[test]
fn each_action_refuses_the_prestate_shape_the_other_one_needs() {
    let config = open_config(9);
    let config_id = solana_program::hash::hash(&config).to_bytes();
    let root_address = Pubkey::new_from_array([0x62; 32]);
    let product_id = [0x63; 32];
    let trading_program = waist().trading_program;
    let mut root = GeneralRootV2::active([0x61; 32], config_id, 9).expect("active root");
    let opened_root = root;
    let (account, _, _) = live_batch_account(
        &mut root,
        &config,
        root_address,
        trading_program,
        1,
        product_id,
        1_000,
    );
    // `OpenBatch` creates its primary state, so a campaign that supplied one is
    // describing a different execution than the one it asked for.
    assert!(matches!(
        derive_general_request_v1(GeneralRequestInputV1 {
            action: Action::OpenBatch,
            root: opened_root,
            root_address,
            config: &config,
            outcome_count: 1,
            product_id,
            trading_program,
            primary_state_account: Some(&account),
            evidence: GeneralRequestEvidenceV1::default(),
        }),
        Err(BuilderError::Binding(_))
    ));
    // `CloseBatch` names one that already exists, and there is nothing to read.
    assert!(matches!(
        derive_general_request_v1(GeneralRequestInputV1 {
            action: Action::CloseBatch,
            root,
            root_address,
            config: &config,
            outcome_count: 1,
            product_id,
            trading_program,
            primary_state_account: None,
            evidence: GeneralRequestEvidenceV1::default(),
        }),
        Err(BuilderError::Binding(_))
    ));
    // A Batch under a different Product is not this market's batch, and the
    // join refuses before an address is derived from it.
    assert!(matches!(
        derive_general_request_v1(GeneralRequestInputV1 {
            action: Action::CloseBatch,
            root,
            root_address,
            config: &config,
            outcome_count: 1,
            product_id: [0x71; 32],
            trading_program,
            primary_state_account: Some(&account),
            evidence: GeneralRequestEvidenceV1::default(),
        }),
        Err(BuilderError::Binding(_))
    ));
    // No action refuses `UnsupportedRoute` any more, and an action whose own
    // evidence is absent says which record it wanted rather than being built
    // against a vacancy. `Consider` reads its batch identity out of the
    // certificate it considers, so without one there is no address to derive.
    assert!(matches!(
        derive_general_request_v1(GeneralRequestInputV1 {
            action: Action::Consider,
            root,
            root_address,
            config: &config,
            outcome_count: 1,
            product_id,
            trading_program,
            primary_state_account: None,
            evidence: GeneralRequestEvidenceV1::default(),
        }),
        Err(BuilderError::Binding(_))
    ));
}

/// General's emitted base EffectProgram V3, the geometry authority Trading's
/// `require_geometry` actually reads (`SelectedEffectProgramV4` derefs to it).
fn effect_v3(action: Action) -> Vec<u8> {
    let (fixed, item) = general_effect_instruction_count_v3(action);
    let count = fixed.checked_add(item).expect("effect instruction count");
    let mut instructions = vec![GENERAL_EFFECT_INSTRUCTION_PLACEHOLDER_V3; count];
    let mut templates = vec![0_u8; general_effect_template_bytes_v3(action)];
    let bytes = general_effect_program_bytes_v3(action).expect("base effect width");
    let mut scratch = vec![0_u8; bytes];
    let mut output = vec![0_u8; bytes];
    dclutch_general_adapter_contract::effect_artifacts_v3::encode_general_effect_program_v3_atomic(
        action,
        &mut instructions,
        &mut templates,
        &mut scratch,
        &mut output,
    )
    .expect("General base EffectProgram");
    output
}

/// Trading's `require_geometry` register join, executed host-side against
/// General's own emitted artifacts.
///
/// The on-chain executor collapses every one of these equalities into a single
/// undifferentiated `TradingSbfError::Content` (`0x4003`) after roughly 371k CU,
/// so a General artifact whose four register geometries disagree stays
/// invisible until a real-ELF submission -- and no General campaign had ever
/// submitted one: `tools/gauntlet/general/` reaches the accelerator through a
/// purpose-built caller and never runs this join at all. Each pair is checked
/// separately so a failure names the two artifacts that disagree.
///
/// THIS TEST IS CURRENTLY RED, and it is red because the protocol contradicts
/// itself rather than because the check is wrong:
///
/// - `dclutch_general_adapter_contract::artifacts_v3` admission REQUIRES
///   `account.item_account_stride() == GENERAL_SCRATCH_PAGE_RULE_STRIDE_V3`
///   (1) and `effect.item_account_stride() == 0`, because under Profile13 the
///   item-rule table is repurposed as the dynamic fixed-span template bank and
///   is not a Product-N semantic account stride;
/// - `dclutch_trading_sbf::hot_v3::require_geometry` REQUIRES
///   `account.item_account_stride() == effect.item_account_stride()`, with no
///   dynamic-fixed-span case, even though the same function already special-
///   cases dynamic spans when it counts logical accounts.
///
/// Both cannot hold, so fourteen of General's fifteen actions cannot execute
/// through Trading Hot at all. The resolution belongs to the owner of the
/// Profile13 dynamic-span contract, not to this campaign; the Direct family
/// passes only because both of its strides are zero.
#[test]
fn every_general_action_declares_one_register_geometry_across_its_four_artifacts() {
    let mut mismatches = 0_usize;
    for action in dclutch_general_adapter_contract::release_v3::GENERAL_ACTIONS_V4 {
        let account_profile_bytes = account_profile(action);
        let request_profile_bytes = general_request_profile_bytes_v1(action).to_vec();
        let effect_bytes = effect_v3(action);
        let profile = AccountProfileV2::decode(&account_profile_bytes).expect("AccountProfile");
        let request_profile =
            RequestProfileV1::decode(&request_profile_bytes).expect("RequestProfile");
        let effect =
            dclutch_effect_kernel::v3::ProgramV3::decode(&effect_bytes).expect("EffectProgram V3");
        let transition_bytes =
            dclutch_general_adapter_contract::transition_artifacts_v3::general_transition_program_bytes_lean_v3(
                action,
            );
        let transition =
            dclutch_transition_vm::v3::ProgramV3::decode(transition_bytes).expect("Transition");
        for (label, left, right) in [
            (
                "fixed_account_count account/effect",
                u64::from(profile.fixed_account_count()),
                u64::from(effect.fixed_account_count()),
            ),
            (
                "common_scalar_count account/request",
                u64::from(profile.common_scalar_count()),
                u64::from(request_profile.common_scalar_count()),
            ),
            (
                "item_scalar_stride account/request",
                u64::from(profile.item_scalar_stride()),
                u64::from(request_profile.item_scalar_stride()),
            ),
            (
                "common_identity_count account/request",
                u64::from(profile.common_identity_count()),
                u64::from(request_profile.common_identity_count()),
            ),
            (
                "item_identity_stride account/request",
                u64::from(profile.item_identity_stride()),
                u64::from(request_profile.item_identity_stride()),
            ),
            (
                "common_scalar_count account/transition",
                u64::from(profile.common_scalar_count()),
                u64::from(transition.common_scalar_count()),
            ),
            (
                "item_scalar_stride account/transition",
                u64::from(profile.item_scalar_stride()),
                u64::from(transition.item_scalar_stride()),
            ),
            (
                "common_identity_count account/transition",
                u64::from(profile.common_identity_count()),
                u64::from(transition.common_identity_count()),
            ),
            (
                "item_identity_stride account/transition",
                u64::from(profile.item_identity_stride()),
                u64::from(transition.item_identity_stride()),
            ),
            (
                "common_scalar_count account/effect",
                u64::from(profile.common_scalar_count()),
                u64::from(effect.common_scalar_count()),
            ),
            (
                "item_scalar_stride account/effect",
                u64::from(profile.item_scalar_stride()),
                u64::from(effect.item_scalar_stride()),
            ),
            (
                "common_identity_count account/effect",
                u64::from(profile.common_identity_count()),
                u64::from(effect.common_identity_count()),
            ),
            (
                "item_identity_stride account/effect",
                u64::from(profile.item_identity_stride()),
                u64::from(effect.item_identity_stride()),
            ),
        ] {
            if left != right {
                eprintln!("MISMATCH {action:?}: {label}: profile={left} artifact={right}");
                mismatches += 1;
            }
        }
        // ASKED, NOT RESTATED -- and this row is why the rest of this list is a
        // hazard. The item account stride is the one pair here that is NOT an
        // equality: under a dynamic-fixed-span profile `AccountProfileV2` forces
        // its own stride nonzero (span-template geometry it never multiplies by)
        // while an effect with no per-item accounts declares zero, so General's
        // artifacts are REQUIRED to differ. This test used to hand-copy the
        // equality `hot_v3::require_geometry` had, and when `861032b8` corrected
        // the runtime the copy went on reporting fifteen mismatches against a law
        // Trading no longer enforced. Both sides now ask the contract that owns
        // the field, so there is one author and no copy to drift.
        if !profile.admits_effect_item_account_stride(effect.item_account_stride()) {
            eprintln!(
                "MISMATCH {action:?}: item_account_stride account/effect: \
                 profile={} artifact={} (dynamic spans: {})",
                profile.item_account_stride(),
                effect.item_account_stride(),
                profile.uses_dynamic_fixed_spans(),
            );
            mismatches += 1;
        }
    }
    assert_eq!(
        mismatches, 0,
        "General artifacts cannot satisfy Trading's require_geometry; see this test's doc comment"
    );
}

/// One live General market: everything the fifteen actions share.
#[derive(Clone)]
struct LiveMarketV1 {
    config: Vec<u8>,
    root: GeneralRootV2,
    root_address: Pubkey,
    trading_program: Pubkey,
    product_id: [u8; 32],
    outcome_count: u32,
}

/// Every record one of the fifteen arms reads, produced by the protocol.
///
/// NOT ONE OF THESE IS TYPED HERE. The batch comes out of `GeneralBatchV1::open`
/// and `close`, the order out of `GeneralOrderV1::encode_into` and the batch's
/// own `admit`, the submission out of `GeneralCandidateV1::submit`, and the
/// verifier cursor, the certificate and the settlement manifest are the three
/// outputs of ONE run of `verify_candidate_row_v1` -- the manifest has exactly
/// one producer in this tree and it is that verb, so a test that spelled one
/// would be inventing the record the settlement half authenticates. The
/// selection cursor is `consider_verified_candidate_v2` over that certificate
/// and the settlement cursor is `initialize_runtime_settlement_in_place_v2`
/// over that pair.
struct LiveRecordsV1 {
    batch_account: Vec<u8>,
    batch_id: [u8; 32],
    order_account: Vec<u8>,
    signed_terms: Vec<u8>,
    order_id: [u8; 32],
    candidate_image: Vec<u8>,
    candidate_id: [u8; 32],
    candidate_account: Vec<u8>,
    verifier_account: Vec<u8>,
    verified: Vec<u8>,
    manifest: Vec<u8>,
    selection_account: Vec<u8>,
    settlement_account: Vec<u8>,
}

/// The one slot the collection half runs at.
const LIVE_ADMISSION_SLOT: u64 = 1_000;
/// Revision the candidate's single page is pinned at.
const LIVE_PAGE_REVISION: u64 = 11;
/// Lamports one verification crank pays.
const LIVE_CRANK_REWARD: u64 = 5_000;
/// The maker every order in the fixture belongs to.
const LIVE_OWNER: [u8; 32] = [0xc1; 32];
/// The solver funding the submission.
const LIVE_SOLVER: [u8; 32] = [0xc3; 32];
/// The rent principal every lifecycle envelope declares.
const LIVE_RENT_PRINCIPAL: u64 = 2_282_880;
/// The beneficiary every lifecycle envelope declares.
const LIVE_BENEFICIARY: [u8; 32] = [0x64; 32];

/// Wrap one semantic record in the physical lifecycle envelope the chain holds,
/// with the canonical bump its own recipe derives.
fn envelope(
    kind: GeneralLocalStateKindV3,
    outcome_count: u32,
    seeds: GeneralStateAddressSeedsV3,
    trading_program: Pubkey,
    body: &[u8],
) -> Vec<u8> {
    let bump = Pubkey::find_program_address(
        seeds.as_slices().expect("state seed slices").as_slice(),
        &trading_program,
    )
    .1;
    let bytes = general_local_state_len_v3(kind, outcome_count).expect("envelope width");
    let mut scratch = vec![0_u8; bytes];
    let mut account = vec![0_u8; bytes];
    encode_general_local_state_v3_atomic(
        GeneralLocalStateHeaderV3 {
            kind,
            bump,
            rent_principal: LIVE_RENT_PRINCIPAL,
            beneficiary: LIVE_BENEFICIARY,
        },
        body,
        &mut scratch,
        &mut account,
    )
    .expect("lifecycle envelope");
    account
}

/// Run one whole General market, from an opened batch to an initialized
/// settlement cursor, through the protocol's own semantic owners.
#[allow(clippy::too_many_lines)]
fn live_records(market: &mut LiveMarketV1) -> LiveRecordsV1 {
    let width = market.outcome_count;
    let count = usize::try_from(width).expect("runtime width");
    let config = GeneralConfigV3::decode(&market.config).expect("General config");
    let root_seed = market.root_address.to_bytes();
    let collection_close = LIVE_ADMISSION_SLOT + config.collection_slots();
    let settlement_close = collection_close + config.selection_slots() + config.settlement_slots();

    let revision = market.root.revision();
    let opening = GeneralBatchOpeningV1 {
        outcome_count: width,
        sequence: market.root.next_batch_sequence(),
        generation: market.root.generation(),
        market: market.root.market(),
        product_id: market.product_id,
        config_id: market.root.config_id(),
        price_scale: config.price_scale(),
        collection_close_slot: collection_close,
        settlement_close_slot: settlement_close,
        max_orders: config.max_orders_per_candidate(),
    };
    let mut batch = GeneralBatchV1::open(&mut market.root, opening, revision, LIVE_ADMISSION_SLOT)
        .expect("open batch");
    let batch_id = batch.batch_id();

    let mut order_account = vec![0_u8; general_order_len_v1(width).expect("order width")];
    GeneralOrderV1::encode_into(
        GeneralOrderHeaderV1 {
            outcome_count: width,
            nonce: 1,
            owner_id: LIVE_OWNER,
            market: market.root.market(),
            batch_id,
            generation: market.root.generation(),
            max_lots: 10,
            max_quote_debit_per_lot: 2,
            min_quote_credit_per_lot: 0,
            valid_until_slot: settlement_close,
        },
        &vec![1_u64; count],
        &vec![0_u64; count],
        GeneralOrderStateV1 {
            phase: GeneralOrderPhaseV1::Placed,
            admitted_slot: LIVE_ADMISSION_SLOT,
            released_slot: 0,
        },
        &mut order_account,
    )
    .expect("order record");
    let order = GeneralOrderV1::decode(&order_account).expect("order record");
    let order_id = order.order_id();
    batch
        .admit(
            order,
            MakerFundingV1 {
                owner_id: LIVE_OWNER,
                available_quote: u64::MAX / 4,
                available_claims: &vec![u64::MAX / 4; count],
            },
            LIVE_ADMISSION_SLOT,
        )
        .expect("admit order");
    let mut signed_terms =
        vec![0_u8; general_signed_order_terms_len_v1(width).expect("signed terms width")];
    order
        .encode_signed_terms_into(&mut signed_terms)
        .expect("signed terms");

    let revision = market.root.revision();
    assert_eq!(
        batch
            .close(&mut market.root, revision)
            .expect("close batch"),
        batch_id,
        "closing the batch changed its identity",
    );

    // The candidate carries its OWN digest, and `CandidateV2` checks nothing
    // about that field: encode once to fix every other byte, then re-encode
    // with the digest those bytes produce.
    let prices = {
        let mut values = vec![config.price_scale() / u64::from(width); count];
        let remainder = config.price_scale() - values.iter().sum::<u64>();
        if let Some(first) = values.first_mut() {
            *first += remainder;
        }
        values
    };
    let mut candidate_image = vec![0_u8; candidate_len(width).expect("candidate width")];
    let header = CandidateHeaderV2 {
        outcome_count: width,
        page_count: 1,
        candidate_coordinate: 1,
        price_scale: config.price_scale(),
        candidate_id: [0xb5; 32],
        product_id: market.product_id,
        batch_id,
    };
    CandidateV2::encode_into(header, &prices, &mut candidate_image).expect("draft candidate");
    let candidate_id = general_candidate_identity_v1(&candidate_image).expect("candidate identity");
    CandidateV2::encode_into(
        CandidateHeaderV2 {
            candidate_id,
            ..header
        },
        &prices,
        &mut candidate_image,
    )
    .expect("addressed candidate");
    let image = CandidateV2::decode(&candidate_image).expect("candidate image");

    let opening = GeneralCandidateOpeningV1 {
        outcome_count: width,
        page_count: 1,
        page_revision: LIVE_PAGE_REVISION,
        submitted_slot: collection_close,
        candidate_id,
        batch_id,
        solver_id: LIVE_SOLVER,
        row_count: 1,
        reward_rate_lamports: LIVE_CRANK_REWARD,
    };
    let submission = GeneralCandidateV1::submit(
        batch,
        image,
        LIVE_PAGE_REVISION,
        1,
        LIVE_CRANK_REWARD,
        LIVE_SOLVER,
        opening.work_capacity().expect("work capacity"),
        collection_close,
    )
    .expect("submit candidate");

    let mut row = vec![0_u8; execution_len(width).expect("execution width")];
    ExecutionV2::encode_into(
        ExecutionHeaderV2 {
            outcome_count: width,
            page_coordinate: 1,
            execution_coordinate: 1,
            nonce: order.header().nonce,
            order_id,
            owner_id: LIVE_OWNER,
            max_lots: order.header().max_lots,
            lots: 2,
        },
        &vec![1_u64; count],
        &vec![0_u64; count],
        &mut row,
    )
    .expect("execution row");
    let mut page = vec![0_u8; page_len(width, 1).expect("page width")];
    PageV2::encode_into(
        PageHeaderV2 {
            outcome_count: width,
            page_coordinate: 1,
            page_count: 1,
            revision: LIVE_PAGE_REVISION,
            candidate_id,
        },
        &[row.as_slice()],
        &mut page,
    )
    .expect("page");

    // ONE ROW, WHICH IS THE WHOLE CANDIDATE. The single page is also the last,
    // so this crank closes the only globally grouped order: it emits the
    // one-entry settlement manifest AND completes the certificate, which is why
    // this fixture needs one verification rather than a chain of them.
    let cursor_len = runtime_verifier_len_v2(width).expect("verifier width");
    let verified_len = verified_candidate_len(width).expect("certificate width");
    let manifest_len = settlement_manifest_len_v2(width, 1).expect("manifest width");
    let zero_verified = vec![0_u8; verified_len];
    let mut cursor_scratch = vec![0_u8; cursor_len];
    let mut cursor_output = vec![0xa5_u8; cursor_len];
    let mut verified_scratch = vec![0_u8; verified_len];
    let mut verified_output = zero_verified.clone();
    let mut manifest_scratch = vec![0_u8; manifest_len];
    let mut manifest_output = vec![0xa5_u8; manifest_len];
    let summary = verify_candidate_row_v1(
        CandidateVerifyRowViewV1 {
            batch,
            submission,
            candidate: &candidate_image,
            page: &page,
            order: &order_account,
            cursor_before: &vec![0_u8; cursor_len],
            verified_before: &zero_verified,
            expected_page_index: 0,
            expected_row_index: 0,
            expected_revision: 0,
        },
        CandidateVerifyRowBuffersV1 {
            cursor_scratch: &mut cursor_scratch,
            cursor_output: &mut cursor_output,
            verified_scratch: &mut verified_scratch,
            verified_output: &mut verified_output,
            manifest_scratch: &mut manifest_scratch,
            manifest_output: &mut manifest_output,
        },
    )
    .expect("verify the candidate's only row");
    assert!(
        summary.complete,
        "the one row of a one-page candidate did not complete it",
    );

    let mut selection = vec![0_u8; RUNTIME_SELECTION_CURSOR_BYTES_V2];
    let mut selection_scratch = vec![0_u8; RUNTIME_SELECTION_CURSOR_BYTES_V2];
    consider_verified_candidate_v2(
        SelectionPolicyV1 {
            policy_id: config.selection_policy_id(),
            criterion_count: 1,
            criteria: [SelectionCriterion::MaximizeFilledLots; MAX_SELECTION_CRITERIA],
        },
        &vec![0_u8; RUNTIME_SELECTION_CURSOR_BYTES_V2],
        &verified_output,
        0,
        &mut selection_scratch,
        &mut selection,
    )
    .expect("consider the certificate");

    let mut settlement = vec![0_u8; settlement_cursor_len(width).expect("cursor width")];
    initialize_runtime_settlement_in_place_v2(&cursor_output, &verified_output, 0, &mut settlement)
        .expect("initialize settlement");

    let batch_account = envelope(
        GeneralLocalStateKindV3::Batch,
        width,
        GeneralStateAddressSeedsV3::batch(root_seed, batch_id).expect("Batch seeds"),
        market.trading_program,
        &batch.to_bytes(),
    );
    let candidate_account = envelope(
        GeneralLocalStateKindV3::Candidate,
        width,
        GeneralStateAddressSeedsV3::candidate(root_seed, candidate_id).expect("Candidate seeds"),
        market.trading_program,
        &summary.submission.to_bytes(),
    );
    LiveRecordsV1 {
        batch_account,
        batch_id,
        order_account: envelope(
            GeneralLocalStateKindV3::Order,
            width,
            GeneralStateAddressSeedsV3::order(root_seed, order_id).expect("Order seeds"),
            market.trading_program,
            &order_account,
        ),
        signed_terms,
        order_id,
        candidate_image,
        candidate_id,
        candidate_account,
        verifier_account: envelope(
            GeneralLocalStateKindV3::Verifier,
            width,
            GeneralStateAddressSeedsV3::verifier(root_seed, candidate_id).expect("Verifier seeds"),
            market.trading_program,
            &cursor_output,
        ),
        verified: verified_output,
        manifest: manifest_output,
        selection_account: envelope(
            GeneralLocalStateKindV3::Selection,
            width,
            GeneralStateAddressSeedsV3::selection(root_seed, batch_id).expect("Selection seeds"),
            market.trading_program,
            &selection,
        ),
        settlement_account: envelope(
            GeneralLocalStateKindV3::Settlement,
            width,
            GeneralStateAddressSeedsV3::settlement(root_seed, candidate_id)
                .expect("Settlement seeds"),
            market.trading_program,
            &settlement,
        ),
    }
}

/// The exact primary state and evidence one action names, and nothing else.
///
/// The table is the whole per-action surface of the deriver stated once from
/// the campaign's side: an action gets the account its own profile declares as
/// its primary state and the records its own profile declares as evidence. A
/// row that handed an action a record it does not name would be refused at a
/// named line, which is what
/// `each_action_refuses_the_evidence_shape_another_one_needs` executes.
fn action_input<'a>(
    market: &'a LiveMarketV1,
    action: Action,
    records: &'a LiveRecordsV1,
) -> GeneralRequestInputV1<'a> {
    let (primary_state_account, evidence) = match action {
        // The two batch actions and the two order-in-batch actions name the
        // Batch window as their primary state; `OpenBatch` creates it.
        Action::OpenBatch => (None, GeneralRequestEvidenceV1::default()),
        Action::CloseBatch => (
            Some(records.batch_account.as_slice()),
            GeneralRequestEvidenceV1::default(),
        ),
        Action::PlaceOrder => (
            Some(records.batch_account.as_slice()),
            GeneralRequestEvidenceV1 {
                signed_order_terms: Some(&records.signed_terms),
                ..GeneralRequestEvidenceV1::default()
            },
        ),
        Action::CancelOrder => (
            Some(records.batch_account.as_slice()),
            GeneralRequestEvidenceV1 {
                order_account: Some(&records.order_account),
                ..GeneralRequestEvidenceV1::default()
            },
        ),
        Action::ReleaseOrder => (
            Some(records.order_account.as_slice()),
            GeneralRequestEvidenceV1::default(),
        ),
        Action::SubmitCandidate => (
            None,
            GeneralRequestEvidenceV1 {
                candidate_image: Some(&records.candidate_image),
                ..GeneralRequestEvidenceV1::default()
            },
        ),
        // A verifier account that is still vacant IS the candidate's first row,
        // and the runtime accepts `(0, 0, 0)` only against that vacancy.
        Action::VerifyCandidateRow => (
            Some(records.candidate_account.as_slice()),
            GeneralRequestEvidenceV1 {
                verifier_account: Some(&records.verifier_account),
                ..GeneralRequestEvidenceV1::default()
            },
        ),
        Action::CloseCandidate => (
            Some(records.candidate_account.as_slice()),
            GeneralRequestEvidenceV1::default(),
        ),
        Action::Consider => (
            Some(records.selection_account.as_slice()),
            GeneralRequestEvidenceV1 {
                verified_candidate: Some(&records.verified),
                ..GeneralRequestEvidenceV1::default()
            },
        ),
        Action::Freeze => (
            Some(records.selection_account.as_slice()),
            GeneralRequestEvidenceV1::default(),
        ),
        Action::InitializeSettlement => (
            None,
            GeneralRequestEvidenceV1 {
                verified_candidate: Some(&records.verified),
                ..GeneralRequestEvidenceV1::default()
            },
        ),
        Action::Collect | Action::Distribute => (
            Some(records.settlement_account.as_slice()),
            GeneralRequestEvidenceV1 {
                settlement_manifest: Some(&records.manifest),
                ..GeneralRequestEvidenceV1::default()
            },
        ),
        Action::Materialize | Action::Close => (
            Some(records.settlement_account.as_slice()),
            GeneralRequestEvidenceV1::default(),
        ),
    };
    GeneralRequestInputV1 {
        action,
        root: market.root,
        root_address: market.root_address,
        config: &market.config,
        outcome_count: market.outcome_count,
        product_id: market.product_id,
        trading_program: market.trading_program,
        primary_state_account,
        evidence,
    }
}

/// One founded market, one collection half, and the fifteen requests it admits.
fn live_market() -> (LiveMarketV1, LiveRecordsV1) {
    let config = open_config(9);
    let config_id = solana_program::hash::hash(&config).to_bytes();
    let mut market = LiveMarketV1 {
        config,
        root: GeneralRootV2::active([0x61; 32], config_id, 9).expect("active root"),
        root_address: Pubkey::new_from_array([0x62; 32]),
        trading_program: waist().trading_program,
        product_id: [0x63; 32],
        outcome_count: OUTCOME_COUNT,
    };
    let records = live_records(&mut market);
    (market, records)
}

/// EVERY action derives a request, and every request is canonical to two
/// independent readers.
///
/// The bar is deliberately the pair the chain applies before any semantics run:
/// `ControllerRequestV3::decode` is the codec's own per-action canonical form
/// -- which subject, which coordinates and which bump witnesses that action may
/// carry -- and `validate_request` against
/// `general_request_profile_v1(action)` is the same pass
/// `authenticate_general_artifacts_v3` runs on chain before Trading projects a
/// register. A request that satisfies both is one the selected artifacts admit;
/// a fifteen-arm derivation that satisfied neither would have been invisible
/// until a real ELF refused it with `AdmittedTransport`.
///
/// It is TOTAL over `GENERAL_ACTIONS_V5` rather than a list, so a sixteenth
/// action cannot join without a deriver.
#[test]
fn every_general_action_derives_a_request_its_own_profile_admits() {
    let (market, records) = live_market();
    for action in GENERAL_ACTIONS_V5 {
        let derived = derive_general_request_v1(action_input(&market, action, &records))
            .unwrap_or_else(|error| panic!("derive {action:?}: {error:?}"));
        assert_eq!(derived.action, action);
        // THE READER IS THE CHAIN'S, NOT ONE GENERATION'S. Seven of the
        // fifteen actions kept the `DCGREQ02` wire and eight speak `DCGREQ03`;
        // `decode_general_request_v3` selects the generation off the shared
        // selector byte exactly as the accelerator boundary does, so a request
        // built in the wrong generation is refused here rather than at a real
        // ELF.
        let decoded = decode_general_request_v3(&derived.request)
            .unwrap_or_else(|error| panic!("canonical request for {action:?}: {error:?}"));
        assert_eq!(decoded.action, action);
        assert_eq!(decoded.candidate_id, derived.subject_id);
        assert_eq!(decoded.state_bump, derived.primary_state_bump);
        assert_eq!(decoded.terminal_record_bump, derived.secondary_state_bump);
        assert_eq!(decoded.result_state_bump, derived.result_state_bump);
        assert_ne!(derived.primary_state, market.root_address);
        validate_request(
            general_request_profile_v1(action).expect("published request profile"),
            0,
            &derived.request,
        )
        .unwrap_or_else(|error| panic!("{action:?} profile refuses its own request: {error:?}"));
    }
}

/// The subject and the state are two different coordinates, and eleven of the
/// fifteen actions prove it.
///
/// A deriver that returned the subject for both would pass every round-trip
/// above and derive the wrong account for `PlaceOrder`, `CancelOrder` and
/// `Consider` -- whose primary state is keyed on a batch while their subject is
/// an order or a candidate. This states the whole join table once.
#[test]
fn each_action_names_the_state_its_family_policy_selects() {
    let (market, records) = live_market();
    let root = market.root_address.to_bytes();
    let program = market.trading_program;
    let address = |seeds: GeneralStateAddressSeedsV3| {
        Pubkey::find_program_address(seeds.as_slices().expect("seed slices").as_slice(), &program).0
    };
    let batch = address(GeneralStateAddressSeedsV3::batch(root, records.batch_id).expect("batch"));
    let order = address(GeneralStateAddressSeedsV3::order(root, records.order_id).expect("order"));
    let candidate = address(
        GeneralStateAddressSeedsV3::candidate(root, records.candidate_id).expect("candidate"),
    );
    let selection =
        address(GeneralStateAddressSeedsV3::selection(root, records.batch_id).expect("selection"));
    let settlement = address(
        GeneralStateAddressSeedsV3::settlement(root, records.candidate_id).expect("settlement"),
    );
    // `OpenBatch` is the one action whose state does not yet exist: the market
    // already holds `records.batch_id` at sequence zero, so the open names the
    // NEXT occurrence and its address is derived from the subject it predicts.
    let opened = derive_general_request_v1(action_input(&market, Action::OpenBatch, &records))
        .expect("derive OpenBatch");
    let next = opened.subject_id.expect("OpenBatch names its occurrence");
    assert_ne!(next, records.batch_id, "the open re-named the live batch");
    assert_eq!(
        opened.primary_state,
        address(GeneralStateAddressSeedsV3::batch(root, next).expect("next batch")),
    );
    for (action, expected_state, expected_subject) in [
        (Action::CloseBatch, batch, Some(records.batch_id)),
        (Action::PlaceOrder, batch, Some(records.order_id)),
        (Action::CancelOrder, batch, Some(records.order_id)),
        (Action::ReleaseOrder, order, Some(records.order_id)),
        (
            Action::SubmitCandidate,
            candidate,
            Some(records.candidate_id),
        ),
        (
            Action::VerifyCandidateRow,
            candidate,
            Some(records.candidate_id),
        ),
        (
            Action::CloseCandidate,
            candidate,
            Some(records.candidate_id),
        ),
        (Action::Consider, selection, Some(records.candidate_id)),
        (Action::Freeze, selection, None),
        (
            Action::InitializeSettlement,
            settlement,
            Some(records.candidate_id),
        ),
        (Action::Collect, settlement, Some(records.candidate_id)),
        (Action::Materialize, settlement, Some(records.candidate_id)),
        (Action::Distribute, settlement, Some(records.candidate_id)),
        (Action::Close, settlement, Some(records.candidate_id)),
    ] {
        let derived = derive_general_request_v1(action_input(&market, action, &records))
            .unwrap_or_else(|error| panic!("derive {action:?}: {error:?}"));
        assert_eq!(derived.primary_state, expected_state, "{action:?} state");
        assert_eq!(derived.subject_id, expected_subject, "{action:?} subject");
    }
}

/// The four actions that name a second account derive it, and the eleven that
/// do not leave both witnesses zero.
#[test]
fn the_secondary_and_result_states_are_exactly_the_four_actions_that_declare_them() {
    let (market, records) = live_market();
    let root = market.root_address.to_bytes();
    let program = market.trading_program;
    let address = |seeds: GeneralStateAddressSeedsV3| {
        Pubkey::find_program_address(seeds.as_slices().expect("seed slices").as_slice(), &program).0
    };
    let mut secondary_actions = Vec::new();
    let mut result_actions = Vec::new();
    for action in GENERAL_ACTIONS_V5 {
        let derived = derive_general_request_v1(action_input(&market, action, &records))
            .unwrap_or_else(|error| panic!("derive {action:?}: {error:?}"));
        assert_eq!(
            derived.secondary_state.is_some(),
            derived.secondary_state_bump != 0,
            "{action:?} secondary address and bump disagree about existing",
        );
        assert_eq!(
            derived.result_state.is_some(),
            derived.result_state_bump != 0,
            "{action:?} result address and bump disagree about existing",
        );
        if derived.secondary_state.is_some() {
            secondary_actions.push(action);
        }
        if derived.result_state.is_some() {
            result_actions.push(action);
        }
        match action {
            Action::PlaceOrder | Action::CancelOrder => assert_eq!(
                derived.secondary_state,
                Some(address(
                    GeneralStateAddressSeedsV3::order(root, records.order_id).expect("order")
                )),
                "{action:?} names its order record",
            ),
            Action::VerifyCandidateRow => {
                assert_eq!(
                    derived.secondary_state,
                    Some(address(
                        GeneralStateAddressSeedsV3::verifier(root, records.candidate_id)
                            .expect("verifier")
                    )),
                );
                assert_eq!(
                    derived.result_state,
                    Some(address(
                        GeneralStateAddressSeedsV3::verified_candidate(root, records.candidate_id)
                            .expect("verified")
                    )),
                );
            }
            _ => {}
        }
    }
    assert_eq!(
        secondary_actions,
        vec![
            Action::Close,
            Action::PlaceOrder,
            Action::CancelOrder,
            Action::VerifyCandidateRow,
        ],
        "the actions declaring a secondary state moved",
    );
    assert_eq!(
        result_actions,
        vec![Action::VerifyCandidateRow],
        "only VerifyCandidateRow may carry a result-state witness",
    );
}

/// An action handed another action's record refuses at a named line.
#[test]
fn each_action_refuses_the_evidence_shape_another_one_needs() {
    let (market, records) = live_market();
    let strip = |mut input: GeneralRequestInputV1<'_>| {
        input.evidence = GeneralRequestEvidenceV1::default();
        derive_general_request_v1(input)
    };
    // Each of the five arms that reads a record its primary state does not
    // carry says so rather than deriving an address from a vacancy.
    for action in [
        Action::PlaceOrder,
        Action::CancelOrder,
        Action::SubmitCandidate,
        Action::Consider,
        Action::InitializeSettlement,
        Action::Collect,
        Action::Distribute,
    ] {
        assert!(
            matches!(
                strip(action_input(&market, action, &records)),
                Err(BuilderError::Binding(_))
            ),
            "{action:?} accepted an execution with none of the evidence it names",
        );
    }
    // A Batch envelope where an Order is named, and the reverse: the kind check
    // is what keeps a mismatched record from reaching a decoder that would only
    // say the bytes were the wrong width.
    let mut swapped = action_input(&market, Action::ReleaseOrder, &records);
    swapped.primary_state_account = Some(&records.batch_account);
    assert!(matches!(
        swapped_result(swapped),
        Err(BuilderError::Binding(_))
    ));
    let mut swapped = action_input(&market, Action::CancelOrder, &records);
    swapped.evidence.order_account = Some(&records.candidate_account);
    assert!(matches!(
        swapped_result(swapped),
        Err(BuilderError::Binding(_))
    ));
    // The settlement four read their coordinate off the cursor, and a selection
    // cursor is not one.
    let mut swapped = action_input(&market, Action::Materialize, &records);
    swapped.primary_state_account = Some(&records.selection_account);
    assert!(matches!(
        swapped_result(swapped),
        Err(BuilderError::Binding(_))
    ));
}

fn swapped_result(input: GeneralRequestInputV1<'_>) -> Result<GeneralRequestV1, BuilderError> {
    derive_general_request_v1(input)
}

/// TWO BATCHES ON ONE MARKET SELECT SEPARATELY, and the address is what says so.
///
/// Until `6ce8929ed` `GENERAL_SELECTION_STATE_RECIPE_V3` was keyed by the root
/// ALONE -- "one per General root" -- and nothing writes a frozen selection back
/// to `Open`, so after the first `Freeze` a market could open, fill and close as
/// many batches as it liked and could CLEAR in exactly one. General as built was
/// one call auction per Market. The recipe carries the batch identity now, and
/// this is its first exercise: the builder is where a campaign learns which
/// cursor an action names, and until this commit it could not derive one at all.
///
/// `Consider` takes its batch coordinate from the certificate it considers --
/// the first consideration of a batch CREATES the cursor, so it cannot come from
/// the cursor -- and `Freeze` takes it from the cursor itself. Both derivations
/// have to land on the same address for the same batch and on different
/// addresses for different ones, which is the whole property.
#[test]
fn a_second_batch_on_one_market_derives_its_own_selection_cursor() {
    let (mut market, first) = live_market();
    let second = live_records(&mut market);
    assert_ne!(
        first.batch_id, second.batch_id,
        "the second occurrence is a different batch identity",
    );
    let derive = |action, records: &LiveRecordsV1| {
        derive_general_request_v1(action_input(&market, action, records))
            .unwrap_or_else(|error| panic!("derive {action:?}: {error:?}"))
    };
    let first_consider = derive(Action::Consider, &first);
    let second_consider = derive(Action::Consider, &second);
    let first_freeze = derive(Action::Freeze, &first);
    let second_freeze = derive(Action::Freeze, &second);
    assert_eq!(
        first_freeze.primary_state, first_consider.primary_state,
        "the freeze must close the cursor the consideration opened",
    );
    assert_eq!(
        second_freeze.primary_state, second_consider.primary_state,
        "the freeze must close the cursor the consideration opened",
    );
    assert_ne!(
        first_consider.primary_state, second_consider.primary_state,
        "two batches on one market would clear into one cursor",
    );
    // The subjects move for the ordinary reason -- two batches carry two
    // candidates -- and the point is that the STATE moves as well. A recipe
    // that reverted to the root alone would keep both subjects and collapse
    // both addresses, which is exactly the shape the assertion above forbids
    // and the shape a subject-only assertion would have missed.
    assert_ne!(first_consider.subject_id, second_consider.subject_id);
    assert_eq!(first_freeze.subject_id, None);
    assert_eq!(second_freeze.subject_id, None);
}
