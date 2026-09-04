//! Permissionless close of one drained, settled Direct maker replay.
//!
//! Wall 22's missing decrement, end to end: this is the ONLY route that ever
//! reduces `open_maker_root_count`, and therefore the only path from a filled
//! market to `CloseCapability`'s zero-count gate. It runs inside Retiring --
//! `consume_nonce_v2` refuses every non-Open phase, so the count can only
//! fall once retirement begins -- and closes exactly one maker replay per
//! invocation.
//!
//! # Two authors, one decrement
//!
//! The market-selected release carries a fifth ProgramSet entry
//! (`DIRECT_CLOSE_MAKER_SELECTOR_V1`) whose transition bytecode refuses a
//! non-Retiring header and a drained count and computes the decrement itself
//! (`nonzero` + `sub_into`); its effect writes the count word. This
//! executable independently derives the same poststate through
//! `close_maker_replay_v2` -- which also carries the refusals the release
//! artifacts cannot see, because they never observe the replay account:
//! `live_count != 0` and `fee_owed != 0`. Commit happens only where the two
//! agree, exactly as the begin-retiring route commits only where
//! `prepare_retiring_tail` and its released transition agree.
//!
//! # The fee gate, stated once
//!
//! The maker replay is the SOLE record of the FEE-TX2 receivable
//! (`fee_settlement_v1` reads the amount off this account and nothing else),
//! so a close that ignored `fee_owed` would erase a debt with no residue.
//! [`crate::TradingSbfError::CloseMakerFeeOutstanding`] refuses it by name;
//! fee settlement is deliberately phase-free, so settle-then-close is always
//! available in Retiring and the refusal strands nobody.
//!
//! # Rent, and the pending ruling
//!
//! The whole observed balance follows the landed Lean plan
//! (`MakerClosePlan`: principal plus `unclassified_donation`, all to the
//! immutably recorded `rent_owner` -- refund conservation proved). Refusing a
//! nonzero donation instead would hand a griefer a 1-lamport transfer that
//! strands the replay permanently, the exact outcome `CloseSeal`'s cap
//! commentary documents against. The permissionless closer's reward is
//! `DIRECT_CLOSE_MAKER_CLOSER_REWARD_V1 = 0` until cohort-9 ruling 1 carves
//! one from the donation slice.
//!
//! # No expected-state digests
//!
//! Sibling closes rewrite the root's count word, so a pinned digest would let
//! each close grief the next submission (the `fee_settlement_v1` argument).
//! Every economic value is derived from program-owned state; the commit
//! re-checks that the root bytes it rewrites are the exact bytes it planned
//! from, which is the pin's whole guarantee without its griefability.

extern crate alloc;

use alloc::vec;

use dclutch_account_profile_contract::{
    ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V1, AccountObservationV1, AccountProfileV1,
    ProjectionRegistersV2, derive_effect_permissions, project_atomic,
};
use dclutch_capability_contract::funding::funded_rent_persists_v1;
use dclutch_capability_program_contract::{
    CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1, CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityProgramV1,
    CapabilityRegistersV2, CapabilityRootHeaderV1,
    set_v2::{CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2, CapabilityProgramSetV2},
};
use dclutch_direct_codec::{
    close_maker_v1,
    close_maker_v1::{
        DIRECT_CLOSE_MAKER_IDENTITY_COUNT_V1, DIRECT_CLOSE_MAKER_REQUEST_SCHEMA_ID_V1,
        DIRECT_CLOSE_MAKER_ROOT_ACCOUNT_V1, DIRECT_CLOSE_MAKER_ROOT_IDENTITY_V1,
        DIRECT_CLOSE_MAKER_SCALAR_COUNT_V1, DIRECT_CLOSE_MAKER_SELECTOR_SCALAR_V1,
        DIRECT_CLOSE_MAKER_SELECTOR_V1, DIRECT_CLOSE_MAKER_TRADING_IDENTITY_V1,
        DirectCloseMakerReceiptV1, DirectCloseMakerRequestV1,
    },
    successor::{
        DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1, DIRECT_ROOT_SCHEMA_ID_V1, DIRECT_ROOT_STATE_BYTES_V1,
        DirectCoordinatesV1, DirectRootStateLayoutV1, DirectRootStateV1, MakerReplayCloseResultV2,
        MakerReplayRootV1, MakerReplaySeedsV1, SuccessorError, close_maker_replay_v2,
    },
};
use dclutch_effect_kernel::v2::{
    AccountInput, AccountPermission, ProgramV2 as EffectProgramV2, ResolvedEffect,
    SCHEMA_RELEASE_ID as EFFECT_SCHEMA_RELEASE_ID_V2, project_with_aliases_and_requests_atomic,
};
use dclutch_market_core_codec::{CoreState, MarketCoreStateSeedsV2, STATE_BYTES};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_activation_auth_v1::{
    authenticate_activated_role_in_frame_v1, authenticate_activation_cache_identity_v1,
    require_cache_account,
};
use dclutch_registry_contract::ActivatedExecutionReleaseSetViewV1;
use dclutch_registry_svm::AuthenticatedRoleReceiptV1;
use dclutch_release_set_contract::ExecutionRoleV1;
use dclutch_transition_vm::v2::{RegisterInput, RegisterOutput};
use solana_program::{
    account_info::AccountInfo, hash::hash, program::set_return_data, program_error::ProgramError,
    pubkey::Pubkey,
};
use solana_sdk_ids::{system_program, sysvar};

