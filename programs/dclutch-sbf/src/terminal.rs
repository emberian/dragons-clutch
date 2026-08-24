//! Same-PDA terminal categorical-Market compaction.
//!
//! The active Market's immutable `rent_refund` bytes remain a beneficiary
//! authority, never a caller-selected lamport destination. Compaction binds
//! that authority to its permanent program-owned RentCredit PDA, credits the
//! exact current-Rent shrink delta, preserves the credit data byte-for-byte,
//! and writes the terminal representation in one SVM instruction.

use dclutch_collateral_contract::{
    AccountPrivilege, CompactTerminalMarketV1, InstructionTag, validate_account_frame,
};
use dclutch_core_contract::MarketRoot;
use dclutch_market_contract::market::{CategoricalMarketV1, decode_market_outcome_count};
use dclutch_rent_contract::{CreditBalancePlanV1, RefundAuthority, RentCreditV1};
use dclutch_terminal_contract::{
    TERMINAL_CATEGORICAL_MARKET_BYTES, TerminalCategoricalMarketV1, decode_terminal_outcome_count,
};
use solana_program::{
    account_info::AccountInfo, hash::hash, program_error::ProgramError, pubkey::Pubkey, rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::sysvar;

use crate::{
    AdapterError,
    authenticate::MARKET_SEED,
    records::{authenticate_rent_credit, map_rent_error, require_unchanged_rent_credit},
};

#[derive(Clone, Copy)]
struct TerminalProjectionPlan {
    market: Pubkey,
    root: MarketRoot,
    outcome_count: u8,
    active_bytes: usize,
    active_state_digest: [u8; 32],
    market_lamports_before_credit: u64,
    market_lamports_after_credit: u64,
    rent_credit_lamports: u64,
    terminal_bytes: [u8; TERMINAL_CATEGORICAL_MARKET_BYTES],
}

/// Fully authenticated facts required to compact one active Market.
///
/// The move-only plan is constructed only from canonical active Market bytes, an
/// authenticated Rent sysvar, and the derived permanent RentCredit bound to
/// the root's immutable beneficiary authority. It is not an instruction wire.
struct TerminalCompactionPlan {
    projection: TerminalProjectionPlan,
    rent_credit_account: Pubkey,
    rent_credit_state: RentCreditV1,
    credit_balance: CreditBalancePlanV1,
}

/// Compact one Retired active Market into its same-PDA terminal form.
///
/// The exact borrowed frame is `[market writable nonsigner, permanent
/// RentCredit writable nonsigner, Rent sysvar readonly nonsigner]`; all three
/// keys must be distinct. Authentication and application are deliberately
/// joined here so a caller never owns an independently executable credit or
/// projection handoff.
pub(crate) fn process_compact_terminal_market(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction: CompactTerminalMarketV1,
) -> Result<(), ProgramError> {
    if accounts.len() != 3 {
        return Err(AdapterError::AccountFrameLength.into());
    }
    let market = accounts.first().ok_or(AdapterError::AccountFrameLength)?;
    let rent_credit = accounts.get(1).ok_or(AdapterError::AccountFrameLength)?;
    let rent_sysvar = accounts.get(2).ok_or(AdapterError::AccountFrameLength)?;
    let privileges = [market, rent_credit, rent_sysvar].map(|account| AccountPrivilege {
        is_signer: account.is_signer,
        is_writable: account.is_writable,
        is_executable: account.executable,
    });
    validate_account_frame(InstructionTag::CompactTerminalMarket, &privileges)
        .map_err(|_| AdapterError::AccountPrivilege)?;
    compact_terminal_market(
        program_id,
        market,
        rent_credit,
        rent_sysvar,
        instruction.generation(),
    )
}

#[inline(never)]
fn compact_terminal_market(
    program_id: &Pubkey,
    market: &AccountInfo<'_>,
    rent_credit: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
    expected_generation: u64,
) -> Result<(), ProgramError> {
    let plan = authenticate_terminal_compaction(
        program_id,
        market,
        rent_credit,
        rent_sysvar,
        expected_generation,
    )?;
    apply_terminal_compaction(program_id, market, rent_credit, plan)
}

/// Authenticate a mutable active Market, writable RentCredit, and Rent sysvar,
/// then build one exact terminal-compaction plan.
///
/// This does not mutate either account.  The resulting plan has already
/// checked program ownership, canonical Market bytes, the identity-derived
/// Market PDA, Retired phase, zero children, empty Hoard/supply economics,
/// exact Rent delta, derived credit PDA/data/authority, and the terminal
/// contract's exact 312-byte projection.
#[inline(never)]
fn authenticate_terminal_compaction(
    program_id: &Pubkey,
    market: &AccountInfo<'_>,
    rent_credit: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
    expected_generation: u64,
) -> Result<TerminalCompactionPlan, ProgramError> {
    authenticate_market_account(program_id, market)?;
    let rent = authenticate_rent_sysvar(rent_sysvar)?;
    if market.key == rent_sysvar.key
        || market.key == rent_credit.key
        || rent_credit.key == rent_sysvar.key
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    if rent_credit.is_signer || !rent_credit.is_writable || rent_credit.executable {
        return Err(AdapterError::AccountPrivilege.into());
    }
    let data = market
        .try_borrow_data()
        .map_err(|_| AdapterError::AccountData)?;
    let projection = plan_from_active_bytes(
        program_id,
        market.key,
        market.owner,
        market.lamports(),
        &data,
        &rent,
    )?;
    if projection.root.identity().generation() != expected_generation {
        return Err(AdapterError::ReplayMismatch.into());
    }
    let authority = RefundAuthority::new(projection.root.rent_refund()).map_err(map_rent_error)?;
    let rent_credit_state = authenticate_rent_credit(program_id, rent_credit, authority, None)?;
    let credit_balance =
        CreditBalancePlanV1::new(rent_credit.lamports(), projection.rent_credit_lamports)
            .map_err(map_rent_error)?;
    Ok(TerminalCompactionPlan {
        projection,
        rent_credit_account: *rent_credit.key,
        rent_credit_state,
        credit_balance,
    })
}

/// Credit the exact shrink delta and write the terminal Market atomically.
#[inline(never)]
fn apply_terminal_compaction(
    program_id: &Pubkey,
    market: &AccountInfo<'_>,
    rent_credit: &AccountInfo<'_>,
    plan: TerminalCompactionPlan,
) -> Result<(), ProgramError> {
    authenticate_market_account(program_id, market)?;
    if market.key != &plan.projection.market
        || rent_credit.key != &plan.rent_credit_account
        || market.data_len() != plan.projection.active_bytes
    {
        return Err(AdapterError::MarketTransition.into());
    }
    let observed_credit = authenticate_rent_credit(
        program_id,
        rent_credit,
        plan.rent_credit_state.refund_authority(),
        None,
    )?;
    if observed_credit != plan.rent_credit_state
        || market.lamports() != plan.projection.market_lamports_before_credit
        || rent_credit.lamports() != plan.credit_balance.credit_before()
        || plan.credit_balance.credited_lamports() != plan.projection.rent_credit_lamports
    {
        return Err(AdapterError::MarketTransition.into());
    }
    let active_data = market
        .try_borrow_data()
        .map_err(|_| AdapterError::AccountData)?;
    if hash(&active_data).to_bytes() != plan.projection.active_state_digest {
        return Err(AdapterError::MarketTransition.into());
    }
    drop(active_data);

    preflight_mutable(market)?;
    preflight_mutable(rent_credit)?;
    {
        let mut market_lamports = market
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::AccountData)?;
        let mut credit_lamports = rent_credit
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::AccountData)?;
        **market_lamports = plan.projection.market_lamports_after_credit;
        **credit_lamports = plan.credit_balance.credit_after();
    }
    if market.lamports() != plan.projection.market_lamports_after_credit
        || plan
            .projection
            .market_lamports_before_credit
            .checked_sub(market.lamports())
            != Some(plan.projection.rent_credit_lamports)
    {
        return Err(AdapterError::MarketTransition.into());
    }
    plan.credit_balance
        .validate_post(rent_credit.lamports())
        .map_err(map_rent_error)?;
    require_unchanged_rent_credit(program_id, rent_credit, plan.rent_credit_state)?;

    // `resize` is the Solana-program 3.x AccountInfo same-account
    // reallocation primitive.  The account remains owned by this program;
    // only the exact active suffix is discarded.
    market
        .resize(TERMINAL_CATEGORICAL_MARKET_BYTES)
        .map_err(|_| AdapterError::MarketTransition)?;
    let mut data = market
        .try_borrow_mut_data()
        .map_err(|_| AdapterError::AccountData)?;
    if data.len() != TERMINAL_CATEGORICAL_MARKET_BYTES {
        return Err(AdapterError::MarketTransition.into());
    }
    data.copy_from_slice(&plan.projection.terminal_bytes);
    drop(data);

    if market.data_len() != TERMINAL_CATEGORICAL_MARKET_BYTES
        || market
            .try_lamports()
            .map_err(|_| AdapterError::AccountData)?
            != plan.projection.market_lamports_after_credit
    {
        return Err(AdapterError::MarketTransition.into());
    }
    let terminal_data = market
        .try_borrow_data()
        .map_err(|_| AdapterError::AccountData)?;
    require_unchanged_rent_credit(program_id, rent_credit, plan.rent_credit_state)?;
    verify_terminal_projection(&terminal_data, plan.projection.outcome_count)
}

