//! Exact Profile13 account rules for the General successor.
//!
//! Child-frame order and privileges come directly from the Claims and Custody
//! semantic owners. Repeated appearances of the same semantic role are encoded
//! as authenticated route aliases, so no General-local physical account table
//! can drift from the child adapters. The sole dynamic span is the Trading-owned
//! authenticated scratch-page bank; its count comes from the protected scalar
//! derived from canonical register-bank geometry.

use dclutch_account_profile_contract::v2::{
    AccountPrestateV2, DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE, DYNAMIC_FIXED_SPAN_HEADER_BYTES,
    OPERATION_BYTES, RULE_BYTES, TrustedBuiltinIdentityV2, TrustedEnvironmentV2,
    TrustedIdentityEnvironmentV2,
    encode::{
        AccountAliasInputV2, AccountCoordinateV2, AccountEffectPermissionsV2,
        AccountOperationInputV2, AccountPrivilegesV2, AccountRuleInputV2,
        AccountRuleWithPrestateInputV2, IdentityCoordinateV2, RegisterGeometryV2,
        ScalarCoordinateV2,
        encode_account_profile_with_dynamic_fixed_span_v2_generated_atomic_with_item_operations,
    },
};
use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_HEADER_BYTES_V1,
    hot_v3::{
        HOT_RUNTIME_CONFIG_COORDINATE_V3, HOT_RUNTIME_PORTFOLIO_COORDINATE_V3,
        HOT_RUNTIME_PRODUCT_COORDINATE_V3, HOT_RUNTIME_ROOT_COORDINATE_V3,
    },
};
use dclutch_claims_svm::frame_spec_v1::{
    ClaimsFrameDataV1, ClaimsFrameRoleV1, ClaimsFrameSpecV1, FramePrivilegesV1,
};
use dclutch_custody_contract::{
    CustodyFrameDataV1, CustodyFramePrivilegesV1, CustodyFrameRoleV1, CustodyFrameSpecV1,
};
use dclutch_general_codec::{Action, SELECTION_POLICY_BYTES};
use dclutch_general_config_contract::{
    GENERAL_ROOT_CONFIG_ID_OFFSET_V2, GENERAL_ROOT_LIFECYCLE_OFFSET_V2,
    GENERAL_ROOT_MARKET_OFFSET_V2, GENERAL_ROOT_NEXT_BATCH_SEQUENCE_OFFSET_V2,
    GENERAL_ROOT_OPEN_BATCHES_OFFSET_V2, GENERAL_ROOT_REVISION_OFFSET_V2,
    v3::{GENERAL_CONFIG_BYTES_V3, GeneralConfigV3Layout},
};
use dclutch_product_runtime_v2::{
    PORTFOLIO_CLAIM_BASIS_ID_OFFSET, PORTFOLIO_COEFFICIENT_BYTES,
    PORTFOLIO_COEFFICIENT_COUNT_OFFSET, PORTFOLIO_HEADER_BYTES,
};
use dclutch_product_runtime_v2_admission::{
    PRODUCT_RECORD_BYTES_V2, PRODUCT_RECORD_PRODUCT_ID_OFFSET_V2,
};

use crate::{
    artifacts_v3::GENERAL_PRODUCT_TAIL_COUNT_SCALAR_V3,
    candidate_v1::{GENERAL_CANDIDATE_BYTES_V1, GeneralCandidateLayoutV1},
    collection_v1::{
        GENERAL_BATCH_BYTES_V1, GENERAL_ORDER_HEADER_BYTES_V1, GENERAL_ORDER_ROW_BASE_V1,
        GENERAL_ORDER_ROW_DELIVER_OFFSET_V1, GENERAL_ORDER_ROW_RECEIVE_OFFSET_V1,
        GENERAL_ORDER_ROW_STRIDE_V1, GeneralBatchLayoutV1, GeneralOrderLayoutV1,
    },
    effect_artifacts_v3::{
        GeneralChildFrameV3, general_custody_callee_account_count_v3,
        general_custody_callee_coordinate_v3, general_effect_account_count_v3,
        general_effect_route_count_v3, general_effect_route_frame_v3,
    },
    hot_candidate_v3::{
        GENERAL_HOT_COMMON_IDENTITIES_V3, GENERAL_HOT_COMMON_SCALARS_V3,
        general_hot_item_scalar_stride_v3, identity, item_scalar, scalar,
    },
    local_state_v3::{GENERAL_LOCAL_STATE_HEADER_BYTES_V3, GeneralLocalStateLayoutV3},
    runtime_manifest::SETTLEMENT_MANIFEST_HEADER_BYTES_V2,
    runtime_selection::RUNTIME_SELECTION_CURSOR_BYTES_V2,
    runtime_verify::RUNTIME_VERIFIER_HEADER_BYTES_V2,
    runtime_width::{
        CANDIDATE_HEADER_BYTES_V2, CandidateLayoutV2, PAGE_HEADER_BYTES_V2,
        SETTLEMENT_CURSOR_HEADER_BYTES_V2, VERIFIED_CANDIDATE_HEADER_BYTES_V2,
    },
    state_artifacts_v3::{
        GENERAL_CLOSE_PAYER_ACCOUNT_V3, GENERAL_CLOSE_RENT_CREDIT_ACCOUNT_V3,
        GENERAL_PRIMARY_PAYER_ACCOUNT_V3, GENERAL_PRIMARY_RENT_CREDIT_ACCOUNT_V3,
        GENERAL_PRIMARY_STATE_ACCOUNT_V3, GENERAL_TERMINAL_STATE_ACCOUNT_V3,
        GENERAL_VERIFY_PAYER_ACCOUNT_V3, GENERAL_VERIFY_RENT_CREDIT_ACCOUNT_V3,
        GENERAL_VERIFY_RESULT_STATE_ACCOUNT_V3, GENERAL_VERIFY_VERIFIER_STATE_ACCOUNT_V3,
        GeneralReadonlyEvidenceKindV3, general_readonly_evidence_count_v3,
        general_readonly_evidence_v3,
    },
};

/// Profile13 discriminator required by every successor General account artifact.
pub const GENERAL_ACCOUNT_PROFILE_ARTIFACT_V3: u16 = DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE;

/// Release-selected external account widths not owned by General.
///
/// Every value is either an exact fixed width or a checked nonzero prefix for
/// an adapter-authenticated variable record. The selected release builder gets
/// these values from the named semantic owner; no runtime caller controls them.
///
/// The Realm-selected collateral Mint, token Account and Token Program widths
/// are deliberately absent: those three belong to the token program the Realm
/// selected and to the loader that deployed it, never to General. Their
/// coordinates are emitted opaque, so there is no value here for a caller to
/// supply and no width for this profile to assert.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralExternalAccountWidthsV3 {
    /// Checked nonzero linked-basis prefix.
    pub linked_basis_prefix: u32,
    /// Finalized result-domain record width.
    pub result_domain: u32,
    /// Runtime Rent sysvar width.
    pub rent_sysvar: u32,
    /// Canonical Core Market width.
    pub core_market: u32,
    /// Current Registry activation-cache width.
    pub activation_cache: u32,
    /// Loader-v3 Program account width.
    pub upgradeable_program: u32,
    /// Checked nonzero Trading ProgramData prefix.
    pub trading_programdata_prefix: u32,
    /// Checked nonzero Claims ProgramData prefix.
    pub claims_programdata_prefix: u32,
    /// Checked nonzero Core ProgramData prefix.
    pub core_programdata_prefix: u32,
    /// Immutable Realm record width.
    pub realm_record: u32,
    /// Canonical RentCredit width.
    pub rent_credit: u32,
}

impl GeneralExternalAccountWidthsV3 {
    fn validate(self) -> Result<()> {
        if self.linked_basis_prefix == 0
            || self.result_domain == 0
            || self.rent_sysvar == 0
            || self.core_market == 0
            || self.activation_cache == 0
            || self.upgradeable_program == 0
            || self.trading_programdata_prefix == 0
            || self.claims_programdata_prefix == 0
            || self.core_programdata_prefix == 0
            || self.realm_record == 0
            || self.rent_credit == 0
        {
            Err(GeneralAccountRuleErrorV3::ExternalWidth)
        } else {
            Ok(())
        }
    }
}

/// Stable refusal from General Profile13 rule generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralAccountRuleErrorV3 {
    /// An action-selected coordinate or child frame was outside its exact range.
    Geometry,
    /// A release-selected external width was zero or otherwise unusable.
    ExternalWidth,
}

/// Result alias for General Profile13 rule generation.
pub type Result<T> = core::result::Result<T, GeneralAccountRuleErrorV3>;

/// First coordinate past the action's last child-route range.
///
/// This is the fixed count minus the trailing Custody callee, and it is what
/// every child-frame walk must bound itself by: the callee belongs to no frame.
fn general_child_frame_end_v3(action: Action) -> Result<u16> {
    // The PRE-System count, not the profile's fixed count. Two accounts are now
    // appended past every child frame -- the Custody callee, which this already
    // stepped back over, and the System program behind it -- and a walk bounded
    // by the count that includes them asks `child_coordinate` to place an
    // account belonging to no frame, which returns `Geometry` and takes every
    // privilege union in the action down with it. That is exactly how the
    // System append first surfaced: as coordinate 2 of Consider losing its
    // rule, four coordinates away from anything that moved.
    crate::effect_artifacts_v3::general_effect_account_count_before_system_v3(action)
        .checked_sub(general_custody_callee_account_count_v3(action))
        .ok_or(GeneralAccountRuleErrorV3::Geometry)
}

/// Exact base logical account count before authenticated scratch pages.
pub fn general_account_profile_fixed_count_v3(action: Action) -> Result<u16> {
    general_effect_account_count_v3(action).map_err(|_| GeneralAccountRuleErrorV3::Geometry)
}

/// Exact count of canonical fixed AccountProfile operations for one action.
///
/// The operation list belongs next to the rules for the same reason the rules
/// belong here: it is part of the artifact, and a second author will drift. It
/// used to be hand-written once in the release builder and once again in a test
/// fixture, with nothing able to compare them -- `AccountProfileV2::operation`
/// is private, so admission cannot read the operations back out of the encoded
/// bytes. That is why the root-lifecycle conjunct below is fail-closed rather
/// than admission-checked: an artifact that omits it leaves scalar
/// `ROOT_LIFECYCLE_OBSERVATION` at zero, which is not
/// `GeneralLifecycleV2::Active`, and every action refuses.
///
/// The root-identity projection is NOT fail-closed in that sense and cannot be
/// made so, which is why it is pinned by a test instead: see
/// [`general_root_identity_operation_index_v3`].
#[must_use]
pub const fn general_account_profile_operation_count_v3(action: Action) -> u16 {
    // Every count below gained ONE on 2026-09-01: the semantic-basis
    // projection, appended at each action's own last coordinate so that no
    // existing index moved. See the arm that emits it for why the register
    // needed a source at all.
    //
    // TEN of them gained one more on 2026-09-04: the Market generation, which
    // `authenticated_general_domain` requires of every action and which only
    // five profiles wrote. The other five are unchanged because the operation
    // they already carried is the one the derived index names. See
    // `general_generation_operation_index_v3`.
    match action {
        Action::SubmitCandidate => 38,
        Action::VerifyCandidateRow => 17,
        Action::CloseCandidate => 24,
        Action::OpenBatch => 24,
        Action::CloseBatch => 22,
        Action::PlaceOrder => 35,
        Action::CancelOrder => 33,
        Action::ReleaseOrder => 21,
        Action::Close => 15,
        // The two sources of the selection deadline
        // `GeneralTransitionV3.lean`'s `.freeze` arm compares the clock
        // against: the batch's own collection close, out of the evidence
        // record, and the config's selection window.
        Action::Freeze => 13,
        Action::Consider
        | Action::InitializeSettlement
        | Action::Collect
        | Action::Materialize
        | Action::Distribute => 11,
    }
}

/// Number of operations evaluated once per Product outcome.
///
/// PlaceOrder's signed terms carry one receive/deliver pair per outcome. The
/// source is a fixed evidence account, but the destinations are item registers,
/// so these two affine projections belong to Profile13's item-operation body.
const fn general_account_profile_item_operation_count_v3(action: Action) -> u16 {
    if matches!(action, Action::PlaceOrder) {
        2
    } else {
        0
    }
}

/// Operations evaluated once per action, before any Product-item tail.
const fn general_account_profile_fixed_operation_count_v3(action: Action) -> u16 {
    general_account_profile_operation_count_v3(action)
        .saturating_sub(general_account_profile_item_operation_count_v3(action))
}

/// Index of the sole operation naming the General root in the seed register.
///
/// Every one of the eight seed orders in [`crate::state_seeds_v3`] opens with
/// `CommonIdentity(GENERAL_ROOT_IDENTITY_REGISTER_V3)`, and until this
/// operation existed NOTHING wrote that register: not an AccountProfile
/// operation, not one of the fifteen Lean-emitted RequestProfiles (whose
/// identity destinations are exactly `{0, 3, 29}`), and not a trusted
/// environment (General declares `CurrentSlot`, `CurrentExecutingProgram` and
/// `SystemProgram`, none of which is the root). The executor therefore derived
/// every General state address from 32 zero bytes where the root belongs --
/// a well-formed address, identical for every General root in existence, so two
/// roots would collide on one occurrence identity. `crates/dclutch-operator`
/// injected the key host-side before consulting the same policy, so the host
/// and the chain derived different addresses from one artifact.
///
/// It is the LAST fixed operation deliberately. Appending leaves every earlier
/// ordinal, and therefore every earlier operation's bytes, exactly where they
/// were; and the register it writes is read by the lifecycle adapter after the
/// whole projection chain has run, never by another operation, so its position
/// among the operations is semantically free while every other pair's relative
/// order is load-bearing.
/// Index of the operation that sources the semantic basis into its register.
///
/// WHY THIS IS DERIVED AND NOT A NUMBER. The fixed operation body ends in a
/// TWO-operation block whose indices are both computed -- the creation-payer
/// owner anchor, then the root identity -- and the root identity documents that
/// it must stay last. A literal index appended past them moved them and left
/// their old ordinals handled by nothing; a literal index placed at the root
/// identity's old slot was silently claimed by the owner anchor, which had
/// shifted into it. Both attempts surfaced as `Geometry` on `Consider`, which is
/// the cheapest action and therefore the first to run out of arms.
///
/// So this one is computed too, and sits immediately in front of that block:
/// two back from the root identity where an owner anchor exists, one back for
/// `SubmitCandidate` and `CloseCandidate`, which have none. Every EARLIER
/// ordinal keeps its position and its bytes, which is the property the root
/// identity's own comment exists to protect.
/// Index of the operation that sources the Product record's digest.
///
/// Sits immediately in front of the semantic basis, which sits in front of the
/// two-operation derived tail. Derived for the reason that one is: a literal
/// anywhere near a span whose start can move stops being reached without
/// anything going red, and surfaces as `Geometry` on `Consider` -- the cheapest
/// action, and so the first to run out of arms.
/// Index of the operation that sources the config's Market generation.
///
/// DERIVED FOR ALL FIFTEEN, because the conjunct that needs it is not one
/// action's. `authenticated_general_domain` calls
/// `GeneralConfigV3::require_market(environment.generation, ..)` before any
/// action does anything else, and `general_hot_environment_from_bank_v3` reads
/// that generation out of `scalar::GENERATION`. Nothing else in the executing
/// frame writes that register -- not the fifteen Lean-emitted RequestProfiles,
/// whose identity destinations are exactly `{0, 3, 29}` and whose scalars do
/// not name it, and not a trusted environment, which is
/// `CurrentSlot`/`CurrentExecutingProgram`/`SystemProgram`. So a profile that
/// omits this operation makes its own action UNEXECUTABLE against any founded
/// market whose generation is nonzero, and does it silently: the register is a
/// well-formed zero and the refusal is `ConfigMarket`, which reads like a
/// caller naming the wrong market.
///
/// It was a LITERAL on five actions and absent from the other ten. Five of the
/// ten are also joined a second time, deeper, by their own candidate
/// projector (`root.generation() != environment.generation`), and it was one
/// of those -- `CloseBatch` -- that surfaced it on 2026-09-04 through the real
/// Trading ELF: `env.gen=0 root.gen=9`. The literals said the register was a
/// property of the five actions that happened to have one, which is exactly
/// the shape that let ten omissions sit unremarked. Derived, the operation
/// exists for every action by construction and cannot be omitted by a
/// sixteenth.
///
/// It sits immediately in front of the System-program bind, or of the product
/// digest for `CloseCandidate`, which declares no System identity. For four of
/// the five actions that carried a literal this IS that literal;
/// `CancelOrder`'s generation and payer projections swap, which is free --
/// `OP_PROJECT_KEY` and `OP_PROJECT_DATA_U64` write different banks and
/// neither reads the other.
const fn general_generation_operation_index_v3(action: Action) -> u16 {
    match general_system_program_operation_index_v3(action) {
        Some(system) => system.saturating_sub(1),
        None => general_product_digest_operation_index_v3(action).saturating_sub(1),
    }
}

/// Index of the operation binding the System-program coordinate to its identity.
///
/// Derived, in front of the product digest, for the reason the two behind it
/// are: a literal near a span whose start can move stops being reached with
/// nothing going red.
const fn general_system_program_operation_index_v3(action: Action) -> Option<u16> {
    if crate::state_artifacts_v3::general_declares_system_program_v3(action) {
        Some(general_product_digest_operation_index_v3(action).saturating_sub(1))
    } else {
        None
    }
}

const fn general_product_digest_operation_index_v3(action: Action) -> u16 {
    general_semantic_basis_operation_index_v3(action).saturating_sub(1)
}

const fn general_semantic_basis_operation_index_v3(action: Action) -> u16 {
    let tail = if matches!(action, Action::SubmitCandidate | Action::CloseCandidate) {
        1
    } else {
        2
    };
    general_root_identity_operation_index_v3(action).saturating_sub(tail)
}

const fn general_root_identity_operation_index_v3(action: Action) -> u16 {
    general_account_profile_fixed_operation_count_v3(action).saturating_sub(1)
}

