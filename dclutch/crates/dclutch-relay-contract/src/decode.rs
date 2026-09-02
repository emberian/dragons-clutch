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
    ADDRESS_BYTES, COPTION_NONE_TAG_V1, COPTION_SOME_TAG_V1, DBC_ADMITTED_DATA_LENGTHS_V1,
    DBC_CLOCK_POSITION_V1, DBC_DISCRIMINATOR_BYTES_V1, DBC_DISCRIMINATOR_OFFSET_V1,
    DBC_FINISH_CURVE_TIMESTAMP_OFFSET_V1, DBC_IS_MIGRATED_OFFSET_V1,
    DBC_MIGRATION_PROGRESS_OFFSET_V1, DBC_PROGRAM_POSITION_V1, DBC_PROGRAMDATA_POSITION_V1,
    DBC_TRANSFER_HOOK_POOL_DISCRIMINATOR_V1, DBC_VENUE_INLINE_BYTES_V1, DBC_VENUE_POSITION_V1,
    DBC_VENUE_SET_CARDINALITY_V1, DBC_VIRTUAL_POOL_DISCRIMINATOR_V1, Error,
    MIGRATION_PROGRESS_CREATED_POOL_V1, MIGRATION_PROGRESS_LOCKED_VESTING_V1,
    MIGRATION_PROGRESS_POST_BONDING_CURVE_V1, MIGRATION_PROGRESS_PRE_BONDING_CURVE_V1,
    MINT_ADMITTED_DATA_LENGTHS_V1, MINT_AUTHORITY_CLOCK_POSITION_V1, MINT_AUTHORITY_HELD_V1,
    MINT_AUTHORITY_MINT_POSITION_V1, MINT_AUTHORITY_PROGRAM_POSITION_V1,
    MINT_AUTHORITY_PROGRAMDATA_POSITION_V1, MINT_AUTHORITY_RENOUNCED_V1,
    MINT_AUTHORITY_SET_CARDINALITY_V1, MINT_AUTHORITY_TAG_OFFSET_V1,
    MINT_FREEZE_AUTHORITY_TAG_OFFSET_V1, MINT_INLINE_BYTES_V1, MINT_IS_INITIALIZED_OFFSET_V1,
    OBSERVED_CLOCK_SLOT_OFFSET_V1, OBSERVED_CLOCK_SYSVAR_KEY_V1,
    OBSERVED_CLOCK_UNIX_TIMESTAMP_OFFSET_V1, OBSERVED_SYSVAR_OWNER_V1,
    RELAYED_OBSERVABLE_DBC_MIGRATION_PROGRESS_V1, RELAYED_OBSERVABLE_DBC_RAW_EXPONENT_V1,
    RELAYED_OBSERVABLE_MINT_AUTHORITY_RAW_EXPONENT_V1,
    RELAYED_OBSERVABLE_MINT_AUTHORITY_RENOUNCED_V1, Result,
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
    /// SPL Token-2022 `Mint.mint_authority`, as a renunciation proposition
    /// over a terminal window.
    ///
    /// "Has this token's supply become permanently fixed?" The observed
    /// program enforces the terminality itself — `processor.rs:722` refuses
    /// `SetAuthority(MintTokens)` with `FixedSupply` once the authority is
    /// `None` — so an observation of the renounced state is a proof about
    /// every later slot and not only about the observed one.
    Token2022MintAuthorityRenouncedV1,
}

impl RelayedObservableV1 {
    /// Select one row of the table.
    pub fn from_selector(selector: u32) -> Result<Self> {
        if selector == RELAYED_OBSERVABLE_DBC_MIGRATION_PROGRESS_V1 {
            Ok(Self::DbcMigrationProgressV1)
        } else if selector == RELAYED_OBSERVABLE_MINT_AUTHORITY_RENOUNCED_V1 {
            Ok(Self::Token2022MintAuthorityRenouncedV1)
        } else {
            Err(Error::UnknownObservable)
        }
    }

    /// The `observable_selector` naming this row.
    pub const fn selector(self) -> u32 {
        match self {
            Self::DbcMigrationProgressV1 => RELAYED_OBSERVABLE_DBC_MIGRATION_PROGRESS_V1,
            Self::Token2022MintAuthorityRenouncedV1 => {
                RELAYED_OBSERVABLE_MINT_AUTHORITY_RENOUNCED_V1
            }
        }
    }

