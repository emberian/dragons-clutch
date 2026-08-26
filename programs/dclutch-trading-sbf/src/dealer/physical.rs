//! Exact Dealer child requests and receipt/postcondition verification.
//!
//! This module is the physical boundary behind the one Trading role. It turns
//! an already accepted Dealer transition into runtime-width Claims and
//! distinct-owner Custody packets. Nothing is committed here: the composing
//! outer invokes the current Registry-authenticated children, verifies every
//! receipt and postcondition, and writes the Dealer tail last.

use dclutch_claims_svm::{
    CLAIMS_PLAN_HEADER_BYTES_V1, CallerRole as ClaimsCallerRole, ClaimsAction, ClaimsPlanV1,
    ClaimsReceiptV1, NO_POSITION_REVISION,
};
use dclutch_custody_contract::{
    CallerRoleV1, CompartmentV1, ContextV1, CustodyReceiptV1, CustodyRequestV1, OperationV1,
};
use dclutch_dealer_codec::{
    ClaimAction, CustodyRole, MAX_CUSTODY_TRANSFERS, MAX_OUTCOMES, Plan, Policy, Side,
};
use solana_program::hash::hash;

use super::DealerTransitionProjectionV2;

/// Exact maximum runtime-width Claims packet admitted by the current measured profile.
pub const MAX_DEALER_CLAIMS_PACKET_BYTES_V2: usize = CLAIMS_PLAN_HEADER_BYTES_V1 + MAX_OUTCOMES * 8;

/// Stable refusal from Dealer physical planning or acknowledgement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerPhysicalError {
    /// A required identity or digest was zero.
    ZeroIdentity,
    /// A physical endpoint did not match its immutable economic role.
    EndpointMismatch,
    /// Runtime outcome width or vector bytes did not join the Product width.
    WidthMismatch,
    /// Checked balance, replay, or revision arithmetic failed.
    Arithmetic,
    /// The semantic plan exceeded the fixed effect capacity.
    Capacity,
    /// Claims request construction or acknowledgement refused.
    Claims,
    /// Custody request construction or acknowledgement refused.
    Custody,
    /// A poststate differed from the exact accepted transition.
    Postcondition,
}

/// Result alias for the Dealer physical boundary.
pub type Result<T> = core::result::Result<T, DealerPhysicalError>;

/// One exact collateral endpoint and its authenticated pre-balance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CollateralEndpointV2 {
    /// Exact token account identity.
    pub account: [u8; 32],
    /// External token owner, or zero for a Custody-owned vault.
    pub external_owner: [u8; 32],
    /// Economic compartment owned by Custody's ABI.
    pub compartment: CompartmentV1,
    /// Vault namespace, or zero for an external token account.
    pub vault_context: [u8; 32],
    /// Authenticated token balance before the outer action.
    pub balance: u64,
}

impl CollateralEndpointV2 {
    fn validate(self) -> Result<()> {
        if is_zero(self.account) {
            return Err(DealerPhysicalError::ZeroIdentity);
        }
        let external = self.compartment == CompartmentV1::External;
        if external != !is_zero(self.external_owner) || external != is_zero(self.vault_context) {
            return Err(DealerPhysicalError::EndpointMismatch);
        }
        Ok(())
    }
}

/// Authenticated common physical coordinates for one Dealer transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerPhysicalContextV2 {
    /// Current Trading program.
    pub trading_program: [u8; 32],
    /// Current Claims program.
    pub claims_program: [u8; 32],
    /// Current Custody program.
    pub custody_program: [u8; 32],
    /// Immutable Registry release set.
    pub release_set: [u8; 32],
    /// Canonical Core Market.
    pub market: [u8; 32],
    /// Immutable Realm.
    pub realm: [u8; 32],
    /// Canonical Trading child root and custody replay namespace.
    pub child_root: [u8; 32],
    /// Realm-selected collateral mint.
    pub mint: [u8; 32],
    /// Realm-selected Token or Token-2022 program.
    pub token_program: [u8; 32],
    /// Digest of the complete canonical parent Trading packet.
    pub parent_request_digest: [u8; 32],
    /// Exact Market generation.
    pub generation: u64,
    /// Claims aggregate revision before this transition.
    pub claims_market_revision: u64,
    /// Internal Dealer Claims Position revision before this transition.
    pub dealer_position_revision: u64,
    /// External Dealer capital-owner Claims Position revision.
    pub dealer_owner_position_revision: u64,
    /// Taker Claims Position owner, zero when the semantic plan has no taker.
    pub taker_owner: [u8; 32],
    /// Taker Claims Position revision, absent sentinel when there is no taker.
    pub taker_position_revision: u64,
    /// Custody replay revision before the first emitted transfer.
    pub custody_replay_revision: u64,
}