/// Generate one exact canonical fixed AccountProfile operation.
///
/// Order is load-bearing: these bytes are the artifact, and the artifact's
/// digest is what the descriptor and the capability seal name.
pub fn general_account_profile_operation_v3(
    action: Action,
    index: u16,
) -> Result<AccountOperationInputV2> {
    let primary = AccountCoordinateV2::fixed(GENERAL_PRIMARY_STATE_ACCOUNT_V3);
    let terminal = AccountCoordinateV2::fixed(GENERAL_TERMINAL_STATE_ACCOUNT_V3);
    let creation_payer = if action == Action::VerifyCandidateRow {
        GENERAL_VERIFY_PAYER_ACCOUNT_V3
    } else if matches!(
        action,
        Action::Close | Action::PlaceOrder | Action::CancelOrder
    ) {
        GENERAL_CLOSE_PAYER_ACCOUNT_V3
    } else {
        GENERAL_PRIMARY_PAYER_ACCOUNT_V3
    };
    match index {
        // The Product-owned runtime width, from the authenticated Portfolio.
        0 => Ok(AccountOperationInputV2::ProjectTailCountU32 {
            account: AccountCoordinateV2::fixed(narrow(HOT_RUNTIME_PORTFOLIO_COORDINATE_V3)?),
            destination: ScalarCoordinateV2::common(GENERAL_PRODUCT_TAIL_COUNT_SCALAR_V3),
            data_offset: width(PORTFOLIO_COEFFICIENT_COUNT_OFFSET)?,
        }),
        1 => Ok(AccountOperationInputV2::ProjectDataU8 {
            account: primary,
            destination: common_scalar(scalar::PRIMARY_BUMP_OBSERVATION)?,
            data_offset: GeneralLocalStateLayoutV3::bump(),
        }),
        2 => Ok(AccountOperationInputV2::ProjectDataU64 {
            account: primary,
            destination: common_scalar(scalar::PRIMARY_PRINCIPAL_OBSERVATION)?,
            data_offset: GeneralLocalStateLayoutV3::rent_principal(),
        }),
        3 => Ok(AccountOperationInputV2::ProjectDataIdentity {
            account: primary,
            destination: common_identity(identity::PRIMARY_BENEFICIARY_OBSERVATION)?,
            data_offset: GeneralLocalStateLayoutV3::beneficiary(),
        }),
        // The capability root's own lifecycle byte, out of the mutable
        // `GeneralRootV2` tail behind the immutable common header. The header
        // proves identity and says nothing about whether the capability still
        // accepts work; without this projection a `Retiring` or `Retired`
        // General capability executes hot actions exactly like a live one.
        4 => Ok(AccountOperationInputV2::ProjectDataU8 {
            account: AccountCoordinateV2::fixed(narrow(HOT_RUNTIME_ROOT_COORDINATE_V3)?),
            destination: common_scalar(scalar::ROOT_LIFECYCLE_OBSERVATION)?,
            data_offset: width(
                CAPABILITY_ROOT_HEADER_BYTES_V1
                    .checked_add(GENERAL_ROOT_LIFECYCLE_OFFSET_V2)
                    .ok_or(GeneralAccountRuleErrorV3::Geometry)?,
            )?,
        }),
        // THE SELECTION DEADLINE'S TWO TERMS.
        //
        // `Freeze` read no clock at all until 2026-09-04, and could not: its
        // profile declared nine fixed accounts and ZERO readonly evidence, and
        // its primary state -- `RuntimeSelectionCursorV2` -- carries the batch
        // identity and no close slot, so no register held a deadline for
        // `currentSlot` to be compared against. That made early freeze live:
        // a solver who submitted one thin valid candidate, cranked its
        // consideration and froze in the same slot excluded every fuller
        // candidate (`MECHANISM_BATCH_SPINE_2026_09_04.md` §2(d)(i)).
        //
        // The batch record is the only account carrying the first term, so
        // `Freeze` names it as readonly evidence -- the smaller of the two
        // repairs `f66dbb078` named, and the same shape as `CloseBatch`'s own
        // projection of this field. The second term is the config's, out of
        // the account the runtime prefix already carries.
        //
        // Neither register is caller-trusted on its own: which batch is
        // presented here is joined against the cursor's `batch_id` by the
        // accelerator, and the `nonzero` in front of each in the Lean arm is
        // what refuses a projection that lost its source rather than admitting
        // a zero deadline.
        5 if action == Action::Freeze => Ok(AccountOperationInputV2::ProjectDataU64 {
            account: evidence_account(action, 0, GeneralReadonlyEvidenceKindV3::ClosedBatch)?,
            destination: common_scalar(scalar::BATCH_COLLECTION_CLOSE_SLOT)?,
            data_offset: batch_body_offset(GeneralBatchLayoutV1::COLLECTION_CLOSE_SLOT)?,
        }),
        6 if action == Action::Freeze => Ok(AccountOperationInputV2::ProjectDataU64 {
            account: AccountCoordinateV2::fixed(narrow(HOT_RUNTIME_CONFIG_COORDINATE_V3)?),
            destination: common_scalar(scalar::CONFIG_SELECTION_SLOTS)?,
            data_offset: width(GeneralConfigV3Layout::SELECTION_SLOTS)?,
        }),
        // Verify's mutable Candidate owns the refundable work escrow. Project
        // its actual balance independently of the body compartments so the
        // semantic projector can prove exact pre/post capitalization and the
        // Effect can assert the post-transfer balance.
        5 if action == Action::VerifyCandidateRow => Ok(AccountOperationInputV2::ProjectLamports {
            account: primary,
            destination: common_scalar(scalar::OBSERVED_POSITION_LAMPORTS)?,
        }),
        // The resumable verifier is lifecycle state coordinate 6. Its local
        // envelope observations are zero on the create branch and exact on the
        // authenticate branch; Lifecycle V5 remains the sole branch selector.
        6 if action == Action::VerifyCandidateRow => Ok(AccountOperationInputV2::ProjectDataU8 {
            account: AccountCoordinateV2::fixed(GENERAL_VERIFY_VERIFIER_STATE_ACCOUNT_V3),
            destination: common_scalar(scalar::TERMINAL_BUMP_OBSERVATION)?,
            data_offset: GeneralLocalStateLayoutV3::bump(),
        }),
        7 if action == Action::VerifyCandidateRow => Ok(AccountOperationInputV2::ProjectDataU64 {
            account: AccountCoordinateV2::fixed(GENERAL_VERIFY_VERIFIER_STATE_ACCOUNT_V3),
            destination: common_scalar(scalar::TERMINAL_PRINCIPAL_OBSERVATION)?,
            data_offset: GeneralLocalStateLayoutV3::rent_principal(),
        }),
        8 if action == Action::VerifyCandidateRow => {
            Ok(AccountOperationInputV2::ProjectDataIdentity {
                account: AccountCoordinateV2::fixed(GENERAL_VERIFY_VERIFIER_STATE_ACCOUNT_V3),
                destination: common_identity(identity::TERMINAL_BENEFICIARY_OBSERVATION)?,
                data_offset: GeneralLocalStateLayoutV3::beneficiary(),
            })
        }
        // The permissionless caller both pays any state creation and receives
        // the exact candidate-funded crank reward. Its transaction signature
        // authenticates the payer role, never the candidate semantics.
        9 if action == Action::VerifyCandidateRow => Ok(AccountOperationInputV2::ProjectKey {
            account: AccountCoordinateV2::fixed(GENERAL_VERIFY_PAYER_ACCOUNT_V3),
            destination: common_identity(identity::PAYER)?,
        }),
        // Candidate is an authenticated existing state, so its real lamport
        // balance may be projected only after anchoring its owner to the
        // trusted executing Trading program. The resumable Verifier remains
        // LifecycleBound: projecting its possibly-vacant lamports would be a
        // noncanonical lifecycle observation, and Verify consumes no such
        // scalar.
        10 if action == Action::VerifyCandidateRow => Ok(AccountOperationInputV2::RequireOwner {
            account: primary,
            expected: common_identity(identity::TRADING_PROGRAM)?,
        }),
        // Candidate close authenticates the persisted work compartments,
        // their physical balance, the joined Batch deadline, and the two
        // distinct lamport beneficiaries. CurrentSlot is supplied only by the
        // trusted environment selected below.
        5 if action == Action::CloseCandidate => Ok(AccountOperationInputV2::ProjectLamports {
            account: primary,
            destination: common_scalar(scalar::OBSERVED_POSITION_LAMPORTS)?,
        }),
        6 if action == Action::CloseCandidate => Ok(AccountOperationInputV2::ProjectDataU8 {
            account: primary,
            destination: common_scalar(scalar::CANDIDATE_STATUS_OBSERVATION)?,
            data_offset: candidate_body_offset(GeneralCandidateLayoutV1::STATUS_OFFSET)?,
        }),
        7 if action == Action::CloseCandidate => Ok(AccountOperationInputV2::ProjectDataU64 {
            account: primary,
            destination: common_scalar(scalar::CANDIDATE_VERIFICATION_REMAINING_OBSERVATION)?,
            data_offset: candidate_body_offset(
                GeneralCandidateLayoutV1::VERIFICATION_REMAINING_OFFSET,
            )?,
        }),
        8 if action == Action::CloseCandidate => Ok(AccountOperationInputV2::ProjectDataU64 {
            account: primary,
            destination: common_scalar(scalar::CANDIDATE_CLEANUP_REMAINING_OBSERVATION)?,
            data_offset: candidate_body_offset(GeneralCandidateLayoutV1::CLEANUP_REMAINING_OFFSET)?,
        }),
        9 if action == Action::CloseCandidate => Ok(AccountOperationInputV2::ProjectDataU64 {
            account: primary,
            destination: common_scalar(scalar::CANDIDATE_REWARD_RATE)?,
            data_offset: candidate_body_offset(GeneralCandidateLayoutV1::REWARD_RATE_OFFSET)?,
        }),
        10 if action == Action::CloseCandidate => {
            Ok(AccountOperationInputV2::ProjectDataIdentity {
                account: primary,
                destination: common_identity(identity::CANDIDATE)?,
                data_offset: candidate_body_offset(GeneralCandidateLayoutV1::CANDIDATE_ID_OFFSET)?,
            })
        }
        11 if action == Action::CloseCandidate => {
            Ok(AccountOperationInputV2::ProjectDataIdentity {
                account: primary,
                destination: common_identity(identity::SELECTION_BATCH)?,
                data_offset: candidate_body_offset(GeneralCandidateLayoutV1::BATCH_ID_OFFSET)?,
            })
        }
        12 if action == Action::CloseCandidate => {
            Ok(AccountOperationInputV2::ProjectDataIdentity {
                account: primary,
                destination: common_identity(identity::OWNER)?,
                data_offset: candidate_body_offset(GeneralCandidateLayoutV1::SOLVER_ID_OFFSET)?,
            })
        }
        13 if action == Action::CloseCandidate => Ok(AccountOperationInputV2::ProjectDataU64 {
            account: close_candidate_batch_account()?,
            destination: common_scalar(scalar::BATCH_SETTLEMENT_CLOSE_SLOT)?,
            data_offset: batch_body_offset(GeneralBatchLayoutV1::SETTLEMENT_CLOSE_SLOT)?,
        }),
        14 if action == Action::CloseCandidate => Ok(AccountOperationInputV2::ProjectKey {
            account: AccountCoordinateV2::fixed(GENERAL_PRIMARY_PAYER_ACCOUNT_V3),
            destination: common_identity(identity::PAYER)?,
        }),
        15 if action == Action::CloseCandidate => Ok(AccountOperationInputV2::ProjectKey {
            account: AccountCoordinateV2::fixed(GENERAL_PRIMARY_RENT_CREDIT_ACCOUNT_V3),
            destination: common_identity(identity::RENT_CREDIT)?,
        }),
        16 if action == Action::CloseCandidate => Ok(AccountOperationInputV2::ProjectDataU8 {
            account: close_candidate_batch_account()?,
            destination: common_scalar(scalar::BATCH_STATUS_OBSERVATION)?,
            data_offset: batch_body_offset(GeneralBatchLayoutV1::STATUS)?,
        }),
        17 if action == Action::CloseCandidate => Ok(AccountOperationInputV2::RequireOwner {
            account: primary,
            expected: common_identity(identity::TRADING_PROGRAM)?,
        }),
        18 if action == Action::CloseCandidate => Ok(AccountOperationInputV2::ProjectLamports {
            account: AccountCoordinateV2::fixed(GENERAL_PRIMARY_PAYER_ACCOUNT_V3),
            destination: common_scalar(scalar::OBSERVED_ADMISSION_LAMPORTS)?,
        }),
        19 if action == Action::CloseCandidate => Ok(AccountOperationInputV2::ProjectLamports {
            account: AccountCoordinateV2::fixed(GENERAL_PRIMARY_RENT_CREDIT_ACCOUNT_V3),
            destination: common_scalar(scalar::ESCROW_BALANCE_OBSERVATION)?,
        }),
        5 if action == Action::SubmitCandidate => Ok(AccountOperationInputV2::ProjectDataU8 {
            account: submit_evidence_account(0, GeneralReadonlyEvidenceKindV3::ClosedBatch)?,
            destination: common_scalar(scalar::BATCH_STATUS_OBSERVATION)?,
            data_offset: batch_body_offset(GeneralBatchLayoutV1::STATUS)?,
        }),
        6 if action == Action::SubmitCandidate => Ok(AccountOperationInputV2::ProjectDataU32 {
            account: submit_evidence_account(0, GeneralReadonlyEvidenceKindV3::ClosedBatch)?,
            destination: common_scalar(scalar::BATCH_POST_ORDER_COUNT)?,
            data_offset: batch_body_offset(GeneralBatchLayoutV1::OUTCOME_COUNT)?,
        }),
        7 if action == Action::SubmitCandidate => Ok(AccountOperationInputV2::ProjectDataU64 {
            account: submit_evidence_account(0, GeneralReadonlyEvidenceKindV3::ClosedBatch)?,
            destination: common_scalar(scalar::BATCH_COLLECTION_CLOSE_SLOT)?,
            data_offset: batch_body_offset(GeneralBatchLayoutV1::COLLECTION_CLOSE_SLOT)?,
        }),
        8 if action == Action::SubmitCandidate => Ok(AccountOperationInputV2::ProjectDataU64 {
            account: submit_evidence_account(0, GeneralReadonlyEvidenceKindV3::ClosedBatch)?,
            destination: common_scalar(scalar::BATCH_SETTLEMENT_CLOSE_SLOT)?,
            data_offset: batch_body_offset(GeneralBatchLayoutV1::SETTLEMENT_CLOSE_SLOT)?,
        }),
        9 if action == Action::SubmitCandidate => Ok(AccountOperationInputV2::ProjectKey {
            account: submit_evidence_account(0, GeneralReadonlyEvidenceKindV3::ClosedBatch)?,
            destination: common_identity(identity::SELECTION_BATCH)?,
        }),
        10 if action == Action::SubmitCandidate => {
            Ok(AccountOperationInputV2::ProjectDataIdentity {
                account: submit_evidence_account(0, GeneralReadonlyEvidenceKindV3::ClosedBatch)?,
                destination: common_identity(identity::SELECTION_PRODUCT)?,
                data_offset: batch_body_offset(GeneralBatchLayoutV1::PRODUCT_ID)?,
            })
        }
        11 if action == Action::SubmitCandidate => Ok(AccountOperationInputV2::ProjectDataU64 {
            account: submit_evidence_account(0, GeneralReadonlyEvidenceKindV3::ClosedBatch)?,
            destination: common_scalar(scalar::ORDER_MAX_LOTS)?,
            data_offset: batch_body_offset(GeneralBatchLayoutV1::PRICE_SCALE)?,
        }),
        12 if action == Action::SubmitCandidate => Ok(AccountOperationInputV2::ProjectDataU32 {
            account: submit_evidence_account(1, GeneralReadonlyEvidenceKindV3::CandidateImage)?,
            destination: common_scalar(scalar::ZERO)?,
            data_offset: width(CandidateLayoutV2::OUTCOME_COUNT)?,
        }),
        13 if action == Action::SubmitCandidate => Ok(AccountOperationInputV2::ProjectDataU32 {
            account: submit_evidence_account(1, GeneralReadonlyEvidenceKindV3::CandidateImage)?,
            destination: common_scalar(scalar::CANDIDATE_PAGE_COUNT)?,
            data_offset: width(CandidateLayoutV2::PAGE_COUNT)?,
        }),
        14 if action == Action::SubmitCandidate => Ok(AccountOperationInputV2::ProjectDataU32 {
            account: submit_evidence_account(1, GeneralReadonlyEvidenceKindV3::CandidateImage)?,
            destination: common_scalar(scalar::SELECTION_BEST_CANDIDATE_COORDINATE)?,
            data_offset: width(CandidateLayoutV2::CANDIDATE_COORDINATE)?,
        }),
        15 if action == Action::SubmitCandidate => Ok(AccountOperationInputV2::ProjectDataU64 {
            account: submit_evidence_account(1, GeneralReadonlyEvidenceKindV3::CandidateImage)?,
            destination: common_scalar(scalar::SELECTION_PRICE_SCALE)?,
            data_offset: width(CandidateLayoutV2::PRICE_SCALE)?,
        }),
        16 if action == Action::SubmitCandidate => {
            Ok(AccountOperationInputV2::ProjectDataIdentity {
                account: submit_evidence_account(1, GeneralReadonlyEvidenceKindV3::CandidateImage)?,
                destination: common_identity(identity::BEST_VERIFIED_DIGEST)?,
                data_offset: width(CandidateLayoutV2::CANDIDATE_ID)?,
            })
        }
        17 if action == Action::SubmitCandidate => {
            Ok(AccountOperationInputV2::ProjectDataIdentity {
                account: submit_evidence_account(1, GeneralReadonlyEvidenceKindV3::CandidateImage)?,
                destination: common_identity(identity::ORDER)?,
                data_offset: width(CandidateLayoutV2::PRODUCT_ID)?,
            })
        }
        18 if action == Action::SubmitCandidate => {
            Ok(AccountOperationInputV2::ProjectDataIdentity {
                account: submit_evidence_account(1, GeneralReadonlyEvidenceKindV3::CandidateImage)?,
                destination: common_identity(identity::SELECTION_POLICY)?,
                data_offset: width(CandidateLayoutV2::BATCH_ID)?,
            })
        }
        19 if action == Action::SubmitCandidate => Ok(AccountOperationInputV2::ProjectDataU32 {
            account: submit_evidence_account(2, GeneralReadonlyEvidenceKindV3::SubmittedCandidate)?,
            destination: common_scalar(scalar::VERIFY_POST_ORDER_COUNT)?,
            data_offset: width(GeneralCandidateLayoutV1::OUTCOME_COUNT_OFFSET)?,
        }),
        20 if action == Action::SubmitCandidate => Ok(AccountOperationInputV2::ProjectDataU32 {
            account: submit_evidence_account(2, GeneralReadonlyEvidenceKindV3::SubmittedCandidate)?,
            destination: common_scalar(scalar::VERIFY_POST_PAGE)?,
            data_offset: width(GeneralCandidateLayoutV1::PAGE_COUNT_OFFSET)?,
        }),
        21 if action == Action::SubmitCandidate => Ok(AccountOperationInputV2::ProjectDataU8 {
            account: submit_evidence_account(2, GeneralReadonlyEvidenceKindV3::SubmittedCandidate)?,
            destination: common_scalar(scalar::CANDIDATE_STATUS_OBSERVATION)?,
            data_offset: width(GeneralCandidateLayoutV1::STATUS_OFFSET)?,
        }),
        22 if action == Action::SubmitCandidate => Ok(AccountOperationInputV2::ProjectDataU64 {
            account: submit_evidence_account(2, GeneralReadonlyEvidenceKindV3::SubmittedCandidate)?,
            destination: common_scalar(scalar::CANDIDATE_PAGE_REVISION)?,
            data_offset: width(GeneralCandidateLayoutV1::PAGE_REVISION_OFFSET)?,
        }),
        23 if action == Action::SubmitCandidate => {
            Ok(AccountOperationInputV2::ProjectDataIdentity {
                account: submit_evidence_account(
                    2,
                    GeneralReadonlyEvidenceKindV3::SubmittedCandidate,
                )?,
                destination: common_identity(identity::RESULT_BENEFICIARY_OBSERVATION)?,
                data_offset: width(GeneralCandidateLayoutV1::CANDIDATE_ID_OFFSET)?,
            })
        }
        24 if action == Action::SubmitCandidate => {
            Ok(AccountOperationInputV2::ProjectDataIdentity {
                account: submit_evidence_account(
                    2,
                    GeneralReadonlyEvidenceKindV3::SubmittedCandidate,
                )?,
                destination: common_identity(identity::BENEFICIARY)?,
                data_offset: width(GeneralCandidateLayoutV1::BATCH_ID_OFFSET)?,
            })
        }
        25 if action == Action::SubmitCandidate => {
            Ok(AccountOperationInputV2::ProjectDataIdentity {
                account: submit_evidence_account(
                    2,
                    GeneralReadonlyEvidenceKindV3::SubmittedCandidate,
                )?,
                destination: common_identity(identity::OWNER)?,
                data_offset: width(GeneralCandidateLayoutV1::SOLVER_ID_OFFSET)?,
            })
        }
        26 if action == Action::SubmitCandidate => Ok(AccountOperationInputV2::ProjectDataU64 {
            account: submit_evidence_account(2, GeneralReadonlyEvidenceKindV3::SubmittedCandidate)?,
            destination: common_scalar(scalar::CANDIDATE_SUBMITTED_SLOT)?,
            data_offset: width(GeneralCandidateLayoutV1::SUBMITTED_SLOT_OFFSET)?,
        }),
        27 if action == Action::SubmitCandidate => Ok(AccountOperationInputV2::ProjectDataU32 {
            account: submit_evidence_account(2, GeneralReadonlyEvidenceKindV3::SubmittedCandidate)?,
            destination: common_scalar(scalar::CANDIDATE_ROW_COUNT)?,
            data_offset: width(GeneralCandidateLayoutV1::ROW_COUNT_OFFSET)?,
        }),
        28 if action == Action::SubmitCandidate => Ok(AccountOperationInputV2::ProjectDataU64 {
            account: submit_evidence_account(2, GeneralReadonlyEvidenceKindV3::SubmittedCandidate)?,
            destination: common_scalar(scalar::CANDIDATE_REWARD_RATE)?,
            data_offset: width(GeneralCandidateLayoutV1::REWARD_RATE_OFFSET)?,
        }),
        29 if action == Action::SubmitCandidate => Ok(AccountOperationInputV2::ProjectDataU64 {
            account: submit_evidence_account(2, GeneralReadonlyEvidenceKindV3::SubmittedCandidate)?,
            destination: common_scalar(scalar::CANDIDATE_VERIFICATION_REMAINING_OBSERVATION)?,
            data_offset: width(GeneralCandidateLayoutV1::VERIFICATION_REMAINING_OFFSET)?,
        }),
        30 if action == Action::SubmitCandidate => Ok(AccountOperationInputV2::ProjectDataU64 {
            account: submit_evidence_account(2, GeneralReadonlyEvidenceKindV3::SubmittedCandidate)?,
            destination: common_scalar(scalar::CANDIDATE_CLEANUP_REMAINING_OBSERVATION)?,
            data_offset: width(GeneralCandidateLayoutV1::CLEANUP_REMAINING_OFFSET)?,
        }),
        // The solver who pays is the solver the candidate names. This was a
        // `RequireKey` against `identity::OWNER`, which the SAME profile pass
        // projects out of the candidate record -- and a guard reads the INPUT
        // identity bank while a projection writes a separate output bank, so
        // it compared against 32 zero bytes and could never hold. The law is
        // re-proven where both values exist: the emitted transition carries
        // `identity_eq(PAYER, OWNER)`.
        31 if action == Action::SubmitCandidate => Ok(AccountOperationInputV2::ProjectKey {
            account: AccountCoordinateV2::fixed(GENERAL_PRIMARY_PAYER_ACCOUNT_V3),
            destination: common_identity(identity::PAYER)?,
        }),
        32 if action == Action::SubmitCandidate => Ok(AccountOperationInputV2::RequireOwner {
            account: AccountCoordinateV2::fixed(GENERAL_PRIMARY_PAYER_ACCOUNT_V3),
            expected: common_identity(identity::RESULT_OWNER)?,
        }),
        // The batch pair's projections: the root tail, the config windows, and
        // the three identities the batch record binds. `zero` receives the
        // persisted batch width on CloseBatch -- the real width conjunct --
        // and the Product width again on OpenBatch, whose record does not yet
        // exist and whose width the effect pins to the same source.
        // A coordinate carrying a data-effect grant with no lifecycle creation
        // must anchor its owner explicitly, exactly as Close anchors the
        // settlement state it debits. The composite root is Trading's PDA.
        5 if matches!(action, Action::OpenBatch | Action::CloseBatch) => {
            Ok(AccountOperationInputV2::RequireOwner {
                account: AccountCoordinateV2::fixed(narrow(HOT_RUNTIME_ROOT_COORDINATE_V3)?),
                expected: common_identity(identity::TRADING_PROGRAM)?,
            })
        }
        6 if matches!(action, Action::OpenBatch | Action::CloseBatch) => {
            if action == Action::OpenBatch {
                Ok(AccountOperationInputV2::ProjectDataU32 {
                    account: AccountCoordinateV2::fixed(narrow(
                        HOT_RUNTIME_PORTFOLIO_COORDINATE_V3,
                    )?),
                    destination: common_scalar(scalar::ZERO)?,
                    data_offset: width(PORTFOLIO_COEFFICIENT_COUNT_OFFSET)?,
                })
            } else {
                Ok(AccountOperationInputV2::ProjectDataU32 {
                    account: primary,
                    destination: common_scalar(scalar::ZERO)?,
                    data_offset: batch_body_offset(GeneralBatchLayoutV1::OUTCOME_COUNT)?,
                })
            }
        }
        7 if matches!(action, Action::OpenBatch | Action::CloseBatch) => {
            Ok(AccountOperationInputV2::ProjectDataU64 {
                account: AccountCoordinateV2::fixed(narrow(HOT_RUNTIME_ROOT_COORDINATE_V3)?),
                destination: common_scalar(scalar::ROOT_REVISION_OBSERVATION)?,
                data_offset: root_tail_offset(GENERAL_ROOT_REVISION_OFFSET_V2)?,
            })
        }
        8 if matches!(action, Action::OpenBatch | Action::CloseBatch) => {
            Ok(AccountOperationInputV2::ProjectDataU64 {
                account: AccountCoordinateV2::fixed(narrow(HOT_RUNTIME_ROOT_COORDINATE_V3)?),
                destination: common_scalar(scalar::ROOT_OPEN_BATCHES_OBSERVATION)?,
                data_offset: root_tail_offset(GENERAL_ROOT_OPEN_BATCHES_OFFSET_V2)?,
            })
        }
        9 if matches!(action, Action::OpenBatch | Action::CloseBatch) => {
            Ok(AccountOperationInputV2::ProjectDataIdentity {
                account: AccountCoordinateV2::fixed(narrow(HOT_RUNTIME_ROOT_COORDINATE_V3)?),
                destination: common_identity(identity::GENERAL_CONFIG_ID)?,
                data_offset: root_tail_offset(GENERAL_ROOT_CONFIG_ID_OFFSET_V2)?,
            })
        }
        10 if matches!(action, Action::OpenBatch | Action::CloseBatch) => {
            Ok(AccountOperationInputV2::ProjectDataIdentity {
                account: AccountCoordinateV2::fixed(narrow(HOT_RUNTIME_ROOT_COORDINATE_V3)?),
                destination: common_identity(identity::MARKET)?,
                data_offset: root_tail_offset(GENERAL_ROOT_MARKET_OFFSET_V2)?,
            })
        }
        11 if matches!(action, Action::OpenBatch | Action::CloseBatch) => {
            Ok(AccountOperationInputV2::ProjectDataIdentity {
                account: AccountCoordinateV2::fixed(narrow(HOT_RUNTIME_PRODUCT_COORDINATE_V3)?),
                destination: common_identity(identity::SELECTION_PRODUCT)?,
                data_offset: width(PRODUCT_RECORD_PRODUCT_ID_OFFSET_V2)?,
            })
        }
        12 if action == Action::OpenBatch => Ok(AccountOperationInputV2::ProjectDataU64 {
            account: AccountCoordinateV2::fixed(narrow(HOT_RUNTIME_ROOT_COORDINATE_V3)?),
            destination: common_scalar(scalar::ROOT_NEXT_BATCH_SEQUENCE_OBSERVATION)?,
            data_offset: root_tail_offset(GENERAL_ROOT_NEXT_BATCH_SEQUENCE_OFFSET_V2)?,
        }),
        13 if action == Action::OpenBatch => Ok(AccountOperationInputV2::ProjectDataU64 {
            account: AccountCoordinateV2::fixed(narrow(HOT_RUNTIME_CONFIG_COORDINATE_V3)?),
            destination: common_scalar(scalar::CONFIG_COLLECTION_SLOTS)?,
            data_offset: width(GeneralConfigV3Layout::COLLECTION_SLOTS)?,
        }),
        14 if action == Action::OpenBatch => Ok(AccountOperationInputV2::ProjectDataU64 {
            account: AccountCoordinateV2::fixed(narrow(HOT_RUNTIME_CONFIG_COORDINATE_V3)?),
            destination: common_scalar(scalar::CONFIG_SELECTION_SLOTS)?,
            data_offset: width(GeneralConfigV3Layout::SELECTION_SLOTS)?,
        }),
        15 if action == Action::OpenBatch => Ok(AccountOperationInputV2::ProjectDataU64 {
            account: AccountCoordinateV2::fixed(narrow(HOT_RUNTIME_CONFIG_COORDINATE_V3)?),
            destination: common_scalar(scalar::CONFIG_SETTLEMENT_SLOTS)?,
            data_offset: width(GeneralConfigV3Layout::SETTLEMENT_SLOTS)?,
        }),
        16 if action == Action::OpenBatch => Ok(AccountOperationInputV2::ProjectDataU32 {
            account: AccountCoordinateV2::fixed(narrow(HOT_RUNTIME_CONFIG_COORDINATE_V3)?),
            destination: common_scalar(scalar::CONFIG_MAX_ORDERS)?,
            data_offset: width(GeneralConfigV3Layout::MAX_ORDERS_PER_CANDIDATE)?,
        }),
        17 if action == Action::OpenBatch => Ok(AccountOperationInputV2::ProjectDataU64 {
            account: AccountCoordinateV2::fixed(narrow(HOT_RUNTIME_CONFIG_COORDINATE_V3)?),
            destination: common_scalar(scalar::SELECTION_PRICE_SCALE)?,
            data_offset: width(GeneralConfigV3Layout::PRICE_SCALE)?,
        }),
        // PlaceOrder's projections. The batch window supplies its status,
        // clock, both close slots, bound and counters; the SIGNED TERMS
        // evidence supplies every order coordinate the record will carry --
        // including the per-outcome rows, projected affinely into the item
        // bank at the image's own stride -- and the maker it names twice (the
        // owner identity the deposit draws on, and the created record's rent
        // beneficiary); the root, Product record and config supply the three
        // independently-sourced identities the batch bindings require. The
        // maker's authority is operation 27: the PAYER must BE the recorded
        // owner, and the payer rule is the signer rule. The created order
        // state is observed at 5..=7 the way Close observes its terminal.
        5 if action == Action::PlaceOrder => Ok(AccountOperationInputV2::ProjectDataU8 {
            account: terminal,
            destination: common_scalar(scalar::TERMINAL_BUMP_OBSERVATION)?,
            data_offset: GeneralLocalStateLayoutV3::bump(),
        }),
        6 if action == Action::PlaceOrder => Ok(AccountOperationInputV2::ProjectDataU64 {
            account: terminal,
            destination: common_scalar(scalar::TERMINAL_PRINCIPAL_OBSERVATION)?,
            data_offset: GeneralLocalStateLayoutV3::rent_principal(),
        }),
        // The created record's rent beneficiary is the MAKER, and the value
        // arrives through the plan's beneficiary register: the signed terms'
        // owner is projected into the observation coordinate the create plan
        // reads, so the lifecycle mints the record with the maker as its
        // beneficiary. (The account is necessarily vacant here: the creation
        // suite -- replay, vault, Position, all create-only -- is the replay
        // guard, so there is no live beneficiary to observe.)
        7 if action == Action::PlaceOrder => Ok(AccountOperationInputV2::ProjectDataIdentity {
            account: order_terms_account(action)?,
            destination: common_identity(identity::TERMINAL_BENEFICIARY_OBSERVATION)?,
            data_offset: width(GeneralOrderLayoutV1::OWNER_ID)?,
        }),
        8 if action == Action::PlaceOrder => Ok(AccountOperationInputV2::ProjectDataU32 {
            account: primary,
            destination: common_scalar(scalar::ZERO)?,
            data_offset: batch_body_offset(GeneralBatchLayoutV1::OUTCOME_COUNT)?,
        }),
        9 if action == Action::PlaceOrder => Ok(AccountOperationInputV2::ProjectDataU8 {
            account: primary,
            destination: common_scalar(scalar::BATCH_STATUS_OBSERVATION)?,
            data_offset: batch_body_offset(GeneralBatchLayoutV1::STATUS)?,
        }),
        10 if action == Action::PlaceOrder => Ok(AccountOperationInputV2::ProjectDataU64 {
            account: primary,
            destination: common_scalar(scalar::BATCH_COLLECTION_CLOSE_SLOT)?,
            data_offset: batch_body_offset(GeneralBatchLayoutV1::COLLECTION_CLOSE_SLOT)?,
        }),
        11 if action == Action::PlaceOrder => Ok(AccountOperationInputV2::ProjectDataU64 {
            account: primary,
            destination: common_scalar(scalar::BATCH_SETTLEMENT_CLOSE_SLOT)?,
            data_offset: batch_body_offset(GeneralBatchLayoutV1::SETTLEMENT_CLOSE_SLOT)?,
        }),
        12 if action == Action::PlaceOrder => Ok(AccountOperationInputV2::ProjectDataU32 {
            account: primary,
            destination: common_scalar(scalar::CONFIG_MAX_ORDERS)?,
            data_offset: batch_body_offset(GeneralBatchLayoutV1::MAX_ORDERS)?,
        }),
        13 if action == Action::PlaceOrder => Ok(AccountOperationInputV2::ProjectDataU32 {
            account: primary,
            destination: common_scalar(scalar::BATCH_ORDER_COUNT_OBSERVATION)?,
            data_offset: batch_body_offset(GeneralBatchLayoutV1::ORDER_COUNT)?,
        }),
        14 if action == Action::PlaceOrder => Ok(AccountOperationInputV2::ProjectDataU64 {
            account: primary,
            destination: common_scalar(scalar::BATCH_QUOTE_RESERVE_OBSERVATION)?,
            data_offset: batch_body_offset(GeneralBatchLayoutV1::COMMITTED_QUOTE_RESERVE)?,
        }),
        15 if action == Action::PlaceOrder => Ok(AccountOperationInputV2::ProjectDataU32 {
            account: order_terms_account(action)?,
            destination: common_scalar(scalar::SCRATCH_A)?,
            data_offset: width(GeneralOrderLayoutV1::OUTCOME_COUNT)?,
        }),
        16 if action == Action::PlaceOrder => Ok(AccountOperationInputV2::ProjectDataU64 {
            account: order_terms_account(action)?,
            destination: common_scalar(scalar::ORDER_NONCE)?,
            data_offset: width(GeneralOrderLayoutV1::NONCE)?,
        }),
        17 if action == Action::PlaceOrder => Ok(AccountOperationInputV2::ProjectDataU64 {
            account: order_terms_account(action)?,
            destination: common_scalar(scalar::ORDER_MAX_LOTS)?,
            data_offset: width(GeneralOrderLayoutV1::MAX_LOTS)?,
        }),
        18 if action == Action::PlaceOrder => Ok(AccountOperationInputV2::ProjectDataU64 {
            account: order_terms_account(action)?,
            destination: common_scalar(scalar::ORDER_MAX_QUOTE_DEBIT_PER_LOT)?,
            data_offset: width(GeneralOrderLayoutV1::MAX_QUOTE_DEBIT_PER_LOT)?,
        }),
        19 if action == Action::PlaceOrder => Ok(AccountOperationInputV2::ProjectDataU64 {
            account: order_terms_account(action)?,
            destination: common_scalar(scalar::ORDER_VALID_UNTIL_SLOT)?,
            data_offset: width(GeneralOrderLayoutV1::VALID_UNTIL_SLOT)?,
        }),
        20 if action == Action::PlaceOrder => Ok(AccountOperationInputV2::ProjectDataIdentity {
            account: order_terms_account(action)?,
            destination: common_identity(identity::OWNER)?,
            data_offset: width(GeneralOrderLayoutV1::OWNER_ID)?,
        }),
        21 if action == Action::PlaceOrder => Ok(AccountOperationInputV2::ProjectDataIdentity {
            account: order_terms_account(action)?,
            destination: common_identity(identity::SELECTION_BATCH)?,
            data_offset: width(GeneralOrderLayoutV1::BATCH_ID)?,
        }),
        22 if action == Action::PlaceOrder => Ok(AccountOperationInputV2::ProjectDataIdentity {
            account: order_terms_account(action)?,
            destination: common_identity(identity::CANDIDATE)?,
            data_offset: width(GeneralOrderLayoutV1::BATCH_ID)?,
        }),
        23 if action == Action::PlaceOrder => Ok(AccountOperationInputV2::ProjectDataIdentity {
            account: AccountCoordinateV2::fixed(narrow(HOT_RUNTIME_ROOT_COORDINATE_V3)?),
            destination: common_identity(identity::MARKET)?,
            data_offset: root_tail_offset(GENERAL_ROOT_MARKET_OFFSET_V2)?,
        }),
        24 if action == Action::PlaceOrder => Ok(AccountOperationInputV2::ProjectDataIdentity {
            account: AccountCoordinateV2::fixed(narrow(HOT_RUNTIME_ROOT_COORDINATE_V3)?),
            destination: common_identity(identity::GENERAL_CONFIG_ID)?,
            data_offset: root_tail_offset(GENERAL_ROOT_CONFIG_ID_OFFSET_V2)?,
        }),
        25 if action == Action::PlaceOrder => Ok(AccountOperationInputV2::ProjectDataIdentity {
            account: AccountCoordinateV2::fixed(narrow(HOT_RUNTIME_PRODUCT_COORDINATE_V3)?),
            destination: common_identity(identity::SELECTION_PRODUCT)?,
            data_offset: width(PRODUCT_RECORD_PRODUCT_ID_OFFSET_V2)?,
        }),
        // The maker who pays is the maker the signed terms name; see
        // SubmitCandidate above for why this cannot be a guard here.
        26 if action == Action::PlaceOrder => Ok(AccountOperationInputV2::ProjectKey {
            account: AccountCoordinateV2::fixed(GENERAL_CLOSE_PAYER_ACCOUNT_V3),
            destination: common_identity(identity::PAYER)?,
        }),
        // PlaceOrder's two item operations are THE TAIL, after every fixed
        // one, and they were written here as `30` and `31`. That was true
        // until the fixed body grew by the semantic-basis projection, and a
        // literal in a span whose start moves is a number that stops being
        // what it names without anything going red -- it simply stops being
        // reached, and the encoder reports `Geometry` for an action that has
        // run out of arms. Derived from the fixed count now, which is the
        // definition of "the tail" rather than a snapshot of where it was.
        first_item
            if action == Action::PlaceOrder
                && first_item == general_account_profile_fixed_operation_count_v3(action) =>
        {
            Ok(AccountOperationInputV2::ProjectDataU64Affine {
                account: order_terms_account(action)?,
                destination: ScalarCoordinateV2::item(narrow_u32(item_scalar::CURSOR_INVENTORY)?),
                data_offset: width(
                    GENERAL_ORDER_HEADER_BYTES_V1 + GENERAL_ORDER_ROW_RECEIVE_OFFSET_V1,
                )?,
                data_stride: width(GENERAL_ORDER_ROW_STRIDE_V1)?,
            })
        }
        second_item
            if action == Action::PlaceOrder
                && second_item
                    == general_account_profile_fixed_operation_count_v3(action)
                        .saturating_add(1) =>
        {
            Ok(AccountOperationInputV2::ProjectDataU64Affine {
                account: order_terms_account(action)?,
                destination: ScalarCoordinateV2::item(narrow_u32(item_scalar::QUANTITY)?),
                data_offset: width(
                    GENERAL_ORDER_HEADER_BYTES_V1 + GENERAL_ORDER_ROW_DELIVER_OFFSET_V1,
                )?,
                data_stride: width(GENERAL_ORDER_ROW_STRIDE_V1)?,
            })
        }
        // CancelOrder's projections. The second derived state (the order, at
        // the terminal coordinate) is observed the way Close observes its
        // terminal record; the batch window's counters and clock feed the
        // cancellation conjuncts; the order record supplies its own terms and
        // its maker; and the root, the Product record, and the config supply
        // the three independently-sourced identities the batch bindings
        // require. The maker's authority is operation 27: the PAYER account
        // must BE the recorded owner, and the payer rule is the signer rule,
        // so NotTheMaker is a refusal of the frame itself.
        5 if action == Action::CancelOrder => Ok(AccountOperationInputV2::ProjectDataU8 {
            account: terminal,
            destination: common_scalar(scalar::TERMINAL_BUMP_OBSERVATION)?,
            data_offset: GeneralLocalStateLayoutV3::bump(),
        }),
        6 if action == Action::CancelOrder => Ok(AccountOperationInputV2::ProjectDataU64 {
            account: terminal,
            destination: common_scalar(scalar::TERMINAL_PRINCIPAL_OBSERVATION)?,
            data_offset: GeneralLocalStateLayoutV3::rent_principal(),
        }),
        7 if action == Action::CancelOrder => Ok(AccountOperationInputV2::ProjectDataIdentity {
            account: terminal,
            destination: common_identity(identity::TERMINAL_BENEFICIARY_OBSERVATION)?,
            data_offset: GeneralLocalStateLayoutV3::beneficiary(),
        }),
        8 if action == Action::CancelOrder => Ok(AccountOperationInputV2::ProjectDataU32 {
            account: primary,
            destination: common_scalar(scalar::ZERO)?,
            data_offset: batch_body_offset(GeneralBatchLayoutV1::OUTCOME_COUNT)?,
        }),
        9 if action == Action::CancelOrder => Ok(AccountOperationInputV2::ProjectDataU8 {
            account: primary,
            destination: common_scalar(scalar::BATCH_STATUS_OBSERVATION)?,
            data_offset: batch_body_offset(GeneralBatchLayoutV1::STATUS)?,
        }),
        10 if action == Action::CancelOrder => Ok(AccountOperationInputV2::ProjectDataU64 {
            account: primary,
            destination: common_scalar(scalar::BATCH_COLLECTION_CLOSE_SLOT)?,
            data_offset: batch_body_offset(GeneralBatchLayoutV1::COLLECTION_CLOSE_SLOT)?,
        }),
        11 if action == Action::CancelOrder => Ok(AccountOperationInputV2::ProjectDataU32 {
            account: primary,
            destination: common_scalar(scalar::BATCH_ORDER_COUNT_OBSERVATION)?,
            data_offset: batch_body_offset(GeneralBatchLayoutV1::ORDER_COUNT)?,
        }),
        12 if action == Action::CancelOrder => Ok(AccountOperationInputV2::ProjectDataU32 {
            account: primary,
            destination: common_scalar(scalar::BATCH_CANCELLED_COUNT_OBSERVATION)?,
            data_offset: batch_body_offset(GeneralBatchLayoutV1::CANCELLED_COUNT)?,
        }),
        13 if action == Action::CancelOrder => Ok(AccountOperationInputV2::ProjectDataU64 {
            account: primary,
            destination: common_scalar(scalar::BATCH_QUOTE_RESERVE_OBSERVATION)?,
            data_offset: batch_body_offset(GeneralBatchLayoutV1::COMMITTED_QUOTE_RESERVE)?,
        }),
        14 if action == Action::CancelOrder => Ok(AccountOperationInputV2::ProjectDataU32 {
            account: terminal,
            destination: common_scalar(scalar::SCRATCH_A)?,
            data_offset: order_body_offset(GeneralOrderLayoutV1::OUTCOME_COUNT)?,
        }),
        15 if action == Action::CancelOrder => Ok(AccountOperationInputV2::ProjectDataU8 {
            account: terminal,
            destination: common_scalar(scalar::ORDER_PHASE_OBSERVATION)?,
            data_offset: order_body_offset(GeneralOrderLayoutV1::STATE_PHASE)?,
        }),
        16 if action == Action::CancelOrder => Ok(AccountOperationInputV2::ProjectDataU64 {
            account: terminal,
            destination: common_scalar(scalar::ORDER_ADMITTED_SLOT_OBSERVATION)?,
            data_offset: order_body_offset(GeneralOrderLayoutV1::STATE_ADMITTED_SLOT)?,
        }),
        17 if action == Action::CancelOrder => Ok(AccountOperationInputV2::ProjectDataU64 {
            account: terminal,
            destination: common_scalar(scalar::ORDER_MAX_LOTS)?,
            data_offset: order_body_offset(GeneralOrderLayoutV1::MAX_LOTS)?,
        }),
        18 if action == Action::CancelOrder => Ok(AccountOperationInputV2::ProjectDataU64 {
            account: terminal,
            destination: common_scalar(scalar::ORDER_MAX_QUOTE_DEBIT_PER_LOT)?,
            data_offset: order_body_offset(GeneralOrderLayoutV1::MAX_QUOTE_DEBIT_PER_LOT)?,
        }),
        19 if action == Action::CancelOrder => Ok(AccountOperationInputV2::ProjectDataU64 {
            account: terminal,
            destination: common_scalar(scalar::ORDER_NONCE)?,
            data_offset: order_body_offset(GeneralOrderLayoutV1::NONCE)?,
        }),
        20 if action == Action::CancelOrder => Ok(AccountOperationInputV2::ProjectDataIdentity {
            account: terminal,
            destination: common_identity(identity::OWNER)?,
            data_offset: order_body_offset(GeneralOrderLayoutV1::OWNER_ID)?,
        }),
        // The order's batch bytes are the register the batch address is
        // DERIVED from, and they ride the CANDIDATE register for the escrow
        // legs (`build_order_escrow_packets_v1`).
        21 if action == Action::CancelOrder => Ok(AccountOperationInputV2::ProjectDataIdentity {
            account: terminal,
            destination: common_identity(identity::SELECTION_BATCH)?,
            data_offset: order_body_offset(GeneralOrderLayoutV1::BATCH_ID)?,
        }),
        22 if action == Action::CancelOrder => Ok(AccountOperationInputV2::ProjectDataIdentity {
            account: terminal,
            destination: common_identity(identity::CANDIDATE)?,
            data_offset: order_body_offset(GeneralOrderLayoutV1::BATCH_ID)?,
        }),
        23 if action == Action::CancelOrder => Ok(AccountOperationInputV2::ProjectDataIdentity {
            account: AccountCoordinateV2::fixed(narrow(HOT_RUNTIME_ROOT_COORDINATE_V3)?),
            destination: common_identity(identity::MARKET)?,
            data_offset: root_tail_offset(GENERAL_ROOT_MARKET_OFFSET_V2)?,
        }),
        24 if action == Action::CancelOrder => Ok(AccountOperationInputV2::ProjectDataIdentity {
            account: AccountCoordinateV2::fixed(narrow(HOT_RUNTIME_ROOT_COORDINATE_V3)?),
            destination: common_identity(identity::GENERAL_CONFIG_ID)?,
            data_offset: root_tail_offset(GENERAL_ROOT_CONFIG_ID_OFFSET_V2)?,
        }),
        25 if action == Action::CancelOrder => Ok(AccountOperationInputV2::ProjectDataIdentity {
            account: AccountCoordinateV2::fixed(narrow(HOT_RUNTIME_PRODUCT_COORDINATE_V3)?),
            destination: common_identity(identity::SELECTION_PRODUCT)?,
            data_offset: width(PRODUCT_RECORD_PRODUCT_ID_OFFSET_V2)?,
        }),
        // The maker who pays is the maker the order record names; see
        // SubmitCandidate above for why this cannot be a guard here.
        26 if action == Action::CancelOrder => Ok(AccountOperationInputV2::ProjectKey {
            account: AccountCoordinateV2::fixed(GENERAL_CLOSE_PAYER_ACCOUNT_V3),
            destination: common_identity(identity::PAYER)?,
        }),
        // ReleaseOrder's projections. Every conjunct input with an
        // authoritative frame source is pinned here: the order record's own
        // fields (the wire the order repair fixed at these offsets), the root
        // Market as the record's one independently-sourced binding, the config
        // generation, and the escrow vault's observed token balance -- the
        // residual, read from the account the transfer will draw on. The
        // child-frame plumbing registers (custody keys, mint, revisions,
        // Position lamports, and the observed vault balance -- a token
        // account's rule is opaque, so no projection may read it) are the
        // runtime bank's, exactly as they are for the settlement seven, and
        // the child programs are their authority: an overstated residual dies
        // at the token program, an understated one leaves the vault nonzero
        // and the vault close refuses.
        5 if action == Action::ReleaseOrder => Ok(AccountOperationInputV2::ProjectDataU32 {
            account: primary,
            destination: common_scalar(scalar::ZERO)?,
            data_offset: order_body_offset(GeneralOrderLayoutV1::OUTCOME_COUNT)?,
        }),
        6 if action == Action::ReleaseOrder => Ok(AccountOperationInputV2::ProjectDataU8 {
            account: primary,
            destination: common_scalar(scalar::ORDER_PHASE_OBSERVATION)?,
            data_offset: order_body_offset(GeneralOrderLayoutV1::STATE_PHASE)?,
        }),
        7 if action == Action::ReleaseOrder => Ok(AccountOperationInputV2::ProjectDataU64 {
            account: primary,
            destination: common_scalar(scalar::ORDER_ADMITTED_SLOT_OBSERVATION)?,
            data_offset: order_body_offset(GeneralOrderLayoutV1::STATE_ADMITTED_SLOT)?,
        }),
        8 if action == Action::ReleaseOrder => Ok(AccountOperationInputV2::ProjectDataU64 {
            account: primary,
            destination: common_scalar(scalar::ORDER_VALID_UNTIL_SLOT)?,
            data_offset: order_body_offset(GeneralOrderLayoutV1::VALID_UNTIL_SLOT)?,
        }),
        9 if action == Action::ReleaseOrder => Ok(AccountOperationInputV2::ProjectDataU64 {
            account: primary,
            destination: common_scalar(scalar::ORDER_MAX_LOTS)?,
            data_offset: order_body_offset(GeneralOrderLayoutV1::MAX_LOTS)?,
        }),
        10 if action == Action::ReleaseOrder => Ok(AccountOperationInputV2::ProjectDataU64 {
            account: primary,
            destination: common_scalar(scalar::ORDER_MAX_QUOTE_DEBIT_PER_LOT)?,
            data_offset: order_body_offset(GeneralOrderLayoutV1::MAX_QUOTE_DEBIT_PER_LOT)?,
        }),
        11 if action == Action::ReleaseOrder => Ok(AccountOperationInputV2::ProjectDataU64 {
            account: primary,
            destination: common_scalar(scalar::ORDER_NONCE)?,
            data_offset: order_body_offset(GeneralOrderLayoutV1::NONCE)?,
        }),
        12 if action == Action::ReleaseOrder => Ok(AccountOperationInputV2::ProjectDataIdentity {
            account: primary,
            destination: common_identity(identity::OWNER)?,
            data_offset: order_body_offset(GeneralOrderLayoutV1::OWNER_ID)?,
        }),
        // The batch identity rides the CANDIDATE register for every escrow
        // leg: an admission has no candidate, and the batch is the lifecycle
        // the Custody replay belongs to (`build_order_escrow_packets_v1`).
        13 if action == Action::ReleaseOrder => Ok(AccountOperationInputV2::ProjectDataIdentity {
            account: primary,
            destination: common_identity(identity::CANDIDATE)?,
            data_offset: order_body_offset(GeneralOrderLayoutV1::BATCH_ID)?,
        }),
        14 if action == Action::ReleaseOrder => Ok(AccountOperationInputV2::ProjectDataIdentity {
            account: AccountCoordinateV2::fixed(narrow(HOT_RUNTIME_ROOT_COORDINATE_V3)?),
            destination: common_identity(identity::MARKET)?,
            data_offset: root_tail_offset(GENERAL_ROOT_MARKET_OFFSET_V2)?,
        }),
        // CloseBatch's four batch-record projections: the status and counter
        // its disjunction reads, and the window and bound it compares.
        12 if action == Action::CloseBatch => Ok(AccountOperationInputV2::ProjectDataU8 {
            account: primary,
            destination: common_scalar(scalar::BATCH_STATUS_OBSERVATION)?,
            data_offset: batch_body_offset(GeneralBatchLayoutV1::STATUS)?,
        }),
        13 if action == Action::CloseBatch => Ok(AccountOperationInputV2::ProjectDataU32 {
            account: primary,
            destination: common_scalar(scalar::BATCH_ORDER_COUNT_OBSERVATION)?,
            data_offset: batch_body_offset(GeneralBatchLayoutV1::ORDER_COUNT)?,
        }),
        14 if action == Action::CloseBatch => Ok(AccountOperationInputV2::ProjectDataU64 {
            account: primary,
            destination: common_scalar(scalar::BATCH_COLLECTION_CLOSE_SLOT)?,
            data_offset: batch_body_offset(GeneralBatchLayoutV1::COLLECTION_CLOSE_SLOT)?,
        }),
        15 if action == Action::CloseBatch => Ok(AccountOperationInputV2::ProjectDataU32 {
            account: primary,
            destination: common_scalar(scalar::CONFIG_MAX_ORDERS)?,
            data_offset: batch_body_offset(GeneralBatchLayoutV1::MAX_ORDERS)?,
        }),
        // Every other action creates its primary state, so its lifecycle plan is
        // what proves the account's owner. Close destroys that account instead:
        // its rule is Exact, not LifecycleBound, and a debiting data-writing
        // coordinate with no lifecycle creation must anchor its owner
        // explicitly. Close also creates the terminal record, and that plan's
        // protected outputs are only sound when the profile itself observes the
        // record's bump, rent principal and rent beneficiary.
        5 if action == Action::Close => Ok(AccountOperationInputV2::RequireOwner {
            account: primary,
            expected: common_identity(identity::TRADING_PROGRAM)?,
        }),
        6 if action == Action::Close => Ok(AccountOperationInputV2::ProjectDataU8 {
            account: terminal,
            destination: common_scalar(scalar::TERMINAL_BUMP_OBSERVATION)?,
            data_offset: GeneralLocalStateLayoutV3::bump(),
        }),
        7 if action == Action::Close => Ok(AccountOperationInputV2::ProjectDataU64 {
            account: terminal,
            destination: common_scalar(scalar::TERMINAL_PRINCIPAL_OBSERVATION)?,
            data_offset: GeneralLocalStateLayoutV3::rent_principal(),
        }),
        8 if action == Action::Close => Ok(AccountOperationInputV2::ProjectDataIdentity {
            account: terminal,
            destination: common_identity(identity::TERMINAL_BENEFICIARY_OBSERVATION)?,
            data_offset: GeneralLocalStateLayoutV3::beneficiary(),
        }),
        // THE MARKET GENERATION EVERY ACTION'S DOMAIN AUTHENTICATION READS.
        // See `general_generation_operation_index_v3` for why this is one
        // derived arm and not fifteen literals, and for the ten actions that
        // had no such operation at all.
        generation if generation == general_generation_operation_index_v3(action) => {
            Ok(AccountOperationInputV2::ProjectDataU64 {
                account: AccountCoordinateV2::fixed(narrow(HOT_RUNTIME_CONFIG_COORDINATE_V3)?),
                destination: common_scalar(scalar::GENERATION)?,
                data_offset: width(GeneralConfigV3Layout::GENERATION)?,
            })
        }
        // THE SYSTEM PROGRAM, BOUND TO THE IDENTITY THE PROFILE ALREADY TRUSTED.
        //
        // `TrustedBuiltinIdentityV2::SystemProgram` writes the program's key
        // into `RESULT_OWNER` and has done since this profile was written. What
        // was missing was an account to compare it against, so this operation
        // is the other half: the coordinate declared in `state_artifacts_v3`
        // must BE the program the trusted environment named, which is what
        // stops a caller supplying some other executable there.
        system if Some(system) == general_system_program_operation_index_v3(action) => {
            Ok(AccountOperationInputV2::RequireKey {
                account: AccountCoordinateV2::fixed(
                    crate::state_artifacts_v3::general_system_program_account_v3(action)
                        .ok_or(GeneralAccountRuleErrorV3::Geometry)?,
                ),
                expected: common_identity(identity::RESULT_OWNER)?,
            })
        }
        // THE PRODUCT RECORD DIGEST, WHICH NO RULE COULD EXPRESS UNTIL NOW.
        //
        // `authenticated_general_domain`'s fourth conjunct recomputes
        // `hash(product account)` and compares it to this register. Nothing
        // sourced it, and nothing COULD: every one of the twenty operations the
        // AccountProfile vocabulary carried read bytes at an offset, or a key,
        // an owner, lamports, or a tail count, and none of them computed a
        // digest. It was not a missing rule but a missing primitive, which is
        // why the basis conjunct in front of it could be repaired and this one
        // could not.
        //
        // `ProjectDataDigest` (`a5bb4390`, opcode 20) is that primitive, and it
        // does NOT teach the interpreter to hash: the digest is a fact the
        // ADAPTER establishes and this projects it exactly as `ProjectKey`
        // projects the key. An observation whose adapter established none
        // refuses `DataDigestUnavailable` rather than reading a zero register,
        // which is what stops a missing supply from looking like a match
        // against thirty-two zero bytes.
        digest if digest == general_product_digest_operation_index_v3(action) => {
            Ok(AccountOperationInputV2::ProjectDataDigest {
                account: AccountCoordinateV2::fixed(narrow(HOT_RUNTIME_PRODUCT_COORDINATE_V3)?),
                destination: common_identity(identity::PRODUCT_RECORD_DIGEST)?,
            })
        }
        // THE SEMANTIC BASIS, WHICH NOTHING SOURCED.
        //
        // Every one of the fifteen actions crosses `authenticated_general_domain`
        // before it does anything else, and that function hands the config the
        // bank's `SEMANTIC_BASIS_ID` for `require_market` to compare against its
        // own `claim_basis_id`. No operation in this file ever wrote that
        // register, so it arrived as thirty-two zero bytes and the conjunct could
        // not be satisfied by ANY producer -- only by a harness writing the
        // register by hand, which is what every General fixture here was doing.
        // Measured 2026-09-01 through real Trading ELFs: config `0x56` x32,
        // bank zero.
        //
        // The source is the authenticated PORTFOLIO record and MUST NOT be the
        // config. A register projected out of the config account would be
        // compared against the account it was read from and pass forever; this
        // tree has recorded three guards whose two sides moved together, one of
        // them inside the mechanism that had just been credited with closing a
        // hole. Portfolio is the Product-side authority for the same value and
        // is already this profile's source for operation zero, so the comparison
        // joins two records written by different acts.
        basis if basis == general_semantic_basis_operation_index_v3(action) => {
            Ok(AccountOperationInputV2::ProjectDataIdentity {
                account: AccountCoordinateV2::fixed(narrow(HOT_RUNTIME_PORTFOLIO_COORDINATE_V3)?),
                destination: common_identity(identity::SEMANTIC_BASIS_ID)?,
                data_offset: width(PORTFOLIO_CLAIM_BASIS_ID_OFFSET)?,
            })
        }
        // Every exact signer account Lifecycle may debit is anchored to the
        // trusted System Program identity. SubmitCandidate already carries
        // this same anchor in its signed-evidence suffix; CloseCandidate has
        // no create payer. This operation is second-to-last in the fixed body,
        // immediately before the root-identity projection, so PlaceOrder's two
        // affine item projections remain the exact tail.
        owner_anchor
            if !matches!(action, Action::SubmitCandidate | Action::CloseCandidate)
                && owner_anchor
                    == general_root_identity_operation_index_v3(action)
                        .checked_sub(1)
                        .ok_or(GeneralAccountRuleErrorV3::Geometry)? =>
        {
            Ok(AccountOperationInputV2::RequireOwner {
                account: AccountCoordinateV2::fixed(creation_payer),
                expected: common_identity(identity::RESULT_OWNER)?,
            })
        }
        // The General root, named into the register every state seed order
        // opens with. See `general_root_identity_operation_index_v3`.
        root_identity if root_identity == general_root_identity_operation_index_v3(action) => {
            Ok(AccountOperationInputV2::ProjectKey {
                account: AccountCoordinateV2::fixed(narrow(HOT_RUNTIME_ROOT_COORDINATE_V3)?),
                destination: common_identity(identity::GENERAL_ROOT)?,
            })
        }
        _ => Err(GeneralAccountRuleErrorV3::Geometry),
    }
}

