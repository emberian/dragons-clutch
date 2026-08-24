// SPDX-License-Identifier: AGPL-3.0-or-later
//! Private retirement of Source's persisted principal/donation custody.
//!
//! The program-owned custody body, not Product or an instruction payload,
//! owns allocated/remaining principal and every observed donation. Product's
//! final counted-retirement receipt authenticates lifecycle completion and the
//! immutable FundingTerms destinations before this adapter closes the ledger.

use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::{require_system_program, SYSTEM_PROGRAM_ID};
use crate::instructions::product_series_current::AuthenticatedSeriesMarketLinkV2;
use crate::source_plane_v3::runtime_key;
use crate::source_plane_v3_actions::authenticate_source_funding_custody_v1;
use clutch_source_plane_v3::ContentId;
use clutch_source_plane_v3_runtime::{
    account_data_id, AuthenticatedSourceRouteV1, RuntimeKey,
    SourceFundingCustodyLedgerV1, SourceWorkScheduleBindingV1,
};
use clutch_product_series::SeriesMarketLinkPhaseV2;
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

const SOURCE_FUNDING_CUSTODY_POSTTERMINAL_AUTH_DOMAIN_V2: &[u8] =
    b"dragons-clutch/sbf/source-funding-custody-postterminal-auth/v2";
const SOURCE_FUNDING_CUSTODY_RETIREMENT_DOMAIN_V2: &[u8] =
    b"dragons-clutch/sbf/source-funding-custody-retirement/v2";
const SOURCE_FUNDING_CUSTODY_LIFECYCLE_TERMINAL_DOMAIN_V1: &[u8] =
    b"dragons-clutch/sbf/source-funding-custody-lifecycle-terminal/v1";

/// Exhaustive terminal reason accepted by current Source custody retirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SourceFundingCustodyTerminalDispositionV1 {
    Successful,
    SourceAbsent,
    SourceRefused,
}

impl SourceFundingCustodyTerminalDispositionV1 {
    const fn wire_byte(self) -> u8 {
        match self {
            Self::Successful => 1,
            Self::SourceAbsent => 2,
            Self::SourceRefused => 3,
        }
    }
}

/// One canonical Source/Failure/Product terminal tuple. Amounts are excluded;
/// the hostile custody ledger remains their sole owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourceFundingCustodyLifecycleTerminalFactsV1 {
    pub(crate) disposition: SourceFundingCustodyTerminalDispositionV1,
    pub(crate) capitalization_authority_id: ContentId,
    pub(crate) capitalization_receipt_id: ContentId,
    pub(crate) pre_root_source_occurrence_id: ContentId,
    pub(crate) product_link_account: RuntimeKey,
    pub(crate) product_link_authentication_id: ContentId,
    pub(crate) product_link_semantic_id: ContentId,
    pub(crate) source_terminal_postwrite_id: ContentId,
    pub(crate) source_result_or_absence_close_receipt_id: ContentId,
    pub(crate) source_product_release_binding_id: ContentId,
    pub(crate) failure_family_terminal_receipt_id: ContentId,
    pub(crate) market_instance_id: ContentId,
    pub(crate) series_plan_id: ContentId,
    pub(crate) ordinal: u32,
    pub(crate) source_generation: u64,
    pub(crate) source_release_manifest_id: ContentId,
    pub(crate) source_release_authentication_id: ContentId,
    pub(crate) source_route_id: ContentId,
    pub(crate) source_work_schedule_id: ContentId,
    pub(crate) source_lifecycle_id: ContentId,
    pub(crate) source_occurrence_id: ContentId,
    pub(crate) source_occurrence_account: RuntimeKey,
    pub(crate) source_occurrence_authentication_id: ContentId,
    pub(crate) source_repair_generation: u64,
    pub(crate) source_funding_custody: RuntimeKey,
    pub(crate) lamport_principal_refund: RuntimeKey,
    pub(crate) neutral_lamport_sink: RuntimeKey,
}

