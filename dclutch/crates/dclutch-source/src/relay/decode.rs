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
//! [`encode_account_set_id_preimage_v1`]: crate::relay::release::encode_account_set_id_preimage_v1

use dclutch_registry::{ArtifactReleaseV1, DeploymentObservationV1};

use crate::relay::{
    ADDRESS_BYTES, COPTION_NONE_TAG_V1, COPTION_SOME_TAG_V1, DBC_ADMITTED_DATA_LENGTHS_V1,
    DBC_CLOCK_POSITION_V1, DBC_DISCRIMINATOR_BYTES_V1, DBC_DISCRIMINATOR_OFFSET_V1,
    DBC_FINISH_CURVE_TIMESTAMP_OFFSET_V1, DBC_IS_MIGRATED_OFFSET_V1,
    DBC_MIGRATION_PROGRESS_OFFSET_V1, DBC_PROGRAM_POSITION_V1, DBC_PROGRAMDATA_POSITION_V1,
    DBC_TRANSFER_HOOK_POOL_DISCRIMINATOR_V1, DBC_VENUE_INLINE_BYTES_V1, DBC_VENUE_POSITION_V1,
    DBC_VENUE_SET_CARDINALITY_V1, DBC_VIRTUAL_POOL_DISCRIMINATOR_V1,
    EPOCH_SCHEDULE_ADMITTED_DATA_LENGTHS_V1, EPOCH_SCHEDULE_FIRST_NORMAL_EPOCH_OFFSET_V1,
    EPOCH_SCHEDULE_FIRST_NORMAL_SLOT_OFFSET_V1, EPOCH_SCHEDULE_INLINE_BYTES_V1,
    EPOCH_SCHEDULE_SLOTS_PER_EPOCH_OFFSET_V1, EPOCH_SCHEDULE_WARMUP_OFFSET_V1, Error,
    FEATURE_ACTIVATED_AT_OFFSET_V1, FEATURE_ADMITTED_DATA_LENGTHS_V1,
    FEATURE_GATE_CLOCK_POSITION_V1, FEATURE_GATE_SET_CARDINALITY_V1,
    FEATURE_GATE_STATE_POSITION_V1, FEATURE_INLINE_BYTES_V1, FEATURE_NONE_TAG_V1,
    FEATURE_NOT_ACTIVATED_SENTINEL_V1, FEATURE_PROGRAM_ID_V1, FEATURE_SOME_TAG_V1,
    FEATURE_TAG_OFFSET_V1, MEAN_SLOT_TIME_CLOCK_POSITION_V1, MEAN_SLOT_TIME_SET_CARDINALITY_V1,
    MEAN_SLOT_TIME_STATE_POSITION_V1, MIGRATION_PROGRESS_CREATED_POOL_V1,
    MIGRATION_PROGRESS_LOCKED_VESTING_V1, MIGRATION_PROGRESS_POST_BONDING_CURVE_V1,
    MIGRATION_PROGRESS_PRE_BONDING_CURVE_V1, MINT_ADMITTED_DATA_LENGTHS_V1,
    MINT_AUTHORITY_CLOCK_POSITION_V1, MINT_AUTHORITY_HELD_V1, MINT_AUTHORITY_MINT_POSITION_V1,
    MINT_AUTHORITY_PROGRAM_POSITION_V1, MINT_AUTHORITY_PROGRAMDATA_POSITION_V1,
    MINT_AUTHORITY_RENOUNCED_V1, MINT_AUTHORITY_SET_CARDINALITY_V1, MINT_AUTHORITY_TAG_OFFSET_V1,
    MINT_FREEZE_AUTHORITY_TAG_OFFSET_V1, MINT_INLINE_BYTES_V1, MINT_IS_INITIALIZED_OFFSET_V1,
    OBSERVED_CLOCK_EPOCH_OFFSET_V1, OBSERVED_CLOCK_EPOCH_START_TIMESTAMP_OFFSET_V1,
    OBSERVED_CLOCK_SLOT_OFFSET_V1, OBSERVED_CLOCK_SYSVAR_KEY_V1,
    OBSERVED_CLOCK_UNIX_TIMESTAMP_OFFSET_V1, OBSERVED_EPOCH_SCHEDULE_SYSVAR_KEY_V1,
    OBSERVED_SYSVAR_OWNER_V1, RELAYED_OBSERVABLE_DBC_MIGRATION_PROGRESS_V1,
    RELAYED_OBSERVABLE_DBC_RAW_EXPONENT_V1, RELAYED_OBSERVABLE_FEATURE_GATE_ACTIVATION_V1,
    RELAYED_OBSERVABLE_FEATURE_GATE_RAW_EXPONENT_V1,
    RELAYED_OBSERVABLE_MEAN_SLOT_TIME_RAW_EXPONENT_V1, RELAYED_OBSERVABLE_MEAN_SLOT_TIME_V1,
    RELAYED_OBSERVABLE_MINT_AUTHORITY_RAW_EXPONENT_V1,
    RELAYED_OBSERVABLE_MINT_AUTHORITY_RENOUNCED_V1, Result,
    identity::{LOADER_V3_PROGRAM_ID, reconstruct_deployment_observation_v1},
    record::RelayedObservationRecordViewV1,
    release::{AccountSetEntryV1, RelayedAdapterConfigV1},
    wire::AccountObservationV1,
};

/// [`DBC_VENUE_INLINE_BYTES_V1`] as the `u16` an account-set geometry states.
///
/// The emitted decoding-rules table states every width in `usize` because it
/// indexes byte slices with them; a pinned account's inline width is a `u16` on
/// the wire, and neither `u16::try_from` nor a truncating `as` can narrow one
/// in a `const fn` under this crate's deny table. So the `u16` is written and
/// the pin below LICENSES it: an emitter that moved the width would stop this
/// crate compiling instead of silently disagreeing with a hand-typed number.
/// Same shape as `HEADER_MAGIC_OFFSET` in `lib.rs`.
const DBC_VENUE_INLINE_BYTES_U16_V1: u16 = 424;
const _: () = assert!(
    DBC_VENUE_INLINE_BYTES_U16_V1 as usize == DBC_VENUE_INLINE_BYTES_V1,
    "the emitted DBC venue inline width no longer fits the u16 an account-set \
     geometry states it in"
);

/// [`MINT_INLINE_BYTES_V1`] as the `u16` an account-set geometry states.
///
/// Licensed by the same proof as [`DBC_VENUE_INLINE_BYTES_U16_V1`].
const MINT_INLINE_BYTES_U16_V1: u16 = 82;
const _: () = assert!(
    MINT_INLINE_BYTES_U16_V1 as usize == MINT_INLINE_BYTES_V1,
    "the emitted Token-2022 Mint inline width no longer fits the u16 an \
     account-set geometry states it in"
);

/// [`FEATURE_INLINE_BYTES_V1`] as the `u16` an account-set geometry states.
///
/// Licensed by the same proof as [`DBC_VENUE_INLINE_BYTES_U16_V1`].
const FEATURE_INLINE_BYTES_U16_V1: u16 = 9;
const _: () = assert!(
    FEATURE_INLINE_BYTES_U16_V1 as usize == FEATURE_INLINE_BYTES_V1,
    "the emitted Feature account inline width no longer fits the u16 an \
     account-set geometry states it in"
);

/// [`EPOCH_SCHEDULE_INLINE_BYTES_V1`] as the `u16` an account-set geometry
/// states.
///
/// Licensed by the same proof as [`DBC_VENUE_INLINE_BYTES_U16_V1`].
const EPOCH_SCHEDULE_INLINE_BYTES_U16_V1: u16 = 33;
const _: () = assert!(
    EPOCH_SCHEDULE_INLINE_BYTES_U16_V1 as usize == EPOCH_SCHEDULE_INLINE_BYTES_V1,
    "the emitted EpochSchedule inline width no longer fits the u16 an \
     account-set geometry states it in"
);

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
    /// A Feature program account's `activated_at`, as "activated by slot
    /// `S`": the decision parent of the flagship conditional market (decision
    /// 0029, tenth item). Both branches are OBSERVED — `Some(a)` yields `a`,
    /// `None` yields a sentinel above every slot — and the Product's cut at
    /// `S + 1` separates them.
    FeatureGateActivationV1,
    /// Mainnet's mean slot duration since the current epoch began, in
    /// milliseconds, from the `Clock` and `EpochSchedule` sysvars alone: the
    /// metric parent of the flagship (decision 0029, tenth item). One
    /// attested record, one atom, no venue program.
    MeanSlotTimeSinceEpochStartV1,
}

