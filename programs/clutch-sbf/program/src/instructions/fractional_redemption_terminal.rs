//! Capability-disabled Fractional/Product terminal composition seam.
//!
//! This module freezes the private receipts and exact joins needed for
//! Fractional action 10 without adding a dispatch route. The current capability
//! manifest contains no `79/v1` tuple, so
//! [`authenticate_fractional_runtime_release_v1`] always refuses before a
//! terminal receipt can exist. Activation must add the reviewed release tuple
//! and replace this preparation-only seam with one adapter function that writes
//! ClaimLedger, closes both a4/a5 accounts, distributes only their rent lamports,
//! and consumes Product terminality in the same Solana instruction.

use crate::accounts::{require, Outcome};
use crate::capabilities;
use crate::error::{ClutchError, Refusal};
use crate::instructions::product_artifact::AuthenticatedRegistryCapabilityV2;
use crate::instructions::product_occurrence::{
    mint_product_occurrence_family_terminal_v1,
    AuthenticatedProductOccurrenceFamilyCloseAuthorizationV1,
    AuthenticatedProductOccurrenceFamilyTerminalV1, AuthenticatedProductOccurrenceRootV1,
};
use clutch_fractional_redemption_runtime::{
    EmptyLedgerClosePlanV1, FractionalRedemptionActionV1, FRACTIONAL_REDEMPTION_FAMILY_TAG,
    FRACTIONAL_REDEMPTION_FAMILY_VERSION,
};
use clutch_product_series::{ContentId, MarketInstanceV2Id, ProductOccurrenceFamilyV1};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

const FRACTIONAL_RUNTIME_RELEASE_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/fractional-runtime-release-authentication/v1";
const FRACTIONAL_PRODUCT_TERMINAL_RECEIPT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/fractional-product-terminal-receipt/v1";

/// Loader- and artifact-authenticated Fractional runtime release.
///
/// Private fields prevent a caller from naming an ELF or manifest ID. The only
/// constructor also requires the exact action-10 capability tuple, which is not
/// present in any current profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedFractionalRuntimeReleaseV1 {
    id: ContentId,
    program_account: Pubkey,
    programdata_account: Pubkey,
    series_registry_account: Pubkey,
    series_plan_id: clutch_product_series::SeriesPlanV5Id,
    release_id: ContentId,
    capability_profile_id: ContentId,
}

impl AuthenticatedFractionalRuntimeReleaseV1 {
    /// Domain-separated authentication receipt identity.
    pub const fn id(self) -> ContentId {
        self.id
    }

    /// Exact currently executing program.
    pub const fn program_account(self) -> Pubkey {
        self.program_account
    }

    /// Exact loader-linked ProgramData account.
    pub const fn programdata_account(self) -> Pubkey {
        self.programdata_account
    }

    /// Exact SeriesRegistry which selected the release and profile.
    pub const fn series_registry_account(self) -> Pubkey {
        self.series_registry_account
    }

    /// Exact recurring Series selected by that registry.
    pub const fn series_plan_id(self) -> clutch_product_series::SeriesPlanV5Id {
        self.series_plan_id
    }

    /// Content identity of the complete loader-bound program release.
    pub const fn release_id(self) -> ContentId {
        self.release_id
    }

    /// Exact capability-profile content identity.
    pub const fn capability_profile_id(self) -> ContentId {
        self.capability_profile_id
    }
}

/// Authenticate the exact loader/release/profile tuple for Fractional close.
///
/// Every current capability profile fails the first check. Keeping the check
/// here makes the Product preauthorization the only remaining composition seam
/// once a future release explicitly admits action 10.
pub fn authenticate_fractional_runtime_release_v1(
    registry: AuthenticatedRegistryCapabilityV2,
) -> Outcome<AuthenticatedFractionalRuntimeReleaseV1> {
    require(
        capabilities::extension_intent_action_enabled(
            FRACTIONAL_REDEMPTION_FAMILY_TAG,
            FRACTIONAL_REDEMPTION_FAMILY_VERSION,
            FractionalRedemptionActionV1::CloseEmptyLedger.tag(),
        ),
        ClutchError::AuthorizationUnavailable,
    )?;
    let release_id = registry
        .release()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .content_id();
    let capability_profile_id = registry.capability_profile_id();
    let family = [FRACTIONAL_REDEMPTION_FAMILY_TAG];
    let version = [FRACTIONAL_REDEMPTION_FAMILY_VERSION];
    let action = [FractionalRedemptionActionV1::CloseEmptyLedger.tag()];
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            FRACTIONAL_RUNTIME_RELEASE_AUTHENTICATION_DOMAIN_V1,
            registry.program_account().as_ref(),
            registry.programdata_account().as_ref(),
            registry.series_registry_account().as_ref(),
            &registry.series_plan_id().bytes(),
            &release_id.bytes(),
            &capability_profile_id.bytes(),
            &family,
            &version,
            &action,
        ])
        .to_bytes(),
    );
    Ok(AuthenticatedFractionalRuntimeReleaseV1 {
        id,
        program_account: registry.program_account(),
        programdata_account: registry.programdata_account(),
        series_registry_account: registry.series_registry_account(),
        series_plan_id: registry.series_plan_id(),
        release_id,
        capability_profile_id,
    })
}

