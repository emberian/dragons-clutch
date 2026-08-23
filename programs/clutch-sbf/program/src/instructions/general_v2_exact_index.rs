//! Capability-disabled SBF boundary for the counted exact settlement indexes.
//!
//! This module owns canonical PDA/owner/privilege authentication and the
//! account mutations for fresh creation, V1-root upgrade, bounded reads,
//! checked root transitions, and terminal sibling retirement.  No dispatcher
//! capability is enabled by defining these adapters.

use core::cell::Ref;

use clutch_general_v2_contract as contract;
use clutch_general_v2_contract::{
    DeletableRentOwnerV1, Id32, IndexedSettlementRootV1AccountV1, MarketBindingV2,
    SettlementRootV1AccountV1, Sha256BackendV1, INDEXED_SETTLEMENT_ROOT_BYTES_V1,
    MARKET_BINDING_ACCOUNT_BYTES_V2,
};
use clutch_general_v2_runtime::exact_index_plane::{
    authenticate_counted_exact_index_read_v1,
    authenticate_counted_exact_index_retirement_v1,
    construct_counted_exact_index_root_v1, indexed_pair_coverage_from_sealed_accounts_v1,
    retire_counted_exact_index_root_v1, AuthenticateCountedExactIndexReadInputV1,
    CloseExactIndexPlaneInputV1, ConstructExactIndexPlaneInputV1,
    CountedExactIndexRootCreatePostwritesV1, ExactIndexCloseAccountInputV1,
    ExactIndexCreateAccountInputV1, ExactIndexPlaneErrorV1, ExactIndexReadAccountInputV1,
    IndexedPairCoverageV1,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use crate::accounts::{expect_pda, require, require_signer, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::{
    require_creatable, require_system_program, RentParameters,
};
use crate::seeds;

use super::general_v2_settlement_producer_v5::{create_from_payer, encode_account};

#[derive(Clone, Copy, Debug)]
struct RuntimeSha256;

impl Sha256BackendV1 for RuntimeSha256 {
    fn sha256(&self, parts: &[&[u8]]) -> [u8; contract::ID_BYTES] {
        solana_sha256_hasher::hashv(parts).to_bytes()
    }
}

fn id(key: &Pubkey) -> Id32 {
    Id32::from_bytes(key.to_bytes())
}

fn exact<T>(result: Result<T, ExactIndexPlaneErrorV1>) -> Outcome<T> {
    result.map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))
}

fn borrow_data<'a, 'info>(account: &'a AccountInfo<'info>) -> Outcome<Ref<'a, [u8]>> {
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    Ok(Ref::map(data, |bytes| &**bytes))
}

fn require_program_account(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    writable: bool,
    exact_len: usize,
) -> Outcome<()> {
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    require(!account.is_signer, ClutchError::MismatchedState)?;
    require(
        account.is_writable == writable,
        if writable {
            ClutchError::NotWritable
        } else {
            ClutchError::UnexpectedWritable
        },
    )?;
    require(account.data_len() == exact_len, ClutchError::WrongDataLength)
}

fn canonical_index_accounts(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    root: &IndexedSettlementRootV1AccountV1,
    locator: &AccountInfo<'_>,
    adjacency: &AccountInfo<'_>,
) -> Outcome<((Pubkey, u8), (Pubkey, u8), (Pubkey, u8))> {
    let base = root.base();
    let canonical_root = seeds::general_v2_settlement_root_pda(
        program_id,
        &base.epoch().bytes(),
        &base.settlement_candidate_id().bytes(),
    );
    let root_bytes = root_account.key.to_bytes();
    let canonical_locator = seeds::general_v2_frozen_order_locator_pda(program_id, &root_bytes);
    let canonical_adjacency = seeds::general_v2_candidate_adjacency_pda(program_id, &root_bytes);
    expect_pda(root_account.key, canonical_root, Some(base.stored_bump()))?;
    require(
        *locator.key == canonical_locator.0
            && *adjacency.key == canonical_adjacency.0
            && root.locator_account() == id(locator.key)
            && root.adjacency_account() == id(adjacency.key),
        ClutchError::WrongPda,
    )?;
    Ok((canonical_root, canonical_locator, canonical_adjacency))
}