impl SourceFundingCustodyLifecycleTerminalFactsV1 {
    fn id(self) -> ContentId {
        ContentId::from_bytes(
            solana_sha256_hasher::hashv(&[
                SOURCE_FUNDING_CUSTODY_LIFECYCLE_TERMINAL_DOMAIN_V1,
                &[self.disposition.wire_byte()],
                &self.capitalization_authority_id.bytes(),
                &self.capitalization_receipt_id.bytes(),
                &self.pre_root_source_occurrence_id.bytes(),
                &self.product_link_account.bytes(),
                &self.product_link_authentication_id.bytes(),
                &self.product_link_semantic_id.bytes(),
                &self.source_terminal_postwrite_id.bytes(),
                &self.source_result_or_absence_close_receipt_id.bytes(),
                &self.source_product_release_binding_id.bytes(),
                &self.failure_family_terminal_receipt_id.bytes(),
                &self.market_instance_id.bytes(),
                &self.series_plan_id.bytes(),
                &self.ordinal.to_le_bytes(),
                &self.source_generation.to_le_bytes(),
                &self.source_release_manifest_id.bytes(),
                &self.source_release_authentication_id.bytes(),
                &self.source_route_id.bytes(),
                &self.source_work_schedule_id.bytes(),
                &self.source_lifecycle_id.bytes(),
                &self.source_occurrence_id.bytes(),
                &self.source_occurrence_account.bytes(),
                &self.source_occurrence_authentication_id.bytes(),
                &self.source_repair_generation.to_le_bytes(),
                &self.source_funding_custody.bytes(),
                &self.lamport_principal_refund.bytes(),
                &self.neutral_lamport_sink.bytes(),
            ])
            .to_bytes(),
        )
    }
}

/// Exact founder/custody facts derived only from the hostile live 0xbd ledger,
/// authenticated retiring Product LinkV2, and authenticated route/schedule.
/// Failure consumes this private value while returning the rest of the final
/// branch tuple; callers never supply either side.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourceFundingCustodyLiveFounderFactsV1 {
    pub(crate) capitalization_authority_id: ContentId,
    pub(crate) capitalization_receipt_id: ContentId,
    pub(crate) pre_root_source_occurrence_id: ContentId,
    pub(crate) product_link_account: RuntimeKey,
    pub(crate) product_link_authentication_id: ContentId,
    pub(crate) product_link_semantic_id: ContentId,
    pub(crate) market_instance_id: ContentId,
    pub(crate) series_plan_id: ContentId,
    pub(crate) ordinal: u32,
    pub(crate) source_release_manifest_id: ContentId,
    pub(crate) source_release_authentication_id: ContentId,
    pub(crate) source_route_id: ContentId,
    pub(crate) source_work_schedule_id: ContentId,
    pub(crate) source_lifecycle_id: ContentId,
    pub(crate) source_generation: u64,
    pub(crate) source_occurrence_id: ContentId,
    pub(crate) source_occurrence_account: RuntimeKey,
    pub(crate) source_occurrence_authentication_id: ContentId,
    pub(crate) source_repair_generation: u64,
    pub(crate) source_funding_custody: RuntimeKey,
    pub(crate) lamport_principal_refund: RuntimeKey,
    pub(crate) neutral_lamport_sink: RuntimeKey,
}

/// Default-refusing boundary implemented only by Failure's exact final
/// successful or SourceAbsent/SourceRefused postwrite.
pub(crate) trait AuthenticatedSourceFundingCustodyLifecycleTerminalAuthorityV1 {
    /// Return the exact terminal tuple retained by the final Failure/Product
    /// postwrite. No instruction payload or caller projection supplies these
    /// identities.
    fn source_funding_custody_lifecycle_terminal_facts_v1(
        &self,
        _founder: SourceFundingCustodyLiveFounderFactsV1,
    ) -> Outcome<SourceFundingCustodyLifecycleTerminalFactsV1> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
}

/// Private non-Copy terminal capability consumed by Product retirement.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedSourceFundingCustodyLifecycleTerminalV1 {
    id: ContentId,
    facts: SourceFundingCustodyLifecycleTerminalFactsV1,
}

impl AuthenticatedSourceFundingCustodyLifecycleTerminalV1 {
    pub(crate) const fn id(&self) -> ContentId {
        self.id
    }

