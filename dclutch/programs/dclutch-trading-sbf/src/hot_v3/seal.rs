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
    CAPABILITY_SEAL_BYTES_V1, CAPABILITY_SEAL_CLOSE_NO_BUMP_CANDIDATE_V1,
    CAPABILITY_SEAL_ROW_COUNT_V1, CapabilitySealCloseRequestV1, CapabilitySealKeyV1,
    CapabilitySealRequestV1, SealedArtifactV1, SealedDescriptorClosureV1, SealedRecordRowV1,
    SealedRoleV1,
};
use dclutch_registry_activation_auth_v1::{
    authenticate_activated_role_in_frame_v1, authenticate_activation_cache_identity_v1,
};
use dclutch_registry_contract::ActivatedExecutionReleaseSetViewV1;
use dclutch_release_set_contract::ExecutionRoleV1;
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
use solana_sdk_ids::{system_program, sysvar};
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
    // The bump this act already derived to sign the account into existence is
    // persisted with the verdict, so every later reader reproduces the address
    // instead of searching for it. See `CAPABILITY_SEAL_BUMP_OFFSET_V1`.
    SealedDescriptorClosureV1::encode(key, rows, bump, &mut data)
        .map_err(|_| TradingSbfError::Commit)?;
    Ok(())
}

/// The seal being closed, on the close outer.
pub const CLOSE_SEAL_ACCOUNT_V1: usize = 0;
/// The closer: signer, writable, and the sole beneficiary of the liberated rent.
pub const CLOSE_SEAL_CLOSER_ACCOUNT_V1: usize = 1;
/// The Registry program the seal's record addresses were derived under.
pub const CLOSE_SEAL_REGISTRY_ACCOUNT_V1: usize = 2;
/// A Registry-owned activation cache that still authenticates its own Trading role.
pub const CLOSE_SEAL_ACTIVATION_CACHE_ACCOUNT_V1: usize = 3;
/// This Program, as the Loader-owned executable the cache's Trading role names.
pub const CLOSE_SEAL_TRADING_PROGRAM_ACCOUNT_V1: usize = 4;
/// That executable's ProgramData, whose deployment slot carries the pin.
pub const CLOSE_SEAL_TRADING_PROGRAMDATA_ACCOUNT_V1: usize = 5;
/// The Rent sysvar, for the one exact liberated amount.
pub const CLOSE_SEAL_RENT_ACCOUNT_V1: usize = 6;
/// Exact account count of the seal close outer.
pub const CLOSE_SEAL_ACCOUNT_COUNT_V1: usize = 7;

