//! Interpretation: turning a sealed record's certified bytes into one
//! observation, under the release-pinned decoding rules.
//!
//! Everything else in this crate carries observations without reading them.
//! This module is the one place that reads them, and it is deliberately the
//! *last* place in the family: the relayer signed account bytes, a quorum
//! certified the set, and only now — on the observing cluster, in code an
//! immutable adapter release pins — does anything decide what those bytes mean.
//!
//! That ordering is the whole point of the family. A relayer that could sign
//! "the pool graduated" would be trusted with the proposition; a relayer that
//! signs "account `X`, owned by `Y`, 424 bytes, here is its prefix" is trusted
//! only with the reading. Swapping the trust root swaps who does the reading and
//! moves nothing here.
//!
//! # What this module does not do
//!
//! It hashes nothing, exactly like the rest of the crate. The caller recomputes
//! the `account_set_id` over the preimage [`encode_account_set_id_preimage_v1`]
//! writes and passes the result in; this module compares it against the identity
//! the record and the configuration already committed to.
//!
//! [`encode_account_set_id_preimage_v1`]: crate::release::encode_account_set_id_preimage_v1

use dclutch_registry_contract::{ArtifactReleaseV1, DeploymentObservationV1};

use crate::{
    ADDRESS_BYTES, DBC_ADMITTED_DATA_LENGTHS_V1, DBC_CLOCK_POSITION_V1, DBC_DISCRIMINATOR_BYTES_V1,
    DBC_DISCRIMINATOR_OFFSET_V1, DBC_FINISH_CURVE_TIMESTAMP_OFFSET_V1, DBC_IS_MIGRATED_OFFSET_V1,
    DBC_MIGRATION_PROGRESS_OFFSET_V1, DBC_PROGRAM_POSITION_V1, DBC_PROGRAMDATA_POSITION_V1,
    DBC_TRANSFER_HOOK_POOL_DISCRIMINATOR_V1, DBC_VENUE_INLINE_BYTES_V1, DBC_VENUE_POSITION_V1,
    DBC_VENUE_SET_CARDINALITY_V1, DBC_VIRTUAL_POOL_DISCRIMINATOR_V1, Error,
    MIGRATION_PROGRESS_CREATED_POOL_V1, MIGRATION_PROGRESS_LOCKED_VESTING_V1,
    MIGRATION_PROGRESS_POST_BONDING_CURVE_V1, MIGRATION_PROGRESS_PRE_BONDING_CURVE_V1,
    OBSERVED_CLOCK_SLOT_OFFSET_V1, OBSERVED_CLOCK_SYSVAR_KEY_V1,
    OBSERVED_CLOCK_UNIX_TIMESTAMP_OFFSET_V1, OBSERVED_SYSVAR_OWNER_V1,
    RELAYED_OBSERVABLE_DBC_MIGRATION_PROGRESS_V1, RELAYED_OBSERVABLE_DBC_RAW_EXPONENT_V1, Result,
    identity::{LOADER_V3_PROGRAM_ID, reconstruct_deployment_observation_v1},
    record::RelayedObservationRecordViewV1,
    release::{AccountSetEntryV1, RelayedAdapterConfigV1},
    wire::AccountObservationV1,
};

/// Which observable of this adapter release's decoding-rules table one Source
/// produces.
///
/// The table is code, pinned by `ProviderReleaseV1.adapter_release_id`; the row
/// is data, pinned by `ProviderReleaseV1.decoding_rules_id` through
/// [`RelayedAdapterConfigV1::observable_selector`]. An unrecognized selector is
/// a refusal and never a fall-through to row zero.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelayedObservableV1 {
    /// Meteora DBC `VirtualPool.migration_progress`, as a graduation
    /// proposition over a terminal window.
    DbcMigrationProgressV1,
}

impl RelayedObservableV1 {
    /// Select one row of the table.
    pub fn from_selector(selector: u32) -> Result<Self> {
        if selector == RELAYED_OBSERVABLE_DBC_MIGRATION_PROGRESS_V1 {
            Ok(Self::DbcMigrationProgressV1)
        } else {
            Err(Error::UnknownObservable)
        }
    }

