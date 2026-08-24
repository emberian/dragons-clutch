//! Pure, non-authority projections used by current Failure action 12.
//!
//! These functions own adapter-domain preimages which must be reproduced by
//! both the SBF writer and a chain-derived unsigned-transaction constructor.
//! A digest returned here proves no account ownership, PDA, lifecycle, or
//! write. The SBF adapter must mint its private authenticated capabilities only
//! after performing those checks and hostile-reopening every postimage.

use clutch_product_series::{ContentId, SERIES_FUNDING_COMPONENT_COUNT};

const MARKET_LIFECYCLE_ROOT_AUTHENTICATION_DOMAIN_V3: &[u8] =
    b"dragons-clutch/sbf/market-lifecycle-root-authentication/v3\0";
const SERIES_FAILURE_RELEASE_PREAUTHENTICATION_DOMAIN_V4: &[u8] =
    b"dragons-clutch/sbf/series-failure-release-preauthentication/v4\0";
const SERIES_FUNDING_AUTHENTICATION_DOMAIN_V5: &[u8] =
    b"dragons-clutch/series-funding-account-authentication/v5\0";
const INACTIVE_RESOLUTION_AUTHENTICATION_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/inactive-resolution-authentication/v5\0";
const GENERAL_MARKET_BINDING_DATA_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/general-market-binding/data/v5\0";
const GENERAL_MARKET_RUNTIME_DATA_DOMAIN_V3: &[u8] =
    b"dragons-clutch/general-market/runtime-data/v3\0";
const GENERAL_MARKET_NARROW_AUTHENTICATION_DOMAIN_V5: &[u8] =
    b"dragons-clutch/general-market/narrow-authentication/v5\0";
const GENERAL_MARKET_LIABILITY_AUTHORITY_DOMAIN_V5: &[u8] =
    b"dragons-clutch/general-market/liability-authority/v5\0";
const FAILURE_MARKET_RESOLUTION_FINALIZATION_EVIDENCE_DOMAIN_V5: &[u8] =
    b"dragons-clutch/failure-market-resolution-finalization-evidence/v5\0";
const MARKET_RESOLUTION_ACTIVATION_POSTWRITE_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/market-resolution/activation-postwrite/v5\0";
const FAILURE_MARKET_RESOLUTION_PHYSICAL_POSTWRITE_DOMAIN_V6: &[u8] =
    b"dragons-clutch/sbf/failure-market-resolution-physical-postwrite/v6\0";
const FAILURE_MARKET_RESOLUTION_ACTIVATION_AUTHENTICATION_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/failure-market-resolution-activation/v5\0";
const FAILURE_MARKET_INTERVAL_CELL_AUTHENTICATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/failure-market-interval-cell-account-authentication/v2";
const FAILURE_MARKET_RUNTIME_SESSION_POSTWRITE_DOMAIN_V3: &[u8] =
    b"dragons-clutch/sbf/failure-market-runtime-session-postwrite/v3\0";
const FAILURE_MARKET_RESOLUTION_POSTWRITE_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/failure-market-resolution-postwrite/v5\0";
const RESOLVED_DISPOSITION_BYTE_V2: u8 = 1;

/// Hash backend for deterministic action-12 projections.
pub trait FailureAction12ProjectionHashV1 {
    /// Hash the exact concatenation of `parts` with SHA-256.
    fn hashv(&self, parts: &[&[u8]]) -> [u8; 32];
}

fn id<H: FailureAction12ProjectionHashV1 + ?Sized>(hash: &H, parts: &[&[u8]]) -> ContentId {
    ContentId::from_bytes(hash.hashv(parts))
}

/// Project the private current RootV3 hostile-authentication identity.
#[allow(clippy::too_many_arguments)]
pub fn project_market_lifecycle_root_authentication_v3<
    H: FailureAction12ProjectionHashV1 + ?Sized,
>(
    hash: &H,
    account: [u8; 32],
    owner_program: [u8; 32],
    data_id: ContentId,
    semantic_id: ContentId,
    binding_id: ContentId,
    observed_lamports: u64,
    rent_principal_lamports: u64,
    stored_bump: u8,
    writable: bool,
) -> ContentId {
    id(
        hash,
        &[
            MARKET_LIFECYCLE_ROOT_AUTHENTICATION_DOMAIN_V3,
            &account,
            &owner_program,
            &data_id.bytes(),
            &semantic_id.bytes(),
            &binding_id.bytes(),
            &observed_lamports.to_le_bytes(),
            &rent_principal_lamports.to_le_bytes(),
            &[stored_bump, u8::from(writable)],
        ],
    )
}

