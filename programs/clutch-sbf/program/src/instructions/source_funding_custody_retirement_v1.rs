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
use crate::source_plane_v3::runtime_key;
use crate::source_plane_v3_actions::authenticate_source_funding_custody_v1;
use clutch_source_plane_v3::ContentId;
use clutch_source_plane_v3_runtime::{
    account_data_id, AuthenticatedSourceRouteV1, RuntimeKey,
    SourceFundingCustodyLedgerV1, SourceWorkScheduleBindingV1,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

const SOURCE_FUNDING_CUSTODY_POSTTERMINAL_AUTH_DOMAIN_V2: &[u8] =
    b"dragons-clutch/sbf/source-funding-custody-postterminal-auth/v2";
const SOURCE_FUNDING_CUSTODY_RETIREMENT_DOMAIN_V2: &[u8] =
    b"dragons-clutch/sbf/source-funding-custody-retirement/v2";

/// Product-owned terminal identities and immutable destinations. No amount is
/// supplied: all lamport accounting comes from the hostile-decoded ledger.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourceFundingCustodyRetirementAccountingV2 {
    pub(crate) funding_terms_id: ContentId,
    pub(crate) product_retirement_authority_id: ContentId,
    pub(crate) capitalization_receipt_id: ContentId,
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedSourceFundingCustodyRetirementV2 {
    id: ContentId,
    product_retirement_authority_id: ContentId,
    facts: SourceFundingCustodyRetirementFactsV2,
    custody_account_data_after_id: ContentId,
}

impl AuthenticatedSourceFundingCustodyRetirementV2 {
    pub(crate) const fn id(self) -> ContentId {
        self.id
    }

    pub(crate) const fn product_retirement_authority_id(self) -> ContentId {
        self.product_retirement_authority_id
    }

    pub(crate) const fn facts(self) -> SourceFundingCustodyRetirementFactsV2 {
        self.facts
    }

    pub(crate) const fn custody_account_data_after_id(self) -> ContentId {
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
        accounting.capitalization_receipt_id,
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
            &accounting.capitalization_receipt_id.bytes(),
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
        assert!(retire.contains(".checked_sub(ledger_before.remaining_principal_lamports)"));
        assert!(retire.contains("ledger_before.remaining_principal_lamports"));
        assert!(retire.contains("ledger_before.donation_lamports"));
        assert!(source.contains("!account.is_signer"));
        assert!(source.contains("all_distinct_ids(&terminal_ids)"));
    }
}