fn authenticate_market_account(
    program_id: &Pubkey,
    market: &AccountInfo<'_>,
) -> Result<(), ProgramError> {
    if market.owner != program_id || !market.is_writable || market.is_signer || market.executable {
        return Err(AdapterError::AccountIdentity.into());
    }
    Ok(())
}

fn authenticate_rent_sysvar(rent_sysvar: &AccountInfo<'_>) -> Result<Rent, ProgramError> {
    if rent_sysvar.key != &sysvar::rent::ID
        || rent_sysvar.owner != &sysvar::ID
        || rent_sysvar.is_signer
        || rent_sysvar.is_writable
        || rent_sysvar.executable
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    Rent::from_account_info(rent_sysvar).map_err(|_| AdapterError::AccountData.into())
}

fn preflight_mutable(account: &AccountInfo<'_>) -> Result<(), ProgramError> {
    drop(
        account
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::AccountData)?,
    );
    drop(
        account
            .try_borrow_mut_data()
            .map_err(|_| AdapterError::AccountData)?,
    );
    Ok(())
}

fn plan_from_active_bytes(
    program_id: &Pubkey,
    market_key: &Pubkey,
    market_owner: &Pubkey,
    market_lamports: u64,
    active_bytes: &[u8],
    rent: &Rent,
) -> Result<TerminalProjectionPlan, ProgramError> {
    if market_owner != program_id {
        return Err(AdapterError::AccountIdentity.into());
    }
    // The binary active and terminal layouts are both 312 bytes.  Do not let
    // their equal widths turn an already-reclaimed account into an idempotent
    // second compaction: a complete canonical terminal envelope is a replay,
    // never an active Market input.
    if decode_terminal_outcome_count(active_bytes).is_ok() {
        return Err(AdapterError::ReplayMismatch.into());
    }
    let outcome_count =
        decode_market_outcome_count(active_bytes).map_err(|_| AdapterError::AccountData)?;
    match outcome_count {
        2 => plan_width::<2>(program_id, market_key, market_lamports, active_bytes, rent),
        3 => plan_width::<3>(program_id, market_key, market_lamports, active_bytes, rent),
        4 => plan_width::<4>(program_id, market_key, market_lamports, active_bytes, rent),
        5 => plan_width::<5>(program_id, market_key, market_lamports, active_bytes, rent),
        6 => plan_width::<6>(program_id, market_key, market_lamports, active_bytes, rent),
        7 => plan_width::<7>(program_id, market_key, market_lamports, active_bytes, rent),
        8 => plan_width::<8>(program_id, market_key, market_lamports, active_bytes, rent),
        9 => plan_width::<9>(program_id, market_key, market_lamports, active_bytes, rent),
        10 => plan_width::<10>(program_id, market_key, market_lamports, active_bytes, rent),
        11 => plan_width::<11>(program_id, market_key, market_lamports, active_bytes, rent),
        12 => plan_width::<12>(program_id, market_key, market_lamports, active_bytes, rent),
        13 => plan_width::<13>(program_id, market_key, market_lamports, active_bytes, rent),
        14 => plan_width::<14>(program_id, market_key, market_lamports, active_bytes, rent),
        15 => plan_width::<15>(program_id, market_key, market_lamports, active_bytes, rent),
        16 => plan_width::<16>(program_id, market_key, market_lamports, active_bytes, rent),
        _ => Err(AdapterError::AccountData.into()),
    }
}

