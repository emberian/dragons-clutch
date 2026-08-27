//! Record-keyed Custody lifecycle for registered Direct Buy liquidity.
//!
//! A registered Buy is not a revocable external delegate. Registration creates
//! one Custody replay cursor and one `TradingPrincipal` Vault keyed by the live
//! Direct record, then deposits the exact worst-case reserve. Partial fills keep
//! both resources live. Full fill, cancel, expiry, and invalidation return the
//! exact residual, close the Vault, and close the replay cursor before Direct
//! commits the terminal record disposition.

extern crate alloc;

use alloc::{boxed::Box, vec::Vec};
use dclutch_account_profile_contract::lifecycle_v3::StateLifecyclePlanV3;
use dclutch_custody_contract::{
    CallerRoleV1, CompartmentV1, ContextV1, CustodyAuthoritySeedsV1, CustodyReplaySeedsV1,
    CustodyReplayV1, CustodyRequestV1, CustodyVaultSeedsV1, OperationV1,
};
use dclutch_direct_codec::successor::{
    DirectRegisteredIntentV2, MakerReplayRootV1, RegisteredFillCandidateV2,
    RegisteredIntentCreationV2, RegisteredIntentSeedsV2, RegisteredOrdinaryInputV2,
    RegisteredOrdinarySettlementV2, RegisteredRecordAfterFillV2, RegisteredRecordCloseV2,
    RegisteredTerminalResultV2, settle_registered_ordinary_v2,
};
use dclutch_market_core_codec::{CoreMarketViewV1, Phase};
use solana_program::pubkey::Pubkey;

use super::lifecycle::{
    DirectRegisteredCreationLifecycleV3, validate_registered_creation_lifecycle_v3,
    validate_registered_record_close_lifecycle_v3,
};
use super::physical::{
    DirectExternalCollateralV2, DirectExternalDebitV2, DirectPhysicalError, Result,
};

/// Three canonical Custody steps create one funded Buy reserve.
pub const DIRECT_BUY_ESCROW_REGISTRATION_STEPS_V2: usize = 3;
/// At most three steps return and close one terminal Buy reserve.
pub const DIRECT_BUY_ESCROW_TERMINAL_STEPS_V2: usize = 3;
/// At most two settlement transfers, one residual refund, and two closes.
pub const DIRECT_BUY_ESCROW_FILL_STEPS_V2: usize = 5;

/// Fixed release and request facts for one Buy escrow lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectBuyEscrowContextV2 {
    /// Sparse-Core view after exact reference and Registry authentication.
    pub core_market: CoreMarketViewV1,
    /// Descriptor-derived composite Direct capability root.
    pub direct_root: [u8; 32],
    /// Current Registry-selected Trading program.
    pub trading_program: [u8; 32],
    /// SHA-256 of the complete canonical parent Trading request.
    pub parent_request_digest: [u8; 32],
}

impl DirectBuyEscrowContextV2 {
    fn validate(self, terminal: bool) -> Result<()> {
        let phase_valid = if terminal {
            matches!(
                self.core_market.phase(),
                Phase::Open | Phase::Terminal | Phase::Retiring
            )
        } else {
            self.core_market.phase() == Phase::Open
        };
        if self.direct_root == [0; 32]
            || self.trading_program == [0; 32]
            || self.parent_request_digest == [0; 32]
            || self.core_market.release_set().bindings[2]
                .program
                .to_bytes()
                != self.trading_program
            || !phase_valid
        {
            return Err(DirectPhysicalError::Binding);
        }
        Ok(())
    }

    const fn custody_program(self) -> [u8; 32] {
        self.core_market.release_set().bindings[4]
            .program
            .to_bytes()
    }
}

/// Canonical record, replay, Vault, and transfer-authority accounts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectBuyEscrowAccountsV2 {
    /// Trading-owned live Direct record PDA.
    pub record: [u8; 32],
    /// Custody-owned replay PDA keyed by `record`.
    pub replay: [u8; 32],
    /// Custody-owned `TradingPrincipal` Vault PDA keyed by `record`.
    pub vault: [u8; 32],
    /// Custody transfer-authority PDA selected by Market/release set.
    pub custody_authority: [u8; 32],
}