use crate::{TradingSbfError, dispatch::TradingFamilyContextV1};

use crate::market_admission_v1::TRADING_RETIRING_MARKET_ADMISSIBLE_PRESTATES_V1;
pub use dclutch_direct_codec::close_maker_v1::DIRECT_CLOSE_MAKER_ACCOUNT_COUNT_V1;

struct Accounts<'accounts, 'info> {
    root: &'accounts AccountInfo<'info>,
    market: &'accounts AccountInfo<'info>,
    manifest_raw: &'accounts AccountInfo<'info>,
    program_set_raw: &'accounts AccountInfo<'info>,
    program_set_staging: &'accounts AccountInfo<'info>,
    descriptor_raw: &'accounts AccountInfo<'info>,
    descriptor_staging: &'accounts AccountInfo<'info>,
    config_raw: &'accounts AccountInfo<'info>,
    config_staging: &'accounts AccountInfo<'info>,
    profile_raw: &'accounts AccountInfo<'info>,
    profile_staging: &'accounts AccountInfo<'info>,
    effect_raw: &'accounts AccountInfo<'info>,
    effect_staging: &'accounts AccountInfo<'info>,
    cache: &'accounts AccountInfo<'info>,
    core_program: &'accounts AccountInfo<'info>,
    core_programdata: &'accounts AccountInfo<'info>,
    trading_program: &'accounts AccountInfo<'info>,
    trading_programdata: &'accounts AccountInfo<'info>,
    registry: &'accounts AccountInfo<'info>,
    rent: &'accounts AccountInfo<'info>,
    replay: &'accounts AccountInfo<'info>,
    rent_owner: &'accounts AccountInfo<'info>,
}