    /// The `observable_selector` naming this row.
    pub const fn selector(self) -> u32 {
        match self {
            Self::DbcMigrationProgressV1 => RELAYED_OBSERVABLE_DBC_MIGRATION_PROGRESS_V1,
        }
    }

    /// The declared base-ten scale of the atom this row produces.
    pub const fn raw_exponent(self) -> i32 {
        match self {
            Self::DbcMigrationProgressV1 => RELAYED_OBSERVABLE_DBC_RAW_EXPONENT_V1,
        }
    }

    /// The exact cardinality of the ordered account set this row reads.
    pub const fn set_cardinality(self) -> u16 {
        match self {
            Self::DbcMigrationProgressV1 => DBC_VENUE_SET_CARDINALITY_V1,
        }
    }
}

/// The explicit four-state graduation enum of a DBC `VirtualPool`.
///
/// The flow is **not monotone per step** — without locked vesting it jumps
/// `PreBondingCurve -> LockedVesting -> CreatedPool` — so nothing here compares
/// two states by order, and [`Self::is_terminal`] is equality with one state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MigrationProgressV1 {
    /// The bonding curve has not completed.
    PreBondingCurve,
    /// The curve completed; migration has not begun.
    PostBondingCurve,
    /// Migration is in flight through locked vesting.
    LockedVesting,
    /// The migrated pool exists. The only terminal state.
    CreatedPool,
}

impl MigrationProgressV1 {
    /// Hostile-decode the observed byte. A fifth value is not a state.
    pub fn from_byte(byte: u8) -> Result<Self> {
        match byte {
            MIGRATION_PROGRESS_PRE_BONDING_CURVE_V1 => Ok(Self::PreBondingCurve),
            MIGRATION_PROGRESS_POST_BONDING_CURVE_V1 => Ok(Self::PostBondingCurve),
            MIGRATION_PROGRESS_LOCKED_VESTING_V1 => Ok(Self::LockedVesting),
            MIGRATION_PROGRESS_CREATED_POOL_V1 => Ok(Self::CreatedPool),
            _ => Err(Error::UnenumeratedVenueState),
        }
    }

    /// The on-wire discriminant.
    pub const fn byte(self) -> u8 {
        match self {
            Self::PreBondingCurve => MIGRATION_PROGRESS_PRE_BONDING_CURVE_V1,
            Self::PostBondingCurve => MIGRATION_PROGRESS_POST_BONDING_CURVE_V1,
            Self::LockedVesting => MIGRATION_PROGRESS_LOCKED_VESTING_V1,
            Self::CreatedPool => MIGRATION_PROGRESS_CREATED_POOL_V1,
        }
    }

    /// Whether this state terminates a terminal-window graduation proposition.
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::CreatedPool)
    }
}

/// One interpreted observation, ready to be joined to a Source and a Product.
///
/// `atoms` carries the observable's own discriminant at the row's declared
/// exponent. This module does not decide which outcome an atom selects: the
/// Product's `ResultDomainV2` cuts do, which is what keeps one venue's rules
/// reusable across Products that carve the same observable differently.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelayedObservationOutcomeV1 {
    observable: RelayedObservableV1,
    atoms: i128,
    observed_unix_seconds: i64,
    observed_slot: u64,
    venue_deployment: DeploymentObservationV1,
}

impl RelayedObservationOutcomeV1 {
    /// The table row this observation was read under.
    pub const fn observable(self) -> RelayedObservableV1 {
        self.observable
    }
    /// The normalized atom, at `observable().raw_exponent()`.
    pub const fn atoms(self) -> i128 {
        self.atoms
    }
    /// The observed cluster's own `unix_timestamp` at the observed slot.
    pub const fn observed_unix_seconds(self) -> i64 {
        self.observed_unix_seconds
    }
    /// The finalized foreign slot every accepted body was read at.
    pub const fn observed_slot(self) -> u64 {
        self.observed_slot
    }
    /// The venue deployment reconstructed from the attested Loader V3 bodies.
    pub const fn venue_deployment(self) -> DeploymentObservationV1 {
        self.venue_deployment
    }
}

