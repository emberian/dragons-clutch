//! Shared fixed-role child invocation under one authenticated Core Market.

use alloc::{boxed::Box, vec::Vec};

use dclutch_market_core_codec::{
    Admission, CoreEffectAckV1, CoreEffectEnvelopeV1, CoreState, Identity, MarketCoreStateSeedsV1,
    Request, Role, STATE_BYTES,
};
use solana_program::{
    account_info::AccountInfo,
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
};

use crate::{
    CoreSbfError,
    release::{authenticate_role, identity},
};

/// Exact common account prefix shared by fixed-role child effects.
pub const FIXED_ROLE_COMMON_ACCOUNT_COUNT_V1: usize = 8;

/// Exact common fixed-role account frame.
///
/// The account order deliberately matches the child-visible prefix. Core
/// changes only two privileges in the CPI: the derived authority becomes a
/// signer and the writable Core Market is forwarded read-only.
pub(crate) struct FixedRoleAccountsV1<'accounts, 'info> {
    accounts: &'accounts [AccountInfo<'info>],
    authority: &'accounts AccountInfo<'info>,
    market: &'accounts AccountInfo<'info>,
    cache: &'accounts AccountInfo<'info>,
    registry: &'accounts AccountInfo<'info>,
    core_program: &'accounts AccountInfo<'info>,
    core_programdata: &'accounts AccountInfo<'info>,
    target_program: &'accounts AccountInfo<'info>,
    target_programdata: &'accounts AccountInfo<'info>,
}

impl<'accounts, 'info> FixedRoleAccountsV1<'accounts, 'info> {
    /// Parse the common prefix and universal privilege contract.
    pub(crate) fn parse(
        program_id: &Pubkey,
        accounts: &'accounts [AccountInfo<'info>],
    ) -> Result<Self, CoreSbfError> {
        if accounts.len() < FIXED_ROLE_COMMON_ACCOUNT_COUNT_V1 {
            return Err(CoreSbfError::AccountFrame);
        }
        let authority = account(accounts, 0)?;
        let market = account(accounts, 1)?;
        let cache = account(accounts, 2)?;
        let registry = account(accounts, 3)?;
        let core_program = account(accounts, 4)?;
        let core_programdata = account(accounts, 5)?;
        let target_program = account(accounts, 6)?;
        let target_programdata = account(accounts, 7)?;
        if authority.is_signer
            || authority.is_writable
            || authority.executable
            || market.is_signer
            || !market.is_writable
            || market.executable
            || cache.is_signer
            || cache.is_writable
            || cache.executable
            || registry.is_signer
            || registry.is_writable
            || !registry.executable
            || core_program.key != program_id
            || core_program.is_signer
            || core_program.is_writable
            || !core_program.executable
            || core_programdata.is_signer
            || core_programdata.is_writable
            || core_programdata.executable
            || target_program.is_signer
            || target_program.is_writable
            || !target_program.executable
            || target_programdata.is_signer
            || target_programdata.is_writable
            || target_programdata.executable
        {
            return Err(CoreSbfError::AccountFrame);
        }
        Ok(Self {
            accounts,
            authority,
            market,
            cache,
            registry,
            core_program,
            core_programdata,
            target_program,
            target_programdata,
        })
    }

    pub(crate) fn authority(&self) -> &'accounts AccountInfo<'info> {
        self.authority
    }

    pub(crate) fn market(&self) -> &'accounts AccountInfo<'info> {
        self.market
    }

    pub(crate) fn cache(&self) -> &'accounts AccountInfo<'info> {
        self.cache
    }

    pub(crate) fn registry(&self) -> &'accounts AccountInfo<'info> {
        self.registry
    }

    pub(crate) fn core_program(&self) -> &'accounts AccountInfo<'info> {
        self.core_program
    }

    pub(crate) fn core_programdata(&self) -> &'accounts AccountInfo<'info> {
        self.core_programdata
    }

    pub(crate) fn target_program(&self) -> &'accounts AccountInfo<'info> {
        self.target_program
    }

    pub(crate) fn target_programdata(&self) -> &'accounts AccountInfo<'info> {
        self.target_programdata
    }

    pub(crate) fn child_accounts(
        &self,
        count: usize,
    ) -> Result<&'accounts [AccountInfo<'info>], CoreSbfError> {
        self.accounts.get(..count).ok_or(CoreSbfError::AccountFrame)
    }
}

