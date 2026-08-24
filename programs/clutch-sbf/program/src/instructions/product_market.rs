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
use crate::instructions::product_series::{
    authenticate_series_artifact_accounts_v4, authenticate_series_lifecycle_replay_v1,
    complete_and_record_series_lifecycle_admission_v1, read_series_funding_account_v2,
    read_series_registry_account_v2, AuthenticatedSeriesFundingAccountV2,
    AuthenticatedSeriesLifecycleReplayPostwriteV1, AuthenticatedSeriesLifecycleReplayV1,
    AuthenticatedSeriesOccurrenceCompletionV2, SeriesLifecycleLinkRetirementAggregateAuthorityV1,
};
use crate::seeds;
use clutch_liveness::runtime_adapter_v1::{
    decode_runtime_policy_account_v1, RuntimePersistedAccountViewV1,
};
use clutch_liveness::runtime_v1::RuntimeCompartmentKindV1;
use clutch_liveness::Id as LivenessId;
use clutch_product_series::{
    authenticate_market_foundation_account_graph_bytes_v2,
    AuthenticatedMarketFamilyAuthorityV1, CompiledProductSeriesBundleV5, ContentId,
    MarketFamilyAggregatorV1, MarketFamilyV1, MarketFoundationAccountGraphV2,
    MarketFoundationScheduleV2, MarketFoundationSlotV2, MarketFoundationStepProjectionV2,
    MarketFoundingAbortProjectionV1, MarketInstanceTerminalProjectionV1, MarketInstanceV2Id,
    MarketLifecyclePhaseV1, MarketLifecycleReplayReceiptV1, MarketLifecycleRootV1,
    MarketResolutionActivationV1, MarketSharedCoreTerminalProjectionV1, MarketSharedCoreV1,
    SeriesAttachmentPlanV4, SeriesFundingComponentV2,
    SeriesFundingQuoteV4,
    SeriesLinkObligationAdmissionProjectionV1, SeriesLinkObligationDispositionV1,
    SeriesLinkObligationStatusV1, SeriesLinkObligationTerminalProjectionV1,
    SeriesLinkObligationV1, SeriesMarketDispositionV1, SeriesMarketLinkPhaseV1,
    SeriesMarketAdmissionProjectionV1, SeriesMarketLinkRetirementProjectionV1,
    SeriesMarketLinkV1, SeriesMarketLinkV1Id, SeriesPlanV5Id, SourceOccurrenceV1Id,
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

use super::product_series::AuthenticatedCompiledProductSeriesBundleV5;

const MARKET_LIFECYCLE_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/market-lifecycle-account-authentication/v1";
const MARKET_INSTANCE_TERMINAL_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/market-instance-terminal-authentication/v1";
const SERIES_WRAPPER_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/series-wrapper-authentication/v1";
const SERIES_WRAPPER_TERMINAL_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/series-wrapper-terminal-authentication/v1";
const SERIES_FAILURE_RELEASE_AUTHENTICATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/series-failure-release-authentication/v2";
const SERIES_FAILURE_RESOLUTION_LINK_PREAUTHORIZATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/series-failure-resolution-link-preauthorization/v1";
const SERIES_MARKET_LINK_RETIREMENT_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/series-market-link-retirement-authentication/v1";
const SERIES_MARKET_LINK_CLOSE_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/series-market-link-close-authentication/v1";
const SERIES_FAILURE_EXHAUSTED_LINK_PREAUTHORIZATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/series-failure-exhausted-link-preauthorization/v2";
const MARKET_RECOVERY_SCHEDULE_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/market-recovery-schedule-authentication/v1";
const MARKET_FOUNDATION_DEBIT_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/market-foundation-debit-authentication/v1";
const MARKET_FOUNDER_FOUNDATION_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/market-founder-foundation-authentication/v1";
const MARKET_FOUNDATION_STEP_AUTHENTICATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/market-foundation-step-authentication/v2";
const MARKET_FOUNDATION_ACTIVATION_AUTHENTICATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/market-foundation-activation-authentication/v2";
const SERIES_MARKET_LINK_ACTIVATION_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/series-market-link-activation-authentication/v1";
const PRODUCT_FOUNDATION_COMPLETION_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/product-foundation-completion-authentication/v1";
const PRODUCT_FOUNDER_ACTIVATION_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/product-founder-activation-authentication/v1";
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
const FAILURE_SHARED_CORE_TERMINAL_POSTWRITE_DOMAIN_V1: &[u8] =
    b"dragons-clutch/market-failure-shared-core-terminal-postwrite/v1";

/// Authenticate the fixed Product/General/Failure core of one immutable
/// foundation account graph against the executing program's canonical PDAs.
///
/// Fractional policy/ledger, outcome mints, and outcome custody remain owned
/// by their respective typed founding receipts because their PDA preimages
/// include identities not repeated by the Product root. This helper must not
/// guess those identities. It does make the common core non-caller-shaped,
/// including the reusable V2 Failure interval cell and append-only history.
pub(crate) fn require_canonical_market_foundation_core_v2(
    program_id: &Pubkey,
    root_account: Pubkey,
    account_graph: &MarketFoundationAccountGraphV2,
) -> Outcome<()> {
    let market = account_graph.market_instance_id.bytes();
    let generation = account_graph.generation;
    require_canonical_market_foundation_core_accounts_v2(
        program_id,
        root_account,
        market,
        generation,
        |slot| {
            account_graph
                .account(slot)
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))
        },
    )
}

fn require_canonical_market_foundation_core_accounts_v2<F>(
    program_id: &Pubkey,
    root_account: Pubkey,
    market: [u8; 32],
    generation: u64,
    mut account: F,
) -> Outcome<()>
where
    F: FnMut(MarketFoundationSlotV2) -> Outcome<ContentId>,
{
    let market_binding = account(MarketFoundationSlotV2::MarketBinding)?;
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
        account(MarketFoundationSlotV2::LifecycleRoot)?.bytes()
            == root_account.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    for (slot, expected) in fixed {
        require(
            account(slot)?.bytes() == expected.to_bytes(),
            ClutchError::MismatchedState,
        )?;
    }
    Ok(())
}

/// Authenticate the fixed Product/General/Failure graph core directly from a
/// caller-owned 1,544-byte preimage without constructing the full graph value.
fn require_canonical_market_foundation_core_bytes_v2(
    program_id: &Pubkey,
    root_account: Pubkey,
    authenticated: clutch_product_series::AuthenticatedMarketFoundationAccountGraphBytesV2<'_>,
) -> Outcome<()> {
    require_canonical_market_foundation_core_accounts_v2(
        program_id,
        root_account,
        authenticated.market_instance_id().bytes(),
        authenticated.generation(),
        |slot| {
            authenticated
                .account(slot)
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))
        },
    )
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

/// Narrow authority required to persist exactly one General-family admission.
///
/// The default refusal prevents a coherent set of caller-provided successor
/// fields from becoming write authority. The Product/General join implements
/// this only for its private, authenticated preauthorization plus General
/// postwrite capability.
pub(crate) trait AuthenticatedGeneralFamilyRootWriteV1 {
    #[allow(clippy::too_many_arguments)]
    fn authenticate_general_family_root_write(
        &self,
        _root_account: Pubkey,
        _root_pre_semantic_id: ContentId,
        _root_pre_data_id: ContentId,
        _root_pre_authentication_id: ContentId,
        _market_instance_id: MarketInstanceV2Id,
        _market_binding_id: ContentId,
        _generation: u64,
        _general_root_id: ContentId,
        _family_admission_sequence: u32,
        _product_preauthorization_id: ContentId,
        _general_postwrite_semantic_id: ContentId,
        _general_postwrite_data_id: ContentId,
        _general_postwrite_authentication_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
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

/// Exact Product-derived transition facts accepted by the bounded per-Series
/// lifecycle aggregate before one `0xad/1` link may be physically closed.
///
/// This is not caller authority and owns no parallel persisted state. Every
/// field is derived from hostile-authenticated Product root/link prestates and
/// their exact postwrites. The future counted Series aggregate receives this
/// value only through the crate-private default-refusing trait below.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SeriesMarketLinkRetirementPostwriteFactsV1 {
    root_account: Pubkey,
    root_binding_id: ContentId,
    root_authentication_before: ContentId,
    root_authentication_after: ContentId,
    root_semantic_before: ContentId,
    root_semantic_after: ContentId,
    root_data_before: ContentId,
    root_data_after: ContentId,
    root_transition_sequence_before: u64,
    root_transition_sequence_after: u64,
    admitted_series_links: u32,
    live_series_links_before: u32,
    live_series_links_after: u32,
    retired_series_links_before: u32,
    retired_series_links_after: u32,
    resolution_semantic_id: ContentId,
    resolution_data_id: ContentId,
    resolution_activation_receipt_id: ContentId,
    link_account: Pubkey,
    link_binding_id: ContentId,
    link_authentication_before: ContentId,
    link_authentication_after: ContentId,
    link_semantic_before: SeriesMarketLinkV1Id,
    link_semantic_retiring: SeriesMarketLinkV1Id,
    link_semantic_after: SeriesMarketLinkV1Id,
    link_data_before: ContentId,
    link_data_after: ContentId,
    link_transition_sequence_before: u64,
    link_transition_sequence_after: u64,
    retirement_projection_id: ContentId,
    market_admission_receipt_id: ContentId,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    series_plan_id: SeriesPlanV5Id,
    ordinal: u32,
    rent_refund_owner: Pubkey,
    neutral_lamport_sink: Pubkey,
    observed_balance_lamports: u64,
    rent_principal_lamports: u64,
    surplus_lamports: u64,
    refund_balance_before: u64,
    refund_balance_after: u64,
    sink_balance_before: u64,
    sink_balance_after: u64,
}

impl SeriesMarketLinkRetirementPostwriteFactsV1 {
    fn validate(self) -> Outcome<()> {
        require(
            self.root_account != Pubkey::default()
                && self.link_account != Pubkey::default()
                && self.root_account != self.link_account
                && self.rent_refund_owner != self.neutral_lamport_sink
                && self.rent_refund_owner != self.root_account
                && self.rent_refund_owner != self.link_account
                && self.neutral_lamport_sink != self.root_account
                && self.neutral_lamport_sink != self.link_account
                && self.generation != 0
                && self.rent_principal_lamports != 0
                && self.root_transition_sequence_after
                    == self
                        .root_transition_sequence_before
                        .checked_add(1)
                        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?
                && self.link_transition_sequence_after
                    == self
                        .link_transition_sequence_before
                        .checked_add(2)
                        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?
                && self.live_series_links_before != 0
                && self.live_series_links_after
                    == self
                        .live_series_links_before
                        .checked_sub(1)
                        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?
                && self.retired_series_links_after
                    == self
                        .retired_series_links_before
                        .checked_add(1)
                        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?
                && self.admitted_series_links != 0
                && self
                    .live_series_links_before
                    .checked_add(self.retired_series_links_before)
                    .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?
                    == self.admitted_series_links
                && self
                    .live_series_links_after
                    .checked_add(self.retired_series_links_after)
                    .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?
                    == self.admitted_series_links
                && self.observed_balance_lamports
                    == self
                        .rent_principal_lamports
                        .checked_add(self.surplus_lamports)
                        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?
                && self.refund_balance_after
                    == self
                        .refund_balance_before
                        .checked_add(self.rent_principal_lamports)
                        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?
                && self.sink_balance_after
                    == self
                        .sink_balance_before
                        .checked_add(self.surplus_lamports)
                        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?,
            ClutchError::MismatchedState,
        )?;
        for id in [
            self.root_binding_id,
            self.root_authentication_before,
            self.root_authentication_after,
            self.root_semantic_before,
            self.root_semantic_after,
            self.root_data_before,
            self.root_data_after,
            self.resolution_semantic_id,
            self.resolution_data_id,
            self.resolution_activation_receipt_id,
            self.link_binding_id,
            self.link_authentication_before,
            self.link_authentication_after,
            self.link_semantic_before.content_id(),
            self.link_semantic_retiring.content_id(),
            self.link_semantic_after.content_id(),
            self.link_data_before,
            self.link_data_after,
            self.retirement_projection_id,
            self.market_admission_receipt_id,
            self.market_instance_id.content_id(),
            self.series_plan_id.content_id(),
        ] {
            require_live_content_id(id)?;
        }
        require(
            self.link_semantic_before != self.link_semantic_retiring
                && self.link_semantic_retiring != self.link_semantic_after
                && self.link_semantic_before != self.link_semantic_after
                && self.root_semantic_before != self.root_semantic_after
                && self.root_authentication_before != self.root_authentication_after
                && self.link_authentication_before != self.link_authentication_after,
            ClutchError::MismatchedState,
        )
    }

    pub(crate) fn id(self) -> Outcome<ContentId> {
        self.validate()?;
        let id = ContentId::from_bytes(
            solana_sha256_hasher::hashv(&[
                SERIES_MARKET_LINK_RETIREMENT_AUTHENTICATION_DOMAIN_V1,
                self.root_account.as_ref(),
                &self.root_binding_id.bytes(),
                &self.root_authentication_before.bytes(),
                &self.root_authentication_after.bytes(),
                &self.root_semantic_before.bytes(),
                &self.root_semantic_after.bytes(),
                &self.root_data_before.bytes(),
                &self.root_data_after.bytes(),
                &self.root_transition_sequence_before.to_le_bytes(),
                &self.root_transition_sequence_after.to_le_bytes(),
                &self.admitted_series_links.to_le_bytes(),
                &self.live_series_links_before.to_le_bytes(),
                &self.live_series_links_after.to_le_bytes(),
                &self.retired_series_links_before.to_le_bytes(),
                &self.retired_series_links_after.to_le_bytes(),
                &self.resolution_semantic_id.bytes(),
                &self.resolution_data_id.bytes(),
                &self.resolution_activation_receipt_id.bytes(),
                self.link_account.as_ref(),
                &self.link_binding_id.bytes(),
                &self.link_authentication_before.bytes(),
                &self.link_authentication_after.bytes(),
                &self.link_semantic_before.bytes(),
                &self.link_semantic_retiring.bytes(),
                &self.link_semantic_after.bytes(),
                &self.link_data_before.bytes(),
                &self.link_data_after.bytes(),
                &self.link_transition_sequence_before.to_le_bytes(),
                &self.link_transition_sequence_after.to_le_bytes(),
                &self.retirement_projection_id.bytes(),
                &self.market_admission_receipt_id.bytes(),
                &self.market_instance_id.bytes(),
                &self.generation.to_le_bytes(),
                &self.series_plan_id.bytes(),
                &self.ordinal.to_le_bytes(),
                self.rent_refund_owner.as_ref(),
                self.neutral_lamport_sink.as_ref(),
                &self.observed_balance_lamports.to_le_bytes(),
                &self.rent_principal_lamports.to_le_bytes(),
                &self.surplus_lamports.to_le_bytes(),
                &self.refund_balance_before.to_le_bytes(),
                &self.refund_balance_after.to_le_bytes(),
                &self.sink_balance_before.to_le_bytes(),
                &self.sink_balance_after.to_le_bytes(),
            ])
            .to_bytes(),
        );
        require_live_content_id(id)?;
        Ok(id)
    }

    pub(crate) const fn root_account(self) -> Pubkey {
        self.root_account
    }
    pub(crate) const fn root_binding_id(self) -> ContentId {
        self.root_binding_id
    }
    pub(crate) const fn root_authentication_before(self) -> ContentId {
        self.root_authentication_before
    }
    pub(crate) const fn root_authentication_after(self) -> ContentId {
        self.root_authentication_after
    }
    pub(crate) const fn root_semantic_before(self) -> ContentId {
        self.root_semantic_before
    }
    pub(crate) const fn root_semantic_after(self) -> ContentId {
        self.root_semantic_after
    }
    pub(crate) const fn root_data_before(self) -> ContentId {
        self.root_data_before
    }
    pub(crate) const fn root_data_after(self) -> ContentId {
        self.root_data_after
    }
    pub(crate) const fn root_transition_sequence_before(self) -> u64 {
        self.root_transition_sequence_before
    }
    pub(crate) const fn link_account(self) -> Pubkey {
        self.link_account
    }
    pub(crate) const fn link_binding_id(self) -> ContentId {
        self.link_binding_id
    }
    pub(crate) const fn link_authentication_before(self) -> ContentId {
        self.link_authentication_before
    }
    pub(crate) const fn link_authentication_after(self) -> ContentId {
        self.link_authentication_after
    }
    pub(crate) const fn link_semantic_before(self) -> SeriesMarketLinkV1Id {
        self.link_semantic_before
    }
    pub(crate) const fn link_semantic_retiring(self) -> SeriesMarketLinkV1Id {
        self.link_semantic_retiring
    }
    pub(crate) const fn link_semantic_after(self) -> SeriesMarketLinkV1Id {
        self.link_semantic_after
    }
    pub(crate) const fn link_data_before(self) -> ContentId {
        self.link_data_before
    }
    pub(crate) const fn link_data_after(self) -> ContentId {
        self.link_data_after
    }
    pub(crate) const fn link_transition_sequence_before(self) -> u64 {
        self.link_transition_sequence_before
    }
    pub(crate) const fn market_instance_id(self) -> MarketInstanceV2Id {
        self.market_instance_id
    }
    pub(crate) const fn generation(self) -> u64 {
        self.generation
    }
    pub(crate) const fn series_plan_id(self) -> SeriesPlanV5Id {
        self.series_plan_id
    }
    pub(crate) const fn ordinal(self) -> u32 {
        self.ordinal
    }
    pub(crate) const fn retirement_projection_id(self) -> ContentId {
        self.retirement_projection_id
    }
    pub(crate) const fn market_admission_receipt_id(self) -> ContentId {
        self.market_admission_receipt_id
    }
    pub(crate) const fn root_transition_sequence_after(self) -> u64 {
        self.root_transition_sequence_after
    }
    pub(crate) const fn link_transition_sequence_after(self) -> u64 {
        self.link_transition_sequence_after
    }
    pub(crate) const fn admitted_series_links(self) -> u32 {
        self.admitted_series_links
    }
    pub(crate) const fn live_series_links_before(self) -> u32 {
        self.live_series_links_before
    }
    pub(crate) const fn live_series_links_after(self) -> u32 {
        self.live_series_links_after
    }
    pub(crate) const fn retired_series_links_before(self) -> u32 {
        self.retired_series_links_before
    }
    pub(crate) const fn retired_series_links_after(self) -> u32 {
        self.retired_series_links_after
    }
    pub(crate) const fn resolution_semantic_id(self) -> ContentId {
        self.resolution_semantic_id
    }
    pub(crate) const fn resolution_data_id(self) -> ContentId {
        self.resolution_data_id
    }
    pub(crate) const fn resolution_activation_receipt_id(self) -> ContentId {
        self.resolution_activation_receipt_id
    }
    pub(crate) const fn rent_refund_owner(self) -> Pubkey {
        self.rent_refund_owner
    }
    pub(crate) const fn neutral_lamport_sink(self) -> Pubkey {
        self.neutral_lamport_sink
    }
    pub(crate) const fn observed_balance_lamports(self) -> u64 {
        self.observed_balance_lamports
    }
    pub(crate) const fn rent_principal_lamports(self) -> u64 {
        self.rent_principal_lamports
    }
    pub(crate) const fn surplus_lamports(self) -> u64 {
        self.surplus_lamports
    }
    pub(crate) const fn refund_balance_before(self) -> u64 {
        self.refund_balance_before
    }
    pub(crate) const fn refund_balance_after(self) -> u64 {
        self.refund_balance_after
    }
    pub(crate) const fn sink_balance_before(self) -> u64 {
        self.sink_balance_before
    }
    pub(crate) const fn sink_balance_after(self) -> u64 {
        self.sink_balance_after
    }
}

/// Private receipt proving exact root/link retirement, physical principal
/// refund, and surplus disposition after a counted Series aggregate accepted
/// the same link exactly once.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedSeriesMarketLinkRetirementV1 {
    id: ContentId,
    aggregate_postwrite_id: ContentId,
    facts: SeriesMarketLinkRetirementPostwriteFactsV1,
}