/// Exact encoded width of the action-selected General Profile13 artifact.
///
/// # Errors
///
/// Refuses an action whose geometry overflows a `usize`.
pub fn general_account_profile_bytes_v3(action: Action) -> Result<usize> {
    let fixed_count = general_account_profile_fixed_count_v3(action)?;
    // The fixed rules and nothing else. The `+ 1` was the scratch-page span's
    // single item-rule template and the entry beneath it was the span itself;
    // General declares neither now.
    let rules = usize::from(fixed_count)
        .checked_mul(RULE_BYTES)
        .ok_or(GeneralAccountRuleErrorV3::Geometry)?;
    let operations = usize::from(general_account_profile_operation_count_v3(action))
        .checked_mul(OPERATION_BYTES)
        .ok_or(GeneralAccountRuleErrorV3::Geometry)?;
    DYNAMIC_FIXED_SPAN_HEADER_BYTES
        .checked_add(rules)
        .and_then(|value| value.checked_add(operations))
        .ok_or(GeneralAccountRuleErrorV3::Geometry)
}

/// Encode the action-selected General Profile13 artifact atomically.
///
/// The rules and the operation list already have exactly one author, which is
/// this module. The ENCODER INVOCATION did not: the trusted-environment
/// declaration, the scratch-page span, the extra scratch-page rule and the
/// register geometry were written out twice, once in the release builder and
/// once in a contract test fixture, with nothing able to compare them -- the
/// same shape `2e890d4` had to undo for the operation list, one level up.
/// This is that fix applied to the invocation, so the artifact has one author
/// end to end.
///
/// `scratch` and `output` must both be exactly [`general_account_profile_bytes_v3`]
/// wide. `output` is unchanged on every refusal.
///
/// # Errors
///
/// Refuses an action-selected geometry this module cannot generate, and every
/// refusal the AccountProfile V2 encoder raises against the candidate.
pub fn encode_general_account_profile_v3_atomic(
    action: Action,
    widths: GeneralExternalAccountWidthsV3,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<()> {
    let fixed_count = general_account_profile_fixed_count_v3(action)?;
    let mut operations = [GENERAL_ACCOUNT_PROFILE_OPERATION_PLACEHOLDER_V3;
        GENERAL_MAX_ACCOUNT_PROFILE_OPERATIONS_V3];
    let operation_count = usize::from(general_account_profile_operation_count_v3(action));
    for index in 0..operation_count {
        let operation = general_account_profile_operation_v3(
            action,
            u16::try_from(index).map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
        )?;
        *operations
            .get_mut(index)
            .ok_or(GeneralAccountRuleErrorV3::Geometry)? = operation;
    }
    let item_operation_count = usize::from(general_account_profile_item_operation_count_v3(action));
    let fixed_operation_count = operation_count
        .checked_sub(item_operation_count)
        .ok_or(GeneralAccountRuleErrorV3::Geometry)?;
    encode_account_profile_with_dynamic_fixed_span_v2_generated_atomic_with_item_operations(
        // The window-gated actions read the trusted current slot; a caller may
        // not state what time it is. `Freeze` joined them on 2026-09-04 with
        // the selection-window conjunct; the settlement six that remain declare
        // none, and adding one there would move six digests for nothing.
        if matches!(
            action,
            Action::OpenBatch
                | Action::CloseBatch
                | Action::PlaceOrder
                | Action::CancelOrder
                | Action::ReleaseOrder
                | Action::SubmitCandidate
                | Action::CloseCandidate
                | Action::Freeze
        ) {
            TrustedEnvironmentV2::CurrentSlot {
                destination: narrow_u32(scalar::CURRENT_SLOT)?,
            }
        } else {
            TrustedEnvironmentV2::None
        },
        TrustedIdentityEnvironmentV2::CurrentExecutingProgram {
            destination: narrow_u32(identity::TRADING_PROGRAM)?,
        },
        if action != Action::CloseCandidate {
            TrustedBuiltinIdentityV2::SystemProgram {
                destination: narrow_u32(identity::RESULT_OWNER)?,
            }
        } else {
            TrustedBuiltinIdentityV2::None
        },
        // NO DYNAMIC SPAN. General's only span was the input scratch-page
        // transport, whose page count came from the RETURN-DATA bound and whose
        // pages have no producer that can exist -- see
        // `docs/design/GENERAL_INPUT_TRANSPORT_2026_09_02.md`. The bank arrives
        // inline in the CPI instruction data instead. The profile stays
        // Profile13 because that is the encoder carrying General's trusted
        // environment, variable-data prestates and route aliases; it now
        // declares zero spans, which `validate_dynamic_fixed_spans` admits
        // exactly when the item-rule table and the item operations are empty
        // too, and both are.
        &[],
        fixed_count,
        |coordinate| {
            general_account_profile_rule_v3(action, coordinate, widths)
                .map_err(|_| dclutch_account_profile_contract::v2::Error::InvalidLength)
        },
        &[],
        operations
            .get(..fixed_operation_count)
            .ok_or(GeneralAccountRuleErrorV3::Geometry)?,
        operations
            .get(fixed_operation_count..operation_count)
            .ok_or(GeneralAccountRuleErrorV3::Geometry)?,
        RegisterGeometryV2 {
            common_scalars: narrow_u32(GENERAL_HOT_COMMON_SCALARS_V3)?,
            item_scalar_stride: narrow_u32(general_hot_item_scalar_stride_v3(action))?,
            common_identities: narrow_u32(GENERAL_HOT_COMMON_IDENTITIES_V3)?,
            item_identity_stride: 0,
        },
        scratch,
        output,
    )
    .map_err(|_| GeneralAccountRuleErrorV3::Geometry)
}

/// Widest canonical operation list any action declares.
const GENERAL_MAX_ACCOUNT_PROFILE_OPERATIONS_V3: usize = 38;

/// Inert operation the fixed-width operation buffer is filled with.
const GENERAL_ACCOUNT_PROFILE_OPERATION_PLACEHOLDER_V3: AccountOperationInputV2 =
    AccountOperationInputV2::ProjectLamports {
        account: AccountCoordinateV2::fixed(0),
        destination: ScalarCoordinateV2::common(0),
    };

const _: () = assert!(
    GENERAL_MAX_ACCOUNT_PROFILE_OPERATIONS_V3
        == general_account_profile_operation_count_v3(Action::SubmitCandidate) as usize
);

fn narrow_u32(value: u32) -> Result<u16> {
    u16::try_from(value).map_err(|_| GeneralAccountRuleErrorV3::Geometry)
}

fn narrow(value: usize) -> Result<u16> {
    u16::try_from(value).map_err(|_| GeneralAccountRuleErrorV3::Geometry)
}

fn width(value: usize) -> Result<u32> {
    u32::try_from(value).map_err(|_| GeneralAccountRuleErrorV3::Geometry)
}

/// One `GeneralRootV2` tail offset behind the immutable capability header.
fn root_tail_offset(offset: usize) -> Result<u32> {
    width(
        CAPABILITY_ROOT_HEADER_BYTES_V1
            .checked_add(offset)
            .ok_or(GeneralAccountRuleErrorV3::Geometry)?,
    )
}

/// One batch-record offset behind the General local-state envelope header.
fn batch_body_offset(offset: usize) -> Result<u32> {
    GeneralLocalStateLayoutV3::body()
        .checked_add(width(offset)?)
        .ok_or(GeneralAccountRuleErrorV3::Geometry)
}

/// One Candidate-record offset behind the General local-state envelope.
fn candidate_body_offset(offset: usize) -> Result<u32> {
    GeneralLocalStateLayoutV3::body()
        .checked_add(width(offset)?)
        .ok_or(GeneralAccountRuleErrorV3::Geometry)
}

/// CloseCandidate's independently authenticated Batch evidence coordinate.
fn close_candidate_batch_account() -> Result<AccountCoordinateV2> {
    let selected =
        crate::state_artifacts_v3::general_readonly_evidence_v3(Action::CloseCandidate, 0)
            .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?;
    if selected.kind != GeneralReadonlyEvidenceKindV3::ClosedBatch {
        return Err(GeneralAccountRuleErrorV3::Geometry);
    }
    Ok(AccountCoordinateV2::fixed(selected.coordinate))
}

/// The signed-terms evidence coordinate for one admission.
fn order_terms_account(action: Action) -> Result<AccountCoordinateV2> {
    let selected = crate::state_artifacts_v3::general_readonly_evidence_v3(action, 0)
        .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?;
    if selected.kind != GeneralReadonlyEvidenceKindV3::OrderTerms {
        return Err(GeneralAccountRuleErrorV3::Geometry);
    }
    Ok(AccountCoordinateV2::fixed(selected.coordinate))
}

fn submit_evidence_account(
    index: u16,
    expected: GeneralReadonlyEvidenceKindV3,
) -> Result<AccountCoordinateV2> {
    evidence_account(Action::SubmitCandidate, index, expected)
}

/// One action's readonly-evidence coordinate, refusing a kind it does not name.
///
/// The kind is an argument rather than a comment because the coordinate is a
/// derived number: `general_readonly_evidence_start_v3` moves with the account
/// prefix, and an operation pointed at the wrong evidence account reads
/// well-formed bytes at a plausible offset instead of refusing.
fn evidence_account(
    action: Action,
    index: u16,
    expected: GeneralReadonlyEvidenceKindV3,
) -> Result<AccountCoordinateV2> {
    let selected = crate::state_artifacts_v3::general_readonly_evidence_v3(action, index)
        .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?;
    if selected.kind != expected {
        return Err(GeneralAccountRuleErrorV3::Geometry);
    }
    Ok(AccountCoordinateV2::fixed(selected.coordinate))
}

/// One order-record offset behind the General local-state envelope header.
fn order_body_offset(offset: usize) -> Result<u32> {
    GeneralLocalStateLayoutV3::body()
        .checked_add(width(offset)?)
        .ok_or(GeneralAccountRuleErrorV3::Geometry)
}

fn common_scalar(coordinate: u32) -> Result<ScalarCoordinateV2> {
    Ok(ScalarCoordinateV2::common(
        u16::try_from(coordinate).map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
    ))
}

fn common_identity(coordinate: u32) -> Result<IdentityCoordinateV2> {
    Ok(IdentityCoordinateV2::common(
        u16::try_from(coordinate).map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
    ))
}

/// Generate one exact fixed rule in the action-selected General Profile13.
pub fn general_account_profile_rule_v3(
    action: Action,
    coordinate: u16,
    widths: GeneralExternalAccountWidthsV3,
) -> Result<AccountRuleWithPrestateInputV2> {
    widths.validate()?;
    if coordinate >= general_account_profile_fixed_count_v3(action)? {
        return Err(GeneralAccountRuleErrorV3::Geometry);
    }
    if coordinate < 5 {
        return common_rule(action, coordinate, widths);
    }
    // The System program: executable, never a signer, never writable, no data
    // this profile reads. Uniform across every action and checked BEFORE the
    // per-action state branches, because the commit needs it for any action
    // whose lifecycle creates a state and it costs nothing for the rest.
    // `AuthenticatedOpaqueReadonlyData` is the prestate Direct's ordinary
    // profile already uses for exactly this account.
    if crate::state_artifacts_v3::general_system_program_account_v3(action) == Some(coordinate) {
        return Ok(AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: AccountPrivilegesV2::new(false, false, true),
                effect_permissions: AccountEffectPermissionsV2::new(false, false, false),
                alias: AccountAliasInputV2::SelfCoordinate,
                data_length: 0,
                data_item_stride: 0,
            },
            prestate: AccountPrestateV2::AuthenticatedOpaqueReadonlyData,
        });
    }
    if action == Action::VerifyCandidateRow {
        match coordinate {
            GENERAL_PRIMARY_STATE_ACCOUNT_V3 => {
                return Ok(AccountRuleWithPrestateInputV2 {
                    rule: AccountRuleInputV2 {
                        privileges: AccountPrivilegesV2::new(false, true, false),
                        effect_permissions: AccountEffectPermissionsV2::new(true, false, true),
                        alias: AccountAliasInputV2::SelfCoordinate,
                        data_length: u32::try_from(
                            GENERAL_LOCAL_STATE_HEADER_BYTES_V3 + GENERAL_CANDIDATE_BYTES_V1,
                        )
                        .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
                        data_item_stride: 0,
                    },
                    prestate: AccountPrestateV2::Exact,
                });
            }
            GENERAL_VERIFY_VERIFIER_STATE_ACCOUNT_V3 => {
                return Ok(AccountRuleWithPrestateInputV2 {
                    rule: AccountRuleInputV2 {
                        privileges: AccountPrivilegesV2::new(false, true, false),
                        effect_permissions: AccountEffectPermissionsV2::new(false, true, true),
                        alias: AccountAliasInputV2::SelfCoordinate,
                        data_length: u32::try_from(
                            GENERAL_LOCAL_STATE_HEADER_BYTES_V3 + RUNTIME_VERIFIER_HEADER_BYTES_V2,
                        )
                        .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
                        data_item_stride: 40,
                    },
                    prestate: AccountPrestateV2::LifecycleBound,
                });
            }
            GENERAL_VERIFY_RESULT_STATE_ACCOUNT_V3 => {
                return Ok(AccountRuleWithPrestateInputV2 {
                    rule: AccountRuleInputV2 {
                        privileges: AccountPrivilegesV2::new(false, true, false),
                        effect_permissions: AccountEffectPermissionsV2::new(false, true, true),
                        alias: AccountAliasInputV2::SelfCoordinate,
                        data_length: u32::try_from(VERIFIED_CANDIDATE_HEADER_BYTES_V2)
                            .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
                        data_item_stride: 16,
                    },
                    prestate: AccountPrestateV2::LifecycleBound,
                });
            }
            GENERAL_VERIFY_PAYER_ACCOUNT_V3 => {
                return Ok(exact_rule(
                    true,
                    true,
                    false,
                    0,
                    0,
                    // Effect credits the permissionless crank; Lifecycle may
                    // debit the same signer when either the resumable Verifier
                    // or conditional terminal Result is vacant. The emitted
                    // effect never consumes the debit authority, but the
                    // profile must cover both live and create lifecycle
                    // branches selected by the same immutable artifacts.
                    AccountEffectPermissionsV2::new(true, true, false),
                ));
            }
            GENERAL_VERIFY_RENT_CREDIT_ACCOUNT_V3 => {
                return Ok(exact_rule(
                    false,
                    true,
                    false,
                    widths.rent_credit,
                    0,
                    no_effects(),
                ));
            }
            _ => {}
        }
    }
    if coordinate == GENERAL_PRIMARY_STATE_ACCOUNT_V3
        || (matches!(
            action,
            Action::Close | Action::PlaceOrder | Action::CancelOrder
        ) && coordinate == GENERAL_TERMINAL_STATE_ACCOUNT_V3)
    {
        return local_state_rule(action, coordinate);
    }
    let two_state = matches!(
        action,
        Action::Close | Action::PlaceOrder | Action::CancelOrder
    );
    let payer = if two_state {
        GENERAL_CLOSE_PAYER_ACCOUNT_V3
    } else {
        GENERAL_PRIMARY_PAYER_ACCOUNT_V3
    };
    let rent_credit = if two_state {
        GENERAL_CLOSE_RENT_CREDIT_ACCOUNT_V3
    } else {
        GENERAL_PRIMARY_RENT_CREDIT_ACCOUNT_V3
    };
    if coordinate == payer {
        return Ok(exact_rule(
            true,
            true,
            false,
            0,
            0,
            if action == Action::CloseCandidate {
                AccountEffectPermissionsV2::new(false, true, false)
            } else {
                // Every other action selecting this coordinate carries at
                // least one Create or AuthenticateOrCreate plan. This debit
                // permission is lifecycle authority even when the currently
                // observed branch is already live.
                AccountEffectPermissionsV2::new(true, false, false)
            },
        ));
    }
    if coordinate == rent_credit {
        return Ok(exact_rule(
            false,
            true,
            false,
            widths.rent_credit,
            0,
            if matches!(action, Action::Close | Action::CloseCandidate) {
                AccountEffectPermissionsV2::new(false, true, false)
            } else {
                no_effects()
            },
        ));
    }
    let mut evidence = 0_u16;
    while evidence < general_readonly_evidence_count_v3(action) {
        let selected = general_readonly_evidence_v3(action, evidence)
            .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?;
        if selected.coordinate == coordinate {
            return evidence_rule(selected.kind);
        }
        evidence = evidence
            .checked_add(1)
            .ok_or(GeneralAccountRuleErrorV3::Geometry)?;
    }
    // The release-selected Custody program the action's Custody routes are
    // invoked through, appended past every route range. Readonly executable, no
    // effect permission, no asserted width: the loader that deployed it owns
    // the record and the Registry activation cache, not this profile, is the
    // sole authority on which program the Custody role selects. It belongs to
    // no child frame, so it is answered before `child_rule` -- which would
    // refuse a coordinate `child_coordinate` cannot place.
    if general_custody_callee_coordinate_v3(action)
        .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?
        == Some(coordinate)
    {
        return Ok(opaque_rule(AccountPrivilegesV2::new(false, false, true)));
    }
    child_rule(action, coordinate, widths)
}