/// Exact prepaid account-creation funding for registration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectBuyEscrowCreationFundingV2 {
    /// System payer for both Custody accounts.
    pub payer: [u8; 32],
    /// Exact replay-account rent principal.
    pub replay_rent_lamports: u64,
    /// Exact token-Vault rent principal.
    pub vault_rent_lamports: u64,
}

/// Complete Buy registration observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectBuyEscrowRegistrationInputV2 {
    /// Sole accepted Direct registration candidate.
    pub creation: RegisteredIntentCreationV2,
    /// Authenticated physical account coordinates.
    pub accounts: DirectBuyEscrowAccountsV2,
    /// Maker source with exact Custody delegation and worst-case allowance.
    pub source: DirectExternalDebitV2,
    /// Exact account-creation principals.
    pub funding: DirectBuyEscrowCreationFundingV2,
    /// Fixed current Core/release/request facts.
    pub context: DirectBuyEscrowContextV2,
    /// Exact generic root/maker/record lifecycle plans.
    pub lifecycle: DirectRegisteredCreationLifecycleV3,
}

/// Three exact Custody requests and their checked terminal token facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectBuyEscrowRegistrationPlanV2 {
    /// Initialize replay, open Vault, deposit reserve.
    pub requests: [CustodyRequestV1; DIRECT_BUY_ESCROW_REGISTRATION_STEPS_V2],
    /// External source balance after the deposit.
    pub source_after: u64,
    /// Delegate allowance after the deposit; exactly zero at registration.
    pub delegated_after: u64,
    /// Record-keyed Vault balance after the deposit.
    pub vault_after: u64,
    /// Exact generic root/maker/record lifecycle plans committed last.
    pub lifecycle: DirectRegisteredCreationLifecycleV3,
}

/// Build the exact funded resting-Buy Custody lifecycle.
pub fn prepare_buy_escrow_registration_v2(
    input: DirectBuyEscrowRegistrationInputV2,
) -> Result<DirectBuyEscrowRegistrationPlanV2> {
    input.context.validate(false)?;
    let record = input.creation.record;
    let reserve = record.reserved_collateral();
    validate_buy_record(input.context, record)?;
    validate_accounts(input.context, record, input.accounts)?;
    validate_registered_creation_lifecycle_v3(
        input.creation,
        input.context.trading_program,
        input.context.direct_root,
        input.lifecycle,
    )?;
    if input.funding.payer == [0; 32]
        || input.funding.replay_rent_lamports == 0
        || input.funding.vault_rent_lamports == 0
        || input.source.account != record.intent().collateral_account
        || input.source.owner != record.maker()
        || input.source.delegate != input.accounts.custody_authority
        || input.source.delegated_amount != reserve
        || input.source.balance < reserve
        || reserve == 0
    {
        return Err(DirectPhysicalError::Binding);
    }
    let source_after = input
        .source
        .balance
        .checked_sub(reserve)
        .ok_or(DirectPhysicalError::Arithmetic)?;
    let rent_refund = record.rent_owner();
    let initialize = request(
        input.context,
        record,
        input.accounts,
        OperationShapeV2::InitializeReplay {
            payer: input.funding.payer,
            rent_refund,
            rent_lamports: input.funding.replay_rent_lamports,
        },
        0,
        0,
    )?;
    let open = request(
        input.context,
        record,
        input.accounts,
        OperationShapeV2::OpenVault {
            payer: input.funding.payer,
            rent_refund,
            rent_lamports: input.funding.vault_rent_lamports,
        },
        1,
        1,
    )?;
    let deposit = request(
        input.context,
        record,
        input.accounts,
        OperationShapeV2::Deposit {
            source: input.source.account,
            source_owner: input.source.owner,
            amount: reserve,
        },
        2,
        2,
    )?;
    Ok(DirectBuyEscrowRegistrationPlanV2 {
        requests: [initialize, open, deposit],
        source_after,
        delegated_after: 0,
        vault_after: reserve,
        lifecycle: input.lifecycle,
    })
}

