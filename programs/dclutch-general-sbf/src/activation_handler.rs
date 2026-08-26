//! Reusable General capability activation handler for the canonical Trading controller.
//!
//! The standalone General entrypoint is only a measurement wrapper. Release
//! admission for the implementation family belongs to the shared Trading
//! controller; this handler independently authenticates the Core-signed effect
//! and owns only General root/funding semantics.

use dclutch_capability_contract::{
    CapabilityFundingDerivationV1, CapabilityManifestV1, ContentId, FUNDING_STATE_BYTES,
    FundingCustodyObservationV1, FundingStateV1,
};
use dclutch_general_config_contract::{
    GENERAL_CONFIG_BYTES_V2, GENERAL_ROOT_BYTES_V2, GENERAL_ROOT_PDA_DOMAIN_V2,
    GeneralActivationDispositionV2, GeneralActivationRequestV2, GeneralConfigV2, GeneralRootV2,
    activate_general_owned_v2,
};
use dclutch_market_core_codec::{
    CORE_EFFECT_DIGEST_DOMAIN_V1, CORE_EFFECT_ENVELOPE_BYTES_V1, CoreEffectAckV1,
    CoreEffectActionV1, CoreEffectEnvelopeV1, Identity, Role,
};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    hash::{hash, hashv},
    program::{invoke_signed, set_return_data},
    program_error::ProgramError,
    pubkey::Pubkey,
};
use solana_sdk_ids::system_program;
use solana_system_interface::instruction::{allocate, assign, transfer as system_transfer};

use crate::{ACTIVATE_ACCOUNT_COUNT_V2, ACTIVATE_INSTRUCTION_BYTES_V2, GeneralSbfError};

const AUTHORITY: usize = 0;
const CORE_PROGRAM: usize = 1;
const ROOT: usize = 2;
const CONFIG: usize = 3;
const MANIFEST: usize = 4;
const FUNDING: usize = 5;
const RENT_CREDIT: usize = 6;
const SYSTEM: usize = 7;
const ACTIVATION_POSTSTATE_DOMAIN_V2: &[u8] = b"dclutch/general/activation-poststate/v2";

