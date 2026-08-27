//! Exact fractional-redemption successor over canonical full-width accounts.
//!
//! Executable actions 2 through 9 mutate only the sole owners of affected
//! facts: owner credit/tombstone, Position V3 and GEN1
//! Replay for claimant state, ClaimLedger V3 for native supply, Hoard V2 for
//! locked-principal/cash classification, and `0xa5/v1` for the global
//! fractional sequence and aggregate numerator credit. The immutable
//! `0xa4/v2` policy commits the exact PDA-bound Resolution V5 data identity.

use crate::accounts::{
    expect_pda, require, require_count, require_distinct, require_signer, Outcome,
};
use crate::claim_release::authenticate_claim_issuance_release_with_programdata_v1;
use crate::error::{ClutchError, Refusal};
use crate::{capabilities, seeds, token};
use clutch_collateral_adapter_v2::{
    accept_fractional_bearer_claim_burn_v3, prepare_claim_redemption_collateral_v2,
    prepare_fractional_bearer_claim_burn_v3, prepare_zero_claim_redemption_collateral_v2,
    AcceptedBearerRedemptionCollateralV3, ClaimLedgerV3, Id as CollateralId,
    TransferAuthorityKindV2, TransferAuthorityV2, CLAIM_LEDGER_V3_BYTES,
};
use clutch_fractional_redemption_runtime::{
    accept_bearer_credit_burn_v1, accept_bearer_exact_burn_v1, bind_fractional_context_v1,
    bind_fractional_internal_context_v1, finish_bearer_credit_v1, finish_bearer_exact_v1,
    finish_external_credit_transfer_v1, merge_credit_v1, prepare_bearer_credit_v1,
    prepare_bearer_exact_v1, prepare_external_credit_merge_v1,
    prepare_external_credit_transfer_v1,
    project_fractional_family_terminal_receipt_v1, redeem_internal_exact_v1,
    redeem_internal_to_credit_v1, seal_claims_exhausted_v1, transfer_credit_v1,
    verify_fractional_family_admission_postwrite_v1,
    verify_fractional_family_terminal_postwrite_v1, BearerClaimPrestateV1,
    close_zero_credit_v1, CreditCreationV1, CreditPayoutPoststateV1, CreditPayoutTargetV1,
    CreditPrestateV1, EmptyLedgerClosePlanV1, Error as FractionalError,
    FractionalCreditTombstoneV2, FractionalCreditV2, FractionalFamilyAdmissionReceiptV1,
    FractionalFamilyTerminalReceiptV1, FractionalInitializationPlanV1, FractionalLedgerV1,
    FractionalPolicyV2, FractionalRedeemIntentV1, FractionalRedemptionActionV1,
    FractionalTerminalIntentV1, FractionalTransferIntentV1, FractionalCloseCreditIntentV1,
    InternalPositionV1, RedemptionSourcePoststateV1,
    VerifiedFractionalFamilyAdmissionPostwriteV1, VerifiedFractionalFamilyTerminalPostwriteV1,
    FRACTIONAL_CREDIT_ACCOUNT_BYTES, FRACTIONAL_CREDIT_TOMBSTONE_BYTES,
    FRACTIONAL_LEDGER_ACCOUNT_BYTES, FRACTIONAL_POLICY_ACCOUNT_BYTES,
    FRACTIONAL_REDEMPTION_FAMILY_TAG, FRACTIONAL_REDEMPTION_FAMILY_VERSION,
};
use clutch_product_series::ContentId;
use clutch_retirement::{
    admit_initial_rent_split, admit_reopen_rent_split, Identity32V1, RentSplitAdmissionPlanV2,
    POSITION_V3_BYTES,
};
use solana_account_info::AccountInfo;
use solana_cpi::{invoke, invoke_signed};
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use super::collateral_position_v3::{
    authenticate_general_market_liabilities_v2, authenticate_general_market_value_authority_v2,
    authenticate_general_position_replay_v2, authenticate_resolution_v5,
};
use super::external_redemption_v3::{
    accept_zero_claim_collateral_payout, bearer_claim_observation_v3,
    invoke_claim_collateral_payout, observe_outcome_mints_for_bearer_v3, runtime_account_view,
};
use super::product_artifact::AuthenticatedRegistryCapabilityV3;
use super::product_market::authenticate_market_lifecycle_root_v1;
use super::genesis::{
    allocate_data, assign_data, read_rent, require_creatable, require_system_program,
    transfer_data, SYSTEM_PROGRAM_ID,
};

/// Exact account count for action 2.
pub const REDEEM_INTERNAL_EXACT_ACCOUNT_COUNT_V1: usize = 15;
/// Fixed exact-bearer prefix before one canonical mint per active outcome.
pub const REDEEM_BEARER_EXACT_PREFIX_ACCOUNTS_V1: usize = 21;
/// Live-credit action-4 width, including its authenticated Rent sysvar.
pub const REDEEM_INTERNAL_CREDIT_LIVE_ACCOUNT_COUNT_V1: usize = 19;
/// Extra payer/System roles required only for fresh creation or reopen.
pub const CREDIT_CREATION_SUFFIX_ACCOUNTS_V1: usize = 2;
/// Bearer-credit roles following the active outcome-mint suffix.
pub const REDEEM_BEARER_CREDIT_POST_MINT_ACCOUNTS_V1: usize = 4;
/// Exact fixed account count for the supply-exhaustion seal.
pub const SEAL_CLAIMS_EXHAUSTED_ACCOUNT_COUNT_V1: usize = 12;

const FRACTIONAL_RUNTIME_RELEASE_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/sbf/fractional/runtime-release-authentication/v1\0";
const FRACTIONAL_ADMISSION_POSTWRITE_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/sbf/fractional/admission-postwrite-authentication/v1\0";
const FRACTIONAL_TERMINAL_POSTWRITE_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/sbf/fractional/terminal-postwrite-authentication/v1\0";
const FRACTIONAL_COLLATERAL_EXECUTION_RECEIPT_DOMAIN_V2: &[u8] =
    b"dragons-clutch/sbf/fractional/collateral-execution-receipt/v2\0";

/// Private same-instruction join between the canonical Fractional transition
/// and the exact current collateral/claim loader releases that executed it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AuthenticatedFractionalCollateralExecutionV2 {
    transition_id: CollateralId,
    receipt_id: CollateralId,
}

fn bind_fractional_collateral_execution_v2(
    transition_id: CollateralId,
    collateral_release_receipt_id: CollateralId,
    claim_release_receipt_id: CollateralId,
    claim_burn_receipt_id: CollateralId,
    collateral_delta_receipt_id: CollateralId,
) -> Outcome<AuthenticatedFractionalCollateralExecutionV2> {
    transition_id
        .require_live()
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    collateral_release_receipt_id
        .require_live()
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    collateral_delta_receipt_id
        .require_live()
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    require(
        claim_release_receipt_id.is_zero() == claim_burn_receipt_id.is_zero(),
        ClutchError::AuthorizationUnavailable,
    )?;
    if !claim_release_receipt_id.is_zero() {
        claim_release_receipt_id
            .require_live()
            .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
        claim_burn_receipt_id
            .require_live()
            .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    }
    let receipt_id = CollateralId::from_bytes(
        solana_sha256_hasher::hashv(&[
            FRACTIONAL_COLLATERAL_EXECUTION_RECEIPT_DOMAIN_V2,
            &transition_id.bytes(),
            &collateral_release_receipt_id.bytes(),
            &claim_release_receipt_id.bytes(),
            &claim_burn_receipt_id.bytes(),
            &collateral_delta_receipt_id.bytes(),
        ])
        .to_bytes(),
    );
    receipt_id
        .require_live()
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    Ok(AuthenticatedFractionalCollateralExecutionV2 {
        transition_id,
        receipt_id,
    })
}

fn collateral_delta_receipt_id(
    accepted: AcceptedBearerRedemptionCollateralV3,
) -> CollateralId {
    match accepted {
        AcceptedBearerRedemptionCollateralV3::Zero(value) => value.receipt_id(),
        AcceptedBearerRedemptionCollateralV3::Nonzero(value) => value.receipt_id(),
    }
}

const IX_ACTOR: usize = 0;
const IX_REALM: usize = 1;
const IX_PROFILE: usize = 2;
const IX_COLLATERAL_POLICY: usize = 3;
const IX_COLLATERAL_TOKEN_PROGRAM: usize = 4;
const IX_MARKET_BINDING: usize = 5;
const IX_MARKET_RUNTIME: usize = 6;
const IX_MARKET_INSTANCE: usize = 7;
const IX_HOARD: usize = 8;
const IX_CLAIM_LEDGER: usize = 9;
const IX_RESOLUTION: usize = 10;
const IX_FRACTIONAL_POLICY: usize = 11;
const IX_FRACTIONAL_LEDGER: usize = 12;
const IX_POSITION: usize = 13;
const IX_REPLAY: usize = 14;

const IX_CREDIT: usize = 15;
const IX_MARKET_LIFECYCLE_ROOT: usize = 16;
const IX_NEUTRAL_SINK: usize = 17;
const IX_CREDIT_RENT: usize = 18;

mod bearer_ix {
    pub const CLAIMANT: usize = 0;
    pub const REALM: usize = 1;
    pub const PROFILE: usize = 2;
    pub const COLLATERAL_POLICY: usize = 3;
    pub const COLLATERAL_TOKEN_PROGRAM: usize = 4;
    pub const MARKET_BINDING: usize = 5;
    pub const MARKET_RUNTIME: usize = 6;
    pub const MARKET_INSTANCE: usize = 7;
    pub const HOARD: usize = 8;
    pub const CLAIM_LEDGER: usize = 9;
    pub const RESOLUTION: usize = 10;
    pub const FRACTIONAL_POLICY: usize = 11;
    pub const FRACTIONAL_LEDGER: usize = 12;
    pub const COLLATERAL_MINT: usize = 13;
    pub const DESTINATION: usize = 14;
    pub const HOARD_AUTHORITY: usize = 15;
    pub const HOARD_TOKEN: usize = 16;
    pub const OUTCOME_TOKEN_PROGRAM: usize = 17;
    pub const OUTCOME_TOKEN_PROGRAMDATA: usize = 18;
    pub const SOURCE: usize = 19;
    pub const COLLATERAL_TOKEN_PROGRAMDATA: usize = 20;
    pub const OUTCOME_MINTS: usize = super::REDEEM_BEARER_EXACT_PREFIX_ACCOUNTS_V1;
}

mod seal_ix {
    pub const REALM: usize = 0;
    pub const PROFILE: usize = 1;
    pub const COLLATERAL_POLICY: usize = 2;
    pub const COLLATERAL_TOKEN_PROGRAM: usize = 3;
    pub const MARKET_BINDING: usize = 4;
    pub const MARKET_RUNTIME: usize = 5;
    pub const MARKET_INSTANCE: usize = 6;
    pub const HOARD: usize = 7;
    pub const CLAIM_LEDGER: usize = 8;
    pub const RESOLUTION: usize = 9;
    pub const FRACTIONAL_POLICY: usize = 10;
    pub const FRACTIONAL_LEDGER: usize = 11;
}

mod move_ix {
    pub const SOURCE_CLAIMANT: usize = 0;
    pub const DESTINATION_CLAIMANT: usize = 1;
    pub const REALM: usize = 2;
    pub const PROFILE: usize = 3;
    pub const COLLATERAL_POLICY: usize = 4;
    pub const COLLATERAL_TOKEN_PROGRAM: usize = 5;
    pub const MARKET_BINDING: usize = 6;
    pub const MARKET_RUNTIME: usize = 7;
    pub const MARKET_INSTANCE: usize = 8;
    pub const HOARD: usize = 9;
    pub const CLAIM_LEDGER: usize = 10;
    pub const RESOLUTION: usize = 11;
    pub const FRACTIONAL_POLICY: usize = 12;
    pub const FRACTIONAL_LEDGER: usize = 13;
    pub const SOURCE_CREDIT: usize = 14;
    pub const DESTINATION_CREDIT: usize = 15;
    pub const PAYOUT: usize = 16;
}

mod close_credit_ix {
    pub const CLAIMANT: usize = 0;
    pub const REALM: usize = 1;
    pub const PROFILE: usize = 2;
    pub const COLLATERAL_POLICY: usize = 3;
    pub const COLLATERAL_TOKEN_PROGRAM: usize = 4;
    pub const MARKET_BINDING: usize = 5;
    pub const MARKET_RUNTIME: usize = 6;
    pub const MARKET_INSTANCE: usize = 7;
    pub const HOARD: usize = 8;
    pub const CLAIM_LEDGER: usize = 9;
    pub const RESOLUTION: usize = 10;
    pub const FRACTIONAL_POLICY: usize = 11;
    pub const FRACTIONAL_LEDGER: usize = 12;
    pub const CREDIT: usize = 13;
    pub const PAYER: usize = 14;
    pub const MARKET_ROOT: usize = 15;
    pub const NEUTRAL: usize = 16;
    pub const RENT: usize = 17;
    pub const COUNT: usize = 18;
}

fn map_fractional(error: FractionalError) -> Refusal {
    match error {
        FractionalError::ReplayMismatch | FractionalError::ReplayRefused => {
            Refusal::Adapter(ClutchError::Replay)
        }
        FractionalError::Arithmetic => Refusal::Adapter(ClutchError::Arithmetic),
        FractionalError::Truncated
        | FractionalError::TrailingBytes
        | FractionalError::WrongTag
        | FractionalError::WrongVersion
        | FractionalError::NonCanonicalPadding => Refusal::Adapter(ClutchError::NonCanonical),
        _ => Refusal::Adapter(ClutchError::MismatchedState),
    }
}

fn require_program_state(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    writable: bool,
    exact_len: usize,
) -> Outcome<()> {
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    require(!account.is_signer, ClutchError::MismatchedState)?;
    require(
        account.is_writable == writable,
        if writable {
            ClutchError::NotWritable
        } else {
            ClutchError::UnexpectedWritable
        },
    )?;
    require(
        account.data_len() == exact_len,
        ClutchError::WrongDataLength,
    )
}

/// Private proof that the current loader-authenticated registry release admits
/// one exact Fractional action in the compiled deployment profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedFractionalRuntimeReleaseV1 {
    release_id: Identity32V1,
    capability_profile_id: ContentId,
    action: FractionalRedemptionActionV1,
    authentication_id: Identity32V1,
}

