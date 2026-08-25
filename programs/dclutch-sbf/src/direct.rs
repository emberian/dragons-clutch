//! Permissionless Direct V2 matching and custody runtime.
//!
//! This module is the narrow SVM trust boundary for the allocation-free Direct
//! contract. It authenticates every account and native Ed25519 instruction,
//! runs the pure transition before the first CPI/write, and then applies the
//! exact token, Position, Market, replay, record, and RentCredit effects.

use alloc::{vec, vec::Vec};

use dclutch_capability_contract::{ActivationPolicy, CapabilityManifestV1};
use dclutch_collateral_contract::{
    COLLATERAL_CUSTODY_PDA_DOMAIN, COLLATERAL_VAULT_PDA_DOMAIN, CollateralCustodyV1,
};
use dclutch_core_contract::{ContentId, MarketRoot, Phase};
use dclutch_direct_contract::{
    DIRECT_INTENT_ESCROW_PDA_DOMAIN_V2, DIRECT_INTENT_RECORD_BYTES_V2,
    DIRECT_INTENT_RECORD_PDA_DOMAIN_V2, DirectCapabilitySelectionV2, DirectIntentRecordV2,
    DirectIntentV2, DirectPositionV2, InlineParticipantAccountsV2, LiveRecordCloseV2,
    MAKER_REPLAY_ROOT_BYTES_V2, MAKER_REPLAY_ROOT_PDA_DOMAIN_V2, MakerReplayRootV2,
    ParticipantAccountsV2, ReplayRootStateV2, RuntimeComplementaryBuyMatchInPlaceV2,
    RuntimeComplementarySellMatchInPlaceV2, RuntimeInlineComplementaryMatchInPlaceV2,
    RuntimeInlineOrdinaryMatchInPlaceV2, RuntimeOrdinaryMatchInPlaceV2,
    RuntimeRegistrationInPlaceV2, RuntimeUnwindInPlaceV2, RuntimeUnwindKindV2, Side,
    VENUE_FEE_POLICY_SCHEMA_RELEASE_ID_V3, VenueFeePolicyV3, adapter, cancel_through_v1,
    close_replay_registration_v2, prepare_replay_root_close_v2,
    register_intent_runtime_in_place_v2, settle_inline_complementary_runtime_in_place_v2,
    settle_inline_ordinary_runtime_in_place_v2, settle_merge_runtime_in_place_v2,
    settle_ordinary_runtime_in_place_v2, settle_split_runtime_in_place_v2,
    terminal_rent_credit_close_plan_v1, unwind_intent_runtime_in_place_v2,
    validate_direct_capability_selection_v2,
};
use dclutch_market_contract::market::{CategoricalMarketV1, decode_market_outcome_count};
use dclutch_realm_contract::{POSITION_PDA_DOMAIN, REALM_PDA_DOMAIN, RealmV1};
use dclutch_record_contract::{ContentDigest, RecordKeyV1, SchemaReleaseId};
use dclutch_rent_contract::{
    RENT_CREDIT_BYTES_V1, RENT_CREDIT_PDA_DOMAIN_V1, RefundAuthority, RentCreditV1,
};
use dclutch_token_svm::{
    ACCOUNT_BYTES, AuthorityRole, CollateralAdapterReleaseV1, ExactTransferInput, Mint,
    TokenAccount, close_account, initialize_account3, transfer_checked,
};
use solana_instructions_sysvar::{load_current_index_checked, load_instruction_at_checked};
use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::{Sysvar, SysvarSerialize},
};
use solana_sdk_ids::{native_loader, system_program, sysvar};
use solana_system_interface::instruction::{allocate, assign, create_account, transfer};

use crate::{
    AdapterError,
    authenticate::MARKET_SEED,
    realm::{
        recognized_program_loader, require_authority_policy, require_freeze_policy,
        select_adapter_release,
    },
    records::{
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, derive_record_pda,
        with_authenticated_finalized_record_v1,
    },
};

const SPLIT_BASE: usize = 12;
const INLINE_ORDINARY_BASE: usize = 13;
const INLINE_COMPLEMENT_BASE: usize = 15;

fn copied<T: Copy>(values: &[T], index: usize) -> Result<T, ProgramError> {
    values
        .get(index)
        .copied()
        .ok_or_else(|| AdapterError::Arithmetic.into())
}

fn replace<T>(values: &mut [T], index: usize, value: T) -> Result<(), ProgramError> {
    let destination = values.get_mut(index).ok_or(AdapterError::Arithmetic)?;
    *destination = value;
    Ok(())
}

#[derive(Clone, Copy)]
struct RealmFacts {
    realm: RealmV1,
    release: CollateralAdapterReleaseV1,
    mint: Mint,
}

#[derive(Clone, Copy)]
struct MarketSigner {
    digest: [u8; 32],
    bump: u8,
}

#[derive(Clone, Copy)]
struct MarketFacts {
    root: MarketRoot,
    hoard_atoms: u64,
    outcome_count: u8,
}

#[derive(Clone, Copy)]
struct TransferFacts {
    source: TokenAccount,
    destination: TokenAccount,
    authority_role: AuthorityRole,
    source_lamports: u64,
    destination_lamports: u64,
    mint_lamports: u64,
}

#[derive(Clone, Copy)]
struct PolicyFacts {
    policy: VenueFeePolicyV3,
    digest: [u8; 32],
}

#[derive(Clone, Copy)]
struct RootFacts {
    state: ReplayRootStateV2,
    created: bool,
    bump: u8,
}

struct RootSeeds<'a> {
    market: &'a Pubkey,
    generation: [u8; 8],
    maker: &'a [u8; 32],
    bump: [u8; 1],
}

impl RootSeeds<'_> {
    fn refs(&self) -> [&[u8]; 5] {
        [
            MAKER_REPLAY_ROOT_PDA_DOMAIN_V2,
            self.market.as_ref(),
            self.generation.as_slice(),
            self.maker.as_slice(),
            self.bump.as_slice(),
        ]
    }
}

fn root_seed_parts<'a>(
    market: &'a Pubkey,
    generation: u64,
    maker: &'a [u8; 32],
    bump: u8,
) -> RootSeeds<'a> {
    RootSeeds {
        market,
        generation: generation.to_le_bytes(),
        maker,
        bump: [bump],
    }
}

struct OwnedRecordSeeds {
    market: [u8; 32],
    generation: [u8; 8],
    maker: [u8; 32],
    nonce: [u8; 8],
    bump: [u8; 1],
}

impl OwnedRecordSeeds {
    fn new(intent: DirectIntentV2, bump: u8) -> Self {
        Self {
            market: *intent.market(),
            generation: intent.generation().to_le_bytes(),
            maker: *intent.maker(),
            nonce: intent.nonce().to_le_bytes(),
            bump: [bump],
        }
    }
    fn refs(&self) -> [&[u8]; 6] {
        [
            DIRECT_INTENT_RECORD_PDA_DOMAIN_V2,
            self.market.as_slice(),
            self.generation.as_slice(),
            self.maker.as_slice(),
            self.nonce.as_slice(),
            self.bump.as_slice(),
        ]
    }
}

struct EscrowSeeds<'a> {
    record: &'a Pubkey,
    bump: [u8; 1],
}

impl<'a> EscrowSeeds<'a> {
    fn new(record: &'a Pubkey, bump: u8) -> Self {
        Self {
            record,
            bump: [bump],
        }
    }
    fn refs(&self) -> [&[u8]; 3] {
        [
            DIRECT_INTENT_ESCROW_PDA_DOMAIN_V2,
            self.record.as_ref(),
            self.bump.as_slice(),
        ]
    }
}

/// Dispatch one exact Direct V2 action after family-magic routing.
pub(crate) fn dispatch(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    let header = adapter::decode_adapter_header_v2(instruction_data)
        .map_err(|_| AdapterError::InvalidInstruction)?;
    validate_frame(header.action, usize::from(header.participants), accounts)?;
    let market_index = match header.action {
        adapter::AdapterActionV2::RegisterBuy
        | adapter::AdapterActionV2::RegisterSell
        | adapter::AdapterActionV2::InlineOrdinary
        | adapter::AdapterActionV2::InlineSplit
        | adapter::AdapterActionV2::InlineMerge => 2,
        _ => 0,
    };
    let market_data = account(accounts, market_index)?
        .try_borrow_data()
        .map_err(|_| AdapterError::DirectAuthentication)?;
    let outcome_count = decode_market_outcome_count(&market_data)
        .map_err(|_| AdapterError::DirectAuthentication)?;
    drop(market_data);
    match header.action {
        adapter::AdapterActionV2::RegisterBuy | adapter::AdapterActionV2::RegisterSell => {
            process_register(program_id, accounts, instruction_data, outcome_count)
        }
        adapter::AdapterActionV2::CancelBuy | adapter::AdapterActionV2::CancelSell => {
            process_cancel(program_id, accounts, instruction_data, outcome_count)
        }
        adapter::AdapterActionV2::ExpireBuy | adapter::AdapterActionV2::ExpireSell => {
            process_expire(program_id, accounts, instruction_data, outcome_count)
        }
        adapter::AdapterActionV2::CloseInvalidatedBuy
        | adapter::AdapterActionV2::CloseInvalidatedSell => {
            process_close_invalidated(program_id, accounts, instruction_data, outcome_count)
        }
        adapter::AdapterActionV2::Ordinary => {
            process_ordinary(program_id, accounts, instruction_data, outcome_count)
        }
        adapter::AdapterActionV2::Split | adapter::AdapterActionV2::Merge => process_complementary(
            program_id,
            accounts,
            instruction_data,
            header.action,
            outcome_count,
        ),
        adapter::AdapterActionV2::InlineOrdinary => {
            process_inline_ordinary(program_id, accounts, instruction_data, outcome_count)
        }
        adapter::AdapterActionV2::InlineSplit | adapter::AdapterActionV2::InlineMerge => {
            if header.participants != 2 {
                return Err(AdapterError::DirectAuthentication.into());
            }
            process_inline_complementary(program_id, accounts, instruction_data, header.action)
        }
        adapter::AdapterActionV2::CloseReplayRegistration => {
            process_close_registration(program_id, accounts, instruction_data, outcome_count)
        }
        adapter::AdapterActionV2::CloseReplayRoot => {
            process_close_root(program_id, accounts, instruction_data, outcome_count)
        }
        adapter::AdapterActionV2::CancelThrough => {
            process_cancel_through(program_id, accounts, instruction_data)
        }
    }
}

fn validate_frame(
    action: adapter::AdapterActionV2,
    participants: usize,
    accounts: &[AccountInfo<'_>],
) -> Result<(), ProgramError> {
    let mut metas = Vec::new();
    metas
        .try_reserve_exact(accounts.len())
        .map_err(|_| AdapterError::Arithmetic)?;
    for value in accounts {
        metas.push(adapter::AdapterAccountMetaV2 {
            key: value.key.to_bytes(),
            is_signer: value.is_signer,
            is_writable: value.is_writable,
        });
    }
    adapter::validate_account_frame_v2(action, participants, &metas)
        .map_err(|_| AdapterError::AccountPrivilege)?;
    for value in accounts {
        if value.executable
            && value.key != &system_program::ID
            && value.key.to_bytes() != dclutch_token_svm::LEGACY_TOKEN_PROGRAM_ID
            && value.key.to_bytes() != dclutch_token_svm::TOKEN_2022_PROGRAM_ID
        {
            return Err(AdapterError::DirectAuthentication.into());
        }
    }
    Ok(())
}

fn authenticate_current_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    data: &[u8],
    instructions: &AccountInfo<'_>,
) -> Result<(u16, Instruction), ProgramError> {
    if instructions.key != &solana_instructions_sysvar::ID
        || instructions.owner != &sysvar::ID
        || instructions.is_writable
        || instructions.is_signer
    {
        return Err(AdapterError::DirectAuthentication.into());
    }
    let current =
        load_current_index_checked(instructions).map_err(|_| AdapterError::DirectAuthentication)?;
    let loaded = load_instruction_at_checked(usize::from(current), instructions)
        .map_err(|_| AdapterError::DirectAuthentication)?;
    if loaded.program_id != *program_id
        || loaded.data.as_slice() != data
        || loaded.accounts.len() != accounts.len()
    {
        return Err(AdapterError::DirectAuthentication.into());
    }
    for (meta, actual) in loaded.accounts.iter().zip(accounts) {
        if meta.pubkey != *actual.key
            || meta.is_signer != actual.is_signer
            || meta.is_writable != actual.is_writable
        {
            return Err(AdapterError::DirectAuthentication.into());
        }
    }
    if current == 0 {
        return Err(AdapterError::DirectAuthentication.into());
    }
    let preceding = load_instruction_at_checked(usize::from(current - 1), instructions)
        .map_err(|_| AdapterError::DirectAuthentication)?;
    if !preceding.accounts.is_empty() {
        return Err(AdapterError::DirectAuthentication.into());
    }
    Ok((current, preceding))
}

fn authorization_runtime(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    data: &[u8],
    instructions: &AccountInfo<'_>,
    expectations: &[(u16, &DirectIntentV2)],
) -> Result<Vec<adapter::Ed25519AuthorizationV2>, ProgramError> {
    let (current, preceding) =
        authenticate_current_instruction(program_id, accounts, data, instructions)?;
    let mut mapped = Vec::new();
    mapped
        .try_reserve_exact(expectations.len())
        .map_err(|_| AdapterError::Arithmetic)?;
    for (message_offset, intent) in expectations {
        let start = usize::from(*message_offset);
        let end = start
            .checked_add(dclutch_direct_contract::DIRECT_INTENT_BYTES_V2)
            .ok_or(AdapterError::DirectAuthentication)?;
        mapped.push(adapter::Ed25519ExpectationV2 {
            message_offset: *message_offset,
            signer: *intent.maker(),
            message: data
                .get(start..end)
                .ok_or(AdapterError::DirectAuthentication)?,
        });
    }
    let view = adapter::Ed25519InstructionViewV2 {
        program_id: preceding.program_id.to_bytes(),
        ed25519_data: &preceding.data,
        preceding_index: current - 1,
        current_index: current,
        current_data: data,
    };
    let mut output = Vec::new();
    output
        .try_reserve_exact(mapped.len())
        .map_err(|_| AdapterError::Arithmetic)?;
    for index in 0..mapped.len() {
        output.push(
            adapter::inspect_preceding_ed25519_batch_item_v2(view, &mapped, index)
                .map_err(|_| AdapterError::DirectAuthentication)?,
        );
    }
    Ok(output)
}

fn message_authorization(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    data: &[u8],
    instructions: &AccountInfo<'_>,
    signer: [u8; 32],
    message: &[u8],
) -> Result<adapter::Ed25519AuthorizationV2, ProgramError> {
    let (current, preceding) =
        authenticate_current_instruction(program_id, accounts, data, instructions)?;
    adapter::inspect_preceding_ed25519_v2(
        adapter::Ed25519InstructionViewV2 {
            program_id: preceding.program_id.to_bytes(),
            ed25519_data: &preceding.data,
            preceding_index: current - 1,
            current_index: current,
            current_data: data,
        },
        adapter::Ed25519ExpectationV2 {
            message_offset: 16,
            signer,
            message,
        },
    )
    .map_err(|_| AdapterError::DirectAuthentication.into())
}

fn account<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
    index: usize,
) -> Result<&'a AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or(AdapterError::AccountFrameLength.into())
}

fn map_phase(phase: Phase) -> adapter::MarketPhaseV2 {
    match phase {
        Phase::Founding => adapter::MarketPhaseV2::Founding,
        Phase::Open => adapter::MarketPhaseV2::Open,
        Phase::Resolved => adapter::MarketPhaseV2::Resolved,
        Phase::Retiring => adapter::MarketPhaseV2::Retiring,
        Phase::Retired => adapter::MarketPhaseV2::Retired,
    }
}