/// Execute one Core-authenticated General activation or exact replay.
#[inline(never)]
pub(crate) fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if accounts.len() != ACTIVATE_ACCOUNT_COUNT_V2
        || instruction_data.len() != ACTIVATE_INSTRUCTION_BYTES_V2
    {
        return Err(GeneralSbfError::AccountFrame.into());
    }
    let envelope_bytes = instruction_data
        .get(..CORE_EFFECT_ENVELOPE_BYTES_V1)
        .ok_or(GeneralSbfError::Instruction)?;
    let request_bytes = instruction_data
        .get(CORE_EFFECT_ENVELOPE_BYTES_V1..)
        .ok_or(GeneralSbfError::Instruction)?;
    let envelope =
        CoreEffectEnvelopeV1::decode(envelope_bytes).map_err(|_| GeneralSbfError::Instruction)?;
    let request = GeneralActivationRequestV2::decode(request_bytes)
        .map_err(|_| GeneralSbfError::Instruction)?;
    let request_digest = identity(hash(request_bytes).to_bytes())?;
    envelope
        .validate_role_request(request_bytes.len(), request_digest)
        .map_err(|_| GeneralSbfError::RootActivation)?;
    validate_frame(program_id, accounts, envelope, request)?;

    let config = decode_config(accounts, request)?;
    if config.generation() != envelope.generation() {
        return Err(GeneralSbfError::RootActivation.into());
    }
    let manifest_id =
        ContentId::new(request.manifest_id()).map_err(|_| GeneralSbfError::RootActivation)?;
    let config_id =
        ContentId::new(request.config_id()).map_err(|_| GeneralSbfError::RootActivation)?;
    let funding = decode_funding(accounts)?;
    let market = envelope.market().to_bytes();
    let generation = envelope.generation().to_le_bytes();
    let (expected_root, root_bump) = Pubkey::find_program_address(
        &[
            GENERAL_ROOT_PDA_DOMAIN_V2,
            &market,
            &generation,
            &request.config_id(),
        ],
        program_id,
    );
    if accounts[ROOT].key != &expected_root || request.root() != expected_root.to_bytes() {
        return Err(GeneralSbfError::RootActivation.into());
    }
    let existing_root = decode_existing_root(program_id, &accounts[ROOT])?;
    let custody = FundingCustodyObservationV1::native_only(
        accounts[FUNDING].lamports(),
        request.exact_funding_rent_lamports(),
    )
    .map_err(|_| GeneralSbfError::RootActivation)?;
    let plan = {
        let manifest_bytes = accounts[MANIFEST]
            .try_borrow_data()
            .map_err(|_| GeneralSbfError::Borrow)?;
        if hash(&manifest_bytes).to_bytes() != request.manifest_id() {
            return Err(GeneralSbfError::RootActivation.into());
        }
        let manifest = CapabilityManifestV1::decode(&manifest_bytes)
            .map_err(|_| GeneralSbfError::RootActivation)?;
        let derivation = CapabilityFundingDerivationV1::new(
            market,
            envelope.generation(),
            manifest_id,
            manifest,
            funding,
        )
        .map_err(|_| GeneralSbfError::RootActivation)?;
        let (expected_funding, _) =
            Pubkey::find_program_address(&derivation.seed_components(), program_id);
        if accounts[FUNDING].key != &expected_funding
            || request.funding_state() != expected_funding.to_bytes()
        {
            return Err(GeneralSbfError::RootActivation.into());
        }
        activate_general_owned_v2(
            market,
            envelope.generation(),
            manifest_id,
            manifest,
            request.entry_index(),
            config_id,
            config,
            funding,
            custody,
            request.current_slot(),
            request.exact_root_rent_lamports(),
            accounts[ROOT].lamports(),
            existing_root,
        )
        .map_err(|_| GeneralSbfError::RootActivation)?
    };

    match plan.disposition() {
        GeneralActivationDispositionV2::Create => commit_create(
            program_id,
            accounts,
            request,
            market,
            generation,
            root_bump,
            plan.root(),
            plan.funding_after(),
            plan.creation(),
        )?,
        GeneralActivationDispositionV2::Idempotent => {
            if accounts[ROOT].lamports() != request.exact_root_rent_lamports() {
                return Err(GeneralSbfError::RootActivation.into());
            }
        }
    }

    let acknowledgement = activation_ack(
        program_id,
        envelope,
        envelope_bytes,
        request_bytes,
        plan.root(),
        plan.funding_after(),
    )?;
    set_return_data(
        &acknowledgement
            .encode()
            .map_err(|_| GeneralSbfError::Commit)?,
    );
    Ok(())
}

