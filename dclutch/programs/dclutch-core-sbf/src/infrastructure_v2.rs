//! The infrastructure succession ceremony — `InitializeProtocolInfrastructureV2`.
//!
//! `docs/design/PROFILE_UPGRADE_RULING_2026_08_31.md` §5, executed. The V1
//! profile made the Registry and Rent selections write-once with no second
//! write route, which made any Registry (or Rent) upgrade a protocol-wide
//! brick (P-008). This route is the repair the ruling chose: a V2 profile at
//! its own one-seed PDA, write-once by the same vacancy discipline, created
//! under evidence STRICTLY stronger than V1's creation — V1's whole gate,
//! plus the predecessor conjuncts `DeclareSuccessor` taught the lineage
//! machinery. V1 is never touched: it stays on chain byte-identical forever,
//! a sealed historical record still content-walkable from V2's predecessor
//! artifact ids.
//!
//! The conjunct geometry, each refusing by name:
//!
//! 1. **V1's whole gate.** Core's live ProgramData upgrade authority signs;
//!    both presented deployments authenticate against their finalized
//!    records — a MOVED binding by first-admission full-ELF hashing (the
//!    claimed digest is attacker-publishable until hashed), an UNMOVED
//!    binding by the pinned fast path, admission-sound because V1's first
//!    admission hashed those exact bytes and its pin still holds; registry
//!    and rent are distinct and neither is Core; both releases are
//!    slot-pinned shapes.
//! 2. **Predecessor presence** (`InfrastructurePredecessorAbsent`). The V1
//!    profile stands written at its derived PDA and hostile-decodes.
//!    Succession without a predecessor is `process_initialize`'s job.
//! 3. **Identity invariance** (`InfrastructureIdentityMoved`). V2's registry
//!    program equals V1's; likewise rent. Bytes may move, identity never.
//! 4. **Forward-only** (`InfrastructureNotForward`). A moved binding's
//!    successor record binds a strictly later deployment slot than the
//!    predecessor record it replaces; an unmoved binding is byte-identical
//!    to V1's; a succession in which nothing moved selects nothing and
//!    would only burn the one V2 vacancy — refused.
//! 5. **Consent** (`InfrastructureConsentMissing`). For each moved binding,
//!    the PREDECESSOR record's bound upgrade authority signs: the key the
//!    Loader already required for the physical upgrade consents to the
//!    re-selection, on chain. An unmoved binding stands the System program
//!    in its consent slot and must not sign.
//! 6. **One succession per domain** (`InfrastructureAlreadySucceeded`). The
//!    V2 PDA is vacant, or holds a profile that was BORN at V2 and has not
//!    yet succeeded. This was raw vacancy while the ceremony was the only
//!    writer of a V2; a genesis cohort now writes its own at birth, so the
//!    rule reads the two predecessor ids instead — sentinels mean unspent,
//!    real artifact releases mean spent. One succession per domain, ever;
//!    not one V2 per domain.
//! 7. **Read-back.** What was persisted decodes back to what was composed.

use core::convert::TryFrom;