fn plan_width<const N: usize>(
    program_id: &Pubkey,
    market_key: &Pubkey,
    market_lamports: u64,
    active_bytes: &[u8],
    rent: &Rent,
) -> Result<TerminalProjectionPlan, ProgramError> {
    let active_len = active_bytes.len();
    let active =
        CategoricalMarketV1::<N>::decode(active_bytes).map_err(|_| AdapterError::AccountData)?;
    let root = active.root();
    let identity_digest = hash(&root.identity().to_bytes()).to_bytes();
    let (expected_market, _) =
        Pubkey::find_program_address(&[MARKET_SEED, &identity_digest], program_id);
    if market_key != &expected_market {
        return Err(AdapterError::AccountIdentity.into());
    }

    let terminal = TerminalCategoricalMarketV1::<N>::from_reclaimed_active(&active)
        .map_err(|_| AdapterError::MarketTransition)?;
    let terminal_bytes = terminal
        .to_bytes()
        .map_err(|_| AdapterError::MarketTransition)?;
    let expected_active_bytes =
        CategoricalMarketV1::<N>::encoded_len().map_err(|_| AdapterError::Arithmetic)?;
    if active_len != expected_active_bytes {
        return Err(AdapterError::AccountData.into());
    }
    let active_minimum = rent.minimum_balance(active_len);
    let terminal_minimum = rent.minimum_balance(TERMINAL_CATEGORICAL_MARKET_BYTES);
    let rent_credit_lamports = active_minimum
        .checked_sub(terminal_minimum)
        .ok_or(AdapterError::Arithmetic)?;
    if market_lamports < active_minimum {
        return Err(AdapterError::MarketTransition.into());
    }
    let market_lamports_after_credit = market_lamports
        .checked_sub(rent_credit_lamports)
        .ok_or(AdapterError::Arithmetic)?;
    if market_lamports_after_credit < terminal_minimum {
        return Err(AdapterError::MarketTransition.into());
    }

    Ok(TerminalProjectionPlan {
        market: *market_key,
        root,
        outcome_count: u8::try_from(N).map_err(|_| AdapterError::Arithmetic)?,
        active_bytes: active_len,
        active_state_digest: hash(active_bytes).to_bytes(),
        market_lamports_before_credit: market_lamports,
        market_lamports_after_credit,
        rent_credit_lamports,
        terminal_bytes,
    })
}

