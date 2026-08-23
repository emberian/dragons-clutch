// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{
    admit_realm_collateral_account_v2, validate_series_collateral_vault_v2, BoundRealmCollateralV2,
    CpiAccountMetaV2, CustodyBindingV2, Error, Id, Result, RuntimeAccountViewV2,
    SeriesCollateralTerminalJoinV2, TokenAccountRoleV2, TransferAuthorityKindV2,
    TransferAuthorityV2, SERIES_ACTIVATION_GENERATION_V2,
};

/// Solana's canonical all-zero System Program address.
///
/// This is the sole typed exception to [`Id::ZERO`] denoting inactive padding.
pub const SOLANA_SYSTEM_PROGRAM: Id = Id::ZERO;
/// Exact close-account instruction-data width for both admitted token families.
pub const CLOSE_ACCOUNT_DATA_V2_BYTES: usize = 1;

/// Runtime account view extended with its exact lamport balance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeLamportAccountViewV2<'a> {
    /// Hostile runtime account state and transaction flags.
    pub account: RuntimeAccountViewV2<'a>,
    /// Exact current lamport balance read from the runtime account.
    pub lamports: u64,
}

/// Complete Series authorization for closing one empty collateral vault.
///
/// `stored_vault_rent_principal_lamports` is lamport-denominated account-rent
/// principal persisted at creation. It is never collateral-token principal,
/// Market work capital, a fee, or a liveness reserve.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesCollateralVaultCloseRequestV2 {
    /// Authenticated one-shot Series terminal join.
    pub terminal: SeriesCollateralTerminalJoinV2,
    /// Ordered Series component in `1..=5`.
    pub component: u16,
    /// Exact segregated collateral vault being closed.
    pub vault: CustodyBindingV2,
    /// Exact zero-data System-owned component lamport-vault PDA.
    pub component_lamport_vault: Id,
    /// Persisted payer-funded rent principal for this token vault.
    pub stored_vault_rent_principal_lamports: u64,
    /// Authenticated SeriesFunding PDA authority used for `invoke_signed`.
    pub authority: TransferAuthorityV2,
}

/// Exact release-selected close-account CPI intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CloseCustodyCpiV2 {
    /// Realm-selected legacy SPL Token or Token-2022 program.
    pub token_program: Id,
    /// Ordered vault, lamport destination, and owner-authority metas.
    pub accounts: [CpiAccountMetaV2; 3],
    /// Release-frozen close-account discriminator.
    pub data: [u8; CLOSE_ACCOUNT_DATA_V2_BYTES],
    /// Always true: Series collateral vaults have canonical PDA owners.
    pub program_signed: bool,
}

/// Validated empty-vault snapshot and its sole permitted external invocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedSeriesCollateralVaultCloseV2 {
    request: SeriesCollateralVaultCloseRequestV2,
    vault_lamports_before: u64,
    component_lamports_before: u64,
    cpi: CloseCustodyCpiV2,
}

impl PreparedSeriesCollateralVaultCloseV2 {
    /// Sole external invocation this prepared transition permits.
    pub const fn cpi(self) -> CloseCustodyCpiV2 {
        self.cpi
    }

    /// Exact lamports the close must move into the component lamport vault.
    pub const fn close_lamports(self) -> u64 {
        self.vault_lamports_before
    }
}

/// Accepted close delta, ready for the separately postchecked rent split.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClosedSeriesCollateralVaultV2 {
    terminal_receipt: Id,
    series_plan: Id,
    component: u16,
    closed_vault: Id,
    component_lamport_vault: Id,
    component_lamports_before_close: u64,
    component_lamports_after_close: u64,
    extracted_lamports: u64,
    refundable_rent_principal_lamports: u64,
    neutral_surplus_lamports: u64,
    payer_lamport_refund: Id,
    neutral_lamport_sink: Id,
}

impl ClosedSeriesCollateralVaultV2 {
    /// Exact one-shot terminal receipt authorizing the close.
    pub const fn terminal_receipt(self) -> Id {
        self.terminal_receipt
    }