/// Authenticated token/replay observations for one ordinary registered fill.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectBuyEscrowFillInputV2 {
    /// Complete Direct input; the pure successor remains the settlement owner.
    pub direct: RegisteredOrdinaryInputV2,
    /// Authenticated physical account coordinates for the registered Buy.
    pub accounts: DirectBuyEscrowAccountsV2,
    /// Current exact Custody replay state.
    pub replay: CustodyReplayV1,
    /// Current record-keyed `TradingPrincipal` balance.
    pub vault_balance: u64,
    /// Seller-signed external collateral destination.
    pub seller_destination: DirectExternalCollateralV2,
    /// Immutable config-recipient external token account.
    pub fee_destination: DirectExternalCollateralV2,
    /// Buyer-signed source, used only for a terminal price-improvement refund.
    pub buyer_refund_destination: DirectExternalCollateralV2,
    /// Exact current Vault rent, used only if this fill closes the record.
    pub vault_rent_lamports: u64,
    /// Exact current replay rent, used only if this fill closes the record.
    pub replay_rent_lamports: u64,
    /// Enabled record Close plan exactly for a terminal fill; absent for partial fills.
    pub record_lifecycle: Option<StateLifecyclePlanV3>,
    /// Fixed current Core/release/request facts.
    pub context: DirectBuyEscrowContextV2,
}

/// Exact ordinary Buy escrow requests and preflighted token poststate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectBuyEscrowFillPlanV2 {
    /// Sole checked Direct ordinary settlement.
    pub settlement: Box<RegisteredOrdinarySettlementV2>,
    /// Seller net, combined fee, optional residual refund, optional closes.
    /// The boxed slice is bounded by [`DIRECT_BUY_ESCROW_FILL_STEPS_V2`].
    pub requests: Box<[CustodyRequestV1]>,
    /// Number of positive canonical requests.
    pub request_count: u8,
    /// Record-keyed Vault balance after all token transfers.
    pub vault_after: u64,
    /// Seller external token balance after settlement.
    pub seller_destination_after: u64,
    /// Fee-recipient token balance after settlement.
    pub fee_destination_after: u64,
    /// Buyer signed source balance after an optional residual refund.
    pub buyer_refund_destination_after: u64,
    /// Whether this fill must close Vault and replay before Direct state commits.
    pub closes_escrow: bool,
    /// Enabled generic record Close plan exactly for a terminal fill.
    pub record_lifecycle: Option<StateLifecyclePlanV3>,
}