impl<'accounts, 'info> Accounts<'accounts, 'info> {
    fn parse(
        program_id: &Pubkey,
        accounts: &'accounts [AccountInfo<'info>],
    ) -> Result<Self, ProgramError> {
        if accounts.len() != DIRECT_CLOSE_MAKER_ACCOUNT_COUNT_V1
            || accounts.iter().any(|account| account.is_signer)
        {
            return Err(TradingSbfError::CloseMakerFrame.into());
        }
        for (index, account) in accounts.iter().enumerate() {
            let (expected_writable, expected_executable) =
                close_maker_v1::direct_close_maker_account_privileges_v1(index)
                    .ok_or(TradingSbfError::CloseMakerFrame)?;
            if account.is_writable != expected_writable || account.executable != expected_executable
            {
                return Err(TradingSbfError::CloseMakerFrame.into());
            }
            if accounts
                .get(index.saturating_add(1)..)
                .is_some_and(|suffix| suffix.iter().any(|other| other.key == account.key))
            {
                return Err(TradingSbfError::CloseMakerFrame.into());
            }
        }
        let value = Self {
            root: get(
                accounts,
                close_maker_v1::DIRECT_CLOSE_MAKER_ROOT_TOP_ACCOUNT_V1,
            )?,
            market: get(
                accounts,
                close_maker_v1::DIRECT_CLOSE_MAKER_MARKET_ACCOUNT_V1,
            )?,
            manifest_raw: get(
                accounts,
                close_maker_v1::DIRECT_CLOSE_MAKER_MANIFEST_RAW_ACCOUNT_V1,
            )?,
            program_set_raw: get(
                accounts,
                close_maker_v1::DIRECT_CLOSE_MAKER_PROGRAM_SET_RAW_ACCOUNT_V1,
            )?,
            program_set_staging: get(
                accounts,
                close_maker_v1::DIRECT_CLOSE_MAKER_PROGRAM_SET_STAGING_ACCOUNT_V1,
            )?,
            descriptor_raw: get(
                accounts,
                close_maker_v1::DIRECT_CLOSE_MAKER_DESCRIPTOR_RAW_ACCOUNT_V1,
            )?,
            descriptor_staging: get(
                accounts,
                close_maker_v1::DIRECT_CLOSE_MAKER_DESCRIPTOR_STAGING_ACCOUNT_V1,
            )?,
            config_raw: get(
                accounts,
                close_maker_v1::DIRECT_CLOSE_MAKER_CONFIG_RAW_ACCOUNT_V1,
            )?,
            config_staging: get(
                accounts,
                close_maker_v1::DIRECT_CLOSE_MAKER_CONFIG_STAGING_ACCOUNT_V1,
            )?,
            profile_raw: get(
                accounts,
                close_maker_v1::DIRECT_CLOSE_MAKER_PROFILE_RAW_ACCOUNT_V1,
            )?,
            profile_staging: get(
                accounts,
                close_maker_v1::DIRECT_CLOSE_MAKER_PROFILE_STAGING_ACCOUNT_V1,
            )?,
            effect_raw: get(
                accounts,
                close_maker_v1::DIRECT_CLOSE_MAKER_EFFECT_RAW_ACCOUNT_V1,
            )?,
            effect_staging: get(
                accounts,
                close_maker_v1::DIRECT_CLOSE_MAKER_EFFECT_STAGING_ACCOUNT_V1,
            )?,
            cache: get(
                accounts,
                close_maker_v1::DIRECT_CLOSE_MAKER_ACTIVATION_CACHE_ACCOUNT_V1,
            )?,
            core_program: get(
                accounts,
                close_maker_v1::DIRECT_CLOSE_MAKER_CORE_PROGRAM_ACCOUNT_V1,
            )?,
            core_programdata: get(
                accounts,
                close_maker_v1::DIRECT_CLOSE_MAKER_CORE_PROGRAMDATA_ACCOUNT_V1,
            )?,
            trading_program: get(
                accounts,
                close_maker_v1::DIRECT_CLOSE_MAKER_TRADING_PROGRAM_ACCOUNT_V1,
            )?,
            trading_programdata: get(
                accounts,
                close_maker_v1::DIRECT_CLOSE_MAKER_TRADING_PROGRAMDATA_ACCOUNT_V1,
            )?,
            registry: get(
                accounts,
                close_maker_v1::DIRECT_CLOSE_MAKER_REGISTRY_ACCOUNT_V1,
            )?,
            rent: get(accounts, close_maker_v1::DIRECT_CLOSE_MAKER_RENT_ACCOUNT_V1)?,
            replay: get(
                accounts,
                close_maker_v1::DIRECT_CLOSE_MAKER_REPLAY_ACCOUNT_V1,
            )?,
            rent_owner: get(
                accounts,
                close_maker_v1::DIRECT_CLOSE_MAKER_RENT_OWNER_ACCOUNT_V1,
            )?,
        };
        if value.trading_program.key != program_id
            || value.rent.key != &sysvar::rent::ID
            || value.cache.owner != value.registry.key
        {
            return Err(TradingSbfError::CloseMakerFrame.into());
        }
        Ok(value)
    }
}

