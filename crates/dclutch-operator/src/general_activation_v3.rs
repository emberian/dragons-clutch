//! Chain-derived General V3 capability activation.
//!
//! This is the planner half of the General V3 activation adapter. It reads one
//! finalized snapshot of the live Core Market, the Market-selected capability
//! manifest, the finalized `GeneralConfigV3` record, the General-owned
//! `FundingStateV1`, and the capability-root account; it selects the unique
//! General manifest entry; it derives the composite root address from the
//! resulting `CapabilityExecutionSelectionV1`; and it calls
//! [`activate_general_owned_v3`] for the exact General-owned poststate. It
//! performs no RPC, no signing, no submission, and no account mutation.
//!
//! It deliberately does **not** build a Trading instruction. No in-tree route
//! can consume this plan yet, for two reasons this module's tests pin as
//! executable facts rather than prose:
//!
//! 1. `programs/dclutch-trading-sbf/src/outer.rs::process_activation` — the sole
//!    creator of capability roots — writes an all-zero family tail. A
//!    `GeneralRootV2` is refused at its magic, so the root that seam creates is
//!    not a root the General hot path will accept.
//! 2. That seam authenticates the record at `selection.capability_release()` as
//!    a `CapabilityProgramV1` (`DCLTCPR1`), while `hot_v3.rs` authenticates the
//!    record at the *same* selection field as a `CapabilityProgramSetV2`
//!    (`DCLTCPS2`). The selection is a seed of the root PDA, so one selection
//!    cannot satisfy both, and a General V3 root is therefore unreachable
//!    through the only route that creates roots.
//!
//! Both are recorded in the accompanying decision record. Until a successor
//! activation route exists, this planner is what produces the exact composite
//! root bytes every downstream General fixture and pre-commitment needs, and it
//! is the single place that computes them.

use dclutch_capability_contract::{
    ActivationPolicy, CapabilityEntryV1, CapabilityFundingDerivationV1, CapabilityManifestV1,
    ContentId, FUNDING_STATE_BYTES, FundingCustodyObservationV1, FundingStateV1,
};
use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityProgramV1, CapabilityRootHeaderV1,
};
use dclutch_general_config_contract::{
    GENERAL_CAPABILITY_KIND_ID_V1, GENERAL_ROOT_BYTES_V2, GENERAL_ROOT_SCHEMA_ID_V2,
    GeneralActivationDispositionV2, GeneralLifecycleV2, GeneralRootV2,
    root_v3::activate_general_owned_v3,
    v3::{GENERAL_CONFIG_BYTES_V3, GeneralConfigV3},
};
use dclutch_market_core_codec::{CoreState, MarketCoreStateSeedsV2, Phase, STATE_BYTES};
use dclutch_release_set_contract::CapabilityExecutionSelectionV1;
use solana_program::{hash::hash, pubkey::Pubkey};
use solana_sdk_ids::system_program;

use crate::{Finality, Observation, ObservedAccount};

/// Exact composite General capability-root account width.
pub const GENERAL_COMPOSITE_ROOT_BYTES_V3: usize =
    CAPABILITY_ROOT_HEADER_BYTES_V1 + GENERAL_ROOT_BYTES_V2;

/// One same-finalized chain snapshot for a General V3 activation attempt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneralActivationStateV3 {
    /// Core-owned canonical Market state account.
    pub market: ObservedAccount,
    /// Registry-owned finalized capability-manifest raw record.
    pub manifest_record: ObservedAccount,
    /// Registry-owned finalized `GeneralConfigV3` raw record.
    pub config_record: ObservedAccount,
    /// Trading-owned General `FundingStateV1` account.
    pub funding_state: ObservedAccount,
    /// The composite capability root: System-owned and vacant, or already created.
    pub capability_root: ObservedAccount,
    /// Registry-authenticated current Core program.
    pub core_program: Pubkey,
    /// Registry-authenticated current Trading program.
    pub trading_program: Pubkey,
    /// Exact Rent-exempt minimum for [`GENERAL_COMPOSITE_ROOT_BYTES_V3`].
    pub exact_root_rent_lamports: u64,
    /// Exact Rent-exempt minimum for [`FUNDING_STATE_BYTES`].
    pub exact_funding_rent_lamports: u64,
    /// Slot the activation would execute in.
    pub current_slot: u64,
    /// Lowest finalized slot accepted for this attempt.
    pub minimum_finalized_slot: u64,
}