use dclutch_market::capability_manifest::funding::funded_rent_persists_v1;
use dclutch_registry::{
    ARTIFACT_RELEASE_BYTES_V1, ARTIFACT_RELEASE_SCHEMA_ID_V1, ArtifactReleaseV1,
    require_slot_pinned_release_v1,
};
use dclutch_registry::release_set::{
    ArtifactReleaseIdV1, ExecutionRoleBindingV1, PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2, PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2, ProtocolInfrastructureProfileV1,
    ProtocolInfrastructureProfileV2,
};
use solana_program::{
    account_info::AccountInfo,
    hash::hash,
    program::{invoke, invoke_signed},
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::system_program;
use solana_system_interface::instruction::{allocate, assign, transfer};

use crate::{
    CoreSbfError,
    frame::InitializeInfrastructureV2Accounts,
    infrastructure::{
        ArtifactAdmissionV1, authenticate_artifact_release,
        authenticate_current_core_upgrade_authority,
    },
    records::authenticate_finalized_record,
};

/// Execute the succession ceremony once.
#[inline(never)]
pub(crate) fn process_initialize_v2(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
) -> Result<(), solana_program::program_error::ProgramError> {
    let frame = InitializeInfrastructureV2Accounts::parse(accounts)?;
    // Conjunct 1: the party that could already replace the reader itself.
    authenticate_current_core_upgrade_authority(
        program_id,
        frame.core_programdata,
        frame.upgrade_authority,
    )?;
    let rent = Rent::from_account_info(frame.rent).map_err(|_| CoreSbfError::Infrastructure)?;

    // Conjunct 2: the predecessor stands written and decodes.
    let predecessor = authenticate_predecessor_profile(program_id, frame.predecessor_profile)?;

    // Conjunct 1, per binding. Whether a binding MOVED is decided by content:
    // the presented successor record's digest against the id V1 pinned. A
    // moved binding is a first admission — the full deployed ELF is hashed
    // against the new record's claim. An unmoved binding rides V1's
    // admission: its first admission hashed these exact bytes, and the pinned
    // fast path re-proves the pin still holds against this observation.
    let registry_moved = moved(frame.registry_artifact_raw, predecessor.registry())?;
    let rent_moved = moved(frame.rent_artifact_raw, predecessor.rent())?;
    let (registry_binding, registry_release) = authenticate_artifact_release(
        frame.registry_program.key,
        frame.registry_artifact_raw,
        frame.registry_artifact_staging,
        frame.registry_program,
        frame.registry_programdata,
        admission(registry_moved),
    )?;
    let (rent_binding, rent_release) = authenticate_artifact_release(
        frame.registry_program.key,
        frame.rent_artifact_raw,
        frame.rent_artifact_staging,
        frame.rent_program,
        frame.rent_programdata,
        admission(rent_moved),
    )?;
    if frame.registry_program.key == program_id || frame.rent_program.key == program_id {
        return Err(CoreSbfError::Infrastructure.into());
    }

    // Conjunct 3: bytes may move, identity never.
    if registry_binding.program() != predecessor.registry().program()
        || rent_binding.program() != predecessor.rent().program()
    {
        return Err(CoreSbfError::InfrastructureIdentityMoved.into());
    }

    // Conjunct 4's degenerate arm: a succession that moves nothing selects
    // nothing new, and the no-fork discipline means it would spend the one
    // vacancy this domain will ever have. The lineage machinery refuses
    // self-succession for the same reason.
    if !registry_moved && !rent_moved {
        return Err(CoreSbfError::InfrastructureNotForward.into());
    }

    // Conjuncts 4 and 5, one arm per binding.
    authenticate_succession_arm(
        frame.registry_program.key,
        predecessor.registry(),
        registry_release,
        registry_moved,
        frame.predecessor_registry_artifact_raw,
        frame.predecessor_registry_artifact_staging,
        frame.registry_consent_authority,
    )?;
    authenticate_succession_arm(
        frame.registry_program.key,
        predecessor.rent(),
        rent_release,
        rent_moved,
        frame.predecessor_rent_artifact_raw,
        frame.predecessor_rent_artifact_staging,
        frame.rent_consent_authority,
    )?;

    let profile = ProtocolInfrastructureProfileV2::new(
        registry_binding,
        rent_binding,
        predecessor.registry().artifact_release(),
        predecessor.rent().artifact_release(),
    )
    .map_err(|_| CoreSbfError::Infrastructure)?;

    // Conjuncts 6 and 7.
    create_profile_v2(program_id, &frame, &rent, profile)?;
    Ok(())
}

/// Whether the presented successor record differs from the V1-pinned one.
///
/// This is the sole definition of "the binding moved across the succession",
/// mirroring the lineage route's artifact-release-id comparison. It reads the
/// raw record bytes only to hash them; every authentication of those bytes
/// (ownership, PDA, staging cursor, schema, deployment) still happens in
/// [`authenticate_artifact_release`] afterward.
fn moved(
    successor_raw: &AccountInfo<'_>,
    predecessor_binding: ExecutionRoleBindingV1,
) -> Result<bool, CoreSbfError> {
    let bytes = successor_raw
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Infrastructure)?;
    if bytes.len() != ARTIFACT_RELEASE_BYTES_V1 {
        return Err(CoreSbfError::Infrastructure);
    }
    let digest = ArtifactReleaseIdV1::new(hash(&bytes).to_bytes())
        .map_err(|_| CoreSbfError::Infrastructure)?;
    Ok(digest != predecessor_binding.artifact_release())
}

