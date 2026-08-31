//! Declare the successor of one release set.
//!
//! The whole authorization requirement of this route reduces to a single
//! field. Every other field of every `ArtifactReleaseV1` in a successor set is
//! forced by observation at activation time — the program id, the ProgramData,
//! the whole ELF digest, the deployment slot and the upgrade authority are all
//! pinned to the truth, and a set that lies about any of them cannot be
//! activated at all. The exception is `semantic_release_id`, which is
//! publisher-supplied and which nothing on chain can check. So the one thing a
//! forged successor could choose freely is the one thing that needs consent,
//! and consent is exactly what this route collects.
//!
//! That yields the supersession symmetry: stranding a market on `A` requires
//! moving some role's ProgramData slot, which requires that role's upgrade
//! authority; authoring `A`'s successor requires the upgrade authority of
//! exactly the roles whose artifacts moved. The coalition that can create the
//! hazard is the coalition that can author the remedy, and a set nobody can
//! supersede is a set nobody needs a successor for.
//!
//! Reading the predecessor's cache here is not a superseded-cache carve-out.
//! Account 2 admits no role and confers no privilege: it is read only as the
//! source of `A`'s own bindings and bound slots, for comparisons that can only
//! ever REFUSE. A check that cannot admit anything cannot be an exemption from
//! a check.

use core::convert::TryFrom;

use dclutch_registry_activation_auth_v1::release_lineage_address_and_bump_v1;
use dclutch_registry_contract::{
    ActivatedExecutionReleaseSetViewV1, IDENTITY_BYTES, RELEASE_LINEAGE_BYTES_V1,
    RELEASE_LINEAGE_PDA_DOMAIN_V1, ReleaseLineageV1,
};
use dclutch_registry_svm::lineage_v1::{
    DECLARE_SUCCESSOR_ACCOUNT_COUNT_V1, DECLARE_SUCCESSOR_AUTHORITY_BASE_ACCOUNT_V1,
    DeclareSuccessorV1,
};
use dclutch_release_set_contract::{
    EXECUTION_ROLE_COUNT_V1, EXECUTION_ROLE_ORDER_V1, ExecutionRoleV1,
};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, program::invoke_signed,
    program_error::ProgramError, pubkey::Pubkey, rent::Rent,
};
use solana_sdk_ids::system_program;
use solana_system_interface::instruction::create_account;

use crate::{RegistryError, authenticate_cache_identity, authenticate_rent_and_system, next};

/// One declaration frame, already width- and privilege-checked.
struct DeclareFrame<'accounts, 'info> {
    payer: &'accounts AccountInfo<'info>,
    lineage: &'accounts AccountInfo<'info>,
    predecessor_cache: &'accounts AccountInfo<'info>,
    successor_cache: &'accounts AccountInfo<'info>,
    authority: [&'accounts AccountInfo<'info>; EXECUTION_ROLE_COUNT_V1],
    system: &'accounts AccountInfo<'info>,
    rent_sysvar: &'accounts AccountInfo<'info>,
}

/// Declare that one release set is superseded by another.
#[inline(never)]
pub(crate) fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    DeclareSuccessorV1::decode(instruction_data).map_err(|_| RegistryError::Instruction)?;
    let frame = DeclareFrame::parse(accounts)?;
    let rent = authenticate_rent_and_system(frame.system, frame.rent_sysvar)?;

    let lineage = {
        let predecessor_bytes = frame
            .predecessor_cache
            .try_borrow_data()
            .map_err(|_| RegistryError::Borrow)?;
        let successor_bytes = frame
            .successor_cache
            .try_borrow_data()
            .map_err(|_| RegistryError::Borrow)?;
        let predecessor = decode_cache(program_id, frame.predecessor_cache, &predecessor_bytes)?;
        let successor = decode_cache(program_id, frame.successor_cache, &successor_bytes)?;
        compose_lineage(predecessor, successor, &frame.authority)?
    };

    create_lineage_record(program_id, &frame, &rent, lineage)
}

