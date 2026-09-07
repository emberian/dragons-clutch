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
//! # Rent, and the three credits
//!
//! The whole observed balance follows the landed Lean plan (`MakerClosePlan`):
//! the immutably recorded `rent_owner` receives `rent_principal` EXACTLY, the
//! closer at coordinate 24 receives the carve, and every remaining lamport of
//! `unclassified_donation` is credited to the upkeep vault at coordinate 23
//! and receipted there by CPI (decision 0024 item 4). Refusing a nonzero
//! donation instead would hand a griefer a 1-lamport transfer that strands the
//! replay permanently, the exact outcome `CloseSeal`'s cap commentary
//! documents against; housing it is the alternative that route takes.
//!
//! The carve is not a constant in this executable. It is
//! `ProtocolParametersV1::closer_carve` over the donation, read out of the
//! governed record at coordinate 22 -- Custody-owned, at the address this
//! route derives for itself -- so moving the closer's pay is a proposal
//! against that record and its delay, never an ELF.
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

use dclutch_custody::CallerRoleV1;
use dclutch_custody::upkeep_vault_v1::{
    UPKEEP_VAULT_RECORD_BYTES_V1, UpkeepCreditV1, UpkeepOperationV1, UpkeepProtocolCallerV1,
    UpkeepRequestV1, UpkeepSourceClassV1, UpkeepVaultSeedsV1,
};
use dclutch_market::capability_manifest::funding::funded_rent_persists_v1;
use dclutch_market::capability_program::{
    CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1, CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityProgramV1,
    CapabilityRegistersV2, CapabilityRootHeaderV1,
    set_v2::{CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2, CapabilityProgramSetV2},
};
use dclutch_market::protocol_parameters::{
    ProtocolParametersRecordSeedsV1, ProtocolParametersV1,
    authenticate_protocol_parameters_account_v1,
};
use dclutch_market::{CoreState, MarketCoreStateSeedsV2, STATE_BYTES};
use dclutch_registry::ActivatedExecutionReleaseSetViewV1;
use dclutch_registry::activation_auth_v1::{
    authenticate_activated_role_in_frame_v1, authenticate_activation_cache_identity_v1,
    require_cache_account,
};
use dclutch_registry::record::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry::release_set::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_registry::svm::AuthenticatedRoleReceiptV1;
use dclutch_trading::{
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
use dclutch_vm::account_profile::{
    ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V1, AccountObservationV1, AccountProfileV1,
    ProjectionRegistersV2, derive_effect_permissions, project_atomic,
};
use dclutch_vm::effect::v2::{
    AccountInput, AccountPermission, ProgramV2 as EffectProgramV2, ResolvedEffect,
    SCHEMA_RELEASE_ID as EFFECT_SCHEMA_RELEASE_ID_V2, project_with_aliases_and_requests_atomic,
};
use dclutch_vm::v2::{RegisterInput, RegisterOutput};
use solana_program::{
    account_info::AccountInfo,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::{invoke_signed, set_return_data},
    program_error::ProgramError,
    pubkey::Pubkey,
};
use solana_sdk_ids::{system_program, sysvar};

use crate::{TradingSbfError, dispatch::TradingFamilyContextV1};

use crate::market_admission_v1::TRADING_RETIRING_MARKET_ADMISSIBLE_PRESTATES_V1;
pub use dclutch_trading::close_maker_v1::DIRECT_CLOSE_MAKER_ACCOUNT_COUNT_V1;

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
    parameters: &'accounts AccountInfo<'info>,
    upkeep_vault: &'accounts AccountInfo<'info>,
    closer: &'accounts AccountInfo<'info>,
    custody_program: &'accounts AccountInfo<'info>,
    caller_authority: &'accounts AccountInfo<'info>,
}

