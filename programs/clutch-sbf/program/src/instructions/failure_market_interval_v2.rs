// SPDX-License-Identifier: AGPL-3.0-or-later
//! Capability-disabled SBF seam for reusable Failure interval accounts.
//!
//! The Failure runtime owns every semantic byte in `0xab/v2` and `0xac/v2`.
//! `clutch-solana-layout` owns only their four-byte physical frames. This
//! module authenticates owner, fresh successor PDA, exact frame/body, present
//! principal, and stale preimages before writing a pure private-field plan.
//! It deliberately exposes no instruction route and no account initializer:
//! initialization remains unavailable until Product supplies its private
//! accepted foundation-step receipt for slots 8 and 9.

use crate::accounts::{expect_pda, require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::failure_market_admission::AuthenticatedFailureMarketRootV2;
use crate::seeds;
use clutch_failure_policy_runtime::market_interval_cell_v2::{
    FailureMarketIntervalCellPhaseV2, FailureMarketIntervalCellPlanV2,
    FailureMarketIntervalCellResetReceiptV2, FailureMarketIntervalCellStateIdV2,
    FailureMarketIntervalCellV2, FAILURE_MARKET_INTERVAL_CELL_BYTES_V2,
};
use clutch_failure_policy_runtime::market_interval_history_v2::{
    FailureMarketIntervalFamilySealReceiptV2, FailureMarketIntervalFundingReceiptV2,
    FailureMarketIntervalHistoryAppendReceiptV2, FailureMarketIntervalHistoryPlanV2,
    FailureMarketIntervalHistoryStateIdV2, FailureMarketIntervalHistoryV2,
    FAILURE_MARKET_INTERVAL_HISTORY_BYTES_V2,
};
use clutch_failure_policy_runtime::market_quote_v1::FailureMarketRecoveryQuoteAdmissionReceiptV1;
use clutch_product_series::ContentId as ProductContentId;
use clutch_solana_layout::failure_market_interval_v2::{
    FailureMarketIntervalCellAccountV2, FailureMarketIntervalHistoryAccountV2,
    FAILURE_MARKET_INTERVAL_CELL_BODY_BYTES_V2, FAILURE_MARKET_INTERVAL_HISTORY_BODY_BYTES_V2,
};
use clutch_solana_layout::registry::{
    FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_BYTES, FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_BYTES,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

const CELL_AUTHENTICATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/failure-market-interval-cell-account-authentication/v2";
const HISTORY_AUTHENTICATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/failure-market-interval-history-account-authentication/v2";

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

    /// Product-authenticated reusable-account capitalization.
    pub(crate) const fn funding(self) -> FailureMarketIntervalFundingReceiptV2 {
        self.funding
    }

    /// Market-scoped liveness quote admission used for hostile decoding.
    pub(crate) const fn quote(self) -> FailureMarketRecoveryQuoteAdmissionReceiptV1 {
        self.quote
    }
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
    let cell_authentication_id = account_authentication_id(
        CELL_AUTHENTICATION_DOMAIN_V2,
        cell_account,
        cell_data_id,
        cell_state_id.bytes(),
    );
    let history_authentication_id = account_authentication_id(
        HISTORY_AUTHENTICATION_DOMAIN_V2,
        history_account,
        history_data_id,
        history_state_id.bytes(),
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
        funding,
        quote,
    })
}

/// Persist one private-field cell transition over the exact authenticated
/// preimage. Used by begin, paid advance, resolution, and exhaustion owners;
/// it does not itself mint any of those authorities.
pub(crate) fn write_failure_market_interval_cell_plan_v2(
    program_id: &Pubkey,
    cell_account: &AccountInfo<'_>,
    history_account: &AccountInfo<'_>,
    authenticated: AuthenticatedFailureMarketIntervalAccountsV2,
    plan: FailureMarketIntervalCellPlanV2,
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
    require(
        matches!(
            (authenticated.cell.phase(), next.phase()),
            (
                FailureMarketIntervalCellPhaseV2::Idle,
                FailureMarketIntervalCellPhaseV2::Active
            ) | (
                FailureMarketIntervalCellPhaseV2::Active,
                FailureMarketIntervalCellPhaseV2::Active
            ) | (
                FailureMarketIntervalCellPhaseV2::Active,
                FailureMarketIntervalCellPhaseV2::Resolved
            )
        ),
        ClutchError::MismatchedState,
    )?;
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
    );
    let history_authentication_id = account_authentication_id(
        HISTORY_AUTHENTICATION_DOMAIN_V2,
        history_account,
        history_data_id,
        next_history_id.bytes(),
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
) -> ProductContentId {
    ProductContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            domain,
            account.key.as_ref(),
            account.owner.as_ref(),
            &data_id.bytes(),
            &state_id,
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
