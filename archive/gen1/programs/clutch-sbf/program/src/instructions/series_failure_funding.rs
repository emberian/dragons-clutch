//! Shared typed Series-to-Failure present-funding receipt.
//!
//! This module is always compiled so the Product/Series and Failure families
//! can share one exact receipt without either laboratory feature enabling the
//! other's routes or changing the ELF capability profile. The constructor is
//! crate-private: only an adapter path that has authenticated the Series
//! funding root, canonical MarketCore vault, quote, and exact balances may
//! mint the receipt.

use crate::accounts::{expect_pda, require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::{require_system_program, transfer_data, SYSTEM_PROGRAM_ID};
use crate::seeds;
use clutch_product_series::{ContentId, MarketInstanceV2Id, SeriesFundingQuoteId, SeriesPlanV5Id};
use solana_account_info::AccountInfo;
use solana_cpi::invoke_signed;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

const SERIES_MARKET_CORE_FUNDING_RECEIPT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/series-market-core-funding-receipt/v1";

/// Exact occurrence-owned MarketCore debit admitted from Series custody.
///
/// The receipt is the only Series-side authority for funding a fresh Failure
/// root and its permanent replay tombstone. Recovery work/rent and Clock facts
/// are deliberately absent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesMarketCoreFundingReceiptV1 {
    receipt_id: ContentId,
    series_plan_id: SeriesPlanV5Id,
    ordinal: u32,
    market_instance_id: MarketInstanceV2Id,
    funding_quote_id: SeriesFundingQuoteId,
    funding_state_account: Pubkey,
    market_core_lamport_vault: Pubkey,
    lamport_principal_refund: ContentId,
    neutral_lamport_sink: ContentId,
    generation: u64,
    market_core_debit_lamports: u64,
    failure_root_rent_principal_lamports: u64,
    replay_tombstone_rent_principal_lamports: u64,
    vault_balance_before: u64,
    vault_balance_after_failure_accounts: u64,
    vault_balance_after: u64,
}

impl SeriesMarketCoreFundingReceiptV1 {
    /// Semantic identity of this exact account/debit plan.
    pub const fn id(self) -> ContentId {
        self.receipt_id
    }

    /// Exact registered Series.
    pub const fn series_plan_id(self) -> SeriesPlanV5Id {
        self.series_plan_id
    }

    /// Exact occurrence ordinal.
    pub const fn ordinal(self) -> u32 {
        self.ordinal
    }

    /// Exact full-width economic market.
    pub const fn market_instance_id(self) -> MarketInstanceV2Id {
        self.market_instance_id
    }

    /// Quote which owns every admitted amount.
    pub const fn funding_quote_id(self) -> SeriesFundingQuoteId {
        self.funding_quote_id
    }

    /// Mutable Series funding root whose principal was consumed.
    pub const fn funding_state_account(self) -> Pubkey {
        self.funding_state_account
    }

    /// Exact MarketCore lamport compartment PDA.
    pub const fn market_core_lamport_vault(self) -> Pubkey {
        self.market_core_lamport_vault
    }

    /// Immutable payer/refund owner.
    pub const fn lamport_principal_refund(self) -> ContentId {
        self.lamport_principal_refund
    }

    /// Immutable unowned-lamport sink.
    pub const fn neutral_lamport_sink(self) -> ContentId {
        self.neutral_lamport_sink
    }

    /// Canonical one-shot activation generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Complete MarketCore debit selected by the quote.
    pub const fn market_core_debit_lamports(self) -> u64 {
        self.market_core_debit_lamports
    }

    /// Exact a0 Failure-root rent principal.
    pub const fn failure_root_rent_principal_lamports(self) -> u64 {
        self.failure_root_rent_principal_lamports
    }

    /// Exact permanent a3 replay-tombstone rent principal.
    pub const fn replay_tombstone_rent_principal_lamports(self) -> u64 {
        self.replay_tombstone_rent_principal_lamports
    }

    /// Authenticated custody balance before the atomic outflow.
    pub const fn vault_balance_before(self) -> u64 {
        self.vault_balance_before
    }

    /// Balance immediately after a0/a3 rent funding, before other MarketCore outflows.
    pub const fn vault_balance_after_failure_accounts(self) -> u64 {
        self.vault_balance_after_failure_accounts
    }

    /// Required custody balance after the complete MarketCore outflow.
    pub const fn vault_balance_after(self) -> u64 {
        self.vault_balance_after
    }
}

