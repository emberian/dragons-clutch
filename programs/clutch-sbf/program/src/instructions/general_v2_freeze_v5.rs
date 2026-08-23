//! Nonempty General V5 FreezeEpoch successor (fresh local action 43).
//!
//! Historical action 6 freezes only the bounded empty-book laboratory and is
//! never reinterpreted here. Action 43 carries the same 32-byte Epoch-semantics
//! payload but presents exact MarketInstance/Genesis/Product/PriceGrid bodies
//! and one through four exact 4,140-byte OrderPage V5 accounts after the fixed
//! authority frame. One specialized layout traversal authenticates
//! every page body and Position-generation tail, derives the V5 set commitment,
//! live/owner/cardinality and width/expiry facts, and retains the headers used
//! for exact PDA and post-seal checks. No transaction-supplied summary exists.
//!
//! All page and root prestates are authenticated before mutation. The pages are
//! then sealed, their returned headers are compared to the traversal receipt,
//! the present-funded keeper reward moves, and Epoch/Window/Budget poststates
//! are encoded. Any refusal rolls the complete Solana instruction back.

use crate::accounts::{require, require_distinct, Outcome};
use crate::capabilities;
use crate::error::{ClutchError, Refusal};
use crate::instructions::artifact::read_clock_slot;
use crate::seeds;
use clutch_general_v2_contract as contract;
use clutch_general_v2_contract::{DeletableRentOwnerV1, Id32, Sha256BackendV1};
use clutch_product_series::{
    ContentId, MarketGenesisProfileV2, MarketInstancePreimageV2, NativeClaimBasisV1,
    PriceMeasurePolicyV1, ProductTemplateV4, QuantizedEdgePolicyV1,
};
use clutch_solana_layout::order_page_v5::{
    freeze_page_set_prestate_v5, seal_page_v5, FreezePageSetContextV5, ORDER_PAGE_V5_BYTES,
};
use clutch_solana_layout::projection::OwnerInterner;
use clutch_solana_layout::{account_len, Hash32, PriceGridAccount};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use super::product_artifact::authenticate_product_artifact_v1;

const SBF_FRAME_LIMIT_BYTES: usize = 4_096;
const _: () = assert!(core::mem::size_of::<OwnerInterner>() < SBF_FRAME_LIMIT_BYTES);
const _: () = assert!(
    core::mem::size_of::<clutch_solana_layout::order_page_v5::FreezePageSetPrestateV5>()
        < SBF_FRAME_LIMIT_BYTES
);
const _: () = assert!(
    core::mem::size_of::<contract::FreezeEpochPoststateV1>() < SBF_FRAME_LIMIT_BYTES
);
const _: () = assert!(core::mem::size_of::<NativeClaimBasisV1>() < SBF_FRAME_LIMIT_BYTES);
const _: () = assert!(ORDER_PAGE_V5_BYTES > SBF_FRAME_LIMIT_BYTES);

/// Exact action-43 payload width: one authenticated Epoch-semantics identity.
pub const FREEZE_EPOCH_V5_PAYLOAD_BYTES: usize = contract::FREEZE_EPOCH_PAYLOAD_BYTES;
/// Fixed root and immutable authority accounts before the canonical V5 page set.
pub const FREEZE_EPOCH_V5_FIXED_ACCOUNT_COUNT: usize = 13;
/// Minimum action-43 account count: thirteen fixed roles and one nonempty V5 page.
pub const FREEZE_EPOCH_V5_MIN_ACCOUNT_COUNT: usize = FREEZE_EPOCH_V5_FIXED_ACCOUNT_COUNT + 1;
/// Maximum action-43 account count: thirteen fixed roles and four V5 pages.
pub const FREEZE_EPOCH_V5_MAX_ACCOUNT_COUNT: usize = FREEZE_EPOCH_V5_FIXED_ACCOUNT_COUNT + 4;