    /// Canonical one-shot Series activation generation.
    pub const fn activation_generation(self) -> u64 {
        SERIES_ACTIVATION_GENERATION_V2
    }

    /// Exact lamports extracted from only the closed token vault.
    pub const fn extracted_lamports(self) -> u64 {
        self.extracted_lamports
    }

    /// Payer-funded token-vault rent principal to refund.
    pub const fn refundable_rent_principal_lamports(self) -> u64 {
        self.refundable_rent_principal_lamports
    }

    /// Unattributed prefunding or donation surplus to send to the neutral sink.
    pub const fn neutral_surplus_lamports(self) -> u64 {
        self.neutral_surplus_lamports
    }
}

/// Prepare a release-authorized close of one empty Series collateral vault.
///
/// The hostile token bytes must prove the exact Realm mint, custody authority,
/// owner guard, extensions, and a zero collateral-atom balance. The close moves
/// lamports only; nonzero collateral principal refuses before CPI.
pub fn prepare_series_collateral_vault_close_v2(
    bound: BoundRealmCollateralV2,
    request: SeriesCollateralVaultCloseRequestV2,
    vault: RuntimeLamportAccountViewV2<'_>,
    component_lamport_vault: RuntimeLamportAccountViewV2<'_>,
) -> Result<PreparedSeriesCollateralVaultCloseV2> {
    request.terminal.validate(bound)?;
    validate_series_collateral_vault_v2(
        bound,
        request.terminal.funding,
        request.component,
        request.vault,
    )?;
    request.authority.validate()?;
    request.component_lamport_vault.require_live()?;
    if request.stored_vault_rent_principal_lamports == 0
        || request.authority.kind != TransferAuthorityKindV2::ProgramDerived
        || request.authority.address != request.vault.owner_authority
        || request.authority.address != request.terminal.funding.funding_authority
        || vault.account.key == component_lamport_vault.account.key
        || vault.account.key != request.vault.account
        || !vault.account.is_writable
    {
        return Err(Error::SeriesJoinMismatch);
    }
    admit_system_lamport_vault_v2(component_lamport_vault, request.component_lamport_vault)?;
    let observation = admit_realm_collateral_account_v2(
        bound,
        vault.account,
        TokenAccountRoleV2::SegregatedVault(request.vault),
    )?;
    if observation.mint != bound.policy().mint
        || observation.owner_authority != request.vault.owner_authority
    {
        return Err(Error::SeriesJoinMismatch);
    }
    if observation.amount_atoms != 0 {
        return Err(Error::CustodyNotEmpty);
    }
    if vault.lamports < request.stored_vault_rent_principal_lamports {
        return Err(Error::RentPrincipalNotCovered);
    }
    let release = bound.release();
    Ok(PreparedSeriesCollateralVaultCloseV2 {
        request,
        vault_lamports_before: vault.lamports,
        component_lamports_before: component_lamport_vault.lamports,
        cpi: CloseCustodyCpiV2 {
            token_program: release.token_program,
            accounts: [
                CpiAccountMetaV2 {
                    address: request.vault.account,
                    writable: true,
                    signer: false,
                },
                CpiAccountMetaV2 {
                    address: request.component_lamport_vault,
                    writable: true,
                    signer: false,
                },
                CpiAccountMetaV2 {
                    address: request.authority.address,
                    writable: false,
                    signer: true,
                },
            ],
            data: [release.close_account_discriminator],
            program_signed: true,
        },
    })
}

