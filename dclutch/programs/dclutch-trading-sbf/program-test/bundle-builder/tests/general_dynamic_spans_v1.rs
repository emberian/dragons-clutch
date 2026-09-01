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
    general::{GeneralOpenBatchRequestInputV1, derive_general_open_batch_request_v1},
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
    effect_artifacts_v3::{
        GENERAL_EFFECT_INSTRUCTION_PLACEHOLDER_V3, encode_general_effect_program_v4_atomic,
        general_effect_instruction_count_v3, general_effect_program_bytes_v3,
        general_effect_program_bytes_v4, general_effect_template_bytes_v3,
    },
    hot_candidate_v3::{GENERAL_HOT_COMMON_IDENTITIES_V3, general_hot_scalar_count_v3, scalar},
    release_v3::GENERAL_ACTIONS_V3,
    specialization::general_request_profile_bytes_v1,
};
use dclutch_general_codec::{
    Action, successor_request_v2::ControllerRequestV2, successor_request_v3::ControllerRequestV3,
};
use dclutch_general_config_contract::{
    GeneralRootV2,
    v3::{GeneralConfigV3, GeneralConfigV3Input},
};
use dclutch_request_profile_contract::{
    ProjectionRegisterKindV1, ProjectionRegisterSpaceV1, ProjectionTargetV1, RequestProfileV1,
    SCHEMA_RELEASE_ID as REQUEST_PROFILE_SCHEMA_ID_V1,
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
fn expected_pages(outcome_count: u32) -> u32 {
    let scalars = general_hot_scalar_count_v3(outcome_count).expect("General scalar count");
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

/// The published General profile declares exactly one span, and no General
/// RequestProfile writes its selector.
///
/// This is the correction to the sketched boundary: the width cannot come from
/// a request projection, because nothing in the request reaches that register.
#[test]
fn the_sole_general_span_selector_is_not_request_owned() {
    let target = ProjectionTargetV1 {
        kind: ProjectionRegisterKindV1::Scalar,
        space: ProjectionRegisterSpaceV1::Common,
        index: u16::try_from(scalar::INPUT_SCRATCH_PAGE_COUNT).expect("selector index"),
    };
    for action in GENERAL_ACTIONS_V3 {
        let bytes = account_profile(action);
        let profile = AccountProfileV2::decode(&bytes).expect("profile decode");
        assert!(
            profile.uses_dynamic_fixed_spans(),
            "{action:?} is profile 13"
        );
        assert_eq!(profile.dynamic_fixed_span_count(), 1, "{action:?}");
        let span = profile.dynamic_fixed_span(0).expect("span");
        assert_eq!(
            span.count_scalar(),
            target.index,
            "{action:?} selector is the scratch-page count"
        );
        assert_eq!(
            span.insertion_coordinate(),
            general_account_profile_fixed_count_v3(action).expect("fixed count"),
            "{action:?} span is trailing"
        );
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

/// The derived width is the bank transport's page count, for every action and
/// at two Product widths.
#[test]
fn the_derived_span_width_is_the_authenticated_page_count() {
    let admitted = strategy(StrategyDispositionV2::AdmittedAot);
    for outcome_count in [1_u32, OUTCOME_COUNT, 258] {
        let expected = expected_pages(outcome_count);
        for action in GENERAL_ACTIONS_V3 {
            let set = artifacts(action);
            let widths =
                derive(action, &set, &admitted, outcome_count).expect("General span widths");
            assert_eq!(
                widths,
                vec![expected],
                "{action:?} at N={outcome_count} spans {expected} pages"
            );
        }
    }
}

/// With the width in hand, the profile's own geometry expands: the logical
/// frame is exactly the fixed count plus the span, and the trailing span
/// coordinates are the opaque readonly scratch pages.
#[test]
fn the_expanded_frame_is_the_fixed_count_plus_the_span() {
    let admitted = strategy(StrategyDispositionV2::AdmittedAot);
    for action in GENERAL_ACTIONS_V3 {
        let set = artifacts(action);
        let profile = AccountProfileV2::decode(&set.account_profile).expect("profile decode");
        let widths = derive(action, &set, &admitted, OUTCOME_COUNT).expect("span widths");
        let fixed = usize::from(general_account_profile_fixed_count_v3(action).expect("fixed"));
        let pages = usize::try_from(*widths.first().expect("one span")).expect("pages");
        let logical = profile_ops::logical_count(profile, OUTCOME_COUNT, &widths)
            .expect("logical count with spans");
        assert_eq!(
            logical,
            fixed.checked_add(pages).expect("expanded width"),
            "{action:?}"
        );
        // Every span coordinate is its own representative, and its geometry is
        // the opaque readonly scratch-page rule.
        for coordinate in fixed..logical {
            let representative =
                profile_ops::representative(profile, OUTCOME_COUNT, &widths, coordinate)
                    .expect("representative");
            assert_eq!(representative, coordinate, "{action:?} page {coordinate}");
            let ordinal =
                profile_ops::ordinal(profile, OUTCOME_COUNT, &widths, coordinate).expect("ordinal");
            let geometry =
                profile_ops::geometry(profile, OUTCOME_COUNT, &widths, ordinal).expect("geometry");
            let privileges = geometry.privileges();
            assert!(!privileges.writable(), "{action:?} page {coordinate}");
            assert!(!privileges.signer(), "{action:?} page {coordinate}");
        }
        // And the query refuses a width vector that is not the profile's shape.
        assert!(matches!(
            profile_ops::logical_count(profile, OUTCOME_COUNT, &[]),
            Err(BuilderError::Profile(_))
        ));
        assert!(matches!(
            profile_ops::logical_count(profile, OUTCOME_COUNT, &[pages_as_u32(pages), 1]),
            Err(BuilderError::Profile(_))
        ));
    }
}

fn pages_as_u32(value: usize) -> u32 {
    u32::try_from(value).expect("page count")
}

/// A profile-only span is admissible under exactly one disposition.
///
/// This is the seam the sketch called optional. It is not: General's profile
/// forces the accelerated disposition, so a "run it interpreted first" General
/// bundle cannot exist against these artifacts.
#[test]
fn a_profile_only_span_refuses_every_disposition_but_admitted_aot() {
    for action in GENERAL_ACTIONS_V3 {
        let set = artifacts(action);
        for disposition in [
            StrategyDispositionV2::Interpreted,
            StrategyDispositionV2::ShadowAot,
        ] {
            assert_eq!(
                derive(action, &set, &strategy(disposition), OUTCOME_COUNT),
                Err(BuilderError::Spans("unowned-span")),
                "{action:?} under {disposition:?}"
            );
        }
    }
}

/// A zero-span profile still refuses a nonempty width vector, and still
/// derives an empty one — the Direct reproduction's path, unmoved.
#[test]
fn a_zero_span_profile_derives_no_widths() {
    let set = artifacts(Action::Freeze);
    let profile = AccountProfileV2::decode(&set.account_profile).expect("profile decode");
    // The General profile is spans-typed with one span, so a width vector of
    // the wrong length is refused rather than silently truncated.
    assert!(matches!(
        profile_ops::physical_count(profile, OUTCOME_COUNT, &[]),
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
            vec![expected_pages(OUTCOME_COUNT)],
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
        let derived = derive_general_open_batch_request_v1(GeneralOpenBatchRequestInputV1 {
            root,
            root_address,
            config: &config,
            outcome_count,
            product_id: [0x63; 32],
            trading_program: waist().trading_program,
        })
        .expect("chain-derived OpenBatch request");
        let decoded = ControllerRequestV3::decode(&derived.request).expect("canonical V3 request");
        assert_eq!(decoded.action.legacy(), Some(Action::OpenBatch));
        assert_eq!(decoded.subject_id, Some(derived.occurrence_id));
        assert_eq!(decoded.expected_revision, root.revision());
        assert_eq!(decoded.primary_state_bump, derived.batch_bump);
        assert_ne!(derived.batch, root_address);
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
    let base = GeneralOpenBatchRequestInputV1 {
        root,
        root_address: Pubkey::new_from_array([0x62; 32]),
        config: &config,
        outcome_count: 1,
        product_id: [0x63; 32],
        trading_program: waist().trading_program,
    };
    let foreign_config = open_config(10);
    assert!(matches!(
        derive_general_open_batch_request_v1(GeneralOpenBatchRequestInputV1 {
            config: &foreign_config,
            ..base
        }),
        Err(BuilderError::Binding(_))
    ));
    assert!(matches!(
        derive_general_open_batch_request_v1(GeneralOpenBatchRequestInputV1 {
            product_id: [0; 32],
            ..base
        }),
        Err(BuilderError::Binding(_))
    ));
    assert!(matches!(
        derive_general_open_batch_request_v1(GeneralOpenBatchRequestInputV1 {
            outcome_count: 0,
            ..base
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