/// Project Product's one-use current Failure-link release preauthorization.
#[allow(clippy::too_many_arguments)]
pub fn project_series_failure_release_preauthentication_v4<
    H: FailureAction12ProjectionHashV1 + ?Sized,
>(
    hash: &H,
    disposition_byte: u8,
    program_id: [u8; 32],
    root_account: [u8; 32],
    root_data_id: ContentId,
    root_authentication_id: ContentId,
    root_semantic_id: ContentId,
    root_binding_id: ContentId,
    link_account: [u8; 32],
    link_data_id: ContentId,
    link_authentication_id: ContentId,
    link_semantic_id: [u8; 32],
    link_binding_id: ContentId,
    transition_sequence: u64,
    failure_sessions_started: u64,
    failure_session_transcript_id: ContentId,
) -> ContentId {
    id(
        hash,
        &[
            SERIES_FAILURE_RELEASE_PREAUTHENTICATION_DOMAIN_V4,
            &[disposition_byte],
            &program_id,
            &root_account,
            &root_data_id.bytes(),
            &root_authentication_id.bytes(),
            &root_semantic_id.bytes(),
            &root_binding_id.bytes(),
            &link_account,
            &link_data_id.bytes(),
            &link_authentication_id.bytes(),
            &link_semantic_id,
            &link_binding_id.bytes(),
            &transition_sequence.to_le_bytes(),
            &failure_sessions_started.to_le_bytes(),
            &failure_session_transcript_id.bytes(),
        ],
    )
}

/// Project hostile authentication of one current FundingV5 account.
#[allow(clippy::too_many_arguments)]
pub fn project_series_funding_authentication_v5<H: FailureAction12ProjectionHashV1 + ?Sized>(
    hash: &H,
    account: [u8; 32],
    program_id: [u8; 32],
    data_id: ContentId,
    state_id: ContentId,
    rent_principal_lamports: u64,
    collateral_vault_rent_principal_lamports: [u64; SERIES_FUNDING_COMPONENT_COUNT],
    observed_lamports: u64,
    stored_bump: u8,
) -> ContentId {
    let mut vault_rent = [0u8; 8 * SERIES_FUNDING_COMPONENT_COUNT];
    for (index, principal) in collateral_vault_rent_principal_lamports.iter().enumerate() {
        let at = index * 8;
        vault_rent[at..at + 8].copy_from_slice(&principal.to_le_bytes());
    }
    id(
        hash,
        &[
            SERIES_FUNDING_AUTHENTICATION_DOMAIN_V5,
            &account,
            &program_id,
            &data_id.bytes(),
            &state_id.bytes(),
            &rent_principal_lamports.to_le_bytes(),
            &vault_rent,
            &observed_lamports.to_le_bytes(),
            &[stored_bump],
        ],
    )
}

/// Project hostile authentication of Product's inactive ResolutionV5 account.
#[allow(clippy::too_many_arguments)]
pub fn project_inactive_resolution_authentication_v5<
    H: FailureAction12ProjectionHashV1 + ?Sized,
>(
    hash: &H,
    program_id: [u8; 32],
    account: [u8; 32],
    semantic_id: ContentId,
    data_id: ContentId,
    observed_lamports: u64,
    market_instance_id: [u8; 32],
    native_claim_basis_id: ContentId,
    generation: u64,
    outcome_count: u8,
) -> ContentId {
    id(
        hash,
        &[
            INACTIVE_RESOLUTION_AUTHENTICATION_DOMAIN_V5,
            &program_id,
            &account,
            &semantic_id.bytes(),
            &data_id.bytes(),
            &observed_lamports.to_le_bytes(),
            &market_instance_id,
            &native_claim_basis_id.bytes(),
            &generation.to_le_bytes(),
            &[outcome_count],
        ],
    )
}

/// Project the account-bound General BindingV5 data identity.
pub fn project_general_market_binding_data_id_v5<H: FailureAction12ProjectionHashV1 + ?Sized>(
    hash: &H,
    account: [u8; 32],
    data: &[u8],
) -> ContentId {
    id(
        hash,
        &[GENERAL_MARKET_BINDING_DATA_DOMAIN_V5, &account, data],
    )
}

