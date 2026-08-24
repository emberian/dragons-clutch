// SPDX-License-Identifier: AGPL-3.0-or-later
//! Reverse-order physical retirement for terminal Market Failure accounts.
//!
//! Product supplies its private whole-Market terminal receipt only after every
//! liability family is closed. This owner then reopens the permanent Failure
//! replay and the writable durable terminal tuple, reconstructs the unique
//! interval seal, closes `0xab/v2` and `0xac/v2`, and finally closes the
//! mutable `0xa0/v3` runtime. Immutable admission `0xa0/v2` remains readable
//! for the later final root disposition; permanent replay `0xa3/v2` is never
//! drained or reassigned.

use crate::accounts::{require, require_distinct, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::failure_market_family_terminal_v2::AuthenticatedFailureMarketFamilyTerminalOwnerV2;
use crate::instructions::failure_market_interval_v2::close_failure_market_interval_accounts_v2;
use crate::instructions::failure_market_replay_v2::authenticate_failure_market_replay_v2;
use crate::instructions::failure_market_runtime::authenticate_failure_market_runtime_root_v1;
use crate::instructions::genesis::SYSTEM_PROGRAM_ID;
use crate::instructions::product_market::AuthenticatedMarketInstanceTerminalV1;
use clutch_failure_policy_runtime::market_runtime_v1::FailureMarketRuntimePhaseV1;
use clutch_product_series::ContentId;
use clutch_solana_layout::product_series::MarketLifecycleRootAccountV1;
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

const TERMINAL_DEPENDENTS_CLOSE_DOMAIN_V2: &[u8] =
    b"dragons-clutch/sbf/failure-market-terminal-dependents-close/v2\0";

/// Exact authenticated close of deletable Failure dependents. The immutable
/// admission root remains live and the permanent replay remains unchanged.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedFailureMarketTerminalDependentsCloseV2 {
    id: ContentId,
    market_terminal_id: ContentId,
    family_terminal_owner_id: ContentId,
    interval_close_id: ContentId,
    runtime_account: Pubkey,
    rent_refund_owner: Pubkey,
    neutral_sink: Pubkey,
    refunded_principal_lamports: u64,
    neutralized_donation_lamports: u64,
}

impl AuthenticatedFailureMarketTerminalDependentsCloseV2 {
    pub(crate) const fn id(self) -> ContentId {
        self.id
    }

    pub(crate) const fn market_terminal_id(self) -> ContentId {
        self.market_terminal_id
    }

    pub(crate) const fn family_terminal_owner_id(self) -> ContentId {
        self.family_terminal_owner_id
    }

    pub(crate) const fn runtime_account(self) -> Pubkey {
        self.runtime_account
    }

    pub(crate) const fn refunded_principal_lamports(self) -> u64 {
        self.refunded_principal_lamports
    }

    pub(crate) const fn neutralized_donation_lamports(self) -> u64 {
        self.neutralized_donation_lamports
    }
}