const fn admission(moved: bool) -> ArtifactAdmissionV1 {
    if moved {
        ArtifactAdmissionV1::FirstAdmission
    } else {
        ArtifactAdmissionV1::AlreadyPinned
    }
}

/// Conjunct 2: the V1 profile, present and decodable — nothing more.
///
/// Deliberately NOT `authenticate_profile`: that read re-authenticates V1's
/// pinned deployments, and after the upgrade this ceremony repairs, that read
/// refuses `ReleaseSuperseded` — which is the situation this route exists
/// for. Presence means: the exact derived PDA, Core-owned, exact V1 width,
/// non-executable, rent-exempt, and hostile-decoding to a canonical V1
/// profile. The profile's content is trusted exactly as far as its own
/// write-once creation authenticated it, which is the same first-admission
/// ceremony this route re-runs for the moved binding.
fn authenticate_predecessor_profile(
    program_id: &Pubkey,
    predecessor_profile: &AccountInfo<'_>,
) -> Result<ProtocolInfrastructureProfileV1, CoreSbfError> {
    let expected =
        Pubkey::find_program_address(&[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1], program_id)
            .0;
    if predecessor_profile.key != &expected
        || predecessor_profile.owner != program_id
        || predecessor_profile.data_len() != PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1
        || predecessor_profile.executable
        || !funded_rent_persists_v1(predecessor_profile.lamports())
    {
        return Err(CoreSbfError::InfrastructurePredecessorAbsent);
    }
    let bytes = predecessor_profile
        .try_borrow_data()
        .map_err(|_| CoreSbfError::InfrastructurePredecessorAbsent)?;
    ProtocolInfrastructureProfileV1::decode(&bytes)
        .map_err(|_| CoreSbfError::InfrastructurePredecessorAbsent)
}

/// Conjuncts 4 and 5 for one binding.
///
/// MOVED: the predecessor's own finalized record is presented, content-pinned
/// to the id V1 admitted, and read for exactly two facts — its deployment
/// slot (the forward-only comparison) and its bound upgrade authority (the
/// consent signer). UNMOVED: the binding is byte-identical to V1's by
/// definition of unmoved plus conjunct 3, no evidence is needed and none may
/// be offered — the System program stands in all three slots, unsigned.
#[allow(clippy::too_many_arguments)]
fn authenticate_succession_arm(
    registry: &Pubkey,
    predecessor_binding: ExecutionRoleBindingV1,
    successor_release: ArtifactReleaseV1,
    moved: bool,
    predecessor_raw: &AccountInfo<'_>,
    predecessor_staging: &AccountInfo<'_>,
    consent: &AccountInfo<'_>,
) -> Result<(), CoreSbfError> {
    if !moved {
        // Nothing is being consented to, so nothing may stand in the slots
        // that could look like consent or like predecessor evidence.
        if predecessor_raw.key != &system_program::ID
            || predecessor_staging.key != &system_program::ID
            || consent.key != &system_program::ID
            || consent.is_signer
        {
            return Err(CoreSbfError::InfrastructureConsentMissing);
        }
        return Ok(());
    }
    let predecessor_release = authenticate_predecessor_record(
        registry,
        predecessor_raw,
        predecessor_staging,
        predecessor_binding,
    )?;
    // Conjunct 4: under Loader V3 a ProgramData slot only moves forward, so
    // strictly greater is exactly "was upgraded after".
    if successor_release.deployment_slot() <= predecessor_release.deployment_slot() {
        return Err(CoreSbfError::InfrastructureNotForward);
    }
    // Conjunct 5: the key that moved the bytes consents to the re-selection.
    // An `Immutable` predecessor binds no authority, and its bytes cannot
    // have moved — a "moved" claim against it is a contradiction rather than
    // a missing signature, refused under the same name.
    let bound = predecessor_release
        .upgrade_authority()
        .ok_or(CoreSbfError::InfrastructureConsentMissing)?;
    if !consent.is_signer || consent.key.to_bytes() != bound {
        return Err(CoreSbfError::InfrastructureConsentMissing);
    }
    Ok(())
}

