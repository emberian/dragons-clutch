// SPDX-License-Identifier: AGPL-3.0-or-later
//! Capability-disabled SBF seam for reusable Failure interval accounts.
//!
//! The Failure runtime owns every semantic byte in `0xab/v2` and `0xac/v2`.
//! `clutch-solana-layout` owns only their four-byte physical frames. This
//! module authenticates owner, fresh successor PDA, exact frame/body, present
//! principal, and stale preimages before writing a pure private-field plan.
//! It deliberately exposes no instruction route. Initialization is available
//! only through a crate-private Product adapter that joins the two accepted
//! retained-slot preallocation receipts for slots 8 and 9.

use crate::accounts::{expect_pda, require, require_distinct, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::failure_market_admission::{
    authenticate_failure_market_root_v2, AuthenticatedFailureMarketRootV2,
};
use crate::instructions::genesis::{
    allocate_data, assign_data, read_rent, require_system_program, SYSTEM_PROGRAM_ID,
};
use crate::instructions::product_market::{
    authenticate_market_instance_terminal_v1, AuthenticatedMarketFoundationPreallocationV2,
    AuthenticatedMarketInstanceTerminalV1,
};
use crate::seeds;
use clutch_failure_policy_runtime::market_interval_cell_v2::{
    initialize_failure_market_interval_cell_v2, FailureMarketIntervalCellDispositionV2,
    FailureMarketIntervalCellExhaustionPlanV2, FailureMarketIntervalCellPhaseV2,
    FailureMarketIntervalCellPlanV2, FailureMarketIntervalCellResetReceiptV2,
    FailureMarketIntervalCellStateIdV2, FailureMarketIntervalCellV2,
    FAILURE_MARKET_INTERVAL_CELL_BYTES_V2,
};
use clutch_failure_policy_runtime::market_interval_history_v2::{
    admit_failure_market_interval_history_v2, plan_close_failure_market_interval_accounts_v2,
    AuthenticatedFailureMarketIntervalFundingV2, FailureMarketIntervalCloseAuthorizationIdV2,
    FailureMarketIntervalFamilySealReceiptV2, FailureMarketIntervalFundingFactsV2,
    FailureMarketIntervalFundingReceiptV2, FailureMarketIntervalHistoryAppendReceiptV2,
    FailureMarketIntervalHistoryPlanV2, FailureMarketIntervalHistoryStateIdV2,
    FailureMarketIntervalHistoryV2, FAILURE_MARKET_INTERVAL_HISTORY_BYTES_V2,
};
use clutch_failure_policy_runtime::market_policy_v1::{
    FailureMarketAccountIdV1, FailureMarketAdmissionStateIdV1,
};
use clutch_failure_policy_runtime::market_quote_v1::FailureMarketRecoveryQuoteAdmissionReceiptV1;
use clutch_product_series::{ContentId as ProductContentId, MarketFoundationSlotV2};
use clutch_solana_layout::failure_market_interval_v2::{
    FailureMarketIntervalCellAccountV2, FailureMarketIntervalHistoryAccountV2,
    FAILURE_MARKET_INTERVAL_CELL_BODY_BYTES_V2, FAILURE_MARKET_INTERVAL_HISTORY_BODY_BYTES_V2,
};
use clutch_solana_layout::product_series::MarketLifecycleRootAccountV1;
use clutch_solana_layout::registry::{
    FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_BYTES, FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_BYTES,
};
use solana_account_info::AccountInfo;
use solana_cpi::invoke_signed;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

const CELL_AUTHENTICATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/failure-market-interval-cell-account-authentication/v2";
const HISTORY_AUTHENTICATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/failure-market-interval-history-account-authentication/v2";
const CLOSE_AUTHENTICATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/failure-market-interval-physical-close/v2";

const _: () =
    assert!(FAILURE_MARKET_INTERVAL_CELL_BODY_BYTES_V2 == FAILURE_MARKET_INTERVAL_CELL_BYTES_V2);
const _: () = assert!(
    FAILURE_MARKET_INTERVAL_HISTORY_BODY_BYTES_V2 == FAILURE_MARKET_INTERVAL_HISTORY_BYTES_V2
);

/// Exact authenticated reusable-cell and append-only-history pair.
///
/// Private fields prevent an instruction module from lowering caller IDs into
/// account authority. The pure funding and quote receipts used to decode the
/// bodies are retained for same-instruction postwrite reauthentication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedFailureMarketIntervalAccountsV2 {
    cell_account: Pubkey,
    cell_bump: u8,
    cell: FailureMarketIntervalCellV2,
    cell_state_id: FailureMarketIntervalCellStateIdV2,
    cell_data_id: ProductContentId,
    cell_authentication_id: ProductContentId,
    cell_observed_lamports: u64,
    history_account: Pubkey,
    history_bump: u8,
    history: FailureMarketIntervalHistoryV2,
    history_state_id: FailureMarketIntervalHistoryStateIdV2,
    history_data_id: ProductContentId,
    history_authentication_id: ProductContentId,
    history_observed_lamports: u64,
    admission_root_account: Pubkey,
    admission_state_id: FailureMarketAdmissionStateIdV1,
    funding: FailureMarketIntervalFundingReceiptV2,
    quote: FailureMarketRecoveryQuoteAdmissionReceiptV1,
}

impl AuthenticatedFailureMarketIntervalAccountsV2 {
    /// Exact reusable cell account.
    pub(crate) const fn cell_account(self) -> Pubkey {
        self.cell_account
    }

    /// Complete authenticated reusable-cell state.
    pub(crate) const fn cell(self) -> FailureMarketIntervalCellV2 {
        self.cell
    }

    /// Complete reusable-cell semantic commitment.
    pub(crate) const fn cell_state_id(self) -> FailureMarketIntervalCellStateIdV2 {
        self.cell_state_id
    }