fn common_rule(
    action: Action,
    coordinate: u16,
    widths: GeneralExternalAccountWidthsV3,
) -> Result<AccountRuleWithPrestateInputV2> {
    match coordinate {
        // The capability root. Physically writable already; what is withheld is
        // the EFFECT grant, and the seven settlement actions need none — none of
        // them advances the root.
        //
        // **`OpenBatch` and `CloseBatch` will need it, and it is available.**
        // `GeneralRootV2::open_batch` / `close_batch` advance `revision`,
        // `next_batch_sequence` and `open_batches`, and neither has ever run on
        // a chain: every caller in the tree is host-side or a test, so this
        // root's revision has been frozen at its activation value since the day
        // it was created. Establishing that the write is possible at all was the
        // one blocker-class question in front of those two triples, and the
        // answer is yes on every leg:
        //
        // * the composite root is a TRADING-owned PDA -- `outer.rs` allocates it
        //   and assigns it to the family program, and Core's activation
        //   post-condition requires exactly that owner;
        // * Trading's commit path guards root writes by OFFSET, not by owner:
        //   `require_root_write_is_state_only` refuses only offsets below
        //   `CAPABILITY_ROOT_HEADER_BYTES_V1`, which is precisely the boundary
        //   the `GeneralRootV2` tail begins at;
        // * coordinate 0 is deliberately exempt from the read-only clamp Trading
        //   applies to common coordinates 1..=4;
        // * and Direct and Series already do it -- Direct's registered and
        //   ordinary effect programs write the root's open-maker count through
        //   this exact shape.
        //
        // So the change those triples need here is one argument: `no_effects()`
        // becomes `AccountEffectPermissionsV2::new(false, false, true)`, action-
        // selected, for the two actions that advance the root and no others.
        // Granting it to an action that does not write the root would widen what
        // a release may do for nothing in return.
        0 => Ok(exact_rule(
            false,
            true,
            false,
            u32::try_from(
                dclutch_capability_program_contract::CAPABILITY_ROOT_HEADER_BYTES_V1
                    .checked_add(dclutch_general_config_contract::GENERAL_ROOT_BYTES_V2)
                    .ok_or(GeneralAccountRuleErrorV3::Geometry)?,
            )
            .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
            0,
            // The one argument the long comment above promised: the two
            // root-advancing actions get the data-effect grant, and no other
            // action does. Trading's own offset guard keeps every such write
            // behind the immutable capability header.
            if matches!(action, Action::OpenBatch | Action::CloseBatch) {
                AccountEffectPermissionsV2::new(false, false, true)
            } else {
                no_effects()
            },
        )),
        1 => Ok(exact_rule(
            false,
            false,
            false,
            u32::try_from(GENERAL_CONFIG_BYTES_V3)
                .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
            0,
            no_effects(),
        )),
        2 => Ok(rule(
            physical_role_privileges(action, ChildRoleV3::ProductRecord)?,
            u32::try_from(PRODUCT_RECORD_BYTES_V2)
                .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
            0,
            AccountPrestateV2::Exact,
        )),
        3 => Ok(rule(
            physical_role_privileges(action, ChildRoleV3::PortfolioRecord)?,
            u32::try_from(PORTFOLIO_HEADER_BYTES)
                .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
            u32::try_from(PORTFOLIO_COEFFICIENT_BYTES)
                .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
            AccountPrestateV2::Exact,
        )),
        4 => Ok(variable_rule_with(
            physical_role_privileges(action, ChildRoleV3::BasisRecord)?,
            widths.linked_basis_prefix,
        )),
        _ => Err(GeneralAccountRuleErrorV3::Geometry),
    }
}