/// Close one seal the live Trading release can no longer address, and pay its
/// rent to whoever did the chore.
///
/// # The account class this exists to bound
///
/// `trading_semantic_release` is the fourth PDA seed of a seal, so a Trading
/// release "does not invalidate a seal so much as stop addressing it"
/// (decision 0005). Every Trading release therefore strands the rent of every
/// seal written under its predecessor, across all descriptors times actions,
/// and the class grows with the release cadence rather than with the Market
/// count. Omission `P-006` records that; this is its close.
///
/// # Beneficiary
///
/// The closer signs and keeps the rent. This is the funded-crank pattern: the
/// account is a chore nobody is obliged to do, so the reward is carved out of
/// exactly the rent the chore liberates and out of nothing else. No Market's
/// funding may receive it — a seal is not per-Market, so paying one Market's
/// funding would make it pay for every other Market's executions, which is the
/// same reasoning that put the seal outside `FundingStateV1` (`0005` §Rent).
/// Burn was rejected because it preserves the stranding it was meant to end.
///
/// It is permissionless, and racing is harmless: the first closer wins the
/// chore and every later attempt refuses by absence. There is no wrong signer
/// here, only a signer who is too late.
///
/// # The cap, and where it is actually enforced
///
/// Only rent the close liberates may flow. Today that is the whole balance,
/// and the reason is a gate rather than an arithmetic: artifact profile 1
/// defines no lamport role beyond rent exemption, `SealedDescriptorClosureV1`
/// refuses every other profile at `decode`, and this route refuses every
/// request that does not name profile 1. A future seal class that carries a
/// bounty or an escrow beyond exemption is a different profile byte with a
/// different owner for those lamports, and it will refuse here until it gets
/// its own close naming its own beneficiary.
///
/// So the cap is not implemented as "pay `min(balance, exemption)` and leave
/// the rest", and that is deliberate on two counts. Leaving a residue is not
/// even available — a zero-data account holding less than
/// `Rent::minimum_balance(0)` is rent-paying, and the runtime rejects the
/// transaction that creates one — and refusing an over-funded seal instead
/// would hand a griefer a 1-lamport transfer that re-strands the account
/// permanently, which is the exact outcome this route exists to prevent.
///
/// # The second arm, for a seal that cannot state its own bump
///
/// A seal written before the bump moved into the byte at offset 20 carries an
/// unwritten zero there, so the address it lives at cannot be reproduced from
/// the body alone and the ordinary arm refuses it. Those accounts are real
/// seals at real canonical addresses whose rent nothing could reclaim. The
/// request's bump candidate opens the second arm for exactly them:
/// `decode_defunct` requires every canonical field the ordinary decode does and
/// requires the bump byte to be zero, and the address is reproduced from the
/// candidate instead of the body.
///
/// The arms partition rather than overlap, on the one byte: candidate zero
/// selects the ordinary arm, which requires a nonzero persisted bump and never
/// looks at the candidate; any other candidate selects the defunct arm, which
/// requires a zero persisted bump. A well-formed seal offered under a nonzero
/// candidate is refused, so the tolerant arm can never be turned on a seal the
/// strict one governs. Nothing else is relaxed — in particular the live-release
/// refusal below applies identically, which is where this close's soundness
/// lives.
///
/// # What makes this sound, and it is one conjunct
///
/// A seal is write-once, and a close that let a *different* verdict be written
/// at the same address afterwards would destroy that. It cannot, because
/// re-creation is governed by the same seed the close is:
/// `process_capability_seal_v1` derives the seal address from
/// `root.trading_semantic_release`, which is the semantic release of the
/// Trading role in an activation cache that authenticates against THIS
/// deployed Program (`HotFrameV3::parse` requires
/// `trading_program.key == program_id`; `cached_role_deployment_observation_v1`
/// requires the release's pinned deployment slot to equal what ProgramData
/// currently reports). So the writer can only ever reach addresses whose
/// release seed is a live one — and this route refuses to close any seal whose
/// release seed IS live. A closed seal is therefore an address the live
/// executable cannot write to, not merely one it declines to.
///
/// The residual is named rather than papered over: `semantic_release_id` is
/// publisher-supplied and nothing on chain checks it
/// (`programs/dclutch-registry-sbf/src/lineage_v1.rs`), so a coalition that can
/// activate a release set naming an old semantic release could re-open a closed
/// address under new bytes. That coalition is the roles' upgrade authorities —
/// the same coalition that could ship arbitrary Trading code, and therefore the
/// same one every seal's verdict already trusts. Write-once holds against
/// everyone the seal was ever protecting against.
pub fn process_capability_seal_close_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    let request = CapabilitySealCloseRequestV1::decode(instruction_data)
        .map_err(|_| TradingSbfError::CloseSealFrame)?;
    if accounts.len() != CLOSE_SEAL_ACCOUNT_COUNT_V1 {
        return Err(TradingSbfError::CloseSealFrame.into());
    }
    let seal = account(accounts, CLOSE_SEAL_ACCOUNT_V1)?;
    let closer = account(accounts, CLOSE_SEAL_CLOSER_ACCOUNT_V1)?;
    let registry = account(accounts, CLOSE_SEAL_REGISTRY_ACCOUNT_V1)?;
    let cache = account(accounts, CLOSE_SEAL_ACTIVATION_CACHE_ACCOUNT_V1)?;
    let trading_program = account(accounts, CLOSE_SEAL_TRADING_PROGRAM_ACCOUNT_V1)?;
    let trading_programdata = account(accounts, CLOSE_SEAL_TRADING_PROGRAMDATA_ACCOUNT_V1)?;
    let rent_account = account(accounts, CLOSE_SEAL_RENT_ACCOUNT_V1)?;
    // The beneficiary must be a plain writable System wallet that signed, which
    // is `record_v1::require_system_wallet`'s shape: a program-owned refund
    // destination is an account whose bytes mean something to somebody, and
    // crediting one is a write this route has no authority to make.
    if !closer.is_signer
        || !closer.is_writable
        || closer.executable
        || closer.owner != &system_program::ID
        || !closer
            .try_data_is_empty()
            .map_err(|_| TradingSbfError::CloseSealFrame)?
        || seal.key == closer.key
        || registry.is_signer
        || registry.is_writable
        || !registry.executable
        || trading_program.key != program_id
        || rent_account.key != &sysvar::rent::ID
        || rent_account.is_signer
        || rent_account.is_writable
        || rent_account.executable
    {
        return Err(TradingSbfError::CloseSealFrame.into());
    }
    let rent =
        Rent::from_account_info(rent_account).map_err(|_| TradingSbfError::CloseSealFrame)?;
    // Absence lands here, and it is what a second close of the same seal meets:
    // the account is System-owned and empty, so it is not this Program's seal.
    if seal.owner != program_id
        || !seal.is_writable
        || seal.is_signer
        || seal.executable
        || seal.data_len() != CAPABILITY_SEAL_BYTES_V1
        || !rent.is_exempt(seal.lamports(), CAPABILITY_SEAL_BYTES_V1)
    {
        return Err(TradingSbfError::CloseSealAccount.into());
    }
    // The two arms partition on the request's bump candidate, and each one
    // reads the body its own decoder admits. Nothing below this line differs
    // between them: the same Registry agreement, the same live-release refusal,
    // the same whole-balance close.
    let key = match request.bump_candidate() {
        CAPABILITY_SEAL_CLOSE_NO_BUMP_CANDIDATE_V1 => {
            require_own_seal_address_v1(program_id, seal)?
        }
        candidate => require_defunct_seal_address_v1(program_id, seal, candidate)?,
    };
    if registry.key.to_bytes() != key.registry_program() {
        return Err(TradingSbfError::CloseSealFrame.into());
    }
    let live = live_trading_semantic_release_v1(
        program_id,
        registry,
        cache,
        trading_program,
        trading_programdata,
    )?;
    if live == key.trading_semantic_release() {
        return Err(TradingSbfError::CloseSealLiveRelease.into());
    }

    let liberated = seal.lamports();
    let closer_after = closer
        .lamports()
        .checked_add(liberated)
        .ok_or(TradingSbfError::Commit)?;
    {
        let mut closer_lamports = closer
            .try_borrow_mut_lamports()
            .map_err(|_| TradingSbfError::Commit)?;
        let mut seal_lamports = seal
            .try_borrow_mut_lamports()
            .map_err(|_| TradingSbfError::Commit)?;
        **closer_lamports = closer_after;
        **seal_lamports = 0;
    }
    seal.resize(0).map_err(|_| TradingSbfError::Commit)?;
    seal.assign(&system_program::ID);
    if closer.lamports() != closer_after
        || seal.lamports() != 0
        || seal.owner != &system_program::ID
        || !seal
            .try_data_is_empty()
            .map_err(|_| TradingSbfError::Commit)?
    {
        return Err(TradingSbfError::Commit.into());
    }
    Ok(())
}

