//! Callable current Product-bound Source retirement first stage.
//!
//! This module owns the flat account contract shared by General action 47 and
//! Product retirement. It does not begin whole-Market retirement or latch the
//! Source shared-core slot. Instead, it consumes the same-instruction Failure
//! family terminal, drains the exact Source lifecycle custody, retires the
//! final LinkV3 through RootV3, and records the Link retirement in ReplayV3.

use crate::accounts::{require, require_count, require_distinct, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::failure_market_family_terminal_v2::
    AuthenticatedFailureMarketFamilyTerminalReceiptV3;
use crate::instructions::product_series_current::retirement_v5::{
    consume_failure_family_terminal_v5,
    retire_failed_source_and_count_series_link_v5,
    retire_successful_source_and_count_series_link_v5,
    AuthenticatedProductSourceSeriesRetirementV5,
};
use crate::source_plane_v3::authenticate_route;
use crate::source_plane_v3_actions::authenticate_source_work_schedule_artifact;
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

/// Exact number of accounts in the Product Source-retirement core.
///
/// RootV3, LinkV3, FundingV5, and lifecycle ReplayV3 are shared with the
/// surrounding Product/General frame. The remaining twelve roles are the
/// additional Source route, custody, and disposition accounts.
pub(crate) const PRODUCT_SOURCE_RETIREMENT_ACCOUNT_COUNT_V5: usize = 16;

pub(crate) const IX_SOURCE_RETIREMENT_ROOT_V3: usize = 0;
pub(crate) const IX_SOURCE_RETIREMENT_LINK_V3: usize = 1;
pub(crate) const IX_SOURCE_RETIREMENT_FUNDING_V5: usize = 2;
pub(crate) const IX_SOURCE_RETIREMENT_LIFECYCLE_REPLAY_V3: usize = 3;
pub(crate) const IX_SOURCE_RETIREMENT_RELEASE: usize = 4;
pub(crate) const IX_SOURCE_RETIREMENT_ADAPTER_PROGRAM: usize = 5;
pub(crate) const IX_SOURCE_RETIREMENT_ADAPTER_PROGRAMDATA: usize = 6;
pub(crate) const IX_SOURCE_RETIREMENT_PARSER_PROGRAM: usize = 7;
pub(crate) const IX_SOURCE_RETIREMENT_PARSER_PROGRAMDATA: usize = 8;
pub(crate) const IX_SOURCE_RETIREMENT_PARSER_CONFIG: usize = 9;
pub(crate) const IX_SOURCE_RETIREMENT_SOURCE_SPEC: usize = 10;
pub(crate) const IX_SOURCE_RETIREMENT_WORK_SCHEDULE: usize = 11;
pub(crate) const IX_SOURCE_RETIREMENT_CUSTODY: usize = 12;
pub(crate) const IX_SOURCE_RETIREMENT_PRINCIPAL_REFUND: usize = 13;
pub(crate) const IX_SOURCE_RETIREMENT_NEUTRAL_SINK: usize = 14;
pub(crate) const IX_SOURCE_RETIREMENT_SYSTEM_PROGRAM: usize = 15;

/// Semantic role at one exact Product Source-retirement account index.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProductSourceRetirementAccountRoleV5 {
    MarketLifecycleRootV3,
    SeriesMarketLinkV3,
    SeriesFundingV5,
    SeriesLifecycleReplayV3,
    SourceRelease,
    SourceAdapterProgram,
    SourceAdapterProgramData,
    SourceParserProgram,
    SourceParserProgramData,
    SourceParserConfig,
    SourceSpec,
    SourceWorkSchedule,
    SourceFundingCustody,
    PrincipalRefund,
    NeutralSink,
    SystemProgram,
}

/// One exact role and effective Solana privilege in the composite action-47
/// frame. FundingV5 is writable because the later Product finalizer closes it
/// in the same instruction; this first stage itself only reads the account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProductSourceRetirementAccountMetaV5 {
    pub(crate) role: ProductSourceRetirementAccountRoleV5,
    pub(crate) writable: bool,
    pub(crate) signer: bool,
}

const fn meta(
    role: ProductSourceRetirementAccountRoleV5,
    writable: bool,
) -> ProductSourceRetirementAccountMetaV5 {
    ProductSourceRetirementAccountMetaV5 {
        role,
        writable,
        signer: false,
    }
}

/// Frozen ordered Source first-stage account contract.
pub(crate) const PRODUCT_SOURCE_RETIREMENT_ACCOUNT_METAS_V5:
    [ProductSourceRetirementAccountMetaV5; PRODUCT_SOURCE_RETIREMENT_ACCOUNT_COUNT_V5] = [
        meta(ProductSourceRetirementAccountRoleV5::MarketLifecycleRootV3, true),
        meta(ProductSourceRetirementAccountRoleV5::SeriesMarketLinkV3, true),
        meta(ProductSourceRetirementAccountRoleV5::SeriesFundingV5, true),
        meta(ProductSourceRetirementAccountRoleV5::SeriesLifecycleReplayV3, true),
        meta(ProductSourceRetirementAccountRoleV5::SourceRelease, false),
        meta(ProductSourceRetirementAccountRoleV5::SourceAdapterProgram, false),
        meta(ProductSourceRetirementAccountRoleV5::SourceAdapterProgramData, false),
        meta(ProductSourceRetirementAccountRoleV5::SourceParserProgram, false),
        meta(ProductSourceRetirementAccountRoleV5::SourceParserProgramData, false),
        meta(ProductSourceRetirementAccountRoleV5::SourceParserConfig, false),
        meta(ProductSourceRetirementAccountRoleV5::SourceSpec, false),
        meta(ProductSourceRetirementAccountRoleV5::SourceWorkSchedule, false),
        meta(ProductSourceRetirementAccountRoleV5::SourceFundingCustody, true),
        meta(ProductSourceRetirementAccountRoleV5::PrincipalRefund, true),
        meta(ProductSourceRetirementAccountRoleV5::NeutralSink, true),
        meta(ProductSourceRetirementAccountRoleV5::SystemProgram, false),
    ];