impl AuthenticatedSeriesMarketLinkRetirementV1 {
    pub(crate) const fn id(self) -> ContentId {
        self.id
    }
    pub(crate) const fn aggregate_postwrite_id(self) -> ContentId {
        self.aggregate_postwrite_id
    }
    pub(crate) const fn facts(self) -> SeriesMarketLinkRetirementPostwriteFactsV1 {
        self.facts
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

/// Current RegistryV2/BundleV5 authority for phased founder continuation.
///
/// This capability carries no caller-decoded artifact bodies. It is minted
/// only by joining the hostile-authenticated Founding root, its exact founder
/// link, the loader-authenticated ReleaseV2/ProfileV4 receipt, and the
/// recompiled BundleV5 receipt. Permissionless continuation therefore does
/// not let a caller replace the Series, quote, attachment, compiler graph, or
/// central capability profile selected by the founder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedMarketFounderFoundationV1 {
    id: ContentId,
    root_account: Pubkey,
    root_authentication_id: ContentId,
    link_account: Pubkey,
    link_authentication_id: ContentId,
    founder_link_id: SeriesMarketLinkV1Id,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    series_plan_id: SeriesPlanV5Id,
    ordinal: u32,
    funding_terms_id: ContentId,
    funding_quote_id: ContentId,
    attachment_plan_id: ContentId,
    compiler_bundle_id: ContentId,
    registry_release_id: ContentId,
    capability_profile_id: ContentId,
    foundation_schedule_id: ContentId,
    foundation_account_graph_id: ContentId,
}

impl AuthenticatedMarketFounderFoundationV1 {
    pub(crate) const fn id(self) -> ContentId {
        self.id
    }

    pub(crate) const fn root_account(self) -> Pubkey {
        self.root_account
    }

    pub(crate) const fn root_authentication_id(self) -> ContentId {
        self.root_authentication_id
    }

    pub(crate) const fn link_account(self) -> Pubkey {
        self.link_account
    }

    pub(crate) const fn link_authentication_id(self) -> ContentId {
        self.link_authentication_id
    }

    pub(crate) const fn market_instance_id(self) -> MarketInstanceV2Id {
        self.market_instance_id
    }

    pub(crate) const fn generation(self) -> u64 {
        self.generation
    }

    pub(crate) const fn series_plan_id(self) -> SeriesPlanV5Id {
        self.series_plan_id
    }

    pub(crate) const fn ordinal(self) -> u32 {
        self.ordinal
    }

    pub(crate) const fn funding_quote_id(self) -> ContentId {
        self.funding_quote_id
    }

    pub(crate) const fn funding_terms_id(self) -> ContentId {
        self.funding_terms_id
    }

    pub(crate) const fn attachment_plan_id(self) -> ContentId {
        self.attachment_plan_id
    }

    pub(crate) const fn compiler_bundle_id(self) -> ContentId {
        self.compiler_bundle_id
    }

    pub(crate) const fn registry_release_id(self) -> ContentId {
        self.registry_release_id
    }

    pub(crate) const fn capability_profile_id(self) -> ContentId {
        self.capability_profile_id
    }

    pub(crate) const fn foundation_schedule_id(self) -> ContentId {
        self.foundation_schedule_id
    }

    pub(crate) const fn foundation_account_graph_id(self) -> ContentId {
        self.foundation_account_graph_id
    }

    fn authenticate_debit(self, debit: AuthenticatedMarketFoundationDebitV1) -> Outcome<()> {
        require(
            self.id != ContentId::ZERO
                && self.root_account == debit.root_account
                && self.root_authentication_id == debit.root_authentication_id
                && self.founder_link_id == debit.founder_link_id
                && self.market_instance_id == debit.market_instance_id
                && self.generation == debit.generation
                && self.funding_quote_id == debit.funding_quote_id
                && self.foundation_schedule_id == debit.foundation_schedule_id
                && self.foundation_account_graph_id == debit.foundation_account_graph_id,
            ClutchError::MismatchedState,
        )
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
    link_binding_id: ContentId,
    wrapper_obligation_configuration_id: ContentId,
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

/// Default-refusing Failure archive owner consumed by the Product link release.
///
/// The implementation must be minted only after the terminal reusable cell was
/// appended to the exact market history and the cell was hostile-reauthenticated
/// in canonical Idle. Product separately authenticates the live `0xad` prestate
/// and joins both owners on the complete Market/session tuple.
pub(crate) trait AuthenticatedSeriesFailureArchivePostwriteV2 {
    fn archive_postwrite_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }

    fn append_receipt_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }

    fn reset_receipt_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }

    fn market_instance_id(&self) -> Outcome<MarketInstanceV2Id> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }

    fn generation(&self) -> Outcome<u64> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }

    fn source_occurrence_id(&self) -> Outcome<SourceOccurrenceV1Id> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }

    fn session_binding_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }

    fn session_terminal_receipt_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }

    fn resolution_link_preauthorization_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }

    /// Exact Product link preauthorization under the typed release disposition.
    /// Existing Resolution owners inherit the historical getter; Exhausted
    /// owners must override this method directly.
    fn release_link_preauthorization_id(&self) -> Outcome<ContentId> {
        self.resolution_link_preauthorization_id()
    }

    /// Disjoint release path. Existing owners remain Resolution-only.
    fn release_disposition(&self) -> Outcome<FailureSessionReleaseDispositionV2> {
        Ok(FailureSessionReleaseDispositionV2::Resolved)
    }

    #[allow(clippy::too_many_arguments)]
    fn authenticate_series_failure_archive_postwrite_v2(
        &self,
        _archive_postwrite_id: ContentId,
        _append_receipt_id: ContentId,
        _reset_receipt_id: ContentId,
        _market_instance_id: MarketInstanceV2Id,
        _generation: u64,
        _source_occurrence_id: SourceOccurrenceV1Id,
        _session_binding_id: ContentId,
        _session_terminal_receipt_id: ContentId,
        _resolution_link_preauthorization_id: ContentId,
    ) -> Outcome<()> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }

    /// Authenticate the same archive under the exhaustive typed release seam.
    #[allow(clippy::too_many_arguments)]
    fn authenticate_series_failure_archive_release_postwrite_v2(
        &self,
        archive_postwrite_id: ContentId,
        append_receipt_id: ContentId,
        reset_receipt_id: ContentId,
        market_instance_id: MarketInstanceV2Id,
        generation: u64,
        source_occurrence_id: SourceOccurrenceV1Id,
        session_binding_id: ContentId,
        session_terminal_receipt_id: ContentId,
        disposition: FailureSessionReleaseDispositionV2,
        release_link_preauthorization_id: ContentId,
    ) -> Outcome<()> {
        require(
            disposition == FailureSessionReleaseDispositionV2::Resolved,
            ClutchError::MismatchedState,
        )?;
        self.authenticate_series_failure_archive_postwrite_v2(
            archive_postwrite_id,
            append_receipt_id,
            reset_receipt_id,
            market_instance_id,
            generation,
            source_occurrence_id,
            session_binding_id,
            session_terminal_receipt_id,
            release_link_preauthorization_id,
        )
    }
}

/// Scoped read projection for one physically writable Failure-session link.
///
/// The transaction must grant write privilege because the same atomic outer
/// releases the pin after Resolution, Source terminalization, and interval
/// archive. This value gives the Resolution writer only immutable facts from
/// the hostile-authenticated prestate; it is not a globally relaxed link
/// authenticator and cannot itself mutate `0xad`.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedWritableFailureResolutionLinkV1 {
    id: ContentId,
    link_account: Pubkey,
    owner_program: Pubkey,
    observed_lamports: u64,
    data_id: ContentId,
    authentication_id: ContentId,
    semantic_id: SeriesMarketLinkV1Id,
    market_root_account: Pubkey,
    market_binding_id: ContentId,
    series_plan_id: SeriesPlanV5Id,
    ordinal: u32,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    source_occurrence_id: SourceOccurrenceV1Id,
    transition_sequence: u64,
    failure_sessions_started: u32,
    active_failure_sessions: u32,
    failure_session_transcript_id: ContentId,
}

impl AuthenticatedWritableFailureResolutionLinkV1 {
    pub(crate) const fn id(&self) -> ContentId {
        self.id
    }

    pub(crate) const fn link_account(&self) -> Pubkey {
        self.link_account
    }

    pub(crate) const fn owner_program(&self) -> Pubkey {
        self.owner_program
    }

    pub(crate) const fn observed_lamports(&self) -> u64 {
        self.observed_lamports
    }

    pub(crate) const fn data_id(&self) -> ContentId {
        self.data_id
    }

    pub(crate) const fn authentication_id(&self) -> ContentId {
        self.authentication_id
    }

    pub(crate) const fn semantic_id(&self) -> SeriesMarketLinkV1Id {
        self.semantic_id
    }

    pub(crate) const fn market_root_account(&self) -> Pubkey {
        self.market_root_account
    }

    pub(crate) const fn market_binding_id(&self) -> ContentId {
        self.market_binding_id
    }

    pub(crate) const fn series_plan_id(&self) -> SeriesPlanV5Id {
        self.series_plan_id
    }

    pub(crate) const fn ordinal(&self) -> u32 {
        self.ordinal
    }

    pub(crate) const fn market_instance_id(&self) -> MarketInstanceV2Id {
        self.market_instance_id
    }

    pub(crate) const fn generation(&self) -> u64 {
        self.generation
    }

    pub(crate) const fn source_occurrence_id(&self) -> SourceOccurrenceV1Id {
        self.source_occurrence_id
    }

    pub(crate) const fn transition_sequence(&self) -> u64 {
        self.transition_sequence
    }

    pub(crate) const fn failure_sessions_started(&self) -> u32 {
        self.failure_sessions_started
    }

    pub(crate) const fn failure_session_transcript_id(&self) -> ContentId {
        self.failure_session_transcript_id
    }
}

fn writable_failure_resolution_link_preauthorization_id_v1(
    program_id: &Pubkey,
    value: &AuthenticatedWritableFailureResolutionLinkV1,
) -> ContentId {
    ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SERIES_FAILURE_RESOLUTION_LINK_PREAUTHORIZATION_DOMAIN_V1,
            program_id.as_ref(),
            value.link_account.as_ref(),
            &value.owner_program.to_bytes(),
            &value.observed_lamports.to_le_bytes(),
            &value.data_id.bytes(),
            &value.authentication_id.bytes(),
            &value.semantic_id.bytes(),
            value.market_root_account.as_ref(),
            &value.market_binding_id.bytes(),
            &value.series_plan_id.bytes(),
            &value.ordinal.to_le_bytes(),
            &value.market_instance_id.bytes(),
            &value.generation.to_le_bytes(),
            &value.source_occurrence_id.bytes(),
            &value.transition_sequence.to_le_bytes(),
            &value.failure_sessions_started.to_le_bytes(),
            &value.failure_session_transcript_id.bytes(),
            &value.active_failure_sessions.to_le_bytes(),
        ])
        .to_bytes(),
    )
}

/// Exhaustive reason one pinned Failure session may release its Product link.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FailureSessionReleaseDispositionV2 {
    /// Resolution was atomically persisted before archive/reset/release.
    Resolved,
    /// The finite unresolved session exhausted its authenticated schedule.
    Exhausted,
}

impl FailureSessionReleaseDispositionV2 {
    const fn wire_byte(self) -> u8 {
        match self {
            Self::Resolved => 1,
            Self::Exhausted => 2,
        }
    }
}

fn require_failure_session_release_disposition_v2(
    authenticated: FailureSessionReleaseDispositionV2,
    archived: FailureSessionReleaseDispositionV2,
) -> Outcome<()> {
    require(authenticated == archived, ClutchError::MismatchedState)
}

/// Product-authenticated writable pinned link for the unresolved exhausted path.
///
/// Unlike Resolution, the shared root remains read-only and its resolution
/// triple must stay canonical zero throughout action13. The exact root and
/// root prestate is committed by the private ID and the exact link facts are
/// retained so an archive receipt cannot be spliced across Markets, sessions,
/// or a later root transition.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedWritableFailureExhaustedLinkV2 {
    id: ContentId,
    root_account: Pubkey,
    link_account: Pubkey,
    owner_program: Pubkey,
    observed_lamports: u64,
    data_id: ContentId,
    authentication_id: ContentId,
    semantic_id: SeriesMarketLinkV1Id,
    market_binding_id: ContentId,
    series_plan_id: SeriesPlanV5Id,
    ordinal: u32,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    source_occurrence_id: SourceOccurrenceV1Id,
    transition_sequence: u64,
    failure_sessions_started: u32,
    failure_session_transcript_id: ContentId,
}

/// Exact Product release preauthorization under a disjoint typed disposition.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum AuthenticatedWritableFailureSessionReleaseLinkV2 {
    Resolved(AuthenticatedWritableFailureResolutionLinkV1),
    Exhausted(AuthenticatedWritableFailureExhaustedLinkV2),
}

impl AuthenticatedWritableFailureSessionReleaseLinkV2 {
    pub(crate) const fn resolved(
        value: AuthenticatedWritableFailureResolutionLinkV1,
    ) -> Self {
        Self::Resolved(value)
    }

    pub(crate) const fn disposition(&self) -> FailureSessionReleaseDispositionV2 {
        match self {
            Self::Resolved(_) => FailureSessionReleaseDispositionV2::Resolved,
            Self::Exhausted(_) => FailureSessionReleaseDispositionV2::Exhausted,
        }
    }

    pub(crate) const fn id(&self) -> ContentId {
        match self {
            Self::Resolved(value) => value.id(),
            Self::Exhausted(value) => value.id,
        }
    }

    pub(crate) const fn link_account(&self) -> Pubkey {
        match self {
            Self::Resolved(value) => value.link_account(),
            Self::Exhausted(value) => value.link_account,
        }
    }

    pub(crate) const fn owner_program(&self) -> Pubkey {
        match self {
            Self::Resolved(value) => value.owner_program(),
            Self::Exhausted(value) => value.owner_program,
        }
    }

    pub(crate) const fn observed_lamports(&self) -> u64 {
        match self {
            Self::Resolved(value) => value.observed_lamports(),
            Self::Exhausted(value) => value.observed_lamports,
        }
    }

    pub(crate) const fn data_id(&self) -> ContentId {
        match self {
            Self::Resolved(value) => value.data_id(),
            Self::Exhausted(value) => value.data_id,
        }
    }

    pub(crate) const fn authentication_id(&self) -> ContentId {
        match self {
            Self::Resolved(value) => value.authentication_id(),
            Self::Exhausted(value) => value.authentication_id,
        }
    }

    pub(crate) const fn semantic_id(&self) -> SeriesMarketLinkV1Id {
        match self {
            Self::Resolved(value) => value.semantic_id(),
            Self::Exhausted(value) => value.semantic_id,
        }
    }

    pub(crate) const fn market_root_account(&self) -> Pubkey {
        match self {
            Self::Resolved(value) => value.market_root_account(),
            Self::Exhausted(value) => value.root_account,
        }
    }

    pub(crate) const fn market_binding_id(&self) -> ContentId {
        match self {
            Self::Resolved(value) => value.market_binding_id(),
            Self::Exhausted(value) => value.market_binding_id,
        }
    }

    pub(crate) const fn series_plan_id(&self) -> SeriesPlanV5Id {
        match self {
            Self::Resolved(value) => value.series_plan_id(),
            Self::Exhausted(value) => value.series_plan_id,
        }
    }

    pub(crate) const fn ordinal(&self) -> u32 {
        match self {
            Self::Resolved(value) => value.ordinal(),
            Self::Exhausted(value) => value.ordinal,
        }
    }

    pub(crate) const fn market_instance_id(&self) -> MarketInstanceV2Id {
        match self {
            Self::Resolved(value) => value.market_instance_id(),
            Self::Exhausted(value) => value.market_instance_id,
        }
    }

    pub(crate) const fn generation(&self) -> u64 {
        match self {
            Self::Resolved(value) => value.generation(),
            Self::Exhausted(value) => value.generation,
        }
    }

    pub(crate) const fn source_occurrence_id(&self) -> SourceOccurrenceV1Id {
        match self {
            Self::Resolved(value) => value.source_occurrence_id(),
            Self::Exhausted(value) => value.source_occurrence_id,
        }
    }

    pub(crate) const fn transition_sequence(&self) -> u64 {
        match self {
            Self::Resolved(value) => value.transition_sequence(),
            Self::Exhausted(value) => value.transition_sequence,
        }
    }

    pub(crate) const fn failure_sessions_started(&self) -> u32 {
        match self {
            Self::Resolved(value) => value.failure_sessions_started(),
            Self::Exhausted(value) => value.failure_sessions_started,
        }
    }

    pub(crate) const fn failure_session_transcript_id(&self) -> ContentId {
        match self {
            Self::Resolved(value) => value.failure_session_transcript_id(),
            Self::Exhausted(value) => value.failure_session_transcript_id,
        }
    }
}

/// Default-refusing Failure Begin owner for the sole exclusive link pin.
pub(crate) trait AuthenticatedSeriesFailureSessionBeginV2 {
    #[allow(clippy::too_many_arguments)]
    fn authenticate_series_failure_session_begin_v2(
        &self,
        _root_account: Pubkey,
        _root_authentication_id: ContentId,
        _link_account: Pubkey,
        _link_authentication_id: ContentId,
        _series_plan_id: SeriesPlanV5Id,
        _ordinal: u32,
        _market_instance_id: MarketInstanceV2Id,
        _generation: u64,
        _source_occurrence_id: SourceOccurrenceV1Id,
        _begin_admission_receipt_id: ContentId,
    ) -> Outcome<()> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
}

/// Private Product postwrite proving one exact Failure session pin was released.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedSeriesFailureSessionReleaseV2 {
    id: ContentId,
    link_account: Pubkey,
    link_authentication_before: ContentId,
    link_authentication_after: ContentId,
    link_semantic_before: ContentId,
    link_semantic_after: ContentId,
    transition_sequence_before: u64,
    transition_sequence_after: u64,
    failure_session_transcript_before: ContentId,
    failure_session_transcript_after: ContentId,
    session_terminal_receipt_id: ContentId,
    archive_postwrite_id: ContentId,
    append_receipt_id: ContentId,
    reset_receipt_id: ContentId,
    release_link_preauthorization_id: ContentId,
    release_disposition: FailureSessionReleaseDispositionV2,
}

impl AuthenticatedSeriesFailureSessionReleaseV2 {
    pub(crate) const fn id(&self) -> ContentId {
        self.id
    }

    pub(crate) const fn link_account(&self) -> Pubkey {
        self.link_account
    }

    pub(crate) const fn link_authentication_before(&self) -> ContentId {
        self.link_authentication_before
    }

    pub(crate) const fn link_authentication_after(&self) -> ContentId {
        self.link_authentication_after
    }

    pub(crate) const fn link_semantic_before(&self) -> ContentId {
        self.link_semantic_before
    }

    pub(crate) const fn link_semantic_after(&self) -> ContentId {
        self.link_semantic_after
    }

    pub(crate) const fn transition_sequence_before(&self) -> u64 {
        self.transition_sequence_before
    }

