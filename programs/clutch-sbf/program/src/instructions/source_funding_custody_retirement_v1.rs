// SPDX-License-Identifier: AGPL-3.0-or-later
//! Private Source-custody retirement under Product's durable terminal owner.
//!
//! Source child creation, paid work, terminal persistence, and physical result
//! close all recycle their unused principal through one release-selected
//! System-owned PDA. This module is the only adapter allowed to drain that PDA.
//! It accepts no instruction payload and no caller-selected recipient or
//! amount: Product must first hostile-reopen its FundingTerms, founder,
//! Source-terminal/result-close, Failure-family, and counted-retirement facts
//! and implement the default-refusing authority below.

use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::{require_system_program, transfer_data, SYSTEM_PROGRAM_ID};
use crate::seeds;
use crate::source_plane_v3::runtime_key;
use crate::source_plane_v3_actions::{
    authenticate_source_funding_custody_v1, AuthenticatedSourceFundingCustodyV1,
};
use clutch_source_plane_v3::ContentId;
use clutch_source_plane_v3_runtime::{
    account_data_id, AuthenticatedSourceRouteV1, RuntimeKey, SourceWorkScheduleBindingV1,
};
use solana_account_info::AccountInfo;
use solana_cpi::invoke_signed;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;
use std::vec;

const SOURCE_FUNDING_CUSTODY_POSTTERMINAL_AUTH_DOMAIN_V1: &[u8] =
    b"dragons-clutch/sbf/source-funding-custody-postterminal-auth/v1";
const SOURCE_FUNDING_CUSTODY_RETIREMENT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/sbf/source-funding-custody-retirement/v1";

/// Product-owned durable accounting supplied only by its private retirement
/// composer. None of these fields are decoded from Source instruction bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourceFundingCustodyRetirementAccountingV1 {
    pub(crate) funding_terms_id: ContentId,
    pub(crate) product_retirement_authority_id: ContentId,
    pub(crate) capitalization_receipt_id: ContentId,
    pub(crate) pre_root_source_occurrence_id: ContentId,
    pub(crate) source_terminal_receipt_id: ContentId,
    pub(crate) source_result_close_receipt_id: ContentId,
    pub(crate) failure_family_terminal_receipt_id: ContentId,
    pub(crate) counted_retirement_receipt_id: ContentId,
    pub(crate) source_funding_custody: RuntimeKey,
    pub(crate) lamport_principal_refund: RuntimeKey,
    pub(crate) neutral_lamport_sink: RuntimeKey,
    pub(crate) allocated_principal_lamports: u64,
    pub(crate) completed_principal_lamports: u64,
}

/// Complete facts equality-checked by Product before Source may move a single
/// lamport. The observed balance and exhaustive split are computed locally.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourceFundingCustodyRetirementFactsV1 {
    pub(crate) accounting: SourceFundingCustodyRetirementAccountingV1,
    pub(crate) source_route_id: ContentId,
    pub(crate) source_work_schedule_id: ContentId,
    pub(crate) source_lifecycle_id: ContentId,
    pub(crate) custody_authentication_id: ContentId,
    pub(crate) custody_account_data_id: ContentId,
    pub(crate) custody_balance_before: u64,
    pub(crate) unused_principal_lamports: u64,
    pub(crate) neutral_donation_and_surplus_lamports: u64,
    pub(crate) principal_refund_balance_before: u64,
    pub(crate) principal_refund_balance_after: u64,
    pub(crate) neutral_sink_balance_before: u64,
    pub(crate) neutral_sink_balance_after: u64,
}

/// Default-refusing Product retirement owner.
///
/// The sole implementation belongs beside Product's hostile 0xad retirement
/// authentication. Matching scalar fields are not authority: the
/// implementation must compare them with its retained FundingTerms, founder,
/// Failure/Source terminal, and counted aggregate postwrites.
pub(crate) trait AuthenticatedSourceFundingCustodyRetirementAuthorityV1 {
    fn authenticate_source_funding_custody_retirement_v1(
        &self,
        _facts: SourceFundingCustodyRetirementFactsV1,
    ) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
}

/// Private postwrite consumed by Product before FundingV2 may close.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedSourceFundingCustodyRetirementV1 {
    id: ContentId,
    product_retirement_authority_id: ContentId,
    facts: SourceFundingCustodyRetirementFactsV1,
    custody_balance_after: u64,
}

impl AuthenticatedSourceFundingCustodyRetirementV1 {
    pub(crate) const fn id(self) -> ContentId {
        self.id
    }

    pub(crate) const fn product_retirement_authority_id(self) -> ContentId {
        self.product_retirement_authority_id
    }