impl DealerPhysicalContextV2 {
    fn validate(self, policy: Policy) -> Result<()> {
        for identity in [
            self.trading_program,
            self.claims_program,
            self.custody_program,
            self.release_set,
            self.market,
            self.realm,
            self.child_root,
            self.mint,
            self.token_program,
            self.parent_request_digest,
        ] {
            if is_zero(identity) {
                return Err(DealerPhysicalError::ZeroIdentity);
            }
        }
        if self.release_set != policy.release_set_id || self.market != policy.market_id {
            return Err(DealerPhysicalError::EndpointMismatch);
        }
        if is_zero(self.taker_owner) != (self.taker_position_revision == NO_POSITION_REVISION) {
            return Err(DealerPhysicalError::EndpointMismatch);
        }
        Ok(())
    }
}

/// All exact collateral roles observed for one action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerCollateralFrameV2 {
    /// Internal TradingPrincipal vault.
    pub dealer_quote: CollateralEndpointV2,
    /// Taker external quote account.
    pub taker_quote: CollateralEndpointV2,
    /// Internal realized-fee vault.
    pub fee_vault: CollateralEndpointV2,
    /// Internal funded-liveness vault.
    pub liveness_vault: CollateralEndpointV2,
    /// Permissionless executor external account.
    pub executor: CollateralEndpointV2,
    /// Dealer capital-owner external account.
    pub dealer_owner: CollateralEndpointV2,
    /// Terminal unwind-recipient external account.
    pub unwind_recipient: CollateralEndpointV2,
    /// Realized-fee-recipient external account.
    pub fee_recipient: CollateralEndpointV2,
    /// Market HoardPrincipal vault.
    pub market_hoard: CollateralEndpointV2,
}

impl DealerCollateralFrameV2 {
    fn endpoints(self) -> [CollateralEndpointV2; 9] {
        [
            self.dealer_quote,
            self.taker_quote,
            self.fee_vault,
            self.liveness_vault,
            self.executor,
            self.dealer_owner,
            self.unwind_recipient,
            self.fee_recipient,
            self.market_hoard,
        ]
    }

    fn validate(self, policy: Policy, context: DealerPhysicalContextV2) -> Result<()> {
        let endpoints = self.endpoints();
        for endpoint in endpoints {
            endpoint.validate()?;
        }
        for (index, endpoint) in endpoints.iter().enumerate() {
            for other in endpoints.iter().skip(index + 1) {
                if endpoint.account == other.account && endpoint != other {
                    return Err(DealerPhysicalError::EndpointMismatch);
                }
            }
        }
        for (endpoint, compartment, vault_context) in [
            (
                self.dealer_quote,
                CompartmentV1::TradingPrincipal,
                context.child_root,
            ),
            (self.fee_vault, CompartmentV1::FeeVault, context.child_root),
            (
                self.liveness_vault,
                CompartmentV1::LivenessVault,
                context.child_root,
            ),
            (
                self.market_hoard,
                CompartmentV1::HoardPrincipal,
                context.market,
            ),
        ] {
            if endpoint.compartment != compartment
                || endpoint.vault_context != vault_context
                || !is_zero(endpoint.external_owner)
            {
                return Err(DealerPhysicalError::EndpointMismatch);
            }
        }
        for (endpoint, owner) in [
            (self.dealer_owner, policy.dealer_id),
            (self.unwind_recipient, policy.unwind_recipient_id),
            (self.fee_recipient, policy.fee_recipient_id),
        ] {
            if endpoint.compartment != CompartmentV1::External || endpoint.external_owner != owner {
                return Err(DealerPhysicalError::EndpointMismatch);
            }
        }
        for endpoint in [self.taker_quote, self.executor] {
            if endpoint.compartment != CompartmentV1::External {
                return Err(DealerPhysicalError::EndpointMismatch);
            }
        }
        Ok(())
    }