/// Complete chain-derived General V3 activation plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneralCapabilityActivationV3 {
    /// Manifest-bound activation selection; also the composite root's PDA seeds.
    pub selection: CapabilityExecutionSelectionV1,
    /// Immutable composite-root header.
    pub root_header: CapabilityRootHeaderV1,
    /// Canonical composite-root address under the Trading program.
    pub root: Pubkey,
    /// Canonical composite-root PDA bump.
    pub root_bump: u8,
    /// Exact `CapabilityRootHeaderV1 || GeneralRootV2` account bytes.
    pub composite_root: Vec<u8>,
    /// Whether the root is created or exactly replayed.
    pub disposition: GeneralActivationDispositionV2,
    /// Exact General-owned mutable root tail.
    pub root_state: GeneralRootV2,
    /// Exact General-owned `FundingStateV1` poststate.
    pub funding_after: FundingStateV1,
    /// Unique General manifest entry index.
    pub entry_index: u16,
    /// Exact finalized observation shared by every input.
    pub observation: Observation,
}

/// Stable refusal from chain-derived General V3 activation planning.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralActivationErrorV3 {
    /// Accounts did not share one sufficiently recent finalized snapshot.
    Snapshot,
    /// Market owner, width, canonical encoding, or derived address differed.
    Market,
    /// The manifest record did not hash to the Market-selected manifest.
    Manifest,
    /// No unique General entry, or the entry disagreed with the config.
    Entry,
    /// Config bytes, width, or record digest refused.
    Config,
    /// FundingState owner, width, derivation, or custody observation refused.
    Funding,
    /// The root account was neither exactly vacant nor an exact prior activation.
    Root,
    /// Market phase did not admit the entry's immutable activation policy.
    Phase,
    /// The General activation contract refused.
    Activation(dclutch_general_config_contract::RootError),
    /// Checked arithmetic or an encoding width overflowed.
    Arithmetic,
}

/// Plan one General V3 capability activation from one finalized snapshot.
pub fn plan_general_capability_activation_v3(
    state: &GeneralActivationStateV3,
) -> Result<GeneralCapabilityActivationV3, GeneralActivationErrorV3> {
    let observation = require_one_finalized_snapshot(state)?;
    let core = authenticate_market(state)?;
    let manifest_id = content(core.identity.capability_manifest.to_bytes())?;
    if hash(&state.manifest_record.data).to_bytes() != manifest_id.to_bytes() {
        return Err(GeneralActivationErrorV3::Manifest);
    }
    let manifest = CapabilityManifestV1::decode(&state.manifest_record.data)
        .map_err(|_| GeneralActivationErrorV3::Manifest)?;

    if state.config_record.data.len() != GENERAL_CONFIG_BYTES_V3 {
        return Err(GeneralActivationErrorV3::Config);
    }
    let config = GeneralConfigV3::decode(&state.config_record.data)
        .map_err(|_| GeneralActivationErrorV3::Config)?;
    let config_id = content(hash(&state.config_record.data).to_bytes())?;

    let (entry_index, entry) = select_general_entry(manifest, config_id)?;
    require_admissible_phase(entry, core.phase)?;

    let market_key = state.market.key.to_bytes();
    let generation = core.identity.generation;
    let selection = CapabilityExecutionSelectionV1::new(
        entry_index,
        manifest_id,
        entry.kind_id(),
        entry.release_id(),
        config_id,
    )
    .map_err(|_| GeneralActivationErrorV3::Entry)?;
    let root_header = CapabilityRootHeaderV1::new(
        content(core.identity.selected_release_set.to_bytes())?,
        market_key,
        generation,
        selection,
    )
    .map_err(|_| GeneralActivationErrorV3::Root)?;
    let (root, root_bump) =
        Pubkey::find_program_address(&root_header.seeds().as_slices(), &state.trading_program);
    if root != state.capability_root.key {
        return Err(GeneralActivationErrorV3::Root);
    }

    let funding = authenticate_funding(state, manifest_id, manifest, root_header)?;
    let existing_root_state = authenticate_root_prestate(state)?;
    let custody = FundingCustodyObservationV1::native_only(
        state.funding_state.lamports,
        state.exact_funding_rent_lamports,
    )
    .map_err(|_| GeneralActivationErrorV3::Funding)?;

    let activation = activate_general_owned_v3(
        market_key,
        generation,
        manifest_id,
        manifest,
        entry_index,
        config_id,
        config,
        funding,
        custody,
        state.current_slot,
        state.exact_root_rent_lamports,
        state.capability_root.lamports,
        existing_root_state,
    )
    .map_err(GeneralActivationErrorV3::Activation)?;

    let composite_root = compose_general_root_v3(root_header, activation.root_state());
    Ok(GeneralCapabilityActivationV3 {
        selection,
        root_header,
        root,
        root_bump,
        composite_root,
        disposition: activation.disposition(),
        root_state: activation.root_state(),
        funding_after: activation.funding_after(),
        entry_index,
        observation,
    })
}