    pub(crate) const fn transition_sequence_after(&self) -> u64 {
        self.transition_sequence_after
    }

    pub(crate) const fn failure_session_transcript_before(&self) -> ContentId {
        self.failure_session_transcript_before
    }

    pub(crate) const fn failure_session_transcript_after(&self) -> ContentId {
        self.failure_session_transcript_after
    }

    pub(crate) const fn session_terminal_receipt_id(&self) -> ContentId {
        self.session_terminal_receipt_id
    }

    pub(crate) const fn archive_postwrite_id(&self) -> ContentId {
        self.archive_postwrite_id
    }

    pub(crate) const fn append_receipt_id(&self) -> ContentId {
        self.append_receipt_id
    }

    pub(crate) const fn reset_receipt_id(&self) -> ContentId {
        self.reset_receipt_id
    }

    pub(crate) const fn release_link_preauthorization_id(&self) -> ContentId {
        self.release_link_preauthorization_id
    }

    pub(crate) const fn release_disposition(&self) -> FailureSessionReleaseDispositionV2 {
        self.release_disposition
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
    /// Immutable complete Series-link binding identity.
    pub const fn link_binding_id(self) -> ContentId {
        self.link_binding_id
    }
    /// Immutable configuration governing the Wrapper obligation.
    pub const fn wrapper_obligation_configuration_id(self) -> ContentId {
        self.wrapper_obligation_configuration_id
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
    let link_binding_id = binding
        .id()
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
        link_binding_id,
        wrapper_obligation_configuration_id: binding.obligation_configuration_id.content_id(),
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
    let schedule_id = schedule
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let account_graph_id = account_graph
        .id(schedule)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    authenticate_market_foundation_preallocation_join_v2(
        root,
        account,
        schedule,
        slot,
        schedule_id,
        account_graph_id,
        account_graph.market_instance_id,
        account_graph.generation,
        account_graph
            .account(slot)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
    )
}

/// Stack-bounded retained-slot authentication from one hostile caller-owned
/// graph preimage. The pure Product owner validates the complete 1,544 bytes;
/// this adapter only joins that authenticated view to the live root and PDA.
pub(crate) fn authenticate_market_foundation_preallocation_from_bytes_v2(
    root: AuthenticatedMarketLifecycleRootV1<'_>,
    account: &AccountInfo<'_>,
    schedule: &MarketFoundationScheduleV2,
    account_graph_bytes: &[u8],
    slot: MarketFoundationSlotV2,
) -> Outcome<AuthenticatedMarketFoundationPreallocationV2> {
    let graph = authenticate_market_foundation_account_graph_bytes_v2(
        account_graph_bytes,
        schedule,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require_canonical_market_foundation_core_bytes_v2(
        root.owner_program(),
        root.account(),
        graph,
    )?;
    authenticate_market_foundation_preallocation_join_v2(
        root,
        account,
        schedule,
        slot,
        graph.foundation_schedule_id(),
        graph.graph_id(),
        graph.market_instance_id(),
        graph.generation(),
        graph
            .account(slot)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
    )
}

#[allow(clippy::too_many_arguments)]
fn authenticate_market_foundation_preallocation_join_v2(
    root: AuthenticatedMarketLifecycleRootV1<'_>,
    account: &AccountInfo<'_>,
    schedule: &MarketFoundationScheduleV2,
    slot: MarketFoundationSlotV2,
    schedule_id: clutch_product_series::MarketFoundationScheduleV2Id,
    account_graph_id: clutch_product_series::MarketFoundationAccountGraphV2Id,
    graph_market_instance_id: MarketInstanceV2Id,
    graph_generation: u64,
    graph_slot_account: ContentId,
) -> Outcome<AuthenticatedMarketFoundationPreallocationV2> {
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
    require(
        schedule_id == binding.foundation_schedule_id
            && account_graph_id == binding.foundation_account_graph_id
            && graph_market_instance_id == binding.market_instance_id
            && graph_generation == binding.generation
            && graph_slot_account == ContentId::from_bytes(account.key.to_bytes())
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

/// Join the exact current Series compiler/capability graph to one Founding
/// Market and its sole founder link.
pub(crate) fn authenticate_market_founder_foundation_v1(
    program_id: &Pubkey,
    root: AuthenticatedMarketLifecycleRootV1<'_>,
    founder_link: AuthenticatedSeriesMarketLinkV1<'_>,
    capability: AuthenticatedRegistryCapabilityV3,
    compiler_bundle: AuthenticatedCompiledProductSeriesBundleV5,
) -> Outcome<AuthenticatedMarketFounderFoundationV1> {
    authenticate_market_founder_foundation_with_link_privilege_v1(
        program_id,
        root,
        founder_link,
        capability,
        compiler_bundle,
        false,
    )
}

fn authenticate_market_founder_foundation_with_link_privilege_v1(
    program_id: &Pubkey,
    root: AuthenticatedMarketLifecycleRootV1<'_>,
    founder_link: AuthenticatedSeriesMarketLinkV1<'_>,
    capability: AuthenticatedRegistryCapabilityV3,
    compiler_bundle: AuthenticatedCompiledProductSeriesBundleV5,
    expected_link_writable: bool,
) -> Outcome<AuthenticatedMarketFounderFoundationV1> {
    let binding = root.state().binding();
    let link_binding = founder_link.state().binding();
    let founder_link_id = founder_link
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let market_binding_id = binding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let bundle = compiler_bundle.bundle();
    let compiler_bundle_id = compiler_bundle.bundle_id().content_id();
    require(
        root.is_writable()
            && root.state().phase() == MarketLifecyclePhaseV1::Founding
            && founder_link.state().phase() == SeriesMarketLinkPhaseV1::PendingMarket
            && founder_link.is_writable() == expected_link_writable
            && founder_link_id == root.state().capital().founder_link_id
            && link_binding.disposition == SeriesMarketDispositionV1::Founder
            && link_binding.market_instance_id == binding.market_instance_id
            && link_binding.generation == binding.generation
            && link_binding.market_binding_id == market_binding_id
            && link_binding.market_root_account_id.bytes() == root.account().to_bytes()
            && link_binding.compiler_output_id == compiler_bundle_id
            && link_binding.series_plan_id == bundle.series_plan_id
            && link_binding.funding_terms_id == bundle.funding_terms_id
            && link_binding.funding_quote_id == bundle.funding_quote_id.content_id()
            && link_binding.attachment_plan_id == bundle.attachment_plan_id.content_id()
            && link_binding.capability_profile_id
                == bundle.capability_profile_id.content_id()
            && capability.program_account() == *program_id
            && capability.series_registry_account() != root.account()
            && capability.series_registry_account() != founder_link.account()
            && capability.series_plan_id() == link_binding.series_plan_id
            && capability.funding_terms_id() == link_binding.funding_terms_id
            && capability.compiler_bundle_id() == compiler_bundle_id
            && capability.registry_release_id() == binding.registry_release_id
            && capability.capability_profile_id() == binding.capability_profile_id
            && bundle.registry_release_id == binding.registry_release_id
            && bundle.capability_profile_id.content_id() == binding.capability_profile_id
            && bundle.product_template_id.content_id() == binding.product_template_id
            && bundle.native_claim_basis_id.content_id() == binding.native_claim_basis_id
            && bundle.evidence_only_recovery_policy_id.content_id()
                == binding.recovery_policy_id
            && bundle.price_measure_policy_id.content_id()
                == binding.price_measure_policy_id
            && bundle.market_genesis_profile_id.content_id()
                == binding.market_genesis_profile_id
            && bundle.source_plane_contract_id == binding.source_plane_contract_id
            && bundle.source_spec_id == binding.source_spec_id,
        ClutchError::MismatchedState,
    )?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            MARKET_FOUNDER_FOUNDATION_AUTHENTICATION_DOMAIN_V1,
            program_id.as_ref(),
            root.account().as_ref(),
            &root.authentication_id().bytes(),
            founder_link.account().as_ref(),
            &founder_link.authentication_id().bytes(),
            &founder_link_id.bytes(),
            capability.series_registry_account().as_ref(),
            capability.programdata_account().as_ref(),
            capability.release_artifact_account().as_ref(),
            capability.profile_artifact_account().as_ref(),
            compiler_bundle.artifact_account().as_ref(),
            &compiler_bundle_id.bytes(),
            &binding.market_instance_id.bytes(),
            &binding.generation.to_le_bytes(),
            &link_binding.series_plan_id.bytes(),
            &link_binding.ordinal.to_le_bytes(),
            &link_binding.funding_terms_id.bytes(),
            &link_binding.funding_quote_id.bytes(),
            &link_binding.attachment_plan_id.bytes(),
            &binding.registry_release_id.bytes(),
            &binding.capability_profile_id.bytes(),
            &binding.foundation_schedule_id.bytes(),
            &binding.foundation_account_graph_id.bytes(),
        ])
        .to_bytes(),
    );
    require_live_content_id(id)?;
    Ok(AuthenticatedMarketFounderFoundationV1 {
        id,
        root_account: root.account(),
        root_authentication_id: root.authentication_id(),
        link_account: founder_link.account(),
        link_authentication_id: founder_link.authentication_id(),
        founder_link_id,
        market_instance_id: binding.market_instance_id,
        generation: binding.generation,
        series_plan_id: link_binding.series_plan_id,
        ordinal: link_binding.ordinal,
        funding_terms_id: link_binding.funding_terms_id.content_id(),
        funding_quote_id: link_binding.funding_quote_id,
        attachment_plan_id: link_binding.attachment_plan_id,
        compiler_bundle_id,
        registry_release_id: binding.registry_release_id,
        capability_profile_id: binding.capability_profile_id,
        foundation_schedule_id: binding.foundation_schedule_id.content_id(),
        foundation_account_graph_id: binding.foundation_account_graph_id.content_id(),
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
    founder: AuthenticatedMarketFounderFoundationV1,
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
        founder.id != ContentId::ZERO
            && founder.root_account == root.account()
            && founder.root_authentication_id == root.authentication_id()
            && founder.link_account == founder_link.account()
            && founder.link_authentication_id == founder_link.authentication_id()
            && founder.founder_link_id == founder_link_id
            && founder.market_instance_id == binding.market_instance_id
            && founder.generation == binding.generation
            && founder.series_plan_id == link_binding.series_plan_id
            && founder.ordinal == link_binding.ordinal
            && founder.funding_terms_id == link_binding.funding_terms_id.content_id()
            && founder.funding_quote_id == link_binding.funding_quote_id
            && founder.attachment_plan_id == link_binding.attachment_plan_id
            && founder.compiler_bundle_id == link_binding.compiler_output_id
            && founder.registry_release_id == binding.registry_release_id
            && founder.capability_profile_id == binding.capability_profile_id
            && founder.foundation_schedule_id == binding.foundation_schedule_id.content_id()
            && founder.foundation_account_graph_id
                == binding.foundation_account_graph_id.content_id()
            && founder_link_id == capital.founder_link_id
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

/// Default-refusing family owner for one exact Foundation postwrite.
///
/// Implementations stay beside the family codec/CPI adapter and may succeed
/// only from their private postwrite receipt. A content ID or decoded body by
/// itself cannot authorize Product's bitmap/transcript transition.
pub(crate) trait AuthenticatedMarketFoundationStepPostwriteV2 {
    fn accepted_poststate_receipt_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }

    #[allow(clippy::too_many_arguments)]
    fn authenticate_market_foundation_step_postwrite_v2(
        &self,
        _founder_authorization_id: ContentId,
        _debit_id: ContentId,
        _market_instance_id: MarketInstanceV2Id,
        _generation: u64,
        _slot: MarketFoundationSlotV2,
        _account: Pubkey,
        _principal_lamports: u64,
        _donation_floor_lamports: u64,
        _observed_balance_lamports: u64,
        _rent_refund_owner: Pubkey,
        _neutral_lamport_sink: Pubkey,
        _accepted_poststate_receipt_id: ContentId,
    ) -> Outcome<()> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
}

/// Private Product receipt for one exact accepted Foundation transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedMarketFoundationStepV2 {
    id: ContentId,
    founder_authorization_id: ContentId,
    debit_id: ContentId,
    accepted_poststate_receipt_id: ContentId,
    root_account: Pubkey,
    root_semantic_before: ContentId,
    root_semantic_after: ContentId,
    root_data_before: ContentId,
    root_data_after: ContentId,
    root_authentication_before: ContentId,
    root_authentication_after: ContentId,
    slot: MarketFoundationSlotV2,
    account: Pubkey,
}

impl AuthenticatedMarketFoundationStepV2 {
    pub(crate) const fn id(self) -> ContentId {
        self.id
    }

    pub(crate) const fn founder_authorization_id(self) -> ContentId {
        self.founder_authorization_id
    }

    pub(crate) const fn debit_id(self) -> ContentId {
        self.debit_id
    }

    pub(crate) const fn accepted_poststate_receipt_id(self) -> ContentId {
        self.accepted_poststate_receipt_id
    }

    pub(crate) const fn root_account(self) -> Pubkey {
        self.root_account
    }

    pub(crate) const fn root_semantic_before(self) -> ContentId {
        self.root_semantic_before
    }

    pub(crate) const fn root_semantic_after(self) -> ContentId {
        self.root_semantic_after
    }

    pub(crate) const fn root_data_before(self) -> ContentId {
        self.root_data_before
    }

    pub(crate) const fn root_data_after(self) -> ContentId {
        self.root_data_after
    }

    pub(crate) const fn root_authentication_before(self) -> ContentId {
        self.root_authentication_before
    }

    pub(crate) const fn root_authentication_after(self) -> ContentId {
        self.root_authentication_after
    }

    pub(crate) const fn slot(self) -> MarketFoundationSlotV2 {
        self.slot
    }

    pub(crate) const fn account(self) -> Pubkey {
        self.account
    }

    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(crate) const fn test_only(
        id: ContentId,
        founder_authorization_id: ContentId,
        debit_id: ContentId,
        accepted_poststate_receipt_id: ContentId,
        root_account: Pubkey,
        root_semantic_before: ContentId,
        root_semantic_after: ContentId,
        root_data_before: ContentId,
        root_data_after: ContentId,
        root_authentication_before: ContentId,
        root_authentication_after: ContentId,
        slot: MarketFoundationSlotV2,
        account: Pubkey,
    ) -> Self {
        Self {
            id,
            founder_authorization_id,
            debit_id,
            accepted_poststate_receipt_id,
            root_account,
            root_semantic_before,
            root_semantic_after,
            root_data_before,
            root_data_after,
            root_authentication_before,
            root_authentication_after,
            slot,
            account,
        }
    }
}

/// Default-refusing owner of the exhaustive Market-core activation postwrite.
///
/// A family adapter may implement this only after every itemized Foundation
/// slot has been reauthenticated and the pure accepted Market-core composite
/// has joined the same root transcript. Product independently derives the
/// founder admission receipt and exact root successor.
pub(crate) trait AuthenticatedMarketFoundationActivationPostwriteV2 {
    fn accepted_market_core_receipt_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }

    #[allow(clippy::too_many_arguments)]
    fn authenticate_market_foundation_activation_postwrite_v2(
        &self,
        _founder_authorization_id: ContentId,
        _root_account: Pubkey,
        _root_authentication_id: ContentId,
        _market_instance_id: MarketInstanceV2Id,
        _generation: u64,
        _founder_link_account: Pubkey,
        _founder_link_semantic_id: SeriesMarketLinkV1Id,
        _foundation_schedule_id: ContentId,
        _foundation_account_graph_id: ContentId,
        _foundation_transcript_id: ContentId,
        _accepted_market_core_receipt_id: ContentId,
    ) -> Outcome<()> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
}

/// Product-owned durable completion authority derived only from the exact
/// hostile-reopened Founding root and its still-pending founder link.
///
/// Family owners have already committed their accepted postwrites into the
/// root's foundation transcript and family aggregator. Reopening that complete
/// root is the canonical durable join; no ephemeral family receipt or caller
/// content ID is allowed to substitute for it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AuthenticatedProductFoundationCompletionV1 {
    id: ContentId,
    founder_authorization_id: ContentId,
    root_account: Pubkey,
    root_authentication_id: ContentId,
    root_semantic_id: ContentId,
    root_data_id: ContentId,
    founder_link_account: Pubkey,
    founder_link_semantic_id: SeriesMarketLinkV1Id,
    founder_link_authentication_id: ContentId,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    foundation_schedule_id: ContentId,
    foundation_account_graph_id: ContentId,
    foundation_transcript_id: ContentId,
}

impl AuthenticatedMarketFoundationActivationPostwriteV2
    for AuthenticatedProductFoundationCompletionV1
{
    fn accepted_market_core_receipt_id(&self) -> Outcome<ContentId> {
        require_live_content_id(self.id)?;
        Ok(self.id)
    }

    #[allow(clippy::too_many_arguments)]
    fn authenticate_market_foundation_activation_postwrite_v2(
        &self,
        founder_authorization_id: ContentId,
        root_account: Pubkey,
        root_authentication_id: ContentId,
        market_instance_id: MarketInstanceV2Id,
        generation: u64,
        founder_link_account: Pubkey,
        founder_link_semantic_id: SeriesMarketLinkV1Id,
        foundation_schedule_id: ContentId,
        foundation_account_graph_id: ContentId,
        foundation_transcript_id: ContentId,
        accepted_market_core_receipt_id: ContentId,
    ) -> Outcome<()> {
        require(
            accepted_market_core_receipt_id == self.id
                && founder_authorization_id == self.founder_authorization_id
                && root_account == self.root_account
                && root_authentication_id == self.root_authentication_id
                && market_instance_id == self.market_instance_id
                && generation == self.generation
                && founder_link_account == self.founder_link_account
                && founder_link_semantic_id == self.founder_link_semantic_id
                && foundation_schedule_id == self.foundation_schedule_id
                && foundation_account_graph_id == self.foundation_account_graph_id
                && foundation_transcript_id == self.foundation_transcript_id,
            ClutchError::MismatchedState,
        )
    }
}

/// Private Product receipt for the sole Founding-to-Active transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedMarketFoundationActivationV2 {
    id: ContentId,
    founder_authorization_id: ContentId,
    root_account: Pubkey,
    root_authentication_before: ContentId,
    root_authentication_after: ContentId,
    founder_link_account: Pubkey,
    founder_link_semantic_id: SeriesMarketLinkV1Id,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    market_admission_sequence: u64,
    market_admission_receipt_id: ContentId,
    accepted_market_core_receipt_id: ContentId,
}

impl AuthenticatedMarketFoundationActivationV2 {
    pub(crate) const fn id(self) -> ContentId {
        self.id
    }

    pub(crate) const fn root_account(self) -> Pubkey {
        self.root_account
    }

    pub(crate) const fn founder_link_account(self) -> Pubkey {
        self.founder_link_account
    }

    pub(crate) const fn market_instance_id(self) -> MarketInstanceV2Id {
        self.market_instance_id
    }

    pub(crate) const fn generation(self) -> u64 {
        self.generation
    }

    pub(crate) const fn market_admission_receipt_id(self) -> ContentId {
        self.market_admission_receipt_id
    }
}