/// Writable counted General Epoch root.
pub const IX_FREEZE_V5_EPOCH: usize = 0;
/// Read-only immutable EconomicDomainV2 artifact.
pub const IX_FREEZE_V5_DOMAIN: usize = 1;
/// Writable candidate Window.
pub const IX_FREEZE_V5_WINDOW: usize = 2;
/// Writable present-funded Epoch Budget.
pub const IX_FREEZE_V5_BUDGET: usize = 3;
/// Read-only immutable MarketBinding.
pub const IX_FREEZE_V5_BINDING: usize = 4;
/// Read-only content-addressed MarketInstancePreimageV2 authority.
pub const IX_FREEZE_V5_MARKET_INSTANCE: usize = 5;
/// Read-only content-addressed MarketGenesisProfileV2 authority.
pub const IX_FREEZE_V5_MARKET_GENESIS: usize = 6;
/// Read-only content-addressed ProductTemplateV4 authority.
pub const IX_FREEZE_V5_PRODUCT_TEMPLATE: usize = 7;
/// Read-only content-addressed NativeClaimBasisV1 authority.
pub const IX_FREEZE_V5_NATIVE_BASIS: usize = 8;
/// Read-only content-addressed PriceMeasurePolicyV1 authority.
pub const IX_FREEZE_V5_PRICE_POLICY: usize = 9;
/// Read-only canonical PriceGrid account selected by Genesis V2.
pub const IX_FREEZE_V5_PRICE_GRID: usize = 10;
/// Read-only Clock sysvar.
pub const IX_FREEZE_V5_CLOCK: usize = 11;
/// Writable keeper reward destination.
pub const IX_FREEZE_V5_KEEPER: usize = 12;
/// First writable canonical OrderPage V5; the complete set follows in order.
pub const IX_FREEZE_V5_PAGES: usize = FREEZE_EPOCH_V5_FIXED_ACCOUNT_COUNT;

/// Native Solana SHA-256 adapter for the pure root transition.
#[derive(Clone, Copy, Debug)]
struct RuntimeSha256;

impl Sha256BackendV1 for RuntimeSha256 {
    fn sha256(&self, parts: &[&[u8]]) -> [u8; contract::ID_BYTES] {
        solana_sha256_hasher::hashv(parts).to_bytes()
    }
}