/// Settle one ordinary registered fill solely from record-keyed Buy custody.
pub fn prepare_buy_escrow_fill_v2(
    input: &DirectBuyEscrowFillInputV2,
) -> Result<Box<DirectBuyEscrowFillPlanV2>> {
    input.context.validate(false)?;
    let buyer_before = input.direct.buyer.record;
    let seller_before = input.direct.seller.record;
    validate_buy_record(input.context, buyer_before)?;
    validate_accounts(input.context, buyer_before, input.accounts)?;
    validate_replay(input.context, buyer_before, input.accounts, input.replay, 1)?;
    if seller_before.intent().side != 0
        || seller_before.intent().market != input.context.core_market.market().to_bytes()
        || seller_before.intent().generation != input.context.core_market.generation()
        || input.vault_balance != buyer_before.reserved_collateral()
        || input.seller_destination.account != seller_before.intent().collateral_account
        || input.seller_destination.owner != seller_before.maker()
        || input.fee_destination.owner != input.direct.execution.config.fee_recipient()
        || input.buyer_refund_destination.account != buyer_before.intent().collateral_account
        || input.buyer_refund_destination.owner != buyer_before.maker()
        || input.accounts.vault == input.seller_destination.account
        || input.accounts.vault == input.fee_destination.account
        || input.accounts.vault == input.buyer_refund_destination.account
        || input.buyer_refund_destination.account == input.seller_destination.account
        || input.buyer_refund_destination.account == input.fee_destination.account
    {
        return Err(DirectPhysicalError::Binding);
    }
    if input.seller_destination.account == input.fee_destination.account
        && input.seller_destination != input.fee_destination
    {
        return Err(DirectPhysicalError::Binding);
    }

    let settlement = Box::new(
        settle_registered_ordinary_v2(input.direct).map_err(|_| DirectPhysicalError::Settlement)?,
    );
    let mut requests = Vec::with_capacity(DIRECT_BUY_ESCROW_FILL_STEPS_V2);
    let mut revision = input.replay.next_revision;
    let mut vault_after = input.vault_balance;
    let mut seller_after = input.seller_destination.balance;
    let mut fee_after = if input.seller_destination.account == input.fee_destination.account {
        seller_after
    } else {
        input.fee_destination.balance
    };

    if settlement.seller_net_collateral_credit != 0 {
        let request = request(
            input.context,
            buyer_before,
            input.accounts,
            OperationShapeV2::Withdraw {
                destination: input.seller_destination.account,
                destination_owner: input.seller_destination.owner,
                destination_compartment: CompartmentV1::External,
                destination_vault_context: [0; 32],
                amount: settlement.seller_net_collateral_credit,
            },
            revision,
            request_index(&requests)?,
        )?;
        append_request(&mut requests, request, DIRECT_BUY_ESCROW_FILL_STEPS_V2)?;
        revision = request.resulting_revision;
        vault_after = vault_after
            .checked_sub(settlement.seller_net_collateral_credit)
            .ok_or(DirectPhysicalError::Arithmetic)?;
        seller_after = seller_after
            .checked_add(settlement.seller_net_collateral_credit)
            .ok_or(DirectPhysicalError::Arithmetic)?;
        if input.seller_destination.account == input.fee_destination.account {
            fee_after = seller_after;
        }
    }

    if settlement.total_fee_transfer != 0 {
        let request = request(
            input.context,
            buyer_before,
            input.accounts,
            OperationShapeV2::Withdraw {
                destination: input.fee_destination.account,
                destination_owner: input.fee_destination.owner,
                destination_compartment: CompartmentV1::External,
                destination_vault_context: [0; 32],
                amount: settlement.total_fee_transfer,
            },
            revision,
            request_index(&requests)?,
        )?;
        append_request(&mut requests, request, DIRECT_BUY_ESCROW_FILL_STEPS_V2)?;
        revision = request.resulting_revision;
        vault_after = vault_after
            .checked_sub(settlement.total_fee_transfer)
            .ok_or(DirectPhysicalError::Arithmetic)?;
        fee_after = fee_after
            .checked_add(settlement.total_fee_transfer)
            .ok_or(DirectPhysicalError::Arithmetic)?;
        if input.seller_destination.account == input.fee_destination.account {
            seller_after = fee_after;
        }
    }

    let (expected_residual, close) = match settlement.buyer.record {
        RegisteredRecordAfterFillV2::Live(record) => (record.reserved_collateral(), None),
        RegisteredRecordAfterFillV2::Closed(close) => (close.collateral_refund, Some(close)),
    };
    match (close, input.record_lifecycle) {
        (None, None) => {}
        (Some(close), Some(lifecycle)) => validate_registered_record_close_lifecycle_v3(
            buyer_before,
            close,
            input.context.trading_program,
            lifecycle,
        )?,
        (None, Some(_)) | (Some(_), None) => return Err(DirectPhysicalError::State),
    }
    if vault_after != expected_residual {
        return Err(DirectPhysicalError::Postcondition);
    }
    let mut buyer_refund_after = input.buyer_refund_destination.balance;
    if let Some(close) = close {
        if close.collateral_refund != 0 {
            let request = request(
                input.context,
                buyer_before,
                input.accounts,
                OperationShapeV2::Withdraw {
                    destination: input.buyer_refund_destination.account,
                    destination_owner: input.buyer_refund_destination.owner,
                    destination_compartment: CompartmentV1::External,
                    destination_vault_context: [0; 32],
                    amount: close.collateral_refund,
                },
                revision,
                request_index(&requests)?,
            )?;
            append_request(&mut requests, request, DIRECT_BUY_ESCROW_FILL_STEPS_V2)?;
            revision = request.resulting_revision;
            vault_after = vault_after
                .checked_sub(close.collateral_refund)
                .ok_or(DirectPhysicalError::Arithmetic)?;
            buyer_refund_after = buyer_refund_after
                .checked_add(close.collateral_refund)
                .ok_or(DirectPhysicalError::Arithmetic)?;
        }
        if vault_after != 0 || input.vault_rent_lamports == 0 || input.replay_rent_lamports == 0 {
            return Err(DirectPhysicalError::Postcondition);
        }
        let close_vault = request(
            input.context,
            buyer_before,
            input.accounts,
            OperationShapeV2::CloseVault {
                rent_refund: buyer_before.rent_owner(),
                rent_lamports: input.vault_rent_lamports,
            },
            revision,
            request_index(&requests)?,
        )?;
        append_request(&mut requests, close_vault, DIRECT_BUY_ESCROW_FILL_STEPS_V2)?;
        revision = close_vault.resulting_revision;
        let close_replay = request(
            input.context,
            buyer_before,
            input.accounts,
            OperationShapeV2::CloseReplay {
                rent_refund: buyer_before.rent_owner(),
                rent_lamports: input.replay_rent_lamports,
            },
            revision,
            request_index(&requests)?,
        )?;
        append_request(&mut requests, close_replay, DIRECT_BUY_ESCROW_FILL_STEPS_V2)?;
    }

    let request_count =
        u8::try_from(requests.len()).map_err(|_| DirectPhysicalError::Arithmetic)?;
    Ok(Box::new(DirectBuyEscrowFillPlanV2 {
        settlement,
        requests: requests.into_boxed_slice(),
        request_count,
        vault_after,
        seller_destination_after: seller_after,
        fee_destination_after: fee_after,
        buyer_refund_destination_after: buyer_refund_after,
        closes_escrow: close.is_some(),
        record_lifecycle: input.record_lifecycle,
    }))
}