fn authenticated_join<'a>(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    root_body: &'a [u8],
    locator: &AccountInfo<'_>,
    locator_body: &'a [u8],
    adjacency: &AccountInfo<'_>,
    adjacency_body: &'a [u8],
    root_writable: bool,
    children_writable: bool,
) -> Outcome<AuthenticateCountedExactIndexReadInputV1<'a>> {
    require_program_account(
        program_id,
        root_account,
        root_writable,
        INDEXED_SETTLEMENT_ROOT_BYTES_V1,
    )?;
    require(locator.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(adjacency.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(
        locator.is_writable == children_writable
            && adjacency.is_writable == children_writable
            && !locator.is_signer
            && !adjacency.is_signer
            && !locator.executable
            && !adjacency.executable,
        ClutchError::MismatchedState,
    )?;
    let root = IndexedSettlementRootV1AccountV1::decode(root_body)?;
    let (root_pda, locator_pda, adjacency_pda) =
        canonical_index_accounts(program_id, root_account, &root, locator, adjacency)?;
    Ok(AuthenticateCountedExactIndexReadInputV1 {
        program_id: id(program_id),
        root: ExactIndexReadAccountInputV1 {
            account: id(root_account.key),
            body: root_body,
            owner: id(root_account.owner),
            canonical_account: id(&root_pda.0),
            canonical_bump: root_pda.1,
            writable: root_account.is_writable,
            executable: root_account.executable,
        },
        locator: ExactIndexReadAccountInputV1 {
            account: id(locator.key),
            body: locator_body,
            owner: id(locator.owner),
            canonical_account: id(&locator_pda.0),
            canonical_bump: locator_pda.1,
            writable: locator.is_writable,
            executable: locator.executable,
        },
        adjacency: ExactIndexReadAccountInputV1 {
            account: id(adjacency.key),
            body: adjacency_body,
            owner: id(adjacency.owner),
            canonical_account: id(&adjacency_pda.0),
            canonical_bump: adjacency_pda.1,
            writable: adjacency.is_writable,
            executable: adjacency.executable,
        },
    })
}

/// Full-body-ID-authenticated bounded pair projection from three read-only PDAs.
pub fn read_pair_coverage_v1(
    program_id: &Pubkey,
    root: &AccountInfo<'_>,
    locator: &AccountInfo<'_>,
    adjacency: &AccountInfo<'_>,
    buy_order: u8,
    sell_order: u8,
) -> Outcome<IndexedPairCoverageV1> {
    let root_body = borrow_data(root)?;
    let locator_body = borrow_data(locator)?;
    let adjacency_body = borrow_data(adjacency)?;
    let joined = authenticated_join(
        program_id,
        root,
        &root_body,
        locator,
        &locator_body,
        adjacency,
        &adjacency_body,
        false,
        false,
    )?;
    let sealed = exact(authenticate_counted_exact_index_read_v1(joined))?;
    exact(indexed_pair_coverage_from_sealed_accounts_v1(
        sealed, buy_order, sell_order,
    ))
}

fn require_fresh_child(
    program_id: &Pubkey,
    root: &AccountInfo<'_>,
    child: &AccountInfo<'_>,
    adjacency: bool,
) -> Outcome<(Pubkey, u8)> {
    require_creatable(child)?;
    require(
        child.is_writable && !child.is_signer && !child.executable,
        ClutchError::MismatchedState,
    )?;
    let root_bytes = root.key.to_bytes();
    let canonical = if adjacency {
        seeds::general_v2_candidate_adjacency_pda(program_id, &root_bytes)
    } else {
        seeds::general_v2_frozen_order_locator_pda(program_id, &root_bytes)
    };
    require(*child.key == canonical.0, ClutchError::WrongPda)?;
    Ok(canonical)
}

fn create_input(
    program_id: &Pubkey,
    system_program: &AccountInfo<'_>,
    payer: &AccountInfo<'_>,
    target: &AccountInfo<'_>,
    rent_minimum: u64,
    bump: u8,
) -> ExactIndexCreateAccountInputV1 {
    ExactIndexCreateAccountInputV1 {
        account: id(target.key),
        program_id: id(program_id),
        system_program: id(system_program.key),
        payer: id(payer.key),
        payer_lamports: payer.lamports(),
        target_lamports: target.lamports(),
        target_owner: id(target.owner),
        target_data_len: target.data_len(),
        target_writable: target.is_writable,
        target_executable: target.executable,
        rent_exempt_minimum: rent_minimum,
        stored_bump: bump,
    }
}

fn require_create_input_matches(
    expected: ExactIndexCreateAccountInputV1,
    actual: ExactIndexCreateAccountInputV1,
) -> Outcome<()> {
    require(expected == actual, ClutchError::MismatchedState)
}

/// Atomically create the 1,196-byte root and both exact active-width children.
/// The caller supplies only immutable traversal inputs already authenticated by
/// its action-specific account frame; every persisted row and body ID remains
/// pure-derived.
#[allow(clippy::too_many_arguments)]
pub(crate) fn create_fresh_counted_root_v1<'info>(
    program_id: &Pubkey,
    payer: &AccountInfo<'info>,
    root: &AccountInfo<'info>,
    locator: &AccountInfo<'info>,
    adjacency: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    rent: &RentParameters,
    base: &SettlementRootV1AccountV1,
    neutral_sink: Id32,
    input: ConstructExactIndexPlaneInputV1<'_>,
) -> Outcome<CountedExactIndexRootCreatePostwritesV1> {
    require_signer(payer)?;
    require(payer.is_writable, ClutchError::NotWritable)?;
    require_system_program(system_program)?;
    require_creatable(root)?;
    require(root.is_writable && !root.is_signer, ClutchError::MismatchedState)?;
    let root_pda = seeds::general_v2_settlement_root_pda(
        program_id,
        &base.epoch().bytes(),
        &base.settlement_candidate_id().bytes(),
    );
    expect_pda(root.key, root_pda, Some(base.stored_bump()))?;
    let locator_pda = require_fresh_child(program_id, root, locator, false)?;
    let adjacency_pda = require_fresh_child(program_id, root, adjacency, true)?;
    let actual_locator = create_input(
        program_id,
        system_program,
        payer,
        locator,
        input.locator_create.rent_exempt_minimum,
        locator_pda.1,
    );
    let actual_adjacency = create_input(
        program_id,
        system_program,
        payer,
        adjacency,
        input.adjacency_create.rent_exempt_minimum,
        adjacency_pda.1,
    );
    require_create_input_matches(input.locator_create, actual_locator)?;
    require_create_input_matches(input.adjacency_create, actual_adjacency)?;
    require(
        input.settlement_root_account == id(root.key) && input.settlement_root == base,
        ClutchError::MismatchedState,
    )?;
    let root_minimum = rent.minimum_balance(INDEXED_SETTLEMENT_ROOT_BYTES_V1)?;
    let root_rent = contract::prepare_fresh_indexed_settlement_root_rent_v1(
        base,
        id(root.key),
        root.lamports(),
        root_minimum,
        payer.lamports(),
        neutral_sink,
        &RuntimeSha256,
    )?;
    let plan = exact(construct_counted_exact_index_root_v1(root_rent, input))?;
    require(
        rent.minimum_balance(plan.index_postwrites().locator_data_len().map_err(|_| {
            Refusal::Adapter(ClutchError::MismatchedState)
        })?)? == actual_locator.rent_exempt_minimum
            && rent.minimum_balance(
                plan.index_postwrites()
                    .adjacency_data_len()
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
            )? == actual_adjacency.rent_exempt_minimum,
        ClutchError::MismatchedState,
    )?;

    let root_epoch = base.epoch().bytes();
    let candidate = base.settlement_candidate_id().bytes();
    let root_bump = [root_pda.1];
    create_from_payer(
        program_id,
        payer,
        root,
        system_program,
        rent,
        INDEXED_SETTLEMENT_ROOT_BYTES_V1,
        root_rent.rent_after(),
        &[
            seeds::SEED_GENERAL_V2_SETTLEMENT_ROOT,
            &root_epoch,
            &candidate,
            &root_bump,
        ],
    )?;
    let root_bytes = root.key.to_bytes();
    let locator_bump = [locator_pda.1];
    create_from_payer(
        program_id,
        payer,
        locator,
        system_program,
        rent,
        plan.index_postwrites()
            .locator_data_len()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        DeletableRentOwnerV1 {
            payer: actual_locator.payer,
            refundable_principal: actual_locator.rent_exempt_minimum,
            donation_floor: actual_locator.target_lamports,
        },
        &[
            seeds::SEED_GENERAL_V2_FROZEN_ORDER_LOCATOR,
            &root_bytes,
            &locator_bump,
        ],
    )?;
    let adjacency_bump = [adjacency_pda.1];
    create_from_payer(
        program_id,
        payer,
        adjacency,
        system_program,
        rent,
        plan.index_postwrites()
            .adjacency_data_len()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        DeletableRentOwnerV1 {
            payer: actual_adjacency.payer,
            refundable_principal: actual_adjacency.rent_exempt_minimum,
            donation_floor: actual_adjacency.target_lamports,
        },
        &[
            seeds::SEED_GENERAL_V2_CANDIDATE_ADJACENCY,
            &root_bytes,
            &adjacency_bump,
        ],
    )?;
    encode_account(root, |out| plan.indexed_root().encode(out))?;
    {
        let mut data = locator
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        exact(plan.index_postwrites().encode_locator(&mut data))?;
    }
    {
        let mut data = adjacency
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        exact(plan.index_postwrites().encode_adjacency(&mut data))?;
    }
    Ok(plan)
}