/// Default-refusing owner for an Active-root converger admission.
///
/// The concrete outer composer must own the live FundingV2 reservation and
/// Source occurrence. Product rechecks the current RegistryV2/BundleV5 graph,
/// derives the exact root admission projection, and writes both `0xaa` and
/// `0xad`; a caller-provided link body is never sufficient authority.
pub(crate) trait AuthenticatedConvergerSeriesMarketAdmissionV1 {
    #[allow(clippy::too_many_arguments)]
    fn authenticate_converger_series_market_admission_v1(
        &self,
        _root_account: Pubkey,
        _root_authentication_id: ContentId,
        _link_account: Pubkey,
        _link_authentication_id: ContentId,
        _link_semantic_id: SeriesMarketLinkV1Id,
        _series_plan_id: SeriesPlanV5Id,
        _ordinal: u32,
        _market_instance_id: MarketInstanceV2Id,
        _generation: u64,
        _source_occurrence_id: SourceOccurrenceV1Id,
        _funding_reservation_receipt_id: ContentId,
        _market_admission_sequence: u64,
        _market_admission_receipt_id: ContentId,
    ) -> Outcome<()> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
}

/// Private Product receipt for one exact PendingMarket-to-Active link.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedSeriesMarketLinkActivationV1 {
    id: ContentId,
    root_account: Pubkey,
    root_authentication_before: ContentId,
    root_authentication_after: ContentId,
    link_account: Pubkey,
    link_authentication_before: ContentId,
    link_authentication_after: ContentId,
    link_semantic_before: SeriesMarketLinkV1Id,
    link_semantic_after: SeriesMarketLinkV1Id,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    series_plan_id: SeriesPlanV5Id,
    ordinal: u32,
    source_occurrence_id: SourceOccurrenceV1Id,
    disposition: SeriesMarketDispositionV1,
    funding_reservation_receipt_id: ContentId,
    market_admission_sequence: u64,
    market_admission_receipt_id: ContentId,
    compiler_bundle_id: ContentId,
    capability_profile_id: ContentId,
}

impl AuthenticatedSeriesMarketLinkActivationV1 {
    pub(crate) const fn id(self) -> ContentId {
        self.id
    }

    pub(crate) const fn root_account(self) -> Pubkey {
        self.root_account
    }

    pub(crate) const fn link_account(self) -> Pubkey {
        self.link_account
    }

    pub(crate) const fn link_semantic_before(self) -> SeriesMarketLinkV1Id {
        self.link_semantic_before
    }

    pub(crate) const fn market_instance_id(self) -> MarketInstanceV2Id {
        self.market_instance_id
    }

    pub(crate) const fn generation(self) -> u64 {
        self.generation
    }

    pub(crate) const fn series_plan_id(self) -> SeriesPlanV5Id {
        self.series_plan_id
    }

    pub(crate) const fn ordinal(self) -> u32 {
        self.ordinal
    }

    pub(crate) const fn source_occurrence_id(self) -> SourceOccurrenceV1Id {
        self.source_occurrence_id
    }

    pub(crate) const fn disposition(self) -> SeriesMarketDispositionV1 {
        self.disposition
    }

    pub(crate) const fn funding_reservation_receipt_id(self) -> ContentId {
        self.funding_reservation_receipt_id
    }

    pub(crate) const fn market_admission_receipt_id(self) -> ContentId {
        self.market_admission_receipt_id
    }

    pub(crate) const fn compiler_bundle_id(self) -> ContentId {
        self.compiler_bundle_id
    }

    pub(crate) const fn capability_profile_id(self) -> ContentId {
        self.capability_profile_id
    }
}

/// Consume one family-private accepted poststate and advance the exact Product
/// foundation bitmap, balance partition, and ordered transcript.
#[allow(clippy::too_many_arguments)]
fn accept_market_foundation_postwrite_v1<'next>(
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

/// Advance one exact founder slot only after its private family owner accepts
/// the same postwrite tuple authenticated by Product's debit receipt.
#[allow(clippy::too_many_arguments)]
pub(crate) fn advance_market_foundation_step_v2<'next, A>(
    program_id: &Pubkey,
    founder: AuthenticatedMarketFounderFoundationV1,
    root_account: &AccountInfo<'_>,
    root: AuthenticatedMarketLifecycleRootV1<'_>,
    foundation_vault: &AccountInfo<'_>,
    destination: &AccountInfo<'_>,
    debit: AuthenticatedMarketFoundationDebitV1,
    schedule: &MarketFoundationScheduleV2,
    account_graph: &MarketFoundationAccountGraphV2,
    postwrite: &A,
    successor_output: &mut MarketLifecycleRootV1,
    rebound_output: &'next mut MarketLifecycleRootAccountV1,
) -> Outcome<(
    AuthenticatedMarketLifecycleRootV1<'next>,
    AuthenticatedMarketFoundationStepV2,
)>
where
    A: AuthenticatedMarketFoundationStepPostwriteV2 + ?Sized,
{
    founder.authenticate_debit(debit)?;
    require(
        founder.root_account == root.account()
            && founder.root_authentication_id == root.authentication_id(),
        ClutchError::MismatchedState,
    )?;
    let accepted_poststate_receipt_id = postwrite.accepted_poststate_receipt_id()?;
    require_live_content_id(accepted_poststate_receipt_id)?;
    postwrite.authenticate_market_foundation_step_postwrite_v2(
        founder.id,
        debit.id,
        debit.market_instance_id,
        debit.generation,
        debit.slot,
        debit.destination,
        debit.principal_lamports,
        debit.destination_donation_floor_lamports,
        debit.destination_observed_balance_lamports,
        debit.rent_refund_owner,
        debit.neutral_lamport_sink,
        accepted_poststate_receipt_id,
    )?;
    let root_semantic_before = root
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let root_data_before = root.data_id();
    let root_authentication_before = root.authentication_id();
    let rebound = accept_market_foundation_postwrite_v1(
        program_id,
        root_account,
        root,
        foundation_vault,
        destination,
        debit,
        schedule,
        account_graph,
        accepted_poststate_receipt_id,
        successor_output,
        rebound_output,
    )?;
    let slot_index = u8::try_from(
        debit
            .slot
            .index()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?;
    let root_semantic_after = rebound
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let root_data_after = rebound.data_id();
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            MARKET_FOUNDATION_STEP_AUTHENTICATION_DOMAIN_V2,
            program_id.as_ref(),
            root_account.key.as_ref(),
            &root_semantic_before.bytes(),
            &root_semantic_after.bytes(),
            &root_data_before.bytes(),
            &root_data_after.bytes(),
            &root_authentication_before.bytes(),
            &rebound.authentication_id().bytes(),
            &founder.id.bytes(),
            &debit.id.bytes(),
            &accepted_poststate_receipt_id.bytes(),
            &[slot_index],
            debit.destination.as_ref(),
            &debit.market_instance_id.bytes(),
            &debit.generation.to_le_bytes(),
        ])
        .to_bytes(),
    );
    require_live_content_id(id)?;
    Ok((
        rebound,
        AuthenticatedMarketFoundationStepV2 {
            id,
            founder_authorization_id: founder.id,
            debit_id: debit.id,
            accepted_poststate_receipt_id,
            root_account: *root_account.key,
            root_semantic_before,
            root_semantic_after,
            root_data_before,
            root_data_after,
            root_authentication_before,
            root_authentication_after: rebound.authentication_id(),
            slot: debit.slot,
            account: debit.destination,
        },
    ))
}

fn authenticate_product_foundation_completion_v1(
    program_id: &Pubkey,
    founder: AuthenticatedMarketFounderFoundationV1,
    root: AuthenticatedMarketLifecycleRootV1<'_>,
    founder_link: AuthenticatedSeriesMarketLinkV1<'_>,
    schedule: &MarketFoundationScheduleV2,
    account_graph: &MarketFoundationAccountGraphV2,
) -> Outcome<AuthenticatedProductFoundationCompletionV1> {
    let binding = root.state().binding();
    let root_semantic_id = root
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let link_semantic_id = founder_link
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let schedule_id = schedule
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let graph_id = account_graph
        .id(schedule)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        root.is_writable()
            && founder_link.is_writable()
            && root.state().phase() == MarketLifecyclePhaseV1::Founding
            && founder_link.state().phase() == SeriesMarketLinkPhaseV1::PendingMarket
            && founder.root_account() == root.account()
            && founder.root_authentication_id() == root.authentication_id()
            && founder.link_account() == founder_link.account()
            && founder.link_authentication_id() == founder_link.authentication_id()
            && founder.founder_link_id == link_semantic_id
            && binding.foundation_schedule_id == schedule_id
            && binding.foundation_account_graph_id == graph_id
            && account_graph.market_instance_id == binding.market_instance_id
            && account_graph.generation == binding.generation
            && root.state().foundation().complete()
            && root.state().capital().principal_remaining_lamports == 0
            && root.state().product_families().activation_ready()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        ClutchError::MismatchedState,
    )?;
    let foundation_transcript_id = root.state().foundation().transcript_id;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            PRODUCT_FOUNDATION_COMPLETION_AUTHENTICATION_DOMAIN_V1,
            program_id.as_ref(),
            root.account().as_ref(),
            &root_semantic_id.bytes(),
            &root.data_id().bytes(),
            &root.authentication_id().bytes(),
            founder_link.account().as_ref(),
            &link_semantic_id.bytes(),
            &founder_link.data_id().bytes(),
            &founder_link.authentication_id().bytes(),
            &founder.id().bytes(),
            &binding.market_instance_id.bytes(),
            &binding.generation.to_le_bytes(),
            &schedule_id.bytes(),
            &graph_id.bytes(),
            &root.state().foundation().sequence.to_le_bytes(),
            &foundation_transcript_id.bytes(),
            &root.state().transition_sequence().to_le_bytes(),
            &root.state().product_families().transition_sequence().to_le_bytes(),
        ])
        .to_bytes(),
    );
    require_live_content_id(id)?;
    Ok(AuthenticatedProductFoundationCompletionV1 {
        id,
        founder_authorization_id: founder.id(),
        root_account: root.account(),
        root_authentication_id: root.authentication_id(),
        root_semantic_id,
        root_data_id: root.data_id(),
        founder_link_account: founder_link.account(),
        founder_link_semantic_id: link_semantic_id,
        founder_link_authentication_id: founder_link.authentication_id(),
        market_instance_id: binding.market_instance_id,
        generation: binding.generation,
        foundation_schedule_id: schedule_id.content_id(),
        foundation_account_graph_id: graph_id.content_id(),
        foundation_transcript_id,
    })
}

/// Activate one fully accepted founder Market while leaving its physical link
/// pending until this exact root postwrite can be consumed by the sole outer.
#[allow(clippy::too_many_arguments)]
fn activate_market_foundation_v2<'next>(
    program_id: &Pubkey,
    founder: AuthenticatedMarketFounderFoundationV1,
    root_account: &AccountInfo<'_>,
    root: AuthenticatedMarketLifecycleRootV1<'_>,
    founder_link: AuthenticatedSeriesMarketLinkV1<'_>,
    schedule: &MarketFoundationScheduleV2,
    postwrite: &AuthenticatedProductFoundationCompletionV1,
    successor_output: &mut MarketLifecycleRootV1,
    rebound_output: &'next mut MarketLifecycleRootAccountV1,
) -> Outcome<(
    AuthenticatedMarketLifecycleRootV1<'next>,
    AuthenticatedMarketFoundationActivationV2,
)>
{
    let binding = root.state().binding();
    let link_semantic_id = founder_link
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let schedule_id = schedule
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let admission = SeriesMarketAdmissionProjectionV1::new(
        binding,
        *founder_link.state(),
        1,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let admission_receipt_id = admission.id();
    let accepted_market_core_receipt_id = postwrite.accepted_market_core_receipt_id()?;
    require_live_content_id(accepted_market_core_receipt_id)?;
    require(
        founder.id != ContentId::ZERO
            && founder.root_account == root.account()
            && founder.root_authentication_id == root.authentication_id()
            && founder.link_account == founder_link.account()
            && founder.link_authentication_id == founder_link.authentication_id()
            && founder.founder_link_id == link_semantic_id
            && founder.market_instance_id == binding.market_instance_id
            && founder.generation == binding.generation
            && founder.foundation_schedule_id == schedule_id.content_id()
            && root.is_writable()
            && *root_account.key == root.account()
            && root.state().phase() == MarketLifecyclePhaseV1::Founding
            && root.state().admitted_series_links() == 1
            && root.state().live_series_links() == 1
            && founder_link.state().phase() == SeriesMarketLinkPhaseV1::PendingMarket
            && founder_link.is_writable()
            && admission_receipt_id != accepted_market_core_receipt_id,
        ClutchError::MismatchedState,
    )?;
    postwrite.authenticate_market_foundation_activation_postwrite_v2(
        founder.id,
        root.account(),
        root.authentication_id(),
        binding.market_instance_id,
        binding.generation,
        founder_link.account(),
        link_semantic_id,
        schedule_id.content_id(),
        binding.foundation_account_graph_id.content_id(),
        root.state().foundation().transcript_id,
        accepted_market_core_receipt_id,
    )?;
    *successor_output = (*root.state())
        .activate(schedule, accepted_market_core_receipt_id)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let authentication_before = root.authentication_id();
    let rebound = write_market_lifecycle_root_v1(
        program_id,
        root_account,
        root,
        successor_output,
        rebound_output,
    )?;
    require(
        rebound.state().phase() == MarketLifecyclePhaseV1::Active,
        ClutchError::MismatchedState,
    )?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            MARKET_FOUNDATION_ACTIVATION_AUTHENTICATION_DOMAIN_V2,
            program_id.as_ref(),
            root_account.key.as_ref(),
            &authentication_before.bytes(),
            &rebound.authentication_id().bytes(),
            &founder.id.bytes(),
            founder_link.account().as_ref(),
            &founder_link.authentication_id().bytes(),
            &link_semantic_id.bytes(),
            &binding.market_instance_id.bytes(),
            &binding.generation.to_le_bytes(),
            &admission_receipt_id.bytes(),
            &accepted_market_core_receipt_id.bytes(),
        ])
        .to_bytes(),
    );
    require_live_content_id(id)?;
    Ok((
        rebound,
        AuthenticatedMarketFoundationActivationV2 {
            id,
            founder_authorization_id: founder.id,
            root_account: *root_account.key,
            root_authentication_before: authentication_before,
            root_authentication_after: rebound.authentication_id(),
            founder_link_account: founder_link.account(),
            founder_link_semantic_id: link_semantic_id,
            market_instance_id: binding.market_instance_id,
            generation: binding.generation,
            market_admission_sequence: 1,
            market_admission_receipt_id: admission_receipt_id,
            accepted_market_core_receipt_id,
        },
    ))
}

#[allow(clippy::too_many_arguments)]
fn mint_series_market_link_activation_v1(
    program_id: &Pubkey,
    root_account: Pubkey,
    root_authentication_before: ContentId,
    root_authentication_after: ContentId,
    link_account: Pubkey,
    link_authentication_before: ContentId,
    link_authentication_after: ContentId,
    link_before: SeriesMarketLinkV1,
    link_after: SeriesMarketLinkV1,
    market_admission_sequence: u64,
    market_admission_receipt_id: ContentId,
    lineage_receipt_id: ContentId,
) -> Outcome<AuthenticatedSeriesMarketLinkActivationV1> {
    let binding = link_before.binding();
    let link_semantic_before = link_before
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let link_semantic_after = link_after
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        lineage_receipt_id != ContentId::ZERO
            && link_before.phase() == SeriesMarketLinkPhaseV1::PendingMarket
            && link_after.phase() == SeriesMarketLinkPhaseV1::Active
            && link_after.binding() == binding
            && link_after.market_admission_sequence() == market_admission_sequence
            && link_after.market_admission_receipt_id() == market_admission_receipt_id,
        ClutchError::MismatchedState,
    )?;
    let disposition = binding.disposition;
    let disposition_byte = match disposition {
        SeriesMarketDispositionV1::Founder => 1,
        SeriesMarketDispositionV1::Converger => 2,
    };
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SERIES_MARKET_LINK_ACTIVATION_AUTHENTICATION_DOMAIN_V1,
            program_id.as_ref(),
            root_account.as_ref(),
            &root_authentication_before.bytes(),
            &root_authentication_after.bytes(),
            link_account.as_ref(),
            &link_authentication_before.bytes(),
            &link_authentication_after.bytes(),
            &link_semantic_before.bytes(),
            &link_semantic_after.bytes(),
            &binding.market_instance_id.bytes(),
            &binding.generation.to_le_bytes(),
            &binding.series_plan_id.bytes(),
            &binding.ordinal.to_le_bytes(),
            &binding.source_occurrence_id.bytes(),
            &[disposition_byte],
            &binding.funding_debit_receipt_id.bytes(),
            &market_admission_sequence.to_le_bytes(),
            &market_admission_receipt_id.bytes(),
            &binding.compiler_output_id.bytes(),
            &binding.capability_profile_id.bytes(),
            &lineage_receipt_id.bytes(),
        ])
        .to_bytes(),
    );
    require_live_content_id(id)?;
    Ok(AuthenticatedSeriesMarketLinkActivationV1 {
        id,
        root_account,
        root_authentication_before,
        root_authentication_after,
        link_account,
        link_authentication_before,
        link_authentication_after,
        link_semantic_before,
        link_semantic_after,
        market_instance_id: binding.market_instance_id,
        generation: binding.generation,
        series_plan_id: binding.series_plan_id,
        ordinal: binding.ordinal,
        source_occurrence_id: binding.source_occurrence_id,
        disposition,
        funding_reservation_receipt_id: binding.funding_debit_receipt_id,
        market_admission_sequence,
        market_admission_receipt_id,
        compiler_bundle_id: binding.compiler_output_id,
        capability_profile_id: binding.capability_profile_id,
    })
}

/// Activate the sole founder link only after the exact root activation
/// postwrite is hostile-reopened as Active.
#[allow(clippy::too_many_arguments)]
fn activate_founder_series_market_link_v1<'next>(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    root: AuthenticatedMarketLifecycleRootV1<'_>,
    link_account: &AccountInfo<'_>,
    link: AuthenticatedSeriesMarketLinkV1<'_>,
    activation: AuthenticatedMarketFoundationActivationV2,
    root_rebound_output: &mut MarketLifecycleRootAccountV1,
    link_rebound_output: &'next mut SeriesMarketLinkAccountV1,
) -> Outcome<(
    AuthenticatedSeriesMarketLinkV1<'next>,
    AuthenticatedSeriesMarketLinkActivationV1,
)> {
    let root_binding = root.state().binding();
    let live_root = authenticate_market_lifecycle_root_v1(
        program_id,
        root_account,
        root_binding.market_instance_id,
        root_binding.generation,
        true,
        root_rebound_output,
    )?;
    let link_before = *link.state();
    let link_binding = link_before.binding();
    let link_semantic_before = link_before
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        root.is_writable()
            && live_root.state() == root.state()
            && live_root.authentication_id() == root.authentication_id()
            && live_root.state().phase() == MarketLifecyclePhaseV1::Active
            && live_root.authentication_id() == activation.root_authentication_after
            && activation.root_account == root.account()
            && activation.founder_link_account == link.account()
            && activation.founder_link_semantic_id == link_semantic_before
            && activation.market_instance_id == root_binding.market_instance_id
            && activation.generation == root_binding.generation
            && activation.market_admission_sequence == 1
            && link.is_writable()
            && link_before.phase() == SeriesMarketLinkPhaseV1::PendingMarket
            && link_binding.disposition == SeriesMarketDispositionV1::Founder
            && link_binding.market_root_account_id.bytes() == root.account().to_bytes()
            && link_binding.market_binding_id
                == root_binding
                    .id()
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            && link_binding.market_instance_id == root_binding.market_instance_id
            && link_binding.generation == root_binding.generation,
        ClutchError::MismatchedState,
    )?;
    let successor = link_before
        .activate(
            activation.market_admission_sequence,
            activation.market_admission_receipt_id,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let link_authentication_before = link.authentication_id();
    let rebound = write_series_market_link_v1(
        program_id,
        link_account,
        link,
        &successor,
        link_rebound_output,
    )?;
    let receipt = mint_series_market_link_activation_v1(
        program_id,
        root.account(),
        activation.root_authentication_before,
        live_root.authentication_id(),
        link.account(),
        link_authentication_before,
        rebound.authentication_id(),
        link_before,
        *rebound.state(),
        activation.market_admission_sequence,
        activation.market_admission_receipt_id,
        activation.id,
    )?;
    Ok((rebound, receipt))
}

