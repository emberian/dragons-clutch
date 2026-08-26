//! Dealer Custody activation and quiescent closure planning.
//!
//! The common Trading outer owns root creation and its RentCredit. This module
//! owns only the Dealer-specific child sequence: initialize one Custody replay,
//! open three segregated vaults, fund present principal/liveness, then at
//! terminal retirement close those zero-balance vaults and the replay cursor.

use dclutch_custody_contract::{
    CallerRoleV1, CompartmentV1, ContextV1, CustodyReplaySeedsV1, CustodyRequestV1,
    CustodyVaultSeedsV1, OperationV1,
};
use dclutch_dealer_codec::{CandidateView, Phase, Policy, root_tail::RootTail};
use solana_program::pubkey::Pubkey;

use super::physical::{CollateralEndpointV2, DealerPhysicalContextV2, DealerPhysicalError, Result};

/// Maximum Custody requests in initial replay/vault/funding setup.
pub const MAX_DEALER_ACTIVATION_CUSTODY_REQUESTS_V2: usize = 6;
/// Exact Custody requests in quiescent vault/replay closure.
pub const DEALER_CLOSE_CUSTODY_REQUESTS_V2: usize = 4;

/// Prepaid lamport and token funding selected for activation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerActivationFundingV2 {
    /// System account paying already-present replay/vault rent principal.
    pub payer: [u8; 32],
    /// Immutable recipient of all recovered replay/vault rent principal.
    pub rent_refund: [u8; 32],
    /// Exact replay-account rent lamports.
    pub replay_rent_lamports: u64,
    /// Exact TradingPrincipal vault rent lamports.
    pub principal_vault_rent_lamports: u64,
    /// Exact FeeVault rent lamports.
    pub fee_vault_rent_lamports: u64,
    /// Exact LivenessVault rent lamports.
    pub liveness_vault_rent_lamports: u64,
    /// Present TradingPrincipal collateral transferred at activation.
    pub initial_principal: u64,
}

/// Exact vacant/open-vault identities used by Dealer activation and closure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerLifecycleAccountsV2 {
    /// Canonical Custody replay PDA.
    pub replay: [u8; 32],
    /// Dealer owner's external collateral account.
    pub dealer_owner: CollateralEndpointV2,
    /// TradingPrincipal vault.
    pub principal_vault: CollateralEndpointV2,
    /// Realized FeeVault.
    pub fee_vault: CollateralEndpointV2,
    /// Present LivenessVault.
    pub liveness_vault: CollateralEndpointV2,
}

/// Complete ordered Custody activation plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerActivationCustodyPlanV2 {
    requests: [Option<CustodyRequestV1>; MAX_DEALER_ACTIVATION_CUSTODY_REQUESTS_V2],
    count: u8,
    /// Required Custody replay revision after every request succeeds.
    pub resulting_replay_revision: u64,
}

impl DealerActivationCustodyPlanV2 {
    /// Borrow the fixed-capacity ordered requests.
    pub fn requests(
        &self,
    ) -> &[Option<CustodyRequestV1>; MAX_DEALER_ACTIVATION_CUSTODY_REQUESTS_V2] {
        &self.requests
    }

    /// Return the exact number of active requests.
    pub const fn count(self) -> u8 {
        self.count
    }
}