/// Project the account-bound General RuntimeV3 data identity.
pub fn project_general_market_runtime_data_id_v3<H: FailureAction12ProjectionHashV1 + ?Sized>(
    hash: &H,
    account: [u8; 32],
    data: &[u8],
) -> ContentId {
    id(
        hash,
        &[GENERAL_MARKET_RUNTIME_DATA_DOMAIN_V3, &account, data],
    )
}

/// Project the joined current General BindingV5/RuntimeV3 authentication.
#[allow(clippy::too_many_arguments)]
pub fn project_general_market_narrow_authentication_v5<
    H: FailureAction12ProjectionHashV1 + ?Sized,
>(
    hash: &H,
    program_id: [u8; 32],
    binding_account: [u8; 32],
    binding_data_id: ContentId,
    runtime_account: [u8; 32],
    runtime_data_id: ContentId,
) -> ContentId {
    id(
        hash,
        &[
            GENERAL_MARKET_NARROW_AUTHENTICATION_DOMAIN_V5,
            &program_id,
            &binding_account,
            &binding_data_id.bytes(),
            &runtime_account,
            &runtime_data_id.bytes(),
        ],
    )
}

/// Project the exact current General/collateral liability authority receipt.
#[allow(clippy::too_many_arguments)]
pub fn project_general_market_liability_authority_v5<
    H: FailureAction12ProjectionHashV1 + ?Sized,
>(
    hash: &H,
    authenticated_general_market_id: ContentId,
    market_instance_account: [u8; 32],
    market_instance_id: ContentId,
    hoard_account: [u8; 32],
    hoard_semantic_id: ContentId,
    hoard_lamports: u64,
    claim_ledger_account: [u8; 32],
    claim_ledger_semantic_id: ContentId,
    claim_ledger_lamports: u64,
    collateral_policy_id: ContentId,
    collateral_release_id: ContentId,
) -> ContentId {
    id(
        hash,
        &[
            GENERAL_MARKET_LIABILITY_AUTHORITY_DOMAIN_V5,
            &authenticated_general_market_id.bytes(),
            &market_instance_account,
            &market_instance_id.bytes(),
            &hoard_account,
            &hoard_semantic_id.bytes(),
            &hoard_lamports.to_le_bytes(),
            &claim_ledger_account,
            &claim_ledger_semantic_id.bytes(),
            &claim_ledger_lamports.to_le_bytes(),
            &collateral_policy_id.bytes(),
            &collateral_release_id.bytes(),
        ],
    )
}

/// Registry coordinates committed by action-12 finalization evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureAction12RegistryProjectionV5 {
    /// Series registry account.
    pub series_registry_account: [u8; 32],
    /// Checked registry program account.
    pub program_account: [u8; 32],
    /// Checked registry ProgramData account.
    pub programdata_account: [u8; 32],
    /// Checked RegistryRelease artifact account.
    pub release_artifact_account: [u8; 32],
    /// Checked CapabilityProfile artifact account.
    pub profile_artifact_account: [u8; 32],
    /// RegistryRelease semantic ID.
    pub registry_release_id: ContentId,
    /// CapabilityProfile semantic ID.
    pub capability_profile_id: ContentId,
}

/// Failure receipt coordinates committed by action-12 finalization evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureAction12ReceiptProjectionV5 {
    /// Failure resolution receipt ID.
    pub resolution_id: ContentId,
    /// Failure policy binding ID.
    pub failure_policy_binding_id: ContentId,
    /// Cell prestate ID.
    pub cell_before: ContentId,
    /// Cell resolved poststate ID.
    pub cell_after: ContentId,
    /// Active session binding.
    pub session_binding_id: ContentId,
    /// Successful Source handoff.
    pub source_handoff_id: ContentId,
    /// Terminal Product work ID.
    pub terminal_work_id: ContentId,
    /// Product exhaustive certificate ID.
    pub product_certificate_id: ContentId,
    /// Final liveness work receipt ID.
    pub last_runtime_work_receipt_id: ContentId,
    /// Completed liveness call count.
    pub completed_work_calls: u64,
    /// Exact reward already accounted by the cell.
    pub exact_reward_lamports: u64,
}

