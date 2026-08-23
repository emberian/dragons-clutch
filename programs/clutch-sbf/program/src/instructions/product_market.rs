//! Authenticated SBF boundary for the shared Product Market lifecycle.
//!
//! The pure Product crate owns deterministic state. This module owns hostile
//! account decoding, exact `0xaa/1` and `0xad/1` PDA/owner/full-body checks,
//! atomic state writes, and private non-decodable terminal authority. Merely
//! compiling these helpers does not enable an instruction route.

use crate::accounts::{expect_pda, require, require_distinct, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::{
    allocate_data, assign_data, read_rent, require_system_program, transfer_data, SYSTEM_PROGRAM_ID,
};
use crate::instructions::product_artifact::{
    authenticate_product_artifact_v1, AuthenticatedRegistryCapabilityV3,
};
use crate::seeds;
use clutch_liveness::runtime_adapter_v1::{
    decode_runtime_policy_account_v1, RuntimePersistedAccountViewV1,
};
use clutch_liveness::runtime_v1::RuntimeCompartmentKindV1;
use clutch_liveness::Id as LivenessId;
use clutch_product_series::{
    CompiledProductSeriesBundleV5, ContentId, MarketFoundationAccountGraphV2,
    MarketFoundationScheduleV2, MarketFoundationSlotV2, MarketFoundationStepProjectionV2,
    MarketFoundingAbortProjectionV1, MarketInstanceTerminalProjectionV1, MarketInstanceV2Id,
    MarketLifecyclePhaseV1, MarketLifecycleReplayReceiptV1, MarketLifecycleRootV1,
    MarketResolutionActivationV1, SeriesAttachmentPlanV4, SeriesFundingComponentV2,
    SeriesFundingQuoteV4,
    SeriesLinkObligationAdmissionProjectionV1, SeriesLinkObligationDispositionV1,
    SeriesLinkObligationStatusV1, SeriesLinkObligationTerminalProjectionV1,
    SeriesLinkObligationV1, SeriesMarketDispositionV1, SeriesMarketLinkPhaseV1,
    SeriesMarketLinkV1, SeriesMarketLinkV1Id, SeriesPlanV5Id,
};
use clutch_solana_layout::failure_recovery::{
    decode_failure_account_body_v1, FAILURE_LIVENESS_POLICY_ACCOUNT_BYTES_V1,
    FAILURE_LIVENESS_POLICY_BODY_BYTES_V1,
};
use clutch_solana_layout::product_series::{
    series_market_link_authentication_id_v1, MarketLifecycleReplayAccountV1,
    MarketLifecycleRootAccountV1, SeriesMarketLinkAccountV1,
    MARKET_LIFECYCLE_REPLAY_ACCOUNT_BYTES_V1, MARKET_LIFECYCLE_ROOT_ACCOUNT_BYTES_V1,
    SERIES_MARKET_LINK_ACCOUNT_BYTES_V1,
};
use clutch_solana_layout::registry;
use clutch_source_plane_v3_runtime::AuthenticatedClockBucketV1;
use solana_account_info::AccountInfo;
use solana_cpi::invoke_signed;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

const MARKET_LIFECYCLE_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/market-lifecycle-account-authentication/v1";
const MARKET_INSTANCE_TERMINAL_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/market-instance-terminal-authentication/v1";
const SERIES_WRAPPER_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/series-wrapper-authentication/v1";
const SERIES_WRAPPER_TERMINAL_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/series-wrapper-terminal-authentication/v1";
const MARKET_RECOVERY_SCHEDULE_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/market-recovery-schedule-authentication/v1";
const MARKET_FOUNDATION_DEBIT_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/market-foundation-debit-authentication/v1";
const MARKET_FOUNDATION_PREALLOCATION_AUTHENTICATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/market-foundation-preallocation-authentication/v2";
const MARKET_FOUNDATION_VAULT_ABORT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/market-foundation-vault-abort/v1";
const MARKET_FOUNDATION_VAULT_TERMINAL_DOMAIN_V1: &[u8] =
    b"dragons-clutch/market-foundation-vault-terminal/v1";
const MARKET_LIFECYCLE_REPLAY_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/market-lifecycle-replay-authentication/v1";
const MARKET_LIFECYCLE_ROOT_CLOSE_DOMAIN_V1: &[u8] =
    b"dragons-clutch/market-lifecycle-root-close/v1";
const MARKET_LIFECYCLE_ABORT_CLOSE_DOMAIN_V1: &[u8] =
    b"dragons-clutch/market-lifecycle-abort-close/v1";

/// Authenticate the fixed Product/General/Failure core of one immutable
/// foundation account graph against the executing program's canonical PDAs.
///
/// Fractional policy/ledger, outcome mints, and outcome custody remain owned
/// by their respective typed founding receipts because their PDA preimages
/// include identities not repeated by the Product root. This helper must not
/// guess those identities. It does make the common core non-caller-shaped,
/// including the reusable V2 Failure interval cell and append-only history.
fn require_canonical_market_foundation_core_v2(
    program_id: &Pubkey,
    root_account: Pubkey,
    account_graph: &MarketFoundationAccountGraphV2,
) -> Outcome<()> {
    let market = account_graph.market_instance_id.bytes();
    let generation = account_graph.generation;
    let market_binding = account_graph
        .account(MarketFoundationSlotV2::MarketBinding)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let fixed = [
        (
            MarketFoundationSlotV2::LifecycleRoot,
            seeds::product_market_lifecycle_root_pda(program_id, &market, generation).0,
        ),
        (
            MarketFoundationSlotV2::MarketBinding,
            seeds::general_v2_market_binding_pda(program_id, &market).0,
        ),
        (
            MarketFoundationSlotV2::MarketRuntime,
            seeds::general_v2_market_runtime_pda(program_id, &market_binding.bytes()).0,
        ),
        (
            MarketFoundationSlotV2::Hoard,
            seeds::hoard_v2_pda(program_id, &market).0,
        ),
        (
            MarketFoundationSlotV2::ClaimLedger,
            seeds::claim_ledger_v3_pda(program_id, &market).0,
        ),
        (
            MarketFoundationSlotV2::FailureAdmissionRoot,
            seeds::failure_market_root_v2_pda(program_id, &market, generation).0,
        ),
        (
            MarketFoundationSlotV2::FailureRuntimeRoot,
            seeds::failure_external_root_pda(program_id, &market, generation).0,
        ),
        (
            MarketFoundationSlotV2::FailureReplay,
            seeds::failure_market_replay_v2_pda(program_id, &market, generation).0,
        ),
        (
            MarketFoundationSlotV2::FailureIntervalWork,
            seeds::failure_market_interval_cell_v2_pda(program_id, &market, generation).0,
        ),
        (
            MarketFoundationSlotV2::FailureIntervalHistory,
            seeds::failure_market_interval_history_v2_pda(program_id, &market, generation).0,
        ),
        (
            MarketFoundationSlotV2::ResolutionV5,
            seeds::resolution_v5_pda(program_id, &market).0,
        ),
        (
            MarketFoundationSlotV2::ProductReplayAnchor,
            seeds::product_market_lifecycle_replay_pda(program_id, &market, generation).0,
        ),
    ];
    require(
        account_graph
            .account(MarketFoundationSlotV2::LifecycleRoot)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            .bytes()
            == root_account.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    for (slot, expected) in fixed {
        require(
            account_graph
                .account(slot)
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                .bytes()
                == expected.to_bytes(),
            ClutchError::MismatchedState,
        )?;
    }
    Ok(())
}

/// Exact authenticated shared `0xaa/1` account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedMarketLifecycleRootV1<'state> {
    account: Pubkey,
    owner_program: Pubkey,
    value: &'state MarketLifecycleRootAccountV1,
    observed_lamports: u64,
    writable: bool,
    data_id: ContentId,
    authentication_id: ContentId,
}

impl<'state> AuthenticatedMarketLifecycleRootV1<'state> {
    /// Physical root account.
    pub const fn account(self) -> Pubkey {
        self.account
    }
    /// Program which authenticated and owns the account.
    pub const fn owner_program(self) -> Pubkey {
        self.owner_program
    }

    /// Complete hostile-decoded account value.
    pub const fn value(self) -> &'state MarketLifecycleRootAccountV1 {
        self.value
    }

    /// Complete pure Market lifecycle state.
    pub const fn state(self) -> &'state MarketLifecycleRootV1 {
        &self.value.state
    }

    /// Exact lamports observed with the authenticated bytes.
    pub const fn observed_lamports(self) -> u64 {
        self.observed_lamports
    }

    /// Whether the outer message granted writable privilege.
    pub const fn is_writable(self) -> bool {
        self.writable
    }

    /// SHA-256 of the exact framed account bytes.
    pub const fn data_id(self) -> ContentId {
        self.data_id
    }

    /// Account/PDA/body/rent authentication identity.
    pub const fn authentication_id(self) -> ContentId {
        self.authentication_id
    }

    /// Exact persisted refundable root rent principal.
    pub const fn rent_principal_lamports(self) -> u64 {
        self.value.rent_principal_lamports
    }
}

/// Private hostile authentication of one permanent compact `0xb0/1` anchor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedMarketLifecycleReplayV1 {
    account: Pubkey,
    value: MarketLifecycleReplayAccountV1,
    observed_lamports: u64,
    data_id: ContentId,
    authentication_id: ContentId,
}

impl AuthenticatedMarketLifecycleReplayV1 {
    /// Canonical replay account.
    pub const fn account(self) -> Pubkey {
        self.account
    }
    /// Complete Product-owned semantic receipt.
    pub const fn receipt(self) -> MarketLifecycleReplayReceiptV1 {
        self.value.receipt
    }
    /// Exact permanent anchor principal.
    pub const fn permanent_rent_principal_lamports(self) -> u64 {
        self.value.permanent_rent_principal_lamports
    }
    /// Full hostile account authentication identity.
    pub const fn authentication_id(self) -> ContentId {
        self.authentication_id
    }
}

/// Exact authenticated per-Series `0xad/1` link account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedSeriesMarketLinkV1<'state> {
    account: Pubkey,
    owner_program: Pubkey,
    value: &'state SeriesMarketLinkAccountV1,
    observed_lamports: u64,
    writable: bool,
    data_id: ContentId,
    authentication_id: ContentId,
}

impl<'state> AuthenticatedSeriesMarketLinkV1<'state> {
    /// Physical link account.
    pub const fn account(self) -> Pubkey {
        self.account
    }

    /// Program which authenticated and owns the account.
    pub const fn owner_program(self) -> Pubkey {
        self.owner_program
    }

    /// Complete hostile-decoded account value.
    pub const fn value(self) -> &'state SeriesMarketLinkAccountV1 {
        self.value
    }

    /// Complete pure link state.
    pub const fn state(self) -> &'state SeriesMarketLinkV1 {
        &self.value.state
    }

    /// Exact lamports observed with the authenticated bytes.
    pub const fn observed_lamports(self) -> u64 {
        self.observed_lamports
    }

    /// Whether the outer message granted writable privilege.
    pub const fn is_writable(self) -> bool {
        self.writable
    }

    /// SHA-256 of the exact framed account bytes.
    pub const fn data_id(self) -> ContentId {
        self.data_id
    }

    /// Account/PDA/body/rent authentication identity.
    pub const fn authentication_id(self) -> ContentId {
        self.authentication_id
    }
}

/// Private proof that one exact quote-owned FoundationVault principal was
/// transferred into one canonical still-unallocated foundation account.
///
/// This receipt is not a family poststate. A family owner must consume it in
/// the same atomic instruction, allocate/write/authenticate its account, and
/// return a private accepted-poststate receipt before Product may advance the
/// root bitmap and transcript.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedMarketFoundationDebitV1 {
    id: ContentId,
    root_account: Pubkey,
    root_authentication_id: ContentId,
    market_binding_id: ContentId,
    failure_policy_binding_id: ContentId,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    founder_link_id: SeriesMarketLinkV1Id,
    funding_quote_id: ContentId,
    foundation_schedule_id: ContentId,
    foundation_account_graph_id: ContentId,
    slot: MarketFoundationSlotV2,
    root_transition_sequence: u64,
    foundation_vault: Pubkey,
    destination: Pubkey,
    principal_lamports: u64,
    principal_before_lamports: u64,
    principal_after_lamports: u64,
    vault_donation_lamports: u64,
    destination_donation_floor_lamports: u64,
    destination_observed_balance_lamports: u64,
    rent_refund_owner: Pubkey,
    neutral_lamport_sink: Pubkey,
}