    pub(crate) const fn facts(&self) -> SourceFundingCustodyLifecycleTerminalFactsV1 {
        self.facts
    }
}

/// Authenticate one exhaustive final lifecycle tuple against the hostile live
/// custody and Failure's private terminal postwrite.
pub(crate) fn authenticate_source_funding_custody_lifecycle_terminal_v1<
    A: AuthenticatedSourceFundingCustodyLifecycleTerminalAuthorityV1 + ?Sized,
>(
    authority: &A,
    route: AuthenticatedSourceRouteV1,
    schedule: SourceWorkScheduleBindingV1,
    link: &AuthenticatedSeriesMarketLinkV2<'_>,
    custody: AuthenticatedSourceFundingCustodyV1,
) -> Outcome<AuthenticatedSourceFundingCustodyLifecycleTerminalV1> {
    let ledger = custody.ledger();
    let link_state = link.state();
    let link_binding = link_state.binding();
    let link_semantic_id = link_state
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let founder = SourceFundingCustodyLiveFounderFactsV1 {
        capitalization_authority_id: ledger.capitalization_authority_id,
        capitalization_receipt_id: ledger.capitalization_receipt_id,
        pre_root_source_occurrence_id: link_binding.source_occurrence_receipt_id,
        product_link_account: RuntimeKey::from_bytes(link.account().to_bytes()),
        product_link_authentication_id: ContentId::from_bytes(
            link.authentication_id().bytes(),
        ),
        product_link_semantic_id: ContentId::from_bytes(link_semantic_id.bytes()),
        market_instance_id: ContentId::from_bytes(link_binding.market_instance_id.bytes()),
        series_plan_id: ContentId::from_bytes(link_binding.series_plan_id.bytes()),
        ordinal: link_binding.ordinal,
        source_release_manifest_id: route.release_manifest_id(),
        source_release_authentication_id: route.release_authentication_id(),
        source_route_id: route.route_id(),
        source_work_schedule_id: schedule.source_work_schedule_id(),
        source_lifecycle_id: schedule.lifecycle_id(),
        source_generation: schedule.generation(),
        source_occurrence_id: ContentId::from_bytes(link_binding.source_occurrence_id.bytes()),
        source_occurrence_account: RuntimeKey::from_bytes(
            link_binding.source_occurrence_account_id.bytes(),
        ),
        source_occurrence_authentication_id:
            link_binding.source_occurrence_account_authentication_id,
        source_repair_generation: link_binding.source_repair_generation,
        source_funding_custody: custody.account(),
        lamport_principal_refund: ledger.principal_refund,
        neutral_lamport_sink: ledger.neutral_sink,
    };
    require(
        ledger.is_live()
            && !founder.capitalization_authority_id.is_zero()
            && !founder.capitalization_receipt_id.is_zero()
            && founder.capitalization_authority_id != founder.capitalization_receipt_id
            && link.is_writable()
            && link_state.phase() == SeriesMarketLinkPhaseV2::Retiring
            && !founder.product_link_authentication_id.is_zero()
            && !founder.product_link_semantic_id.is_zero()
            && link_binding.source_occurrence_receipt_id
                == founder.pre_root_source_occurrence_id
            && link_binding.market_instance_id.bytes() == founder.market_instance_id.bytes()
            && link_binding.series_plan_id.bytes() == founder.series_plan_id.bytes()
            && link_binding.ordinal == founder.ordinal
            && link_binding.source_release_id == route.release_manifest_id()
            && link_binding.source_route_id == route.route_id()
            && link_binding.clock_policy_id == route.clock_policy_id()
            && link_binding.source_plane_contract_id == route.source_plane_contract_id()
            && link_binding.source_spec_id == route.source_spec_id()
            && link_binding.generation == schedule.generation()
            && link_binding.source_occurrence_id.bytes() == founder.source_occurrence_id.bytes()
            && link_binding.source_occurrence_account_id.bytes()
                == founder.source_occurrence_account.bytes()
            && link_binding.source_occurrence_account_authentication_id
                == founder.source_occurrence_authentication_id
            && link_binding.source_repair_generation == founder.source_repair_generation
            && link_binding.rent_refund_owner.bytes() == ledger.principal_refund.bytes()
            && link_binding.neutral_lamport_sink.bytes() == ledger.neutral_sink.bytes()
            && ledger.release_manifest_id == route.release_manifest_id()
            && ledger.route_id == route.route_id()
            && ledger.source_work_schedule_id == schedule.source_work_schedule_id()
            && ledger.lifecycle_id == schedule.lifecycle_id()
            && ledger.custody_account == custody.account()
            && ledger.neutral_sink == route.neutral_sink(),
        ClutchError::MismatchedState,
    )?;
    let expected = authority.source_funding_custody_lifecycle_terminal_facts_v1(founder)?;
    let ids = [
        expected.capitalization_authority_id,
        expected.capitalization_receipt_id,
        expected.pre_root_source_occurrence_id,
        expected.product_link_authentication_id,
        expected.product_link_semantic_id,
        expected.source_terminal_postwrite_id,
        expected.source_result_or_absence_close_receipt_id,
        expected.source_product_release_binding_id,
        expected.failure_family_terminal_receipt_id,
        expected.market_instance_id,
        expected.series_plan_id,
        expected.source_release_manifest_id,
        expected.source_release_authentication_id,
        expected.source_route_id,
        expected.source_work_schedule_id,
        expected.source_lifecycle_id,
        expected.source_occurrence_id,
        expected.source_occurrence_authentication_id,
    ];
    require(
        ids.iter().all(|id| !id.is_zero())
            && all_distinct_ids(&ids)
            && custody.ledger().is_live()
            && expected.capitalization_authority_id == founder.capitalization_authority_id
            && expected.capitalization_receipt_id
                == founder.capitalization_receipt_id
            && expected.pre_root_source_occurrence_id
                == founder.pre_root_source_occurrence_id
            && expected.product_link_account == founder.product_link_account
            && expected.product_link_authentication_id
                == founder.product_link_authentication_id
            && expected.product_link_semantic_id == founder.product_link_semantic_id
            && expected.market_instance_id == founder.market_instance_id
            && expected.series_plan_id == founder.series_plan_id
            && expected.ordinal == founder.ordinal
            && expected.source_release_manifest_id == route.release_manifest_id()
            && expected.source_release_manifest_id == ledger.release_manifest_id
            && expected.source_release_authentication_id == route.release_authentication_id()
            && expected.source_route_id == route.route_id()
            && expected.source_work_schedule_id == schedule.source_work_schedule_id()
            && expected.source_lifecycle_id == schedule.lifecycle_id()
            && expected.source_generation == schedule.generation()
            && expected.source_generation == founder.source_generation
            && expected.source_occurrence_id == founder.source_occurrence_id
            && expected.source_occurrence_account == founder.source_occurrence_account
            && expected.source_occurrence_authentication_id
                == founder.source_occurrence_authentication_id
            && expected.source_repair_generation == founder.source_repair_generation
            && expected.source_funding_custody == custody.account()
            && expected.lamport_principal_refund == ledger.principal_refund
            && expected.neutral_lamport_sink == ledger.neutral_sink
            && expected.neutral_lamport_sink == route.neutral_sink()
            && expected.source_funding_custody != expected.lamport_principal_refund
            && expected.source_funding_custody != expected.neutral_lamport_sink
            && expected.source_funding_custody != expected.source_occurrence_account
            && expected.product_link_account != expected.source_funding_custody
            && expected.product_link_account != expected.source_occurrence_account
            && expected.product_link_account != expected.lamport_principal_refund
            && expected.product_link_account != expected.neutral_lamport_sink
            && expected.source_occurrence_account != expected.lamport_principal_refund
            && expected.source_occurrence_account != expected.neutral_lamport_sink
            && expected.lamport_principal_refund != expected.neutral_lamport_sink,
        ClutchError::MismatchedState,
    )?;
    let id = expected.id();
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(AuthenticatedSourceFundingCustodyLifecycleTerminalV1 { id, facts: expected })
}