/// The predecessor's finalized artifact record, content-pinned to V1's id.
///
/// The record digest IS the artifact-release id (the profile content-pins the
/// record, `infrastructure.rs`'s `AlreadyPinned` doctrine), so hashing the
/// presented bytes and comparing to the V1 binding leaves an attacker no
/// record to substitute: a different record has a different digest. The
/// deployment behind this record is deliberately NOT observed — it is the
/// superseded one; its slot and bound authority are read from the record,
/// which first admission proved truthful when the profile pinned it.
fn authenticate_predecessor_record(
    registry: &Pubkey,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    binding: ExecutionRoleBindingV1,
) -> Result<ArtifactReleaseV1, CoreSbfError> {
    let bytes = raw
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Infrastructure)?;
    if bytes.len() != ARTIFACT_RELEASE_BYTES_V1 {
        return Err(CoreSbfError::Infrastructure);
    }
    let digest = hash(&bytes).to_bytes();
    if ArtifactReleaseIdV1::new(digest).map_err(|_| CoreSbfError::Infrastructure)?
        != binding.artifact_release()
    {
        return Err(CoreSbfError::Infrastructure);
    }
    authenticate_finalized_record(
        registry,
        raw,
        staging,
        ARTIFACT_RELEASE_SCHEMA_ID_V1,
        digest,
        &bytes,
    )?;
    let release = ArtifactReleaseV1::decode(&bytes).map_err(|_| CoreSbfError::Infrastructure)?;
    if release.program() != binding.program() {
        return Err(CoreSbfError::Infrastructure);
    }
    require_slot_pinned_release_v1(release).map_err(|_| CoreSbfError::Infrastructure)?;
    Ok(release)
}

/// What the V2 PDA says about whether this domain has already succeeded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProfileSuccessionStateV2 {
    /// No V2 has ever been written. A pre-genesis-arm cohort, or a cohort whose
    /// Core predates the genesis profile.
    Vacant,
    /// A genesis profile stands here: born at V2, succession unspent.
    BornAtV2,
    /// A succeeded profile stands here. One succession per domain, ever.
    Succeeded,
}

/// Classify the V2 PDA without trusting anything the caller said about it.
///
/// Anything occupying the PDA that is not a decodable Core-owned V2 of the
/// exact width is `Succeeded`, not `Vacant`: an account this ceremony cannot
/// read is never treated as room to write. The refusal that follows names the
/// no-fork rule, which is the accusation that fits — the domain is occupied by
/// something, and this ceremony is not the thing that gets to overwrite it.
fn profile_succession_state_v2(
    program_id: &Pubkey,
    profile: &AccountInfo<'_>,
) -> Result<ProfileSuccessionStateV2, solana_program::program_error::ProgramError> {
    if profile.owner == &system_program::ID && profile.data_len() == 0 && !profile.executable {
        return Ok(ProfileSuccessionStateV2::Vacant);
    }
    if profile.owner != program_id
        || profile.executable
        || profile.data_len() != PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2
    {
        return Ok(ProfileSuccessionStateV2::Succeeded);
    }
    let bytes = profile
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Infrastructure)?;
    Ok(match ProtocolInfrastructureProfileV2::decode(&bytes) {
        Ok(standing) if standing.born_at_v2() => ProfileSuccessionStateV2::BornAtV2,
        _ => ProfileSuccessionStateV2::Succeeded,
    })
}