    fn role(self, role: CustodyRole) -> CollateralEndpointV2 {
        match role {
            CustodyRole::DealerQuote => self.dealer_quote,
            CustodyRole::TakerQuote => self.taker_quote,
            CustodyRole::FeeVault => self.fee_vault,
            CustodyRole::LivenessVault => self.liveness_vault,
            CustodyRole::Executor => self.executor,
            CustodyRole::DealerOwner => self.dealer_owner,
            CustodyRole::UnwindRecipient => self.unwind_recipient,
            CustodyRole::FeeRecipient => self.fee_recipient,
            CustodyRole::MarketHoard => self.market_hoard,
        }
    }
}

/// One owned runtime-width Claims child packet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerClaimsPacketV2 {
    bytes: [u8; MAX_DEALER_CLAIMS_PACKET_BYTES_V2],
    len: usize,
    expected_payout: u64,
}

impl DealerClaimsPacketV2 {
    /// Borrow the exact packet bytes, excluding inactive capacity.
    pub fn as_bytes(&self) -> &[u8] {
        self.bytes.get(..self.len).unwrap_or(&[])
    }

    /// Decode the exact borrowed Claims plan.
    pub fn decode(&self) -> Result<ClaimsPlanV1<'_>> {
        ClaimsPlanV1::decode(self.as_bytes()).map_err(|_| DealerPhysicalError::Claims)
    }

    /// Return the payout committed by the accepted Dealer transition.
    ///
    /// Transfer and liquidity packets always commit zero. Terminal redemption
    /// commits the exact semantic payout, so a caller cannot choose the value
    /// accepted from the Claims receipt.
    pub const fn expected_payout(self) -> u64 {
        self.expected_payout
    }
}

/// One exact Custody request plus expected token balances after it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerCustodyEffectV2 {
    /// Exact child request.
    pub request: CustodyRequestV1,
    /// Expected source balance after the transfer.
    pub source_after: u64,
    /// Expected destination balance after the transfer.
    pub destination_after: u64,
}

/// Complete child-effect plan prepared before any CPI or write.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerPhysicalPlanV2 {
    claims: Option<DealerClaimsPacketV2>,
    custody: [Option<DealerCustodyEffectV2>; MAX_CUSTODY_TRANSFERS],
    custody_count: u8,
    post_balances: [u64; 9],
}

impl DealerPhysicalPlanV2 {
    /// Return the optional exact Claims child packet.
    pub const fn claims(self) -> Option<DealerClaimsPacketV2> {
        self.claims
    }

    /// Borrow the ordered Custody effects.
    pub fn custody(&self) -> &[Option<DealerCustodyEffectV2>; MAX_CUSTODY_TRANSFERS] {
        &self.custody
    }

    /// Return the exact number of active Custody requests.
    pub const fn custody_count(self) -> u8 {
        self.custody_count
    }

    /// Return expected post-balance for one semantic collateral role.
    pub fn expected_balance(
        self,
        frame: DealerCollateralFrameV2,
        role: CustodyRole,
    ) -> Result<u64> {
        let account = frame.role(role).account;
        let endpoints = frame.endpoints();
        endpoints
            .iter()
            .position(|endpoint| endpoint.account == account)
            .and_then(|index| self.post_balances.get(index).copied())
            .ok_or(DealerPhysicalError::EndpointMismatch)
    }
}