    pub(crate) const fn facts(self) -> SourceFundingCustodyRetirementFactsV1 {
        self.facts
    }

    pub(crate) const fn custody_balance_after(self) -> u64 {
        self.custody_balance_after
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

fn checked_retirement_partition(
    allocated_principal_lamports: u64,
    completed_principal_lamports: u64,
    observed_balance_lamports: u64,
) -> Outcome<(u64, u64)> {
    let unused_principal_lamports = allocated_principal_lamports
        .checked_sub(completed_principal_lamports)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let neutral_donation_and_surplus_lamports = observed_balance_lamports
        .checked_sub(unused_principal_lamports)
        .ok_or(Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))?;
    Ok((
        unused_principal_lamports,
        neutral_donation_and_surplus_lamports,
    ))
}

/// Drain one exact terminal Source custody under Product's private retirement
/// authority. Unused allocated principal returns to the immutable FundingTerms
/// payer; every other observed lamport goes to the release-selected neutral
/// sink. Both destinations are writable non-signers.
#[allow(clippy::too_many_arguments)]
pub(crate) fn retire_source_funding_custody_v1<
    A: AuthenticatedSourceFundingCustodyRetirementAuthorityV1 + ?Sized,
>(
    program_id: &Pubkey,
    authority: &A,
    route: AuthenticatedSourceRouteV1,
    schedule: SourceWorkScheduleBindingV1,
    accounting: SourceFundingCustodyRetirementAccountingV1,
    custody_account: &AccountInfo<'_>,
    principal_refund: &AccountInfo<'_>,
    neutral_sink: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
) -> Outcome<AuthenticatedSourceFundingCustodyRetirementV1> {
    require_system_program(system_program)?;
    let custody = authenticate_source_funding_custody_v1(
        program_id,
        route,
        schedule,
        custody_account,
    )?;
    require_system_destination(principal_refund, accounting.lamport_principal_refund)?;
    require_system_destination(neutral_sink, accounting.neutral_lamport_sink)?;
    require(
        accounting.source_funding_custody == custody.account()
            && accounting.source_funding_custody == runtime_key(custody_account.key)
            && accounting.neutral_lamport_sink == route.neutral_sink()
            && accounting.lamport_principal_refund != accounting.neutral_lamport_sink
            && accounting.lamport_principal_refund != accounting.source_funding_custody
            && accounting.neutral_lamport_sink != accounting.source_funding_custody
            && !accounting.funding_terms_id.is_zero()
            && !accounting.product_retirement_authority_id.is_zero()
            && !accounting.capitalization_receipt_id.is_zero()
            && !accounting.pre_root_source_occurrence_id.is_zero()
            && !accounting.source_terminal_receipt_id.is_zero()
            && !accounting.source_result_close_receipt_id.is_zero()
            && !accounting.failure_family_terminal_receipt_id.is_zero()
            && !accounting.counted_retirement_receipt_id.is_zero(),
        ClutchError::MismatchedState,
    )?;
    let custody_balance_before = custody_account.lamports();
    let (unused_principal_lamports, neutral_donation_and_surplus_lamports) =
        checked_retirement_partition(
            accounting.allocated_principal_lamports,
            accounting.completed_principal_lamports,
            custody_balance_before,
        )?;
    let custody_account_data_id = account_data_id(runtime_key(custody_account.key), &[])
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let custody_authentication_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SOURCE_FUNDING_CUSTODY_POSTTERMINAL_AUTH_DOMAIN_V1,
            program_id.as_ref(),
            &route.route_id().bytes(),
            &schedule.source_work_schedule_id().bytes(),
            &schedule.lifecycle_id().bytes(),
            &custody.id().bytes(),
            custody_account.key.as_ref(),
            custody_account.owner.as_ref(),
            &custody_account_data_id.bytes(),
            &custody_balance_before.to_le_bytes(),
        ])
        .to_bytes(),
    );
    require(
        !custody_authentication_id.is_zero(),
        ClutchError::MismatchedState,
    )?;
    let principal_refund_balance_before = principal_refund.lamports();
    let neutral_sink_balance_before = neutral_sink.lamports();
    let principal_refund_balance_after = principal_refund_balance_before
        .checked_add(unused_principal_lamports)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let neutral_sink_balance_after = neutral_sink_balance_before
        .checked_add(neutral_donation_and_surplus_lamports)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let facts = SourceFundingCustodyRetirementFactsV1 {
        accounting,
        source_route_id: route.route_id(),
        source_work_schedule_id: schedule.source_work_schedule_id(),
        source_lifecycle_id: schedule.lifecycle_id(),
        custody_authentication_id,
        custody_account_data_id,
        custody_balance_before,
        unused_principal_lamports,
        neutral_donation_and_surplus_lamports,
        principal_refund_balance_before,
        principal_refund_balance_after,
        neutral_sink_balance_before,
        neutral_sink_balance_after,
    };
    let product_retirement_authority_id =
        authority.authenticate_source_funding_custody_retirement_v1(facts)?;
    require(
        product_retirement_authority_id == accounting.product_retirement_authority_id,
        ClutchError::AuthorizationUnavailable,
    )?;
    let lifecycle = schedule.lifecycle_id().bytes();
    let (_, custody_bump) = seeds::source_funding_custody_pda(program_id, &lifecycle);
    let bump = [custody_bump];
    let signer_seeds: &[&[u8]] = &[
        seeds::SEED_SOURCE_FUNDING_CUSTODY_V1,
        &lifecycle,
        &bump,
    ];
    if unused_principal_lamports != 0 {
        let refund = Instruction::new_with_bytes(
            SYSTEM_PROGRAM_ID,
            &transfer_data(unused_principal_lamports),
            vec![
                AccountMeta::new(*custody_account.key, true),
                AccountMeta::new(*principal_refund.key, false),
            ],
        );
        invoke_signed(
            &refund,
            &[
                custody_account.clone(),
                principal_refund.clone(),
                system_program.clone(),
            ],
            &[signer_seeds],
        )
        .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))?;
    }
    if neutral_donation_and_surplus_lamports != 0 {
        let neutral = Instruction::new_with_bytes(
            SYSTEM_PROGRAM_ID,
            &transfer_data(neutral_donation_and_surplus_lamports),
            vec![
                AccountMeta::new(*custody_account.key, true),
                AccountMeta::new(*neutral_sink.key, false),
            ],
        );
        invoke_signed(
            &neutral,
            &[
                custody_account.clone(),
                neutral_sink.clone(),
                system_program.clone(),
            ],
            &[signer_seeds],
        )
        .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))?;
    }
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
            SOURCE_FUNDING_CUSTODY_RETIREMENT_DOMAIN_V1,
            &product_retirement_authority_id.bytes(),
            &accounting.funding_terms_id.bytes(),
            &accounting.capitalization_receipt_id.bytes(),
            &accounting.pre_root_source_occurrence_id.bytes(),
            &accounting.source_terminal_receipt_id.bytes(),
            &accounting.source_result_close_receipt_id.bytes(),
            &accounting.failure_family_terminal_receipt_id.bytes(),
            &accounting.counted_retirement_receipt_id.bytes(),
            &facts.source_route_id.bytes(),
            &facts.source_work_schedule_id.bytes(),
            &facts.source_lifecycle_id.bytes(),
            &custody_authentication_id.bytes(),
            &custody_account_data_id.bytes(),
            custody_account.key.as_ref(),
            principal_refund.key.as_ref(),
            neutral_sink.key.as_ref(),
            &accounting.allocated_principal_lamports.to_le_bytes(),
            &accounting.completed_principal_lamports.to_le_bytes(),
            &unused_principal_lamports.to_le_bytes(),
            &neutral_donation_and_surplus_lamports.to_le_bytes(),
            &custody_balance_before.to_le_bytes(),
            &principal_refund_balance_before.to_le_bytes(),
            &principal_refund_balance_after.to_le_bytes(),
            &neutral_sink_balance_before.to_le_bytes(),
            &neutral_sink_balance_after.to_le_bytes(),
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(AuthenticatedSourceFundingCustodyRetirementV1 {
        id,
        product_retirement_authority_id,
        facts,
        custody_balance_after: 0,
    })
}

#[cfg(test)]
mod adversarial_tests {
    use super::*;

    #[test]
    fn completed_principal_cannot_exceed_allocation() {
        assert!(checked_retirement_partition(9, 10, 0).is_err());
    }

    #[test]
    fn observed_balance_cannot_undercollateralize_unused_principal() {
        assert!(checked_retirement_partition(10, 3, 6).is_err());
    }

    #[test]
    fn every_lamport_is_partitioned_once() {
        assert_eq!(checked_retirement_partition(10, 3, 12), Ok((7, 5)));
    }

    #[test]
    fn retirement_has_no_instruction_payload_or_signing_recipient() {
        let source = include_str!("source_funding_custody_retirement_v1.rs");
        let retire = source
            .split("pub(crate) fn retire_source_funding_custody_v1")
            .nth(1)
            .expect("private retirement adapter");
        assert!(!retire.contains("payload"));
        assert!(source.contains("!account.is_signer"));
        assert!(retire.contains("authority.authenticate_source_funding_custody_retirement_v1"));
        assert!(retire.contains("custody_account.lamports() == 0"));
    }
}