fn verify_terminal_projection(bytes: &[u8], outcome_count: u8) -> Result<(), ProgramError> {
    match outcome_count {
        2 => verify_terminal_width::<2>(bytes),
        3 => verify_terminal_width::<3>(bytes),
        4 => verify_terminal_width::<4>(bytes),
        5 => verify_terminal_width::<5>(bytes),
        6 => verify_terminal_width::<6>(bytes),
        7 => verify_terminal_width::<7>(bytes),
        8 => verify_terminal_width::<8>(bytes),
        9 => verify_terminal_width::<9>(bytes),
        10 => verify_terminal_width::<10>(bytes),
        11 => verify_terminal_width::<11>(bytes),
        12 => verify_terminal_width::<12>(bytes),
        13 => verify_terminal_width::<13>(bytes),
        14 => verify_terminal_width::<14>(bytes),
        15 => verify_terminal_width::<15>(bytes),
        16 => verify_terminal_width::<16>(bytes),
        _ => Err(AdapterError::MarketTransition.into()),
    }
}

fn verify_terminal_width<const N: usize>(bytes: &[u8]) -> Result<(), ProgramError> {
    TerminalCategoricalMarketV1::<N>::decode(bytes)
        .map(|_| ())
        .map_err(|_| AdapterError::MarketTransition.into())
}