/// Prepare replay/vault creation and present funding before root commit.
pub fn prepare_activation_custody_v2(
    policy: Policy,
    active: CandidateView<'_>,
    context: DealerPhysicalContextV2,
    accounts: DealerLifecycleAccountsV2,
    funding: DealerActivationFundingV2,
) -> Result<DealerActivationCustodyPlanV2> {
    if context.custody_replay_revision != 0
        || context.release_set != policy.release_set_id
        || context.market != policy.market_id
        || active.outcome_count != policy.outcome_count
        || active.work_funding < policy.minimum_work_funding
        || funding.payer == [0; 32]
        || funding.rent_refund == [0; 32]
        || accounts.replay == [0; 32]
        || funding.replay_rent_lamports == 0
        || funding.principal_vault_rent_lamports == 0
        || funding.fee_vault_rent_lamports == 0
        || funding.liveness_vault_rent_lamports == 0
        || funding.initial_principal < active.quote_reserve_floor
    {
        return Err(DealerPhysicalError::EndpointMismatch);
    }
    validate_lifecycle_accounts(policy, context, accounts)?;
    let required_capital = funding
        .initial_principal
        .checked_add(active.work_funding)
        .ok_or(DealerPhysicalError::Arithmetic)?;
    if accounts.dealer_owner.balance < required_capital {
        return Err(DealerPhysicalError::Arithmetic);
    }
    let mut requests = [None; MAX_DEALER_ACTIVATION_CUSTODY_REQUESTS_V2];
    let mut next_revision = 0_u64;
    push(
        &mut requests,
        0,
        replay_request(
            context,
            OperationV1::InitializeReplay,
            funding.payer,
            funding.rent_refund,
            next_revision,
            funding.replay_rent_lamports,
        )?,
    )?;
    next_revision = 1;
    for (index, endpoint, rent) in [
        (
            1,
            accounts.principal_vault,
            funding.principal_vault_rent_lamports,
        ),
        (2, accounts.fee_vault, funding.fee_vault_rent_lamports),
        (
            3,
            accounts.liveness_vault,
            funding.liveness_vault_rent_lamports,
        ),
    ] {
        push(
            &mut requests,
            index,
            vault_request(
                context,
                endpoint,
                OperationV1::OpenVault,
                funding.payer,
                funding.rent_refund,
                next_revision,
                rent,
            )?,
        )?;
        next_revision = next_revision
            .checked_add(1)
            .ok_or(DealerPhysicalError::Arithmetic)?;
    }
    let mut count = 4_usize;
    if funding.initial_principal != 0 {
        let request = transfer_request(
            context,
            accounts.dealer_owner,
            accounts.principal_vault,
            next_revision,
            u16::try_from(count).map_err(|_| DealerPhysicalError::Capacity)?,
            funding.initial_principal,
        )?;
        push(&mut requests, count, request)?;
        count += 1;
        next_revision = next_revision
            .checked_add(1)
            .ok_or(DealerPhysicalError::Arithmetic)?;
    }
    let request = transfer_request(
        context,
        accounts.dealer_owner,
        accounts.liveness_vault,
        next_revision,
        u16::try_from(count).map_err(|_| DealerPhysicalError::Capacity)?,
        active.work_funding,
    )?;
    push(&mut requests, count, request)?;
    count += 1;
    next_revision = next_revision
        .checked_add(1)
        .ok_or(DealerPhysicalError::Arithmetic)?;
    Ok(DealerActivationCustodyPlanV2 {
        requests,
        count: u8::try_from(count).map_err(|_| DealerPhysicalError::Capacity)?,
        resulting_replay_revision: next_revision,
    })
}

/// Exact rent evidence required for quiescent vault/replay closure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerCloseRentV2 {
    /// Principal vault lamports recovered to the persisted beneficiary.
    pub principal_vault_lamports: u64,
    /// Fee vault lamports recovered to the persisted beneficiary.
    pub fee_vault_lamports: u64,
    /// Liveness vault lamports recovered to the persisted beneficiary.
    pub liveness_vault_lamports: u64,
    /// Replay lamports recovered to the persisted beneficiary.
    pub replay_lamports: u64,
    /// Beneficiary persisted by the replay initialization request.
    pub rent_refund: [u8; 32],
}

/// Ordered close-vault, close-replay plan after semantic retirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerCloseCustodyPlanV2 {
    requests: [CustodyRequestV1; DEALER_CLOSE_CUSTODY_REQUESTS_V2],
    /// Required next revision immediately before the replay account closes.
    pub terminal_replay_revision: u64,
}

impl DealerCloseCustodyPlanV2 {
    /// Borrow the four exact closure requests.
    pub const fn requests(&self) -> &[CustodyRequestV1; DEALER_CLOSE_CUSTODY_REQUESTS_V2] {
        &self.requests
    }
}

