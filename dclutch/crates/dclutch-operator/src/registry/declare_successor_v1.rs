//! Chain-derived unsigned release-set successor declaration.
//!
//! `DeclareSuccessor` is the one Registry route whose wire carries no arguments
//! at all: both endpoints, the moved-role mask and every consenting authority
//! are read out of the accounts by the program. A host builder for it therefore
//! has exactly one job — present the eleven-account frame the program will
//! read — and exactly one temptation, which is to let the caller say who signs.
//!
//! It does not. [`RegistryDeclareSuccessorState`] has no authority field. Which
//! key must stand in which consent slot is a fact carried by the successor's own
//! activation cache, and this builder reads it there. A caller that could name
//! the consenting authority could name the wrong one, and a frame built around a
//! caller-named key would be refused on chain by conjunct 6 anyway — so naming
//! it here would buy nothing but a second author for the one fact the route
//! exists to collect. The builder projects; the chain authenticates.
//!
//! The two endpoints are likewise not taken as arguments. The caller supplies
//! two accounts; the release-set ids are read out of their bytes and each cache
//! address is re-derived from the id it carries
//! (`super::authenticate_cache_identity`), so a caller that points at the wrong
//! account is refused before a hop is composed rather than after it lands.
//!
//! Like every other module in this crate it performs no RPC, holds no key,
//! signs nothing and submits nothing. [`RegistryDeclareSuccessorReport`] names
//! the signatures the frame will require; obtaining them is the caller's
//! problem and deliberately not this crate's.

use dclutch_core_contract::ContentId;
use dclutch_registry::activation_auth_v1::{
    activation_cache_address_v1, release_lineage_address_and_bump_v1,
};
use dclutch_registry::release_set::{
    EXECUTION_ROLE_COUNT_V1, EXECUTION_ROLE_ORDER_V1, ExecutionRoleV1,
};
use dclutch_registry::svm::lineage_v1::{
    DECLARE_SUCCESSOR_ACCOUNT_COUNT_V1, DECLARE_SUCCESSOR_AUTHORITY_BASE_ACCOUNT_V1,
    DeclareSuccessorV1,
};
use dclutch_registry::{
    ActivatedExecutionReleaseSetViewV1, IDENTITY_BYTES, RELEASE_LINEAGE_BYTES_V1, ReleaseLineageV1,
};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::system_program;

use super::{
    Error, authenticate_aliases, authenticate_cache_identity, authenticate_payer,
    authenticate_system_program, decode_rent, same_observation,
};
use crate::{Observation, ObservedAccount};

/// Same-finalized inputs for one release-set successor declaration.
///
/// Six accounts, not eleven. The five per-role consent slots are absent because
/// they are derived, not supplied — see the module documentation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryDeclareSuccessorState {
    /// System wallet signing and paying the lineage record's rent.
    pub payer: ObservedAccount,
    /// The derived, pristine lineage address this declaration will create.
    pub lineage: ObservedAccount,
    /// The predecessor's activation cache: read for its bindings, never admitted.
    pub predecessor_cache: ObservedAccount,
    /// The successor's activation cache: bindings, slots and authorities.
    pub successor_cache: ObservedAccount,
    /// Canonical executable System Program.
    pub system_program: ObservedAccount,
    /// Canonical Rent sysvar.
    pub rent_sysvar: ObservedAccount,
}

/// What one role contributes to a declaration frame.
///
/// The pair `(slot, must_sign)` is the whole of conjunct 6 for this role, and
/// it is derived from the two caches rather than chosen. An unmoved role's slot
/// is `system_program::ID` and must NOT sign: its binding is byte-identical on
/// both sides, so it makes no new claim and nothing may stand where consent
/// would go.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeclareSuccessorRoleConsentV1 {
    /// The semantic role this slot speaks for.
    pub role: ExecutionRoleV1,
    /// Whether this role's artifact release id changed across the hop.
    pub moved: bool,
    /// The exact account the frame must carry in this role's consent slot.
    pub slot: Pubkey,
    /// Whether that account must sign the transaction.
    pub must_sign: bool,
}

/// Fully checked unsigned successor declaration and the record it would write.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryDeclareSuccessorReport {
    /// Exact unsigned eleven-account Registry instruction.
    pub instruction: Instruction,
    /// Shared finalized observation selecting every input.
    pub observation: Observation,
    /// Canonical derived lineage-record address, keyed by the PREDECESSOR.
    pub lineage: Pubkey,
    /// Canonical bump the Registry will sign the record into existence with.
    pub lineage_bump: u8,
    /// The release set this hop is declared FOR, read out of its own cache.
    pub predecessor: ContentId,
    /// The release set a market on the predecessor migrates to.
    pub successor: ContentId,
    /// The exact 248 bytes this declaration would persist, composed locally.
    ///
    /// A caller can print this and compare it against what actually lands. The
    /// record carries no clock, so a hop declared long after the fact composes
    /// to exactly the bytes it would have composed to at the time.
    pub record: ReleaseLineageV1,
    /// Per-role consent projection in canonical role order.
    pub consent: [DeclareSuccessorRoleConsentV1; EXECUTION_ROLE_COUNT_V1],
    /// Distinct keys whose signatures the frame requires, payer first.
    ///
    /// Roles commonly share one upgrade authority, so this is deduplicated: on
    /// a cluster whose five roles all bind the same deployer, a hop that moved
    /// four roles still needs exactly two signatures.
    pub required_signers: Vec<Pubkey>,
    /// Exact lamports the payer will spend creating the 248-byte record.
    pub lineage_rent_debit_lamports: u64,
}

