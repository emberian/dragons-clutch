//! The capability seal: Trading's write-once artifact prologue and its reader.
//!
//! Decision 0005. Split out of `hot_v3` unchanged as the first step of the
//! DECOMP palimpsest decomposition -- the seal is a self-contained surface
//! (one permissionless instruction that writes a verdict, plus the three
//! readers the hot path uses to spend it) that had no reason to sit inside an
//! 11,588-line execution module. Every item below is byte-for-byte what
//! `hot_v3` held; the gate on this move is a byte-identical shipped ELF.

use dclutch_account_profile_contract::{
    lifecycle_v3::StateLifecyclePolicyV5,
    v2::{AccountProfileV2, SCHEMA_RELEASE_ID as ACCOUNT_PROFILE_SCHEMA_ID_V2},
};
use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1,
    hot_v3::{HOT_FIXED_ACCOUNT_COUNT_V3, HotExecutionEnvelopeV3},
    v4::{
        CAPABILITY_PROGRAM_V4_BYTES, SCHEMA_RELEASE_ID as PROGRAM_SCHEMA_ID_V4,
        SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5,
    },
};
use dclutch_capability_seal_contract::{
    CAPABILITY_SEAL_BYTES_V1, CAPABILITY_SEAL_ROW_COUNT_V1, CapabilitySealKeyV1,
    CapabilitySealRequestV1, SealedArtifactV1, SealedDescriptorClosureV1, SealedRecordRowV1,
    SealedRoleV1,
};
use dclutch_transition_vm::v3::{
    ProgramV3 as TransitionProgramV3, SCHEMA_RELEASE_ID as TRANSITION_SCHEMA_ID_V3,
};
use solana_program::{
    account_info::AccountInfo,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::system_program;
use solana_system_interface::instruction::{allocate, assign, transfer as system_transfer};

use crate::TradingSbfError;

use super::{
    HotFrameV3, HotRoleAuthenticationV3, StaticRegisterOwnershipV5, account,
    authenticate_market_boxed_v3, authenticate_root_boxed_v3, borrow_finalized_record,
    decode_capability_program_boxed_v3, decode_request_profile, decode_selected_effect_v4,
    require_static_register_ownership_v5,
};

/// First account after the fixed hot prefix on the seal outer: the rent payer.
pub const SEAL_PAYER_ACCOUNT_V1: usize = HOT_FIXED_ACCOUNT_COUNT_V3;
/// System Program on the seal outer.
pub const SEAL_SYSTEM_PROGRAM_ACCOUNT_V1: usize = SEAL_PAYER_ACCOUNT_V1 + 1;
/// Exact account count of the seal outer.
pub const SEAL_ACCOUNT_COUNT_V1: usize = SEAL_SYSTEM_PROGRAM_ACCOUNT_V1 + 1;

/// Write one validated-artifact seal for a descriptor closure and action.
///
/// Decision 0005. This is the hot path's own artifact prologue, run once and
/// persisted. Every validator it calls is the very function the hot path calls
/// without a seal -- `CapabilityProgramV4::decode`,
/// `StateLifecyclePolicyV5::decode_selected`, `AccountProfileV2::decode`,
/// `decode_request_profile`, `TransitionProgramV3::decode`,
/// `decode_selected_effect_v4`, `validate_account_profile_join` and
/// `require_static_register_ownership_v5` -- so the persisted verdict is a
/// memoisation of this executable's own answer and not a second opinion.
///
/// The act is permissionless because its output is a pure function of immutable
/// public bytes: the only freedom a caller has is whether a seal exists and
/// when. It is write-once: an already-sealed address refuses rather than being
/// rewritten, so nothing can replace a verdict once one is recorded.
pub fn process_capability_seal_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    let request =
        CapabilitySealRequestV1::decode(instruction_data).map_err(|_| TradingSbfError::Content)?;
    if accounts.len() != SEAL_ACCOUNT_COUNT_V1 {
        return Err(TradingSbfError::Content.into());
    }
    let payer = account(accounts, SEAL_PAYER_ACCOUNT_V1)?;
    let system = account(accounts, SEAL_SYSTEM_PROGRAM_ACCOUNT_V1)?;
    if !payer.is_signer
        || !payer.is_writable
        || payer.executable
        || system.key != &system_program::ID
        || !system.executable
    {
        return Err(TradingSbfError::Content.into());
    }
    let frame = HotFrameV3::parse_seal(program_id, accounts)?;
    let rent = Rent::from_account_info(frame.rent).map_err(|_| TradingSbfError::Content)?;

    // The Market and the capability root are authenticated exactly as a hot
    // action authenticates them, because the only fact this act needs from them
    // is the one a hot action will re-derive: the Registry the Market selected
    // and the Trading interpreter release currently bound to it. The envelope
    // is reconstructed from the root's own immutable header, whose seeds bind
    // it to the root address under this Program.
    let root_header = {
        let bytes = frame
            .root
            .try_borrow_data()
            .map_err(|_| TradingSbfError::Root)?;
        CapabilityRootHeaderV1::decode(
            bytes
                .get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
                .ok_or(TradingSbfError::Root)?,
        )
        .map_err(|_| TradingSbfError::Root)?
    };
    let envelope = HotExecutionEnvelopeV3::new(
        u32::try_from(instruction_data.len()).map_err(|_| TradingSbfError::Content)?,
        root_header.release_set().to_bytes(),
        root_header.market(),
        root_header.generation(),
        [0xff; 32],
    )
    .map_err(|_| TradingSbfError::Content)?;
    let market = authenticate_market_boxed_v3(&frame, envelope)?;
    let root = authenticate_root_boxed_v3(
        program_id,
        &frame,
        envelope,
        &market,
        HotRoleAuthenticationV3::ReauthenticateRegistry,
    )?;

    let key = CapabilitySealKeyV1::new(
        PROGRAM_SCHEMA_ID_V4,
        request.descriptor_digest(),
        request.action(),
        root.trading_semantic_release,
        frame.registry.key.to_bytes(),
    )
    .map_err(|_| TradingSbfError::Content)?;
    // Write-once: an existing seal is never replaced, so a recorded verdict
    // cannot be swapped for another and a griefer cannot poison the address.
    let seeds = key.seeds();
    let base = seeds.as_slices();
    let (expected, bump) = Pubkey::find_program_address(&base, program_id);
    let seal = frame.capability_seal;
    if seal.key != &expected
        || seal.owner != &system_program::ID
        || seal.data_len() != 0
        || seal.executable
        || !seal.is_writable
        || seal.is_signer
    {
        return Err(TradingSbfError::Content.into());
    }

    let rows = validate_descriptor_closure_v1(&frame, &rent, key, request.action())?;

    let space = u64::try_from(CAPABILITY_SEAL_BYTES_V1).map_err(|_| TradingSbfError::Commit)?;
    let minimum = rent.minimum_balance(CAPABILITY_SEAL_BYTES_V1);
    let deficit = minimum.saturating_sub(seal.lamports());
    if deficit > 0 {
        invoke(
            &system_transfer(payer.key, seal.key, deficit),
            &[payer.clone(), seal.clone(), system.clone()],
        )
        .map_err(|_| TradingSbfError::Commit)?;
    }
    let bump_seed = [bump];
    let signer = [
        base[0], base[1], base[2], base[3], base[4], base[5], &bump_seed,
    ];
    invoke_signed(
        &allocate(seal.key, space),
        &[seal.clone(), system.clone()],
        &[&signer],
    )
    .map_err(|_| TradingSbfError::Commit)?;
    invoke_signed(
        &assign(seal.key, program_id),
        &[seal.clone(), system.clone()],
        &[&signer],
    )
    .map_err(|_| TradingSbfError::Commit)?;
    let mut data = seal
        .try_borrow_mut_data()
        .map_err(|_| TradingSbfError::Commit)?;
    if data.len() != CAPABILITY_SEAL_BYTES_V1 {
        return Err(TradingSbfError::Commit.into());
    }
    SealedDescriptorClosureV1::encode(key, rows, &mut data).map_err(|_| TradingSbfError::Commit)?;
    Ok(())
}

