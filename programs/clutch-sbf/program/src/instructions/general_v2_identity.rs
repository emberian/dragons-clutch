//! Non-production General V2 empty-book candidate-identity laboratory.
//!
//! This module deliberately exposes only the eight extension actions admitted
//! by its mutually exclusive capability profile. It proves a signed,
//! committing local-SBF identity lifecycle over one genesis-assisted Product
//! market and an empty RelationV2 book. It creates no orders, positions,
//! entitlements, receipts, pots, token accounts, trades, or settlement.

use crate::accounts::{require, require_count, require_signer, Outcome};
use crate::capabilities;
use crate::error::{ClutchError, Refusal};
use crate::instructions::artifact::read_clock_slot;
use crate::instructions::genesis::{
    allocate_data, assign_data, read_rent, require_creatable, require_system_program,
    transfer_data, RentParameters, MAX_PERMITTED_DATA_INCREASE, SYSTEM_PROGRAM_ID,
};
use crate::seeds;
use clutch_batch::relation_v2::{
    verify_economic_candidate_v2, EconomicBookV2, EconomicCandidateV2, EconomicDomainV2,
    PricePreconditionV2,
};
use clutch_general_v2_contract as contract;
use clutch_general_v2_contract::{
    decode_identity_lab_payload_v1, DeletableRentOwnerV1, GeneralEpochPhaseV1, Id32,
    IdentityLabPayloadV1, Sha256BackendV1, WriteCandidateFeedPayloadV1,
};
use clutch_price_measure::{
    verify_quantized_price_measure_v3_smooth, AdapterBindingsV3, PriceVectorV3,
    QuantizedAtomWitnessV3,
};
use clutch_product_series::{
    FixedCodec, MarketGenesisProfileV2, NativeClaimBasisV1, PriceMeasurePolicyV1,
    QuantizedEdgePolicyV1, BASIS_BYTES, MARKET_GENESIS_PROFILE_V2_BYTES,
    PRICE_MEASURE_POLICY_BYTES,
};
use clutch_solana_layout::registry::GeneralV2Action;
use solana_account_info::AccountInfo;
use solana_cpi::invoke_signed;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

/// Native Solana SHA-256 adapter for the pure contract's byte-exact backend seam.
#[derive(Clone, Copy, Debug)]
struct RuntimeSha256;

impl Sha256BackendV1 for RuntimeSha256 {
    fn sha256(&self, parts: &[&[u8]]) -> [u8; contract::ID_BYTES] {
        solana_sha256_hasher::hashv(parts).to_bytes()
    }
}

/// Decode one strict action payload and enter exactly one reviewed handler.
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    action: GeneralV2Action,
    payload: &[u8],
) -> Outcome<()> {
    require(sequence == 0, ClutchError::Replay)?;
    require(
        capabilities::extension_intent_action_enabled(74, 1, action.tag()),
        ClutchError::UnsupportedInstruction,
    )?;
    match decode_identity_lab_payload_v1(action.tag(), payload)? {
        IdentityLabPayloadV1::InitEpoch(request) => init_epoch(program_id, accounts, request),
        IdentityLabPayloadV1::FreezeEpoch(request) => freeze_epoch(program_id, accounts, request),
        IdentityLabPayloadV1::BeginCandidate(request) => {
            begin_candidate(program_id, accounts, request)
        }
        IdentityLabPayloadV1::WriteCandidateFeed(WriteCandidateFeedPayloadV1::Open(request)) => {
            open_candidate_feed(program_id, accounts, request)
        }
        IdentityLabPayloadV1::WriteCandidateFeed(WriteCandidateFeedPayloadV1::Segment(request)) => {
            write_candidate_feed_segment(program_id, accounts, request)
        }
        IdentityLabPayloadV1::SealCandidate(request) => {
            seal_candidate(program_id, accounts, request)
        }
        IdentityLabPayloadV1::InitClearWork(request) => {
            init_clear_work(program_id, accounts, request)
        }
        IdentityLabPayloadV1::CompleteCandidateVerification(request) => {
            complete_candidate_verification(program_id, accounts, request)
        }
        IdentityLabPayloadV1::FinalizeSelection(request) => {
            finalize_selection(program_id, accounts, request)
        }
    }
}

fn id(key: &Pubkey) -> Id32 {
    Id32::from_bytes(key.to_bytes())
}

fn require_role(
    program_id: &Pubkey,
    account: &AccountInfo,
    writable: bool,
    exact_len: usize,
) -> Outcome<()> {
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    require(
        account.is_writable == writable,
        if writable {
            ClutchError::NotWritable
        } else {
            ClutchError::UnexpectedWritable
        },
    )?;
    require(
        account.data_len() == exact_len,
        ClutchError::WrongDataLength,
    )
}

fn require_readonly_artifact(
    program_id: &Pubkey,
    account: &AccountInfo,
    exact_len: usize,
) -> Outcome<()> {
    require_role(program_id, account, false, exact_len)
}

fn require_writable_destination(account: &AccountInfo) -> Outcome<()> {
    require(account.is_writable, ClutchError::NotWritable)?;
    require(!account.executable, ClutchError::ExecutableAccount)
}

fn require_readonly_actor(account: &AccountInfo) -> Outcome<()> {
    require(!account.is_writable, ClutchError::UnexpectedWritable)?;
    require(!account.executable, ClutchError::ExecutableAccount)
}

fn require_distinct_pairs(accounts: &[AccountInfo], pairs: &[(usize, usize)]) -> Outcome<()> {
    for (left, right) in pairs {
        require(
            accounts[*left].key != accounts[*right].key,
            ClutchError::AccountAlias,
        )?;
    }
    Ok(())
}

fn borrow_data<'a, 'b>(account: &'a AccountInfo<'b>) -> Outcome<core::cell::Ref<'a, [u8]>> {
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    Ok(core::cell::Ref::map(data, |bytes| &**bytes))
}

fn encode_account(
    account: &AccountInfo,
    encode: impl FnOnce(&mut [u8]) -> Result<(), contract::CodecError>,
) -> Outcome<()> {
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    encode(&mut data)?;
    Ok(())
}

fn rent_owner(
    payer: &AccountInfo,
    target: &AccountInfo,
    rent: &RentParameters,
    space: usize,
) -> Outcome<DeletableRentOwnerV1> {
    Ok(DeletableRentOwnerV1 {
        payer: id(payer.key),
        refundable_principal: rent.minimum_balance(space)?,
        donation_floor: target.lamports(),
    })
}

#[allow(clippy::too_many_arguments)]
fn create_from_payer<'a>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    target: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent: &RentParameters,
    space: usize,
    owner: DeletableRentOwnerV1,
    extra_deposit: u64,
    signer_seeds: &[&[u8]],
) -> Outcome<()> {
    require_creatable(target)?;
    require(
        space <= MAX_PERMITTED_DATA_INCREASE,
        ClutchError::AccountCreationFailed,
    )?;
    let principal = rent.minimum_balance(space)?;
    require(
        owner.payer == id(payer.key)
            && owner.refundable_principal == principal
            && owner.donation_floor == target.lamports(),
        ClutchError::MismatchedState,
    )?;
    let debit = principal
        .checked_add(extra_deposit)
        .ok_or(ClutchError::Arithmetic)?;
    let expected = target
        .lamports()
        .checked_add(debit)
        .ok_or(ClutchError::Arithmetic)?;
    let transfer = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &transfer_data(debit),
        vec![
            AccountMeta::new(*payer.key, true),
            AccountMeta::new(*target.key, false),
        ],
    );
    invoke_signed(
        &transfer,
        &[payer.clone(), target.clone(), system_program.clone()],
        &[],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        target.lamports() == expected,
        ClutchError::AccountCreationFailed,
    )?;
    allocate_and_assign(
        program_id,
        target,
        system_program,
        space,
        expected,
        signer_seeds,
    )
}

fn allocate_and_assign<'a>(
    program_id: &Pubkey,
    target: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    space: usize,
    expected_lamports: u64,
    signer_seeds: &[&[u8]],
) -> Outcome<()> {
    let allocate = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &allocate_data(space),
        vec![AccountMeta::new(*target.key, true)],
    );
    invoke_signed(
        &allocate,
        &[target.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        target.data_len() == space
            && *target.owner == SYSTEM_PROGRAM_ID
            && target.lamports() == expected_lamports,
        ClutchError::AccountCreationFailed,
    )?;
    let assign = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &assign_data(program_id),
        vec![AccountMeta::new(*target.key, true)],
    );
    invoke_signed(
        &assign,
        &[target.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        target.owner == program_id && target.lamports() == expected_lamports,
        ClutchError::AccountCreationFailed,
    )
}