/// Execute one exact permissionless maker-replay close.
#[inline(never)]
pub fn process_direct_close_maker_v1(
    program_id: &Pubkey,
    account_infos: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    let request = DirectCloseMakerRequestV1::decode(instruction_data)
        .map_err(|_| TradingSbfError::Content)?;
    let accounts = Accounts::parse(program_id, account_infos)?;

    let root_data = accounts
        .root
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Root)?;
    if root_data.len() != CAPABILITY_ROOT_HEADER_BYTES_V1 + DIRECT_ROOT_STATE_BYTES_V1 {
        return Err(TradingSbfError::Root.into());
    }
    let header = CapabilityRootHeaderV1::decode(
        root_data
            .get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
            .ok_or(TradingSbfError::Root)?,
    )
    .map_err(|_| TradingSbfError::Root)?;
    let release_set = header.release_set().to_bytes();
    let trading_receipt = reauthenticate_roles(&accounts, release_set)?;
    let context = TradingFamilyContextV1::authenticate(
        program_id,
        accounts.root.key,
        accounts.root.owner,
        &root_data,
        trading_receipt,
    )?;
    if context.market() != request.market || context.generation() != request.generation {
        return Err(TradingSbfError::Content.into());
    }
    authenticate_market(&accounts, request, release_set)?;

    // The semantic close first: it owns every refusal about the replay itself
    // (coordinates, live intents, the outstanding fee, rent funding), and its
    // poststate is the number the released artifacts must independently
    // produce below.
    let pre_root_state = DirectRootStateV1::decode(
        root_data
            .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
            .ok_or(TradingSbfError::Root)?,
    )
    .map_err(|_| TradingSbfError::Root)?;
    let closed = authenticate_replay_close(program_id, &accounts, request, pre_root_state)?;
    require_rent_owner_destination(&accounts, closed.plan.rent_owner)?;
    let direct_post = closed.root.encode();

    let manifest_data = accounts
        .manifest_raw
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    authenticate_persisted_raw(
        accounts.registry.key,
        accounts.manifest_raw,
        dclutch_capability_contract::CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        context.selection().manifest().to_bytes(),
        header.record_bumps().manifest_raw(),
        &manifest_data,
    )?;
    let program_set_data = accounts
        .program_set_raw
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let program_set_id = context.selection().capability_release().to_bytes();
    authenticate_finalized_record(
        accounts.registry.key,
        accounts.program_set_raw,
        accounts.program_set_staging,
        CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
        program_set_id,
        &program_set_data,
    )?;
    let set =
        CapabilityProgramSetV2::decode_selected(program_set_id, program_set_id, &program_set_data)
            .map_err(|_| TradingSbfError::Content)?;
    let selected = set
        .select_descriptor(instruction_data)
        .map_err(|_| TradingSbfError::UnsupportedContent)?;
    if selected.schema().to_bytes() != CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1 {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    let descriptor_data = accounts
        .descriptor_raw
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    authenticate_finalized_record(
        accounts.registry.key,
        accounts.descriptor_raw,
        accounts.descriptor_staging,
        selected.schema().to_bytes(),
        selected.program().to_bytes(),
        &descriptor_data,
    )?;
    let config_data = accounts
        .config_raw
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    authenticate_finalized_record(
        accounts.registry.key,
        accounts.config_raw,
        accounts.config_staging,
        DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1,
        context.selection().config().to_bytes(),
        &config_data,
    )?;
    let descriptor = crate::dispatch::authenticate_activation_program(
        context,
        selected.program(),
        &manifest_data,
        &descriptor_data,
        &config_data,
    )?;
    if descriptor.request_schema().to_bytes() != DIRECT_CLOSE_MAKER_REQUEST_SCHEMA_ID_V1
        || descriptor.root_schema().to_bytes() != DIRECT_ROOT_SCHEMA_ID_V1
        || descriptor.config_schema().to_bytes() != DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1
        || usize::try_from(descriptor.root_state_bytes()).ok() != Some(DIRECT_ROOT_STATE_BYTES_V1)
    {
        return Err(TradingSbfError::Content.into());
    }

    let profile_data = accounts
        .profile_raw
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    authenticate_finalized_record(
        accounts.registry.key,
        accounts.profile_raw,
        accounts.profile_staging,
        ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V1,
        descriptor.account_profile().to_bytes(),
        &profile_data,
    )?;
    let profile = AccountProfileV1::decode_selected(
        descriptor.account_profile().to_bytes(),
        hash(&profile_data).to_bytes(),
        &profile_data,
    )
    .map_err(|_| TradingSbfError::Content)?;
    let effect_data = accounts
        .effect_raw
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    authenticate_finalized_record(
        accounts.registry.key,
        accounts.effect_raw,
        accounts.effect_staging,
        EFFECT_SCHEMA_RELEASE_ID_V2,
        descriptor.effect_schema().to_bytes(),
        &effect_data,
    )?;
    let effect = EffectProgramV2::decode_selected(
        descriptor.effect_schema().to_bytes(),
        hash(&effect_data).to_bytes(),
        &effect_data,
    )
    .map_err(|_| TradingSbfError::Content)?;

    authenticate_artifact_transition(
        program_id,
        accounts.root,
        &root_data,
        profile,
        descriptor,
        effect,
        direct_post,
    )?;

    let mut post_root = root_data.to_vec();
    post_root
        .get_mut(CAPABILITY_ROOT_HEADER_BYTES_V1..)
        .ok_or(TradingSbfError::Root)?
        .copy_from_slice(&direct_post);
    let post_root_digest = hash(&post_root).to_bytes();
    let pre_root_digest = hash(&root_data).to_bytes();
    let receipt = DirectCloseMakerReceiptV1 {
        request_digest: hash(instruction_data).to_bytes(),
        market: request.market,
        maker: request.maker,
        maker_root: accounts.replay.key.to_bytes(),
        rent_owner: closed.plan.rent_owner,
        post_root_digest,
        rent_principal: closed.plan.rent_principal,
        unclassified_donation: closed.plan.unclassified_donation,
        total_credit: closed.plan.total_credit,
        remaining_open_maker_roots: closed.root.open_maker_root_count(),
    }
    .to_bytes()
    .map_err(|_| TradingSbfError::Content)?;
    drop(effect_data);
    drop(profile_data);
    drop(config_data);
    drop(descriptor_data);
    drop(program_set_data);
    drop(manifest_data);
    drop(root_data);

    commit(&accounts, pre_root_digest, &post_root, closed)?;
    set_return_data(&receipt);
    Ok(())
}