#[inline(never)]
fn authenticate_market(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
) -> Result<MarketFacts, ProgramError> {
    if account.owner != program_id || account.executable {
        return Err(AdapterError::DirectAuthentication.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::DirectAuthentication)?;
    let outcome_count =
        decode_market_outcome_count(&data).map_err(|_| AdapterError::DirectAuthentication)?;
    match outcome_count {
        2 => authenticate_market_width::<2>(program_id, account.key, &data),
        3 => authenticate_market_width::<3>(program_id, account.key, &data),
        4 => authenticate_market_width::<4>(program_id, account.key, &data),
        5 => authenticate_market_width::<5>(program_id, account.key, &data),
        6 => authenticate_market_width::<6>(program_id, account.key, &data),
        7 => authenticate_market_width::<7>(program_id, account.key, &data),
        8 => authenticate_market_width::<8>(program_id, account.key, &data),
        9 => authenticate_market_width::<9>(program_id, account.key, &data),
        10 => authenticate_market_width::<10>(program_id, account.key, &data),
        11 => authenticate_market_width::<11>(program_id, account.key, &data),
        12 => authenticate_market_width::<12>(program_id, account.key, &data),
        13 => authenticate_market_width::<13>(program_id, account.key, &data),
        14 => authenticate_market_width::<14>(program_id, account.key, &data),
        15 => authenticate_market_width::<15>(program_id, account.key, &data),
        16 => authenticate_market_width::<16>(program_id, account.key, &data),
        _ => Err(AdapterError::DirectAuthentication.into()),
    }
}

#[inline(never)]
fn authenticate_market_width<const N: usize>(
    program_id: &Pubkey,
    key: &Pubkey,
    data: &[u8],
) -> Result<MarketFacts, ProgramError> {
    let market =
        CategoricalMarketV1::<N>::decode(data).map_err(|_| AdapterError::DirectAuthentication)?;
    let identity_digest = hash(&market.root().identity().to_bytes()).to_bytes();
    let (expected, _) = Pubkey::find_program_address(&[MARKET_SEED, &identity_digest], program_id);
    if key != &expected {
        return Err(AdapterError::DirectAuthentication.into());
    }
    let encoded = encode_market(&market)?;
    if encoded.as_slice() != data {
        return Err(AdapterError::ContentIdentity.into());
    }
    Ok(MarketFacts {
        root: market.root(),
        hoard_atoms: market.hoard_atoms(),
        outcome_count: u8::try_from(N).map_err(|_| AdapterError::Arithmetic)?,
    })
}

fn market_signer(
    program_id: &Pubkey,
    market_account: &AccountInfo<'_>,
    root: MarketRoot,
) -> Result<MarketSigner, ProgramError> {
    let digest = hash(&root.identity().to_bytes()).to_bytes();
    let (expected, bump) = Pubkey::find_program_address(&[MARKET_SEED, &digest], program_id);
    if market_account.key != &expected {
        return Err(AdapterError::DirectAuthentication.into());
    }
    Ok(MarketSigner { digest, bump })
}

fn authenticate_position(
    program_id: &Pubkey,
    position_account: &AccountInfo<'_>,
    market_account: &AccountInfo<'_>,
    maker: &[u8; 32],
    generation: u64,
    outcome_count: u8,
) -> Result<DirectPositionV2, ProgramError> {
    let (expected, _) = Pubkey::find_program_address(
        &[
            POSITION_PDA_DOMAIN,
            market_account.key.as_ref(),
            maker.as_slice(),
        ],
        program_id,
    );
    if position_account.key != &expected || position_account.owner != program_id {
        return Err(AdapterError::DirectAuthentication.into());
    }
    let data = position_account
        .try_borrow_data()
        .map_err(|_| AdapterError::DirectAuthentication)?;
    let position =
        DirectPositionV2::decode(&data).map_err(|_| AdapterError::DirectAuthentication)?;
    if position.market() != market_account.key.as_ref()
        || position.owner() != *maker
        || position.generation() != generation
        || position.outcome_count() != outcome_count
    {
        return Err(AdapterError::DirectAuthentication.into());
    }
    Ok(position)
}

fn root_pda(
    program_id: &Pubkey,
    market: &Pubkey,
    generation: u64,
    maker: &[u8; 32],
) -> (Pubkey, u8) {
    let generation = generation.to_le_bytes();
    Pubkey::find_program_address(
        &[
            MAKER_REPLAY_ROOT_PDA_DOMAIN_V2,
            market.as_ref(),
            generation.as_slice(),
            maker.as_slice(),
        ],
        program_id,
    )
}

fn authenticate_root_state(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    market: &Pubkey,
    generation: u64,
    maker: &[u8; 32],
    permit_absent: bool,
) -> Result<RootFacts, ProgramError> {
    let (expected, bump) = root_pda(program_id, market, generation, maker);
    if root_account.key != &expected || root_account.executable {
        return Err(AdapterError::DirectAuthentication.into());
    }
    if is_prefunded_vacant(root_account) {
        if !permit_absent {
            return Err(AdapterError::DirectAuthentication.into());
        }
        return Ok(RootFacts {
            state: ReplayRootStateV2::absent(bump),
            created: true,
            bump,
        });
    }
    if root_account.owner != program_id || root_account.data_len() != MAKER_REPLAY_ROOT_BYTES_V2 {
        return Err(AdapterError::DirectAuthentication.into());
    }
    let data = root_account
        .try_borrow_data()
        .map_err(|_| AdapterError::DirectAuthentication)?;
    let root = MakerReplayRootV2::decode(&data).map_err(|_| AdapterError::DirectAuthentication)?;
    let mut canonical = [0; MAKER_REPLAY_ROOT_BYTES_V2];
    root.encode(&mut canonical)
        .map_err(|_| AdapterError::DirectAuthentication)?;
    if canonical.as_slice() != &data[..]
        || root.market() != market.as_ref()
        || root.generation() != generation
        || root.maker() != maker
        || root.bump() != bump
    {
        return Err(AdapterError::DirectAuthentication.into());
    }
    Ok(RootFacts {
        state: ReplayRootStateV2::existing(root),
        created: false,
        bump,
    })
}

fn existing_root(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    market: &Pubkey,
    generation: u64,
    maker: &[u8; 32],
) -> Result<MakerReplayRootV2, ProgramError> {
    match authenticate_root_state(program_id, root_account, market, generation, maker, false)?.state
    {
        ReplayRootStateV2::Existing(root) => Ok(root),
        ReplayRootStateV2::Absent { .. } => Err(AdapterError::DirectAuthentication.into()),
    }
}

fn authenticate_stored_root(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
) -> Result<MakerReplayRootV2, ProgramError> {
    if account.owner != program_id || account.data_len() != MAKER_REPLAY_ROOT_BYTES_V2 {
        return Err(AdapterError::DirectAuthentication.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::DirectAuthentication)?;
    let root = MakerReplayRootV2::decode(&data).map_err(|_| AdapterError::DirectAuthentication)?;
    let market = Pubkey::new_from_array(*root.market());
    let (expected, bump) = root_pda(program_id, &market, root.generation(), root.maker());
    let mut canonical = [0; MAKER_REPLAY_ROOT_BYTES_V2];
    root.encode(&mut canonical)
        .map_err(|_| AdapterError::DirectAuthentication)?;
    if account.key != &expected || root.bump() != bump || canonical.as_slice() != &data[..] {
        return Err(AdapterError::DirectAuthentication.into());
    }
    Ok(root)
}

fn record_pda(
    program_id: &Pubkey,
    market: &Pubkey,
    generation: u64,
    maker: &[u8; 32],
    nonce: u64,
) -> (Pubkey, u8) {
    let generation = generation.to_le_bytes();
    let nonce = nonce.to_le_bytes();
    Pubkey::find_program_address(
        &[
            DIRECT_INTENT_RECORD_PDA_DOMAIN_V2,
            market.as_ref(),
            generation.as_slice(),
            maker.as_slice(),
            nonce.as_slice(),
        ],
        program_id,
    )
}

fn authenticate_record(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
) -> Result<DirectIntentRecordV2, ProgramError> {
    if account.owner != program_id || account.data_len() != DIRECT_INTENT_RECORD_BYTES_V2 {
        return Err(AdapterError::DirectAuthentication.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::DirectAuthentication)?;
    let record =
        DirectIntentRecordV2::decode(&data).map_err(|_| AdapterError::DirectAuthentication)?;
    let intent = record.intent_ref();
    let (expected, bump) = record_pda(
        program_id,
        &Pubkey::new_from_array(*intent.market()),
        intent.generation(),
        intent.maker(),
        intent.nonce(),
    );
    let mut canonical = [0; DIRECT_INTENT_RECORD_BYTES_V2];
    record
        .encode(&mut canonical)
        .map_err(|_| AdapterError::DirectAuthentication)?;
    if account.key != &expected || record.bump() != bump || canonical.as_slice() != &data[..] {
        return Err(AdapterError::DirectAuthentication.into());
    }
    Ok(record)
}

fn authenticate_new_record(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    intent: DirectIntentV2,
) -> Result<u8, ProgramError> {
    let market = Pubkey::new_from_array(*intent.market());
    let (expected, bump) = record_pda(
        program_id,
        &market,
        intent.generation(),
        intent.maker(),
        intent.nonce(),
    );
    if account.key != &expected || !is_prefunded_vacant(account) {
        return Err(AdapterError::DirectAuthentication.into());
    }
    Ok(bump)
}

fn escrow_pda(program_id: &Pubkey, record: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[DIRECT_INTENT_ESCROW_PDA_DOMAIN_V2, record.as_ref()],
        program_id,
    )
}

fn participant_accounts(
    root: &AccountInfo<'_>,
    record: Option<&AccountInfo<'_>>,
    escrow: Option<&AccountInfo<'_>>,
    position: &AccountInfo<'_>,
    collateral: &AccountInfo<'_>,
) -> ParticipantAccountsV2 {
    ParticipantAccountsV2 {
        replay_root: root.key.to_bytes(),
        record: record.map_or([0; 32], |value| value.key.to_bytes()),
        escrow: escrow.map_or([0; 32], |value| value.key.to_bytes()),
        position: position.key.to_bytes(),
        collateral: collateral.key.to_bytes(),
    }
}

fn inline_accounts(
    root: &AccountInfo<'_>,
    position: &AccountInfo<'_>,
    collateral: &AccountInfo<'_>,
) -> InlineParticipantAccountsV2 {
    InlineParticipantAccountsV2 {
        replay_root: root.key.to_bytes(),
        position: position.key.to_bytes(),
        collateral: collateral.key.to_bytes(),
    }
}

fn authenticate_policy<'info>(
    program_id: &Pubkey,
    market: MarketRoot,
    policy_account: &AccountInfo<'info>,
    staging: &AccountInfo<'info>,
    manifest_account: &AccountInfo<'info>,
    rent_sysvar: &AccountInfo<'info>,
) -> Result<PolicyFacts, ProgramError> {
    let manifest_id = market.identity().capability_manifest_id();
    let manifest_key = RecordKeyV1::new(
        SchemaReleaseId::new(CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1)
            .map_err(|_| AdapterError::DirectAuthentication)?,
        ContentDigest::new(manifest_id.to_bytes())
            .map_err(|_| AdapterError::DirectAuthentication)?,
    );
    let (expected_manifest, _) = derive_record_pda(program_id, manifest_key, false);
    if manifest_account.key != &expected_manifest || manifest_account.owner != program_id {
        return Err(AdapterError::DirectAuthentication.into());
    }
    let manifest_data = manifest_account
        .try_borrow_data()
        .map_err(|_| AdapterError::DirectAuthentication)?;
    let manifest = CapabilityManifestV1::decode(&manifest_data)
        .map_err(|_| AdapterError::DirectAuthentication)?;
    if manifest.as_bytes() != &manifest_data[..]
        || hash(manifest.as_bytes()).to_bytes() != manifest_id.to_bytes()
    {
        return Err(AdapterError::DirectAuthentication.into());
    }
    let policy_data = policy_account
        .try_borrow_data()
        .map_err(|_| AdapterError::DirectAuthentication)?;
    let digest = hash(&policy_data).to_bytes();
    let selected = manifest
        .required_founding_entry_for_config(
            ContentId::new(digest).map_err(|_| AdapterError::DirectAuthentication)?,
        )
        .map_err(|_| AdapterError::DirectAuthentication)?;
    let entry = selected.entry();
    let funding = entry.funding_quote();
    validate_direct_capability_selection_v2(
        DirectCapabilitySelectionV2 {
            kind_id: entry.kind_id().to_bytes(),
            release_id: entry.release_id().to_bytes(),
            config_id: entry.config_id().to_bytes(),
            capacity_profile_id: entry.capacity_profile_id().to_bytes(),
            child_schema_id: entry.child_schema_id().to_bytes(),
            child_derivation_id: entry.child_derivation_id().to_bytes(),
            required_at_founding: entry.activation_policy() == ActivationPolicy::RequiredAtFounding,
            activation_deadline_slot: entry.activation_deadline_slot(),
            dependency_count: entry.dependency_count(),
            native_funding_total: funding.native_lamports_total(),
            realm_funding_total: funding.realm_collateral_total(),
            has_realm_funding_binding: funding.realm_collateral().is_some(),
        },
        digest,
    )
    .map_err(|_| AdapterError::DirectAuthentication)?;
    let policy = with_authenticated_finalized_record_v1(
        program_id,
        policy_account,
        staging,
        rent_sysvar,
        VENUE_FEE_POLICY_SCHEMA_RELEASE_ID_V3,
        digest,
        |record| {
            VenueFeePolicyV3::decode(record.exact_content())
                .map_err(|_| AdapterError::DirectAuthentication.into())
        },
    )?;
    drop(policy_data);
    Ok(PolicyFacts { policy, digest })
}

fn authenticate_realm(
    program_id: &Pubkey,
    realm_account: &AccountInfo<'_>,
    mint_account: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    root: MarketRoot,
) -> Result<RealmFacts, ProgramError> {
    if realm_account.owner != program_id
        || mint_account.owner != token_program.key
        || !recognized_program_loader(token_program.owner)
        || !token_program.executable
    {
        return Err(AdapterError::DirectAuthentication.into());
    }
    let data = realm_account
        .try_borrow_data()
        .map_err(|_| AdapterError::DirectAuthentication)?;
    let realm = RealmV1::decode(&data).map_err(|_| AdapterError::DirectAuthentication)?;
    let digest = hash(&data).to_bytes();
    let (expected, _) = Pubkey::find_program_address(&[REALM_PDA_DOMAIN, &digest], program_id);
    if realm.to_bytes().as_slice() != &data[..]
        || root.identity().realm_id().to_bytes() != digest
        || realm_account.key != &expected
        || realm.token_program() != token_program.key.as_ref()
        || realm.collateral_mint() != mint_account.key.as_ref()
    {
        return Err(AdapterError::DirectAuthentication.into());
    }
    let release = select_adapter_release(*realm.collateral_adapter_release_id())
        .map_err(|_| AdapterError::DirectAuthentication)?;
    if release.token_program() != token_program.key.to_bytes() {
        return Err(AdapterError::DirectAuthentication.into());
    }
    let mint_data = mint_account
        .try_borrow_data()
        .map_err(|_| AdapterError::DirectAuthentication)?;
    let mint = release
        .profile()
        .check_mint(token_program.key.to_bytes(), &mint_data)
        .map_err(|_| AdapterError::DirectAuthentication)?;
    require_authority_policy(realm.mint_authority_policy(), &mint.mint_authority)
        .map_err(|_| AdapterError::DirectAuthentication)?;
    require_freeze_policy(realm.freeze_authority_policy(), &mint.freeze_authority)
        .map_err(|_| AdapterError::DirectAuthentication)?;
    Ok(RealmFacts {
        realm,
        release,
        mint,
    })
}

fn authenticate_token_account(
    account: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    realm: RealmFacts,
) -> Result<TokenAccount, ProgramError> {
    if account.owner != token_program.key {
        return Err(AdapterError::DirectAuthentication.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::DirectAuthentication)?;
    let token = realm
        .release
        .profile()
        .check_transfer_account(token_program.key.to_bytes(), &data)
        .map_err(|_| AdapterError::DirectAuthentication)?;
    if token.mint != *realm.realm.collateral_mint() {
        return Err(AdapterError::DirectAuthentication.into());
    }
    Ok(token)
}

fn buy_debit_authority(
    account: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    realm: RealmFacts,
) -> Result<adapter::BuyDebitAuthorityV2, ProgramError> {
    let token = authenticate_token_account(account, token_program, realm)?;
    let delegate = match token.delegate {
        dclutch_token_svm::COption::Some(value) => value,
        dclutch_token_svm::COption::None => return Err(AdapterError::DirectAuthentication.into()),
    };
    Ok(adapter::BuyDebitAuthorityV2 {
        token_account: account.key.to_bytes(),
        mint: token.mint,
        owner: token.owner,
        delegate,
        delegated_amount: token.delegated_amount,
    })
}

fn require_inline_buy_debit_residual(
    account: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    realm: RealmFacts,
    before: adapter::BuyDebitAuthorityV2,
    consumed: u64,
) -> Result<(), ProgramError> {
    let after = authenticate_token_account(account, token_program, realm)?;
    let residual = before
        .delegated_amount
        .checked_sub(consumed)
        .ok_or(AdapterError::DirectPostcondition)?;
    // The token program clears a depleted delegate only when it executes a
    // positive transfer. A zero-price fill performs no token CPI and leaves an
    // existing zero-allowance delegate representation unchanged.
    let expected_delegate = if consumed != 0 && residual == 0 {
        dclutch_token_svm::COption::None
    } else {
        dclutch_token_svm::COption::Some(before.delegate)
    };
    if after.owner != before.owner
        || after.mint != before.mint
        || after.delegate != expected_delegate
        || after.delegated_amount != residual
    {
        return Err(AdapterError::DirectPostcondition.into());
    }
    Ok(())
}

fn escrow_authority(
    account: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    realm: RealmFacts,
    expected_amount: u64,
) -> Result<adapter::EscrowAuthorityV2, ProgramError> {
    let token = authenticate_token_account(account, token_program, realm)?;
    if token.delegate != dclutch_token_svm::COption::None
        || token.close_authority != dclutch_token_svm::COption::None
        || token.native_reserve != dclutch_token_svm::COption::None
        || token.amount < expected_amount
    {
        return Err(AdapterError::DirectAuthentication.into());
    }
    Ok(adapter::EscrowAuthorityV2 {
        token_account: account.key.to_bytes(),
        mint: token.mint,
        authority: token.owner,
    })
}

fn escrow_donation(
    account: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    realm: RealmFacts,
    reserved_collateral: u64,
) -> Result<u64, ProgramError> {
    authenticate_token_account(account, token_program, realm)?
        .amount
        .checked_sub(reserved_collateral)
        .ok_or_else(|| AdapterError::DirectAuthentication.into())
}

fn authenticate_custody(
    program_id: &Pubkey,
    market_account: &AccountInfo<'_>,
    custody_account: &AccountInfo<'_>,
    generation: u64,
) -> Result<CollateralCustodyV1, ProgramError> {
    let (expected, _) = Pubkey::find_program_address(
        &[COLLATERAL_CUSTODY_PDA_DOMAIN, market_account.key.as_ref()],
        program_id,
    );
    if custody_account.key != &expected || custody_account.owner != program_id {
        return Err(AdapterError::DirectAuthentication.into());
    }
    let data = custody_account
        .try_borrow_data()
        .map_err(|_| AdapterError::DirectAuthentication)?;
    let custody =
        CollateralCustodyV1::decode(&data).map_err(|_| AdapterError::DirectAuthentication)?;
    if custody.to_bytes().as_slice() != &data[..]
        || custody.market() != market_account.key.to_bytes()
        || custody.generation() != generation
    {
        return Err(AdapterError::DirectAuthentication.into());
    }
    Ok(custody)
}

fn authenticate_vault(
    program_id: &Pubkey,
    market_account: &AccountInfo<'_>,
    vault: &AccountInfo<'_>,
    mint: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    realm: RealmFacts,
) -> Result<TokenAccount, ProgramError> {
    let (expected, _) = Pubkey::find_program_address(
        &[COLLATERAL_VAULT_PDA_DOMAIN, market_account.key.as_ref()],
        program_id,
    );
    if vault.key != &expected || vault.owner != token_program.key {
        return Err(AdapterError::DirectAuthentication.into());
    }
    let data = vault
        .try_borrow_data()
        .map_err(|_| AdapterError::DirectAuthentication)?;
    realm
        .release
        .profile()
        .check_custody_account(
            token_program.key.to_bytes(),
            &data,
            mint.key.to_bytes(),
            market_account.key.to_bytes(),
        )
        .map_err(|_| AdapterError::DirectAuthentication.into())
}

fn authenticate_rent_credit(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    authority: &[u8; 32],
) -> Result<(RentCreditV1, u8), ProgramError> {
    let refund =
        RefundAuthority::new(*authority).map_err(|_| AdapterError::DirectAuthentication)?;
    let (expected, bump) = Pubkey::find_program_address(
        &[RENT_CREDIT_PDA_DOMAIN_V1, authority.as_slice()],
        program_id,
    );
    if account.key != &expected
        || account.owner != program_id
        || account.data_len() != RENT_CREDIT_BYTES_V1
        || account.executable
    {
        return Err(AdapterError::DirectAuthentication.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::DirectAuthentication)?;
    let credit = RentCreditV1::decode(&data).map_err(|_| AdapterError::DirectAuthentication)?;
    credit
        .validate_binding(refund, bump)
        .map_err(|_| AdapterError::DirectAuthentication)?;
    if credit.to_bytes().as_slice() != &data[..] {
        return Err(AdapterError::DirectAuthentication.into());
    }
    Ok((credit, bump))
}

fn require_system_and_rent(
    system: &AccountInfo<'_>,
    rent: &AccountInfo<'_>,
) -> Result<Rent, ProgramError> {
    if system.key != &system_program::ID
        || system.owner != &native_loader::ID
        || !system.executable
        || rent.key != &sysvar::rent::ID
        || rent.owner != &sysvar::ID
    {
        return Err(AdapterError::DirectAuthentication.into());
    }
    Rent::from_account_info(rent).map_err(|_| AdapterError::DirectAuthentication.into())
}

fn is_prefunded_vacant(account: &AccountInfo<'_>) -> bool {
    account.owner == &system_program::ID && !account.executable && account.data_is_empty()
}

fn creation_top_up(account: &AccountInfo<'_>, minimum_balance: u64) -> Result<u64, ProgramError> {
    if !is_prefunded_vacant(account) {
        return Err(AdapterError::DirectAuthentication.into());
    }
    Ok(minimum_balance.saturating_sub(account.lamports()))
}

fn current_slot() -> Result<u64, ProgramError> {
    Clock::get()
        .map(|clock| clock.slot)
        .map_err(|_| AdapterError::DirectAuthentication.into())
}

fn encode_market<const N: usize>(market: &CategoricalMarketV1<N>) -> Result<Vec<u8>, ProgramError> {
    let mut output = exact_zeroed(
        CategoricalMarketV1::<N>::encoded_len().map_err(|_| AdapterError::DirectAuthentication)?,
    )?;
    market
        .encode(&mut output)
        .map_err(|_| AdapterError::DirectAuthentication)?;
    Ok(output)
}

fn exact_zeroed(length: usize) -> Result<Vec<u8>, ProgramError> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| AdapterError::Arithmetic)?;
    output.resize(length, 0);
    Ok(output)
}

fn preflight_mutable(accounts: &[&AccountInfo<'_>]) -> Result<(), ProgramError> {
    for value in accounts {
        drop(
            value
                .try_borrow_mut_lamports()
                .map_err(|_| AdapterError::DirectAuthentication)?,
        );
        drop(
            value
                .try_borrow_mut_data()
                .map_err(|_| AdapterError::DirectAuthentication)?,
        );
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn authenticate_transfer(
    source: &AccountInfo<'_>,
    destination: &AccountInfo<'_>,
    mint: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    realm: RealmFacts,
    authority: &Pubkey,
    quantity: u64,
) -> Result<TransferFacts, ProgramError> {
    if source.owner != token_program.key
        || destination.owner != token_program.key
        || mint.owner != token_program.key
    {
        return Err(AdapterError::DirectAuthentication.into());
    }
    let mint_data = mint
        .try_borrow_data()
        .map_err(|_| AdapterError::DirectAuthentication)?;
    let source_data = source
        .try_borrow_data()
        .map_err(|_| AdapterError::DirectAuthentication)?;
    let destination_data = destination
        .try_borrow_data()
        .map_err(|_| AdapterError::DirectAuthentication)?;
    let facts = realm
        .release
        .profile()
        .check_transfer(ExactTransferInput {
            program_id: token_program.key.to_bytes(),
            mint_address: mint.key.to_bytes(),
            mint_data: &mint_data,
            source_data: &source_data,
            destination_data: &destination_data,
            authority: authority.to_bytes(),
            amount: quantity,
            decimals: realm.mint.decimals,
        })
        .map_err(|_| AdapterError::DirectAuthentication)?;
    if facts.mint() != realm.mint {
        return Err(AdapterError::DirectAuthentication.into());
    }
    Ok(TransferFacts {
        source: facts.source(),
        destination: facts.destination(),
        authority_role: facts.authority_role(),
        source_lamports: source.lamports(),
        destination_lamports: destination.lamports(),
        mint_lamports: mint.lamports(),
    })
}

#[allow(clippy::too_many_arguments)]
fn authenticate_transfer_post(
    source: &AccountInfo<'_>,
    destination: &AccountInfo<'_>,
    mint: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    realm: RealmFacts,
    before: TransferFacts,
    quantity: u64,
) -> Result<(), ProgramError> {
    if source.owner != token_program.key
        || destination.owner != token_program.key
        || mint.owner != token_program.key
        || source.lamports() != before.source_lamports
        || destination.lamports() != before.destination_lamports
        || mint.lamports() != before.mint_lamports
    {
        return Err(AdapterError::DirectPostcondition.into());
    }
    let mint_data = mint
        .try_borrow_data()
        .map_err(|_| AdapterError::DirectPostcondition)?;
    let source_data = source
        .try_borrow_data()
        .map_err(|_| AdapterError::DirectPostcondition)?;
    let destination_data = destination
        .try_borrow_data()
        .map_err(|_| AdapterError::DirectPostcondition)?;
    let mint_after = realm
        .release
        .profile()
        .check_mint(token_program.key.to_bytes(), &mint_data)
        .map_err(|_| AdapterError::DirectPostcondition)?;
    let source_after = realm
        .release
        .profile()
        .check_transfer_account(token_program.key.to_bytes(), &source_data)
        .map_err(|_| AdapterError::DirectPostcondition)?;
    let destination_after = realm
        .release
        .profile()
        .check_transfer_account(token_program.key.to_bytes(), &destination_data)
        .map_err(|_| AdapterError::DirectPostcondition)?;
    let mut expected_source = before.source;
    expected_source.amount = expected_source
        .amount
        .checked_sub(quantity)
        .ok_or(AdapterError::DirectPostcondition)?;
    if before.authority_role == AuthorityRole::Delegate {
        expected_source.delegated_amount = expected_source
            .delegated_amount
            .checked_sub(quantity)
            .ok_or(AdapterError::DirectPostcondition)?;
        if expected_source.delegated_amount == 0 {
            expected_source.delegate = dclutch_token_svm::COption::None;
        }
    }
    let mut expected_destination = before.destination;
    expected_destination.amount = expected_destination
        .amount
        .checked_add(quantity)
        .ok_or(AdapterError::DirectPostcondition)?;
    if mint_after != realm.mint
        || source_after != expected_source
        || destination_after != expected_destination
    {
        return Err(AdapterError::DirectPostcondition.into());
    }
    Ok(())
}

fn transfer_instruction(
    realm: RealmFacts,
    source: &Pubkey,
    destination: &Pubkey,
    mint: &Pubkey,
    authority: &Pubkey,
    quantity: u64,
) -> Result<Instruction, ProgramError> {
    let spec = transfer_checked(
        realm.release.token_program(),
        source.to_bytes(),
        mint.to_bytes(),
        destination.to_bytes(),
        authority.to_bytes(),
        quantity,
        realm.mint.decimals,
    )
    .map_err(|_| AdapterError::DirectAuthentication)?;
    let accounts = vec![
        AccountMeta::new(*source, false),
        AccountMeta::new_readonly(*mint, false),
        AccountMeta::new(*destination, false),
        AccountMeta::new_readonly(*authority, true),
    ];
    Ok(Instruction {
        program_id: Pubkey::new_from_array(*spec.program_id()),
        accounts,
        data: Vec::from(*spec.data()),
    })
}

#[allow(clippy::too_many_arguments)]
fn execute_transfer_signed<'info>(
    source: &AccountInfo<'info>,
    destination: &AccountInfo<'info>,
    mint: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    authority: &AccountInfo<'info>,
    realm: RealmFacts,
    quantity: u64,
    signer_seeds: &[&[u8]],
) -> Result<(), ProgramError> {
    if quantity == 0 {
        return Ok(());
    }
    let before = authenticate_transfer(
        source,
        destination,
        mint,
        token_program,
        realm,
        authority.key,
        quantity,
    )?;
    let instruction = transfer_instruction(
        realm,
        source.key,
        destination.key,
        mint.key,
        authority.key,
        quantity,
    )?;
    invoke_signed(
        &instruction,
        &[
            source.clone(),
            mint.clone(),
            destination.clone(),
            authority.clone(),
            token_program.clone(),
        ],
        &[signer_seeds],
    )
    .map_err(|_| AdapterError::DirectTokenCpi)?;
    authenticate_transfer_post(
        source,
        destination,
        mint,
        token_program,
        realm,
        before,
        quantity,
    )
}

fn create_pda<'info>(
    payer: &AccountInfo<'info>,
    created: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    rent_lamports: u64,
    space: usize,
    owner: &Pubkey,
    signer_seeds: &[&[u8]],
) -> Result<(), ProgramError> {
    let space = u64::try_from(space).map_err(|_| AdapterError::Arithmetic)?;
    let before = created.lamports();
    let top_up = creation_top_up(created, rent_lamports)?;
    if before == 0 {
        let instruction = create_account(payer.key, created.key, rent_lamports, space, owner);
        invoke_signed(
            &instruction,
            &[payer.clone(), created.clone(), system.clone()],
            &[signer_seeds],
        )
        .map_err(|_| AdapterError::DirectCreateCpi)?;
    } else {
        if top_up != 0 {
            invoke(
                &transfer(payer.key, created.key, top_up),
                &[payer.clone(), created.clone(), system.clone()],
            )
            .map_err(|_| AdapterError::DirectCreateCpi)?;
        }
        invoke_signed(
            &allocate(created.key, space),
            &[created.clone(), system.clone()],
            &[signer_seeds],
        )
        .map_err(|_| AdapterError::DirectCreateCpi)?;
        invoke_signed(
            &assign(created.key, owner),
            &[created.clone(), system.clone()],
            &[signer_seeds],
        )
        .map_err(|_| AdapterError::DirectCreateCpi)?;
    }
    let expected_lamports = before.checked_add(top_up).ok_or(AdapterError::Arithmetic)?;
    if created.owner != owner
        || created.lamports() != expected_lamports
        || created.lamports() < rent_lamports
        || created.data_len() != usize::try_from(space).map_err(|_| AdapterError::Arithmetic)?
    {
        return Err(AdapterError::DirectPostcondition.into());
    }
    Ok(())
}

fn initialize_escrow<'info>(
    escrow: &AccountInfo<'info>,
    mint: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    record: &Pubkey,
) -> Result<(), ProgramError> {
    let spec = initialize_account3(
        token_program.key.to_bytes(),
        escrow.key.to_bytes(),
        mint.key.to_bytes(),
        record.to_bytes(),
    )
    .map_err(|_| AdapterError::DirectAuthentication)?;
    let instruction = Instruction {
        program_id: *token_program.key,
        accounts: vec![
            AccountMeta::new(*escrow.key, false),
            AccountMeta::new_readonly(*mint.key, false),
        ],
        data: Vec::from(*spec.data()),
    };
    invoke(
        &instruction,
        &[escrow.clone(), mint.clone(), token_program.clone()],
    )
    .map_err(|_| AdapterError::DirectTokenCpi.into())
}

fn close_token_instruction(
    token_program: &Pubkey,
    source: &Pubkey,
    credit: &Pubkey,
    authority: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let spec = close_account(
        token_program.to_bytes(),
        source.to_bytes(),
        credit.to_bytes(),
        authority.to_bytes(),
    )
    .map_err(|_| AdapterError::DirectAuthentication)?;
    Ok(Instruction {
        program_id: *token_program,
        accounts: vec![
            AccountMeta::new(*source, false),
            AccountMeta::new(*credit, false),
            AccountMeta::new_readonly(*authority, true),
        ],
        data: Vec::from(*spec.data()),
    })
}

#[allow(clippy::too_many_arguments)]
fn close_token_to_credit<'info>(
    program_id: &Pubkey,
    source: &AccountInfo<'info>,
    credit: &AccountInfo<'info>,
    authority: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    rent: &Rent,
    persisted_payer: &[u8; 32],
    signer_seeds: &[&[u8]],
) -> Result<(), ProgramError> {
    let (_, credit_bump) = authenticate_rent_credit(program_id, credit, persisted_payer)?;
    let source_before = source.lamports();
    let credit_before = credit.lamports();
    let credit_data = credit
        .try_borrow_data()
        .map_err(|_| AdapterError::DirectAuthentication)?;
    let plan = terminal_rent_credit_close_plan_v1(
        *persisted_payer,
        &credit_data,
        credit_bump,
        source_before,
        rent.minimum_balance(source.data_len()),
        credit_before,
    )
    .map_err(|_| AdapterError::DirectAuthentication)?;
    drop(credit_data);
    let instruction =
        close_token_instruction(token_program.key, source.key, credit.key, authority.key)?;
    invoke_signed(
        &instruction,
        &[
            source.clone(),
            credit.clone(),
            authority.clone(),
            token_program.clone(),
        ],
        &[signer_seeds],
    )
    .map_err(|_| AdapterError::DirectClose)?;
    plan.source_close()
        .validate_post(source.lamports(), credit.lamports())
        .map_err(|_| AdapterError::DirectPostcondition)?;
    let source_data = source
        .try_borrow_data()
        .map_err(|_| AdapterError::DirectPostcondition)?;
    if source_data.iter().any(|byte| *byte != 0) {
        return Err(AdapterError::DirectPostcondition.into());
    }
    Ok(())
}