/// Prestate and release observations authenticated before a fixed-role CPI.
pub(crate) struct AuthenticatedFixedRoleV1 {
    pub(crate) state: Box<CoreState>,
    pub(crate) state_bytes: Box<[u8; STATE_BYTES]>,
    pub(crate) core_admission: Box<Admission>,
    pub(crate) target_admission: Box<Admission>,
}

/// Authenticate one Market, current Core and target deployments, and caller PDA.
#[inline(never)]
pub(crate) fn authenticate_fixed_role(
    program_id: &Pubkey,
    frame: &FixedRoleAccountsV1<'_, '_>,
    request: Request,
    envelope: CoreEffectEnvelopeV1,
    role_request: &[u8],
    target_role: Role,
) -> Result<AuthenticatedFixedRoleV1, CoreSbfError> {
    let state_bytes = Box::new(read_market_bytes(program_id, frame.market())?);
    let state =
        Box::new(CoreState::decode(state_bytes.as_ref()).map_err(|_| CoreSbfError::Market)?);
    authenticate_market(program_id, frame.market(), *state, request)?;
    envelope
        .validate_role_request(role_request.len(), identity(hash(role_request).to_bytes())?)
        .map_err(|_| CoreSbfError::Instruction)?;
    if envelope.target_role() != target_role
        || envelope.caller_program().to_bytes() != program_id.to_bytes()
        || envelope.market() != request.market
        || envelope.release_set() != state.identity.selected_release_set
        || envelope.generation() != request.generation
        || envelope.parent_state_digest().to_bytes() != hash(state_bytes.as_ref()).to_bytes()
    {
        return Err(CoreSbfError::Instruction);
    }
    let caller_seeds = envelope
        .caller_authority_seeds()
        .map_err(|_| CoreSbfError::CallerAuthority)?;
    let expected_authority = Pubkey::find_program_address(&caller_seeds.as_slices(), program_id).0;
    if frame.authority().key != &expected_authority
        || envelope.caller_authority().to_bytes() != expected_authority.to_bytes()
    {
        return Err(CoreSbfError::CallerAuthority);
    }
    let core_admission = Box::new(authenticate_role(
        frame.cache(),
        frame.registry(),
        frame.core_program(),
        frame.core_programdata(),
        state.identity.registry_program,
        state.identity.selected_release_set.to_bytes(),
        Role::Core,
    )?);
    let target_admission = Box::new(authenticate_role(
        frame.cache(),
        frame.registry(),
        frame.target_program(),
        frame.target_programdata(),
        state.identity.registry_program,
        state.identity.selected_release_set.to_bytes(),
        target_role,
    )?);
    if core_admission.selected != target_admission.selected {
        return Err(CoreSbfError::Release);
    }
    Ok(AuthenticatedFixedRoleV1 {
        state,
        state_bytes,
        core_admission,
        target_admission,
    })
}

/// Invoke the selected child with an exact prefix of the outer account frame.
#[inline(never)]
pub(crate) fn invoke_fixed_role(
    program_id: &Pubkey,
    frame: &FixedRoleAccountsV1<'_, '_>,
    envelope: CoreEffectEnvelopeV1,
    envelope_bytes: &[u8],
    role_request: &[u8],
    child_account_count: usize,
) -> Result<(), ProgramError> {
    let child_accounts = frame.child_accounts(child_account_count)?;
    let mut data = Vec::with_capacity(envelope_bytes.len().saturating_add(role_request.len()));
    data.extend_from_slice(envelope_bytes);
    data.extend_from_slice(role_request);
    let mut metas = Vec::with_capacity(child_accounts.len());
    for (index, value) in child_accounts.iter().enumerate() {
        let signer = index == 0;
        let writable = index != 1 && value.is_writable;
        metas.push(if writable {
            AccountMeta::new(*value.key, signer)
        } else {
            AccountMeta::new_readonly(*value.key, signer)
        });
    }
    let instruction = Instruction {
        program_id: *frame.target_program().key,
        accounts: metas,
        data,
    };
    let caller_seeds = envelope
        .caller_authority_seeds()
        .map_err(|_| CoreSbfError::CallerAuthority)?;
    let (_, bump) = Pubkey::find_program_address(&caller_seeds.as_slices(), program_id);
    let [domain, release, market, role, context, request_digest] = caller_seeds.as_slices();
    let bump_seed = [bump];
    let signer: [&[u8]; 7] = [
        domain,
        release,
        market,
        role,
        context,
        request_digest,
        &bump_seed,
    ];
    let mut infos = Vec::with_capacity(child_accounts.len().saturating_add(1));
    infos.extend(child_accounts.iter().cloned());
    infos.push(frame.target_program().clone());
    invoke_signed(&instruction, &infos, &[&signer]).map_err(|_| CoreSbfError::ChildCpi.into())
}