/// Run the complete artifact conjunction a hot action would run, once.
///
/// Returns the canonical rows the verdict is recorded as. Every record borrow
/// ends with this call; nothing it decodes outlives it.
#[inline(never)]
fn validate_descriptor_closure_v1<'info>(
    frame: &HotFrameV3<'_, 'info>,
    rent: &Rent,
    key: CapabilitySealKeyV1,
    action: u32,
) -> Result<[SealedRecordRowV1; CAPABILITY_SEAL_ROW_COUNT_V1], ProgramError> {
    let descriptor_data = borrow_finalized_record(
        *frame,
        frame.descriptor_raw,
        frame.descriptor_staging,
        rent,
        PROGRAM_SCHEMA_ID_V4,
        key.descriptor_digest(),
    )?;
    if descriptor_data.len() != CAPABILITY_PROGRAM_V4_BYTES {
        return Err(TradingSbfError::Content.into());
    }
    let descriptor = decode_capability_program_boxed_v3(&descriptor_data)?;

    let lifecycle_data = borrow_finalized_record(
        *frame,
        frame.lifecycle_raw,
        frame.lifecycle_staging,
        rent,
        descriptor.lifecycle().schema().to_bytes(),
        descriptor.lifecycle().program().to_bytes(),
    )?;
    if descriptor.lifecycle().schema().to_bytes() != SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5
        || descriptor.derivation_policy() != descriptor.lifecycle().program()
    {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    let selected_lifecycle = descriptor.lifecycle().program().to_bytes();
    let lifecycle = StateLifecyclePolicyV5::decode_selected(
        selected_lifecycle,
        selected_lifecycle,
        &lifecycle_data,
    )
    .map_err(|_| TradingSbfError::Content)?;

    let account_profile_data = borrow_finalized_record(
        *frame,
        frame.account_profile_raw,
        frame.account_profile_staging,
        rent,
        descriptor.account_profile().schema().to_bytes(),
        descriptor.account_profile().program().to_bytes(),
    )?;
    if descriptor.account_profile().schema().to_bytes() != ACCOUNT_PROFILE_SCHEMA_ID_V2 {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    let account_profile =
        AccountProfileV2::decode(&account_profile_data).map_err(|_| TradingSbfError::Content)?;
    lifecycle
        .validate_account_profile_join(account_profile)
        .map_err(|_| TradingSbfError::Content)?;

    let request_profile_data = borrow_finalized_record(
        *frame,
        frame.request_profile_raw,
        frame.request_profile_staging,
        rent,
        descriptor.request_profile().schema().to_bytes(),
        descriptor.request_profile().program().to_bytes(),
    )?;
    let request_profile = decode_request_profile(*descriptor, &request_profile_data)?;

    let transition_data = borrow_finalized_record(
        *frame,
        frame.transition_raw,
        frame.transition_staging,
        rent,
        descriptor.transition().schema().to_bytes(),
        descriptor.transition().program().to_bytes(),
    )?;
    if descriptor.transition().schema().to_bytes() != TRANSITION_SCHEMA_ID_V3 {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    let transition =
        TransitionProgramV3::decode(&transition_data).map_err(|_| TradingSbfError::Content)?;

    let effect_data = borrow_finalized_record(
        *frame,
        frame.effect_raw,
        frame.effect_staging,
        rent,
        descriptor.effect().schema().to_bytes(),
        descriptor.effect().program().to_bytes(),
    )?;
    // Decoded for its verdict only; the seal records that this executable
    // accepted these bytes, not the view it built from them.
    let _ = decode_selected_effect_v4(descriptor.effect().schema().to_bytes(), &effect_data)?;

    require_static_register_ownership_v5(StaticRegisterOwnershipV5 {
        account_profile,
        policy: lifecycle,
        action,
        request: request_profile,
        transition,
    })?;

    Ok([
        seal_row_v1(
            SealedRoleV1::Descriptor,
            PROGRAM_SCHEMA_ID_V4,
            key.descriptor_digest(),
            descriptor_data.len(),
            frame.descriptor_raw,
            frame.descriptor_staging,
        )?,
        seal_row_v1(
            SealedRoleV1::LifecyclePolicy,
            descriptor.lifecycle().schema().to_bytes(),
            descriptor.lifecycle().program().to_bytes(),
            lifecycle_data.len(),
            frame.lifecycle_raw,
            frame.lifecycle_staging,
        )?,
        seal_row_v1(
            SealedRoleV1::AccountProfile,
            descriptor.account_profile().schema().to_bytes(),
            descriptor.account_profile().program().to_bytes(),
            account_profile_data.len(),
            frame.account_profile_raw,
            frame.account_profile_staging,
        )?,
        seal_row_v1(
            SealedRoleV1::RequestProfile,
            descriptor.request_profile().schema().to_bytes(),
            descriptor.request_profile().program().to_bytes(),
            request_profile_data.len(),
            frame.request_profile_raw,
            frame.request_profile_staging,
        )?,
        seal_row_v1(
            SealedRoleV1::TransitionProgram,
            descriptor.transition().schema().to_bytes(),
            descriptor.transition().program().to_bytes(),
            transition_data.len(),
            frame.transition_raw,
            frame.transition_staging,
        )?,
        seal_row_v1(
            SealedRoleV1::EffectProgram,
            descriptor.effect().schema().to_bytes(),
            descriptor.effect().program().to_bytes(),
            effect_data.len(),
            frame.effect_raw,
            frame.effect_staging,
        )?,
    ])
}

/// Record one row from the accounts `borrow_finalized_record` just authenticated.
#[allow(clippy::too_many_arguments)]
fn seal_row_v1(
    role: SealedRoleV1,
    schema: [u8; 32],
    digest: [u8; 32],
    width: usize,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
) -> Result<SealedRecordRowV1, ProgramError> {
    SealedRecordRowV1::new(
        role,
        u32::try_from(width).map_err(|_| TradingSbfError::Content)?,
        schema,
        digest,
        raw.key.to_bytes(),
        staging.key.to_bytes(),
    )
    .map_err(|_| TradingSbfError::Content.into())
}

/// Authenticate the Trading validated-artifact seal for one selected action.
///
/// Decision 0005. This proves the seal account is the canonical PDA for the
/// exact descriptor, action, authenticated Trading interpreter release and
/// Market-selected Registry, is owned by this Program, is read-only and
/// rent-exempt at its exact width, and carries a canonical body that agrees
/// with that derivation. It consumes nothing from the seal; every artifact the
/// seal names is still bound to its own digest, live, by
/// `borrow_finalized_record`.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
pub(super) fn authenticate_capability_seal_v3<'a>(
    program_id: &Pubkey,
    frame: HotFrameV3<'_, '_>,
    rent: &Rent,
    descriptor_schema: [u8; 32],
    descriptor_digest: [u8; 32],
    action: u32,
    trading_semantic_release: [u8; 32],
    bytes: &'a [u8],
) -> Result<SealedDescriptorClosureV1<'a>, ProgramError> {
    let key = CapabilitySealKeyV1::new(
        descriptor_schema,
        descriptor_digest,
        action,
        trading_semantic_release,
        frame.registry.key.to_bytes(),
    )
    .map_err(|_| TradingSbfError::Content)?;
    let seal = frame.capability_seal;
    let expected = Pubkey::find_program_address(&key.seeds().as_slices(), program_id).0;
    if seal.key != &expected
        || seal.owner != program_id
        || seal.is_signer
        || seal.is_writable
        || seal.executable
        || seal.data_len() != CAPABILITY_SEAL_BYTES_V1
        || bytes.len() != CAPABILITY_SEAL_BYTES_V1
        || !rent.is_exempt(seal.lamports(), CAPABILITY_SEAL_BYTES_V1)
    {
        return Err(TradingSbfError::Content.into());
    }
    let closure = SealedDescriptorClosureV1::decode(bytes).map_err(|_| TradingSbfError::Content)?;
    closure
        .require_key(key)
        .map_err(|_| TradingSbfError::Content)?;
    Ok(closure)
}

/// Borrow one live raw record against the finalized coordinates a Trading seal
/// derived and persisted.
///
/// Seal materialization authenticated the real raw/staging pair under the
/// Market-selected Registry and wrote both coordinates into a write-once
/// Trading-owned verdict. Sealed execution carries the raw account again in
/// the staging slot; the exact alias is a wire-shape assertion, not a claim
/// that a raw account is a vacant staging cursor. The live raw body is still
/// reauthenticated by owner, privileges, rent, exact width, and complete-body
/// digest before the sealed token is minted.
#[allow(clippy::too_many_arguments)]
pub(super) fn borrow_sealed_record<'a, 'info>(
    frame: HotFrameV3<'_, 'info>,
    closure: SealedDescriptorClosureV1,
    role: SealedRoleV1,
    raw: &'a AccountInfo<'info>,
    staging: &AccountInfo<'info>,
    rent: &Rent,
    schema: [u8; 32],
    digest: [u8; 32],
) -> Result<core::cell::Ref<'a, [u8]>, ProgramError> {
    let row: SealedRecordRowV1 = closure.row(role).map_err(|_| TradingSbfError::Content)?;
    if row.schema() != schema
        || row.content_digest() != digest
        || row.raw_record_account() != raw.key.to_bytes()
        || row.staging_account() == row.raw_record_account()
        || staging.key != raw.key
        || staging.owner != raw.owner
        || staging.is_signer != raw.is_signer
        || staging.is_writable != raw.is_writable
        || staging.executable != raw.executable
    {
        return Err(TradingSbfError::Content.into());
    }
    let data = raw
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    if raw.owner != frame.registry.key
        || raw.is_signer
        || raw.is_writable
        || raw.executable
        || usize::try_from(row.exact_data_length()).map_err(|_| TradingSbfError::Content)?
            != data.len()
        || solana_program::hash::hash(&data).to_bytes() != digest
        || !rent.is_exempt(raw.lamports(), data.len())
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(core::cell::Ref::map(data, |bytes| &**bytes))
}

/// Mint one sealed-artifact token for a record this invocation just borrowed.
pub(super) fn sealed_token<'a>(
    closure: SealedDescriptorClosureV1,
    role: SealedRoleV1,
    schema: [u8; 32],
    digest: [u8; 32],
    bytes: &'a [u8],
) -> Result<SealedArtifactV1<'a>, ProgramError> {
    closure
        .authenticate_artifact(role, schema, digest, bytes)
        .map_err(|_| TradingSbfError::Content.into())
}