/// Write the decremented root, drain the replay to its recorded owner, and
/// return the replay's account to the System program.
fn commit(
    accounts: &Accounts<'_, '_>,
    pre_root_digest: [u8; 32],
    post_root: &[u8],
    closed: MakerReplayCloseResultV2,
) -> Result<(), ProgramError> {
    let mut root_commit = accounts
        .root
        .try_borrow_mut_data()
        .map_err(|_| TradingSbfError::Commit)?;
    // The bytes being rewritten must be the exact bytes the close was planned
    // from -- the digest pin's guarantee, derived internally rather than
    // carried on the wire where a sibling close could grief it.
    if hash(&root_commit).to_bytes() != pre_root_digest || root_commit.len() != post_root.len() {
        return Err(TradingSbfError::Commit.into());
    }
    root_commit.copy_from_slice(post_root);
    drop(root_commit);

    let total_credit = closed.plan.total_credit;
    if accounts.replay.lamports() != total_credit {
        return Err(TradingSbfError::Commit.into());
    }
    let destination_after = accounts
        .rent_owner
        .lamports()
        .checked_add(total_credit)
        .ok_or(TradingSbfError::Commit)?;
    {
        let mut destination_lamports = accounts
            .rent_owner
            .try_borrow_mut_lamports()
            .map_err(|_| TradingSbfError::Commit)?;
        let mut replay_lamports = accounts
            .replay
            .try_borrow_mut_lamports()
            .map_err(|_| TradingSbfError::Commit)?;
        **destination_lamports = destination_after;
        **replay_lamports = 0;
    }
    accounts
        .replay
        .resize(0)
        .map_err(|_| TradingSbfError::Commit)?;
    accounts.replay.assign(&system_program::ID);
    if accounts.rent_owner.lamports() != destination_after
        || accounts.replay.lamports() != 0
        || accounts.replay.owner != &system_program::ID
        || !accounts
            .replay
            .try_data_is_empty()
            .map_err(|_| TradingSbfError::Commit)?
    {
        return Err(TradingSbfError::Commit.into());
    }
    Ok(())
}

/// Authenticate the replay account as the canonical one for the request's
/// coordinate and run the semantic close over it.
///
/// Every refusal here names its condition: a wrong or non-canonical account is
/// [`TradingSbfError::CloseMakerReplayAccount`], standing registered intents
/// are [`TradingSbfError::CloseMakerLiveIntents`], and an unsettled fee is
/// [`TradingSbfError::CloseMakerFeeOutstanding`]. A non-Retiring root or a
/// drained count refuses as [`TradingSbfError::Transition`], the same answer
/// the released transition bytecode gives for the same facts.
#[inline(never)]
fn authenticate_replay_close(
    program_id: &Pubkey,
    accounts: &Accounts<'_, '_>,
    request: DirectCloseMakerRequestV1,
    pre_root_state: DirectRootStateV1,
) -> Result<MakerReplayCloseResultV2, ProgramError> {
    let coordinates = DirectCoordinatesV1::new(request.market, request.generation)
        .map_err(|_| TradingSbfError::Content)?;
    let replay = accounts.replay;
    let data = replay
        .try_borrow_data()
        .map_err(|_| TradingSbfError::CloseMakerReplayAccount)?;
    let maker_root =
        MakerReplayRootV1::decode(&data).map_err(|_| TradingSbfError::CloseMakerReplayAccount)?;
    let seeds = MakerReplaySeedsV1::new(coordinates, request.maker)
        .map_err(|_| TradingSbfError::Content)?;
    let [domain, market, generation, maker] = seeds.as_slices();
    let bump = [maker_root.bump()];
    let expected =
        Pubkey::create_program_address(&[domain, market, generation, maker, &bump], program_id)
            .map_err(|_| TradingSbfError::CloseMakerReplayAccount)?;
    if replay.owner != program_id
        || replay.key != &expected
        || maker_root.market() != request.market
        || maker_root.generation() != request.generation
        || maker_root.maker() != request.maker
        || !funded_rent_persists_v1(replay.lamports())
    {
        return Err(TradingSbfError::CloseMakerReplayAccount.into());
    }
    let closed =
        close_maker_replay_v2(pre_root_state, maker_root, replay.lamports()).map_err(|error| {
            match error {
                SuccessorError::FeeOwedOutstanding => TradingSbfError::CloseMakerFeeOutstanding,
                SuccessorError::LiveCountInvariant => TradingSbfError::CloseMakerLiveIntents,
                SuccessorError::InvalidRootPhase | SuccessorError::MakerRootCountInvariant => {
                    TradingSbfError::Transition
                }
                _ => TradingSbfError::CloseMakerReplayAccount,
            }
        })?;
    Ok(closed)
}