/// Accept only the exact close terminal state and isolated lamport delta.
pub fn accept_series_collateral_vault_close_v2(
    prepared: PreparedSeriesCollateralVaultCloseV2,
    vault_after: RuntimeLamportAccountViewV2<'_>,
    component_lamport_vault_after: RuntimeLamportAccountViewV2<'_>,
) -> Result<ClosedSeriesCollateralVaultV2> {
    if vault_after.account.key != prepared.request.vault.account
        || !vault_after.account.is_writable
        || vault_after.account.is_signer
        || vault_after.account.executable
        || vault_after.account.owner_program != SOLANA_SYSTEM_PROGRAM
        || !vault_after.account.data.is_empty()
        || vault_after.lamports != 0
    {
        return Err(Error::CloseDeltaMismatch);
    }
    admit_system_lamport_vault_v2(
        component_lamport_vault_after,
        prepared.request.component_lamport_vault,
    )
    .map_err(|_| Error::CloseDeltaMismatch)?;
    if component_lamport_vault_after
        .lamports
        .checked_sub(prepared.component_lamports_before)
        != Some(prepared.vault_lamports_before)
    {
        return Err(Error::CloseDeltaMismatch);
    }
    let neutral_surplus_lamports = prepared
        .vault_lamports_before
        .checked_sub(prepared.request.stored_vault_rent_principal_lamports)
        .ok_or(Error::RentPrincipalNotCovered)?;
    let funding = prepared.request.terminal.funding;
    Ok(ClosedSeriesCollateralVaultV2 {
        terminal_receipt: prepared.request.terminal.terminal_receipt,
        series_plan: funding.series_plan,
        component: prepared.request.component,
        closed_vault: prepared.request.vault.account,
        component_lamport_vault: prepared.request.component_lamport_vault,
        component_lamports_before_close: prepared.component_lamports_before,
        component_lamports_after_close: component_lamport_vault_after.lamports,
        extracted_lamports: prepared.vault_lamports_before,
        refundable_rent_principal_lamports: prepared.request.stored_vault_rent_principal_lamports,
        neutral_surplus_lamports,
        payer_lamport_refund: funding.payer_lamport_refund,
        neutral_lamport_sink: funding.neutral_lamport_sink,
    })
}

/// One exact lamport credit in the post-close Series rent disposition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesVaultRentCreditV2 {
    /// Exact destination account.
    pub destination: Id,
    /// Exact lamports to credit; the neutral credit may be zero.
    pub lamports: u64,
}

/// Pre-state and exact credits for the post-close rent disposition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedSeriesVaultRentDispositionV2 {
    closed: ClosedSeriesCollateralVaultV2,
    refund_lamports_before: u64,
    neutral_lamports_before: u64,
    credits: [SeriesVaultRentCreditV2; 2],
}

impl PreparedSeriesVaultRentDispositionV2 {
    /// Ordered payer-rent refund then neutral-surplus credits.
    pub const fn credits(self) -> [SeriesVaultRentCreditV2; 2] {
        self.credits
    }

    /// Whether the neutral credit is active. A zero surplus emits no transfer.
    pub const fn credit_count(self) -> u8 {
        if self.credits[1].lamports == 0 {
            1
        } else {
            2
        }
    }
}

/// Admit the exact System-owned accounts immediately before the rent split.
pub fn prepare_series_vault_rent_disposition_v2(
    closed: ClosedSeriesCollateralVaultV2,
    component_lamport_vault: RuntimeLamportAccountViewV2<'_>,
    payer_refund: RuntimeLamportAccountViewV2<'_>,
    neutral_sink: RuntimeLamportAccountViewV2<'_>,
) -> Result<PreparedSeriesVaultRentDispositionV2> {
    admit_system_lamport_vault_v2(component_lamport_vault, closed.component_lamport_vault)?;
    admit_system_lamport_destination_v2(payer_refund, closed.payer_lamport_refund)?;
    admit_system_lamport_destination_v2(neutral_sink, closed.neutral_lamport_sink)?;
    if closed.component_lamport_vault == closed.payer_lamport_refund
        || closed.component_lamport_vault == closed.neutral_lamport_sink
        || closed.payer_lamport_refund == closed.neutral_lamport_sink
        || component_lamport_vault.lamports != closed.component_lamports_after_close
        || component_lamport_vault
            .lamports
            .checked_sub(closed.component_lamports_before_close)
            != Some(closed.extracted_lamports)
    {
        return Err(Error::SeriesJoinMismatch);
    }
    Ok(PreparedSeriesVaultRentDispositionV2 {
        closed,
        refund_lamports_before: payer_refund.lamports,
        neutral_lamports_before: neutral_sink.lamports,
        credits: [
            SeriesVaultRentCreditV2 {
                destination: closed.payer_lamport_refund,
                lamports: closed.refundable_rent_principal_lamports,
            },
            SeriesVaultRentCreditV2 {
                destination: closed.neutral_lamport_sink,
                lamports: closed.neutral_surplus_lamports,
            },
        ],
    })
}

