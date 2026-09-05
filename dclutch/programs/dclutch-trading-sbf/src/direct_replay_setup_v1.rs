//! Permissionless first-use creation of Direct's Trading-role Custody replay.
//!
//! The top-level wire does not carry a Custody request. Trading authenticates
//! the canonical Open Market, derives the buyer maker root from the Market,
//! generation, and maker, derives the complete Custody request, and signs only
//! that request's release-pinned Trading caller-authority PDA.

extern crate alloc;

use alloc::vec::Vec;

use dclutch_core_contract::ContentId;
use dclutch_custody::{
    CUSTODY_RECEIPT_BYTES_V1, CUSTODY_REPLAY_BYTES_V1, CallerRoleV1, CompartmentV1, ContextV1,
    CustodyFrameSpecV1, CustodyReceiptV1, CustodyReplaySeedsV1, CustodyReplayV1, CustodyRequestV1,
    OperationV1,
};
use dclutch_market::{CoreState, MarketCoreStateSeedsV2, STATE_BYTES};
use dclutch_registry::release_set::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_trading::{
    replay_setup_v1::{
        DirectReplaySetupReceiptV1, DirectReplaySetupRequestV1,
        direct_replay_setup_parent_digest_v1,
    },
    successor::{DirectCoordinatesV1, MakerReplaySeedsV1},
};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke_signed, set_return_data},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::{system_program, sysvar};

use crate::TradingSbfError;
use crate::child_refused_v1;
use crate::market_admission_v1::TRADING_OPEN_MARKET_ADMISSIBLE_PRESTATES_V1;

/// Exact top-level account count: the thirteen-account Custody frame followed
/// by the executable Custody program.
pub const DIRECT_REPLAY_SETUP_ACCOUNT_COUNT_V1: usize = 14;

const CALLER_AUTHORITY: usize = 0;
const MARKET: usize = 1;
const REGISTRY: usize = 3;
const TRADING_PROGRAM: usize = 4;
const REALM: usize = 6;
const REPLAY: usize = 8;
const PAYER: usize = 9;
const RENT: usize = 11;
const RENT_REFUND: usize = 12;
const CUSTODY_PROGRAM: usize = 13;

const _: () = assert!(
    dclutch_custody::INITIALIZE_REPLAY_ACCOUNT_COUNT_V1 as usize + 1
        == DIRECT_REPLAY_SETUP_ACCOUNT_COUNT_V1
);

#[derive(Clone, Copy)]
struct ReplayInvocationV1 {
    request: DirectReplaySetupRequestV1,
    maker_root: [u8; 32],
    top_request_digest: [u8; 32],
    custody_request_digest: [u8; 32],
    observed_lamports: u64,
    payer_before: u64,
    refund_before: u64,
    exact_rent: u64,
}

/// Execute one exact permissionless Direct replay setup request.
#[inline(never)]
pub fn process_direct_replay_setup_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    let request = DirectReplaySetupRequestV1::decode(instruction_data)
        .map_err(|_| TradingSbfError::Content)?;
    authenticate_top_frame(program_id, accounts)?;
    let invocation = invoke_replay_child_v1(program_id, accounts, instruction_data, request)?;
    authenticate_and_emit_replay_v1(program_id, accounts, invocation)
}

#[inline(never)]
fn invoke_replay_child_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
    request: DirectReplaySetupRequestV1,
) -> Result<ReplayInvocationV1, ProgramError> {
    let maker_root = Pubkey::find_program_address(
        &MakerReplaySeedsV1::new(
            DirectCoordinatesV1::new(request.market, request.generation)
                .map_err(|_| TradingSbfError::Content)?,
            request.maker,
        )
        .map_err(|_| TradingSbfError::Content)?
        .as_slices(),
        program_id,
    )
    .0;
    let payer = account(accounts, PAYER)?;
    let rent_refund = account(accounts, RENT_REFUND)?;
    if rent_refund.key == payer.key
        || maker_root == *rent_refund.key
        || maker_root == *payer.key
        || maker_root == *account(accounts, MARKET)?.key
    {
        return Err(TradingSbfError::Content.into());
    }
    let rent =
        Rent::from_account_info(account(accounts, RENT)?).map_err(|_| TradingSbfError::Content)?;
    let exact_rent = rent.minimum_balance(CUSTODY_REPLAY_BYTES_V1);
    let top_request_digest = hash(instruction_data).to_bytes();
    let custody_request = authenticate_market_and_derive_request_v1(
        accounts,
        request,
        program_id.to_bytes(),
        maker_root.to_bytes(),
        payer.key.to_bytes(),
        rent_refund.key.to_bytes(),
        exact_rent,
        top_request_digest,
    )?;
    let custody_request_bytes = custody_request
        .to_bytes()
        .map_err(|_| TradingSbfError::Content)?;
    let custody_request_digest = hash(&custody_request_bytes).to_bytes();
    authenticate_child_coordinates(
        program_id,
        accounts,
        &custody_request,
        custody_request_digest,
    )?;

    let replay = account(accounts, REPLAY)?;
    let observed_lamports = replay.lamports();
    let payer_before = payer.lamports();
    let refund_before = rent_refund.lamports();
    invoke_custody(
        program_id,
        accounts,
        &custody_request_bytes,
        &custody_request,
        custody_request_digest,
    )?;
    Ok(ReplayInvocationV1 {
        request,
        maker_root: maker_root.to_bytes(),
        top_request_digest,
        custody_request_digest,
        observed_lamports,
        payer_before,
        refund_before,
        exact_rent,
    })
}