/// Prepare complete Dealer child closure after the accepted Retire transition.
///
/// The common outer may close the Trading root only after all four requests
/// acknowledge. Its own RentCredit, not this module, selects the root-rent
/// beneficiary.
pub fn prepare_close_custody_v2(
    policy: Policy,
    tail: RootTail,
    context: DealerPhysicalContextV2,
    accounts: DealerLifecycleAccountsV2,
    rent: DealerCloseRentV2,
) -> Result<DealerCloseCustodyPlanV2> {
    if tail.phase != Phase::Retired
        || accounts.replay == [0; 32]
        || context.release_set != policy.release_set_id
        || context.market != policy.market_id
        || rent.rent_refund == [0; 32]
        || rent.principal_vault_lamports == 0
        || rent.fee_vault_lamports == 0
        || rent.liveness_vault_lamports == 0
        || rent.replay_lamports == 0
    {
        return Err(DealerPhysicalError::EndpointMismatch);
    }
    validate_lifecycle_accounts(policy, context, accounts)?;
    let mut revision = context.custody_replay_revision;
    let mut requests = [replay_request(
        context,
        OperationV1::CloseReplay,
        [0; 32],
        rent.rent_refund,
        revision,
        rent.replay_lamports,
    )?; DEALER_CLOSE_CUSTODY_REQUESTS_V2];
    for (index, endpoint, lamports) in [
        (0, accounts.principal_vault, rent.principal_vault_lamports),
        (1, accounts.fee_vault, rent.fee_vault_lamports),
        (2, accounts.liveness_vault, rent.liveness_vault_lamports),
    ] {
        *requests
            .get_mut(index)
            .ok_or(DealerPhysicalError::Capacity)? = vault_request(
            context,
            endpoint,
            OperationV1::CloseVault,
            [0; 32],
            rent.rent_refund,
            revision,
            lamports,
        )?;
        revision = revision
            .checked_add(1)
            .ok_or(DealerPhysicalError::Arithmetic)?;
    }
    *requests.get_mut(3).ok_or(DealerPhysicalError::Capacity)? = replay_request(
        context,
        OperationV1::CloseReplay,
        [0; 32],
        rent.rent_refund,
        revision,
        rent.replay_lamports,
    )?;
    Ok(DealerCloseCustodyPlanV2 {
        requests,
        terminal_replay_revision: revision,
    })
}

fn validate_lifecycle_accounts(
    policy: Policy,
    context: DealerPhysicalContextV2,
    accounts: DealerLifecycleAccountsV2,
) -> Result<()> {
    if accounts.dealer_owner.compartment != CompartmentV1::External
        || accounts.dealer_owner.external_owner != policy.dealer_id
        || accounts.dealer_owner.vault_context != [0; 32]
    {
        return Err(DealerPhysicalError::EndpointMismatch);
    }
    for (endpoint, compartment) in [
        (accounts.principal_vault, CompartmentV1::TradingPrincipal),
        (accounts.fee_vault, CompartmentV1::FeeVault),
        (accounts.liveness_vault, CompartmentV1::LivenessVault),
    ] {
        if endpoint.account == [0; 32]
            || endpoint.compartment != compartment
            || endpoint.external_owner != [0; 32]
            || endpoint.vault_context != context.child_root
            || endpoint.balance != 0
        {
            return Err(DealerPhysicalError::EndpointMismatch);
        }
    }
    if accounts.principal_vault.account == accounts.fee_vault.account
        || accounts.principal_vault.account == accounts.liveness_vault.account
        || accounts.fee_vault.account == accounts.liveness_vault.account
        || accounts.dealer_owner.account == accounts.principal_vault.account
        || accounts.dealer_owner.account == accounts.fee_vault.account
        || accounts.dealer_owner.account == accounts.liveness_vault.account
    {
        return Err(DealerPhysicalError::EndpointMismatch);
    }
    let custody_program = Pubkey::new_from_array(context.custody_program);
    let replay_request = replay_request(
        context,
        OperationV1::InitializeReplay,
        [1; 32],
        [1; 32],
        0,
        1,
    )?;
    let expected_replay = Pubkey::find_program_address(
        &CustodyReplaySeedsV1::from_request(replay_request).as_slices(),
        &custody_program,
    )
    .0
    .to_bytes();
    if accounts.replay != expected_replay {
        return Err(DealerPhysicalError::EndpointMismatch);
    }
    for (endpoint, compartment) in [
        (accounts.principal_vault, CompartmentV1::TradingPrincipal),
        (accounts.fee_vault, CompartmentV1::FeeVault),
        (accounts.liveness_vault, CompartmentV1::LivenessVault),
    ] {
        let expected_vault = Pubkey::find_program_address(
            &CustodyVaultSeedsV1::new(
                context.market,
                context.release_set,
                context.child_root,
                compartment,
            )
            .as_slices(),
            &custody_program,
        )
        .0
        .to_bytes();
        if endpoint.account != expected_vault {
            return Err(DealerPhysicalError::EndpointMismatch);
        }
    }
    Ok(())
}