/// Private Fractional terminal receipt consumed only by Product composition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedFractionalDomainTerminalReceiptV1 {
    id: ContentId,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    policy_account: Pubkey,
    ledger_account: Pubkey,
    resolution_account: Pubkey,
    resolution_semantic_id: ContentId,
    resolution_data_id: ContentId,
    native_claim_basis_id: ContentId,
    policy_terminal_state_id: ContentId,
    ledger_terminal_state_id: ContentId,
    claim_ledger_post_state_id: ContentId,
    claim_ledger_transition_id: ContentId,
    release_id: ContentId,
}

impl AuthenticatedFractionalDomainTerminalReceiptV1 {
    /// Exact non-decodable receipt identity.
    pub const fn id(self) -> ContentId {
        self.id
    }

    /// Full-width MarketInstanceV2 identity.
    pub const fn market_instance_id(self) -> MarketInstanceV2Id {
        self.market_instance_id
    }

    /// Exact occurrence/fractional generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Physical closed a4 policy account.
    pub const fn policy_account(self) -> Pubkey {
        self.policy_account
    }

    /// Physical closed a5 aggregate-ledger account.
    pub const fn ledger_account(self) -> Pubkey {
        self.ledger_account
    }

    /// Physical canonical Resolution V5 account bound to the closed domain.
    pub const fn resolution_account(self) -> Pubkey {
        self.resolution_account
    }

    /// Body-only identity of the exact final Resolution V5 state.
    pub const fn resolution_semantic_id(self) -> ContentId {
        self.resolution_semantic_id
    }

    /// PDA-and-body identity of the exact final Resolution V5 state.
    pub const fn resolution_data_id(self) -> ContentId {
        self.resolution_data_id
    }

    /// Canonical NativeClaimBasis jointly bound by Resolution and ClaimLedger.
    pub const fn native_claim_basis_id(self) -> ContentId {
        self.native_claim_basis_id
    }

    /// Exact immutable a4 terminal state identity.
    pub const fn policy_terminal_state_id(self) -> ContentId {
        self.policy_terminal_state_id
    }

    /// Exact transient a5 terminal successor identity.
    pub const fn ledger_terminal_state_id(self) -> ContentId {
        self.ledger_terminal_state_id
    }

    /// Exact Retiring ClaimLedger V3 semantic identity.
    pub const fn claim_ledger_post_state_id(self) -> ContentId {
        self.claim_ledger_post_state_id
    }

    /// Shared ClaimLedger/a5 terminal transition identity.
    pub const fn claim_ledger_transition_id(self) -> ContentId {
        self.claim_ledger_transition_id
    }

    /// Exact current Fractional runtime release identity.
    pub const fn release_id(self) -> ContentId {
        self.release_id
    }
}