/// Assemble the exact `CapabilityRootHeaderV1 || GeneralRootV2` account bytes.
///
/// This mirrors `initialize_root_account_v1` exactly. That function cannot be
/// used here because it takes a `CapabilityProgramV1`, and the V3 General
/// descriptor generation is `CapabilityProgramV4`; the equivalence is pinned by
/// `composition_is_byte_identical_to_initialize_root_account_v1`.
#[must_use]
pub fn compose_general_root_v3(header: CapabilityRootHeaderV1, state: GeneralRootV2) -> Vec<u8> {
    let mut output = Vec::with_capacity(GENERAL_COMPOSITE_ROOT_BYTES_V3);
    output.extend_from_slice(&header.to_bytes());
    output.extend_from_slice(&state.to_bytes());
    output
}

/// Derive the canonical composite General capability-root address.
#[must_use]
pub fn general_capability_root_address_v3(
    header: CapabilityRootHeaderV1,
    trading_program: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(&header.seeds().as_slices(), trading_program)
}

/// Select the unique General manifest entry for one config identity.
///
/// This is the operator's projection of the same conjunction
/// `dclutch_general_config_contract` evaluates, and it is never authority: an
/// ambiguous or absent entry refuses here exactly as it refuses on chain.
fn select_general_entry(
    manifest: CapabilityManifestV1<'_>,
    config_id: ContentId,
) -> Result<(u16, CapabilityEntryV1), GeneralActivationErrorV3> {
    let kind = ContentId::new(GENERAL_CAPABILITY_KIND_ID_V1)
        .map_err(|_| GeneralActivationErrorV3::Entry)?;
    let mut selected: Option<(u16, CapabilityEntryV1)> = None;
    let mut index = 0_u16;
    while index < manifest.entry_count() {
        let entry = manifest
            .entry(index)
            .map_err(|_| GeneralActivationErrorV3::Entry)?;
        if entry.kind_id() == kind && entry.config_id() == config_id {
            if selected.is_some() {
                return Err(GeneralActivationErrorV3::Entry);
            }
            selected = Some((index, entry));
        }
        index = index
            .checked_add(1)
            .ok_or(GeneralActivationErrorV3::Arithmetic)?;
    }
    let (index, entry) = selected.ok_or(GeneralActivationErrorV3::Entry)?;
    if entry.child_schema_id().to_bytes() != GENERAL_ROOT_SCHEMA_ID_V2 {
        return Err(GeneralActivationErrorV3::Entry);
    }
    Ok((index, entry))
}

fn require_admissible_phase(
    entry: CapabilityEntryV1,
    phase: Phase,
) -> Result<(), GeneralActivationErrorV3> {
    match (entry.activation_policy(), phase) {
        (ActivationPolicy::RequiredAtFounding, Phase::Founding)
        | (ActivationPolicy::PrepaidLazy, Phase::Founding | Phase::Open) => Ok(()),
        _ => Err(GeneralActivationErrorV3::Phase),
    }
}

fn require_one_finalized_snapshot(
    state: &GeneralActivationStateV3,
) -> Result<Observation, GeneralActivationErrorV3> {
    let observation = state.market.observation;
    if observation.finality != Finality::Finalized
        || observation.slot == 0
        || observation.slot < state.minimum_finalized_slot
        || state.current_slot < observation.slot
    {
        return Err(GeneralActivationErrorV3::Snapshot);
    }
    for account in [
        &state.manifest_record,
        &state.config_record,
        &state.funding_state,
        &state.capability_root,
    ] {
        if account.observation != observation {
            return Err(GeneralActivationErrorV3::Snapshot);
        }
    }
    Ok(observation)
}

fn authenticate_market(
    state: &GeneralActivationStateV3,
) -> Result<CoreState, GeneralActivationErrorV3> {
    if state.market.owner != state.core_program
        || state.market.executable
        || state.market.data.len() != STATE_BYTES
    {
        return Err(GeneralActivationErrorV3::Market);
    }
    let core =
        CoreState::decode(&state.market.data).map_err(|_| GeneralActivationErrorV3::Market)?;
    let canonical = core
        .encode()
        .map_err(|_| GeneralActivationErrorV3::Market)?;
    let seeds = MarketCoreStateSeedsV2::new(core.identity);
    let expected = Pubkey::find_program_address(&seeds.as_slices(), &state.core_program).0;
    if canonical.as_slice() != state.market.data.as_slice()
        || expected != state.market.key
        || core.identity.market_id.to_bytes() != state.market.key.to_bytes()
    {
        return Err(GeneralActivationErrorV3::Market);
    }
    Ok(core)
}

