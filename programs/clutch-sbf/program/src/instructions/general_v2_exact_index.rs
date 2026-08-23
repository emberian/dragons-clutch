//! Capability-disabled SBF boundary for compact counted settlement indexes.

use core::cell::Ref;

use clutch_general_v2_contract as contract;
use clutch_general_v2_contract::{
    DeletableRentOwnerV1, Id32, IndexedSettlementRootV1AccountV1, MarketBindingV2,
    SettlementRootV1AccountV1, Sha256BackendV1, INDEXED_SETTLEMENT_ROOT_BYTES_V1,
    MARKET_BINDING_ACCOUNT_BYTES_V2,
};
use clutch_general_v2_runtime::exact_index_plane::{
    adjacency_data_len_v1, authenticate_counted_exact_index_read_v1,
    authenticate_counted_exact_index_retirement_v1, exact_index_slice_reference_count_v1,
    authenticate_feed_full_data_id_v1,
    indexed_pair_coverage_from_sealed_accounts_v1, locator_data_len_v1,
    retire_counted_exact_index_root_v1, stream_counted_exact_index_root_v1,
    AuthenticateCountedExactIndexReadInputV1, CloseExactIndexPlaneInputV1,
    ConstructExactIndexStreamingInputV1, ExactIndexCloseAccountInputV1,
    ExactIndexCreateAccountInputV1, ExactIndexPlaneErrorV1, ExactIndexReadAccountInputV1,
    IndexedPairCoverageV1,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use crate::accounts::{expect_pda, require, require_signer, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::{
    require_creatable, require_system_program, RentParameters, SYSTEM_PROGRAM_ID,
};
use crate::seeds;

use super::general_v2_settlement_producer_v5::{create_from_payer, encode_account};
use super::general_v2_settlement_traversal_v5::AuthenticatedSettlementTraversalV5;

#[derive(Clone, Copy, Debug)]
struct RuntimeSha256;
impl Sha256BackendV1 for RuntimeSha256 {
    fn sha256(&self, parts: &[&[u8]]) -> [u8; contract::ID_BYTES] {
        solana_sha256_hasher::hashv(parts).to_bytes()
    }
}

fn id(key: &Pubkey) -> Id32 { Id32::from_bytes(key.to_bytes()) }
fn exact<T>(value: Result<T, ExactIndexPlaneErrorV1>) -> Outcome<T> {
    value.map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))
}
fn borrow_data<'a, 'info>(account: &'a AccountInfo<'info>) -> Outcome<Ref<'a, [u8]>> {
    let data = account.try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    Ok(Ref::map(data, |bytes| &**bytes))
}
fn require_program_account(program_id: &Pubkey, account: &AccountInfo<'_>, writable: bool,
    exact_len: Option<usize>) -> Outcome<()>
{
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    require(!account.is_signer, ClutchError::MismatchedState)?;
    require(account.is_writable == writable, if writable { ClutchError::NotWritable }
        else { ClutchError::UnexpectedWritable })?;
    if let Some(len) = exact_len { require(account.data_len() == len, ClutchError::WrongDataLength)?; }
    Ok(())
}