impl RelayedObservableV1 {
    /// Select one row of the table.
    pub fn from_selector(selector: u32) -> Result<Self> {
        if selector == RELAYED_OBSERVABLE_DBC_MIGRATION_PROGRESS_V1 {
            Ok(Self::DbcMigrationProgressV1)
        } else if selector == RELAYED_OBSERVABLE_MINT_AUTHORITY_RENOUNCED_V1 {
            Ok(Self::Token2022MintAuthorityRenouncedV1)
        } else if selector == RELAYED_OBSERVABLE_FEATURE_GATE_ACTIVATION_V1 {
            Ok(Self::FeatureGateActivationV1)
        } else if selector == RELAYED_OBSERVABLE_MEAN_SLOT_TIME_V1 {
            Ok(Self::MeanSlotTimeSinceEpochStartV1)
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
            Self::FeatureGateActivationV1 => RELAYED_OBSERVABLE_FEATURE_GATE_ACTIVATION_V1,
            Self::MeanSlotTimeSinceEpochStartV1 => RELAYED_OBSERVABLE_MEAN_SLOT_TIME_V1,
        }
    }

    /// The declared base-ten scale of the atom this row produces.
    pub const fn raw_exponent(self) -> i32 {
        match self {
            Self::DbcMigrationProgressV1 => RELAYED_OBSERVABLE_DBC_RAW_EXPONENT_V1,
            Self::Token2022MintAuthorityRenouncedV1 => {
                RELAYED_OBSERVABLE_MINT_AUTHORITY_RAW_EXPONENT_V1
            }
            Self::FeatureGateActivationV1 => RELAYED_OBSERVABLE_FEATURE_GATE_RAW_EXPONENT_V1,
            Self::MeanSlotTimeSinceEpochStartV1 => {
                RELAYED_OBSERVABLE_MEAN_SLOT_TIME_RAW_EXPONENT_V1
            }
        }
    }

    /// The exact cardinality of the ordered account set this row reads.
    pub const fn set_cardinality(self) -> u16 {
        match self {
            Self::DbcMigrationProgressV1 => DBC_VENUE_SET_CARDINALITY_V1,
            Self::Token2022MintAuthorityRenouncedV1 => MINT_AUTHORITY_SET_CARDINALITY_V1,
            Self::FeatureGateActivationV1 => FEATURE_GATE_SET_CARDINALITY_V1,
            Self::MeanSlotTimeSinceEpochStartV1 => MEAN_SLOT_TIME_SET_CARDINALITY_V1,
        }
    }

    /// The exact inline width this row's state position is pinned at.
    ///
    /// Every row carries its state account WHOLE, which the decoding rules
    /// prove: `venueInlineBytes == admittedDataLengths` for row 0,
    /// `mintInlineBytes == mintAdmittedDataLengths` for row 1,
    /// `featureInlineBytes == featureAdmittedDataLengths` for row 2 and
    /// `epochScheduleInlineBytes == epochScheduleAdmittedDataLengths` for row
    /// 3. A founder building the pinned account set reads this rather than
    /// typing the number a second time.
    pub const fn state_inline_bytes(self) -> u16 {
        match self {
            Self::DbcMigrationProgressV1 => DBC_VENUE_INLINE_BYTES_U16_V1,
            Self::Token2022MintAuthorityRenouncedV1 => MINT_INLINE_BYTES_U16_V1,
            Self::FeatureGateActivationV1 => FEATURE_INLINE_BYTES_U16_V1,
            Self::MeanSlotTimeSinceEpochStartV1 => EPOCH_SCHEDULE_INLINE_BYTES_U16_V1,
        }
    }

    /// Where each structural role sits in this row's ordered account set.
    ///
    /// Rows 0 and 1 are [`RelayedVenueKindV1::LoaderV3`] and carry all four
    /// roles; rows 2 and 3 are [`RelayedVenueKindV1::Native`] and carry no
    /// `program`/`programdata` position at all — there is no upgradeable
    /// program to authenticate, only an owner to pin.
    pub const fn set_layout(self) -> RelayedSetLayoutV1 {
        match self {
            Self::DbcMigrationProgressV1 => RelayedSetLayoutV1 {
                program: Some(DBC_PROGRAM_POSITION_V1),
                programdata: Some(DBC_PROGRAMDATA_POSITION_V1),
                state: DBC_VENUE_POSITION_V1,
                clock: DBC_CLOCK_POSITION_V1,
            },
            Self::Token2022MintAuthorityRenouncedV1 => RelayedSetLayoutV1 {
                program: Some(MINT_AUTHORITY_PROGRAM_POSITION_V1),
                programdata: Some(MINT_AUTHORITY_PROGRAMDATA_POSITION_V1),
                state: MINT_AUTHORITY_MINT_POSITION_V1,
                clock: MINT_AUTHORITY_CLOCK_POSITION_V1,
            },
            Self::FeatureGateActivationV1 => RelayedSetLayoutV1 {
                program: None,
                programdata: None,
                state: FEATURE_GATE_STATE_POSITION_V1,
                clock: FEATURE_GATE_CLOCK_POSITION_V1,
            },
            Self::MeanSlotTimeSinceEpochStartV1 => RelayedSetLayoutV1 {
                program: None,
                programdata: None,
                state: MEAN_SLOT_TIME_STATE_POSITION_V1,
                clock: MEAN_SLOT_TIME_CLOCK_POSITION_V1,
            },
        }
    }

    /// Which program owns this row's state account, and therefore whether the
    /// pinned account set carries `Program`/`ProgramData` positions at all.
    pub const fn venue_kind(self) -> RelayedVenueKindV1 {
        match self {
            Self::DbcMigrationProgressV1 | Self::Token2022MintAuthorityRenouncedV1 => {
                RelayedVenueKindV1::LoaderV3
            }
            Self::FeatureGateActivationV1 | Self::MeanSlotTimeSinceEpochStartV1 => {
                RelayedVenueKindV1::Native
            }
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
    RelayedObservableV1::FeatureGateActivationV1,
    RelayedObservableV1::MeanSlotTimeSinceEpochStartV1,
];

/// What owns a row's state account, and therefore how the cross-cluster
/// deployment defense (`MAINNET_STATE_RELAY.md` §4.6) is discharged.
///
/// * [`Self::LoaderV3`]: the state is owned by an upgradeable program. The set
///   carries the program's `Program` and `ProgramData` bodies and the adapter
///   rebuilds a [`DeploymentObservationV1`] and authenticates it against the
///   pinned release — the Loopscale defense in full, because the venue can be
///   upgraded under a market.
/// * [`Self::Native`]: the state is owned by a program the validator itself
///   implements (the Feature program, the sysvar owner). There is no
///   `ProgramData`, no ELF digest and no upgrade authority: what the defense
///   pins is the owner's address, which the pinned account-set entry already
///   binds byte for byte and which no transaction on the observed cluster can
///   move. So the two program positions are ABSENT from the set, and
///   `require_pinned_venue` is replaced by [`require_native_owner`] plus, for
///   the sysvar row, an address check below it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelayedVenueKindV1 {
    /// An upgradeable Loader V3 program. `require_pinned_venue` authenticates
    /// its deployment cross-cluster.
    LoaderV3,
    /// A program the runtime itself implements. `require_native_owner`
    /// authenticates its owner and, where the row also pins one, its address.
    Native,
}

/// Which position of one observable's ordered account set carries which role.
///
/// The spine of an interpretation — authenticate the observed venue, read the
/// observed cluster's own clock, check staleness — is identical for every row
/// of the table. The only thing that varies is *where in the set* each of
/// those accounts sits, and how many accounts there are. Holding that as data
/// on the row, beside [`RelayedObservableV1::set_cardinality`], is what lets a
/// second observable be authored without editing
/// [`interpret_sealed_record_v1`]. Before this existed the DBC positions were
/// read inside the orchestration, and an observable with a different
/// cardinality would have had to fork it.
///
/// `state` and `clock` are required on every row: every observable reads its
/// own state and needs the observed cluster's time. `program` and
/// `programdata` are `None` on a [`RelayedVenueKindV1::Native`] row, whose set
/// has no such positions to name — see [`RelayedVenueKindV1`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelayedSetLayoutV1 {
    /// Loader V3 `Program` account of the observed venue. `None` for a
    /// [`RelayedVenueKindV1::Native`] row.
    pub program: Option<u16>,
    /// Loader V3 `ProgramData` account holding the observed venue's ELF.
    /// `None` for a [`RelayedVenueKindV1::Native`] row.
    pub programdata: Option<u16>,
    /// The account whose attested bytes carry this observable's own state.
    pub state: u16,
    /// The observed cluster's `Clock` sysvar.
    pub clock: u16,
}