/// Non-detachable receipt for the sole founder activation chain.
///
/// Product mints this only after the complete Founding root becomes Active,
/// the exact founder `0xad` becomes Active, FundingV2 consumes the pending
/// ordinal, and permanent `0xb8` records that same admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedProductFounderActivationV1 {
    id: ContentId,
    foundation_completion_id: ContentId,
    root_activation_id: ContentId,
    link_activation_id: ContentId,
    occurrence_completion_id: ContentId,
    lifecycle_replay_postwrite_id: ContentId,
    root_account: Pubkey,
    root_authentication_after: ContentId,
    link_account: Pubkey,
    link_authentication_after: ContentId,
    funding_account: Pubkey,
    funding_state_after_id: ContentId,
    lifecycle_replay_account: Pubkey,
    lifecycle_replay_authentication_after: ContentId,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    series_plan_id: SeriesPlanV5Id,
    ordinal: u32,
    compiler_bundle_id: ContentId,
}

impl AuthenticatedProductFounderActivationV1 {
    pub(crate) const fn id(self) -> ContentId {
        self.id
    }

    pub(crate) const fn root_account(self) -> Pubkey {
        self.root_account
    }

    pub(crate) const fn link_account(self) -> Pubkey {
        self.link_account
    }

    pub(crate) const fn market_instance_id(self) -> MarketInstanceV2Id {
        self.market_instance_id
    }

    pub(crate) const fn generation(self) -> u64 {
        self.generation
    }

    pub(crate) const fn series_plan_id(self) -> SeriesPlanV5Id {
        self.series_plan_id
    }

    pub(crate) const fn ordinal(self) -> u32 {
        self.ordinal
    }
}

/// Atomically activate the complete Product founder root and link, consume the
/// exact pending FundingV2 reservation, and record the same ordinal in the
/// permanent counted Series replay.
///
/// All four mutable accounts are hostile-reopened at entry and after their
/// respective writes. Raw root activation, link activation, Funding
/// completion, and replay admission are private and cannot mint detachable
/// authority. Any final refusal rolls every earlier write back with the SVM
/// instruction.
#[allow(clippy::too_many_arguments)]
pub(crate) fn activate_and_complete_product_market_founder_v1<'root, 'link>(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    link_account: &AccountInfo<'_>,
    registry_account: &AccountInfo<'_>,
    funding_account: &AccountInfo<'_>,
    lifecycle_replay_account: &AccountInfo<'_>,
    funding_quote_account: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
    series_artifact_accounts: &[AccountInfo<'_>],
    account_graph: &MarketFoundationAccountGraphV2,
    capability: AuthenticatedRegistryCapabilityV3,
    compiler_bundle: AuthenticatedCompiledProductSeriesBundleV5,
    root_before_output: &mut MarketLifecycleRootAccountV1,
    link_before_output: &mut SeriesMarketLinkAccountV1,
    root_successor_output: &mut MarketLifecycleRootV1,
    root_after_output: &'root mut MarketLifecycleRootAccountV1,
    root_verify_output: &mut MarketLifecycleRootAccountV1,
    link_after_output: &'link mut SeriesMarketLinkAccountV1,
) -> Outcome<(
    AuthenticatedMarketLifecycleRootV1<'root>,
    AuthenticatedSeriesMarketLinkV1<'link>,
    AuthenticatedSeriesFundingAccountV2,
    AuthenticatedSeriesLifecycleReplayV1,
    AuthenticatedProductFounderActivationV1,
)> {
    require_distinct(&[
        root_account.clone(),
        link_account.clone(),
        registry_account.clone(),
        funding_account.clone(),
        lifecycle_replay_account.clone(),
        funding_quote_account.clone(),
        rent_sysvar.clone(),
    ])?;
    let rent = read_rent(rent_sysvar)?;

    let root_data = root_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    MarketLifecycleRootAccountV1::decode_into(&root_data, root_before_output)?;
    let root_binding = root_before_output.state.binding();
    drop(root_data);
    let root = authenticate_market_lifecycle_root_v1(
        program_id,
        root_account,
        root_binding.market_instance_id,
        root_binding.generation,
        true,
        root_before_output,
    )?;

    let link_data = link_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    SeriesMarketLinkAccountV1::decode_into(&link_data, link_before_output)?;
    let link_binding = link_before_output.state.binding();
    drop(link_data);
    let link = authenticate_series_market_link_v1(
        program_id,
        link_account,
        link_binding.series_plan_id,
        link_binding.ordinal,
        root_binding.market_instance_id,
        root_binding.generation,
        *root_account.key,
        true,
        link_before_output,
    )?;

    let bundle = compiler_bundle.bundle();
    let compiler_bundle_id = compiler_bundle.bundle_id();
    let artifacts = authenticate_series_artifact_accounts_v4(
        program_id,
        series_artifact_accounts,
        capability.series_plan_id(),
        capability.funding_terms_id(),
    )?;
    let registry = read_series_registry_account_v2(
        program_id,
        registry_account,
        capability.series_plan_id(),
        &rent,
    )?;
    let funding = read_series_funding_account_v2(
        program_id,
        funding_account,
        registry,
        &artifacts,
        &rent,
    )?;
    let replay = authenticate_series_lifecycle_replay_v1(
        program_id,
        lifecycle_replay_account,
        capability.series_plan_id(),
        true,
        &rent,
    )?;
    let quote = authenticate_product_artifact_v1::<SeriesFundingQuoteV4>(
        program_id,
        funding_quote_account,
        bundle.funding_quote_id.content_id(),
    )?;
    let registry_data = registry_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let registry_data_id =
        ContentId::from_bytes(solana_sha256_hasher::hashv(&[&registry_data[..]]).to_bytes());
    drop(registry_data);
    require(
        capability.program_account() == *program_id
            && capability.series_registry_account() == *registry_account.key
            && registry.activation_consumed()
            && registry.value().compiler_bundle_id == compiler_bundle_id
            && registry.value().funding_terms_id == bundle.funding_terms_id
            && compiler_bundle_id.content_id() == link_binding.compiler_output_id
            && bundle.series_plan_id == link_binding.series_plan_id
            && bundle.funding_terms_id == link_binding.funding_terms_id
            && bundle.funding_quote_id.content_id() == link_binding.funding_quote_id
            && bundle.attachment_plan_id.content_id() == link_binding.attachment_plan_id
            && quote.semantic_id() == bundle.funding_quote_id.content_id()
            && root.state().phase() == MarketLifecyclePhaseV1::Founding
            && link.state().phase() == SeriesMarketLinkPhaseV1::PendingMarket
            && funding.value().state.phase == clutch_product_series::SeriesFundingPhaseV2::Pending
            && replay.state().binding().series_plan_id == link_binding.series_plan_id,
        ClutchError::MismatchedState,
    )?;

    let founder = authenticate_market_founder_foundation_with_link_privilege_v1(
        program_id,
        root,
        link,
        capability,
        compiler_bundle,
        true,
    )?;
    let completion = authenticate_product_foundation_completion_v1(
        program_id,
        founder,
        root,
        link,
        &quote.value().foundation,
        account_graph,
    )?;
    let (root_after, root_activation) = activate_market_foundation_v2(
        program_id,
        founder,
        root_account,
        root,
        link,
        &quote.value().foundation,
        &completion,
        root_successor_output,
        root_after_output,
    )?;
    let (link_after, link_activation) = activate_founder_series_market_link_v1(
        program_id,
        root_account,
        root_after,
        link_account,
        link,
        root_activation,
        root_verify_output,
        link_after_output,
    )?;
    let (funding_after, replay_after, occurrence, replay_postwrite) =
        complete_and_record_series_lifecycle_admission_v1(
            program_id,
            funding_account,
            funding,
            lifecycle_replay_account,
            replay,
            link_account,
            &artifacts,
            link_activation,
            &rent,
        )?;
    let funding_state_after_id = funding_after
        .value()
        .state
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .content_id();
    require(
        root_after.state().phase() == MarketLifecyclePhaseV1::Active
            && link_after.state().phase() == SeriesMarketLinkPhaseV1::Active
            && occurrence.link_activation_id() == link_activation.id()
            && occurrence.market_admission_receipt_id()
                == link_activation.market_admission_receipt_id()
            && occurrence.funding_state_after_id() == funding_state_after_id
            && replay_postwrite.event_id() == occurrence.id()
            && replay_after.authentication_id()
                == replay_postwrite.authentication_after_id(),
        ClutchError::MismatchedState,
    )?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            PRODUCT_FOUNDER_ACTIVATION_AUTHENTICATION_DOMAIN_V1,
            program_id.as_ref(),
            &completion.id.bytes(),
            &root_activation.id().bytes(),
            &link_activation.id().bytes(),
            &occurrence.id().bytes(),
            &replay_postwrite.id().bytes(),
            root_account.key.as_ref(),
            &root_after.authentication_id().bytes(),
            link_account.key.as_ref(),
            &link_after.authentication_id().bytes(),
            funding_account.key.as_ref(),
            &funding_state_after_id.bytes(),
            lifecycle_replay_account.key.as_ref(),
            &replay_after.authentication_id().bytes(),
            registry_account.key.as_ref(),
            &registry_data_id.bytes(),
            &root_binding.market_instance_id.bytes(),
            &root_binding.generation.to_le_bytes(),
            &link_binding.series_plan_id.bytes(),
            &link_binding.ordinal.to_le_bytes(),
            &compiler_bundle_id.bytes(),
        ])
        .to_bytes(),
    );
    require_live_content_id(id)?;
    let receipt = AuthenticatedProductFounderActivationV1 {
        id,
        foundation_completion_id: completion.id,
        root_activation_id: root_activation.id(),
        link_activation_id: link_activation.id(),
        occurrence_completion_id: occurrence.id(),
        lifecycle_replay_postwrite_id: replay_postwrite.id(),
        root_account: *root_account.key,
        root_authentication_after: root_after.authentication_id(),
        link_account: *link_account.key,
        link_authentication_after: link_after.authentication_id(),
        funding_account: *funding_account.key,
        funding_state_after_id,
        lifecycle_replay_account: *lifecycle_replay_account.key,
        lifecycle_replay_authentication_after: replay_after.authentication_id(),
        market_instance_id: root_binding.market_instance_id,
        generation: root_binding.generation,
        series_plan_id: link_binding.series_plan_id,
        ordinal: link_binding.ordinal,
        compiler_bundle_id: compiler_bundle_id.content_id(),
    };
    Ok((root_after, link_after, funding_after, replay_after, receipt))
}