fn close_program_to_credit(
    program_id: &Pubkey,
    source: &AccountInfo<'_>,
    credit: &AccountInfo<'_>,
    rent: &Rent,
    persisted_payer: &[u8; 32],
) -> Result<(), ProgramError> {
    if source.owner != program_id {
        return Err(AdapterError::DirectAuthentication.into());
    }
    let (_, credit_bump) = authenticate_rent_credit(program_id, credit, persisted_payer)?;
    let source_before = source.lamports();
    let credit_before = credit.lamports();
    let credit_data = credit
        .try_borrow_data()
        .map_err(|_| AdapterError::DirectAuthentication)?;
    let plan = terminal_rent_credit_close_plan_v1(
        *persisted_payer,
        &credit_data,
        credit_bump,
        source_before,
        rent.minimum_balance(source.data_len()),
        credit_before,
    )
    .map_err(|_| AdapterError::DirectAuthentication)?;
    drop(credit_data);
    {
        let mut credit_lamports = credit
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::DirectClose)?;
        let mut source_lamports = source
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::DirectClose)?;
        **credit_lamports = plan.source_close().credit_after();
        **source_lamports = 0;
    }
    source.resize(0).map_err(|_| AdapterError::DirectClose)?;
    source.assign(&system_program::ID);
    plan.source_close()
        .validate_post(source.lamports(), credit.lamports())
        .map_err(|_| AdapterError::DirectPostcondition)?;
    if source.owner != &system_program::ID || !source.data_is_empty() {
        return Err(AdapterError::DirectPostcondition.into());
    }
    Ok(())
}

fn persist_root(account: &AccountInfo<'_>, root: &MakerReplayRootV2) -> Result<(), ProgramError> {
    let mut bytes = [0; MAKER_REPLAY_ROOT_BYTES_V2];
    root.encode(&mut bytes)
        .map_err(|_| AdapterError::DirectPostcondition)?;
    {
        let mut data = account
            .try_borrow_mut_data()
            .map_err(|_| AdapterError::DirectPostcondition)?;
        if data.len() != bytes.len() {
            return Err(AdapterError::DirectPostcondition.into());
        }
        data.copy_from_slice(&bytes);
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::DirectPostcondition)?;
    if MakerReplayRootV2::decode(&data) != Ok(*root) || bytes.as_slice() != &data[..] {
        return Err(AdapterError::DirectPostcondition.into());
    }
    Ok(())
}

fn persist_record(
    account: &AccountInfo<'_>,
    record: &DirectIntentRecordV2,
) -> Result<(), ProgramError> {
    let mut bytes = [0; DIRECT_INTENT_RECORD_BYTES_V2];
    record
        .encode(&mut bytes)
        .map_err(|_| AdapterError::DirectPostcondition)?;
    {
        let mut data = account
            .try_borrow_mut_data()
            .map_err(|_| AdapterError::DirectPostcondition)?;
        if data.len() != bytes.len() {
            return Err(AdapterError::DirectPostcondition.into());
        }
        data.copy_from_slice(&bytes);
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::DirectPostcondition)?;
    if DirectIntentRecordV2::decode(&data) != Ok(*record) || bytes.as_slice() != &data[..] {
        return Err(AdapterError::DirectPostcondition.into());
    }
    Ok(())
}

fn persist_position(
    account: &AccountInfo<'_>,
    position: &DirectPositionV2,
) -> Result<(), ProgramError> {
    {
        let mut data = account
            .try_borrow_mut_data()
            .map_err(|_| AdapterError::DirectPostcondition)?;
        position
            .encode_into(&mut data)
            .map_err(|_| AdapterError::DirectPostcondition)?;
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::DirectPostcondition)?;
    if DirectPositionV2::decode(&data) != Ok(*position) {
        return Err(AdapterError::DirectPostcondition.into());
    }
    Ok(())
}

fn persist_market(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    bytes: &[u8],
) -> Result<(), ProgramError> {
    {
        let mut data = account
            .try_borrow_mut_data()
            .map_err(|_| AdapterError::DirectPostcondition)?;
        if data.len() != bytes.len() {
            return Err(AdapterError::DirectPostcondition.into());
        }
        data.copy_from_slice(bytes);
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::DirectPostcondition)?;
    if bytes != &data[..] {
        return Err(AdapterError::DirectPostcondition.into());
    }
    drop(data);
    authenticate_market(program_id, account).map_err(|_| AdapterError::DirectPostcondition)?;
    Ok(())
}