/// Reproduce a seal account's own address from its own body, and return the key.
///
/// This is `authenticate_capability_seal_v3`'s address argument used the other
/// way round. There, a consumer derives the key it wants and the seal must
/// match; here there is no consumer and no wanted key, so the body states the
/// key and the address is reproduced from it with `create_program_address` and
/// the persisted bump. A body that lies about any seed reproduces some other
/// address and refuses, so a Trading-owned account carrying plausible seal bytes
/// at a non-canonical address can never be closed as though it were a seal — and
/// the key this returns is exactly the one the address proves.
fn require_own_seal_address_v1(
    program_id: &Pubkey,
    seal: &AccountInfo<'_>,
) -> Result<CapabilitySealKeyV1, ProgramError> {
    let bytes = seal
        .try_borrow_data()
        .map_err(|_| TradingSbfError::CloseSealAccount)?;
    let closure =
        SealedDescriptorClosureV1::decode(&bytes).map_err(|_| TradingSbfError::CloseSealAccount)?;
    let key = closure
        .key()
        .map_err(|_| TradingSbfError::CloseSealAccount)?;
    let bump = closure
        .bump()
        .map_err(|_| TradingSbfError::CloseSealAccount)?;
    require_reproduced_seal_address_v1(program_id, seal, key, bump)?;
    Ok(key)
}