/// Close the reusable interval accounts before the mutable runtime root.
///
/// Every postbalance is planned before the first account mutation. A refusal
/// during either close rolls the complete instruction back under SVM atomicity.
#[allow(clippy::too_many_arguments)]
pub(crate) fn close_failure_market_terminal_dependents_v2<'a>(
    program_id: &Pubkey,
    market_root_account: &AccountInfo<'a>,
    admission_root_account: &AccountInfo<'a>,
    runtime_root_account: &AccountInfo<'a>,
    interval_cell_account: &AccountInfo<'a>,
    interval_history_account: &AccountInfo<'a>,
    replay_account: &AccountInfo<'a>,
    rent_refund_owner: &AccountInfo<'a>,
    neutral_sink: &AccountInfo<'a>,
    terminal_owner: AuthenticatedFailureMarketFamilyTerminalOwnerV2,
    market_terminal: AuthenticatedMarketInstanceTerminalV1,
    market_root_output: &mut MarketLifecycleRootAccountV1,
) -> Outcome<AuthenticatedFailureMarketTerminalDependentsCloseV2> {
    require_distinct(&[
        market_root_account.clone(),
        admission_root_account.clone(),
        runtime_root_account.clone(),
        interval_cell_account.clone(),
        interval_history_account.clone(),
        replay_account.clone(),
        rent_refund_owner.clone(),
        neutral_sink.clone(),
    ])?;
    for recipient in [rent_refund_owner, neutral_sink] {
        require(
            recipient.is_writable
                && !recipient.is_signer
                && !recipient.executable
                && *recipient.owner == SYSTEM_PROGRAM_ID
                && recipient.data_len() == 0,
            ClutchError::MismatchedState,
        )?;
    }
    let admission = terminal_owner.admission();
    require(
        *admission_root_account.key == admission.account()
            && !admission_root_account.is_writable
            && !market_root_account.is_writable
            && runtime_root_account.is_writable
            && interval_cell_account.is_writable
            && interval_history_account.is_writable
            && !replay_account.is_writable,
        ClutchError::MismatchedState,
    )?;
    let live_runtime = authenticate_failure_market_runtime_root_v1(
        program_id,
        admission_root_account,
        runtime_root_account,
        admission,
        true,
    )?;
    let live_replay = authenticate_failure_market_replay_v2(
        program_id,
        replay_account,
        admission,
        terminal_owner.replay().funding(),
        false,
    )?;
    let interval = terminal_owner.interval();
    let history = interval.history();
    let runtime_funding = live_runtime.state().root_funding();
    require(
        live_runtime == terminal_owner.runtime()
            && live_replay == terminal_owner.replay()
            && *runtime_root_account.key == live_runtime.account()
            && *interval_cell_account.key == interval.cell_account()
            && *interval_history_account.key == interval.history_account()
            && market_terminal.root_account() == *market_root_account.key
            && market_terminal.owner_program() == *program_id
            && market_terminal.market_instance_id()
                == admission.state().binding().facts().market_instance_id
            && market_terminal.generation() == admission.state().binding().facts().generation
            && market_terminal.failure_terminal_receipt_id()
                == terminal_owner.family_terminal_receipt_id()
            && live_runtime.state().phase() == FailureMarketRuntimePhaseV1::FamilyTerminal
            && live_runtime.state().family_terminal_receipt_id()
                == terminal_owner.family_terminal_receipt_id()
            && history.family_terminal_receipt_id().bytes()
                == terminal_owner.family_terminal_receipt_id().bytes()
            && history.rent_refund_owner().bytes() == rent_refund_owner.key.to_bytes()
            && history.neutral_sink().bytes() == neutral_sink.key.to_bytes()
            && runtime_funding.rent_refund_owner.bytes() == rent_refund_owner.key.to_bytes()
            && runtime_funding.neutral_sink.bytes() == neutral_sink.key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let runtime_balance = runtime_root_account.lamports();
    let runtime_donation = runtime_balance
        .checked_sub(runtime_funding.rent_principal_lamports)
        .ok_or(ClutchError::MismatchedState)?;
    let interval_refund = history
        .work_rent_principal_lamports()
        .checked_add(history.history_rent_principal_lamports())
        .ok_or(ClutchError::Arithmetic)?;
    let interval_donation = interval_cell_account
        .lamports()
        .checked_sub(history.work_rent_principal_lamports())
        .and_then(|work| {
            interval_history_account
                .lamports()
                .checked_sub(history.history_rent_principal_lamports())
                .and_then(|history| work.checked_add(history))
        })
        .ok_or(ClutchError::MismatchedState)?;
    let refunded_principal_lamports = interval_refund
        .checked_add(runtime_funding.rent_principal_lamports)
        .ok_or(ClutchError::Arithmetic)?;
    let neutralized_donation_lamports = interval_donation
        .checked_add(runtime_donation)
        .ok_or(ClutchError::Arithmetic)?;
    let refund_after = rent_refund_owner
        .lamports()
        .checked_add(refunded_principal_lamports)
        .ok_or(ClutchError::Arithmetic)?;
    let sink_after = neutral_sink
        .lamports()
        .checked_add(neutralized_donation_lamports)
        .ok_or(ClutchError::Arithmetic)?;
    let seal = terminal_owner.family_seal()?;
    let interval_close = close_failure_market_interval_accounts_v2(
        program_id,
        admission_root_account,
        market_root_account,
        interval_cell_account,
        interval_history_account,
        rent_refund_owner,
        neutral_sink,
        interval,
        seal,
        market_terminal,
        market_root_output,
    )?;
    let runtime_refund_after = rent_refund_owner
        .lamports()
        .checked_add(runtime_funding.rent_principal_lamports)
        .ok_or(ClutchError::Arithmetic)?;
    let runtime_sink_after = neutral_sink
        .lamports()
        .checked_add(runtime_donation)
        .ok_or(ClutchError::Arithmetic)?;
    {
        let mut runtime_lamports = runtime_root_account
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let mut refund_lamports = rent_refund_owner
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let mut sink_lamports = neutral_sink
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        **runtime_lamports = 0;
        **refund_lamports = runtime_refund_after;
        **sink_lamports = runtime_sink_after;
    }
    runtime_root_account
        .resize(0)
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    runtime_root_account.assign(&SYSTEM_PROGRAM_ID);
    require(
        runtime_root_account.lamports() == 0
            && runtime_root_account.data_len() == 0
            && *runtime_root_account.owner == SYSTEM_PROGRAM_ID
            && rent_refund_owner.lamports() == refund_after
            && neutral_sink.lamports() == sink_after,
        ClutchError::MismatchedState,
    )?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            TERMINAL_DEPENDENTS_CLOSE_DOMAIN_V2,
            &terminal_owner.id().bytes(),
            &market_terminal.id().bytes(),
            &interval_close.id().bytes(),
            admission_root_account.key.as_ref(),
            runtime_root_account.key.as_ref(),
            replay_account.key.as_ref(),
            rent_refund_owner.key.as_ref(),
            neutral_sink.key.as_ref(),
            &refunded_principal_lamports.to_le_bytes(),
            &neutralized_donation_lamports.to_le_bytes(),
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(AuthenticatedFailureMarketTerminalDependentsCloseV2 {
        id,
        market_terminal_id: market_terminal.id(),
        family_terminal_owner_id: terminal_owner.id(),
        interval_close_id: interval_close.id(),
        runtime_account: *runtime_root_account.key,
        rent_refund_owner: *rent_refund_owner.key,
        neutral_sink: *neutral_sink.key,
        refunded_principal_lamports,
        neutralized_donation_lamports,
    })
}

#[cfg(test)]
mod adversarial_retirement_tests {
    #[test]
    fn terminal_dependents_plan_before_writes_and_keep_replay_permanent() {
        let source = include_str!("failure_market_retirement_v2.rs");
        let close = source
            .split("fn close_failure_market_terminal_dependents_v2")
            .nth(1)
            .and_then(|value| value.split("#[cfg(test)]").next())
            .expect("terminal dependent close");
        let plan = close.find("let runtime_balance").expect("first projection");
        let first_write = close
            .find("close_failure_market_interval_accounts_v2")
            .expect("first write");
        assert!(plan < first_write);
        for guard in [
            "authenticate_failure_market_runtime_root_v1",
            "authenticate_failure_market_replay_v2",
            "market_terminal.failure_terminal_receipt_id()",
            "terminal_owner.family_seal()?",
            "runtime_root_account.assign(&SYSTEM_PROGRAM_ID)",
        ] {
            assert!(close.contains(guard));
        }
        assert!(!close.contains("replay_account.assign"));
        assert!(!close.contains("admission_root_account.assign"));
    }
}