/// Execute fresh General local action 43 over one exact nonempty V5 book.
#[inline(never)]
pub fn freeze_epoch_v5(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    request: contract::FreezeEpochPayloadV1,
) -> Outcome<()> {
    require(
        (FREEZE_EPOCH_V5_MIN_ACCOUNT_COUNT..=FREEZE_EPOCH_V5_MAX_ACCOUNT_COUNT)
            .contains(&accounts.len()),
        ClutchError::AccountCount,
    )?;
    require_distinct(accounts)?;
    require_role(
        program_id,
        &accounts[IX_FREEZE_V5_EPOCH],
        true,
        contract::GENERAL_EPOCH_ACCOUNT_BYTES,
    )?;
    require_role(
        program_id,
        &accounts[IX_FREEZE_V5_DOMAIN],
        false,
        contract::ECONOMIC_DOMAIN_ACCOUNT_BYTES,
    )?;
    require_role(
        program_id,
        &accounts[IX_FREEZE_V5_WINDOW],
        true,
        contract::WINDOW_ACCOUNT_BYTES,
    )?;
    require_role(
        program_id,
        &accounts[IX_FREEZE_V5_BUDGET],
        true,
        contract::EPOCH_BUDGET_ACCOUNT_BYTES,
    )?;
    require_role(
        program_id,
        &accounts[IX_FREEZE_V5_BINDING],
        false,
        contract::MARKET_BINDING_ACCOUNT_BYTES,
    )?;
    require_writable_destination(&accounts[IX_FREEZE_V5_KEEPER])?;
    for page in &accounts[IX_FREEZE_V5_PAGES..] {
        require_role(program_id, page, true, ORDER_PAGE_V5_BYTES)?;
    }
    let slot = read_clock_slot(&accounts[IX_FREEZE_V5_CLOCK])?;

    let epoch = decode_epoch(&accounts[IX_FREEZE_V5_EPOCH])?;
    let domain = decode_domain(&accounts[IX_FREEZE_V5_DOMAIN])?;
    let window = decode_window(&accounts[IX_FREEZE_V5_WINDOW])?;
    let budget = decode_budget(&accounts[IX_FREEZE_V5_BUDGET])?;
    let binding = decode_binding(&accounts[IX_FREEZE_V5_BINDING])?;

    authenticate_root_pdas(
        program_id,
        accounts,
        &epoch,
        &domain,
        &window,
        &budget,
        &binding,
    )?;
    require_compartment_balance(
        &accounts[IX_FREEZE_V5_BUDGET],
        budget.rent,
        &[
            budget.freeze_remaining,
            budget.finalize_remaining,
            budget.solver_remaining,
            budget.root_close_remaining,
            budget.selected_rent_remaining,
        ],
    )?;
    authenticate_product_authority(program_id, accounts, &binding, &domain)?;

    let context = FreezePageSetContextV5::new(
        Hash32::from_bytes(epoch.market_runtime.bytes()),
        Hash32::from_bytes(accounts[IX_FREEZE_V5_EPOCH].key.to_bytes()),
        binding.outcome_count,
        epoch.epoch_index,
    )?;
    let prestate = authenticate_page_set(accounts, context)?;
    authenticate_page_pdas(program_id, accounts, &prestate)?;

    let book = contract::FreezeOrderSetFactsV5 {
        order_set: Id32::new(prestate.order_set().bytes())?,
        page_count: prestate.page_count(),
        populated_order_count: prestate.populated_order_count(),
        live_order_count: prestate.live_order_count(),
        owner_count: prestate.owner_count(),
        position_generation_count: prestate.position_generation_count(),
    };
    let post = derive_freeze_poststate(
        accounts,
        slot,
        request,
        &epoch,
        &domain,
        &window,
        &budget,
        &binding,
        book,
    )?;
    require(
        post.epoch.order_set == book.order_set,
        ClutchError::MismatchedState,
    )?;

    let mut page_index = 0u16;
    while page_index < prestate.page_count() {
        let account_index = IX_FREEZE_V5_PAGES
            .checked_add(usize::from(page_index))
            .ok_or(ClutchError::Arithmetic)?;
        let sealed = seal_page_v5(
            &mut borrow_data_mut(&accounts[account_index])?,
            prestate.order_set(),
            prestate.populated_order_count(),
        )?;
        prestate.binds_sealed_header(page_index, &sealed)?;
        page_index = page_index
            .checked_add(1)
            .ok_or(ClutchError::Arithmetic)?;
    }

    move_lamports(
        &accounts[IX_FREEZE_V5_BUDGET],
        &accounts[IX_FREEZE_V5_KEEPER],
        post.keeper_reward,
    )?;
    encode_account(&accounts[IX_FREEZE_V5_EPOCH], |out| {
        post.epoch.encode(out)
    })?;
    encode_account(&accounts[IX_FREEZE_V5_WINDOW], |out| {
        post.window.encode(out)
    })?;
    encode_account(&accounts[IX_FREEZE_V5_BUDGET], |out| {
        post.budget.encode(out)
    })
}

// Each hostile root decode owns a separate, non-inlined sub-4-KiB frame and
// returns heap storage. The top-level action therefore never retains five
// account-sized decode destinations in one SBF frame.
#[inline(never)]
fn decode_epoch(account: &AccountInfo) -> Outcome<Box<contract::GeneralEpochV6AccountV1>> {
    Ok(Box::new(contract::GeneralEpochV6AccountV1::decode(
        &borrow_data(account)?,
    )?))
}

#[inline(never)]
fn decode_domain(account: &AccountInfo) -> Outcome<Box<contract::EconomicDomainV2AccountV1>> {
    Ok(Box::new(contract::EconomicDomainV2AccountV1::decode(
        &borrow_data(account)?,
    )?))
}