impl AuthenticatedFractionalRuntimeReleaseV1 {
    pub(crate) const fn release_id(self) -> Identity32V1 {
        self.release_id
    }

    pub(crate) const fn capability_profile_id(self) -> ContentId {
        self.capability_profile_id
    }

    pub(crate) const fn action(self) -> FractionalRedemptionActionV1 {
        self.action
    }

    pub(crate) const fn authentication_id(self) -> Identity32V1 {
        self.authentication_id
    }
}

/// Narrow a Series-bound loader/artifact capability into one Fractional action.
///
/// This function continues to refuse all Fractional actions while the central
/// profile leaves their exact tuples disabled. Merely possessing allocated
/// wire coordinates is never accepted as a runtime release.
pub(crate) fn authenticate_fractional_runtime_release_v1(
    program_id: &Pubkey,
    capability: AuthenticatedRegistryCapabilityV3,
    action: FractionalRedemptionActionV1,
) -> Outcome<AuthenticatedFractionalRuntimeReleaseV1> {
    require(
        capability.program_account() == *program_id
            && capabilities::extension_intent_action_enabled(
                FRACTIONAL_REDEMPTION_FAMILY_TAG,
                FRACTIONAL_REDEMPTION_FAMILY_VERSION,
                action.tag(),
            ),
        ClutchError::UnsupportedInstruction,
    )?;
    let release_id = Identity32V1::new(capability.registry_release_id().bytes())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let capability_profile_id = capability.capability_profile_id();
    let programdata_account = capability.programdata_account();
    let release_artifact_account = capability.release_artifact_account();
    let profile_artifact_account = capability.profile_artifact_account();
    let authentication_id = Identity32V1::new(
        solana_sha256_hasher::hashv(&[
            FRACTIONAL_RUNTIME_RELEASE_AUTHENTICATION_DOMAIN_V1,
            program_id.as_ref(),
            programdata_account.as_ref(),
            release_artifact_account.as_ref(),
            profile_artifact_account.as_ref(),
            &release_id.bytes(),
            &capability_profile_id.bytes(),
            &[action.tag()],
        ])
        .to_bytes(),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok(AuthenticatedFractionalRuntimeReleaseV1 {
        release_id,
        capability_profile_id,
        action,
        authentication_id,
    })
}

/// Adapter-authenticated exact Fractional founding postwrite.
///
/// Fields stay private so Product can consume only a value minted from the
/// actual writable a4/a5/ClaimLedger accounts in the current instruction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedFractionalFamilyAdmissionPostwriteV1 {
    verified: VerifiedFractionalFamilyAdmissionPostwriteV1,
    runtime_release: AuthenticatedFractionalRuntimeReleaseV1,
    authentication_id: Identity32V1,
}

impl AuthenticatedFractionalFamilyAdmissionPostwriteV1 {
    pub(crate) const fn family_admission(self) -> FractionalFamilyAdmissionReceiptV1 {
        self.verified.family_admission()
    }

    pub(crate) const fn verification_id(self) -> Identity32V1 {
        self.verified.verification_id()
    }

    pub(crate) const fn runtime_release(self) -> AuthenticatedFractionalRuntimeReleaseV1 {
        self.runtime_release
    }

    pub(crate) const fn authentication_id(self) -> Identity32V1 {
        self.authentication_id
    }
}