/// Product-owned terminal identities and immutable destinations. No amount is
/// supplied: all lamport accounting comes from the hostile-decoded ledger.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourceFundingCustodyRetirementAccountingV2 {
    pub(crate) funding_terms_id: ContentId,
    pub(crate) product_retirement_authority_id: ContentId,
    pub(crate) pre_root_source_occurrence_id: ContentId,
    pub(crate) source_terminal_receipt_id: ContentId,
    pub(crate) source_result_or_absence_close_receipt_id: ContentId,
    pub(crate) source_product_release_binding_id: ContentId,
    pub(crate) failure_family_terminal_receipt_id: ContentId,
    pub(crate) counted_retirement_receipt_id: ContentId,
    pub(crate) source_funding_custody: RuntimeKey,
    pub(crate) lamport_principal_refund: RuntimeKey,
    pub(crate) neutral_lamport_sink: RuntimeKey,
}

/// Complete locally derived pre/post retirement facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourceFundingCustodyRetirementFactsV2 {
    pub(crate) accounting: SourceFundingCustodyRetirementAccountingV2,
    pub(crate) source_route_id: ContentId,
    pub(crate) source_work_schedule_id: ContentId,
    pub(crate) source_lifecycle_id: ContentId,
    pub(crate) custody_authentication_id: ContentId,
    pub(crate) custody_account_data_before_id: ContentId,
    pub(crate) ledger_before: SourceFundingCustodyLedgerV1,
    pub(crate) capitalization_receipt_id: ContentId,
    pub(crate) custody_balance_before: u64,
    pub(crate) allocated_principal_lamports: u64,
    pub(crate) completed_principal_lamports: u64,
    pub(crate) principal_refund_lamports: u64,
    pub(crate) neutral_donation_lamports: u64,
    pub(crate) principal_refund_balance_before: u64,
    pub(crate) principal_refund_balance_after: u64,
    pub(crate) neutral_sink_balance_before: u64,
    pub(crate) neutral_sink_balance_after: u64,
}