impl AuthenticatedMarketFoundationDebitV1 {
    /// Unique exact debit authorization identity.
    pub const fn id(self) -> ContentId {
        self.id
    }
    /// Shared Market root authenticated before the debit.
    pub const fn root_account(self) -> Pubkey {
        self.root_account
    }
    /// Full hostile root authentication identity before the debit.
    pub const fn root_authentication_id(self) -> ContentId {
        self.root_authentication_id
    }
    /// Immutable shared Market binding.
    pub const fn market_binding_id(self) -> ContentId {
        self.market_binding_id
    }
    /// Exact immutable market-scoped Failure policy binding.
    pub const fn failure_policy_binding_id(self) -> ContentId {
        self.failure_policy_binding_id
    }
    /// Full-width shared Market.
    pub const fn market_instance_id(self) -> MarketInstanceV2Id {
        self.market_instance_id
    }
    /// Shared Market generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }
    /// Founder link which supplied the sole MarketCore allocation.
    pub const fn founder_link_id(self) -> SeriesMarketLinkV1Id {
        self.founder_link_id
    }
    /// Exact authenticated QuoteV4 artifact.
    pub const fn funding_quote_id(self) -> ContentId {
        self.funding_quote_id
    }
    /// Exact quote-owned itemization.
    pub const fn foundation_schedule_id(self) -> ContentId {
        self.foundation_schedule_id
    }
    /// Exact pairwise-distinct physical account graph.
    pub const fn foundation_account_graph_id(self) -> ContentId {
        self.foundation_account_graph_id
    }
    /// Canonical slot debited exactly once.
    pub const fn slot(self) -> MarketFoundationSlotV2 {
        self.slot
    }
    /// Root sequence the accepted poststate must commit.
    pub const fn root_transition_sequence(self) -> u64 {
        self.root_transition_sequence
    }
    /// Canonical zero-data FoundationVault.
    pub const fn foundation_vault(self) -> Pubkey {
        self.foundation_vault
    }
    /// Exact slot account funded by the debit.
    pub const fn destination(self) -> Pubkey {
        self.destination
    }
    /// Exact refundable principal transferred.
    pub const fn principal_lamports(self) -> u64 {
        self.principal_lamports
    }
    /// Foundation principal before the transfer.
    pub const fn principal_before_lamports(self) -> u64 {
        self.principal_before_lamports
    }
    /// Foundation principal after the transfer.
    pub const fn principal_after_lamports(self) -> u64 {
        self.principal_after_lamports
    }
    /// FoundationVault donation residue, unchanged by the transfer.
    pub const fn vault_donation_lamports(self) -> u64 {
        self.vault_donation_lamports
    }
    /// Lamports present at the destination before capitalization.
    pub const fn destination_donation_floor_lamports(self) -> u64 {
        self.destination_donation_floor_lamports
    }
    /// Exact destination balance after capitalization.
    pub const fn destination_observed_balance_lamports(self) -> u64 {
        self.destination_observed_balance_lamports
    }
    /// Immutable principal refund recipient.
    pub const fn rent_refund_owner(self) -> Pubkey {
        self.rent_refund_owner
    }
    /// System-owned destination for unsolicited lamports.
    pub const fn neutral_lamport_sink(self) -> Pubkey {
        self.neutral_lamport_sink
    }

    fn projection(
        self,
        accepted_poststate_receipt_id: ContentId,
    ) -> MarketFoundationStepProjectionV2 {
        MarketFoundationStepProjectionV2 {
            binding_id: self.market_binding_id,
            slot: self.slot,
            root_transition_sequence: self.root_transition_sequence,
            principal_lamports: self.principal_lamports,
            principal_before_lamports: self.principal_before_lamports,
            principal_after_lamports: self.principal_after_lamports,
            donation_before_lamports: self.vault_donation_lamports,
            donation_after_lamports: self.vault_donation_lamports,
            account_id: ContentId::from_bytes(self.destination.to_bytes()),
            accepted_poststate_receipt_id,
        }
    }
}

/// Private reauthentication of one retained zero-data Foundation preallocation.
///
/// The shared root's initialized bitmap and transcript prove the quote-owned
/// principal was debited exactly once during Founding. This receipt additionally
/// authenticates the still-unclaimed physical PDA, present principal, and all
/// unsolicited surplus immediately before a family allocates/writes it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedMarketFoundationPreallocationV2 {
    id: ContentId,
    root_account: Pubkey,
    root_authentication_id: ContentId,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    slot: MarketFoundationSlotV2,
    account: Pubkey,
    foundation_schedule_id: ContentId,
    foundation_account_graph_id: ContentId,
    foundation_transcript_id: ContentId,
    principal_lamports: u64,
    donation_lamports: u64,
    observed_balance_lamports: u64,
    rent_refund_owner: Pubkey,
    neutral_lamport_sink: Pubkey,
}

impl AuthenticatedMarketFoundationPreallocationV2 {
    /// Exact complete preallocation authentication identity.
    pub const fn id(self) -> ContentId {
        self.id
    }
    /// Authenticated shared Product root.
    pub const fn root_account(self) -> Pubkey {
        self.root_account
    }
    /// Full hostile root authentication identity.
    pub const fn root_authentication_id(self) -> ContentId {
        self.root_authentication_id
    }
    /// Full-width Market.
    pub const fn market_instance_id(self) -> MarketInstanceV2Id {
        self.market_instance_id
    }
    /// Shared generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }
    /// Exact canonical foundation slot.
    pub const fn slot(self) -> MarketFoundationSlotV2 {
        self.slot
    }
    /// Exact retained zero-data PDA.
    pub const fn account(self) -> Pubkey {
        self.account
    }
    /// Exact quote-owned foundation schedule.
    pub const fn foundation_schedule_id(self) -> ContentId {
        self.foundation_schedule_id
    }
    /// Exact canonical physical account graph.
    pub const fn foundation_account_graph_id(self) -> ContentId {
        self.foundation_account_graph_id
    }
    /// Root's ordered transcript after this preallocation was accepted.
    pub const fn foundation_transcript_id(self) -> ContentId {
        self.foundation_transcript_id
    }
    /// Exact separately itemized refundable principal.
    pub const fn principal_lamports(self) -> u64 {
        self.principal_lamports
    }
    /// Unsolicited lamports excluded from principal.
    pub const fn donation_lamports(self) -> u64 {
        self.donation_lamports
    }
    /// Exact present balance at authentication.
    pub const fn observed_balance_lamports(self) -> u64 {
        self.observed_balance_lamports
    }
    /// Immutable principal/rent refund owner.
    pub const fn rent_refund_owner(self) -> Pubkey {
        self.rent_refund_owner
    }
    /// Immutable unsolicited-lamport sink.
    pub const fn neutral_lamport_sink(self) -> Pubkey {
        self.neutral_lamport_sink
    }
}

/// Private exact disposition of the zero-data FoundationVault.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedMarketFoundationVaultDispositionV1 {
    id: ContentId,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    foundation_vault: Pubkey,
    refund_owner: Pubkey,
    neutral_lamport_sink: Pubkey,
    principal_lamports: u64,
    donation_lamports: u64,
    observed_balance_before: u64,
}

/// Private exact physical close of one fully unwound inert Market root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedMarketFoundingAbortCloseV1 {
    id: ContentId,
    root_account: Pubkey,
    root_authentication_id: ContentId,
    abort_projection: MarketFoundingAbortProjectionV1,
    root_rent_principal_lamports: u64,
    root_surplus_lamports: u64,
    refund_owner: Pubkey,
    neutral_lamport_sink: Pubkey,
}

impl AuthenticatedMarketFoundingAbortCloseV1 {
    /// Exact authenticated physical-close identity.
    pub const fn id(self) -> ContentId {
        self.id
    }
    /// Deleted `0xaa/1` account.
    pub const fn root_account(self) -> Pubkey {
        self.root_account
    }
    /// Full pre-close root authentication identity.
    pub const fn root_authentication_id(self) -> ContentId {
        self.root_authentication_id
    }
    /// Pure abort projection which authorizes ordinal restoration.
    pub const fn abort_projection(self) -> MarketFoundingAbortProjectionV1 {
        self.abort_projection
    }
    /// Exact refundable payer-owned root principal.
    pub const fn root_rent_principal_lamports(self) -> u64 {
        self.root_rent_principal_lamports
    }
    /// All unsolicited root lamports sent only to the neutral sink.
    pub const fn root_surplus_lamports(self) -> u64 {
        self.root_surplus_lamports
    }
    /// Immutable principal refund owner.
    pub const fn refund_owner(self) -> Pubkey {
        self.refund_owner
    }
    /// Immutable surplus sink.
    pub const fn neutral_lamport_sink(self) -> Pubkey {
        self.neutral_lamport_sink
    }
}

impl AuthenticatedMarketFoundationVaultDispositionV1 {
    /// Exact disposition receipt identity.
    pub const fn id(self) -> ContentId {
        self.id
    }
    /// Full-width Market.
    pub const fn market_instance_id(self) -> MarketInstanceV2Id {
        self.market_instance_id
    }
    /// Shared generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }
    /// Canonical drained FoundationVault.
    pub const fn foundation_vault(self) -> Pubkey {
        self.foundation_vault
    }
    /// Exact refunded principal.
    pub const fn principal_lamports(self) -> u64 {
        self.principal_lamports
    }
    /// Exact surplus sent to the neutral sink.
    pub const fn donation_lamports(self) -> u64 {
        self.donation_lamports
    }
}

#[cfg(feature = "non-production-failure-recovery-lab")]
impl clutch_failure_policy_runtime::market_runtime_v1::AuthenticatedFailureMarketRuntimeAdmissionV1
    for AuthenticatedMarketFoundationDebitV1
{
    fn authenticate_failure_market_runtime_admission(
        &self,
        expected: clutch_failure_policy_runtime::market_runtime_v1::FailureMarketRuntimeAdmissionFactsV1,
    ) -> clutch_failure_policy_runtime::Result<()> {
        let funding = expected.root_funding;
        if self.slot != MarketFoundationSlotV2::FailureRuntimeRoot
            || expected.failure_policy_binding_id.bytes() != self.failure_policy_binding_id.bytes()
            || expected.market_instance_id != self.market_instance_id
            || expected.generation != self.generation
            || expected.runtime_account_id.bytes() != self.destination.to_bytes()
            || expected.foundation_receipt_id != self.id
            || funding.rent_refund_owner.bytes() != self.rent_refund_owner.to_bytes()
            || funding.neutral_sink.bytes() != self.neutral_lamport_sink.to_bytes()
            || funding.rent_principal_lamports != self.principal_lamports
            || funding.donation_floor_lamports != self.destination_donation_floor_lamports
            || funding.observed_balance_lamports != self.destination_observed_balance_lamports
            || expected
                .admission_state_id
                .bytes()
                .iter()
                .all(|byte| *byte == 0)
            || expected
                .recovery_funding_receipt_id
                .bytes()
                .iter()
                .all(|byte| *byte == 0)
            || expected
                .runtime_state_commitment
                .bytes()
                .iter()
                .all(|byte| *byte == 0)
        {
            return Err(clutch_failure_policy_runtime::Error::BindingMismatch);
        }
        Ok(())
    }
}

/// Private full-body authority for the market-scoped Recovery reward schedule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedMarketRecoveryScheduleV1 {
    id: ContentId,
    market_root_account: Pubkey,
    market_root_authentication_id: ContentId,
    series_link_account: Pubkey,
    series_link_authentication_id: ContentId,
    funding_quote_id: ContentId,
    liveness_policy_account: Pubkey,
    liveness_policy_id: ContentId,
    recovery_quote_schedule_id: ContentId,
    maximum_calls: u32,
    maximum_lamports_per_call: u64,
    work_capital_lamports: u64,
    account_rent_principal_lamports: u64,
    receipt_program_id: ContentId,
    capability_profile_id: ContentId,
    maximum_progress_units_per_call: u64,
}

impl AuthenticatedMarketRecoveryScheduleV1 {
    /// Exact private join identity.
    pub const fn id(self) -> ContentId {
        self.id
    }
    /// Shared Market root account.
    pub const fn market_root_account(self) -> Pubkey {
        self.market_root_account
    }
    /// Full hostile root-account authentication.
    pub const fn market_root_authentication_id(self) -> ContentId {
        self.market_root_authentication_id
    }
    /// Exact subordinate Series link.
    pub const fn series_link_account(self) -> Pubkey {
        self.series_link_account
    }
    /// Full hostile link-account authentication.
    pub const fn series_link_authentication_id(self) -> ContentId {
        self.series_link_authentication_id
    }
    /// Exact per-Series quote used only for funding provenance/local allocations.
    pub const fn funding_quote_id(self) -> ContentId {
        self.funding_quote_id
    }
    /// Physical immutable liveness-policy account.
    pub const fn liveness_policy_account(self) -> Pubkey {
        self.liveness_policy_account
    }
    /// Market-scoped liveness-policy identity.
    pub const fn liveness_policy_id(self) -> ContentId {
        self.liveness_policy_id
    }
    /// Sole Recovery-compartment reward schedule.
    pub const fn recovery_quote_schedule_id(self) -> ContentId {
        self.recovery_quote_schedule_id
    }
    /// Maximum bounded calls authorized by the policy.
    pub const fn maximum_calls(self) -> u32 {
        self.maximum_calls
    }
    /// Maximum lamports paid by one call.
    pub const fn maximum_lamports_per_call(self) -> u64 {
        self.maximum_lamports_per_call
    }
    /// Exact present work capital.
    pub const fn work_capital_lamports(self) -> u64 {
        self.work_capital_lamports
    }
    /// Exact present Recovery-account rent principal.
    pub const fn account_rent_principal_lamports(self) -> u64 {
        self.account_rent_principal_lamports
    }
    /// Program permitted to mint paid-work/terminal receipts.
    pub const fn receipt_program_id(self) -> ContentId {
        self.receipt_program_id
    }
    /// Current loader-authenticated central capability profile.
    pub const fn capability_profile_id(self) -> ContentId {
        self.capability_profile_id
    }
    /// Central maximum Recovery progress delta per paid call.
    pub const fn maximum_progress_units_per_call(self) -> u64 {
        self.maximum_progress_units_per_call
    }
}