fn decode_config(
    accounts: &[AccountInfo<'_>],
    request: GeneralActivationRequestV2,
) -> Result<GeneralConfigV2, ProgramError> {
    let config_bytes = accounts[CONFIG]
        .try_borrow_data()
        .map_err(|_| GeneralSbfError::Borrow)?;
    if config_bytes.len() != GENERAL_CONFIG_BYTES_V2
        || hash(&config_bytes).to_bytes() != request.config_id()
    {
        return Err(GeneralSbfError::RootActivation.into());
    }
    GeneralConfigV2::decode(&config_bytes).map_err(|_| GeneralSbfError::RootActivation.into())
}

fn decode_funding(accounts: &[AccountInfo<'_>]) -> Result<FundingStateV1, ProgramError> {
    let funding_bytes = accounts[FUNDING]
        .try_borrow_data()
        .map_err(|_| GeneralSbfError::Borrow)?;
    FundingStateV1::decode(&funding_bytes).map_err(|_| GeneralSbfError::RootActivation.into())
}

fn decode_existing_root(
    program_id: &Pubkey,
    root: &AccountInfo<'_>,
) -> Result<Option<GeneralRootV2>, ProgramError> {
    if root.owner == program_id {
        let root_bytes = root
            .try_borrow_data()
            .map_err(|_| GeneralSbfError::Borrow)?;
        return GeneralRootV2::decode(&root_bytes)
            .map(Some)
            .map_err(|_| GeneralSbfError::RootActivation.into());
    }
    if root.owner == &system_program::ID && root.data_len() == 0 {
        Ok(None)
    } else {
        Err(GeneralSbfError::RootActivation.into())
    }
}

#[allow(clippy::too_many_arguments)]
fn commit_create(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: GeneralActivationRequestV2,
    market: [u8; 32],
    generation: [u8; 8],
    root_bump: u8,
    root: GeneralRootV2,
    funding_after: FundingStateV1,
    creation: dclutch_general_config_contract::DustSafeRootCreationV2,
) -> ProgramResult {
    let top_up = creation.funding_top_up_lamports();
    let displaced = creation.displaced_prepaid_lamports();
    let surplus = creation.unsolicited_surplus_lamports();
    let funding_debit = top_up
        .checked_add(displaced)
        .ok_or(GeneralSbfError::Commit)?;
    let funding_after_lamports = accounts[FUNDING]
        .lamports()
        .checked_sub(funding_debit)
        .ok_or(GeneralSbfError::Commit)?;
    let semantic_funding_after = request
        .exact_funding_rent_lamports()
        .checked_add(funding_after.remaining().native_lamports_total())
        .ok_or(GeneralSbfError::Commit)?;
    if funding_after_lamports != semantic_funding_after {
        return Err(GeneralSbfError::Commit.into());
    }
    let root_after = accounts[ROOT]
        .lamports()
        .checked_add(top_up)
        .and_then(|amount| amount.checked_sub(surplus))
        .ok_or(GeneralSbfError::Commit)?;
    if root_after != request.exact_root_rent_lamports() {
        return Err(GeneralSbfError::Commit.into());
    }
    let rent_credit_after = accounts[RENT_CREDIT]
        .lamports()
        .checked_add(displaced)
        .and_then(|amount| amount.checked_add(surplus))
        .ok_or(GeneralSbfError::Commit)?;

    let bump_seed = [root_bump];
    let config_id = request.config_id();
    let signer = [
        GENERAL_ROOT_PDA_DOMAIN_V2,
        market.as_slice(),
        generation.as_slice(),
        config_id.as_slice(),
        bump_seed.as_slice(),
    ];
    if surplus != 0 {
        invoke_signed(
            &system_transfer(accounts[ROOT].key, accounts[RENT_CREDIT].key, surplus),
            &[
                accounts[ROOT].clone(),
                accounts[RENT_CREDIT].clone(),
                accounts[SYSTEM].clone(),
            ],
            &[&signer],
        )
        .map_err(|_| GeneralSbfError::Commit)?;
    }
    set_lamports(&accounts[FUNDING], funding_after_lamports)?;
    set_lamports(&accounts[ROOT], root_after)?;
    set_lamports(&accounts[RENT_CREDIT], rent_credit_after)?;

    let root_space = u64::try_from(GENERAL_ROOT_BYTES_V2).map_err(|_| GeneralSbfError::Commit)?;
    invoke_signed(
        &allocate(accounts[ROOT].key, root_space),
        &[accounts[ROOT].clone(), accounts[SYSTEM].clone()],
        &[&signer],
    )
    .map_err(|_| GeneralSbfError::Commit)?;
    invoke_signed(
        &assign(accounts[ROOT].key, program_id),
        &[accounts[ROOT].clone(), accounts[SYSTEM].clone()],
        &[&signer],
    )
    .map_err(|_| GeneralSbfError::Commit)?;
    if accounts[ROOT].owner != program_id || accounts[ROOT].data_len() != GENERAL_ROOT_BYTES_V2 {
        return Err(GeneralSbfError::Commit.into());
    }
    let mut root_bytes = accounts[ROOT]
        .try_borrow_mut_data()
        .map_err(|_| GeneralSbfError::Borrow)?;
    if root_bytes.iter().any(|byte| *byte != 0) {
        return Err(GeneralSbfError::Commit.into());
    }
    root_bytes.copy_from_slice(&root.to_bytes());
    drop(root_bytes);
    accounts[FUNDING]
        .try_borrow_mut_data()
        .map_err(|_| GeneralSbfError::Borrow)?
        .copy_from_slice(&funding_after.to_bytes());
    Ok(())
}

fn set_lamports(account: &AccountInfo<'_>, value: u64) -> ProgramResult {
    let mut lamports = account
        .try_borrow_mut_lamports()
        .map_err(|_| GeneralSbfError::Borrow)?;
    **lamports = value;
    Ok(())
}

pub(crate) fn activation_ack(
    program_id: &Pubkey,
    envelope: CoreEffectEnvelopeV1,
    envelope_bytes: &[u8],
    request_bytes: &[u8],
    root: GeneralRootV2,
    funding: FundingStateV1,
) -> Result<CoreEffectAckV1, ProgramError> {
    let envelope_length = u32::try_from(envelope_bytes.len())
        .map_err(|_| GeneralSbfError::Instruction)?
        .to_le_bytes();
    let request_length = u32::try_from(request_bytes.len())
        .map_err(|_| GeneralSbfError::Instruction)?
        .to_le_bytes();
    let effect_digest = identity(
        hashv(&[
            &CORE_EFFECT_DIGEST_DOMAIN_V1,
            &envelope_length,
            envelope_bytes,
            &request_length,
            request_bytes,
        ])
        .to_bytes(),
    )?;
    let root_bytes = root.to_bytes();
    let funding_bytes = funding.to_bytes();
    let poststate_digest =
        identity(hashv(&[ACTIVATION_POSTSTATE_DOMAIN_V2, &root_bytes, &funding_bytes]).to_bytes())?;
    CoreEffectAckV1::new(
        envelope.action(),
        envelope.target_role(),
        identity(program_id.to_bytes())?,
        envelope.release_set(),
        envelope.market(),
        envelope.context(),
        effect_digest,
        poststate_digest,
        0,
        root.revision(),
        0,
        1,
    )
    .map_err(|_| GeneralSbfError::Commit.into())
}

fn validate_frame(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    envelope: CoreEffectEnvelopeV1,
    request: GeneralActivationRequestV2,
) -> ProgramResult {
    if !accounts[AUTHORITY].is_signer
        || accounts[AUTHORITY].is_writable
        || accounts[AUTHORITY].executable
        || !accounts[CORE_PROGRAM].executable
        || accounts[CORE_PROGRAM].is_signer
        || accounts[CORE_PROGRAM].is_writable
        || !accounts[ROOT].is_writable
        || accounts[ROOT].is_signer
        || accounts[ROOT].executable
        || accounts[CONFIG].is_writable
        || accounts[CONFIG].is_signer
        || accounts[CONFIG].executable
        || accounts[MANIFEST].is_writable
        || accounts[MANIFEST].is_signer
        || accounts[MANIFEST].executable
        || accounts[FUNDING].owner != program_id
        || accounts[FUNDING].data_len() != FUNDING_STATE_BYTES
        || !accounts[FUNDING].is_writable
        || accounts[FUNDING].is_signer
        || accounts[FUNDING].executable
        || !accounts[RENT_CREDIT].is_writable
        || accounts[RENT_CREDIT].is_signer
        || accounts[RENT_CREDIT].executable
        || accounts[SYSTEM].key != &system_program::ID
        || !accounts[SYSTEM].executable
        || accounts[SYSTEM].is_signer
        || accounts[SYSTEM].is_writable
        || accounts.iter().enumerate().any(|(left, account)| {
            accounts
                .iter()
                .skip(left + 1)
                .any(|other| other.key == account.key)
        })
    {
        return Err(GeneralSbfError::AccountFrame.into());
    }
    if envelope.action() != CoreEffectActionV1::ActivateCapability
        || envelope.target_role() != Role::Trading
        || envelope.caller_program().to_bytes() != accounts[CORE_PROGRAM].key.to_bytes()
        || envelope.caller_authority().to_bytes() != accounts[AUTHORITY].key.to_bytes()
        || envelope.expected_resource_a_revision() != 0
        || envelope.expected_resource_b_revision() != 0
        || request.root() != accounts[ROOT].key.to_bytes()
        || request.funding_state() != accounts[FUNDING].key.to_bytes()
        || request.rent_credit() != accounts[RENT_CREDIT].key.to_bytes()
        || program_id == accounts[CORE_PROGRAM].key
    {
        return Err(GeneralSbfError::RootActivation.into());
    }
    let authority_seeds = envelope
        .caller_authority_seeds()
        .map_err(|_| GeneralSbfError::ReleaseAdmission)?;
    let (expected_authority, _) =
        Pubkey::find_program_address(&authority_seeds.as_slices(), accounts[CORE_PROGRAM].key);
    if accounts[AUTHORITY].key != &expected_authority {
        return Err(GeneralSbfError::ReleaseAdmission.into());
    }
    Ok(())
}

fn identity(bytes: [u8; 32]) -> Result<Identity, ProgramError> {
    Identity::new(bytes).map_err(|_| GeneralSbfError::RootActivation.into())
}