/// Default-refusing Product retirement owner.
pub(crate) trait AuthenticatedSourceFundingCustodyRetirementAuthorityV2 {
    fn authenticate_source_funding_custody_retirement_v2(
        &self,
        _facts: SourceFundingCustodyRetirementFactsV2,
    ) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
}

/// Private ledger-close postwrite consumed before Funding may close.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedSourceFundingCustodyRetirementV2 {
    id: ContentId,
    product_retirement_authority_id: ContentId,
    facts: SourceFundingCustodyRetirementFactsV2,
    custody_account_data_after_id: ContentId,
}

impl AuthenticatedSourceFundingCustodyRetirementV2 {
    pub(crate) const fn id(&self) -> ContentId {
        self.id
    }

    pub(crate) const fn product_retirement_authority_id(&self) -> ContentId {
        self.product_retirement_authority_id
    }

    pub(crate) const fn facts(&self) -> SourceFundingCustodyRetirementFactsV2 {
        self.facts
    }

    pub(crate) const fn custody_account_data_after_id(&self) -> ContentId {
        self.custody_account_data_after_id
    }
}

fn require_system_destination(account: &AccountInfo<'_>, expected: RuntimeKey) -> Outcome<()> {
    require(
        runtime_key(account.key) == expected
            && account.owner == &SYSTEM_PROGRAM_ID
            && account.data_is_empty()
            && account.is_writable
            && !account.is_signer
            && !account.executable,
        ClutchError::MismatchedState,
    )
}

/// Close one exact terminal custody. Remaining recorded principal returns to
/// FundingTerms; recorded and newly observed donations go only to the route's
/// neutral sink. Neither recipient signs.
#[allow(clippy::too_many_arguments)]
pub(crate) fn retire_source_funding_custody_v2<
    A: AuthenticatedSourceFundingCustodyRetirementAuthorityV2 + ?Sized,