#[inline(never)]
fn authenticate_and_emit_replay_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    invocation: ReplayInvocationV1,
) -> ProgramResult {
    let payer = account(accounts, PAYER)?;
    let rent_refund = account(accounts, RENT_REFUND)?;
    let custody_request = authenticate_market_and_derive_request_v1(
        accounts,
        invocation.request,
        program_id.to_bytes(),
        invocation.maker_root,
        payer.key.to_bytes(),
        rent_refund.key.to_bytes(),
        invocation.exact_rent,
        invocation.top_request_digest,
    )?;
    let custody_request_digest = hash(
        &custody_request
            .to_bytes()
            .map_err(|_| TradingSbfError::Content)?,
    )
    .to_bytes();
    if custody_request_digest != invocation.custody_request_digest {
        return Err(TradingSbfError::Transition.into());
    }
    let payer_top_up = invocation
        .exact_rent
        .saturating_sub(invocation.observed_lamports);
    let refunded_excess = invocation
        .observed_lamports
        .saturating_sub(invocation.exact_rent);
    if invocation.payer_before.checked_sub(payer_top_up) != Some(payer.lamports())
        || invocation.refund_before.checked_add(refunded_excess) != Some(rent_refund.lamports())
    {
        return Err(TradingSbfError::Transition.into());
    }
    let (custody_poststate, custody_replay_digest) = authenticate_child_result(
        accounts,
        &custody_request,
        custody_request_digest,
        invocation.exact_rent,
    )?;
    emit_replay_receipt_v1(
        accounts,
        &invocation,
        custody_poststate,
        custody_replay_digest,
        payer_top_up,
        refunded_excess,
    )
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn emit_replay_receipt_v1(
    accounts: &[AccountInfo<'_>],
    invocation: &ReplayInvocationV1,
    custody_poststate: [u8; 32],
    custody_replay_digest: [u8; 32],
    payer_top_up: u64,
    refunded_excess: u64,
) -> ProgramResult {
    let replay = account(accounts, REPLAY)?;
    let rent_refund = account(accounts, RENT_REFUND)?;
    let payer = account(accounts, PAYER)?;
    let receipt = DirectReplaySetupReceiptV1 {
        request_digest: invocation.top_request_digest,
        market: invocation.request.market,
        maker: invocation.request.maker,
        maker_root: invocation.maker_root,
        custody_replay: replay.key.to_bytes(),
        rent_refund: rent_refund.key.to_bytes(),
        payer: payer.key.to_bytes(),
        custody_request_digest: invocation.custody_request_digest,
        custody_poststate,
        custody_replay_digest,
        observed_lamports: invocation.observed_lamports,
        payer_top_up,
        refunded_excess,
        exact_rent: invocation.exact_rent,
        post_lamports: replay.lamports(),
    }
    .to_bytes()
    .map_err(|_| TradingSbfError::Width)?;
    set_return_data(&receipt);
    Ok(())
}

/// Authenticate the Market and derive the sole admitted child request in one
/// frame.
///
/// The Core state is 360 bytes and the canonical re-encoding this
/// authentication compares against is another 368. Neither is wanted once the
/// request exists, and a caller that held them would also be holding the
/// 648-byte request, its 672-byte encoding and a CPI account frame. Confining
/// them here is what keeps both callers under the SBPF v0 4,096-byte bound.
///
/// The rent-beneficiary conjunct moved in with the state it reads. It was
/// already stated twice -- `expected_custody_request_v1` refuses the same
/// mismatch with the same refusal -- so nothing is checked less often.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn authenticate_market_and_derive_request_v1(
    accounts: &[AccountInfo<'_>],
    request: DirectReplaySetupRequestV1,
    trading_program: [u8; 32],
    maker_root: [u8; 32],
    payer: [u8; 32],
    rent_refund: [u8; 32],
    exact_rent: u64,
    top_request_digest: [u8; 32],
) -> Result<CustodyRequestV1, ProgramError> {
    let market_state = authenticate_market(accounts, request)?;
    expected_custody_request_v1(
        request,
        &market_state,
        trading_program,
        maker_root,
        payer,
        rent_refund,
        exact_rent,
        top_request_digest,
    )
}

/// Build the sole Custody request admitted by this route.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub fn expected_custody_request_v1(
    top: DirectReplaySetupRequestV1,
    market: &CoreState,
    trading_program: [u8; 32],
    maker_root: [u8; 32],
    payer: [u8; 32],
    rent_refund: [u8; 32],
    exact_rent: u64,
    top_request_digest: [u8; 32],
) -> Result<CustodyRequestV1, ProgramError> {
    if market.identity.market_id.to_bytes() != top.market
        || market.identity.generation != top.generation
        || market.rent_beneficiary.to_bytes() != rent_refund
    {
        return Err(TradingSbfError::Content.into());
    }
    let parent_request_digest = direct_replay_setup_parent_digest_v1(
        top_request_digest,
        maker_root,
        rent_refund,
        payer,
        exact_rent,
    );
    let request = CustodyRequestV1 {
        operation: OperationV1::InitializeReplay,
        caller_role: CallerRoleV1::Trading,
        source_compartment: CompartmentV1::None,
        destination_compartment: CompartmentV1::None,
        release_set: market.identity.selected_release_set.to_bytes(),
        market: top.market,
        realm: market.identity.realm_id.to_bytes(),
        context: maker_root,
        caller_program: trading_program,
        semantic: ContextV1 {
            candidate: top.maker,
            source_owner: [0; 32],
            destination_owner: [0; 32],
            order: [0; 32],
            parent_request_digest,
            order_nonce: 0,
            generation: top.generation,
            page_index: 0,
            execution_index: 0,
            transfer_index: 0,
        },
        source: [0; 32],
        destination: [0; 32],
        source_vault_context: [0; 32],
        destination_vault_context: [0; 32],
        mint: [0; 32],
        token_program: [0; 32],
        payer,
        rent_refund,
        expected_revision: 0,
        resulting_revision: 1,
        amount: 0,
        rent_lamports: exact_rent,
    };
    request.validate().map_err(|_| TradingSbfError::Content)?;
    Ok(request)
}

#[inline(never)]
fn authenticate_top_frame(program_id: &Pubkey, accounts: &[AccountInfo<'_>]) -> ProgramResult {
    if accounts.len() != DIRECT_REPLAY_SETUP_ACCOUNT_COUNT_V1 {
        return Err(TradingSbfError::Content.into());
    }
    for (index, info) in accounts.iter().enumerate() {
        if accounts
            .get(index.saturating_add(1)..)
            .is_some_and(|suffix| suffix.iter().any(|other| other.key == info.key))
        {
            return Err(TradingSbfError::Content.into());
        }
    }
    let frame = CustodyFrameSpecV1::new(OperationV1::InitializeReplay);
    for index in 0..usize::from(frame.account_count()) {
        let expected = frame
            .account(u16::try_from(index).map_err(|_| TradingSbfError::Content)?)
            .map_err(|_| TradingSbfError::Content)?
            .privileges();
        let info = account(accounts, index)?;
        let expected_signer = index == PAYER;
        if info.is_signer != expected_signer
            || info.is_writable != expected.writable()
            || info.executable != expected.executable()
        {
            return Err(TradingSbfError::Content.into());
        }
    }
    if account(accounts, TRADING_PROGRAM)?.key != program_id
        || !account(accounts, CUSTODY_PROGRAM)?.executable
        || account(accounts, CUSTODY_PROGRAM)?.key == program_id
        || account(accounts, RENT)?.key != &sysvar::rent::ID
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

#[inline(never)]
fn authenticate_market(
    accounts: &[AccountInfo<'_>],
    request: DirectReplaySetupRequestV1,
) -> Result<CoreState, ProgramError> {
    let market = account(accounts, MARKET)?;
    let realm = account(accounts, REALM)?;
    let data = market
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let state = CoreState::decode(&data).map_err(|_| TradingSbfError::Content)?;
    let expected_market = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(state.identity).as_slices(),
        market.owner,
    )
    .0;
    if data.len() != STATE_BYTES
        || state
            .encode()
            .map_err(|_| TradingSbfError::Content)?
            .as_slice()
            != data.as_ref()
        || market.key != &expected_market
        || market.key.to_bytes() != request.market
        || state.identity.market_id.to_bytes() != request.market
        || state.identity.realm_id.to_bytes()
            != hash(
                &realm
                    .try_borrow_data()
                    .map_err(|_| TradingSbfError::Content)?,
            )
            .to_bytes()
        || state.identity.registry_program.to_bytes() != account(accounts, REGISTRY)?.key.to_bytes()
        || state.identity.generation != request.generation
        || !TRADING_OPEN_MARKET_ADMISSIBLE_PRESTATES_V1.admits_phase(state.phase)
        || hash(&data).to_bytes() != request.expected_market_digest
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(state)
}

#[inline(never)]
fn authenticate_child_coordinates(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: &CustodyRequestV1,
    request_digest: [u8; 32],
) -> ProgramResult {
    let caller_seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(request.release_set).map_err(|_| TradingSbfError::Content)?,
        request.market,
        ExecutionRoleV1::Trading,
        request.context,
        request_digest,
    )
    .map_err(|_| TradingSbfError::Content)?;
    let custody_program = account(accounts, CUSTODY_PROGRAM)?;
    let replay = account(accounts, REPLAY)?;
    if account(accounts, CALLER_AUTHORITY)?.key
        != &Pubkey::find_program_address(&caller_seeds.as_slices(), program_id).0
        || replay.key
            != &Pubkey::find_program_address(
                &CustodyReplaySeedsV1::from_request(*request).as_slices(),
                custody_program.key,
            )
            .0
        || replay.owner != &system_program::ID
        || replay.data_len() != 0
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

#[inline(never)]
fn invoke_custody(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request_bytes: &[u8],
    request: &CustodyRequestV1,
    request_digest: [u8; 32],
) -> ProgramResult {
    let frame_count = dclutch_custody::INITIALIZE_REPLAY_ACCOUNT_COUNT_V1 as usize;
    let frame = accounts
        .get(..frame_count)
        .ok_or(TradingSbfError::Content)?;
    let mut metas = Vec::with_capacity(frame_count);
    for (index, info) in frame.iter().enumerate() {
        let signer = index == CALLER_AUTHORITY || info.is_signer;
        metas.push(if info.is_writable {
            AccountMeta::new(*info.key, signer)
        } else {
            AccountMeta::new_readonly(*info.key, signer)
        });
    }
    let custody_program = account(accounts, CUSTODY_PROGRAM)?;
    let instruction = Instruction {
        program_id: *custody_program.key,
        accounts: metas,
        data: request_bytes.to_vec(),
    };
    let mut infos = Vec::with_capacity(frame_count + 1);
    infos.extend(frame.iter().cloned());
    infos.push(custody_program.clone());
    let caller_seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(request.release_set).map_err(|_| TradingSbfError::Content)?,
        request.market,
        ExecutionRoleV1::Trading,
        request.context,
        request_digest,
    )
    .map_err(|_| TradingSbfError::Content)?;
    let bump = Pubkey::find_program_address(&caller_seeds.as_slices(), program_id).1;
    let bump_seed = [bump];
    let [domain, release, market, role, context, digest] = caller_seeds.as_slices();
    invoke_signed(
        &instruction,
        &infos,
        &[&[domain, release, market, role, context, digest, &bump_seed]],
    )
    .map_err(child_refused_v1)?;
    Ok(())
}

#[inline(never)]
fn authenticate_child_result(
    accounts: &[AccountInfo<'_>],
    request: &CustodyRequestV1,
    request_digest: [u8; 32],
    exact_rent: u64,
) -> Result<([u8; 32], [u8; 32]), ProgramError> {
    let custody_program = account(accounts, CUSTODY_PROGRAM)?;
    let replay = account(accounts, REPLAY)?;
    let (producer, receipt_bytes) = get_return_data().ok_or(TradingSbfError::Transition)?;
    if producer != *custody_program.key || receipt_bytes.len() != CUSTODY_RECEIPT_BYTES_V1 {
        return Err(TradingSbfError::Transition.into());
    }
    let receipt =
        CustodyReceiptV1::decode(&receipt_bytes).map_err(|_| TradingSbfError::ChildReceipt)?;
    let bytes = replay
        .try_borrow_data()
        .map_err(|_| TradingSbfError::AccountData)?;
    let replay_digest = hash(&bytes).to_bytes();
    let state = CustodyReplayV1::decode(&bytes).map_err(|_| TradingSbfError::AccountData)?;
    receipt
        .verify_for(*request, request_digest, replay_digest)
        .map_err(|_| TradingSbfError::ChildReceipt)?;
    if replay.owner != custody_program.key
        || replay.lamports() != exact_rent
        || bytes.len() != CUSTODY_REPLAY_BYTES_V1
        || state.caller_role != CallerRoleV1::Trading
        || state.release_set != request.release_set
        || state.market != request.market
        || state.realm != request.realm
        || state.context != request.context
        || state.caller_program != request.caller_program
        || state.rent_refund != request.rent_refund
        || state.open_vault_count != 0
        || state.next_revision != 1
        || state.generation != request.semantic.generation
        || state.last_request_digest != request_digest
        || state.last_poststate_commitment != receipt.evidence.poststate_commitment
    {
        return Err(TradingSbfError::Transition.into());
    }
    Ok((receipt.evidence.poststate_commitment, replay_digest))
}

fn account<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<&'accounts AccountInfo<'info>, ProgramError> {
    accounts.get(index).ok_or(TradingSbfError::Content.into())
}

#[cfg(test)]
mod tests {
    use dclutch_market::Phase;
    use dclutch_market::{Identity, MarketIdentity, Readiness};

    use super::*;
    use dclutch_market::StateBumpsV1;

    fn identity(tag: u8) -> Identity {
        Identity::new([tag; 32]).expect("identity")
    }

    fn fixture() -> (DirectReplaySetupRequestV1, CoreState, [u8; 32]) {
        let state = CoreState {
            phase: Phase::Open,
            readiness: Readiness::Consumed,
            terminal_winner: 0,
            identity: MarketIdentity {
                market_id: identity(1),
                realm_id: identity(2),
                product_record: identity(3),
                product_id: identity(4),
                resolution_policy: identity(5),
                capability_manifest: identity(6),
                selected_release_set: identity(7),
                registry_program: identity(8),
                generation: 9,
            },
            outstanding_capabilities: 1,
            principal_cap_sets: 1_000,
            rent_beneficiary: identity(10),
            terminal_receipt: None,
            bumps: StateBumpsV1::UNRECORDED,
        };
        let request = DirectReplaySetupRequestV1 {
            market: state.identity.market_id.to_bytes(),
            maker: [11; 32],
            expected_market_digest: [12; 32],
            generation: state.identity.generation,
        };
        (request, state, [13; 32])
    }

    #[test]
    fn child_request_is_fully_derived_and_binds_canonical_rent_credit() {
        let (top, state, maker_root) = fixture();
        let request = expected_custody_request_v1(
            top,
            &state,
            [14; 32],
            maker_root,
            [15; 32],
            state.rent_beneficiary.to_bytes(),
            1_000,
            [16; 32],
        )
        .expect("derived request");
        assert_eq!(request.operation, OperationV1::InitializeReplay);
        assert_eq!(request.caller_role, CallerRoleV1::Trading);
        assert_eq!(request.context, maker_root);
        assert_eq!(request.semantic.candidate, top.maker);
        assert_eq!(request.rent_refund, state.rent_beneficiary.to_bytes());
        assert_eq!(request.expected_revision, 0);
        assert_eq!(request.resulting_revision, 1);
        assert_eq!(request.rent_lamports, 1_000);

        assert_eq!(
            expected_custody_request_v1(
                top, &state, [14; 32], maker_root, [15; 32], [17; 32], 1_000, [16; 32],
            ),
            Err(TradingSbfError::Content.into())
        );
        assert_eq!(
            expected_custody_request_v1(
                DirectReplaySetupRequestV1 {
                    generation: top.generation + 1,
                    ..top
                },
                &state,
                [14; 32],
                maker_root,
                [15; 32],
                state.rent_beneficiary.to_bytes(),
                1_000,
                [16; 32],
            ),
            Err(TradingSbfError::Content.into())
        );
    }

    #[test]
    fn child_digest_changes_across_every_setup_authority_axis() {
        let (top, state, maker_root) = fixture();
        let expected = |root, payer, rent, digest| {
            expected_custody_request_v1(
                top,
                &state,
                [14; 32],
                root,
                payer,
                state.rent_beneficiary.to_bytes(),
                rent,
                digest,
            )
            .expect("request")
            .to_bytes()
            .map(|bytes| hash(&bytes).to_bytes())
            .expect("encode")
        };
        let baseline = expected(maker_root, [15; 32], 1_000, [16; 32]);
        for changed in [
            expected([17; 32], [15; 32], 1_000, [16; 32]),
            expected(maker_root, [18; 32], 1_000, [16; 32]),
            expected(maker_root, [15; 32], 1_001, [16; 32]),
            expected(maker_root, [15; 32], 1_000, [19; 32]),
        ] {
            assert_ne!(changed, baseline);
        }
    }
}