fn replay_request(
    context: DealerPhysicalContextV2,
    operation: OperationV1,
    payer: [u8; 32],
    rent_refund: [u8; 32],
    expected_revision: u64,
    rent_lamports: u64,
) -> Result<CustodyRequestV1> {
    let request = CustodyRequestV1 {
        operation,
        caller_role: CallerRoleV1::Trading,
        source_compartment: CompartmentV1::None,
        destination_compartment: CompartmentV1::None,
        release_set: context.release_set,
        market: context.market,
        realm: context.realm,
        context: context.child_root,
        caller_program: context.trading_program,
        semantic: semantic(context, 0),
        source: [0; 32],
        destination: [0; 32],
        source_vault_context: [0; 32],
        destination_vault_context: [0; 32],
        mint: [0; 32],
        token_program: [0; 32],
        payer,
        rent_refund,
        expected_revision,
        resulting_revision: expected_revision
            .checked_add(1)
            .ok_or(DealerPhysicalError::Arithmetic)?,
        amount: 0,
        rent_lamports,
    };
    request
        .validate()
        .map_err(|_| DealerPhysicalError::Custody)?;
    Ok(request)
}

#[allow(clippy::too_many_arguments)]
fn vault_request(
    context: DealerPhysicalContextV2,
    vault: CollateralEndpointV2,
    operation: OperationV1,
    payer: [u8; 32],
    rent_refund: [u8; 32],
    expected_revision: u64,
    rent_lamports: u64,
) -> Result<CustodyRequestV1> {
    let opening = operation == OperationV1::OpenVault;
    let request = CustodyRequestV1 {
        operation,
        caller_role: CallerRoleV1::Trading,
        source_compartment: if opening {
            CompartmentV1::None
        } else {
            vault.compartment
        },
        destination_compartment: if opening {
            vault.compartment
        } else {
            CompartmentV1::None
        },
        release_set: context.release_set,
        market: context.market,
        realm: context.realm,
        context: context.child_root,
        caller_program: context.trading_program,
        semantic: semantic(context, 0),
        source: if opening { [0; 32] } else { vault.account },
        destination: if opening { vault.account } else { [0; 32] },
        source_vault_context: if opening { [0; 32] } else { context.child_root },
        destination_vault_context: if opening { context.child_root } else { [0; 32] },
        mint: context.mint,
        token_program: context.token_program,
        payer,
        rent_refund,
        expected_revision,
        resulting_revision: expected_revision
            .checked_add(1)
            .ok_or(DealerPhysicalError::Arithmetic)?,
        amount: 0,
        rent_lamports,
    };
    request
        .validate()
        .map_err(|_| DealerPhysicalError::Custody)?;
    Ok(request)
}

