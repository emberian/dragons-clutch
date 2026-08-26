//! Action-selected General local-state lifecycle artifacts.
//!
//! These artifacts derive every nonroot state address from authenticated
//! General registers and let the generic Trading lifecycle adapter own the
//! canonical bump, current-Rent principal, immutable RentCredit beneficiary,
//! state key, and Trading owner.  Family evaluation never supplies those
//! protected values.

use dclutch_account_profile_contract::lifecycle_v3::{
    ACTION_PLAN_BYTES, CURRENT_RENT_QUOTE_BYTES_V5, HEADER_BYTES, IMMUTABLE_IDENTITY_BINDING_BYTES,
    PROTECTED_OUTPUT_BYTES, RECIPE_BYTES, SEED_BYTES, StateLifecyclePolicyV4,
    StateLifecyclePolicyV5,
    encode::{
        LifecycleAccountCoordinateV3, LifecycleCurrentRentQuoteInputV5, LifecycleGuardInputV3,
        LifecycleImmutableIdentityBindingInputV4, LifecycleOperationInputV3, LifecyclePlanInputV3,
        LifecycleProtectedOutputsInputV3, LifecycleRecipeInputV3, LifecycleRegisterCoordinateV3,
        LifecycleSeedInputV3, encode_lifecycle_policy_v4_atomic, encode_lifecycle_policy_v5_atomic,
    },
};
use dclutch_claims_svm::{
    liability_basis_state_v2::LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
    protocol_position_v2::PROTOCOL_POSITION_ADMISSION_BYTES_V2,
};
use dclutch_custody_contract::CUSTODY_REPLAY_BYTES_V1;
use dclutch_general_codec::Action;

use crate::{
    hot_candidate_v3::{identity, scalar},
    local_state_v3::{GENERAL_LOCAL_STATE_HEADER_BYTES_V3, GeneralLocalStateLayoutV3},
    runtime_selection::{RUNTIME_SELECTION_CURSOR_BYTES_V2, RuntimeSelectionLayoutV2},
    runtime_width::{SETTLEMENT_CURSOR_HEADER_BYTES_V2, SettlementCursorLayoutV2},
};

/// First action-selected nonroot General state account.
pub const GENERAL_PRIMARY_STATE_ACCOUNT_V3: u16 = 5;
/// Close-only terminal record following the settlement state.
pub const GENERAL_TERMINAL_STATE_ACCOUNT_V3: u16 = 6;
/// Payer coordinate for every non-Close lifecycle.
pub const GENERAL_PRIMARY_PAYER_ACCOUNT_V3: u16 = 6;
/// RentCredit coordinate for every non-Close lifecycle.
pub const GENERAL_PRIMARY_RENT_CREDIT_ACCOUNT_V3: u16 = 7;
/// Close-only terminal-record payer coordinate.
pub const GENERAL_CLOSE_PAYER_ACCOUNT_V3: u16 = 7;
/// Shared Close terminal-create and settlement-close RentCredit coordinate.
pub const GENERAL_CLOSE_RENT_CREDIT_ACCOUNT_V3: u16 = 8;

/// Semantic owner of one readonly General evaluator input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralReadonlyEvidenceKindV3 {
    /// Immutable interpreted best-valid-submitted-candidate policy.
    SelectionPolicy,
    /// Newly submitted complete verified-candidate record.
    SubmittedVerifiedCandidate,
    /// Frozen selection cursor naming the sole winning verified candidate.
    FrozenSelection,
    /// Terminal runtime verifier cursor used to initialize settlement.
    RuntimeVerifier,
    /// Selected complete verified-candidate record.
    SelectedVerifiedCandidate,
    /// Exact settlement order manifest emitted by candidate verification.
    SettlementManifest,
}

/// One action-selected readonly evidence account and its logical coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralReadonlyEvidenceV3 {
    /// AccountProfile coordinate in the expanded logical runtime frame.
    pub coordinate: u16,
    /// Exact semantic record consumed by the stateless evaluator.
    pub kind: GeneralReadonlyEvidenceKindV3,
}

const GENERAL_STATE_SEED_DOMAIN_V3: &[u8] = b"dclutch-general-state-v3";
const SELECTION_STATE_SEED_V3: &[u8] = b"selection";
const SETTLEMENT_STATE_SEED_V3: &[u8] = b"settlement";
const TERMINAL_STATE_SEED_V3: &[u8] = b"terminal";