/// Re-derive and compare the founding-time pinned ordered account set.
///
/// `recomputed_account_set_id` is the caller's SHA-256 over the preimage this
/// crate writes. The entries themselves are caller-supplied and untrusted; they
/// become authoritative only because their digest equals an identity the record
/// and the configuration both committed to before this call.
fn require_pinned_set(
    record: RelayedObservationRecordViewV1<'_>,
    config: RelayedAdapterConfigV1,
    entries: &[AccountSetEntryV1],
    recomputed_account_set_id: [u8; 32],
    cardinality: u16,
) -> Result<()> {
    if recomputed_account_set_id != config.account_set_id()
        || recomputed_account_set_id != record.account_set_id()?
    {
        return Err(Error::AccountSetMismatch);
    }
    if entries.len() != usize::from(cardinality) || record.set_count()? != cardinality {
        return Err(Error::InvalidSetGeometry);
    }
    Ok(())
}

/// Read one position of the sealed record against its founding-time pin.
fn pinned_body<'a>(
    record: RelayedObservationRecordViewV1<'a>,
    entries: &[AccountSetEntryV1],
    position: u16,
) -> Result<AccountObservationV1<'a>> {
    let entry = entries
        .get(usize::from(position))
        .ok_or(Error::InvalidSetGeometry)?;
    let body = record.observation(position)?;
    body.require_pinned_position(entry.key, entry.expected_owner, entry.inline_len)?;
    Ok(body)
}

/// Authenticate the venue program, cross-cluster, against the pinned release.
///
/// This is P-B of the chain-state dossier §6.2, executed: the pinned
/// `elf_digest`, `deployment_slot` and upgrade authority are compared by exact
/// equality, so a venue redeploy mid-market makes every subsequent observation
/// refuse. It does not resolve the market to failure by itself — refusing is all
/// an observation route may do — and the market reaches the Product's named
/// failure outcome through the deadline-driven walk, which is the only path that
/// may select a failure selector at all.
fn require_pinned_venue(
    entries: &[AccountSetEntryV1],
    program: AccountObservationV1<'_>,
    programdata: AccountObservationV1<'_>,
    venue: AccountObservationV1<'_>,
    pinned_venue_release: ArtifactReleaseV1,
) -> Result<DeploymentObservationV1> {
    let program_entry = entries
        .get(usize::from(DBC_PROGRAM_POSITION_V1))
        .ok_or(Error::InvalidSetGeometry)?;
    let programdata_entry = entries
        .get(usize::from(DBC_PROGRAMDATA_POSITION_V1))
        .ok_or(Error::InvalidSetGeometry)?;
    let venue_entry = entries
        .get(usize::from(DBC_VENUE_POSITION_V1))
        .ok_or(Error::InvalidSetGeometry)?;
    if program_entry.expected_owner != LOADER_V3_PROGRAM_ID
        || programdata_entry.expected_owner != LOADER_V3_PROGRAM_ID
    {
        return Err(Error::InvalidLoaderVariant);
    }
    // The Loopscale rule, applied to the position that carries the state: the
    // observed venue account must be owned by the observed venue *program*, and
    // that program's key comes out of the pinned set rather than out of a
    // caller-supplied field.
    if venue_entry.expected_owner != program_entry.key || venue.owner() != program_entry.key {
        return Err(Error::ObservedOwnerMismatch);
    }
    if pinned_venue_release.program().to_bytes() != program_entry.key {
        return Err(Error::VenueDeploymentMismatch);
    }
    let observed = reconstruct_deployment_observation_v1(program, programdata)?;
    pinned_venue_release
        .authenticate_deployment(observed)
        .map_err(|_| Error::VenueDeploymentMismatch)?;
    Ok(observed)
}