/// Conjuncts 6 and 7: the vacancy, the creation, and the read-back belt.
///
/// `create_profile`'s exact discipline at the V2 domain, with the occupied
/// case named: a written V2 account here is not corruption, it is the
/// succession having happened — one per domain, ever.
fn create_profile_v2(
    program_id: &Pubkey,
    frame: &InitializeInfrastructureV2Accounts<'_, '_>,
    rent: &Rent,
    profile: ProtocolInfrastructureProfileV2,
) -> Result<(), solana_program::program_error::ProgramError> {
    let (expected, bump) =
        Pubkey::find_program_address(&[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2], program_id);
    if frame.profile.key != &expected {
        return Err(CoreSbfError::Infrastructure.into());
    }
    // Conjunct 6, and it is the entire no-fork guarantee: ONE SUCCESSION per
    // domain, ever — not one V2 per domain.
    //
    // The rule used to be raw vacancy, which was exact while the only writer of
    // a V2 was this ceremony. A genesis cohort now writes its own V2 at birth
    // (`infrastructure::process_initialize`), so vacancy would refuse the first
    // real succession of every cohort that started clean — reinstating P-008,
    // the protocol-wide brick this ceremony exists to repair, for exactly the
    // cohorts that never carried the defect.
    //
    // What replaces it needs no new field, because the distinction is already
    // in the bytes: a profile whose two predecessor ids are the genesis
    // sentinels was BORN at V2 and has not spent its succession; a profile
    // naming two real V1 artifact releases has. `born_at_v2` carries the
    // soundness argument — only Core writes a V2, genesis writes sentinels only
    // into a vacant PDA, and this ceremony writes real predecessor ids read out
    // of the live V1 and can never write a sentinel back.
    let occupied = match profile_succession_state_v2(program_id, frame.profile)? {
        ProfileSuccessionStateV2::Vacant => false,
        ProfileSuccessionStateV2::BornAtV2 => true,
        ProfileSuccessionStateV2::Succeeded => {
            return Err(CoreSbfError::InfrastructureAlreadySucceeded.into());
        }
    };
    if occupied {
        // The account already exists, is already Core-owned and already exactly
        // 224 bytes, so there is nothing to allocate or assign — and the System
        // program would refuse both. Overwrite in place, then fall through to
        // the same conjunct-7 read-back the created path uses.
        let encoded = profile.to_bytes();
        {
            let mut data = frame
                .profile
                .try_borrow_mut_data()
                .map_err(|_| CoreSbfError::Infrastructure)?;
            if data.len() != encoded.len() {
                return Err(CoreSbfError::Infrastructure.into());
            }
            data.copy_from_slice(&encoded);
        }
        let committed = frame
            .profile
            .try_borrow_data()
            .map_err(|_| CoreSbfError::Infrastructure)?;
        if frame.profile.owner != program_id
            || ProtocolInfrastructureProfileV2::decode(&committed) != Ok(profile)
        {
            return Err(CoreSbfError::Infrastructure.into());
        }
        return Ok(());
    }
    let required = rent.minimum_balance(PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2);
    let top_up = required.saturating_sub(frame.profile.lamports());
    if top_up > 0 {
        invoke(
            &transfer(frame.payer.key, frame.profile.key, top_up),
            &[
                frame.payer.clone(),
                frame.profile.clone(),
                frame.system.clone(),
            ],
        )
        .map_err(|_| CoreSbfError::Creation)?;
    }
    let bump_seed = [bump];
    let signer = [
        PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2,
        bump_seed.as_slice(),
    ];
    invoke_signed(
        &allocate(
            frame.profile.key,
            u64::try_from(PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2)
                .map_err(|_| CoreSbfError::Arithmetic)?,
        ),
        &[frame.profile.clone(), frame.system.clone()],
        &[&signer],
    )
    .map_err(|_| CoreSbfError::Creation)?;
    invoke_signed(
        &assign(frame.profile.key, program_id),
        &[frame.profile.clone(), frame.system.clone()],
        &[&signer],
    )
    .map_err(|_| CoreSbfError::Creation)?;
    let encoded = profile.to_bytes();
    {
        let mut data = frame
            .profile
            .try_borrow_mut_data()
            .map_err(|_| CoreSbfError::Infrastructure)?;
        if data.len() != encoded.len() {
            return Err(CoreSbfError::Infrastructure.into());
        }
        data.copy_from_slice(&encoded);
    }
    // Conjunct 7: read back what was persisted, not the buffer written.
    let committed = frame
        .profile
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Infrastructure)?;
    if frame.profile.owner != program_id
        || ProtocolInfrastructureProfileV2::decode(&committed) != Ok(profile)
    {
        return Err(CoreSbfError::Infrastructure.into());
    }
    Ok(())
}