impl<'accounts, 'info> DeclareFrame<'accounts, 'info> {
    fn parse(accounts: &'accounts [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        if accounts.len() != DECLARE_SUCCESSOR_ACCOUNT_COUNT_V1 {
            return Err(RegistryError::AccountFrame.into());
        }
        let mut iterator = accounts.iter();
        let payer = next(&mut iterator)?;
        let lineage = next(&mut iterator)?;
        let predecessor_cache = next(&mut iterator)?;
        let successor_cache = next(&mut iterator)?;
        let authority = [
            next(&mut iterator)?,
            next(&mut iterator)?,
            next(&mut iterator)?,
            next(&mut iterator)?,
            next(&mut iterator)?,
        ];
        let system = next(&mut iterator)?;
        let rent_sysvar = next(&mut iterator)?;
        let value = Self {
            payer,
            lineage,
            predecessor_cache,
            successor_cache,
            authority,
            system,
            rent_sysvar,
        };
        value.validate_privileges()?;
        Ok(value)
    }

    /// Conjunct 1: the frame is exactly as tabled.
    ///
    /// The per-role signer bit is deliberately NOT decided here — whether a
    /// role's slot must sign is a function of whether its artifact moved, which
    /// is not known until both caches are decoded. Conjunct 6 owns it.
    fn validate_privileges(&self) -> ProgramResult {
        if !self.payer.is_signer
            || !self.payer.is_writable
            || self.payer.executable
            || self.lineage.is_signer
            || !self.lineage.is_writable
            || self.lineage.executable
        {
            return Err(RegistryError::AccountFrame.into());
        }
        for cache in [self.predecessor_cache, self.successor_cache] {
            if cache.is_signer || cache.is_writable || cache.executable {
                return Err(RegistryError::AccountFrame.into());
            }
        }
        for slot in self.authority {
            // The executable refusal is what keeps a program out of a consent
            // slot: a program holds no private key, so a program standing here
            // could only ever look like a consent nothing was able to give.
            //
            // The System Program is the one account this route itself REQUIRES
            // in a consent slot -- conjunct 6 asks for exactly
            // `system_program::ID` in an unmoved role's slot -- and every runtime
            // presents that account as executable. Refusing it here made the two
            // conjuncts mutually unsatisfiable and left every hop with an unmoved
            // role undeclarable by any caller.
            //
            // The exemption concedes nothing, because it decides nothing.
            // Whether that account may stand in a given slot is still conjunct
            // 6's, and there it is admitted only for a role that did NOT move,
            // where it must not sign and no consent is recorded. In a moved
            // role's slot conjunct 6 refuses it, needing a signature that account
            // cannot produce. Every other executable account is refused here
            // exactly as before.
            if slot.is_writable || (slot.executable && slot.key != &system_program::ID) {
                return Err(RegistryError::AccountFrame.into());
            }
        }
        if self.system.is_signer
            || self.system.is_writable
            || !self.system.executable
            || self.rent_sysvar.is_signer
            || self.rent_sysvar.is_writable
            || self.rent_sysvar.executable
        {
            return Err(RegistryError::AccountFrame.into());
        }
        Ok(())
    }
}

/// Conjunct 2: a Registry-owned activation cache at its own derived address.
///
/// Decoding is also the proof that all five roles are activated — a partially
/// written cache cannot decode — so this one check is what guarantees a market
/// that migrates lands somewhere immediately operable.
fn decode_cache<'a>(
    program_id: &Pubkey,
    cache: &AccountInfo<'_>,
    bytes: &'a [u8],
) -> Result<ActivatedExecutionReleaseSetViewV1<'a>, ProgramError> {
    let view = ActivatedExecutionReleaseSetViewV1::decode(bytes)
        .map_err(|_| RegistryError::ActivationCache)?;
    authenticate_cache_identity(program_id, cache, view)?;
    Ok(view)
}