/// Admit and activate one converger atomically against an already-Active root.
#[allow(clippy::too_many_arguments)]
pub(crate) fn admit_converger_series_market_link_v1<'root, 'link, A>(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    root: AuthenticatedMarketLifecycleRootV1<'_>,
    link_account: &AccountInfo<'_>,
    link: AuthenticatedSeriesMarketLinkV1<'_>,
    capability: AuthenticatedRegistryCapabilityV3,
    compiler_bundle: AuthenticatedCompiledProductSeriesBundleV5,
    owner: &A,
    root_successor_output: &mut MarketLifecycleRootV1,
    root_rebound_output: &'root mut MarketLifecycleRootAccountV1,
    link_rebound_output: &'link mut SeriesMarketLinkAccountV1,
) -> Outcome<(
    AuthenticatedMarketLifecycleRootV1<'root>,
    AuthenticatedSeriesMarketLinkV1<'link>,
    AuthenticatedSeriesMarketLinkActivationV1,
)>
where
    A: AuthenticatedConvergerSeriesMarketAdmissionV1 + ?Sized,
{
    let root_binding = root.state().binding();
    let link_before = *link.state();
    let link_binding = link_before.binding();
    let link_semantic_before = link_before
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let bundle = compiler_bundle.bundle();
    let compiler_bundle_id = compiler_bundle.bundle_id().content_id();
    let market_admission_sequence = u64::from(root.state().admitted_series_links())
        .checked_add(1)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let admission = SeriesMarketAdmissionProjectionV1::new(
        root_binding,
        link_before,
        market_admission_sequence,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let market_admission_receipt_id = admission.id();
    require(
        root.is_writable()
            && root.state().phase() == MarketLifecyclePhaseV1::Active
            && link.is_writable()
            && link_before.phase() == SeriesMarketLinkPhaseV1::PendingMarket
            && link_binding.disposition == SeriesMarketDispositionV1::Converger
            && link_binding.market_root_account_id.bytes() == root.account().to_bytes()
            && link_binding.market_binding_id
                == root_binding
                    .id()
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            && link_binding.market_instance_id == root_binding.market_instance_id
            && link_binding.generation == root_binding.generation
            && link_binding.compiler_output_id == compiler_bundle_id
            && link_binding.series_plan_id == bundle.series_plan_id
            && link_binding.funding_terms_id == bundle.funding_terms_id
            && link_binding.funding_quote_id == bundle.funding_quote_id.content_id()
            && link_binding.attachment_plan_id == bundle.attachment_plan_id.content_id()
            && link_binding.capability_profile_id == bundle.capability_profile_id.content_id()
            && capability.program_account() == *program_id
            && capability.series_plan_id() == link_binding.series_plan_id
            && capability.funding_terms_id() == link_binding.funding_terms_id
            && capability.compiler_bundle_id() == compiler_bundle_id
            && capability.registry_release_id() == root_binding.registry_release_id
            && capability.capability_profile_id() == root_binding.capability_profile_id
            && bundle.registry_release_id == root_binding.registry_release_id
            && bundle.capability_profile_id.content_id() == root_binding.capability_profile_id
            && bundle.product_template_id.content_id() == root_binding.product_template_id
            && bundle.native_claim_basis_id.content_id() == root_binding.native_claim_basis_id
            && bundle.evidence_only_recovery_policy_id.content_id()
                == root_binding.recovery_policy_id
            && bundle.price_measure_policy_id.content_id()
                == root_binding.price_measure_policy_id
            && bundle.market_genesis_profile_id.content_id()
                == root_binding.market_genesis_profile_id
            && bundle.source_plane_contract_id == root_binding.source_plane_contract_id
            && bundle.source_spec_id == root_binding.source_spec_id,
        ClutchError::MismatchedState,
    )?;
    owner.authenticate_converger_series_market_admission_v1(
        root.account(),
        root.authentication_id(),
        link.account(),
        link.authentication_id(),
        link_semantic_before,
        link_binding.series_plan_id,
        link_binding.ordinal,
        link_binding.market_instance_id,
        link_binding.generation,
        link_binding.source_occurrence_id,
        link_binding.funding_debit_receipt_id,
        market_admission_sequence,
        market_admission_receipt_id,
    )?;
    *root_successor_output = (*root.state())
        .admit_series_link(admission)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let root_authentication_before = root.authentication_id();
    let root_rebound = write_market_lifecycle_root_v1(
        program_id,
        root_account,
        root,
        root_successor_output,
        root_rebound_output,
    )?;
    let link_successor = link_before
        .activate(market_admission_sequence, market_admission_receipt_id)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let link_authentication_before = link.authentication_id();
    let link_rebound = write_series_market_link_v1(
        program_id,
        link_account,
        link,
        &link_successor,
        link_rebound_output,
    )?;
    let receipt = mint_series_market_link_activation_v1(
        program_id,
        root.account(),
        root_authentication_before,
        root_rebound.authentication_id(),
        link.account(),
        link_authentication_before,
        link_rebound.authentication_id(),
        link_before,
        *link_rebound.state(),
        market_admission_sequence,
        market_admission_receipt_id,
        link_binding.funding_debit_receipt_id,
    )?;
    Ok((root_rebound, link_rebound, receipt))
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

struct ExactGeneralFamilyAdmissionAuthorityV1 {
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    general_root_id: ContentId,
    sequence: u32,
    receipt_id: ContentId,
}

impl AuthenticatedMarketFamilyAuthorityV1 for ExactGeneralFamilyAdmissionAuthorityV1 {
    fn authenticate_admission(
        &self,
        current: &MarketFamilyAggregatorV1,
        family: MarketFamilyV1,
        family_root_id: ContentId,
        family_admission_sequence: u32,
        admission_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        if family != MarketFamilyV1::General
            || current.binding().market_instance_id != self.market_instance_id
            || current.binding().generation != self.generation
            || family_root_id != self.general_root_id
            || family_admission_sequence != self.sequence
            || admission_receipt_id != self.receipt_id
        {
            return Err(clutch_product_series::Error::UnauthenticatedAuthority);
        }
        Ok(())
    }
}

/// Persist only the exact General-family admission authenticated by Product's
/// private cross-owner authority. This is deliberately not a generic root
/// successor writer.
#[allow(clippy::too_many_arguments)]
pub(crate) fn write_authenticated_general_family_admission_root_v1<'next, A>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    authenticated: AuthenticatedMarketLifecycleRootV1<'_>,
    successor: &MarketLifecycleRootV1,
    family_admission_sequence: u32,
    product_preauthorization_id: ContentId,
    general_postwrite_semantic_id: ContentId,
    general_postwrite_data_id: ContentId,
    general_postwrite_authentication_id: ContentId,
    authority: &A,
    rebound_output: &'next mut MarketLifecycleRootAccountV1,
) -> Outcome<AuthenticatedMarketLifecycleRootV1<'next>>
where
    A: AuthenticatedGeneralFamilyRootWriteV1 + ?Sized,
{
    let current = authenticated.state();
    let binding = current.binding();
    let current_semantic_id = current
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let market_binding_id = binding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let general_root_id = current
        .product_families()
        .binding()
        .family_root_id(MarketFamilyV1::General);
    authority
        .authenticate_general_family_root_write(
            authenticated.account(),
            current_semantic_id,
            authenticated.data_id(),
            authenticated.authentication_id(),
            binding.market_instance_id,
            market_binding_id,
            binding.generation,
            general_root_id,
            family_admission_sequence,
            product_preauthorization_id,
            general_postwrite_semantic_id,
            general_postwrite_data_id,
            general_postwrite_authentication_id,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let exact_authority = ExactGeneralFamilyAdmissionAuthorityV1 {
        market_instance_id: binding.market_instance_id,
        generation: binding.generation,
        general_root_id,
        sequence: family_admission_sequence,
        receipt_id: general_postwrite_semantic_id,
    };
    let expected = current
        .admit_product_family_child(
            &exact_authority,
            MarketFamilyV1::General,
            family_admission_sequence,
            general_postwrite_semantic_id,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(expected == *successor, ClutchError::MismatchedState)?;
    write_market_lifecycle_root_v1(
        program_id,
        account,
        authenticated,
        successor,
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

/// Default-refusing authority for the exact Fractional-family admission write.
pub(crate) trait AuthenticatedFractionalFamilyAdmissionRootWriteV1 {
    #[allow(clippy::too_many_arguments)]
    fn authenticate_fractional_family_admission_root_write_v1(
        &self,
        _root_account: Pubkey,
        _root_semantic_before: ContentId,
        _root_data_before: ContentId,
        _root_authentication_before: ContentId,
        _market_instance_id: MarketInstanceV2Id,
        _generation: u64,
        _fractional_root_id: ContentId,
        _family_admission_sequence: u32,
        _fractional_admission_receipt_id: ContentId,
        _fractional_verification_id: ContentId,
        _fractional_postwrite_authentication_id: ContentId,
    ) -> Outcome<()> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
}

/// Default-refusing authority for the exact Fractional-family terminal write.
pub(crate) trait AuthenticatedFractionalFamilyTerminalRootWriteV1 {
    #[allow(clippy::too_many_arguments)]
    fn authenticate_fractional_family_terminal_root_write_v1(
        &self,
        _root_account: Pubkey,
        _root_semantic_before: ContentId,
        _root_data_before: ContentId,
        _root_authentication_before: ContentId,
        _market_instance_id: MarketInstanceV2Id,
        _generation: u64,
        _fractional_root_id: ContentId,
        _family_terminal_sequence: u32,
        _fractional_terminal_receipt_id: ContentId,
        _fractional_policy_terminal_state_id: ContentId,
        _fractional_ledger_terminal_state_id: ContentId,
        _fractional_verification_id: ContentId,
        _fractional_postwrite_authentication_id: ContentId,
        _claim_release_receipt_id: ContentId,
    ) -> Outcome<()> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
}

struct ExactFractionalFamilyAuthorityV1 {
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    fractional_root_id: ContentId,
    sequence: u32,
    receipt_id: ContentId,
    terminal: bool,
}

impl AuthenticatedMarketFamilyAuthorityV1 for ExactFractionalFamilyAuthorityV1 {
    fn authenticate_admission(
        &self,
        current: &MarketFamilyAggregatorV1,
        family: MarketFamilyV1,
        family_root_id: ContentId,
        family_admission_sequence: u32,
        admission_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        if self.terminal
            || family != MarketFamilyV1::Fractional
            || current.binding().market_instance_id != self.market_instance_id
            || current.binding().generation != self.generation
            || family_root_id != self.fractional_root_id
            || family_admission_sequence != self.sequence
            || admission_receipt_id != self.receipt_id
        {
            return Err(clutch_product_series::Error::UnauthenticatedAuthority);
        }
        Ok(())
    }

    fn authenticate_terminal(
        &self,
        current: &MarketFamilyAggregatorV1,
        family: MarketFamilyV1,
        family_root_id: ContentId,
        family_terminal_sequence: u32,
        terminal_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        if !self.terminal
            || family != MarketFamilyV1::Fractional
            || current.binding().market_instance_id != self.market_instance_id
            || current.binding().generation != self.generation
            || family_root_id != self.fractional_root_id
            || family_terminal_sequence != self.sequence
            || terminal_receipt_id != self.receipt_id
        {
            return Err(clutch_product_series::Error::UnauthenticatedAuthority);
        }
        Ok(())
    }
}

/// Persist only the exact Fractional admission authenticated by the private
/// a4/a5/ClaimLedger postwrite owner.
#[allow(clippy::too_many_arguments)]
pub(crate) fn write_authenticated_fractional_family_admission_root_v1<
    'next,
    A: AuthenticatedFractionalFamilyAdmissionRootWriteV1 + ?Sized,
>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    authenticated: AuthenticatedMarketLifecycleRootV1<'_>,
    family_admission_sequence: u32,
    fractional_admission_receipt_id: ContentId,
    fractional_verification_id: ContentId,
    fractional_postwrite_authentication_id: ContentId,
    authority: &A,
    rebound_output: &'next mut MarketLifecycleRootAccountV1,
) -> Outcome<AuthenticatedMarketLifecycleRootV1<'next>> {
    let current = authenticated.state();
    let binding = current.binding();
    let root_semantic_before = current
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let fractional_root_id = current
        .product_families()
        .binding()
        .family_root_id(MarketFamilyV1::Fractional);
    authority.authenticate_fractional_family_admission_root_write_v1(
        authenticated.account(),
        root_semantic_before,
        authenticated.data_id(),
        authenticated.authentication_id(),
        binding.market_instance_id,
        binding.generation,
        fractional_root_id,
        family_admission_sequence,
        fractional_admission_receipt_id,
        fractional_verification_id,
        fractional_postwrite_authentication_id,
    )?;
    let exact = ExactFractionalFamilyAuthorityV1 {
        market_instance_id: binding.market_instance_id,
        generation: binding.generation,
        fractional_root_id,
        sequence: family_admission_sequence,
        receipt_id: fractional_admission_receipt_id,
        terminal: false,
    };
    let successor = current
        .admit_product_family_child(
            &exact,
            MarketFamilyV1::Fractional,
            family_admission_sequence,
            fractional_admission_receipt_id,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    write_market_lifecycle_root_v1(
        program_id,
        account,
        authenticated,
        &successor,
        rebound_output,
    )
}

/// Persist only the exact Fractional terminal receipt and a4/a5 terminal states.
#[allow(clippy::too_many_arguments)]
pub(crate) fn write_authenticated_fractional_family_terminal_root_v1<
    'next,
    A: AuthenticatedFractionalFamilyTerminalRootWriteV1 + ?Sized,
>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    authenticated: AuthenticatedMarketLifecycleRootV1<'_>,
    family_terminal_sequence: u32,
    fractional_terminal_receipt_id: ContentId,
    fractional_policy_terminal_state_id: ContentId,
    fractional_ledger_terminal_state_id: ContentId,
    fractional_verification_id: ContentId,
    fractional_postwrite_authentication_id: ContentId,
    claim_release_receipt_id: ContentId,
    authority: &A,
    rebound_output: &'next mut MarketLifecycleRootAccountV1,
) -> Outcome<AuthenticatedMarketLifecycleRootV1<'next>> {
    let current = authenticated.state();
    let binding = current.binding();
    let root_semantic_before = current
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let fractional_root_id = current
        .product_families()
        .binding()
        .family_root_id(MarketFamilyV1::Fractional);
    authority.authenticate_fractional_family_terminal_root_write_v1(
        authenticated.account(),
        root_semantic_before,
        authenticated.data_id(),
        authenticated.authentication_id(),
        binding.market_instance_id,
        binding.generation,
        fractional_root_id,
        family_terminal_sequence,
        fractional_terminal_receipt_id,
        fractional_policy_terminal_state_id,
        fractional_ledger_terminal_state_id,
        fractional_verification_id,
        fractional_postwrite_authentication_id,
        claim_release_receipt_id,
    )?;
    let exact = ExactFractionalFamilyAuthorityV1 {
        market_instance_id: binding.market_instance_id,
        generation: binding.generation,
        fractional_root_id,
        sequence: family_terminal_sequence,
        receipt_id: fractional_terminal_receipt_id,
        terminal: true,
    };
    let successor = current
        .terminalize_fractional_family(
            &exact,
            family_terminal_sequence,
            fractional_terminal_receipt_id,
            fractional_policy_terminal_state_id,
            fractional_ledger_terminal_state_id,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    write_market_lifecycle_root_v1(
        program_id,
        account,
        authenticated,
        &successor,
        rebound_output,
    )
}

/// Default-refusing bridge from the complete Failure aggregate/replay/seal
/// postwrite into the Product-owned mandatory shared-core latch.
///
/// Getter defaults are deliberately invalid. The concrete Failure owner must
/// retain and authenticate its full a0/a3/ab/ac postwrite; Product accepts no
/// caller projection or founding-time binding as terminal authority.
pub(crate) trait AuthenticatedFailureSharedCoreTerminalOwnerV1 {
    fn postwrite_id(&self) -> ContentId {
        ContentId::ZERO
    }

    fn market_instance_id(&self) -> MarketInstanceV2Id {
        MarketInstanceV2Id::from_bytes([0; 32])
    }

    fn generation(&self) -> u64 {
        0
    }

    fn owner_account_id(&self) -> ContentId {
        ContentId::ZERO
    }

    fn owner_release_id(&self) -> ContentId {
        ContentId::ZERO
    }

    fn family_terminal_receipt_id(&self) -> ContentId {
        ContentId::ZERO
    }

    #[allow(clippy::too_many_arguments)]
    fn authenticate_failure_shared_core_terminal_owner_v1(
        &self,
        _root_account: Pubkey,
        _root_binding_id: ContentId,
        _root_semantic_before_id: ContentId,
        _root_data_before_id: ContentId,
        _root_authentication_before_id: ContentId,
        _projection: MarketSharedCoreTerminalProjectionV1,
    ) -> Outcome<()> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
}

/// Exact Product root postwrite proving the mandatory Failure shared-core
/// receipt was consumed once from the private Failure terminal owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedFailureSharedCoreTerminalPostwriteV1 {
    id: ContentId,
    failure_postwrite_id: ContentId,
    projection: MarketSharedCoreTerminalProjectionV1,
    root_account: Pubkey,
    root_semantic_before_id: ContentId,
    root_semantic_after_id: ContentId,
    root_data_before_id: ContentId,
    root_data_after_id: ContentId,
    root_authentication_before_id: ContentId,
    root_authentication_after_id: ContentId,
}

impl AuthenticatedFailureSharedCoreTerminalPostwriteV1 {
    pub(crate) const fn id(self) -> ContentId {
        self.id
    }

    pub(crate) const fn failure_postwrite_id(self) -> ContentId {
        self.failure_postwrite_id
    }

    pub(crate) const fn projection(self) -> MarketSharedCoreTerminalProjectionV1 {
        self.projection
    }

    pub(crate) const fn root_account(self) -> Pubkey {
        self.root_account
    }

    pub(crate) const fn root_semantic_before_id(self) -> ContentId {
        self.root_semantic_before_id
    }

    pub(crate) const fn root_semantic_after_id(self) -> ContentId {
        self.root_semantic_after_id
    }

    pub(crate) const fn root_data_before_id(self) -> ContentId {
        self.root_data_before_id
    }

    pub(crate) const fn root_data_after_id(self) -> ContentId {
        self.root_data_after_id
    }

    pub(crate) const fn root_authentication_before_id(self) -> ContentId {
        self.root_authentication_before_id
    }

    pub(crate) const fn root_authentication_after_id(self) -> ContentId {
        self.root_authentication_after_id
    }
}

/// Consume the exact current Failure-family terminal postwrite into a live
/// Retiring `0xaa`. The only accepted owner account is the permanent canonical
/// a3/v2 replay for this Market/generation, and the raw root writer remains
/// inaccessible outside this semantic-owner module.
pub(crate) fn record_failure_shared_core_terminal_v1<'next, A>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    authenticated: AuthenticatedMarketLifecycleRootV1<'_>,
    authority: &A,
    successor_output: &mut MarketLifecycleRootV1,
    rebound_output: &'next mut MarketLifecycleRootAccountV1,
) -> Outcome<(
    AuthenticatedMarketLifecycleRootV1<'next>,
    AuthenticatedFailureSharedCoreTerminalPostwriteV1,
)>
where
    A: AuthenticatedFailureSharedCoreTerminalOwnerV1 + ?Sized,
{
    let current = authenticated.state();
    let binding = current.binding();
    let root_binding_id = binding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let root_semantic_before_id = current
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let postwrite_id = authority.postwrite_id();
    let owner_account_id = authority.owner_account_id();
    let owner_release_id = authority.owner_release_id();
    let family_terminal_receipt_id = authority.family_terminal_receipt_id();
    let (expected_owner_account, _) = seeds::failure_market_replay_v2_pda(
        program_id,
        &binding.market_instance_id.bytes(),
        binding.generation,
    );
    require(
        authenticated.is_writable()
            && authenticated.account() == *account.key
            && authenticated.owner_program() == *program_id
            && current.phase() == MarketLifecyclePhaseV1::Retiring
            && current.failure_terminal_receipt_id() == ContentId::ZERO
            && authority.market_instance_id() == binding.market_instance_id
            && authority.generation() == binding.generation
            && owner_account_id.bytes() == expected_owner_account.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    for id in [
        postwrite_id,
        owner_account_id,
        owner_release_id,
        family_terminal_receipt_id,
    ] {
        require_live_content_id(id)?;
    }
    let root_transition_sequence = current
        .transition_sequence()
        .checked_add(1)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let projection = MarketSharedCoreTerminalProjectionV1::new(
        binding,
        MarketSharedCoreV1::Failure,
        owner_account_id,
        owner_release_id,
        family_terminal_receipt_id,
        root_transition_sequence,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    authority.authenticate_failure_shared_core_terminal_owner_v1(
        authenticated.account(),
        root_binding_id,
        root_semantic_before_id,
        authenticated.data_id(),
        authenticated.authentication_id(),
        projection,
    )?;
    *successor_output = (*current)
        .consume_shared_core_terminal(projection)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let rebound = write_market_lifecycle_root_v1(
        program_id,
        account,
        authenticated,
        successor_output,
        rebound_output,
    )?;
    let root_semantic_after_id = rebound
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        rebound.state().failure_terminal_receipt_id() == projection.id()
            && rebound.state().transition_sequence() == root_transition_sequence,
        ClutchError::MismatchedState,
    )?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            FAILURE_SHARED_CORE_TERMINAL_POSTWRITE_DOMAIN_V1,
            &postwrite_id.bytes(),
            &projection.id().bytes(),
            account.key.as_ref(),
            &root_binding_id.bytes(),
            &root_semantic_before_id.bytes(),
            &root_semantic_after_id.bytes(),
            &authenticated.data_id().bytes(),
            &rebound.data_id().bytes(),
            &authenticated.authentication_id().bytes(),
            &rebound.authentication_id().bytes(),
        ])
        .to_bytes(),
    );
    require_live_content_id(id)?;
    let postwrite = AuthenticatedFailureSharedCoreTerminalPostwriteV1 {
        id,
        failure_postwrite_id: postwrite_id,
        projection,
        root_account: *account.key,
        root_semantic_before_id,
        root_semantic_after_id,
        root_data_before_id: authenticated.data_id(),
        root_data_after_id: rebound.data_id(),
        root_authentication_before_id: authenticated.authentication_id(),
        root_authentication_after_id: rebound.authentication_id(),
    };
    Ok((rebound, postwrite))
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

/// Atomically retire one exact `0xad/1` link in its shared `0xaa/1`, require a
/// bounded per-Series aggregate to accept the same link exactly once, refund
/// only the immutable rent principal, and sink every tracked or unsolicited
/// surplus lamport.
///
/// This helper deliberately remains route-less. The default-refusing aggregate
/// boundary prevents a FundingV2 `Closed` projection from standing in for the
/// missing admitted/retired link partition.
#[allow(clippy::too_many_arguments)]
pub(crate) fn retire_and_close_series_market_link_v1<'a>(
    program_id: &Pubkey,
    root_account: &AccountInfo<'a>,
    root: AuthenticatedMarketLifecycleRootV1<'_>,
    link_account: &AccountInfo<'a>,
    link: AuthenticatedSeriesMarketLinkV1<'_>,
    refund_owner: &AccountInfo<'a>,
    neutral_lamport_sink: &AccountInfo<'a>,
    aggregate: &SeriesLifecycleLinkRetirementAggregateAuthorityV1<'_, 'a>,
    root_successor_output: &mut MarketLifecycleRootV1,
    link_retiring_output: &mut SeriesMarketLinkV1,
    link_retired_output: &mut SeriesMarketLinkV1,
    root_rebound_output: &mut MarketLifecycleRootAccountV1,
    link_retiring_rebound_output: &mut SeriesMarketLinkAccountV1,
    link_retired_rebound_output: &mut SeriesMarketLinkAccountV1,
) -> Outcome<AuthenticatedSeriesMarketLinkRetirementV1> {
    require_distinct(&[
        root_account.clone(),
        link_account.clone(),
        refund_owner.clone(),
        neutral_lamport_sink.clone(),
    ])?;
    let root_state = root.state();
    let root_binding = root_state.binding();
    let link_state = link.state();
    let link_binding = link_state.binding();
    let root_binding_id = root_binding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let link_binding_id = link_binding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        root.is_writable()
            && link.is_writable()
            && *root_account.key == root.account()
            && *link_account.key == link.account()
            && root.owner_program() == *program_id
            && link.owner_program() == *program_id
            && root_state.phase() == MarketLifecyclePhaseV1::Active
            && link_state.phase() == SeriesMarketLinkPhaseV1::Active
            && root_state.resolution_semantic_id() != ContentId::ZERO
            && root_state.resolution_data_id() != ContentId::ZERO
            && root_state.resolution_activation_receipt_id() != ContentId::ZERO
            && link_binding.market_instance_id == root_binding.market_instance_id
            && link_binding.generation == root_binding.generation
            && link_binding.market_root_account_id.bytes() == root.account().to_bytes()
            && link_binding.market_binding_id == root_binding_id
            && link_binding.rent_refund_owner.bytes() == refund_owner.key.to_bytes()
            && link_binding.neutral_lamport_sink.bytes()
                == neutral_lamport_sink.key.to_bytes()
            && link_state.market_admission_sequence() != 0
            && link_state.market_admission_receipt_id() != ContentId::ZERO
            && root_state.live_series_links() != 0
            && root_state.retired_series_links() < root_state.admitted_series_links()
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

    let observed_balance_lamports = link.observed_lamports();
    require(
        observed_balance_lamports == link_account.lamports(),
        ClutchError::MismatchedState,
    )?;
    let rent_principal_lamports = link_state.rent_principal_lamports();
    let surplus_lamports = observed_balance_lamports
        .checked_sub(rent_principal_lamports)
        .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        surplus_lamports >= link_state.current_donation_lamports(),
        ClutchError::MismatchedState,
    )?;
    let refund_balance_before = refund_owner.lamports();
    let refund_balance_after = refund_balance_before
        .checked_add(rent_principal_lamports)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let sink_balance_before = neutral_lamport_sink.lamports();
    let sink_balance_after = sink_balance_before
        .checked_add(surplus_lamports)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;

    let root_semantic_before = root_state
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let link_semantic_before = link_state
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    link_state
        .begin_retirement_into(link_retiring_output)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let retirement: SeriesMarketLinkRetirementProjectionV1 = link_retiring_output
        .retirement_projection()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    root_state
        .retire_series_link_into(retirement, root_successor_output)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    link_retiring_output
        .mark_retired_into(retirement, link_retired_output)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    let link_retiring = write_series_market_link_v1(
        program_id,
        link_account,
        link,
        link_retiring_output,
        link_retiring_rebound_output,
    )?;
    let root_rebound = write_market_lifecycle_root_v1(
        program_id,
        root_account,
        root,
        root_successor_output,
        root_rebound_output,
    )?;
    let link_rebound = write_series_market_link_v1(
        program_id,
        link_account,
        link_retiring,
        link_retired_output,
        link_retired_rebound_output,
    )?;

    let root_semantic_after = root_rebound
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let link_semantic_retiring = link_retiring_output
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let link_semantic_after = link_rebound
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        root_rebound.state() == &*root_successor_output
            && link_rebound.state() == &*link_retired_output
            && root_rebound.state().phase() == MarketLifecyclePhaseV1::Active
            && link_rebound.state().phase() == SeriesMarketLinkPhaseV1::Retired
            && link_rebound.observed_lamports() == observed_balance_lamports
            && link_account.lamports() == observed_balance_lamports,
        ClutchError::MismatchedState,
    )?;

    let facts = SeriesMarketLinkRetirementPostwriteFactsV1 {
        root_account: root.account(),
        root_binding_id,
        root_authentication_before: root.authentication_id(),
        root_authentication_after: root_rebound.authentication_id(),
        root_semantic_before,
        root_semantic_after,
        root_data_before: root.data_id(),
        root_data_after: root_rebound.data_id(),
        root_transition_sequence_before: root_state.transition_sequence(),
        root_transition_sequence_after: root_rebound.state().transition_sequence(),
        admitted_series_links: root_rebound.state().admitted_series_links(),
        live_series_links_before: root_state.live_series_links(),
        live_series_links_after: root_rebound.state().live_series_links(),
        retired_series_links_before: root_state.retired_series_links(),
        retired_series_links_after: root_rebound.state().retired_series_links(),
        resolution_semantic_id: root_state.resolution_semantic_id(),
        resolution_data_id: root_state.resolution_data_id(),
        resolution_activation_receipt_id: root_state.resolution_activation_receipt_id(),
        link_account: link.account(),
        link_binding_id,
        link_authentication_before: link.authentication_id(),
        link_authentication_after: link_rebound.authentication_id(),
        link_semantic_before,
        link_semantic_retiring,
        link_semantic_after,
        link_data_before: link.data_id(),
        link_data_after: link_rebound.data_id(),
        link_transition_sequence_before: link_state.transition_sequence(),
        link_transition_sequence_after: link_rebound.state().transition_sequence(),
        retirement_projection_id: retirement.id(),
        market_admission_receipt_id: link_state.market_admission_receipt_id(),
        market_instance_id: root_binding.market_instance_id,
        generation: root_binding.generation,
        series_plan_id: link_binding.series_plan_id,
        ordinal: link_binding.ordinal,
        rent_refund_owner: *refund_owner.key,
        neutral_lamport_sink: *neutral_lamport_sink.key,
        observed_balance_lamports,
        rent_principal_lamports,
        surplus_lamports,
        refund_balance_before,
        refund_balance_after,
        sink_balance_before,
        sink_balance_after,
    };
    let facts_id = facts.id()?;

    debit_program_owned_lamports_v1(link_account, observed_balance_lamports)?;
    credit_program_owned_lamports_v1(refund_owner, rent_principal_lamports)?;
    credit_program_owned_lamports_v1(neutral_lamport_sink, surplus_lamports)?;
    link_account
        .resize(0)
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    link_account.assign(&SYSTEM_PROGRAM_ID);
    require(
        link_account.lamports() == 0
            && link_account.data_len() == 0
            && link_account.owner.to_bytes() == SYSTEM_PROGRAM_ID
            && refund_owner.lamports() == refund_balance_after
            && neutral_lamport_sink.lamports() == sink_balance_after,
        ClutchError::MismatchedState,
    )?;

    let aggregate_postwrite_id = aggregate.accept_product_retirement(facts)?;
    require_live_content_id(aggregate_postwrite_id)?;
    require(
        aggregate_postwrite_id != facts_id
            && aggregate_postwrite_id != retirement.id()
            && aggregate_postwrite_id != link_state.market_admission_receipt_id(),
        ClutchError::MismatchedState,
    )?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SERIES_MARKET_LINK_CLOSE_AUTHENTICATION_DOMAIN_V1,
            &facts_id.bytes(),
            &aggregate_postwrite_id.bytes(),
        ])
        .to_bytes(),
    );
    require_live_content_id(id)?;
    Ok(AuthenticatedSeriesMarketLinkRetirementV1 {
        id,
        aggregate_postwrite_id,
        facts,
    })
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