/// Compile one accepted semantic transition into exact child packets.
pub fn prepare_physical_v2(
    policy: Policy,
    context: DealerPhysicalContextV2,
    frame: DealerCollateralFrameV2,
    transition: DealerTransitionProjectionV2,
) -> Result<DealerPhysicalPlanV2> {
    context.validate(policy)?;
    frame.validate(policy, context)?;
    let claims = compile_claims(policy, context, transition.plan())?;
    let mut balances = frame.endpoints().map(|endpoint| endpoint.balance);
    let mut custody = [None; MAX_CUSTODY_TRANSFERS];
    let mut custody_count = 0_usize;
    for transfer in transition.plan().custody.into_iter().flatten() {
        let source = frame.role(transfer.source);
        let destination = frame.role(transfer.destination);
        let source_index = find_account(frame, source.account)?;
        let destination_index = find_account(frame, destination.account)?;
        if source.account == destination.account {
            return Err(DealerPhysicalError::EndpointMismatch);
        }
        let source_before = *balances
            .get(source_index)
            .ok_or(DealerPhysicalError::EndpointMismatch)?;
        let destination_before = *balances
            .get(destination_index)
            .ok_or(DealerPhysicalError::EndpointMismatch)?;
        let source_after = source_before
            .checked_sub(transfer.amount)
            .ok_or(DealerPhysicalError::Arithmetic)?;
        let destination_after = destination_before
            .checked_add(transfer.amount)
            .ok_or(DealerPhysicalError::Arithmetic)?;
        set_alias_balances(&mut balances, frame, source.account, source_after);
        set_alias_balances(&mut balances, frame, destination.account, destination_after);
        let transfer_index =
            u16::try_from(custody_count).map_err(|_| DealerPhysicalError::Capacity)?;
        let expected_revision = context
            .custody_replay_revision
            .checked_add(u64::try_from(custody_count).map_err(|_| DealerPhysicalError::Capacity)?)
            .ok_or(DealerPhysicalError::Arithmetic)?;
        let request = custody_transfer_request(
            context,
            transfer_index,
            expected_revision,
            source,
            destination,
            transfer.amount,
        )?;
        *custody
            .get_mut(custody_count)
            .ok_or(DealerPhysicalError::Capacity)? = Some(DealerCustodyEffectV2 {
            request,
            source_after,
            destination_after,
        });
        custody_count = custody_count
            .checked_add(1)
            .ok_or(DealerPhysicalError::Capacity)?;
    }
    Ok(DealerPhysicalPlanV2 {
        claims,
        custody,
        custody_count: u8::try_from(custody_count).map_err(|_| DealerPhysicalError::Capacity)?,
        post_balances: balances,
    })
}

fn compile_claims(
    policy: Policy,
    context: DealerPhysicalContextV2,
    plan: Plan,
) -> Result<Option<DealerClaimsPacketV2>> {
    let (
        action,
        source,
        destination,
        source_revision,
        destination_revision,
        payout,
        outcome,
        amount,
    ) = match plan.claim {
        ClaimAction::None => return Ok(None),
        ClaimAction::Transfer {
            side,
            outcome,
            quantity,
        } => match side {
            Side::TakerBuys => (
                ClaimsAction::TransferNative,
                context.child_root,
                context.taker_owner,
                context.dealer_position_revision,
                context.taker_position_revision,
                0,
                outcome,
                quantity,
            ),
            Side::TakerSells => (
                ClaimsAction::TransferNative,
                context.taker_owner,
                context.child_root,
                context.taker_position_revision,
                context.dealer_position_revision,
                0,
                outcome,
                quantity,
            ),
        },
        ClaimAction::Redeem {
            outcome,
            quantity,
            payout,
        } => (
            ClaimsAction::RedeemNativeTerminal,
            context.child_root,
            [0; 32],
            context.dealer_position_revision,
            NO_POSITION_REVISION,
            payout,
            outcome,
            quantity,
        ),
        ClaimAction::AdjustLiquidity {
            add,
            outcome,
            quantity,
        } => {
            if add {
                (
                    ClaimsAction::TransferNative,
                    policy.dealer_id,
                    context.child_root,
                    context.dealer_owner_position_revision,
                    context.dealer_position_revision,
                    0,
                    outcome,
                    quantity,
                )
            } else {
                (
                    ClaimsAction::TransferNative,
                    context.child_root,
                    policy.dealer_id,
                    context.dealer_position_revision,
                    context.dealer_owner_position_revision,
                    0,
                    outcome,
                    quantity,
                )
            }
        }
    };
    if usize::from(outcome) >= usize::from(policy.outcome_count)
        || is_zero(source)
        || (action == ClaimsAction::TransferNative && is_zero(destination))
    {
        return Err(DealerPhysicalError::Claims);
    }
    let count = usize::from(policy.outcome_count);
    let mut quantities = [0_u8; MAX_OUTCOMES * 8];
    let start = usize::from(outcome)
        .checked_mul(8)
        .ok_or(DealerPhysicalError::WidthMismatch)?;
    quantities
        .get_mut(start..start + 8)
        .ok_or(DealerPhysicalError::WidthMismatch)?
        .copy_from_slice(&amount.to_le_bytes());
    let quantity_bytes = quantities
        .get(
            ..count
                .checked_mul(8)
                .ok_or(DealerPhysicalError::WidthMismatch)?,
        )
        .ok_or(DealerPhysicalError::WidthMismatch)?;
    let request_id = context.parent_request_digest;
    let claims_plan = ClaimsPlanV1::new(
        action,
        ClaimsCallerRole::Trading,
        context.release_set,
        context.market,
        request_id,
        source,
        destination,
        context.claims_market_revision,
        source_revision,
        destination_revision,
        u32::from(policy.outcome_count),
        quantity_bytes,
    )
    .map_err(|_| DealerPhysicalError::Claims)?;
    let len = CLAIMS_PLAN_HEADER_BYTES_V1
        .checked_add(quantity_bytes.len())
        .ok_or(DealerPhysicalError::WidthMismatch)?;
    let mut bytes = [0_u8; MAX_DEALER_CLAIMS_PACKET_BYTES_V2];
    claims_plan
        .encode_into(
            bytes
                .get_mut(..len)
                .ok_or(DealerPhysicalError::WidthMismatch)?,
        )
        .map_err(|_| DealerPhysicalError::Claims)?;
    Ok(Some(DealerClaimsPacketV2 {
        bytes,
        len,
        expected_payout: payout,
    }))
}