/// Conjuncts 3 through 6, and the record they compose.
fn compose_lineage(
    predecessor: ActivatedExecutionReleaseSetViewV1<'_>,
    successor: ActivatedExecutionReleaseSetViewV1<'_>,
    authority: &[&AccountInfo<'_>; EXECUTION_ROLE_COUNT_V1],
) -> Result<ReleaseLineageV1, ProgramError> {
    let before_id = predecessor
        .execution_release_set_id()
        .map_err(|_| RegistryError::ActivationCache)?;
    let after_id = successor
        .execution_release_set_id()
        .map_err(|_| RegistryError::ActivationCache)?;

    // Conjunct 3.
    if before_id == after_id {
        return Err(RegistryError::ReleaseLineageSelfSuccession.into());
    }

    // Conjunct 4. A hop may move a role's bytes, never its identity. This is
    // the conjunct that keeps every child address in the protocol fixed, so it
    // is checked for every role before anything else is decided.
    for role in EXECUTION_ROLE_ORDER_V1 {
        let (before, after) = pair(predecessor, successor, role)?;
        if before.release().program() != after.release().program() {
            return Err(RegistryError::ReleaseLineageRoleIdentityMoved.into());
        }
    }

    // Conjunct 5. Under Loader V3 a ProgramData slot only moves forward, so a
    // strictly greater slot is exactly "was upgraded after". An unmoved role
    // needs no slot check: an identical artifact release id is an identical
    // 216-byte record, slot included.
    for role in EXECUTION_ROLE_ORDER_V1 {
        let (before, after) = pair(predecessor, successor, role)?;
        if moved(before, after)
            && after.release().deployment_slot() <= before.release().deployment_slot()
        {
            return Err(RegistryError::ReleaseLineageNotForward.into());
        }
    }

    // Conjunct 6.
    let mut consent = [None; EXECUTION_ROLE_COUNT_V1];
    for role in EXECUTION_ROLE_ORDER_V1 {
        let (before, after) = pair(predecessor, successor, role)?;
        let index = role.role_index();
        let slot = *authority
            .get(index)
            .ok_or(RegistryError::AccountFrame)?;
        if moved(before, after) {
            // An `Immutable` artifact binds no authority, so a hop claiming it
            // moved is a contradiction rather than a missing signature.
            let bound = after
                .release()
                .upgrade_authority()
                .ok_or(RegistryError::ReleaseLineageAuthorityMissing)?;
            if !slot.is_signer || slot.key.to_bytes() != bound {
                return Err(RegistryError::ReleaseLineageAuthorityMissing.into());
            }
            if let Some(entry) = consent.get_mut(index) {
                *entry = Some(bound);
            }
        } else if slot.is_signer || slot.key != &system_program::ID {
            // An unmoved role's binding is byte-identical on both sides and so
            // makes no new claim. Nothing is being consented to, so nothing may
            // stand in the slot that could look like consent.
            return Err(RegistryError::ReleaseLineageAuthorityMissing.into());
        }
    }

    // Under conjuncts 3 and 4 this constructor cannot refuse: equal programs
    // plus no moved artifact would mean equal projections, hence equal set ids,
    // which conjunct 3 already refused. It is a belt on that argument.
    ReleaseLineageV1::new(before_id, after_id, consent)
        .map_err(|_| RegistryError::Release.into())
}

fn pair<'a>(
    predecessor: ActivatedExecutionReleaseSetViewV1<'a>,
    successor: ActivatedExecutionReleaseSetViewV1<'a>,
    role: ExecutionRoleV1,
) -> Result<
    (
        dclutch_registry_contract::ActivatedRoleV1,
        dclutch_registry_contract::ActivatedRoleV1,
    ),
    ProgramError,
> {
    let before = predecessor
        .role(role)
        .map_err(|_| RegistryError::ActivationCache)?;
    let after = successor
        .role(role)
        .map_err(|_| RegistryError::ActivationCache)?;
    Ok((before, after))
}

/// The sole definition of "this role's artifact moved across the hop".
fn moved(
    before: dclutch_registry_contract::ActivatedRoleV1,
    after: dclutch_registry_contract::ActivatedRoleV1,
) -> bool {
    before.artifact_release_id() != after.artifact_release_id()
}

/// Conjunct 7: the record's address, and the vacancy that forbids a fork.
///
/// Split out from the creation so the whole admitted path can be checked
/// without invoking the System program — which matters because a `invoke_signed`
/// from a unit test perturbs process-global syscall stubs and corrupts tests
/// running beside it.
fn authenticate_pristine_lineage_account(
    program_id: &Pubkey,
    lineage_account: &AccountInfo<'_>,
    lineage: ReleaseLineageV1,
) -> Result<u8, ProgramError> {
    let (expected, bump) =
        release_lineage_address_and_bump_v1(program_id, lineage.predecessor().as_bytes());
    if lineage_account.key != &expected {
        return Err(RegistryError::AccountFrame.into());
    }
    // This is the entire no-fork guarantee: a second declaration for the same
    // predecessor finds an account that is no longer pristine.
    if lineage_account.owner != &system_program::ID
        || lineage_account.executable
        || lineage_account.lamports() != 0
        || lineage_account.data_len() != 0
    {
        return Err(RegistryError::ReleaseLineageAlreadyDeclared.into());
    }
    Ok(bump)
}