/// Authenticate exact founding postimages after Fractional has allocated and
/// written Product-prefunded System-owned a4/a5 prestates.
///
/// Product remains the sole owner of Foundation debit/preallocation evidence.
/// This helper neither debits nor refunds those accounts and admits only the
/// exact plan-derived bodies under canonical Fractional/ClaimLedger PDAs.
pub(crate) fn authenticate_fractional_family_admission_postwrite_v1(
    program_id: &Pubkey,
    runtime_release: AuthenticatedFractionalRuntimeReleaseV1,
    plan: FractionalInitializationPlanV1,
    policy_account: &AccountInfo<'_>,
    ledger_account: &AccountInfo<'_>,
    claim_ledger_account: &AccountInfo<'_>,
) -> Outcome<AuthenticatedFractionalFamilyAdmissionPostwriteV1> {
    require(
        runtime_release.action == FractionalRedemptionActionV1::Initialize
            && policy_account.key != ledger_account.key
            && policy_account.key != claim_ledger_account.key
            && ledger_account.key != claim_ledger_account.key,
        ClutchError::MismatchedState,
    )?;
    require_program_state(
        program_id,
        policy_account,
        true,
        FRACTIONAL_POLICY_ACCOUNT_BYTES,
    )?;
    require_program_state(
        program_id,
        ledger_account,
        true,
        FRACTIONAL_LEDGER_ACCOUNT_BYTES,
    )?;
    require_program_state(
        program_id,
        claim_ledger_account,
        true,
        CLAIM_LEDGER_V3_BYTES,
    )?;
    let policy_data = policy_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let ledger_data = ledger_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let claim_ledger_data = claim_ledger_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let policy = FractionalPolicyV2::decode(&policy_data).map_err(map_fractional)?;
    let ledger = FractionalLedgerV1::decode(&ledger_data).map_err(map_fractional)?;
    let claim_ledger = ClaimLedgerV3::decode(&claim_ledger_data)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let receipt = plan.family_admission;
    require(
        policy_account.key.to_bytes() == receipt.policy_account().bytes()
            && ledger_account.key.to_bytes() == receipt.ledger_account().bytes()
            && claim_ledger_account.key.to_bytes() == receipt.claim_ledger_account().bytes(),
        ClutchError::MismatchedState,
    )?;
    let policy_seeds = policy.pda_seeds();
    expect_pda(
        policy_account.key,
        seeds::fractional_policy_v2_pda(
            program_id,
            &policy_seeds.market_instance().bytes(),
            &policy_seeds.resolution_account().bytes(),
            &policy_seeds.resolution_data_id().bytes(),
        ),
        Some(policy_seeds.stored_bump()),
    )?;
    let ledger_seeds = ledger.pda_seeds();
    expect_pda(
        ledger_account.key,
        seeds::fractional_ledger_v1_pda(program_id, &ledger_seeds.policy_account().bytes()),
        Some(ledger_seeds.stored_bump()),
    )?;
    expect_pda(
        claim_ledger_account.key,
        seeds::claim_ledger_v3_pda(program_id, &claim_ledger.market_instance_id.bytes()),
        Some(claim_ledger.stored_bump),
    )?;
    let verified = verify_fractional_family_admission_postwrite_v1(
        plan,
        Identity32V1::new(policy_account.key.to_bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        policy,
        Identity32V1::new(ledger_account.key.to_bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        ledger,
        Identity32V1::new(claim_ledger_account.key.to_bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        claim_ledger,
    )
    .map_err(map_fractional)?;
    let policy_data_id = solana_sha256_hasher::hashv(&[&policy_data[..]]).to_bytes();
    let ledger_data_id = solana_sha256_hasher::hashv(&[&ledger_data[..]]).to_bytes();
    let claim_ledger_data_id =
        solana_sha256_hasher::hashv(&[&claim_ledger_data[..]]).to_bytes();
    let authentication_id = Identity32V1::new(
        solana_sha256_hasher::hashv(&[
            FRACTIONAL_ADMISSION_POSTWRITE_AUTHENTICATION_DOMAIN_V1,
            program_id.as_ref(),
            &runtime_release.authentication_id.bytes(),
            &verified.verification_id().bytes(),
            policy_account.key.as_ref(),
            &policy_data_id,
            ledger_account.key.as_ref(),
            &ledger_data_id,
            claim_ledger_account.key.as_ref(),
            &claim_ledger_data_id,
        ])
        .to_bytes(),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok(AuthenticatedFractionalFamilyAdmissionPostwriteV1 {
        verified,
        runtime_release,
        authentication_id,
    })
}

/// Adapter-authenticated terminal postwrite before a4/a5 deletion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedFractionalFamilyTerminalPostwriteV1 {
    verified: VerifiedFractionalFamilyTerminalPostwriteV1,
    runtime_release: AuthenticatedFractionalRuntimeReleaseV1,
    authentication_id: Identity32V1,
}

impl AuthenticatedFractionalFamilyTerminalPostwriteV1 {
    pub(crate) const fn family_terminal(self) -> FractionalFamilyTerminalReceiptV1 {
        self.verified.family_terminal()
    }

    pub(crate) const fn verification_id(self) -> Identity32V1 {
        self.verified.verification_id()
    }

    pub(crate) const fn resolution_account(self) -> Identity32V1 {
        self.verified.terminal_requirement().resolution_account()
    }

    pub(crate) const fn resolution_semantic_id(self) -> Identity32V1 {
        self.verified
            .terminal_requirement()
            .resolution_semantic_id()
    }

    pub(crate) const fn resolution_data_id(self) -> Identity32V1 {
        self.verified.terminal_requirement().resolution_data_id()
    }

    pub(crate) const fn native_claim_basis_id(self) -> Identity32V1 {
        self.verified
            .terminal_requirement()
            .native_claim_basis_id()
    }

    pub(crate) const fn runtime_release(self) -> AuthenticatedFractionalRuntimeReleaseV1 {
        self.runtime_release
    }

    pub(crate) const fn authentication_id(self) -> Identity32V1 {
        self.authentication_id
    }
}

/// Authenticate the exact terminal ClaimLedger postwrite and both live
/// pre-deletion a4/a5 bodies under a separately authenticated action-10 release.
#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticate_fractional_family_terminal_postwrite_v1(
    program_id: &Pubkey,
    runtime_release: AuthenticatedFractionalRuntimeReleaseV1,
    close: EmptyLedgerClosePlanV1,
    policy_account: &AccountInfo<'_>,
    ledger_account: &AccountInfo<'_>,
    claim_ledger_account: &AccountInfo<'_>,
) -> Outcome<AuthenticatedFractionalFamilyTerminalPostwriteV1> {
    require(
        runtime_release.action == FractionalRedemptionActionV1::CloseEmptyLedger
            && policy_account.key != ledger_account.key
            && policy_account.key != claim_ledger_account.key
            && ledger_account.key != claim_ledger_account.key,
        ClutchError::MismatchedState,
    )?;
    require_program_state(
        program_id,
        policy_account,
        true,
        FRACTIONAL_POLICY_ACCOUNT_BYTES,
    )?;
    require_program_state(
        program_id,
        ledger_account,
        true,
        FRACTIONAL_LEDGER_ACCOUNT_BYTES,
    )?;
    require_program_state(
        program_id,
        claim_ledger_account,
        true,
        CLAIM_LEDGER_V3_BYTES,
    )?;
    let policy_data = policy_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let ledger_data = ledger_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let claim_ledger_data = claim_ledger_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let policy = FractionalPolicyV2::decode(&policy_data).map_err(map_fractional)?;
    let ledger = FractionalLedgerV1::decode(&ledger_data).map_err(map_fractional)?;
    let claim_ledger = ClaimLedgerV3::decode(&claim_ledger_data)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let terminal = project_fractional_family_terminal_receipt_v1(close, runtime_release.release_id)
        .map_err(map_fractional)?;
    require(
        policy_account.key.to_bytes() == terminal.policy_account().bytes()
            && ledger_account.key.to_bytes() == terminal.ledger_account().bytes()
            && claim_ledger_account.key.to_bytes() == terminal.claim_ledger_account().bytes(),
        ClutchError::MismatchedState,
    )?;
    let policy_seeds = policy.pda_seeds();
    expect_pda(
        policy_account.key,
        seeds::fractional_policy_v2_pda(
            program_id,
            &policy_seeds.market_instance().bytes(),
            &policy_seeds.resolution_account().bytes(),
            &policy_seeds.resolution_data_id().bytes(),
        ),
        Some(policy_seeds.stored_bump()),
    )?;
    let ledger_seeds = ledger.pda_seeds();
    expect_pda(
        ledger_account.key,
        seeds::fractional_ledger_v1_pda(program_id, &ledger_seeds.policy_account().bytes()),
        Some(ledger_seeds.stored_bump()),
    )?;
    expect_pda(
        claim_ledger_account.key,
        seeds::claim_ledger_v3_pda(program_id, &claim_ledger.market_instance_id.bytes()),
        Some(claim_ledger.stored_bump),
    )?;
    let verified = verify_fractional_family_terminal_postwrite_v1(
        close,
        terminal,
        Identity32V1::new(policy_account.key.to_bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        policy,
        Identity32V1::new(ledger_account.key.to_bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        ledger,
        Identity32V1::new(claim_ledger_account.key.to_bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        claim_ledger,
        policy_account.lamports(),
        ledger_account.lamports(),
    )
    .map_err(map_fractional)?;
    let policy_data_id = solana_sha256_hasher::hashv(&[&policy_data[..]]).to_bytes();
    let ledger_data_id = solana_sha256_hasher::hashv(&[&ledger_data[..]]).to_bytes();
    let claim_ledger_data_id =
        solana_sha256_hasher::hashv(&[&claim_ledger_data[..]]).to_bytes();
    let authentication_id = Identity32V1::new(
        solana_sha256_hasher::hashv(&[
            FRACTIONAL_TERMINAL_POSTWRITE_AUTHENTICATION_DOMAIN_V1,
            program_id.as_ref(),
            &runtime_release.authentication_id.bytes(),
            &verified.verification_id().bytes(),
            policy_account.key.as_ref(),
            &policy_data_id,
            ledger_account.key.as_ref(),
            &ledger_data_id,
            claim_ledger_account.key.as_ref(),
            &claim_ledger_data_id,
            &policy_account.lamports().to_le_bytes(),
            &ledger_account.lamports().to_le_bytes(),
        ])
        .to_bytes(),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok(AuthenticatedFractionalFamilyTerminalPostwriteV1 {
        verified,
        runtime_release,
        authentication_id,
    })
}

fn decode_fractional_accounts(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    policy_index: usize,
    ledger_index: usize,
    resolution_index: usize,
) -> Outcome<(FractionalPolicyV2, FractionalLedgerV1)> {
    require_program_state(
        program_id,
        &accounts[policy_index],
        false,
        FRACTIONAL_POLICY_ACCOUNT_BYTES,
    )?;
    require_program_state(
        program_id,
        &accounts[ledger_index],
        true,
        FRACTIONAL_LEDGER_ACCOUNT_BYTES,
    )?;
    let policy_data = accounts[policy_index]
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let policy = FractionalPolicyV2::decode(&policy_data).map_err(map_fractional)?;
    drop(policy_data);
    let ledger_data = accounts[ledger_index]
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let ledger = FractionalLedgerV1::decode(&ledger_data).map_err(map_fractional)?;
    drop(ledger_data);
    let policy_seeds = policy.pda_seeds();
    expect_pda(
        accounts[policy_index].key,
        seeds::fractional_policy_v2_pda(
            program_id,
            &policy_seeds.market_instance().bytes(),
            &policy_seeds.resolution_account().bytes(),
            &policy_seeds.resolution_data_id().bytes(),
        ),
        Some(policy_seeds.stored_bump()),
    )?;
    let ledger_seeds = ledger.pda_seeds();
    expect_pda(
        accounts[ledger_index].key,
        seeds::fractional_ledger_v1_pda(program_id, &ledger_seeds.policy_account().bytes()),
        Some(ledger_seeds.stored_bump()),
    )?;
    require(
        policy_seeds.resolution_account().bytes() == accounts[resolution_index].key.to_bytes()
            && ledger_seeds.policy_account().bytes() == accounts[policy_index].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    Ok((policy, ledger))
}

/// Decode and execute one admitted FractionalRedemption successor action.
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    envelope_sequence: u64,
    action: FractionalRedemptionActionV1,
    payload: &[u8],
) -> Outcome<()> {
    match action {
        FractionalRedemptionActionV1::RedeemInternalExact => {
            let intent = FractionalRedeemIntentV1::decode(payload).map_err(map_fractional)?;
            process_redeem_internal_exact(program_id, accounts, envelope_sequence, intent)
        }
        FractionalRedemptionActionV1::RedeemBearerExact => {
            let intent = FractionalRedeemIntentV1::decode(payload).map_err(map_fractional)?;
            process_redeem_bearer_exact(program_id, accounts, envelope_sequence, intent)
        }
        FractionalRedemptionActionV1::RedeemInternalCredit => {
            let intent = FractionalRedeemIntentV1::decode(payload).map_err(map_fractional)?;
            process_redeem_internal_credit(program_id, accounts, envelope_sequence, intent)
        }
        FractionalRedemptionActionV1::RedeemBearerCredit => {
            let intent = FractionalRedeemIntentV1::decode(payload).map_err(map_fractional)?;
            process_redeem_bearer_credit(program_id, accounts, envelope_sequence, intent)
        }
        FractionalRedemptionActionV1::TransferCredit
        | FractionalRedemptionActionV1::MergeCredit => {
            let intent = FractionalTransferIntentV1::decode(payload).map_err(map_fractional)?;
            process_credit_move(program_id, accounts, envelope_sequence, action, intent)
        }
        FractionalRedemptionActionV1::CloseZeroCredit => {
            let intent = FractionalCloseCreditIntentV1::decode(payload).map_err(map_fractional)?;
            process_close_zero_credit(program_id, accounts, envelope_sequence, intent)
        }
        FractionalRedemptionActionV1::SealClaimsExhausted => {
            let intent = FractionalTerminalIntentV1::decode(payload).map_err(map_fractional)?;
            process_seal_claims_exhausted(program_id, accounts, envelope_sequence, intent)
        }
        _ => Err(ClutchError::UnsupportedInstruction.into()),
    }
}

#[inline(never)]
fn process_redeem_internal_exact(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    envelope_sequence: u64,
    intent: FractionalRedeemIntentV1,
) -> Outcome<()> {
    require_count(accounts, REDEEM_INTERNAL_EXACT_ACCOUNT_COUNT_V1)?;
    require_distinct(accounts)?;
    require_signer(&accounts[IX_ACTOR])?;
    require(
        !accounts[IX_ACTOR].is_writable && !accounts[IX_ACTOR].executable,
        ClutchError::UnexpectedWritable,
    )?;
    let mut index = 1usize;
    while index < accounts.len() {
        require(!accounts[index].is_signer, ClutchError::MismatchedState)?;
        index += 1;
    }
    require(
        envelope_sequence == intent.expected_ledger_sequence
            && intent.expected_position_replay_sequence != 0
            && intent.expected_credit_sequence == 0
            && intent.credit_mode == 0
            && accounts[IX_ACTOR].key.to_bytes() == intent.claimant.bytes()
            && accounts[IX_POSITION].key.to_bytes() == intent.claim_source.bytes()
            && accounts[IX_POSITION].key.to_bytes() == intent.payout_target.bytes()
            && accounts[IX_FRACTIONAL_POLICY].key.to_bytes() == intent.credit_or_policy.bytes(),
        ClutchError::MismatchedState,
    )?;

    let liabilities = authenticate_general_market_liabilities_v2(
        program_id,
        &accounts[IX_REALM],
        &accounts[IX_PROFILE],
        &accounts[IX_COLLATERAL_POLICY],
        &accounts[IX_COLLATERAL_TOKEN_PROGRAM],
        &accounts[IX_MARKET_BINDING],
        &accounts[IX_MARKET_RUNTIME],
        &accounts[IX_MARKET_INSTANCE],
        &accounts[IX_HOARD],
        &accounts[IX_CLAIM_LEDGER],
        true,
        true,
    )?;
    require(
        intent.outcome < liabilities.market_binding.base().outcome_count,
        ClutchError::MismatchedState,
    )?;
    let resolution = authenticate_resolution_v5(program_id, &accounts[IX_RESOLUTION], liabilities)?;
    let (policy, ledger) = decode_fractional_accounts(
        program_id,
        accounts,
        IX_FRACTIONAL_POLICY,
        IX_FRACTIONAL_LEDGER,
        IX_RESOLUTION,
    )?;
    require(
        policy.market_instance.bytes() == liabilities.market_binding.base().market_instance_v2_id.bytes()
            && policy.resolution_account.bytes() == resolution.account_id.bytes()
            && policy.resolution_data_id.bytes() == resolution.data_id.bytes()
            && ledger.claim_ledger_account.bytes() == accounts[IX_CLAIM_LEDGER].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let position = authenticate_general_position_replay_v2(
        program_id,
        liabilities.bound,
        &accounts[IX_MARKET_BINDING],
        &accounts[IX_MARKET_RUNTIME],
        &accounts[IX_POSITION],
        &accounts[IX_REPLAY],
        intent.claimant.bytes(),
        intent.expected_position_replay_sequence,
    )?;
    let context = bind_fractional_internal_context_v1(
        Identity32V1::new(accounts[IX_FRACTIONAL_POLICY].key.to_bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        policy,
        Identity32V1::new(accounts[IX_FRACTIONAL_LEDGER].key.to_bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        ledger,
        Identity32V1::new(accounts[IX_CLAIM_LEDGER].key.to_bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        liabilities.claim_ledger,
        liabilities.hoard,
        resolution.resolution,
        liabilities.bound,
    )
    .map_err(map_fractional)?;
    let plan = redeem_internal_exact_v1(
        context,
        intent.expected_ledger_sequence,
        intent.expected_position_replay_sequence,
        InternalPositionV1 {
            position_replay: position.replay,
        },
        intent.outcome,
        intent.quantity,
    )
    .map_err(map_fractional)?;
    require(
        plan.credit_after.is_none()
            && plan.claimant_numerator_after == 0
            && plan.custody_after.payout_atoms() == plan.paid_atoms,
        ClutchError::MismatchedState,
    )?;
    let RedemptionSourcePoststateV1::Internal(source_after) = plan.source_after else {
        return Err(ClutchError::MismatchedState.into());
    };
    require(
        source_after.position_account.bytes() == accounts[IX_POSITION].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;

    accounts[IX_FRACTIONAL_LEDGER]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(&plan.ledger_after.encode().map_err(map_fractional)?);
    plan.custody_after
        .hoard_after()
        .encode(
            &mut accounts[IX_HOARD]
                .try_borrow_mut_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    plan.custody_after
        .fractional()
        .claim_ledger_after()
        .encode(
            &mut accounts[IX_CLAIM_LEDGER]
                .try_borrow_mut_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    accounts[IX_POSITION]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(
            &source_after
                .position_after
                .encode()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        );
    accounts[IX_REPLAY]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(source_after.replay.replay_poststate_body());
    Ok(())
}

fn require_bearer_account_contract(
    accounts: &[AccountInfo<'_>],
    outcome_count: u8,
    selected_outcome: u8,
) -> Outcome<()> {
    let expected_count = REDEEM_BEARER_EXACT_PREFIX_ACCOUNTS_V1
        .checked_add(usize::from(outcome_count))
        .ok_or(ClutchError::Arithmetic)?;
    require_count(accounts, expected_count)?;
    require_signer(&accounts[bearer_ix::CLAIMANT])?;
    require_correlated_bearer_loader_aliases_v2(accounts)?;
    let selected_mint = bearer_ix::OUTCOME_MINTS + usize::from(selected_outcome);
    let mut index = 0usize;
    while index < accounts.len() {
        let expected_writable = matches!(
            index,
            bearer_ix::HOARD
                | bearer_ix::CLAIM_LEDGER
                | bearer_ix::FRACTIONAL_LEDGER
                | bearer_ix::DESTINATION
                | bearer_ix::HOARD_TOKEN
                | bearer_ix::SOURCE
        ) || index == selected_mint;
        require(
            accounts[index].is_writable == expected_writable,
            if expected_writable {
                ClutchError::NotWritable
            } else {
                ClutchError::UnexpectedWritable
            },
        )?;
        require(
            accounts[index].is_signer == (index == bearer_ix::CLAIMANT),
            ClutchError::MismatchedState,
        )?;
        let mut other = index + 1;
        while other < accounts.len() {
            if !bearer_loader_alias_pair_v2(index, other) {
                require(
                    accounts[index].key != accounts[other].key,
                    ClutchError::AccountAlias,
                )?;
            }
            other += 1;
        }
        index += 1;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug)]
enum CreditFundingAdmissionV1 {
    Live,
    Fresh {
        admission: RentSplitAdmissionPlanV2,
        bump: u8,
        payer_index: usize,
        system_index: usize,
    },
    Reopen {
        admission: RentSplitAdmissionPlanV2,
        bump: u8,
        payer_index: usize,
        system_index: usize,
    },
}

fn identity32(bytes: [u8; 32]) -> Outcome<Identity32V1> {
    Identity32V1::new(bytes).map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))
}

fn bearer_loader_alias_pair_v2(left: usize, right: usize) -> bool {
    matches!(
        (left, right),
        (
            bearer_ix::COLLATERAL_TOKEN_PROGRAM,
            bearer_ix::OUTCOME_TOKEN_PROGRAM
        ) | (
            bearer_ix::OUTCOME_TOKEN_PROGRAM,
            bearer_ix::COLLATERAL_TOKEN_PROGRAM
        ) | (
            bearer_ix::COLLATERAL_TOKEN_PROGRAMDATA,
            bearer_ix::OUTCOME_TOKEN_PROGRAMDATA
        ) | (
            bearer_ix::OUTCOME_TOKEN_PROGRAMDATA,
            bearer_ix::COLLATERAL_TOKEN_PROGRAMDATA
        )
    )
}

fn require_correlated_loader_alias_keys_v2(
    collateral_program: [u8; 32],
    outcome_program: [u8; 32],
    collateral_programdata: [u8; 32],
    outcome_programdata: [u8; 32],
) -> Outcome<()> {
    require(
        (collateral_program == outcome_program)
            == (collateral_programdata == outcome_programdata),
        ClutchError::AccountAlias,
    )
}

fn require_correlated_bearer_loader_aliases_v2(
    accounts: &[AccountInfo<'_>],
) -> Outcome<()> {
    require_correlated_loader_alias_keys_v2(
        accounts[bearer_ix::COLLATERAL_TOKEN_PROGRAM].key.to_bytes(),
        accounts[bearer_ix::OUTCOME_TOKEN_PROGRAM].key.to_bytes(),
        accounts[bearer_ix::COLLATERAL_TOKEN_PROGRAMDATA]
            .key
            .to_bytes(),
        accounts[bearer_ix::OUTCOME_TOKEN_PROGRAMDATA]
            .key
            .to_bytes(),
    )
}

#[cfg(test)]
mod loader_alias_tests {
    use super::*;

    #[test]
    fn bearer_loader_aliases_refuse_half_and_cross_pairs() {
        assert!(require_correlated_loader_alias_keys_v2([1; 32], [1; 32], [2; 32], [2; 32])
            .is_ok());
        assert!(require_correlated_loader_alias_keys_v2([1; 32], [3; 32], [2; 32], [4; 32])
            .is_ok());
        assert!(require_correlated_loader_alias_keys_v2([1; 32], [1; 32], [2; 32], [4; 32])
            .is_err());
        assert!(require_correlated_loader_alias_keys_v2([1; 32], [3; 32], [2; 32], [2; 32])
            .is_err());
        assert!(!bearer_loader_alias_pair_v2(
            bearer_ix::COLLATERAL_TOKEN_PROGRAM,
            bearer_ix::OUTCOME_TOKEN_PROGRAMDATA,
        ));
    }
}

#[allow(clippy::too_many_arguments)]
fn prepare_credit_prestate(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    credit_index: usize,
    rent_index: usize,
    funding_index: usize,
    credit_mode: u8,
    policy_account: Identity32V1,
    ledger_account: Identity32V1,
    claimant: Identity32V1,
    neutral_sink: Identity32V1,
) -> Outcome<(CreditPrestateV1, CreditFundingAdmissionV1)> {
    let credit = &accounts[credit_index];
    let expected = seeds::fractional_credit_v2_pda(
        program_id,
        &policy_account.bytes(),
        &claimant.bytes(),
    );
    require(credit.key.to_bytes() == expected.0.to_bytes(), ClutchError::WrongPda)?;
    let rent = read_rent(&accounts[rent_index])?;
    let live_minimum = rent.minimum_balance(FRACTIONAL_CREDIT_ACCOUNT_BYTES)?;
    let tombstone_minimum = rent.minimum_balance(FRACTIONAL_CREDIT_TOMBSTONE_BYTES)?;
    let fresh_refundable = live_minimum
        .checked_sub(tombstone_minimum)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        fresh_refundable != 0 && tombstone_minimum != 0,
        ClutchError::WrongRentSysvar,
    )?;
    match credit_mode {
        1 => {
            require_program_state(program_id, credit, true, FRACTIONAL_CREDIT_ACCOUNT_BYTES)?;
            let data = credit
                .try_borrow_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
            let value = FractionalCreditV2::decode(&data).map_err(map_fractional)?;
            drop(data);
            expect_pda(credit.key, expected, Some(value.stored_bump))?;
            require(
                value.policy_account == policy_account
                    && value.ledger_account == ledger_account
                    && value.claimant == claimant
                    && value.rent.permanent_tombstone_principal >= tombstone_minimum
                    && credit.lamports() >= live_minimum
                    && credit.lamports()
                        >= value
                            .rent
                            .refundable_live_principal
                            .checked_add(value.rent.permanent_tombstone_principal)
                            .and_then(|principal| {
                                principal.checked_add(value.rent.donation_floor)
                            })
                            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
                ClutchError::MismatchedState,
            )?;
            Ok((CreditPrestateV1::Live(value), CreditFundingAdmissionV1::Live))
        }
        2 | 3 => {
            let payer_index = funding_index;
            let system_index = funding_index + 1;
            require_signer(&accounts[payer_index])?;
            require(
                accounts[payer_index].is_writable && !accounts[payer_index].executable,
                ClutchError::NotWritable,
            )?;
            require_system_program(&accounts[system_index])?;
            if credit_mode == 2 {
                require_creatable(credit)?;
                let admission = admit_initial_rent_split(
                    identity32(credit.key.to_bytes())?,
                    identity32(accounts[payer_index].key.to_bytes())?,
                    fresh_refundable,
                    tombstone_minimum,
                    credit.lamports(),
                    accounts[payer_index].lamports(),
                    neutral_sink,
                )
                .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
                Ok((
                    CreditPrestateV1::Create(CreditCreationV1::Fresh {
                        claimant,
                        stored_bump: expected.1,
                        rent: admission.rent(),
                    }),
                    CreditFundingAdmissionV1::Fresh {
                        admission,
                        bump: expected.1,
                        payer_index,
                        system_index,
                    },
                ))
            } else {
                require_program_state(
                    program_id,
                    credit,
                    true,
                    FRACTIONAL_CREDIT_TOMBSTONE_BYTES,
                )?;
                let data = credit
                    .try_borrow_data()
                    .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
                let tombstone =
                    FractionalCreditTombstoneV2::decode(&data).map_err(map_fractional)?;
                drop(data);
                expect_pda(credit.key, expected, Some(tombstone.stored_bump))?;
                require(
                    tombstone.policy_account == policy_account
                        && tombstone.ledger_account == ledger_account
                        && tombstone.claimant == claimant
                        && tombstone.permanent_tombstone_principal >= tombstone_minimum,
                    ClutchError::MismatchedState,
                )?;
                let reopen_refundable = live_minimum
                    .checked_sub(tombstone.permanent_tombstone_principal)
                    .ok_or(Refusal::Adapter(ClutchError::WrongRentSysvar))?;
                require(reopen_refundable != 0, ClutchError::WrongRentSysvar)?;
                let admission = admit_reopen_rent_split(
                    identity32(credit.key.to_bytes())?,
                    identity32(accounts[payer_index].key.to_bytes())?,
                    reopen_refundable,
                    tombstone.permanent_tombstone_principal,
                    credit.lamports(),
                    accounts[payer_index].lamports(),
                    neutral_sink,
                )
                .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
                Ok((
                    CreditPrestateV1::Create(CreditCreationV1::Reopen {
                        tombstone,
                        rent: admission.rent(),
                    }),
                    CreditFundingAdmissionV1::Reopen {
                        admission,
                        bump: expected.1,
                        payer_index,
                        system_index,
                    },
                ))
            }
        }
        _ => Err(ClutchError::MismatchedState.into()),
    }
}

fn apply_credit_funding<'a>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'a>],
    credit_index: usize,
    policy_account: [u8; 32],
    claimant: [u8; 32],
    funding: CreditFundingAdmissionV1,
) -> Outcome<()> {
    let (admission, bump, payer_index, system_index, fresh) = match funding {
        CreditFundingAdmissionV1::Live => return Ok(()),
        CreditFundingAdmissionV1::Fresh {
            admission,
            bump,
            payer_index,
            system_index,
        } => (admission, bump, payer_index, system_index, true),
        CreditFundingAdmissionV1::Reopen {
            admission,
            bump,
            payer_index,
            system_index,
        } => (admission, bump, payer_index, system_index, false),
    };
    let payer = &accounts[payer_index];
    let credit = &accounts[credit_index];
    let payer_before = payer.lamports();
    let debit = payer_before
        .checked_sub(admission.payer_balance_after())
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let transfer = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &transfer_data(debit),
        vec![
            AccountMeta::new(*payer.key, true),
            AccountMeta::new(*credit.key, false),
        ],
    );
    invoke(
        &transfer,
        &[
            payer.clone(),
            credit.clone(),
            accounts[system_index].clone(),
        ],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        payer.lamports() == admission.payer_balance_after()
            && credit.lamports() == admission.account_balance_after(),
        ClutchError::AccountCreationFailed,
    )?;
    if fresh {
        let bump_seed = [bump];
        let signer: [&[u8]; 4] = [
            seeds::SEED_FRACTIONAL_CREDIT_V2,
            &policy_account,
            &claimant,
            &bump_seed,
        ];
        let allocate = Instruction::new_with_bytes(
            SYSTEM_PROGRAM_ID,
            &allocate_data(FRACTIONAL_CREDIT_ACCOUNT_BYTES),
            vec![AccountMeta::new(*credit.key, true)],
        );
        invoke_signed(
            &allocate,
            &[credit.clone(), accounts[system_index].clone()],
            &[&signer],
        )
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
        let assign = Instruction::new_with_bytes(
            SYSTEM_PROGRAM_ID,
            &assign_data(program_id),
            vec![AccountMeta::new(*credit.key, true)],
        );
        invoke_signed(
            &assign,
            &[credit.clone(), accounts[system_index].clone()],
            &[&signer],
        )
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    } else {
        credit
            .resize(FRACTIONAL_CREDIT_ACCOUNT_BYTES)
            .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    }
    require(
        credit.owner == program_id
            && credit.data_len() == FRACTIONAL_CREDIT_ACCOUNT_BYTES
            && credit.lamports() == admission.account_balance_after(),
        ClutchError::AccountCreationFailed,
    )
}

fn require_internal_credit_account_contract(
    accounts: &[AccountInfo<'_>],
    credit_mode: u8,
) -> Outcome<()> {
    let creation = matches!(credit_mode, 2 | 3);
    let expected = REDEEM_INTERNAL_CREDIT_LIVE_ACCOUNT_COUNT_V1
        + if creation { CREDIT_CREATION_SUFFIX_ACCOUNTS_V1 } else { 0 };
    require_count(accounts, expected)?;
    let payer_index = REDEEM_INTERNAL_CREDIT_LIVE_ACCOUNT_COUNT_V1;
    let payer_alias = creation && accounts[IX_ACTOR].key == accounts[payer_index].key;
    let mut index = 0usize;
    while index < accounts.len() {
        let expected_writable = matches!(index, IX_HOARD | IX_CLAIM_LEDGER | IX_FRACTIONAL_LEDGER | IX_POSITION | IX_REPLAY | IX_CREDIT)
            || (creation && index == payer_index)
            || (index == IX_ACTOR && payer_alias);
        require(
            accounts[index].is_writable == expected_writable,
            if expected_writable { ClutchError::NotWritable } else { ClutchError::UnexpectedWritable },
        )?;
        let expected_signer = index == IX_ACTOR || (creation && index == payer_index);
        require(accounts[index].is_signer == expected_signer, ClutchError::MismatchedState)?;
        let mut other = index + 1;
        while other < accounts.len() {
            let allowed_payer_alias = creation
                && ((index == IX_ACTOR && other == payer_index)
                    || (index == payer_index && other == IX_ACTOR));
            if !allowed_payer_alias {
                require(accounts[index].key != accounts[other].key, ClutchError::AccountAlias)?;
            }
            other += 1;
        }
        index += 1;
    }
    Ok(())
}

#[inline(never)]
fn process_redeem_internal_credit(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    envelope_sequence: u64,
    intent: FractionalRedeemIntentV1,
) -> Outcome<()> {
    require_internal_credit_account_contract(accounts, intent.credit_mode)?;
    require(
        envelope_sequence == intent.expected_ledger_sequence
            && intent.expected_position_replay_sequence != 0
            && intent.expected_credit_sequence != 0
            && (1..=3).contains(&intent.credit_mode)
            && accounts[IX_ACTOR].key.to_bytes() == intent.claimant.bytes()
            && accounts[IX_POSITION].key.to_bytes() == intent.claim_source.bytes()
            && accounts[IX_POSITION].key.to_bytes() == intent.payout_target.bytes()
            && accounts[IX_CREDIT].key.to_bytes() == intent.credit_or_policy.bytes(),
        ClutchError::MismatchedState,
    )?;
    let liabilities = authenticate_general_market_liabilities_v2(
        program_id,
        &accounts[IX_REALM],
        &accounts[IX_PROFILE],
        &accounts[IX_COLLATERAL_POLICY],
        &accounts[IX_COLLATERAL_TOKEN_PROGRAM],
        &accounts[IX_MARKET_BINDING],
        &accounts[IX_MARKET_RUNTIME],
        &accounts[IX_MARKET_INSTANCE],
        &accounts[IX_HOARD],
        &accounts[IX_CLAIM_LEDGER],
        true,
        true,
    )?;
    require(
        intent.outcome < liabilities.market_binding.base().outcome_count,
        ClutchError::MismatchedState,
    )?;
    let resolution = authenticate_resolution_v5(program_id, &accounts[IX_RESOLUTION], liabilities)?;
    let (policy, ledger) = decode_fractional_accounts(
        program_id,
        accounts,
        IX_FRACTIONAL_POLICY,
        IX_FRACTIONAL_LEDGER,
        IX_RESOLUTION,
    )?;
    require(
        policy.market_instance.bytes() == liabilities.market_binding.base().market_instance_v2_id.bytes()
            && policy.resolution_account.bytes() == resolution.account_id.bytes()
            && policy.resolution_data_id.bytes() == resolution.data_id.bytes()
            && ledger.claim_ledger_account.bytes() == accounts[IX_CLAIM_LEDGER].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let root = authenticate_market_lifecycle_root_v1(
        program_id,
        &accounts[IX_MARKET_LIFECYCLE_ROOT],
        liabilities.market_binding.base().market_instance_v2_id,
        policy.domain_generation,
        false,
    )?;
    let neutral = root.state().capital().neutral_lamport_sink;
    require(
        accounts[IX_NEUTRAL_SINK].key.to_bytes() == neutral.bytes()
            && !accounts[IX_NEUTRAL_SINK].executable
            && accounts[IX_NEUTRAL_SINK].owner == &SYSTEM_PROGRAM_ID
            && accounts[IX_NEUTRAL_SINK].data_is_empty(),
        ClutchError::MismatchedState,
    )?;
    let (credit_prestate, funding) = prepare_credit_prestate(
        program_id,
        accounts,
        IX_CREDIT,
        IX_CREDIT_RENT,
        REDEEM_INTERNAL_CREDIT_LIVE_ACCOUNT_COUNT_V1,
        intent.credit_mode,
        identity32(accounts[IX_FRACTIONAL_POLICY].key.to_bytes())?,
        identity32(accounts[IX_FRACTIONAL_LEDGER].key.to_bytes())?,
        intent.claimant,
        identity32(neutral.bytes())?,
    )?;
    let position = authenticate_general_position_replay_v2(
        program_id,
        liabilities.bound,
        &accounts[IX_MARKET_BINDING],
        &accounts[IX_MARKET_RUNTIME],
        &accounts[IX_POSITION],
        &accounts[IX_REPLAY],
        intent.claimant.bytes(),
        intent.expected_position_replay_sequence,
    )?;
    let context = bind_fractional_internal_context_v1(
        identity32(accounts[IX_FRACTIONAL_POLICY].key.to_bytes())?,
        policy,
        identity32(accounts[IX_FRACTIONAL_LEDGER].key.to_bytes())?,
        ledger,
        identity32(accounts[IX_CLAIM_LEDGER].key.to_bytes())?,
        liabilities.claim_ledger,
        liabilities.hoard,
        resolution.resolution,
        liabilities.bound,
    )
    .map_err(map_fractional)?;
    let plan = redeem_internal_to_credit_v1(
        context,
        intent.expected_ledger_sequence,
        intent.expected_credit_sequence,
        intent.expected_position_replay_sequence,
        credit_prestate,
        InternalPositionV1 {
            position_replay: position.replay,
        },
        intent.outcome,
        intent.quantity,
    )
    .map_err(map_fractional)?;
    let credit_after = plan
        .credit_after
        .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
    let RedemptionSourcePoststateV1::Internal(source_after) = plan.source_after else {
        return Err(ClutchError::MismatchedState.into());
    };
    require(
        credit_after.claimant == intent.claimant
            && source_after.position_account.bytes() == accounts[IX_POSITION].key.to_bytes()
            && plan.custody_after.payout_atoms() == plan.paid_atoms,
        ClutchError::MismatchedState,
    )?;

    apply_credit_funding(
        program_id,
        accounts,
        IX_CREDIT,
        accounts[IX_FRACTIONAL_POLICY].key.to_bytes(),
        intent.claimant.bytes(),
        funding,
    )?;
    accounts[IX_CREDIT]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(&credit_after.encode().map_err(map_fractional)?);
    accounts[IX_FRACTIONAL_LEDGER]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(&plan.ledger_after.encode().map_err(map_fractional)?);
    plan.custody_after
        .hoard_after()
        .encode(
            &mut accounts[IX_HOARD]
                .try_borrow_mut_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    plan.custody_after
        .fractional()
        .claim_ledger_after()
        .encode(
            &mut accounts[IX_CLAIM_LEDGER]
                .try_borrow_mut_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    accounts[IX_POSITION]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(
            &source_after
                .position_after
                .encode()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        );
    accounts[IX_REPLAY]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(source_after.replay.replay_poststate_body());
    Ok(())
}

#[inline(never)]
fn process_redeem_bearer_exact(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    envelope_sequence: u64,
    intent: FractionalRedeemIntentV1,
) -> Outcome<()> {
    require(
        accounts.len() >= REDEEM_BEARER_EXACT_PREFIX_ACCOUNTS_V1,
        ClutchError::WrongAccountCount,
    )?;
    require(
        envelope_sequence == intent.expected_ledger_sequence
            && intent.expected_credit_sequence == 0
            && intent.expected_position_replay_sequence == 0
            && intent.credit_mode == 0
            && accounts[bearer_ix::CLAIMANT].key.to_bytes() == intent.claimant.bytes()
            && accounts[bearer_ix::SOURCE].key.to_bytes() == intent.claim_source.bytes()
            && accounts[bearer_ix::DESTINATION].key.to_bytes() == intent.payout_target.bytes()
            && accounts[bearer_ix::FRACTIONAL_POLICY].key.to_bytes()
                == intent.credit_or_policy.bytes(),
        ClutchError::MismatchedState,
    )?;

    let value_authority = authenticate_general_market_value_authority_v2(
        program_id,
        &accounts[bearer_ix::REALM],
        &accounts[bearer_ix::PROFILE],
        &accounts[bearer_ix::COLLATERAL_POLICY],
        &accounts[bearer_ix::COLLATERAL_TOKEN_PROGRAM],
        &accounts[bearer_ix::COLLATERAL_TOKEN_PROGRAMDATA],
        &accounts[bearer_ix::MARKET_BINDING],
        &accounts[bearer_ix::MARKET_RUNTIME],
        &accounts[bearer_ix::MARKET_INSTANCE],
        &accounts[bearer_ix::HOARD],
        &accounts[bearer_ix::CLAIM_LEDGER],
        true,
        true,
    )?;
    let liabilities = value_authority.liabilities;
    require(
        intent.outcome < liabilities.market_binding.base().outcome_count,
        ClutchError::MismatchedState,
    )?;
    require_bearer_account_contract(
        accounts,
        liabilities.market_binding.base().outcome_count,
        intent.outcome,
    )?;
    require(
        accounts[bearer_ix::COLLATERAL_MINT].key.to_bytes()
            == liabilities.bound.policy().mint.bytes()
            && accounts[bearer_ix::HOARD_TOKEN].key.to_bytes()
                == liabilities.hoard.token_account.bytes()
            && accounts[bearer_ix::HOARD_AUTHORITY].key.to_bytes()
                == liabilities.hoard.authority.bytes()
            && !accounts[bearer_ix::HOARD_AUTHORITY].executable
            && accounts[bearer_ix::HOARD_AUTHORITY].data_is_empty(),
        ClutchError::MismatchedState,
    )?;
    let market_bytes = liabilities.market_binding.base().market_instance_v2_id.bytes();
    expect_pda(
        accounts[bearer_ix::HOARD_AUTHORITY].key,
        seeds::hoard_authority_v2_pda(program_id, &market_bytes),
        None,
    )?;
    expect_pda(
        accounts[bearer_ix::HOARD_TOKEN].key,
        seeds::hoard_token_v2_pda(program_id, &market_bytes),
        None,
    )?;
    let resolution =
        authenticate_resolution_v5(program_id, &accounts[bearer_ix::RESOLUTION], liabilities)?;
    let claim = authenticate_claim_issuance_release_with_programdata_v1(
        liabilities.bound,
        &accounts[bearer_ix::OUTCOME_TOKEN_PROGRAM],
        &accounts[bearer_ix::OUTCOME_TOKEN_PROGRAMDATA],
    )?;
    let (policy, ledger) = decode_fractional_accounts(
        program_id,
        accounts,
        bearer_ix::FRACTIONAL_POLICY,
        bearer_ix::FRACTIONAL_LEDGER,
        bearer_ix::RESOLUTION,
    )?;
    require(
        policy.market_instance.bytes() == market_bytes
            && policy.resolution_account.bytes() == resolution.account_id.bytes()
            && policy.resolution_data_id.bytes() == resolution.data_id.bytes()
            && ledger.claim_ledger_account.bytes()
                == accounts[bearer_ix::CLAIM_LEDGER].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let observed_before = observe_outcome_mints_for_bearer_v3(
        program_id,
        accounts,
        bearer_ix::OUTCOME_MINTS,
        &accounts[bearer_ix::MARKET_RUNTIME],
        market_bytes,
        liabilities.market_binding.base().outcome_count,
        intent.outcome,
    )?;
    let selected_mint = &accounts[bearer_ix::OUTCOME_MINTS + usize::from(intent.outcome)];
    let token_before = bearer_claim_observation_v3(
        selected_mint,
        &accounts[bearer_ix::SOURCE],
        &accounts[bearer_ix::CLAIMANT],
        &accounts[bearer_ix::MARKET_RUNTIME],
    )?;
    let context = bind_fractional_context_v1(
        Identity32V1::new(accounts[bearer_ix::FRACTIONAL_POLICY].key.to_bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        policy,
        Identity32V1::new(accounts[bearer_ix::FRACTIONAL_LEDGER].key.to_bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        ledger,
        Identity32V1::new(accounts[bearer_ix::CLAIM_LEDGER].key.to_bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        liabilities.claim_ledger,
        liabilities.hoard,
        resolution.resolution,
        liabilities.bound,
        claim.bound(),
    )
    .map_err(map_fractional)?;
    let prepared = prepare_bearer_exact_v1(
        context,
        intent.expected_ledger_sequence,
        BearerClaimPrestateV1 {
            claimant: intent.claimant,
            claim_token_account: intent.claim_source,
            claim_mint: Identity32V1::new(selected_mint.key.to_bytes())
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
            collateral_destination: intent.payout_target,
            claim_issuance_binding: policy.claim_issuance_binding,
            source_claim_atoms: token_before.source_atoms,
            observed_materialized_supply: observed_before.values,
        },
        intent.outcome,
        intent.quantity,
    )
    .map_err(map_fractional)?;
    let prepared_burn = prepare_fractional_bearer_claim_burn_v3(
        claim.bound(),
        CollateralId::from_bytes(accounts[bearer_ix::MARKET_RUNTIME].key.to_bytes()),
        CollateralId::from_bytes(accounts[bearer_ix::CLAIMANT].key.to_bytes()),
        intent.outcome,
        intent.quantity,
        observed_before.values,
        token_before,
        prepared.fractional_claim_ledger(),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    let burn = prepared_burn.burn_intent();
    require(
        burn.mint == CollateralId::from_bytes(selected_mint.key.to_bytes())
            && burn.source_token_account
                == CollateralId::from_bytes(accounts[bearer_ix::SOURCE].key.to_bytes())
            && burn.claimant
                == CollateralId::from_bytes(accounts[bearer_ix::CLAIMANT].key.to_bytes())
            && burn.quantity == intent.quantity,
        ClutchError::MismatchedState,
    )?;
    token::burn(
        &accounts[bearer_ix::OUTCOME_TOKEN_PROGRAM],
        &accounts[bearer_ix::SOURCE],
        selected_mint,
        &accounts[bearer_ix::CLAIMANT],
        intent.quantity,
    )?;
    let observed_after = observe_outcome_mints_for_bearer_v3(
        program_id,
        accounts,
        bearer_ix::OUTCOME_MINTS,
        &accounts[bearer_ix::MARKET_RUNTIME],
        market_bytes,
        liabilities.market_binding.base().outcome_count,
        intent.outcome,
    )?;
    let token_after = bearer_claim_observation_v3(
        selected_mint,
        &accounts[bearer_ix::SOURCE],
        &accounts[bearer_ix::CLAIMANT],
        &accounts[bearer_ix::MARKET_RUNTIME],
    )?;
    let accepted_burn =
        accept_fractional_bearer_claim_burn_v3(prepared_burn, observed_after.values, token_after)
            .map_err(|_| Refusal::Adapter(ClutchError::TokenDeltaMismatch))?;
    let burned = accept_bearer_exact_burn_v1(prepared, accepted_burn).map_err(map_fractional)?;
    let collateral_request = burned.collateral_request();
    let collateral = {
        let mint_data = accounts[bearer_ix::COLLATERAL_MINT]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let hoard_data = accounts[bearer_ix::HOARD_TOKEN]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let destination_data = accounts[bearer_ix::DESTINATION]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        if collateral_request.payout_atoms == 0 {
            let prepared = prepare_zero_claim_redemption_collateral_v2(
                liabilities.bound,
                collateral_request,
                runtime_account_view(&accounts[bearer_ix::COLLATERAL_MINT], &mint_data),
                runtime_account_view(&accounts[bearer_ix::HOARD_TOKEN], &hoard_data),
                runtime_account_view(&accounts[bearer_ix::DESTINATION], &destination_data),
            )
            .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
            drop((mint_data, hoard_data, destination_data));
            AcceptedBearerRedemptionCollateralV3::Zero(accept_zero_claim_collateral_payout(
                prepared,
                &accounts[bearer_ix::COLLATERAL_MINT],
                &accounts[bearer_ix::HOARD_TOKEN],
                &accounts[bearer_ix::DESTINATION],
            )?)
        } else {
            let prepared = prepare_claim_redemption_collateral_v2(
                liabilities.bound,
                collateral_request,
                TransferAuthorityV2 {
                    address: CollateralId::from_bytes(
                        accounts[bearer_ix::HOARD_AUTHORITY].key.to_bytes(),
                    ),
                    kind: TransferAuthorityKindV2::ProgramDerived,
                    is_transaction_signer: false,
                    program_address_authenticated: true,
                    is_writable: accounts[bearer_ix::HOARD_AUTHORITY].is_writable,
                    executable: accounts[bearer_ix::HOARD_AUTHORITY].executable,
                    data_is_empty: accounts[bearer_ix::HOARD_AUTHORITY].data_is_empty(),
                },
                runtime_account_view(&accounts[bearer_ix::COLLATERAL_MINT], &mint_data),
                runtime_account_view(&accounts[bearer_ix::HOARD_TOKEN], &hoard_data),
                runtime_account_view(&accounts[bearer_ix::DESTINATION], &destination_data),
            )
            .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
            drop((mint_data, hoard_data, destination_data));
            let bump = [seeds::hoard_authority_v2_pda(program_id, &market_bytes).1];
            let signer: [&[u8]; 3] = [seeds::SEED_HOARD_AUTHORITY_V2, &market_bytes, &bump];
            AcceptedBearerRedemptionCollateralV3::Nonzero(invoke_claim_collateral_payout(
                prepared,
                &accounts[bearer_ix::COLLATERAL_MINT],
                &accounts[bearer_ix::HOARD_TOKEN],
                &accounts[bearer_ix::DESTINATION],
                &accounts[bearer_ix::HOARD_AUTHORITY],
                &accounts[bearer_ix::COLLATERAL_TOKEN_PROGRAM],
                &signer,
            )?)
        }
    };
    let runtime_execution = bind_fractional_collateral_execution_v2(
        accepted_burn.fractional().transition_id(),
        value_authority.receipt_id,
        claim.receipt_id(),
        accepted_burn.burn_receipt_id(),
        collateral_delta_receipt_id(collateral),
    )?;
    let plan = finish_bearer_exact_v1(burned, collateral).map_err(map_fractional)?;
    let RedemptionSourcePoststateV1::Bearer(source_after) = plan.source_after else {
        return Err(ClutchError::MismatchedState.into());
    };
    require(
        plan.credit_after.is_none()
            && plan.claimant_numerator_after == 0
            && source_after.transition_id.bytes()
                == accepted_burn.fractional().transition_id().bytes()
            && runtime_execution.transition_id == accepted_burn.fractional().transition_id()
            && !runtime_execution.receipt_id.is_zero()
            && source_after.burn_receipt_id.map(Identity32V1::bytes)
                == Some(accepted_burn.burn_receipt_id().bytes())
            && source_after.claim_token_account.bytes()
                == accounts[bearer_ix::SOURCE].key.to_bytes()
            && source_after.claim_mint.bytes() == selected_mint.key.to_bytes()
            && source_after.collateral_destination.bytes()
                == accounts[bearer_ix::DESTINATION].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    accounts[bearer_ix::FRACTIONAL_LEDGER]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(&plan.ledger_after.encode().map_err(map_fractional)?);
    plan.custody_after
        .hoard_after()
        .encode(
            &mut accounts[bearer_ix::HOARD]
                .try_borrow_mut_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    plan.custody_after
        .fractional()
        .claim_ledger_after()
        .encode(
            &mut accounts[bearer_ix::CLAIM_LEDGER]
                .try_borrow_mut_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok(())
}

fn require_bearer_credit_account_contract(
    accounts: &[AccountInfo<'_>],
    outcome_count: u8,
    selected_outcome: u8,
    credit_mode: u8,
) -> Outcome<(usize, usize, usize, usize, usize)> {
    let creation = matches!(credit_mode, 2 | 3);
    let credit_index = bearer_ix::OUTCOME_MINTS
        .checked_add(usize::from(outcome_count))
        .ok_or(ClutchError::Arithmetic)?;
    let root_index = credit_index + 1;
    let neutral_index = credit_index + 2;
    let rent_index = credit_index + 3;
    let funding_index = credit_index + REDEEM_BEARER_CREDIT_POST_MINT_ACCOUNTS_V1;
    let expected = funding_index + if creation { CREDIT_CREATION_SUFFIX_ACCOUNTS_V1 } else { 0 };
    require_count(accounts, expected)?;
    require_signer(&accounts[bearer_ix::CLAIMANT])?;
    require_correlated_bearer_loader_aliases_v2(accounts)?;
    let selected_mint = bearer_ix::OUTCOME_MINTS + usize::from(selected_outcome);
    let payer_alias = creation && accounts[bearer_ix::CLAIMANT].key == accounts[funding_index].key;
    let mut index = 0usize;
    while index < accounts.len() {
        let expected_writable = matches!(
            index,
            bearer_ix::HOARD
                | bearer_ix::CLAIM_LEDGER
                | bearer_ix::FRACTIONAL_LEDGER
                | bearer_ix::DESTINATION
                | bearer_ix::HOARD_TOKEN
                | bearer_ix::SOURCE
        ) || index == selected_mint
            || index == credit_index
            || (creation && index == funding_index)
            || (index == bearer_ix::CLAIMANT && payer_alias);
        require(
            accounts[index].is_writable == expected_writable,
            if expected_writable { ClutchError::NotWritable } else { ClutchError::UnexpectedWritable },
        )?;
        let expected_signer = index == bearer_ix::CLAIMANT || (creation && index == funding_index);
        require(accounts[index].is_signer == expected_signer, ClutchError::MismatchedState)?;
        let mut other = index + 1;
        while other < accounts.len() {
            let payer_alias = creation
                && ((index == bearer_ix::CLAIMANT && other == funding_index)
                    || (index == funding_index && other == bearer_ix::CLAIMANT));
            if !bearer_loader_alias_pair_v2(index, other) && !payer_alias {
                require(accounts[index].key != accounts[other].key, ClutchError::AccountAlias)?;
            }
            other += 1;
        }
        index += 1;
    }
    Ok((credit_index, root_index, neutral_index, rent_index, funding_index))
}

#[inline(never)]
fn process_redeem_bearer_credit(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    envelope_sequence: u64,
    intent: FractionalRedeemIntentV1,
) -> Outcome<()> {
    require(
        accounts.len() >= REDEEM_BEARER_EXACT_PREFIX_ACCOUNTS_V1,
        ClutchError::WrongAccountCount,
    )?;
    require(
        envelope_sequence == intent.expected_ledger_sequence
            && intent.expected_credit_sequence != 0
            && intent.expected_position_replay_sequence == 0
            && (1..=3).contains(&intent.credit_mode)
            && accounts[bearer_ix::CLAIMANT].key.to_bytes() == intent.claimant.bytes()
            && accounts[bearer_ix::SOURCE].key.to_bytes() == intent.claim_source.bytes()
            && accounts[bearer_ix::DESTINATION].key.to_bytes() == intent.payout_target.bytes(),
        ClutchError::MismatchedState,
    )?;
    let value_authority = authenticate_general_market_value_authority_v2(
        program_id,
        &accounts[bearer_ix::REALM],
        &accounts[bearer_ix::PROFILE],
        &accounts[bearer_ix::COLLATERAL_POLICY],
        &accounts[bearer_ix::COLLATERAL_TOKEN_PROGRAM],
        &accounts[bearer_ix::COLLATERAL_TOKEN_PROGRAMDATA],
        &accounts[bearer_ix::MARKET_BINDING],
        &accounts[bearer_ix::MARKET_RUNTIME],
        &accounts[bearer_ix::MARKET_INSTANCE],
        &accounts[bearer_ix::HOARD],
        &accounts[bearer_ix::CLAIM_LEDGER],
        true,
        true,
    )?;
    let liabilities = value_authority.liabilities;
    require(
        intent.outcome < liabilities.market_binding.base().outcome_count,
        ClutchError::MismatchedState,
    )?;
    let (credit_index, root_index, neutral_index, rent_index, funding_index) =
        require_bearer_credit_account_contract(
            accounts,
            liabilities.market_binding.base().outcome_count,
            intent.outcome,
            intent.credit_mode,
        )?;
    require(
        accounts[credit_index].key.to_bytes() == intent.credit_or_policy.bytes()
            && accounts[bearer_ix::COLLATERAL_MINT].key.to_bytes()
                == liabilities.bound.policy().mint.bytes()
            && accounts[bearer_ix::HOARD_TOKEN].key.to_bytes()
                == liabilities.hoard.token_account.bytes()
            && accounts[bearer_ix::HOARD_AUTHORITY].key.to_bytes()
                == liabilities.hoard.authority.bytes()
            && !accounts[bearer_ix::HOARD_AUTHORITY].executable
            && accounts[bearer_ix::HOARD_AUTHORITY].data_is_empty(),
        ClutchError::MismatchedState,
    )?;
    let market_bytes = liabilities.market_binding.base().market_instance_v2_id.bytes();
    expect_pda(
        accounts[bearer_ix::HOARD_AUTHORITY].key,
        seeds::hoard_authority_v2_pda(program_id, &market_bytes),
        None,
    )?;
    expect_pda(
        accounts[bearer_ix::HOARD_TOKEN].key,
        seeds::hoard_token_v2_pda(program_id, &market_bytes),
        None,
    )?;
    let resolution =
        authenticate_resolution_v5(program_id, &accounts[bearer_ix::RESOLUTION], liabilities)?;
    let claim = authenticate_claim_issuance_release_with_programdata_v1(
        liabilities.bound,
        &accounts[bearer_ix::OUTCOME_TOKEN_PROGRAM],
        &accounts[bearer_ix::OUTCOME_TOKEN_PROGRAMDATA],
    )?;
    let (policy, ledger) = decode_fractional_accounts(
        program_id,
        accounts,
        bearer_ix::FRACTIONAL_POLICY,
        bearer_ix::FRACTIONAL_LEDGER,
        bearer_ix::RESOLUTION,
    )?;
    require(
        policy.market_instance.bytes() == market_bytes
            && policy.resolution_account.bytes() == resolution.account_id.bytes()
            && policy.resolution_data_id.bytes() == resolution.data_id.bytes()
            && ledger.claim_ledger_account.bytes()
                == accounts[bearer_ix::CLAIM_LEDGER].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let root = authenticate_market_lifecycle_root_v1(
        program_id,
        &accounts[root_index],
        liabilities.market_binding.base().market_instance_v2_id,
        policy.domain_generation,
        false,
    )?;
    let neutral = root.state().capital().neutral_lamport_sink;
    require(
        accounts[neutral_index].key.to_bytes() == neutral.bytes()
            && !accounts[neutral_index].executable
            && accounts[neutral_index].owner == &SYSTEM_PROGRAM_ID
            && accounts[neutral_index].data_is_empty(),
        ClutchError::MismatchedState,
    )?;
    let (credit_prestate, funding) = prepare_credit_prestate(
        program_id,
        accounts,
        credit_index,
        rent_index,
        funding_index,
        intent.credit_mode,
        identity32(accounts[bearer_ix::FRACTIONAL_POLICY].key.to_bytes())?,
        identity32(accounts[bearer_ix::FRACTIONAL_LEDGER].key.to_bytes())?,
        intent.claimant,
        identity32(neutral.bytes())?,
    )?;
    let observed_before = observe_outcome_mints_for_bearer_v3(
        program_id,
        accounts,
        bearer_ix::OUTCOME_MINTS,
        &accounts[bearer_ix::MARKET_RUNTIME],
        market_bytes,
        liabilities.market_binding.base().outcome_count,
        intent.outcome,
    )?;
    let selected_mint = &accounts[bearer_ix::OUTCOME_MINTS + usize::from(intent.outcome)];
    let token_before = bearer_claim_observation_v3(
        selected_mint,
        &accounts[bearer_ix::SOURCE],
        &accounts[bearer_ix::CLAIMANT],
        &accounts[bearer_ix::MARKET_RUNTIME],
    )?;
    let context = bind_fractional_context_v1(
        identity32(accounts[bearer_ix::FRACTIONAL_POLICY].key.to_bytes())?,
        policy,
        identity32(accounts[bearer_ix::FRACTIONAL_LEDGER].key.to_bytes())?,
        ledger,
        identity32(accounts[bearer_ix::CLAIM_LEDGER].key.to_bytes())?,
        liabilities.claim_ledger,
        liabilities.hoard,
        resolution.resolution,
        liabilities.bound,
        claim.bound(),
    )
    .map_err(map_fractional)?;
    let prepared = prepare_bearer_credit_v1(
        context,
        intent.expected_ledger_sequence,
        intent.expected_credit_sequence,
        credit_prestate,
        BearerClaimPrestateV1 {
            claimant: intent.claimant,
            claim_token_account: intent.claim_source,
            claim_mint: identity32(selected_mint.key.to_bytes())?,
            collateral_destination: intent.payout_target,
            claim_issuance_binding: policy.claim_issuance_binding,
            source_claim_atoms: token_before.source_atoms,
            observed_materialized_supply: observed_before.values,
        },
        intent.outcome,
        intent.quantity,
    )
    .map_err(map_fractional)?;
    let prepared_burn = prepare_fractional_bearer_claim_burn_v3(
        claim.bound(),
        CollateralId::from_bytes(accounts[bearer_ix::MARKET_RUNTIME].key.to_bytes()),
        CollateralId::from_bytes(accounts[bearer_ix::CLAIMANT].key.to_bytes()),
        intent.outcome,
        intent.quantity,
        observed_before.values,
        token_before,
        prepared.fractional_claim_ledger(),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    let burn = prepared_burn.burn_intent();
    require(
        burn.mint == CollateralId::from_bytes(selected_mint.key.to_bytes())
            && burn.source_token_account
                == CollateralId::from_bytes(accounts[bearer_ix::SOURCE].key.to_bytes())
            && burn.claimant
                == CollateralId::from_bytes(accounts[bearer_ix::CLAIMANT].key.to_bytes())
            && burn.quantity == intent.quantity,
        ClutchError::MismatchedState,
    )?;
    token::burn(
        &accounts[bearer_ix::OUTCOME_TOKEN_PROGRAM],
        &accounts[bearer_ix::SOURCE],
        selected_mint,
        &accounts[bearer_ix::CLAIMANT],
        intent.quantity,
    )?;
    let observed_after = observe_outcome_mints_for_bearer_v3(
        program_id,
        accounts,
        bearer_ix::OUTCOME_MINTS,
        &accounts[bearer_ix::MARKET_RUNTIME],
        market_bytes,
        liabilities.market_binding.base().outcome_count,
        intent.outcome,
    )?;
    let token_after = bearer_claim_observation_v3(
        selected_mint,
        &accounts[bearer_ix::SOURCE],
        &accounts[bearer_ix::CLAIMANT],
        &accounts[bearer_ix::MARKET_RUNTIME],
    )?;
    let accepted_burn =
        accept_fractional_bearer_claim_burn_v3(prepared_burn, observed_after.values, token_after)
            .map_err(|_| Refusal::Adapter(ClutchError::TokenDeltaMismatch))?;
    let burned = accept_bearer_credit_burn_v1(prepared, accepted_burn).map_err(map_fractional)?;
    let collateral_request = burned.collateral_request();
    let collateral = {
        let mint_data = accounts[bearer_ix::COLLATERAL_MINT]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let hoard_data = accounts[bearer_ix::HOARD_TOKEN]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let destination_data = accounts[bearer_ix::DESTINATION]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        if collateral_request.payout_atoms == 0 {
            let prepared = prepare_zero_claim_redemption_collateral_v2(
                liabilities.bound,
                collateral_request,
                runtime_account_view(&accounts[bearer_ix::COLLATERAL_MINT], &mint_data),
                runtime_account_view(&accounts[bearer_ix::HOARD_TOKEN], &hoard_data),
                runtime_account_view(&accounts[bearer_ix::DESTINATION], &destination_data),
            )
            .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
            drop((mint_data, hoard_data, destination_data));
            AcceptedBearerRedemptionCollateralV3::Zero(accept_zero_claim_collateral_payout(
                prepared,
                &accounts[bearer_ix::COLLATERAL_MINT],
                &accounts[bearer_ix::HOARD_TOKEN],
                &accounts[bearer_ix::DESTINATION],
            )?)
        } else {
            let prepared = prepare_claim_redemption_collateral_v2(
                liabilities.bound,
                collateral_request,
                TransferAuthorityV2 {
                    address: CollateralId::from_bytes(
                        accounts[bearer_ix::HOARD_AUTHORITY].key.to_bytes(),
                    ),
                    kind: TransferAuthorityKindV2::ProgramDerived,
                    is_transaction_signer: false,
                    program_address_authenticated: true,
                    is_writable: accounts[bearer_ix::HOARD_AUTHORITY].is_writable,
                    executable: accounts[bearer_ix::HOARD_AUTHORITY].executable,
                    data_is_empty: accounts[bearer_ix::HOARD_AUTHORITY].data_is_empty(),
                },
                runtime_account_view(&accounts[bearer_ix::COLLATERAL_MINT], &mint_data),
                runtime_account_view(&accounts[bearer_ix::HOARD_TOKEN], &hoard_data),
                runtime_account_view(&accounts[bearer_ix::DESTINATION], &destination_data),
            )
            .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
            drop((mint_data, hoard_data, destination_data));
            let bump = [seeds::hoard_authority_v2_pda(program_id, &market_bytes).1];
            let signer: [&[u8]; 3] = [seeds::SEED_HOARD_AUTHORITY_V2, &market_bytes, &bump];
            AcceptedBearerRedemptionCollateralV3::Nonzero(invoke_claim_collateral_payout(
                prepared,
                &accounts[bearer_ix::COLLATERAL_MINT],
                &accounts[bearer_ix::HOARD_TOKEN],
                &accounts[bearer_ix::DESTINATION],
                &accounts[bearer_ix::HOARD_AUTHORITY],
                &accounts[bearer_ix::COLLATERAL_TOKEN_PROGRAM],
                &signer,
            )?)
        }
    };
    let runtime_execution = bind_fractional_collateral_execution_v2(
        accepted_burn.fractional().transition_id(),
        value_authority.receipt_id,
        claim.receipt_id(),
        accepted_burn.burn_receipt_id(),
        collateral_delta_receipt_id(collateral),
    )?;
    let plan = finish_bearer_credit_v1(burned, collateral).map_err(map_fractional)?;
    let credit_after = plan
        .credit_after
        .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
    let RedemptionSourcePoststateV1::Bearer(source_after) = plan.source_after else {
        return Err(ClutchError::MismatchedState.into());
    };
    require(
        credit_after.claimant == intent.claimant
            && source_after.transition_id.bytes()
                == accepted_burn.fractional().transition_id().bytes()
            && runtime_execution.transition_id == accepted_burn.fractional().transition_id()
            && !runtime_execution.receipt_id.is_zero()
            && source_after.burn_receipt_id.map(Identity32V1::bytes)
                == Some(accepted_burn.burn_receipt_id().bytes()),
        ClutchError::MismatchedState,
    )?;
    apply_credit_funding(
        program_id,
        accounts,
        credit_index,
        accounts[bearer_ix::FRACTIONAL_POLICY].key.to_bytes(),
        intent.claimant.bytes(),
        funding,
    )?;
    accounts[credit_index]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(&credit_after.encode().map_err(map_fractional)?);
    accounts[bearer_ix::FRACTIONAL_LEDGER]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(&plan.ledger_after.encode().map_err(map_fractional)?);
    plan.custody_after
        .hoard_after()
        .encode(
            &mut accounts[bearer_ix::HOARD]
                .try_borrow_mut_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    plan.custody_after
        .fractional()
        .claim_ledger_after()
        .encode(
            &mut accounts[bearer_ix::CLAIM_LEDGER]
                .try_borrow_mut_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok(())
}

#[derive(Clone, Copy, Debug)]
struct CreditMoveGeometryV1 {
    root: usize,
    neutral: usize,
    rent: usize,
    funding: usize,
}

fn require_credit_move_contract(
    accounts: &[AccountInfo<'_>],
    payout_kind: u8,
    destination_mode: u8,
) -> Outcome<CreditMoveGeometryV1> {
    let creation = matches!(destination_mode, 2 | 3);
    let (root, neutral, rent) = match payout_kind {
        1 => (18, 19, 20),
        2 => (21, 22, 23),
        _ => return Err(ClutchError::MismatchedState.into()),
    };
    let funding = rent + 1;
    let expected = funding + if creation { CREDIT_CREATION_SUFFIX_ACCOUNTS_V1 } else { 0 };
    require_count(accounts, expected)?;
    let payer_alias_source =
        creation && accounts[move_ix::SOURCE_CLAIMANT].key == accounts[funding].key;
    let payer_alias_destination =
        creation && accounts[move_ix::DESTINATION_CLAIMANT].key == accounts[funding].key;
    let mut index = 0usize;
    while index < accounts.len() {
        let payout_writable = if payout_kind == 1 {
            matches!(index, 16 | 17)
        } else {
            matches!(index, 17 | 19)
        };
        let expected_writable = matches!(
            index,
            move_ix::HOARD
                | move_ix::CLAIM_LEDGER
                | move_ix::FRACTIONAL_LEDGER
                | move_ix::SOURCE_CREDIT
                | move_ix::DESTINATION_CREDIT
        ) || payout_writable
            || (creation && index == funding)
            || (index == move_ix::SOURCE_CLAIMANT && payer_alias_source)
            || (index == move_ix::DESTINATION_CLAIMANT && payer_alias_destination);
        require(
            accounts[index].is_writable == expected_writable,
            if expected_writable {
                ClutchError::NotWritable
            } else {
                ClutchError::UnexpectedWritable
            },
        )?;
        let expected_signer = index == move_ix::SOURCE_CLAIMANT
            || index == move_ix::DESTINATION_CLAIMANT
            || (creation && index == funding);
        require(
            accounts[index].is_signer == expected_signer,
            ClutchError::MismatchedState,
        )?;
        let mut other = index + 1;
        while other < accounts.len() {
            let payer_alias = creation
                && other == funding
                && matches!(index, move_ix::SOURCE_CLAIMANT | move_ix::DESTINATION_CLAIMANT);
            if !payer_alias {
                require(accounts[index].key != accounts[other].key, ClutchError::AccountAlias)?;
            }
            other += 1;
        }
        index += 1;
    }
    Ok(CreditMoveGeometryV1 {
        root,
        neutral,
        rent,
        funding,
    })
}

#[inline(never)]
fn process_credit_move(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    envelope_sequence: u64,
    action: FractionalRedemptionActionV1,
    intent: FractionalTransferIntentV1,
) -> Outcome<()> {
    let geometry =
        require_credit_move_contract(accounts, intent.payout_kind, intent.destination_mode)?;
    require(
        envelope_sequence == intent.expected_ledger_sequence
            && accounts[move_ix::SOURCE_CLAIMANT].key.to_bytes()
                == intent.source_claimant.bytes()
            && accounts[move_ix::DESTINATION_CLAIMANT].key.to_bytes()
                == intent.destination_claimant.bytes()
            && accounts[move_ix::SOURCE_CREDIT].key.to_bytes() == intent.source_credit.bytes()
            && accounts[move_ix::DESTINATION_CREDIT].key.to_bytes()
                == intent.destination_credit.bytes()
            && ((action == FractionalRedemptionActionV1::TransferCredit
                && intent.numerator != 0)
                || (action == FractionalRedemptionActionV1::MergeCredit
                    && intent.numerator == 0))
            && ((intent.payout_kind == 1 && intent.expected_payout_replay_sequence != 0)
                || (intent.payout_kind == 2 && intent.expected_payout_replay_sequence == 0)),
        ClutchError::MismatchedState,
    )?;
    let (liabilities, collateral_release_receipt) = if intent.payout_kind == 2 {
        let value_authority = authenticate_general_market_value_authority_v2(
            program_id,
            &accounts[move_ix::REALM],
            &accounts[move_ix::PROFILE],
            &accounts[move_ix::COLLATERAL_POLICY],
            &accounts[move_ix::COLLATERAL_TOKEN_PROGRAM],
            &accounts[move_ix::PAYOUT + 4],
            &accounts[move_ix::MARKET_BINDING],
            &accounts[move_ix::MARKET_RUNTIME],
            &accounts[move_ix::MARKET_INSTANCE],
            &accounts[move_ix::HOARD],
            &accounts[move_ix::CLAIM_LEDGER],
            true,
            true,
        )?;
        (value_authority.liabilities, Some(value_authority.receipt_id))
    } else {
        (
            authenticate_general_market_liabilities_v2(
                program_id,
                &accounts[move_ix::REALM],
                &accounts[move_ix::PROFILE],
                &accounts[move_ix::COLLATERAL_POLICY],
                &accounts[move_ix::COLLATERAL_TOKEN_PROGRAM],
                &accounts[move_ix::MARKET_BINDING],
                &accounts[move_ix::MARKET_RUNTIME],
                &accounts[move_ix::MARKET_INSTANCE],
                &accounts[move_ix::HOARD],
                &accounts[move_ix::CLAIM_LEDGER],
                true,
                true,
            )?,
            None,
        )
    };
    let resolution =
        authenticate_resolution_v5(program_id, &accounts[move_ix::RESOLUTION], liabilities)?;
    let (policy, ledger) = decode_fractional_accounts(
        program_id,
        accounts,
        move_ix::FRACTIONAL_POLICY,
        move_ix::FRACTIONAL_LEDGER,
        move_ix::RESOLUTION,
    )?;
    require(
        policy.market_instance.bytes() == liabilities.market_binding.base().market_instance_v2_id.bytes()
            && policy.resolution_account.bytes() == resolution.account_id.bytes()
            && policy.resolution_data_id.bytes() == resolution.data_id.bytes()
            && ledger.claim_ledger_account.bytes()
                == accounts[move_ix::CLAIM_LEDGER].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let root = authenticate_market_lifecycle_root_v1(
        program_id,
        &accounts[geometry.root],
        liabilities.market_binding.base().market_instance_v2_id,
        policy.domain_generation,
        false,
    )?;
    let neutral = root.state().capital().neutral_lamport_sink;
    require(
        accounts[geometry.neutral].key.to_bytes() == neutral.bytes()
            && accounts[geometry.neutral].owner == &SYSTEM_PROGRAM_ID
            && accounts[geometry.neutral].data_is_empty()
            && !accounts[geometry.neutral].executable,
        ClutchError::MismatchedState,
    )?;
    let policy_account = identity32(accounts[move_ix::FRACTIONAL_POLICY].key.to_bytes())?;
    let ledger_account = identity32(accounts[move_ix::FRACTIONAL_LEDGER].key.to_bytes())?;
    let (source_prestate, source_funding) = prepare_credit_prestate(
        program_id,
        accounts,
        move_ix::SOURCE_CREDIT,
        geometry.rent,
        geometry.funding,
        1,
        policy_account,
        ledger_account,
        intent.source_claimant,
        identity32(neutral.bytes())?,
    )?;
    let CreditPrestateV1::Live(source_credit) = source_prestate else {
        return Err(ClutchError::MismatchedState.into());
    };
    require(
        matches!(source_funding, CreditFundingAdmissionV1::Live),
        ClutchError::MismatchedState,
    )?;
    let (destination_prestate, destination_funding) = prepare_credit_prestate(
        program_id,
        accounts,
        move_ix::DESTINATION_CREDIT,
        geometry.rent,
        geometry.funding,
        intent.destination_mode,
        policy_account,
        ledger_account,
        intent.destination_claimant,
        identity32(neutral.bytes())?,
    )?;
    let context = bind_fractional_internal_context_v1(
        policy_account,
        policy,
        ledger_account,
        ledger,
        identity32(accounts[move_ix::CLAIM_LEDGER].key.to_bytes())?,
        liabilities.claim_ledger,
        liabilities.hoard,
        resolution.resolution,
        liabilities.bound,
    )
    .map_err(map_fractional)?;

    let (plan, runtime_execution) = if intent.payout_kind == 1 {
        require(
            accounts[move_ix::PAYOUT].key.to_bytes() == intent.payout_target.bytes(),
            ClutchError::MismatchedState,
        )?;
        let position = authenticate_general_position_replay_v2(
            program_id,
            liabilities.bound,
            &accounts[move_ix::MARKET_BINDING],
            &accounts[move_ix::MARKET_RUNTIME],
            &accounts[move_ix::PAYOUT],
            &accounts[move_ix::PAYOUT + 1],
            intent.destination_claimant.bytes(),
            intent.expected_payout_replay_sequence,
        )?;
        let target = CreditPayoutTargetV1::Internal {
            position: InternalPositionV1 {
                position_replay: position.replay,
            },
            expected_replay_sequence: intent.expected_payout_replay_sequence,
        };
        if action == FractionalRedemptionActionV1::TransferCredit {
            transfer_credit_v1(
                context,
                intent.expected_ledger_sequence,
                source_credit,
                intent.expected_source_sequence,
                destination_prestate,
                intent.destination_claimant,
                intent.expected_destination_sequence,
                intent.numerator,
                target,
            )
        } else {
            merge_credit_v1(
                context,
                intent.expected_ledger_sequence,
                source_credit,
                intent.expected_source_sequence,
                destination_prestate,
                intent.destination_claimant,
                intent.expected_destination_sequence,
                target,
            )
        }
        .map(|plan| (plan, None))
        .map_err(map_fractional)?
    } else {
        let collateral_release_receipt = collateral_release_receipt
            .ok_or(Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
        require(
            accounts[move_ix::PAYOUT + 1].key.to_bytes() == intent.payout_target.bytes()
                && accounts[move_ix::PAYOUT].key.to_bytes()
                    == liabilities.bound.policy().mint.bytes()
                && accounts[move_ix::PAYOUT + 3].key.to_bytes()
                    == liabilities.hoard.token_account.bytes()
                && accounts[move_ix::PAYOUT + 2].key.to_bytes()
                    == liabilities.hoard.authority.bytes()
                && !accounts[move_ix::PAYOUT + 2].executable
                && accounts[move_ix::PAYOUT + 2].data_is_empty(),
            ClutchError::MismatchedState,
        )?;
        let market_bytes = liabilities.market_binding.base().market_instance_v2_id.bytes();
        expect_pda(
            accounts[move_ix::PAYOUT + 2].key,
            seeds::hoard_authority_v2_pda(program_id, &market_bytes),
            None,
        )?;
        expect_pda(
            accounts[move_ix::PAYOUT + 3].key,
            seeds::hoard_token_v2_pda(program_id, &market_bytes),
            None,
        )?;
        let prepared = if action == FractionalRedemptionActionV1::TransferCredit {
            prepare_external_credit_transfer_v1(
                context,
                intent.expected_ledger_sequence,
                source_credit,
                intent.expected_source_sequence,
                destination_prestate,
                intent.destination_claimant,
                intent.expected_destination_sequence,
                intent.numerator,
                intent.payout_target,
            )
        } else {
            prepare_external_credit_merge_v1(
                context,
                intent.expected_ledger_sequence,
                source_credit,
                intent.expected_source_sequence,
                destination_prestate,
                intent.destination_claimant,
                intent.expected_destination_sequence,
                intent.payout_target,
            )
        }
        .map_err(map_fractional)?;
        let request = prepared.collateral_request();
        let collateral = {
            let mint_data = accounts[move_ix::PAYOUT]
                .try_borrow_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
            let hoard_data = accounts[move_ix::PAYOUT + 3]
                .try_borrow_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
            let destination_data = accounts[move_ix::PAYOUT + 1]
                .try_borrow_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
            if request.payout_atoms == 0 {
                let prepared = prepare_zero_claim_redemption_collateral_v2(
                    liabilities.bound,
                    request,
                    runtime_account_view(&accounts[move_ix::PAYOUT], &mint_data),
                    runtime_account_view(&accounts[move_ix::PAYOUT + 3], &hoard_data),
                    runtime_account_view(&accounts[move_ix::PAYOUT + 1], &destination_data),
                )
                .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
                drop((mint_data, hoard_data, destination_data));
                AcceptedBearerRedemptionCollateralV3::Zero(
                    accept_zero_claim_collateral_payout(
                        prepared,
                        &accounts[move_ix::PAYOUT],
                        &accounts[move_ix::PAYOUT + 3],
                        &accounts[move_ix::PAYOUT + 1],
                    )?,
                )
            } else {
                let prepared = prepare_claim_redemption_collateral_v2(
                    liabilities.bound,
                    request,
                    TransferAuthorityV2 {
                        address: CollateralId::from_bytes(
                            accounts[move_ix::PAYOUT + 2].key.to_bytes(),
                        ),
                        kind: TransferAuthorityKindV2::ProgramDerived,
                        is_transaction_signer: false,
                        program_address_authenticated: true,
                        is_writable: accounts[move_ix::PAYOUT + 2].is_writable,
                        executable: accounts[move_ix::PAYOUT + 2].executable,
                        data_is_empty: accounts[move_ix::PAYOUT + 2].data_is_empty(),
                    },
                    runtime_account_view(&accounts[move_ix::PAYOUT], &mint_data),
                    runtime_account_view(&accounts[move_ix::PAYOUT + 3], &hoard_data),
                    runtime_account_view(&accounts[move_ix::PAYOUT + 1], &destination_data),
                )
                .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
                drop((mint_data, hoard_data, destination_data));
                let bump = [seeds::hoard_authority_v2_pda(program_id, &market_bytes).1];
                let signer: [&[u8]; 3] =
                    [seeds::SEED_HOARD_AUTHORITY_V2, &market_bytes, &bump];
                AcceptedBearerRedemptionCollateralV3::Nonzero(
                    invoke_claim_collateral_payout(
                        prepared,
                        &accounts[move_ix::PAYOUT],
                        &accounts[move_ix::PAYOUT + 3],
                        &accounts[move_ix::PAYOUT + 1],
                        &accounts[move_ix::PAYOUT + 2],
                        &accounts[move_ix::COLLATERAL_TOKEN_PROGRAM],
                        &signer,
                    )?,
                )
            }
        };
        let plan = finish_external_credit_transfer_v1(prepared, collateral)
            .map_err(map_fractional)?;
        let runtime_execution = bind_fractional_collateral_execution_v2(
            plan.custody_after.fractional().transition_id(),
            collateral_release_receipt,
            CollateralId::ZERO,
            CollateralId::ZERO,
            plan.custody_after.receipt_id(),
        )?;
        (plan, Some(runtime_execution))
    };

    require(
        plan.source_after.claimant == intent.source_claimant
            && plan.destination_after.claimant == intent.destination_claimant
            && plan.custody_after.payout_atoms() == plan.paid_atoms
            && match runtime_execution {
                None => intent.payout_kind == 1,
                Some(execution) => {
                    intent.payout_kind == 2
                        && execution.transition_id
                            == plan.custody_after.fractional().transition_id()
                        && !execution.receipt_id.is_zero()
                }
            },
        ClutchError::MismatchedState,
    )?;
    apply_credit_funding(
        program_id,
        accounts,
        move_ix::DESTINATION_CREDIT,
        accounts[move_ix::FRACTIONAL_POLICY].key.to_bytes(),
        intent.destination_claimant.bytes(),
        destination_funding,
    )?;
    accounts[move_ix::SOURCE_CREDIT]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(&plan.source_after.encode().map_err(map_fractional)?);
    accounts[move_ix::DESTINATION_CREDIT]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(&plan.destination_after.encode().map_err(map_fractional)?);
    accounts[move_ix::FRACTIONAL_LEDGER]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(&plan.ledger_after.encode().map_err(map_fractional)?);
    plan.custody_after
        .hoard_after()
        .encode(
            &mut accounts[move_ix::HOARD]
                .try_borrow_mut_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    plan.custody_after
        .fractional()
        .claim_ledger_after()
        .encode(
            &mut accounts[move_ix::CLAIM_LEDGER]
                .try_borrow_mut_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    match plan.payout_after {
        CreditPayoutPoststateV1::Internal(internal) if intent.payout_kind == 1 => {
            accounts[move_ix::PAYOUT]
                .try_borrow_mut_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
                .copy_from_slice(
                    &internal
                        .position_after
                        .encode()
                        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
                );
            accounts[move_ix::PAYOUT + 1]
                .try_borrow_mut_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
                .copy_from_slice(internal.replay.replay_poststate_body());
        }
        CreditPayoutPoststateV1::External {
            claimant,
            collateral_hoard,
            collateral_destination,
            payout_atoms,
        } if intent.payout_kind == 2 => {
            require(
                claimant == intent.destination_claimant
                    && collateral_hoard.bytes()
                        == accounts[move_ix::PAYOUT + 3].key.to_bytes()
                    && collateral_destination == intent.payout_target
                    && payout_atoms == plan.paid_atoms,
                ClutchError::MismatchedState,
            )?;
        }
        _ => return Err(ClutchError::MismatchedState.into()),
    }
    Ok(())
}

fn require_close_credit_contract(accounts: &[AccountInfo<'_>]) -> Outcome<()> {
    require_count(accounts, close_credit_ix::COUNT)?;
    let payer_alias = accounts[close_credit_ix::CLAIMANT].key == accounts[close_credit_ix::PAYER].key;
    let mut index = 0usize;
    while index < accounts.len() {
        let expected_writable = matches!(
            index,
            close_credit_ix::CLAIM_LEDGER
                | close_credit_ix::FRACTIONAL_LEDGER
                | close_credit_ix::CREDIT
                | close_credit_ix::PAYER
                | close_credit_ix::NEUTRAL
        ) || (index == close_credit_ix::CLAIMANT && payer_alias);
        require(
            accounts[index].is_writable == expected_writable,
            if expected_writable {
                ClutchError::NotWritable
            } else {
                ClutchError::UnexpectedWritable
            },
        )?;
        require(
            accounts[index].is_signer
                == (index == close_credit_ix::CLAIMANT
                    || (index == close_credit_ix::PAYER && payer_alias)),
            ClutchError::MismatchedState,
        )?;
        let mut other = index + 1;
        while other < accounts.len() {
            let allowed_payer_alias = index == close_credit_ix::CLAIMANT
                && other == close_credit_ix::PAYER;
            if !allowed_payer_alias {
                require(accounts[index].key != accounts[other].key, ClutchError::AccountAlias)?;
            }
            other += 1;
        }
        index += 1;
    }
    Ok(())
}

#[inline(never)]
fn process_close_zero_credit(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    envelope_sequence: u64,
    intent: FractionalCloseCreditIntentV1,
) -> Outcome<()> {
    require_close_credit_contract(accounts)?;
    require(
        envelope_sequence == intent.expected_ledger_sequence
            && accounts[close_credit_ix::CLAIMANT].key.to_bytes() == intent.claimant.bytes()
            && accounts[close_credit_ix::CREDIT].key.to_bytes() == intent.credit_account.bytes()
            && !accounts[close_credit_ix::PAYER].executable,
        ClutchError::MismatchedState,
    )?;
    let liabilities = authenticate_general_market_liabilities_v2(
        program_id,
        &accounts[close_credit_ix::REALM],
        &accounts[close_credit_ix::PROFILE],
        &accounts[close_credit_ix::COLLATERAL_POLICY],
        &accounts[close_credit_ix::COLLATERAL_TOKEN_PROGRAM],
        &accounts[close_credit_ix::MARKET_BINDING],
        &accounts[close_credit_ix::MARKET_RUNTIME],
        &accounts[close_credit_ix::MARKET_INSTANCE],
        &accounts[close_credit_ix::HOARD],
        &accounts[close_credit_ix::CLAIM_LEDGER],
        false,
        true,
    )?;
    let resolution = authenticate_resolution_v5(
        program_id,
        &accounts[close_credit_ix::RESOLUTION],
        liabilities,
    )?;
    let (policy, ledger) = decode_fractional_accounts(
        program_id,
        accounts,
        close_credit_ix::FRACTIONAL_POLICY,
        close_credit_ix::FRACTIONAL_LEDGER,
        close_credit_ix::RESOLUTION,
    )?;
    require(
        policy.market_instance.bytes() == liabilities.market_binding.base().market_instance_v2_id.bytes()
            && policy.resolution_account.bytes() == resolution.account_id.bytes()
            && policy.resolution_data_id.bytes() == resolution.data_id.bytes()
            && ledger.claim_ledger_account.bytes()
                == accounts[close_credit_ix::CLAIM_LEDGER].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let root = authenticate_market_lifecycle_root_v1(
        program_id,
        &accounts[close_credit_ix::MARKET_ROOT],
        liabilities.market_binding.base().market_instance_v2_id,
        policy.domain_generation,
        false,
    )?;
    let neutral = root.state().capital().neutral_lamport_sink;
    require(
        accounts[close_credit_ix::NEUTRAL].key.to_bytes() == neutral.bytes()
            && accounts[close_credit_ix::NEUTRAL].owner == &SYSTEM_PROGRAM_ID
            && accounts[close_credit_ix::NEUTRAL].data_is_empty()
            && !accounts[close_credit_ix::NEUTRAL].executable,
        ClutchError::MismatchedState,
    )?;
    let policy_account = identity32(accounts[close_credit_ix::FRACTIONAL_POLICY].key.to_bytes())?;
    let ledger_account = identity32(accounts[close_credit_ix::FRACTIONAL_LEDGER].key.to_bytes())?;
    let (credit_prestate, funding) = prepare_credit_prestate(
        program_id,
        accounts,
        close_credit_ix::CREDIT,
        close_credit_ix::RENT,
        close_credit_ix::COUNT,
        1,
        policy_account,
        ledger_account,
        intent.claimant,
        identity32(neutral.bytes())?,
    )?;
    let CreditPrestateV1::Live(credit) = credit_prestate else {
        return Err(ClutchError::MismatchedState.into());
    };
    require(
        matches!(funding, CreditFundingAdmissionV1::Live)
            && accounts[close_credit_ix::PAYER].key.to_bytes() == credit.rent.payer.bytes(),
        ClutchError::MismatchedState,
    )?;
    let context = bind_fractional_internal_context_v1(
        policy_account,
        policy,
        ledger_account,
        ledger,
        identity32(accounts[close_credit_ix::CLAIM_LEDGER].key.to_bytes())?,
        liabilities.claim_ledger,
        liabilities.hoard,
        resolution.resolution,
        liabilities.bound,
    )
    .map_err(map_fractional)?;
    let plan = close_zero_credit_v1(
        context,
        intent.expected_ledger_sequence,
        credit,
        intent.expected_credit_sequence,
        accounts[close_credit_ix::CREDIT].lamports(),
        identity32(neutral.bytes())?,
    )
    .map_err(map_fractional)?;
    require(
        plan.funding.payer.bytes() == accounts[close_credit_ix::PAYER].key.to_bytes()
            && plan.funding.neutral_sink.bytes()
                == accounts[close_credit_ix::NEUTRAL].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let payer_after = accounts[close_credit_ix::PAYER]
        .lamports()
        .checked_add(plan.funding.payer_refund_lamports)
        .ok_or(ClutchError::Arithmetic)?;
    let neutral_after = accounts[close_credit_ix::NEUTRAL]
        .lamports()
        .checked_add(plan.funding.neutral_lamports)
        .ok_or(ClutchError::Arithmetic)?;
    accounts[close_credit_ix::CREDIT]
        .resize(FRACTIONAL_CREDIT_TOMBSTONE_BYTES)
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    accounts[close_credit_ix::CREDIT]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(&plan.tombstone.encode().map_err(map_fractional)?);
    {
        let mut credit_lamports = accounts[close_credit_ix::CREDIT]
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        **credit_lamports = plan.funding.tombstone_lamports;
    }
    {
        let mut payer_lamports = accounts[close_credit_ix::PAYER]
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        **payer_lamports = payer_after;
    }
    {
        let mut neutral_lamports = accounts[close_credit_ix::NEUTRAL]
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        **neutral_lamports = neutral_after;
    }
    require(
        accounts[close_credit_ix::CREDIT].lamports() == plan.funding.tombstone_lamports
            && accounts[close_credit_ix::PAYER].lamports() == payer_after
            && accounts[close_credit_ix::NEUTRAL].lamports() == neutral_after,
        ClutchError::MismatchedState,
    )?;
    accounts[close_credit_ix::FRACTIONAL_LEDGER]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(&plan.ledger_after.encode().map_err(map_fractional)?);
    plan.claim_ledger_after
        .claim_ledger_after()
        .encode(
            &mut accounts[close_credit_ix::CLAIM_LEDGER]
                .try_borrow_mut_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok(())
}

#[inline(never)]
fn process_seal_claims_exhausted(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    envelope_sequence: u64,
    intent: FractionalTerminalIntentV1,
) -> Outcome<()> {
    require_count(accounts, SEAL_CLAIMS_EXHAUSTED_ACCOUNT_COUNT_V1)?;
    require_distinct(accounts)?;
    require(
        envelope_sequence == intent.expected_ledger_sequence,
        ClutchError::Replay,
    )?;
    let mut index = 0usize;
    while index < accounts.len() {
        let expected_writable = matches!(index, seal_ix::CLAIM_LEDGER | seal_ix::FRACTIONAL_LEDGER);
        require(
            accounts[index].is_writable == expected_writable,
            if expected_writable {
                ClutchError::NotWritable
            } else {
                ClutchError::UnexpectedWritable
            },
        )?;
        require(!accounts[index].is_signer, ClutchError::MismatchedState)?;
        index += 1;
    }
    let liabilities = authenticate_general_market_liabilities_v2(
        program_id,
        &accounts[seal_ix::REALM],
        &accounts[seal_ix::PROFILE],
        &accounts[seal_ix::COLLATERAL_POLICY],
        &accounts[seal_ix::COLLATERAL_TOKEN_PROGRAM],
        &accounts[seal_ix::MARKET_BINDING],
        &accounts[seal_ix::MARKET_RUNTIME],
        &accounts[seal_ix::MARKET_INSTANCE],
        &accounts[seal_ix::HOARD],
        &accounts[seal_ix::CLAIM_LEDGER],
        false,
        true,
    )?;
    let resolution =
        authenticate_resolution_v5(program_id, &accounts[seal_ix::RESOLUTION], liabilities)?;
    let (policy, ledger) = decode_fractional_accounts(
        program_id,
        accounts,
        seal_ix::FRACTIONAL_POLICY,
        seal_ix::FRACTIONAL_LEDGER,
        seal_ix::RESOLUTION,
    )?;
    require(
        policy.market_instance.bytes() == liabilities.market_binding.base().market_instance_v2_id.bytes()
            && policy.resolution_account.bytes() == resolution.account_id.bytes()
            && policy.resolution_data_id.bytes() == resolution.data_id.bytes()
            && ledger.claim_ledger_account.bytes()
                == accounts[seal_ix::CLAIM_LEDGER].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let context = bind_fractional_internal_context_v1(
        Identity32V1::new(accounts[seal_ix::FRACTIONAL_POLICY].key.to_bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        policy,
        Identity32V1::new(accounts[seal_ix::FRACTIONAL_LEDGER].key.to_bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        ledger,
        Identity32V1::new(accounts[seal_ix::CLAIM_LEDGER].key.to_bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        liabilities.claim_ledger,
        liabilities.hoard,
        resolution.resolution,
        liabilities.bound,
    )
    .map_err(map_fractional)?;
    let plan = seal_claims_exhausted_v1(context, intent.expected_ledger_sequence)
        .map_err(map_fractional)?;
    require(
        plan.claim_ledger_after.consumed_sequence() == intent.expected_ledger_sequence,
        ClutchError::MismatchedState,
    )?;
    accounts[seal_ix::FRACTIONAL_LEDGER]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(&plan.ledger_after.encode().map_err(map_fractional)?);
    plan.claim_ledger_after
        .claim_ledger_after()
        .encode(
            &mut accounts[seal_ix::CLAIM_LEDGER]
                .try_borrow_mut_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok(())
}

const _: () = assert!(POSITION_V3_BYTES == 480);
