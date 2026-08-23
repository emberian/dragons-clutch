// SPDX-License-Identifier: AGPL-3.0-or-later

//! Atomic Realm Hoard transfer bridge for canonical General Position V3.
//!
//! The base transfer contract retains its small `PositionCashV2` carrier for
//! legacy callers, but this module is the canonical Position V3 path. It
//! derives that carrier from an authenticated full Position body, prepares the
//! exact checked token CPI, and returns the complete Position postimage only
//! after source, destination, Hoard, and mint-supply reloads all match.

use clutch_retirement::{
    GeneralPositionProjectionV3, PositionAccountV3, PositionLifecycleV3, PositionV3Fields,
    PositionV3Sha256Backend,
};
use sha2::{Digest, Sha256};

use crate::{
    accept_collateral_transfer_v2, prepare_collateral_transfer_v2, AcceptedCollateralTransferV2,
    BoundCollateralProfileV2, CheckedTransferCpiV2, CustodyTransferKindV2, Error, Id,
    PositionCashV2, PreparedCollateralTransferV2, Result, RuntimeAccountViewV2,
    TransferAuthorityV2, TransferEndpointV2, TransferRequestV2,
};

/// Content domain for the exact accepted Hoard/Position transition receipt.
pub const POSITION_COLLATERAL_TRANSFER_RECEIPT_DOMAIN_V3: &[u8] =
    b"dragons-clutch/position-v3/collateral-transfer-receipt/v1\0";

/// V3-only transfer request without an independently supplied cash DTO.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionCollateralTransferRequestV3 {
    /// Holder deposit or holder withdrawal; no other kind is admitted.
    pub kind: CustodyTransferKindV2,
    /// Exact source endpoint.
    pub source: TransferEndpointV2,
    /// Exact destination endpoint.
    pub destination: TransferEndpointV2,
    /// Exact source token authority.
    pub authority: TransferAuthorityV2,
    /// Raw collateral atoms; decimals never rescale this value.
    pub amount_atoms: u64,
    /// Locked complete-set principal that the post-CPI Hoard must cover.
    pub locked_collateral_atoms: u64,
}

/// Prepared token CPI and complete canonical Position V3 postimage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedPositionCollateralTransferV3 {
    inner: PreparedCollateralTransferV2,
    position_account_id: Id,
    position_pre_semantic_id: Id,
    position_post_semantic_id: Id,
    position_replay_account_id: Id,
    position_post: PositionAccountV3,
}

impl PreparedPositionCollateralTransferV3 {
    /// Sole external token invocation this transition permits.
    pub const fn cpi(self) -> CheckedTransferCpiV2 {
        self.inner.cpi()
    }

    /// Complete Position V3 postimage, publishable only after acceptance.
    pub const fn position_post(self) -> PositionAccountV3 {
        self.position_post
    }

    /// Exact canonical Position account being mutated.
    pub const fn position_account_id(self) -> Id {
        self.position_account_id
    }

    /// Exact current-generation Replay that must order this Position mutation.
    pub const fn position_replay_account_id(self) -> Id {
        self.position_replay_account_id
    }
}

/// Accepted exact token delta plus the only permitted Position V3 postimage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AcceptedPositionCollateralTransferV3 {
    /// Accepted token custody movement and exact reloaded balances.
    custody: AcceptedCollateralTransferV2,
    /// Exact canonical Position V3 account.
    position_account_id: Id,
    /// Position semantic identity authenticated before CPI.
    position_pre_semantic_id: Id,
    /// Position semantic identity that must be published after CPI.
    position_post_semantic_id: Id,
    /// Current-generation Replay that must commit the same transition.
    position_replay_account_id: Id,
    /// Complete canonical Position postimage.
    position_post: PositionAccountV3,
}

impl AcceptedPositionCollateralTransferV3 {
    /// Exact accepted transfer kind.
    pub const fn kind(self) -> CustodyTransferKindV2 {
        self.custody.kind
    }

    /// Exact raw collateral atoms moved.
    pub const fn amount_atoms(self) -> u64 {
        self.custody.amount_atoms
    }

    /// Exact canonical Position account.
    pub const fn position_account_id(self) -> Id {
        self.position_account_id
    }

    /// Exact Position semantic identity before the CPI-backed mutation.
    pub const fn position_pre_semantic_id(self) -> Id {
        self.position_pre_semantic_id
    }