fn local_state_rule(action: Action, coordinate: u16) -> Result<AccountRuleWithPrestateInputV2> {
    let cancel_order_state = matches!(action, Action::PlaceOrder | Action::CancelOrder)
        && coordinate == GENERAL_TERMINAL_STATE_ACCOUNT_V3;
    let semantic_header = if matches!(action, Action::SubmitCandidate | Action::CloseCandidate) {
        GENERAL_CANDIDATE_BYTES_V1
    } else if matches!(action, Action::Consider | Action::Freeze) {
        RUNTIME_SELECTION_CURSOR_BYTES_V2
    } else if matches!(action, Action::OpenBatch | Action::CloseBatch)
        || (matches!(action, Action::PlaceOrder | Action::CancelOrder) && !cancel_order_state)
    {
        GENERAL_BATCH_BYTES_V1
    } else if action == Action::ReleaseOrder || cancel_order_state {
        // The order record's fixed span; the per-outcome rows are the stride.
        GENERAL_ORDER_ROW_BASE_V1
    } else {
        SETTLEMENT_CURSOR_HEADER_BYTES_V2
    };
    // `LifecycleBound` admits either vacant data or the declared live width,
    // and that is the truth for every coordinate a lifecycle plan may create.
    // Close creates only the terminal record. Its settlement cursor is the
    // account being closed: the Close plan and the operator both require it
    // live at exactly this width, so declaring it possibly-vacant was a
    // weaker refusal than the transition it guards.
    let closed_settlement_state =
        action == Action::Close && coordinate == GENERAL_PRIMARY_STATE_ACCOUNT_V3;
    Ok(AccountRuleWithPrestateInputV2 {
        rule: AccountRuleInputV2 {
            privileges: AccountPrivilegesV2::new(false, true, false),
            effect_permissions: AccountEffectPermissionsV2::new(
                action == Action::CloseCandidate
                    || (action == Action::Close && coordinate == GENERAL_PRIMARY_STATE_ACCOUNT_V3),
                true,
                true,
            ),
            alias: AccountAliasInputV2::SelfCoordinate,
            data_length: u32::try_from(
                GENERAL_LOCAL_STATE_HEADER_BYTES_V3
                    .checked_add(semantic_header)
                    .ok_or(GeneralAccountRuleErrorV3::Geometry)?,
            )
            .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
            data_item_stride: if matches!(action, Action::SubmitCandidate | Action::CloseCandidate)
                || matches!(
                    action,
                    Action::Consider | Action::Freeze | Action::OpenBatch | Action::CloseBatch
                )
                || (matches!(action, Action::PlaceOrder | Action::CancelOrder)
                    && !cancel_order_state)
            {
                0
            } else if action == Action::ReleaseOrder || cancel_order_state {
                u32::try_from(GENERAL_ORDER_ROW_STRIDE_V1)
                    .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?
            } else {
                8
            },
        },
        prestate: if closed_settlement_state || action == Action::CloseCandidate {
            AccountPrestateV2::Exact
        } else {
            AccountPrestateV2::LifecycleBound
        },
    })
}