/// Complete terminal Buy escrow observation after Direct pure termination.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectBuyEscrowTerminalObservationV2 {
    /// Authenticated Direct record before cancel, expiry, or invalidation.
    pub record_before: DirectRegisteredIntentV2,
    /// Authenticated physical account coordinates.
    pub accounts: DirectBuyEscrowAccountsV2,
    /// Current exact Custody replay state.
    pub replay: CustodyReplayV1,
    /// Current record-keyed Vault token balance.
    pub vault_balance: u64,
    /// Persisted maker destination selected by the signed intent.
    pub refund_destination: DirectExternalCollateralV2,
    /// Exact current Vault rent recovered on close.
    pub vault_rent_lamports: u64,
    /// Exact current replay rent recovered on close.
    pub replay_rent_lamports: u64,
    /// Enabled generic record Close plan.
    pub record_lifecycle: StateLifecyclePlanV3,
    /// Fixed current Core/release/request facts.
    pub context: DirectBuyEscrowContextV2,
}

/// Terminal residual refund followed by Vault and replay close.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectBuyEscrowTerminalPlanV2 {
    /// Positive refund if present, then close Vault and replay.
    /// The boxed slice is bounded by [`DIRECT_BUY_ESCROW_TERMINAL_STEPS_V2`].
    pub requests: Box<[CustodyRequestV1]>,
    /// Number of positive canonical requests.
    pub request_count: u8,
    /// External signed destination after residual refund.
    pub refund_destination_after: u64,
}

/// Project a full fill into exact terminal Custody requests.
pub fn prepare_buy_escrow_full_fill_v2(
    input: &DirectBuyEscrowTerminalObservationV2,
    candidate: RegisteredFillCandidateV2,
) -> Result<Box<DirectBuyEscrowTerminalPlanV2>> {
    let close = match candidate.record {
        RegisteredRecordAfterFillV2::Closed(close) => close,
        RegisteredRecordAfterFillV2::Live(_) => return Err(DirectPhysicalError::Binding),
    };
    prepare_buy_escrow_terminal(input, candidate.maker_root, close, false)
}

/// Project cancel, expiry, or invalidation into exact terminal Custody requests.
pub fn prepare_buy_escrow_unwind_v2(
    input: &DirectBuyEscrowTerminalObservationV2,
    terminal: RegisteredTerminalResultV2,
) -> Result<Box<DirectBuyEscrowTerminalPlanV2>> {
    prepare_buy_escrow_terminal(input, terminal.maker_root, terminal.close, true)
}