    /// The declared base-ten scale of the atom this row produces.
    pub const fn raw_exponent(self) -> i32 {
        match self {
            Self::DbcMigrationProgressV1 => RELAYED_OBSERVABLE_DBC_RAW_EXPONENT_V1,
            Self::Token2022MintAuthorityRenouncedV1 => {
                RELAYED_OBSERVABLE_MINT_AUTHORITY_RAW_EXPONENT_V1
            }
        }
    }

    /// The exact cardinality of the ordered account set this row reads.
    pub const fn set_cardinality(self) -> u16 {
        match self {
            Self::DbcMigrationProgressV1 => DBC_VENUE_SET_CARDINALITY_V1,
            Self::Token2022MintAuthorityRenouncedV1 => MINT_AUTHORITY_SET_CARDINALITY_V1,
        }
    }

    /// The exact inline width this row's state position is pinned at.
    ///
    /// Both rows carry their state account WHOLE, which the decoding rules
    /// prove: `venueInlineBytes == admittedDataLengths` for row 0 and
    /// `mintInlineBytes == mintAdmittedDataLengths` for row 1. A founder
    /// building the pinned account set reads this rather than typing the
    /// number a second time.
    pub const fn state_inline_bytes(self) -> u16 {
        match self {
            Self::DbcMigrationProgressV1 => DBC_VENUE_INLINE_BYTES_V1 as u16,
            Self::Token2022MintAuthorityRenouncedV1 => MINT_INLINE_BYTES_V1 as u16,
        }
    }

    /// Where each structural role sits in this row's ordered account set.
    pub const fn set_layout(self) -> RelayedSetLayoutV1 {
        match self {
            Self::DbcMigrationProgressV1 => RelayedSetLayoutV1 {
                program: DBC_PROGRAM_POSITION_V1,
                programdata: DBC_PROGRAMDATA_POSITION_V1,
                state: DBC_VENUE_POSITION_V1,
                clock: DBC_CLOCK_POSITION_V1,
            },
            Self::Token2022MintAuthorityRenouncedV1 => RelayedSetLayoutV1 {
                program: MINT_AUTHORITY_PROGRAM_POSITION_V1,
                programdata: MINT_AUTHORITY_PROGRAMDATA_POSITION_V1,
                state: MINT_AUTHORITY_MINT_POSITION_V1,
                clock: MINT_AUTHORITY_CLOCK_POSITION_V1,
            },
        }
    }
}

/// Every row of this release's decoding-rules table.
///
/// Exhaustive by construction: [`table_rows_are_exhaustive_and_well_formed`]
/// matches on each variant, so a row added to [`RelayedObservableV1`] without
/// being added here does not compile.
///
/// [`table_rows_are_exhaustive_and_well_formed`]: self
pub const RELAYED_OBSERVABLE_TABLE_V1: &[RelayedObservableV1] = &[
    RelayedObservableV1::DbcMigrationProgressV1,
    RelayedObservableV1::Token2022MintAuthorityRenouncedV1,
];

/// Which position of one observable's ordered account set carries which role.
///
/// The spine of an interpretation — authenticate the observed program
/// cross-cluster, read the observed cluster's own clock, check staleness — is
/// identical for every row of the table. The only thing that varies is *where
/// in the set* each of those accounts sits, and how many accounts there are.
/// Holding that as data on the row, beside [`RelayedObservableV1::set_cardinality`],
/// is what lets a second observable be authored without editing
/// [`interpret_sealed_record_v1`]. Before this existed the DBC positions were
/// read inside the orchestration, and an observable with a different
/// cardinality would have had to fork it.
///
/// Every role is required. An observable that wanted no venue program would
/// not be a relayed observable: `require_pinned_venue` is the family's
/// cross-cluster deployment authentication, and dropping it would drop the
/// property that makes the family worth having.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelayedSetLayoutV1 {
    /// Loader V3 `Program` account of the observed venue.
    pub program: u16,
    /// Loader V3 `ProgramData` account holding the observed venue's ELF.
    pub programdata: u16,
    /// The account whose attested bytes carry this observable's own state.
    pub state: u16,
    /// The observed cluster's `Clock` sysvar.
    pub clock: u16,
}