fn evidence_rule(kind: GeneralReadonlyEvidenceKindV3) -> Result<AccountRuleWithPrestateInputV2> {
    match kind {
        // The identity-covered image of an order record: the fixed header,
        // then one interleaved 16-byte row per runtime outcome. Readonly,
        // no effects; the maker's signature on the transaction is what
        // endorses the bytes.
        GeneralReadonlyEvidenceKindV3::OrderTerms => Ok(rule(
            AccountPrivilegesV2::new(false, false, false),
            u32::try_from(GENERAL_ORDER_HEADER_BYTES_V1)
                .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
            u32::try_from(GENERAL_ORDER_ROW_STRIDE_V1)
                .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
            AccountPrestateV2::Exact,
        )),
        GeneralReadonlyEvidenceKindV3::ClosedBatch => Ok(exact_rule(
            false,
            false,
            false,
            u32::try_from(GENERAL_LOCAL_STATE_HEADER_BYTES_V3 + GENERAL_BATCH_BYTES_V1)
                .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
            0,
            no_effects(),
        )),
        GeneralReadonlyEvidenceKindV3::CandidateImage => Ok(exact_rule(
            false,
            false,
            false,
            u32::try_from(CANDIDATE_HEADER_BYTES_V2)
                .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
            8,
            no_effects(),
        )),
        // One immutable page has a fixed hostile-decoded header followed by a
        // runtime number of canonical execution rows. The AccountProfile can
        // authenticate its readonly carrier and minimum prefix, while
        // `PageV2::decode` remains the sole owner of the exact
        // `64 + rows * (112 + 16N)` width and row ordering.
        GeneralReadonlyEvidenceKindV3::CandidatePage => Ok(variable_rule(
            u32::try_from(PAGE_HEADER_BYTES_V2).map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
        )),
        // Verification consumes the actual escrowed order local state, not a
        // detached immutable terms image. Its envelope and mutable window are
        // fixed; the canonical receive/deliver pair contributes one 16-byte
        // row per Product outcome.
        GeneralReadonlyEvidenceKindV3::EscrowedOrder => Ok(exact_rule(
            false,
            false,
            false,
            u32::try_from(GENERAL_LOCAL_STATE_HEADER_BYTES_V3 + GENERAL_ORDER_ROW_BASE_V1)
                .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
            u32::try_from(GENERAL_ORDER_ROW_STRIDE_V1)
                .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
            no_effects(),
        )),
        GeneralReadonlyEvidenceKindV3::SubmittedCandidate => Ok(exact_rule(
            false,
            false,
            false,
            u32::try_from(GENERAL_CANDIDATE_BYTES_V1)
                .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
            0,
            no_effects(),
        )),
        GeneralReadonlyEvidenceKindV3::SelectionPolicy => Ok(exact_rule(
            false,
            false,
            false,
            u32::try_from(SELECTION_POLICY_BYTES)
                .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
            0,
            no_effects(),
        )),
        GeneralReadonlyEvidenceKindV3::SubmittedVerifiedCandidate
        | GeneralReadonlyEvidenceKindV3::SelectedVerifiedCandidate => Ok(exact_rule(
            false,
            false,
            false,
            u32::try_from(VERIFIED_CANDIDATE_HEADER_BYTES_V2)
                .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
            16,
            no_effects(),
        )),
        GeneralReadonlyEvidenceKindV3::FrozenSelection => Ok(exact_rule(
            false,
            false,
            false,
            u32::try_from(RUNTIME_SELECTION_CURSOR_BYTES_V2)
                .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
            0,
            no_effects(),
        )),
        GeneralReadonlyEvidenceKindV3::RuntimeVerifier => Ok(exact_rule(
            false,
            false,
            false,
            u32::try_from(RUNTIME_VERIFIER_HEADER_BYTES_V2)
                .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
            40,
            no_effects(),
        )),
        GeneralReadonlyEvidenceKindV3::SettlementManifest => Ok(variable_rule(
            u32::try_from(SETTLEMENT_MANIFEST_HEADER_BYTES_V2)
                .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
        )),
    }
}

fn child_rule(
    action: Action,
    coordinate: u16,
    widths: GeneralExternalAccountWidthsV3,
) -> Result<AccountRuleWithPrestateInputV2> {
    let (frame, relative) = child_coordinate(action, coordinate)?;
    let role = child_role(frame, relative)?;
    let privileges = physical_role_privileges(action, role)?;
    if let Some(representative) = prior_role_coordinate(action, coordinate, role)? {
        return Ok(AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: AccountPrivilegesV2::new(false, false, false),
                effect_permissions: no_effects(),
                alias: AccountAliasInputV2::Fixed(representative),
                data_length: 0,
                data_item_stride: 0,
            },
            prestate: AccountPrestateV2::AuthenticatedRouteAlias,
        });
    }
    match frame {
        GeneralChildFrameV3::ClaimsProtocolPosition(action) => claims_data_rule(
            ClaimsFrameSpecV1::protocol_position(action)
                .data(relative)
                .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
            privileges,
            widths,
        ),
        GeneralChildFrameV3::ClaimsAffine { position_count } => claims_data_rule(
            ClaimsFrameSpecV1::affine(position_count)
                .and_then(|spec| spec.data(relative))
                .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
            privileges,
            widths,
        ),
        GeneralChildFrameV3::Custody(operation) => custody_data_rule(
            CustodyFrameSpecV1::new(operation)
                .data(relative)
                .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
            privileges,
            widths,
        ),
    }
}