/// The recorded rent owner is the destination, and it must be a plain System
/// wallet: a program-owned refund destination is an account whose bytes mean
/// something to somebody, and crediting one is a write this route has no
/// authority to make (`CloseSeal`'s beneficiary rule, minus the signature --
/// the destination here is program-recorded, not caller-chosen, so nobody
/// needs to sign for it).
fn require_rent_owner_destination(
    accounts: &Accounts<'_, '_>,
    rent_owner: [u8; 32],
) -> Result<(), ProgramError> {
    let destination = accounts.rent_owner;
    if destination.key.to_bytes() != rent_owner
        || destination.owner != &system_program::ID
        || destination.executable
        || !destination
            .try_data_is_empty()
            .map_err(|_| TradingSbfError::CloseMakerFrame)?
    {
        return Err(TradingSbfError::CloseMakerFrame.into());
    }
    Ok(())
}

/// Hold the Market to the request, out of line, in a frame of its own.
///
/// The frame discipline is `direct_begin_retiring_v1::authenticate_market`'s,
/// for the same reason: the decode/re-encode comparison carries 2,304 bytes of
/// `CoreState` and must not share a frame with the route's borrows.
///
/// There is deliberately NO market-digest pin (see the module header), and the
/// Market must be exactly `Retiring`: a Direct root can only have reached its
/// own Retiring phase through a `Retiring` Market, and the Market cannot reach
/// `Retired` while this route still has work -- both physical-close gates
/// refuse a nonzero maker count -- so the requirement can never strand a
/// replay.
#[inline(never)]
fn authenticate_market(
    accounts: &Accounts<'_, '_>,
    request: DirectCloseMakerRequestV1,
    release_set: [u8; 32],
) -> Result<(), ProgramError> {
    let data = accounts
        .market
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    authenticate_market_bytes(
        accounts.market.key,
        accounts.market.owner,
        accounts.core_program.key,
        accounts.registry.key,
        &data,
        request,
        release_set,
    )
}

#[allow(clippy::too_many_arguments)]
fn authenticate_market_bytes(
    market_key: &Pubkey,
    market_owner: &Pubkey,
    core_program: &Pubkey,
    registry: &Pubkey,
    data: &[u8],
    request: DirectCloseMakerRequestV1,
    release_set: [u8; 32],
) -> Result<(), ProgramError> {
    if market_owner != core_program
        || market_key.to_bytes() != request.market
        || data.len() != STATE_BYTES
    {
        return Err(TradingSbfError::Content.into());
    }
    let state = CoreState::decode(data).map_err(|_| TradingSbfError::Content)?;
    let expected = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(state.identity).as_slices(),
        core_program,
    )
    .0;
    if expected != *market_key
        || state
            .encode()
            .map_err(|_| TradingSbfError::Content)?
            .as_slice()
            != data
        || !TRADING_RETIRING_MARKET_ADMISSIBLE_PRESTATES_V1.admits_phase(state.phase)
        || state.identity.market_id.to_bytes() != request.market
        || state.identity.selected_release_set.to_bytes() != release_set
        || state.identity.registry_program.to_bytes() != registry.to_bytes()
        || state.identity.generation != request.generation
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