/// Read the observed cluster's own clock out of the sealed set.
///
/// Foreign time cannot be an adapter assumption and cannot be checked when a
/// record is filled: filling only moves bytes the signer committed to, so the
/// attested `Clock` is decoded here, under the same rules as any other observed
/// account.
///
/// The slot equality is *chain-derived*: the `Clock` sysvar is written at the
/// start of each slot, so a finalized snapshot at slot `S` reports `slot == S`,
/// and the relayer reads both in one `getMultipleAccounts` batch. Requiring it
/// binds the slot the record is *addressed* by to the time its freshness bound
/// is *measured* against, which nothing else in the family does.
fn require_observed_clock(
    record: RelayedObservationRecordViewV1<'_>,
    entries: &[AccountSetEntryV1],
) -> Result<i64> {
    let entry = entries
        .get(usize::from(DBC_CLOCK_POSITION_V1))
        .ok_or(Error::InvalidSetGeometry)?;
    if entry.key != OBSERVED_CLOCK_SYSVAR_KEY_V1 || entry.expected_owner != OBSERVED_SYSVAR_OWNER_V1
    {
        return Err(Error::ObservedClockMismatch);
    }
    let body = pinned_body(record, entries, DBC_CLOCK_POSITION_V1)?;
    if !body.is_fully_inline() || body.executable() {
        return Err(Error::ObservedClockMismatch);
    }
    let inline = body.inline();
    if crate::u64_at(inline, OBSERVED_CLOCK_SLOT_OFFSET_V1)? != record.observed_slot()? {
        return Err(Error::ObservedSlotMismatch);
    }
    let observed_unix_seconds = crate::i64_at(inline, OBSERVED_CLOCK_UNIX_TIMESTAMP_OFFSET_V1)?;
    if observed_unix_seconds <= 0 {
        return Err(Error::ObservedClockMismatch);
    }
    Ok(observed_unix_seconds)
}

/// Apply the DBC `VirtualPool` grammar to one attested venue body.
fn read_dbc_graduation(venue: AccountObservationV1<'_>) -> Result<i128> {
    if !DBC_ADMITTED_DATA_LENGTHS_V1
        .iter()
        .any(|admitted| *admitted == venue.data_len())
    {
        return Err(Error::VenueLengthNotAdmitted);
    }
    if venue.inline().len() != DBC_VENUE_INLINE_BYTES_V1 || venue.executable() {
        return Err(Error::InvalidInlineWidth);
    }
    let inline = venue.inline();
    let discriminator: [u8; 8] = crate::array(inline, DBC_DISCRIMINATOR_OFFSET_V1)?;
    let _ = DBC_DISCRIMINATOR_BYTES_V1;
    if discriminator != DBC_VIRTUAL_POOL_DISCRIMINATOR_V1 {
        // `TransferHookPool` shares the identical 424-byte body after 0.2.0, so
        // the discriminator is what stops one decoding under rules minted for
        // the other. It is named in the table precisely so this refusal is a
        // decision rather than an accident.
        let _ = DBC_TRANSFER_HOOK_POOL_DISCRIMINATOR_V1;
        return Err(Error::VenueDiscriminatorMismatch);
    }
    let progress =
        MigrationProgressV1::from_byte(crate::one(inline, DBC_MIGRATION_PROGRESS_OFFSET_V1)?)?;
    let is_migrated = crate::one(inline, DBC_IS_MIGRATED_OFFSET_V1)?;
    let finish_curve_timestamp = crate::u64_at(inline, DBC_FINISH_CURVE_TIMESTAMP_OFFSET_V1)?;
    if is_migrated > 1 || (is_migrated == 1) != progress.is_terminal() {
        return Err(Error::IncoherentVenueBody);
    }
    if progress.is_terminal() && finish_curve_timestamp == 0 {
        return Err(Error::IncoherentVenueBody);
    }
    if !progress.is_terminal() {
        // Not a negative answer: no answer. A terminal-window graduation
        // proposition is only ever *proved* by graduation, and "it did not
        // graduate" is proved by the deadline passing.
        return Err(Error::WindowNotSatisfied);
    }
    Ok(i128::from(progress.byte()))
}