    /// Owner/PDA/frame/body/balance authentication for the cell preimage.
    pub(crate) const fn cell_authentication_id(self) -> ProductContentId {
        self.cell_authentication_id
    }

    /// Exact append-only history account.
    pub(crate) const fn history_account(self) -> Pubkey {
        self.history_account
    }

    /// Complete authenticated append-only history.
    pub(crate) const fn history(self) -> FailureMarketIntervalHistoryV2 {
        self.history
    }

    /// Complete append-only-history semantic commitment.
    pub(crate) const fn history_state_id(self) -> FailureMarketIntervalHistoryStateIdV2 {
        self.history_state_id
    }

    /// Owner/PDA/frame/body/balance authentication for the history preimage.
    pub(crate) const fn history_authentication_id(self) -> ProductContentId {
        self.history_authentication_id
    }

    /// Exact immutable Failure admission account used for both body joins.
    pub(crate) const fn admission_root_account(self) -> Pubkey {
        self.admission_root_account
    }

    /// Complete immutable Failure admission state used for hostile decoding.
    pub(crate) const fn admission_state_id(self) -> FailureMarketAdmissionStateIdV1 {
        self.admission_state_id
    }

    /// Product-authenticated reusable-account capitalization.
    pub(crate) const fn funding(self) -> FailureMarketIntervalFundingReceiptV2 {
        self.funding
    }

    /// Market-scoped liveness quote admission used for hostile decoding.
    pub(crate) const fn quote(self) -> FailureMarketRecoveryQuoteAdmissionReceiptV1 {
        self.quote
    }
}

/// Atomic postimage of Product-authorized slot-8/slot-9 allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FailureMarketIntervalPostimageV2 {
    accounts: AuthenticatedFailureMarketIntervalAccountsV2,
    funding: FailureMarketIntervalFundingReceiptV2,
}

impl FailureMarketIntervalPostimageV2 {
    /// Newly persisted canonical Idle cell and empty append-only history.
    pub(crate) const fn accounts(self) -> AuthenticatedFailureMarketIntervalAccountsV2 {
        self.accounts
    }

    /// Exact paired Product-preallocation funding receipt.
    pub(crate) const fn funding(self) -> FailureMarketIntervalFundingReceiptV2 {
        self.funding
    }
}

/// Module-private bridge over Product's paired retained-slot receipts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProductFailureMarketIntervalFundingV2 {
    expected: FailureMarketIntervalFundingFactsV2,
}

impl AuthenticatedFailureMarketIntervalFundingV2 for ProductFailureMarketIntervalFundingV2 {
    fn authenticate_failure_market_interval_funding(
        &self,
        expected: FailureMarketIntervalFundingFactsV2,
    ) -> clutch_failure_policy_runtime::Result<()> {
        if self.expected != expected {
            return Err(clutch_failure_policy_runtime::Error::BindingMismatch);
        }
        Ok(())
    }
}

/// Exact authenticated physical close of the sealed reusable interval pair.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedFailureMarketIntervalCloseV2 {
    id: ProductContentId,
    close_authorization_id: FailureMarketIntervalCloseAuthorizationIdV2,
    market_terminal_authentication_id: ProductContentId,
    cell_account: Pubkey,
    history_account: Pubkey,
    rent_refund_owner: Pubkey,
    neutral_sink: Pubkey,
    refunded_principal_lamports: u64,
    neutralized_donation_lamports: u64,
}

impl AuthenticatedFailureMarketIntervalCloseV2 {
    /// Complete Product/Failure/physical close authentication.
    pub(crate) const fn id(self) -> ProductContentId {
        self.id
    }

    /// Pure Failure close authorization consumed by the writer.
    pub(crate) const fn close_authorization_id(
        self,
    ) -> FailureMarketIntervalCloseAuthorizationIdV2 {
        self.close_authorization_id
    }

    /// Live Product terminal-account authentication consumed by this close.
    pub(crate) const fn market_terminal_authentication_id(self) -> ProductContentId {
        self.market_terminal_authentication_id
    }

    /// Deleted reusable-cell account.
    pub(crate) const fn cell_account(self) -> Pubkey {
        self.cell_account
    }

    /// Deleted append-only history account.
    pub(crate) const fn history_account(self) -> Pubkey {
        self.history_account
    }

    /// Immutable principal recipient.
    pub(crate) const fn rent_refund_owner(self) -> Pubkey {
        self.rent_refund_owner
    }

    /// Immutable unsolicited-lamport sink.
    pub(crate) const fn neutral_sink(self) -> Pubkey {
        self.neutral_sink
    }

    /// Exact principal returned to the immutable refund owner.
    pub(crate) const fn refunded_principal_lamports(self) -> u64 {
        self.refunded_principal_lamports
    }

    /// Exact unsolicited surplus sent only to the immutable neutral sink.
    pub(crate) const fn neutralized_donation_lamports(self) -> u64 {
        self.neutralized_donation_lamports
    }
}