fn canonical_index_accounts(program_id: &Pubkey, root_account: &AccountInfo<'_>,
    root: &IndexedSettlementRootV1AccountV1, locator: &AccountInfo<'_>,
    adjacency: &AccountInfo<'_>) -> Outcome<((Pubkey, u8), (Pubkey, u8), (Pubkey, u8))>
{
    let base = root.base();
    let root_pda = seeds::general_v2_settlement_root_pda(program_id, &base.epoch().bytes(),
        &base.settlement_candidate_id().bytes());
    let root_bytes = root_account.key.to_bytes();
    let locator_pda = seeds::general_v2_frozen_order_locator_pda(program_id, &root_bytes);
    let adjacency_pda = seeds::general_v2_candidate_adjacency_pda(program_id, &root_bytes);
    expect_pda(root_account.key, root_pda, Some(base.stored_bump()))?;
    require(*locator.key == locator_pda.0 && *adjacency.key == adjacency_pda.0
        && root.locator_account() == id(locator.key) && root.adjacency_account() == id(adjacency.key),
        ClutchError::WrongPda)?;
    Ok((root_pda, locator_pda, adjacency_pda))
}

#[allow(clippy::too_many_arguments)]
fn authenticated_join<'a>(program_id: &Pubkey, root_account: &AccountInfo<'_>, root_body: &'a [u8],
    locator: &AccountInfo<'_>, locator_body: &'a [u8], adjacency: &AccountInfo<'_>,
    adjacency_body: &'a [u8], feed: &AccountInfo<'_>, feed_body: &'a [u8],
    root_writable: bool, children_writable: bool)
    -> Outcome<AuthenticateCountedExactIndexReadInputV1<'a>>
{
    require_program_account(program_id, root_account, root_writable,
        Some(INDEXED_SETTLEMENT_ROOT_BYTES_V1))?;
    require_program_account(program_id, locator, children_writable, None)?;
    require_program_account(program_id, adjacency, children_writable, None)?;
    require_program_account(program_id, feed, false, None)?;
    let root = IndexedSettlementRootV1AccountV1::decode(root_body)?;
    let (root_pda, locator_pda, adjacency_pda) =
        canonical_index_accounts(program_id, root_account, &root, locator, adjacency)?;
    let base = root.base();
    let feed_pda = seeds::general_v2_feed_pda(
        program_id,
        &base.source_admission_node().bytes(),
    );
    let feed_header = contract::CandidateFeedHeaderV2::decode_account(feed_body, true)?;
    require(*feed.key == feed_pda.0 && base.retained_feed() == id(feed.key)
        && feed_header.stored_bump == feed_pda.1
        && feed_header.node == base.source_admission_node()
        && feed_header.epoch == base.epoch()
        && feed_header.market == base.market()
        && feed_header.settlement_candidate_id == base.settlement_candidate_id(),
        ClutchError::WrongPda)?;
    Ok(AuthenticateCountedExactIndexReadInputV1 {
        program_id: id(program_id),
        root: read_input(root_account, root_body, root_pda),
        locator: read_input(locator, locator_body, locator_pda),
        adjacency: read_input(adjacency, adjacency_body, adjacency_pda),
        feed: read_input(feed, feed_body, feed_pda),
    })
}
fn read_input<'a>(account: &AccountInfo<'_>, body: &'a [u8], canonical: (Pubkey, u8))
    -> ExactIndexReadAccountInputV1<'a>
{
    ExactIndexReadAccountInputV1 { account: id(account.key), body, owner: id(account.owner),
        canonical_account: id(&canonical.0), canonical_bump: canonical.1,
        writable: account.is_writable, executable: account.executable }
}

/// Full child and Feed body-ID authentication followed by bounded local reads.
pub fn read_pair_coverage_v1(program_id: &Pubkey, root: &AccountInfo<'_>,
    locator: &AccountInfo<'_>, adjacency: &AccountInfo<'_>, feed: &AccountInfo<'_>,
    buy_order: u8, sell_order: u8) -> Outcome<IndexedPairCoverageV1>
{
    let root_body = borrow_data(root)?; let locator_body = borrow_data(locator)?;
    let adjacency_body = borrow_data(adjacency)?; let feed_body = borrow_data(feed)?;
    let joined = authenticated_join(program_id, root, &root_body, locator, &locator_body,
        adjacency, &adjacency_body, feed, &feed_body, false, false)?;
    let sealed = exact(authenticate_counted_exact_index_read_v1(joined))?;
    exact(indexed_pair_coverage_from_sealed_accounts_v1(sealed, buy_order, sell_order))
}