/// Run the release-selected close artifacts over the root and require their
/// authored poststate to equal the semantic close's.
#[allow(clippy::too_many_arguments)]
fn authenticate_artifact_transition(
    program_id: &Pubkey,
    root: &AccountInfo<'_>,
    root_data: &[u8],
    profile: AccountProfileV1<'_>,
    descriptor: CapabilityProgramV1<'_>,
    effect: EffectProgramV2<'_>,
    expected_tail: [u8; DIRECT_ROOT_STATE_BYTES_V1],
) -> Result<(), ProgramError> {
    if profile.account_count() != 1
        || profile.scalar_count() != DIRECT_CLOSE_MAKER_SCALAR_COUNT_V1
        || profile.identity_count() != DIRECT_CLOSE_MAKER_IDENTITY_COUNT_V1
        || effect.account_count() != 1
        || effect.scalar_count() != DIRECT_CLOSE_MAKER_SCALAR_COUNT_V1
        || effect.identity_count() != DIRECT_CLOSE_MAKER_IDENTITY_COUNT_V1
        || effect.request_bytes() != 0
    {
        return Err(TradingSbfError::Content.into());
    }
    let mut input_scalars = vec![0_u64; usize::from(profile.scalar_count())];
    let mut input_identities = vec![[0_u8; 32]; usize::from(profile.identity_count())];
    *input_scalars
        .get_mut(usize::from(DIRECT_CLOSE_MAKER_SELECTOR_SCALAR_V1))
        .ok_or(TradingSbfError::Content)? = u64::from(DIRECT_CLOSE_MAKER_SELECTOR_V1);
    *input_identities
        .get_mut(usize::from(DIRECT_CLOSE_MAKER_TRADING_IDENTITY_V1))
        .ok_or(TradingSbfError::Content)? = program_id.to_bytes();
    *input_identities
        .get_mut(usize::from(DIRECT_CLOSE_MAKER_ROOT_IDENTITY_V1))
        .ok_or(TradingSbfError::Content)? = root.key.to_bytes();
    let observation = [AccountObservationV1::new(
        root.key.as_array(),
        root.owner.as_array(),
        root.lamports(),
        root_data,
        root.is_signer,
        root.is_writable,
        root.executable,
    )];
    let mut projection_scratch_scalars = input_scalars.clone();
    let mut projection_scratch_identities = input_identities.clone();
    let mut projected_scalars = input_scalars.clone();
    let mut projected_identities = input_identities.clone();
    project_atomic(
        profile,
        &observation,
        ProjectionRegistersV2::new(
            RegisterInput {
                scalars: &input_scalars,
                identities: &input_identities,
            },
            RegisterOutput {
                scalars: &mut projection_scratch_scalars,
                identities: &mut projection_scratch_identities,
            },
            RegisterOutput {
                scalars: &mut projected_scalars,
                identities: &mut projected_identities,
            },
        ),
    )
    .map_err(|_| TradingSbfError::Content)?;
    let mut transition_scratch_scalars = projected_scalars.clone();
    let mut transition_scratch_identities = projected_identities.clone();
    let mut transition_output_scalars = projected_scalars.clone();
    let mut transition_output_identities = projected_identities.clone();
    descriptor
        .execute(CapabilityRegistersV2::new(
            RegisterInput {
                scalars: &projected_scalars,
                identities: &projected_identities,
            },
            RegisterOutput {
                scalars: &mut transition_scratch_scalars,
                identities: &mut transition_scratch_identities,
            },
            RegisterOutput {
                scalars: &mut transition_output_scalars,
                identities: &mut transition_output_identities,
            },
        ))
        .map_err(|_| TradingSbfError::Transition)?;
    let account_inputs = [AccountInput {
        lamports: root.lamports(),
        data_len: root_data.len(),
    }];
    let mut permissions = [AccountPermission::read_only()];
    derive_effect_permissions(profile, &mut permissions).map_err(|_| TradingSbfError::Content)?;
    let aliases = [profile
        .rule(0)
        .map_err(|_| TradingSbfError::Content)?
        .alias_of()];
    let mut scratch_lamports = [0_u64];
    let mut output_lamports = [0_u64];
    let mut scratch_request = [];
    let mut output_request = [];
    project_with_aliases_and_requests_atomic(
        effect,
        &transition_output_scalars,
        &transition_output_identities,
        &aliases,
        &account_inputs,
        &permissions,
        &mut scratch_lamports,
        &mut output_lamports,
        &mut scratch_request,
        &mut output_request,
    )
    .map_err(|_| TradingSbfError::Content)?;
    if output_lamports != [root.lamports()] || effect.instruction_count() != 2 {
        return Err(TradingSbfError::Content.into());
    }
    let expected_count = u64::from_le_bytes(
        expected_tail
            .get(
                DirectRootStateLayoutV1::OPEN_MAKER_ROOT_COUNT
                    ..DirectRootStateLayoutV1::OPEN_MAKER_ROOT_COUNT + 8,
            )
            .ok_or(TradingSbfError::Content)?
            .try_into()
            .map_err(|_| TradingSbfError::Content)?,
    );
    match (
        effect
            .resolved_effect(0, &transition_output_scalars, &transition_output_identities)
            .map_err(|_| TradingSbfError::Content)?,
        effect
            .resolved_effect(1, &transition_output_scalars, &transition_output_identities)
            .map_err(|_| TradingSbfError::Content)?,
    ) {
        (
            ResolvedEffect::WriteScalar {
                account: DIRECT_CLOSE_MAKER_ROOT_ACCOUNT_V1,
                offset,
                value,
            },
            ResolvedEffect::RequireLamportsEq {
                account: DIRECT_CLOSE_MAKER_ROOT_ACCOUNT_V1,
                value: lamports,
            },
        ) if usize::try_from(offset).ok()
            == Some(
                CAPABILITY_ROOT_HEADER_BYTES_V1 + DirectRootStateLayoutV1::OPEN_MAKER_ROOT_COUNT,
            )
            && value == expected_count
            && lamports == root.lamports() => {}
        _ => return Err(TradingSbfError::UnsupportedContent.into()),
    }
    Ok(())
}