impl RelayedSetLayoutV1 {
    /// The four roles in set order, for range and distinctness checks.
    const fn positions(self) -> [u16; 4] {
        [self.program, self.programdata, self.state, self.clock]
    }

    /// Refuse a table row whose roles collide or fall outside its own set.
    ///
    /// A malformed row is a defect in the emitted decoding-rules table rather
    /// than in any caller's record, so this can never fire for a shipped row —
    /// [`RELAYED_OBSERVABLE_TABLE_V1`] is walked by a test that asserts it. It
    /// is here so that the author of observable #2 gets a named refusal at the
    /// interpretation boundary instead of two roles silently reading one body.
    fn require_well_formed(self, cardinality: u16) -> Result<()> {
        let positions = self.positions();
        let mut outer = 0;
        while outer < positions.len() {
            if positions[outer] >= cardinality {
                return Err(Error::InvalidSetGeometry);
            }
            let mut inner = outer + 1;
            while inner < positions.len() {
                if positions[outer] == positions[inner] {
                    return Err(Error::InvalidSetGeometry);
                }
                inner += 1;
            }
            outer += 1;
        }
        Ok(())
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
    layout: RelayedSetLayoutV1,
    program: AccountObservationV1<'_>,
    programdata: AccountObservationV1<'_>,
    venue: AccountObservationV1<'_>,
    pinned_venue_release: ArtifactReleaseV1,
) -> Result<DeploymentObservationV1> {
    let program_entry = entries
        .get(usize::from(layout.program))
        .ok_or(Error::InvalidSetGeometry)?;
    let programdata_entry = entries
        .get(usize::from(layout.programdata))
        .ok_or(Error::InvalidSetGeometry)?;
    let venue_entry = entries
        .get(usize::from(layout.state))
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
    position: u16,
) -> Result<i64> {
    let entry = entries
        .get(usize::from(position))
        .ok_or(Error::InvalidSetGeometry)?;
    if entry.key != OBSERVED_CLOCK_SYSVAR_KEY_V1 || entry.expected_owner != OBSERVED_SYSVAR_OWNER_V1
    {
        return Err(Error::ObservedClockMismatch);
    }
    let body = pinned_body(record, entries, position)?;
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

/// Whether an SPL Token-2022 mint can still mint.
///
/// Two states, and the observed program itself makes the transition between
/// them one-way: `spl-token-2022 4.0.0 processor.rs:722` reads
/// `mint.base.mint_authority.ok_or(TokenError::FixedSupply)?` before it will
/// write a new one, so `Renounced` is terminal on the observed cluster and not
/// merely terminal in this table's opinion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MintAuthorityStateV1 {
    /// Some authority may still mint. Not a negative answer; not an answer.
    Held,
    /// The authority is `COption::None`. The only terminal state.
    Renounced,
}

impl MintAuthorityStateV1 {
    /// Hostile-decode the observed four-byte `COption` tag.
    ///
    /// `unpack_coption_key` admits exactly `[0,0,0,0]` and `[1,0,0,0]`; every
    /// other word is not a tag this program ever wrote, and is a refusal
    /// rather than a third state.
    pub fn from_tag(tag: u32) -> Result<Self> {
        if tag == COPTION_NONE_TAG_V1 {
            Ok(Self::Renounced)
        } else if tag == COPTION_SOME_TAG_V1 {
            Ok(Self::Held)
        } else {
            Err(Error::UnenumeratedVenueState)
        }
    }

    /// The on-wire discriminant this row reports as its atom.
    pub const fn byte(self) -> u8 {
        match self {
            Self::Held => MINT_AUTHORITY_HELD_V1,
            Self::Renounced => MINT_AUTHORITY_RENOUNCED_V1,
        }
    }

    /// Whether this state terminates the renunciation proposition.
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Renounced)
    }
}