/// Decode and authenticate the immediate child acknowledgment.
#[inline(never)]
pub(crate) fn authenticate_fixed_role_ack(
    frame: &FixedRoleAccountsV1<'_, '_>,
    envelope: CoreEffectEnvelopeV1,
    envelope_bytes: &[u8],
    role_request: &[u8],
) -> Result<CoreEffectAckV1, CoreSbfError> {
    let (producer, bytes) = get_return_data().ok_or(CoreSbfError::ChildAck)?;
    if producer != *frame.target_program().key {
        return Err(CoreSbfError::ChildAck);
    }
    let acknowledgement = CoreEffectAckV1::decode(&bytes).map_err(|_| CoreSbfError::ChildAck)?;
    let envelope_len = u32::try_from(envelope_bytes.len()).map_err(|_| CoreSbfError::Arithmetic)?;
    let role_len = u32::try_from(role_request.len()).map_err(|_| CoreSbfError::Arithmetic)?;
    let full_effect_digest = identity(
        hashv(&[
            &dclutch_market_core_codec::CORE_EFFECT_DIGEST_DOMAIN_V1,
            &envelope_len.to_le_bytes(),
            envelope_bytes,
            &role_len.to_le_bytes(),
            role_request,
        ])
        .to_bytes(),
    )?;
    acknowledgement
        .validate_for(
            envelope,
            identity(frame.target_program().key.to_bytes())?,
            full_effect_digest,
        )
        .map_err(|_| CoreSbfError::ChildAck)?;
    Ok(acknowledgement)
}

/// Require that a read-only child could not alter the Core Market prestate.
pub(crate) fn require_market_unchanged(
    frame: &FixedRoleAccountsV1<'_, '_>,
    expected: &[u8; STATE_BYTES],
) -> Result<(), CoreSbfError> {
    let observed = frame
        .market()
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Market)?;
    if observed.as_ref() != expected {
        return Err(CoreSbfError::Market);
    }
    Ok(())
}

/// Persist one already validated Core candidate after every child postcheck.
pub(crate) fn persist_state(
    market: &AccountInfo<'_>,
    state: CoreState,
) -> Result<(), ProgramError> {
    let bytes = state.encode().map_err(|_| CoreSbfError::Commit)?;
    let mut data = market
        .try_borrow_mut_data()
        .map_err(|_| CoreSbfError::Commit)?;
    if data.len() != STATE_BYTES {
        return Err(CoreSbfError::Commit.into());
    }
    data.copy_from_slice(&bytes);
    if CoreState::decode(&data) != Ok(state) {
        return Err(CoreSbfError::Commit.into());
    }
    Ok(())
}

pub(crate) fn read_market_bytes(
    program_id: &Pubkey,
    market: &AccountInfo<'_>,
) -> Result<[u8; STATE_BYTES], CoreSbfError> {
    if market.owner != program_id || market.data_len() != STATE_BYTES {
        return Err(CoreSbfError::Market);
    }
    let data = market.try_borrow_data().map_err(|_| CoreSbfError::Market)?;
    data.as_ref().try_into().map_err(|_| CoreSbfError::Market)
}

pub(crate) fn authenticate_market(
    program_id: &Pubkey,
    market: &AccountInfo<'_>,
    state: CoreState,
    request: Request,
) -> Result<(), CoreSbfError> {
    if request.market != state.identity.market_id
        || request.generation != state.identity.generation
        || market.key.to_bytes() != state.identity.market_id.to_bytes()
    {
        return Err(CoreSbfError::Market);
    }
    let seeds = MarketCoreStateSeedsV1::new(state.identity);
    if Pubkey::find_program_address(&seeds.as_slices(), program_id).0 != *market.key {
        return Err(CoreSbfError::Market);
    }
    Ok(())
}

fn account<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<&'accounts AccountInfo<'info>, CoreSbfError> {
    accounts.get(index).ok_or(CoreSbfError::AccountFrame)
}

pub(crate) fn nonzero_identity(bytes: [u8; 32]) -> Result<Identity, CoreSbfError> {
    Identity::new(bytes).map_err(|_| CoreSbfError::Reference)
}