/// Promote an exact unresolved Market/link pair into one exclusive Failure pin.
///
/// The root is hostile-reauthenticated read-only in this call. Once Resolution
/// has been recorded, its three persistent fields permanently refuse another
/// subordinate session even after a prior cell was archived and the link pin
/// released.
#[allow(clippy::too_many_arguments)]
pub(crate) fn pin_series_market_link_failure_v1<
    'next,
    A: AuthenticatedSeriesFailureSessionBeginV2 + ?Sized,
>(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    authenticated_root: AuthenticatedMarketLifecycleRootV1<'_>,
    link_account: &AccountInfo<'_>,
    authenticated_link: AuthenticatedSeriesMarketLinkV1<'_>,
    begin_admission_receipt_id: ContentId,
    authority: &A,
    root_rebound_output: &mut MarketLifecycleRootAccountV1,
    link_rebound_output: &'next mut SeriesMarketLinkAccountV1,
) -> Outcome<AuthenticatedSeriesMarketLinkV1<'next>> {
    require_live_content_id(begin_admission_receipt_id)?;
    let root_binding = authenticated_root.state().binding();
    let live_root = authenticate_market_lifecycle_root_v1(
        program_id,
        root_account,
        root_binding.market_instance_id,
        root_binding.generation,
        false,
        root_rebound_output,
    )?;
    let link_binding = authenticated_link.state().binding();
    let root_binding_id = root_binding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require_unresolved_market_resolution_v1(
        live_root.state().resolution_semantic_id(),
        live_root.state().resolution_data_id(),
        live_root.state().resolution_activation_receipt_id(),
    )?;
    require(
        !authenticated_root.is_writable()
            && live_root.account() == authenticated_root.account()
            && live_root.owner_program() == authenticated_root.owner_program()
            && live_root.state() == authenticated_root.state()
            && live_root.observed_lamports() == authenticated_root.observed_lamports()
            && live_root.data_id() == authenticated_root.data_id()
            && live_root.authentication_id() == authenticated_root.authentication_id()
            && root_account.key != link_account.key
            && live_root.state().phase() == MarketLifecyclePhaseV1::Active
            && authenticated_link.is_writable()
            && authenticated_link.state().phase() == SeriesMarketLinkPhaseV1::Active
            && authenticated_link.state().active_failure_sessions() == 0
            && link_binding.market_root_account_id.bytes() == root_account.key.to_bytes()
            && link_binding.market_binding_id == root_binding_id
            && link_binding.market_instance_id == root_binding.market_instance_id
            && link_binding.generation == root_binding.generation,
        ClutchError::MismatchedState,
    )?;
    authority.authenticate_series_failure_session_begin_v2(
        *root_account.key,
        live_root.authentication_id(),
        *link_account.key,
        authenticated_link.authentication_id(),
        link_binding.series_plan_id,
        link_binding.ordinal,
        link_binding.market_instance_id,
        link_binding.generation,
        link_binding.source_occurrence_id,
        begin_admission_receipt_id,
    )?;
    let successor = authenticated_link
        .state()
        .pin_failure_session(begin_admission_receipt_id)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    write_series_market_link_v1(
        program_id,
        link_account,
        authenticated_link,
        &successor,
        link_rebound_output,
    )
}

fn require_unresolved_market_resolution_v1(
    resolution_semantic_id: ContentId,
    resolution_data_id: ContentId,
    resolution_activation_receipt_id: ContentId,
) -> Outcome<()> {
    require(
        resolution_semantic_id == ContentId::ZERO
            && resolution_data_id == ContentId::ZERO
            && resolution_activation_receipt_id == ContentId::ZERO,
        ClutchError::MismatchedState,
    )
}

/// Hostile-authenticate the exact pinned `0xad` prestate for an atomic
/// Resolution/archive/release outer which necessarily receives the account as
/// writable. The compact returned value commits the exact hostile data,
/// authentication, semantic, binding, and session facts but contains no writer;
/// only [`release_series_market_link_failure_v2`] may later consume it to prove
/// that the same prestate was released.
pub(crate) fn authenticate_writable_failure_resolution_link_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    root: AuthenticatedMarketLifecycleRootV1<'_>,
    output: &mut SeriesMarketLinkAccountV1,
) -> Outcome<AuthenticatedWritableFailureResolutionLinkV1> {
    let root_state = root.state();
    let root_binding = root_state.binding();
    let root_binding_id = root_binding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        root.is_writable()
            && root_state.phase() == MarketLifecyclePhaseV1::Active
            && root_state.resolution_semantic_id() == ContentId::ZERO
            && root_state.resolution_data_id() == ContentId::ZERO
            && root_state.resolution_activation_receipt_id() == ContentId::ZERO,
        ClutchError::MismatchedState,
    )?;

    // The Product-owned hostile codec may discover only the immutable Series
    // coordinate from this exact body. Its Market/root half is independently
    // constrained by the authenticated root below before the projection is
    // minted.
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    SeriesMarketLinkAccountV1::decode_into(&data, output)?;
    let decoded_binding = output.state.binding();
    drop(data);
    let live = authenticate_series_market_link_v1(
        program_id,
        account,
        decoded_binding.series_plan_id,
        decoded_binding.ordinal,
        root_binding.market_instance_id,
        root_binding.generation,
        root.account(),
        true,
        output,
    )?;
    let state = *live.state();
    let binding = state.binding();
    let semantic_id = state
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let market_binding_id = binding.market_binding_id;
    let failure_session_transcript_id = state.failure_session_transcript_id();
    require(
        state.phase() == SeriesMarketLinkPhaseV1::Active
            && state.active_failure_sessions() == 1
            && state.failure_sessions_started() != 0
            && !failure_session_transcript_id.is_zero()
            && binding.market_root_account_id.bytes() == root.account().to_bytes()
            && market_binding_id == root_binding_id
            && binding.market_instance_id == root_binding.market_instance_id
            && binding.generation == root_binding.generation,
        ClutchError::MismatchedState,
    )?;
    let mut authenticated = AuthenticatedWritableFailureResolutionLinkV1 {
        id: ContentId::ZERO,
        link_account: live.account(),
        owner_program: live.owner_program(),
        observed_lamports: live.observed_lamports(),
        data_id: live.data_id(),
        authentication_id: live.authentication_id(),
        semantic_id,
        market_root_account: Pubkey::new_from_array(binding.market_root_account_id.bytes()),
        market_binding_id,
        series_plan_id: binding.series_plan_id,
        ordinal: binding.ordinal,
        market_instance_id: binding.market_instance_id,
        generation: binding.generation,
        source_occurrence_id: binding.source_occurrence_id,
        transition_sequence: state.transition_sequence(),
        failure_sessions_started: state.failure_sessions_started(),
        active_failure_sessions: state.active_failure_sessions(),
        failure_session_transcript_id,
    };
    authenticated.id =
        writable_failure_resolution_link_preauthorization_id_v1(program_id, &authenticated);
    require_live_content_id(authenticated.id)?;
    Ok(authenticated)
}

/// Hostile-authenticate the exact unresolved read-only root and writable
/// pinned link for the finite exhausted-session archive path.
#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticate_writable_failure_exhausted_link_v2(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    root: AuthenticatedMarketLifecycleRootV1<'_>,
    link_account: &AccountInfo<'_>,
    root_output: &mut MarketLifecycleRootAccountV1,
    link_output: &mut SeriesMarketLinkAccountV1,
) -> Outcome<AuthenticatedWritableFailureSessionReleaseLinkV2> {
    let cached_binding = root.state().binding();
    let live_root = authenticate_market_lifecycle_root_v1(
        program_id,
        root_account,
        cached_binding.market_instance_id,
        cached_binding.generation,
        false,
        root_output,
    )?;
    let root_binding = live_root.state().binding();
    let root_binding_id = root_binding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let root_semantic_id = live_root
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require_unresolved_market_resolution_v1(
        live_root.state().resolution_semantic_id(),
        live_root.state().resolution_data_id(),
        live_root.state().resolution_activation_receipt_id(),
    )?;
    require(
        !root.is_writable()
            && !live_root.is_writable()
            && live_root.account() == root.account()
            && live_root.owner_program() == root.owner_program()
            && live_root.state() == root.state()
            && live_root.observed_lamports() == root.observed_lamports()
            && live_root.data_id() == root.data_id()
            && live_root.authentication_id() == root.authentication_id()
            && live_root.state().phase() == MarketLifecyclePhaseV1::Active
            && root_account.key != link_account.key,
        ClutchError::MismatchedState,
    )?;

    let data = link_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    SeriesMarketLinkAccountV1::decode_into(&data, link_output)?;
    let decoded_binding = link_output.state.binding();
    drop(data);
    let link = authenticate_series_market_link_v1(
        program_id,
        link_account,
        decoded_binding.series_plan_id,
        decoded_binding.ordinal,
        root_binding.market_instance_id,
        root_binding.generation,
        live_root.account(),
        true,
        link_output,
    )?;
    let state = *link.state();
    let binding = state.binding();
    let semantic_id = state
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let failure_session_transcript_id = state.failure_session_transcript_id();
    require(
        link.is_writable()
            && state.phase() == SeriesMarketLinkPhaseV1::Active
            && state.active_failure_sessions() == 1
            && state.failure_sessions_started() != 0
            && !failure_session_transcript_id.is_zero()
            && binding.market_root_account_id.bytes() == live_root.account().to_bytes()
            && binding.market_binding_id == root_binding_id
            && binding.market_instance_id == root_binding.market_instance_id
            && binding.generation == root_binding.generation,
        ClutchError::MismatchedState,
    )?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SERIES_FAILURE_EXHAUSTED_LINK_PREAUTHORIZATION_DOMAIN_V2,
            &[FailureSessionReleaseDispositionV2::Exhausted.wire_byte()],
            program_id.as_ref(),
            live_root.account().as_ref(),
            &live_root.owner_program().to_bytes(),
            &live_root.observed_lamports().to_le_bytes(),
            &live_root.data_id().bytes(),
            &live_root.authentication_id().bytes(),
            &root_semantic_id.bytes(),
            link.account().as_ref(),
            &link.owner_program().to_bytes(),
            &link.observed_lamports().to_le_bytes(),
            &link.data_id().bytes(),
            &link.authentication_id().bytes(),
            &semantic_id.bytes(),
            &root_binding_id.bytes(),
            &binding.series_plan_id.bytes(),
            &binding.ordinal.to_le_bytes(),
            &binding.market_instance_id.bytes(),
            &binding.generation.to_le_bytes(),
            &binding.source_occurrence_id.bytes(),
            &state.transition_sequence().to_le_bytes(),
            &state.failure_sessions_started().to_le_bytes(),
            &failure_session_transcript_id.bytes(),
        ])
        .to_bytes(),
    );
    require_live_content_id(id)?;
    Ok(AuthenticatedWritableFailureSessionReleaseLinkV2::Exhausted(
        AuthenticatedWritableFailureExhaustedLinkV2 {
            id,
            root_account: live_root.account(),
            link_account: link.account(),
            owner_program: link.owner_program(),
            observed_lamports: link.observed_lamports(),
            data_id: link.data_id(),
            authentication_id: link.authentication_id(),
            semantic_id,
            market_binding_id: root_binding_id,
            series_plan_id: binding.series_plan_id,
            ordinal: binding.ordinal,
            market_instance_id: binding.market_instance_id,
            generation: binding.generation,
            source_occurrence_id: binding.source_occurrence_id,
            transition_sequence: state.transition_sequence(),
            failure_sessions_started: state.failure_sessions_started(),
            failure_session_transcript_id,
        },
    ))
}

/// Release one exact subordinate Failure session only after its terminal cell
/// was appended to durable market history and reset to canonical Idle.
pub(crate) fn release_series_market_link_failure_v2<
    A: AuthenticatedSeriesFailureArchivePostwriteV2 + ?Sized,
>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    authenticated: AuthenticatedSeriesMarketLinkV1<'_>,
    release_link: &AuthenticatedWritableFailureSessionReleaseLinkV2,
    archive: &A,
    rebound_output: &mut SeriesMarketLinkAccountV1,
) -> Outcome<AuthenticatedSeriesFailureSessionReleaseV2> {
    let binding = authenticated.state().binding();
    let semantic_before = authenticated
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .content_id();
    let authentication_before = authenticated.authentication_id();
    let transition_sequence_before = authenticated.state().transition_sequence();
    let failure_session_transcript_before =
        authenticated.state().failure_session_transcript_id();
    let failure_sessions_started_before = authenticated.state().failure_sessions_started();
    let archive_postwrite_id = archive.archive_postwrite_id()?;
    let append_receipt_id = archive.append_receipt_id()?;
    let reset_receipt_id = archive.reset_receipt_id()?;
    let market_instance_id = archive.market_instance_id()?;
    let generation = archive.generation()?;
    let source_occurrence_id = archive.source_occurrence_id()?;
    let session_binding_id = archive.session_binding_id()?;
    let session_terminal_receipt_id = archive.session_terminal_receipt_id()?;
    let release_disposition = archive.release_disposition()?;
    let release_link_preauthorization_id = archive.release_link_preauthorization_id()?;
    require_failure_session_release_disposition_v2(
        release_link.disposition(),
        release_disposition,
    )?;
    for receipt in [
        archive_postwrite_id,
        append_receipt_id,
        reset_receipt_id,
        session_binding_id,
        session_terminal_receipt_id,
        release_link_preauthorization_id,
    ] {
        require_live_content_id(receipt)?;
    }
    require(
        authenticated.is_writable()
            && authenticated.state().phase() == SeriesMarketLinkPhaseV1::Active
            && authenticated.state().active_failure_sessions() == 1
            && release_link.id() == release_link_preauthorization_id
            && release_link.link_account() == *account.key
            && release_link.owner_program() == *program_id
            && release_link.observed_lamports() == authenticated.observed_lamports()
            && release_link.data_id() == authenticated.data_id()
            && release_link.authentication_id() == authentication_before
            && release_link.semantic_id().content_id() == semantic_before
            && release_link.market_root_account()
                == Pubkey::new_from_array(binding.market_root_account_id.bytes())
            && release_link.market_binding_id() == binding.market_binding_id
            && release_link.series_plan_id() == binding.series_plan_id
            && release_link.ordinal() == binding.ordinal
            && release_link.market_instance_id() == binding.market_instance_id
            && release_link.generation() == binding.generation
            && release_link.source_occurrence_id() == binding.source_occurrence_id
            && release_link.transition_sequence() == transition_sequence_before
            && release_link.failure_sessions_started() == failure_sessions_started_before
            && release_link.failure_session_transcript_id()
                == failure_session_transcript_before
            && failure_session_transcript_before == session_binding_id
            && binding.market_instance_id == market_instance_id
            && binding.generation == generation
            && binding.source_occurrence_id == source_occurrence_id
            && archive_postwrite_id != append_receipt_id
            && archive_postwrite_id != reset_receipt_id
            && append_receipt_id != reset_receipt_id
            && session_terminal_receipt_id != session_binding_id,
        ClutchError::MismatchedState,
    )?;
    archive.authenticate_series_failure_archive_release_postwrite_v2(
        archive_postwrite_id,
        append_receipt_id,
        reset_receipt_id,
        market_instance_id,
        generation,
        source_occurrence_id,
        session_binding_id,
        session_terminal_receipt_id,
        release_disposition,
        release_link_preauthorization_id,
    )?;
    let successor = authenticated
        .state()
        .release_failure_session(session_terminal_receipt_id)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
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
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .content_id();
    let transition_sequence_after = rebound.state().transition_sequence();
    let failure_session_transcript_after = rebound.state().failure_session_transcript_id();
    require(
        rebound.state().phase() == SeriesMarketLinkPhaseV1::Active
            && rebound.state().active_failure_sessions() == 0
            && rebound.state().failure_sessions_started() == failure_sessions_started_before
            && transition_sequence_after
                == transition_sequence_before
                    .checked_add(1)
                    .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?
            && failure_session_transcript_after != failure_session_transcript_before,
        ClutchError::MismatchedState,
    )?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SERIES_FAILURE_RELEASE_AUTHENTICATION_DOMAIN_V2,
            account.key.as_ref(),
            &authentication_before.bytes(),
            &rebound.authentication_id().bytes(),
            &semantic_before.bytes(),
            &semantic_after.bytes(),
            &transition_sequence_before.to_le_bytes(),
            &transition_sequence_after.to_le_bytes(),
            &failure_session_transcript_before.bytes(),
            &failure_session_transcript_after.bytes(),
            &session_terminal_receipt_id.bytes(),
            &archive_postwrite_id.bytes(),
            &append_receipt_id.bytes(),
            &reset_receipt_id.bytes(),
            &[release_disposition.wire_byte()],
            &release_link_preauthorization_id.bytes(),
            &binding.series_plan_id.bytes(),
            &binding.ordinal.to_le_bytes(),
            &binding.market_instance_id.bytes(),
            &binding.generation.to_le_bytes(),
            &binding.source_occurrence_id.bytes(),
        ])
        .to_bytes(),
    );
    require_live_content_id(id)?;
    Ok(AuthenticatedSeriesFailureSessionReleaseV2 {
        id,
        link_account: *account.key,
        link_authentication_before: authentication_before,
        link_authentication_after: rebound.authentication_id(),
        link_semantic_before: semantic_before,
        link_semantic_after: semantic_after,
        transition_sequence_before,
        transition_sequence_after,
        failure_session_transcript_before,
        failure_session_transcript_after,
        session_terminal_receipt_id,
        archive_postwrite_id,
        append_receipt_id,
        reset_receipt_id,
        release_link_preauthorization_id,
        release_disposition,
    })
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