>(
    program_id: &Pubkey,
    authority: &A,
    route: AuthenticatedSourceRouteV1,
    schedule: SourceWorkScheduleBindingV1,
    accounting: SourceFundingCustodyRetirementAccountingV2,
    custody_account: &AccountInfo<'_>,
    principal_refund: &AccountInfo<'_>,
    neutral_sink: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
) -> Outcome<AuthenticatedSourceFundingCustodyRetirementV2> {
    require_system_program(system_program)?;
    let custody = authenticate_source_funding_custody_v1(
        program_id,
        route,
        schedule,
        custody_account,
    )?;
    require_system_destination(principal_refund, accounting.lamport_principal_refund)?;
    require_system_destination(neutral_sink, accounting.neutral_lamport_sink)?;
    let terminal_ids = [
        accounting.funding_terms_id,
        accounting.product_retirement_authority_id,
        accounting.pre_root_source_occurrence_id,
        accounting.source_terminal_receipt_id,
        accounting.source_result_or_absence_close_receipt_id,
        accounting.source_product_release_binding_id,
        accounting.failure_family_terminal_receipt_id,
        accounting.counted_retirement_receipt_id,
    ];
    require(
        terminal_ids.iter().all(|id| !id.is_zero())
            && all_distinct_ids(&terminal_ids)
            && accounting.source_funding_custody == custody.account()
            && accounting.lamport_principal_refund == custody.ledger().principal_refund
            && accounting.neutral_lamport_sink == custody.ledger().neutral_sink
            && accounting.neutral_lamport_sink == route.neutral_sink()
            && principal_refund.key != neutral_sink.key
            && custody_account.key != principal_refund.key
            && custody_account.key != neutral_sink.key,
        ClutchError::MismatchedState,
    )?;
    let ledger_before = custody
        .ledger()
        .observe_terminal_balance(
            custody_account.lamports(),
            accounting.counted_retirement_receipt_id,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))?;
    require(
        ledger_before.is_live()
            && !ledger_before.capitalization_receipt_id.is_zero()
            && ledger_before.capitalization_receipt_id
                != ledger_before.capitalization_authority_id
            && terminal_ids
                .iter()
                .all(|id| *id != ledger_before.capitalization_receipt_id)
            && lifecycle_terminal_facts.capitalization_receipt_id
                == ledger_before.capitalization_receipt_id
            && lifecycle_terminal_facts.capitalization_authority_id
                == ledger_before.capitalization_authority_id,
        ClutchError::MismatchedState,
    )?;
    if ledger_before != custody.ledger() {
        let bytes = ledger_before
            .encode()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let mut data = custody_account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        require(data.len() == bytes.len(), ClutchError::WrongDataLength)?;
        data.copy_from_slice(&bytes);
    }
    let custody_balance_before = custody_account.lamports();
    let partition = ledger_before
        .remaining_principal_lamports
        .checked_add(ledger_before.donation_lamports)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        partition == custody_balance_before,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    let completed_principal_lamports = ledger_before
        .allocated_principal_lamports
        .checked_sub(ledger_before.remaining_principal_lamports)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let custody_data = custody_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let custody_account_data_before_id =
        account_data_id(runtime_key(custody_account.key), &custody_data)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    drop(custody_data);
    let ledger_before_id = ledger_before
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let custody_authentication_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SOURCE_FUNDING_CUSTODY_POSTTERMINAL_AUTH_DOMAIN_V2,
            &route.route_id().bytes(),
            &schedule.source_work_schedule_id().bytes(),
            &schedule.lifecycle_id().bytes(),
            custody_account.key.as_ref(),
            &custody_account_data_before_id.bytes(),
            &ledger_before_id.bytes(),
            &custody_balance_before.to_le_bytes(),
        ])
        .to_bytes(),
    );
    let principal_refund_balance_before = principal_refund.lamports();
    let neutral_sink_balance_before = neutral_sink.lamports();
    let principal_refund_balance_after = principal_refund_balance_before
        .checked_add(ledger_before.remaining_principal_lamports)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let neutral_sink_balance_after = neutral_sink_balance_before
        .checked_add(ledger_before.donation_lamports)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let facts = SourceFundingCustodyRetirementFactsV2 {
        accounting,
        source_route_id: route.route_id(),
        source_work_schedule_id: schedule.source_work_schedule_id(),
        source_lifecycle_id: schedule.lifecycle_id(),
        custody_authentication_id,
        custody_account_data_before_id,
        ledger_before,
        capitalization_receipt_id: ledger_before.capitalization_receipt_id,
        custody_balance_before,
        allocated_principal_lamports: ledger_before.allocated_principal_lamports,
        completed_principal_lamports,
        principal_refund_lamports: ledger_before.remaining_principal_lamports,
        neutral_donation_lamports: ledger_before.donation_lamports,
        principal_refund_balance_before,
        principal_refund_balance_after,
        neutral_sink_balance_before,
        neutral_sink_balance_after,
    };
    let product_retirement_authority_id =
        authority.authenticate_source_funding_custody_retirement_v2(facts)?;
    require(
        product_retirement_authority_id == accounting.product_retirement_authority_id,
        ClutchError::AuthorizationUnavailable,
    )?;
    {
        let mut custody_balance = custody_account
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let mut refund_balance = principal_refund
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let mut sink_balance = neutral_sink
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        **custody_balance = 0;
        **refund_balance = principal_refund_balance_after;
        **sink_balance = neutral_sink_balance_after;
    }
    custody_account
        .resize(0)
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    custody_account.assign(&SYSTEM_PROGRAM_ID);
    let custody_account_data_after_id = account_data_id(runtime_key(custody_account.key), &[])
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        custody_account.lamports() == 0
            && custody_account.owner == &SYSTEM_PROGRAM_ID
            && custody_account.data_is_empty()
            && principal_refund.lamports() == principal_refund_balance_after
            && neutral_sink.lamports() == neutral_sink_balance_after,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SOURCE_FUNDING_CUSTODY_RETIREMENT_DOMAIN_V2,
            &product_retirement_authority_id.bytes(),
            &accounting.funding_terms_id.bytes(),
            &ledger_before.capitalization_authority_id.bytes(),
            &ledger_before.capitalization_receipt_id.bytes(),
            &accounting.pre_root_source_occurrence_id.bytes(),
            &accounting.source_terminal_receipt_id.bytes(),
            &accounting.source_result_or_absence_close_receipt_id.bytes(),
            &accounting.source_product_release_binding_id.bytes(),
            &accounting.failure_family_terminal_receipt_id.bytes(),
            &accounting.counted_retirement_receipt_id.bytes(),
            &custody_authentication_id.bytes(),
            &custody_account_data_before_id.bytes(),
            &custody_account_data_after_id.bytes(),
            &ledger_before_id.bytes(),
            &principal_refund_balance_before.to_le_bytes(),
            &principal_refund_balance_after.to_le_bytes(),
            &neutral_sink_balance_before.to_le_bytes(),
            &neutral_sink_balance_after.to_le_bytes(),
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(AuthenticatedSourceFundingCustodyRetirementV2 {
        id,
        product_retirement_authority_id,
        facts,
        custody_account_data_after_id,
    })
}