fn move_lamports(source: &AccountInfo, destination: &AccountInfo, amount: u64) -> Outcome<()> {
    require_writable_destination(source)?;
    require_writable_destination(destination)?;
    require(source.key != destination.key, ClutchError::AccountAlias)?;
    let source_after = source
        .lamports()
        .checked_sub(amount)
        .ok_or(ClutchError::Arithmetic)?;
    let destination_after = destination
        .lamports()
        .checked_add(amount)
        .ok_or(ClutchError::Arithmetic)?;
    **source
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))? = source_after;
    **destination
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))? = destination_after;
    Ok(())
}

fn transfer_from_signer<'a>(
    payer: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    amount: u64,
) -> Outcome<()> {
    let expected = destination
        .lamports()
        .checked_add(amount)
        .ok_or(ClutchError::Arithmetic)?;
    let transfer = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &transfer_data(amount),
        vec![
            AccountMeta::new(*payer.key, true),
            AccountMeta::new(*destination.key, false),
        ],
    );
    invoke_signed(
        &transfer,
        &[payer.clone(), destination.clone(), system_program.clone()],
        &[],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        destination.lamports() == expected,
        ClutchError::AccountCreationFailed,
    )
}

#[allow(clippy::too_many_arguments)]
fn create_from_program_escrow<'a>(
    program_id: &Pubkey,
    source: &AccountInfo<'a>,
    target: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent: &RentParameters,
    space: usize,
    owner: DeletableRentOwnerV1,
    debit: u64,
    signer_seeds: &[&[u8]],
) -> Outcome<()> {
    require_creatable(target)?;
    require(source.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(
        space <= MAX_PERMITTED_DATA_INCREASE,
        ClutchError::AccountCreationFailed,
    )?;
    require(
        owner.refundable_principal == rent.minimum_balance(space)?
            && owner.donation_floor == target.lamports()
            && debit >= owner.refundable_principal,
        ClutchError::MismatchedState,
    )?;
    let expected = target
        .lamports()
        .checked_add(debit)
        .ok_or(ClutchError::Arithmetic)?;
    move_lamports(source, target, debit)?;
    require(
        target.lamports() == expected,
        ClutchError::AccountCreationFailed,
    )?;
    allocate_and_assign(
        program_id,
        target,
        system_program,
        space,
        expected,
        signer_seeds,
    )
}

#[derive(Clone, Copy, Debug)]
struct ProductFacts {
    coordinate_domain_min: u128,
    coordinate_domain_max: u128,
}

#[inline(never)]
fn authenticate_product(
    program_id: &Pubkey,
    binding: contract::MarketBindingV1,
    basis_account: &AccountInfo,
    genesis_account: &AccountInfo,
    policy_account: &AccountInfo,
) -> Outcome<ProductFacts> {
    require_readonly_artifact(program_id, basis_account, BASIS_BYTES)?;
    require_readonly_artifact(program_id, genesis_account, MARKET_GENESIS_PROFILE_V2_BYTES)?;
    require_readonly_artifact(program_id, policy_account, PRICE_MEASURE_POLICY_BYTES)?;
    let basis_data = borrow_data(basis_account)?;
    let basis = NativeClaimBasisV1::decode(&basis_data).map_err(|_| ClutchError::NonCanonical)?;
    let genesis_data = borrow_data(genesis_account)?;
    let genesis =
        MarketGenesisProfileV2::decode(&genesis_data).map_err(|_| ClutchError::NonCanonical)?;
    let policy_data = borrow_data(policy_account)?;
    let policy =
        PriceMeasurePolicyV1::decode(&policy_data).map_err(|_| ClutchError::NonCanonical)?;
    require(
        basis.id().map_err(|_| ClutchError::NonCanonical)?.bytes()
            == binding.native_claim_basis_id.bytes()
            && genesis.id().map_err(|_| ClutchError::NonCanonical)?.bytes()
                == binding.market_genesis_profile_v2_id.bytes()
            && policy.id().map_err(|_| ClutchError::NonCanonical)?.bytes()
                == binding.price_measure_policy_v1_id.bytes(),
        ClutchError::MismatchedState,
    )?;
    genesis
        .validate_bindings(&basis, &policy)
        .map_err(|_| ClutchError::MismatchedState)?;
    require(
        basis.basis_degree == binding.basis_degree
            && basis.outcome_count == binding.outcome_count
            && genesis.relation_policy_id.bytes() == binding.relation_policy_id.bytes()
            && genesis.score_policy_id.bytes() == binding.score_policy_id.bytes()
            && genesis.capability_profile_id.bytes() == capabilities::PROFILE_ID,
        ClutchError::MismatchedState,
    )?;
    Ok(ProductFacts {
        coordinate_domain_min: genesis.coordinate_domain_min,
        coordinate_domain_max: genesis.coordinate_domain_max,
    })
}

// InitEpoch adds the two exact Product bodies omitted by the earlier design
// table. Bare IDs cannot authenticate coordinate bounds or checker behavior.
const INIT_ACCOUNT_COUNT: usize = 13;

#[inline(never)]
fn init_epoch(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    request: contract::InitEpochPayloadV1,
) -> Outcome<()> {
    require_count(accounts, INIT_ACCOUNT_COUNT)?;
    require_signer(&accounts[0])?;
    require_writable_destination(&accounts[0])?;
    require_role(
        program_id,
        &accounts[1],
        false,
        contract::MARKET_BINDING_ACCOUNT_BYTES,
    )?;
    require_role(
        program_id,
        &accounts[2],
        true,
        contract::MARKET_RUNTIME_ACCOUNT_BYTES,
    )?;
    require_system_program(&accounts[10])?;
    let rent = read_rent(&accounts[11])?;
    let slot = read_clock_slot(&accounts[12])?;
    require(request.freeze_deadline_slot > slot, ClutchError::NotActive)?;
    for target in &accounts[3..=6] {
        require_creatable(target)?;
    }
    require_distinct_pairs(
        accounts,
        &[
            (1, 2),
            (1, 3),
            (1, 4),
            (1, 5),
            (1, 6),
            (2, 3),
            (2, 4),
            (2, 5),
            (2, 6),
            (3, 4),
            (3, 5),
            (3, 6),
            (4, 5),
            (4, 6),
            (5, 6),
        ],
    )?;

    let binding = contract::MarketBindingV1::decode(&borrow_data(&accounts[1])?)?;
    let runtime = contract::MarketRuntimeV3AccountV1::decode(&borrow_data(&accounts[2])?)?;
    require(
        binding.market_instance_v2_id == request.market_instance_v2_id
            && runtime.market_binding == id(accounts[1].key)
            && runtime.market_instance_v2_id == request.market_instance_v2_id
            && runtime.next_epoch_index == request.epoch_index
            && binding.market == id(accounts[2].key),
        ClutchError::MismatchedState,
    )?;
    let binding_pda =
        seeds::general_v2_market_binding_pda(program_id, &request.market_instance_v2_id.bytes());
    require(
        *accounts[1].key == binding_pda.0 && binding.stored_bump == binding_pda.1,
        ClutchError::WrongPda,
    )?;
    let runtime_pda = seeds::general_v2_market_runtime_pda(program_id, &accounts[1].key.to_bytes());
    require(
        *accounts[2].key == runtime_pda.0 && runtime.stored_bump == runtime_pda.1,
        ClutchError::WrongPda,
    )?;
    let product = authenticate_product(
        program_id,
        binding,
        &accounts[7],
        &accounts[8],
        &accounts[9],
    )?;

    let epoch_pda =
        seeds::general_v2_epoch_pda(program_id, &accounts[1].key.to_bytes(), request.epoch_index);
    require(*accounts[3].key == epoch_pda.0, ClutchError::WrongPda)?;
    let domain_pda = seeds::general_v2_economic_domain_pda(program_id, &epoch_pda.0.to_bytes());
    let window_pda = seeds::general_v2_window_pda(program_id, &epoch_pda.0.to_bytes());
    let budget_pda = seeds::general_v2_budget_pda(program_id, &epoch_pda.0.to_bytes());
    require(*accounts[4].key == domain_pda.0, ClutchError::WrongPda)?;
    require(*accounts[5].key == window_pda.0, ClutchError::WrongPda)?;
    require(*accounts[6].key == budget_pda.0, ClutchError::WrongPda)?;

    let epoch_rent = rent_owner(
        &accounts[0],
        &accounts[3],
        &rent,
        contract::GENERAL_EPOCH_ACCOUNT_BYTES,
    )?;
    let domain_rent = rent_owner(
        &accounts[0],
        &accounts[4],
        &rent,
        contract::ECONOMIC_DOMAIN_ACCOUNT_BYTES,
    )?;
    let window_rent = rent_owner(
        &accounts[0],
        &accounts[5],
        &rent,
        contract::WINDOW_ACCOUNT_BYTES,
    )?;
    let budget_rent = rent_owner(
        &accounts[0],
        &accounts[6],
        &rent,
        contract::EPOCH_BUDGET_ACCOUNT_BYTES,
    )?;
    let selected_rent = rent.minimum_balance(contract::SELECTED_CANDIDATE_ACCOUNT_BYTES)?;
    let budget_extra = binding
        .freeze_reward
        .checked_add(binding.finalize_reward)
        .and_then(|v| v.checked_add(binding.solver_prize))
        .and_then(|v| v.checked_add(binding.root_close_reward))
        .and_then(|v| v.checked_add(selected_rent))
        .ok_or(ClutchError::Arithmetic)?;

    let post = contract::init_epoch_poststate_v1(
        &RuntimeSha256,
        binding,
        runtime,
        contract::InitEpochTransitionV1 {
            market_binding: id(accounts[1].key),
            market_runtime: id(accounts[2].key),
            epoch: id(accounts[3].key),
            economic_domain: id(accounts[4].key),
            window: id(accounts[5].key),
            budget: id(accounts[6].key),
            funding_payer: id(accounts[0].key),
            payload: request,
            coordinate_domain_min: product.coordinate_domain_min,
            coordinate_domain_max: product.coordinate_domain_max,
            epoch_rent,
            economic_domain_rent: domain_rent,
            window_rent,
            budget_rent,
            selected_candidate_rent_principal: selected_rent,
            epoch_bump: epoch_pda.1,
            economic_domain_bump: domain_pda.1,
            window_bump: window_pda.1,
            budget_bump: budget_pda.1,
        },
    )?;
    let epoch = post.epoch;
    let domain = post.economic_domain;
    let window = post.window;
    let budget = post.budget;
    let runtime_after = post.market_runtime;

    let epoch_index_le = request.epoch_index.to_le_bytes();
    let epoch_seeds: [&[u8]; 4] = [
        seeds::SEED_GENERAL_V2_EPOCH,
        &accounts[1].key.to_bytes(),
        &epoch_index_le,
        &[epoch_pda.1],
    ];
    create_from_payer(
        program_id,
        &accounts[0],
        &accounts[3],
        &accounts[10],
        &rent,
        contract::GENERAL_EPOCH_ACCOUNT_BYTES,
        epoch_rent,
        0,
        &epoch_seeds,
    )?;
    let domain_seeds: [&[u8]; 3] = [
        seeds::SEED_GENERAL_V2_ECONOMIC_DOMAIN,
        &accounts[3].key.to_bytes(),
        &[domain_pda.1],
    ];
    create_from_payer(
        program_id,
        &accounts[0],
        &accounts[4],
        &accounts[10],
        &rent,
        contract::ECONOMIC_DOMAIN_ACCOUNT_BYTES,
        domain_rent,
        0,
        &domain_seeds,
    )?;
    let window_seeds: [&[u8]; 3] = [
        seeds::SEED_GENERAL_V2_WINDOW,
        &accounts[3].key.to_bytes(),
        &[window_pda.1],
    ];
    create_from_payer(
        program_id,
        &accounts[0],
        &accounts[5],
        &accounts[10],
        &rent,
        contract::WINDOW_ACCOUNT_BYTES,
        window_rent,
        0,
        &window_seeds,
    )?;
    let budget_seeds: [&[u8]; 3] = [
        seeds::SEED_GENERAL_V2_BUDGET,
        &accounts[3].key.to_bytes(),
        &[budget_pda.1],
    ];
    create_from_payer(
        program_id,
        &accounts[0],
        &accounts[6],
        &accounts[10],
        &rent,
        contract::EPOCH_BUDGET_ACCOUNT_BYTES,
        budget_rent,
        budget_extra,
        &budget_seeds,
    )?;
    encode_account(&accounts[3], |out| epoch.encode(out))?;
    encode_account(&accounts[4], |out| domain.encode(out))?;
    encode_account(&accounts[5], |out| window.encode(out))?;
    encode_account(&accounts[6], |out| budget.encode(out))?;
    encode_account(&accounts[2], |out| runtime_after.encode(out))
}

#[inline(never)]
fn freeze_epoch(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    request: contract::FreezeEpochPayloadV1,
) -> Outcome<()> {
    require_count(accounts, 7)?;
    require_role(
        program_id,
        &accounts[0],
        true,
        contract::GENERAL_EPOCH_ACCOUNT_BYTES,
    )?;
    require_role(
        program_id,
        &accounts[1],
        false,
        contract::ECONOMIC_DOMAIN_ACCOUNT_BYTES,
    )?;
    require_role(
        program_id,
        &accounts[2],
        true,
        contract::WINDOW_ACCOUNT_BYTES,
    )?;
    require_role(
        program_id,
        &accounts[3],
        true,
        contract::EPOCH_BUDGET_ACCOUNT_BYTES,
    )?;
    require_role(
        program_id,
        &accounts[4],
        false,
        contract::MARKET_BINDING_ACCOUNT_BYTES,
    )?;
    let slot = read_clock_slot(&accounts[5])?;
    require_writable_destination(&accounts[6])?;
    require_distinct_pairs(
        accounts,
        &[(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3), (3, 6)],
    )?;
    let epoch = contract::GeneralEpochV6AccountV1::decode(&borrow_data(&accounts[0])?)?;
    let domain = contract::EconomicDomainV2AccountV1::decode(&borrow_data(&accounts[1])?)?;
    let window = contract::CandidateWindowV4AccountV1::decode(&borrow_data(&accounts[2])?)?;
    let budget = contract::EpochBudgetV2AccountV1::decode(&borrow_data(&accounts[3])?)?;
    let binding = contract::MarketBindingV1::decode(&borrow_data(&accounts[4])?)?;
    require(
        epoch.economic_domain == id(accounts[1].key)
            && epoch.window == id(accounts[2].key)
            && epoch.budget == id(accounts[3].key)
            && epoch.market_binding == id(accounts[4].key)
            && domain.epoch == id(accounts[0].key)
            && window.epoch == id(accounts[0].key)
            && budget.epoch == id(accounts[0].key)
            && window.market == epoch.market_runtime
            && budget.market == epoch.market_runtime
            && binding.market == epoch.market_runtime,
        ClutchError::MismatchedState,
    )?;
    let post = contract::freeze_epoch_poststate_v1(
        &RuntimeSha256,
        contract::FreezeEpochTransitionV1 {
            epoch_id: id(accounts[0].key),
            market_binding_id: id(accounts[4].key),
            market_runtime_id: epoch.market_runtime,
            current_slot: slot,
            payload: request,
            epoch: &epoch,
            economic_domain: &domain,
            window: &window,
            budget: &budget,
            binding: &binding,
        },
    )?;
    move_lamports(&accounts[3], &accounts[6], post.keeper_reward)?;
    encode_account(&accounts[0], |out| post.epoch.encode(out))?;
    encode_account(&accounts[2], |out| post.window.encode(out))?;
    encode_account(&accounts[3], |out| post.budget.encode(out))
}

#[inline(never)]
fn begin_candidate(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    request: contract::BeginCandidatePayloadV1,
) -> Outcome<()> {
    require_count(accounts, 11)?;
    require_signer(&accounts[0])?;
    require_writable_destination(&accounts[0])?;
    require_signer(&accounts[1])?;
    require_readonly_actor(&accounts[1])?;
    require_readonly_actor(&accounts[2])?;
    require_readonly_actor(&accounts[3])?;
    require_role(
        program_id,
        &accounts[4],
        true,
        contract::GENERAL_EPOCH_ACCOUNT_BYTES,
    )?;
    require_role(
        program_id,
        &accounts[5],
        true,
        contract::WINDOW_ACCOUNT_BYTES,
    )?;
    require_role(
        program_id,
        &accounts[6],
        false,
        contract::MARKET_BINDING_ACCOUNT_BYTES,
    )?;
    require_creatable(&accounts[7])?;
    require_system_program(&accounts[8])?;
    let rent = read_rent(&accounts[9])?;
    let slot = read_clock_slot(&accounts[10])?;
    require_distinct_pairs(
        accounts,
        &[
            (4, 5),
            (4, 6),
            (4, 7),
            (5, 6),
            (5, 7),
            (6, 7),
            (0, 7),
            (1, 7),
            (2, 7),
            (3, 7),
        ],
    )?;
    let epoch = contract::GeneralEpochV6AccountV1::decode(&borrow_data(&accounts[4])?)?;
    let window = contract::CandidateWindowV4AccountV1::decode(&borrow_data(&accounts[5])?)?;
    let binding = contract::MarketBindingV1::decode(&borrow_data(&accounts[6])?)?;
    require(
        request.epoch == id(accounts[4].key)
            && epoch.phase == GeneralEpochPhaseV1::Frozen
            && epoch.window == id(accounts[5].key)
            && epoch.market_binding == id(accounts[6].key)
            && window.epoch == request.epoch
            && window.market == epoch.market_runtime
            && binding.market == epoch.market_runtime
            && slot >= window.frozen_slot
            && slot < window.reveal_opens_slot
            && u64::from(epoch.candidate_bundle_count) == window.live_node_count,
        ClutchError::MismatchedState,
    )?;
    let ordinal = window
        .admitted_count
        .checked_add(1)
        .ok_or(ClutchError::Arithmetic)?;
    let node_pda = seeds::general_v2_node_pda(program_id, &accounts[4].key.to_bytes(), ordinal);
    require(*accounts[7].key == node_pda.0, ClutchError::WrongPda)?;
    let node_rent = rent_owner(
        &accounts[0],
        &accounts[7],
        &rent,
        contract::ADMISSION_NODE_ACCOUNT_BYTES,
    )?;
    let post = contract::begin_candidate_poststate_v1(contract::BeginCandidateTransitionV1 {
        epoch_id: id(accounts[4].key),
        market_runtime_id: epoch.market_runtime,
        node_id: id(accounts[7].key),
        payer: id(accounts[0].key),
        submitter: id(accounts[1].key),
        refund_destination: id(accounts[2].key),
        solver_destination: id(accounts[3].key),
        current_slot: slot,
        payload: request,
        node_rent,
        node_bump: node_pda.1,
        epoch: &epoch,
        window: &window,
        binding: &binding,
    })?;
    let ordinal_le = ordinal.to_le_bytes();
    let node_seeds: [&[u8]; 4] = [
        seeds::SEED_GENERAL_V2_NODE,
        &accounts[4].key.to_bytes(),
        &ordinal_le,
        &[node_pda.1],
    ];
    let extra = binding
        .bond_lamports
        .checked_add(binding.node_cleanup_reward)
        .ok_or(ClutchError::Arithmetic)?;
    create_from_payer(
        program_id,
        &accounts[0],
        &accounts[7],
        &accounts[8],
        &rent,
        contract::ADMISSION_NODE_ACCOUNT_BYTES,
        node_rent,
        extra,
        &node_seeds,
    )?;
    require(
        post.commit_payer_funding
            == node_rent
                .refundable_principal
                .checked_add(extra)
                .ok_or(ClutchError::Arithmetic)?,
        ClutchError::MismatchedState,
    )?;
    encode_account(&accounts[7], |out| post.node.encode(out))?;
    encode_account(&accounts[4], |out| post.epoch.encode(out))?;
    encode_account(&accounts[5], |out| post.window.encode(out))
}

#[inline(never)]
fn open_candidate_feed(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    request: contract::OpenCandidateFeedPayloadV1,
) -> Outcome<()> {
    require_count(accounts, 11)?;
    require_signer(&accounts[0])?;
    require_writable_destination(&accounts[0])?;
    require_signer(&accounts[1])?;
    require_readonly_actor(&accounts[1])?;
    require_role(
        program_id,
        &accounts[2],
        false,
        contract::GENERAL_EPOCH_ACCOUNT_BYTES,
    )?;
    require_role(
        program_id,
        &accounts[3],
        true,
        contract::WINDOW_ACCOUNT_BYTES,
    )?;
    require_role(
        program_id,
        &accounts[4],
        false,
        contract::MARKET_BINDING_ACCOUNT_BYTES,
    )?;
    require_role(
        program_id,
        &accounts[5],
        false,
        contract::ECONOMIC_DOMAIN_ACCOUNT_BYTES,
    )?;
    require_role(
        program_id,
        &accounts[6],
        true,
        contract::ADMISSION_NODE_ACCOUNT_BYTES,
    )?;
    require_creatable(&accounts[7])?;
    require_system_program(&accounts[8])?;
    let rent = read_rent(&accounts[9])?;
    let slot = read_clock_slot(&accounts[10])?;
    require_distinct_pairs(
        accounts,
        &[
            (2, 3),
            (2, 4),
            (2, 5),
            (2, 6),
            (2, 7),
            (3, 4),
            (3, 5),
            (3, 6),
            (3, 7),
            (4, 5),
            (4, 6),
            (4, 7),
            (5, 6),
            (5, 7),
            (6, 7),
            (0, 7),
            (1, 7),
        ],
    )?;
    let epoch = contract::GeneralEpochV6AccountV1::decode(&borrow_data(&accounts[2])?)?;
    let window = contract::CandidateWindowV4AccountV1::decode(&borrow_data(&accounts[3])?)?;
    let binding = contract::MarketBindingV1::decode(&borrow_data(&accounts[4])?)?;
    let domain = contract::EconomicDomainV2AccountV1::decode(&borrow_data(&accounts[5])?)?;
    let node = contract::AdmissionNodeV3AccountV1::decode(&borrow_data(&accounts[6])?)?;
    require(
        request.epoch == id(accounts[2].key)
            && request.node == id(accounts[6].key)
            && id(accounts[0].key) == node.payer
            && id(accounts[1].key) == node.submitter_authority
            && epoch.phase == GeneralEpochPhaseV1::Frozen
            && epoch.window == id(accounts[3].key)
            && epoch.market_binding == id(accounts[4].key)
            && epoch.economic_domain == id(accounts[5].key)
            && window.epoch == request.epoch
            && node.epoch == request.epoch
            && node.status == contract::AdmissionNodeStatusV1::Committed
            && node.market == epoch.market_runtime
            && binding.market == epoch.market_runtime
            && domain.epoch == request.epoch
            && domain.transcript.outcome_count == request.outcome_count
            && domain.transcript.price_scale == request.price_scale
            && request.outcome_count == binding.outcome_count
            && request.basis_degree == binding.basis_degree
            && request.price_scale == binding.price_scale
            && request.order_count == 0
            && request.slice_count == 0
            && request.candidate_kind == contract::SettlementCandidateKindV1::Direct
            && binding.candidate_kind_mask & 1 == 1
            && slot >= window.reveal_opens_slot
            && slot < window.submission_closes_slot,
        ClutchError::MismatchedState,
    )?;
    let expected_node =
        seeds::general_v2_node_pda(program_id, &accounts[2].key.to_bytes(), node.ordinal);
    require(
        *accounts[6].key == expected_node.0 && node.stored_bump == expected_node.1,
        ClutchError::WrongPda,
    )?;
    require(
        domain.transcript.relation_policy_id == binding.relation_policy_id
            && domain.transcript.price_measure_policy_v1_id == binding.price_measure_policy_v1_id
            && domain.transcript.native_claim_basis_id == binding.native_claim_basis_id
            && epoch.order_set
                == contract::empty_order_set_digest_v1(
                    &RuntimeSha256,
                    contract::economic_domain_digest_v2(&RuntimeSha256, domain.transcript)?,
                )?,
        ClutchError::MismatchedState,
    )?;
    let feed_pda = seeds::general_v2_feed_pda(program_id, &accounts[6].key.to_bytes());
    require(*accounts[7].key == feed_pda.0, ClutchError::WrongPda)?;
    let feed_len = contract::candidate_feed_account_len(
        request.outcome_count,
        request.order_count,
        request.atom_count,
        request.slice_count,
    )?;
    let work_len = contract::clear_work_account_len(request.outcome_count, request.order_count)?;
    let feed_rent = rent_owner(&accounts[0], &accounts[7], &rent, feed_len)?;
    let work_rent = DeletableRentOwnerV1 {
        payer: id(accounts[0].key),
        refundable_principal: rent.minimum_balance(work_len)?,
        donation_floor: 0,
    };
    let post = contract::open_candidate_feed_poststate_v1(
        &RuntimeSha256,
        contract::OpenCandidateFeedTransitionV1 {
            epoch_id: id(accounts[2].key),
            feed_id: id(accounts[7].key),
            current_slot: slot,
            payload: request,
            feed_rent,
            work_rent,
            feed_bump: feed_pda.1,
            epoch: &epoch,
            window: &window,
            node: &node,
            binding: &binding,
            economic_domain: &domain,
        },
    )?;
    let committed_extra = binding
        .bond_lamports
        .checked_add(binding.node_cleanup_reward)
        .ok_or(ClutchError::Arithmetic)?;
    require(
        post.funding.commit_payer_funding
            == node
                .rent
                .refundable_principal
                .checked_add(committed_extra)
                .ok_or(ClutchError::Arithmetic)?
            && node.work_escrow_lamports == 0
            && node.work_funding_initial == 0,
        ClutchError::MismatchedState,
    )?;
    let feed_seeds: [&[u8]; 3] = [
        seeds::SEED_GENERAL_V2_FEED,
        &accounts[6].key.to_bytes(),
        &[feed_pda.1],
    ];
    create_from_payer(
        program_id,
        &accounts[0],
        &accounts[7],
        &accounts[8],
        &rent,
        feed_len,
        feed_rent,
        binding.feed_close_reward,
        &feed_seeds,
    )?;
    transfer_from_signer(
        &accounts[0],
        &accounts[6],
        &accounts[8],
        post.funding.work_allocation,
    )?;
    encode_account(&accounts[7], |out| {
        post.feed_stage
            .encode(&mut out[..contract::CANDIDATE_FEED_HEADER_BYTES], false)
    })?;
    encode_account(&accounts[6], |out| post.node.encode(out))?;
    encode_account(&accounts[3], |out| post.window.encode(out))
}

#[inline(never)]
fn write_candidate_feed_segment(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    request: contract::CandidateFeedSegmentPayloadV1<'_>,
) -> Outcome<()> {
    require_count(accounts, 6)?;
    require_signer(&accounts[0])?;
    require_readonly_actor(&accounts[0])?;
    require_role(
        program_id,
        &accounts[1],
        false,
        contract::GENERAL_EPOCH_ACCOUNT_BYTES,
    )?;
    require_role(
        program_id,
        &accounts[2],
        false,
        contract::WINDOW_ACCOUNT_BYTES,
    )?;
    require_role(
        program_id,
        &accounts[3],
        false,
        contract::ADMISSION_NODE_ACCOUNT_BYTES,
    )?;
    require(
        accounts[4].owner == program_id,
        ClutchError::WrongProgramOwner,
    )?;
    require_writable_destination(&accounts[4])?;
    let slot = read_clock_slot(&accounts[5])?;
    let epoch = contract::GeneralEpochV6AccountV1::decode(&borrow_data(&accounts[1])?)?;
    let window = contract::CandidateWindowV4AccountV1::decode(&borrow_data(&accounts[2])?)?;
    let node = contract::AdmissionNodeV3AccountV1::decode(&borrow_data(&accounts[3])?)?;
    let header =
        contract::CandidateFeedHeaderV2::decode_account(&borrow_data(&accounts[4])?, false)?;
    require(
        request.epoch == id(accounts[1].key)
            && request.node == id(accounts[3].key)
            && id(accounts[0].key) == node.submitter_authority
            && epoch.window == id(accounts[2].key)
            && node.status == contract::AdmissionNodeStatusV1::Revealed
            && header.epoch == request.epoch
            && header.node == request.node
            && header.market == epoch.market_runtime
            && slot >= window.reveal_opens_slot
            && slot < window.submission_closes_slot,
        ClutchError::MismatchedState,
    )?;
    let feed_pda = seeds::general_v2_feed_pda(program_id, &accounts[3].key.to_bytes());
    require(
        *accounts[4].key == feed_pda.0 && header.stored_bump == feed_pda.1,
        ClutchError::WrongPda,
    )?;
    let write_range = contract::candidate_feed_segment_byte_range_v1(request, header)?;
    require(
        write_range.end <= accounts[4].data_len(),
        ClutchError::WrongDataLength,
    )?;
    let post = contract::candidate_feed_segment_poststate_v1(request, header, node, window, slot)?;
    {
        let mut data = accounts[4]
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        data[write_range].copy_from_slice(request.records);
        post.encode(&mut data[..contract::CANDIDATE_FEED_HEADER_BYTES], false)?;
    }
    contract::CandidateFeedHeaderV2::decode_account(&borrow_data(&accounts[4])?, false)?;
    Ok(())
}

#[inline(never)]
fn seal_candidate(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    request: contract::EpochNodePayloadV1,
) -> Outcome<()> {
    require_count(accounts, 8)?;
    require_signer(&accounts[0])?;
    require_readonly_actor(&accounts[0])?;
    require_role(
        program_id,
        &accounts[1],
        false,
        contract::GENERAL_EPOCH_ACCOUNT_BYTES,
    )?;
    require_role(
        program_id,
        &accounts[2],
        false,
        contract::WINDOW_ACCOUNT_BYTES,
    )?;
    require_role(
        program_id,
        &accounts[3],
        false,
        contract::ADMISSION_NODE_ACCOUNT_BYTES,
    )?;
    require(
        accounts[4].owner == program_id,
        ClutchError::WrongProgramOwner,
    )?;
    require_writable_destination(&accounts[4])?;
    require_role(
        program_id,
        &accounts[5],
        false,
        contract::MARKET_BINDING_ACCOUNT_BYTES,
    )?;
    require_role(
        program_id,
        &accounts[6],
        false,
        contract::ECONOMIC_DOMAIN_ACCOUNT_BYTES,
    )?;
    let slot = read_clock_slot(&accounts[7])?;
    let epoch = contract::GeneralEpochV6AccountV1::decode(&borrow_data(&accounts[1])?)?;
    let window = contract::CandidateWindowV4AccountV1::decode(&borrow_data(&accounts[2])?)?;
    let node = contract::AdmissionNodeV3AccountV1::decode(&borrow_data(&accounts[3])?)?;
    let binding = contract::MarketBindingV1::decode(&borrow_data(&accounts[5])?)?;
    let domain = contract::EconomicDomainV2AccountV1::decode(&borrow_data(&accounts[6])?)?;
    let feed_data = borrow_data(&accounts[4])?;
    let header = contract::CandidateFeedHeaderV2::decode_account(&feed_data, false)?;
    require(
        request.epoch == id(accounts[1].key)
            && request.node == id(accounts[3].key)
            && id(accounts[0].key) == node.submitter_authority
            && epoch.window == id(accounts[2].key)
            && epoch.market_binding == id(accounts[5].key)
            && epoch.economic_domain == id(accounts[6].key)
            && node.status == contract::AdmissionNodeStatusV1::Revealed
            && header.epoch == request.epoch
            && header.node == request.node
            && header.market == epoch.market_runtime
            && header.order_set == epoch.order_set
            && header.candidate_kind == contract::SettlementCandidateKindV1::Direct
            && header.order_count == 0
            && header.slice_count == 0
            && node.candidate_bundle_digest != Id32::ZERO
            && slot >= window.reveal_opens_slot
            && slot < window.submission_closes_slot,
        ClutchError::MismatchedState,
    )?;
    let feed_pda = seeds::general_v2_feed_pda(program_id, &accounts[3].key.to_bytes());
    require(
        *accounts[4].key == feed_pda.0 && header.stored_bump == feed_pda.1,
        ClutchError::WrongPda,
    )?;
    let sealed = contract::seal_empty_book_candidate_v1(
        &RuntimeSha256,
        id(accounts[4].key),
        &feed_data,
        node,
        binding,
        domain,
    )?;
    drop(feed_data);
    encode_account(&accounts[4], |out| {
        sealed.encode(&mut out[..contract::CANDIDATE_FEED_HEADER_BYTES], true)
    })
}

#[inline(never)]
fn init_clear_work(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    request: contract::EpochNodePayloadV1,
) -> Outcome<()> {
    require_count(accounts, 12)?;
    require_writable_destination(&accounts[0])?;
    require_role(
        program_id,
        &accounts[1],
        true,
        contract::GENERAL_EPOCH_ACCOUNT_BYTES,
    )?;
    require_role(
        program_id,
        &accounts[2],
        false,
        contract::WINDOW_ACCOUNT_BYTES,
    )?;
    require_role(
        program_id,
        &accounts[3],
        false,
        contract::MARKET_BINDING_ACCOUNT_BYTES,
    )?;
    require_role(
        program_id,
        &accounts[4],
        false,
        contract::ECONOMIC_DOMAIN_ACCOUNT_BYTES,
    )?;
    require_role(
        program_id,
        &accounts[5],
        true,
        contract::ADMISSION_NODE_ACCOUNT_BYTES,
    )?;
    require(
        accounts[6].owner == program_id,
        ClutchError::WrongProgramOwner,
    )?;
    require(!accounts[6].is_writable, ClutchError::UnexpectedWritable)?;
    require_readonly_artifact(program_id, &accounts[7], BASIS_BYTES)?;
    require_creatable(&accounts[8])?;
    require_system_program(&accounts[9])?;
    let rent = read_rent(&accounts[10])?;
    let slot = read_clock_slot(&accounts[11])?;
    require_distinct_pairs(
        accounts,
        &[
            (1, 2),
            (1, 3),
            (1, 4),
            (1, 5),
            (1, 6),
            (1, 8),
            (2, 3),
            (2, 4),
            (2, 5),
            (2, 6),
            (2, 8),
            (3, 4),
            (3, 5),
            (3, 6),
            (3, 8),
            (4, 5),
            (4, 6),
            (4, 8),
            (5, 6),
            (5, 8),
            (6, 8),
            (0, 8),
        ],
    )?;
    let epoch = contract::GeneralEpochV6AccountV1::decode(&borrow_data(&accounts[1])?)?;
    let window = contract::CandidateWindowV4AccountV1::decode(&borrow_data(&accounts[2])?)?;
    let binding = contract::MarketBindingV1::decode(&borrow_data(&accounts[3])?)?;
    let domain = contract::EconomicDomainV2AccountV1::decode(&borrow_data(&accounts[4])?)?;
    let node = contract::AdmissionNodeV3AccountV1::decode(&borrow_data(&accounts[5])?)?;
    let feed = contract::CandidateFeedHeaderV2::decode_account(&borrow_data(&accounts[6])?, true)?;
    let basis = NativeClaimBasisV1::decode(&borrow_data(&accounts[7])?)
        .map_err(|_| ClutchError::NonCanonical)?;
    require(
        request.epoch == id(accounts[1].key)
            && request.node == id(accounts[5].key)
            && epoch.window == id(accounts[2].key)
            && epoch.market_binding == id(accounts[3].key)
            && epoch.economic_domain == id(accounts[4].key)
            && window.epoch == request.epoch
            && binding.market == epoch.market_runtime
            && domain.epoch == request.epoch
            && node.epoch == request.epoch
            && node.status == contract::AdmissionNodeStatusV1::Revealed
            && node.work_escrow_lamports != 0
            && feed.epoch == request.epoch
            && feed.node == request.node
            && feed.market == epoch.market_runtime
            && feed.order_count == 0
            && feed.slice_count == 0
            && basis.id().map_err(|_| ClutchError::NonCanonical)?.bytes()
                == binding.native_claim_basis_id.bytes()
            && slot >= window.submission_closes_slot
            && slot < window.verification_closes_slot,
        ClutchError::MismatchedState,
    )?;
    let feed_pda = seeds::general_v2_feed_pda(program_id, &accounts[5].key.to_bytes());
    require(
        *accounts[6].key == feed_pda.0 && feed.stored_bump == feed_pda.1,
        ClutchError::WrongPda,
    )?;
    let work_pda = seeds::general_v2_work_pda(program_id, &accounts[5].key.to_bytes());
    require(*accounts[8].key == work_pda.0, ClutchError::WrongPda)?;
    let work_len = contract::clear_work_account_len(feed.outcome_count, feed.order_count)?;
    let work_rent = DeletableRentOwnerV1 {
        payer: node.payer,
        refundable_principal: rent.minimum_balance(work_len)?,
        donation_floor: accounts[8].lamports(),
    };
    let post = contract::init_clear_work_poststate_v1(contract::InitClearWorkTransitionV1 {
        epoch_id: id(accounts[1].key),
        feed_id: id(accounts[6].key),
        work_id: id(accounts[8].key),
        work_rent,
        work_bump: work_pda.1,
        epoch: &epoch,
        node: &node,
        feed: &feed,
        binding: &binding,
    })?;
    require(
        post.work_account_bytes == work_len,
        ClutchError::MismatchedState,
    )?;
    let work_seeds: [&[u8]; 3] = [
        seeds::SEED_GENERAL_V2_WORK,
        &accounts[5].key.to_bytes(),
        &[work_pda.1],
    ];
    create_from_program_escrow(
        program_id,
        &accounts[5],
        &accounts[8],
        &accounts[9],
        &rent,
        work_len,
        work_rent,
        node.work_funding_initial,
        &work_seeds,
    )?;
    encode_account(&accounts[8], |out| {
        post.work
            .encode(&mut out[..contract::CLEAR_WORK_HEADER_BYTES])
    })?;
    encode_account(&accounts[5], |out| post.node.encode(out))?;
    encode_account(&accounts[1], |out| post.epoch.encode(out))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CheckedVerdict {
    Valid(contract::ScoreV2QComponentsV1),
    Refused,
}

/// Authenticate Product bodies and run both independent arithmetic checkers.
#[inline(never)]
fn checked_empty_book_verdict(
    program_id: &Pubkey,
    binding: contract::MarketBindingV1,
    domain: contract::EconomicDomainV2AccountV1,
    node: contract::AdmissionNodeV3AccountV1,
    feed_key: &Pubkey,
    feed_data: &[u8],
    basis_account: &AccountInfo,
    genesis_account: &AccountInfo,
    policy_account: &AccountInfo,
) -> Outcome<CheckedVerdict> {
    require_readonly_artifact(program_id, basis_account, BASIS_BYTES)?;
    require_readonly_artifact(program_id, genesis_account, MARKET_GENESIS_PROFILE_V2_BYTES)?;
    require_readonly_artifact(program_id, policy_account, PRICE_MEASURE_POLICY_BYTES)?;
    let basis = NativeClaimBasisV1::decode(&borrow_data(basis_account)?)
        .map_err(|_| ClutchError::NonCanonical)?;
    let genesis = MarketGenesisProfileV2::decode(&borrow_data(genesis_account)?)
        .map_err(|_| ClutchError::NonCanonical)?;
    let policy = PriceMeasurePolicyV1::decode(&borrow_data(policy_account)?)
        .map_err(|_| ClutchError::NonCanonical)?;
    require(
        basis.id().map_err(|_| ClutchError::NonCanonical)?.bytes()
            == binding.native_claim_basis_id.bytes()
            && genesis.id().map_err(|_| ClutchError::NonCanonical)?.bytes()
                == binding.market_genesis_profile_v2_id.bytes()
            && policy.id().map_err(|_| ClutchError::NonCanonical)?.bytes()
                == binding.price_measure_policy_v1_id.bytes()
            && genesis.capability_profile_id.bytes() == capabilities::PROFILE_ID
            && genesis.relation_policy_id.bytes() == binding.relation_policy_id.bytes()
            && genesis.score_policy_id.bytes() == binding.score_policy_id.bytes()
            && basis.edge_policy_registry_value == 1
            && basis.basis_degree != 0,
        ClutchError::MismatchedState,
    )?;
    genesis
        .validate_bindings(&basis, &policy)
        .map_err(|_| ClutchError::MismatchedState)?;
    let smooth_basis = policy
        .project_smooth_basis(&basis, &genesis, QuantizedEdgePolicyV1::Clamp)
        .map_err(|_| ClutchError::MismatchedState)?;
    let (header, tail) = contract::complete_candidate_feed_v2(feed_data, true)?;
    let mut prices = [0u64; contract::MAX_OUTCOMES];
    let mut index = 0usize;
    while index < usize::from(header.outcome_count) {
        let at = index * 8;
        prices[index] = u64::from_le_bytes(
            tail.prices_le()[at..at + 8]
                .try_into()
                .map_err(|_| ClutchError::WrongDataLength)?,
        );
        index += 1;
    }
    let mut coordinates = [0u128; contract::MAX_QUANTIZED_ATOMS];
    let mut masses = [0u64; contract::MAX_QUANTIZED_ATOMS];
    index = 0;
    while index < usize::from(header.atom_count) {
        let at = index * contract::QUANTIZED_ATOM_BYTES;
        coordinates[index] = u128::from_le_bytes(
            tail.atoms_le()[at..at + 16]
                .try_into()
                .map_err(|_| ClutchError::WrongDataLength)?,
        );
        masses[index] = u64::from_le_bytes(
            tail.atoms_le()[at + 16..at + 24]
                .try_into()
                .map_err(|_| ClutchError::WrongDataLength)?,
        );
        index += 1;
    }
    let price_vector = PriceVectorV3 {
        basis_degree: header.basis_degree,
        native_outcome_count: header.outcome_count,
        price_scale: header.price_scale,
        prices,
    };
    let observed_body =
        contract::quantized_witness_body_digest_v3(&RuntimeSha256, id(feed_key), feed_data, true)?;
    require(
        observed_body == header.price_body_digest,
        ClutchError::MismatchedState,
    )?;
    let bindings = AdapterBindingsV3 {
        candidate_feed: feed_key.to_bytes(),
        relation_domain_digest: header.economic_domain_digest.bytes(),
        basis_digest: header.native_claim_basis_id.bytes(),
        candidate_price_digest: header.candidate_price_digest.bytes(),
        observed_body_digest: observed_body.bytes(),
    };
    let witness = QuantizedAtomWitnessV3 {
        schema_version: header.price_witness_schema,
        quantized_semantics_version: header.quantized_semantics_version,
        candidate_feed: feed_key.to_bytes(),
        relation_domain_digest: header.economic_domain_digest.bytes(),
        basis_digest: header.native_claim_basis_id.bytes(),
        candidate_price_digest: header.candidate_price_digest.bytes(),
        body_digest: observed_body.bytes(),
        basis_degree: header.basis_degree,
        native_outcome_count: header.outcome_count,
        atom_count: header.atom_count,
        common_denominator: header.common_denominator,
        atom_coordinates: coordinates,
        atom_masses: masses,
    };
    if policy
        .validate_witness_contract(&basis, &price_vector, &witness, binding.price_scale)
        .is_err()
        || verify_quantized_price_measure_v3_smooth(
            &bindings,
            &smooth_basis,
            &price_vector,
            &witness,
        )
        .is_err()
    {
        return Ok(CheckedVerdict::Refused);
    }
    let economic_domain = EconomicDomainV2 {
        relation_version: domain.transcript.relation_version,
        market_semantics_digest: domain.transcript.market_instance_v2_id.bytes(),
        epoch_semantics_digest: domain.transcript.epoch_semantics_digest.bytes(),
        relation_policy_digest: domain.transcript.relation_policy_id.bytes(),
        price_policy_digest: domain.transcript.price_measure_policy_v1_id.bytes(),
        epoch_index: domain.transcript.epoch_index,
        outcome_count: domain.transcript.outcome_count,
        price_scale: domain.transcript.price_scale,
    };
    let price = PricePreconditionV2 {
        policy_digest: binding.price_measure_policy_v1_id.bytes(),
        semantic_price_digest: header.candidate_price_digest.bytes(),
        prices,
    };
    let economic = match verify_economic_candidate_v2(
        &economic_domain,
        &EconomicBookV2::empty(),
        &price,
        &EconomicCandidateV2 {
            fills: [0; clutch_batch::MAX_ORDERS],
            honored_aon_mask: header.honored_aon_mask,
            virtual_split: header.virtual_split,
            virtual_merge: header.virtual_merge,
        },
    ) {
        Ok(value) => value,
        Err(_) => return Ok(CheckedVerdict::Refused),
    };
    let base = Id32::from_bytes(economic.economic_candidate_digest);
    require(
        base == header.base_relation_candidate_id
            && base == header.settlement_candidate_id
            && base == node.base_relation_candidate_id
            && base == node.settlement_candidate_id
            && header.settlement_witness_digest
                == contract::empty_settlement_witness_digest_v1(&RuntimeSha256, base)?
            && node.settlement_witness_digest == header.settlement_witness_digest
            && node.candidate_bundle_digest
                == contract::candidate_bundle_digest_v1(&RuntimeSha256, feed_data, true)?,
        ClutchError::MismatchedState,
    )?;
    Ok(CheckedVerdict::Valid(contract::ScoreV2QComponentsV1 {
        certified_risk_flow_atoms: economic.score.risk.certified_risk_flow_atoms,
        cash_equivalent_direct_flow_atoms: economic.score.cash_equivalent_direct_flow_atoms,
        virtual_churn_atoms: economic.score.virtual_churn_atoms,
        settlement_candidate_id: base,
    }))
}

#[inline(never)]
fn complete_candidate_verification(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    request: contract::EpochNodePayloadV1,
) -> Outcome<()> {
    // The authenticated Genesis and PriceMeasure policy bodies are explicit
    // metas. Their earlier omission would have made bare IDs behavioral truth.
    require_count(accounts, 12)?;
    require_writable_destination(&accounts[0])?;
    require_role(
        program_id,
        &accounts[1],
        false,
        contract::GENERAL_EPOCH_ACCOUNT_BYTES,
    )?;
    require_role(
        program_id,
        &accounts[2],
        true,
        contract::WINDOW_ACCOUNT_BYTES,
    )?;
    require_role(
        program_id,
        &accounts[3],
        false,
        contract::MARKET_BINDING_ACCOUNT_BYTES,
    )?;
    require_role(
        program_id,
        &accounts[4],
        false,
        contract::ECONOMIC_DOMAIN_ACCOUNT_BYTES,
    )?;
    require_role(
        program_id,
        &accounts[5],
        true,
        contract::ADMISSION_NODE_ACCOUNT_BYTES,
    )?;
    require(
        accounts[6].owner == program_id,
        ClutchError::WrongProgramOwner,
    )?;
    require(!accounts[6].is_writable, ClutchError::UnexpectedWritable)?;
    require_readonly_artifact(program_id, &accounts[7], BASIS_BYTES)?;
    require_readonly_artifact(program_id, &accounts[8], MARKET_GENESIS_PROFILE_V2_BYTES)?;
    require_readonly_artifact(program_id, &accounts[9], PRICE_MEASURE_POLICY_BYTES)?;
    require(
        accounts[10].owner == program_id,
        ClutchError::WrongProgramOwner,
    )?;
    require_writable_destination(&accounts[10])?;
    let slot = read_clock_slot(&accounts[11])?;
    require_distinct_pairs(
        accounts,
        &[
            (1, 2),
            (1, 3),
            (1, 4),
            (1, 5),
            (1, 6),
            (1, 10),
            (2, 3),
            (2, 4),
            (2, 5),
            (2, 6),
            (2, 10),
            (3, 4),
            (3, 5),
            (3, 6),
            (3, 10),
            (4, 5),
            (4, 6),
            (4, 10),
            (5, 6),
            (5, 10),
            (6, 10),
            (0, 10),
        ],
    )?;
    let epoch = contract::GeneralEpochV6AccountV1::decode(&borrow_data(&accounts[1])?)?;
    let window = contract::CandidateWindowV4AccountV1::decode(&borrow_data(&accounts[2])?)?;
    let binding = contract::MarketBindingV1::decode(&borrow_data(&accounts[3])?)?;
    let domain = contract::EconomicDomainV2AccountV1::decode(&borrow_data(&accounts[4])?)?;
    let node = contract::AdmissionNodeV3AccountV1::decode(&borrow_data(&accounts[5])?)?;
    let feed_data = borrow_data(&accounts[6])?;
    let feed = contract::CandidateFeedHeaderV2::decode_account(&feed_data, true)?;
    let work = contract::ClearWorkHeaderV2::decode_account(&borrow_data(&accounts[10])?)?;
    require(
        request.epoch == id(accounts[1].key)
            && request.node == id(accounts[5].key)
            && epoch.window == id(accounts[2].key)
            && epoch.market_binding == id(accounts[3].key)
            && epoch.economic_domain == id(accounts[4].key)
            && node.epoch == request.epoch
            && node.status == contract::AdmissionNodeStatusV1::Revealed
            && node.work_escrow_lamports == 0
            && feed.epoch == request.epoch
            && feed.node == request.node
            && work.epoch == request.epoch
            && work.node == request.node
            && work.feed == id(accounts[6].key)
            && work.phase == 0
            && work.order_count == 0
            && work.slice_count == 0
            && slot >= window.submission_closes_slot
            && slot < window.verification_closes_slot,
        ClutchError::MismatchedState,
    )?;
    let work_pda = seeds::general_v2_work_pda(program_id, &accounts[5].key.to_bytes());
    require(
        *accounts[10].key == work_pda.0 && work.stored_bump == work_pda.1,
        ClutchError::WrongPda,
    )?;
    let verdict = checked_empty_book_verdict(
        program_id,
        binding,
        domain,
        node,
        accounts[6].key,
        &feed_data,
        &accounts[7],
        &accounts[8],
        &accounts[9],
    )?;
    drop(feed_data);
    let post = contract::complete_candidate_verification_poststate_v1(
        contract::CompleteCandidateVerificationTransitionV1 {
            current_slot: slot,
            verdict: match verdict {
                CheckedVerdict::Valid(score) => {
                    contract::EmptyBookVerificationVerdictV1::Valid(score)
                }
                CheckedVerdict::Refused => contract::EmptyBookVerificationVerdictV1::Refused,
            },
            epoch: &epoch,
            window: &window,
            node: &node,
            work: &work,
            binding: &binding,
        },
    )?;
    move_lamports(&accounts[10], &accounts[0], post.keeper_reward)?;
    encode_account(&accounts[10], |out| {
        post.work
            .encode(&mut out[..contract::CLEAR_WORK_HEADER_BYTES])
    })?;
    encode_account(&accounts[5], |out| post.node.encode(out))?;
    encode_account(&accounts[2], |out| post.window.encode(out))
}

#[inline(never)]
fn finalize_selection(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    request: contract::FinalizeSelectionPayloadV1,
) -> Outcome<()> {
    require_count(accounts, 12)?;
    require_role(
        program_id,
        &accounts[0],
        true,
        contract::GENERAL_EPOCH_ACCOUNT_BYTES,
    )?;
    require_role(
        program_id,
        &accounts[1],
        true,
        contract::WINDOW_ACCOUNT_BYTES,
    )?;
    require_role(
        program_id,
        &accounts[2],
        true,
        contract::EPOCH_BUDGET_ACCOUNT_BYTES,
    )?;
    require_role(
        program_id,
        &accounts[3],
        false,
        contract::ADMISSION_NODE_ACCOUNT_BYTES,
    )?;
    require(
        accounts[4].owner == program_id,
        ClutchError::WrongProgramOwner,
    )?;
    require(!accounts[4].is_writable, ClutchError::UnexpectedWritable)?;
    require_role(
        program_id,
        &accounts[5],
        false,
        contract::MARKET_BINDING_ACCOUNT_BYTES,
    )?;
    require_role(
        program_id,
        &accounts[6],
        false,
        contract::ECONOMIC_DOMAIN_ACCOUNT_BYTES,
    )?;
    require_creatable(&accounts[7])?;
    require_writable_destination(&accounts[8])?;
    require_system_program(&accounts[9])?;
    let rent = read_rent(&accounts[10])?;
    let slot = read_clock_slot(&accounts[11])?;
    require_distinct_pairs(
        accounts,
        &[
            (0, 1),
            (0, 2),
            (0, 3),
            (0, 4),
            (0, 5),
            (0, 6),
            (0, 7),
            (1, 2),
            (1, 3),
            (1, 4),
            (1, 5),
            (1, 6),
            (1, 7),
            (2, 3),
            (2, 4),
            (2, 5),
            (2, 6),
            (2, 7),
            (2, 8),
            (3, 4),
            (3, 5),
            (3, 6),
            (3, 7),
            (4, 5),
            (4, 6),
            (4, 7),
            (5, 6),
            (5, 7),
            (6, 7),
            (7, 8),
        ],
    )?;
    let epoch = contract::GeneralEpochV6AccountV1::decode(&borrow_data(&accounts[0])?)?;
    let window = contract::CandidateWindowV4AccountV1::decode(&borrow_data(&accounts[1])?)?;
    let budget = contract::EpochBudgetV2AccountV1::decode(&borrow_data(&accounts[2])?)?;
    let node = contract::AdmissionNodeV3AccountV1::decode(&borrow_data(&accounts[3])?)?;
    let feed = contract::CandidateFeedHeaderV2::decode_account(&borrow_data(&accounts[4])?, true)?;
    let binding = contract::MarketBindingV1::decode(&borrow_data(&accounts[5])?)?;
    let domain = contract::EconomicDomainV2AccountV1::decode(&borrow_data(&accounts[6])?)?;
    require(
        request.epoch == id(accounts[0].key)
            && epoch.window == id(accounts[1].key)
            && epoch.budget == id(accounts[2].key)
            && epoch.market_binding == id(accounts[5].key)
            && epoch.economic_domain == id(accounts[6].key)
            && window.epoch == request.epoch
            && budget.epoch == request.epoch
            && node.epoch == request.epoch
            && feed.epoch == request.epoch
            && feed.node == id(accounts[3].key)
            && binding.market == epoch.market_runtime
            && domain.epoch == request.epoch
            && feed.economic_domain_digest
                == contract::economic_domain_digest_v2(&RuntimeSha256, domain.transcript)?,
        ClutchError::MismatchedState,
    )?;
    let selected_pda = seeds::general_v2_selected_pda(
        program_id,
        &accounts[0].key.to_bytes(),
        &node.settlement_candidate_id.bytes(),
    );
    require(*accounts[7].key == selected_pda.0, ClutchError::WrongPda)?;
    let selected_principal = rent.minimum_balance(contract::SELECTED_CANDIDATE_ACCOUNT_BYTES)?;
    require(
        budget.selected_rent_initial == selected_principal
            && budget.selected_rent_remaining == selected_principal
            && budget.finalize_remaining == binding.finalize_reward,
        ClutchError::MismatchedState,
    )?;
    let selected_rent = DeletableRentOwnerV1 {
        payer: budget.funding_payer,
        refundable_principal: selected_principal,
        donation_floor: accounts[7].lamports(),
    };
    let post =
        contract::finalize_selection_poststate_v1(contract::FinalizeSelectionTransitionV1 {
            epoch_id: id(accounts[0].key),
            window_id: id(accounts[1].key),
            market_binding_id: id(accounts[5].key),
            feed_id: id(accounts[4].key),
            selected_candidate_id: id(accounts[7].key),
            current_slot: slot,
            selected_rent,
            selected_bump: selected_pda.1,
            epoch: &epoch,
            window: &window,
            budget: &budget,
            node: &node,
            feed: &feed,
        })?;
    let selected_seeds: [&[u8]; 4] = [
        seeds::SEED_GENERAL_V2_SELECTED,
        &accounts[0].key.to_bytes(),
        &node.settlement_candidate_id.bytes(),
        &[selected_pda.1],
    ];
    create_from_program_escrow(
        program_id,
        &accounts[2],
        &accounts[7],
        &accounts[9],
        &rent,
        contract::SELECTED_CANDIDATE_ACCOUNT_BYTES,
        selected_rent,
        selected_principal,
        &selected_seeds,
    )?;
    move_lamports(&accounts[2], &accounts[8], post.finalizer_reward)?;
    encode_account(&accounts[7], |out| post.selected_candidate.encode(out))?;
    encode_account(&accounts[0], |out| post.epoch.encode(out))?;
    encode_account(&accounts[1], |out| post.window.encode(out))?;
    encode_account(&accounts[2], |out| post.budget.encode(out))
}