fn authenticate_market_binding_v2(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
) -> Outcome<MarketBindingV2> {
    require_program_account(
        program_id,
        account,
        false,
        MARKET_BINDING_ACCOUNT_BYTES_V2,
    )?;
    let binding = MarketBindingV2::decode(&borrow_data(account)?)?;
    let base = binding.base();
    expect_pda(
        account.key,
        seeds::general_v2_market_binding_pda(program_id, &base.market_instance_v2_id.bytes()),
        Some(base.stored_bump),
    )?;
    Ok(binding)
}

/// Retire both exact children, refund each persisted principal owner, route all
/// donation/excess to the immutable MarketBinding sink, and advance the root's
/// exact child partition atomically.
#[allow(clippy::too_many_arguments)]
pub(crate) fn retire_exact_index_pair_v1(
    program_id: &Pubkey,
    root: &AccountInfo<'_>,
    locator: &AccountInfo<'_>,
    adjacency: &AccountInfo<'_>,
    market_binding_account: &AccountInfo<'_>,
    locator_payer: &AccountInfo<'_>,
    adjacency_payer: &AccountInfo<'_>,
    neutral_sink: &AccountInfo<'_>,
) -> Outcome<()> {
    let market_binding = authenticate_market_binding_v2(program_id, market_binding_account)?;
    let root_body = borrow_data(root)?;
    let locator_body = borrow_data(locator)?;
    let adjacency_body = borrow_data(adjacency)?;
    let joined = authenticated_join(
        program_id,
        root,
        &root_body,
        locator,
        &locator_body,
        adjacency,
        &adjacency_body,
        true,
        true,
    )?;
    let mutation = exact(authenticate_counted_exact_index_retirement_v1(joined))?;
    let indexed_root = *mutation.indexed_root();
    let plan = exact(retire_counted_exact_index_root_v1(
        &indexed_root,
        CloseExactIndexPlaneInputV1 {
            settlement_root_account: id(root.key),
            settlement_root: indexed_root.base(),
            market_binding_account: id(market_binding_account.key),
            market_binding: &market_binding,
            locator: ExactIndexCloseAccountInputV1 {
                account: id(locator.key),
                body: &locator_body,
                lamports: locator.lamports(),
                owner: id(locator.owner),
                program_id: id(program_id),
                writable: locator.is_writable,
                executable: locator.executable,
            },
            adjacency: ExactIndexCloseAccountInputV1 {
                account: id(adjacency.key),
                body: &adjacency_body,
                lamports: adjacency.lamports(),
                owner: id(adjacency.owner),
                program_id: id(program_id),
                writable: adjacency.is_writable,
                executable: adjacency.executable,
            },
        },
    ))?;
    let close = plan.close_postwrites();
    let credits = [
        close.locator_principal_credit(),
        close.locator_donation_credit(),
        close.adjacency_principal_credit(),
        close.adjacency_donation_credit(),
    ];
    require(
        id(locator_payer.key) == credits[0].recipient()
            && id(neutral_sink.key) == credits[1].recipient()
            && id(adjacency_payer.key) == credits[2].recipient()
            && id(neutral_sink.key) == credits[3].recipient()
            && locator_payer.is_writable
            && adjacency_payer.is_writable
            && neutral_sink.is_writable
            && !locator_payer.executable
            && !adjacency_payer.executable
            && !neutral_sink.executable,
        ClutchError::MismatchedState,
    )?;
    precheck_credit_recipient(locator_payer, &credits)?;
    precheck_credit_recipient(adjacency_payer, &credits)?;
    precheck_credit_recipient(neutral_sink, &credits)?;
    drop(adjacency_body);
    drop(locator_body);
    drop(root_body);
    encode_account(root, |out| plan.indexed_root_poststate().encode(out))?;
    credit_lamports(locator_payer, credits[0].amount())?;
    credit_lamports(neutral_sink, credits[1].amount())?;
    credit_lamports(adjacency_payer, credits[2].amount())?;
    credit_lamports(neutral_sink, credits[3].amount())?;
    close_program_account(locator)?;
    close_program_account(adjacency)
}

fn precheck_credit_recipient(
    account: &AccountInfo<'_>,
    credits: &[clutch_general_v2_runtime::exact_index_plane::ExactIndexCloseCreditV1; 4],
) -> Outcome<()> {
    let recipient = id(account.key);
    let mut after = account.lamports();
    for credit in credits {
        if credit.recipient() == recipient {
            after = after
                .checked_add(credit.amount())
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        }
    }
    let _checked_postbalance = after;
    Ok(())
}

fn credit_lamports(account: &AccountInfo<'_>, amount: u64) -> Outcome<()> {
    let after = account
        .lamports()
        .checked_add(amount)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let mut lamports = account
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    **lamports = after;
    Ok(())
}

fn close_program_account(account: &AccountInfo<'_>) -> Outcome<()> {
    {
        let mut lamports = account
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        **lamports = 0;
    }
    account
        .resize(0)
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    account.assign(&SYSTEM_PROGRAM_ID);
    require(
        account.lamports() == 0
            && account.data_len() == 0
            && *account.owner == SYSTEM_PROGRAM_ID,
        ClutchError::MismatchedState,
    )
}