/// Authenticate the existing liveness semantic owner against one exact shared
/// Market and subordinate Series quote. Later convergers may carry different
/// local allocations, but they cannot replace these shared policy/schedule
/// terms or debit the shared Recovery compartment again.
pub fn authenticate_market_recovery_schedule_v1(
    program_id: &Pubkey,
    root: AuthenticatedMarketLifecycleRootV1<'_>,
    link: AuthenticatedSeriesMarketLinkV1<'_>,
    capability: AuthenticatedRegistryCapabilityV3,
    funding_quote_account: &AccountInfo<'_>,
    liveness_policy_account: &AccountInfo<'_>,
) -> Outcome<AuthenticatedMarketRecoveryScheduleV1> {
    let root_binding = root.state().binding();
    let link_binding = link.state().binding();
    require(
        link_binding.market_instance_id == root_binding.market_instance_id
            && link_binding.generation == root_binding.generation
            && link_binding.market_root_account_id.bytes() == root.account().to_bytes(),
        ClutchError::MismatchedState,
    )?;
    require(
        capability.series_plan_id() == link_binding.series_plan_id
            && capability.capability_profile_id() == root_binding.capability_profile_id
            && capability.registry_release_id() == root_binding.registry_release_id
            && capability.program_account() == *program_id
            && capability
                .profile()
                .rules
                .maximum_recovery_progress_units_per_call
                != 0,
        ClutchError::MismatchedState,
    )?;
    let quote = authenticate_product_artifact_v1::<SeriesFundingQuoteV4>(
        program_id,
        funding_quote_account,
        link_binding.funding_quote_id.content_id(),
    )?;
    require(
        !liveness_policy_account.is_signer
            && !liveness_policy_account.is_writable
            && !liveness_policy_account.executable
            && liveness_policy_account.owner == program_id
            && liveness_policy_account.data_len() == FAILURE_LIVENESS_POLICY_ACCOUNT_BYTES_V1,
        ClutchError::MismatchedState,
    )?;
    let data = liveness_policy_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let frame = decode_failure_account_body_v1(
        &data,
        registry::FAILURE_LIVENESS_POLICY_ACCOUNT_TAG,
        registry::FAILURE_LIVENESS_POLICY_ACCOUNT_VERSION,
        FAILURE_LIVENESS_POLICY_BODY_BYTES_V1,
    )?;
    let policy = decode_runtime_policy_account_v1(
        liveness_id(program_id),
        liveness_id(liveness_policy_account.key),
        RuntimePersistedAccountViewV1 {
            account_id: liveness_id(liveness_policy_account.key),
            owner_program_id: liveness_id(liveness_policy_account.owner),
            lamports: liveness_policy_account.lamports(),
            data: frame.body,
            writable: false,
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let stored_bump = frame.stored_bump;
    let policy_data_id =
        ContentId::from_bytes(solana_sha256_hasher::hashv(&[&data[..]]).to_bytes());
    drop(data);
    expect_pda(
        liveness_policy_account.key,
        seeds::failure_liveness_policy_pda(program_id, &policy.policy_id.bytes()),
        Some(stored_bump),
    )?;
    let recovery = policy.compartments[RuntimeCompartmentKindV1::Recovery.index()];
    let quoted_recovery =
        quote.value().components[SeriesFundingComponentV2::RecoveryReserve.index()];
    let capability_programdata_account = capability.programdata_account();
    let capability_profile_account = capability.profile_artifact_account();
    let maximum_progress_units_per_call = capability
        .profile()
        .rules
        .maximum_recovery_progress_units_per_call;
    require(
        ContentId::from_bytes(policy.policy_id.bytes()) == root_binding.failure_liveness_policy_id
            && ContentId::from_bytes(policy.realm_id.bytes()) == root_binding.realm_id
            && ContentId::from_bytes(policy.neutral_sink.bytes())
                == root.state().capital().neutral_lamport_sink
            && quote.value().evidence_only_recovery_policy_id == root_binding.recovery_policy_id
            && quote.value().failure_liveness_policy_id == root_binding.failure_liveness_policy_id
            && quote.value().failure_recovery_quote_schedule_id
                == root_binding.failure_liveness_quote_schedule_id
            && ContentId::from_bytes(recovery.quote_schedule_id.bytes())
                == root_binding.failure_liveness_quote_schedule_id
            && ContentId::from_bytes(recovery.receipt_program_id.bytes())
                == ContentId::from_bytes(program_id.to_bytes())
            && quoted_recovery.collateral_atoms == 0
            && quoted_recovery.lamports
                == recovery
                    .total_payer_debit_lamports()
                    .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?
            && quote.value().recovery_rent_principal_lamports
                == recovery.account_rent_principal_lamports
            && root.state().capital().recovery_work_principal_lamports
                == recovery.work_capital_lamports
            && root.state().capital().recovery_rent_principal_lamports
                == recovery.account_rent_principal_lamports,
        ClutchError::MismatchedState,
    )?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            MARKET_RECOVERY_SCHEDULE_AUTHENTICATION_DOMAIN_V1,
            root.account().as_ref(),
            &root.authentication_id().bytes(),
            link.account().as_ref(),
            &link.authentication_id().bytes(),
            funding_quote_account.key.as_ref(),
            &quote.semantic_id().bytes(),
            liveness_policy_account.key.as_ref(),
            &policy_data_id.bytes(),
            &policy.policy_id.bytes(),
            &recovery.quote_schedule_id.bytes(),
            capability_programdata_account.as_ref(),
            capability_profile_account.as_ref(),
            &capability.capability_profile_id().bytes(),
            &maximum_progress_units_per_call.to_le_bytes(),
        ])
        .to_bytes(),
    );
    require_live_content_id(id)?;
    Ok(AuthenticatedMarketRecoveryScheduleV1 {
        id,
        market_root_account: root.account(),
        market_root_authentication_id: root.authentication_id(),
        series_link_account: link.account(),
        series_link_authentication_id: link.authentication_id(),
        funding_quote_id: quote.semantic_id(),
        liveness_policy_account: *liveness_policy_account.key,
        liveness_policy_id: ContentId::from_bytes(policy.policy_id.bytes()),
        recovery_quote_schedule_id: ContentId::from_bytes(recovery.quote_schedule_id.bytes()),
        maximum_calls: recovery.maximum_calls,
        maximum_lamports_per_call: recovery.maximum_lamports_per_call,
        work_capital_lamports: recovery.work_capital_lamports,
        account_rent_principal_lamports: recovery.account_rent_principal_lamports,
        receipt_program_id: ContentId::from_bytes(recovery.receipt_program_id.bytes()),
        capability_profile_id: capability.capability_profile_id(),
        maximum_progress_units_per_call,
    })
}

/// Private Product authorization for one exact Structured/wrapper admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedSeriesWrapperAuthorizationV1 {
    id: ContentId,
    link_account: Pubkey,
    link_authentication_id: ContentId,
    link_semantic_id: SeriesMarketLinkV1Id,
    series_plan_id: SeriesPlanV5Id,
    ordinal: u32,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    attachment_plan_id: ContentId,
    compiler_bundle_id: ContentId,
    capability_profile_id: ContentId,
    wrapper_recipe_set_id: ContentId,
    rent_refund_owner: ContentId,
    neutral_lamport_sink: ContentId,
    wrapper_status: SeriesLinkObligationStatusV1,
    wrapper_admission_receipt_id: ContentId,
    link_transition_sequence: u64,
}

/// Default-refusing Structured owner consumed by the Product Wrapper terminalizer.
///
/// Implementations must be private postwrite receipts which prove the complete
/// Structured descriptor/root/mint/vault retirement. Public projection bytes
/// or caller IDs cannot implement this authority by themselves.
pub(crate) trait AuthenticatedSeriesWrapperTerminalOwnerV1 {
    /// Exact Structured aggregate terminal receipt committed by Product.
    fn owner_terminal_receipt_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }

    /// Exact hostile-reauthenticated Structured root account.
    fn structured_root_account(&self) -> Outcome<Pubkey> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }

    /// Complete Structured root semantic postimage.
    fn structured_root_semantic_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }

    /// Exact Structured root account-data digest.
    fn structured_root_data_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }

    /// Authenticate the exact Product link and immutable Wrapper admission.
    #[allow(clippy::too_many_arguments)]
    fn authenticate_series_wrapper_terminal_owner_v1(
        &self,
        _link_account: Pubkey,
        _series_plan_id: SeriesPlanV5Id,
        _ordinal: u32,
        _market_instance_id: MarketInstanceV2Id,
        _generation: u64,
        _wrapper_admission_receipt_id: ContentId,
        _owner_terminal_receipt_id: ContentId,
        _structured_root_account: Pubkey,
        _structured_root_semantic_id: ContentId,
        _structured_root_data_id: ContentId,
    ) -> Outcome<()> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
}

/// Private Product postwrite receipt for one exact Wrapper obligation terminal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedSeriesWrapperTerminalV1 {
    id: ContentId,
    link_account: Pubkey,
    link_authentication_before: ContentId,
    link_authentication_after: ContentId,
    link_semantic_before: ContentId,
    link_semantic_after: ContentId,
    wrapper_admission_receipt_id: ContentId,
    owner_terminal_receipt_id: ContentId,
    product_terminal_projection: SeriesLinkObligationTerminalProjectionV1,
    structured_root_account: Pubkey,
    structured_root_semantic_id: ContentId,
    structured_root_data_id: ContentId,
}

impl AuthenticatedSeriesWrapperTerminalV1 {
    pub(crate) const fn id(self) -> ContentId {
        self.id
    }

    pub(crate) const fn link_account(self) -> Pubkey {
        self.link_account
    }

    pub(crate) const fn link_authentication_before(self) -> ContentId {
        self.link_authentication_before
    }

    pub(crate) const fn link_authentication_after(self) -> ContentId {
        self.link_authentication_after
    }

    pub(crate) const fn link_semantic_before(self) -> ContentId {
        self.link_semantic_before
    }

    pub(crate) const fn link_semantic_after(self) -> ContentId {
        self.link_semantic_after
    }

    pub(crate) const fn wrapper_admission_receipt_id(self) -> ContentId {
        self.wrapper_admission_receipt_id
    }

    pub(crate) const fn product_terminal_projection(
        self,
    ) -> SeriesLinkObligationTerminalProjectionV1 {
        self.product_terminal_projection
    }

    pub(crate) const fn owner_terminal_receipt_id(self) -> ContentId {
        self.owner_terminal_receipt_id
    }

    pub(crate) const fn structured_root_account(self) -> Pubkey {
        self.structured_root_account
    }

    pub(crate) const fn structured_root_semantic_id(self) -> ContentId {
        self.structured_root_semantic_id
    }

    pub(crate) const fn structured_root_data_id(self) -> ContentId {
        self.structured_root_data_id
    }
}

impl AuthenticatedSeriesWrapperAuthorizationV1 {
    /// Authorization identity.
    pub const fn id(self) -> ContentId {
        self.id
    }
    /// Exact SeriesMarketLink account (writable only for first admission).
    pub const fn link_account(self) -> Pubkey {
        self.link_account
    }
    /// Full link account-authentication identity.
    pub const fn link_authentication_id(self) -> ContentId {
        self.link_authentication_id
    }
    /// Exact pre-transition link semantic state.
    pub const fn link_semantic_id(self) -> SeriesMarketLinkV1Id {
        self.link_semantic_id
    }
    /// Exact Series.
    pub const fn series_plan_id(self) -> SeriesPlanV5Id {
        self.series_plan_id
    }
    /// Exact ordinal.
    pub const fn ordinal(self) -> u32 {
        self.ordinal
    }
    /// Shared Market.
    pub const fn market_instance_id(self) -> MarketInstanceV2Id {
        self.market_instance_id
    }
    /// Shared generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }
    /// Exact current V4 attachment identity.
    pub const fn attachment_plan_id(self) -> ContentId {
        self.attachment_plan_id
    }
    /// Exact current V4 compiler bundle identity.
    pub const fn compiler_bundle_id(self) -> ContentId {
        self.compiler_bundle_id
    }
    /// Exact central capability profile.
    pub const fn capability_profile_id(self) -> ContentId {
        self.capability_profile_id
    }
    /// Exact Structured-owned wrapper recipe-set identity pinned by AttachmentV4.
    pub const fn wrapper_recipe_set_id(self) -> ContentId {
        self.wrapper_recipe_set_id
    }
    /// Exact Product-owned refundable rent recipient.
    pub const fn rent_refund_owner(self) -> ContentId {
        self.rent_refund_owner
    }
    /// Exact Product-owned System lamport donation sink.
    pub const fn neutral_lamport_sink(self) -> ContentId {
        self.neutral_lamport_sink
    }
    /// Current exhaustive Product obligation state.
    pub const fn wrapper_status(self) -> SeriesLinkObligationStatusV1 {
        self.wrapper_status
    }
    /// Exact first Structured admission transcript; zero only before creation.
    pub const fn wrapper_admission_receipt_id(self) -> ContentId {
        self.wrapper_admission_receipt_id
    }
    /// Current link transition sequence bound by this authorization.
    pub const fn link_transition_sequence(self) -> u64 {
        self.link_transition_sequence
    }
    /// Whether the same instruction must persist the first Product admission.
    pub const fn requires_product_admission(self) -> bool {
        matches!(
            self.wrapper_status,
            SeriesLinkObligationStatusV1::EnabledNeverFounded
        )
    }
}