#[cfg(test)]
mod adversarial_resolution_repin_tests {
    use super::*;

    fn id(byte: u8) -> ContentId {
        ContentId::from_bytes([byte; 32])
    }

    fn resolution_link_preauthorization() -> AuthenticatedWritableFailureResolutionLinkV1 {
        AuthenticatedWritableFailureResolutionLinkV1 {
            id: ContentId::ZERO,
            link_account: Pubkey::new_from_array([40; 32]),
            owner_program: Pubkey::new_from_array([41; 32]),
            observed_lamports: 42,
            data_id: id(43),
            authentication_id: id(44),
            semantic_id: SeriesMarketLinkV1Id::from_bytes([45; 32]),
            market_root_account: Pubkey::new_from_array([46; 32]),
            market_binding_id: id(47),
            series_plan_id: SeriesPlanV5Id::from_bytes([48; 32]),
            ordinal: 49,
            market_instance_id: MarketInstanceV2Id::from_bytes([50; 32]),
            generation: 51,
            source_occurrence_id: SourceOccurrenceV1Id::from_bytes([52; 32]),
            transition_sequence: 53,
            failure_sessions_started: 54,
            active_failure_sessions: 1,
            failure_session_transcript_id: id(55),
        }
    }

    fn founder_authority() -> AuthenticatedMarketFounderFoundationV1 {
        AuthenticatedMarketFounderFoundationV1 {
            id: id(1),
            root_account: Pubkey::new_from_array([2; 32]),
            root_authentication_id: id(3),
            link_account: Pubkey::new_from_array([4; 32]),
            link_authentication_id: id(5),
            founder_link_id: SeriesMarketLinkV1Id::from_bytes([6; 32]),
            market_instance_id: MarketInstanceV2Id::from_bytes([7; 32]),
            generation: 8,
            series_plan_id: SeriesPlanV5Id::from_bytes([9; 32]),
            ordinal: 10,
            funding_terms_id: id(11),
            funding_quote_id: id(12),
            attachment_plan_id: id(13),
            compiler_bundle_id: id(14),
            registry_release_id: id(15),
            capability_profile_id: id(16),
            foundation_schedule_id: id(17),
            foundation_account_graph_id: id(18),
        }
    }

    fn foundation_debit() -> AuthenticatedMarketFoundationDebitV1 {
        let founder = founder_authority();
        AuthenticatedMarketFoundationDebitV1 {
            id: id(19),
            root_account: founder.root_account,
            root_authentication_id: founder.root_authentication_id,
            market_binding_id: id(20),
            failure_policy_binding_id: id(21),
            market_instance_id: founder.market_instance_id,
            generation: founder.generation,
            founder_link_id: founder.founder_link_id,
            funding_quote_id: founder.funding_quote_id,
            foundation_schedule_id: founder.foundation_schedule_id,
            foundation_account_graph_id: founder.foundation_account_graph_id,
            slot: MarketFoundationSlotV2::ClaimLedger,
            root_transition_sequence: 22,
            foundation_vault: Pubkey::new_from_array([23; 32]),
            destination: Pubkey::new_from_array([24; 32]),
            principal_lamports: 25,
            principal_before_lamports: 26,
            principal_after_lamports: 1,
            vault_donation_lamports: 27,
            destination_donation_floor_lamports: 28,
            destination_observed_balance_lamports: 53,
            rent_refund_owner: Pubkey::new_from_array([29; 32]),
            neutral_lamport_sink: Pubkey::new_from_array([30; 32]),
        }
    }

    struct NoFoundationPostwrite;

    impl AuthenticatedMarketFoundationStepPostwriteV2 for NoFoundationPostwrite {}

    struct NoFoundationActivation;

    impl AuthenticatedMarketFoundationActivationPostwriteV2 for NoFoundationActivation {}

    struct NoConvergerAdmission;

    impl AuthenticatedConvergerSeriesMarketAdmissionV1 for NoConvergerAdmission {}

    #[test]
    fn founder_authority_refuses_schedule_graph_and_root_splices() {
        let founder = founder_authority();
        let debit = foundation_debit();
        assert!(founder.authenticate_debit(debit).is_ok());

        let mut wrong_root = debit;
        wrong_root.root_authentication_id = id(31);
        assert!(founder.authenticate_debit(wrong_root).is_err());

        let mut wrong_schedule = debit;
        wrong_schedule.foundation_schedule_id = id(32);
        assert!(founder.authenticate_debit(wrong_schedule).is_err());

        let mut wrong_graph = debit;
        wrong_graph.foundation_account_graph_id = id(33);
        assert!(founder.authenticate_debit(wrong_graph).is_err());
    }

    #[test]
    fn default_foundation_postwrite_owner_cannot_advance_a_slot() {
        let owner = NoFoundationPostwrite;
        assert!(owner.accepted_poststate_receipt_id().is_err());
        let debit = foundation_debit();
        assert!(owner
            .authenticate_market_foundation_step_postwrite_v2(
                founder_authority().id,
                debit.id,
                debit.market_instance_id,
                debit.generation,
                debit.slot,
                debit.destination,
                debit.principal_lamports,
                debit.destination_donation_floor_lamports,
                debit.destination_observed_balance_lamports,
                debit.rent_refund_owner,
                debit.neutral_lamport_sink,
                id(34),
            )
            .is_err());
    }

    #[test]
    fn default_foundation_activation_owner_cannot_activate_market() {
        let owner = NoFoundationActivation;
        assert!(owner.accepted_market_core_receipt_id().is_err());
        assert!(owner
            .authenticate_market_foundation_activation_postwrite_v2(
                id(1),
                Pubkey::new_from_array([2; 32]),
                id(3),
                MarketInstanceV2Id::from_bytes([4; 32]),
                5,
                Pubkey::new_from_array([6; 32]),
                SeriesMarketLinkV1Id::from_bytes([7; 32]),
                id(8),
                id(9),
                id(10),
                id(11),
            )
            .is_err());
    }

    #[test]
    fn default_converger_owner_cannot_admit_pending_link() {
        let owner = NoConvergerAdmission;
        assert!(owner
            .authenticate_converger_series_market_admission_v1(
                Pubkey::new_from_array([1; 32]),
                id(2),
                Pubkey::new_from_array([3; 32]),
                id(4),
                SeriesMarketLinkV1Id::from_bytes([5; 32]),
                SeriesPlanV5Id::from_bytes([6; 32]),
                7,
                MarketInstanceV2Id::from_bytes([8; 32]),
                9,
                SourceOccurrenceV1Id::from_bytes([10; 32]),
                id(11),
                12,
                id(13),
            )
            .is_err());
    }

    #[test]
    fn any_persisted_resolution_field_permanently_refuses_failure_repin() {
        assert!(require_unresolved_market_resolution_v1(
            ContentId::ZERO,
            ContentId::ZERO,
            ContentId::ZERO,
        )
        .is_ok());
        for fields in [
            (ContentId::from_bytes([1; 32]), ContentId::ZERO, ContentId::ZERO),
            (ContentId::ZERO, ContentId::from_bytes([2; 32]), ContentId::ZERO),
            (ContentId::ZERO, ContentId::ZERO, ContentId::from_bytes([3; 32])),
        ] {
            assert!(require_unresolved_market_resolution_v1(fields.0, fields.1, fields.2).is_err());
        }
    }

    #[test]
    fn writable_resolution_link_identity_refuses_prestate_substitution() {
        let program_id = Pubkey::new_from_array([56; 32]);
        let exact = resolution_link_preauthorization();
        let exact_id = writable_failure_resolution_link_preauthorization_id_v1(
            &program_id,
            &exact,
        );
        let mut changed = resolution_link_preauthorization();
        changed.authentication_id = id(57);
        assert_ne!(
            writable_failure_resolution_link_preauthorization_id_v1(&program_id, &changed),
            exact_id,
        );
        let mut changed = resolution_link_preauthorization();
        changed.failure_session_transcript_id = id(58);
        assert_ne!(
            writable_failure_resolution_link_preauthorization_id_v1(&program_id, &changed),
            exact_id,
        );
        let mut changed = resolution_link_preauthorization();
        changed.ordinal = 59;
        assert_ne!(
            writable_failure_resolution_link_preauthorization_id_v1(&program_id, &changed),
            exact_id,
        );
        let mut changed = resolution_link_preauthorization();
        changed.market_root_account = Pubkey::new_from_array([60; 32]);
        assert_ne!(
            writable_failure_resolution_link_preauthorization_id_v1(&program_id, &changed),
            exact_id,
        );
    }

    #[test]
    fn writable_resolution_link_is_scoped_and_release_consumes_the_same_id() {
        let source = include_str!("product_market.rs");
        let preauth = source
            .split("pub(crate) fn authenticate_writable_failure_resolution_link_v1")
            .nth(1)
            .and_then(|value| value.split("/// Hostile-authenticate the exact unresolved").next())
            .expect("scoped writable preauthorization");
        for guard in [
            "root.is_writable()",
            "root_state.resolution_semantic_id() == ContentId::ZERO",
            "SeriesMarketLinkAccountV1::decode_into(&data, output)?",
            "authenticate_series_market_link_v1",
            "state.active_failure_sessions() == 1",
            "state.failure_sessions_started() != 0",
            "market_binding_id == root_binding_id",
            "failure_session_transcript_id",
        ] {
            assert!(preauth.contains(guard), "missing preauth guard {guard}");
        }
        let release = source
            .split("pub(crate) fn release_series_market_link_failure_v2")
            .nth(1)
            .and_then(|value| value.split("fn require_live_content_id").next())
            .expect("sole Product release");
        for guard in [
            "archive.release_link_preauthorization_id()?",
            "release_link.id() == release_link_preauthorization_id",
            "release_link.authentication_id() == authentication_before",
            "release_link.failure_session_transcript_id()",
            "archive.authenticate_series_failure_archive_release_postwrite_v2",
            "SERIES_FAILURE_RELEASE_AUTHENTICATION_DOMAIN_V2",
            "release_disposition.wire_byte()",
            "write_series_market_link_v1",
        ] {
            assert!(release.contains(guard), "missing release guard {guard}");
        }
    }
    #[test]
    fn founder_activation_has_one_non_detachable_current_outer() {
        let source = include_str!("product_market.rs");
        let outer = source
            .split("pub(crate) fn activate_and_complete_product_market_founder_v1")
            .nth(1)
            .and_then(|value| value.split("/// Admit and activate one converger").next())
            .expect("sole founder activation outer");
        for guard in [
            "authenticate_market_lifecycle_root_v1",
            "authenticate_series_market_link_v1",
            "read_series_registry_account_v2",
            "read_series_funding_account_v2",
            "authenticate_series_lifecycle_replay_v1",
            "authenticate_product_foundation_completion_v1",
            "activate_market_foundation_v2",
            "activate_founder_series_market_link_v1",
            "complete_and_record_series_lifecycle_admission_v1",
            "replay_postwrite.event_id() == occurrence.id()",
        ] {
            assert!(outer.contains(guard), "missing atomic guard {guard}");
        }
        for private_raw in [
            "\nfn activate_market_foundation_v2",
            "\nfn activate_founder_series_market_link_v1",
        ] {
            assert!(source.contains(private_raw), "raw writer escaped: {private_raw}");
        }
        for escaped_raw in [
            "pub(crate) fn activate_market_foundation_v2",
            "pub(crate) fn activate_founder_series_market_link_v1",
        ] {
            assert!(!source.contains(escaped_raw), "detachable writer: {escaped_raw}");
        }
    }

    #[test]
    fn founder_activation_refuses_stale_or_spliced_authority_sourcewise() {
        let source = include_str!("product_market.rs");
        let outer = source
            .split("pub(crate) fn activate_and_complete_product_market_founder_v1")
            .nth(1)
            .and_then(|value| value.split("/// Admit and activate one converger").next())
            .expect("sole founder activation outer");
        for refusal in [
            "capability.series_registry_account() == *registry_account.key",
            "registry.activation_consumed()",
            "registry.value().compiler_bundle_id == compiler_bundle_id",
            "bundle.funding_quote_id.content_id() == link_binding.funding_quote_id",
            "root.state().phase() == MarketLifecyclePhaseV1::Founding",
            "link.state().phase() == SeriesMarketLinkPhaseV1::PendingMarket",
            "funding.value().state.phase == clutch_product_series::SeriesFundingPhaseV2::Pending",
            "replay.state().binding().series_plan_id == link_binding.series_plan_id",
        ] {
            assert!(outer.contains(refusal), "missing splice refusal {refusal}");
        }
        let completion = source
            .split("fn authenticate_product_foundation_completion_v1")
            .nth(1)
            .and_then(|value| value.split("/// Activate one fully accepted").next())
            .expect("foundation completion authority");
        for refusal in [
            "root.state().foundation().complete()",
            "root.state().capital().principal_remaining_lamports == 0",
            "root.state().product_families().activation_ready()",
            "binding.foundation_account_graph_id == graph_id",
            "founder_link.authentication_id()",
        ] {
            assert!(completion.contains(refusal), "missing completion refusal {refusal}");
        }
    }
}

#[cfg(test)]
mod adversarial_series_link_retirement_tests {
    use super::*;

    fn id(byte: u8) -> ContentId {
        ContentId::from_bytes([byte; 32])
    }

    fn facts() -> SeriesMarketLinkRetirementPostwriteFactsV1 {
        SeriesMarketLinkRetirementPostwriteFactsV1 {
            root_account: Pubkey::new_from_array([1; 32]),
            root_binding_id: id(2),
            root_authentication_before: id(3),
            root_authentication_after: id(4),
            root_semantic_before: id(5),
            root_semantic_after: id(6),
            root_data_before: id(7),
            root_data_after: id(8),
            root_transition_sequence_before: 40,
            root_transition_sequence_after: 41,
            admitted_series_links: 3,
            live_series_links_before: 2,
            live_series_links_after: 1,
            retired_series_links_before: 1,
            retired_series_links_after: 2,
            resolution_semantic_id: id(9),
            resolution_data_id: id(10),
            resolution_activation_receipt_id: id(11),
            link_account: Pubkey::new_from_array([12; 32]),
            link_binding_id: id(13),
            link_authentication_before: id(14),
            link_authentication_after: id(15),
            link_semantic_before: SeriesMarketLinkV1Id::from_bytes([16; 32]),
            link_semantic_retiring: SeriesMarketLinkV1Id::from_bytes([17; 32]),
            link_semantic_after: SeriesMarketLinkV1Id::from_bytes([18; 32]),
            link_data_before: id(19),
            link_data_after: id(20),
            link_transition_sequence_before: 50,
            link_transition_sequence_after: 52,
            retirement_projection_id: id(21),
            market_admission_receipt_id: id(22),
            market_instance_id: MarketInstanceV2Id::from_bytes([23; 32]),
            generation: 24,
            series_plan_id: SeriesPlanV5Id::from_bytes([25; 32]),
            ordinal: 26,
            rent_refund_owner: Pubkey::new_from_array([27; 32]),
            neutral_lamport_sink: Pubkey::new_from_array([28; 32]),
            observed_balance_lamports: 100,
            rent_principal_lamports: 70,
            surplus_lamports: 30,
            refund_balance_before: 1_000,
            refund_balance_after: 1_070,
            sink_balance_before: 2_000,
            sink_balance_after: 2_030,
        }
    }

    #[test]
    fn exact_retirement_facts_are_canonical_and_substitution_changes_identity() {
        let exact = facts();
        let exact_id = exact.id().unwrap();
        let mut substituted = exact;
        substituted.market_admission_receipt_id = id(29);
        assert_ne!(substituted.id().unwrap(), exact_id);
        let mut substituted = exact;
        substituted.link_data_after = id(30);
        assert_ne!(substituted.id().unwrap(), exact_id);
    }

    #[test]
    fn unresolved_or_noncanonical_retirement_facts_refuse() {
        let exact = facts();
        for field in 0..3 {
            let mut unresolved = exact;
            match field {
                0 => unresolved.resolution_semantic_id = ContentId::ZERO,
                1 => unresolved.resolution_data_id = ContentId::ZERO,
                _ => unresolved.resolution_activation_receipt_id = ContentId::ZERO,
            }
            assert!(unresolved.validate().is_err());
        }

        let mut bad_sequence = exact;
        bad_sequence.link_transition_sequence_after = 51;
        assert!(bad_sequence.validate().is_err());

        let mut bad_count = exact;
        bad_count.live_series_links_after = 2;
        assert!(bad_count.validate().is_err());

        let mut bad_conservation = exact;
        bad_conservation.surplus_lamports = 29;
        assert!(bad_conservation.validate().is_err());

        let mut aliased = exact;
        aliased.neutral_lamport_sink = aliased.rent_refund_owner;
        assert!(aliased.validate().is_err());
    }

}

#[cfg(test)]
mod adversarial_failure_exhausted_release_tests {
    use super::*;

    #[test]
    fn exhausted_release_is_disjoint_and_requires_unresolved_root_and_exact_link() {
        assert_eq!(FailureSessionReleaseDispositionV2::Resolved.wire_byte(), 1);
        assert_eq!(FailureSessionReleaseDispositionV2::Exhausted.wire_byte(), 2);
        assert!(require_failure_session_release_disposition_v2(
            FailureSessionReleaseDispositionV2::Resolved,
            FailureSessionReleaseDispositionV2::Resolved,
        )
        .is_ok());
        assert!(require_failure_session_release_disposition_v2(
            FailureSessionReleaseDispositionV2::Exhausted,
            FailureSessionReleaseDispositionV2::Exhausted,
        )
        .is_ok());
        assert!(require_failure_session_release_disposition_v2(
            FailureSessionReleaseDispositionV2::Resolved,
            FailureSessionReleaseDispositionV2::Exhausted,
        )
        .is_err());

        let source = include_str!("product_market.rs");
        let exhausted = source
            .split("pub(crate) fn authenticate_writable_failure_exhausted_link_v2")
            .nth(1)
            .and_then(|value| value.split("/// Release one exact subordinate").next())
            .expect("typed exhausted preauthorization");
        for guard in [
            "!root.is_writable()",
            "!live_root.is_writable()",
            "require_unresolved_market_resolution_v1",
            "live_root.authentication_id() == root.authentication_id()",
            "SeriesMarketLinkAccountV1::decode_into(&data, link_output)?",
            "state.active_failure_sessions() == 1",
            "binding.market_binding_id == root_binding_id",
            "FailureSessionReleaseDispositionV2::Exhausted.wire_byte()",
        ] {
            assert!(exhausted.contains(guard), "missing exhausted guard {guard}");
        }
    }

}