/// Complete non-authority preimage for action-12 finalization evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureAction12FinalizationEvidenceProjectionV5 {
    /// Root binding semantic ID.
    pub root_binding_id: ContentId,
    /// Root account and hostile authentication.
    pub root: ([u8; 32], ContentId),
    /// Series link account and hostile authentication.
    pub link: ([u8; 32], ContentId),
    /// One-use resolved-link preauthorization.
    pub link_release_id: ContentId,
    /// Series-link semantic ID.
    pub link_state_id: ContentId,
    /// Registry coordinates.
    pub registry: FailureAction12RegistryProjectionV5,
    /// Compiler bundle account and semantic ID.
    pub bundle: ([u8; 32], ContentId),
    /// Funding account, hostile authentication, and raw data ID.
    pub funding: ([u8; 32], ContentId, ContentId),
    /// Foundation schedule, graph, and transcript IDs.
    pub foundation: (ContentId, ContentId, ContentId),
    /// Inactive Resolution authentication, semantic ID, and data ID.
    pub inactive: (ContentId, ContentId, ContentId),
    /// Resolution rent principal, donation floor, and payer.
    pub inactive_rent: (u64, u64, [u8; 32]),
    /// Liability receipt, Hoard before/after, and ClaimLedger before/after IDs.
    pub liabilities: (ContentId, ContentId, ContentId, ContentId, ContentId),
    /// Complete Failure resolution receipt coordinates.
    pub failure: FailureAction12ReceiptProjectionV5,
    /// Resolution PDA account.
    pub resolution_account: [u8; 32],
    /// Active payout width.
    pub outcome_count: u8,
    /// Exact common payout denominator.
    pub denominator: u64,
    /// Full-width payout vector with canonical zero padding.
    pub weights: [u64; 16],
}

impl FailureAction12FinalizationEvidenceProjectionV5 {
    /// Project the exact finalization-evidence ID. This is not authority.
    pub fn id<H: FailureAction12ProjectionHashV1 + ?Sized>(&self, hash: &H) -> ContentId {
        let mut weights = [0u8; 128];
        for (index, weight) in self.weights.iter().enumerate() {
            let at = index * 8;
            weights[at..at + 8].copy_from_slice(&weight.to_le_bytes());
        }
        let registry = self.registry;
        let failure = self.failure;
        id(
            hash,
            &[
                FAILURE_MARKET_RESOLUTION_FINALIZATION_EVIDENCE_DOMAIN_V5,
                &self.root_binding_id.bytes(),
                &self.root.0,
                &self.root.1.bytes(),
                &self.link.0,
                &self.link.1.bytes(),
                &self.link_release_id.bytes(),
                &self.link_state_id.bytes(),
                &registry.series_registry_account,
                &registry.program_account,
                &registry.programdata_account,
                &registry.release_artifact_account,
                &registry.profile_artifact_account,
                &registry.registry_release_id.bytes(),
                &registry.capability_profile_id.bytes(),
                &self.bundle.0,
                &self.bundle.1.bytes(),
                &self.funding.0,
                &self.funding.1.bytes(),
                &self.funding.2.bytes(),
                &self.foundation.0.bytes(),
                &self.foundation.1.bytes(),
                &self.foundation.2.bytes(),
                &self.inactive.0.bytes(),
                &self.inactive.1.bytes(),
                &self.inactive.2.bytes(),
                &self.inactive_rent.0.to_le_bytes(),
                &self.inactive_rent.1.to_le_bytes(),
                &self.inactive_rent.2,
                &self.liabilities.0.bytes(),
                &self.liabilities.1.bytes(),
                &self.liabilities.2.bytes(),
                &self.liabilities.3.bytes(),
                &self.liabilities.4.bytes(),
                &failure.resolution_id.bytes(),
                &failure.failure_policy_binding_id.bytes(),
                &failure.cell_before.bytes(),
                &failure.cell_after.bytes(),
                &failure.session_binding_id.bytes(),
                &failure.source_handoff_id.bytes(),
                &failure.terminal_work_id.bytes(),
                &failure.product_certificate_id.bytes(),
                &failure.last_runtime_work_receipt_id.bytes(),
                &failure.completed_work_calls.to_le_bytes(),
                &failure.exact_reward_lamports.to_le_bytes(),
                &self.resolution_account,
                &[self.outcome_count],
                &self.denominator.to_le_bytes(),
                &weights,
                &[RESOLVED_DISPOSITION_BYTE_V2],
            ],
        )
    }
}