fn require_fresh_child(program_id: &Pubkey, root: &AccountInfo<'_>, child: &AccountInfo<'_>,
    adjacency: bool) -> Outcome<(Pubkey, u8)>
{
    require_creatable(child)?;
    require(child.is_writable && !child.is_signer && !child.executable,
        ClutchError::MismatchedState)?;
    let root_bytes = root.key.to_bytes();
    let canonical = if adjacency { seeds::general_v2_candidate_adjacency_pda(program_id, &root_bytes) }
        else { seeds::general_v2_frozen_order_locator_pda(program_id, &root_bytes) };
    require(*child.key == canonical.0, ClutchError::WrongPda)?;
    Ok(canonical)
}
fn create_input(program_id: &Pubkey, payer: &AccountInfo<'_>,
    target: &AccountInfo<'_>, rent_minimum: u64, bump: u8) -> ExactIndexCreateAccountInputV1
{
    ExactIndexCreateAccountInputV1 { account: id(target.key), program_id: id(program_id),
        payer: id(payer.key), payer_lamports: payer.lamports(),
        target_lamports: target.lamports(), target_owner: id(target.owner),
        target_data_len: target.data_len(), target_writable: target.is_writable,
        target_executable: target.executable, rent_exempt_minimum: rent_minimum, stored_bump: bump }
}

/// Stream both compact children and their counted root from the sole private
/// action-39 traversal authority. Every allocation is below 4 KiB.
#[allow(clippy::too_many_arguments)]
pub(crate) fn create_fresh_counted_root_v1<'info>(program_id: &Pubkey,
    payer: &AccountInfo<'info>, root: &AccountInfo<'info>, locator: &AccountInfo<'info>,
    adjacency: &AccountInfo<'info>, feed_account: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>, rent: &RentParameters,
    base: &SettlementRootV1AccountV1, neutral_sink: Id32,
    authority: &AuthenticatedSettlementTraversalV5) -> Outcome<()>
{
    require_signer(payer)?; require(payer.is_writable, ClutchError::NotWritable)?;
    require_system_program(system_program)?; require_creatable(root)?;
    require(root.is_writable && !root.is_signer, ClutchError::MismatchedState)?;
    require(authority.feed_account() == base.retained_feed()
        && authority.feed_account() == id(feed_account.key)
        && authority.traversal().candidate_bundle_digest() == base.candidate_bundle_digest()
        && authority.market().base().neutral_sink == neutral_sink, ClutchError::MismatchedState)?;
    require_program_account(program_id, feed_account, false, None)?;
    let feed_body = borrow_data(feed_account)?;
    let feed_full_data = exact(authenticate_feed_full_data_id_v1(
        authority.traversal(), id(feed_account.key), &feed_body,
    ))?;
    drop(feed_body);
    let root_pda = seeds::general_v2_settlement_root_pda(program_id, &base.epoch().bytes(),
        &base.settlement_candidate_id().bytes());
    expect_pda(root.key, root_pda, Some(base.stored_bump()))?;
    let locator_pda = require_fresh_child(program_id, root, locator, false)?;
    let adjacency_pda = require_fresh_child(program_id, root, adjacency, true)?;
    let feed = authority.feed();
    let references = exact(exact_index_slice_reference_count_v1(authority.traversal()))?;
    let locator_len = exact(locator_data_len_v1(feed.order_count))?;
    let adjacency_len = exact(adjacency_data_len_v1(feed.order_count, references))?;
    require(locator_len <= 4_096 && adjacency_len <= 4_096, ClutchError::WrongDataLength)?;
    let locator_minimum = rent.minimum_balance(locator_len)?;
    let adjacency_minimum = rent.minimum_balance(adjacency_len)?;
    let locator_create = create_input(program_id, payer, locator,
        locator_minimum, locator_pda.1);
    let adjacency_create = create_input(program_id, payer, adjacency,
        adjacency_minimum, adjacency_pda.1);
    let root_minimum = rent.minimum_balance(INDEXED_SETTLEMENT_ROOT_BYTES_V1)?;
    let root_rent = contract::prepare_fresh_indexed_settlement_root_rent_v1(base, id(root.key),
        root.lamports(), root_minimum, payer.lamports(), neutral_sink, &RuntimeSha256)?;
    let root_epoch = base.epoch().bytes(); let candidate = base.settlement_candidate_id().bytes();
    let root_bump = [root_pda.1];
    create_from_payer(program_id, payer, root, system_program, rent,
        INDEXED_SETTLEMENT_ROOT_BYTES_V1, root_rent.rent_after(),
        &[seeds::SEED_GENERAL_V2_SETTLEMENT_ROOT, &root_epoch, &candidate, &root_bump])?;
    let root_bytes = root.key.to_bytes(); let locator_bump = [locator_pda.1];
    create_from_payer(program_id, payer, locator, system_program, rent, locator_len,
        DeletableRentOwnerV1 { payer: id(payer.key), refundable_principal: locator_minimum,
            donation_floor: locator_create.target_lamports },
        &[seeds::SEED_GENERAL_V2_FROZEN_ORDER_LOCATOR, &root_bytes, &locator_bump])?;
    let adjacency_bump = [adjacency_pda.1];
    create_from_payer(program_id, payer, adjacency, system_program, rent, adjacency_len,
        DeletableRentOwnerV1 { payer: id(payer.key), refundable_principal: adjacency_minimum,
            donation_floor: adjacency_create.target_lamports },
        &[seeds::SEED_GENERAL_V2_CANDIDATE_ADJACENCY, &root_bytes, &adjacency_bump])?;
    let mut root_data = root.try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let mut locator_data = locator.try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let mut adjacency_data = adjacency.try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    exact(stream_counted_exact_index_root_v1(root_rent,
        ConstructExactIndexStreamingInputV1 { traversal: authority.traversal(),
            feed_full_data,
            settlement_root_account: id(root.key), settlement_root: base,
            capability_profile_id: Id32::new(authority.genesis().capability_profile_id.bytes())?,
            page_physical_slot_counts: authority.page_physical_slot_counts(),
            locator_create, adjacency_create },
        &mut root_data, &mut locator_data, &mut adjacency_data))?;
    Ok(())
}