/// Receipt proving that only the close delta was split and prefunding remained.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AcceptedSeriesVaultRentDispositionV2 {
    /// Exact one-shot terminal receipt.
    pub terminal_receipt: Id,
    /// Exact Series plan.
    pub series_plan: Id,
    /// Ordered Series component in `1..=5`.
    pub component: u16,
    /// Closed collateral token-vault address.
    pub closed_vault: Id,
    /// Preserved component lamport-vault prefunding balance.
    pub component_lamports_after: u64,
    /// Exact payer-funded rent principal refunded.
    pub refunded_rent_principal_lamports: u64,
    /// Exact close surplus sent to the neutral sink.
    pub neutral_surplus_lamports: u64,
}

/// Accept exact payer/neutral credits and restoration of the pre-close balance.
pub fn accept_series_vault_rent_disposition_v2(
    prepared: PreparedSeriesVaultRentDispositionV2,
    component_lamport_vault_after: RuntimeLamportAccountViewV2<'_>,
    payer_refund_after: RuntimeLamportAccountViewV2<'_>,
    neutral_sink_after: RuntimeLamportAccountViewV2<'_>,
) -> Result<AcceptedSeriesVaultRentDispositionV2> {
    let closed = prepared.closed;
    admit_system_lamport_vault_v2(
        component_lamport_vault_after,
        closed.component_lamport_vault,
    )
    .map_err(|_| Error::CloseDeltaMismatch)?;
    admit_system_lamport_destination_v2(payer_refund_after, closed.payer_lamport_refund)
        .map_err(|_| Error::CloseDeltaMismatch)?;
    admit_system_lamport_destination_v2(neutral_sink_after, closed.neutral_lamport_sink)
        .map_err(|_| Error::CloseDeltaMismatch)?;
    if component_lamport_vault_after.lamports != closed.component_lamports_before_close
        || payer_refund_after
            .lamports
            .checked_sub(prepared.refund_lamports_before)
            != Some(closed.refundable_rent_principal_lamports)
        || neutral_sink_after
            .lamports
            .checked_sub(prepared.neutral_lamports_before)
            != Some(closed.neutral_surplus_lamports)
    {
        return Err(Error::CloseDeltaMismatch);
    }
    Ok(AcceptedSeriesVaultRentDispositionV2 {
        terminal_receipt: closed.terminal_receipt,
        series_plan: closed.series_plan,
        component: closed.component,
        closed_vault: closed.closed_vault,
        component_lamports_after: component_lamport_vault_after.lamports,
        refunded_rent_principal_lamports: closed.refundable_rent_principal_lamports,
        neutral_surplus_lamports: closed.neutral_surplus_lamports,
    })
}

fn admit_system_lamport_vault_v2(view: RuntimeLamportAccountViewV2<'_>, exact: Id) -> Result<()> {
    admit_system_lamport_account_v2(view, exact, true)
}

fn admit_system_lamport_destination_v2(
    view: RuntimeLamportAccountViewV2<'_>,
    exact: Id,
) -> Result<()> {
    admit_system_lamport_account_v2(view, exact, true)
}

fn admit_system_lamport_account_v2(
    view: RuntimeLamportAccountViewV2<'_>,
    exact: Id,
    writable: bool,
) -> Result<()> {
    exact.require_live()?;
    if view.account.key != exact
        || view.account.owner_program != SOLANA_SYSTEM_PROGRAM
        || !view.account.data.is_empty()
        || view.account.is_writable != writable
        || view.account.is_signer
        || view.account.executable
    {
        return Err(Error::WrongAccountRole);
    }
    Ok(())
}