/// Interpret one sealed record into exactly one observation.
///
/// Every input except `record` is a founding-time pin the caller authenticated
/// separately; `record` is the object a relayer quorum certified. The order is
/// deliberate and each step refuses on its own field:
///
/// 1. the observable row exists,
/// 2. the ordered account set is the pinned one, by digest,
/// 3. every position matches its pin — key, owning program, inline width,
/// 4. the venue deployment is the pinned one, cross-cluster,
/// 5. the observed clock is the observed cluster's own, at the record's slot,
/// 6. the observation is inside the two-clock staleness bound,
/// 7. the venue body parses, is coherent, and is terminal.
///
/// The record's own consumability — sealed, unconsumed, bound to this Market,
/// generation, material, provider release and key set — is
/// [`RelayedObservationRecordViewV1::require_consumable`]'s job and the caller
/// runs it first.
#[allow(clippy::too_many_arguments)]
pub fn interpret_sealed_record_v1(
    record: RelayedObservationRecordViewV1<'_>,
    config: RelayedAdapterConfigV1,
    entries: &[AccountSetEntryV1],
    recomputed_account_set_id: [u8; 32],
    pinned_venue_release: ArtifactReleaseV1,
    pinned_cluster_id: [u8; ADDRESS_BYTES],
    current_unix_seconds: i64,
) -> Result<RelayedObservationOutcomeV1> {
    let observable = RelayedObservableV1::from_selector(config.observable_selector())?;
    if config.raw_exponent() != observable.raw_exponent() {
        // The declared scale is a founding-time echo of the row's own scale; a
        // configuration that disagrees with the table it selects is refused
        // rather than silently scaled.
        return Err(Error::UnknownObservable);
    }
    crate::identity::require_observed_cluster(record.observed_cluster_id()?, pinned_cluster_id)?;
    require_pinned_set(
        record,
        config,
        entries,
        recomputed_account_set_id,
        observable.set_cardinality(),
    )?;

    let program = pinned_body(record, entries, DBC_PROGRAM_POSITION_V1)?;
    let programdata = pinned_body(record, entries, DBC_PROGRAMDATA_POSITION_V1)?;
    let venue = pinned_body(record, entries, DBC_VENUE_POSITION_V1)?;
    let venue_deployment =
        require_pinned_venue(entries, program, programdata, venue, pinned_venue_release)?;

    let observed_unix_seconds = require_observed_clock(record, entries)?;
    config.require_observation_freshness(current_unix_seconds, observed_unix_seconds)?;

    let atoms = match observable {
        RelayedObservableV1::DbcMigrationProgressV1 => read_dbc_graduation(venue)?,
    };
    Ok(RelayedObservationOutcomeV1 {
        observable,
        atoms,
        observed_unix_seconds,
        observed_slot: record.observed_slot()?,
        venue_deployment,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DBC_GRADUATION_ACCEPTANCE_TABLE_V1, DBC_GRADUATION_ACCEPTANCE_TABLE_V1_COUNT};

    fn venue_body(progress: u8, is_migrated: u8, finish: u64) -> [u8; DBC_VENUE_INLINE_BYTES_V1] {
        let mut data = [0u8; DBC_VENUE_INLINE_BYTES_V1];
        data[..8].copy_from_slice(&DBC_VIRTUAL_POOL_DISCRIMINATOR_V1);
        data[DBC_MIGRATION_PROGRESS_OFFSET_V1] = progress;
        data[DBC_IS_MIGRATED_OFFSET_V1] = is_migrated;
        data[DBC_FINISH_CURVE_TIMESTAMP_OFFSET_V1..DBC_FINISH_CURVE_TIMESTAMP_OFFSET_V1 + 8]
            .copy_from_slice(&finish.to_le_bytes());
        data
    }

    fn observation(body: &[u8]) -> AccountObservationV1<'_> {
        AccountObservationV1::new(
            [0x5a; 32],
            [0x09; 32],
            1_000_000,
            u32::try_from(DBC_VENUE_INLINE_BYTES_V1).expect("fits"),
            body,
            false,
            crate::SHA256_EMPTY_DIGEST,
        )
        .expect("canonical venue body")
    }

    #[test]
    fn the_rust_grammar_agrees_with_the_lean_acceptance_table() {
        assert_eq!(
            DBC_GRADUATION_ACCEPTANCE_TABLE_V1.len(),
            DBC_GRADUATION_ACCEPTANCE_TABLE_V1_COUNT,
            "the Rust side is iterating fewer rows than Lean emitted"
        );
        for (index, (progress, is_migrated, finish, accepted, atoms)) in
            DBC_GRADUATION_ACCEPTANCE_TABLE_V1.iter().enumerate()
        {
            let body = venue_body(*progress, *is_migrated, *finish);
            let produced = read_dbc_graduation(observation(&body));
            assert_eq!(
                produced.is_ok(),
                *accepted,
                "row {index} disagrees with Lean about whether it is a graduation"
            );
            assert_eq!(
                produced.unwrap_or(*atoms),
                *atoms,
                "row {index} produced a different atom than Lean"
            );
        }
    }

    #[test]
    fn a_pre_terminal_state_refuses_as_an_unsatisfied_window_not_as_a_decode_failure() {
        for progress in [
            MIGRATION_PROGRESS_PRE_BONDING_CURVE_V1,
            MIGRATION_PROGRESS_POST_BONDING_CURVE_V1,
            MIGRATION_PROGRESS_LOCKED_VESTING_V1,
        ] {
            let body = venue_body(progress, 0, 1_756_000_500);
            assert_eq!(
                read_dbc_graduation(observation(&body)),
                Err(Error::WindowNotSatisfied),
                "a pre-terminal state must name the window, not the bytes"
            );
        }
    }

    #[test]
    fn a_transfer_hook_pool_refuses_on_its_discriminator() {
        let mut body = venue_body(MIGRATION_PROGRESS_CREATED_POOL_V1, 1, 1_756_000_500);
        body[..8].copy_from_slice(&DBC_TRANSFER_HOOK_POOL_DISCRIMINATOR_V1);
        assert_eq!(
            read_dbc_graduation(observation(&body)),
            Err(Error::VenueDiscriminatorMismatch),
            "an identically shaped account of another type decoded anyway"
        );
    }

    #[test]
    fn a_data_length_outside_the_singleton_admitted_set_refuses() {
        let body = venue_body(MIGRATION_PROGRESS_CREATED_POOL_V1, 1, 1_756_000_500);
        let grown = AccountObservationV1::new(
            [0x5a; 32], [0x09; 32], 1_000_000, 425, &body, false, [0x11; 32],
        )
        .expect("body");
        assert_eq!(
            read_dbc_graduation(grown),
            Err(Error::VenueLengthNotAdmitted)
        );
    }

    #[test]
    fn an_unenumerated_progress_byte_refuses_on_its_own_field() {
        for progress in [4u8, 5, 200, 255] {
            let body = venue_body(progress, 0, 1_756_000_500);
            assert_eq!(
                read_dbc_graduation(observation(&body)),
                Err(Error::UnenumeratedVenueState)
            );
        }
    }

    #[test]
    fn an_unknown_observable_selector_never_falls_through_to_row_zero() {
        assert_eq!(
            RelayedObservableV1::from_selector(0),
            Ok(RelayedObservableV1::DbcMigrationProgressV1)
        );
        for selector in [1u32, 2, 0xffff_ffff] {
            assert_eq!(
                RelayedObservableV1::from_selector(selector),
                Err(Error::UnknownObservable)
            );
        }
    }

    #[test]
    fn terminality_is_equality_with_one_state_and_never_an_ordering() {
        assert!(MigrationProgressV1::CreatedPool.is_terminal());
        for state in [
            MigrationProgressV1::PreBondingCurve,
            MigrationProgressV1::PostBondingCurve,
            MigrationProgressV1::LockedVesting,
        ] {
            assert!(
                !state.is_terminal(),
                "the flow jumps 0 -> 2 -> 3; nothing may treat progress as a counter"
            );
        }
    }
}