/// Join an authenticated active link to its exact BundleV4 and AttachmentV4.
///
/// The returned receipt distinguishes first admission from later live-child
/// additions. Structured remains the sole owner of recipe-set membership.
pub fn authenticate_series_wrapper_authorization_v1(
    program_id: &Pubkey,
    link: AuthenticatedSeriesMarketLinkV1<'_>,
    compiler_bundle_account: &AccountInfo<'_>,
    attachment_account: &AccountInfo<'_>,
) -> Outcome<AuthenticatedSeriesWrapperAuthorizationV1> {
    let binding = link.state().binding();
    let wrapper_status = link
        .state()
        .obligation_status(SeriesLinkObligationV1::Wrapper);
    require(
        link.state().phase() == SeriesMarketLinkPhaseV1::Active
            && matches!(
                wrapper_status,
                SeriesLinkObligationStatusV1::EnabledNeverFounded
                    | SeriesLinkObligationStatusV1::Live
            )
            && (wrapper_status == SeriesLinkObligationStatusV1::Live || link.is_writable()),
        ClutchError::MismatchedState,
    )?;
    let bundle = authenticate_product_artifact_v1::<CompiledProductSeriesBundleV5>(
        program_id,
        compiler_bundle_account,
        binding.compiler_output_id,
    )?;
    let attachment = authenticate_product_artifact_v1::<SeriesAttachmentPlanV4>(
        program_id,
        attachment_account,
        binding.attachment_plan_id,
    )?;
    let attachment_id = attachment
        .value()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        bundle.value().series_plan_id == binding.series_plan_id
            && bundle.value().funding_quote_id == binding.funding_quote_id
            && bundle.value().attachment_plan_id.content_id() == binding.attachment_plan_id
            && bundle.value().capability_profile_id.content_id() == binding.capability_profile_id
            && attachment_id.content_id() == binding.attachment_plan_id
            && attachment.value().funding_quote_id == bundle.value().funding_quote_id
            && attachment.value().funding_quote_id == binding.funding_quote_id,
        ClutchError::MismatchedState,
    )?;
    let link_semantic_id = link
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let wrapper_admission_receipt_id = link
        .state()
        .obligation_admission_receipt_id(SeriesLinkObligationV1::Wrapper);
    let link_transition_sequence = link.state().transition_sequence();
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SERIES_WRAPPER_AUTHENTICATION_DOMAIN_V1,
            link.account().as_ref(),
            &link.authentication_id().bytes(),
            &link_semantic_id.bytes(),
            compiler_bundle_account.key.as_ref(),
            &bundle.semantic_id().bytes(),
            attachment_account.key.as_ref(),
            &attachment.semantic_id().bytes(),
            &attachment.value().wrapper_recipe_set_id.bytes(),
            &[series_link_status_byte(wrapper_status)],
            &wrapper_admission_receipt_id.bytes(),
            &link_transition_sequence.to_le_bytes(),
        ])
        .to_bytes(),
    );
    require_live_content_id(id)?;
    Ok(AuthenticatedSeriesWrapperAuthorizationV1 {
        id,
        link_account: link.account(),
        link_authentication_id: link.authentication_id(),
        link_semantic_id,
        series_plan_id: binding.series_plan_id,
        ordinal: binding.ordinal,
        market_instance_id: binding.market_instance_id,
        generation: binding.generation,
        attachment_plan_id: binding.attachment_plan_id,
        compiler_bundle_id: binding.compiler_output_id,
        capability_profile_id: binding.capability_profile_id,
        wrapper_recipe_set_id: attachment.value().wrapper_recipe_set_id,
        rent_refund_owner: binding.rent_refund_owner,
        neutral_lamport_sink: binding.neutral_lamport_sink,
        wrapper_status,
        wrapper_admission_receipt_id,
        link_transition_sequence,
    })
}

/// Private whole-Market terminal receipt re-derived only from authenticated `0xaa`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedMarketInstanceTerminalV1 {
    id: ContentId,
    root_account: Pubkey,
    owner_program: Pubkey,
    root_semantic_id: ContentId,
    root_data_id: ContentId,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    projection: MarketInstanceTerminalProjectionV1,
}

impl AuthenticatedMarketInstanceTerminalV1 {
    /// Authentication receipt identity.
    pub const fn id(self) -> ContentId {
        self.id
    }

    /// Exact physical terminal root.
    pub const fn root_account(self) -> Pubkey {
        self.root_account
    }

    /// Program which owns the exact root.
    pub const fn owner_program(self) -> Pubkey {
        self.owner_program
    }

    /// Exact semantic identity of the terminal pure state.
    pub const fn root_semantic_id(self) -> ContentId {
        self.root_semantic_id
    }

    /// SHA-256 of the exact terminal framed bytes.
    pub const fn root_data_id(self) -> ContentId {
        self.root_data_id
    }

    /// Full-width shared Market identity.
    pub const fn market_instance_id(self) -> MarketInstanceV2Id {
        self.market_instance_id
    }

    /// Exact Market/Failure generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Exact exhaustive Failure-family receipt consumed by `0xaa`.
    pub const fn failure_terminal_receipt_id(self) -> ContentId {
        self.projection.failure_terminal_receipt_id()
    }

    /// Private structural projection consumed only inside this program.
    pub(crate) const fn projection(self) -> MarketInstanceTerminalProjectionV1 {
        self.projection
    }
}

/// Authenticate the exact shared Market root without trusting a caller DTO.
pub fn authenticate_market_lifecycle_root_v1<'state>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_market_instance_id: MarketInstanceV2Id,
    expected_generation: u64,
    require_writable: bool,
    output: &'state mut MarketLifecycleRootAccountV1,
) -> Outcome<AuthenticatedMarketLifecycleRootV1<'state>> {
    require(
        !account.is_signer
            && !account.executable
            && account.is_writable == require_writable
            && account.owner == program_id
            && account.data_len() == MARKET_LIFECYCLE_ROOT_ACCOUNT_BYTES_V1,
        ClutchError::MismatchedState,
    )?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    MarketLifecycleRootAccountV1::decode_into(&data, output)?;
    let binding = output.state.binding();
    let observed_lamports = account.lamports();
    require(
        binding.market_instance_id == expected_market_instance_id
            && binding.generation == expected_generation
            && observed_lamports >= output.rent_principal_lamports,
        ClutchError::MismatchedState,
    )?;
    let (expected, bump) = seeds::product_market_lifecycle_root_pda(
        program_id,
        &expected_market_instance_id.bytes(),
        expected_generation,
    );
    expect_pda(account.key, (expected, bump), Some(output.stored_bump))?;
    let data_id = ContentId::from_bytes(solana_sha256_hasher::hashv(&[&data[..]]).to_bytes());
    drop(data);
    let semantic_id = output
        .state
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let authentication_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            MARKET_LIFECYCLE_AUTHENTICATION_DOMAIN_V1,
            account.key.as_ref(),
            program_id.as_ref(),
            &data_id.bytes(),
            &semantic_id.bytes(),
            &output.rent_principal_lamports.to_le_bytes(),
            &observed_lamports.to_le_bytes(),
        ])
        .to_bytes(),
    );
    require_live_content_id(authentication_id)?;
    Ok(AuthenticatedMarketLifecycleRootV1 {
        account: *account.key,
        owner_program: *program_id,
        value: output,
        observed_lamports,
        writable: account.is_writable,
        data_id,
        authentication_id,
    })
}

/// Authenticate one exact permanent replay anchor. A mutable Market-root
/// constructor must instead call [`require_market_lifecycle_replay_absent_v1`]
/// and refuse whenever this account is already program-owned.
pub fn authenticate_market_lifecycle_replay_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_market_instance_id: MarketInstanceV2Id,
    expected_generation: u64,
) -> Outcome<AuthenticatedMarketLifecycleReplayV1> {
    authenticate_market_lifecycle_replay_with_mode_v1(
        program_id,
        account,
        expected_market_instance_id,
        expected_generation,
        false,
    )
}

fn authenticate_market_lifecycle_replay_with_mode_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_market_instance_id: MarketInstanceV2Id,
    expected_generation: u64,
    require_writable: bool,
) -> Outcome<AuthenticatedMarketLifecycleReplayV1> {
    require(
        !account.is_signer
            && account.is_writable == require_writable
            && !account.executable
            && account.owner == program_id
            && account.data_len() == MARKET_LIFECYCLE_REPLAY_ACCOUNT_BYTES_V1,
        ClutchError::MismatchedState,
    )?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let value = MarketLifecycleReplayAccountV1::decode(&data)?;
    let receipt = value.receipt;
    require(
        receipt.replay_account_id.bytes() == account.key.to_bytes()
            && receipt.market_instance_id == expected_market_instance_id
            && receipt.generation == expected_generation
            && account.lamports() >= value.permanent_rent_principal_lamports,
        ClutchError::MismatchedState,
    )?;
    let (expected, bump) = seeds::product_market_lifecycle_replay_pda(
        program_id,
        &expected_market_instance_id.bytes(),
        expected_generation,
    );
    expect_pda(account.key, (expected, bump), Some(value.stored_bump))?;
    let data_id = ContentId::from_bytes(solana_sha256_hasher::hashv(&[&data[..]]).to_bytes());
    drop(data);
    let receipt_id = receipt
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let observed_lamports = account.lamports();
    let authentication_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            MARKET_LIFECYCLE_REPLAY_AUTHENTICATION_DOMAIN_V1,
            account.key.as_ref(),
            program_id.as_ref(),
            &data_id.bytes(),
            &receipt_id.bytes(),
            &value.permanent_rent_principal_lamports.to_le_bytes(),
            &observed_lamports.to_le_bytes(),
        ])
        .to_bytes(),
    );
    require_live_content_id(authentication_id)?;
    Ok(AuthenticatedMarketLifecycleReplayV1 {
        account: *account.key,
        value,
        observed_lamports,
        data_id,
        authentication_id,
    })
}

/// Prove the permanent replay coordinate is still uninitialized. Hostile
/// System-owned prefunding is admitted only as an observed donation; any
/// program-owned body is replay and refuses before a new `0xaa` can be created.
pub(crate) fn require_market_lifecycle_replay_absent_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    require_writable: bool,
) -> Outcome<u64> {
    let (expected, bump) = seeds::product_market_lifecycle_replay_pda(
        program_id,
        &market_instance_id.bytes(),
        generation,
    );
    expect_pda(account.key, (expected, bump), None)?;
    require(
        !account.is_signer
            && account.is_writable == require_writable
            && !account.executable
            && account.owner.to_bytes() == SYSTEM_PROGRAM_ID
            && account.data_len() == 0,
        ClutchError::Replay,
    )?;
    Ok(account.lamports())
}

/// Authenticate an exact per-Series link and its shared-root association.
#[allow(clippy::too_many_arguments)]
pub fn authenticate_series_market_link_v1<'state>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_series_plan_id: SeriesPlanV5Id,
    expected_ordinal: u32,
    expected_market_instance_id: MarketInstanceV2Id,
    expected_generation: u64,
    expected_market_root: Pubkey,
    require_writable: bool,
    output: &'state mut SeriesMarketLinkAccountV1,
) -> Outcome<AuthenticatedSeriesMarketLinkV1<'state>> {
    require(
        !account.is_signer
            && !account.executable
            && account.is_writable == require_writable
            && account.owner == program_id
            && account.data_len() == SERIES_MARKET_LINK_ACCOUNT_BYTES_V1,
        ClutchError::MismatchedState,
    )?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    SeriesMarketLinkAccountV1::decode_into(&data, output)?;
    let binding = output.state.binding();
    let accounted_lamports = output
        .state
        .rent_principal_lamports()
        .checked_add(output.state.current_donation_lamports())
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let observed_lamports = account.lamports();
    require(
        binding.series_plan_id == expected_series_plan_id
            && binding.ordinal == expected_ordinal
            && binding.market_instance_id == expected_market_instance_id
            && binding.generation == expected_generation
            && binding.market_root_account_id.bytes() == expected_market_root.to_bytes()
            && observed_lamports >= accounted_lamports,
        ClutchError::MismatchedState,
    )?;
    let (expected, bump) = seeds::product_series_market_link_pda(
        program_id,
        &expected_series_plan_id.bytes(),
        expected_ordinal,
    );
    expect_pda(account.key, (expected, bump), Some(output.stored_bump))?;
    let data_id = ContentId::from_bytes(solana_sha256_hasher::hashv(&[&data[..]]).to_bytes());
    drop(data);
    let semantic_id = output
        .state
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let authentication_id = ContentId::from_bytes(
        series_market_link_authentication_id_v1(
            account.key.to_bytes(),
            program_id.to_bytes(),
            data_id.bytes(),
            semantic_id.bytes(),
            expected_market_root.to_bytes(),
            observed_lamports,
        )
        .0,
    );
    require_live_content_id(authentication_id)?;
    Ok(AuthenticatedSeriesMarketLinkV1 {
        account: *account.key,
        owner_program: *program_id,
        value: output,
        observed_lamports,
        writable: account.is_writable,
        data_id,
        authentication_id,
    })
}