/// The same reproduction for a seal whose body never recorded its own bump.
///
/// A seal written before the bump moved into
/// [`dclutch_capability_seal_contract::CAPABILITY_SEAL_BUMP_OFFSET_V1`] carries
/// an unwritten zero there. `SealedDescriptorClosureV1::decode_defunct` reads
/// exactly those bodies — every conjunct of `decode` except that the bump byte
/// must be zero, so a well-formed seal offered here refuses with `NotDefunct`
/// and lands on [`TradingSbfError::CloseSealAccount`] like every other body
/// this route will not act on. The two decoders partition the byte, so the two
/// arms can never both admit one account.
///
/// The caller-supplied `bump_candidate` replaces only the byte the body could
/// not state, and it is checked the same way the persisted byte is: the address
/// is reproduced from the body's own seeds plus the candidate and must BE the
/// account being closed. At most one candidate satisfies that equality, so a
/// hostile caller can name any byte it likes and reach nothing but a mismatch —
/// the candidate is a memo of a search anyone can repeat offline, never an
/// authority. In particular it cannot aim the close at a different account:
/// the seal account is fixed by the frame, and the seeds come from the body.
#[inline(never)]
fn require_defunct_seal_address_v1(
    program_id: &Pubkey,
    seal: &AccountInfo<'_>,
    bump_candidate: u8,
) -> Result<CapabilitySealKeyV1, ProgramError> {
    let bytes = seal
        .try_borrow_data()
        .map_err(|_| TradingSbfError::CloseSealAccount)?;
    let closure = SealedDescriptorClosureV1::decode_defunct(&bytes)
        .map_err(|_| TradingSbfError::CloseSealAccount)?;
    let key = closure
        .key()
        .map_err(|_| TradingSbfError::CloseSealAccount)?;
    require_reproduced_seal_address_v1(program_id, seal, key, bump_candidate)?;
    Ok(key)
}

/// Refuse unless a key and one bump reproduce the seal account's own address.
///
/// Both arms reproduce the address the same way and out of line, so the
/// argument is stated once and neither arm can drift from it: a body that lies
/// about any seed, or a bump that is not the one that address was derived
/// under, names some other address and refuses here.
#[inline(never)]
fn require_reproduced_seal_address_v1(
    program_id: &Pubkey,
    seal: &AccountInfo<'_>,
    key: CapabilitySealKeyV1,
    bump: u8,
) -> Result<(), ProgramError> {
    let bump_seed = [bump];
    let seeds = key.seeds();
    let base = seeds.as_slices();
    let expected = Pubkey::create_program_address(
        &[
            base[0], base[1], base[2], base[3], base[4], base[5], &bump_seed,
        ],
        program_id,
    )
    .map_err(|_| TradingSbfError::CloseSealAccount)?;
    if seal.key != &expected {
        return Err(TradingSbfError::CloseSealAccount.into());
    }
    Ok(())
}