/// Allocate and write exact Product-prepaid slot-8/slot-9 successors.
///
/// This helper is crate-private and non-routable. The authority must be a
/// private adapter joining both Product preallocation receipts, their common
/// root/schedule/graph/transcript, live Rent, and the exact current zero-data
/// balances. No caller-built amount or generic funding ID is sufficient.
#[allow(clippy::too_many_arguments)]
pub(crate) fn initialize_failure_market_interval_accounts_v2<'a>(
    program_id: &Pubkey,
    admission_root_account: &AccountInfo<'a>,
    cell_account: &AccountInfo<'a>,
    history_account: &AccountInfo<'a>,
    rent_sysvar: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    admission: AuthenticatedFailureMarketRootV2,
    quote: FailureMarketRecoveryQuoteAdmissionReceiptV1,
    work_preallocation: AuthenticatedMarketFoundationPreallocationV2,
    history_preallocation: AuthenticatedMarketFoundationPreallocationV2,
) -> Outcome<FailureMarketIntervalPostimageV2> {
    require_system_program(system_program)?;
    require_distinct(&[
        admission_root_account.clone(),
        cell_account.clone(),
        history_account.clone(),
        rent_sysvar.clone(),
        system_program.clone(),
    ])?;
    let live_admission =
        authenticate_failure_market_root_v2(program_id, admission_root_account, false)?;
    require(live_admission == admission, ClutchError::MismatchedState)?;
    let admission = live_admission;
    let admission_state = admission.state();
    let policy = admission_state.binding().facts();
    require(
        work_preallocation.slot() == MarketFoundationSlotV2::FailureIntervalWork
            && history_preallocation.slot() == MarketFoundationSlotV2::FailureIntervalHistory
            && work_preallocation.id() != history_preallocation.id()
            && work_preallocation.root_account() == history_preallocation.root_account()
            && work_preallocation.root_authentication_id()
                == history_preallocation.root_authentication_id()
            && work_preallocation.market_instance_id() == policy.market_instance_id
            && history_preallocation.market_instance_id() == policy.market_instance_id
            && work_preallocation.generation() == policy.generation
            && history_preallocation.generation() == policy.generation
            && work_preallocation.account() == *cell_account.key
            && history_preallocation.account() == *history_account.key
            && work_preallocation.foundation_schedule_id()
                == history_preallocation.foundation_schedule_id()
            && work_preallocation.foundation_account_graph_id()
                == history_preallocation.foundation_account_graph_id()
            && work_preallocation.foundation_transcript_id()
                == history_preallocation.foundation_transcript_id()
            && work_preallocation.rent_refund_owner() == history_preallocation.rent_refund_owner()
            && work_preallocation.neutral_lamport_sink()
                == history_preallocation.neutral_lamport_sink()
            && work_preallocation.root_account() != admission.account()
            && work_preallocation.root_account() != *cell_account.key
            && work_preallocation.root_account() != *history_account.key,
        ClutchError::MismatchedState,
    )?;
    let funding_facts = FailureMarketIntervalFundingFactsV2 {
        failure_policy_binding_id: admission_state.binding().id(),
        market_instance_id: policy.market_instance_id,
        generation: policy.generation,
        work_preallocation_receipt_id: work_preallocation.id(),
        history_preallocation_receipt_id: history_preallocation.id(),
        foundation_schedule_id: work_preallocation.foundation_schedule_id(),
        foundation_account_graph_id: work_preallocation.foundation_account_graph_id(),
        foundation_transcript_id: work_preallocation.foundation_transcript_id(),
        work_account: FailureMarketAccountIdV1::from_bytes(cell_account.key.to_bytes()),
        history_account: FailureMarketAccountIdV1::from_bytes(history_account.key.to_bytes()),
        rent_refund_owner: FailureMarketAccountIdV1::from_bytes(
            work_preallocation.rent_refund_owner().to_bytes(),
        ),
        neutral_sink: FailureMarketAccountIdV1::from_bytes(
            work_preallocation.neutral_lamport_sink().to_bytes(),
        ),
        work_rent_principal_lamports: work_preallocation.principal_lamports(),
        history_rent_principal_lamports: history_preallocation.principal_lamports(),
        work_donation_floor_lamports: work_preallocation.donation_lamports(),
        work_observed_balance_lamports: work_preallocation.observed_balance_lamports(),
        history_donation_floor_lamports: history_preallocation.donation_lamports(),
        history_observed_balance_lamports: history_preallocation.observed_balance_lamports(),
    };
    let product_preallocation_authority = ProductFailureMarketIntervalFundingV2 {
        expected: funding_facts,
    };
    let work_balance = funding_facts
        .work_rent_principal_lamports
        .checked_add(funding_facts.work_donation_floor_lamports)
        .ok_or(ClutchError::Arithmetic)?;
    let history_balance = funding_facts
        .history_rent_principal_lamports
        .checked_add(funding_facts.history_donation_floor_lamports)
        .ok_or(ClutchError::Arithmetic)?;
    let rent = read_rent(rent_sysvar)?;
    require(
        funding_facts.work_account.bytes() == cell_account.key.to_bytes()
            && funding_facts.history_account.bytes() == history_account.key.to_bytes()
            && funding_facts.failure_policy_binding_id == admission_state.binding().id()
            && funding_facts.market_instance_id == policy.market_instance_id
            && funding_facts.generation == policy.generation
            && funding_facts.work_observed_balance_lamports == work_balance
            && funding_facts.history_observed_balance_lamports == history_balance
            && funding_facts.work_rent_principal_lamports
                == rent.minimum_balance(FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_BYTES)?
            && funding_facts.history_rent_principal_lamports
                == rent.minimum_balance(FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_BYTES)?,
        ClutchError::MismatchedState,
    )?;
    for (account, expected_balance) in [
        (cell_account, work_balance),
        (history_account, history_balance),
    ] {
        require(
            account.owner.to_bytes() == SYSTEM_PROGRAM_ID
                && account.is_writable
                && !account.is_signer
                && !account.executable
                && account.data_len() == 0
                && account.lamports() == expected_balance,
            ClutchError::MismatchedState,
        )?;
    }
    let (expected_cell, cell_bump) = seeds::failure_market_interval_cell_v2_pda(
        program_id,
        &policy.market_instance_id.bytes(),
        policy.generation,
    );
    let (expected_history, history_bump) = seeds::failure_market_interval_history_v2_pda(
        program_id,
        &policy.market_instance_id.bytes(),
        policy.generation,
    );
    expect_pda(cell_account.key, (expected_cell, cell_bump), None)?;
    expect_pda(history_account.key, (expected_history, history_bump), None)?;
    let (history, funding) = admit_failure_market_interval_history_v2(
        &product_preallocation_authority,
        admission_state,
        quote,
        funding_facts,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let cell = initialize_failure_market_interval_cell_v2(admission_state, funding, history, quote)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    allocate_assign_interval_account_v2(
        program_id,
        cell_account,
        system_program,
        seeds::SEED_FAILURE_MARKET_INTERVAL_CELL_V2,
        policy.market_instance_id.bytes(),
        policy.generation,
        cell_bump,
        FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_BYTES,
    )?;
    allocate_assign_interval_account_v2(
        program_id,
        history_account,
        system_program,
        seeds::SEED_FAILURE_MARKET_INTERVAL_HISTORY_V2,
        policy.market_instance_id.bytes(),
        policy.generation,
        history_bump,
        FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_BYTES,
    )?;
    let cell_output = encode_cell(cell_bump, cell)?;
    let history_output = encode_history(history_bump, history)?;
    {
        let mut cell_data = cell_account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let mut history_data = history_account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        require(
            cell_data.iter().all(|byte| *byte == 0) && history_data.iter().all(|byte| *byte == 0),
            ClutchError::AlreadyInitialized,
        )?;
        let cell_destination: &mut [u8; FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_BYTES] = cell_data
            .as_mut()
            .try_into()
            .map_err(|_| Refusal::Adapter(ClutchError::WrongDataLength))?;
        let history_destination: &mut [u8; FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_BYTES] =
            history_data
                .as_mut()
                .try_into()
                .map_err(|_| Refusal::Adapter(ClutchError::WrongDataLength))?;
        cell_destination.copy_from_slice(&cell_output);
        history_destination.copy_from_slice(&history_output);
    }
    require(
        cell_account.lamports() == work_balance && history_account.lamports() == history_balance,
        ClutchError::MismatchedState,
    )?;
    let accounts = authenticate_failure_market_interval_accounts_v2(
        program_id,
        cell_account,
        history_account,
        admission,
        funding,
        quote,
        true,
        true,
    )?;
    require(
        accounts.cell == cell && accounts.history == history,
        ClutchError::MismatchedState,
    )?;
    Ok(FailureMarketIntervalPostimageV2 { accounts, funding })
}

/// Authenticate exact existing `0xab/v2` and `0xac/v2` accounts.
///
/// The funding receipt must have been minted from Product's private account
/// graph/foundation authority. Fresh v2 PDA domains keep withdrawn one-shot v1
/// accounts from aliasing these reusable successors.
#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticate_failure_market_interval_accounts_v2<'a>(
    program_id: &Pubkey,
    cell_account: &AccountInfo<'a>,
    history_account: &AccountInfo<'a>,
    admission: AuthenticatedFailureMarketRootV2,
    funding: FailureMarketIntervalFundingReceiptV2,
    quote: FailureMarketRecoveryQuoteAdmissionReceiptV1,
    cell_writable: bool,
    history_writable: bool,
) -> Outcome<AuthenticatedFailureMarketIntervalAccountsV2> {
    require(
        *cell_account.key != *history_account.key
            && *cell_account.key != admission.account()
            && *history_account.key != admission.account(),
        ClutchError::AccountAlias,
    )?;
    authenticate_account_metadata(
        program_id,
        cell_account,
        FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_BYTES,
        cell_writable,
    )?;
    authenticate_account_metadata(
        program_id,
        history_account,
        FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_BYTES,
        history_writable,
    )?;

    let facts = funding.facts();
    let policy = admission.state().binding().facts();
    require(
        facts.work_account.bytes() == cell_account.key.to_bytes()
            && facts.history_account.bytes() == history_account.key.to_bytes()
            && facts.failure_policy_binding_id == admission.state().binding().id()
            && facts.market_instance_id == policy.market_instance_id
            && facts.generation == policy.generation
            && cell_account.lamports() >= facts.work_observed_balance_lamports
            && history_account.lamports() >= facts.history_observed_balance_lamports,
        ClutchError::MismatchedState,
    )?;

    let history_data = history_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let history_input: &[u8; FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_BYTES] = history_data
        .as_ref()
        .try_into()
        .map_err(|_| Refusal::Adapter(ClutchError::WrongDataLength))?;
    let history_frame = FailureMarketIntervalHistoryAccountV2::decode(history_input)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let history = FailureMarketIntervalHistoryV2::decode_for_admission(
        history_frame.semantic_body(),
        admission.state(),
        quote,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let history_data_id = framed_data_id(history_data.as_ref());
    drop(history_data);

    let cell_data = cell_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let cell_input: &[u8; FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_BYTES] = cell_data
        .as_ref()
        .try_into()
        .map_err(|_| Refusal::Adapter(ClutchError::WrongDataLength))?;
    let cell_frame = FailureMarketIntervalCellAccountV2::decode(cell_input)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let cell = FailureMarketIntervalCellV2::decode_for_admission(
        cell_frame.semantic_body(),
        admission.state(),
        funding,
        history,
        quote,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let cell_data_id = framed_data_id(cell_data.as_ref());
    drop(cell_data);

    expect_pda(
        cell_account.key,
        seeds::failure_market_interval_cell_v2_pda(
            program_id,
            &policy.market_instance_id.bytes(),
            policy.generation,
        ),
        Some(cell_frame.bump()),
    )?;
    expect_pda(
        history_account.key,
        seeds::failure_market_interval_history_v2_pda(
            program_id,
            &policy.market_instance_id.bytes(),
            policy.generation,
        ),
        Some(history_frame.bump()),
    )?;

    let cell_state_id = cell
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let history_state_id = history
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let admission_state_id = admission
        .state()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let cell_authentication_id = account_authentication_id(
        CELL_AUTHENTICATION_DOMAIN_V2,
        cell_account,
        cell_data_id,
        cell_state_id.bytes(),
        admission_state_id.bytes(),
    );
    let history_authentication_id = account_authentication_id(
        HISTORY_AUTHENTICATION_DOMAIN_V2,
        history_account,
        history_data_id,
        history_state_id.bytes(),
        admission_state_id.bytes(),
    );
    require_live_data_id(cell_authentication_id)?;
    require_live_data_id(history_authentication_id)?;
    Ok(AuthenticatedFailureMarketIntervalAccountsV2 {
        cell_account: *cell_account.key,
        cell_bump: cell_frame.bump(),
        cell,
        cell_state_id,
        cell_data_id,
        cell_authentication_id,
        cell_observed_lamports: cell_account.lamports(),
        history_account: *history_account.key,
        history_bump: history_frame.bump(),
        history,
        history_state_id,
        history_data_id,
        history_authentication_id,
        history_observed_lamports: history_account.lamports(),
        admission_root_account: admission.account(),
        admission_state_id,
        funding,
        quote,
    })
}

/// Persist one private-field nonterminal cell transition over the exact
/// authenticated preimage. Only begin and paid advance can use this writer;
/// terminal transitions require their narrower typed writers below.
pub(crate) fn write_failure_market_interval_cell_plan_v2(
    program_id: &Pubkey,
    cell_account: &AccountInfo<'_>,
    history_account: &AccountInfo<'_>,
    authenticated: AuthenticatedFailureMarketIntervalAccountsV2,
    plan: FailureMarketIntervalCellPlanV2,
) -> Outcome<AuthenticatedFailureMarketIntervalAccountsV2> {
    write_failure_market_interval_cell_plan_inner_v2(
        program_id,
        cell_account,
        history_account,
        authenticated,
        plan,
        None,
    )
}

/// Persist only the deterministic finite-exhaustion terminal.
///
/// The resolved-payout terminal intentionally has no corresponding writer:
/// it remains unavailable until Product atomically persists Resolution V5 and
/// returns its private once-only activation receipt.
pub(crate) fn write_failure_market_interval_exhaustion_plan_v2(
    program_id: &Pubkey,
    cell_account: &AccountInfo<'_>,
    history_account: &AccountInfo<'_>,
    authenticated: AuthenticatedFailureMarketIntervalAccountsV2,
    exhaustion: FailureMarketIntervalCellExhaustionPlanV2,
) -> Outcome<AuthenticatedFailureMarketIntervalAccountsV2> {
    let receipt = exhaustion.receipt();
    let facts = receipt.facts();
    let resulting_cell = exhaustion.resulting_cell();
    require(
        receipt.failure_policy_binding_id() == authenticated.cell.failure_policy_binding_id()
            && facts.cell_before == authenticated.cell_state_id
            && facts.cell_after
                == resulting_cell
                    .id()
                    .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?
            && facts.market_instance_id == authenticated.cell.market_instance_id()
            && facts.generation == authenticated.cell.generation()
            && resulting_cell.disposition() == FailureMarketIntervalCellDispositionV2::Exhausted,
        ClutchError::MismatchedState,
    )?;
    write_failure_market_interval_cell_plan_inner_v2(
        program_id,
        cell_account,
        history_account,
        authenticated,
        exhaustion.cell_plan(),
        Some(FailureMarketIntervalCellDispositionV2::Exhausted),
    )
}

fn write_failure_market_interval_cell_plan_inner_v2(
    program_id: &Pubkey,
    cell_account: &AccountInfo<'_>,
    history_account: &AccountInfo<'_>,
    authenticated: AuthenticatedFailureMarketIntervalAccountsV2,
    plan: FailureMarketIntervalCellPlanV2,
    admitted_terminal: Option<FailureMarketIntervalCellDispositionV2>,
) -> Outcome<AuthenticatedFailureMarketIntervalAccountsV2> {
    authenticate_unchanged_account_prestate(
        program_id,
        history_account,
        authenticated.history_account,
        authenticated.history_data_id,
        authenticated.history_observed_lamports,
        FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_BYTES,
        false,
    )?;
    let mut next = authenticated.cell;
    next.commit_plan(plan)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let admitted = match admitted_terminal {
        None => matches!(
            (authenticated.cell.phase(), next.phase()),
            (
                FailureMarketIntervalCellPhaseV2::Idle,
                FailureMarketIntervalCellPhaseV2::Active
            ) | (
                FailureMarketIntervalCellPhaseV2::Active,
                FailureMarketIntervalCellPhaseV2::Active
            )
        ),
        Some(disposition) => {
            authenticated.cell.phase() == FailureMarketIntervalCellPhaseV2::Active
                && next.phase() == FailureMarketIntervalCellPhaseV2::Resolved
                && next.disposition() == disposition
        }
    };
    require(admitted, ClutchError::MismatchedState)?;
    let encoded = encode_cell(authenticated.cell_bump, next)?;
    authenticate_write_prestate(
        program_id,
        cell_account,
        authenticated.cell_account,
        authenticated.cell_data_id,
        authenticated.cell_observed_lamports,
        &encoded,
    )?;
    let mut data = cell_account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    data.copy_from_slice(&encoded);
    drop(data);
    let cell_data_id = framed_data_id(&encoded);
    let cell_state_id = next
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let cell_authentication_id = account_authentication_id(
        CELL_AUTHENTICATION_DOMAIN_V2,
        cell_account,
        cell_data_id,
        cell_state_id.bytes(),
        authenticated.admission_state_id.bytes(),
    );
    require_live_data_id(cell_authentication_id)?;
    Ok(AuthenticatedFailureMarketIntervalAccountsV2 {
        cell: next,
        cell_state_id,
        cell_data_id,
        cell_authentication_id,
        ..authenticated
    })
}

/// Atomically fold one exact terminal into `0xac/v2` and reset `0xab/v2` to
/// canonical Idle. Both complete postimages are derived before either borrow
/// is mutated, and the append/reset receipts are cross-checked explicitly.
#[allow(clippy::too_many_arguments)]
pub(crate) fn write_failure_market_interval_archive_v2<'a>(
    program_id: &Pubkey,
    cell_account: &AccountInfo<'a>,
    history_account: &AccountInfo<'a>,
    authenticated: AuthenticatedFailureMarketIntervalAccountsV2,
    history_plan: FailureMarketIntervalHistoryPlanV2,
    append: FailureMarketIntervalHistoryAppendReceiptV2,
    cell_plan: FailureMarketIntervalCellPlanV2,
    reset: FailureMarketIntervalCellResetReceiptV2,
) -> Outcome<AuthenticatedFailureMarketIntervalAccountsV2> {
    let mut next_history = authenticated.history;
    next_history
        .commit_plan(history_plan)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let mut next_cell = authenticated.cell;
    next_cell
        .commit_plan(cell_plan)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let next_history_id = next_history
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let next_cell_id = next_cell
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    require(
        append.history_before() == authenticated.history_state_id
            && append.history_after() == next_history_id
            && reset.terminal_cell() == authenticated.cell_state_id
            && reset.idle_cell() == next_cell_id
            && reset.append_receipt_id() == append.id(),
        ClutchError::MismatchedState,
    )?;
    let encoded_history = encode_history(authenticated.history_bump, next_history)?;
    let encoded_cell = encode_cell(authenticated.cell_bump, next_cell)?;
    authenticate_write_prestate(
        program_id,
        history_account,
        authenticated.history_account,
        authenticated.history_data_id,
        authenticated.history_observed_lamports,
        &encoded_history,
    )?;
    authenticate_write_prestate(
        program_id,
        cell_account,
        authenticated.cell_account,
        authenticated.cell_data_id,
        authenticated.cell_observed_lamports,
        &encoded_cell,
    )?;
    let mut history_data = history_account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let mut cell_data = cell_account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    history_data.copy_from_slice(&encoded_history);
    cell_data.copy_from_slice(&encoded_cell);
    drop(cell_data);
    drop(history_data);
    let cell_data_id = framed_data_id(&encoded_cell);
    let history_data_id = framed_data_id(&encoded_history);
    let cell_authentication_id = account_authentication_id(
        CELL_AUTHENTICATION_DOMAIN_V2,
        cell_account,
        cell_data_id,
        next_cell_id.bytes(),
        authenticated.admission_state_id.bytes(),
    );
    let history_authentication_id = account_authentication_id(
        HISTORY_AUTHENTICATION_DOMAIN_V2,
        history_account,
        history_data_id,
        next_history_id.bytes(),
        authenticated.admission_state_id.bytes(),
    );
    require_live_data_id(cell_authentication_id)?;
    require_live_data_id(history_authentication_id)?;
    Ok(AuthenticatedFailureMarketIntervalAccountsV2 {
        cell: next_cell,
        cell_state_id: next_cell_id,
        cell_data_id,
        cell_authentication_id,
        history: next_history,
        history_state_id: next_history_id,
        history_data_id,
        history_authentication_id,
        ..authenticated
    })
}

/// Persist the exhaustive family seal. Session appends cannot use this
/// history-only writer and must use the paired archive writer above.
pub(crate) fn write_failure_market_interval_family_seal_v2(
    program_id: &Pubkey,
    cell_account: &AccountInfo<'_>,
    history_account: &AccountInfo<'_>,
    authenticated: AuthenticatedFailureMarketIntervalAccountsV2,
    plan: FailureMarketIntervalHistoryPlanV2,
    seal: FailureMarketIntervalFamilySealReceiptV2,
) -> Outcome<AuthenticatedFailureMarketIntervalAccountsV2> {
    authenticate_unchanged_account_prestate(
        program_id,
        cell_account,
        authenticated.cell_account,
        authenticated.cell_data_id,
        authenticated.cell_observed_lamports,
        FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_BYTES,
        false,
    )?;
    require(
        authenticated.cell.phase() == FailureMarketIntervalCellPhaseV2::Idle,
        ClutchError::MismatchedState,
    )?;
    let mut next = authenticated.history;
    next.commit_plan(plan)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let encoded = encode_history(authenticated.history_bump, next)?;
    let history_state_id = next
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    require(
        seal.facts().history_before == authenticated.history_state_id
            && seal.history_after() == history_state_id
            && next.family_terminal_receipt_id() == seal.facts().family_terminal_receipt_id,
        ClutchError::MismatchedState,
    )?;
    authenticate_write_prestate(
        program_id,
        history_account,
        authenticated.history_account,
        authenticated.history_data_id,
        authenticated.history_observed_lamports,
        &encoded,
    )?;
    let mut data = history_account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    data.copy_from_slice(&encoded);
    drop(data);
    let history_data_id = framed_data_id(&encoded);
    let history_authentication_id = account_authentication_id(
        HISTORY_AUTHENTICATION_DOMAIN_V2,
        history_account,
        history_data_id,
        history_state_id.bytes(),
        authenticated.admission_state_id.bytes(),
    );
    require_live_data_id(history_authentication_id)?;
    Ok(AuthenticatedFailureMarketIntervalAccountsV2 {
        history: next,
        history_state_id,
        history_data_id,
        history_authentication_id,
        ..authenticated
    })
}

/// Close the reusable cell first and append-only history second, only after
/// the exact sealed Failure-family receipt has been consumed by Product's
/// authenticated whole-Market terminal root.
///
/// Both accounts remain readable until every semantic and physical prestate
/// check succeeds. The two accounts and both recipients are then mutated in
/// one outer instruction; any later refusal rolls the entire batch back.
#[allow(clippy::too_many_arguments)]
pub(crate) fn close_failure_market_interval_accounts_v2<'a>(
    program_id: &Pubkey,
    admission_root_account: &AccountInfo<'a>,
    market_root_account: &AccountInfo<'a>,
    cell_account: &AccountInfo<'a>,
    history_account: &AccountInfo<'a>,
    rent_refund_owner: &AccountInfo<'a>,
    neutral_sink: &AccountInfo<'a>,
    authenticated: AuthenticatedFailureMarketIntervalAccountsV2,
    seal: FailureMarketIntervalFamilySealReceiptV2,
    market_terminal: AuthenticatedMarketInstanceTerminalV1,
    market_root_output: &mut MarketLifecycleRootAccountV1,
) -> Outcome<AuthenticatedFailureMarketIntervalCloseV2> {
    require(
        !admission_root_account.is_writable && !market_root_account.is_writable,
        ClutchError::UnexpectedWritable,
    )?;
    let live_admission =
        authenticate_failure_market_root_v2(program_id, admission_root_account, false)?;
    let live_market_terminal = authenticate_market_instance_terminal_v1(
        program_id,
        market_root_account,
        authenticated.cell.market_instance_id(),
        authenticated.cell.generation(),
        market_root_output,
    )?;
    require(
        live_admission.account() == authenticated.admission_root_account
            && live_admission
                .state()
                .id()
                .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?
                == authenticated.admission_state_id
            && live_market_terminal == market_terminal
            && market_terminal.root_account() == *market_root_account.key
            && authenticated.cell.phase() == FailureMarketIntervalCellPhaseV2::Idle
            && market_terminal.owner_program() == *program_id
            && market_terminal.market_instance_id() == authenticated.cell.market_instance_id()
            && market_terminal.generation() == authenticated.cell.generation()
            && market_terminal.failure_terminal_receipt_id().bytes()
                == seal.facts().family_terminal_receipt_id.bytes(),
        ClutchError::MismatchedState,
    )?;
    require(
        *admission_root_account.key != *market_root_account.key
            && *admission_root_account.key != *cell_account.key
            && *admission_root_account.key != *history_account.key
            && *admission_root_account.key != *rent_refund_owner.key
            && *admission_root_account.key != *neutral_sink.key
            && *market_root_account.key != *cell_account.key
            && *market_root_account.key != *history_account.key
            && *market_root_account.key != *rent_refund_owner.key
            && *market_root_account.key != *neutral_sink.key
            && *cell_account.key != *history_account.key
            && *cell_account.key != *rent_refund_owner.key
            && *cell_account.key != *neutral_sink.key
            && *history_account.key != *rent_refund_owner.key
            && *history_account.key != *neutral_sink.key
            && *rent_refund_owner.key != *neutral_sink.key,
        ClutchError::AccountAlias,
    )?;
    for recipient in [rent_refund_owner, neutral_sink] {
        require(recipient.is_writable, ClutchError::NotWritable)?;
        require(!recipient.is_signer, ClutchError::NonCanonical)?;
        require(!recipient.executable, ClutchError::ExecutableAccount)?;
        require(
            recipient.owner.to_bytes() == SYSTEM_PROGRAM_ID && recipient.data_len() == 0,
            ClutchError::WrongProgramOwner,
        )?;
    }
    authenticate_close_account_prestate(
        program_id,
        cell_account,
        authenticated.cell_account,
        authenticated.cell_data_id,
        authenticated.cell_observed_lamports,
    )?;
    authenticate_close_account_prestate(
        program_id,
        history_account,
        authenticated.history_account,
        authenticated.history_data_id,
        authenticated.history_observed_lamports,
    )?;
    let plan = plan_close_failure_market_interval_accounts_v2(
        authenticated.history,
        seal,
        cell_account.lamports(),
        history_account.lamports(),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        plan.work_account.bytes() == cell_account.key.to_bytes()
            && plan.history_account.bytes() == history_account.key.to_bytes()
            && plan.rent_refund_owner.bytes() == rent_refund_owner.key.to_bytes()
            && plan.neutral_sink.bytes() == neutral_sink.key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let refunded_principal_lamports = plan
        .work_rent_refund_lamports
        .checked_add(plan.history_rent_refund_lamports)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let neutralized_donation_lamports = plan
        .work_donation_lamports
        .checked_add(plan.history_donation_lamports)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let closed_account_lamports = cell_account
        .lamports()
        .checked_add(history_account.lamports())
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        refunded_principal_lamports
            .checked_add(neutralized_donation_lamports)
            .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?
            == closed_account_lamports,
        ClutchError::MismatchedState,
    )?;
    let refund_after = rent_refund_owner
        .lamports()
        .checked_add(refunded_principal_lamports)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let sink_after = neutral_sink
        .lamports()
        .checked_add(neutralized_donation_lamports)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    {
        let mut cell_lamports = cell_account
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let mut history_lamports = history_account
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let mut refund_lamports = rent_refund_owner
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let mut sink_lamports = neutral_sink
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        **cell_lamports = 0;
        **history_lamports = 0;
        **refund_lamports = refund_after;
        **sink_lamports = sink_after;
    }
    cell_account
        .resize(0)
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    cell_account.assign(&SYSTEM_PROGRAM_ID);
    history_account
        .resize(0)
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    history_account.assign(&SYSTEM_PROGRAM_ID);
    let id = ProductContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            CLOSE_AUTHENTICATION_DOMAIN_V2,
            &plan.authorization_id.bytes(),
            &market_terminal.id().bytes(),
            cell_account.key.as_ref(),
            history_account.key.as_ref(),
            rent_refund_owner.key.as_ref(),
            neutral_sink.key.as_ref(),
            &refunded_principal_lamports.to_le_bytes(),
            &neutralized_donation_lamports.to_le_bytes(),
        ])
        .to_bytes(),
    );
    require_live_data_id(id)?;
    Ok(AuthenticatedFailureMarketIntervalCloseV2 {
        id,
        close_authorization_id: plan.authorization_id,
        market_terminal_authentication_id: market_terminal.id(),
        cell_account: *cell_account.key,
        history_account: *history_account.key,
        rent_refund_owner: *rent_refund_owner.key,
        neutral_sink: *neutral_sink.key,
        refunded_principal_lamports,
        neutralized_donation_lamports,
    })
}