fn custody_transfer_request(
    context: DealerPhysicalContextV2,
    transfer_index: u16,
    expected_revision: u64,
    source: CollateralEndpointV2,
    destination: CollateralEndpointV2,
    amount: u64,
) -> Result<CustodyRequestV1> {
    let request = CustodyRequestV1 {
        operation: OperationV1::Transfer,
        caller_role: CallerRoleV1::Trading,
        source_compartment: source.compartment,
        destination_compartment: destination.compartment,
        release_set: context.release_set,
        market: context.market,
        realm: context.realm,
        context: context.child_root,
        caller_program: context.trading_program,
        semantic: ContextV1 {
            candidate: [0; 32],
            source_owner: source.external_owner,
            destination_owner: destination.external_owner,
            order: [0; 32],
            parent_request_digest: context.parent_request_digest,
            order_nonce: context.custody_replay_revision,
            generation: context.generation,
            page_index: 0,
            execution_index: 0,
            transfer_index,
        },
        source: source.account,
        destination: destination.account,
        source_vault_context: source.vault_context,
        destination_vault_context: destination.vault_context,
        mint: context.mint,
        token_program: context.token_program,
        payer: [0; 32],
        rent_refund: [0; 32],
        expected_revision,
        resulting_revision: expected_revision
            .checked_add(1)
            .ok_or(DealerPhysicalError::Arithmetic)?,
        amount,
        rent_lamports: 0,
    };
    request
        .validate()
        .map_err(|_| DealerPhysicalError::Custody)?;
    Ok(request)
}

/// Verify one immediate Claims acknowledgement against the exact prepared packet.
pub fn verify_claims_receipt_v2(
    context: DealerPhysicalContextV2,
    packet: DealerClaimsPacketV2,
    receipt_bytes: &[u8],
) -> Result<()> {
    let plan = packet.decode()?;
    let receipt =
        ClaimsReceiptV1::decode(receipt_bytes).map_err(|_| DealerPhysicalError::Claims)?;
    let source_present = !is_zero(plan.source_owner());
    let destination_present = !is_zero(plan.destination_owner());
    let expected_source = if source_present {
        plan.expected_source_revision()
            .checked_add(1)
            .ok_or(DealerPhysicalError::Arithmetic)?
    } else {
        NO_POSITION_REVISION
    };
    let expected_destination = if destination_present {
        plan.expected_destination_revision()
            .checked_add(1)
            .ok_or(DealerPhysicalError::Arithmetic)?
    } else {
        NO_POSITION_REVISION
    };
    if receipt.caller_role() != ClaimsCallerRole::Trading
        || receipt.action() != plan.action()
        || receipt.release_set_id() != context.release_set
        || receipt.market() != context.market
        || receipt.request_id() != context.parent_request_digest
        || receipt.packet_digest() != hash(packet.as_bytes()).to_bytes()
        || receipt.claims_program() != context.claims_program
        || receipt.pre_market_revision() != context.claims_market_revision
        || receipt.post_market_revision()
            != context
                .claims_market_revision
                .checked_add(1)
                .ok_or(DealerPhysicalError::Arithmetic)?
        || receipt.post_source_revision() != expected_source
        || receipt.post_destination_revision() != expected_destination
        || receipt.payout() != packet.expected_payout()
    {
        return Err(DealerPhysicalError::Postcondition);
    }
    Ok(())
}