/// Derive the three addresses one successor declaration reads and writes.
///
/// A caller holding two release-set ids and nothing else needs this to know
/// which accounts to fetch. It is the frame's coordinates only — every one of
/// them is re-derived from account BYTES inside
/// [`build_registry_declare_successor_v1`], so a wrong id here cannot smuggle a
/// wrong account past the builder.
#[must_use]
pub fn declare_successor_frame_addresses_v1(
    registry_program: Pubkey,
    predecessor_release_set_id: &[u8; 32],
    successor_release_set_id: &[u8; 32],
) -> DeclareSuccessorFrameAddressesV1 {
    let (lineage, lineage_bump) =
        release_lineage_address_and_bump_v1(&registry_program, predecessor_release_set_id);
    DeclareSuccessorFrameAddressesV1 {
        predecessor_cache: activation_cache_address_v1(
            &registry_program,
            predecessor_release_set_id,
        ),
        successor_cache: activation_cache_address_v1(&registry_program, successor_release_set_id),
        lineage,
        lineage_bump,
    }
}

/// The three derived coordinates of one declaration frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeclareSuccessorFrameAddressesV1 {
    /// The predecessor's activation cache.
    pub predecessor_cache: Pubkey,
    /// The successor's activation cache.
    pub successor_cache: Pubkey,
    /// The lineage record, keyed by the predecessor.
    pub lineage: Pubkey,
    /// The canonical bump for that record.
    pub lineage_bump: u8,
}

/// Build the exact eleven-account Registry successor declaration.
///
/// # Errors
///
/// Refuses locally on everything it can see the chain will refuse — a cache
/// that is not the Registry's own at its own derived address
/// ([`Error::InvalidActivationCache`]), the conjunct 3 through 5 hop checks
/// ([`Error::LineageSelfSuccession`], [`Error::LineageRoleIdentityMoved`],
/// [`Error::LineageNotForward`]), a moved role that binds no authority to ask
/// ([`Error::LineageAuthorityMissing`]), and a lineage address that is either
/// not the derived one or no longer pristine
/// ([`Error::LineageAlreadyDeclared`]). Building a frame the chain will refuse
/// is not a service to the caller.
pub fn build_registry_declare_successor_v1(
    registry_program: Pubkey,
    state: &RegistryDeclareSuccessorState,
) -> Result<RegistryDeclareSuccessorReport, Error> {
    let accounts = [
        &state.payer,
        &state.lineage,
        &state.predecessor_cache,
        &state.successor_cache,
        &state.system_program,
        &state.rent_sysvar,
    ];
    let observation = same_observation(&accounts)?;
    authenticate_aliases(&accounts)?;
    authenticate_payer(&state.payer)?;
    authenticate_system_program(&state.system_program)?;
    let rent = decode_rent(&state.rent_sysvar)?;

    // Conjunct 2, both sides: a Registry-owned cache of the one exact width, at
    // the address the release set it names derives. This is where the endpoints
    // come from — the caller supplied accounts, not ids.
    let predecessor = authenticate_cache_identity(registry_program, &state.predecessor_cache)?;
    let successor = authenticate_cache_identity(registry_program, &state.successor_cache)?;

    let (record, consent) = compose_hop(predecessor, successor)?;

    // Conjunct 7: the record's address, and the vacancy that forbids a fork.
    let (lineage, lineage_bump) =
        release_lineage_address_and_bump_v1(&registry_program, record.predecessor().as_bytes());
    if state.lineage.key != lineage {
        return Err(Error::InvalidLineageAddress);
    }
    if state.lineage.owner != system_program::ID
        || state.lineage.executable
        || state.lineage.lamports != 0
        || !state.lineage.data.is_empty()
    {
        return Err(Error::LineageAlreadyDeclared);
    }

    let lineage_rent_debit_lamports = rent.minimum_balance(RELEASE_LINEAGE_BYTES_V1);
    if state.payer.lamports < lineage_rent_debit_lamports {
        return Err(Error::InsufficientPayer);
    }

    let mut metas = Vec::with_capacity(DECLARE_SUCCESSOR_ACCOUNT_COUNT_V1);
    metas.push(AccountMeta::new(state.payer.key, true));
    metas.push(AccountMeta::new(lineage, false));
    metas.push(AccountMeta::new_readonly(
        state.predecessor_cache.key,
        false,
    ));
    metas.push(AccountMeta::new_readonly(state.successor_cache.key, false));
    if metas.len() != DECLARE_SUCCESSOR_AUTHORITY_BASE_ACCOUNT_V1 {
        return Err(Error::Encoding);
    }
    for role_consent in &consent {
        metas.push(AccountMeta::new_readonly(
            role_consent.slot,
            role_consent.must_sign,
        ));
    }
    metas.push(AccountMeta::new_readonly(state.system_program.key, false));
    metas.push(AccountMeta::new_readonly(state.rent_sysvar.key, false));
    if metas.len() != DECLARE_SUCCESSOR_ACCOUNT_COUNT_V1 {
        return Err(Error::Encoding);
    }

    let mut required_signers = vec![state.payer.key];
    for role_consent in &consent {
        if role_consent.must_sign && !required_signers.contains(&role_consent.slot) {
            required_signers.push(role_consent.slot);
        }
    }

    Ok(RegistryDeclareSuccessorReport {
        instruction: Instruction {
            program_id: registry_program,
            accounts: metas,
            data: DeclareSuccessorV1::to_bytes().to_vec(),
        },
        observation,
        lineage,
        lineage_bump,
        predecessor: record.predecessor(),
        successor: record.successor(),
        record,
        consent,
        required_signers,
        lineage_rent_debit_lamports,
    })
}