fn authenticate_account_metadata(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_len: usize,
    writable: bool,
) -> Outcome<()> {
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(!account.is_signer, ClutchError::NonCanonical)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    require(
        account.is_writable == writable,
        if writable {
            ClutchError::NotWritable
        } else {
            ClutchError::UnexpectedWritable
        },
    )?;
    require(
        account.data_len() == expected_len,
        ClutchError::WrongDataLength,
    )
}

#[allow(clippy::too_many_arguments)]
fn allocate_assign_interval_account_v2<'a>(
    program_id: &Pubkey,
    account: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    seed_domain: &[u8],
    market_instance_id: [u8; 32],
    generation: u64,
    bump: u8,
    account_len: usize,
) -> Outcome<()> {
    let generation_bytes = generation.to_le_bytes();
    let bump_seed = [bump];
    let signer_seeds: [&[u8]; 4] = [
        seed_domain,
        &market_instance_id,
        &generation_bytes,
        &bump_seed,
    ];
    let observed_lamports = account.lamports();
    let allocate = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &allocate_data(account_len),
        vec![AccountMeta::new(*account.key, true)],
    );
    invoke_signed(
        &allocate,
        &[account.clone(), system_program.clone()],
        &[&signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    let assign = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &assign_data(program_id),
        vec![AccountMeta::new(*account.key, true)],
    );
    invoke_signed(
        &assign,
        &[account.clone(), system_program.clone()],
        &[&signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        account.owner == program_id
            && account.data_len() == account_len
            && account.lamports() == observed_lamports,
        ClutchError::AccountCreationFailed,
    )
}

fn authenticate_write_prestate<const N: usize>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_account: Pubkey,
    expected_data_id: ProductContentId,
    expected_lamports: u64,
    output: &[u8; N],
) -> Outcome<()> {
    require(
        account.owner == program_id
            && account.is_writable
            && !account.is_signer
            && !account.executable
            && *account.key == expected_account
            && account.data_len() == N
            && account.lamports() == expected_lamports
            && framed_data_id(
                account
                    .try_borrow_data()
                    .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
                    .as_ref(),
            ) == expected_data_id
            && output.iter().any(|byte| *byte != 0),
        ClutchError::MismatchedState,
    )
}