#[inline(never)]
fn decode_window(account: &AccountInfo) -> Outcome<Box<contract::CandidateWindowV4AccountV1>> {
    Ok(Box::new(contract::CandidateWindowV4AccountV1::decode(
        &borrow_data(account)?,
    )?))
}

#[inline(never)]
fn decode_budget(account: &AccountInfo) -> Outcome<Box<contract::EpochBudgetV2AccountV1>> {
    Ok(Box::new(contract::EpochBudgetV2AccountV1::decode(
        &borrow_data(account)?,
    )?))
}

#[inline(never)]
fn decode_binding(account: &AccountInfo) -> Outcome<Box<contract::MarketBindingV1>> {
    Ok(Box::new(contract::MarketBindingV1::decode(
        &borrow_data(account)?,
    )?))
}

/// Retain the bounded V5 traversal receipt on the heap after the account-body
/// borrows and owner scratch have been released.
#[inline(never)]
fn authenticate_page_set(
    accounts: &[AccountInfo],
    context: FreezePageSetContextV5,
) -> Outcome<Box<clutch_solana_layout::order_page_v5::FreezePageSetPrestateV5>> {
    let mut owners = boxed_empty_interner();
    let borrows: Vec<core::cell::Ref<'_, [u8]>> = accounts[IX_FREEZE_V5_PAGES..]
        .iter()
        .map(borrow_data)
        .collect::<Outcome<Vec<_>>>()?;
    let bodies: Vec<&[u8]> = borrows.iter().map(|data| &data[..]).collect();
    let prestate = freeze_page_set_prestate_v5(context, &bodies, &mut owners)?;
    prestate.binds_context(context)?;
    Ok(Box::new(prestate))
}

/// Derive the pure atomic root poststate in its own bounded frame and retain
/// only its heap pointer in the top-level SBF handler.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn derive_freeze_poststate(
    accounts: &[AccountInfo],
    slot: u64,
    request: contract::FreezeEpochPayloadV1,
    epoch: &contract::GeneralEpochV6AccountV1,
    domain: &contract::EconomicDomainV2AccountV1,
    window: &contract::CandidateWindowV4AccountV1,
    budget: &contract::EpochBudgetV2AccountV1,
    binding: &contract::MarketBindingV1,
    book: contract::FreezeOrderSetFactsV5,
) -> Outcome<Box<contract::FreezeEpochPoststateV1>> {
    Ok(Box::new(contract::freeze_epoch_v5_poststate_v1(
        &RuntimeSha256,
        contract::FreezeEpochTransitionV1 {
            epoch_id: id(accounts[IX_FREEZE_V5_EPOCH].key),
            market_binding_id: id(accounts[IX_FREEZE_V5_BINDING].key),
            market_runtime_id: epoch.market_runtime,
            current_slot: slot,
            payload: request,
            epoch,
            economic_domain: domain,
            window,
            budget,
            binding,
        },
        book,
    )?))
}