fn all_distinct_ids(values: &[ContentId]) -> bool {
    let mut index = 0usize;
    while index < values.len() {
        let mut prior = 0usize;
        while prior < index {
            if values[prior] == values[index] {
                return false;
            }
            prior += 1;
        }
        index += 1;
    }
    true
}

#[cfg(test)]
mod adversarial_tests {
    use super::*;

    struct RefusingRetirement;
    impl AuthenticatedSourceFundingCustodyRetirementAuthorityV2 for RefusingRetirement {}

    #[test]
    fn default_retirement_authority_refuses() {
        let _ = RefusingRetirement;
    }

    #[test]
    fn lifecycle_terminal_facts_come_only_from_final_failure_authority() {
        let source = include_str!("source_funding_custody_retirement_v1.rs");
        let authenticate = source
            .split("pub(crate) fn authenticate_source_funding_custody_lifecycle_terminal_v1")
            .nth(1)
            .and_then(|value| {
                value
                    .split("/// Product-owned terminal identities")
                    .next()
            })
            .expect("bounded lifecycle terminal authentication");
        assert!(authenticate.contains(
            "authority.source_funding_custody_lifecycle_terminal_facts_v1(founder)?"
        ));
        assert!(!authenticate.contains(
            "expected: SourceFundingCustodyLifecycleTerminalFactsV1"
        ));
    }