fn authenticate_close_account_prestate(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_account: Pubkey,
    expected_data_id: ProductContentId,
    expected_lamports: u64,
) -> Outcome<()> {
    require(
        account.owner == program_id
            && account.is_writable
            && !account.is_signer
            && !account.executable
            && *account.key == expected_account
            && account.lamports() == expected_lamports
            && framed_data_id(
                account
                    .try_borrow_data()
                    .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
                    .as_ref(),
            ) == expected_data_id,
        ClutchError::MismatchedState,
    )
}

#[allow(clippy::too_many_arguments)]
fn authenticate_unchanged_account_prestate(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_account: Pubkey,
    expected_data_id: ProductContentId,
    expected_lamports: u64,
    expected_len: usize,
    writable: bool,
) -> Outcome<()> {
    authenticate_account_metadata(program_id, account, expected_len, writable)?;
    require(
        *account.key == expected_account
            && account.lamports() == expected_lamports
            && framed_data_id(
                account
                    .try_borrow_data()
                    .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
                    .as_ref(),
            ) == expected_data_id,
        ClutchError::MismatchedState,
    )
}

fn encode_cell(
    bump: u8,
    value: FailureMarketIntervalCellV2,
) -> Outcome<[u8; FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_BYTES]> {
    let mut semantic = [0; FAILURE_MARKET_INTERVAL_CELL_BYTES_V2];
    value
        .encode_into(&mut semantic)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let frame = FailureMarketIntervalCellAccountV2::new(bump, semantic)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let mut output = [0; FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_BYTES];
    frame
        .encode_into(&mut output)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    Ok(output)
}