    /// Exact Position semantic identity after the CPI-backed mutation.
    pub const fn position_post_semantic_id(self) -> Id {
        self.position_post_semantic_id
    }

    /// Exact Position Replay that must commit this receipt.
    pub const fn position_replay_account_id(self) -> Id {
        self.position_replay_account_id
    }

    /// Complete canonical Position postimage.
    pub const fn position_post(self) -> PositionAccountV3 {
        self.position_post
    }

    /// Exact visible Hoard balance after the accepted CPI.
    pub const fn hoard_atoms_after(self) -> u64 {
        match self.custody.hoard_atoms_after {
            Some(value) => value,
            None => 0,
        }
    }

    /// Canonical receipt digest for a Position Replay intent.
    pub fn receipt_id(&self) -> Result<Id> {
        let kind = [transfer_kind_byte(self.custody.kind)];
        let amount = self.custody.amount_atoms.to_le_bytes();
        let source_after = self.custody.source_atoms_after.to_le_bytes();
        let destination_after = self.custody.destination_atoms_after.to_le_bytes();
        let mint_supply = self.custody.mint_supply_after.to_le_bytes();
        let hoard_after = self
            .custody
            .hoard_atoms_after
            .ok_or(Error::MismatchedBinding)?
            .to_le_bytes();
        let receipt = crate::digest(
            POSITION_COLLATERAL_TRANSFER_RECEIPT_DOMAIN_V3,
            &[
                &self.position_account_id.bytes(),
                &self.position_replay_account_id.bytes(),
                &self.position_pre_semantic_id.bytes(),
                &self.position_post_semantic_id.bytes(),
                &kind,
                &amount,
                &source_after,
                &destination_after,
                &mint_supply,
                &hoard_after,
            ],
        );
        receipt.require_live()?;
        Ok(receipt)
    }
}

/// Prepare a Holder↔Hoard CPI directly from canonical General Position V3.
#[allow(clippy::too_many_arguments)]
pub fn prepare_position_collateral_transfer_v3(
    bound: BoundCollateralProfileV2,
    position_account_id: Id,
    position: GeneralPositionProjectionV3,
    request: PositionCollateralTransferRequestV3,
    mint: RuntimeAccountViewV2<'_>,
    source: RuntimeAccountViewV2<'_>,
    destination: RuntimeAccountViewV2<'_>,
) -> Result<PreparedPositionCollateralTransferV3> {
    position_account_id.require_live()?;
    let position_before = position.position();
    position_before
        .validate()
        .map_err(|_| Error::MismatchedBinding)?;
    require_position_collateral_join(bound, position_before)?;
    if position_before.lifecycle() != PositionLifecycleV3::Open
        || position_account_id == source.key
        || position_account_id == destination.key
        || position_account_id == mint.key
    {
        return Err(Error::WrongAccountRole);
    }
    let owner = Id::from_bytes(position_before.owner().bytes());
    match request.kind {
        CustodyTransferKindV2::HolderDeposit => {
            require_holder_endpoint(request.source, owner)?;
            if request.authority.address != owner {
                return Err(Error::WrongAccountRole);
            }
        }
        CustodyTransferKindV2::HolderWithdrawal => {
            require_holder_endpoint(request.destination, owner)?;
        }
        _ => return Err(Error::WrongAccountRole),
    }
    let position_cash = PositionCashV2 {
        cash_atoms: position_before.cash_atoms(),
        reserved_cash_atoms: position_before.reserved_cash_atoms(),
    };
    let inner = prepare_collateral_transfer_v2(
        bound,
        TransferRequestV2 {
            kind: request.kind,
            source: request.source,
            destination: request.destination,
            authority: request.authority,
            amount_atoms: request.amount_atoms,
            position_cash: Some(position_cash),
            locked_collateral_atoms: request.locked_collateral_atoms,
        },
        mint,
        source,
        destination,
    )?;
    let next_cash = match request.kind {
        CustodyTransferKindV2::HolderDeposit => {
            position_cash.after_deposit(request.amount_atoms)?
        }
        CustodyTransferKindV2::HolderWithdrawal => {
            position_cash.after_withdrawal(request.amount_atoms)?
        }
        _ => return Err(Error::WrongAccountRole),
    };
    let position_post = with_position_cash(position_before, next_cash)?;
    let position_pre_semantic_id = position_semantic_id(position_before)?;
    let position_post_semantic_id = position_semantic_id(position_post)?;
    if position_pre_semantic_id == position_post_semantic_id {
        return Err(Error::MismatchedBinding);
    }
    Ok(PreparedPositionCollateralTransferV3 {
        inner,
        position_account_id,
        position_pre_semantic_id,
        position_post_semantic_id,
        position_replay_account_id: Id::from_bytes(position_before.replay_account().bytes()),
        position_post,
    })
}