/// Apply the SPL Token-2022 `Mint` grammar to one attested account body.
///
/// SPL Token carries NO discriminator. What plays its role here is the
/// admitted length together with the two `COption` tags: `Account` is 165
/// bytes and `Multisig` is 355, so no other account this program owns can
/// present as 82, and an 82-byte body whose tags are not one of the two
/// admitted words is not a `Mint` this program wrote.
fn read_mint_authority_renounced(mint: AccountObservationV1<'_>) -> Result<i128> {
    if !MINT_ADMITTED_DATA_LENGTHS_V1
        .iter()
        .any(|admitted| *admitted == mint.data_len())
    {
        return Err(Error::VenueLengthNotAdmitted);
    }
    if mint.inline().len() != MINT_INLINE_BYTES_V1 || mint.executable() {
        return Err(Error::InvalidInlineWidth);
    }
    let inline = mint.inline();
    let state =
        MintAuthorityStateV1::from_tag(crate::u32_at(inline, MINT_AUTHORITY_TAG_OFFSET_V1)?)?;
    let is_initialized = crate::one(inline, MINT_IS_INITIALIZED_OFFSET_V1)?;
    let freeze_authority_tag = crate::u32_at(inline, MINT_FREEZE_AUTHORITY_TAG_OFFSET_V1)?;
    if is_initialized != 1 {
        // AN UNINITIALIZED MINT IS ALL ZEROES, and all zeroes reads as
        // `COption::None`. Without this line a freshly allocated 82-byte
        // account would prove the proposition the instant it existed. This is
        // the row's sharpest refusal and `Pack::unpack_from_slice` refuses the
        // same byte for the same reason.
        return Err(Error::IncoherentVenueBody);
    }
    if freeze_authority_tag != COPTION_NONE_TAG_V1 && freeze_authority_tag != COPTION_SOME_TAG_V1 {
        // A body that is not a well-formed `Mint` in its second `COption` is
        // not one in its first either. With no discriminator to lean on, this
        // is what stops a foreign 82-byte body decoding under these rules.
        return Err(Error::IncoherentVenueBody);
    }
    if !state.is_terminal() {
        // Not a negative answer: no answer. A terminal-window renunciation is
        // only ever *proved* by renunciation, and "it did not renounce" is
        // proved by the deadline passing.
        return Err(Error::WindowNotSatisfied);
    }
    Ok(i128::from(state.byte()))
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

    let layout = observable.set_layout();
    layout.require_well_formed(observable.set_cardinality())?;

    let program = pinned_body(record, entries, layout.program)?;
    let programdata = pinned_body(record, entries, layout.programdata)?;
    let state = pinned_body(record, entries, layout.state)?;
    let venue_deployment = require_pinned_venue(
        entries,
        layout,
        program,
        programdata,
        state,
        pinned_venue_release,
    )?;

    let observed_unix_seconds = require_observed_clock(record, entries, layout.clock)?;
    config.require_observation_freshness(current_unix_seconds, observed_unix_seconds)?;

    // The ONE observable-specific line of the whole interpretation. Everything
    // above is the family's spine, driven by the row's own layout; everything
    // below is shape-free. A second observable adds one arm here and one row to
    // `set_layout`, and edits nothing else in this function.
    let atoms = match observable {
        RelayedObservableV1::DbcMigrationProgressV1 => read_dbc_graduation(state)?,
        RelayedObservableV1::Token2022MintAuthorityRenouncedV1 => {
            read_mint_authority_renounced(state)?
        }
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
    #![allow(clippy::indexing_slicing)]

    use super::*;
    use crate::{
        DBC_GRADUATION_ACCEPTANCE_TABLE_V1, DBC_GRADUATION_ACCEPTANCE_TABLE_V1_COUNT,
        MINT_AUTHORITY_ACCEPTANCE_TABLE_V1, MINT_AUTHORITY_ACCEPTANCE_TABLE_V1_COUNT,
    };

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
        assert_eq!(
            RelayedObservableV1::from_selector(1),
            Ok(RelayedObservableV1::Token2022MintAuthorityRenouncedV1)
        );
        // Driven off the table rather than off a literal, so adding a row
        // moves this boundary instead of turning the test into a lie.
        for selector in [2u32, 3, 0xffff_ffff] {
            assert!(
                !RELAYED_OBSERVABLE_TABLE_V1
                    .iter()
                    .any(|row| row.selector() == selector)
            );
            assert_eq!(
                RelayedObservableV1::from_selector(selector),
                Err(Error::UnknownObservable)
            );
        }
    }

    fn mint_body(
        authority_tag: u32,
        is_initialized: u8,
        freeze_tag: u32,
    ) -> [u8; MINT_INLINE_BYTES_V1] {
        let mut data = [0u8; MINT_INLINE_BYTES_V1];
        data[MINT_AUTHORITY_TAG_OFFSET_V1..MINT_AUTHORITY_TAG_OFFSET_V1 + 4]
            .copy_from_slice(&authority_tag.to_le_bytes());
        data[MINT_IS_INITIALIZED_OFFSET_V1] = is_initialized;
        data[MINT_FREEZE_AUTHORITY_TAG_OFFSET_V1..MINT_FREEZE_AUTHORITY_TAG_OFFSET_V1 + 4]
            .copy_from_slice(&freeze_tag.to_le_bytes());
        data
    }

    fn mint_observation(body: &[u8]) -> AccountObservationV1<'_> {
        AccountObservationV1::new(
            [0x77; 32],
            [0x2a; 32],
            1_461_600,
            u32::try_from(MINT_INLINE_BYTES_V1).expect("fits"),
            body,
            false,
            crate::SHA256_EMPTY_DIGEST,
        )
        .expect("canonical mint body")
    }

    #[test]
    fn the_rust_mint_grammar_agrees_with_the_lean_acceptance_table() {
        assert_eq!(
            MINT_AUTHORITY_ACCEPTANCE_TABLE_V1.len(),
            MINT_AUTHORITY_ACCEPTANCE_TABLE_V1_COUNT,
            "the Rust side is iterating fewer rows than Lean emitted"
        );
        for (index, (authority_tag, is_initialized, freeze_tag, accepted, atoms)) in
            MINT_AUTHORITY_ACCEPTANCE_TABLE_V1.iter().enumerate()
        {
            let body = mint_body(*authority_tag, *is_initialized, *freeze_tag);
            let produced = read_mint_authority_renounced(mint_observation(&body));
            assert_eq!(
                produced.is_ok(),
                *accepted,
                "row {index} disagrees with Lean about whether it is a renunciation"
            );
            assert_eq!(
                produced.unwrap_or(*atoms),
                *atoms,
                "row {index} produced a different atom than Lean"
            );
        }
    }

    /// The row's sharpest refusal, stated on its own so it cannot be lost in a
    /// table walk: a zeroed 82-byte account reads as `COption::None` in both
    /// tags, and only `is_initialized` stands between it and a proof.
    #[test]
    fn a_zeroed_account_does_not_prove_a_renunciation() {
        let zeroed = [0u8; MINT_INLINE_BYTES_V1];
        assert_eq!(
            read_mint_authority_renounced(mint_observation(&zeroed)),
            Err(Error::IncoherentVenueBody)
        );
        // POSITIVE CONTROL in the same run: the same tags with the one byte
        // set is the graduation of this proposition.
        let initialized = mint_body(COPTION_NONE_TAG_V1, 1, COPTION_NONE_TAG_V1);
        assert_eq!(
            read_mint_authority_renounced(mint_observation(&initialized)),
            Ok(i128::from(MINT_AUTHORITY_RENOUNCED_V1))
        );
    }

    /// A held authority is NO ANSWER, exactly as a pre-terminal pool is, and
    /// the two rows must not be distinguishable on that point.
    #[test]
    fn a_held_authority_refuses_as_an_unsatisfied_window_not_as_a_decode_failure() {
        for freeze_tag in [COPTION_NONE_TAG_V1, COPTION_SOME_TAG_V1] {
            let body = mint_body(COPTION_SOME_TAG_V1, 1, freeze_tag);
            assert_eq!(
                read_mint_authority_renounced(mint_observation(&body)),
                Err(Error::WindowNotSatisfied),
                "a held authority is not a low value and not a decode failure"
            );
        }
    }

    /// A length no `Mint` ever has refuses on the length, and the two other
    /// accounts this program owns are exactly the lengths it must refuse.
    #[test]
    fn a_token_account_or_multisig_length_is_not_admitted() {
        for foreign_len in [165_u32, 355, 81, 83, 0] {
            assert!(
                !MINT_ADMITTED_DATA_LENGTHS_V1
                    .iter()
                    .any(|admitted| *admitted == foreign_len),
                "{foreign_len} must not be an admitted Mint length"
            );
        }
        let body = mint_body(COPTION_NONE_TAG_V1, 1, COPTION_SOME_TAG_V1);
        let wrong_length = AccountObservationV1::new(
            [0x77; 32], [0x2a; 32], 1_461_600, 165, &body, false, [0x11; 32],
        )
        .expect("a body whose account is longer than its inline prefix");
        assert_eq!(
            read_mint_authority_renounced(wrong_length),
            Err(Error::VenueLengthNotAdmitted)
        );
    }

    /// The two rows must not be able to resolve each other's markets.
    #[test]
    fn the_two_rows_are_distinguishable_in_every_way_that_matters() {
        let dbc = RelayedObservableV1::DbcMigrationProgressV1;
        let mint = RelayedObservableV1::Token2022MintAuthorityRenouncedV1;
        assert_ne!(dbc.selector(), mint.selector());
        // A graduated pool body under the mint grammar, and a renounced mint
        // body under the graduation grammar: both refuse.
        let pool = venue_body(MIGRATION_PROGRESS_CREATED_POOL_V1, 1, 1_756_000_500);
        assert!(read_mint_authority_renounced(observation(&pool)).is_err());
        let renounced = mint_body(COPTION_NONE_TAG_V1, 1, COPTION_SOME_TAG_V1);
        assert!(read_dbc_graduation(mint_observation(&renounced)).is_err());
        // And their terminal atoms are different numbers, so a Product carving
        // one observable cannot be silently resolved by the other.
        assert_ne!(
            read_dbc_graduation(observation(&pool)).expect("graduated"),
            read_mint_authority_renounced(mint_observation(&renounced)).expect("renounced")
        );
    }

    #[test]
    fn every_table_row_states_a_well_formed_set_layout() {
        // The table is exhaustive by construction: this match has no wildcard,
        // so a row added to `RelayedObservableV1` fails to compile until it is
        // added to `RELAYED_OBSERVABLE_TABLE_V1` too.
        for observable in RELAYED_OBSERVABLE_TABLE_V1.iter().copied() {
            match observable {
                RelayedObservableV1::DbcMigrationProgressV1 => {}
                RelayedObservableV1::Token2022MintAuthorityRenouncedV1 => {}
            }
            let layout = observable.set_layout();
            layout
                .require_well_formed(observable.set_cardinality())
                .expect("a shipped row's roles must be distinct and inside its own set");
            assert_eq!(
                RelayedObservableV1::from_selector(observable.selector()),
                Ok(observable),
                "a row's selector must round-trip to the row"
            );
        }
        assert_eq!(RELAYED_OBSERVABLE_TABLE_V1.len(), 2);
        // The state width every row pins is the one its own grammar admits.
        assert_eq!(
            u32::from(RelayedObservableV1::DbcMigrationProgressV1.state_inline_bytes()),
            DBC_ADMITTED_DATA_LENGTHS_V1[0]
        );
        assert_eq!(
            u32::from(RelayedObservableV1::Token2022MintAuthorityRenouncedV1.state_inline_bytes()),
            MINT_ADMITTED_DATA_LENGTHS_V1[0]
        );
    }

    #[test]
    fn a_malformed_layout_refuses_by_name_rather_than_reading_one_body_twice() {
        let sound = RelayedObservableV1::DbcMigrationProgressV1.set_layout();
        assert_eq!(
            sound.require_well_formed(DBC_VENUE_SET_CARDINALITY_V1),
            Ok(())
        );
        // Two roles on one position: the defect the check exists to catch.
        let collided = RelayedSetLayoutV1 {
            state: sound.clock,
            ..sound
        };
        assert_eq!(
            collided.require_well_formed(DBC_VENUE_SET_CARDINALITY_V1),
            Err(Error::InvalidSetGeometry)
        );
        // A role outside the row's own declared cardinality.
        assert_eq!(
            sound.require_well_formed(DBC_VENUE_SET_CARDINALITY_V1 - 1),
            Err(Error::InvalidSetGeometry)
        );
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