#[cfg(test)]
mod tests {
    use dclutch_core_contract::{ContentId, MarketIdentity, MarketRoot, Phase};
    use dclutch_market_contract::market::CategoricalSettlementSummaryV1;
    use dclutch_product_contract::{ContentId as ProductContentId, terminal::ResolutionKind};
    use dclutch_rent_contract::RENT_CREDIT_PDA_DOMAIN_V1;
    use solana_program::rent::Rent;
    use std::{boxed::Box, vec, vec::Vec};

    use super::*;

    const GENERATION: u64 = 7;

    fn content_id(value: u8) -> ContentId {
        ContentId::new([value; 32]).expect("nonzero test ID")
    }

    fn product_content_id(value: u8) -> ProductContentId {
        ProductContentId::new([value; 32]).expect("nonzero test product ID")
    }

    fn identity() -> MarketIdentity {
        MarketIdentity::new(
            content_id(1),
            content_id(2),
            content_id(3),
            content_id(4),
            content_id(5),
            GENERATION,
        )
    }

    fn retired_market<const N: usize>() -> CategoricalMarketV1<N> {
        let root = MarketRoot::founding(identity(), [6; 32]).expect("root");
        let mut market =
            CategoricalMarketV1::new(root, 0, [0; N], CategoricalSettlementSummaryV1::empty())
                .expect("founding Market");
        market
            .transition_phase(GENERATION, Phase::Open)
            .expect("open");
        let settlement = CategoricalSettlementSummaryV1::resolved::<N>(
            product_content_id(8),
            ResolutionKind::Occurrence,
            0,
            1,
        )
        .expect("settlement");
        market
            .resolve_with_summary(GENERATION, settlement)
            .expect("resolve");
        market
            .transition_phase(GENERATION, Phase::Retiring)
            .expect("retiring");
        market
            .transition_phase(GENERATION, Phase::Retired)
            .expect("retired");
        market
    }

    fn active_bytes<const N: usize>() -> Vec<u8> {
        let market = retired_market::<N>();
        let mut bytes = vec![0; CategoricalMarketV1::<N>::encoded_len().expect("length")];
        market.encode(&mut bytes).expect("canonical active Market");
        bytes
    }

    fn expected_key(program_id: &Pubkey) -> Pubkey {
        let digest = hash(&identity().to_bytes()).to_bytes();
        Pubkey::find_program_address(&[MARKET_SEED, &digest], program_id).0
    }