/// Crate-private constructor used only after the Product/Series adapter has
/// authenticated the complete preimage and exact physical balance.
#[allow(clippy::too_many_arguments)]
pub(crate) fn mint_series_market_core_funding_receipt_v1(
    series_plan_id: SeriesPlanV5Id,
    ordinal: u32,
    market_instance_id: MarketInstanceV2Id,
    funding_quote_id: SeriesFundingQuoteId,
    funding_state_account: Pubkey,
    market_core_lamport_vault: Pubkey,
    lamport_principal_refund: ContentId,
    neutral_lamport_sink: ContentId,
    generation: u64,
    market_core_debit_lamports: u64,
    failure_root_rent_principal_lamports: u64,
    replay_tombstone_rent_principal_lamports: u64,
    vault_balance_before: u64,
    vault_balance_after_failure_accounts: u64,
    vault_balance_after: u64,
) -> SeriesMarketCoreFundingReceiptV1 {
    let ordinal_bytes = ordinal.to_le_bytes();
    let generation_bytes = generation.to_le_bytes();
    let market_core_debit = market_core_debit_lamports.to_le_bytes();
    let failure_root_rent = failure_root_rent_principal_lamports.to_le_bytes();
    let replay_tombstone_rent = replay_tombstone_rent_principal_lamports.to_le_bytes();
    let balance_before = vault_balance_before.to_le_bytes();
    let balance_after_failure_accounts = vault_balance_after_failure_accounts.to_le_bytes();
    let balance_after = vault_balance_after.to_le_bytes();
    let receipt_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SERIES_MARKET_CORE_FUNDING_RECEIPT_DOMAIN_V1,
            &series_plan_id.bytes(),
            &ordinal_bytes,
            &market_instance_id.bytes(),
            &funding_quote_id.bytes(),
            funding_state_account.as_ref(),
            market_core_lamport_vault.as_ref(),
            &lamport_principal_refund.bytes(),
            &neutral_lamport_sink.bytes(),
            &generation_bytes,
            &market_core_debit,
            &failure_root_rent,
            &replay_tombstone_rent,
            &balance_before,
            &balance_after_failure_accounts,
            &balance_after,
        ])
        .to_bytes(),
    );
    SeriesMarketCoreFundingReceiptV1 {
        receipt_id,
        series_plan_id,
        ordinal,
        market_instance_id,
        funding_quote_id,
        funding_state_account,
        market_core_lamport_vault,
        lamport_principal_refund,
        neutral_lamport_sink,
        generation,
        market_core_debit_lamports,
        failure_root_rent_principal_lamports,
        replay_tombstone_rent_principal_lamports,
        vault_balance_before,
        vault_balance_after_failure_accounts,
        vault_balance_after,
    }
}

/// Transfer the receipt-owned a0/a3 rent principal from the canonical
/// MarketCore vault and require the exact intermediate custody balance.
///
/// Both destinations may carry prior third-party lamports. The full quoted
/// principal is still transferred, preserving payer ownership while leaving
/// those prior lamports separately observable as donations when Failure
/// allocates and persists the accounts. This helper does not allocate, assign,
/// or write either Failure account.
pub fn fund_series_failure_accounts_v1<'a>(
    program_id: &Pubkey,
    receipt: SeriesMarketCoreFundingReceiptV1,
    market_core_vault: &AccountInfo<'a>,
    failure_root: &AccountInfo<'a>,
    replay_tombstone: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
) -> Outcome<()> {
    require_system_program(system_program)?;
    require(
        *market_core_vault.key == receipt.market_core_lamport_vault
            && market_core_vault.is_writable
            && !market_core_vault.is_signer
            && !market_core_vault.executable
            && *market_core_vault.owner == SYSTEM_PROGRAM_ID
            && market_core_vault.data_is_empty(),
        ClutchError::MismatchedState,
    )?;
    let series = receipt.series_plan_id.bytes();
    let component = [0_u8];
    let (expected_vault, vault_bump) =
        seeds::series_lamport_vault_pda(program_id, &series, component[0]);
    expect_pda(market_core_vault.key, (expected_vault, vault_bump), None)?;
    let market = receipt.market_instance_id.bytes();
    let (expected_root, _) =
        seeds::failure_external_root_pda(program_id, &market, receipt.generation);
    let (expected_tombstone, _) =
        seeds::failure_replay_tombstone_pda(program_id, &market, receipt.generation);
    require(
        *failure_root.key == expected_root && *replay_tombstone.key == expected_tombstone,
        ClutchError::MismatchedState,
    )?;
    require(
        failure_root.key != replay_tombstone.key
            && failure_root.key != market_core_vault.key
            && replay_tombstone.key != market_core_vault.key,
        ClutchError::AccountAlias,
    )?;
    for destination in [failure_root, replay_tombstone] {
        require(
            destination.is_writable
                && !destination.is_signer
                && !destination.executable
                && *destination.owner == SYSTEM_PROGRAM_ID
                && destination.data_is_empty(),
            ClutchError::MismatchedState,
        )?;
    }
    require(
        market_core_vault.lamports() == receipt.vault_balance_before,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    let root_before = failure_root.lamports();
    let tombstone_before = replay_tombstone.lamports();
    let root_after = root_before
        .checked_add(receipt.failure_root_rent_principal_lamports)
        .ok_or(ClutchError::Arithmetic)?;
    let tombstone_after = tombstone_before
        .checked_add(receipt.replay_tombstone_rent_principal_lamports)
        .ok_or(ClutchError::Arithmetic)?;
    let bump = [vault_bump];
    let signer_seeds = &[
        seeds::SEED_SERIES_LAMPORT_VAULT_V1,
        &series,
        &component,
        &bump,
    ];
    transfer_from_market_core_vault(
        market_core_vault,
        failure_root,
        system_program,
        receipt.failure_root_rent_principal_lamports,
        signer_seeds,
    )?;
    require(
        failure_root.lamports() == root_after,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    transfer_from_market_core_vault(
        market_core_vault,
        replay_tombstone,
        system_program,
        receipt.replay_tombstone_rent_principal_lamports,
        signer_seeds,
    )?;
    require(
        replay_tombstone.lamports() == tombstone_after
            && market_core_vault.lamports() == receipt.vault_balance_after_failure_accounts,
        ClutchError::SeriesCustodyDeltaMismatch,
    )
}

fn transfer_from_market_core_vault<'a>(
    vault: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    lamports: u64,
    signer_seeds: &[&[u8]],
) -> Outcome<()> {
    require(lamports != 0, ClutchError::MismatchedState)?;
    let instruction = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &transfer_data(lamports),
        vec![
            AccountMeta::new(*vault.key, true),
            AccountMeta::new(*destination.key, false),
        ],
    );
    invoke_signed(
        &instruction,
        &[vault.clone(), destination.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))
}