/// Accept the exact CPI reloads and release the complete Position V3 postimage.
pub fn accept_position_collateral_transfer_v3(
    prepared: PreparedPositionCollateralTransferV3,
    mint_after: RuntimeAccountViewV2<'_>,
    source_after: RuntimeAccountViewV2<'_>,
    destination_after: RuntimeAccountViewV2<'_>,
) -> Result<AcceptedPositionCollateralTransferV3> {
    let custody =
        accept_collateral_transfer_v2(prepared.inner, mint_after, source_after, destination_after)?;
    let next = custody
        .next_position_cash
        .ok_or(Error::PostAdmissionFailed)?;
    if prepared.position_post.cash_atoms() != next.cash_atoms
        || prepared.position_post.reserved_cash_atoms() != next.reserved_cash_atoms
        || custody.hoard_atoms_after.is_none()
    {
        return Err(Error::PostAdmissionFailed);
    }
    Ok(AcceptedPositionCollateralTransferV3 {
        custody,
        position_account_id: prepared.position_account_id,
        position_pre_semantic_id: prepared.position_pre_semantic_id,
        position_post_semantic_id: prepared.position_post_semantic_id,
        position_replay_account_id: prepared.position_replay_account_id,
        position_post: prepared.position_post,
    })
}

fn require_position_collateral_join(
    bound: BoundCollateralProfileV2,
    position: PositionAccountV3,
) -> Result<()> {
    if Id::from_bytes(position.market_instance_id().bytes()) != bound.market().market
        || Id::from_bytes(position.realm_id().bytes()) != bound.market().realm
        || Id::from_bytes(position.collateral_policy_id().bytes()) != bound.policy_id()
        || Id::from_bytes(position.collateral_release_id().bytes()) != bound.release().id()?
    {
        return Err(Error::MismatchedBinding);
    }
    Ok(())
}

fn require_holder_endpoint(endpoint: TransferEndpointV2, owner: Id) -> Result<()> {
    match endpoint.token_role {
        crate::TokenAccountRoleV2::Holder { owner: token_owner }
            if token_owner == owner
                && endpoint.semantic_owner == owner
                && endpoint.compartment == 0 =>
        {
            Ok(())
        }
        _ => Err(Error::WrongAccountRole),
    }
}

fn with_position_cash(
    position: PositionAccountV3,
    cash: PositionCashV2,
) -> Result<PositionAccountV3> {
    let mut fields: PositionV3Fields = position.fields();
    fields.cash_atoms = cash.cash_atoms;
    fields.reserved_cash_atoms = cash.reserved_cash_atoms;
    PositionAccountV3::new(fields).map_err(|_| Error::MismatchedBinding)
}

fn position_semantic_id(position: PositionAccountV3) -> Result<Id> {
    position
        .semantic_id(&CollateralPositionSha256V3)
        .map(|identity| Id::from_bytes(identity.bytes()))
        .map_err(|_| Error::MismatchedBinding)
}

const fn transfer_kind_byte(kind: CustodyTransferKindV2) -> u8 {
    match kind {
        CustodyTransferKindV2::HolderDeposit => 1,
        CustodyTransferKindV2::HolderWithdrawal => 2,
        CustodyTransferKindV2::SegregatedFunding => 3,
        CustodyTransferKindV2::OccurrenceDisbursement => 4,
        CustodyTransferKindV2::PrincipalRefund => 5,
        CustodyTransferKindV2::DonationDisposition => 6,
    }
}

#[derive(Clone, Copy, Debug)]
struct CollateralPositionSha256V3;

impl PositionV3Sha256Backend for CollateralPositionSha256V3 {
    fn sha256(&self, domain: &[u8], body: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(domain);
        hasher.update(body);
        hasher.finalize().into()
    }
}