/// Project the collateral Resolution/Hoard/ClaimLedger postwrite receipt.
#[allow(clippy::too_many_arguments)]
pub fn project_market_resolution_activation_postwrite_v5<
    H: FailureAction12ProjectionHashV1 + ?Sized,
>(
    hash: &H,
    liability_receipt_id: ContentId,
    activation_plan_receipt_id: ContentId,
    resolution_account: [u8; 32],
    resolution_semantic_id: ContentId,
    resolution_data_id: ContentId,
    resolution_lamports: u64,
    hoard_account: [u8; 32],
    hoard_semantic_id: ContentId,
    hoard_lamports: u64,
    claim_ledger_account: [u8; 32],
    claim_ledger_semantic_id: ContentId,
    claim_ledger_lamports: u64,
) -> ContentId {
    id(
        hash,
        &[
            MARKET_RESOLUTION_ACTIVATION_POSTWRITE_DOMAIN_V5,
            &liability_receipt_id.bytes(),
            &activation_plan_receipt_id.bytes(),
            &resolution_account,
            &resolution_semantic_id.bytes(),
            &resolution_data_id.bytes(),
            &resolution_lamports.to_le_bytes(),
            &hoard_account,
            &hoard_semantic_id.bytes(),
            &hoard_lamports.to_le_bytes(),
            &claim_ledger_account,
            &claim_ledger_semantic_id.bytes(),
            &claim_ledger_lamports.to_le_bytes(),
        ],
    )
}

/// Project the exact physical inactive-to-finalized ResolutionV5 write.
#[allow(clippy::too_many_arguments)]
pub fn project_failure_resolution_physical_postwrite_v6<
    H: FailureAction12ProjectionHashV1 + ?Sized,
>(
    hash: &H,
    program_id: [u8; 32],
    resolution_account: [u8; 32],
    inactive_authentication_id: ContentId,
    inactive_semantic_id: ContentId,
    inactive_data_id: ContentId,
    resolution_semantic_id: ContentId,
    resolution_data_id: ContentId,
    collateral_postwrite_id: ContentId,
    inactive_observed_lamports: u64,
) -> ContentId {
    id(
        hash,
        &[
            FAILURE_MARKET_RESOLUTION_PHYSICAL_POSTWRITE_DOMAIN_V6,
            &program_id,
            &resolution_account,
            &inactive_authentication_id.bytes(),
            &inactive_semantic_id.bytes(),
            &inactive_data_id.bytes(),
            &resolution_semantic_id.bytes(),
            &resolution_data_id.bytes(),
            &collateral_postwrite_id.bytes(),
            &inactive_observed_lamports.to_le_bytes(),
        ],
    )
}

/// Project the private atomic Product/Collateral/Failure activation ID.
#[allow(clippy::too_many_arguments)]
pub fn project_failure_resolution_activation_v5<H: FailureAction12ProjectionHashV1 + ?Sized>(
    hash: &H,
    market_root_account: [u8; 32],
    root_authentication_before: ContentId,
    root_authentication_after: ContentId,
    series_link_account: [u8; 32],
    link_authentication_id: ContentId,
    link_release_id: ContentId,
    resolution_account: [u8; 32],
    inactive_authentication_id: ContentId,
    physical_postwrite_id: ContentId,
    failure_resolution_id: ContentId,
    product_certificate_id: ContentId,
    finalization_evidence_id: ContentId,
    product_activation_id: ContentId,
    collateral_plan_receipt_id: ContentId,
    collateral_postwrite_id: ContentId,
    inactive_rent_principal: u64,
    inactive_donation_floor: u64,
) -> ContentId {
    id(
        hash,
        &[
            FAILURE_MARKET_RESOLUTION_ACTIVATION_AUTHENTICATION_DOMAIN_V5,
            &market_root_account,
            &root_authentication_before.bytes(),
            &root_authentication_after.bytes(),
            &series_link_account,
            &link_authentication_id.bytes(),
            &link_release_id.bytes(),
            &resolution_account,
            &inactive_authentication_id.bytes(),
            &physical_postwrite_id.bytes(),
            &failure_resolution_id.bytes(),
            &product_certificate_id.bytes(),
            &finalization_evidence_id.bytes(),
            &product_activation_id.bytes(),
            &collateral_plan_receipt_id.bytes(),
            &collateral_postwrite_id.bytes(),
            &inactive_rent_principal.to_le_bytes(),
            &inactive_donation_floor.to_le_bytes(),
            &[RESOLVED_DISPOSITION_BYTE_V2],
        ],
    )
}

