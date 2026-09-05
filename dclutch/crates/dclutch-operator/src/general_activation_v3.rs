//! Chain-derived General V3 capability activation.
//!
//! This is the planner half of the General V3 activation adapter. It reads one
//! finalized snapshot of the live Core Market, the Market-selected capability
//! manifest, the finalized `GeneralConfigV3` record, the General-owned
//! ordered Trading-owned `FundingLedgerV2` set, and the capability-root
//! account; it selects the unique
//! General manifest entry; it derives the composite root address from the
//! resulting `CapabilityExecutionSelectionV1`; and it derives the exact
//! General-owned root plus selected-row funding poststate. It performs no RPC,
//! no signing, no submission, and no account mutation.
//!
//! It deliberately does **not** build a Trading instruction, and it no longer
//! needs to. The two blockers this header used to name are both closed:
//!
//! 1. `programs/dclutch-trading-sbf/src/outer.rs::process_activation` — the sole
//!    creator of capability roots — used to write an all-zero family tail. Since
//!    `ec3731d` the tail IS the effect program's projected request buffer, and
//!    an activation that projects nothing into a nonzero tail refuses rather
//!    than committing a root no family can decode.
//! 2. That seam used to authenticate the record at
//!    `selection.capability_release()` as a `CapabilityProgramV1` (`DCLTCPR1`)
//!    only. Since `bc5da76` it reads the release generation off the raw
//!    record's OWN PDA and admits a `CapabilityProgramSetV2` (`DCLTCPS2`),
//!    selecting the activation descriptor out of the set. That is a fact about
//!    a finalized record's address, not a kind branch.
//!
//! General's own three activation artifacts exist as of the GEN-ART lane, and
//! `programs/dclutch-trading-sbf/program-test/tests/activation.rs`
//! (`Campaign::General`) creates a real `GeneralRootV2` through that seam on a
//! validator, then requires this module's planner to agree with it byte for
//! byte on both the root tail and the selected FundingLedgerV2 row
//! poststate. So this planner is no longer the only thing that can produce
//! those bytes -- it is now one of two independent authorities that produce the
//! same ones, which is a considerably stronger position than it had.

use dclutch_market::capability_manifest::{
    ActivationPolicy, CapabilityEntryV1, CapabilityFundingLedgerDerivationV2, CapabilityManifestV1,
    ContentId, FUNDING_LEDGER_ACTIVE_ADMISSIBLE_STATES_V2,
    FUNDING_LEDGER_PENDING_ADMISSIBLE_STATES_V2, FundingLedgerV2, manifest_entry_for_ledger_row_v2,
    validate_funding_ledger_masks_v2,
};
use dclutch_market::capability_program::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1, SelectedRecordBumpsV1,
};
use dclutch_trading::general_config::{
    GENERAL_CAPABILITY_KIND_ID_V1, GENERAL_ROOT_BYTES_V2, GENERAL_ROOT_SCHEMA_ID_V2,
    GeneralActivationDispositionV2, GeneralLifecycleV2, GeneralRootV2,
    v3::{GENERAL_CONFIG_BYTES_V3, GeneralConfigV3},
};
use dclutch_market::{
    CapabilityFundingHeaderV2, CoreState, MarketCoreStateSeedsV2, Phase, STATE_BYTES,
};
use dclutch_registry::release_set::CapabilityExecutionSelectionV1;
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
    /// Ordered Trading-owned funding ledgers covering the dependency closure.
    pub funding_ledgers: Vec<GeneralFundingLedgerInputV2>,
    /// The composite capability root: System-owned and vacant, or already created.
    pub capability_root: ObservedAccount,
    /// Registry-authenticated current Core program.
    pub core_program: Pubkey,
    /// Registry-authenticated current Trading program.
    pub trading_program: Pubkey,
    /// Exact Rent-exempt minimum for [`GENERAL_COMPOSITE_ROOT_BYTES_V3`].
    pub exact_root_rent_lamports: u64,
    /// Slot the activation would execute in.
    pub current_slot: u64,
    /// Lowest finalized slot accepted for this attempt.
    pub minimum_finalized_slot: u64,
}