/// Conjuncts 7 and 8: the pristine account, its creation, and the belt.
fn create_lineage_record(
    program_id: &Pubkey,
    frame: &DeclareFrame<'_, '_>,
    rent: &Rent,
    lineage: ReleaseLineageV1,
) -> ProgramResult {
    let predecessor = lineage.predecessor();
    let bump = authenticate_pristine_lineage_account(program_id, frame.lineage, lineage)?;

    let space = u64::try_from(RELEASE_LINEAGE_BYTES_V1).map_err(|_| RegistryError::Arithmetic)?;
    let lamports = rent.minimum_balance(RELEASE_LINEAGE_BYTES_V1);
    let bump_seed = [bump];
    let signer: [&[u8]; 3] = [
        RELEASE_LINEAGE_PDA_DOMAIN_V1,
        predecessor.as_bytes(),
        bump_seed.as_slice(),
    ];
    invoke_signed(
        &create_account(
            frame.payer.key,
            frame.lineage.key,
            lamports,
            space,
            program_id,
        ),
        &[
            frame.payer.clone(),
            frame.lineage.clone(),
            frame.system.clone(),
        ],
        &[&signer],
    )
    .map_err(|_| RegistryError::CreateCpi)?;
    if frame.lineage.owner != program_id
        || frame.lineage.executable
        || frame.lineage.data_len() != RELEASE_LINEAGE_BYTES_V1
        || frame.lineage.lamports() != lamports
    {
        return Err(RegistryError::CreateCpi.into());
    }

    let encoded = lineage.to_bytes();
    {
        let mut output = frame
            .lineage
            .try_borrow_mut_data()
            .map_err(|_| RegistryError::Borrow)?;
        output
            .get_mut(..RELEASE_LINEAGE_BYTES_V1)
            .ok_or(RegistryError::ActivationCache)?
            .copy_from_slice(&encoded);
    }
    // Conjunct 8: read back what was actually persisted rather than trusting
    // the buffer that was written, the `require_consistent_completion` belt.
    let written = frame
        .lineage
        .try_borrow_data()
        .map_err(|_| RegistryError::Borrow)?;
    let decoded = ReleaseLineageV1::decode(&written).map_err(|_| RegistryError::Release)?;
    if decoded != lineage {
        return Err(RegistryError::Release.into());
    }
    Ok(())
}

/// The declaration's consent projection, for tests and for an auditor.
///
/// Exported so a test can exercise conjuncts 3 through 6 against real decoded
/// caches without a runtime able to serve the creation CPI.
/// Conjunct 7's checks alone, for a test with no runtime to create accounts.
#[cfg(test)]
pub(crate) fn authenticate_pristine_lineage_account_for_test(
    program_id: &Pubkey,
    lineage_account: &AccountInfo<'_>,
    lineage: ReleaseLineageV1,
) -> Result<u8, ProgramError> {
    authenticate_pristine_lineage_account(program_id, lineage_account, lineage)
}

/// Conjunct 1 alone: the width and the privilege sweep, nothing decoded.
///
/// Exported because conjunct 1 has an ADMITTING direction that no refusal test
/// can reach. Every other unit test here observes it only by the error that does
/// not come back, and the canonical frame cannot be driven through
/// `process_instruction` to the end because the creation is a System CPI no
/// unit-test runtime serves.
#[cfg(test)]
pub(crate) fn validate_declaration_frame_for_test(accounts: &[AccountInfo<'_>]) -> ProgramResult {
    DeclareFrame::parse(accounts).map(|_| ())
}

#[cfg(test)]
pub(crate) fn compose_lineage_for_test(
    predecessor: ActivatedExecutionReleaseSetViewV1<'_>,
    successor: ActivatedExecutionReleaseSetViewV1<'_>,
    authority: &[&AccountInfo<'_>; EXECUTION_ROLE_COUNT_V1],
) -> Result<ReleaseLineageV1, ProgramError> {
    compose_lineage(predecessor, successor, authority)
}

const _: () = assert!(
    IDENTITY_BYTES == 32,
    "a consenting authority is one 32-byte key"
);

/// The one name for a role's consent slot in the declaration frame.
#[cfg(test)]
pub(crate) const fn authority_account_index(role: ExecutionRoleV1) -> usize {
    DECLARE_SUCCESSOR_AUTHORITY_BASE_ACCOUNT_V1 + role.role_index()
}

const _: () = assert!(
    DECLARE_SUCCESSOR_AUTHORITY_BASE_ACCOUNT_V1 + EXECUTION_ROLE_COUNT_V1
        <= DECLARE_SUCCESSOR_ACCOUNT_COUNT_V1,
    "the consent block must fit inside the declaration frame"
);

const _: () = assert!(
    RELEASE_LINEAGE_PDA_DOMAIN_V1.len() == 26,
    "the lineage signer seed must keep the domain width it signs with"
);