    fn test_account(
        key: Pubkey,
        signer: bool,
        writable: bool,
        lamports: u64,
        data: Vec<u8>,
        owner: Pubkey,
        executable: bool,
    ) -> AccountInfo<'static> {
        AccountInfo::new(
            Box::leak(Box::new(key)),
            signer,
            writable,
            Box::leak(Box::new(lamports)),
            Box::leak(data.into_boxed_slice()),
            Box::leak(Box::new(owner)),
            executable,
        )
    }

    fn rent_account(rent: &Rent) -> AccountInfo<'static> {
        let mut account = test_account(
            sysvar::rent::ID,
            false,
            false,
            1,
            vec![0; Rent::size_of()],
            sysvar::ID,
            false,
        );
        assert_eq!(rent.to_account_info(&mut account), Some(()));
        account
    }

    fn rent_credit_account(
        program_id: &Pubkey,
        authority_bytes: [u8; 32],
        writable: bool,
        lamports: u64,
    ) -> (AccountInfo<'static>, RentCreditV1) {
        let authority = RefundAuthority::new(authority_bytes).expect("refund authority");
        let authority_seed = authority.to_bytes();
        let (key, bump) = Pubkey::find_program_address(
            &[RENT_CREDIT_PDA_DOMAIN_V1, authority_seed.as_slice()],
            program_id,
        );
        let state = RentCreditV1::new(authority, bump);
        (
            test_account(
                key,
                false,
                writable,
                lamports,
                state.to_bytes().to_vec(),
                *program_id,
                false,
            ),
            state,
        )
    }

    fn market_account<const N: usize>(
        program_id: &Pubkey,
        rent: &Rent,
        surplus: u64,
    ) -> AccountInfo<'static> {
        let data = active_bytes::<N>();
        let lamports = rent
            .minimum_balance(data.len())
            .checked_add(surplus)
            .expect("bounded fixture balance");
        test_account(
            expected_key(program_id),
            false,
            true,
            lamports,
            data,
            *program_id,
            false,
        )
    }

    #[test]
    fn plans_minimum_and_maximum_widths_with_the_exact_rent_delta() {
        let program_id = Pubkey::new_from_array([9; 32]);
        let rent = Rent::default();
        for (outcomes, active) in [(2u8, active_bytes::<2>()), (16u8, active_bytes::<16>())] {
            let active_minimum = rent.minimum_balance(active.len());
            let plan = plan_from_active_bytes(
                &program_id,
                &expected_key(&program_id),
                &program_id,
                active_minimum,
                &active,
                &rent,
            )
            .expect("authenticated plan");
            assert_eq!(plan.outcome_count, outcomes);
            assert_eq!(plan.active_bytes, active.len());
            assert_eq!(
                plan.rent_credit_lamports,
                active_minimum - rent.minimum_balance(TERMINAL_CATEGORICAL_MARKET_BYTES)
            );
            assert_eq!(
                plan.market_lamports_after_credit,
                rent.minimum_balance(TERMINAL_CATEGORICAL_MARKET_BYTES)
            );
            assert_eq!(plan.root.phase(), Phase::Retired);
        }
    }

    #[test]
    fn hostile_pda_header_and_rent_shortfall_are_refused_before_mutation() {
        let program_id = Pubkey::new_from_array([10; 32]);
        let rent = Rent::default();
        let active = active_bytes::<3>();
        let minimum = rent.minimum_balance(active.len());
        assert_eq!(
            plan_from_active_bytes(
                &program_id,
                &Pubkey::new_from_array([11; 32]),
                &program_id,
                minimum,
                &active,
                &rent,
            )
            .err(),
            Some(AdapterError::AccountIdentity.into())
        );
        assert_eq!(
            plan_from_active_bytes(
                &program_id,
                &expected_key(&program_id),
                &program_id,
                minimum.checked_sub(1).expect("positive active rent"),
                &active,
                &rent,
            )
            .err(),
            Some(AdapterError::MarketTransition.into())
        );

        let mut malformed = active;
        *malformed.get_mut(10).expect("active header byte") = 1;
        assert_eq!(
            plan_from_active_bytes(
                &program_id,
                &expected_key(&program_id),
                &program_id,
                minimum,
                &malformed,
                &rent,
            )
            .err(),
            Some(AdapterError::AccountData.into())
        );
    }

    #[test]
    fn adapter_binds_the_immutable_beneficiary_to_permanent_credit() {
        let program_id = Pubkey::new_from_array([12; 32]);
        let rent = Rent::default();
        let market = market_account::<4>(&program_id, &rent, 17);
        let (credit, expected_credit_state) = rent_credit_account(&program_id, [6; 32], true, 1);
        let rent_sysvar = rent_account(&rent);

        let plan = authenticate_terminal_compaction(
            &program_id,
            &market,
            &credit,
            &rent_sysvar,
            GENERATION,
        )
        .expect("authenticated terminal compaction");
        let expected_delta = rent
            .minimum_balance(market.data_len())
            .checked_sub(rent.minimum_balance(TERMINAL_CATEGORICAL_MARKET_BYTES))
            .expect("terminal is smaller");

        assert_eq!(plan.projection.root.rent_refund(), [6; 32]);
        assert_eq!(plan.rent_credit_state, expected_credit_state);
        assert_eq!(plan.credit_balance.credit_before(), 1);
        assert_eq!(plan.credit_balance.credited_lamports(), expected_delta);
        assert_eq!(
            plan.credit_balance.credit_after(),
            1u64.checked_add(expected_delta).expect("bounded delta")
        );
        assert_eq!(
            plan.projection.market_lamports_after_credit,
            rent.minimum_balance(TERMINAL_CATEGORICAL_MARKET_BYTES) + 17
        );
        assert_eq!(
            authenticate_terminal_compaction(
                &program_id,
                &market,
                &credit,
                &rent_sysvar,
                GENERATION + 1,
            )
            .err(),
            Some(AdapterError::ReplayMismatch.into())
        );
    }

    #[test]
    fn hostile_credit_binding_or_privilege_refuses_without_partial_mutation() {
        let program_id = Pubkey::new_from_array([13; 32]);
        let rent = Rent::default();
        let market = market_account::<3>(&program_id, &rent, 0);
        let market_before = market.lamports();
        let market_data_before = market.try_borrow_data().expect("market data").to_vec();
        let rent_sysvar = rent_account(&rent);
        let (readonly_credit, _) = rent_credit_account(&program_id, [6; 32], false, 1);
        assert_eq!(
            authenticate_terminal_compaction(
                &program_id,
                &market,
                &readonly_credit,
                &rent_sysvar,
                GENERATION,
            )
            .err(),
            Some(AdapterError::AccountPrivilege.into())
        );

        let (wrong_credit, _) = rent_credit_account(&program_id, [7; 32], true, 1);
        let wrong_credit_before = wrong_credit.lamports();
        let wrong_credit_data_before = wrong_credit
            .try_borrow_data()
            .expect("credit data")
            .to_vec();
        assert_eq!(
            authenticate_terminal_compaction(
                &program_id,
                &market,
                &wrong_credit,
                &rent_sysvar,
                GENERATION,
            )
            .err(),
            Some(AdapterError::AccountIdentity.into())
        );

        assert_eq!(market.lamports(), market_before);
        assert_eq!(
            market.try_borrow_data().expect("market data").as_ref(),
            market_data_before.as_slice()
        );
        assert_eq!(wrong_credit.lamports(), wrong_credit_before);
        assert_eq!(
            wrong_credit
                .try_borrow_data()
                .expect("credit data")
                .as_ref(),
            wrong_credit_data_before.as_slice()
        );
    }

    #[test]
    fn under_reserve_credit_accepts_top_up_but_overflow_refuses() {
        let program_id = Pubkey::new_from_array([14; 32]);
        let rent = Rent::default();
        let market = market_account::<2>(&program_id, &rent, 0);
        let rent_sysvar = rent_account(&rent);
        let (under_reserve, _) = rent_credit_account(&program_id, [6; 32], true, 0);
        assert!(
            authenticate_terminal_compaction(
                &program_id,
                &market,
                &under_reserve,
                &rent_sysvar,
                GENERATION,
            )
            .is_ok()
        );

        let (overflow, _) = rent_credit_account(&program_id, [6; 32], true, u64::MAX);
        let market_before = market.lamports();
        assert_eq!(
            authenticate_terminal_compaction(
                &program_id,
                &market,
                &overflow,
                &rent_sysvar,
                GENERATION,
            )
            .err(),
            Some(AdapterError::Arithmetic.into())
        );
        assert_eq!(market.lamports(), market_before);
        assert_eq!(overflow.lamports(), u64::MAX);
    }

    #[test]
    fn canonical_compact_binary_market_refuses_replay_before_any_mutation() {
        let program_id = Pubkey::new_from_array([16; 32]);
        let rent = Rent::default();
        let active = active_bytes::<2>();
        let active_minimum = rent.minimum_balance(active.len());
        let projection = plan_from_active_bytes(
            &program_id,
            &expected_key(&program_id),
            &program_id,
            active_minimum,
            &active,
            &rent,
        )
        .expect("active Market compacts once");
        assert_eq!(
            TerminalCategoricalMarketV1::<2>::decode(&projection.terminal_bytes),
            Ok(TerminalCategoricalMarketV1::<2>::from_reclaimed_active(
                &CategoricalMarketV1::<2>::decode(&active).expect("active Market")
            )
            .expect("terminal projection"))
        );

        let market = test_account(
            expected_key(&program_id),
            false,
            true,
            projection.market_lamports_after_credit,
            projection.terminal_bytes.to_vec(),
            program_id,
            false,
        );
        let (credit, _) =
            rent_credit_account(&program_id, [6; 32], true, projection.rent_credit_lamports);
        let rent_sysvar = rent_account(&rent);
        let market_before = market.lamports();
        let market_data_before = market.try_borrow_data().expect("terminal bytes").to_vec();
        let credit_before = credit.lamports();
        let credit_data_before = credit.try_borrow_data().expect("credit bytes").to_vec();

        assert_eq!(
            authenticate_terminal_compaction(
                &program_id,
                &market,
                &credit,
                &rent_sysvar,
                GENERATION,
            ),
            Err(AdapterError::ReplayMismatch.into())
        );
        assert_eq!(market.lamports(), market_before);
        assert_eq!(
            market.try_borrow_data().expect("terminal bytes").as_ref(),
            market_data_before.as_slice()
        );
        assert_eq!(credit.lamports(), credit_before);
        assert_eq!(
            credit.try_borrow_data().expect("credit bytes").as_ref(),
            credit_data_before.as_slice()
        );
    }

    #[test]
    fn stale_move_only_plan_refuses_before_balance_or_layout_mutation() {
        let program_id = Pubkey::new_from_array([15; 32]);
        let rent = Rent::default();
        let market = market_account::<5>(&program_id, &rent, 23);
        let (credit, _) = rent_credit_account(&program_id, [6; 32], true, 9);
        let rent_sysvar = rent_account(&rent);
        let plan = authenticate_terminal_compaction(
            &program_id,
            &market,
            &credit,
            &rent_sysvar,
            GENERATION,
        )
        .expect("authenticated plan");

        {
            let mut data = market.try_borrow_mut_data().expect("market data");
            let byte = data.get_mut(20).expect("active Market byte");
            *byte ^= 1;
        }
        let hostile_data = market.try_borrow_data().expect("market data").to_vec();
        let market_before = market.lamports();
        let credit_before = credit.lamports();
        let active_len = market.data_len();

        assert_eq!(
            apply_terminal_compaction(&program_id, &market, &credit, plan),
            Err(AdapterError::MarketTransition.into())
        );
        assert_eq!(market.lamports(), market_before);
        assert_eq!(credit.lamports(), credit_before);
        assert_eq!(market.data_len(), active_len);
        assert_eq!(
            market.try_borrow_data().expect("market data").as_ref(),
            hostile_data.as_slice()
        );
    }
}