fn prepare_buy_escrow_terminal(
    input: &DirectBuyEscrowTerminalObservationV2,
    maker_root: MakerReplayRootV1,
    close: RegisteredRecordCloseV2,
    is_unwind: bool,
) -> Result<Box<DirectBuyEscrowTerminalPlanV2>> {
    input.context.validate(true)?;
    validate_buy_record(input.context, input.record_before)?;
    validate_accounts(input.context, input.record_before, input.accounts)?;
    validate_replay(
        input.context,
        input.record_before,
        input.accounts,
        input.replay,
        1,
    )?;
    if maker_root.maker() != input.record_before.maker()
        || maker_root.market() != input.record_before.intent().market
        || maker_root.generation() != input.record_before.intent().generation
        || close.closed_nonce != input.record_before.intent().nonce
        || close.claim_refund != 0
        || (is_unwind && close.collateral_refund != input.record_before.reserved_collateral())
        || close.collateral_refund > input.record_before.reserved_collateral()
        || close.rent_owner != input.record_before.rent_owner()
        || input.vault_balance != close.collateral_refund
        || input.refund_destination.account != input.record_before.intent().collateral_account
        || input.refund_destination.owner != input.record_before.maker()
        || input.vault_rent_lamports == 0
        || input.replay_rent_lamports == 0
    {
        return Err(DirectPhysicalError::Binding);
    }
    validate_registered_record_close_lifecycle_v3(
        input.record_before,
        close,
        input.context.trading_program,
        input.record_lifecycle,
    )?;

    let mut requests = Vec::with_capacity(DIRECT_BUY_ESCROW_TERMINAL_STEPS_V2);
    let mut revision = input.replay.next_revision;
    let refund_destination_after = input
        .refund_destination
        .balance
        .checked_add(close.collateral_refund)
        .ok_or(DirectPhysicalError::Arithmetic)?;
    if close.collateral_refund != 0 {
        let refund = request(
            input.context,
            input.record_before,
            input.accounts,
            OperationShapeV2::Withdraw {
                destination: input.refund_destination.account,
                destination_owner: input.refund_destination.owner,
                destination_compartment: CompartmentV1::External,
                destination_vault_context: [0; 32],
                amount: close.collateral_refund,
            },
            revision,
            request_index(&requests)?,
        )?;
        append_request(&mut requests, refund, DIRECT_BUY_ESCROW_TERMINAL_STEPS_V2)?;
        revision = refund.resulting_revision;
    }
    let close_vault = request(
        input.context,
        input.record_before,
        input.accounts,
        OperationShapeV2::CloseVault {
            rent_refund: input.record_before.rent_owner(),
            rent_lamports: input.vault_rent_lamports,
        },
        revision,
        request_index(&requests)?,
    )?;
    append_request(
        &mut requests,
        close_vault,
        DIRECT_BUY_ESCROW_TERMINAL_STEPS_V2,
    )?;
    revision = close_vault.resulting_revision;
    let close_replay = request(
        input.context,
        input.record_before,
        input.accounts,
        OperationShapeV2::CloseReplay {
            rent_refund: input.record_before.rent_owner(),
            rent_lamports: input.replay_rent_lamports,
        },
        revision,
        request_index(&requests)?,
    )?;
    append_request(
        &mut requests,
        close_replay,
        DIRECT_BUY_ESCROW_TERMINAL_STEPS_V2,
    )?;
    let request_count =
        u8::try_from(requests.len()).map_err(|_| DirectPhysicalError::Arithmetic)?;
    Ok(Box::new(DirectBuyEscrowTerminalPlanV2 {
        requests: requests.into_boxed_slice(),
        request_count,
        refund_destination_after,
    }))
}

pub(super) enum OperationShapeV2 {
    InitializeReplay {
        payer: [u8; 32],
        rent_refund: [u8; 32],
        rent_lamports: u64,
    },
    OpenVault {
        payer: [u8; 32],
        rent_refund: [u8; 32],
        rent_lamports: u64,
    },
    Deposit {
        source: [u8; 32],
        source_owner: [u8; 32],
        amount: u64,
    },
    Withdraw {
        destination: [u8; 32],
        destination_owner: [u8; 32],
        destination_compartment: CompartmentV1,
        destination_vault_context: [u8; 32],
        amount: u64,
    },
    CloseVault {
        rent_refund: [u8; 32],
        rent_lamports: u64,
    },
    CloseReplay {
        rent_refund: [u8; 32],
        rent_lamports: u64,
    },
}