/// Exact exhaustive Source terminal branch selected by the authenticated
/// Failure family receipt. Failed Source paths additionally carry the durable
/// Source-owned terminal account; successful paths have no such account.
#[derive(Debug)]
pub(crate) enum ProductSourceRetirementDispositionV5<'account, 'info> {
    Successful,
    Failed {
        persisted_source_terminal_account: &'account AccountInfo<'info>,
    },
}

fn require_product_source_retirement_account_contract_v5(
    accounts: &[AccountInfo<'_>],
) -> Outcome<()> {
    require_count(accounts, PRODUCT_SOURCE_RETIREMENT_ACCOUNT_COUNT_V5)?;
    require_distinct(accounts)?;
    let mut index = 0usize;
    while index < PRODUCT_SOURCE_RETIREMENT_ACCOUNT_COUNT_V5 {
        let observed = &accounts[index];
        let expected = PRODUCT_SOURCE_RETIREMENT_ACCOUNT_METAS_V5[index];
        require(
            observed.key != &Pubkey::default()
                && observed.is_writable == expected.writable
                && observed.is_signer == expected.signer,
            ClutchError::MismatchedState,
        )?;
        index += 1;
    }
    Ok(())
}

/// Consume the same-instruction Failure family terminal and perform the sole
/// Product-bound Source first stage.
///
/// The caller must hostile-reconstruct `failure_terminal` from the persisted
/// Failure accounts in this same instruction. This function immediately moves
/// it through Product preauthorization, Source custody closure, RootV3/LinkV3
/// link retirement, and lifecycle ReplayV3. No copyable or ID-only terminal
/// handoff is returned.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(crate) fn compose_product_source_series_retirement_v5<'account, 'info>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'info>],
    failure_terminal: AuthenticatedFailureMarketFamilyTerminalReceiptV3,
    disposition: ProductSourceRetirementDispositionV5<'account, 'info>,
) -> Outcome<AuthenticatedProductSourceSeriesRetirementV5> {
    require_product_source_retirement_account_contract_v5(accounts)?;
    let root_account = &accounts[IX_SOURCE_RETIREMENT_ROOT_V3];
    let link_account = &accounts[IX_SOURCE_RETIREMENT_LINK_V3];
    let funding_account = &accounts[IX_SOURCE_RETIREMENT_FUNDING_V5];
    let replay_account = &accounts[IX_SOURCE_RETIREMENT_LIFECYCLE_REPLAY_V3];
    let route = authenticate_route(
        program_id,
        &accounts[IX_SOURCE_RETIREMENT_RELEASE],
        &accounts[IX_SOURCE_RETIREMENT_ADAPTER_PROGRAM],
        &accounts[IX_SOURCE_RETIREMENT_ADAPTER_PROGRAMDATA],
        &accounts[IX_SOURCE_RETIREMENT_PARSER_PROGRAM],
        &accounts[IX_SOURCE_RETIREMENT_PARSER_PROGRAMDATA],
        &accounts[IX_SOURCE_RETIREMENT_PARSER_CONFIG],
        &accounts[IX_SOURCE_RETIREMENT_SOURCE_SPEC],
    )
    .map_err(Refusal::from)?;
    let schedule = authenticate_source_work_schedule_artifact(
        program_id,
        route,
        &accounts[IX_SOURCE_RETIREMENT_WORK_SCHEDULE],
    )?;
    let failure = consume_failure_family_terminal_v5(
        program_id,
        root_account,
        link_account,
        failure_terminal,
    )?;
    match disposition {
        ProductSourceRetirementDispositionV5::Successful => {
            retire_successful_source_and_count_series_link_v5(
                program_id,
                root_account,
                link_account,
                funding_account,
                replay_account,
                failure,
                route,
                schedule,
                &accounts[IX_SOURCE_RETIREMENT_CUSTODY],
                &accounts[IX_SOURCE_RETIREMENT_PRINCIPAL_REFUND],
                &accounts[IX_SOURCE_RETIREMENT_NEUTRAL_SINK],
                &accounts[IX_SOURCE_RETIREMENT_SYSTEM_PROGRAM],
            )
        }
        ProductSourceRetirementDispositionV5::Failed {
            persisted_source_terminal_account,
        } => {
            require(
                persisted_source_terminal_account.key != &Pubkey::default()
                    && !persisted_source_terminal_account.is_writable
                    && !persisted_source_terminal_account.is_signer
                    && accounts
                        .iter()
                        .all(|account| account.key != persisted_source_terminal_account.key),
                ClutchError::AccountAlias,
            )?;
            retire_failed_source_and_count_series_link_v5(
                program_id,
                root_account,
                link_account,
                funding_account,
                replay_account,
                failure,
                route,
                schedule,
                persisted_source_terminal_account,
                &accounts[IX_SOURCE_RETIREMENT_CUSTODY],
                &accounts[IX_SOURCE_RETIREMENT_PRINCIPAL_REFUND],
                &accounts[IX_SOURCE_RETIREMENT_NEUTRAL_SINK],
                &accounts[IX_SOURCE_RETIREMENT_SYSTEM_PROGRAM],
            )
        }
    }
}