/// Authenticate Core and Trading for the root's release set from one read of
/// the Registry-owned activation cache; `direct_begin_retiring_v1` documents
/// both the mechanism and the frame discipline.
#[inline(never)]
fn reauthenticate_roles<'info>(
    accounts: &Accounts<'_, 'info>,
    release_set: [u8; 32],
) -> Result<AuthenticatedRoleReceiptV1, ProgramError> {
    require_cache_account(accounts.registry.key, accounts.cache).map_err(TradingSbfError::from)?;
    let data = accounts
        .cache
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Release)?;
    let activated =
        ActivatedExecutionReleaseSetViewV1::decode(&data).map_err(|_| TradingSbfError::Release)?;
    authenticate_activation_cache_identity_v1(
        accounts.registry,
        accounts.cache,
        &release_set,
        activated,
    )
    .map_err(TradingSbfError::from)?;
    let core_receipt = authenticate_activated_role_in_frame_v1(
        accounts.cache,
        activated,
        ExecutionRoleV1::Core,
        accounts.core_program,
        accounts.core_programdata,
    )
    .map_err(TradingSbfError::from)?;
    if core_receipt.program().to_bytes() != accounts.core_program.key.to_bytes() {
        return Err(TradingSbfError::Release.into());
    }
    authenticate_activated_role_in_frame_v1(
        accounts.cache,
        activated,
        ExecutionRoleV1::Trading,
        accounts.trading_program,
        accounts.trading_programdata,
    )
    .map_err(|error| TradingSbfError::from(error).into())
}

#[allow(clippy::too_many_arguments)]
fn authenticate_finalized_record(
    registry: &Pubkey,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    schema: [u8; 32],
    digest: [u8; 32],
    bytes: &[u8],
) -> Result<(), ProgramError> {
    let expected_raw =
        Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], registry).0;
    let expected_staging =
        Pubkey::find_program_address(&[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest], registry).0;
    if raw.key != &expected_raw
        || raw.owner != registry
        || hash(bytes).to_bytes() != digest
        || !funded_rent_persists_v1(raw.lamports())
        || staging.key != &expected_staging
        || staging.owner != &system_program::ID
        || staging.data_len() != 0
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn authenticate_persisted_raw(
    registry: &Pubkey,
    raw: &AccountInfo<'_>,
    schema: [u8; 32],
    digest: [u8; 32],
    raw_bump: u8,
    bytes: &[u8],
) -> Result<(), ProgramError> {
    let bump = [raw_bump];
    let expected = Pubkey::create_program_address(
        &[RAW_RECORD_PDA_SEED_V1, &schema, &digest, &bump],
        registry,
    )
    .map_err(|_| TradingSbfError::Content)?;
    if raw.key != &expected
        || raw.owner != registry
        || hash(bytes).to_bytes() != digest
        || !funded_rent_persists_v1(raw.lamports())
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

fn get<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<&'accounts AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or_else(|| TradingSbfError::CloseMakerFrame.into())
}