pub(super) fn request(
    context: DirectBuyEscrowContextV2,
    record: DirectRegisteredIntentV2,
    accounts: DirectBuyEscrowAccountsV2,
    shape: OperationShapeV2,
    expected_revision: u64,
    transfer_index: u16,
) -> Result<CustodyRequestV1> {
    let realm = context.core_market.realm();
    let mut value = CustodyRequestV1 {
        operation: OperationV1::InitializeReplay,
        caller_role: CallerRoleV1::Trading,
        source_compartment: CompartmentV1::None,
        destination_compartment: CompartmentV1::None,
        release_set: context.core_market.release_set().release_set_id.to_bytes(),
        market: context.core_market.market().to_bytes(),
        realm: realm.realm_id.to_bytes(),
        context: accounts.record,
        caller_program: context.trading_program,
        semantic: ContextV1 {
            candidate: [0; 32],
            source_owner: [0; 32],
            destination_owner: [0; 32],
            order: accounts.record,
            parent_request_digest: context.parent_request_digest,
            order_nonce: record.intent().nonce,
            generation: record.intent().generation,
            page_index: 0,
            execution_index: record.intent().outcome,
            transfer_index,
        },
        source: [0; 32],
        destination: [0; 32],
        source_vault_context: [0; 32],
        destination_vault_context: [0; 32],
        mint: [0; 32],
        token_program: [0; 32],
        payer: [0; 32],
        rent_refund: [0; 32],
        expected_revision,
        resulting_revision: expected_revision
            .checked_add(1)
            .ok_or(DirectPhysicalError::Arithmetic)?,
        amount: 0,
        rent_lamports: 0,
    };
    match shape {
        OperationShapeV2::InitializeReplay {
            payer,
            rent_refund,
            rent_lamports,
        } => {
            value.payer = payer;
            value.rent_refund = rent_refund;
            value.rent_lamports = rent_lamports;
        }
        OperationShapeV2::OpenVault {
            payer,
            rent_refund,
            rent_lamports,
        } => {
            value.operation = OperationV1::OpenVault;
            value.destination_compartment = CompartmentV1::TradingPrincipal;
            value.destination = accounts.vault;
            value.destination_vault_context = accounts.record;
            value.mint = realm.collateral_mint.to_bytes();
            value.token_program = realm.token_program.to_bytes();
            value.payer = payer;
            value.rent_refund = rent_refund;
            value.rent_lamports = rent_lamports;
        }
        OperationShapeV2::Deposit {
            source,
            source_owner,
            amount,
        } => {
            value.operation = OperationV1::Transfer;
            value.source_compartment = CompartmentV1::External;
            value.destination_compartment = CompartmentV1::TradingPrincipal;
            value.semantic.source_owner = source_owner;
            value.source = source;
            value.destination = accounts.vault;
            value.destination_vault_context = accounts.record;
            value.mint = realm.collateral_mint.to_bytes();
            value.token_program = realm.token_program.to_bytes();
            value.amount = amount;
        }
        OperationShapeV2::Withdraw {
            destination,
            destination_owner,
            destination_compartment,
            destination_vault_context,
            amount,
        } => {
            value.operation = OperationV1::Transfer;
            value.source_compartment = CompartmentV1::TradingPrincipal;
            value.destination_compartment = destination_compartment;
            value.semantic.destination_owner = destination_owner;
            value.source = accounts.vault;
            value.destination = destination;
            value.source_vault_context = accounts.record;
            value.destination_vault_context = destination_vault_context;
            value.mint = realm.collateral_mint.to_bytes();
            value.token_program = realm.token_program.to_bytes();
            value.amount = amount;
        }
        OperationShapeV2::CloseVault {
            rent_refund,
            rent_lamports,
        } => {
            value.operation = OperationV1::CloseVault;
            value.source_compartment = CompartmentV1::TradingPrincipal;
            value.source = accounts.vault;
            value.source_vault_context = accounts.record;
            value.mint = realm.collateral_mint.to_bytes();
            value.token_program = realm.token_program.to_bytes();
            value.rent_refund = rent_refund;
            value.rent_lamports = rent_lamports;
        }
        OperationShapeV2::CloseReplay {
            rent_refund,
            rent_lamports,
        } => {
            value.operation = OperationV1::CloseReplay;
            value.rent_refund = rent_refund;
            value.rent_lamports = rent_lamports;
        }
    }
    value.validate().map_err(|_| DirectPhysicalError::Custody)?;
    Ok(value)
}

