// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{
    BoundRealmCollateralV2, CustodyBindingV2, CustodyTransferKindV2, Error, Id, PositionCashV2,
    Result, TokenAccountRoleV2, TransferAuthorityKindV2, TransferAuthorityV2, TransferEndpointV2,
    TransferRequestV2,
};

/// Number of ordered collateral funding compartments in a Series.
pub const SERIES_COLLATERAL_COMPONENT_COUNT_V2: u16 = 5;
/// Canonical one-shot Series activation generation.
pub const SERIES_ACTIVATION_GENERATION_V2: u64 = 1;

/// Immutable identities joining Series funding to Realm-selected collateral.
///
/// The live SBF adapter must construct this only after authenticating the exact
/// accounts and canonical bodies named here. The funding-state account remains
/// the sole owner of mutable principal and donation facts; this join does not
/// duplicate them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesCollateralFundingJoinV2 {
    /// Immutable Realm selected by the Series plan.
    pub realm: Id,
    /// Immutable collateral Profile selected by the Realm.
    pub profile: Id,
    /// Exact SeriesPlanV5 identity owning the five compartment names.
    pub series_plan: Id,
    /// Exact immutable SeriesFundingTermsV2 identity.
    pub funding_terms: Id,
    /// Exact authenticated SeriesFunding state-account identity.
    pub funding_state_account: Id,
    /// Exact quote identity from which funding quantities were derived.
    pub quote: Id,
    /// Canonical SeriesFunding authority PDA owning collateral vaults.
    pub funding_authority: Id,
    /// Terms-bound collateral-principal refund token account.
    pub collateral_principal_refund_token_account: Id,
    /// Terms-bound neutral collateral disposition token account.
    pub neutral_collateral_disposition_token_account: Id,
    /// Terms-bound payer lamport refund account.
    pub payer_lamport_refund: Id,
    /// Registry-owned neutral lamport sink.
    pub neutral_lamport_sink: Id,
}

impl SeriesCollateralFundingJoinV2 {
    /// Join authenticated Series identities to the exact Realm/Profile bound.
    pub fn validate(self, bound: BoundRealmCollateralV2) -> Result<()> {
        for identity in [
            self.realm,
            self.profile,
            self.series_plan,
            self.funding_terms,
            self.funding_state_account,
            self.quote,
            self.funding_authority,
            self.collateral_principal_refund_token_account,
            self.neutral_collateral_disposition_token_account,
            self.payer_lamport_refund,
            self.neutral_lamport_sink,
        ] {
            identity.require_live()?;
        }
        let realm = bound.realm();
        if self.realm != realm.realm
            || self.profile != realm.profile
            || self.series_plan == self.funding_terms
            || self.series_plan == self.funding_state_account
            || self.funding_terms == self.funding_state_account
            || self.funding_authority == self.funding_state_account
            || self.collateral_principal_refund_token_account
                == self.neutral_collateral_disposition_token_account
            || self.payer_lamport_refund == self.neutral_lamport_sink
        {
            return Err(Error::SeriesJoinMismatch);
        }
        Ok(())
    }
}

/// One-shot terminal authorization joined to the exact Series funding graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesCollateralTerminalJoinV2 {
    /// Authenticated funding graph being terminated.
    pub funding: SeriesCollateralFundingJoinV2,
    /// Receipt minted from consumed registry anchor, exact funding account/body,
    /// and the pure terminal projection.
    pub terminal_receipt: Id,
}

impl SeriesCollateralTerminalJoinV2 {
    /// Validate the funding graph and non-replayable terminal receipt identity.
    pub fn validate(self, bound: BoundRealmCollateralV2) -> Result<()> {
        self.funding.validate(bound)?;
        self.terminal_receipt.require_live()?;
        if self.terminal_receipt == self.funding.funding_state_account
            || self.terminal_receipt == self.funding.funding_terms
            || self.terminal_receipt == self.funding.series_plan
        {
            return Err(Error::SeriesJoinMismatch);
        }
        Ok(())
    }

    /// Canonical activation generation. There is no caller-supplied generation.
    pub const fn activation_generation(self) -> u64 {
        SERIES_ACTIVATION_GENERATION_V2
    }
}