#[derive(Clone, Copy)]
enum MarketOperation {
    RegisterChildren {
        generation: u64,
        expected_prior_count: u64,
        count: u8,
    },
    RetireChild {
        generation: u64,
        expected_prior_count: u64,
    },
    Split {
        quantity: u64,
    },
    Merge {
        quantity: u64,
    },
    InlineComplementary {
        generation: u64,
        expected_prior_count: u64,
        created: u8,
        side: Side,
        quantity: u64,
    },
}

#[inline(never)]
fn mutate_market(
    account: &AccountInfo<'_>,
    outcome_count: u8,
    operation: MarketOperation,
) -> Result<Vec<u8>, ProgramError> {
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::DirectAuthentication)?;
    match outcome_count {
        2 => mutate_market_width::<2>(&data, operation),
        3 => mutate_market_width::<3>(&data, operation),
        4 => mutate_market_width::<4>(&data, operation),
        5 => mutate_market_width::<5>(&data, operation),
        6 => mutate_market_width::<6>(&data, operation),
        7 => mutate_market_width::<7>(&data, operation),
        8 => mutate_market_width::<8>(&data, operation),
        9 => mutate_market_width::<9>(&data, operation),
        10 => mutate_market_width::<10>(&data, operation),
        11 => mutate_market_width::<11>(&data, operation),
        12 => mutate_market_width::<12>(&data, operation),
        13 => mutate_market_width::<13>(&data, operation),
        14 => mutate_market_width::<14>(&data, operation),
        15 => mutate_market_width::<15>(&data, operation),
        16 => mutate_market_width::<16>(&data, operation),
        _ => Err(AdapterError::DirectAuthentication.into()),
    }
}

#[inline(never)]
fn mutate_market_width<const N: usize>(
    data: &[u8],
    operation: MarketOperation,
) -> Result<Vec<u8>, ProgramError> {
    let mut market =
        CategoricalMarketV1::<N>::decode(data).map_err(|_| AdapterError::DirectAuthentication)?;
    match operation {
        MarketOperation::RegisterChildren {
            generation,
            expected_prior_count,
            count,
        } => {
            let mut index = 0u8;
            while index < count {
                let prior = expected_prior_count
                    .checked_add(u64::from(index))
                    .ok_or(AdapterError::Arithmetic)?;
                market
                    .register_child(generation, prior)
                    .map_err(|_| AdapterError::DirectTransition)?;
                index = index.checked_add(1).ok_or(AdapterError::Arithmetic)?;
            }
        }
        MarketOperation::RetireChild {
            generation,
            expected_prior_count,
        } => market
            .retire_child(generation, expected_prior_count)
            .map_err(|_| AdapterError::DirectTransition)?,
        MarketOperation::Split { quantity } => market
            .split_complete_set(quantity)
            .map_err(|_| AdapterError::DirectTransition)?,
        MarketOperation::Merge { quantity } => market
            .merge_complete_set(quantity)
            .map_err(|_| AdapterError::DirectTransition)?,
        MarketOperation::InlineComplementary {
            generation,
            expected_prior_count,
            created,
            side,
            quantity,
        } => {
            let mut index = 0u8;
            while index < created {
                let prior = expected_prior_count
                    .checked_add(u64::from(index))
                    .ok_or(AdapterError::Arithmetic)?;
                market
                    .register_child(generation, prior)
                    .map_err(|_| AdapterError::DirectTransition)?;
                index = index.checked_add(1).ok_or(AdapterError::Arithmetic)?;
            }
            match side {
                Side::Buy => market.split_complete_set(quantity),
                Side::Sell => market.merge_complete_set(quantity),
            }
            .map_err(|_| AdapterError::DirectTransition)?;
        }
    }
    encode_market(&market)
}

// Action processors follow. Keeping authentication and execution in one file
// makes the currently unverified SVM boundary explicit and reviewable.

fn process_close_registration(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    data: &[u8],
    outcome_count: u8,
) -> Result<(), ProgramError> {
    adapter::decode_close_replay_registration_instruction_v2(data)
        .map_err(|_| AdapterError::InvalidInstruction)?;
    let market_account = account(accounts, 0)?;
    let root_account = account(accounts, 1)?;
    let market = authenticate_market(program_id, market_account)?;
    if market.outcome_count != outcome_count {
        return Err(AdapterError::DirectAuthentication.into());
    }
    let root = authenticate_stored_root(program_id, root_account)?;
    if root.market() != market_account.key.as_ref()
        || root.generation() != market.root.identity().generation()
    {
        return Err(AdapterError::DirectAuthentication.into());
    }
    let next = close_replay_registration_v2(root, map_phase(market.root.phase()))
        .map_err(|_| AdapterError::DirectTransition)?;
    let market_before = snapshot_data(market_account)?;
    let market_lamports = market_account.lamports();
    let root_lamports = root_account.lamports();
    preflight_mutable(&[market_account, root_account])?;
    persist_root(root_account, &next)?;
    if snapshot_data(market_account)? != market_before
        || market_account.lamports() != market_lamports
        || root_account.lamports() != root_lamports
    {
        return Err(AdapterError::DirectPostcondition.into());
    }
    Ok(())
}

fn process_cancel_through(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    data: &[u8],
) -> Result<(), ProgramError> {
    let message = adapter::decode_cancel_through_instruction_v1(data)
        .map_err(|_| AdapterError::InvalidInstruction)?;
    let market_account = account(accounts, 0)?;
    let root_account = account(accounts, 1)?;
    let instructions = account(accounts, 2)?;
    let root = authenticate_stored_root(program_id, root_account)?;
    let market_data = market_account
        .try_borrow_data()
        .map_err(|_| AdapterError::DirectAuthentication)?;
    let outcome_count = decode_market_outcome_count(&market_data)
        .map_err(|_| AdapterError::DirectAuthentication)?;
    drop(market_data);
    let market = authenticate_market(program_id, market_account)?;
    if market.outcome_count != outcome_count {
        return Err(AdapterError::DirectAuthentication.into());
    }
    let phase = map_phase(market.root.phase());
    let generation = market.root.identity().generation();
    if root.market() != market_account.key.as_ref() || root.generation() != generation {
        return Err(AdapterError::DirectAuthentication.into());
    }
    let signed = message.signed_preimage();
    let auth = message_authorization(
        program_id,
        accounts,
        data,
        instructions,
        *message.maker(),
        &signed,
    )?;
    let next = cancel_through_v1(root, message, auth, phase)
        .map_err(|_| AdapterError::DirectTransition)?;
    let market_before = snapshot_data(market_account)?;
    let market_lamports = market_account.lamports();
    let root_lamports = root_account.lamports();
    preflight_mutable(&[root_account])?;
    persist_root(root_account, &next)?;
    if snapshot_data(market_account)? != market_before
        || market_account.lamports() != market_lamports
        || root_account.lamports() != root_lamports
    {
        return Err(AdapterError::DirectPostcondition.into());
    }
    Ok(())
}

fn process_close_root(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    data: &[u8],
    outcome_count: u8,
) -> Result<(), ProgramError> {
    adapter::decode_close_replay_root_instruction_v2(data)
        .map_err(|_| AdapterError::InvalidInstruction)?;
    let market_account = account(accounts, 0)?;
    let root_account = account(accounts, 1)?;
    let credit = account(accounts, 2)?;
    let system = account(accounts, 3)?;
    let rent_account = account(accounts, 4)?;
    let rent = require_system_and_rent(system, rent_account)?;
    let market = authenticate_market(program_id, market_account)?;
    if market.outcome_count != outcome_count {
        return Err(AdapterError::DirectAuthentication.into());
    }
    let root = authenticate_stored_root(program_id, root_account)?;
    if root.market() != market_account.key.as_ref()
        || root.generation() != market.root.identity().generation()
    {
        return Err(AdapterError::DirectAuthentication.into());
    }
    let closure = prepare_replay_root_close_v2(root, map_phase(market.root.phase()))
        .map_err(|_| AdapterError::DirectTransition)?;
    authenticate_rent_credit(program_id, credit, &closure.rent_refund_payer)?;
    let market_after = mutate_market(
        market_account,
        outcome_count,
        MarketOperation::RetireChild {
            generation: root.generation(),
            expected_prior_count: market.root.outstanding_children(),
        },
    )?;
    let credit_before = credit.lamports();
    let root_before = root_account.lamports();
    let expected_credit = credit_before
        .checked_add(root_before)
        .ok_or(AdapterError::Arithmetic)?;
    preflight_mutable(&[market_account, root_account, credit])?;
    persist_market(program_id, market_account, &market_after)?;
    close_program_to_credit(
        program_id,
        root_account,
        credit,
        &rent,
        &closure.rent_refund_payer,
    )?;
    if credit.lamports() != expected_credit {
        return Err(AdapterError::DirectPostcondition.into());
    }
    Ok(())
}

fn snapshot_data(account: &AccountInfo<'_>) -> Result<Vec<u8>, ProgramError> {
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::DirectAuthentication)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(data.len())
        .map_err(|_| AdapterError::Arithmetic)?;
    output.extend_from_slice(&data);
    Ok(output)
}

fn process_register(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    data: &[u8],
    outcome_count: u8,
) -> Result<(), ProgramError> {
    let mut intents = Vec::new();
    intents
        .try_reserve_exact(1)
        .map_err(|_| AdapterError::Arithmetic)?;
    intents.push(
        adapter::decode_register_instruction_v2(data)
            .map_err(|_| AdapterError::InvalidInstruction)?,
    );
    let intent = intents.first().ok_or(AdapterError::Arithmetic)?;
    let payer = account(accounts, 0)?;
    let credit = account(accounts, 1)?;
    let market_account = account(accounts, 2)?;
    if market_account.key.to_bytes() != *intent.market()
        || payer.owner != &system_program::ID
        || !payer.data_is_empty()
    {
        return Err(AdapterError::DirectAuthentication.into());
    }
    let market = authenticate_market(program_id, market_account)?;
    if market.outcome_count != outcome_count
        || market.root.identity().generation() != intent.generation()
    {
        return Err(AdapterError::DirectAuthentication.into());
    }
    let (
        policy_index,
        staging_index,
        manifest_index,
        root_index,
        record_index,
        position_index,
        system_index,
        rent_index,
        instructions_index,
    ) = match intent.side() {
        Side::Buy => (4, 5, 6, 7, 8, 10, 14, 15, 16),
        Side::Sell => (3, 4, 5, 6, 7, 8, 9, 10, 11),
    };
    let system = account(accounts, system_index)?;
    let rent_account = account(accounts, rent_index)?;
    let rent = require_system_and_rent(system, rent_account)?;
    let policy = authenticate_policy(
        program_id,
        market.root,
        account(accounts, policy_index)?,
        account(accounts, staging_index)?,
        account(accounts, manifest_index)?,
        rent_account,
    )?;
    authenticate_rent_credit(program_id, credit, &payer.key.to_bytes())?;
    let authorizations = authorization_runtime(
        program_id,
        accounts,
        data,
        account(accounts, instructions_index)?,
        &[(16, intent)],
    )?;
    let authorization = authorizations.first().ok_or(AdapterError::Arithmetic)?;
    let root_account = account(accounts, root_index)?;
    let root = authenticate_root_state(
        program_id,
        root_account,
        market_account.key,
        intent.generation(),
        intent.maker(),
        true,
    )?;
    let record_account = account(accounts, record_index)?;
    let record_bump = authenticate_new_record(program_id, record_account, *intent)?;
    let position_account = account(accounts, position_index)?;
    let mut positions = Vec::new();
    positions
        .try_reserve_exact(1)
        .map_err(|_| AdapterError::Arithmetic)?;
    positions.push(authenticate_position(
        program_id,
        position_account,
        market_account,
        intent.maker(),
        intent.generation(),
        outcome_count,
    )?);

    let (participant, collateral_mint, debit, realm_facts, escrow_account) = match intent.side() {
        Side::Buy => {
            let realm_account = account(accounts, 3)?;
            let escrow = account(accounts, 9)?;
            let source = account(accounts, 11)?;
            let mint = account(accounts, 12)?;
            let token_program = account(accounts, 13)?;
            let realm =
                authenticate_realm(program_id, realm_account, mint, token_program, market.root)?;
            let (expected_escrow, _) = escrow_pda(program_id, record_account.key);
            if escrow.key != &expected_escrow || !is_prefunded_vacant(escrow) {
                return Err(AdapterError::DirectAuthentication.into());
            }
            (
                participant_accounts(
                    root_account,
                    Some(record_account),
                    Some(escrow),
                    position_account,
                    source,
                ),
                Some(mint.key.to_bytes()),
                Some(buy_debit_authority(source, token_program, realm)?),
                Some(realm),
                Some(escrow),
            )
        }
        Side::Sell => (
            ParticipantAccountsV2 {
                replay_root: root_account.key.to_bytes(),
                record: record_account.key.to_bytes(),
                escrow: [0; 32],
                position: position_account.key.to_bytes(),
                collateral: *intent.collateral_account(),
            },
            None,
            None,
            None,
            None,
        ),
    };
    let root_created = root.created;
    let root_bump = root.bump;
    let mut roots = Vec::new();
    roots
        .try_reserve_exact(1)
        .map_err(|_| AdapterError::Arithmetic)?;
    roots.push(root.state);
    let mut records = Vec::new();
    records
        .try_reserve_exact(1)
        .map_err(|_| AdapterError::Arithmetic)?;
    records.push(None);
    let registration = register_intent_runtime_in_place_v2(RuntimeRegistrationInPlaceV2 {
        replay_root: roots.get_mut(0).ok_or(AdapterError::Arithmetic)?,
        intent,
        authorization,
        phase: map_phase(market.root.phase()),
        slot: current_slot()?,
        accounts: &participant,
        system_payer: payer.key.to_bytes(),
        collateral_mint,
        buy_debit_authority: debit.as_ref(),
        record_bump,
        fee_policy: policy.policy,
        fee_config_digest: policy.digest,
        position: positions.get_mut(0).ok_or(AdapterError::Arithmetic)?,
        record: records.get_mut(0).ok_or(AdapterError::Arithmetic)?,
    })
    .map_err(|_| AdapterError::DirectTransition)?;
    let registration_root = match roots.first().ok_or(AdapterError::Arithmetic)? {
        ReplayRootStateV2::Existing(value) => value,
        ReplayRootStateV2::Absent { .. } => return Err(AdapterError::DirectTransition.into()),
    };
    let registration_record = records
        .first()
        .ok_or(AdapterError::Arithmetic)?
        .as_ref()
        .ok_or(AdapterError::DirectTransition)?;

    let root_rent = if root_created {
        rent.minimum_balance(MAKER_REPLAY_ROOT_BYTES_V2)
    } else {
        0
    };
    let record_rent = rent.minimum_balance(DIRECT_INTENT_RECORD_BYTES_V2);
    let escrow_rent = if intent.side() == Side::Buy {
        rent.minimum_balance(ACCOUNT_BYTES)
    } else {
        0
    };
    let root_top_up = if root_created {
        creation_top_up(root_account, root_rent)?
    } else {
        0
    };
    let record_top_up = creation_top_up(record_account, record_rent)?;
    let escrow_top_up = match escrow_account {
        Some(escrow) => creation_top_up(escrow, escrow_rent)?,
        None => 0,
    };
    let total_top_up = root_top_up
        .checked_add(record_top_up)
        .and_then(|value| value.checked_add(escrow_top_up))
        .ok_or(AdapterError::Arithmetic)?;
    let payer_before = payer.lamports();
    let payer_after = payer_before
        .checked_sub(total_top_up)
        .ok_or(AdapterError::DirectAuthentication)?;
    let credit_lamports = credit.lamports();
    let market_after = if root_created {
        Some(mutate_market(
            market_account,
            outcome_count,
            MarketOperation::RegisterChildren {
                generation: intent.generation(),
                expected_prior_count: market.root.outstanding_children(),
                count: 1,
            },
        )?)
    } else {
        None
    };
    let root_bytes = root_seed_parts(
        market_account.key,
        intent.generation(),
        intent.maker(),
        root_bump,
    );
    let record_bytes = OwnedRecordSeeds::new(*intent, record_bump);
    let mut mutable = Vec::new();
    mutable
        .try_reserve_exact(8)
        .map_err(|_| AdapterError::Arithmetic)?;
    mutable.extend_from_slice(&[payer, market_account, root_account, record_account]);
    if intent.side() == Side::Sell {
        mutable.push(position_account);
    }
    if let Some(escrow) = escrow_account {
        mutable.push(escrow);
        mutable.push(account(accounts, 11)?);
    }
    preflight_mutable(&mutable)?;

    if root_created {
        create_pda(
            payer,
            root_account,
            system,
            root_rent,
            MAKER_REPLAY_ROOT_BYTES_V2,
            program_id,
            &root_bytes.refs(),
        )?;
    }
    create_pda(
        payer,
        record_account,
        system,
        record_rent,
        DIRECT_INTENT_RECORD_BYTES_V2,
        program_id,
        &record_bytes.refs(),
    )?;
    if let (Some(realm), Some(escrow)) = (realm_facts, escrow_account) {
        let mint = account(accounts, 12)?;
        let token_program = account(accounts, 13)?;
        let (_, escrow_bump) = escrow_pda(program_id, record_account.key);
        let escrow_seed = EscrowSeeds::new(record_account.key, escrow_bump);
        create_pda(
            payer,
            escrow,
            system,
            escrow_rent,
            ACCOUNT_BYTES,
            token_program.key,
            &escrow_seed.refs(),
        )?;
        initialize_escrow(escrow, mint, token_program, record_account.key)?;
        execute_transfer_signed(
            account(accounts, 11)?,
            escrow,
            mint,
            token_program,
            root_account,
            realm,
            registration.reserved_collateral_debit,
            &root_bytes.refs(),
        )?;
        let escrow_facts = escrow_authority(
            escrow,
            token_program,
            realm,
            registration_record.reserved_collateral(),
        )?;
        if escrow_facts.authority != record_account.key.to_bytes()
            || escrow_facts.token_account != escrow.key.to_bytes()
            || authenticate_token_account(escrow, token_program, realm)?.amount
                != registration_record.reserved_collateral()
        {
            return Err(AdapterError::DirectPostcondition.into());
        }
    }
    if let Some(market_after) = market_after {
        persist_market(program_id, market_account, &market_after)?;
    }
    persist_root(root_account, registration_root)?;
    persist_record(record_account, registration_record)?;
    if intent.side() == Side::Sell {
        persist_position(
            position_account,
            positions.first().ok_or(AdapterError::Arithmetic)?,
        )?;
    }
    if payer.lamports() != payer_after || credit.lamports() != credit_lamports {
        return Err(AdapterError::DirectPostcondition.into());
    }
    Ok(())
}