/// Stable refusal from General lifecycle-artifact generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralStateArtifactErrorV3 {
    /// A checked physical width overflowed.
    Geometry,
    /// The generic lifecycle semantic owner refused the candidate.
    Lifecycle(dclutch_account_profile_contract::lifecycle_v3::Error),
}

/// Product/release-authenticated child widths needed by Initialize rent quotes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralChildRentWidthsV5 {
    /// Exact LiabilityBasis Position bytes, including the Product-N balance tail.
    pub position: u32,
    /// Exact selected Token or Token-2022 vault-account bytes.
    pub custody_vault: u32,
}

impl GeneralChildRentWidthsV5 {
    /// Derive the exact Position width from Product N and join the selected vault width.
    pub fn new(outcome_count: u32, custody_vault: u32) -> Result<Self> {
        let position = u32::try_from(LIABILITY_BASIS_POSITION_HEADER_BYTES_V2)
            .map_err(|_| GeneralStateArtifactErrorV3::Geometry)?
            .checked_add(
                outcome_count
                    .checked_mul(8)
                    .ok_or(GeneralStateArtifactErrorV3::Geometry)?,
            )
            .ok_or(GeneralStateArtifactErrorV3::Geometry)?;
        if outcome_count == 0 || custody_vault == 0 {
            return Err(GeneralStateArtifactErrorV3::Geometry);
        }
        Ok(Self {
            position,
            custody_vault,
        })
    }
}

/// Result alias for General state artifacts.
pub type Result<T> = core::result::Result<T, GeneralStateArtifactErrorV3>;

/// Exact lifecycle artifact width for one General action.
pub fn general_state_lifecycle_bytes_v3(action: Action) -> Result<usize> {
    general_state_lifecycle_bytes(action, false)
}

/// Exact Lifecycle V5 artifact width for one successor General action.
pub fn general_state_lifecycle_bytes_v5(action: Action) -> Result<usize> {
    general_state_lifecycle_bytes(action, true)
}

fn general_state_lifecycle_bytes(action: Action, current_rent_v5: bool) -> Result<usize> {
    let (recipes, seeds, plans) = lifecycle_counts(action);
    recipes
        .checked_mul(RECIPE_BYTES)
        .and_then(|value| value.checked_add(seeds.checked_mul(SEED_BYTES)?))
        .and_then(|value| value.checked_add(plans.checked_mul(ACTION_PLAN_BYTES)?))
        .and_then(|value| value.checked_add(plans.checked_mul(PROTECTED_OUTPUT_BYTES)?))
        .and_then(|value| {
            value.checked_add(
                lifecycle_binding_count(action).checked_mul(IMMUTABLE_IDENTITY_BINDING_BYTES)?,
            )
        })
        .and_then(|value| {
            value.checked_add(
                if current_rent_v5 {
                    lifecycle_current_rent_quote_count(action)
                } else {
                    0
                }
                .checked_mul(CURRENT_RENT_QUOTE_BYTES_V5)?,
            )
        })
        .and_then(|value| HEADER_BYTES.checked_add(value))
        .ok_or(GeneralStateArtifactErrorV3::Geometry)
}

/// Number of exact readonly evaluator inputs selected by one action.
#[must_use]
pub const fn general_readonly_evidence_count_v3(action: Action) -> u16 {
    match action {
        Action::Consider => 2,
        Action::Freeze => 0,
        Action::InitializeSettlement => 3,
        Action::Collect | Action::Distribute => 2,
        Action::Materialize | Action::Close => 1,
    }
}