/// Reauthenticate one already-funded, not-yet-allocated family account from
/// the exact root bitmap, content-addressed schedule/graph, and hostile physical
/// prestate. No caller-supplied amount can become authority.
pub(crate) fn authenticate_market_foundation_preallocation_v2(
    root: AuthenticatedMarketLifecycleRootV1<'_>,
    account: &AccountInfo<'_>,
    schedule: &MarketFoundationScheduleV2,
    account_graph: &MarketFoundationAccountGraphV2,
    slot: MarketFoundationSlotV2,
) -> Outcome<AuthenticatedMarketFoundationPreallocationV2> {
    require_canonical_market_foundation_core_v2(
        root.owner_program(),
        root.account(),
        account_graph,
    )?;
    let index = slot
        .index()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let bit = 1_u64
        .checked_shl(u32::try_from(index).map_err(|_| ClutchError::Arithmetic)?)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        matches!(
            slot,
            MarketFoundationSlotV2::FailureReplay
                | MarketFoundationSlotV2::FailureIntervalWork
                | MarketFoundationSlotV2::FailureIntervalHistory
                | MarketFoundationSlotV2::ResolutionV5
                | MarketFoundationSlotV2::FractionalPolicy
                | MarketFoundationSlotV2::FractionalLedger
                | MarketFoundationSlotV2::ProductReplayAnchor
        ) && (matches!(
            root.state().phase(),
            MarketLifecyclePhaseV1::Active | MarketLifecyclePhaseV1::Retiring
        ) || (slot == MarketFoundationSlotV2::ProductReplayAnchor
            && root.state().phase() == MarketLifecyclePhaseV1::Terminal))
            && root.state().foundation().initialized_bitmap & bit != 0,
        ClutchError::MismatchedState,
    )?;
    let binding = root.state().binding();
    let capital = root.state().capital();
    let schedule_id = schedule
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let account_graph_id = account_graph
        .id(schedule)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        schedule_id == binding.foundation_schedule_id
            && account_graph_id == binding.foundation_account_graph_id
            && account_graph.market_instance_id == binding.market_instance_id
            && account_graph.generation == binding.generation
            && account_graph
                .account(slot)
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                == ContentId::from_bytes(account.key.to_bytes())
            && account.is_writable
            && !account.is_signer
            && !account.executable
            && account.owner.to_bytes() == SYSTEM_PROGRAM_ID
            && account.data_len() == 0,
        ClutchError::MismatchedState,
    )?;
    let principal_lamports = schedule.slot_principal_lamports[index];
    let observed_balance_lamports = account.lamports();
    let donation_lamports = observed_balance_lamports
        .checked_sub(principal_lamports)
        .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?;
    let rent_refund_owner = Pubkey::new_from_array(capital.rent_refund_owner.bytes());
    let neutral_lamport_sink = Pubkey::new_from_array(capital.neutral_lamport_sink.bytes());
    require(
        principal_lamports != 0
            && account.key != &root.account()
            && account.key != &rent_refund_owner
            && account.key != &neutral_lamport_sink,
        ClutchError::AccountAlias,
    )?;
    let foundation_transcript_id = root.state().foundation().transcript_id;
    let slot_index = u64::try_from(index).map_err(|_| ClutchError::Arithmetic)?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            MARKET_FOUNDATION_PREALLOCATION_AUTHENTICATION_DOMAIN_V2,
            root.account().as_ref(),
            &root.authentication_id().bytes(),
            &binding.market_instance_id.bytes(),
            &binding.generation.to_le_bytes(),
            &slot_index.to_le_bytes(),
            account.key.as_ref(),
            &schedule_id.bytes(),
            &account_graph_id.bytes(),
            &foundation_transcript_id.bytes(),
            &principal_lamports.to_le_bytes(),
            &donation_lamports.to_le_bytes(),
            &observed_balance_lamports.to_le_bytes(),
            rent_refund_owner.as_ref(),
            neutral_lamport_sink.as_ref(),
        ])
        .to_bytes(),
    );
    require_live_content_id(id)?;
    Ok(AuthenticatedMarketFoundationPreallocationV2 {
        id,
        root_account: root.account(),
        root_authentication_id: root.authentication_id(),
        market_instance_id: binding.market_instance_id,
        generation: binding.generation,
        slot,
        account: *account.key,
        foundation_schedule_id: schedule_id.content_id(),
        foundation_account_graph_id: account_graph_id.content_id(),
        foundation_transcript_id,
        principal_lamports,
        donation_lamports,
        observed_balance_lamports,
        rent_refund_owner,
        neutral_lamport_sink,
    })
}

/// Debit one exact quote-owned foundation slot from the canonical Product
/// FoundationVault into its still-zero-data destination.
///
/// This performs no allocation, assignment, family write, or Product root
/// mutation. Those operations must complete in the same outer instruction and
/// feed a private family-owned poststate receipt into
/// [`accept_market_foundation_postwrite_v1`]. Any later refusal rolls the
/// System transfer back atomically.
#[allow(clippy::too_many_arguments)]
pub(crate) fn debit_market_foundation_slot_v1<'a>(
    program_id: &Pubkey,
    root: AuthenticatedMarketLifecycleRootV1<'_>,
    founder_link: AuthenticatedSeriesMarketLinkV1<'_>,
    funding_quote_account: &AccountInfo<'a>,
    foundation_vault: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    account_graph: &MarketFoundationAccountGraphV2,
    slot: MarketFoundationSlotV2,
) -> Outcome<AuthenticatedMarketFoundationDebitV1> {
    require_system_program(system_program)?;
    require_canonical_market_foundation_core_v2(program_id, root.account(), account_graph)?;
    require_distinct(&[
        funding_quote_account.clone(),
        foundation_vault.clone(),
        destination.clone(),
        system_program.clone(),
    ])?;
    require(
        root.is_writable()
            && root.state().phase() == MarketLifecyclePhaseV1::Founding
            && founder_link.state().phase() == SeriesMarketLinkPhaseV1::PendingMarket,
        ClutchError::MismatchedState,
    )?;
    let binding = root.state().binding();
    let capital = root.state().capital();
    let link_binding = founder_link.state().binding();
    let founder_link_id = founder_link
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        founder_link_id == capital.founder_link_id
            && link_binding.disposition == SeriesMarketDispositionV1::Founder
            && link_binding.market_instance_id == binding.market_instance_id
            && link_binding.generation == binding.generation
            && link_binding.market_binding_id
                == binding
                    .id()
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            && link_binding.market_root_account_id.bytes() == root.account().to_bytes()
            && link_binding.rent_refund_owner == capital.rent_refund_owner
            && link_binding.neutral_lamport_sink == capital.neutral_lamport_sink,
        ClutchError::MismatchedState,
    )?;
    let quote = authenticate_product_artifact_v1::<SeriesFundingQuoteV4>(
        program_id,
        funding_quote_account,
        link_binding.funding_quote_id.content_id(),
    )?;
    let schedule = &quote.value().foundation;
    let schedule_id = schedule
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let account_graph_id = account_graph
        .id(schedule)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let index = slot
        .index()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let bit = 1_u64
        .checked_shl(u32::try_from(index).map_err(|_| ClutchError::Arithmetic)?)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        slot != MarketFoundationSlotV2::LifecycleRoot
            && schedule_id == binding.foundation_schedule_id
            && account_graph_id == binding.foundation_account_graph_id
            && account_graph.market_instance_id == binding.market_instance_id
            && account_graph.generation == binding.generation
            && account_graph
                .account(slot)
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                == ContentId::from_bytes(destination.key.to_bytes())
            && root.state().foundation().initialized_bitmap & bit == 0
            && schedule.slot_principal_lamports[index] != 0,
        ClutchError::MismatchedState,
    )?;
    let expected_vault = Pubkey::new_from_array(binding.foundation_vault_id.bytes());
    let (derived_vault, bump) = seeds::product_market_foundation_vault_pda(
        program_id,
        &binding.market_instance_id.bytes(),
        binding.generation,
    );
    expect_pda(foundation_vault.key, (derived_vault, bump), None)?;
    require(
        *foundation_vault.key == expected_vault
            && foundation_vault.owner.to_bytes() == SYSTEM_PROGRAM_ID
            && foundation_vault.is_writable
            && !foundation_vault.is_signer
            && !foundation_vault.executable
            && foundation_vault.data_len() == 0
            && destination.owner.to_bytes() == SYSTEM_PROGRAM_ID
            && destination.is_writable
            && !destination.is_signer
            && !destination.executable
            && destination.data_len() == 0,
        ClutchError::MismatchedState,
    )?;
    let rent_refund_owner = Pubkey::new_from_array(capital.rent_refund_owner.bytes());
    let neutral_lamport_sink = Pubkey::new_from_array(capital.neutral_lamport_sink.bytes());
    require(
        destination.key != &root.account()
            && destination.key != &founder_link.account()
            && destination.key != &rent_refund_owner
            && destination.key != &neutral_lamport_sink
            && foundation_vault.key != &root.account()
            && foundation_vault.key != &founder_link.account()
            && foundation_vault.key != &rent_refund_owner
            && foundation_vault.key != &neutral_lamport_sink,
        ClutchError::AccountAlias,
    )?;
    let principal_lamports = schedule.slot_principal_lamports[index];
    let principal_before_lamports = capital.principal_remaining_lamports;
    let principal_after_lamports = principal_before_lamports
        .checked_sub(principal_lamports)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let observed_vault_before = foundation_vault.lamports();
    let vault_donation_lamports = observed_vault_before
        .checked_sub(principal_before_lamports)
        .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        vault_donation_lamports >= capital.vault_current_donation_lamports,
        ClutchError::MismatchedState,
    )?;
    let expected_vault_after = principal_after_lamports
        .checked_add(vault_donation_lamports)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let destination_donation_floor_lamports = destination.lamports();
    let destination_observed_balance_lamports = destination_donation_floor_lamports
        .checked_add(principal_lamports)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let transfer = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &transfer_data(principal_lamports),
        vec![
            AccountMeta::new(*foundation_vault.key, true),
            AccountMeta::new(*destination.key, false),
        ],
    );
    let market = binding.market_instance_id.bytes();
    let generation = binding.generation.to_le_bytes();
    let bump_seed = [bump];
    let signer_seeds: [&[u8]; 4] = [
        seeds::SEED_PRODUCT_MARKET_FOUNDATION_VAULT,
        &market,
        &generation,
        &bump_seed,
    ];
    invoke_signed(
        &transfer,
        &[
            foundation_vault.clone(),
            destination.clone(),
            system_program.clone(),
        ],
        &[&signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        foundation_vault.lamports() == expected_vault_after
            && destination.lamports() == destination_observed_balance_lamports,
        ClutchError::MismatchedState,
    )?;
    let market_binding_id = binding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let root_transition_sequence = root
        .state()
        .transition_sequence()
        .checked_add(1)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let slot_index = u64::try_from(index).map_err(|_| ClutchError::Arithmetic)?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            MARKET_FOUNDATION_DEBIT_AUTHENTICATION_DOMAIN_V1,
            root.account().as_ref(),
            &root.authentication_id().bytes(),
            &market_binding_id.bytes(),
            &binding.market_failure_policy_binding_id.bytes(),
            &binding.market_instance_id.bytes(),
            &binding.generation.to_le_bytes(),
            &founder_link_id.bytes(),
            &quote.semantic_id().bytes(),
            &schedule_id.bytes(),
            &account_graph_id.bytes(),
            &slot_index.to_le_bytes(),
            &root_transition_sequence.to_le_bytes(),
            foundation_vault.key.as_ref(),
            destination.key.as_ref(),
            &principal_lamports.to_le_bytes(),
            &principal_before_lamports.to_le_bytes(),
            &principal_after_lamports.to_le_bytes(),
            &vault_donation_lamports.to_le_bytes(),
            &destination_donation_floor_lamports.to_le_bytes(),
            &destination_observed_balance_lamports.to_le_bytes(),
            rent_refund_owner.as_ref(),
            neutral_lamport_sink.as_ref(),
        ])
        .to_bytes(),
    );
    require_live_content_id(id)?;
    Ok(AuthenticatedMarketFoundationDebitV1 {
        id,
        root_account: root.account(),
        root_authentication_id: root.authentication_id(),
        market_binding_id,
        failure_policy_binding_id: binding.market_failure_policy_binding_id,
        market_instance_id: binding.market_instance_id,
        generation: binding.generation,
        founder_link_id,
        funding_quote_id: quote.semantic_id(),
        foundation_schedule_id: schedule_id.content_id(),
        foundation_account_graph_id: account_graph_id.content_id(),
        slot,
        root_transition_sequence,
        foundation_vault: *foundation_vault.key,
        destination: *destination.key,
        principal_lamports,
        principal_before_lamports,
        principal_after_lamports,
        vault_donation_lamports,
        destination_donation_floor_lamports,
        destination_observed_balance_lamports,
        rent_refund_owner,
        neutral_lamport_sink,
    })
}