/// Validate one of the five exact Series collateral-vault bindings.
pub fn validate_series_collateral_vault_v2(
    bound: BoundRealmCollateralV2,
    funding: SeriesCollateralFundingJoinV2,
    component: u16,
    vault: CustodyBindingV2,
) -> Result<()> {
    funding.validate(bound)?;
    vault.validate(bound.release())?;
    if component == 0
        || component > SERIES_COLLATERAL_COMPONENT_COUNT_V2
        || vault.semantic_owner != funding.series_plan
        || vault.owner_authority != funding.funding_authority
        || vault.compartment != component
    {
        return Err(Error::SeriesJoinMismatch);
    }
    Ok(())
}

/// Build a signer-funded Holder → Series compartment transfer request.
pub fn series_segregated_funding_request_v2(
    bound: BoundRealmCollateralV2,
    funding: SeriesCollateralFundingJoinV2,
    component: u16,
    vault: CustodyBindingV2,
    payer_token_owner: Id,
    payer_authority: TransferAuthorityV2,
    amount_atoms: u64,
) -> Result<TransferRequestV2> {
    validate_series_collateral_vault_v2(bound, funding, component, vault)?;
    payer_token_owner.require_live()?;
    payer_authority.validate()?;
    if amount_atoms == 0
        || payer_authority.kind != TransferAuthorityKindV2::TransactionSigner
        || payer_authority.address != payer_token_owner
    {
        return Err(Error::SeriesJoinMismatch);
    }
    Ok(TransferRequestV2 {
        kind: CustodyTransferKindV2::SegregatedFunding,
        source: TransferEndpointV2 {
            token_role: TokenAccountRoleV2::Holder {
                owner: payer_token_owner,
            },
            semantic_owner: payer_token_owner,
            compartment: 0,
        },
        destination: TransferEndpointV2 {
            token_role: TokenAccountRoleV2::SegregatedVault(vault),
            semantic_owner: funding.series_plan,
            compartment: component,
        },
        authority: payer_authority,
        amount_atoms,
        position_cash: None::<PositionCashV2>,
        locked_collateral_atoms: 0,
    })
}

/// Build a terminal Series principal-refund transfer to its exact Terms account.
pub fn series_principal_refund_request_v2(
    bound: BoundRealmCollateralV2,
    terminal: SeriesCollateralTerminalJoinV2,
    component: u16,
    vault: CustodyBindingV2,
    authority: TransferAuthorityV2,
    amount_atoms: u64,
) -> Result<TransferRequestV2> {
    terminal_transfer_request_v2(
        bound,
        terminal,
        component,
        vault,
        authority,
        amount_atoms,
        CustodyTransferKindV2::PrincipalRefund,
        terminal.funding.collateral_principal_refund_token_account,
    )
}

/// Build a terminal Series donation disposition to its exact neutral account.
pub fn series_donation_disposition_request_v2(
    bound: BoundRealmCollateralV2,
    terminal: SeriesCollateralTerminalJoinV2,
    component: u16,
    vault: CustodyBindingV2,
    authority: TransferAuthorityV2,
    amount_atoms: u64,
) -> Result<TransferRequestV2> {
    terminal_transfer_request_v2(
        bound,
        terminal,
        component,
        vault,
        authority,
        amount_atoms,
        CustodyTransferKindV2::DonationDisposition,
        terminal
            .funding
            .neutral_collateral_disposition_token_account,
    )
}

fn terminal_transfer_request_v2(
    bound: BoundRealmCollateralV2,
    terminal: SeriesCollateralTerminalJoinV2,
    component: u16,
    vault: CustodyBindingV2,
    authority: TransferAuthorityV2,
    amount_atoms: u64,
    kind: CustodyTransferKindV2,
    exact_destination: Id,
) -> Result<TransferRequestV2> {
    terminal.validate(bound)?;
    validate_series_collateral_vault_v2(bound, terminal.funding, component, vault)?;
    authority.validate()?;
    if amount_atoms == 0
        || authority.kind != TransferAuthorityKindV2::ProgramDerived
        || authority.address != terminal.funding.funding_authority
    {
        return Err(Error::SeriesJoinMismatch);
    }
    Ok(TransferRequestV2 {
        kind,
        source: TransferEndpointV2 {
            token_role: TokenAccountRoleV2::SegregatedVault(vault),
            semantic_owner: terminal.funding.series_plan,
            compartment: component,
        },
        destination: TransferEndpointV2 {
            token_role: TokenAccountRoleV2::ReceiveOnly {
                account: exact_destination,
            },
            semantic_owner: terminal.funding.funding_terms,
            compartment: 0,
        },
        authority,
        amount_atoms,
        position_cash: None::<PositionCashV2>,
        locked_collateral_atoms: 0,
    })
}