/// Authenticate the complete immutable Product authority before sealing its
/// first nonempty book.
///
/// The binding's IDs are not treated as body proofs. Every content-addressed
/// body is hostile-decoded through the shared boxed artifact authenticator,
/// the MarketInstance joins Template/Basis/Policy/Genesis, and the canonical
/// PriceGrid PDA joins Genesis and the exact integer simplex scale. This keeps
/// the 2,352-byte basis off the SBF stack and prevents a withdrawn RelationV2
/// policy from acquiring a fresh frozen book.
#[inline(never)]
fn authenticate_product_authority(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    binding: &contract::MarketBindingV1,
    domain: &contract::EconomicDomainV2AccountV1,
) -> Outcome<()> {
    let expected_relation = clutch_general_v2_runtime::quantized_relation_v2_policy_id_v2()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let expected_score = clutch_general_v2_runtime::score_v2_q_policy_id_v1()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let market = authenticate_product_artifact_v1::<MarketInstancePreimageV2>(
        program_id,
        &accounts[IX_FREEZE_V5_MARKET_INSTANCE],
        ContentId::from_bytes(binding.market_instance_v2_id.bytes()),
    )?;
    let genesis = authenticate_product_artifact_v1::<MarketGenesisProfileV2>(
        program_id,
        &accounts[IX_FREEZE_V5_MARKET_GENESIS],
        ContentId::from_bytes(binding.market_genesis_profile_v2_id.bytes()),
    )?;
    let template = authenticate_product_artifact_v1::<ProductTemplateV4>(
        program_id,
        &accounts[IX_FREEZE_V5_PRODUCT_TEMPLATE],
        market.value().product_template_id.content_id(),
    )?;
    let basis = authenticate_product_artifact_v1::<NativeClaimBasisV1>(
        program_id,
        &accounts[IX_FREEZE_V5_NATIVE_BASIS],
        ContentId::from_bytes(binding.native_claim_basis_id.bytes()),
    )?;
    let price_policy = authenticate_product_artifact_v1::<PriceMeasurePolicyV1>(
        program_id,
        &accounts[IX_FREEZE_V5_PRICE_POLICY],
        ContentId::from_bytes(binding.price_measure_policy_v1_id.bytes()),
    )?;
    market
        .value()
        .validate_bindings(
            template.value(),
            basis.value(),
            price_policy.value(),
            genesis.value(),
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    genesis
        .value()
        .validate_partition_bindings(
            basis.value(),
            price_policy.value(),
            QuantizedEdgePolicyV1::Clamp,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    require_role(
        program_id,
        &accounts[IX_FREEZE_V5_PRICE_GRID],
        false,
        account_len::PRICE_GRID,
    )?;
    let grid = PriceGridAccount::decode(&borrow_data(&accounts[IX_FREEZE_V5_PRICE_GRID])?)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let grid_pda = seeds::grid_pda(program_id, &grid.realm.bytes(), &grid.grid.bytes());
    require(
        *accounts[IX_FREEZE_V5_PRICE_GRID].key == grid_pda.0
            && grid.stored_bump == grid_pda.1,
        ClutchError::WrongPda,
    )?;

    let transcript = domain.transcript;
    require(
        binding.relation_policy_id == expected_relation
            && binding.score_policy_id == expected_score
            && market.value().market_genesis_profile_id.content_id().bytes()
                == binding.market_genesis_profile_v2_id.bytes()
            && genesis.value().relation_policy_id.bytes() == binding.relation_policy_id.bytes()
            && genesis.value().score_policy_id.bytes() == binding.score_policy_id.bytes()
            && genesis.value().capability_profile_id.bytes() == capabilities::PROFILE_ID
            && basis.value().basis_degree == binding.basis_degree
            && (2..=3).contains(&basis.value().basis_degree)
            && basis.value().outcome_count == binding.outcome_count
            && basis.value().denominator == binding.price_scale
            && basis.value().edge_policy_registry_value == 1
            && binding.price_scale <= price_policy.value().maximum_price_scale
            && transcript.market_instance_v2_id == binding.market_instance_v2_id
            && transcript.relation_policy_id == binding.relation_policy_id
            && transcript.price_measure_policy_v1_id == binding.price_measure_policy_v1_id
            && transcript.native_claim_basis_id == binding.native_claim_basis_id
            && transcript.outcome_count == binding.outcome_count
            && transcript.price_scale == binding.price_scale
            && transcript.coordinate_domain_min == genesis.value().coordinate_domain_min
            && transcript.coordinate_domain_max == genesis.value().coordinate_domain_max
            && grid.grid.bytes() == genesis.value().price_grid_id.bytes()
            && grid.realm.bytes() == genesis.value().realm_id.bytes()
            && grid.price_scale == binding.price_scale,
        ClutchError::MismatchedState,
    )
}

#[allow(clippy::too_many_arguments)]
fn authenticate_root_pdas(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    epoch: &contract::GeneralEpochV6AccountV1,
    domain: &contract::EconomicDomainV2AccountV1,
    window: &contract::CandidateWindowV4AccountV1,
    budget: &contract::EpochBudgetV2AccountV1,
    binding: &contract::MarketBindingV1,
) -> Outcome<()> {
    let binding_pda =
        seeds::general_v2_market_binding_pda(program_id, &binding.market_instance_v2_id.bytes());
    let epoch_pda = seeds::general_v2_epoch_pda(
        program_id,
        &accounts[IX_FREEZE_V5_BINDING].key.to_bytes(),
        epoch.epoch_index,
    );
    let domain_pda =
        seeds::general_v2_economic_domain_pda(program_id, &epoch_pda.0.to_bytes());
    let window_pda = seeds::general_v2_window_pda(program_id, &epoch_pda.0.to_bytes());
    let budget_pda = seeds::general_v2_budget_pda(program_id, &epoch_pda.0.to_bytes());
    require(
        *accounts[IX_FREEZE_V5_BINDING].key == binding_pda.0
            && binding.stored_bump == binding_pda.1
            && *accounts[IX_FREEZE_V5_EPOCH].key == epoch_pda.0
            && epoch.stored_bump == epoch_pda.1
            && *accounts[IX_FREEZE_V5_DOMAIN].key == domain_pda.0
            && domain.stored_bump == domain_pda.1
            && *accounts[IX_FREEZE_V5_WINDOW].key == window_pda.0
            && window.stored_bump == window_pda.1
            && *accounts[IX_FREEZE_V5_BUDGET].key == budget_pda.0
            && budget.stored_bump == budget_pda.1,
        ClutchError::WrongPda,
    )
}

fn authenticate_page_pdas(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    prestate: &clutch_solana_layout::order_page_v5::FreezePageSetPrestateV5,
) -> Outcome<()> {
    let mut page_index = 0u16;
    while page_index < prestate.page_count() {
        let account_index = IX_FREEZE_V5_PAGES
            .checked_add(usize::from(page_index))
            .ok_or(ClutchError::Arithmetic)?;
        let header = prestate.header(page_index)?;
        let pda = seeds::general_v2_order_page_v5_pda(
            program_id,
            &accounts[IX_FREEZE_V5_EPOCH].key.to_bytes(),
            page_index,
        );
        require(
            *accounts[account_index].key == pda.0 && header.stored_bump == pda.1,
            ClutchError::WrongPda,
        )?;
        page_index = page_index
            .checked_add(1)
            .ok_or(ClutchError::Arithmetic)?;
    }
    Ok(())
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

fn require_writable_destination(account: &AccountInfo) -> Outcome<()> {
    require(account.is_writable, ClutchError::NotWritable)?;
    require(!account.executable, ClutchError::ExecutableAccount)
}

fn borrow_data<'a, 'b>(account: &'a AccountInfo<'b>) -> Outcome<core::cell::Ref<'a, [u8]>> {
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    Ok(core::cell::Ref::map(data, |bytes| &**bytes))
}

fn borrow_data_mut<'a, 'b>(
    account: &'a AccountInfo<'b>,
) -> Outcome<core::cell::RefMut<'a, &'b mut [u8]>> {
    account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))
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

fn require_compartment_balance(
    account: &AccountInfo,
    rent: DeletableRentOwnerV1,
    live_compartments: &[u64],
) -> Outcome<()> {
    let mut expected = rent
        .refundable_principal
        .checked_add(rent.donation_floor)
        .ok_or(ClutchError::Arithmetic)?;
    for amount in live_compartments {
        expected = expected
            .checked_add(*amount)
            .ok_or(ClutchError::Arithmetic)?;
    }
    require(account.lamports() == expected, ClutchError::MismatchedState)
}

/// Allocate bounded owner scratch without entering an unsafe adapter seam.
/// The 2,050-byte initializer remains below the 4-KiB SBF frame bound and the
/// retained value itself lives on the heap for the full traversal.
fn boxed_empty_interner() -> Box<OwnerInterner> {
    Box::new(OwnerInterner::NEW)
}