fn transfer_request(
    context: DealerPhysicalContextV2,
    source: CollateralEndpointV2,
    destination: CollateralEndpointV2,
    expected_revision: u64,
    transfer_index: u16,
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
            source_owner: source.external_owner,
            destination_owner: destination.external_owner,
            ..semantic(context, transfer_index)
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

const fn semantic(context: DealerPhysicalContextV2, transfer_index: u16) -> ContextV1 {
    ContextV1 {
        candidate: [0; 32],
        source_owner: [0; 32],
        destination_owner: [0; 32],
        order: [0; 32],
        parent_request_digest: context.parent_request_digest,
        order_nonce: context.custody_replay_revision,
        generation: context.generation,
        page_index: 0,
        execution_index: 0,
        transfer_index,
    }
}

fn push<const N: usize>(
    requests: &mut [Option<CustodyRequestV1>; N],
    index: usize,
    request: CustodyRequestV1,
) -> Result<()> {
    *requests
        .get_mut(index)
        .ok_or(DealerPhysicalError::Capacity)? = Some(request);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_dealer_codec::{
        CANDIDATE_BYTES, CandidateInput, CurveBand, CurveInput, encode_candidate,
    };

    fn context(revision: u64) -> DealerPhysicalContextV2 {
        DealerPhysicalContextV2 {
            trading_program: [1; 32],
            claims_program: [2; 32],
            custody_program: [3; 32],
            release_set: [4; 32],
            market: [5; 32],
            realm: [6; 32],
            child_root: [7; 32],
            mint: [8; 32],
            token_program: [9; 32],
            parent_request_digest: [10; 32],
            generation: 2,
            claims_market_revision: 1,
            dealer_position_revision: 1,
            dealer_owner_position_revision: 1,
            taker_owner: [0; 32],
            taker_position_revision: dclutch_claims_svm::NO_POSITION_REVISION,
            custody_replay_revision: revision,
        }
    }

    fn policy() -> Policy {
        Policy {
            market_id: [5; 32],
            release_set_id: [4; 32],
            dealer_id: [11; 32],
            fee_recipient_id: [12; 32],
            unwind_recipient_id: [13; 32],
            outcome_count: 2,
            quote_scale: 100,
            fee_numerator: 1,
            fee_denominator: 100,
            minimum_work_funding: 10,
            replacement_delay: 5,
        }
    }

    fn endpoint(account: u8, owner: [u8; 32], compartment: CompartmentV1) -> CollateralEndpointV2 {
        CollateralEndpointV2 {
            account: [account; 32],
            external_owner: owner,
            compartment,
            vault_context: if compartment == CompartmentV1::External {
                [0; 32]
            } else {
                [7; 32]
            },
            balance: 0,
        }
    }

    fn accounts() -> DealerLifecycleAccountsV2 {
        let context = context(0);
        let custody_program = Pubkey::new_from_array(context.custody_program);
        let replay_template = replay_request(
            context,
            OperationV1::InitializeReplay,
            [1; 32],
            [1; 32],
            0,
            1,
        )
        .expect("replay template");
        let replay = Pubkey::find_program_address(
            &CustodyReplaySeedsV1::from_request(replay_template).as_slices(),
            &custody_program,
        )
        .0
        .to_bytes();
        let vault = |compartment| {
            Pubkey::find_program_address(
                &CustodyVaultSeedsV1::new(
                    context.market,
                    context.release_set,
                    context.child_root,
                    compartment,
                )
                .as_slices(),
                &custody_program,
            )
            .0
            .to_bytes()
        };
        let mut dealer_owner = endpoint(21, [11; 32], CompartmentV1::External);
        dealer_owner.balance = 100;
        DealerLifecycleAccountsV2 {
            replay,
            dealer_owner,
            principal_vault: CollateralEndpointV2 {
                account: vault(CompartmentV1::TradingPrincipal),
                ..endpoint(22, [0; 32], CompartmentV1::TradingPrincipal)
            },
            fee_vault: CollateralEndpointV2 {
                account: vault(CompartmentV1::FeeVault),
                ..endpoint(23, [0; 32], CompartmentV1::FeeVault)
            },
            liveness_vault: CollateralEndpointV2 {
                account: vault(CompartmentV1::LivenessVault),
                ..endpoint(24, [0; 32], CompartmentV1::LivenessVault)
            },
        }
    }

    fn candidate() -> [u8; CANDIDATE_BYTES] {
        let bids = [CurveBand {
            capacity: 10,
            price_numerator: 40,
        }];
        let asks = [CurveBand {
            capacity: 10,
            price_numerator: 60,
        }];
        let curves = [CurveInput {
            bids: &bids,
            asks: &asks,
        }; 2];
        let mut bytes = [0; CANDIDATE_BYTES];
        encode_candidate(
            &mut bytes,
            CandidateInput {
                candidate_id: [30; 32],
                revision: 1,
                valid_from: 0,
                expires_at: 100,
                quote_reserve_floor: 50,
                work_funding: 20,
                work_reward: 1,
                minimum_inventory: &[0, 0],
                maximum_inventory: &[10, 10],
                curves: &curves,
            },
        )
        .expect("candidate");
        bytes
    }

    #[test]
    fn activation_initializes_replay_three_vaults_then_funds_principal_and_work() {
        let bytes = candidate();
        let active = CandidateView::decode(&bytes).expect("candidate");
        let plan = prepare_activation_custody_v2(
            policy(),
            active,
            context(0),
            accounts(),
            DealerActivationFundingV2 {
                payer: [31; 32],
                rent_refund: [32; 32],
                replay_rent_lamports: 100,
                principal_vault_rent_lamports: 200,
                fee_vault_rent_lamports: 200,
                liveness_vault_rent_lamports: 200,
                initial_principal: 50,
            },
        )
        .expect("activation plan");
        assert_eq!(plan.count(), 6);
        assert_eq!(plan.resulting_replay_revision, 6);
        let requests = plan.requests();
        assert_eq!(
            requests[0].expect("replay").operation,
            OperationV1::InitializeReplay
        );
        assert_eq!(
            requests[3].expect("liveness").destination_compartment,
            CompartmentV1::LivenessVault
        );
        assert_eq!(requests[4].expect("principal").amount, 50);
        assert_eq!(requests[5].expect("work").amount, 20);
    }

    #[test]
    fn retired_zero_vaults_close_before_replay_and_preserve_refund() {
        let tail = RootTail {
            phase: Phase::Retired,
            active_candidate_id: [30; 32],
            pending_candidate_id: [0; 32],
            active_revision: 1,
            pending_revision: 0,
            state_revision: 9,
            buy_used: [0; dclutch_dealer_codec::MAX_OUTCOMES],
            sell_used: [0; dclutch_dealer_codec::MAX_OUTCOMES],
            fee_base: 0,
            active_work_remaining: 0,
            pending_work_funding: 0,
        };
        let plan = prepare_close_custody_v2(
            policy(),
            tail,
            context(6),
            accounts(),
            DealerCloseRentV2 {
                principal_vault_lamports: 200,
                fee_vault_lamports: 200,
                liveness_vault_lamports: 200,
                replay_lamports: 100,
                rent_refund: [32; 32],
            },
        )
        .expect("close plan");
        assert!(
            plan.requests()[..3]
                .iter()
                .all(|request| request.operation == OperationV1::CloseVault)
        );
        assert_eq!(plan.requests()[3].operation, OperationV1::CloseReplay);
        assert_eq!(plan.requests()[3].rent_refund, [32; 32]);
        assert_eq!(plan.terminal_replay_revision, 9);
    }

    #[test]
    fn activation_and_close_refuse_underfunding_nonterminal_or_nonzero_vaults() {
        let bytes = candidate();
        let active = CandidateView::decode(&bytes).expect("candidate");
        assert!(
            prepare_activation_custody_v2(
                policy(),
                active,
                context(0),
                accounts(),
                DealerActivationFundingV2 {
                    payer: [31; 32],
                    rent_refund: [32; 32],
                    replay_rent_lamports: 100,
                    principal_vault_rent_lamports: 200,
                    fee_vault_rent_lamports: 200,
                    liveness_vault_rent_lamports: 200,
                    initial_principal: 49,
                }
            )
            .is_err()
        );
        let mut short_capital = accounts();
        short_capital.dealer_owner.balance = 69;
        assert_eq!(
            prepare_activation_custody_v2(
                policy(),
                active,
                context(0),
                short_capital,
                DealerActivationFundingV2 {
                    payer: [31; 32],
                    rent_refund: [32; 32],
                    replay_rent_lamports: 100,
                    principal_vault_rent_lamports: 200,
                    fee_vault_rent_lamports: 200,
                    liveness_vault_rent_lamports: 200,
                    initial_principal: 50,
                }
            ),
            Err(DealerPhysicalError::Arithmetic)
        );
        let mut substituted_pda = accounts();
        substituted_pda.replay = [99; 32];
        assert!(
            prepare_activation_custody_v2(
                policy(),
                active,
                context(0),
                substituted_pda,
                DealerActivationFundingV2 {
                    payer: [31; 32],
                    rent_refund: [32; 32],
                    replay_rent_lamports: 100,
                    principal_vault_rent_lamports: 200,
                    fee_vault_rent_lamports: 200,
                    liveness_vault_rent_lamports: 200,
                    initial_principal: 50,
                }
            )
            .is_err()
        );
        let mut hostile_accounts = accounts();
        hostile_accounts.fee_vault.balance = 1;
        let open_tail = RootTail::initialize(active);
        assert!(
            prepare_close_custody_v2(
                policy(),
                open_tail,
                context(6),
                hostile_accounts,
                DealerCloseRentV2 {
                    principal_vault_lamports: 200,
                    fee_vault_lamports: 200,
                    liveness_vault_lamports: 200,
                    replay_lamports: 100,
                    rent_refund: [32; 32],
                }
            )
            .is_err()
        );
    }
}