/// Read the Trading semantic release the live deployment is currently activated
/// under, from a Registry-owned activation cache that proves it.
///
/// # Why this witness cannot be stale
///
/// The cache is not trusted for being handed over. `require_cache_account`
/// (inside `authenticate_activation_cache_identity_v1`) takes Registry
/// ownership, non-executability and the one exact width; the address is then
/// reproduced from `[ACTIVATION_PDA_DOMAIN_V1, release_set_id]` under the
/// Registry, so only an account the Registry itself opened for exactly that
/// release set can stand here. The release set id is read out of the cache's own
/// body because there is no Market in this frame to name one — that is the
/// weaker of the two identity shapes the crate documents, and it is the right
/// one here: the caller is not selecting a generation to execute under, it is
/// exhibiting *some* generation that is still live, and the address derivation
/// is what makes "still live" a Registry fact rather than a claim.
///
/// `authenticate_activated_role_in_frame_v1` then runs the Trading role against
/// the deployed Program and its ProgramData. That is where staleness dies: a
/// cache for a superseded release set refuses with `ReleaseSuperseded` because
/// the Loader moved the deployment slot on upgrade and the release's pin no
/// longer matches (decision 0012). So the semantic release this returns is one
/// that a hot action could actually derive a seal address from, today, in this
/// slot — which is exactly the class of address the close must not touch.
fn live_trading_semantic_release_v1(
    program_id: &Pubkey,
    registry: &AccountInfo<'_>,
    cache: &AccountInfo<'_>,
    trading_program: &AccountInfo<'_>,
    trading_programdata: &AccountInfo<'_>,
) -> Result<[u8; 32], ProgramError> {
    let data = cache
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Release)?;
    let activated =
        ActivatedExecutionReleaseSetViewV1::decode(&data).map_err(|_| TradingSbfError::Release)?;
    let release_set_id = *activated
        .execution_release_set_id()
        .map_err(|_| TradingSbfError::Release)?
        .as_bytes();
    authenticate_activation_cache_identity_v1(registry, cache, &release_set_id, activated)
        .map_err(TradingSbfError::from)?;
    let receipt = authenticate_activated_role_in_frame_v1(
        cache,
        activated,
        ExecutionRoleV1::Trading,
        trading_program,
        trading_programdata,
    )
    .map_err(TradingSbfError::from)?;
    // The frame already required `trading_program.key == program_id` and the
    // observation already required `program.key == release.program()`. This is
    // the same comparison stated on the receipt, which is the form every other
    // caller in this program states it in, and it is what ties the release this
    // returns to the executable that is running.
    if receipt.program().as_bytes() != &program_id.to_bytes() {
        return Err(TradingSbfError::Release.into());
    }
    Ok(receipt.semantic_release_id().to_bytes())
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
    // FOR THIS ACTION, on the generic Hot path every family crosses.
    //
    // A single lifecycle policy may carry plans for several actions whose
    // AccountProfiles are DIFFERENT accounts at the same coordinate -- the
    // Dealer LP frame puts the Open payer and the Close RentCredit both at fixed
    // slot 7, so one grants debit and the other credit and no shared answer
    // exists. Joining the whole policy to one action's profile therefore asked
    // whether a sibling action's plans fit a frame never built for them, and
    // refused `Content` at runtime for every such family.
    //
    // The canonically empty policy still joins vacuously here, which is what
    // Dealer equity relies on: it carries no plans at all, so there is nothing
    // for any action to answer for.
    lifecycle
        .validate_account_profile_join_for_action(account_profile, action)
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
///
/// # Why the seal's own bump is read rather than searched
///
/// The address is REPRODUCED from the bump the seal persists, not found. This
/// is the argument `borrow_finalized_record_at` already makes in this module
/// and it is not weakened here: a wrong bump reproduces a different address,
/// which fails the equality against the account the frame supplied, and refuses.
/// The bump is not caller input in any sense -- the seal is Trading-owned and
/// write-once, its only writer is `process_capability_seal_v1` above, and that
/// writer derives the bump canonically and can only write at the address that
/// derivation names. So the byte is a memo of this program's own computation.
///
/// The seed set joins on `trading_semantic_release`, so this release addresses
/// only seals written under itself; there is no older body for this reader to
/// meet. That is also why persisting the bump needed no migration: an upgrade
/// already moves every seal to a new, empty address that must be minted afresh.
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
    if seal.owner != program_id
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
    let seeds = key.seeds();
    let base = seeds.as_slices();
    let bump_seed = [closure.bump().map_err(|_| TradingSbfError::Content)?];
    let expected = Pubkey::create_program_address(
        &[
            base[0], base[1], base[2], base[3], base[4], base[5], &bump_seed,
        ],
        program_id,
    )
    .map_err(|_| TradingSbfError::Content)?;
    if seal.key != &expected {
        return Err(TradingSbfError::Content.into());
    }
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
    require_sealed_record_coordinates_v1(
        row,
        raw,
        staging,
        schema,
        digest,
        frame.uses_sealed_execution_aliases(),
    )?;
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SealedRecordCoordinateModeV1 {
    Distinct,
    DirectAlias,
}