enum UnwindKind {
    Cancel,
    Expire,
    Invalidated,
}

fn process_cancel(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    data: &[u8],
    outcome_count: u8,
) -> Result<(), ProgramError> {
    process_unwind(
        program_id,
        accounts,
        data,
        UnwindKind::Cancel,
        outcome_count,
    )
}

fn process_expire(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    data: &[u8],
    outcome_count: u8,
) -> Result<(), ProgramError> {
    process_unwind(
        program_id,
        accounts,
        data,
        UnwindKind::Expire,
        outcome_count,
    )
}

fn process_close_invalidated(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    data: &[u8],
    outcome_count: u8,
) -> Result<(), ProgramError> {
    process_unwind(
        program_id,
        accounts,
        data,
        UnwindKind::Invalidated,
        outcome_count,
    )
}

fn process_unwind(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    data: &[u8],
    kind: UnwindKind,
    outcome_count: u8,
) -> Result<(), ProgramError> {
    let record_index = if matches!(
        adapter::decode_adapter_header_v2(data)
            .map_err(|_| AdapterError::InvalidInstruction)?
            .action,
        adapter::AdapterActionV2::CancelBuy
            | adapter::AdapterActionV2::ExpireBuy
            | adapter::AdapterActionV2::CloseInvalidatedBuy
    ) {
        3
    } else {
        2
    };
    let record_account = account(accounts, record_index)?;
    let mut records = Vec::new();
    records
        .try_reserve_exact(1)
        .map_err(|_| AdapterError::Arithmetic)?;
    records.push(authenticate_record(program_id, record_account)?);
    let record = records.first().ok_or(AdapterError::Arithmetic)?;
    let intent = record.intent_ref();
    let side = intent.side();
    match kind {
        UnwindKind::Cancel => adapter::decode_cancel_instruction_v2(data, side)
            .map(|_| ())
            .map_err(|_| AdapterError::InvalidInstruction)?,
        UnwindKind::Expire => adapter::decode_expire_instruction_v2(data, side)
            .map_err(|_| AdapterError::InvalidInstruction)?,
        UnwindKind::Invalidated => adapter::decode_close_invalidated_instruction_v1(data, side)
            .map_err(|_| AdapterError::InvalidInstruction)?,
    }
    let market_account = account(accounts, 0)?;
    let market = authenticate_market(program_id, market_account)?;
    if market_account.key.to_bytes() != *intent.market()
        || market.outcome_count != outcome_count
        || market.root.identity().generation() != intent.generation()
    {
        return Err(AdapterError::DirectAuthentication.into());
    }
    let (root_index, position_index, credit_index, system_index, rent_index, instructions_index) =
        match side {
            Side::Buy => (2, 5, 7, 10, 11, Some(12)),
            Side::Sell => (1, 3, 4, 5, 6, Some(7)),
        };
    let root_account = account(accounts, root_index)?;
    let root = existing_root(
        program_id,
        root_account,
        market_account.key,
        intent.generation(),
        intent.maker(),
    )?;
    let position_account = account(accounts, position_index)?;
    let position = authenticate_position(
        program_id,
        position_account,
        market_account,
        intent.maker(),
        intent.generation(),
        outcome_count,
    )?;
    let credit = account(accounts, credit_index)?;
    let system = account(accounts, system_index)?;
    let rent_account = account(accounts, rent_index)?;
    let rent = require_system_and_rent(system, rent_account)?;
    authenticate_rent_credit(program_id, credit, record.rent_payer())?;
    let (participant, mint_address, escrow_facts, realm_facts) = match side {
        Side::Buy => {
            let realm_account = account(accounts, 1)?;
            let escrow = account(accounts, 4)?;
            let destination = account(accounts, 6)?;
            let mint = account(accounts, 8)?;
            let token_program = account(accounts, 9)?;
            let realm =
                authenticate_realm(program_id, realm_account, mint, token_program, market.root)?;
            let (expected_escrow, _) = escrow_pda(program_id, record_account.key);
            if escrow.key != &expected_escrow
                || destination.key.to_bytes() != *intent.collateral_account()
            {
                return Err(AdapterError::DirectAuthentication.into());
            }
            authenticate_token_account(destination, token_program, realm)?;
            (
                participant_accounts(
                    root_account,
                    Some(record_account),
                    Some(escrow),
                    position_account,
                    destination,
                ),
                Some(mint.key.to_bytes()),
                Some(escrow_authority(
                    escrow,
                    token_program,
                    realm,
                    record.reserved_collateral(),
                )?),
                Some(realm),
            )
        }
        Side::Sell => (
            ParticipantAccountsV2 {
                replay_root: root_account.key.to_bytes(),
                record: record_account.key.to_bytes(),
                escrow: [0; 32],
                position: position_account.key.to_bytes(),
                collateral: *intent.collateral_account(),
            },
            None,
            None,
            None,
        ),
    };
    let phase = map_phase(market.root.phase());
    let mut root_after = root;
    let mut position_after = position;
    let close = match kind {
        UnwindKind::Cancel => {
            let message = adapter::decode_cancel_instruction_v2(data, side)
                .map_err(|_| AdapterError::InvalidInstruction)?;
            let signed = message.signed_preimage();
            let instruction_account = account(
                accounts,
                instructions_index.ok_or(AdapterError::DirectAuthentication)?,
            )?;
            let authorization = message_authorization(
                program_id,
                accounts,
                data,
                instruction_account,
                *intent.maker(),
                &signed,
            )?;
            unwind_intent_runtime_in_place_v2(RuntimeUnwindInPlaceV2 {
                replay_root: &mut root_after,
                record,
                kind: RuntimeUnwindKindV2::Cancel {
                    authorization: &authorization,
                },
                phase,
                accounts: &participant,
                collateral_mint: mint_address,
                escrow_authority: escrow_facts.as_ref(),
                position: &mut position_after,
            })
            .map_err(|_| AdapterError::DirectTransition)?
        }
        UnwindKind::Expire => unwind_intent_runtime_in_place_v2(RuntimeUnwindInPlaceV2 {
            replay_root: &mut root_after,
            record,
            kind: RuntimeUnwindKindV2::Expire {
                slot: current_slot()?,
            },
            phase,
            accounts: &participant,
            collateral_mint: mint_address,
            escrow_authority: escrow_facts.as_ref(),
            position: &mut position_after,
        })
        .map_err(|_| AdapterError::DirectTransition)?,
        UnwindKind::Invalidated => unwind_intent_runtime_in_place_v2(RuntimeUnwindInPlaceV2 {
            replay_root: &mut root_after,
            record,
            kind: RuntimeUnwindKindV2::Invalidated,
            phase,
            accounts: &participant,
            collateral_mint: mint_address,
            escrow_authority: escrow_facts.as_ref(),
            position: &mut position_after,
        })
        .map_err(|_| AdapterError::DirectTransition)?,
    };
    execute_unwind(
        program_id,
        accounts,
        market_account,
        root_account,
        record_account,
        position_account,
        credit,
        &rent,
        intent,
        &root_after,
        &position_after,
        &close,
        realm_facts,
    )
}

#[allow(clippy::too_many_arguments)]
fn execute_unwind<'info>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'info>],
    market_account: &AccountInfo<'info>,
    root_account: &AccountInfo<'info>,
    record_account: &AccountInfo<'info>,
    position_account: &AccountInfo<'info>,
    credit: &AccountInfo<'info>,
    rent: &Rent,
    intent: &DirectIntentV2,
    root_after: &MakerReplayRootV2,
    position_after: &DirectPositionV2,
    close: &LiveRecordCloseV2,
    realm: Option<RealmFacts>,
) -> Result<(), ProgramError> {
    let market_before = snapshot_data(market_account)?;
    let market_lamports = market_account.lamports();
    let root_lamports = root_account.lamports();
    let credit_before = credit.lamports();
    let mut expected_credit = credit_before
        .checked_add(record_account.lamports())
        .ok_or(AdapterError::Arithmetic)?;
    let record_before = authenticate_record(program_id, record_account)?;
    if record_before.intent() != *intent
        || close.collateral_refund != record_before.reserved_collateral()
        || close.claim_refund != record_before.reserved_claims()
    {
        return Err(AdapterError::DirectPostcondition.into());
    }
    let record_seeds = OwnedRecordSeeds::new(*intent, record_before.bump());
    let mut mutable = vec![root_account, record_account, credit];
    if intent.side() == Side::Sell {
        mutable.push(position_account);
    }
    if intent.side() == Side::Buy {
        let escrow = account(accounts, 4)?;
        let destination = account(accounts, 6)?;
        mutable.push(escrow);
        mutable.push(destination);
        expected_credit = expected_credit
            .checked_add(escrow.lamports())
            .ok_or(AdapterError::Arithmetic)?;
    }
    preflight_mutable(&mutable)?;
    if let Some(realm) = realm {
        let escrow = account(accounts, 4)?;
        let destination = account(accounts, 6)?;
        let mint = account(accounts, 8)?;
        let token_program = account(accounts, 9)?;
        let escrow_balance = authenticate_token_account(escrow, token_program, realm)?.amount;
        let unclassified_donation = escrow_balance
            .checked_sub(record_before.reserved_collateral())
            .ok_or(AdapterError::DirectPostcondition)?;
        if escrow_balance
            != close
                .collateral_refund
                .checked_add(unclassified_donation)
                .ok_or(AdapterError::Arithmetic)?
        {
            return Err(AdapterError::DirectPostcondition.into());
        }
        // Any balance above the record-owned refund is an unclassified token
        // donation. It can never become a close blocker or an arbitrary sink:
        // return it only to the collateral account fixed by the signed intent.
        execute_transfer_signed(
            escrow,
            destination,
            mint,
            token_program,
            record_account,
            realm,
            escrow_balance,
            &record_seeds.refs(),
        )?;
        close_token_to_credit(
            program_id,
            escrow,
            credit,
            record_account,
            token_program,
            rent,
            &close.rent_refund_payer,
            &record_seeds.refs(),
        )?;
    }
    persist_root(root_account, root_after)?;
    if intent.side() == Side::Sell {
        persist_position(position_account, position_after)?;
    }
    close_program_to_credit(
        program_id,
        record_account,
        credit,
        rent,
        &close.rent_refund_payer,
    )?;
    if snapshot_data(market_account)? != market_before
        || market_account.lamports() != market_lamports
        || root_account.lamports() != root_lamports
        || credit.lamports() != expected_credit
    {
        return Err(AdapterError::DirectPostcondition.into());
    }
    Ok(())
}

fn process_ordinary(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    data: &[u8],
    outcome_count: u8,
) -> Result<(), ProgramError> {
    let instruction = adapter::decode_ordinary_instruction_v2(data)
        .map_err(|_| AdapterError::InvalidInstruction)?;
    let market_account = account(accounts, 0)?;
    let market = authenticate_market(program_id, market_account)?;
    if market.outcome_count != outcome_count {
        return Err(AdapterError::DirectAuthentication.into());
    }
    let rent = require_system_and_rent(account(accounts, 8)?, account(accounts, 9)?)?;
    let realm = authenticate_realm(
        program_id,
        account(accounts, 1)?,
        account(accounts, 6)?,
        account(accounts, 7)?,
        market.root,
    )?;
    let policy = authenticate_policy(
        program_id,
        market.root,
        account(accounts, 2)?,
        account(accounts, 3)?,
        account(accounts, 4)?,
        account(accounts, 9)?,
    )?;
    let fee_account = account(accounts, 5)?;
    authenticate_token_account(fee_account, account(accounts, 7)?, realm)?;

    let seller_root_account = account(accounts, 10)?;
    let seller_record_account = account(accounts, 11)?;
    let seller_position_account = account(accounts, 12)?;
    let seller_collateral = account(accounts, 13)?;
    let seller_credit = account(accounts, 14)?;
    let buyer_root_account = account(accounts, 15)?;
    let buyer_record_account = account(accounts, 16)?;
    let buyer_escrow = account(accounts, 17)?;
    let buyer_position_account = account(accounts, 18)?;
    let buyer_collateral = account(accounts, 19)?;
    let buyer_credit = account(accounts, 20)?;
    let mut records = Vec::new();
    records
        .try_reserve_exact(2)
        .map_err(|_| AdapterError::Arithmetic)?;
    records.push(authenticate_record(program_id, seller_record_account)?);
    records.push(authenticate_record(program_id, buyer_record_account)?);
    let mut roots = Vec::new();
    roots
        .try_reserve_exact(2)
        .map_err(|_| AdapterError::Arithmetic)?;
    let mut positions = Vec::new();
    positions
        .try_reserve_exact(2)
        .map_err(|_| AdapterError::Arithmetic)?;
    {
        let ask = copied(&records, 0)?.intent();
        let bid = copied(&records, 1)?.intent();
        if market_account.key.to_bytes() != *ask.market()
            || market_account.key.to_bytes() != *bid.market()
            || market.root.identity().generation() != ask.generation()
            || ask.generation() != bid.generation()
            || seller_collateral.key.to_bytes() != *ask.collateral_account()
            || buyer_collateral.key.to_bytes() != *bid.collateral_account()
        {
            return Err(AdapterError::DirectAuthentication.into());
        }
        roots.push(existing_root(
            program_id,
            seller_root_account,
            market_account.key,
            ask.generation(),
            ask.maker(),
        )?);
        roots.push(existing_root(
            program_id,
            buyer_root_account,
            market_account.key,
            bid.generation(),
            bid.maker(),
        )?);
        positions.push(authenticate_position(
            program_id,
            seller_position_account,
            market_account,
            ask.maker(),
            ask.generation(),
            outcome_count,
        )?);
        positions.push(authenticate_position(
            program_id,
            buyer_position_account,
            market_account,
            bid.maker(),
            bid.generation(),
            outcome_count,
        )?);
    }
    authenticate_token_account(seller_collateral, account(accounts, 7)?, realm)?;
    authenticate_token_account(buyer_collateral, account(accounts, 7)?, realm)?;
    let (expected_escrow, _) = escrow_pda(program_id, buyer_record_account.key);
    if buyer_escrow.key != &expected_escrow {
        return Err(AdapterError::DirectAuthentication.into());
    }
    let escrow = escrow_authority(
        buyer_escrow,
        account(accounts, 7)?,
        realm,
        records
            .get(1)
            .ok_or(AdapterError::Arithmetic)?
            .reserved_collateral(),
    )?;
    let buyer_escrow_donation = escrow_donation(
        buyer_escrow,
        account(accounts, 7)?,
        realm,
        records
            .get(1)
            .ok_or(AdapterError::Arithmetic)?
            .reserved_collateral(),
    )?;
    authenticate_rent_credit(
        program_id,
        seller_credit,
        records
            .first()
            .ok_or(AdapterError::Arithmetic)?
            .rent_payer(),
    )?;
    authenticate_rent_credit(
        program_id,
        buyer_credit,
        records.get(1).ok_or(AdapterError::Arithmetic)?.rent_payer(),
    )?;
    let seller_accounts = participant_accounts(
        seller_root_account,
        Some(seller_record_account),
        None,
        seller_position_account,
        seller_collateral,
    );
    let buyer_accounts = participant_accounts(
        buyer_root_account,
        Some(buyer_record_account),
        Some(buyer_escrow),
        buyer_position_account,
        buyer_collateral,
    );
    let mut seller_close = None;
    let mut buyer_close = None;
    let settlement = {
        let (seller_roots, buyer_roots) = roots.split_at_mut(1);
        let seller_root = seller_roots.get_mut(0).ok_or(AdapterError::Arithmetic)?;
        let buyer_root = buyer_roots.get_mut(0).ok_or(AdapterError::Arithmetic)?;
        let (seller_records, buyer_records) = records.split_at_mut(1);
        let seller_record = seller_records.get_mut(0).ok_or(AdapterError::Arithmetic)?;
        let buyer_record = buyer_records.get_mut(0).ok_or(AdapterError::Arithmetic)?;
        let (seller_positions, buyer_positions) = positions.split_at_mut(1);
        let seller_position = seller_positions.first().ok_or(AdapterError::Arithmetic)?;
        let buyer_position = buyer_positions.get_mut(0).ok_or(AdapterError::Arithmetic)?;
        settle_ordinary_runtime_in_place_v2(RuntimeOrdinaryMatchInPlaceV2 {
            phase: map_phase(market.root.phase()),
            slot: current_slot()?,
            seller_replay_root: seller_root,
            buyer_replay_root: buyer_root,
            seller_record,
            buyer_record,
            seller_close: &mut seller_close,
            buyer_close: &mut buyer_close,
            seller_accounts: &seller_accounts,
            buyer_accounts: &buyer_accounts,
            seller_position,
            buyer_position,
            collateral_mint: account(accounts, 6)?.key.to_bytes(),
            buyer_escrow_authority: &escrow,
            fill: instruction.fill,
            execution_price: instruction.execution_price,
            fee_policy: policy.policy,
            fee_config_digest: policy.digest,
            fee_recipient_account: fee_account.key.to_bytes(),
        })
        .map_err(|_| AdapterError::DirectTransition)?
    };
    let market_before = snapshot_data(market_account)?;
    let market_lamports = market_account.lamports();
    let seller_root_lamports = seller_root_account.lamports();
    let buyer_root_lamports = buyer_root_account.lamports();
    let mint = account(accounts, 6)?;
    let token_program = account(accounts, 7)?;
    let buyer_record = records.get(1).ok_or(AdapterError::Arithmetic)?;
    let buyer_seeds = OwnedRecordSeeds::new(buyer_record.intent(), buyer_record.bump());
    preflight_mutable(&[
        seller_root_account,
        seller_record_account,
        seller_collateral,
        seller_credit,
        buyer_root_account,
        buyer_record_account,
        buyer_escrow,
        buyer_position_account,
        buyer_collateral,
        buyer_credit,
        fee_account,
    ])?;
    execute_transfer_signed(
        buyer_escrow,
        seller_collateral,
        mint,
        token_program,
        buyer_record_account,
        realm,
        settlement.seller_collateral_credit,
        &buyer_seeds.refs(),
    )?;
    execute_transfer_signed(
        buyer_escrow,
        fee_account,
        mint,
        token_program,
        buyer_record_account,
        realm,
        settlement.venue_fee_transfer,
        &buyer_seeds.refs(),
    )?;
    persist_root(
        seller_root_account,
        roots.first().ok_or(AdapterError::Arithmetic)?,
    )?;
    persist_root(
        buyer_root_account,
        roots.get(1).ok_or(AdapterError::Arithmetic)?,
    )?;
    persist_position(
        buyer_position_account,
        positions.get(1).ok_or(AdapterError::Arithmetic)?,
    )?;
    finish_record(
        program_id,
        seller_record_account,
        seller_credit,
        &rent,
        if seller_close.is_none() {
            Some(records.first().ok_or(AdapterError::Arithmetic)?)
        } else {
            None
        },
        seller_close.as_ref(),
        None,
    )?;
    finish_record(
        program_id,
        buyer_record_account,
        buyer_credit,
        &rent,
        if buyer_close.is_none() {
            Some(records.get(1).ok_or(AdapterError::Arithmetic)?)
        } else {
            None
        },
        buyer_close.as_ref(),
        Some(BuyCloseAccounts {
            escrow: buyer_escrow,
            destination: buyer_collateral,
            mint,
            token_program,
            realm,
            expected_donation: buyer_escrow_donation,
        }),
    )?;
    if snapshot_data(market_account)? != market_before
        || market_account.lamports() != market_lamports
        || seller_root_account.lamports() != seller_root_lamports
        || buyer_root_account.lamports() != buyer_root_lamports
    {
        return Err(AdapterError::DirectPostcondition.into());
    }
    Ok(())
}