fn authenticate_market_binding_v2(program_id: &Pubkey, account: &AccountInfo<'_>)
    -> Outcome<MarketBindingV2>
{
    require_program_account(program_id, account, false, Some(MARKET_BINDING_ACCOUNT_BYTES_V2))?;
    let binding = MarketBindingV2::decode(&borrow_data(account)?)?; let base = binding.base();
    expect_pda(account.key, seeds::general_v2_market_binding_pda(program_id,
        &base.market_instance_v2_id.bytes()), Some(base.stored_bump))?;
    Ok(binding)
}

/// Retire both compact projections only while the root-bound retained Feed is
/// still present and full-body authenticated.
#[allow(clippy::too_many_arguments)]
pub(crate) fn retire_exact_index_pair_v1(program_id: &Pubkey, root: &AccountInfo<'_>,
    locator: &AccountInfo<'_>, adjacency: &AccountInfo<'_>, feed: &AccountInfo<'_>,
    market_binding_account: &AccountInfo<'_>, locator_payer: &AccountInfo<'_>,
    adjacency_payer: &AccountInfo<'_>, neutral_sink: &AccountInfo<'_>) -> Outcome<()>
{
    let binding = authenticate_market_binding_v2(program_id, market_binding_account)?;
    let root_body = borrow_data(root)?; let locator_body = borrow_data(locator)?;
    let adjacency_body = borrow_data(adjacency)?; let feed_body = borrow_data(feed)?;
    let joined = authenticated_join(program_id, root, &root_body, locator, &locator_body,
        adjacency, &adjacency_body, feed, &feed_body, true, true)?;
    let mutation = exact(authenticate_counted_exact_index_retirement_v1(joined))?;
    let plan = exact(retire_counted_exact_index_root_v1(mutation, CloseExactIndexPlaneInputV1 {
        market_binding_account: id(market_binding_account.key), market_binding: &binding,
        locator: close_input(program_id, locator),
        adjacency: close_input(program_id, adjacency),
    }))?;
    let close = plan.close_postwrites();
    let credits = [close.locator_principal_credit(), close.locator_donation_credit(),
        close.adjacency_principal_credit(), close.adjacency_donation_credit()];
    require(id(locator_payer.key) == credits[0].recipient()
        && id(neutral_sink.key) == credits[1].recipient()
        && id(adjacency_payer.key) == credits[2].recipient()
        && id(neutral_sink.key) == credits[3].recipient()
        && locator_payer.is_writable && adjacency_payer.is_writable && neutral_sink.is_writable,
        ClutchError::MismatchedState)?;
    for recipient in [locator_payer, adjacency_payer, neutral_sink] {
        require(recipient.key != root.key && recipient.key != locator.key
            && recipient.key != adjacency.key && recipient.key != feed.key
            && recipient.key != market_binding_account.key, ClutchError::AccountAlias)?;
    }
    require(neutral_sink.key != locator_payer.key && neutral_sink.key != adjacency_payer.key,
        ClutchError::AccountAlias)?;
    precheck_credit(locator_payer, &credits)?; precheck_credit(adjacency_payer, &credits)?;
    precheck_credit(neutral_sink, &credits)?;
    drop(feed_body); drop(adjacency_body); drop(locator_body); drop(root_body);
    encode_account(root, |out| plan.indexed_root_poststate().encode(out))?;
    credit_lamports(locator_payer, credits[0].amount())?;
    credit_lamports(neutral_sink, credits[1].amount())?;
    credit_lamports(adjacency_payer, credits[2].amount())?;
    credit_lamports(neutral_sink, credits[3].amount())?;
    close_program_account(locator)?; close_program_account(adjacency)
}
fn close_input(program_id: &Pubkey, account: &AccountInfo<'_>)
    -> ExactIndexCloseAccountInputV1
{
    ExactIndexCloseAccountInputV1 { account: id(account.key), lamports: account.lamports(),
        owner: id(account.owner), program_id: id(program_id), writable: account.is_writable,
        executable: account.executable }
}

/// Close the 1,196-byte indexed root after base and compact projections retire.
pub(crate) fn retire_indexed_root_v1(program_id: &Pubkey, root: &AccountInfo<'_>,
    market_binding_account: &AccountInfo<'_>, root_payer: &AccountInfo<'_>,
    neutral_sink: &AccountInfo<'_>) -> Outcome<()>
{
    require_program_account(program_id, root, true, Some(INDEXED_SETTLEMENT_ROOT_BYTES_V1))?;
    let binding = authenticate_market_binding_v2(program_id, market_binding_account)?;
    let body = borrow_data(root)?; let indexed = IndexedSettlementRootV1AccountV1::decode(&body)?;
    let base = indexed.base();
    expect_pda(root.key, seeds::general_v2_settlement_root_pda(program_id, &base.epoch().bytes(),
        &base.settlement_candidate_id().bytes()), Some(base.stored_bump()))?;
    let terminal = indexed.terminal_projection(&RuntimeSha256, id(root.key))?;
    let rent = base.root_rent(); rent.validate()?;
    require(indexed.is_terminal() && terminal.base().root_account() == id(root.key)
        && base.market_binding() == id(market_binding_account.key)
        && binding.base().market == base.market()
        && binding.base().market_instance_v2_id == base.market_instance_v2_id()
        && binding.batch_policy_id() == base.batch_policy_id()
        && binding.base().neutral_sink == id(neutral_sink.key)
        && rent.payer == id(root_payer.key) && root_payer.is_writable && neutral_sink.is_writable
        && root_payer.key != neutral_sink.key && root_payer.key != root.key
        && neutral_sink.key != root.key && root_payer.key != market_binding_account.key
        && neutral_sink.key != market_binding_account.key, ClutchError::MismatchedState)?;
    let minimum = rent.refundable_principal.checked_add(rent.donation_floor)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(root.lamports() >= minimum, ClutchError::MismatchedState)?;
    let donation = root.lamports().checked_sub(rent.refundable_principal)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    root_payer.lamports().checked_add(rent.refundable_principal)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    neutral_sink.lamports().checked_add(donation)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    drop(body); credit_lamports(root_payer, rent.refundable_principal)?;
    credit_lamports(neutral_sink, donation)?; close_program_account(root)
}

fn precheck_credit(account: &AccountInfo<'_>, credits: &[clutch_general_v2_runtime::exact_index_plane::ExactIndexCloseCreditV1; 4])
    -> Outcome<()>
{
    let recipient = id(account.key); let mut after = account.lamports();
    for credit in credits { if credit.recipient() == recipient {
        after = after.checked_add(credit.amount()).ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    }}
    let _checked = after; Ok(())
}
fn credit_lamports(account: &AccountInfo<'_>, amount: u64) -> Outcome<()> {
    let after = account.lamports().checked_add(amount).ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let mut lamports = account.try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    **lamports = after; Ok(())
}
fn close_program_account(account: &AccountInfo<'_>) -> Outcome<()> {
    { let mut lamports = account.try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?; **lamports = 0; }
    account.resize(0).map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    account.assign(&SYSTEM_PROGRAM_ID);
    require(account.lamports() == 0 && account.data_len() == 0
        && *account.owner == SYSTEM_PROGRAM_ID, ClutchError::MismatchedState)
}