/// Private preparation joining Fractional's exact dual close to Product 0xaa.
///
/// This function has no dispatch route and cannot be reached in any current
/// profile because its `release` argument cannot be minted. It deliberately
/// does not mutate accounts; a future activation must perform every write,
/// lamport split, and Product consume inside one non-returning execution path.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_fractional_product_terminal_v1(
    root: AuthenticatedProductOccurrenceRootV1,
    authorization: AuthenticatedProductOccurrenceFamilyCloseAuthorizationV1,
    release: AuthenticatedFractionalRuntimeReleaseV1,
    close: EmptyLedgerClosePlanV1,
    owner_program_id: &Pubkey,
    policy_account: &AccountInfo<'_>,
    ledger_account: &AccountInfo<'_>,
) -> Outcome<(
    AuthenticatedFractionalDomainTerminalReceiptV1,
    AuthenticatedProductOccurrenceFamilyTerminalV1,
)> {
    let requirement = close.terminal_requirement();
    let binding = authorization.binding();
    let policy_funding = close.policy_funding();
    let ledger_funding = close.ledger_funding();
    let policy_total = policy_funding
        .payer_refund_lamports()
        .checked_add(policy_funding.neutral_lamports())
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let ledger_total = ledger_funding
        .payer_refund_lamports()
        .checked_add(ledger_funding.neutral_lamports())
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        authorization.root_account() == root.account()
            && authorization.family() == ProductOccurrenceFamilyV1::Fractional
            && release.program_account() == *owner_program_id
            && release.series_plan_id() == binding.series_plan_id
            && release.release_id() == binding.registry_release_id
            && release.capability_profile_id() == binding.capability_profile_id
            && requirement.market_instance_id().bytes() == binding.market_instance_id.bytes()
            && requirement.domain_generation() == binding.generation
            && requirement.resolution_account().bytes() == binding.resolution_account_id.bytes()
            && requirement.native_claim_basis_id().bytes() == binding.native_claim_basis_id.bytes()
            && policy_account.key.to_bytes() == requirement.policy_account().bytes()
            && ledger_account.key.to_bytes() == requirement.ledger_account().bytes()
            && policy_account.key != ledger_account.key
            && policy_account.owner == owner_program_id
            && ledger_account.owner == owner_program_id
            && policy_account.is_writable
            && ledger_account.is_writable
            && !policy_account.is_signer
            && !ledger_account.is_signer
            && !policy_account.executable
            && !ledger_account.executable
            && policy_account.lamports() == policy_total
            && ledger_account.lamports() == ledger_total
            && policy_funding.neutral_sink() == ledger_funding.neutral_sink(),
        ClutchError::MismatchedState,
    )?;
    let market_instance_id =
        MarketInstanceV2Id::from_bytes(requirement.market_instance_id().bytes());
    let resolution_account = Pubkey::new_from_array(requirement.resolution_account().bytes());
    let resolution_semantic_id =
        ContentId::from_bytes(requirement.resolution_semantic_id().bytes());
    let resolution_data_id = ContentId::from_bytes(requirement.resolution_data_id().bytes());
    let native_claim_basis_id = ContentId::from_bytes(requirement.native_claim_basis_id().bytes());
    let policy_terminal_state_id =
        ContentId::from_bytes(requirement.policy_terminal_state_id().bytes());
    let ledger_terminal_state_id =
        ContentId::from_bytes(requirement.ledger_terminal_state_id().bytes());
    let claim_ledger_post_state_id =
        ContentId::from_bytes(requirement.claim_ledger_post_state_id().bytes());
    let claim_ledger_transition_id =
        ContentId::from_bytes(requirement.claim_ledger_transition_id().bytes());
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            FRACTIONAL_PRODUCT_TERMINAL_RECEIPT_DOMAIN_V1,
            &authorization.id().bytes(),
            &authorization.root_semantic_id().bytes(),
            &release.id().bytes(),
            &release.release_id().bytes(),
            &market_instance_id.bytes(),
            &requirement.domain_generation().to_le_bytes(),
            resolution_account.as_ref(),
            &resolution_semantic_id.bytes(),
            &resolution_data_id.bytes(),
            &native_claim_basis_id.bytes(),
            policy_account.key.as_ref(),
            ledger_account.key.as_ref(),
            &policy_terminal_state_id.bytes(),
            &ContentId::from_bytes(requirement.ledger_before_state_id().bytes()).bytes(),
            &ledger_terminal_state_id.bytes(),
            &ContentId::from_bytes(requirement.claim_ledger_account().bytes()).bytes(),
            &claim_ledger_post_state_id.bytes(),
            &claim_ledger_transition_id.bytes(),
            &policy_funding.payer().bytes(),
            &policy_funding.payer_refund_lamports().to_le_bytes(),
            &policy_funding.neutral_lamports().to_le_bytes(),
            &ledger_funding.payer().bytes(),
            &ledger_funding.payer_refund_lamports().to_le_bytes(),
            &ledger_funding.neutral_lamports().to_le_bytes(),
            &policy_funding.neutral_sink().bytes(),
        ])
        .to_bytes(),
    );
    let receipt = AuthenticatedFractionalDomainTerminalReceiptV1 {
        id,
        market_instance_id,
        generation: requirement.domain_generation(),
        policy_account: *policy_account.key,
        ledger_account: *ledger_account.key,
        resolution_account,
        resolution_semantic_id,
        resolution_data_id,
        native_claim_basis_id,
        policy_terminal_state_id,
        ledger_terminal_state_id,
        claim_ledger_post_state_id,
        claim_ledger_transition_id,
        release_id: release.release_id(),
    };
    let product_terminal = mint_product_occurrence_family_terminal_v1(
        root,
        authorization,
        ContentId::from_bytes(owner_program_id.to_bytes()),
        release.release_id(),
        policy_account,
        receipt.id(),
        [policy_terminal_state_id, ledger_terminal_state_id],
    )?;
    Ok((receipt, product_terminal))
}