/// Conjuncts 3 through 6, projected from the two caches and nothing else.
///
/// This is deliberately the same shape as the program's own `compose_lineage`,
/// in the same order, reading the same fields — so that a frame this function
/// admits is one the program admits, and a hop the program would refuse is
/// refused here by the same name rather than discovered in a simulation log.
fn compose_hop(
    predecessor: ActivatedExecutionReleaseSetViewV1<'_>,
    successor: ActivatedExecutionReleaseSetViewV1<'_>,
) -> Result<
    (
        ReleaseLineageV1,
        [DeclareSuccessorRoleConsentV1; EXECUTION_ROLE_COUNT_V1],
    ),
    Error,
> {
    let before_id = predecessor
        .execution_release_set_id()
        .map_err(Error::Registry)?;
    let after_id = successor
        .execution_release_set_id()
        .map_err(Error::Registry)?;

    // Conjunct 3.
    if before_id == after_id {
        return Err(Error::LineageSelfSuccession);
    }

    let mut projected = [None; EXECUTION_ROLE_COUNT_V1];
    let mut consent = [None; EXECUTION_ROLE_COUNT_V1];
    for role in EXECUTION_ROLE_ORDER_V1 {
        let before = predecessor.role(role).map_err(Error::Registry)?;
        let after = successor.role(role).map_err(Error::Registry)?;
        let index = role.role_index();

        // Conjunct 4. A hop may move a role's bytes, never its identity.
        if before.release().program() != after.release().program() {
            return Err(Error::LineageRoleIdentityMoved);
        }

        let moved = before.artifact_release_id() != after.artifact_release_id();

        // Conjunct 5. An unmoved role needs no slot check: an identical artifact
        // release id is an identical record, slot included.
        if moved && after.release().deployment_slot() <= before.release().deployment_slot() {
            return Err(Error::LineageNotForward);
        }

        // Conjunct 6, projected. The moved role's consenting key is the one the
        // SUCCESSOR's cache binds — never a key this builder was handed.
        let (slot, authority) = if moved {
            // An `Immutable` artifact binds no authority, so a hop claiming it
            // moved is a contradiction rather than a missing signature.
            let bound = after
                .release()
                .upgrade_authority()
                .ok_or(Error::LineageAuthorityMissing)?;
            (Pubkey::new_from_array(bound), Some(bound))
        } else {
            (system_program::ID, None)
        };
        if let Some(entry) = projected.get_mut(index) {
            *entry = Some(DeclareSuccessorRoleConsentV1 {
                role,
                moved,
                slot,
                must_sign: moved,
            });
        }
        if let Some(entry) = consent.get_mut(index) {
            *entry = authority;
        }
    }

    let mut ordered = Vec::with_capacity(EXECUTION_ROLE_COUNT_V1);
    for entry in projected {
        ordered.push(entry.ok_or(Error::Encoding)?);
    }
    let projected: [DeclareSuccessorRoleConsentV1; EXECUTION_ROLE_COUNT_V1] =
        ordered.try_into().map_err(|_| Error::Encoding)?;

    let record = ReleaseLineageV1::new(before_id, after_id, consent).map_err(Error::Registry)?;
    Ok((record, projected))
}

const _: () = assert!(
    IDENTITY_BYTES == 32,
    "a consenting authority is one 32-byte key"
);

#[cfg(test)]
mod tests;