impl<'accounts, 'info> Accounts<'accounts, 'info> {
    fn parse(
        program_id: &Pubkey,
        accounts: &'accounts [AccountInfo<'info>],
    ) -> Result<Self, ProgramError> {
        if accounts.len() != DIRECT_CLOSE_MAKER_ACCOUNT_COUNT_V1 {
            return Err(TradingSbfError::CloseMakerFrame.into());
        }
        // THE SIGNER RULE, exempting exactly one coordinate by name. The route
        // stays permissionless -- no party to the market signs it -- but the
        // closer must sign to OWN the carve, never to be authorized by it
        // (`FUNDED_CRANK_V1.md` section 6). Any other signer, or a missing
        // one, is `CloseMakerCloser`: a frame refusal about the closer, told
        // apart from every other frame refusal so a caller that signed with
        // the wrong key learns which key was wrong.
        if accounts.iter().enumerate().any(|(index, account)| {
            account.is_signer != (index == close_maker_v1::DIRECT_CLOSE_MAKER_CLOSER_ACCOUNT_V1)
        }) {
            return Err(TradingSbfError::CloseMakerCloser.into());
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
            parameters: get(
                accounts,
                close_maker_v1::DIRECT_CLOSE_MAKER_PROTOCOL_PARAMETERS_ACCOUNT_V1,
            )?,
            upkeep_vault: get(
                accounts,
                close_maker_v1::DIRECT_CLOSE_MAKER_UPKEEP_VAULT_ACCOUNT_V1,
            )?,
            closer: get(
                accounts,
                close_maker_v1::DIRECT_CLOSE_MAKER_CLOSER_ACCOUNT_V1,
            )?,
            custody_program: get(
                accounts,
                close_maker_v1::DIRECT_CLOSE_MAKER_CUSTODY_PROGRAM_ACCOUNT_V1,
            )?,
            caller_authority: get(
                accounts,
                close_maker_v1::DIRECT_CLOSE_MAKER_CALLER_AUTHORITY_ACCOUNT_V1,
            )?,
        };
        if value.trading_program.key != program_id
            || value.rent.key != &sysvar::rent::ID
            || value.cache.owner != value.registry.key
        {
            return Err(TradingSbfError::CloseMakerFrame.into());
        }
        if !value.custody_program.executable {
            return Err(TradingSbfError::CloseMakerUpkeepVault.into());
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
    let parameters = authenticate_economics(&accounts)?;
    let closed =
        authenticate_replay_close(program_id, &accounts, request, pre_root_state, parameters)?;
    require_rent_owner_destination(&accounts, closed.plan.rent_owner)?;
    let direct_post = closed.root.encode();

    let manifest_data = accounts
        .manifest_raw
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    authenticate_persisted_raw(
        accounts.registry.key,
        accounts.manifest_raw,
        dclutch_market::capability_manifest::CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
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
        closer: accounts.closer.key.to_bytes(),
        rent_principal: closed.plan.rent_principal,
        unclassified_donation: closed.plan.unclassified_donation,
        closer_reward: closed.plan.closer_reward,
        upkeep_credit: closed.plan.upkeep_credit,
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

    let upkeep_credit = closed.plan.upkeep_credit;
    commit(&accounts, pre_root_digest, &post_root, closed)?;
    // AFTER the lamports moved, and after the close's own receipt exists to be
    // named: the vault's credit route RECOGNIZES lamports already at the
    // address, and its `receipt_digest` is this close's receipt, so the act is
    // receipted twice and neither receipt can be written without the other.
    // The CPI also overwrites return data, so this route's own answer is set
    // last.
    receipt_upkeep_credit(
        program_id,
        &accounts,
        release_set,
        request.market,
        upkeep_credit,
        hash(&receipt).to_bytes(),
    )?;
    set_return_data(&receipt);
    Ok(())
}

/// Receipt the donation remainder in the upkeep vault, by CPI into Custody.
///
/// The lamports are already there: [`commit`] debited the Trading-owned replay
/// and credited the Custody-owned vault directly, which the runtime admits for
/// a credit. This call only makes the vault's record SAY where they came from,
/// under [`UpkeepSourceClassV1::Donation`] and the caller-authority PDA this
/// program signs with. A zero credit -- every cohort to date -- calls nothing:
/// the vault contract refuses a zero amount by name, and a close with no
/// donation has nothing to receipt.
#[inline(never)]
fn receipt_upkeep_credit(
    program_id: &Pubkey,
    accounts: &Accounts<'_, '_>,
    release_set: [u8; 32],
    market: [u8; 32],
    amount: u64,
    receipt_digest: [u8; 32],
) -> Result<(), ProgramError> {
    if amount == 0 {
        return Ok(());
    }
    let context = accounts.replay.key.to_bytes();
    let body = UpkeepRequestV1 {
        operation: UpkeepOperationV1::Credit,
        credit: Some(UpkeepCreditV1 {
            source_class: UpkeepSourceClassV1::Donation,
            caller: Some(UpkeepProtocolCallerV1 {
                caller_role: CallerRoleV1::Trading,
                release_set,
                market,
                context,
            }),
            receipt_digest,
            amount,
        }),
    }
    // ONE ACCUSATION, so one code. Every way this encode can refuse -- a zero
    // amount, a shape the class and the caller disagree about, Custody named as
    // its own caller -- is unreachable from the literal three lines above it,
    // and all three would mean the same thing if they were reachable: this
    // executable built a request the vault's own contract does not admit. There
    // is no caller mistake to distinguish here, which is the only condition
    // under which a coarse code is honest.
    .to_bytes()
    .map_err(|_| TradingSbfError::CloseMakerUpkeepVault)?;
    // The authority is derived FROM the request bytes, exactly as the vault
    // re-derives it on the other side, so the caller cannot name a PDA for one
    // request and spend it on another. Its refusal is the same accusation as
    // the encode's: a zero release set, market or context here is a release the
    // root already authenticated being zero, not a frame a caller chose.
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        release_set,
        market,
        ExecutionRoleV1::Trading,
        context,
        hash(&body).to_bytes(),
    )
    .map_err(|_| TradingSbfError::CloseMakerUpkeepVault)?;
    let (expected_authority, bump) = Pubkey::find_program_address(&seeds.as_slices(), program_id);
    if accounts.caller_authority.key != &expected_authority {
        return Err(TradingSbfError::CloseMakerUpkeepVault.into());
    }
    let instruction = Instruction {
        program_id: *accounts.custody_program.key,
        accounts: vec![
            AccountMeta::new(*accounts.upkeep_vault.key, false),
            AccountMeta::new_readonly(*accounts.caller_authority.key, true),
            AccountMeta::new_readonly(*accounts.cache.key, false),
            AccountMeta::new_readonly(*accounts.registry.key, false),
            AccountMeta::new_readonly(*accounts.trading_program.key, false),
            AccountMeta::new_readonly(*accounts.trading_programdata.key, false),
            AccountMeta::new_readonly(*accounts.rent.key, false),
        ],
        data: body.to_vec(),
    };
    let infos = [
        accounts.upkeep_vault.clone(),
        accounts.caller_authority.clone(),
        accounts.cache.clone(),
        accounts.registry.clone(),
        accounts.trading_program.clone(),
        accounts.trading_programdata.clone(),
        accounts.rent.clone(),
        accounts.custody_program.clone(),
    ];
    let bump_seed = [bump];
    let [domain, release, market_seed, role, context_seed, digest] = seeds.as_slices();
    invoke_signed(
        &instruction,
        &infos,
        &[&[
            domain,
            release,
            market_seed,
            role,
            context_seed,
            digest,
            &bump_seed,
        ]],
    )
    .map_err(crate::child_refused_v1)?;
    Ok(())
}

/// Write the decremented root, split the replay's whole balance three ways,
/// and return the replay's account to the System program.
///
/// The split is the plan's, to the lamport: the recorded owner receives the
/// principal EXACTLY, the closer receives the carve, the vault receives the
/// rest of the donation, and the replay is left at zero. The observed balance
/// is checked against the sum before anything moves, so a replay whose balance
/// changed between the plan and the commit refuses rather than stranding a
/// remainder in a closed account.
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
    let closer_reward = closed.plan.closer_reward;
    let upkeep_credit = closed.plan.upkeep_credit;
    let moved = total_credit
        .checked_add(closer_reward)
        .and_then(|value| value.checked_add(upkeep_credit))
        .ok_or(TradingSbfError::Commit)?;
    if accounts.replay.lamports() != moved {
        return Err(TradingSbfError::Commit.into());
    }
    let destination_after = accounts
        .rent_owner
        .lamports()
        .checked_add(total_credit)
        .ok_or(TradingSbfError::Commit)?;
    let closer_after = accounts
        .closer
        .lamports()
        .checked_add(closer_reward)
        .ok_or(TradingSbfError::Commit)?;
    let vault_after = accounts
        .upkeep_vault
        .lamports()
        .checked_add(upkeep_credit)
        .ok_or(TradingSbfError::Commit)?;
    {
        let mut destination_lamports = accounts
            .rent_owner
            .try_borrow_mut_lamports()
            .map_err(|_| TradingSbfError::Commit)?;
        let mut closer_lamports = accounts
            .closer
            .try_borrow_mut_lamports()
            .map_err(|_| TradingSbfError::Commit)?;
        let mut vault_lamports = accounts
            .upkeep_vault
            .try_borrow_mut_lamports()
            .map_err(|_| TradingSbfError::Commit)?;
        let mut replay_lamports = accounts
            .replay
            .try_borrow_mut_lamports()
            .map_err(|_| TradingSbfError::Commit)?;
        **destination_lamports = destination_after;
        **closer_lamports = closer_after;
        **vault_lamports = vault_after;
        **replay_lamports = 0;
    }
    accounts
        .replay
        .resize(0)
        .map_err(|_| TradingSbfError::Commit)?;
    accounts.replay.assign(&system_program::ID);
    if accounts.rent_owner.lamports() != destination_after
        || accounts.closer.lamports() != closer_after
        || accounts.upkeep_vault.lamports() != vault_after
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
    parameters: ProtocolParametersV1,
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
    let closed = close_maker_replay_v2(
        pre_root_state,
        maker_root,
        replay.lamports(),
        // The carve's share and ceiling, from the governed record this frame
        // carried and `authenticate_economics` held to its own address. Never
        // a constant in this executable: that is the whole of decision 0024's
        // amendment.
        parameters,
    )
    .map_err(|error| match error {
        SuccessorError::FeeOwedOutstanding => TradingSbfError::CloseMakerFeeOutstanding,
        SuccessorError::LiveCountInvariant => TradingSbfError::CloseMakerLiveIntents,
        SuccessorError::InvalidRootPhase | SuccessorError::MakerRootCountInvariant => {
            TradingSbfError::Transition
        }
        _ => TradingSbfError::CloseMakerReplayAccount,
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
    // RULING R1/R5's missing conjunct: the Custody program the vault credit is
    // a CPI into is the one THIS release set names, read out of the same cache
    // this frame already carries. Its ProgramData is not in the frame, so the
    // role's binding is read rather than frame-authenticated -- the discipline
    // `upkeep_vault_v1::protocol_frame` uses for the calling program, from the
    // other side of the same CPI.
    let custody = activated
        .role(ExecutionRoleV1::Custody)
        .map_err(|_| TradingSbfError::Release)?
        .release();
    if custody.program().to_bytes() != accounts.custody_program.key.to_bytes() {
        return Err(TradingSbfError::CloseMakerUpkeepVault.into());
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

/// Decision 0024's three economics coordinates, held before a lamport moves.
///
/// The governed record is READ, never assumed: its owner is the release set's
/// Custody program, its address is the one this route derives from
/// [`ProtocolParametersRecordSeedsV1`] under that program, and its bytes decode
/// through the contract's own hostile decoder. A consumer that took a value out
/// of a record it had not held to this would be applying whatever economics a
/// stranger wrote at whatever address the stranger passed.
#[inline(never)]
fn authenticate_economics(
    accounts: &Accounts<'_, '_>,
) -> Result<ProtocolParametersV1, ProgramError> {
    let expected_vault = Pubkey::find_program_address(
        &UpkeepVaultSeedsV1.as_slices(),
        accounts.custody_program.key,
    )
    .0;
    if accounts.upkeep_vault.key != &expected_vault
        || accounts.upkeep_vault.owner != accounts.custody_program.key
        || accounts.upkeep_vault.data_len() != UPKEEP_VAULT_RECORD_BYTES_V1
    {
        return Err(TradingSbfError::CloseMakerUpkeepVault.into());
    }
    // The closer is a plain empty System wallet for the reason the rent owner
    // is one: crediting a program-owned account is a write with bytes in it,
    // and this route has no authority to make one.
    if accounts.closer.owner != &system_program::ID
        || accounts.closer.executable
        || !accounts
            .closer
            .try_data_is_empty()
            .map_err(|_| TradingSbfError::CloseMakerCloser)?
    {
        return Err(TradingSbfError::CloseMakerCloser.into());
    }
    let expected_record = Pubkey::find_program_address(
        &ProtocolParametersRecordSeedsV1.as_slices(),
        accounts.custody_program.key,
    )
    .0;
    let data = accounts
        .parameters
        .try_borrow_data()
        .map_err(|_| TradingSbfError::CloseMakerParameters)?;
    authenticate_protocol_parameters_account_v1(
        accounts.parameters.owner.to_bytes(),
        accounts.parameters.key.to_bytes(),
        accounts.custody_program.key.to_bytes(),
        expected_record.to_bytes(),
        &data,
    )
    .map_err(|_| TradingSbfError::CloseMakerParameters.into())
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