struct BuyCloseAccounts<'a, 'info> {
    escrow: &'a AccountInfo<'info>,
    destination: &'a AccountInfo<'info>,
    mint: &'a AccountInfo<'info>,
    token_program: &'a AccountInfo<'info>,
    realm: RealmFacts,
    expected_donation: u64,
}

fn finish_record<'info>(
    program_id: &Pubkey,
    record_account: &AccountInfo<'info>,
    credit: &AccountInfo<'info>,
    rent: &Rent,
    live_record: Option<&DirectIntentRecordV2>,
    close: Option<&LiveRecordCloseV2>,
    buy: Option<BuyCloseAccounts<'_, 'info>>,
) -> Result<(), ProgramError> {
    if let Some(record) = live_record {
        if close.is_some() {
            return Err(AdapterError::DirectPostcondition.into());
        }
        if let Some(accounts) = buy.as_ref() {
            let escrow = authenticate_token_account(
                accounts.escrow,
                accounts.token_program,
                accounts.realm,
            )?;
            let expected_balance = record
                .reserved_collateral()
                .checked_add(accounts.expected_donation)
                .ok_or(AdapterError::Arithmetic)?;
            if escrow.amount != expected_balance {
                return Err(AdapterError::DirectPostcondition.into());
            }
        }
        return persist_record(record_account, record);
    }
    let close = close.ok_or(AdapterError::DirectPostcondition)?;
    let record = authenticate_record(program_id, record_account)?;
    let intent = record.intent();
    let seeds = OwnedRecordSeeds::new(intent, record.bump());
    if let Some(accounts) = buy {
        let escrow_balance =
            authenticate_token_account(accounts.escrow, accounts.token_program, accounts.realm)?
                .amount;
        let expected_balance = close
            .collateral_refund
            .checked_add(accounts.expected_donation)
            .ok_or(AdapterError::Arithmetic)?;
        if escrow_balance != expected_balance {
            return Err(AdapterError::DirectPostcondition.into());
        }
        // `expected_donation` was measured before the pure transition and all
        // CPIs. The exact post-transfer equality proves a partial fill neither
        // consumes nor manufactures that surplus; a terminal fill returns it
        // only to the collateral destination fixed by the signed intent.
        execute_transfer_signed(
            accounts.escrow,
            accounts.destination,
            accounts.mint,
            accounts.token_program,
            record_account,
            accounts.realm,
            escrow_balance,
            &seeds.refs(),
        )?;
        close_token_to_credit(
            program_id,
            accounts.escrow,
            credit,
            record_account,
            accounts.token_program,
            rent,
            &close.rent_refund_payer,
            &seeds.refs(),
        )?;
    } else if close.claim_refund != 0 || close.collateral_refund != 0 {
        return Err(AdapterError::DirectPostcondition.into());
    }
    close_program_to_credit(
        program_id,
        record_account,
        credit,
        rent,
        &close.rent_refund_payer,
    )
}

fn process_complementary(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    data: &[u8],
    action: adapter::AdapterActionV2,
    outcome_count: u8,
) -> Result<(), ProgramError> {
    let instruction =
        adapter::decode_complementary_instruction_view_v2(data, action, outcome_count)
            .map_err(|_| AdapterError::InvalidInstruction)?;
    let count = usize::from(outcome_count);
    let mut execution_prices = vec![0u64; count];
    for (index, price) in execution_prices.iter_mut().enumerate() {
        *price = instruction
            .execution_price(index)
            .map_err(|_| AdapterError::InvalidInstruction)?;
    }
    let market_account = account(accounts, 0)?;
    let market = authenticate_market(program_id, market_account)?;
    if market.outcome_count != outcome_count {
        return Err(AdapterError::DirectAuthentication.into());
    }
    let rent = require_system_and_rent(account(accounts, 10)?, account(accounts, 11)?)?;
    let realm = authenticate_realm(
        program_id,
        account(accounts, 1)?,
        account(accounts, 8)?,
        account(accounts, 9)?,
        market.root,
    )?;
    let policy = authenticate_policy(
        program_id,
        market.root,
        account(accounts, 2)?,
        account(accounts, 3)?,
        account(accounts, 4)?,
        account(accounts, 11)?,
    )?;
    let vault = account(accounts, 5)?;
    authenticate_custody(
        program_id,
        market_account,
        account(accounts, 6)?,
        market.root.identity().generation(),
    )?;
    let vault_facts = authenticate_vault(
        program_id,
        market_account,
        vault,
        account(accounts, 8)?,
        account(accounts, 9)?,
        realm,
    )?;
    if vault_facts.amount < market.hoard_atoms {
        return Err(AdapterError::DirectAuthentication.into());
    }
    let fee_account = account(accounts, 7)?;
    authenticate_token_account(fee_account, account(accounts, 9)?, realm)?;
    match action {
        adapter::AdapterActionV2::Split => process_split(
            program_id,
            accounts,
            instruction.fill(),
            &execution_prices,
            market_account,
            market,
            realm,
            policy,
            vault,
            fee_account,
            &rent,
        ),
        adapter::AdapterActionV2::Merge => process_merge(
            program_id,
            accounts,
            instruction.fill(),
            &execution_prices,
            market_account,
            market,
            realm,
            policy,
            vault,
            fee_account,
            &rent,
        ),
        _ => Err(AdapterError::InvalidInstruction.into()),
    }
}