/// Consume one family-private accepted poststate and advance the exact Product
/// foundation bitmap, balance partition, and ordered transcript.
#[allow(clippy::too_many_arguments)]
pub(crate) fn accept_market_foundation_postwrite_v1<'next>(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    root: AuthenticatedMarketLifecycleRootV1<'_>,
    foundation_vault: &AccountInfo<'_>,
    destination: &AccountInfo<'_>,
    debit: AuthenticatedMarketFoundationDebitV1,
    schedule: &MarketFoundationScheduleV2,
    account_graph: &MarketFoundationAccountGraphV2,
    accepted_poststate_receipt_id: ContentId,
    successor_output: &mut MarketLifecycleRootV1,
    rebound_output: &'next mut MarketLifecycleRootAccountV1,
) -> Outcome<AuthenticatedMarketLifecycleRootV1<'next>> {
    require_live_content_id(accepted_poststate_receipt_id)?;
    let schedule_id = schedule
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let account_graph_id = account_graph
        .id(schedule)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let expected_vault_balance = debit
        .principal_after_lamports
        .checked_add(debit.vault_donation_lamports)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        root.is_writable()
            && *root_account.key == debit.root_account
            && root.authentication_id() == debit.root_authentication_id
            && *foundation_vault.key == debit.foundation_vault
            && *destination.key == debit.destination
            && foundation_vault.owner.to_bytes() == SYSTEM_PROGRAM_ID
            && foundation_vault.data_len() == 0
            && foundation_vault.lamports() == expected_vault_balance
            && destination.lamports() == debit.destination_observed_balance_lamports
            && schedule_id.content_id() == debit.foundation_schedule_id
            && account_graph_id.content_id() == debit.foundation_account_graph_id,
        ClutchError::MismatchedState,
    )?;
    *successor_output = root
        .state()
        .record_foundation_step(
            schedule,
            account_graph,
            debit.projection(accepted_poststate_receipt_id),
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    write_market_lifecycle_root_v1(
        program_id,
        root_account,
        root,
        successor_output,
        rebound_output,
    )
}

/// Consume the exact private Failure-runtime postimage as slot 6 and advance
/// Product only after the 2,172-byte account has been allocated, assigned,
/// persisted, and reauthenticated by the Failure owner.
#[cfg(feature = "non-production-failure-recovery-lab")]
#[allow(clippy::too_many_arguments)]
pub(crate) fn accept_failure_market_runtime_foundation_v1<'next>(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    root: AuthenticatedMarketLifecycleRootV1<'_>,
    foundation_vault: &AccountInfo<'_>,
    runtime_account: &AccountInfo<'_>,
    debit: AuthenticatedMarketFoundationDebitV1,
    schedule: &MarketFoundationScheduleV2,
    account_graph: &MarketFoundationAccountGraphV2,
    postimage: crate::instructions::failure_market_runtime::FailureMarketRuntimePostimageV1,
    successor_output: &mut MarketLifecycleRootV1,
    rebound_output: &'next mut MarketLifecycleRootAccountV1,
) -> Outcome<AuthenticatedMarketLifecycleRootV1<'next>> {
    let runtime = postimage.root();
    let receipt = postimage.admission_receipt();
    let facts = receipt.facts();
    require(
        debit.slot == MarketFoundationSlotV2::FailureRuntimeRoot
            && runtime.account() == debit.destination
            && facts.runtime_account_id.bytes() == debit.destination.to_bytes()
            && facts.foundation_receipt_id == debit.id
            && facts.market_instance_id == debit.market_instance_id
            && facts.generation == debit.generation
            && facts.root_funding.rent_refund_owner.bytes() == debit.rent_refund_owner.to_bytes()
            && facts.root_funding.neutral_sink.bytes() == debit.neutral_lamport_sink.to_bytes()
            && facts.root_funding.rent_principal_lamports == debit.principal_lamports
            && facts.root_funding.donation_floor_lamports
                == debit.destination_donation_floor_lamports
            && facts.root_funding.observed_balance_lamports
                == debit.destination_observed_balance_lamports,
        ClutchError::MismatchedState,
    )?;
    accept_market_foundation_postwrite_v1(
        program_id,
        root_account,
        root,
        foundation_vault,
        runtime_account,
        debit,
        schedule,
        account_graph,
        ContentId::from_bytes(receipt.id().bytes()),
        successor_output,
        rebound_output,
    )
}

/// After the immutable founding deadline, atomically refund every still-held
/// FoundationVault principal, sink all observed surplus, and persist the root's
/// `Founding -> Aborting` transition. The authenticated Clock receipt is the
/// sole current-bucket authority.
#[allow(clippy::too_many_arguments)]
pub(crate) fn begin_market_foundation_abort_v1<'next, 'a>(
    program_id: &Pubkey,
    root_account: &AccountInfo<'a>,
    root: AuthenticatedMarketLifecycleRootV1<'_>,
    foundation_vault: &AccountInfo<'a>,
    refund_owner: &AccountInfo<'a>,
    neutral_lamport_sink: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    clock: AuthenticatedClockBucketV1,
    successor_output: &mut MarketLifecycleRootV1,
    rebound_output: &'next mut MarketLifecycleRootAccountV1,
) -> Outcome<(
    AuthenticatedMarketLifecycleRootV1<'next>,
    AuthenticatedMarketFoundationVaultDispositionV1,
)> {
    require_system_program(system_program)?;
    require_distinct(&[
        root_account.clone(),
        foundation_vault.clone(),
        refund_owner.clone(),
        neutral_lamport_sink.clone(),
        system_program.clone(),
    ])?;
    let binding = root.state().binding();
    let capital = root.state().capital();
    require(
        root.is_writable()
            && *root_account.key == root.account()
            && root.state().phase() == MarketLifecyclePhaseV1::Founding
            && clock.policy_id().bytes() == binding.clock_policy_id.bytes()
            && clock.bucket() > binding.founding_deadline_bucket,
        ClutchError::MismatchedState,
    )?;
    let (expected_vault, bump) = seeds::product_market_foundation_vault_pda(
        program_id,
        &binding.market_instance_id.bytes(),
        binding.generation,
    );
    expect_pda(foundation_vault.key, (expected_vault, bump), None)?;
    require(
        binding.foundation_vault_id.bytes() == foundation_vault.key.to_bytes()
            && capital.rent_refund_owner.bytes() == refund_owner.key.to_bytes()
            && capital.neutral_lamport_sink.bytes() == neutral_lamport_sink.key.to_bytes()
            && foundation_vault.is_writable
            && !foundation_vault.is_signer
            && !foundation_vault.executable
            && foundation_vault.owner.to_bytes() == SYSTEM_PROGRAM_ID
            && foundation_vault.data_len() == 0
            && refund_owner.is_writable
            && !refund_owner.executable
            && refund_owner.owner.to_bytes() == SYSTEM_PROGRAM_ID
            && refund_owner.data_len() == 0
            && neutral_lamport_sink.is_writable
            && !neutral_lamport_sink.executable
            && neutral_lamport_sink.owner.to_bytes() == SYSTEM_PROGRAM_ID
            && neutral_lamport_sink.data_len() == 0,
        ClutchError::MismatchedState,
    )?;
    let observed_balance_before = foundation_vault.lamports();
    let principal_lamports = capital.principal_remaining_lamports;
    let donation_lamports = observed_balance_before
        .checked_sub(principal_lamports)
        .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        donation_lamports >= capital.vault_current_donation_lamports,
        ClutchError::MismatchedState,
    )?;
    let refund_before = refund_owner.lamports();
    let sink_before = neutral_lamport_sink.lamports();
    transfer_from_foundation_vault_v1(
        foundation_vault,
        refund_owner,
        system_program,
        principal_lamports,
        &binding.market_instance_id.bytes(),
        binding.generation,
        bump,
    )?;
    transfer_from_foundation_vault_v1(
        foundation_vault,
        neutral_lamport_sink,
        system_program,
        donation_lamports,
        &binding.market_instance_id.bytes(),
        binding.generation,
        bump,
    )?;
    require(
        foundation_vault.lamports() == 0
            && refund_owner.lamports()
                == refund_before
                    .checked_add(principal_lamports)
                    .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?
            && neutral_lamport_sink.lamports()
                == sink_before
                    .checked_add(donation_lamports)
                    .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?,
        ClutchError::MismatchedState,
    )?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            MARKET_FOUNDATION_VAULT_ABORT_DOMAIN_V1,
            root.account().as_ref(),
            &root.authentication_id().bytes(),
            &binding.market_instance_id.bytes(),
            &binding.generation.to_le_bytes(),
            foundation_vault.key.as_ref(),
            refund_owner.key.as_ref(),
            neutral_lamport_sink.key.as_ref(),
            &clock.id().bytes(),
            &clock.bucket().to_le_bytes(),
            &principal_lamports.to_le_bytes(),
            &donation_lamports.to_le_bytes(),
            &observed_balance_before.to_le_bytes(),
        ])
        .to_bytes(),
    );
    require_live_content_id(id)?;
    let disposition = AuthenticatedMarketFoundationVaultDispositionV1 {
        id,
        market_instance_id: binding.market_instance_id,
        generation: binding.generation,
        foundation_vault: *foundation_vault.key,
        refund_owner: *refund_owner.key,
        neutral_lamport_sink: *neutral_lamport_sink.key,
        principal_lamports,
        donation_lamports,
        observed_balance_before,
    };
    root.state()
        .begin_abort_into(clock.bucket(), donation_lamports, id, successor_output)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let rebound = write_market_lifecycle_root_v1(
        program_id,
        root_account,
        root,
        successor_output,
        rebound_output,
    )?;
    Ok((rebound, disposition))
}

fn transfer_from_foundation_vault_v1<'a>(
    foundation_vault: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    lamports: u64,
    market_instance_id: &[u8; 32],
    generation: u64,
    bump: u8,
) -> Outcome<()> {
    if lamports == 0 {
        return Ok(());
    }
    let instruction = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &transfer_data(lamports),
        vec![
            AccountMeta::new(*foundation_vault.key, true),
            AccountMeta::new(*destination.key, false),
        ],
    );
    let generation_bytes = generation.to_le_bytes();
    let bump_seed = [bump];
    let signer_seeds: [&[u8]; 4] = [
        seeds::SEED_PRODUCT_MARKET_FOUNDATION_VAULT,
        market_instance_id,
        &generation_bytes,
        &bump_seed,
    ];
    invoke_signed(
        &instruction,
        &[
            foundation_vault.clone(),
            destination.clone(),
            system_program.clone(),
        ],
        &[&signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))
}

/// Close a fully unwound inert `0xaa/1` root after every initialized non-root
/// slot has been closed in reverse order. The exact stored root rent principal
/// returns to its immutable owner; every unsolicited lamport goes only to the
/// neutral sink. The returned private receipt is the sole Product authority
/// for restoring or explicitly resolving the founder link reservation.
pub(crate) fn close_aborted_market_lifecycle_root_v1(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    root: AuthenticatedMarketLifecycleRootV1<'_>,
    foundation_vault: &AccountInfo<'_>,
    refund_owner: &AccountInfo<'_>,
    neutral_lamport_sink: &AccountInfo<'_>,
) -> Outcome<AuthenticatedMarketFoundingAbortCloseV1> {
    require_distinct(&[
        root_account.clone(),
        foundation_vault.clone(),
        refund_owner.clone(),
        neutral_lamport_sink.clone(),
    ])?;
    let binding = root.state().binding();
    let capital = root.state().capital();
    let abort_projection = root
        .state()
        .finalize_abort()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let (expected_vault, vault_bump) = seeds::product_market_foundation_vault_pda(
        program_id,
        &binding.market_instance_id.bytes(),
        binding.generation,
    );
    expect_pda(foundation_vault.key, (expected_vault, vault_bump), None)?;
    require(
        root.is_writable()
            && *root_account.key == root.account()
            && root_account.owner == program_id
            && root_account.data_len() == MARKET_LIFECYCLE_ROOT_ACCOUNT_BYTES_V1
            && binding.foundation_vault_id.bytes() == foundation_vault.key.to_bytes()
            && foundation_vault.is_writable
            && !foundation_vault.is_signer
            && !foundation_vault.executable
            && foundation_vault.owner.to_bytes() == SYSTEM_PROGRAM_ID
            && foundation_vault.data_len() == 0
            && foundation_vault.lamports() == 0
            && capital.rent_refund_owner.bytes() == refund_owner.key.to_bytes()
            && capital.neutral_lamport_sink.bytes() == neutral_lamport_sink.key.to_bytes()
            && abort_projection.refund_owner().bytes() == refund_owner.key.to_bytes()
            && abort_projection.neutral_lamport_sink().bytes()
                == neutral_lamport_sink.key.to_bytes()
            && refund_owner.is_writable
            && !refund_owner.executable
            && refund_owner.owner.to_bytes() == SYSTEM_PROGRAM_ID
            && refund_owner.data_len() == 0
            && neutral_lamport_sink.is_writable
            && !neutral_lamport_sink.executable
            && neutral_lamport_sink.owner.to_bytes() == SYSTEM_PROGRAM_ID
            && neutral_lamport_sink.data_len() == 0,
        ClutchError::MismatchedState,
    )?;
    let root_pre_balance = root_account.lamports();
    let root_principal = root.rent_principal_lamports();
    let root_surplus = root_pre_balance
        .checked_sub(root_principal)
        .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?;
    let refund_before = refund_owner.lamports();
    let sink_before = neutral_lamport_sink.lamports();
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            MARKET_LIFECYCLE_ABORT_CLOSE_DOMAIN_V1,
            root.account().as_ref(),
            &root.authentication_id().bytes(),
            &abort_projection.id().bytes(),
            foundation_vault.key.as_ref(),
            refund_owner.key.as_ref(),
            neutral_lamport_sink.key.as_ref(),
            &root_pre_balance.to_le_bytes(),
            &root_principal.to_le_bytes(),
            &root_surplus.to_le_bytes(),
        ])
        .to_bytes(),
    );
    require_live_content_id(id)?;
    debit_program_owned_lamports_v1(root_account, root_pre_balance)?;
    credit_program_owned_lamports_v1(refund_owner, root_principal)?;
    credit_program_owned_lamports_v1(neutral_lamport_sink, root_surplus)?;
    root_account
        .resize(0)
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    root_account.assign(&SYSTEM_PROGRAM_ID);
    require(
        root_account.lamports() == 0
            && root_account.data_len() == 0
            && root_account.owner.to_bytes() == SYSTEM_PROGRAM_ID
            && refund_owner.lamports()
                == refund_before
                    .checked_add(root_principal)
                    .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?
            && neutral_lamport_sink.lamports()
                == sink_before
                    .checked_add(root_surplus)
                    .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?,
        ClutchError::MismatchedState,
    )?;
    Ok(AuthenticatedMarketFoundingAbortCloseV1 {
        id,
        root_account: *root_account.key,
        root_authentication_id: root.authentication_id(),
        abort_projection,
        root_rent_principal_lamports: root_principal,
        root_surplus_lamports: root_surplus,
        refund_owner: *refund_owner.key,
        neutral_lamport_sink: *neutral_lamport_sink.key,
    })
}