/// Return one exact readonly evaluator input.
pub fn general_readonly_evidence_v3(
    action: Action,
    index: u16,
) -> Result<GeneralReadonlyEvidenceV3> {
    let base = general_readonly_evidence_start_v3(action);
    let kind = match (action, index) {
        (Action::Consider, 0) => GeneralReadonlyEvidenceKindV3::SelectionPolicy,
        (Action::Consider, 1) => GeneralReadonlyEvidenceKindV3::SubmittedVerifiedCandidate,
        (Action::InitializeSettlement, 0) => GeneralReadonlyEvidenceKindV3::FrozenSelection,
        (Action::InitializeSettlement, 1) => GeneralReadonlyEvidenceKindV3::RuntimeVerifier,
        (Action::InitializeSettlement, 2)
        | (Action::Collect | Action::Materialize | Action::Distribute | Action::Close, 0) => {
            GeneralReadonlyEvidenceKindV3::SelectedVerifiedCandidate
        }
        (Action::Collect | Action::Distribute, 1) => {
            GeneralReadonlyEvidenceKindV3::SettlementManifest
        }
        _ => return Err(GeneralStateArtifactErrorV3::Geometry),
    };
    Ok(GeneralReadonlyEvidenceV3 {
        coordinate: base
            .checked_add(index)
            .ok_or(GeneralStateArtifactErrorV3::Geometry)?,
        kind,
    })
}

/// First readonly evidence coordinate after local lifecycle accounts.
#[must_use]
pub const fn general_readonly_evidence_start_v3(action: Action) -> u16 {
    if matches!(action, Action::Close) {
        9
    } else {
        8
    }
}

/// First child-route account after action-selected readonly evidence.
pub const fn general_child_account_start_v3(action: Action) -> u16 {
    general_readonly_evidence_start_v3(action) + general_readonly_evidence_count_v3(action)
}