fn validate_buy_record(
    context: DirectBuyEscrowContextV2,
    record: DirectRegisteredIntentV2,
) -> Result<()> {
    if record.intent().side != 1
        || record.intent().lifecycle != 2
        || record.intent().market != context.core_market.market().to_bytes()
        || record.intent().generation != context.core_market.generation()
    {
        return Err(DirectPhysicalError::Binding);
    }
    Ok(())
}

pub(super) fn validate_accounts(
    context: DirectBuyEscrowContextV2,
    record: DirectRegisteredIntentV2,
    accounts: DirectBuyEscrowAccountsV2,
) -> Result<()> {
    if [
        accounts.record,
        accounts.replay,
        accounts.vault,
        accounts.custody_authority,
    ]
    .contains(&[0; 32])
        || accounts.record == accounts.replay
        || accounts.record == accounts.vault
        || accounts.replay == accounts.vault
    {
        return Err(DirectPhysicalError::Binding);
    }
    let record_seeds = RegisteredIntentSeedsV2::from_record(record);
    let (record_key, record_bump) = derive(context.trading_program, &record_seeds.as_slices())?;
    if record_key != accounts.record || record_bump != record.bump() {
        return Err(DirectPhysicalError::Binding);
    }
    let seed_request = request(
        context,
        record,
        accounts,
        OperationShapeV2::InitializeReplay {
            payer: record.maker(),
            rent_refund: record.rent_owner(),
            rent_lamports: 1,
        },
        0,
        0,
    )?;
    let custody_program = context.custody_program();
    let authority_seeds = CustodyAuthoritySeedsV1::from_request(seed_request);
    let replay_seeds = CustodyReplaySeedsV1::from_request(seed_request);
    let vault_seeds = CustodyVaultSeedsV1::new(
        seed_request.market,
        seed_request.release_set,
        accounts.record,
        CompartmentV1::TradingPrincipal,
    );
    if derive(custody_program, &authority_seeds.as_slices())?.0 != accounts.custody_authority
        || derive(custody_program, &replay_seeds.as_slices())?.0 != accounts.replay
        || derive(custody_program, &vault_seeds.as_slices())?.0 != accounts.vault
    {
        return Err(DirectPhysicalError::Binding);
    }
    Ok(())
}

pub(super) fn validate_replay(
    context: DirectBuyEscrowContextV2,
    record: DirectRegisteredIntentV2,
    accounts: DirectBuyEscrowAccountsV2,
    replay: CustodyReplayV1,
    open_vault_count: u32,
) -> Result<()> {
    if replay.caller_role != CallerRoleV1::Trading
        || replay.release_set != context.core_market.release_set().release_set_id.to_bytes()
        || replay.market != context.core_market.market().to_bytes()
        || replay.realm != context.core_market.realm().realm_id.to_bytes()
        || replay.context != accounts.record
        || replay.caller_program != context.trading_program
        || replay.rent_refund != record.rent_owner()
        || replay.open_vault_count != open_vault_count
        || replay.generation != record.intent().generation
    {
        return Err(DirectPhysicalError::Binding);
    }
    Ok(())
}

fn request_index(requests: &[CustodyRequestV1]) -> Result<u16> {
    u16::try_from(requests.len()).map_err(|_| DirectPhysicalError::Arithmetic)
}

fn append_request(
    requests: &mut Vec<CustodyRequestV1>,
    request: CustodyRequestV1,
    maximum: usize,
) -> Result<()> {
    if requests.len() >= maximum {
        return Err(DirectPhysicalError::Arithmetic);
    }
    requests.push(request);
    Ok(())
}

fn derive(program: [u8; 32], seeds: &[&[u8]]) -> Result<([u8; 32], u8)> {
    let (address, bump) = Pubkey::find_program_address(seeds, &Pubkey::new_from_array(program));
    Ok((address.to_bytes(), bump))
}