/// Atomically replace the terminal mutable `0xaa/1` with its compact permanent
/// `0xb0/1` replay anchor, drain FoundationVault donations, refund only the
/// exact root rent principal, and send every physical surplus to the canonical
/// neutral sink.
#[allow(clippy::too_many_arguments)]
pub(crate) fn close_market_lifecycle_to_replay_v1<'a>(
    program_id: &Pubkey,
    root_account: &AccountInfo<'a>,
    root: AuthenticatedMarketLifecycleRootV1<'_>,
    foundation_vault: &AccountInfo<'a>,
    replay_account: &AccountInfo<'a>,
    refund_owner: &AccountInfo<'a>,
    neutral_lamport_sink: &AccountInfo<'a>,
    rent_sysvar: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    schedule: &MarketFoundationScheduleV2,
    account_graph: &MarketFoundationAccountGraphV2,
) -> Outcome<AuthenticatedMarketLifecycleReplayV1> {
    require_system_program(system_program)?;
    require_distinct(&[
        root_account.clone(),
        foundation_vault.clone(),
        replay_account.clone(),
        refund_owner.clone(),
        neutral_lamport_sink.clone(),
        rent_sysvar.clone(),
        system_program.clone(),
    ])?;
    let binding = root.state().binding();
    let capital = root.state().capital();
    require(
        root.is_writable()
            && *root_account.key == root.account()
            && root.state().phase() == MarketLifecyclePhaseV1::Terminal
            && root.state().live_series_links() == 0
            && root.state().admitted_series_links() == root.state().retired_series_links()
            && capital.principal_remaining_lamports == 0
            && capital.rent_refund_owner.bytes() == refund_owner.key.to_bytes()
            && capital.neutral_lamport_sink.bytes() == neutral_lamport_sink.key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let replay_preallocation = authenticate_market_foundation_preallocation_v2(
        root,
        replay_account,
        schedule,
        account_graph,
        MarketFoundationSlotV2::ProductReplayAnchor,
    )?;
    let rent = read_rent(rent_sysvar)?;
    let replay_principal = rent.minimum_balance(MARKET_LIFECYCLE_REPLAY_ACCOUNT_BYTES_V1)?;
    require(
        replay_preallocation.principal_lamports == replay_principal
            && replay_preallocation.observed_balance_lamports == replay_account.lamports(),
        ClutchError::MismatchedState,
    )?;
    let (expected_vault, vault_bump) = seeds::product_market_foundation_vault_pda(
        program_id,
        &binding.market_instance_id.bytes(),
        binding.generation,
    );
    expect_pda(foundation_vault.key, (expected_vault, vault_bump), None)?;
    require(
        binding.foundation_vault_id.bytes() == foundation_vault.key.to_bytes()
            && foundation_vault.is_writable
            && !foundation_vault.is_signer
            && !foundation_vault.executable
            && foundation_vault.owner.to_bytes() == SYSTEM_PROGRAM_ID
            && foundation_vault.data_len() == 0
            && refund_owner.is_writable
            && !refund_owner.executable
            && refund_owner.owner.to_bytes() == SYSTEM_PROGRAM_ID
            && refund_owner.data_len() == 0
            && neutral_lamport_sink.is_writable
            && !neutral_lamport_sink.executable
            && neutral_lamport_sink.owner.to_bytes() == SYSTEM_PROGRAM_ID
            && neutral_lamport_sink.data_len() == 0,
        ClutchError::MismatchedState,
    )?;
    let vault_donation = foundation_vault.lamports();
    require(
        vault_donation >= capital.vault_current_donation_lamports,
        ClutchError::MismatchedState,
    )?;
    let sink_before = neutral_lamport_sink.lamports();
    transfer_from_foundation_vault_v1(
        foundation_vault,
        neutral_lamport_sink,
        system_program,
        vault_donation,
        &binding.market_instance_id.bytes(),
        binding.generation,
        vault_bump,
    )?;
    require(
        foundation_vault.lamports() == 0,
        ClutchError::MismatchedState,
    )?;
    let foundation_disposition_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            MARKET_FOUNDATION_VAULT_TERMINAL_DOMAIN_V1,
            root.account().as_ref(),
            &root.authentication_id().bytes(),
            &binding.market_instance_id.bytes(),
            &binding.generation.to_le_bytes(),
            foundation_vault.key.as_ref(),
            neutral_lamport_sink.key.as_ref(),
            &vault_donation.to_le_bytes(),
        ])
        .to_bytes(),
    );
    require_live_content_id(foundation_disposition_id)?;
    let root_pre_balance = root_account.lamports();
    let root_principal = root.rent_principal_lamports();
    let root_surplus = root_pre_balance
        .checked_sub(root_principal)
        .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?;
    let root_close_disposition_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            MARKET_LIFECYCLE_ROOT_CLOSE_DOMAIN_V1,
            root.account().as_ref(),
            &root.authentication_id().bytes(),
            refund_owner.key.as_ref(),
            neutral_lamport_sink.key.as_ref(),
            &root_pre_balance.to_le_bytes(),
            &root_principal.to_le_bytes(),
            &root_surplus.to_le_bytes(),
        ])
        .to_bytes(),
    );
    require_live_content_id(root_close_disposition_id)?;
    let terminal = root
        .state()
        .terminal_projection()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let receipt = MarketLifecycleReplayReceiptV1::seal(
        ContentId::from_bytes(replay_account.key.to_bytes()),
        ContentId::from_bytes(root_account.key.to_bytes()),
        root.state(),
        terminal,
        foundation_disposition_id,
        root_close_disposition_id,
        replay_principal,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let (_, replay_bump) = seeds::product_market_lifecycle_replay_pda(
        program_id,
        &binding.market_instance_id.bytes(),
        binding.generation,
    );
    let generation_bytes = binding.generation.to_le_bytes();
    let market_bytes = binding.market_instance_id.bytes();
    let bump_seed = [replay_bump];
    let replay_signer: [&[u8]; 4] = [
        seeds::SEED_PRODUCT_MARKET_LIFECYCLE_REPLAY,
        &market_bytes,
        &generation_bytes,
        &bump_seed,
    ];
    let allocate = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &allocate_data(MARKET_LIFECYCLE_REPLAY_ACCOUNT_BYTES_V1),
        vec![AccountMeta::new(*replay_account.key, true)],
    );
    invoke_signed(
        &allocate,
        &[replay_account.clone(), system_program.clone()],
        &[&replay_signer],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    let assign = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &assign_data(program_id),
        vec![AccountMeta::new(*replay_account.key, true)],
    );
    invoke_signed(
        &assign,
        &[replay_account.clone(), system_program.clone()],
        &[&replay_signer],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        replay_account.owner == program_id
            && replay_account.data_len() == MARKET_LIFECYCLE_REPLAY_ACCOUNT_BYTES_V1,
        ClutchError::MismatchedState,
    )?;
    let replay_value = MarketLifecycleReplayAccountV1 {
        receipt,
        permanent_rent_principal_lamports: replay_principal,
        stored_bump: replay_bump,
    };
    {
        let mut data = replay_account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        replay_value.encode(&mut data)?;
    }
    let replay_donation = replay_preallocation.donation_lamports;
    debit_program_owned_lamports_v1(replay_account, replay_donation)?;
    credit_program_owned_lamports_v1(neutral_lamport_sink, replay_donation)?;
    require(
        replay_account.lamports() == replay_principal,
        ClutchError::MismatchedState,
    )?;
    let refund_before = refund_owner.lamports();
    debit_program_owned_lamports_v1(root_account, root_pre_balance)?;
    credit_program_owned_lamports_v1(refund_owner, root_principal)?;
    credit_program_owned_lamports_v1(neutral_lamport_sink, root_surplus)?;
    root_account
        .resize(0)
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    root_account.assign(&SYSTEM_PROGRAM_ID);
    require(
        root_account.lamports() == 0
            && root_account.data_len() == 0
            && root_account.owner.to_bytes() == SYSTEM_PROGRAM_ID
            && refund_owner.lamports()
                == refund_before
                    .checked_add(root_principal)
                    .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?
            && neutral_lamport_sink.lamports()
                == sink_before
                    .checked_add(vault_donation)
                    .and_then(|value| value.checked_add(replay_donation))
                    .and_then(|value| value.checked_add(root_surplus))
                    .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?,
        ClutchError::MismatchedState,
    )?;
    authenticate_market_lifecycle_replay_with_mode_v1(
        program_id,
        replay_account,
        binding.market_instance_id,
        binding.generation,
        true,
    )
}

fn debit_program_owned_lamports_v1(account: &AccountInfo<'_>, amount: u64) -> Outcome<()> {
    if amount == 0 {
        return Ok(());
    }
    let mut lamports = account
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    **lamports = lamports
        .checked_sub(amount)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    Ok(())
}

fn credit_program_owned_lamports_v1(account: &AccountInfo<'_>, amount: u64) -> Outcome<()> {
    if amount == 0 {
        return Ok(());
    }
    let mut lamports = account
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    **lamports = lamports
        .checked_add(amount)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    Ok(())
}

/// Re-open a terminal root and mint the private whole-Market receipt.
pub fn authenticate_market_instance_terminal_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_market_instance_id: MarketInstanceV2Id,
    expected_generation: u64,
    root_output: &mut MarketLifecycleRootAccountV1,
) -> Outcome<AuthenticatedMarketInstanceTerminalV1> {
    let root = authenticate_market_lifecycle_root_v1(
        program_id,
        account,
        expected_market_instance_id,
        expected_generation,
        false,
        root_output,
    )?;
    require(
        root.state().phase() == MarketLifecyclePhaseV1::Terminal,
        ClutchError::MismatchedState,
    )?;
    let projection = root
        .state()
        .terminal_projection()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        projection.market_instance_id() == expected_market_instance_id
            && projection.generation() == expected_generation,
        ClutchError::MismatchedState,
    )?;
    let root_semantic_id = root
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            MARKET_INSTANCE_TERMINAL_AUTHENTICATION_DOMAIN_V1,
            account.key.as_ref(),
            program_id.as_ref(),
            &root.data_id().bytes(),
            &root_semantic_id.bytes(),
            &projection.id().bytes(),
            &root.observed_lamports().to_le_bytes(),
        ])
        .to_bytes(),
    );
    require_live_content_id(id)?;
    Ok(AuthenticatedMarketInstanceTerminalV1 {
        id,
        root_account: root.account(),
        owner_program: root.owner_program(),
        root_semantic_id,
        root_data_id: root.data_id(),
        market_instance_id: expected_market_instance_id,
        generation: expected_generation,
        projection,
    })
}

/// Atomically finalize a fully retired root and return its private terminal receipt.
pub fn finalize_market_lifecycle_terminal_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    authenticated: AuthenticatedMarketLifecycleRootV1<'_>,
    successor_output: &mut MarketLifecycleRootV1,
    rebound_output: &mut MarketLifecycleRootAccountV1,
) -> Outcome<AuthenticatedMarketInstanceTerminalV1> {
    let binding = authenticated.state().binding();
    let (successor, _) = authenticated
        .state()
        .finalize_terminal()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    *successor_output = successor;
    write_market_lifecycle_root_v1(
        program_id,
        account,
        authenticated,
        successor_output,
        rebound_output,
    )?;
    authenticate_market_instance_terminal_v1(
        program_id,
        account,
        binding.market_instance_id,
        binding.generation,
        rebound_output,
    )
}