/// One canonical observed FundingLedgerV2 input and its chain-derived Rent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneralFundingLedgerInputV2 {
    /// Trading-owned ledger account at the controller-scoped canonical PDA.
    pub account: ObservedAccount,
    /// Exact Rent-exempt minimum for this ledger's dynamic byte width.
    pub exact_rent_lamports: u64,
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
    /// Canonical physical-ledger count and exact logical dependency union.
    pub funding_header: CapabilityFundingHeaderV2,
    /// Exact full poststate bytes in the input ledger-account order.
    pub funding_after: Vec<Vec<u8>>,
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
    /// FundingLedger owner, partition, derivation, or custody refused.
    Funding,
    /// The root account was neither exactly vacant nor an exact prior activation.
    Root,
    /// Market phase did not admit the entry's immutable activation policy.
    Phase,
    /// The General activation contract refused.
    Activation(dclutch_trading::general_config::RootError),
    /// Checked arithmetic or an encoding width overflowed.
    Arithmetic,
    /// `dclutch_market::capability_manifest` refused; the cause is its own.
    Capability(dclutch_market::capability_manifest::Error),
    /// `dclutch_trading::general_config` refused; the cause is its own.
    GeneralConfig(dclutch_trading::general_config::v3::GeneralConfigErrorV3),
    /// `dclutch_registry::release_set` refused; the cause is its own.
    ReleaseSet(dclutch_registry::release_set::Error),
    /// `dclutch_market::capability_program` refused; the cause is its own.
    CapabilityProgram(dclutch_market::capability_program::Error),
    /// `dclutch_market` refused; the cause is its own.
    MarketCore(dclutch_market::Error),
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
        .map_err(GeneralActivationErrorV3::Capability)?;

    if state.config_record.data.len() != GENERAL_CONFIG_BYTES_V3 {
        return Err(GeneralActivationErrorV3::Config);
    }
    let config = GeneralConfigV3::decode(&state.config_record.data)
        .map_err(GeneralActivationErrorV3::GeneralConfig)?;
    let config_id = content(hash(&state.config_record.data).to_bytes())?;

    let (entry_index, entry) = select_general_entry(manifest, config_id)?;
    require_admissible_phase(entry, core.phase)?;
    require_general_entry(entry, config_id, config, core.identity.generation)?;

    let market_key = state.market.key.to_bytes();
    let generation = core.identity.generation;
    let selection = CapabilityExecutionSelectionV1::new(
        entry_index,
        manifest_id,
        entry.kind_id(),
        entry.release_id(),
        config_id,
    )
    .map_err(GeneralActivationErrorV3::ReleaseSet)?;
    let root_header = CapabilityRootHeaderV1::new(
        content(core.identity.selected_release_set.to_bytes())?,
        market_key,
        generation,
        selection,
        // The planner builds this header to DERIVE the root address and the
        // funding coordinates; the root PDA seeds are the semantic identities
        // alone, so the record bumps the on-chain activation fills in are not
        // among them. Whatever the chain writes there does not move any address
        // this planner computes.
        SelectedRecordBumpsV1::default(),
    )
    .map_err(GeneralActivationErrorV3::CapabilityProgram)?;
    let (root, root_bump) =
        Pubkey::find_program_address(&root_header.seeds().as_slices(), &state.trading_program);
    if root != state.capability_root.key {
        return Err(GeneralActivationErrorV3::Root);
    }

    let required_mask = dependency_closure_mask(manifest, entry_index)?;
    let (funding_header, funding_prestate) =
        authenticate_funding_ledgers(state, manifest_id, manifest, root_header, required_mask)?;
    let existing_root_state = authenticate_root_prestate(state)?;
    let expected_root_state = GeneralRootV2::active(market_key, config_id.to_bytes(), generation)
        .map_err(GeneralActivationErrorV3::Activation)?;
    let (disposition, root_state, funding_after) = if let Some(present) = existing_root_state {
        if present != expected_root_state
            || state.capability_root.lamports != state.exact_root_rent_lamports
        {
            return Err(GeneralActivationErrorV3::Root);
        }
        require_funding_ledger_states(&funding_prestate, manifest_id, manifest, entry_index, true)?;
        (
            GeneralActivationDispositionV2::Idempotent,
            present,
            funding_prestate,
        )
    } else {
        require_funding_ledger_states(
            &funding_prestate,
            manifest_id,
            manifest,
            entry_index,
            false,
        )?;
        let (funding_after, selected_rent, selected_creation) = activate_funding_ledgers(
            funding_prestate,
            manifest_id,
            manifest,
            entry_index,
            state.current_slot,
        )?;
        if selected_creation != 0
            || selected_rent
                .checked_add(state.capability_root.lamports)
                .ok_or(GeneralActivationErrorV3::Arithmetic)?
                != state.exact_root_rent_lamports
        {
            return Err(GeneralActivationErrorV3::Funding);
        }
        (
            GeneralActivationDispositionV2::Create,
            expected_root_state,
            funding_after,
        )
    };

    let composite_root = compose_general_root_v3(root_header, root_state);
    Ok(GeneralCapabilityActivationV3 {
        selection,
        root_header,
        root,
        root_bump,
        composite_root,
        disposition,
        root_state,
        funding_header,
        funding_after,
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
/// `dclutch_trading::general_config` evaluates, and it is never authority: an
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
            .map_err(GeneralActivationErrorV3::Capability)?;
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
        &state.capability_root,
    ] {
        if account.observation != observation {
            return Err(GeneralActivationErrorV3::Snapshot);
        }
    }
    if state
        .funding_ledgers
        .iter()
        .any(|ledger| ledger.account.observation != observation)
    {
        return Err(GeneralActivationErrorV3::Snapshot);
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
        CoreState::decode(&state.market.data).map_err(GeneralActivationErrorV3::MarketCore)?;
    let canonical = core
        .encode()
        .map_err(GeneralActivationErrorV3::MarketCore)?;
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

fn authenticate_funding_ledgers(
    state: &GeneralActivationStateV3,
    manifest_id: ContentId,
    manifest: CapabilityManifestV1<'_>,
    root_header: CapabilityRootHeaderV1,
    required_mask: u16,
) -> Result<(CapabilityFundingHeaderV2, Vec<Vec<u8>>), GeneralActivationErrorV3> {
    let mut masks = Vec::with_capacity(state.funding_ledgers.len());
    let mut bytes = Vec::with_capacity(state.funding_ledgers.len());
    for input in &state.funding_ledgers {
        if input.account.owner != state.trading_program
            || input.account.executable
            || input.exact_rent_lamports == 0
        {
            return Err(GeneralActivationErrorV3::Funding);
        }
        let ledger = FundingLedgerV2::decode(&input.account.data)
            .map_err(GeneralActivationErrorV3::Capability)?;
        let selected_bit = 1_u16
            .checked_shl(u32::from(root_header.selection().entry_index()))
            .ok_or(GeneralActivationErrorV3::Arithmetic)?;
        if ledger.selected_mask() != selected_bit {
            // ManifestV1 does not bind a controller per entry. The offline
            // planner has no authenticated Core/Resolution release premise,
            // so it admits only the one Trading-owned selected-entry ledger.
            return Err(GeneralActivationErrorV3::Funding);
        }
        let authenticated = ledger
            .authenticate(manifest_id, manifest)
            .map_err(GeneralActivationErrorV3::Capability)?;
        let derivation = CapabilityFundingLedgerDerivationV2::new(
            state.trading_program.to_bytes(),
            root_header.market(),
            root_header.generation(),
            manifest_id,
            ledger,
        )
        .map_err(GeneralActivationErrorV3::Capability)?;
        let expected =
            Pubkey::find_program_address(&derivation.seed_components(), &state.trading_program).0;
        if expected != input.account.key {
            return Err(GeneralActivationErrorV3::Funding);
        }
        authenticated
            .validate_native_custody(input.account.lamports, input.exact_rent_lamports, false)
            .map_err(GeneralActivationErrorV3::Capability)?;
        let mut row_index = 0_u16;
        while row_index < ledger.slot_count() {
            let entry_index = manifest_entry_for_ledger_row_v2(ledger.selected_mask(), row_index)
                .map_err(GeneralActivationErrorV3::Capability)?;
            if manifest
                .entry(entry_index)
                .map_err(GeneralActivationErrorV3::Capability)?
                .funding_quote()
                .realm_collateral()
                .is_some()
            {
                return Err(GeneralActivationErrorV3::Funding);
            }
            row_index = row_index
                .checked_add(1)
                .ok_or(GeneralActivationErrorV3::Arithmetic)?;
        }
        masks.push(ledger.selected_mask());
        bytes.push(input.account.data.clone());
    }
    validate_funding_ledger_masks_v2(manifest.entry_count(), required_mask, &masks)
        .map_err(GeneralActivationErrorV3::Capability)?;
    let physical_count =
        u8::try_from(bytes.len()).map_err(|_| GeneralActivationErrorV3::Funding)?;
    let logical_count =
        u8::try_from(required_mask.count_ones()).map_err(|_| GeneralActivationErrorV3::Funding)?;
    let header = CapabilityFundingHeaderV2::new(physical_count, logical_count, required_mask)
        .map_err(GeneralActivationErrorV3::MarketCore)?;
    Ok((header, bytes))
}

fn dependency_closure_mask(
    manifest: CapabilityManifestV1<'_>,
    selected_entry_index: u16,
) -> Result<u16, GeneralActivationErrorV3> {
    let selected_bit = 1_u16
        .checked_shl(u32::from(selected_entry_index))
        .ok_or(GeneralActivationErrorV3::Entry)?;
    let mut closure = selected_bit;
    loop {
        let before = closure;
        let mut entry_index = 0_u16;
        while entry_index < manifest.entry_count() {
            let entry_bit = 1_u16
                .checked_shl(u32::from(entry_index))
                .ok_or(GeneralActivationErrorV3::Arithmetic)?;
            if closure & entry_bit != 0 {
                let entry = manifest
                    .entry(entry_index)
                    .map_err(GeneralActivationErrorV3::Capability)?;
                let mut position = 0_usize;
                while position < usize::from(entry.dependency_count()) {
                    let dependency = entry
                        .dependency(position)
                        .map_err(GeneralActivationErrorV3::Capability)?;
                    closure |= 1_u16
                        .checked_shl(u32::from(dependency))
                        .ok_or(GeneralActivationErrorV3::Arithmetic)?;
                    position = position
                        .checked_add(1)
                        .ok_or(GeneralActivationErrorV3::Arithmetic)?;
                }
            }
            entry_index = entry_index
                .checked_add(1)
                .ok_or(GeneralActivationErrorV3::Arithmetic)?;
        }
        if before == closure {
            return Ok(closure);
        }
    }
}

fn activate_funding_ledgers(
    mut ledgers: Vec<Vec<u8>>,
    manifest_id: ContentId,
    manifest: CapabilityManifestV1<'_>,
    selected_entry_index: u16,
    current_slot: u64,
) -> Result<(Vec<Vec<u8>>, u64, u64), GeneralActivationErrorV3> {
    let mut selected_debit = None;
    for bytes in &mut ledgers {
        let ledger =
            FundingLedgerV2::decode(bytes).map_err(GeneralActivationErrorV3::Capability)?;
        let selected_bit = 1_u16
            .checked_shl(u32::from(selected_entry_index))
            .ok_or(GeneralActivationErrorV3::Arithmetic)?;
        if ledger.selected_mask() & selected_bit != 0 {
            let debit = FundingLedgerV2::activate_in_place(
                bytes,
                manifest_id,
                manifest,
                selected_entry_index,
                current_slot,
            )
            .map_err(GeneralActivationErrorV3::Capability)?;
            if selected_debit.is_some() {
                return Err(GeneralActivationErrorV3::Funding);
            }
            selected_debit = Some((debit.rent_lamports(), debit.creation_lamports()));
        }
    }
    let (rent, creation) = selected_debit.ok_or(GeneralActivationErrorV3::Funding)?;
    Ok((ledgers, rent, creation))
}

fn require_funding_ledger_states(
    ledgers: &[Vec<u8>],
    manifest_id: ContentId,
    manifest: CapabilityManifestV1<'_>,
    selected_entry_index: u16,
    selected_active: bool,
) -> Result<(), GeneralActivationErrorV3> {
    let mut observed_selected = false;
    for bytes in ledgers {
        let authenticated = FundingLedgerV2::decode(bytes)
            .and_then(|ledger| ledger.authenticate(manifest_id, manifest))
            .map_err(GeneralActivationErrorV3::Capability)?;
        let ledger = authenticated.ledger();
        let mut row_index = 0_u16;
        while row_index < ledger.slot_count() {
            let entry_index = manifest_entry_for_ledger_row_v2(ledger.selected_mask(), row_index)
                .map_err(GeneralActivationErrorV3::Capability)?;
            let slot = authenticated
                .slot(entry_index)
                .map_err(GeneralActivationErrorV3::Capability)?;
            let is_selected = entry_index == selected_entry_index;
            if is_selected {
                if observed_selected
                    || (selected_active
                        && (!FUNDING_LEDGER_ACTIVE_ADMISSIBLE_STATES_V2.admits(slot.status())
                            || slot.activation_slot() == 0))
                    || (!selected_active
                        && (!FUNDING_LEDGER_PENDING_ADMISSIBLE_STATES_V2.admits(slot.status())
                            || slot.activation_slot() != 0))
                {
                    return Err(GeneralActivationErrorV3::Funding);
                }
                observed_selected = true;
            } else if !FUNDING_LEDGER_ACTIVE_ADMISSIBLE_STATES_V2.admits(slot.status())
                || slot.activation_slot() == 0
            {
                return Err(GeneralActivationErrorV3::Funding);
            }
            row_index = row_index
                .checked_add(1)
                .ok_or(GeneralActivationErrorV3::Arithmetic)?;
        }
    }
    if !observed_selected {
        return Err(GeneralActivationErrorV3::Funding);
    }
    Ok(())
}

fn require_general_entry(
    entry: CapabilityEntryV1,
    config_id: ContentId,
    config: GeneralConfigV3,
    generation: u64,
) -> Result<(), GeneralActivationErrorV3> {
    if config.generation() != generation
        || entry.release_id().to_bytes() != config.program_set_id()
        || entry.config_id() != config_id
        || entry.capacity_profile_id().to_bytes() != config.capacity_profile_id()
    {
        return Err(GeneralActivationErrorV3::Entry);
    }
    Ok(())
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

#[cfg(test)]
mod tests;