/// Verify one immediate Custody acknowledgement and its exact balance deltas.
pub fn verify_custody_receipt_v2(
    effect: DealerCustodyEffectV2,
    receipt_bytes: &[u8],
    poststate_commitment: [u8; 32],
) -> Result<()> {
    let request_bytes = effect
        .request
        .to_bytes()
        .map_err(|_| DealerPhysicalError::Custody)?;
    let request_digest = hash(&request_bytes).to_bytes();
    let receipt =
        CustodyReceiptV1::decode(receipt_bytes).map_err(|_| DealerPhysicalError::Custody)?;
    receipt
        .verify_for(effect.request, request_digest, poststate_commitment)
        .map_err(|_| DealerPhysicalError::Custody)?;
    if receipt.evidence.source_after != effect.source_after
        || receipt.evidence.destination_after != effect.destination_after
    {
        return Err(DealerPhysicalError::Postcondition);
    }
    Ok(())
}

/// Verify final Claims inventory and every alias-aware collateral balance.
pub fn verify_postconditions_v2(
    transition: DealerTransitionProjectionV2,
    plan: DealerPhysicalPlanV2,
    frame: DealerCollateralFrameV2,
    observed_inventory: &[u64],
    observed_balances: [u64; 9],
) -> Result<()> {
    if transition.post_inventory() != observed_inventory || plan.post_balances != observed_balances
    {
        return Err(DealerPhysicalError::Postcondition);
    }
    for (index, endpoint) in frame.endpoints().iter().enumerate() {
        for (other_index, other) in frame.endpoints().iter().enumerate() {
            if endpoint.account == other.account
                && observed_balances.get(index) != observed_balances.get(other_index)
            {
                return Err(DealerPhysicalError::Postcondition);
            }
        }
    }
    Ok(())
}

fn find_account(frame: DealerCollateralFrameV2, account: [u8; 32]) -> Result<usize> {
    frame
        .endpoints()
        .iter()
        .position(|endpoint| endpoint.account == account)
        .ok_or(DealerPhysicalError::EndpointMismatch)
}

fn set_alias_balances(
    balances: &mut [u64; 9],
    frame: DealerCollateralFrameV2,
    account: [u8; 32],
    value: u64,
) {
    for (index, endpoint) in frame.endpoints().iter().enumerate() {
        if endpoint.account == account {
            if let Some(balance) = balances.get_mut(index) {
                *balance = value;
            }
        }
    }
}