/// Generate one complete protected-output lifecycle policy atomically.
pub fn encode_general_state_lifecycle_v3_atomic(
    action: Action,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<()> {
    let expected = general_state_lifecycle_bytes_v3(action)?;
    if scratch.len() != expected || output.len() != expected {
        return Err(GeneralStateArtifactErrorV3::Geometry);
    }
    if action == Action::Close {
        encode_close(action, None, scratch, output)
    } else {
        encode_primary(action, None, scratch, output)
    }
}

/// Generate one complete General Lifecycle V5 policy with current-Rent quotes.
///
/// Only Initialize declares child-creation quotes. Other actions still select
/// the V5 schema with an empty quote table, preventing a V4 fallback in the
/// successor artifact chain.
pub fn encode_general_state_lifecycle_v5_atomic(
    action: Action,
    child_widths: Option<GeneralChildRentWidthsV5>,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<()> {
    let expected = general_state_lifecycle_bytes_v5(action)?;
    if scratch.len() != expected || output.len() != expected {
        return Err(GeneralStateArtifactErrorV3::Geometry);
    }
    let quotes = general_current_rent_quotes_v5(action, child_widths)?;
    let selected = quotes.as_slice(action);
    if action == Action::Close {
        encode_close(action, Some(selected), scratch, output)
    } else {
        encode_primary(action, Some(selected), scratch, output)
    }
}

fn encode_primary(
    action: Action,
    current_rent_quotes: Option<&[LifecycleCurrentRentQuoteInputV5]>,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<()> {
    let selection = matches!(action, Action::Consider | Action::Freeze);
    let data_base = if selection {
        u32::try_from(
            GENERAL_LOCAL_STATE_HEADER_BYTES_V3
                .checked_add(RUNTIME_SELECTION_CURSOR_BYTES_V2)
                .ok_or(GeneralStateArtifactErrorV3::Geometry)?,
        )
        .map_err(|_| GeneralStateArtifactErrorV3::Geometry)?
    } else {
        u32::try_from(
            GENERAL_LOCAL_STATE_HEADER_BYTES_V3
                .checked_add(SETTLEMENT_CURSOR_HEADER_BYTES_V2)
                .ok_or(GeneralStateArtifactErrorV3::Geometry)?,
        )
        .map_err(|_| GeneralStateArtifactErrorV3::Geometry)?
    };
    let recipe = [LifecycleRecipeInputV3 {
        state: LifecycleAccountCoordinateV3::fixed(GENERAL_PRIMARY_STATE_ACCOUNT_V3),
        seed_start: 0,
        seed_count: if selection { 4 } else { 5 },
        bump_offset: if selection { 3 } else { 4 },
        data_base,
        data_stride: if selection {
            0
        } else {
            SettlementCursorLayoutV2::inventory_stride()
        },
    }];
    let selection_seeds = [
        LifecycleSeedInputV3::Literal(GENERAL_STATE_SEED_DOMAIN_V3),
        LifecycleSeedInputV3::CommonIdentity(identity_u16(identity::GENERAL_ROOT)?),
        LifecycleSeedInputV3::Literal(SELECTION_STATE_SEED_V3),
        LifecycleSeedInputV3::CanonicalBump,
    ];
    let settlement_seeds = [
        LifecycleSeedInputV3::Literal(GENERAL_STATE_SEED_DOMAIN_V3),
        LifecycleSeedInputV3::CommonIdentity(identity_u16(identity::GENERAL_ROOT)?),
        LifecycleSeedInputV3::CommonIdentity(identity_u16(identity::CANDIDATE)?),
        LifecycleSeedInputV3::Literal(SETTLEMENT_STATE_SEED_V3),
        LifecycleSeedInputV3::CanonicalBump,
    ];
    let plan = [LifecyclePlanInputV3 {
        action: action as u32,
        operation: LifecycleOperationInputV3::AuthenticateOrCreate,
        recipe: 0,
        payer: Some(LifecycleAccountCoordinateV3::fixed(
            GENERAL_PRIMARY_PAYER_ACCOUNT_V3,
        )),
        rent_credit: Some(LifecycleAccountCoordinateV3::fixed(
            GENERAL_PRIMARY_RENT_CREDIT_ACCOUNT_V3,
        )),
        principal: Some(LifecycleRegisterCoordinateV3::common(scalar_u16(
            scalar::PRIMARY_PRINCIPAL_OBSERVATION,
        )?)),
        beneficiary: Some(LifecycleRegisterCoordinateV3::common(identity_u16(
            identity::PRIMARY_BENEFICIARY_OBSERVATION,
        )?)),
        guard: LifecycleGuardInputV3::Always,
    }];
    let protected = [Some(primary_protected()?)];
    let bindings = selection_or_settlement_bindings(action)?;
    let seeds = if selection {
        &selection_seeds[..]
    } else {
        &settlement_seeds[..]
    };
    if let Some(quotes) = current_rent_quotes {
        encode_lifecycle_policy_v5_atomic(
            &recipe,
            seeds,
            &plan,
            &protected,
            bindings.as_slice(),
            quotes,
            scratch,
            output,
        )
        .map_err(GeneralStateArtifactErrorV3::Lifecycle)?;
        StateLifecyclePolicyV5::decode_selected([1; 32], [1; 32], output)
            .map_err(GeneralStateArtifactErrorV3::Lifecycle)?;
    } else {
        encode_lifecycle_policy_v4_atomic(
            &recipe,
            seeds,
            &plan,
            &protected,
            bindings.as_slice(),
            scratch,
            output,
        )
        .map_err(GeneralStateArtifactErrorV3::Lifecycle)?;
        StateLifecyclePolicyV4::decode_selected([1; 32], [1; 32], output)
            .map_err(GeneralStateArtifactErrorV3::Lifecycle)?;
    }
    Ok(())
}

fn encode_close(
    action: Action,
    current_rent_quotes: Option<&[LifecycleCurrentRentQuoteInputV5]>,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<()> {
    let settlement_base = u32::try_from(
        GENERAL_LOCAL_STATE_HEADER_BYTES_V3
            .checked_add(SETTLEMENT_CURSOR_HEADER_BYTES_V2)
            .ok_or(GeneralStateArtifactErrorV3::Geometry)?,
    )
    .map_err(|_| GeneralStateArtifactErrorV3::Geometry)?;
    let recipes = [
        LifecycleRecipeInputV3 {
            state: LifecycleAccountCoordinateV3::fixed(GENERAL_PRIMARY_STATE_ACCOUNT_V3),
            seed_start: 0,
            seed_count: 5,
            bump_offset: 4,
            data_base: settlement_base,
            data_stride: SettlementCursorLayoutV2::inventory_stride(),
        },
        LifecycleRecipeInputV3 {
            state: LifecycleAccountCoordinateV3::fixed(GENERAL_TERMINAL_STATE_ACCOUNT_V3),
            seed_start: 5,
            seed_count: 6,
            bump_offset: 5,
            data_base: settlement_base,
            data_stride: SettlementCursorLayoutV2::inventory_stride(),
        },
    ];
    let seeds = [
        LifecycleSeedInputV3::Literal(GENERAL_STATE_SEED_DOMAIN_V3),
        LifecycleSeedInputV3::CommonIdentity(identity_u16(identity::GENERAL_ROOT)?),
        LifecycleSeedInputV3::CommonIdentity(identity_u16(identity::CANDIDATE)?),
        LifecycleSeedInputV3::Literal(SETTLEMENT_STATE_SEED_V3),
        LifecycleSeedInputV3::CanonicalBump,
        LifecycleSeedInputV3::Literal(GENERAL_STATE_SEED_DOMAIN_V3),
        LifecycleSeedInputV3::CommonIdentity(identity_u16(identity::GENERAL_ROOT)?),
        LifecycleSeedInputV3::CommonIdentity(identity_u16(identity::CANDIDATE)?),
        LifecycleSeedInputV3::CommonScalar {
            index: scalar_u16(scalar::CURSOR_TERMINAL_COORDINATE)?,
            width: 8,
        },
        LifecycleSeedInputV3::Literal(TERMINAL_STATE_SEED_V3),
        LifecycleSeedInputV3::CanonicalBump,
    ];
    // Same action entries remain canonical by operation tag: Close precedes
    // AuthenticateOrCreate. Creation is nevertheless applied before Effects
    // by the generic Trading lifecycle adapter.
    let plans = [
        LifecyclePlanInputV3 {
            action: action as u32,
            operation: LifecycleOperationInputV3::Close,
            recipe: 0,
            payer: None,
            rent_credit: Some(LifecycleAccountCoordinateV3::fixed(
                GENERAL_CLOSE_RENT_CREDIT_ACCOUNT_V3,
            )),
            principal: Some(LifecycleRegisterCoordinateV3::common(scalar_u16(
                scalar::PRIMARY_PRINCIPAL_OBSERVATION,
            )?)),
            beneficiary: Some(LifecycleRegisterCoordinateV3::common(identity_u16(
                identity::PRIMARY_BENEFICIARY_OBSERVATION,
            )?)),
            guard: LifecycleGuardInputV3::Always,
        },
        LifecyclePlanInputV3 {
            action: action as u32,
            operation: LifecycleOperationInputV3::AuthenticateOrCreate,
            recipe: 1,
            payer: Some(LifecycleAccountCoordinateV3::fixed(
                GENERAL_CLOSE_PAYER_ACCOUNT_V3,
            )),
            rent_credit: Some(LifecycleAccountCoordinateV3::fixed(
                GENERAL_CLOSE_RENT_CREDIT_ACCOUNT_V3,
            )),
            principal: Some(LifecycleRegisterCoordinateV3::common(scalar_u16(
                scalar::TERMINAL_PRINCIPAL_OBSERVATION,
            )?)),
            beneficiary: Some(LifecycleRegisterCoordinateV3::common(identity_u16(
                identity::TERMINAL_BENEFICIARY_OBSERVATION,
            )?)),
            guard: LifecycleGuardInputV3::Always,
        },
    ];
    let protected = [None, Some(terminal_protected()?)];
    let bindings = close_bindings()?;
    if let Some(quotes) = current_rent_quotes {
        encode_lifecycle_policy_v5_atomic(
            &recipes, &seeds, &plans, &protected, &bindings, quotes, scratch, output,
        )
        .map_err(GeneralStateArtifactErrorV3::Lifecycle)?;
        StateLifecyclePolicyV5::decode_selected([1; 32], [1; 32], output)
            .map_err(GeneralStateArtifactErrorV3::Lifecycle)?;
    } else {
        encode_lifecycle_policy_v4_atomic(
            &recipes, &seeds, &plans, &protected, &bindings, scratch, output,
        )
        .map_err(GeneralStateArtifactErrorV3::Lifecycle)?;
        StateLifecyclePolicyV4::decode_selected([1; 32], [1; 32], output)
            .map_err(GeneralStateArtifactErrorV3::Lifecycle)?;
    }
    Ok(())
}

struct CurrentRentQuoteBufferV5 {
    values: [LifecycleCurrentRentQuoteInputV5; 4],
}

impl CurrentRentQuoteBufferV5 {
    fn as_slice(&self, action: Action) -> &[LifecycleCurrentRentQuoteInputV5] {
        if action == Action::InitializeSettlement {
            &self.values
        } else {
            &[]
        }
    }
}

fn general_current_rent_quotes_v5(
    action: Action,
    child_widths: Option<GeneralChildRentWidthsV5>,
) -> Result<CurrentRentQuoteBufferV5> {
    let child_widths = match (action, child_widths) {
        (Action::InitializeSettlement, Some(widths))
            if widths.position != 0 && widths.custody_vault != 0 =>
        {
            widths
        }
        (Action::InitializeSettlement, _) | (_, Some(_)) => {
            return Err(GeneralStateArtifactErrorV3::Geometry);
        }
        (_, None) => GeneralChildRentWidthsV5 {
            // These inactive values are never serialized because every other
            // action has an empty V5 quote table.
            position: 0,
            custody_vault: 0,
        },
    };
    Ok(CurrentRentQuoteBufferV5 {
        values: [
            LifecycleCurrentRentQuoteInputV5 {
                exact_data_len: child_widths.position,
                scalar_destination: scalar_u16(scalar::POSITION_RENT_PRINCIPAL)?,
            },
            LifecycleCurrentRentQuoteInputV5 {
                exact_data_len: u32::try_from(PROTOCOL_POSITION_ADMISSION_BYTES_V2)
                    .map_err(|_| GeneralStateArtifactErrorV3::Geometry)?,
                scalar_destination: scalar_u16(scalar::ADMISSION_RENT_PRINCIPAL)?,
            },
            LifecycleCurrentRentQuoteInputV5 {
                exact_data_len: u32::try_from(CUSTODY_REPLAY_BYTES_V1)
                    .map_err(|_| GeneralStateArtifactErrorV3::Geometry)?,
                scalar_destination: scalar_u16(scalar::CUSTODY_REPLAY_RENT_LAMPORTS)?,
            },
            LifecycleCurrentRentQuoteInputV5 {
                exact_data_len: child_widths.custody_vault,
                scalar_destination: scalar_u16(scalar::CUSTODY_VAULT_RENT_LAMPORTS)?,
            },
        ],
    })
}

const fn lifecycle_current_rent_quote_count(action: Action) -> usize {
    match action {
        Action::InitializeSettlement => 4,
        _ => 0,
    }
}

fn primary_protected() -> Result<LifecycleProtectedOutputsInputV3> {
    Ok(LifecycleProtectedOutputsInputV3 {
        created: scalar_u16(scalar::PRIMARY_CREATED)?,
        bump_observation: scalar_u16(scalar::PRIMARY_BUMP_OBSERVATION)?,
        bump: scalar_u16(scalar::PRIMARY_CANONICAL_BUMP)?,
        historical_rent_principal: scalar_u16(scalar::PRIMARY_RENT_PRINCIPAL)?,
        beneficiary: identity_u16(identity::PRIMARY_BENEFICIARY)?,
        state: identity_u16(identity::PRIMARY_STATE)?,
        owner: identity_u16(identity::PRIMARY_OWNER)?,
    })
}

fn terminal_protected() -> Result<LifecycleProtectedOutputsInputV3> {
    Ok(LifecycleProtectedOutputsInputV3 {
        created: scalar_u16(scalar::TERMINAL_CREATED)?,
        bump_observation: scalar_u16(scalar::TERMINAL_BUMP_OBSERVATION)?,
        bump: scalar_u16(scalar::TERMINAL_CANONICAL_BUMP)?,
        historical_rent_principal: scalar_u16(scalar::TERMINAL_RENT_PRINCIPAL)?,
        beneficiary: identity_u16(identity::TERMINAL_BENEFICIARY)?,
        state: identity_u16(identity::TERMINAL_STATE)?,
        owner: identity_u16(identity::TERMINAL_OWNER)?,
    })
}

struct BindingBufferV4 {
    values: [LifecycleImmutableIdentityBindingInputV4; 5],
    len: usize,
}

impl BindingBufferV4 {
    fn as_slice(&self) -> &[LifecycleImmutableIdentityBindingInputV4] {
        self.values.get(..self.len).unwrap_or(&[])
    }
}

fn selection_or_settlement_bindings(action: Action) -> Result<BindingBufferV4> {
    let empty = LifecycleImmutableIdentityBindingInputV4 {
        plan: 0,
        data_offset: 0,
        canonical: LifecycleRegisterCoordinateV3::common(0),
    };
    let mut output = BindingBufferV4 {
        values: [empty; 5],
        len: lifecycle_binding_count(action),
    };
    if matches!(action, Action::Consider | Action::Freeze) {
        for (slot, (body_offset, canonical)) in [
            (
                RuntimeSelectionLayoutV2::product_id(),
                identity::SELECTION_PRODUCT,
            ),
            (
                RuntimeSelectionLayoutV2::batch_id(),
                identity::SELECTION_BATCH,
            ),
            (
                RuntimeSelectionLayoutV2::policy_id(),
                identity::SELECTION_POLICY,
            ),
            (
                RuntimeSelectionLayoutV2::best_candidate_id(),
                identity::CANDIDATE,
            ),
            (
                RuntimeSelectionLayoutV2::best_verified_digest(),
                identity::BEST_VERIFIED_DIGEST,
            ),
        ]
        .into_iter()
        .enumerate()
        {
            output.values[slot] = binding(0, body_offset, canonical)?;
        }
    } else {
        output.values[0] = binding(
            0,
            SettlementCursorLayoutV2::candidate_id(),
            identity::CANDIDATE,
        )?;
    }
    Ok(output)
}

fn close_bindings() -> Result<[LifecycleImmutableIdentityBindingInputV4; 1]> {
    Ok([binding(
        1,
        SettlementCursorLayoutV2::candidate_id(),
        identity::CANDIDATE,
    )?])
}

fn binding(
    plan: u16,
    body_offset: u32,
    canonical: u32,
) -> Result<LifecycleImmutableIdentityBindingInputV4> {
    Ok(LifecycleImmutableIdentityBindingInputV4 {
        plan,
        data_offset: GeneralLocalStateLayoutV3::body()
            .checked_add(body_offset)
            .ok_or(GeneralStateArtifactErrorV3::Geometry)?,
        canonical: LifecycleRegisterCoordinateV3::common(identity_u16(canonical)?),
    })
}

const fn lifecycle_counts(action: Action) -> (usize, usize, usize) {
    match action {
        Action::Consider | Action::Freeze => (1, 4, 1),
        Action::InitializeSettlement
        | Action::Collect
        | Action::Materialize
        | Action::Distribute => (1, 5, 1),
        Action::Close => (2, 11, 2),
    }
}

const fn lifecycle_binding_count(action: Action) -> usize {
    match action {
        Action::Consider | Action::Freeze => 5,
        Action::InitializeSettlement
        | Action::Collect
        | Action::Materialize
        | Action::Distribute => 1,
        Action::Close => 1,
    }
}

fn scalar_u16(value: u32) -> Result<u16> {
    u16::try_from(value).map_err(|_| GeneralStateArtifactErrorV3::Geometry)
}

fn identity_u16(value: u32) -> Result<u16> {
    u16::try_from(value).map_err(|_| GeneralStateArtifactErrorV3::Geometry)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::vec;

    use super::*;

    const ACTIONS: [Action; 7] = [
        Action::Consider,
        Action::Freeze,
        Action::InitializeSettlement,
        Action::Collect,
        Action::Materialize,
        Action::Distribute,
        Action::Close,
    ];

    #[test]
    fn all_actions_encode_exact_canonical_lifecycle_geometry() {
        for action in ACTIONS {
            let width = general_state_lifecycle_bytes_v3(action).expect("width");
            let mut scratch = vec![0_u8; width];
            let mut output = vec![0x55_u8; width];
            encode_general_state_lifecycle_v3_atomic(action, &mut scratch, &mut output)
                .expect("lifecycle artifact");
            let policy =
                StateLifecyclePolicyV4::decode_selected([1; 32], [1; 32], &output).expect("decode");
            assert_eq!(
                policy.action_plan_count(action as u32),
                Ok(if action == Action::Close { 2 } else { 1 })
            );
            assert_eq!(
                policy
                    .action_plan(action as u32, if action == Action::Close { 1 } else { 0 })
                    .expect("selected")
                    .uses_canonical_bump(),
                Ok(true)
            );
        }
    }

    #[test]
    fn v5_binds_exact_initialize_child_rent_widths_at_n_one_and_258() {
        for outcome_count in [1_u32, 258] {
            let widths = GeneralChildRentWidthsV5::new(outcome_count, 165)
                .expect("authenticated child widths");
            for action in ACTIONS {
                let width = general_state_lifecycle_bytes_v5(action).expect("V5 width");
                let mut scratch = vec![0_u8; width];
                let mut output = vec![0x55_u8; width];
                encode_general_state_lifecycle_v5_atomic(
                    action,
                    (action == Action::InitializeSettlement).then_some(widths),
                    &mut scratch,
                    &mut output,
                )
                .expect("V5 lifecycle artifact");
                let policy = StateLifecyclePolicyV5::decode_selected([1; 32], [1; 32], &output)
                    .expect("V5 decode");
                let expected_count = if action == Action::InitializeSettlement {
                    4
                } else {
                    0
                };
                assert_eq!(policy.current_rent_quote_count(), expected_count);
                if action == Action::InitializeSettlement {
                    for (ordinal, (data_len, destination)) in [
                        (widths.position, scalar::POSITION_RENT_PRINCIPAL),
                        (
                            u32::try_from(PROTOCOL_POSITION_ADMISSION_BYTES_V2)
                                .expect("admission width"),
                            scalar::ADMISSION_RENT_PRINCIPAL,
                        ),
                        (
                            u32::try_from(CUSTODY_REPLAY_BYTES_V1).expect("replay width"),
                            scalar::CUSTODY_REPLAY_RENT_LAMPORTS,
                        ),
                        (widths.custody_vault, scalar::CUSTODY_VAULT_RENT_LAMPORTS),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        let quote = policy
                            .current_rent_quote(u16::try_from(ordinal).expect("ordinal"))
                            .expect("quote");
                        assert_eq!(quote.exact_data_len(), data_len);
                        assert_eq!(
                            quote.scalar_destination().index(),
                            u16::try_from(destination).expect("bounded destination")
                        );
                    }
                }
            }
        }
        assert_eq!(
            GeneralChildRentWidthsV5::new(0, 165),
            Err(GeneralStateArtifactErrorV3::Geometry)
        );
        assert_eq!(
            GeneralChildRentWidthsV5::new(1, 0),
            Err(GeneralStateArtifactErrorV3::Geometry)
        );
    }

    #[test]
    fn v5_refuses_absent_initialize_or_extraneous_noninitialize_widths() {
        for (action, widths) in [
            (Action::InitializeSettlement, None),
            (
                Action::Freeze,
                Some(GeneralChildRentWidthsV5::new(1, 165).expect("widths")),
            ),
        ] {
            let width = general_state_lifecycle_bytes_v5(action).expect("width");
            let mut scratch = vec![0_u8; width];
            let mut output = vec![0x55_u8; width];
            let before = output.clone();
            assert_eq!(
                encode_general_state_lifecycle_v5_atomic(
                    action,
                    widths,
                    &mut scratch,
                    &mut output,
                ),
                Err(GeneralStateArtifactErrorV3::Geometry)
            );
            assert_eq!(output, before);
        }
    }

    #[test]
    fn all_actions_select_exact_readonly_evidence_before_child_routes() {
        let expected = [
            (Action::Consider, 8, 2, 10),
            (Action::Freeze, 8, 0, 8),
            (Action::InitializeSettlement, 8, 3, 11),
            (Action::Collect, 8, 2, 10),
            (Action::Materialize, 8, 1, 9),
            (Action::Distribute, 8, 2, 10),
            (Action::Close, 9, 1, 10),
        ];
        for (action, start, count, child) in expected {
            assert_eq!(general_readonly_evidence_start_v3(action), start);
            assert_eq!(general_readonly_evidence_count_v3(action), count);
            assert_eq!(general_child_account_start_v3(action), child);
            for index in 0..count {
                assert_eq!(
                    general_readonly_evidence_v3(action, index)
                        .expect("selected evidence")
                        .coordinate,
                    start + index
                );
            }
            assert_eq!(
                general_readonly_evidence_v3(action, count),
                Err(GeneralStateArtifactErrorV3::Geometry)
            );
        }
    }

    #[test]
    fn nonexact_capacity_preserves_output() {
        let action = Action::Close;
        let width = general_state_lifecycle_bytes_v3(action).expect("width");
        let mut scratch = vec![0_u8; width];
        let mut output = vec![0x55_u8; width - 1];
        let before = output.clone();
        assert_eq!(
            encode_general_state_lifecycle_v3_atomic(action, &mut scratch, &mut output),
            Err(GeneralStateArtifactErrorV3::Geometry)
        );
        assert_eq!(output, before);
    }
}