/// Persist a pure successor and immediately reauthenticate the full root bytes.
fn write_market_lifecycle_root_v1<'next>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    authenticated: AuthenticatedMarketLifecycleRootV1<'_>,
    successor: &MarketLifecycleRootV1,
    rebound_output: &'next mut MarketLifecycleRootAccountV1,
) -> Outcome<AuthenticatedMarketLifecycleRootV1<'next>> {
    let binding = authenticated.state().binding();
    require(
        account.is_writable
            && *account.key == authenticated.account
            && account.owner == program_id
            && successor.binding() == binding,
        ClutchError::MismatchedState,
    )?;
    let live = authenticate_market_lifecycle_root_v1(
        program_id,
        account,
        binding.market_instance_id,
        binding.generation,
        true,
        rebound_output,
    )?;
    require(
        live.account == authenticated.account
            && live.owner_program == authenticated.owner_program
            && live.value == authenticated.value
            && live.observed_lamports == authenticated.observed_lamports
            && live.writable == authenticated.writable
            && live.data_id == authenticated.data_id
            && live.authentication_id == authenticated.authentication_id,
        ClutchError::MismatchedState,
    )?;
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    MarketLifecycleRootAccountV1::encode_parts(
        successor,
        authenticated.value.rent_principal_lamports,
        authenticated.value.stored_bump,
        &mut data,
    )?;
    drop(data);
    let rebound = authenticate_market_lifecycle_root_v1(
        program_id,
        account,
        successor.binding().market_instance_id,
        successor.binding().generation,
        true,
        rebound_output,
    )?;
    require(
        rebound.value.state == *successor
            && rebound.value.rent_principal_lamports == authenticated.value.rent_principal_lamports
            && rebound.value.stored_bump == authenticated.value.stored_bump,
        ClutchError::MismatchedState,
    )?;
    Ok(rebound)
}

/// Default-refusing same-program authority for the sole Resolution V5 root write.
///
/// The isolated Failure/Collateral composer implements this only for a private
/// postwrite receipt constructed after all three liability accounts have been
/// hostile-reauthenticated. A pure activation value is never sufficient.
pub(crate) trait AuthenticatedMarketResolutionActivationWriteV1 {
    /// Authenticate the exact Product/Failure/Collateral postwrite join.
    fn authenticate_market_resolution_activation_write_v1(
        &self,
        _root_authentication_before: ContentId,
        _expected: MarketResolutionActivationV1,
        _slot10_preallocation_id: ContentId,
        _collateral_plan_receipt_id: ContentId,
        _collateral_postwrite_receipt_id: ContentId,
        _failure_resolution_receipt_id: ContentId,
    ) -> Outcome<()> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
}

/// Record the once-only Product Resolution activation through its narrow authority.
pub(crate) fn record_market_resolution_activation_v1<
    'next,
    A: AuthenticatedMarketResolutionActivationWriteV1 + ?Sized,
>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    authenticated: AuthenticatedMarketLifecycleRootV1<'_>,
    activation: MarketResolutionActivationV1,
    slot10_preallocation_id: ContentId,
    collateral_plan_receipt_id: ContentId,
    collateral_postwrite_receipt_id: ContentId,
    failure_resolution_receipt_id: ContentId,
    authority: &A,
    rebound_output: &'next mut MarketLifecycleRootAccountV1,
) -> Outcome<AuthenticatedMarketLifecycleRootV1<'next>> {
    authority.authenticate_market_resolution_activation_write_v1(
        authenticated.authentication_id(),
        activation,
        slot10_preallocation_id,
        collateral_plan_receipt_id,
        collateral_postwrite_receipt_id,
        failure_resolution_receipt_id,
    )?;
    let successor = authenticated
        .state()
        .record_resolution_activation(activation)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    write_market_lifecycle_root_v1(
        program_id,
        account,
        authenticated,
        &successor,
        rebound_output,
    )
}

/// Persist a pure per-Series link successor and reauthenticate exact bytes.
fn write_series_market_link_v1<'next>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    authenticated: AuthenticatedSeriesMarketLinkV1<'_>,
    successor: &SeriesMarketLinkV1,
    rebound_output: &'next mut SeriesMarketLinkAccountV1,
) -> Outcome<AuthenticatedSeriesMarketLinkV1<'next>> {
    let binding = authenticated.state().binding();
    require(
        account.is_writable
            && *account.key == authenticated.account
            && account.owner == program_id
            && successor.binding() == binding,
        ClutchError::MismatchedState,
    )?;
    let live = authenticate_series_market_link_v1(
        program_id,
        account,
        binding.series_plan_id,
        binding.ordinal,
        binding.market_instance_id,
        binding.generation,
        Pubkey::new_from_array(binding.market_root_account_id.bytes()),
        true,
        rebound_output,
    )?;
    require(
        live.account == authenticated.account
            && live.owner_program == authenticated.owner_program
            && live.value == authenticated.value
            && live.observed_lamports == authenticated.observed_lamports
            && live.writable == authenticated.writable
            && live.data_id == authenticated.data_id
            && live.authentication_id == authenticated.authentication_id,
        ClutchError::MismatchedState,
    )?;
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    SeriesMarketLinkAccountV1::encode_parts(successor, authenticated.value.stored_bump, &mut data)?;
    drop(data);
    let binding = successor.binding();
    let rebound = authenticate_series_market_link_v1(
        program_id,
        account,
        binding.series_plan_id,
        binding.ordinal,
        binding.market_instance_id,
        binding.generation,
        Pubkey::new_from_array(binding.market_root_account_id.bytes()),
        true,
        rebound_output,
    )?;
    require(
        rebound.value.state == *successor
            && rebound.value.stored_bump == authenticated.value.stored_bump,
        ClutchError::MismatchedState,
    )?;
    Ok(rebound)
}

/// Persist the first Product-side Wrapper admission in the same instruction
/// that an authenticated Structured owner accepts its root/descriptor.
pub(crate) fn admit_series_wrapper_obligation_v1<'next>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    authenticated: AuthenticatedSeriesMarketLinkV1<'_>,
    authorization: AuthenticatedSeriesWrapperAuthorizationV1,
    structured_admission_receipt_id: ContentId,
    rebound_output: &'next mut SeriesMarketLinkAccountV1,
) -> Outcome<AuthenticatedSeriesMarketLinkV1<'next>> {
    require_live_content_id(structured_admission_receipt_id)?;
    let semantic_id = authenticated
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        authenticated.is_writable()
            && authorization.requires_product_admission()
            && authorization.link_account == authenticated.account()
            && authorization.link_authentication_id == authenticated.authentication_id()
            && authorization.link_semantic_id == semantic_id
            && authorization.wrapper_admission_receipt_id == ContentId::ZERO
            && authorization.link_transition_sequence
                == authenticated.state().transition_sequence(),
        ClutchError::MismatchedState,
    )?;
    let next_sequence = authorization
        .link_transition_sequence
        .checked_add(1)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let successor = authenticated
        .state()
        .admit_obligation(SeriesLinkObligationAdmissionProjectionV1 {
            link_semantic_id: semantic_id,
            obligation: SeriesLinkObligationV1::Wrapper,
            link_transition_sequence: next_sequence,
            owner_admission_receipt_id: structured_admission_receipt_id,
        })
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    write_series_market_link_v1(
        program_id,
        account,
        authenticated,
        &successor,
        rebound_output,
    )
}

/// Consume one exact Structured terminal postwrite into the live Wrapper latch.
pub(crate) fn terminalize_series_wrapper_obligation_v1<
    'next,
    A: AuthenticatedSeriesWrapperTerminalOwnerV1 + ?Sized,
>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    authenticated: AuthenticatedSeriesMarketLinkV1<'_>,
    owner: &A,
    rebound_output: &'next mut SeriesMarketLinkAccountV1,
) -> Outcome<AuthenticatedSeriesWrapperTerminalV1> {
    let binding = authenticated.state().binding();
    let semantic_before = authenticated
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let semantic_before_content = semantic_before.content_id();
    let admission_receipt = authenticated
        .state()
        .obligation_admission_receipt_id(SeriesLinkObligationV1::Wrapper);
    let owner_terminal_receipt_id = owner.owner_terminal_receipt_id()?;
    let structured_root_account = owner.structured_root_account()?;
    let structured_root_semantic_id = owner.structured_root_semantic_id()?;
    let structured_root_data_id = owner.structured_root_data_id()?;
    require_live_content_id(admission_receipt)?;
    require_live_content_id(owner_terminal_receipt_id)?;
    require_live_content_id(structured_root_semantic_id)?;
    require_live_content_id(structured_root_data_id)?;
    require(
        authenticated.is_writable()
            && authenticated.state().phase() == SeriesMarketLinkPhaseV1::Active
            && authenticated
                .state()
                .obligation_status(SeriesLinkObligationV1::Wrapper)
                == SeriesLinkObligationStatusV1::Live
            && owner_terminal_receipt_id != admission_receipt
            && structured_root_account != Pubkey::default()
            && structured_root_account != authenticated.account(),
        ClutchError::MismatchedState,
    )?;
    owner.authenticate_series_wrapper_terminal_owner_v1(
        authenticated.account(),
        binding.series_plan_id,
        binding.ordinal,
        binding.market_instance_id,
        binding.generation,
        admission_receipt,
        owner_terminal_receipt_id,
        structured_root_account,
        structured_root_semantic_id,
        structured_root_data_id,
    )?;
    let next_sequence = authenticated
        .state()
        .transition_sequence()
        .checked_add(1)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let projection = SeriesLinkObligationTerminalProjectionV1 {
        link_semantic_id: semantic_before,
        obligation: SeriesLinkObligationV1::Wrapper,
        disposition: SeriesLinkObligationDispositionV1::Terminal,
        link_transition_sequence: next_sequence,
        owner_terminal_receipt_id,
    };
    let projection_id = projection
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let successor = authenticated
        .state()
        .consume_obligation(projection)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let authentication_before = authenticated.authentication_id();
    let rebound = write_series_market_link_v1(
        program_id,
        account,
        authenticated,
        &successor,
        rebound_output,
    )?;
    let semantic_after = rebound
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let semantic_after_content = semantic_after.content_id();
    require(
        rebound
            .state()
            .obligation_status(SeriesLinkObligationV1::Wrapper)
            == SeriesLinkObligationStatusV1::Terminal
            && rebound
                .state()
                .obligation_terminal_receipt_id(SeriesLinkObligationV1::Wrapper)
                == projection_id,
        ClutchError::MismatchedState,
    )?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SERIES_WRAPPER_TERMINAL_AUTHENTICATION_DOMAIN_V1,
            account.key.as_ref(),
            &authentication_before.bytes(),
            &rebound.authentication_id().bytes(),
            &semantic_before_content.bytes(),
            &semantic_after_content.bytes(),
            &admission_receipt.bytes(),
            &owner_terminal_receipt_id.bytes(),
            &projection_id.bytes(),
            structured_root_account.as_ref(),
            &structured_root_semantic_id.bytes(),
            &structured_root_data_id.bytes(),
            &binding.series_plan_id.bytes(),
            &binding.ordinal.to_le_bytes(),
            &binding.market_instance_id.bytes(),
            &binding.generation.to_le_bytes(),
        ])
        .to_bytes(),
    );
    require_live_content_id(id)?;
    Ok(AuthenticatedSeriesWrapperTerminalV1 {
        id,
        link_account: *account.key,
        link_authentication_before: authentication_before,
        link_authentication_after: rebound.authentication_id(),
        link_semantic_before: semantic_before_content,
        link_semantic_after: semantic_after_content,
        wrapper_admission_receipt_id: admission_receipt,
        owner_terminal_receipt_id,
        product_terminal_projection: projection,
        structured_root_account,
        structured_root_semantic_id,
        structured_root_data_id,
    })
}

/// Promote an authenticated active link into a private Failure pin successor.
pub(crate) fn pin_series_market_link_failure_v1<'next>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    authenticated: AuthenticatedSeriesMarketLinkV1<'_>,
    failure_begin_receipt_id: ContentId,
    rebound_output: &'next mut SeriesMarketLinkAccountV1,
) -> Outcome<AuthenticatedSeriesMarketLinkV1<'next>> {
    require(
        authenticated.state().phase() == SeriesMarketLinkPhaseV1::Active,
        ClutchError::MismatchedState,
    )?;
    let successor = authenticated
        .state()
        .pin_failure_session(failure_begin_receipt_id)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    write_series_market_link_v1(
        program_id,
        account,
        authenticated,
        &successor,
        rebound_output,
    )
}

fn require_live_content_id(id: ContentId) -> Outcome<()> {
    require(!id.is_zero(), ClutchError::MismatchedState)
}

const fn series_link_status_byte(status: SeriesLinkObligationStatusV1) -> u8 {
    match status {
        SeriesLinkObligationStatusV1::CapabilityDisabled => 1,
        SeriesLinkObligationStatusV1::EnabledNeverFounded => 2,
        SeriesLinkObligationStatusV1::Live => 3,
        SeriesLinkObligationStatusV1::Terminal => 4,
    }
}

fn liveness_id(key: &Pubkey) -> LivenessId {
    LivenessId::from_bytes(key.to_bytes())
}