    #[test]
    fn lifecycle_terminal_founder_is_derived_from_exact_retiring_link() {
        let source = include_str!("source_funding_custody_retirement_v1.rs");
        let authenticate = source
            .split("pub(crate) fn authenticate_source_funding_custody_lifecycle_terminal_v1")
            .nth(1)
            .and_then(|value| value.split("/// Product-owned terminal identities").next())
            .expect("bounded lifecycle terminal authentication");
        for exact_join in [
            "link_state.phase() == SeriesMarketLinkPhaseV2::Retiring",
            "product_link_account: RuntimeKey::from_bytes(link.account().to_bytes())",
            "link.authentication_id().bytes()",
            "product_link_semantic_id: ContentId::from_bytes(link_semantic_id.bytes())",
            "pre_root_source_occurrence_id: link_binding.source_occurrence_receipt_id",
            "source_occurrence_id: ContentId::from_bytes(link_binding.source_occurrence_id.bytes())",
            "link_binding.source_occurrence_account_id.bytes()",
            "link_binding.source_occurrence_account_authentication_id",
            "link_binding.market_instance_id.bytes() == founder.market_instance_id.bytes()",
            "link_binding.series_plan_id.bytes() == founder.series_plan_id.bytes()",
            "link_binding.ordinal == founder.ordinal",
            "link_binding.generation == schedule.generation()",
            "link_binding.source_repair_generation == founder.source_repair_generation",
        ] {
            assert!(authenticate.contains(exact_join), "missing {exact_join}");
        }
        for returned_join in [
            "expected.pre_root_source_occurrence_id",
            "expected.product_link_account == founder.product_link_account",
            "expected.product_link_authentication_id",
            "expected.product_link_semantic_id == founder.product_link_semantic_id",
            "expected.source_occurrence_id == founder.source_occurrence_id",
            "expected.source_occurrence_account == founder.source_occurrence_account",
            "expected.source_occurrence_authentication_id",
            "expected.source_repair_generation == founder.source_repair_generation",
        ] {
            assert!(authenticate.contains(returned_join), "missing {returned_join}");
        }
    }

    #[test]
    fn retirement_accepts_no_amount_or_signing_recipient() {
        let source = include_str!("source_funding_custody_retirement_v1.rs");
        let accounting = source
            .split("pub(crate) struct SourceFundingCustodyRetirementAccountingV2")
            .nth(1)
            .and_then(|value| value.split("/// Complete locally derived").next())
            .expect("caller-neutral retirement accounting");
        let retire = source
            .split("pub(crate) fn retire_source_funding_custody_v2")
            .nth(1)
            .expect("private ledger retirement");
        assert!(!accounting.contains("allocated_principal_lamports"));
        assert!(!accounting.contains("completed_principal_lamports"));
        assert!(!accounting.contains("capitalization_receipt_id"));
        assert!(source.contains(
            "capitalization_receipt_id: ledger_before.capitalization_receipt_id"
        ));
        assert!(retire.contains(".checked_sub(ledger_before.remaining_principal_lamports)"));
        assert!(retire.contains("ledger_before.remaining_principal_lamports"));
        assert!(retire.contains("ledger_before.donation_lamports"));
        assert!(source.contains("!account.is_signer"));
        assert!(source.contains("all_distinct_ids(&terminal_ids)"));
    }

    #[test]
    fn terminal_and_retirement_capabilities_are_non_clone_consumables() {
        let source = include_str!("source_funding_custody_retirement_v1.rs");
        for name in [
            "AuthenticatedSourceFundingCustodyLifecycleTerminalV1",
            "AuthenticatedSourceFundingCustodyRetirementV2",
        ] {
            let prefix = source
                .split(&format!("pub(crate) struct {name}"))
                .next()
                .expect("capability declaration")
                .rsplit("#[derive(")
                .next()
                .expect("derive list")
                .split(")]" )
                .next()
                .expect("bounded derive list");
            assert!(!prefix.contains("Clone"));
            assert!(!prefix.contains("Copy"));
        }
    }
}