/// Authenticate the wire coordinates persisted by one seal row.
///
/// Ordinary families carry the exact finalized raw account and the exact
/// vacant staging cursor the seal observed. Direct ordinary execution is the
/// sole family gate that turns on the all-six alias shape; in that shape the
/// second wire coordinate repeats the already-authenticated raw account while
/// the row continues to preserve the distinct historical staging coordinate.
#[allow(clippy::too_many_arguments)]
fn require_sealed_record_coordinates_v1(
    row: SealedRecordRowV1,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    schema: [u8; 32],
    digest: [u8; 32],
    direct_alias_shape: bool,
) -> Result<SealedRecordCoordinateModeV1, ProgramError> {
    if row.schema() != schema
        || row.content_digest() != digest
        || row.raw_record_account() != raw.key.to_bytes()
        || row.staging_account() == row.raw_record_account()
    {
        return Err(TradingSbfError::Content.into());
    }
    if direct_alias_shape {
        if staging.key != raw.key
            || staging.owner != raw.owner
            || staging.is_signer != raw.is_signer
            || staging.is_writable != raw.is_writable
            || staging.executable != raw.executable
        {
            return Err(TradingSbfError::Content.into());
        }
        return Ok(SealedRecordCoordinateModeV1::DirectAlias);
    }
    if staging.key.to_bytes() != row.staging_account()
        || staging.owner != &system_program::ID
        || staging.data_len() != 0
        || staging.is_signer
        || staging.is_writable
        || staging.executable
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(SealedRecordCoordinateModeV1::Distinct)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{boxed::Box, vec, vec::Vec};

    fn account(
        key: Pubkey,
        owner: Pubkey,
        signer: bool,
        writable: bool,
        executable: bool,
        data: Vec<u8>,
    ) -> AccountInfo<'static> {
        AccountInfo::new(
            Box::leak(Box::new(key)),
            signer,
            writable,
            Box::leak(Box::new(1_u64)),
            Box::leak(data.into_boxed_slice()),
            Box::leak(Box::new(owner)),
            executable,
        )
    }

    fn row(
        role: SealedRoleV1,
        schema: [u8; 32],
        digest: [u8; 32],
        raw: Pubkey,
        staging: Pubkey,
    ) -> SealedRecordRowV1 {
        SealedRecordRowV1::new(role, 8, schema, digest, raw.to_bytes(), staging.to_bytes())
            .expect("sealed row")
    }

    #[test]
    fn sealed_record_coordinates_admit_exact_distinct_or_direct_alias_only() {
        let registry = Pubkey::new_unique();
        let raw_key = Pubkey::new_unique();
        let staging_key = Pubkey::new_unique();
        let schema = [0x31; 32];
        let digest = [0x41; 32];
        let descriptor = row(
            SealedRoleV1::Descriptor,
            schema,
            digest,
            raw_key,
            staging_key,
        );
        let raw = account(raw_key, registry, false, false, false, vec![7; 8]);
        let staging = account(
            staging_key,
            system_program::ID,
            false,
            false,
            false,
            Vec::new(),
        );
        assert_eq!(
            require_sealed_record_coordinates_v1(
                descriptor, &raw, &staging, schema, digest, false,
            )
            .expect("exact persisted pair"),
            SealedRecordCoordinateModeV1::Distinct,
        );
        assert_eq!(
            require_sealed_record_coordinates_v1(descriptor, &raw, &raw, schema, digest, true,)
                .expect("authorized Direct alias"),
            SealedRecordCoordinateModeV1::DirectAlias,
        );
        assert_eq!(
            require_sealed_record_coordinates_v1(descriptor, &raw, &raw, schema, digest, false,),
            Err(TradingSbfError::Content.into()),
            "an ordinary family cannot spend the Direct alias shape",
        );
    }

    #[test]
    fn sealed_record_coordinates_refuse_substituted_or_crossed_rows() {
        let registry = Pubkey::new_unique();
        let raw_key = Pubkey::new_unique();
        let staging_key = Pubkey::new_unique();
        let schema = [0x51; 32];
        let digest = [0x61; 32];
        let descriptor = row(
            SealedRoleV1::Descriptor,
            schema,
            digest,
            raw_key,
            staging_key,
        );
        let raw = account(raw_key, registry, false, false, false, vec![7; 8]);
        let staging = account(
            staging_key,
            system_program::ID,
            false,
            false,
            false,
            Vec::new(),
        );
        let wrong_raw = account(
            Pubkey::new_unique(),
            registry,
            false,
            false,
            false,
            vec![7; 8],
        );
        let wrong_staging = account(
            Pubkey::new_unique(),
            system_program::ID,
            false,
            false,
            false,
            Vec::new(),
        );
        assert_eq!(
            require_sealed_record_coordinates_v1(
                descriptor, &wrong_raw, &staging, schema, digest, false,
            ),
            Err(TradingSbfError::Content.into()),
            "wrong raw coordinate",
        );
        assert_eq!(
            require_sealed_record_coordinates_v1(
                descriptor,
                &raw,
                &wrong_staging,
                schema,
                digest,
                false,
            ),
            Err(TradingSbfError::Content.into()),
            "wrong staging coordinate",
        );
        assert_eq!(
            require_sealed_record_coordinates_v1(descriptor, &staging, &raw, schema, digest, false,),
            Err(TradingSbfError::Content.into()),
            "swapped raw/staging pair",
        );

        let adjacent = row(
            SealedRoleV1::LifecyclePolicy,
            [0x52; 32],
            [0x62; 32],
            Pubkey::new_unique(),
            Pubkey::new_unique(),
        );
        assert_eq!(
            require_sealed_record_coordinates_v1(adjacent, &raw, &staging, schema, digest, false,),
            Err(TradingSbfError::Content.into()),
            "an adjacent seal row cannot stand in for Descriptor",
        );
    }
}