#[allow(clippy::too_many_arguments)]
fn process_split<'info>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'info>],
    fill: u64,
    execution_prices: &[u64],
    market_account: &AccountInfo<'info>,
    market: MarketFacts,
    realm: RealmFacts,
    policy: PolicyFacts,
    vault: &AccountInfo<'info>,
    fee_account: &AccountInfo<'info>,
    rent: &Rent,
) -> Result<(), ProgramError> {
    let first = SPLIT_BASE;
    let seed_record = authenticate_record(program_id, account(accounts, first + 1)?)?;
    let seed_intent = seed_record.intent();
    if seed_intent.market() != market_account.key.as_ref()
        || seed_intent.generation() != market.root.identity().generation()
    {
        return Err(AdapterError::DirectAuthentication.into());
    }
    let seed_root = existing_root(
        program_id,
        account(accounts, first)?,
        market_account.key,
        seed_intent.generation(),
        seed_intent.maker(),
    )?;
    let seed_position = authenticate_position(
        program_id,
        account(accounts, first + 3)?,
        market_account,
        seed_intent.maker(),
        seed_intent.generation(),
        market.outcome_count,
    )?;
    let seed_accounts = participant_accounts(
        account(accounts, first)?,
        Some(account(accounts, first + 1)?),
        Some(account(accounts, first + 2)?),
        account(accounts, first + 3)?,
        account(accounts, first + 4)?,
    );
    let seed_escrow = escrow_authority(
        account(accounts, first + 2)?,
        account(accounts, 9)?,
        realm,
        seed_record.reserved_collateral(),
    )?;
    let count = usize::from(market.outcome_count);
    let mut roots = vec![seed_root; count];
    let mut records = vec![seed_record; count];
    let mut positions = vec![seed_position; count];
    let mut participant_accounts_array = vec![seed_accounts; count];
    let mut escrows = vec![seed_escrow; count];
    let seed_donation = escrow_donation(
        account(accounts, first + 2)?,
        account(accounts, 9)?,
        realm,
        seed_record.reserved_collateral(),
    )?;
    let mut escrow_donations = vec![seed_donation; count];
    let mut record_closes = vec![None; count];
    for index in 0..count {
        let base = SPLIT_BASE
            .checked_add(index.checked_mul(6).ok_or(AdapterError::Arithmetic)?)
            .ok_or(AdapterError::Arithmetic)?;
        let record = authenticate_record(program_id, account(accounts, base + 1)?)?;
        let intent = record.intent();
        replace(&mut records, index, record)?;
        let root = existing_root(
            program_id,
            account(accounts, base)?,
            market_account.key,
            intent.generation(),
            intent.maker(),
        )?;
        replace(&mut roots, index, root)?;
        let position = authenticate_position(
            program_id,
            account(accounts, base + 3)?,
            market_account,
            intent.maker(),
            intent.generation(),
            market.outcome_count,
        )?;
        replace(&mut positions, index, position)?;
        let (expected_escrow, _) = escrow_pda(program_id, account(accounts, base + 1)?.key);
        if account(accounts, base + 2)?.key != &expected_escrow
            || account(accounts, base + 4)?.key.to_bytes() != *intent.collateral_account()
        {
            return Err(AdapterError::DirectAuthentication.into());
        }
        authenticate_token_account(account(accounts, base + 4)?, account(accounts, 9)?, realm)?;
        authenticate_rent_credit(
            program_id,
            account(accounts, base + 5)?,
            record.rent_payer(),
        )?;
        let participant = participant_accounts(
            account(accounts, base)?,
            Some(account(accounts, base + 1)?),
            Some(account(accounts, base + 2)?),
            account(accounts, base + 3)?,
            account(accounts, base + 4)?,
        );
        replace(&mut participant_accounts_array, index, participant)?;
        let escrow = escrow_authority(
            account(accounts, base + 2)?,
            account(accounts, 9)?,
            realm,
            record.reserved_collateral(),
        )?;
        replace(&mut escrows, index, escrow)?;
        let donation = escrow_donation(
            account(accounts, base + 2)?,
            account(accounts, 9)?,
            realm,
            record.reserved_collateral(),
        )?;
        replace(&mut escrow_donations, index, donation)?;
    }
    let mut gross_debits = vec![0u64; count];
    let mut fee_debits = vec![0u64; count];
    let settlement = settle_split_runtime_in_place_v2(RuntimeComplementaryBuyMatchInPlaceV2 {
        phase: map_phase(market.root.phase()),
        slot: current_slot()?,
        outcome_count: market.outcome_count,
        buyer_replay_roots: &mut roots,
        buyer_records: &mut records,
        buyer_accounts: &participant_accounts_array,
        buyer_positions: &mut positions,
        collateral_mint: account(accounts, 8)?.key.to_bytes(),
        escrow_authorities: &escrows,
        record_closes: &mut record_closes,
        fill,
        execution_prices,
        gross_debits: &mut gross_debits,
        fee_debits: &mut fee_debits,
        fee_policy: policy.policy,
        fee_config_digest: policy.digest,
        fee_recipient_account: fee_account.key.to_bytes(),
    })
    .map_err(|_| AdapterError::DirectTransition)?;
    let market_after = mutate_market(
        market_account,
        market.outcome_count,
        MarketOperation::Split {
            quantity: settlement.market_vault_collateral_credit,
        },
    )?;
    let market_lamports = market_account.lamports();
    let mut mutable = vec![market_account, vault, fee_account];
    for index in 0..count {
        let base = SPLIT_BASE + index * 6;
        mutable.extend_from_slice(&[
            account(accounts, base)?,
            account(accounts, base + 1)?,
            account(accounts, base + 2)?,
            account(accounts, base + 3)?,
            account(accounts, base + 4)?,
            account(accounts, base + 5)?,
        ]);
    }
    preflight_mutable(&mutable)?;
    for index in 0..count {
        let base = SPLIT_BASE + index * 6;
        let record_account = account(accounts, base + 1)?;
        let record = copied(&records, index)?;
        let seeds = OwnedRecordSeeds::new(record.intent(), record.bump());
        execute_transfer_signed(
            account(accounts, base + 2)?,
            vault,
            account(accounts, 8)?,
            account(accounts, 9)?,
            record_account,
            realm,
            copied(&gross_debits, index)?,
            &seeds.refs(),
        )?;
        execute_transfer_signed(
            account(accounts, base + 2)?,
            fee_account,
            account(accounts, 8)?,
            account(accounts, 9)?,
            record_account,
            realm,
            copied(&fee_debits, index)?,
            &seeds.refs(),
        )?;
        persist_root(
            account(accounts, base)?,
            roots.get(index).ok_or(AdapterError::Arithmetic)?,
        )?;
        persist_position(
            account(accounts, base + 3)?,
            positions.get(index).ok_or(AdapterError::Arithmetic)?,
        )?;
        let close = record_closes.get(index).ok_or(AdapterError::Arithmetic)?;
        finish_record(
            program_id,
            record_account,
            account(accounts, base + 5)?,
            rent,
            if close.is_none() {
                Some(records.get(index).ok_or(AdapterError::Arithmetic)?)
            } else {
                None
            },
            close.as_ref(),
            Some(BuyCloseAccounts {
                escrow: account(accounts, base + 2)?,
                destination: account(accounts, base + 4)?,
                mint: account(accounts, 8)?,
                token_program: account(accounts, 9)?,
                realm,
                expected_donation: copied(&escrow_donations, index)?,
            }),
        )?;
    }
    persist_market(program_id, market_account, &market_after)?;
    if market_account.lamports() != market_lamports {
        return Err(AdapterError::DirectPostcondition.into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn process_merge<'info>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'info>],
    fill: u64,
    execution_prices: &[u64],
    market_account: &AccountInfo<'info>,
    market: MarketFacts,
    realm: RealmFacts,
    policy: PolicyFacts,
    vault: &AccountInfo<'info>,
    fee_account: &AccountInfo<'info>,
    rent: &Rent,
) -> Result<(), ProgramError> {
    let first = SPLIT_BASE;
    let seed_record = authenticate_record(program_id, account(accounts, first + 1)?)?;
    let seed_intent = seed_record.intent();
    if seed_intent.market() != market_account.key.as_ref()
        || seed_intent.generation() != market.root.identity().generation()
    {
        return Err(AdapterError::DirectAuthentication.into());
    }
    let seed_root = existing_root(
        program_id,
        account(accounts, first)?,
        market_account.key,
        seed_intent.generation(),
        seed_intent.maker(),
    )?;
    let seed_position = authenticate_position(
        program_id,
        account(accounts, first + 2)?,
        market_account,
        seed_intent.maker(),
        seed_intent.generation(),
        market.outcome_count,
    )?;
    let seed_accounts = participant_accounts(
        account(accounts, first)?,
        Some(account(accounts, first + 1)?),
        None,
        account(accounts, first + 2)?,
        account(accounts, first + 3)?,
    );
    let count = usize::from(market.outcome_count);
    let mut roots = vec![seed_root; count];
    let mut records = vec![seed_record; count];
    let mut positions = vec![seed_position; count];
    let mut participant_accounts_array = vec![seed_accounts; count];
    let mut record_closes = vec![None; count];
    for index in 0..count {
        let base = SPLIT_BASE
            .checked_add(index.checked_mul(5).ok_or(AdapterError::Arithmetic)?)
            .ok_or(AdapterError::Arithmetic)?;
        let record = authenticate_record(program_id, account(accounts, base + 1)?)?;
        let intent = record.intent();
        replace(&mut records, index, record)?;
        let root = existing_root(
            program_id,
            account(accounts, base)?,
            market_account.key,
            intent.generation(),
            intent.maker(),
        )?;
        replace(&mut roots, index, root)?;
        let position = authenticate_position(
            program_id,
            account(accounts, base + 2)?,
            market_account,
            intent.maker(),
            intent.generation(),
            market.outcome_count,
        )?;
        replace(&mut positions, index, position)?;
        if account(accounts, base + 3)?.key.to_bytes() != *intent.collateral_account() {
            return Err(AdapterError::DirectAuthentication.into());
        }
        authenticate_token_account(account(accounts, base + 3)?, account(accounts, 9)?, realm)?;
        authenticate_rent_credit(
            program_id,
            account(accounts, base + 4)?,
            record.rent_payer(),
        )?;
        let participant = participant_accounts(
            account(accounts, base)?,
            Some(account(accounts, base + 1)?),
            None,
            account(accounts, base + 2)?,
            account(accounts, base + 3)?,
        );
        replace(&mut participant_accounts_array, index, participant)?;
    }
    let mut gross_credits = vec![0u64; count];
    let mut fee_debits = vec![0u64; count];
    let mut net_credits = vec![0u64; count];
    let settlement = settle_merge_runtime_in_place_v2(RuntimeComplementarySellMatchInPlaceV2 {
        phase: map_phase(market.root.phase()),
        slot: current_slot()?,
        outcome_count: market.outcome_count,
        seller_replay_roots: &mut roots,
        seller_records: &mut records,
        seller_accounts: &participant_accounts_array,
        seller_positions: &positions,
        record_closes: &mut record_closes,
        fill,
        execution_prices,
        gross_credits: &mut gross_credits,
        fee_debits: &mut fee_debits,
        net_credits: &mut net_credits,
        fee_policy: policy.policy,
        fee_config_digest: policy.digest,
        fee_recipient_account: fee_account.key.to_bytes(),
    })
    .map_err(|_| AdapterError::DirectTransition)?;
    let market_after = mutate_market(
        market_account,
        market.outcome_count,
        MarketOperation::Merge {
            quantity: settlement.market_vault_collateral_debit,
        },
    )?;
    let signer = market_signer(program_id, market_account, market.root)?;
    let signer_bump = [signer.bump];
    let signer_seeds = [
        MARKET_SEED,
        signer.digest.as_slice(),
        signer_bump.as_slice(),
    ];
    let market_lamports = market_account.lamports();
    let mut mutable = vec![market_account, vault, fee_account];
    for index in 0..count {
        let base = SPLIT_BASE + index * 5;
        mutable.extend_from_slice(&[
            account(accounts, base)?,
            account(accounts, base + 1)?,
            account(accounts, base + 3)?,
            account(accounts, base + 4)?,
        ]);
    }
    preflight_mutable(&mutable)?;
    for index in 0..count {
        let base = SPLIT_BASE + index * 5;
        execute_transfer_signed(
            vault,
            account(accounts, base + 3)?,
            account(accounts, 8)?,
            account(accounts, 9)?,
            market_account,
            realm,
            copied(&net_credits, index)?,
            &signer_seeds,
        )?;
    }
    execute_transfer_signed(
        vault,
        fee_account,
        account(accounts, 8)?,
        account(accounts, 9)?,
        market_account,
        realm,
        settlement.venue_fee_transfer,
        &signer_seeds,
    )?;
    for index in 0..count {
        let base = SPLIT_BASE + index * 5;
        persist_root(
            account(accounts, base)?,
            roots.get(index).ok_or(AdapterError::Arithmetic)?,
        )?;
        let close = record_closes.get(index).ok_or(AdapterError::Arithmetic)?;
        finish_record(
            program_id,
            account(accounts, base + 1)?,
            account(accounts, base + 4)?,
            rent,
            if close.is_none() {
                Some(records.get(index).ok_or(AdapterError::Arithmetic)?)
            } else {
                None
            },
            close.as_ref(),
            None,
        )?;
    }
    persist_market(program_id, market_account, &market_after)?;
    if market_account.lamports() != market_lamports {
        return Err(AdapterError::DirectPostcondition.into());
    }
    Ok(())
}

fn process_inline_ordinary(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    data: &[u8],
    outcome_count: u8,
) -> Result<(), ProgramError> {
    let instruction = adapter::decode_inline_ordinary_instruction_view_v2(data)
        .map_err(|_| AdapterError::InvalidInstruction)?;
    let mut intents = Vec::new();
    intents
        .try_reserve_exact(2)
        .map_err(|_| AdapterError::Arithmetic)?;
    intents.push(
        instruction
            .seller_intent()
            .map_err(|_| AdapterError::InvalidInstruction)?,
    );
    intents.push(
        instruction
            .buyer_intent()
            .map_err(|_| AdapterError::InvalidInstruction)?,
    );
    let payer = account(accounts, 0)?;
    let credit = account(accounts, 1)?;
    let market_account = account(accounts, 2)?;
    let market = authenticate_market(program_id, market_account)?;
    if market.outcome_count != outcome_count {
        return Err(AdapterError::DirectAuthentication.into());
    }
    let rent = require_system_and_rent(account(accounts, 10)?, account(accounts, 11)?)?;
    if payer.owner != &system_program::ID || !payer.data_is_empty() {
        return Err(AdapterError::DirectAuthentication.into());
    }
    authenticate_rent_credit(program_id, credit, &payer.key.to_bytes())?;
    let realm = authenticate_realm(
        program_id,
        account(accounts, 3)?,
        account(accounts, 8)?,
        account(accounts, 9)?,
        market.root,
    )?;
    let policy = authenticate_policy(
        program_id,
        market.root,
        account(accounts, 4)?,
        account(accounts, 5)?,
        account(accounts, 6)?,
        account(accounts, 11)?,
    )?;
    let fee_account = account(accounts, 7)?;
    authenticate_token_account(fee_account, account(accounts, 9)?, realm)?;
    let seller_root_account = account(accounts, INLINE_ORDINARY_BASE)?;
    let seller_position_account = account(accounts, INLINE_ORDINARY_BASE + 1)?;
    let seller_collateral = account(accounts, INLINE_ORDINARY_BASE + 2)?;
    let buyer_root_account = account(accounts, INLINE_ORDINARY_BASE + 3)?;
    let buyer_position_account = account(accounts, INLINE_ORDINARY_BASE + 4)?;
    let buyer_collateral = account(accounts, INLINE_ORDINARY_BASE + 5)?;
    let ask = intents.first().ok_or(AdapterError::Arithmetic)?;
    let bid = intents.get(1).ok_or(AdapterError::Arithmetic)?;
    if market_account.key.to_bytes() != *ask.market()
        || market_account.key.to_bytes() != *bid.market()
        || market.root.identity().generation() != ask.generation()
        || ask.generation() != bid.generation()
        || seller_collateral.key.to_bytes() != *ask.collateral_account()
        || buyer_collateral.key.to_bytes() != *bid.collateral_account()
    {
        return Err(AdapterError::DirectAuthentication.into());
    }
    let mut root_facts = Vec::new();
    root_facts
        .try_reserve_exact(2)
        .map_err(|_| AdapterError::Arithmetic)?;
    root_facts.push(authenticate_root_state(
        program_id,
        seller_root_account,
        market_account.key,
        ask.generation(),
        ask.maker(),
        true,
    )?);
    root_facts.push(authenticate_root_state(
        program_id,
        buyer_root_account,
        market_account.key,
        bid.generation(),
        bid.maker(),
        true,
    )?);
    let mut positions = Vec::new();
    positions
        .try_reserve_exact(2)
        .map_err(|_| AdapterError::Arithmetic)?;
    positions.push(authenticate_position(
        program_id,
        seller_position_account,
        market_account,
        ask.maker(),
        ask.generation(),
        outcome_count,
    )?);
    positions.push(authenticate_position(
        program_id,
        buyer_position_account,
        market_account,
        bid.maker(),
        bid.generation(),
        outcome_count,
    )?);
    authenticate_token_account(seller_collateral, account(accounts, 9)?, realm)?;
    let debit = buy_debit_authority(buyer_collateral, account(accounts, 9)?, realm)?;
    let authorizations = authorization_runtime(
        program_id,
        accounts,
        data,
        account(accounts, 12)?,
        &[(34, ask), (266, bid)],
    )?;
    let seller_accounts = inline_accounts(
        seller_root_account,
        seller_position_account,
        seller_collateral,
    );
    let buyer_accounts =
        inline_accounts(buyer_root_account, buyer_position_account, buyer_collateral);
    let mut roots = Vec::new();
    roots
        .try_reserve_exact(2)
        .map_err(|_| AdapterError::Arithmetic)?;
    roots.push(root_facts.first().ok_or(AdapterError::Arithmetic)?.state);
    roots.push(root_facts.get(1).ok_or(AdapterError::Arithmetic)?.state);
    let settlement = {
        let (seller_roots, buyer_roots) = roots.split_at_mut(1);
        let (seller_positions, buyer_positions) = positions.split_at_mut(1);
        settle_inline_ordinary_runtime_in_place_v2(RuntimeInlineOrdinaryMatchInPlaceV2 {
            phase: map_phase(market.root.phase()),
            slot: current_slot()?,
            seller_replay_root: seller_roots.get_mut(0).ok_or(AdapterError::Arithmetic)?,
            buyer_replay_root: buyer_roots.get_mut(0).ok_or(AdapterError::Arithmetic)?,
            root_creation_payer: payer.key.to_bytes(),
            seller_intent: ask,
            buyer_intent: bid,
            seller_authorization: authorizations.first().ok_or(AdapterError::Arithmetic)?,
            buyer_authorization: authorizations.get(1).ok_or(AdapterError::Arithmetic)?,
            seller_accounts: &seller_accounts,
            buyer_accounts: &buyer_accounts,
            seller_position: seller_positions
                .get_mut(0)
                .ok_or(AdapterError::Arithmetic)?,
            buyer_position: buyer_positions.get_mut(0).ok_or(AdapterError::Arithmetic)?,
            collateral_mint: account(accounts, 8)?.key.to_bytes(),
            buyer_debit_authority: &debit,
            fill: instruction.fill(),
            execution_price: instruction.execution_price(),
            fee_policy: policy.policy,
            fee_config_digest: policy.digest,
            fee_recipient_account: fee_account.key.to_bytes(),
        })
        .map_err(|_| AdapterError::DirectTransition)?
    };
    let seller_root_after = match roots.first().ok_or(AdapterError::Arithmetic)? {
        ReplayRootStateV2::Existing(value) => value,
        ReplayRootStateV2::Absent { .. } => return Err(AdapterError::DirectTransition.into()),
    };
    let buyer_root_after = match roots.get(1).ok_or(AdapterError::Arithmetic)? {
        ReplayRootStateV2::Existing(value) => value,
        ReplayRootStateV2::Absent { .. } => return Err(AdapterError::DirectTransition.into()),
    };
    let mut created = 0_u64;
    for root in &root_facts {
        if root.created {
            created = created.checked_add(1).ok_or(AdapterError::Arithmetic)?;
        }
    }
    let market_after = if created == 0 {
        None
    } else {
        Some(mutate_market(
            market_account,
            outcome_count,
            MarketOperation::RegisterChildren {
                generation: ask.generation(),
                expected_prior_count: market.root.outstanding_children(),
                count: u8::try_from(created).map_err(|_| AdapterError::Arithmetic)?,
            },
        )?)
    };
    let root_rent = rent.minimum_balance(MAKER_REPLAY_ROOT_BYTES_V2);
    let seller_top_up = if root_facts.first().ok_or(AdapterError::Arithmetic)?.created {
        creation_top_up(seller_root_account, root_rent)?
    } else {
        0
    };
    let buyer_top_up = if root_facts.get(1).ok_or(AdapterError::Arithmetic)?.created {
        creation_top_up(buyer_root_account, root_rent)?
    } else {
        0
    };
    let total_top_up = seller_top_up
        .checked_add(buyer_top_up)
        .ok_or(AdapterError::Arithmetic)?;
    let payer_after = payer
        .lamports()
        .checked_sub(total_top_up)
        .ok_or(AdapterError::DirectAuthentication)?;
    let credit_lamports = credit.lamports();
    let seller_seeds = root_seed_parts(
        market_account.key,
        ask.generation(),
        ask.maker(),
        root_facts.first().ok_or(AdapterError::Arithmetic)?.bump,
    );
    let buyer_seeds = root_seed_parts(
        market_account.key,
        bid.generation(),
        bid.maker(),
        root_facts.get(1).ok_or(AdapterError::Arithmetic)?.bump,
    );
    preflight_mutable(&[
        payer,
        market_account,
        seller_root_account,
        seller_position_account,
        seller_collateral,
        buyer_root_account,
        buyer_position_account,
        buyer_collateral,
        fee_account,
    ])?;
    if root_facts.first().ok_or(AdapterError::Arithmetic)?.created {
        create_pda(
            payer,
            seller_root_account,
            account(accounts, 10)?,
            root_rent,
            MAKER_REPLAY_ROOT_BYTES_V2,
            program_id,
            &seller_seeds.refs(),
        )?;
    }
    if root_facts.get(1).ok_or(AdapterError::Arithmetic)?.created {
        create_pda(
            payer,
            buyer_root_account,
            account(accounts, 10)?,
            root_rent,
            MAKER_REPLAY_ROOT_BYTES_V2,
            program_id,
            &buyer_seeds.refs(),
        )?;
    }
    execute_transfer_signed(
        buyer_collateral,
        seller_collateral,
        account(accounts, 8)?,
        account(accounts, 9)?,
        buyer_root_account,
        realm,
        settlement.gross_collateral_transfer,
        &buyer_seeds.refs(),
    )?;
    execute_transfer_signed(
        buyer_collateral,
        fee_account,
        account(accounts, 8)?,
        account(accounts, 9)?,
        buyer_root_account,
        realm,
        settlement.venue_fee_transfer,
        &buyer_seeds.refs(),
    )?;
    require_inline_buy_debit_residual(
        buyer_collateral,
        account(accounts, 9)?,
        realm,
        debit,
        settlement.buyer_total_collateral_debit,
    )?;
    if let Some(market_after) = market_after {
        persist_market(program_id, market_account, &market_after)?;
    }
    persist_root(seller_root_account, seller_root_after)?;
    persist_root(buyer_root_account, buyer_root_after)?;
    persist_position(
        seller_position_account,
        positions.first().ok_or(AdapterError::Arithmetic)?,
    )?;
    persist_position(
        buyer_position_account,
        positions.get(1).ok_or(AdapterError::Arithmetic)?,
    )?;
    if payer.lamports() != payer_after || credit.lamports() != credit_lamports {
        return Err(AdapterError::DirectPostcondition.into());
    }
    Ok(())
}

fn process_inline_complementary(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    data: &[u8],
    action: adapter::AdapterActionV2,
) -> Result<(), ProgramError> {
    let instruction = adapter::decode_inline_complementary_instruction_view_v2(data, action)
        .map_err(|_| AdapterError::InvalidInstruction)?;
    let mut intents = Vec::new();
    intents
        .try_reserve_exact(2)
        .map_err(|_| AdapterError::Arithmetic)?;
    intents.push(
        instruction
            .intent(0)
            .map_err(|_| AdapterError::InvalidInstruction)?,
    );
    intents.push(
        instruction
            .intent(1)
            .map_err(|_| AdapterError::InvalidInstruction)?,
    );
    let mut execution_prices = Vec::new();
    execution_prices
        .try_reserve_exact(2)
        .map_err(|_| AdapterError::Arithmetic)?;
    execution_prices.push(
        instruction
            .execution_price(0)
            .map_err(|_| AdapterError::InvalidInstruction)?,
    );
    execution_prices.push(
        instruction
            .execution_price(1)
            .map_err(|_| AdapterError::InvalidInstruction)?,
    );
    let payer = account(accounts, 0)?;
    let credit = account(accounts, 1)?;
    let market_account = account(accounts, 2)?;
    let market = authenticate_market(program_id, market_account)?;
    if market.outcome_count != 2 {
        return Err(AdapterError::DirectAuthentication.into());
    }
    let rent = require_system_and_rent(account(accounts, 12)?, account(accounts, 13)?)?;
    if payer.owner != &system_program::ID || !payer.data_is_empty() {
        return Err(AdapterError::DirectAuthentication.into());
    }
    authenticate_rent_credit(program_id, credit, &payer.key.to_bytes())?;
    let realm = authenticate_realm(
        program_id,
        account(accounts, 3)?,
        account(accounts, 10)?,
        account(accounts, 11)?,
        market.root,
    )?;
    let policy = authenticate_policy(
        program_id,
        market.root,
        account(accounts, 4)?,
        account(accounts, 5)?,
        account(accounts, 6)?,
        account(accounts, 13)?,
    )?;
    let vault = account(accounts, 7)?;
    authenticate_custody(
        program_id,
        market_account,
        account(accounts, 8)?,
        market.root.identity().generation(),
    )?;
    let vault_facts = authenticate_vault(
        program_id,
        market_account,
        vault,
        account(accounts, 10)?,
        account(accounts, 11)?,
        realm,
    )?;
    if vault_facts.amount < market.hoard_atoms {
        return Err(AdapterError::DirectAuthentication.into());
    }
    let fee_account = account(accounts, 9)?;
    authenticate_token_account(fee_account, account(accounts, 11)?, realm)?;
    let mut roots: Vec<ReplayRootStateV2> = Vec::new();
    let mut root_facts: Vec<RootFacts> = Vec::new();
    let mut positions: Vec<DirectPositionV2> = Vec::new();
    let mut participant_accounts_array: Vec<InlineParticipantAccountsV2> = Vec::new();
    let mut debits = Vec::new();
    roots
        .try_reserve_exact(2)
        .map_err(|_| AdapterError::Arithmetic)?;
    root_facts
        .try_reserve_exact(2)
        .map_err(|_| AdapterError::Arithmetic)?;
    positions
        .try_reserve_exact(2)
        .map_err(|_| AdapterError::Arithmetic)?;
    participant_accounts_array
        .try_reserve_exact(2)
        .map_err(|_| AdapterError::Arithmetic)?;
    debits
        .try_reserve_exact(2)
        .map_err(|_| AdapterError::Arithmetic)?;
    for index in 0..2 {
        let base = INLINE_COMPLEMENT_BASE + index * 3;
        let intent = copied(&intents, index)?;
        if market_account.key.to_bytes() != *intent.market()
            || market.root.identity().generation() != intent.generation()
            || account(accounts, base + 2)?.key.to_bytes() != *intent.collateral_account()
        {
            return Err(AdapterError::DirectAuthentication.into());
        }
        let facts = authenticate_root_state(
            program_id,
            account(accounts, base)?,
            market_account.key,
            intent.generation(),
            intent.maker(),
            true,
        )?;
        root_facts.push(facts);
        roots.push(facts.state);
        positions.push(authenticate_position(
            program_id,
            account(accounts, base + 1)?,
            market_account,
            intent.maker(),
            intent.generation(),
            2,
        )?);
        participant_accounts_array.push(inline_accounts(
            account(accounts, base)?,
            account(accounts, base + 1)?,
            account(accounts, base + 2)?,
        ));
        match action {
            adapter::AdapterActionV2::InlineSplit => {
                let debit = buy_debit_authority(
                    account(accounts, base + 2)?,
                    account(accounts, 11)?,
                    realm,
                )?;
                debits.push(Some(debit));
            }
            adapter::AdapterActionV2::InlineMerge => {
                authenticate_token_account(
                    account(accounts, base + 2)?,
                    account(accounts, 11)?,
                    realm,
                )?;
                debits.push(None);
            }
            _ => return Err(AdapterError::InvalidInstruction.into()),
        }
    }
    let authorizations = authorization_runtime(
        program_id,
        accounts,
        data,
        account(accounts, 14)?,
        &[
            (42, intents.first().ok_or(AdapterError::Arithmetic)?),
            (274, intents.get(1).ok_or(AdapterError::Arithmetic)?),
        ],
    )?;
    let side = if action == adapter::AdapterActionV2::InlineSplit {
        Side::Buy
    } else {
        Side::Sell
    };
    let mut gross_collateral = vec![0_u64; 2];
    let mut fees = vec![0_u64; 2];
    let mut net_seller_credits = vec![0_u64; 2];
    let settlement =
        settle_inline_complementary_runtime_in_place_v2(RuntimeInlineComplementaryMatchInPlaceV2 {
            phase: map_phase(market.root.phase()),
            slot: current_slot()?,
            side,
            replay_roots: &mut roots,
            root_creation_payer: payer.key.to_bytes(),
            intents: &intents,
            authorizations: &authorizations,
            accounts: &participant_accounts_array,
            positions: &mut positions,
            collateral_mint: account(accounts, 10)?.key.to_bytes(),
            buy_debit_authorities: &debits,
            fill: instruction.fill(),
            execution_prices: &execution_prices,
            fee_policy: policy.policy,
            fee_config_digest: policy.digest,
            fee_recipient_account: fee_account.key.to_bytes(),
            gross_collateral: &mut gross_collateral,
            fees: &mut fees,
            net_seller_credits: &mut net_seller_credits,
        })
        .map_err(|_| AdapterError::DirectTransition)?;
    let mut created = 0_u64;
    for facts in &root_facts {
        if facts.created {
            created = created.checked_add(1).ok_or(AdapterError::Arithmetic)?;
        }
    }
    let market_after = mutate_market(
        market_account,
        2,
        MarketOperation::InlineComplementary {
            generation: market.root.identity().generation(),
            expected_prior_count: market.root.outstanding_children(),
            created: u8::try_from(created).map_err(|_| AdapterError::Arithmetic)?,
            side,
            quantity: settlement.market_vault_transfer,
        },
    )?;
    let root_rent = rent.minimum_balance(MAKER_REPLAY_ROOT_BYTES_V2);
    let mut total_top_up = 0_u64;
    for (index, facts) in root_facts.iter().enumerate() {
        if facts.created {
            total_top_up = total_top_up
                .checked_add(creation_top_up(
                    account(accounts, INLINE_COMPLEMENT_BASE + index * 3)?,
                    root_rent,
                )?)
                .ok_or(AdapterError::Arithmetic)?;
        }
    }
    let payer_after = payer
        .lamports()
        .checked_sub(total_top_up)
        .ok_or(AdapterError::DirectAuthentication)?;
    let credit_lamports = credit.lamports();
    let mut mutable = vec![payer, market_account, vault, fee_account];
    for index in 0..2 {
        let base = INLINE_COMPLEMENT_BASE + index * 3;
        mutable.extend_from_slice(&[
            account(accounts, base)?,
            account(accounts, base + 1)?,
            account(accounts, base + 2)?,
        ]);
    }
    preflight_mutable(&mutable)?;
    for (index, facts) in root_facts.iter().enumerate() {
        if facts.created {
            let intent = copied(&intents, index)?;
            let seeds = root_seed_parts(
                market_account.key,
                intent.generation(),
                intent.maker(),
                facts.bump,
            );
            create_pda(
                payer,
                account(accounts, INLINE_COMPLEMENT_BASE + index * 3)?,
                account(accounts, 12)?,
                root_rent,
                MAKER_REPLAY_ROOT_BYTES_V2,
                program_id,
                &seeds.refs(),
            )?;
        }
    }
    match side {
        Side::Buy => {
            for (index, facts) in root_facts.iter().enumerate() {
                let base = INLINE_COMPLEMENT_BASE + index * 3;
                let intent = copied(&intents, index)?;
                let seeds = root_seed_parts(
                    market_account.key,
                    intent.generation(),
                    intent.maker(),
                    facts.bump,
                );
                execute_transfer_signed(
                    account(accounts, base + 2)?,
                    vault,
                    account(accounts, 10)?,
                    account(accounts, 11)?,
                    account(accounts, base)?,
                    realm,
                    copied(&gross_collateral, index)?,
                    &seeds.refs(),
                )?;
                execute_transfer_signed(
                    account(accounts, base + 2)?,
                    fee_account,
                    account(accounts, 10)?,
                    account(accounts, 11)?,
                    account(accounts, base)?,
                    realm,
                    copied(&fees, index)?,
                    &seeds.refs(),
                )?;
                let consumed = copied(&gross_collateral, index)?
                    .checked_add(copied(&fees, index)?)
                    .ok_or(AdapterError::Arithmetic)?;
                require_inline_buy_debit_residual(
                    account(accounts, base + 2)?,
                    account(accounts, 11)?,
                    realm,
                    copied(&debits, index)?.ok_or(AdapterError::DirectPostcondition)?,
                    consumed,
                )?;
            }
        }
        Side::Sell => {
            let signer = market_signer(program_id, market_account, market.root)?;
            let bump = [signer.bump];
            let seeds = [MARKET_SEED, signer.digest.as_slice(), bump.as_slice()];
            for (index, net_credit) in net_seller_credits.iter().copied().enumerate() {
                execute_transfer_signed(
                    vault,
                    account(accounts, INLINE_COMPLEMENT_BASE + index * 3 + 2)?,
                    account(accounts, 10)?,
                    account(accounts, 11)?,
                    market_account,
                    realm,
                    net_credit,
                    &seeds,
                )?;
            }
            execute_transfer_signed(
                vault,
                fee_account,
                account(accounts, 10)?,
                account(accounts, 11)?,
                market_account,
                realm,
                settlement.venue_fee_transfer,
                &seeds,
            )?;
        }
    }
    for index in 0..2 {
        let base = INLINE_COMPLEMENT_BASE + index * 3;
        let root = match roots.get(index).ok_or(AdapterError::Arithmetic)? {
            ReplayRootStateV2::Existing(value) => value,
            ReplayRootStateV2::Absent { .. } => {
                return Err(AdapterError::DirectPostcondition.into());
            }
        };
        persist_root(account(accounts, base)?, root)?;
        persist_position(
            account(accounts, base + 1)?,
            positions.get(index).ok_or(AdapterError::Arithmetic)?,
        )?;
    }
    persist_market(program_id, market_account, &market_after)?;
    if payer.lamports() != payer_after || credit.lamports() != credit_lamports {
        return Err(AdapterError::DirectPostcondition.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use alloc::boxed::Box;
    use core::ops::Range;
    use std::{vec, vec::Vec};

    use super::*;

    fn overwrite(data: &mut [u8], range: Range<usize>, source: &[u8]) {
        assert_eq!(data.get(range.clone()).map(<[u8]>::len), Some(source.len()));
        if let Some(destination) = data.get_mut(range) {
            destination.copy_from_slice(source);
        }
    }

    fn fill(data: &mut [u8], range: Range<usize>, value: u8) {
        assert!(data.get(range.clone()).is_some());
        if let Some(destination) = data.get_mut(range) {
            destination.fill(value);
        }
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

    fn close_root_frame() -> [AccountInfo<'static>; 5] {
        [
            test_account(
                Pubkey::new_unique(),
                false,
                true,
                7,
                vec![1],
                Pubkey::new_unique(),
                false,
            ),
            test_account(
                Pubkey::new_unique(),
                false,
                true,
                7,
                vec![2],
                Pubkey::new_unique(),
                false,
            ),
            test_account(
                Pubkey::new_unique(),
                false,
                true,
                7,
                vec![3],
                Pubkey::new_unique(),
                false,
            ),
            test_account(
                system_program::ID,
                false,
                false,
                0,
                vec![],
                native_loader::ID,
                true,
            ),
            test_account(sysvar::rent::ID, false, false, 0, vec![], sysvar::ID, false),
        ]
    }

    #[test]
    fn physical_frame_refuses_alias_privilege_and_executable_substitution_without_writes() {
        let valid = close_root_frame();
        assert_eq!(
            validate_frame(adapter::AdapterActionV2::CloseReplayRoot, 1, &valid),
            Ok(())
        );
        let market_before = valid[0].try_borrow_data().expect("market data").to_vec();
        let market_lamports = valid[0].lamports();

        let mut readonly_market = close_root_frame();
        readonly_market[0] = test_account(
            *readonly_market[0].key,
            false,
            false,
            7,
            vec![1],
            *readonly_market[0].owner,
            false,
        );
        assert_eq!(
            validate_frame(
                adapter::AdapterActionV2::CloseReplayRoot,
                1,
                &readonly_market
            ),
            Err(AdapterError::AccountPrivilege.into())
        );

        let mut executable_market = close_root_frame();
        executable_market[0] = test_account(
            *executable_market[0].key,
            false,
            true,
            7,
            vec![1],
            *executable_market[0].owner,
            true,
        );
        assert_eq!(
            validate_frame(
                adapter::AdapterActionV2::CloseReplayRoot,
                1,
                &executable_market
            ),
            Err(AdapterError::DirectAuthentication.into())
        );

        let mut alias = close_root_frame();
        alias[2] = test_account(
            *alias[1].key,
            false,
            true,
            7,
            vec![3],
            Pubkey::new_unique(),
            false,
        );
        assert_eq!(
            validate_frame(adapter::AdapterActionV2::CloseReplayRoot, 1, &alias),
            Err(AdapterError::AccountPrivilege.into())
        );
        assert_eq!(
            valid[0].try_borrow_data().expect("market data").as_ref(),
            market_before.as_slice()
        );
        assert_eq!(valid[0].lamports(), market_lamports);
    }

    fn serialize_instruction(output: &mut Vec<u8>, instruction: &Instruction) {
        output.extend_from_slice(
            &u16::try_from(instruction.accounts.len())
                .expect("bounded test accounts")
                .to_le_bytes(),
        );
        for meta in &instruction.accounts {
            let flags = u8::from(meta.is_signer) | (u8::from(meta.is_writable) << 1);
            output.push(flags);
            output.extend_from_slice(meta.pubkey.as_ref());
        }
        output.extend_from_slice(instruction.program_id.as_ref());
        output.extend_from_slice(
            &u16::try_from(instruction.data.len())
                .expect("bounded test data")
                .to_le_bytes(),
        );
        output.extend_from_slice(&instruction.data);
    }

    fn instructions_sysvar(previous: Instruction, current: Instruction) -> AccountInfo<'static> {
        let mut first = Vec::new();
        serialize_instruction(&mut first, &previous);
        let mut second = Vec::new();
        serialize_instruction(&mut second, &current);
        let header = 6usize;
        let first_offset = u16::try_from(header).expect("small header");
        let second_offset = u16::try_from(header + first.len()).expect("small fixture");
        let mut data = Vec::new();
        data.extend_from_slice(&2u16.to_le_bytes());
        data.extend_from_slice(&first_offset.to_le_bytes());
        data.extend_from_slice(&second_offset.to_le_bytes());
        data.extend_from_slice(&first);
        data.extend_from_slice(&second);
        data.extend_from_slice(&1u16.to_le_bytes());
        test_account(
            solana_instructions_sysvar::ID,
            false,
            false,
            0,
            data,
            sysvar::ID,
            false,
        )
    }

    fn exact_ed25519_instruction(
        signer: [u8; 32],
        message_offset: u16,
        message_len: u16,
    ) -> Instruction {
        let public_key_offset = 16_u16;
        let signature_offset = 48_u16;
        let mut data = Vec::new();
        data.extend_from_slice(&1_u16.to_le_bytes());
        data.extend_from_slice(&signature_offset.to_le_bytes());
        data.extend_from_slice(&u16::MAX.to_le_bytes());
        data.extend_from_slice(&public_key_offset.to_le_bytes());
        data.extend_from_slice(&u16::MAX.to_le_bytes());
        data.extend_from_slice(&message_offset.to_le_bytes());
        data.extend_from_slice(&message_len.to_le_bytes());
        data.extend_from_slice(&1_u16.to_le_bytes());
        data.extend_from_slice(&signer);
        data.extend_from_slice(&[1_u8; adapter::ED25519_SIGNATURE_BYTES]);
        Instruction {
            program_id: Pubkey::new_from_array(adapter::ED25519_PROGRAM_ID_3_0),
            accounts: Vec::new(),
            data,
        }
    }

    #[test]
    fn current_instruction_binding_refuses_cpi_substitution_and_nonempty_native_accounts() {
        let program_id = Pubkey::new_unique();
        let accounts = [
            test_account(
                Pubkey::new_unique(),
                false,
                true,
                1,
                vec![],
                Pubkey::new_unique(),
                false,
            ),
            test_account(
                Pubkey::new_unique(),
                false,
                true,
                1,
                vec![],
                Pubkey::new_unique(),
                false,
            ),
        ];
        let data = adapter::encode_close_replay_registration_instruction_v2();
        let metas = accounts
            .iter()
            .map(|value| AccountMeta {
                pubkey: *value.key,
                is_signer: value.is_signer,
                is_writable: value.is_writable,
            })
            .collect();
        let previous = Instruction {
            program_id: Pubkey::new_from_array(adapter::ED25519_PROGRAM_ID_3_0),
            accounts: Vec::new(),
            data: vec![1],
        };
        let current = Instruction {
            program_id,
            accounts: metas,
            data: data.to_vec(),
        };
        let sysvar_account = instructions_sysvar(previous.clone(), current.clone());
        assert!(
            authenticate_current_instruction(&program_id, &accounts, &data, &sysvar_account)
                .is_ok()
        );

        let substituted = instructions_sysvar(
            previous.clone(),
            Instruction {
                program_id: Pubkey::new_unique(),
                ..current.clone()
            },
        );
        assert_eq!(
            authenticate_current_instruction(&program_id, &accounts, &data, &substituted).err(),
            Some(AdapterError::DirectAuthentication.into())
        );
        let nonempty_previous = instructions_sysvar(
            Instruction {
                accounts: vec![AccountMeta::new_readonly(Pubkey::new_unique(), false)],
                ..previous
            },
            current,
        );
        assert_eq!(
            authenticate_current_instruction(&program_id, &accounts, &data, &nonempty_previous)
                .err(),
            Some(AdapterError::DirectAuthentication.into())
        );
    }

    #[test]
    fn signed_message_binding_refuses_descriptor_program_forgery_and_trailing_bytes() {
        let program_id = Pubkey::new_unique();
        let signer = [9_u8; 32];
        let message = [3_u8; 32];
        let mut data = vec![0_u8; 16 + message.len()];
        overwrite(&mut data, 16..48, &message);
        let current = Instruction {
            program_id,
            accounts: Vec::new(),
            data: data.clone(),
        };
        let exact = exact_ed25519_instruction(signer, 16, 32);
        let sysvar_account = instructions_sysvar(exact.clone(), current.clone());
        assert!(
            message_authorization(&program_id, &[], &data, &sysvar_account, signer, &message,)
                .is_ok()
        );

        let mut wrong_offset = exact.clone();
        overwrite(&mut wrong_offset.data, 10..12, &17_u16.to_le_bytes());
        let wrong_offset_sysvar = instructions_sysvar(wrong_offset, current.clone());
        assert_eq!(
            message_authorization(
                &program_id,
                &[],
                &data,
                &wrong_offset_sysvar,
                signer,
                &message,
            )
            .err(),
            Some(AdapterError::DirectAuthentication.into())
        );

        let mut wrong_index = exact.clone();
        overwrite(&mut wrong_index.data, 14..16, &0_u16.to_le_bytes());
        let wrong_index_sysvar = instructions_sysvar(wrong_index, current.clone());
        assert_eq!(
            message_authorization(
                &program_id,
                &[],
                &data,
                &wrong_index_sysvar,
                signer,
                &message,
            )
            .err(),
            Some(AdapterError::DirectAuthentication.into())
        );

        let mut forged = exact.clone();
        fill(&mut forged.data, 48..112, 0);
        let forged_sysvar = instructions_sysvar(forged, current.clone());
        assert_eq!(
            message_authorization(&program_id, &[], &data, &forged_sysvar, signer, &message,).err(),
            Some(AdapterError::DirectAuthentication.into())
        );

        let wrong_program_sysvar = instructions_sysvar(
            Instruction {
                program_id: Pubkey::new_unique(),
                ..exact.clone()
            },
            current.clone(),
        );
        assert_eq!(
            message_authorization(
                &program_id,
                &[],
                &data,
                &wrong_program_sysvar,
                signer,
                &message,
            )
            .err(),
            Some(AdapterError::DirectAuthentication.into())
        );

        let mut trailing = exact;
        trailing.data.push(0);
        let trailing_sysvar = instructions_sysvar(trailing, current);
        assert_eq!(
            message_authorization(&program_id, &[], &data, &trailing_sysvar, signer, &message,)
                .err(),
            Some(AdapterError::DirectAuthentication.into())
        );
    }

    #[test]
    fn wrong_first_use_root_is_rejected_without_creation_side_effects() {
        let program_id = Pubkey::new_unique();
        let market = Pubkey::new_unique();
        let maker = [7; 32];
        let wrong = test_account(
            Pubkey::new_unique(),
            false,
            true,
            0,
            vec![],
            system_program::ID,
            false,
        );
        assert_eq!(
            authenticate_root_state(&program_id, &wrong, &market, 3, &maker, true).err(),
            Some(AdapterError::DirectAuthentication.into())
        );
        assert_eq!(wrong.lamports(), 0);
        assert!(wrong.data_is_empty());
        assert_eq!(wrong.owner, &system_program::ID);
    }

    #[test]
    fn prefunded_first_use_root_is_absent_and_only_needs_the_rent_shortfall() {
        let program_id = Pubkey::new_unique();
        let market = Pubkey::new_unique();
        let maker = [7; 32];
        let (root_key, bump) = root_pda(&program_id, &market, 3, &maker);
        let dust = 1_u64;
        let root = test_account(
            root_key,
            false,
            true,
            dust,
            vec![],
            system_program::ID,
            false,
        );
        let facts = authenticate_root_state(&program_id, &root, &market, 3, &maker, true)
            .expect("system-owned empty PDA is canonical absence despite dust");
        assert!(facts.created);
        assert_eq!(facts.bump, bump);
        assert_eq!(
            creation_top_up(&root, 100).expect("valid prefunded creation"),
            99
        );
        assert_eq!(root.lamports(), dust);
        assert!(root.data_is_empty());
        assert_eq!(root.owner, &system_program::ID);
    }
}