fn child_coordinate(action: Action, coordinate: u16) -> Result<(GeneralChildFrameV3, u16)> {
    let mut route = 0_u16;
    while route < general_effect_route_count_v3(action) {
        let selected = general_effect_route_frame_v3(action, route)
            .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?;
        let count = selected
            .frame
            .account_count()
            .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?;
        let end = selected
            .account_start
            .checked_add(count)
            .ok_or(GeneralAccountRuleErrorV3::Geometry)?;
        if coordinate >= selected.account_start && coordinate < end {
            return Ok((selected.frame, coordinate - selected.account_start));
        }
        route = route
            .checked_add(1)
            .ok_or(GeneralAccountRuleErrorV3::Geometry)?;
    }
    Err(GeneralAccountRuleErrorV3::Geometry)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ChildRoleV3 {
    CallerAuthority,
    CoreMarket,
    ActivationCache,
    RegistryProgram,
    TradingProgram,
    TradingProgramData,
    RentSysvar,
    SystemProgram,
    ProductRecord,
    PortfolioRecord,
    BasisRecord,
    Claims(ClaimsFrameRoleV1),
    Custody(CustodyFrameRoleV1),
}

fn child_role(frame: GeneralChildFrameV3, relative: u16) -> Result<ChildRoleV3> {
    match frame {
        GeneralChildFrameV3::ClaimsProtocolPosition(action) => {
            ClaimsFrameSpecV1::protocol_position(action)
                .account(relative)
                .map(|account| normalize_claims_role(account.role()))
                .map_err(|_| GeneralAccountRuleErrorV3::Geometry)
        }
        GeneralChildFrameV3::ClaimsAffine { position_count } => {
            ClaimsFrameSpecV1::affine(position_count)
                .and_then(|spec| spec.account(relative))
                .map(|account| normalize_claims_role(account.role()))
                .map_err(|_| GeneralAccountRuleErrorV3::Geometry)
        }
        GeneralChildFrameV3::Custody(operation) => CustodyFrameSpecV1::new(operation)
            .account(relative)
            .map(|account| normalize_custody_role(account.role()))
            .map_err(|_| GeneralAccountRuleErrorV3::Geometry),
    }
}

fn normalize_claims_role(role: ClaimsFrameRoleV1) -> ChildRoleV3 {
    match role {
        ClaimsFrameRoleV1::CallerAuthority => ChildRoleV3::CallerAuthority,
        ClaimsFrameRoleV1::CoreMarket => ChildRoleV3::CoreMarket,
        ClaimsFrameRoleV1::ActivationCache => ChildRoleV3::ActivationCache,
        ClaimsFrameRoleV1::RegistryProgram => ChildRoleV3::RegistryProgram,
        ClaimsFrameRoleV1::TradingProgram | ClaimsFrameRoleV1::CallerProgram => {
            ChildRoleV3::TradingProgram
        }
        ClaimsFrameRoleV1::TradingProgramData | ClaimsFrameRoleV1::CallerProgramData => {
            ChildRoleV3::TradingProgramData
        }
        ClaimsFrameRoleV1::RentSysvar => ChildRoleV3::RentSysvar,
        ClaimsFrameRoleV1::SystemProgram => ChildRoleV3::SystemProgram,
        ClaimsFrameRoleV1::ProductRecord => ChildRoleV3::ProductRecord,
        ClaimsFrameRoleV1::PortfolioRecord => ChildRoleV3::PortfolioRecord,
        ClaimsFrameRoleV1::BasisRecord => ChildRoleV3::BasisRecord,
        other => ChildRoleV3::Claims(other),
    }
}

fn normalize_custody_role(role: CustodyFrameRoleV1) -> ChildRoleV3 {
    match role {
        CustodyFrameRoleV1::CallerAuthority => ChildRoleV3::CallerAuthority,
        CustodyFrameRoleV1::CoreMarket => ChildRoleV3::CoreMarket,
        CustodyFrameRoleV1::ActivationCache => ChildRoleV3::ActivationCache,
        CustodyFrameRoleV1::RegistryProgram => ChildRoleV3::RegistryProgram,
        CustodyFrameRoleV1::CallerProgram => ChildRoleV3::TradingProgram,
        CustodyFrameRoleV1::CallerProgramData => ChildRoleV3::TradingProgramData,
        CustodyFrameRoleV1::RentSysvar => ChildRoleV3::RentSysvar,
        CustodyFrameRoleV1::SystemProgram => ChildRoleV3::SystemProgram,
        other => ChildRoleV3::Custody(other),
    }
}

fn prior_role_coordinate(
    action: Action,
    coordinate: u16,
    role: ChildRoleV3,
) -> Result<Option<u16>> {
    let common = match role {
        ChildRoleV3::ProductRecord => Some(2),
        ChildRoleV3::PortfolioRecord => Some(3),
        ChildRoleV3::BasisRecord => Some(4),
        _ => None,
    };
    if common.is_some() {
        return Ok(common);
    }
    let mut prior = crate::state_artifacts_v3::general_child_account_start_v3(action);
    while prior < coordinate {
        let (frame, relative) = child_coordinate(action, prior)?;
        if child_role(frame, relative)? == role {
            return Ok(Some(prior));
        }
        prior = prior
            .checked_add(1)
            .ok_or(GeneralAccountRuleErrorV3::Geometry)?;
    }
    Ok(None)
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ChildPrivilegeFactsV3 {
    signer: bool,
    writable: bool,
    executable: bool,
}

impl ChildPrivilegeFactsV3 {
    fn include(&mut self, other: Self) {
        self.signer |= other.signer;
        self.writable |= other.writable;
        self.executable |= other.executable;
    }

    fn account_privileges(self) -> AccountPrivilegesV2 {
        AccountPrivilegesV2::new(self.signer, self.writable, self.executable)
    }
}

fn child_privilege_facts(
    frame: GeneralChildFrameV3,
    relative: u16,
) -> Result<ChildPrivilegeFactsV3> {
    match frame {
        GeneralChildFrameV3::ClaimsProtocolPosition(action) => {
            ClaimsFrameSpecV1::protocol_position(action)
                .account(relative)
                .map(|account| claims_privilege_facts(account.privileges()))
                .map_err(|_| GeneralAccountRuleErrorV3::Geometry)
        }
        GeneralChildFrameV3::ClaimsAffine { position_count } => {
            ClaimsFrameSpecV1::affine(position_count)
                .and_then(|spec| spec.account(relative))
                .map(|account| claims_privilege_facts(account.privileges()))
                .map_err(|_| GeneralAccountRuleErrorV3::Geometry)
        }
        GeneralChildFrameV3::Custody(operation) => CustodyFrameSpecV1::new(operation)
            .account(relative)
            .map(|account| custody_privilege_facts(account.privileges()))
            .map_err(|_| GeneralAccountRuleErrorV3::Geometry),
    }
}

fn claims_privilege_facts(value: FramePrivilegesV1) -> ChildPrivilegeFactsV3 {
    ChildPrivilegeFactsV3 {
        signer: value.signer(),
        writable: value.writable(),
        executable: value.executable(),
    }
}

fn custody_privilege_facts(value: CustodyFramePrivilegesV1) -> ChildPrivilegeFactsV3 {
    ChildPrivilegeFactsV3 {
        signer: value.signer(),
        writable: value.writable(),
        executable: value.executable(),
    }
}

fn physical_role_privileges(action: Action, role: ChildRoleV3) -> Result<AccountPrivilegesV2> {
    let mut union = ChildPrivilegeFactsV3::default();
    let mut coordinate = crate::state_artifacts_v3::general_child_account_start_v3(action);
    // Every coordinate in this range belongs to a child frame EXCEPT the
    // trailing Custody callee, which belongs to none -- it is the account the
    // CPI is made THROUGH, not one the CPI is made WITH. `child_coordinate`
    // cannot place it and returns `Geometry`, which would take every privilege
    // union in the action down with it, so the walk stops before it.
    let count = general_child_frame_end_v3(action)?;
    while coordinate < count {
        let (frame, relative) = child_coordinate(action, coordinate)?;
        if child_role(frame, relative)? == role {
            union.include(child_privilege_facts(frame, relative)?);
        }
        coordinate = coordinate
            .checked_add(1)
            .ok_or(GeneralAccountRuleErrorV3::Geometry)?;
    }
    Ok(union.account_privileges())
}

fn claims_data_rule(
    data: ClaimsFrameDataV1,
    privileges: AccountPrivilegesV2,
    widths: GeneralExternalAccountWidthsV3,
) -> Result<AccountRuleWithPrestateInputV2> {
    match data {
        ClaimsFrameDataV1::Exact(bytes) => Ok(rule(privileges, bytes, 0, AccountPrestateV2::Exact)),
        ClaimsFrameDataV1::OpaqueData | ClaimsFrameDataV1::PositionOwnerIdentity => {
            Ok(opaque_rule(privileges))
        }
        ClaimsFrameDataV1::ProductTail { base, item_stride } => Ok(rule(
            privileges,
            base,
            item_stride,
            AccountPrestateV2::Exact,
        )),
        ClaimsFrameDataV1::LinkedBasisRecord => {
            Ok(variable_rule_with(privileges, widths.linked_basis_prefix))
        }
        ClaimsFrameDataV1::ProductRecord => exact_external(privileges, PRODUCT_RECORD_BYTES_V2),
        ClaimsFrameDataV1::ResultDomainRecord => Ok(rule(
            privileges,
            widths.result_domain,
            0,
            AccountPrestateV2::Exact,
        )),
        ClaimsFrameDataV1::PortfolioRecord => Ok(rule(
            privileges,
            u32::try_from(PORTFOLIO_HEADER_BYTES)
                .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
            u32::try_from(PORTFOLIO_COEFFICIENT_BYTES)
                .map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
            AccountPrestateV2::Exact,
        )),
        ClaimsFrameDataV1::RentSysvar => Ok(rule(
            privileges,
            widths.rent_sysvar,
            0,
            AccountPrestateV2::Exact,
        )),
        ClaimsFrameDataV1::CoreMarket => Ok(rule(
            privileges,
            widths.core_market,
            0,
            AccountPrestateV2::Exact,
        )),
        ClaimsFrameDataV1::ActivationCache => Ok(rule(
            privileges,
            widths.activation_cache,
            0,
            AccountPrestateV2::Exact,
        )),
        ClaimsFrameDataV1::UpgradeableProgram => Ok(rule(
            privileges,
            widths.upgradeable_program,
            0,
            AccountPrestateV2::Exact,
        )),
        ClaimsFrameDataV1::ProgramData(role) => {
            let prefix = match role {
                dclutch_claims_svm::frame_spec_v1::ClaimsProgramDataRoleV1::Trading
                | dclutch_claims_svm::frame_spec_v1::ClaimsProgramDataRoleV1::Caller => {
                    widths.trading_programdata_prefix
                }
                dclutch_claims_svm::frame_spec_v1::ClaimsProgramDataRoleV1::Claims => {
                    widths.claims_programdata_prefix
                }
                dclutch_claims_svm::frame_spec_v1::ClaimsProgramDataRoleV1::Core => {
                    widths.core_programdata_prefix
                }
            };
            Ok(variable_rule_with(privileges, prefix))
        }
        ClaimsFrameDataV1::RentCredit => Ok(rule(
            privileges,
            widths.rent_credit,
            0,
            AccountPrestateV2::Exact,
        )),
    }
}

fn custody_data_rule(
    data: CustodyFrameDataV1,
    privileges: AccountPrivilegesV2,
    widths: GeneralExternalAccountWidthsV3,
) -> Result<AccountRuleWithPrestateInputV2> {
    match data {
        CustodyFrameDataV1::Exact(bytes) => {
            Ok(rule(privileges, bytes, 0, AccountPrestateV2::Exact))
        }
        CustodyFrameDataV1::OpaqueData => Ok(opaque_rule(privileges)),
        CustodyFrameDataV1::CoreMarket => Ok(rule(
            privileges,
            widths.core_market,
            0,
            AccountPrestateV2::Exact,
        )),
        CustodyFrameDataV1::ActivationCache => Ok(rule(
            privileges,
            widths.activation_cache,
            0,
            AccountPrestateV2::Exact,
        )),
        CustodyFrameDataV1::UpgradeableProgram => Ok(rule(
            privileges,
            widths.upgradeable_program,
            0,
            AccountPrestateV2::Exact,
        )),
        // A Realm-selected token program owns the byte width of its own mint
        // and token accounts -- a Token-2022 mint carrying extensions is not
        // 82 bytes and an ImmutableOwner account is not 165 -- and the loader
        // that deployed that program owns the program record's width. None of
        // those three widths is General's to assert, and Custody independently
        // authenticates all three accounts against the authenticated Realm, so
        // the outer restatement was strictly weaker than the child's own
        // check. This is ee1dc7d's collateral-width ruling applied to the
        // General child frames.
        CustodyFrameDataV1::TokenProgram
        | CustodyFrameDataV1::TokenMint
        | CustodyFrameDataV1::TokenAccount => Ok(opaque_rule(privileges)),
        CustodyFrameDataV1::CallerProgramData => Ok(variable_rule_with(
            privileges,
            widths.trading_programdata_prefix,
        )),
        CustodyFrameDataV1::RealmRecord => Ok(rule(
            privileges,
            widths.realm_record,
            0,
            AccountPrestateV2::Exact,
        )),
        CustodyFrameDataV1::RentSysvar => Ok(rule(
            privileges,
            widths.rent_sysvar,
            0,
            AccountPrestateV2::Exact,
        )),
    }
}

fn exact_external(
    privileges: AccountPrivilegesV2,
    bytes: usize,
) -> Result<AccountRuleWithPrestateInputV2> {
    Ok(rule(
        privileges,
        u32::try_from(bytes).map_err(|_| GeneralAccountRuleErrorV3::Geometry)?,
        0,
        AccountPrestateV2::Exact,
    ))
}

const fn no_effects() -> AccountEffectPermissionsV2 {
    AccountEffectPermissionsV2::new(false, false, false)
}

const fn exact_rule(
    signer: bool,
    writable: bool,
    executable: bool,
    data_length: u32,
    data_item_stride: u32,
    effect_permissions: AccountEffectPermissionsV2,
) -> AccountRuleWithPrestateInputV2 {
    AccountRuleWithPrestateInputV2 {
        rule: AccountRuleInputV2 {
            privileges: AccountPrivilegesV2::new(signer, writable, executable),
            effect_permissions,
            alias: AccountAliasInputV2::SelfCoordinate,
            data_length,
            data_item_stride,
        },
        prestate: AccountPrestateV2::Exact,
    }
}

const fn rule(
    privileges: AccountPrivilegesV2,
    data_length: u32,
    data_item_stride: u32,
    prestate: AccountPrestateV2,
) -> AccountRuleWithPrestateInputV2 {
    AccountRuleWithPrestateInputV2 {
        rule: AccountRuleInputV2 {
            privileges,
            effect_permissions: no_effects(),
            alias: AccountAliasInputV2::SelfCoordinate,
            data_length,
            data_item_stride,
        },
        prestate,
    }
}

const fn variable_rule(prefix: u32) -> AccountRuleWithPrestateInputV2 {
    variable_rule_with(AccountPrivilegesV2::new(false, false, false), prefix)
}

const fn variable_rule_with(
    privileges: AccountPrivilegesV2,
    prefix: u32,
) -> AccountRuleWithPrestateInputV2 {
    rule(
        privileges,
        prefix,
        0,
        AccountPrestateV2::AdapterAuthenticatedVariableData,
    )
}

const fn opaque_rule(privileges: AccountPrivilegesV2) -> AccountRuleWithPrestateInputV2 {
    rule(
        privileges,
        0,
        0,
        AccountPrestateV2::AuthenticatedOpaqueReadonlyData,
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic)]

    extern crate std;

    use dclutch_account_profile_contract::lifecycle_v3::encode::LifecycleSeedInputV3;
    use dclutch_effect_kernel::v2::FixedRole;
    use std::vec;

    use super::*;
    use crate::effect_artifacts_v3::{
        GENERAL_EFFECT_INSTRUCTION_PLACEHOLDER_V3, encode_general_effect_program_v3_atomic,
        general_effect_instruction_count_v3, general_effect_program_bytes_v3,
        general_effect_template_bytes_v3,
    };
    use crate::state_seeds_v3::{GENERAL_ROOT_IDENTITY_REGISTER_V3, GeneralStateRecipeV3};

    /// The exact emitted Effect bytes for one action.
    fn effect(action: Action) -> vec::Vec<u8> {
        let (fixed, item) = general_effect_instruction_count_v3(action);
        let mut instructions = vec![GENERAL_EFFECT_INSTRUCTION_PLACEHOLDER_V3; fixed + item];
        let mut templates = vec![0; general_effect_template_bytes_v3(action)];
        let len = general_effect_program_bytes_v3(action).expect("program width");
        let mut scratch = vec![0; len];
        let mut output = vec![0; len];
        encode_general_effect_program_v3_atomic(
            action,
            &mut instructions,
            &mut templates,
            &mut scratch,
            &mut output,
        )
        .expect("action artifact");
        output
    }

    const WIDTHS: GeneralExternalAccountWidthsV3 = GeneralExternalAccountWidthsV3 {
        linked_basis_prefix: 64,
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

    const ACTIONS: [Action; 15] = [
        Action::Consider,
        Action::Freeze,
        Action::InitializeSettlement,
        Action::Collect,
        Action::Materialize,
        Action::Distribute,
        Action::Close,
        Action::OpenBatch,
        Action::PlaceOrder,
        Action::CancelOrder,
        Action::CloseBatch,
        Action::SubmitCandidate,
        Action::VerifyCandidateRow,
        Action::ReleaseOrder,
        Action::CloseCandidate,
    ];

    /// The window-gated actions put the CURRENT SLOT in their register bank.
    ///
    /// This is small and it is the reason the input scratch-page transport
    /// could never have a producer. `TrustedEnvironmentV2::CurrentSlot` makes
    /// Trading seed `scalar::CURRENT_SLOT` from `Clock::get()` on every
    /// execution, and `require_trusted_environment_v3` refuses a projection
    /// that disagrees -- so the bank's bytes, and every digest over them, are
    /// different in every slot. Anything outside the executing instruction that
    /// has to STATE that bank is therefore valid for exactly one slot, which no
    /// caller can deliver into. Measured before the transport went inline: the
    /// same bundle one slot later refused `0x4018 AdmittedTransport` after
    /// 501,968 CU (`1fee82fa`).
    #[test]
    fn the_window_gated_actions_declare_the_current_slot_in_their_bank() {
        for action in ACTIONS {
            let bytes = general_account_profile_bytes_v3(action).expect("profile width");
            let mut encoded = vec![0_u8; bytes];
            let mut scratch = vec![0_u8; bytes];
            encode_general_account_profile_v3_atomic(action, WIDTHS, &mut scratch, &mut encoded)
                .expect("account profile");
            let profile = dclutch_account_profile_contract::v2::AccountProfileV2::decode(&encoded)
                .expect("decode");
            let declared = matches!(
                profile.trusted_environment(),
                dclutch_account_profile_contract::v2::TrustedEnvironmentV2::CurrentSlot { .. }
            );
            assert_eq!(
                declared,
                matches!(
                    action,
                    Action::OpenBatch
                        | Action::CloseBatch
                        | Action::PlaceOrder
                        | Action::CancelOrder
                        | Action::ReleaseOrder
                        | Action::SubmitCandidate
                        | Action::CloseCandidate
                        // Joined on 2026-09-04 by the selection-window
                        // conjunct; before it, `Freeze` was the one action
                        // with a window and no clock.
                        | Action::Freeze
                ),
                "{action:?}",
            );
        }
    }

    /// EVERY ACTION'S PROFILE WRITES THE MARKET GENERATION, BECAUSE EVERY
    /// ACTION'S DOMAIN AUTHENTICATION READS IT.
    ///
    /// `authenticated_general_domain` calls
    /// `GeneralConfigV3::require_market(environment.generation, ..)` before an
    /// action touches its own state -- all fifteen evaluators call it, and
    /// `general_hot_environment_from_bank_v3` reads that generation out of
    /// `scalar::GENERATION`. Nothing else in the executing frame writes that
    /// register: the fifteen Lean-emitted RequestProfiles do not name it, and
    /// General's trusted environment is
    /// `CurrentSlot`/`CurrentExecutingProgram`/`SystemProgram`. So a profile
    /// that omits this operation makes its own action UNEXECUTABLE against any
    /// market whose generation is nonzero, and does it silently -- the register
    /// is a well-formed zero, and the refusal is `ConfigMarket`, which reads
    /// like a caller naming the wrong market.
    ///
    /// TEN OF FIFTEEN OMITTED IT. Five wrote it at a literal index --
    /// `OpenBatch`, `CloseBatch`, `PlaceOrder`, `CancelOrder`, `ReleaseOrder`
    /// -- and the other ten had no such operation. It stayed silent for the
    /// ordinary reason: the only harness that ran those actions,
    /// `programs/dclutch-general-accelerator-sbf/program-test/tests/lifecycle.rs`,
    /// HAND-WRITES its input bank and writes the generation itself, so it
    /// exercised the reader and never the artifact that has to feed it.
    /// Measured 2026-09-04 by the first `CloseBatch` fed from this profile
    /// through the real Trading ELF: `env.gen=0 root.gen=9`.
    ///
    /// The repair is one derived index rather than fifteen literals, so this
    /// test asserts a TOTAL property and no longer carries a debt list a
    /// sixteenth action could quietly join.
    #[test]
    fn every_action_projects_the_market_generation_its_domain_authentication_reads() {
        let expected = AccountOperationInputV2::ProjectDataU64 {
            account: AccountCoordinateV2::fixed(
                u16::try_from(HOT_RUNTIME_CONFIG_COORDINATE_V3).expect("config coordinate"),
            ),
            destination: common_scalar(scalar::GENERATION).expect("generation register"),
            data_offset: u32::try_from(GeneralConfigV3Layout::GENERATION).expect("offset"),
        };
        for action in ACTIONS {
            let count = general_account_profile_fixed_operation_count_v3(action);
            let writers = (0..count)
                .filter(|index| {
                    general_account_profile_operation_v3(action, *index).expect("exact operation")
                        == expected
                })
                .count();
            assert_eq!(
                writers, 1,
                "{action:?} declares {writers} operations sourcing the Market generation, and \
                 `authenticated_general_domain` requires exactly one",
            );
        }
    }

    #[test]
    fn every_action_rule_is_total_and_no_action_declares_a_span() {
        for action in ACTIONS {
            let count = general_account_profile_fixed_count_v3(action).expect("fixed count");
            let mut coordinate = 0_u16;
            while coordinate < count {
                general_account_profile_rule_v3(action, coordinate, WIDTHS).expect("exact rule");
                coordinate += 1;
            }
            assert_eq!(
                general_account_profile_rule_v3(action, count, WIDTHS),
                Err(GeneralAccountRuleErrorV3::Geometry)
            );
            // NO SPAN, asserted through the encoded artifact rather than
            // through a builder that no longer exists. `scalar::INPUT_SCRATCH_PAGE_COUNT`
            // survives as a reserved coordinate so the 151 common scalars do
            // not renumber; nothing writes it and nothing reads it.
            let bytes = general_account_profile_bytes_v3(action).expect("profile width");
            let mut encoded = vec![0_u8; bytes];
            let mut scratch = vec![0_u8; bytes];
            encode_general_account_profile_v3_atomic(action, WIDTHS, &mut scratch, &mut encoded)
                .expect("account profile");
            let profile = dclutch_account_profile_contract::v2::AccountProfileV2::decode(&encoded)
                .expect("decode");
            assert_eq!(profile.dynamic_fixed_span_count(), 0);
            assert_eq!(profile.item_account_stride(), 0);
        }
    }

    /// Guards that cannot hold. The list is EMPTY, and must stay empty.
    ///
    /// It held three entries when this census was written -- SubmitCandidate
    /// operation 31, PlaceOrder operation 26 and CancelOrder operation 27, each
    /// a `RequireKey` against `identity::OWNER`. All three are now
    /// `ProjectKey` into `identity::PAYER`, and the law they stated is carried
    /// by `identity_eq(PAYER, OWNER)` in each of those three emitted
    /// transitions, which run over the projected bank where both values exist.
    const UNSATISFIABLE_GUARDS_V3: [(u8, u16); 0] = [];

    /// Every account guard names a register the INPUT identity bank carries.
    ///
    /// `OP_REQUIRE_KEY` and `OP_REQUIRE_OWNER` compare an account fact against
    /// `input_identities` (`dclutch-account-profile-contract/src/v2.rs:3178`),
    /// while `OP_PROJECT_KEY`/`OP_PROJECT_OWNER`/`OP_PROJECT_DATA_IDENTITY`
    /// write a SEPARATE output bank. So a guard can never observe anything an
    /// operation in the same pass wrote, whatever the operation order is. Only
    /// three identity registers reach a General profile's input bank: register
    /// zero, pre-seeded with the parent request digest by the outer
    /// (`hot_v3.rs:12481`); `TRADING_PROGRAM`, from
    /// `TrustedIdentityEnvironmentV2::CurrentExecutingProgram`; and
    /// `RESULT_OWNER`, from `TrustedBuiltinIdentityV2::SystemProgram`, which
    /// `CloseCandidate` alone does not declare.
    ///
    /// All nineteen of General's remaining guards name one of those and hold.
    /// Three more used to name `identity::OWNER`, a register the same pass
    /// projects, and were therefore unsatisfiable and fail-closed:
    /// SubmitCandidate, PlaceOrder and CancelOrder could not execute at all.
    /// This is a convicted defect class in this tree -- guards whose two sides
    /// move together.
    ///
    /// The repair was not to delete them. The law they stated -- the payer must
    /// BE the maker or solver the record names -- is real, and General already
    /// re-proved exactly this shape on the other side: the emitted transition
    /// runs over the PROJECTED bank and carries
    /// `identity_eq(PRIMARY_BENEFICIARY, OWNER)` at
    /// `transition_artifacts_v3.rs:246`. So each guard became
    /// `ProjectKey` into `identity::PAYER` -- the construction CloseCandidate
    /// already used -- and each of those three transitions gained
    /// `identity_eq(PAYER, OWNER)`, authored in
    /// `formal/dclutch-semantics/DClutchSemantics/GeneralTransitionV3.lean` and
    /// regenerated into `generated_transition_programs_v3.rs`.
    ///
    /// Do not make this test pass by adding a row to `UNSATISFIABLE_GUARDS_V3`.
    #[test]
    fn every_account_guard_names_a_register_the_input_bank_carries() {
        let trading = IdentityCoordinateV2::common(
            u16::try_from(identity::TRADING_PROGRAM).expect("Trading register"),
        );
        let result_owner = IdentityCoordinateV2::common(
            u16::try_from(identity::RESULT_OWNER).expect("result-owner register"),
        );
        let parent_digest = IdentityCoordinateV2::common(
            u16::try_from(identity::PARENT_REQUEST_DIGEST).expect("parent-digest register"),
        );
        let mut guards = 0_usize;
        let mut unsatisfiable = 0_usize;
        for action in ACTIONS {
            let count = general_account_profile_operation_count_v3(action);
            let mut index = 0_u16;
            while index < count {
                let expected = match general_account_profile_operation_v3(action, index)
                    .expect("exact operation")
                {
                    AccountOperationInputV2::RequireKey { expected, .. }
                    | AccountOperationInputV2::RequireOwner { expected, .. } => expected,
                    _ => {
                        index += 1;
                        continue;
                    }
                };
                guards += 1;
                let carried = expected == trading
                    || expected == parent_digest
                    || (expected == result_owner && action != Action::CloseCandidate);
                if !carried {
                    unsatisfiable += 1;
                    assert!(
                        UNSATISFIABLE_GUARDS_V3.contains(&(action as u8, index)),
                        "{action:?} operation {index} guards against a register no General \
                         input bank carries, and it is not one of the three named ones",
                    );
                }
                index += 1;
            }
        }
        assert_eq!(
            guards, 33,
            "General declares thirty-three account guards: nineteen, plus the \
             System-program RequireKey on each of the fourteen actions that \
             declare a System identity -- CloseCandidate declares none and \
             therefore has no such guard, which is the conjunct this very test \
             refused when the account was first added to all fifteen",
        );
        assert_eq!(
            unsatisfiable,
            UNSATISFIABLE_GUARDS_V3.len(),
            "the named unsatisfiable guards are exactly the ones still present",
        );
    }

    /// Every register a General state recipe seeds on has an artifact writer.
    ///
    /// `state_seeds_v3` is the sole author of the eight General state seed
    /// orders, and every one of them opens with
    /// `CommonIdentity(GENERAL_ROOT_IDENTITY_REGISTER_V3)`. A register the
    /// recipe READS and no artifact WRITES is not a build error and not a
    /// geometry refusal: `AccountProfileV2::common_identity_count()` is 45 and
    /// 27 < 45, so `validate_seed_against_profile` accepts it, and the
    /// lifecycle adapter then derives the state address from 32 zero bytes
    /// where the root belongs. That derives a real, well-formed, WRONG address
    /// -- one that is the same for every General root in existence, so two
    /// roots would collide on one occurrence identity -- and the only thing
    /// that refuses it on chain is the undifferentiated address join, which
    /// looks exactly like a caller passing the wrong account.
    ///
    /// The operation list is the only place the join is checkable at all:
    /// `AccountProfileV2::operation` is private, so admission cannot read the
    /// operations back out of the encoded artifact. This test reads them from
    /// their one author instead.
    #[test]
    fn every_action_projects_the_root_key_into_the_register_its_state_recipes_seed_on() {
        let expected = AccountOperationInputV2::ProjectKey {
            account: AccountCoordinateV2::fixed(
                u16::try_from(HOT_RUNTIME_ROOT_COORDINATE_V3).expect("root coordinate"),
            ),
            destination: IdentityCoordinateV2::common(GENERAL_ROOT_IDENTITY_REGISTER_V3),
        };
        for action in ACTIONS {
            let count = general_account_profile_operation_count_v3(action);
            let mut writers = 0_usize;
            let mut index = 0_u16;
            while index < count {
                if general_account_profile_operation_v3(action, index).expect("exact operation")
                    == expected
                {
                    writers += 1;
                }
                index += 1;
            }
            assert_eq!(
                writers, 1,
                "{action:?} declares {writers} operations writing the root identity register, \
                 and every General state recipe seeds on it",
            );
            assert!(
                GeneralStateRecipeV3::primary_for_action(action)
                    .lifecycle_seeds()
                    .iter()
                    .any(|seed| matches!(
                        seed,
                        LifecycleSeedInputV3::CommonIdentity(GENERAL_ROOT_IDENTITY_REGISTER_V3)
                    )),
                "{action:?} primary state recipe does not seed on the root identity register",
            );
        }
    }

    #[test]
    fn submit_candidate_profile_owns_one_fixed_candidate_and_three_exact_evidence_records() {
        let action = Action::SubmitCandidate;
        // 11 until the System program became a runtime coordinate rather than
        // only a trusted identity; every action that declares one gained
        // exactly that account.
        assert_eq!(general_account_profile_fixed_count_v3(action), Ok(12));
        // 34 until the semantic-basis projection landed; every action's count
        // gained exactly one. 37 until the Market generation gained a derived
        // index and this action, which had none, gained the operation.
        assert_eq!(general_account_profile_operation_count_v3(action), 38);

        let candidate =
            general_account_profile_rule_v3(action, GENERAL_PRIMARY_STATE_ACCOUNT_V3, WIDTHS)
                .expect("candidate state rule");
        assert_eq!(candidate.prestate, AccountPrestateV2::LifecycleBound);
        assert_eq!(
            candidate.rule.data_length,
            u32::try_from(GENERAL_LOCAL_STATE_HEADER_BYTES_V3 + GENERAL_CANDIDATE_BYTES_V1)
                .expect("candidate width"),
        );
        assert_eq!(candidate.rule.data_item_stride, 0);

        let payer =
            general_account_profile_rule_v3(action, GENERAL_PRIMARY_PAYER_ACCOUNT_V3, WIDTHS)
                .expect("solver payer rule");
        assert_eq!(payer.prestate, AccountPrestateV2::Exact);
        assert_eq!(
            payer.rule.privileges,
            AccountPrivilegesV2::new(true, true, false),
        );
        assert_eq!(
            payer.rule.effect_permissions,
            AccountEffectPermissionsV2::new(true, false, false),
        );

        for (index, kind, base, stride) in [
            (
                0,
                GeneralReadonlyEvidenceKindV3::ClosedBatch,
                u32::try_from(GENERAL_LOCAL_STATE_HEADER_BYTES_V3 + GENERAL_BATCH_BYTES_V1)
                    .expect("closed batch width"),
                0,
            ),
            (
                1,
                GeneralReadonlyEvidenceKindV3::CandidateImage,
                u32::try_from(CANDIDATE_HEADER_BYTES_V2).expect("candidate image header"),
                8,
            ),
            (
                2,
                GeneralReadonlyEvidenceKindV3::SubmittedCandidate,
                u32::try_from(GENERAL_CANDIDATE_BYTES_V1).expect("submission width"),
                0,
            ),
        ] {
            let selected = general_readonly_evidence_v3(action, index).expect("evidence");
            assert_eq!(selected.kind, kind);
            assert_eq!(selected.coordinate, 8 + index);
            let rule = general_account_profile_rule_v3(action, selected.coordinate, WIDTHS)
                .expect("evidence rule");
            assert_eq!(rule.prestate, AccountPrestateV2::Exact);
            assert_eq!(rule.rule.data_length, base);
            assert_eq!(rule.rule.data_item_stride, stride);
            assert_eq!(
                rule.rule.privileges,
                AccountPrivilegesV2::new(false, false, false),
            );
            assert_eq!(rule.rule.effect_permissions, no_effects());
        }

        let bytes = general_account_profile_bytes_v3(action).expect("profile width");
        let mut scratch = vec![0_u8; bytes];
        let mut output = vec![0x55_u8; bytes];
        encode_general_account_profile_v3_atomic(action, WIDTHS, &mut scratch, &mut output)
            .expect("SubmitCandidate profile");
        let profile = dclutch_account_profile_contract::v2::AccountProfileV2::decode(&output)
            .expect("canonical profile");
        // THE FRAME IS THE FIXED FRAME, at every Product width. The twelve
        // fixed accounts used to be followed by a scratch-page span whose width
        // came from the return-data bound; the bank arrives inline now and the
        // span is gone, so the only admitted span vector is the empty one.
        assert_eq!(
            profile.logical_account_count_with_dynamic_spans(1, &[]),
            // 11 fixed accounts until the System program became one.
            Ok(usize::from(12_u16)),
        );
        assert!(
            profile
                .logical_account_count_with_dynamic_spans(1, &[3])
                .is_err(),
            "a width for a span this profile does not declare must refuse",
        );
    }

    /// Every child role the General Effect routes to has a program coordinate
    /// the Hot executor can resolve it through.
    ///
    /// `selected_role_program_v3` resolves a child route's callee by scanning
    /// the downgraded effect accounts for the key the Registry activation cache
    /// names for that role, and accepts only a UNIQUE readonly executable
    /// account. `CustodyFrameRoleV1` has no `CustodyProgram` variant, so none of
    /// the Custody frames can carry their own callee and this topology carried
    /// none anywhere: two Custody routes on `InitializeSettlement`, one each on
    /// `Collect`/`Materialize`/`Distribute`, three on `Close`, and no coordinate
    /// any of them could be invoked through. The Claims frames declare
    /// `ClaimsProgram` themselves, which is why that half always resolved.
    ///
    /// The roles come from the REAL emitted Effect bytes; the privileges come
    /// from `general_account_profile_rule_v3`, which is the sole rule source
    /// both profile encoders in this tree call.
    #[test]
    fn every_child_role_the_effect_routes_to_has_an_invocable_program_coordinate() {
        for action in [
            Action::Consider,
            Action::Freeze,
            Action::InitializeSettlement,
            Action::Collect,
            Action::Materialize,
            Action::Distribute,
            Action::Close,
        ] {
            let bytes = effect(action);
            let effect = dclutch_effect_kernel::v3::ProgramV3::decode(&bytes).expect("effect");
            let count = general_account_profile_fixed_count_v3(action).expect("count");
            assert_eq!(effect.fixed_account_count(), count);

            // A role's callee: the Custody one is the appended coordinate, and
            // the Claims one is found by asking the frames themselves which
            // coordinate carries `ClaimsProgram`.
            let claims_callee = (crate::state_artifacts_v3::general_child_account_start_v3(action)
                ..general_child_frame_end_v3(action).expect("child end"))
                .find(|coordinate| {
                    child_coordinate(action, *coordinate)
                        .and_then(|(frame, relative)| child_role(frame, relative))
                        .is_ok_and(|role| {
                            role == ChildRoleV3::Claims(ClaimsFrameRoleV1::ClaimsProgram)
                        })
                });
            let custody_callee =
                general_custody_callee_coordinate_v3(action).expect("callee coordinate");

            let mut route = 0_u16;
            while route < effect.route_count() {
                let role = effect.route(route).expect("route").role();
                let coordinate = match role {
                    FixedRole::Claims => claims_callee,
                    FixedRole::Custody => custody_callee,
                    // No General route declares any other role; a new one would
                    // arrive here with no callee and fail, which is the point.
                    _ => None,
                }
                .expect("every routed role must name a callee coordinate");
                let rule = general_account_profile_rule_v3(action, coordinate, WIDTHS)
                    .expect("callee rule");
                assert_eq!(
                    rule.rule.privileges,
                    AccountPrivilegesV2::new(false, false, true),
                    "{action:?} {role:?} callee at {coordinate} is not a readonly executable"
                );
                assert_eq!(
                    rule.rule.effect_permissions,
                    AccountEffectPermissionsV2::new(false, false, false),
                    "{action:?} {role:?} callee at {coordinate} carries an effect permission"
                );
                assert_eq!(
                    rule.rule.alias,
                    AccountAliasInputV2::SelfCoordinate,
                    "{action:?} {role:?} callee at {coordinate} is an alias"
                );
                route += 1;
            }

            // An alias onto a callee is the SECOND way to refuse the same
            // lookup: `downgraded_effect_accounts_v3` pushes one entry per
            // logical coordinate, aliases included, so an aliased callee matches
            // twice and the executor refuses on the second.
            for coordinate in 0..count {
                let rule = general_account_profile_rule_v3(action, coordinate, WIDTHS)
                    .expect("fixed rule");
                if let AccountAliasInputV2::Fixed(representative) = rule.rule.alias {
                    for callee in [claims_callee, custody_callee].into_iter().flatten() {
                        assert_ne!(
                            representative, callee,
                            "{action:?} callee at {callee} is aliased from {coordinate}"
                        );
                    }
                }
            }

            // The callee sits past every route range, so adding it renumbered
            // no frame, and it is absent exactly when no route needs it.
            //
            // It used to be the LAST account outright. The System program is
            // appended behind it now, so the invariant is stated against the
            // pre-System count -- which is what "past every route range" always
            // meant, and is true whether or not a second account follows.
            match custody_callee {
                Some(coordinate) => {
                    assert_eq!(
                        coordinate + 1,
                        crate::effect_artifacts_v3::general_effect_account_count_before_system_v3(
                            action
                        )
                    );
                    assert!(coordinate >= general_child_frame_end_v3(action).expect("child end"));
                    assert!(effect.route_count() > 0);
                }
                None => assert_eq!(effect.route_count(), 0),
            }
        }
    }

    #[test]
    fn child_frames_reuse_semantic_roles_through_authenticated_aliases() {
        for action in [Action::InitializeSettlement, Action::Close] {
            let child = crate::state_artifacts_v3::general_child_account_start_v3(action);
            // The trailing Custody callee belongs to no child frame, so the
            // child-frame walk stops before it.
            let count = general_child_frame_end_v3(action).expect("child frame end");
            let mut aliases = 0_usize;
            let mut repeated_signer = false;
            for coordinate in child..count {
                let rule = general_account_profile_rule_v3(action, coordinate, WIDTHS)
                    .expect("exact child rule");
                let (frame, relative) = child_coordinate(action, coordinate).expect("child");
                let role = child_role(frame, relative).expect("role");
                if let AccountAliasInputV2::Fixed(representative) = rule.rule.alias {
                    aliases += 1;
                    assert_eq!(
                        rule.rule.privileges,
                        AccountPrivilegesV2::new(false, false, false),
                        "aliases carry no second privilege truth"
                    );
                    if child_privilege_facts(frame, relative)
                        .expect("FrameSpec privilege")
                        .signer
                    {
                        repeated_signer = true;
                        assert_eq!(
                            general_account_profile_rule_v3(action, representative, WIDTHS)
                                .expect("prior physical representative")
                                .rule
                                .privileges,
                            physical_role_privileges(action, role).expect("physical union"),
                        );
                    }
                }
            }
            assert!(aliases > 0);
            assert!(
                repeated_signer,
                "fixture must exercise repeated signed child roles"
            );
        }
    }

    #[test]
    fn zero_external_width_refuses_before_any_rule_is_emitted() {
        let hostile = GeneralExternalAccountWidthsV3 {
            realm_record: 0,
            ..WIDTHS
        };
        assert_eq!(
            general_account_profile_rule_v3(Action::Freeze, 0, hostile),
            Err(GeneralAccountRuleErrorV3::ExternalWidth)
        );
    }

    /// Every Realm-selected collateral coordinate emits no width claim.
    ///
    /// A Token-2022 mint carrying extensions is not 82 bytes, an
    /// ImmutableOwner token account is not 165, and the width of a program
    /// record belongs to the loader that deployed it. This test names those
    /// coordinates through the Custody FrameSpec that owns them, so a rule
    /// that went back to asserting any of the three fails here rather than on
    /// a validator.
    #[test]
    fn realm_selected_collateral_coordinates_assert_no_width() {
        let mut observed = 0_usize;
        for action in ACTIONS {
            let count = general_account_profile_fixed_count_v3(action).expect("fixed count");
            let child_start = crate::state_artifacts_v3::general_child_account_start_v3(action);
            for coordinate in child_start..count {
                let (frame, relative) = match child_coordinate(action, coordinate) {
                    Ok(value) => value,
                    Err(_) => continue,
                };
                let GeneralChildFrameV3::Custody(operation) = frame else {
                    continue;
                };
                let data = CustodyFrameSpecV1::new(operation)
                    .data(relative)
                    .expect("custody frame data");
                if !matches!(
                    data,
                    CustodyFrameDataV1::TokenMint
                        | CustodyFrameDataV1::TokenAccount
                        | CustodyFrameDataV1::TokenProgram
                ) {
                    continue;
                }
                let rule = general_account_profile_rule_v3(action, coordinate, WIDTHS)
                    .expect("collateral rule");
                // A repeated semantic role is an authenticated route alias and
                // carries no second truth of any kind; every other appearance
                // is the physical coordinate and must be opaque.
                if rule.prestate != AccountPrestateV2::AuthenticatedRouteAlias {
                    observed += 1;
                    assert_eq!(
                        rule.prestate,
                        AccountPrestateV2::AuthenticatedOpaqueReadonlyData,
                        "{action:?} coordinate {coordinate} restates a width General does not own"
                    );
                }
                assert_eq!(rule.rule.data_length, 0);
                assert_eq!(rule.rule.data_item_stride, 0);
            }
        }
        assert!(
            observed >= 3,
            "fixture must reach every collateral data kind"
        );
    }

    /// The account Close destroys must be live, never possibly vacant.
    ///
    /// `LifecycleBound` admits vacant data. The Close plan closes the
    /// settlement cursor and the operator requires it live at exactly the
    /// declared width, so a `LifecycleBound` settlement coordinate is a
    /// weaker prestate than the transition it guards -- and, since a
    /// LifecycleBound coordinate must carry an AuthenticateOrCreate plan, it
    /// also refuses the policy/profile join outright.
    #[test]
    fn close_binds_only_the_terminal_record_it_creates() {
        let settlement = general_account_profile_rule_v3(
            Action::Close,
            GENERAL_PRIMARY_STATE_ACCOUNT_V3,
            WIDTHS,
        )
        .expect("Close settlement rule");
        assert_eq!(settlement.prestate, AccountPrestateV2::Exact);
        let terminal = general_account_profile_rule_v3(
            Action::Close,
            GENERAL_TERMINAL_STATE_ACCOUNT_V3,
            WIDTHS,
        )
        .expect("Close terminal rule");
        assert_eq!(terminal.prestate, AccountPrestateV2::LifecycleBound);
        assert_eq!(settlement.rule.data_length, terminal.rule.data_length);
        assert_eq!(
            settlement.rule.data_item_stride,
            terminal.rule.data_item_stride
        );
        for action in ACTIONS {
            if action == Action::Close {
                continue;
            }
            let expected = if matches!(action, Action::VerifyCandidateRow | Action::CloseCandidate)
            {
                // These two authenticate a pre-existing Candidate. Verify
                // carries an explicit RequireOwner before its lamport
                // projection; CloseCandidate carries the same anchor before
                // closing it.
                AccountPrestateV2::Exact
            } else {
                AccountPrestateV2::LifecycleBound
            };
            assert_eq!(
                general_account_profile_rule_v3(action, GENERAL_PRIMARY_STATE_ACCOUNT_V3, WIDTHS)
                    .expect("primary state rule")
                    .prestate,
                expected,
                "{action:?} primary-state prestate"
            );
        }
    }

    #[test]
    fn candidate_lamport_effects_are_minimal_and_the_debit_owner_is_anchored() {
        assert_eq!(
            general_account_profile_operation_v3(Action::VerifyCandidateRow, 10),
            Ok(AccountOperationInputV2::RequireOwner {
                account: AccountCoordinateV2::fixed(GENERAL_PRIMARY_STATE_ACCOUNT_V3),
                expected: IdentityCoordinateV2::common(
                    u16::try_from(identity::TRADING_PROGRAM).expect("Trading identity"),
                ),
            })
        );
        let verify_payer = general_account_profile_rule_v3(
            Action::VerifyCandidateRow,
            GENERAL_VERIFY_PAYER_ACCOUNT_V3,
            WIDTHS,
        )
        .expect("Verify payer");
        assert_eq!(
            verify_payer.rule.effect_permissions,
            AccountEffectPermissionsV2::new(true, true, false),
            "the same signer is credited by Effect and may fund either vacant Verify state"
        );
        for coordinate in [
            GENERAL_PRIMARY_PAYER_ACCOUNT_V3,
            GENERAL_PRIMARY_RENT_CREDIT_ACCOUNT_V3,
        ] {
            let beneficiary =
                general_account_profile_rule_v3(Action::CloseCandidate, coordinate, WIDTHS)
                    .expect("CloseCandidate beneficiary");
            assert_eq!(
                beneficiary.rule.effect_permissions,
                AccountEffectPermissionsV2::new(false, true, false),
                "CloseCandidate beneficiary {coordinate} may only receive"
            );
        }
    }

    /// The published profile encoder generates every action, and the width it
    /// advertises is the width it writes.
    ///
    /// Before this function existed the encoder invocation had two authors and
    /// only one of them -- the release builder -- ever ran for every authored
    /// action; the contract-side copy was pinned to `Freeze`. Now there is one
    /// author and this exercises it across the whole action set, including
    /// `Close`, which is the only action with a nine-operation list.
    #[test]
    fn the_published_profile_encoder_generates_every_action() {
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
        for action in ACTIONS {
            let bytes = general_account_profile_bytes_v3(action).expect("profile width");
            let mut scratch = vec![0_u8; bytes];
            let mut output = vec![0x55_u8; bytes];
            encode_general_account_profile_v3_atomic(action, WIDTHS, &mut scratch, &mut output)
                .unwrap_or_else(|error| panic!("{action:?} profile artifact: {error:?}"));
            let profile = dclutch_account_profile_contract::v2::AccountProfileV2::decode(&output)
                .expect("the encoder emits a decodable profile");
            assert_eq!(
                profile.artifact_profile(),
                DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE
            );
            assert_eq!(profile.dynamic_fixed_span_count(), 0);
            assert_eq!(profile.bytes().len(), bytes);
            // One rule per fixed coordinate and nothing after it: the
            // scratch-page template the span expanded is gone with the span.
            assert_eq!(
                profile
                    .logical_account_count_with_dynamic_spans(1, &[])
                    .expect("logical count"),
                usize::from(general_account_profile_fixed_count_v3(action).expect("fixed count"))
            );
        }

        // Buffers that are not the exact advertised width refuse, and `output`
        // keeps its fill.
        let bytes = general_account_profile_bytes_v3(Action::Freeze).expect("profile width");
        let mut scratch = vec![0_u8; bytes];
        let mut short = vec![0x55_u8; bytes - 1];
        assert_eq!(
            encode_general_account_profile_v3_atomic(
                Action::Freeze,
                WIDTHS,
                &mut scratch,
                &mut short,
            ),
            Err(GeneralAccountRuleErrorV3::Geometry)
        );
        assert!(short.iter().all(|byte| *byte == 0x55));
    }
}