fn authenticate_funding(
    state: &GeneralActivationStateV3,
    manifest_id: ContentId,
    manifest: CapabilityManifestV1<'_>,
    root_header: CapabilityRootHeaderV1,
) -> Result<FundingStateV1, GeneralActivationErrorV3> {
    if state.funding_state.owner != state.trading_program
        || state.funding_state.executable
        || state.funding_state.data.len() != FUNDING_STATE_BYTES
    {
        return Err(GeneralActivationErrorV3::Funding);
    }
    let funding = FundingStateV1::decode(&state.funding_state.data)
        .map_err(|_| GeneralActivationErrorV3::Funding)?;
    let derivation = CapabilityFundingDerivationV1::new(
        root_header.market(),
        root_header.generation(),
        manifest_id,
        manifest,
        funding,
    )
    .map_err(|_| GeneralActivationErrorV3::Funding)?;
    let expected =
        Pubkey::find_program_address(&derivation.seed_components(), &state.trading_program).0;
    if expected != state.funding_state.key {
        return Err(GeneralActivationErrorV3::Funding);
    }
    Ok(funding)
}

fn authenticate_root_prestate(
    state: &GeneralActivationStateV3,
) -> Result<Option<GeneralRootV2>, GeneralActivationErrorV3> {
    let root = &state.capability_root;
    if root.executable {
        return Err(GeneralActivationErrorV3::Root);
    }
    if root.data.is_empty() {
        if root.owner != system_program::ID {
            return Err(GeneralActivationErrorV3::Root);
        }
        return Ok(None);
    }
    if root.owner != state.trading_program || root.data.len() != GENERAL_COMPOSITE_ROOT_BYTES_V3 {
        return Err(GeneralActivationErrorV3::Root);
    }
    let tail = root
        .data
        .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
        .ok_or(GeneralActivationErrorV3::Root)?;
    GeneralRootV2::decode(tail)
        .map(Some)
        .map_err(GeneralActivationErrorV3::Activation)
}

fn content(bytes: [u8; 32]) -> Result<ContentId, GeneralActivationErrorV3> {
    ContentId::new(bytes).map_err(|_| GeneralActivationErrorV3::Arithmetic)
}

/// Retire a planned General root, for hostile fixtures that need a zombie.
///
/// This is a fixture and analysis helper: it applies the semantic owner's own
/// lifecycle transitions to a planned root so that a caller can build the exact
/// composite bytes a `Retiring` or `Retired` capability would present. It never
/// bypasses `GeneralRootV2`'s guards.
pub fn retire_planned_general_root_v3(
    plan: &GeneralCapabilityActivationV3,
    terminal: GeneralLifecycleV2,
) -> Result<GeneralCapabilityActivationV3, GeneralActivationErrorV3> {
    let mut state = plan.root_state;
    match terminal {
        GeneralLifecycleV2::Active => {}
        GeneralLifecycleV2::Retiring => state
            .begin_retiring(state.revision())
            .map_err(GeneralActivationErrorV3::Activation)?,
        GeneralLifecycleV2::Retired => {
            state
                .begin_retiring(state.revision())
                .map_err(GeneralActivationErrorV3::Activation)?;
            state
                .retire(state.revision())
                .map_err(GeneralActivationErrorV3::Activation)?;
        }
    }
    Ok(GeneralCapabilityActivationV3 {
        composite_root: compose_general_root_v3(plan.root_header, state),
        root_state: state,
        ..plan.clone()
    })
}

/// Whether the sole in-tree root-creating seam can consume a V3 selection.
///
/// `outer.rs::process_activation` authenticates the record at
/// `selection.capability_release()` as a `CapabilityProgramV1`. For a General
/// V3 capability that field is the digest of a `CapabilityProgramSetV2`, whose
/// bytes are not a `CapabilityProgramV1`. This returns the refusal that seam
/// would produce, so the gap is a value a caller can branch on rather than a
/// comment.
#[must_use]
pub fn common_activation_seam_admits_v3(capability_release_record: &[u8]) -> bool {
    CapabilityProgramV1::decode(capability_release_record).is_ok()
}

#[cfg(test)]
mod tests;