fn encode_history(
    bump: u8,
    value: FailureMarketIntervalHistoryV2,
) -> Outcome<[u8; FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_BYTES]> {
    let mut semantic = [0; FAILURE_MARKET_INTERVAL_HISTORY_BYTES_V2];
    value
        .encode_into(&mut semantic)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let frame = FailureMarketIntervalHistoryAccountV2::new(bump, semantic)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let mut output = [0; FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_BYTES];
    frame
        .encode_into(&mut output)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    Ok(output)
}

fn account_authentication_id(
    domain: &[u8],
    account: &AccountInfo<'_>,
    data_id: ProductContentId,
    state_id: [u8; 32],
    admission_state_id: [u8; 32],
) -> ProductContentId {
    ProductContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            domain,
            account.key.as_ref(),
            account.owner.as_ref(),
            &data_id.bytes(),
            &state_id,
            &admission_state_id,
            &account.lamports().to_le_bytes(),
        ])
        .to_bytes(),
    )
}

fn framed_data_id(data: &[u8]) -> ProductContentId {
    ProductContentId::from_bytes(solana_sha256_hasher::hashv(&[data]).to_bytes())
}

fn require_live_data_id(id: ProductContentId) -> Outcome<()> {
    require(!id.is_zero(), ClutchError::MismatchedState)
}