/// Project hostile authentication of the resolved Failure interval cell frame.
#[allow(clippy::too_many_arguments)]
pub fn project_failure_interval_cell_authentication_v2<
    H: FailureAction12ProjectionHashV1 + ?Sized,
>(
    hash: &H,
    cell_account: [u8; 32],
    owner_program: [u8; 32],
    framed_data_id: ContentId,
    cell_state_id: ContentId,
    admission_state_id: ContentId,
    observed_lamports: u64,
) -> ContentId {
    id(
        hash,
        &[
            FAILURE_MARKET_INTERVAL_CELL_AUTHENTICATION_DOMAIN_V2,
            &cell_account,
            &owner_program,
            &framed_data_id.bytes(),
            &cell_state_id.bytes(),
            &admission_state_id.bytes(),
            &observed_lamports.to_le_bytes(),
        ],
    )
}

/// Project the current resolved Failure runtime physical-postwrite ID.
pub fn project_failure_runtime_session_postwrite_v3<H: FailureAction12ProjectionHashV1 + ?Sized>(
    hash: &H,
    runtime_account: [u8; 32],
    runtime_before: ContentId,
    runtime_after: ContentId,
    transition_receipt_id: ContentId,
    observed_lamports: u64,
) -> ContentId {
    id(
        hash,
        &[
            FAILURE_MARKET_RUNTIME_SESSION_POSTWRITE_DOMAIN_V3,
            &runtime_account,
            &runtime_before.bytes(),
            &runtime_after.bytes(),
            &transition_receipt_id.bytes(),
            &observed_lamports.to_le_bytes(),
        ],
    )
}

/// Final action-12 projection consumed by Source no-reopen terminal identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureAction12ResolutionProjectionV5 {
    failure_resolution_id: ContentId,
    postwrite_id: ContentId,
}

impl FailureAction12ResolutionProjectionV5 {
    /// Exact Failure interval resolution receipt ID.
    pub const fn failure_resolution_id(self) -> ContentId {
        self.failure_resolution_id
    }

    /// Exact final physical action-12 postwrite ID.
    pub const fn postwrite_id(self) -> ContentId {
        self.postwrite_id
    }
}

/// Project the final post-cell/post-runtime action-12 identity. This is not authority.
#[allow(clippy::too_many_arguments)]
pub fn project_failure_action12_resolution_v5<H: FailureAction12ProjectionHashV1 + ?Sized>(
    hash: &H,
    admission_root_account: [u8; 32],
    runtime_root_account: [u8; 32],
    market_root_account: [u8; 32],
    series_link_account: [u8; 32],
    interval_cell_account: [u8; 32],
    activation_id: ContentId,
    root_authentication_after: ContentId,
    cell_authentication_after: ContentId,
    cell_state_after: ContentId,
    runtime_postwrite_id: ContentId,
    runtime_transition_receipt_id: ContentId,
    resolution_semantic_id: ContentId,
    resolution_data_id: ContentId,
    resolution_account_id: ContentId,
    failure_resolution_id: ContentId,
) -> FailureAction12ResolutionProjectionV5 {
    let postwrite_id = id(
        hash,
        &[
            FAILURE_MARKET_RESOLUTION_POSTWRITE_DOMAIN_V5,
            &admission_root_account,
            &runtime_root_account,
            &market_root_account,
            &series_link_account,
            &interval_cell_account,
            &activation_id.bytes(),
            &root_authentication_after.bytes(),
            &cell_authentication_after.bytes(),
            &cell_state_after.bytes(),
            &runtime_postwrite_id.bytes(),
            &runtime_transition_receipt_id.bytes(),
            &resolution_semantic_id.bytes(),
            &resolution_data_id.bytes(),
            &resolution_account_id.bytes(),
        ],
    );
    FailureAction12ResolutionProjectionV5 {
        failure_resolution_id,
        postwrite_id,
    }
}