fn is_zero(identity: [u8; 32]) -> bool {
    identity.iter().all(|byte| *byte == 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_dealer_codec::{CustodyTransfer, Phase, root_tail::RootTail};

    fn endpoint(
        byte: u8,
        owner: u8,
        compartment: CompartmentV1,
        context: u8,
        balance: u64,
    ) -> CollateralEndpointV2 {
        CollateralEndpointV2 {
            account: [byte; 32],
            external_owner: if compartment == CompartmentV1::External {
                [owner; 32]
            } else {
                [0; 32]
            },
            compartment,
            vault_context: if compartment == CompartmentV1::External {
                [0; 32]
            } else {
                [context; 32]
            },
            balance,
        }
    }

    fn policy() -> Policy {
        Policy {
            market_id: [2; 32],
            release_set_id: [3; 32],
            dealer_id: [4; 32],
            fee_recipient_id: [5; 32],
            unwind_recipient_id: [6; 32],
            outcome_count: 3,
            quote_scale: 100,
            fee_numerator: 1,
            fee_denominator: 100,
            minimum_work_funding: 1,
            replacement_delay: 1,
        }
    }

    fn context() -> DealerPhysicalContextV2 {
        DealerPhysicalContextV2 {
            trading_program: [7; 32],
            claims_program: [8; 32],
            custody_program: [9; 32],
            release_set: [3; 32],
            market: [2; 32],
            realm: [10; 32],
            child_root: [11; 32],
            mint: [12; 32],
            token_program: [13; 32],
            parent_request_digest: [14; 32],
            generation: 2,
            claims_market_revision: 9,
            dealer_position_revision: 4,
            dealer_owner_position_revision: 6,
            taker_owner: [15; 32],
            taker_position_revision: 5,
            custody_replay_revision: 20,
        }
    }

    fn frame() -> DealerCollateralFrameV2 {
        DealerCollateralFrameV2 {
            dealer_quote: endpoint(20, 0, CompartmentV1::TradingPrincipal, 11, 1_000),
            taker_quote: endpoint(21, 15, CompartmentV1::External, 0, 500),
            fee_vault: endpoint(22, 0, CompartmentV1::FeeVault, 11, 0),
            liveness_vault: endpoint(23, 0, CompartmentV1::LivenessVault, 11, 50),
            executor: endpoint(24, 16, CompartmentV1::External, 0, 0),
            dealer_owner: endpoint(25, 4, CompartmentV1::External, 0, 2_000),
            unwind_recipient: endpoint(26, 6, CompartmentV1::External, 0, 0),
            fee_recipient: endpoint(27, 5, CompartmentV1::External, 0, 0),
            market_hoard: endpoint(28, 0, CompartmentV1::HoardPrincipal, 2, 5_000),
        }
    }

    fn transition(
        claim: ClaimAction,
        custody: [Option<CustodyTransfer>; 3],
        inventory: [u64; MAX_OUTCOMES],
    ) -> DealerTransitionProjectionV2 {
        super::super::DealerTransitionProjectionV2::for_physical_test(
            RootTail {
                phase: Phase::Open,
                active_candidate_id: [30; 32],
                pending_candidate_id: [0; 32],
                active_revision: 1,
                pending_revision: 0,
                state_revision: 2,
                buy_used: [0; MAX_OUTCOMES],
                sell_used: [0; MAX_OUTCOMES],
                fee_base: 0,
                active_work_remaining: 50,
                pending_work_funding: 0,
            },
            Plan { claim, custody },
            3,
            inventory,
        )
    }

    #[test]
    fn fill_compiles_distinct_owner_claims_and_sequential_custody_requests() {
        let semantic = transition(
            ClaimAction::Transfer {
                side: Side::TakerBuys,
                outcome: 1,
                quantity: 7,
            },
            [
                Some(CustodyTransfer {
                    source: CustodyRole::TakerQuote,
                    destination: CustodyRole::DealerQuote,
                    amount: 30,
                }),
                Some(CustodyTransfer {
                    source: CustodyRole::TakerQuote,
                    destination: CustodyRole::FeeVault,
                    amount: 2,
                }),
                Some(CustodyTransfer {
                    source: CustodyRole::LivenessVault,
                    destination: CustodyRole::Executor,
                    amount: 1,
                }),
            ],
            [0; MAX_OUTCOMES],
        );
        let plan = prepare_physical_v2(policy(), context(), frame(), semantic).expect("plan");
        let packet = plan.claims().expect("claims");
        let claims = packet.decode().expect("decode claims");
        assert_eq!(
            (claims.source_owner(), claims.destination_owner()),
            ([11; 32], [15; 32])
        );
        assert_eq!(claims.quantity(1), Ok(7));
        assert_eq!(plan.custody_count(), 3);
        let effects = plan.custody();
        assert_eq!(effects[0].expect("first").request.expected_revision, 20);
        assert_eq!(effects[1].expect("second").request.expected_revision, 21);
        assert_eq!(effects[1].expect("second").source_after, 468);
        assert_eq!(plan.expected_balance(frame(), CustodyRole::FeeVault), Ok(2));
    }

    #[test]
    fn liquidity_claims_use_external_dealer_and_internal_child_root_positions() {
        let semantic = transition(
            ClaimAction::AdjustLiquidity {
                add: true,
                outcome: 2,
                quantity: 9,
            },
            [None; 3],
            [0; MAX_OUTCOMES],
        );
        let plan = prepare_physical_v2(policy(), context(), frame(), semantic).expect("plan");
        let packet = plan.claims().expect("claims");
        let claims = packet.decode().expect("claims decode");
        assert_eq!(
            (claims.source_owner(), claims.destination_owner()),
            ([4; 32], [11; 32])
        );
        assert_eq!(claims.quantity(2), Ok(9));
    }

    #[test]
    fn substituted_compartment_owner_and_late_underflow_refuse_before_plan() {
        let semantic = transition(
            ClaimAction::None,
            [
                Some(CustodyTransfer {
                    source: CustodyRole::TakerQuote,
                    destination: CustodyRole::DealerQuote,
                    amount: 501,
                }),
                None,
                None,
            ],
            [0; MAX_OUTCOMES],
        );
        assert_eq!(
            prepare_physical_v2(policy(), context(), frame(), semantic),
            Err(DealerPhysicalError::Arithmetic)
        );
        let mut hostile = frame();
        hostile.fee_recipient.external_owner = [99; 32];
        assert_eq!(
            prepare_physical_v2(policy(), context(), hostile, semantic),
            Err(DealerPhysicalError::EndpointMismatch)
        );
    }
}