impl RelayedSetLayoutV1 {
    /// Every role of this row, in set order. `None` marks a `program` or
    /// `programdata` role absent on a [`RelayedVenueKindV1::Native`] row;
    /// `state` and `clock` are always present.
    const fn positions(self) -> [Option<u16>; 4] {
        [
            self.program,
            self.programdata,
            Some(self.state),
            Some(self.clock),
        ]
    }

    /// Refuse a table row whose present roles collide or fall outside its own
    /// set.
    ///
    /// A malformed row is a defect in the emitted decoding-rules table rather
    /// than in any caller's record, so this can never fire for a shipped row —
    /// [`RELAYED_OBSERVABLE_TABLE_V1`] is walked by a test that asserts it. It
    /// is here so that the author of observable #4 gets a named refusal at the
    /// interpretation boundary instead of two roles silently reading one body.
    fn require_well_formed(self, cardinality: u16) -> Result<()> {
        let positions = self.positions();
        let mut remaining = positions.as_slice();
        while let [role, rest @ ..] = remaining {
            if let Some(role) = role
                && (*role >= cardinality || rest.contains(&Some(*role)))
            {
                return Err(Error::InvalidSetGeometry);
            }
            remaining = rest;
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
    venue_deployment: Option<DeploymentObservationV1>,
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
    /// `None` for a [`RelayedVenueKindV1::Native`] row, which has no
    /// `Program`/`ProgramData` positions to reconstruct one from.
    pub const fn venue_deployment(self) -> Option<DeploymentObservationV1> {
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
    // Both positions are required for a `LoaderV3` row — `venue_kind` never
    // pairs one with `None` — so these `ok_or`s name that invariant rather
    // than a reachable runtime state.
    let program_position = layout.program.ok_or(Error::InvalidSetGeometry)?;
    let programdata_position = layout.programdata.ok_or(Error::InvalidSetGeometry)?;
    let program_entry = entries
        .get(usize::from(program_position))
        .ok_or(Error::InvalidSetGeometry)?;
    let programdata_entry = entries
        .get(usize::from(programdata_position))
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
    if crate::relay::u64_at(inline, OBSERVED_CLOCK_SLOT_OFFSET_V1)? != record.observed_slot()? {
        return Err(Error::ObservedSlotMismatch);
    }
    let observed_unix_seconds =
        crate::relay::i64_at(inline, OBSERVED_CLOCK_UNIX_TIMESTAMP_OFFSET_V1)?;
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
    let discriminator: [u8; 8] = crate::relay::array(inline, DBC_DISCRIMINATOR_OFFSET_V1)?;
    let _ = DBC_DISCRIMINATOR_BYTES_V1;
    if discriminator != DBC_VIRTUAL_POOL_DISCRIMINATOR_V1 {
        // `TransferHookPool` shares the identical 424-byte body after 0.2.0, so
        // the discriminator is what stops one decoding under rules minted for
        // the other. It is named in the table precisely so this refusal is a
        // decision rather than an accident.
        let _ = DBC_TRANSFER_HOOK_POOL_DISCRIMINATOR_V1;
        return Err(Error::VenueDiscriminatorMismatch);
    }
    let progress = MigrationProgressV1::from_byte(crate::relay::one(
        inline,
        DBC_MIGRATION_PROGRESS_OFFSET_V1,
    )?)?;
    let is_migrated = crate::relay::one(inline, DBC_IS_MIGRATED_OFFSET_V1)?;
    let finish_curve_timestamp =
        crate::relay::u64_at(inline, DBC_FINISH_CURVE_TIMESTAMP_OFFSET_V1)?;
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
    let state = MintAuthorityStateV1::from_tag(crate::relay::u32_at(
        inline,
        MINT_AUTHORITY_TAG_OFFSET_V1,
    )?)?;
    let is_initialized = crate::relay::one(inline, MINT_IS_INITIALIZED_OFFSET_V1)?;
    let freeze_authority_tag = crate::relay::u32_at(inline, MINT_FREEZE_AUTHORITY_TAG_OFFSET_V1)?;
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

/// Authenticate a [`RelayedVenueKindV1::Native`] row's state account.
///
/// The runtime itself is the only thing that can move what owns a Feature
/// account or a sysvar, so the defense a native row needs is an owner pin
/// rather than `require_pinned_venue`'s cross-cluster ELF check: the pinned
/// entry's `expected_owner` must itself be the address the row names, and the
/// attested body must report that same owner. [`RelayedVenueKindV1`]'s doc
/// explains why the two `Program`/`ProgramData` positions are absent instead
/// of being read here.
fn require_native_owner(
    entries: &[AccountSetEntryV1],
    layout: RelayedSetLayoutV1,
    state: AccountObservationV1<'_>,
    expected_owner: [u8; ADDRESS_BYTES],
) -> Result<()> {
    let state_entry = entries
        .get(usize::from(layout.state))
        .ok_or(Error::InvalidSetGeometry)?;
    if state_entry.expected_owner != expected_owner || state.owner() != expected_owner {
        return Err(Error::ObservedOwnerMismatch);
    }
    Ok(())
}

/// Apply the Feature program's `bincode(Option<u64>)` grammar to one attested
/// state body: `featureActivationAtoms` in the decoding rules, and the
/// decision parent of the flagship conditional market (decision 0029, tenth
/// item). Both branches are OBSERVED — `Some(a)` yields `a`, `None` yields a
/// sentinel above every slot a `u64` can name — and the Product's own cut at
/// `S + 1` is what separates them.
fn read_feature_gate_activation(body: AccountObservationV1<'_>) -> Result<i128> {
    if !FEATURE_ADMITTED_DATA_LENGTHS_V1
        .iter()
        .any(|admitted| *admitted == body.data_len())
    {
        return Err(Error::VenueLengthNotAdmitted);
    }
    if body.inline().len() != FEATURE_INLINE_BYTES_V1 || body.executable() {
        return Err(Error::InvalidInlineWidth);
    }
    let inline = body.inline();
    let tag = crate::relay::one(inline, FEATURE_TAG_OFFSET_V1)?;
    let activated_at = crate::relay::u64_at(inline, FEATURE_ACTIVATED_AT_OFFSET_V1)?;
    if tag == FEATURE_SOME_TAG_V1 {
        Ok(i128::from(activated_at))
    } else if tag == FEATURE_NONE_TAG_V1 {
        if activated_at == 0 {
            Ok(i128::from(FEATURE_NOT_ACTIVATED_SENTINEL_V1))
        } else {
            // `bincode`'s `None` never carries a payload; a `None` tag with a
            // nonzero slot is not canonical bincode and not a body this
            // program wrote.
            Err(Error::IncoherentVenueBody)
        }
    } else {
        Err(Error::UnenumeratedVenueState)
    }
}

/// Apply the `bincode(EpochSchedule)` and `Clock` grammars together:
/// `meanSlotTimeMillis` in the decoding rules, computing mainnet's own mean
/// slot duration since the current epoch began — the metric parent of the
/// flagship conditional market (decision 0029, tenth item). One attested
/// record, one atom, no venue program.
///
/// `schedule` is this row's own state position; `clock` is the same clock
/// position [`require_observed_clock`] already read once for its freshness
/// bound. Reading it a second time here is the minimal way to hand this row
/// the clock's inline bytes without widening that function's return type for
/// every other row.
fn read_mean_slot_time(
    schedule: AccountObservationV1<'_>,
    clock: AccountObservationV1<'_>,
) -> Result<i128> {
    if !EPOCH_SCHEDULE_ADMITTED_DATA_LENGTHS_V1
        .iter()
        .any(|admitted| *admitted == schedule.data_len())
    {
        return Err(Error::VenueLengthNotAdmitted);
    }
    if schedule.inline().len() != EPOCH_SCHEDULE_INLINE_BYTES_V1 || schedule.executable() {
        return Err(Error::InvalidInlineWidth);
    }
    let schedule_inline = schedule.inline();
    let slots_per_epoch =
        crate::relay::u64_at(schedule_inline, EPOCH_SCHEDULE_SLOTS_PER_EPOCH_OFFSET_V1)?;
    let warmup = crate::relay::one(schedule_inline, EPOCH_SCHEDULE_WARMUP_OFFSET_V1)?;
    if warmup > 1 {
        // `bincode(bool)` admits only `0` and `1`; anything else is not a
        // schedule this runtime ever wrote.
        return Err(Error::IncoherentVenueBody);
    }
    let first_normal_epoch =
        crate::relay::u64_at(schedule_inline, EPOCH_SCHEDULE_FIRST_NORMAL_EPOCH_OFFSET_V1)?;
    let first_normal_slot =
        crate::relay::u64_at(schedule_inline, EPOCH_SCHEDULE_FIRST_NORMAL_SLOT_OFFSET_V1)?;

    // The clock inline is read fresh here rather than trusting the caller's
    // earlier read: this function's contract is the same "one attested body,
    // hostile-parsed" as every other `read_*` in this module.
    let clock_inline = clock.inline();
    let slot = crate::relay::u64_at(clock_inline, OBSERVED_CLOCK_SLOT_OFFSET_V1)?;
    let epoch_start_timestamp =
        crate::relay::i64_at(clock_inline, OBSERVED_CLOCK_EPOCH_START_TIMESTAMP_OFFSET_V1)?;
    let epoch = crate::relay::u64_at(clock_inline, OBSERVED_CLOCK_EPOCH_OFFSET_V1)?;
    let unix_timestamp =
        crate::relay::i64_at(clock_inline, OBSERVED_CLOCK_UNIX_TIMESTAMP_OFFSET_V1)?;

    if slots_per_epoch == 0 {
        return Err(Error::IncoherentVenueBody);
    }
    if epoch < first_normal_epoch {
        // A warmup epoch's start-slot arithmetic is a different function this
        // row does not carry; mainnet's own `first_normal_epoch` is zero, so
        // this can only fire for a schedule this row was never meant to read.
        return Err(Error::IncoherentVenueBody);
    }
    let epochs_past_first_normal = epoch
        .checked_sub(first_normal_epoch)
        .ok_or(Error::ArithmeticOverflow)?;
    let slots_into_normal = epochs_past_first_normal
        .checked_mul(slots_per_epoch)
        .ok_or(Error::ArithmeticOverflow)?;
    let epoch_start_slot = first_normal_slot
        .checked_add(slots_into_normal)
        .ok_or(Error::ArithmeticOverflow)?;

    if slot <= epoch_start_slot {
        // No elapsed slots, no mean: an observation at the epoch's own first
        // slot is not a negative answer, it is no answer.
        return Err(Error::IncoherentVenueBody);
    }
    if unix_timestamp < epoch_start_timestamp {
        // The observed cluster's own clock disagreeing with itself about
        // which came first is not a body this row can make sense of.
        return Err(Error::IncoherentVenueBody);
    }

    let elapsed_slots = slot
        .checked_sub(epoch_start_slot)
        .ok_or(Error::ArithmeticOverflow)?;
    let elapsed_seconds = unix_timestamp
        .checked_sub(epoch_start_timestamp)
        .ok_or(Error::ArithmeticOverflow)?;
    let elapsed_seconds: u128 =
        u128::try_from(elapsed_seconds).map_err(|_| Error::ArithmeticOverflow)?;
    let elapsed_millis = elapsed_seconds
        .checked_mul(1000)
        .ok_or(Error::ArithmeticOverflow)?;
    let atoms = elapsed_millis
        .checked_div(u128::from(elapsed_slots))
        .ok_or(Error::ArithmeticOverflow)?;
    i128::try_from(atoms).map_err(|_| Error::ArithmeticOverflow)
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
/// 4. the venue is authenticated: a [`RelayedVenueKindV1::LoaderV3`] row
///    reconstructs and checks its deployment cross-cluster; a
///    [`RelayedVenueKindV1::Native`] row checks its state account's owner
///    against the runtime-owned pin, and, where the row itself pins the
///    account's address, that address too,
///
/// `pinned_venue_release` is `Some` for exactly the [`RelayedVenueKindV1::LoaderV3`]
/// rows and `None` for the [`RelayedVenueKindV1::Native`] ones, and the
/// mismatch in either direction is its own refusal
/// ([`Error::VenueReleaseAbsent`], [`Error::VenueReleaseNotPinnable`]) rather
/// than a silently ignored argument. The transport frame carries the release's
/// two positions only in the kind that has one to carry
/// ([`CONSUME_RECORD_NATIVE_VENUE_FRAME_V1`]).
///
/// [`CONSUME_RECORD_NATIVE_VENUE_FRAME_V1`]: crate::relay::frame::CONSUME_RECORD_NATIVE_VENUE_FRAME_V1
/// 5. the observed clock is the observed cluster's own, at the record's slot,
/// 6. the observation is inside the two-clock staleness bound,
/// 7. the venue body parses, is coherent, and (for a terminal-window row) is
///    terminal.
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
    pinned_venue_release: Option<ArtifactReleaseV1>,
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
    crate::relay::identity::require_observed_cluster(
        record.observed_cluster_id()?,
        pinned_cluster_id,
    )?;
    require_pinned_set(
        record,
        config,
        entries,
        recomputed_account_set_id,
        observable.set_cardinality(),
    )?;

    let layout = observable.set_layout();
    layout.require_well_formed(observable.set_cardinality())?;

    let state = pinned_body(record, entries, layout.state)?;

    // The venue's own authentication: the ONE place the two `VenueKind`s
    // diverge before the observable-specific read at the bottom.
    let venue_deployment = match observable.venue_kind() {
        RelayedVenueKindV1::LoaderV3 => {
            // `set_layout` never pairs a `LoaderV3` row with an absent
            // program role — `every_table_row_states_a_well_formed_set_layout`
            // walks every shipped row to say so — so these `ok_or`s name that
            // invariant rather than a reachable runtime state.
            let program_position = layout.program.ok_or(Error::InvalidSetGeometry)?;
            let programdata_position = layout.programdata.ok_or(Error::InvalidSetGeometry)?;
            let program = pinned_body(record, entries, program_position)?;
            let programdata = pinned_body(record, entries, programdata_position)?;
            Some(require_pinned_venue(
                entries,
                layout,
                program,
                programdata,
                state,
                pinned_venue_release.ok_or(Error::VenueReleaseAbsent)?,
            )?)
        }
        RelayedVenueKindV1::Native => {
            if pinned_venue_release.is_some() {
                // The argument used to be unconditional and this arm ignored
                // it, which made a caller's belief that it had pinned a
                // deployment unfalsifiable. A native row is refused for
                // carrying one rather than quietly resolved without it.
                return Err(Error::VenueReleaseNotPinnable);
            }
            // Exactly two rows are `Native` — `venue_kind` says so, and the
            // table walk below keeps it true — so telling them apart needs
            // only the one `if`.
            let is_feature_gate =
                matches!(observable, RelayedObservableV1::FeatureGateActivationV1);
            let expected_owner = if is_feature_gate {
                FEATURE_PROGRAM_ID_V1
            } else {
                OBSERVED_SYSVAR_OWNER_V1
            };
            require_native_owner(entries, layout, state, expected_owner)?;
            if matches!(
                observable,
                RelayedObservableV1::MeanSlotTimeSinceEpochStartV1
            ) {
                // The `EpochSchedule` sysvar's address is fixed, unlike a
                // Feature account's, so this row pins it directly rather than
                // trusting the owner check alone.
                let schedule_entry = entries
                    .get(usize::from(layout.state))
                    .ok_or(Error::InvalidSetGeometry)?;
                if schedule_entry.key != OBSERVED_EPOCH_SCHEDULE_SYSVAR_KEY_V1 {
                    // No `Error` variant names "the observed sysvar is not the
                    // pinned one"; that is the same defect
                    // `require_pinned_venue`'s owner check exists to catch,
                    // one level up, so it is refused under the same name
                    // rather than adding a variant for one row.
                    return Err(Error::ObservedOwnerMismatch);
                }
            }
            None
        }
    };

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
        RelayedObservableV1::FeatureGateActivationV1 => read_feature_gate_activation(state)?,
        RelayedObservableV1::MeanSlotTimeSinceEpochStartV1 => {
            // The clock position was already read once above, for its own
            // unix-timestamp bound; reading it again is the minimal way to
            // hand this row the clock's inline bytes — see
            // `read_mean_slot_time`'s doc.
            let clock = pinned_body(record, entries, layout.clock)?;
            read_mean_slot_time(state, clock)?
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
    use crate::relay::{
        DBC_GRADUATION_ACCEPTANCE_TABLE_V1, DBC_GRADUATION_ACCEPTANCE_TABLE_V1_COUNT,
        MINT_AUTHORITY_ACCEPTANCE_TABLE_V1, MINT_AUTHORITY_ACCEPTANCE_TABLE_V1_COUNT,
        SOLANA_MAINNET_GENESIS_HASH_V1,
        record::{
            RelayedRecordBindingV1, append_relayed_observation_in_place_v1,
            create_relayed_observation_record_into_v1, relayed_observation_record_bytes_v1,
            seal_relayed_observation_in_place_v1,
        },
        wire::{AttestationMessageV1, ObservationSetSealV1},
    };
    use dclutch_core_contract::ContentId;
    use dclutch_registry::ArtifactUpgradePolicyV1;
    use dclutch_registry::release_set::ProgramIdentityV1;
    use dclutch_registry::svm::{LOADER_V3_PROGRAM_BYTES, LOADER_V3_PROGRAMDATA_METADATA_BYTES};

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
            crate::relay::SHA256_EMPTY_DIGEST,
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
        assert_eq!(
            RelayedObservableV1::from_selector(2),
            Ok(RelayedObservableV1::FeatureGateActivationV1)
        );
        assert_eq!(
            RelayedObservableV1::from_selector(3),
            Ok(RelayedObservableV1::MeanSlotTimeSinceEpochStartV1)
        );
        // Driven off the table rather than off a literal, so adding a row
        // moves this boundary instead of turning the test into a lie.
        for selector in [4u32, 5, 0xffff_ffff] {
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
            crate::relay::SHA256_EMPTY_DIGEST,
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
                !MINT_ADMITTED_DATA_LENGTHS_V1.contains(&foreign_len),
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
                RelayedObservableV1::FeatureGateActivationV1 => {}
                RelayedObservableV1::MeanSlotTimeSinceEpochStartV1 => {}
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
            // A `Native` row's set carries no venue program to authenticate.
            match observable.venue_kind() {
                RelayedVenueKindV1::LoaderV3 => {
                    assert!(layout.program.is_some());
                    assert!(layout.programdata.is_some());
                }
                RelayedVenueKindV1::Native => {
                    assert_eq!(layout.program, None);
                    assert_eq!(layout.programdata, None);
                }
            }
        }
        assert_eq!(RELAYED_OBSERVABLE_TABLE_V1.len(), 4);
        // The state width every row pins is the one its own grammar admits.
        assert_eq!(
            u32::from(RelayedObservableV1::DbcMigrationProgressV1.state_inline_bytes()),
            DBC_ADMITTED_DATA_LENGTHS_V1[0]
        );
        assert_eq!(
            u32::from(RelayedObservableV1::Token2022MintAuthorityRenouncedV1.state_inline_bytes()),
            MINT_ADMITTED_DATA_LENGTHS_V1[0]
        );
        assert_eq!(
            u32::from(RelayedObservableV1::FeatureGateActivationV1.state_inline_bytes()),
            FEATURE_ADMITTED_DATA_LENGTHS_V1[0]
        );
        assert_eq!(
            u32::from(RelayedObservableV1::MeanSlotTimeSinceEpochStartV1.state_inline_bytes()),
            EPOCH_SCHEDULE_ADMITTED_DATA_LENGTHS_V1[0]
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

    fn feature_body(tag: u8, activated_at: u64) -> [u8; FEATURE_INLINE_BYTES_V1] {
        let mut data = [0u8; FEATURE_INLINE_BYTES_V1];
        data[FEATURE_TAG_OFFSET_V1] = tag;
        data[FEATURE_ACTIVATED_AT_OFFSET_V1..FEATURE_ACTIVATED_AT_OFFSET_V1 + 8]
            .copy_from_slice(&activated_at.to_le_bytes());
        data
    }

    fn feature_observation(body: &[u8]) -> AccountObservationV1<'_> {
        AccountObservationV1::new(
            [0x33; 32],
            FEATURE_PROGRAM_ID_V1,
            1_000_000,
            u32::try_from(FEATURE_INLINE_BYTES_V1).expect("fits"),
            body,
            false,
            crate::relay::SHA256_EMPTY_DIGEST,
        )
        .expect("canonical feature body")
    }

    #[test]
    fn an_activated_feature_carries_its_slot_and_a_dormant_one_carries_the_sentinel() {
        let activated = feature_body(FEATURE_SOME_TAG_V1, 12345);
        assert_eq!(
            read_feature_gate_activation(feature_observation(&activated)),
            Ok(i128::from(12345u64))
        );
        let dormant = feature_body(FEATURE_NONE_TAG_V1, 0);
        assert_eq!(
            read_feature_gate_activation(feature_observation(&dormant)),
            Ok(i128::from(FEATURE_NOT_ACTIVATED_SENTINEL_V1))
        );
    }

    #[test]
    fn an_unenumerated_feature_tag_refuses() {
        let body = feature_body(2, 999);
        assert_eq!(
            read_feature_gate_activation(feature_observation(&body)),
            Err(Error::UnenumeratedVenueState)
        );
    }

    #[test]
    fn a_noncanonical_dormant_feature_refuses() {
        let body = feature_body(FEATURE_NONE_TAG_V1, 7);
        assert_eq!(
            read_feature_gate_activation(feature_observation(&body)),
            Err(Error::IncoherentVenueBody)
        );
    }

    fn schedule_body(
        slots_per_epoch: u64,
        warmup: u8,
        first_normal_epoch: u64,
        first_normal_slot: u64,
    ) -> [u8; EPOCH_SCHEDULE_INLINE_BYTES_V1] {
        let mut data = [0u8; EPOCH_SCHEDULE_INLINE_BYTES_V1];
        data[EPOCH_SCHEDULE_SLOTS_PER_EPOCH_OFFSET_V1
            ..EPOCH_SCHEDULE_SLOTS_PER_EPOCH_OFFSET_V1 + 8]
            .copy_from_slice(&slots_per_epoch.to_le_bytes());
        data[EPOCH_SCHEDULE_WARMUP_OFFSET_V1] = warmup;
        data[EPOCH_SCHEDULE_FIRST_NORMAL_EPOCH_OFFSET_V1
            ..EPOCH_SCHEDULE_FIRST_NORMAL_EPOCH_OFFSET_V1 + 8]
            .copy_from_slice(&first_normal_epoch.to_le_bytes());
        data[EPOCH_SCHEDULE_FIRST_NORMAL_SLOT_OFFSET_V1
            ..EPOCH_SCHEDULE_FIRST_NORMAL_SLOT_OFFSET_V1 + 8]
            .copy_from_slice(&first_normal_slot.to_le_bytes());
        data
    }

    fn schedule_observation(body: &[u8]) -> AccountObservationV1<'_> {
        schedule_observation_at(OBSERVED_EPOCH_SCHEDULE_SYSVAR_KEY_V1, body)
    }

    /// The same body under a caller-chosen address, so a test can put a
    /// sysvar-owned account that is *not* the `EpochSchedule` at the row's
    /// state position.
    fn schedule_observation_at(key: [u8; ADDRESS_BYTES], body: &[u8]) -> AccountObservationV1<'_> {
        AccountObservationV1::new(
            key,
            OBSERVED_SYSVAR_OWNER_V1,
            1_000_000,
            u32::try_from(EPOCH_SCHEDULE_INLINE_BYTES_V1).expect("fits"),
            body,
            false,
            crate::relay::SHA256_EMPTY_DIGEST,
        )
        .expect("canonical epoch schedule body")
    }

    fn clock_body(
        slot: u64,
        epoch_start_timestamp: i64,
        epoch: u64,
        unix_timestamp: i64,
    ) -> [u8; crate::relay::MAINNET_CLOCK_SYSVAR_BYTES_V1] {
        let mut data = [0u8; crate::relay::MAINNET_CLOCK_SYSVAR_BYTES_V1];
        data[OBSERVED_CLOCK_SLOT_OFFSET_V1..OBSERVED_CLOCK_SLOT_OFFSET_V1 + 8]
            .copy_from_slice(&slot.to_le_bytes());
        data[OBSERVED_CLOCK_EPOCH_START_TIMESTAMP_OFFSET_V1
            ..OBSERVED_CLOCK_EPOCH_START_TIMESTAMP_OFFSET_V1 + 8]
            .copy_from_slice(&epoch_start_timestamp.to_le_bytes());
        data[OBSERVED_CLOCK_EPOCH_OFFSET_V1..OBSERVED_CLOCK_EPOCH_OFFSET_V1 + 8]
            .copy_from_slice(&epoch.to_le_bytes());
        data[OBSERVED_CLOCK_UNIX_TIMESTAMP_OFFSET_V1..OBSERVED_CLOCK_UNIX_TIMESTAMP_OFFSET_V1 + 8]
            .copy_from_slice(&unix_timestamp.to_le_bytes());
        data
    }

    fn clock_observation(body: &[u8]) -> AccountObservationV1<'_> {
        AccountObservationV1::new(
            OBSERVED_CLOCK_SYSVAR_KEY_V1,
            OBSERVED_SYSVAR_OWNER_V1,
            1_000_000,
            u32::try_from(crate::relay::MAINNET_CLOCK_SYSVAR_BYTES_V1).expect("fits"),
            body,
            false,
            crate::relay::SHA256_EMPTY_DIGEST,
        )
        .expect("canonical clock body")
    }

    /// Mainnet's schedule (432,000 slots, no warmup), epoch 800, 100,000
    /// slots into the epoch, 40,000 seconds elapsed: 400 ms. The Lean side's
    /// `a_worked_mean`.
    #[test]
    fn the_lean_worked_mean_agrees_with_the_rust_grammar() {
        let schedule = schedule_body(432_000, 0, 0, 0);
        let clock = clock_body(345_700_000, 1_700_000_000, 800, 1_700_040_000);
        assert_eq!(
            read_mean_slot_time(schedule_observation(&schedule), clock_observation(&clock)),
            Ok(400)
        );
    }

    #[test]
    fn an_observation_at_the_epochs_own_first_slot_refuses() {
        let schedule = schedule_body(432_000, 0, 0, 0);
        // Exactly the epoch's own first slot: no elapsed slots, no mean.
        let clock = clock_body(345_600_000, 1_700_000_000, 800, 1_700_040_000);
        assert_eq!(
            read_mean_slot_time(schedule_observation(&schedule), clock_observation(&clock)),
            Err(Error::IncoherentVenueBody)
        );
    }

    #[test]
    fn a_warmup_epoch_refuses() {
        let schedule = schedule_body(432_000, 0, 5, 100);
        let clock = clock_body(3_000, 1_700_000_000, 2, 1_700_040_000);
        assert_eq!(
            read_mean_slot_time(schedule_observation(&schedule), clock_observation(&clock)),
            Err(Error::IncoherentVenueBody)
        );
    }

    #[test]
    fn a_clock_before_its_own_epoch_refuses() {
        let schedule = schedule_body(432_000, 0, 0, 0);
        let clock = clock_body(345_700_000, 1_700_040_000, 800, 1_700_000_000);
        assert_eq!(
            read_mean_slot_time(schedule_observation(&schedule), clock_observation(&clock)),
            Err(Error::IncoherentVenueBody)
        );
    }

    #[test]
    fn a_zero_slot_rate_refuses() {
        let schedule = schedule_body(0, 0, 0, 0);
        let clock = clock_body(345_700_000, 1_700_000_000, 800, 1_700_040_000);
        assert_eq!(
            read_mean_slot_time(schedule_observation(&schedule), clock_observation(&clock)),
            Err(Error::IncoherentVenueBody)
        );
    }

    // Everything above reads one grammar out of one body. Everything below
    // drives the whole interpretation — row selection, the pinned set, every
    // pinned position, the venue's authentication, the observed clock and the
    // staleness bound — over a record that was really created, appended to and
    // sealed, exactly as `record.rs`'s own lifecycle test builds one.

    const MARKET: [u8; ADDRESS_BYTES] = [0x10; 32];
    const GENERATION: u64 = 7;
    const SOURCE_MATERIAL: [u8; 32] = [0x11; 32];
    /// One value plays all three roles `require_pinned_set` compares against
    /// each other — the record's binding, the adapter configuration, and the
    /// digest the caller recomputed — because nothing here hashes.
    const ACCOUNT_SET: [u8; 32] = [0x12; 32];
    const PROVIDER_RELEASE: [u8; 32] = [0x13; 32];
    const KEY_SET: [u8; 32] = [0x14; 32];
    const RELAY_FAMILY: [u8; 32] = [0x50; 32];
    const DECODING_RULES: [u8; 32] = [0x51; 32];
    const BENEFICIARY: [u8; ADDRESS_BYTES] = [0x15; 32];
    const SEED_DIGEST: [u8; 32] = [0x20; 32];
    const CREATED: i64 = 1_699_000_000;
    const SEALED: i64 = 1_700_040_000;

    /// The observed cluster's own numbers. The slot is the one the record is
    /// addressed by, and `require_observed_clock` binds it to the attested
    /// `Clock`; the epoch-800 pair is the Lean's worked 400 ms witness.
    const SLOT: u64 = 345_700_000;
    const EPOCH: u64 = 800;
    const EPOCH_START_UNIX: i64 = 1_700_000_000;
    const OBSERVED_UNIX: i64 = 1_700_040_000;
    /// 120 seconds after the observation: inside both bounds the configuration
    /// below declares.
    const CURRENT_UNIX: i64 = 1_700_040_120;
    const MAX_AGE_SECONDS: u64 = 900;
    const MAX_SKEW_SECONDS: u64 = 60;

    const NATIVE_RECORD_BYTES: usize = 1432;
    const LOADER_V3_RECORD_BYTES: usize = 2552;

    const VENUE_PROGRAM_KEY: [u8; ADDRESS_BYTES] = [0x09; 32];
    const VENUE_PROGRAMDATA_KEY: [u8; ADDRESS_BYTES] = [0xf4; 32];
    const FEATURE_ACCOUNT_KEY: [u8; ADDRESS_BYTES] = [0x33; 32];
    const VENUE_POOL_KEY: [u8; ADDRESS_BYTES] = [0x5a; 32];

    fn binding() -> RelayedRecordBindingV1 {
        RelayedRecordBindingV1 {
            market: MARKET,
            generation: GENERATION,
            source_material_id: SOURCE_MATERIAL,
            account_set_id: ACCOUNT_SET,
            provider_release_id: PROVIDER_RELEASE,
            relayer_key_set_id: KEY_SET,
            observed_cluster_id: SOLANA_MAINNET_GENESIS_HASH_V1,
            observed_slot: SLOT,
        }
    }

    /// Create, fill and seal one real record over `bodies`, in set order.
    ///
    /// The running set digest is any distinct nonzero value per position: this
    /// crate hashes nothing and compares what the caller folded, and the seal
    /// carries the last one.
    fn sealed_record<const N: usize>(bodies: &[AccountObservationV1<'_>]) -> [u8; N] {
        let set_count = u16::try_from(bodies.len()).expect("a set this fixture can build");
        assert_eq!(
            relayed_observation_record_bytes_v1(set_count),
            Ok(N),
            "the fixture's record width must be the one its cardinality names"
        );
        let mut bytes = [0u8; N];
        create_relayed_observation_record_into_v1(
            &mut bytes,
            binding(),
            BENEFICIARY,
            set_count,
            1,
            SEED_DIGEST,
            CREATED,
        )
        .expect("create");
        let mut folded = SEED_DIGEST;
        for (index, body) in bodies.iter().enumerate() {
            let position = u16::try_from(index).expect("a set this fixture can build");
            folded = [0xa1u8.saturating_add(u8::try_from(index).expect("small")); 32];
            let message = AttestationMessageV1::new(
                SOLANA_MAINNET_GENESIS_HASH_V1,
                RELAY_FAMILY,
                DECODING_RULES,
                ACCOUNT_SET,
                SLOT,
                position,
                set_count,
                *body,
            )
            .expect("attestation");
            append_relayed_observation_in_place_v1(&mut bytes, binding(), message, folded)
                .expect("append");
        }
        seal_relayed_observation_in_place_v1(
            &mut bytes,
            binding(),
            ObservationSetSealV1::new(
                SOLANA_MAINNET_GENESIS_HASH_V1,
                RELAY_FAMILY,
                ACCOUNT_SET,
                SLOT,
                set_count,
                folded,
            )
            .expect("seal message"),
            0,
            SEALED,
        )
        .expect("seal");
        bytes
    }

    /// The configuration echoes the row's own selector and scale, which is what
    /// keeps the interpretation from refusing `UnknownObservable` before it
    /// reaches anything a fixture is about.
    fn adapter_config(observable: RelayedObservableV1) -> RelayedAdapterConfigV1 {
        RelayedAdapterConfigV1::new(
            ACCOUNT_SET,
            observable.selector(),
            observable.raw_exponent(),
            MAX_AGE_SECONDS,
            MAX_SKEW_SECONDS,
        )
        .expect("adapter configuration")
    }

    fn clock_entry() -> AccountSetEntryV1 {
        AccountSetEntryV1 {
            key: OBSERVED_CLOCK_SYSVAR_KEY_V1,
            expected_owner: OBSERVED_SYSVAR_OWNER_V1,
            inline_len: u16::try_from(crate::relay::MAINNET_CLOCK_SYSVAR_BYTES_V1).expect("fits"),
        }
    }

    fn observed_clock() -> [u8; crate::relay::MAINNET_CLOCK_SYSVAR_BYTES_V1] {
        clock_body(SLOT, EPOCH_START_UNIX, EPOCH, OBSERVED_UNIX)
    }

    fn feature_entries() -> [AccountSetEntryV1; 2] {
        [
            AccountSetEntryV1 {
                key: FEATURE_ACCOUNT_KEY,
                expected_owner: FEATURE_PROGRAM_ID_V1,
                inline_len: RelayedObservableV1::FeatureGateActivationV1.state_inline_bytes(),
            },
            clock_entry(),
        ]
    }

    fn mean_slot_time_entries(state_key: [u8; ADDRESS_BYTES]) -> [AccountSetEntryV1; 2] {
        [
            AccountSetEntryV1 {
                key: state_key,
                expected_owner: OBSERVED_SYSVAR_OWNER_V1,
                inline_len: RelayedObservableV1::MeanSlotTimeSinceEpochStartV1.state_inline_bytes(),
            },
            clock_entry(),
        ]
    }

    fn loader_v3_program_body() -> [u8; LOADER_V3_PROGRAM_BYTES] {
        let mut data = [0u8; LOADER_V3_PROGRAM_BYTES];
        data[..4].copy_from_slice(&2u32.to_le_bytes());
        data[4..].copy_from_slice(&VENUE_PROGRAMDATA_KEY);
        data
    }

    /// The 45-byte `ProgramData` metadata prefix: variant tag, deployment slot,
    /// and the `Immutable` upgrade-authority tag at offset 12.
    fn loader_v3_programdata_metadata(
        deployment_slot: u64,
    ) -> [u8; LOADER_V3_PROGRAMDATA_METADATA_BYTES] {
        let mut data = [0u8; LOADER_V3_PROGRAMDATA_METADATA_BYTES];
        data[..4].copy_from_slice(&3u32.to_le_bytes());
        data[4..12].copy_from_slice(&deployment_slot.to_le_bytes());
        data[12] = 0;
        data
    }

    fn program_observation(body: &[u8]) -> AccountObservationV1<'_> {
        AccountObservationV1::new(
            VENUE_PROGRAM_KEY,
            LOADER_V3_PROGRAM_ID,
            1,
            u32::try_from(LOADER_V3_PROGRAM_BYTES).expect("fits"),
            body,
            true,
            crate::relay::SHA256_EMPTY_DIGEST,
        )
        .expect("loader v3 program body")
    }

    fn programdata_observation(body: &[u8]) -> AccountObservationV1<'_> {
        AccountObservationV1::new(
            VENUE_PROGRAMDATA_KEY,
            LOADER_V3_PROGRAM_ID,
            1,
            2_326_622,
            body,
            false,
            [0xee; 32],
        )
        .expect("loader v3 programdata body")
    }

    fn dbc_entries() -> [AccountSetEntryV1; 4] {
        [
            AccountSetEntryV1 {
                key: VENUE_PROGRAM_KEY,
                expected_owner: LOADER_V3_PROGRAM_ID,
                inline_len: u16::try_from(LOADER_V3_PROGRAM_BYTES).expect("fits"),
            },
            AccountSetEntryV1 {
                key: VENUE_PROGRAMDATA_KEY,
                expected_owner: LOADER_V3_PROGRAM_ID,
                inline_len: u16::try_from(LOADER_V3_PROGRAMDATA_METADATA_BYTES).expect("fits"),
            },
            AccountSetEntryV1 {
                key: VENUE_POOL_KEY,
                expected_owner: VENUE_PROGRAM_KEY,
                inline_len: RelayedObservableV1::DbcMigrationProgressV1.state_inline_bytes(),
            },
            clock_entry(),
        ]
    }

    fn pinned_release() -> ArtifactReleaseV1 {
        ArtifactReleaseV1::new(
            ProgramIdentityV1::new(VENUE_PROGRAM_KEY).expect("program"),
            ProgramIdentityV1::new(LOADER_V3_PROGRAM_ID).expect("loader"),
            VENUE_PROGRAMDATA_KEY,
            ContentId::new([0x77; 32]).expect("semantic release"),
            [0xee; 32],
            423_941_138,
            ArtifactUpgradePolicyV1::Immutable,
            None,
        )
        .expect("artifact release")
    }

    /// The positive control the interpretation never had.
    ///
    /// Both `Native` rows — the two parents of the flagship conditional market
    /// — driven through `interpret_sealed_record_v1` end to end, on a record
    /// that was really created, appended to and sealed. Every other test in
    /// this module calls one `read_*` helper directly, so without this a suite
    /// of refusals could not tell a guard firing from an instrument that was
    /// never connected.
    #[test]
    fn a_native_row_interprets_with_no_venue_release_and_reports_no_deployment() {
        let clock = observed_clock();

        // The Feature row, activated: the atom is the activation slot itself.
        let activated = feature_body(FEATURE_SOME_TAG_V1, 344_000_000);
        let bytes: [u8; NATIVE_RECORD_BYTES] =
            sealed_record(&[feature_observation(&activated), clock_observation(&clock)]);
        let outcome = interpret_sealed_record_v1(
            RelayedObservationRecordViewV1::decode(&bytes).expect("decodes"),
            adapter_config(RelayedObservableV1::FeatureGateActivationV1),
            &feature_entries(),
            ACCOUNT_SET,
            None,
            SOLANA_MAINNET_GENESIS_HASH_V1,
            CURRENT_UNIX,
        )
        .expect("a native row interprets with no pinned venue release");
        assert_eq!(
            outcome.observable(),
            RelayedObservableV1::FeatureGateActivationV1
        );
        assert_eq!(outcome.atoms(), i128::from(344_000_000u64));
        assert_eq!(outcome.observed_slot(), SLOT);
        assert_eq!(outcome.observed_unix_seconds(), OBSERVED_UNIX);
        assert_eq!(outcome.venue_deployment(), None);

        // ...and dormant: the sentinel above every slot a `u64` can name, which
        // the Product's own cut at `S + 1` is what separates from a slot.
        let dormant = feature_body(FEATURE_NONE_TAG_V1, 0);
        let bytes: [u8; NATIVE_RECORD_BYTES] =
            sealed_record(&[feature_observation(&dormant), clock_observation(&clock)]);
        let outcome = interpret_sealed_record_v1(
            RelayedObservationRecordViewV1::decode(&bytes).expect("decodes"),
            adapter_config(RelayedObservableV1::FeatureGateActivationV1),
            &feature_entries(),
            ACCOUNT_SET,
            None,
            SOLANA_MAINNET_GENESIS_HASH_V1,
            CURRENT_UNIX,
        )
        .expect("a dormant feature is observed, not refused");
        assert_eq!(
            outcome.atoms(),
            i128::from(FEATURE_NOT_ACTIVATED_SENTINEL_V1)
        );
        assert_eq!(outcome.observed_slot(), SLOT);
        assert_eq!(outcome.observed_unix_seconds(), OBSERVED_UNIX);
        assert_eq!(outcome.venue_deployment(), None);

        // The mean-slot-time row, on the Lean's worked witness: 432,000 slots
        // per epoch, 100,000 slots and 40,000 seconds into epoch 800 — 400 ms,
        // now reached through the orchestration rather than the grammar alone.
        let schedule = schedule_body(432_000, 0, 0, 0);
        let bytes: [u8; NATIVE_RECORD_BYTES] =
            sealed_record(&[schedule_observation(&schedule), clock_observation(&clock)]);
        let outcome = interpret_sealed_record_v1(
            RelayedObservationRecordViewV1::decode(&bytes).expect("decodes"),
            adapter_config(RelayedObservableV1::MeanSlotTimeSinceEpochStartV1),
            &mean_slot_time_entries(OBSERVED_EPOCH_SCHEDULE_SYSVAR_KEY_V1),
            ACCOUNT_SET,
            None,
            SOLANA_MAINNET_GENESIS_HASH_V1,
            CURRENT_UNIX,
        )
        .expect("the sysvar pair interprets with no pinned venue release");
        assert_eq!(
            outcome.observable(),
            RelayedObservableV1::MeanSlotTimeSinceEpochStartV1
        );
        assert_eq!(outcome.atoms(), 400);
        assert_eq!(outcome.observed_slot(), SLOT);
        assert_eq!(outcome.observed_unix_seconds(), OBSERVED_UNIX);
        assert_eq!(outcome.venue_deployment(), None);
    }

    /// A native row has no upgradeable deployment to pin, and the argument was
    /// once unconditional and ignored on this arm — which made a caller's
    /// belief that it had pinned one unfalsifiable. Both native rows refuse a
    /// release rather than resolving without it.
    #[test]
    fn a_native_row_carrying_a_venue_release_refuses_rather_than_ignoring_it() {
        let clock = observed_clock();

        let activated = feature_body(FEATURE_SOME_TAG_V1, 344_000_000);
        let bytes: [u8; NATIVE_RECORD_BYTES] =
            sealed_record(&[feature_observation(&activated), clock_observation(&clock)]);
        assert_eq!(
            interpret_sealed_record_v1(
                RelayedObservationRecordViewV1::decode(&bytes).expect("decodes"),
                adapter_config(RelayedObservableV1::FeatureGateActivationV1),
                &feature_entries(),
                ACCOUNT_SET,
                Some(pinned_release()),
                SOLANA_MAINNET_GENESIS_HASH_V1,
                CURRENT_UNIX,
            ),
            Err(Error::VenueReleaseNotPinnable)
        );

        let schedule = schedule_body(432_000, 0, 0, 0);
        let bytes: [u8; NATIVE_RECORD_BYTES] =
            sealed_record(&[schedule_observation(&schedule), clock_observation(&clock)]);
        assert_eq!(
            interpret_sealed_record_v1(
                RelayedObservationRecordViewV1::decode(&bytes).expect("decodes"),
                adapter_config(RelayedObservableV1::MeanSlotTimeSinceEpochStartV1),
                &mean_slot_time_entries(OBSERVED_EPOCH_SCHEDULE_SYSVAR_KEY_V1),
                ACCOUNT_SET,
                Some(pinned_release()),
                SOLANA_MAINNET_GENESIS_HASH_V1,
                CURRENT_UNIX,
            ),
            Err(Error::VenueReleaseNotPinnable)
        );
    }

    /// The other direction: a `LoaderV3` row cannot authenticate its venue
    /// cross-cluster without the pinned release, and its absence is a refusal
    /// on its own name rather than a fall-through to the native path.
    ///
    /// The whole four-position set is built, because the program and
    /// `ProgramData` bodies are read against their pins before the release is
    /// looked at — so this refusal is reached past every earlier guard.
    #[test]
    fn a_loader_v3_row_with_no_venue_release_refuses_by_name() {
        let program = loader_v3_program_body();
        let metadata = loader_v3_programdata_metadata(423_941_138);
        let pool = venue_body(MIGRATION_PROGRESS_CREATED_POOL_V1, 1, 1_756_000_500);
        let clock = observed_clock();
        let bytes: [u8; LOADER_V3_RECORD_BYTES] = sealed_record(&[
            program_observation(&program),
            programdata_observation(&metadata),
            observation(&pool),
            clock_observation(&clock),
        ]);
        assert_eq!(
            interpret_sealed_record_v1(
                RelayedObservationRecordViewV1::decode(&bytes).expect("decodes"),
                adapter_config(RelayedObservableV1::DbcMigrationProgressV1),
                &dbc_entries(),
                ACCOUNT_SET,
                None,
                SOLANA_MAINNET_GENESIS_HASH_V1,
                CURRENT_UNIX,
            ),
            Err(Error::VenueReleaseAbsent)
        );
    }

    /// Every sysvar reports the same owner, so the owner pin alone does not
    /// identify the `EpochSchedule`: a different sysvar-owned account at the
    /// state position would otherwise reach the grammar and be read as a
    /// schedule. The row pins the address too, under the same refusal name.
    #[test]
    fn a_native_row_whose_state_is_not_the_pinned_sysvar_refuses_on_its_owner() {
        const IMPOSTOR: [u8; ADDRESS_BYTES] = [0x66; 32];
        assert_ne!(IMPOSTOR, OBSERVED_EPOCH_SCHEDULE_SYSVAR_KEY_V1);
        let schedule = schedule_body(432_000, 0, 0, 0);
        let clock = observed_clock();
        let bytes: [u8; NATIVE_RECORD_BYTES] = sealed_record(&[
            schedule_observation_at(IMPOSTOR, &schedule),
            clock_observation(&clock),
        ]);
        assert_eq!(
            interpret_sealed_record_v1(
                RelayedObservationRecordViewV1::decode(&bytes).expect("decodes"),
                adapter_config(RelayedObservableV1::MeanSlotTimeSinceEpochStartV1),
                &mean_slot_time_entries(IMPOSTOR),
                ACCOUNT_SET,
                None,
                SOLANA_MAINNET_GENESIS_HASH_V1,
                CURRENT_UNIX,
            ),
            Err(Error::ObservedOwnerMismatch)
        );
    }
}
