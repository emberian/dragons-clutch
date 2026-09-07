//! Exact ordered account frames for the six relay record routes.
//!
//! Counts, privileges and the complete no-alias policy are checked here; the
//! *semantic* identity of each position (that this really is the Market, that
//! this really is the pinned raw record) stays an adapter obligation and is
//! authenticated separately by PDA derivation and content identity.
//!
//! These frames are relay-owned rather than added to `dclutch-source`.
//! The relay routes are their own instruction family with their own magic, they
//! name account classes that no Source route has (a relayer key set, an
//! observation record), and `SourceAccountRoleV1`'s constructor is private to
//! its crate.  The consumption frame is here too, for the same reason and one
//! more: it names the *venue's* pinned artifact release and the pinned account
//! set, neither of which is a Source concept, and it is dispatched behind this
//! family's own magic rather than the Source instruction wire.

use crate::relay::decode::RelayedVenueKindV1;
use crate::relay::{ADDRESS_BYTES, Error, Result};

/// Semantic role name in one ordered relay frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelayAccountNameV1 {
    /// The permissionless worker paying for and signing the transaction.
    Worker,
    /// The owning Core Market state.
    Market,
    /// The Core Program the Market state is owned by and derived under.
    CoreProgram,
    /// The Registry-owned activation cache for the Market's release set.
    RegistryActivation,
    /// The observation record being created, appended to, sealed or retired.
    Record,
    /// The raw immutable `SourceMaterialV3` record.
    SourceMaterial,
    /// The finalized staging vacancy proving the material record is immutable.
    SourceMaterialStagingVacancy,
    /// The raw immutable `SourceSpecV1` record the material names.
    SourceSpec,
    /// The finalized staging vacancy proving the spec record is immutable.
    SourceSpecStagingVacancy,
    /// The raw immutable `ProviderReleaseV1` record the spec names.
    ProviderRelease,
    /// The finalized staging vacancy proving the release record is immutable.
    ProviderReleaseStagingVacancy,
    /// The raw immutable `WindowSpecV1` record the material names.
    WindowSpec,
    /// The finalized staging vacancy proving the window record is immutable.
    WindowSpecStagingVacancy,
    /// The raw immutable `StatisticSpecV1` record the material names.
    StatisticSpec,
    /// The finalized staging vacancy proving the statistic record is immutable.
    StatisticSpecStagingVacancy,
    /// The raw immutable `RelayerKeySetV1` record.
    RelayerKeySet,
    /// The finalized staging vacancy proving the key set is immutable.
    RelayerKeySetStagingVacancy,
    /// The raw immutable `RelayedAdapterConfigV1` record.
    AdapterConfig,
    /// The finalized staging vacancy proving the adapter config is immutable.
    AdapterConfigStagingVacancy,
    /// The Market's own persisted rent beneficiary.
    RentBeneficiary,
    /// The Rent sysvar.
    RentSysvar,
    /// The Clock sysvar.
    ClockSysvar,
    /// The Instructions sysvar, used only to select the preceding precompile.
    InstructionsSysvar,
    /// The System Program.
    SystemProgram,
    /// The Resolution-owned `SourceResolutionStateV2` this consumption resolves.
    SourceResolutionState,
    /// The terminal `ResolutionCertificateV2` this consumption writes.
    ResolutionCertificate,
    /// The raw immutable `ArtifactReleaseV1` pinning the observed venue program.
    VenueArtifactRelease,
    /// The finalized staging vacancy proving the venue release is immutable.
    VenueArtifactReleaseStagingVacancy,
    /// The raw immutable Product Runtime V2 Product record.
    ProductRecord,
    /// The finalized staging vacancy proving the Product record is immutable.
    ProductRecordStagingVacancy,
    /// The raw immutable Product Runtime V2 result domain.
    ResultDomain,
    /// The finalized staging vacancy proving the result domain is immutable.
    ResultDomainStagingVacancy,
    /// The raw immutable Product Runtime V2 portfolio record.
    PortfolioRecord,
    /// The finalized staging vacancy proving the portfolio record is immutable.
    PortfolioRecordStagingVacancy,
    /// The raw immutable `CapabilityManifestV1` the funding compartments quote.
    CapabilityManifest,
    /// The finalized staging vacancy proving the manifest is immutable.
    CapabilityManifestStagingVacancy,
    /// The raw immutable `RecoveryPolicyV2` record the material selects.
    RecoveryPolicy,
    /// The finalized staging vacancy proving the recovery policy is immutable.
    RecoveryPolicyStagingVacancy,
    /// The Resolution-owned three-compartment funding ledger a walk debits.
    ///
    /// It was called `FailureFunding` while the only route that debited it was
    /// the deadline-failure walk, but the account has always held all three of
    /// a market's Resolution compartments -- recovery, exhaustion and failure
    /// -- in one three-row ledger, and the funded ladder debits the other two.
    /// One account class, one name.
    ResolutionFunding,
    /// The `EnsembleFoldReceiptV1` seat the fold writes beside the certificate.
    EnsembleFoldReceipt,
    /// One member's fragment seat: a kind-1 certificate under the fragment
    /// domain, or a System-owned vacancy for a member that did not answer.
    EnsembleFragmentSeat,
    /// The captor a consumed fragment names, paid its member's bounty.
    EnsembleCaptor,
}

/// One ordered SDK-free account-role requirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelayAccountRoleV1 {
    name: RelayAccountNameV1,
    signer: bool,
    writable: bool,
}

impl RelayAccountRoleV1 {
    /// The semantic role name.
    pub const fn name(self) -> RelayAccountNameV1 {
        self.name
    }
    /// Whether the position must be a transaction signer.
    pub const fn is_signer(self) -> bool {
        self.signer
    }
    /// Whether the position must be writable.
    pub const fn is_writable(self) -> bool {
        self.writable
    }
}

const fn role(name: RelayAccountNameV1, signer: bool, writable: bool) -> RelayAccountRoleV1 {
    RelayAccountRoleV1 {
        name,
        signer,
        writable,
    }
}

const WORKER: RelayAccountRoleV1 = role(RelayAccountNameV1::Worker, true, true);
const MARKET_READ: RelayAccountRoleV1 = role(RelayAccountNameV1::Market, false, false);
const CORE_PROGRAM: RelayAccountRoleV1 = role(RelayAccountNameV1::CoreProgram, false, false);
const ACTIVATION: RelayAccountRoleV1 = role(RelayAccountNameV1::RegistryActivation, false, false);
const RECORD: RelayAccountRoleV1 = role(RelayAccountNameV1::Record, false, true);
const MATERIAL: RelayAccountRoleV1 = role(RelayAccountNameV1::SourceMaterial, false, false);
const MATERIAL_STAGE: RelayAccountRoleV1 = role(
    RelayAccountNameV1::SourceMaterialStagingVacancy,
    false,
    false,
);
const SPEC: RelayAccountRoleV1 = role(RelayAccountNameV1::SourceSpec, false, false);
const SPEC_STAGE: RelayAccountRoleV1 =
    role(RelayAccountNameV1::SourceSpecStagingVacancy, false, false);
const PROVIDER: RelayAccountRoleV1 = role(RelayAccountNameV1::ProviderRelease, false, false);
const PROVIDER_STAGE: RelayAccountRoleV1 = role(
    RelayAccountNameV1::ProviderReleaseStagingVacancy,
    false,
    false,
);
const WINDOW: RelayAccountRoleV1 = role(RelayAccountNameV1::WindowSpec, false, false);
const WINDOW_STAGE: RelayAccountRoleV1 =
    role(RelayAccountNameV1::WindowSpecStagingVacancy, false, false);
const STATISTIC: RelayAccountRoleV1 = role(RelayAccountNameV1::StatisticSpec, false, false);
const STATISTIC_STAGE: RelayAccountRoleV1 = role(
    RelayAccountNameV1::StatisticSpecStagingVacancy,
    false,
    false,
);
const KEY_SET: RelayAccountRoleV1 = role(RelayAccountNameV1::RelayerKeySet, false, false);
const KEY_SET_STAGE: RelayAccountRoleV1 = role(
    RelayAccountNameV1::RelayerKeySetStagingVacancy,
    false,
    false,
);
const CONFIG: RelayAccountRoleV1 = role(RelayAccountNameV1::AdapterConfig, false, false);
const CONFIG_STAGE: RelayAccountRoleV1 = role(
    RelayAccountNameV1::AdapterConfigStagingVacancy,
    false,
    false,
);
const BENEFICIARY_READ: RelayAccountRoleV1 =
    role(RelayAccountNameV1::RentBeneficiary, false, false);
const BENEFICIARY_WRITE: RelayAccountRoleV1 =
    role(RelayAccountNameV1::RentBeneficiary, false, true);
const RENT: RelayAccountRoleV1 = role(RelayAccountNameV1::RentSysvar, false, false);
const CLOCK: RelayAccountRoleV1 = role(RelayAccountNameV1::ClockSysvar, false, false);
const INSTRUCTIONS: RelayAccountRoleV1 = role(RelayAccountNameV1::InstructionsSysvar, false, false);
const SYSTEM: RelayAccountRoleV1 = role(RelayAccountNameV1::SystemProgram, false, false);
const RECORD_CONSUME: RelayAccountRoleV1 = role(RelayAccountNameV1::Record, false, true);
const SOURCE_STATE: RelayAccountRoleV1 =
    role(RelayAccountNameV1::SourceResolutionState, false, true);
const CERTIFICATE: RelayAccountRoleV1 =
    role(RelayAccountNameV1::ResolutionCertificate, false, true);
const VENUE_RELEASE: RelayAccountRoleV1 =
    role(RelayAccountNameV1::VenueArtifactRelease, false, false);
const VENUE_RELEASE_STAGE: RelayAccountRoleV1 = role(
    RelayAccountNameV1::VenueArtifactReleaseStagingVacancy,
    false,
    false,
);
const PRODUCT: RelayAccountRoleV1 = role(RelayAccountNameV1::ProductRecord, false, false);
const PRODUCT_STAGE: RelayAccountRoleV1 = role(
    RelayAccountNameV1::ProductRecordStagingVacancy,
    false,
    false,
);
const RESULT_DOMAIN: RelayAccountRoleV1 = role(RelayAccountNameV1::ResultDomain, false, false);
const RESULT_DOMAIN_STAGE: RelayAccountRoleV1 =
    role(RelayAccountNameV1::ResultDomainStagingVacancy, false, false);
const PORTFOLIO: RelayAccountRoleV1 = role(RelayAccountNameV1::PortfolioRecord, false, false);
const PORTFOLIO_STAGE: RelayAccountRoleV1 = role(
    RelayAccountNameV1::PortfolioRecordStagingVacancy,
    false,
    false,
);
const MANIFEST: RelayAccountRoleV1 = role(RelayAccountNameV1::CapabilityManifest, false, false);
const MANIFEST_STAGE: RelayAccountRoleV1 = role(
    RelayAccountNameV1::CapabilityManifestStagingVacancy,
    false,
    false,
);
const RECOVERY_POLICY: RelayAccountRoleV1 = role(RelayAccountNameV1::RecoveryPolicy, false, false);
const RECOVERY_POLICY_STAGE: RelayAccountRoleV1 = role(
    RelayAccountNameV1::RecoveryPolicyStagingVacancy,
    false,
    false,
);
const SOURCE_STATE_READ: RelayAccountRoleV1 =
    role(RelayAccountNameV1::SourceResolutionState, false, false);
const FOLD_RECEIPT: RelayAccountRoleV1 = role(RelayAccountNameV1::EnsembleFoldReceipt, false, true);
/// A member seat the fold READS; the fragment route wrote it.
const FRAGMENT_SEAT_READ: RelayAccountRoleV1 =
    role(RelayAccountNameV1::EnsembleFragmentSeat, false, false);
/// A member seat the reclaim CLOSES.
const FRAGMENT_SEAT_WRITE: RelayAccountRoleV1 =
    role(RelayAccountNameV1::EnsembleFragmentSeat, false, true);
const CAPTOR: RelayAccountRoleV1 = role(RelayAccountNameV1::EnsembleCaptor, false, true);
const RESOLUTION_FUNDING: RelayAccountRoleV1 =
    role(RelayAccountNameV1::ResolutionFunding, false, true);

/// Exact record-creation frame.
///
/// Every raw record rides with the finalized staging vacancy that proves it is
/// immutable, which is the discipline every successor route already enforces
/// for the Source material.
///
/// The Market is **read-only**. The successor `CoreState` is Core-owned, the
/// record is not one of its children, and Resolution holds no write authority
/// over it; the join this frame makes is a read.  `SourceSpecV1`,
/// `ProviderReleaseV1` and `WindowSpecV1` each take their own slot because the
/// compact V2 material names them by content identity rather than carrying them
/// inline the way the retired V1 material did.
pub const CREATE_RECORD_FRAME_V1: [RelayAccountRoleV1; 21] = [
    WORKER,
    MARKET_READ,
    CORE_PROGRAM,
    ACTIVATION,
    RECORD,
    MATERIAL,
    MATERIAL_STAGE,
    SPEC,
    SPEC_STAGE,
    PROVIDER,
    PROVIDER_STAGE,
    WINDOW,
    WINDOW_STAGE,
    KEY_SET,
    KEY_SET_STAGE,
    CONFIG,
    CONFIG_STAGE,
    BENEFICIARY_READ,
    RENT,
    CLOCK,
    SYSTEM,
];

/// Exact append frame; the Ed25519 precompile rides immediately before it.
///
/// The adapter configuration is deliberately absent.  An append needs the
/// account set (which the record already persists) and the key set (to place
/// the signer); the staleness join needs the *attested mainnet* clock, which is
/// a decoded field of an attested account and is therefore a resolution-time
/// question, not a fill-time one.
pub const APPEND_OBSERVATION_FRAME_V1: [RelayAccountRoleV1; 8] = [
    WORKER,
    MARKET_READ,
    RECORD,
    KEY_SET,
    KEY_SET_STAGE,
    RENT,
    INSTRUCTIONS,
    CLOCK,
];

/// Exact seal frame; one signer per transaction.
pub const SEAL_RECORD_FRAME_V1: [RelayAccountRoleV1; 8] = [
    WORKER,
    MARKET_READ,
    RECORD,
    KEY_SET,
    KEY_SET_STAGE,
    RENT,
    INSTRUCTIONS,
    CLOCK,
];

/// Exact retirement and rent-return frame.
///
/// The Market is present and read-only: the record's own persisted binding is
/// what says which Market it belongs to, and this position is where that claim
/// is checked against a real account rather than taken on the record's word.
pub const RETIRE_RECORD_FRAME_V1: [RelayAccountRoleV1; 4] =
    [WORKER, MARKET_READ, RECORD, BENEFICIARY_WRITE];

/// Exact consumption frame: one sealed record into one terminal result.
///
/// Read the writability column as the authority statement it is.  Exactly three
/// positions are writable — the record whose phase advances to `Consumed`, the
/// Source state that becomes terminal, and the certificate this route creates —
/// and everything else, the Market included, is read.  The relayer key set is
/// deliberately absent: signatures were authenticated when the record was filled
/// and sealed, and after sealing the record's program ownership and PDA
/// derivation are the authority, so re-presenting the key set here would be a
/// second place to get the same question wrong.
///
/// The venue's `ArtifactReleaseV1` is the position that carries P-B.  It is
/// named by the Source spec, so which third-party deployment a market is pinned
/// to is a founding-time content identity rather than a caller's choice.
///
/// # The statistic, and why it was not here
///
/// This frame had no `StatisticSpecV1` position because the family's own
/// reasoning said it needed none: a terminal window over a terminal sample has
/// one observation and one atom, so the statistic was the identity map and the
/// route compared the Source's own unit against the Product's result unit
/// directly.  That reasoning survived exactly until `4cd2b9cb5` gave the
/// statistic a `source_scale_exponent`, which made it the only record entitled
/// to say how those two units relate -- and a record not in the frame is a
/// record no route can read.  A market whose statistic declared a conversion
/// was consumed here at the identity, which is cohort-14 market B's failure
/// with a different provider family in front of it.
///
/// So the statistic rides with its staging vacancy like every other raw record,
/// and both are read-only.  Two positions is what closing it cost.
pub const CONSUME_RECORD_FRAME_V1: [RelayAccountRoleV1; 30] = [
    WORKER,
    MARKET_READ,
    CORE_PROGRAM,
    ACTIVATION,
    RECORD_CONSUME,
    SOURCE_STATE,
    CERTIFICATE,
    MATERIAL,
    MATERIAL_STAGE,
    SPEC,
    SPEC_STAGE,
    PROVIDER,
    PROVIDER_STAGE,
    WINDOW,
    WINDOW_STAGE,
    STATISTIC,
    STATISTIC_STAGE,
    CONFIG,
    CONFIG_STAGE,
    VENUE_RELEASE,
    VENUE_RELEASE_STAGE,
    PRODUCT,
    PRODUCT_STAGE,
    RESULT_DOMAIN,
    RESULT_DOMAIN_STAGE,
    PORTFOLIO,
    PORTFOLIO_STAGE,
    CLOCK,
    RENT,
    SYSTEM,
];

/// Where the venue's `ArtifactReleaseV1` sits in [`CONSUME_RECORD_FRAME_V1`].
pub const CONSUME_VENUE_RELEASE_INDEX_V1: usize = 19;

/// How many positions the venue-release pair occupies.
///
/// The two consumption frames differ by exactly this pair and in no other way,
/// which is what lets one index table serve both:
/// [`consume_position_v1`] subtracts it from every position after the pair.
/// `the_native_consume_frame_is_the_canonical_one_without_its_venue_release`
/// is the assertion, and it walks both tables rather than trusting this
/// sentence.
pub const CONSUME_VENUE_RELEASE_SLOTS_V1: usize = 2;

/// Exact consumption frame for a [`RelayedVenueKindV1::Native`] row.
///
/// [`CONSUME_RECORD_FRAME_V1`] without its venue-release pair, and nothing
/// else moved.  A row whose state account is owned by a program the validator
/// itself implements — the Feature program, the sysvar owner — has no
/// upgradeable venue program: no `ProgramData`, no ELF digest, no upgrade
/// authority, and therefore no `ArtifactReleaseV1` that could pin a
/// deployment.  Demanding one anyway is not a stricter frame, it is a frame no
/// honest caller can fill, which is why the two new observables had a decoder
/// and no way onto a chain until this existed.
///
/// What replaces P-B for these rows is stated in
/// [`RelayedVenueKindV1::Native`]'s own documentation: the pinned account-set
/// entry's `expected_owner`, which the record commits to by digest and which
/// no transaction on the observed cluster can move.
///
/// [`RelayedVenueKindV1::Native`]: crate::relay::decode::RelayedVenueKindV1::Native
pub const CONSUME_RECORD_NATIVE_VENUE_FRAME_V1: [RelayAccountRoleV1; 28] = [
    WORKER,
    MARKET_READ,
    CORE_PROGRAM,
    ACTIVATION,
    RECORD_CONSUME,
    SOURCE_STATE,
    CERTIFICATE,
    MATERIAL,
    MATERIAL_STAGE,
    SPEC,
    SPEC_STAGE,
    PROVIDER,
    PROVIDER_STAGE,
    WINDOW,
    WINDOW_STAGE,
    STATISTIC,
    STATISTIC_STAGE,
    CONFIG,
    CONFIG_STAGE,
    PRODUCT,
    PRODUCT_STAGE,
    RESULT_DOMAIN,
    RESULT_DOMAIN_STAGE,
    PORTFOLIO,
    PORTFOLIO_STAGE,
    CLOCK,
    RENT,
    SYSTEM,
];

/// Which consumption frame one venue kind fills.
pub const fn consume_frame_kind_v1(venue: RelayedVenueKindV1) -> RelayFrameKindV1 {
    match venue {
        RelayedVenueKindV1::LoaderV3 => RelayFrameKindV1::ConsumeRecord,
        RelayedVenueKindV1::Native => RelayFrameKindV1::ConsumeRecordNativeVenue,
    }
}

/// One [`CONSUME_RECORD_FRAME_V1`] position, in the frame the venue kind fills.
///
/// The canonical frame is the coordinate system: a route names the position it
/// wants once, in the 30-slot table everyone already reads, and this moves it
/// into the 28-slot table when the venue-release pair is absent.  A position
/// before the pair is unmoved; the pair itself has no coordinate in the native
/// frame and yields `None`.
pub const fn consume_position_v1(venue: RelayedVenueKindV1, canonical: usize) -> Option<usize> {
    match venue {
        RelayedVenueKindV1::LoaderV3 => Some(canonical),
        RelayedVenueKindV1::Native => {
            if canonical < CONSUME_VENUE_RELEASE_INDEX_V1 {
                Some(canonical)
            } else if canonical < CONSUME_VENUE_RELEASE_INDEX_V1 + CONSUME_VENUE_RELEASE_SLOTS_V1 {
                None
            } else {
                Some(canonical - CONSUME_VENUE_RELEASE_SLOTS_V1)
            }
        }
    }
}

/// Exact deadline-failure frame: a silent market to its pre-disclosed outcome.
///
/// Twenty-two positions, and the interesting fact about them is what is missing.
/// There is no record, no provider release, no adapter configuration, no key set
/// and no venue: the failure outcome is the Product's own and the deadline is
/// the window's own, so nothing about the provider is an input.  That is not
/// economy, it is the property -- a route that needed the relayer to supply
/// anything could not run when the relayer has stopped answering, and the whole
/// point of this walk is that it runs anyway.
///
/// Three positions carry the *prepayment* half of the same property, and they
/// are the reason this frame is not sixteen.  `ResolutionCertificateV2` refuses
/// a `ResolutionFailure` whose `funding_allocation` or `work_paid` is zero, so
/// "permissionless" without "paid" is not a shape this protocol can encode: the
/// capability manifest quotes the bounty, its staging vacancy proves the quote
/// is immutable, and the explicit-failure compartment is the escrow the walk
/// actually debits.  None of the three is relayer-supplied -- the market
/// prepaid them at founding -- so the walk still runs when nobody is answering.
///
/// Exactly three positions are writable: the Source state that becomes terminal,
/// the certificate this route creates, and the escrow it spends.  The worker is
/// writable because it is *paid*, which is the only lamport flow here that is
/// not rent.
pub const COMMIT_DEADLINE_FAILURE_FRAME_V1: [RelayAccountRoleV1; 22] = [
    WORKER,
    MARKET_READ,
    CORE_PROGRAM,
    ACTIVATION,
    SOURCE_STATE,
    CERTIFICATE,
    MATERIAL,
    MATERIAL_STAGE,
    WINDOW,
    WINDOW_STAGE,
    PRODUCT,
    PRODUCT_STAGE,
    RESULT_DOMAIN,
    RESULT_DOMAIN_STAGE,
    PORTFOLIO,
    PORTFOLIO_STAGE,
    MANIFEST,
    MANIFEST_STAGE,
    RESOLUTION_FUNDING,
    CLOCK,
    RENT,
    SYSTEM,
];

/// The fixed prefix of the ensemble fold's frame; `k` member seats and then
/// `k` captors follow it, one pair per declared member in member order
/// (`ensemble_fold_tail_v1`), because the width is the material's `k` and a
/// frame table cannot state it.
///
/// The failure walk's twenty-two, plus the receipt seat, the policy pair (the
/// members are its leading slots), the primary `SourceSpecV1` pair (its
/// provider release is member zero's route, and the fragment in member zero's
/// seat is admitted against it) and the `StatisticSpecV1` pair (the
/// source-to-result shift the median reaches the selector on). The certificate
/// is the market's own terminal seat, untouched by any fragment.
pub const ENSEMBLE_FOLD_FRAME_PREFIX_V1: [RelayAccountRoleV1; 29] = [
    WORKER,
    MARKET_READ,
    CORE_PROGRAM,
    ACTIVATION,
    SOURCE_STATE,
    CERTIFICATE,
    FOLD_RECEIPT,
    MATERIAL,
    MATERIAL_STAGE,
    SPEC,
    SPEC_STAGE,
    WINDOW,
    WINDOW_STAGE,
    STATISTIC,
    STATISTIC_STAGE,
    RECOVERY_POLICY,
    RECOVERY_POLICY_STAGE,
    PRODUCT,
    PRODUCT_STAGE,
    RESULT_DOMAIN,
    RESULT_DOMAIN_STAGE,
    PORTFOLIO,
    PORTFOLIO_STAGE,
    MANIFEST,
    MANIFEST_STAGE,
    RESOLUTION_FUNDING,
    CLOCK,
    RENT,
    SYSTEM,
];
/// Exact reclaim frame: the worker, the Market and the two accounts that pin
/// it as a Core Market of this Program's Resolution role, its terminal Source
/// state, the material pair whose `k` says which member bytes name a seat at
/// all, the vacant seat, the Source's own rent beneficiary, and the System
/// program.
///
/// The material is in frame for one conjunct: a member byte at or above the
/// ensemble's `k` names no seat, and the route refuses on it before deriving
/// an address, so a caller cannot use this route to learn where a seat it has
/// no member for would live.
pub const RECLAIM_MEMBER_SEAT_FRAME_V1: [RelayAccountRoleV1; 10] = [
    WORKER,
    MARKET_READ,
    CORE_PROGRAM,
    ACTIVATION,
    SOURCE_STATE_READ,
    MATERIAL,
    MATERIAL_STAGE,
    FRAGMENT_SEAT_WRITE,
    BENEFICIARY_WRITE,
    SYSTEM,
];
/// Exact funded ordered-recovery frame: one crank of the ladder.
///
/// Eighteen positions, and it is shorter than the failure frame by the whole
/// Product graph.  That is the shape of the transition, not an economy: a crank
/// selects no result, so `ResolutionCertificateV2::validate_terminal_product`
/// REFUSES to be asked about a `RecoveryAdvanced` or `Exhausted` certificate at
/// all, and the only Product fact the receipt carries is the record digest the
/// market's own `SourceMaterialV3` already names.  Naming the Product record,
/// the result domain and the portfolio here would be three accounts read to
/// re-derive a value one authenticated record hands over.
///
/// Two positions the failure frame does not have: the `RecoveryPolicyV2` record
/// and its staging vacancy.  That record is the ladder -- the ordered attempts,
/// their windows and their funding allocations -- and it is exactly why the
/// route exists, so it rides as a raw/vacancy pair like every other finalized
/// record this family reads.
///
/// Three positions are writable: the Source state that advances, the receipt
/// this crank creates, and the ledger it spends.  The worker is writable
/// because it is *paid* -- the crank is permissionless, and a permissionless
/// route nobody is paid to run is a route nobody runs.
pub const ADVANCE_RECOVERY_FRAME_V1: [RelayAccountRoleV1; 18] = [
    WORKER,
    MARKET_READ,
    CORE_PROGRAM,
    ACTIVATION,
    SOURCE_STATE,
    CERTIFICATE,
    MATERIAL,
    MATERIAL_STAGE,
    WINDOW,
    WINDOW_STAGE,
    RECOVERY_POLICY,
    RECOVERY_POLICY_STAGE,
    MANIFEST,
    MANIFEST_STAGE,
    RESOLUTION_FUNDING,
    CLOCK,
    RENT,
    SYSTEM,
];

/// Closed exact account-frame selector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelayFrameKindV1 {
    /// [`CREATE_RECORD_FRAME_V1`].
    CreateRecord,
    /// [`APPEND_OBSERVATION_FRAME_V1`].
    AppendObservation,
    /// [`SEAL_RECORD_FRAME_V1`].
    SealRecord,
    /// [`RETIRE_RECORD_FRAME_V1`].
    RetireRecord,
    /// [`CONSUME_RECORD_FRAME_V1`].
    ConsumeRecord,
    /// [`CONSUME_RECORD_NATIVE_VENUE_FRAME_V1`].
    ConsumeRecordNativeVenue,
    /// [`COMMIT_DEADLINE_FAILURE_FRAME_V1`].
    CommitDeadlineFailure,
    /// [`ADVANCE_RECOVERY_FRAME_V1`].
    AdvanceRecovery,
    /// [`ENSEMBLE_FOLD_FRAME_PREFIX_V1`], then [`ensemble_fold_tail_v1`].
    EnsembleFold,
    /// [`RECLAIM_MEMBER_SEAT_FRAME_V1`].
    ReclaimMemberSeat,
}

/// Return the exact ordered roles for one relay operation.
pub const fn relay_frame_roles_v1(kind: RelayFrameKindV1) -> &'static [RelayAccountRoleV1] {
    match kind {
        RelayFrameKindV1::CreateRecord => &CREATE_RECORD_FRAME_V1,
        RelayFrameKindV1::AppendObservation => &APPEND_OBSERVATION_FRAME_V1,
        RelayFrameKindV1::SealRecord => &SEAL_RECORD_FRAME_V1,
        RelayFrameKindV1::RetireRecord => &RETIRE_RECORD_FRAME_V1,
        RelayFrameKindV1::ConsumeRecord => &CONSUME_RECORD_FRAME_V1,
        RelayFrameKindV1::ConsumeRecordNativeVenue => &CONSUME_RECORD_NATIVE_VENUE_FRAME_V1,
        RelayFrameKindV1::CommitDeadlineFailure => &COMMIT_DEADLINE_FAILURE_FRAME_V1,
        RelayFrameKindV1::AdvanceRecovery => &ADVANCE_RECOVERY_FRAME_V1,
        RelayFrameKindV1::EnsembleFold => &ENSEMBLE_FOLD_FRAME_PREFIX_V1,
        RelayFrameKindV1::ReclaimMemberSeat => &RECLAIM_MEMBER_SEAT_FRAME_V1,
    }
}

/// The role of position `index` in an ensemble fold's tail of `2k` accounts:
/// `k` read-only member seats in member order, then `k` writable captors in
/// the same order. A captor position for a member that did not answer is
/// still in the frame (the founding's recorded payer, or any key -- nothing is
/// paid to it), so the frame's width is a function of `k` alone.
pub const fn ensemble_fold_tail_v1(members: u8, index: usize) -> Option<RelayAccountRoleV1> {
    let members = members as usize;
    if index < members {
        Some(FRAGMENT_SEAT_READ)
    } else if index < 2 * members {
        Some(CAPTOR)
    } else {
        None
    }
}

/// The role of position `index` in the `k` read-only member seats a crank or
/// a failure walk carries after its fixed frame when the material declares an
/// ensemble: the seats are counted so the crank/fold exclusivity is a fact of
/// the frame rather than a caller's word.
pub const fn ensemble_seat_tail_v1(members: u8, index: usize) -> Option<RelayAccountRoleV1> {
    if index < members as usize {
        Some(FRAGMENT_SEAT_READ)
    } else {
        None
    }
}

/// Validate a fixed prefix followed by a tail whose roles a function names.
///
/// The prefix is exact; the tail's length is `tail_len` and every position's
/// signer and writable flags are the ones `tail_role` states; and the no-alias
/// rule spans the whole frame, so a seat cannot be passed twice to answer
/// twice, and a captor cannot be the funding ledger.
pub fn validate_relay_frame_with_tail_v1(
    kind: RelayFrameKindV1,
    accounts: &[RelayAccountPrivilegeV1],
    tail_len: usize,
    tail_role: impl Fn(usize) -> Option<RelayAccountRoleV1>,
) -> Result<()> {
    let prefix = relay_frame_roles_v1(kind);
    let expected = prefix
        .len()
        .checked_add(tail_len)
        .ok_or(Error::ArithmeticOverflow)?;
    if accounts.len() != expected {
        return Err(Error::InvalidAccountFrame);
    }
    for (index, account) in accounts.iter().enumerate() {
        let role = match prefix.get(index) {
            Some(role) => *role,
            None => {
                tail_role(index.saturating_sub(prefix.len())).ok_or(Error::InvalidAccountFrame)?
            }
        };
        if account.is_signer != role.is_signer() || account.is_writable != role.is_writable() {
            return Err(Error::InvalidAccountFrame);
        }
    }
    for (index, account) in accounts.iter().enumerate() {
        for other in accounts.iter().skip(index.saturating_add(1)) {
            if account.key == other.key {
                return Err(Error::InvalidAccountFrame);
            }
        }
    }
    Ok(())
}

/// SDK-free observed account key and privileges.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelayAccountPrivilegeV1 {
    /// The observed account address.
    pub key: [u8; ADDRESS_BYTES],
    /// Whether the runtime reports it as a signer.
    pub is_signer: bool,
    /// Whether the runtime reports it as writable.
    pub is_writable: bool,
}

/// Validate exact count, privileges and complete no-alias policy for one frame.
///
/// The no-alias rule is not decoration: without it a caller could pass the
/// record account in the beneficiary position and close a live record's
/// lamports into itself, or pass the key-set record where the adapter config is
/// expected and have both content-ID checks read the same bytes.
pub fn validate_relay_frame_v1(
    kind: RelayFrameKindV1,
    accounts: &[RelayAccountPrivilegeV1],
) -> Result<()> {
    let roles = relay_frame_roles_v1(kind);
    if accounts.len() != roles.len() {
        return Err(Error::InvalidAccountFrame);
    }
    for (account, expected) in accounts.iter().zip(roles.iter()) {
        if account.is_signer != expected.is_signer()
            || account.is_writable != expected.is_writable()
        {
            return Err(Error::InvalidAccountFrame);
        }
    }
    for (index, account) in accounts.iter().enumerate() {
        for other in accounts.iter().skip(index.saturating_add(1)) {
            if account.key == other.key {
                return Err(Error::InvalidAccountFrame);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(kind: RelayFrameKindV1) -> [RelayAccountPrivilegeV1; 30] {
        let roles = relay_frame_roles_v1(kind);
        let mut built = [RelayAccountPrivilegeV1 {
            key: [0; 32],
            is_signer: false,
            is_writable: false,
        }; 30];
        for (index, expected) in roles.iter().enumerate() {
            let slot = built.get_mut(index).expect("within thirty");
            let mut key = [0u8; 32];
            let first = key.get_mut(0).expect("first byte");
            *first = u8::try_from(index).expect("small") + 1;
            slot.key = key;
            slot.is_signer = expected.is_signer();
            slot.is_writable = expected.is_writable();
        }
        built
    }

    #[test]
    fn each_frame_accepts_exactly_its_own_shape() {
        for kind in [
            RelayFrameKindV1::CreateRecord,
            RelayFrameKindV1::AppendObservation,
            RelayFrameKindV1::SealRecord,
            RelayFrameKindV1::RetireRecord,
            RelayFrameKindV1::ConsumeRecord,
            RelayFrameKindV1::ConsumeRecordNativeVenue,
            RelayFrameKindV1::CommitDeadlineFailure,
            RelayFrameKindV1::AdvanceRecovery,
            RelayFrameKindV1::ReclaimMemberSeat,
        ] {
            let built = frame(kind);
            let width = relay_frame_roles_v1(kind).len();
            let exact = built.get(..width).expect("prefix");
            assert_eq!(validate_relay_frame_v1(kind, exact), Ok(()));
            if width > 0 {
                let short = built.get(..width - 1).expect("short");
                assert_eq!(
                    validate_relay_frame_v1(kind, short),
                    Err(Error::InvalidAccountFrame)
                );
            }
        }
    }

    /// The two consumption frames have ONE author: the canonical thirty, and
    /// this walks the twenty-eight against it position by position.  A slot
    /// added to either frame without the other is what this refuses, and it is
    /// the assertion `consume_position_v1`'s arithmetic rests on.
    #[test]
    fn the_native_consume_frame_is_the_canonical_one_without_its_venue_release() {
        let canonical = relay_frame_roles_v1(RelayFrameKindV1::ConsumeRecord);
        let native = relay_frame_roles_v1(RelayFrameKindV1::ConsumeRecordNativeVenue);
        assert_eq!(
            native.len(),
            canonical.len() - CONSUME_VENUE_RELEASE_SLOTS_V1
        );
        assert_eq!(
            canonical
                .get(CONSUME_VENUE_RELEASE_INDEX_V1)
                .map(|role| role.name()),
            Some(RelayAccountNameV1::VenueArtifactRelease),
        );
        assert_eq!(
            canonical
                .get(CONSUME_VENUE_RELEASE_INDEX_V1 + 1)
                .map(|role| role.name()),
            Some(RelayAccountNameV1::VenueArtifactReleaseStagingVacancy),
        );
        for (index, role) in canonical.iter().enumerate() {
            match consume_position_v1(RelayedVenueKindV1::Native, index) {
                Some(moved) => assert_eq!(native.get(moved), Some(role)),
                None => assert!(
                    index == CONSUME_VENUE_RELEASE_INDEX_V1
                        || index == CONSUME_VENUE_RELEASE_INDEX_V1 + 1
                ),
            }
        }
        assert_eq!(
            consume_frame_kind_v1(RelayedVenueKindV1::LoaderV3),
            RelayFrameKindV1::ConsumeRecord
        );
        assert_eq!(
            consume_frame_kind_v1(RelayedVenueKindV1::Native),
            RelayFrameKindV1::ConsumeRecordNativeVenue
        );
    }

    /// A native row's frame is not a shorter canonical one: the thirty-slot
    /// frame presented as a native consumption refuses on its count.
    #[test]
    fn the_canonical_width_does_not_pass_as_a_native_consumption() {
        let built = frame(RelayFrameKindV1::ConsumeRecord);
        assert_eq!(
            validate_relay_frame_v1(RelayFrameKindV1::ConsumeRecordNativeVenue, &built),
            Err(Error::InvalidAccountFrame)
        );
    }

    #[test]
    fn a_missing_worker_signature_refuses() {
        let mut built = frame(RelayFrameKindV1::SealRecord);
        built.get_mut(0).expect("worker").is_signer = false;
        let exact = built
            .get(..relay_frame_roles_v1(RelayFrameKindV1::SealRecord).len())
            .expect("prefix");
        assert_eq!(
            validate_relay_frame_v1(RelayFrameKindV1::SealRecord, exact),
            Err(Error::InvalidAccountFrame)
        );
    }

    #[test]
    fn a_writable_market_where_a_readonly_one_belongs_refuses() {
        // Every route reads the Market and none writes it. A frame that asks
        // for it writable is asking Resolution for an authority over Core state
        // that the role does not have.
        let mut built = frame(RelayFrameKindV1::AppendObservation);
        built.get_mut(1).expect("market").is_writable = true;
        let exact = built
            .get(..relay_frame_roles_v1(RelayFrameKindV1::AppendObservation).len())
            .expect("prefix");
        assert_eq!(
            validate_relay_frame_v1(RelayFrameKindV1::AppendObservation, exact),
            Err(Error::InvalidAccountFrame)
        );
    }

    #[test]
    fn consumption_writes_exactly_the_record_the_source_state_and_the_certificate() {
        let expected = [
            RelayAccountNameV1::Worker,
            RelayAccountNameV1::Record,
            RelayAccountNameV1::SourceResolutionState,
            RelayAccountNameV1::ResolutionCertificate,
        ];
        let mut seen = 0usize;
        for role in CONSUME_RECORD_FRAME_V1
            .iter()
            .filter(|role| role.is_writable())
        {
            assert_eq!(
                Some(role.name()),
                expected.get(seen).copied(),
                "consumption acquired a write authority it does not have"
            );
            seen = seen.saturating_add(1);
        }
        assert_eq!(seen, expected.len(), "a writable position went missing");
        assert!(
            !CONSUME_RECORD_FRAME_V1
                .iter()
                .any(|role| role.name() == RelayAccountNameV1::RelayerKeySet),
            "the key set has no business in a route that verifies no signature"
        );
    }

    #[test]
    fn the_deadline_walk_needs_nothing_the_relayer_supplies() {
        // The executable form of "a silent relayer cannot make a market
        // unresolvable": if any of these appeared in the frame, the walk would
        // depend on the party that has stopped answering.
        for absent in [
            RelayAccountNameV1::Record,
            RelayAccountNameV1::ProviderRelease,
            RelayAccountNameV1::AdapterConfig,
            RelayAccountNameV1::RelayerKeySet,
            RelayAccountNameV1::VenueArtifactRelease,
            RelayAccountNameV1::SourceSpec,
        ] {
            assert!(
                !COMMIT_DEADLINE_FAILURE_FRAME_V1
                    .iter()
                    .any(|role| role.name() == absent),
                "the deadline walk depends on {absent:?}, which a silent relayer controls"
            );
        }
    }

    #[test]
    fn the_deadline_walk_writes_exactly_the_source_the_certificate_and_the_escrow() {
        // The other half of section 4.8's property. "Permissionless" without
        // "paid" is not a shape `ResolutionCertificateV2` can encode, so a walk
        // frame with no escrow in it could only ever produce a certificate the
        // Lean-owned schema refuses.
        let expected = [
            RelayAccountNameV1::Worker,
            RelayAccountNameV1::SourceResolutionState,
            RelayAccountNameV1::ResolutionCertificate,
            RelayAccountNameV1::ResolutionFunding,
        ];
        let mut seen = 0usize;
        for role in COMMIT_DEADLINE_FAILURE_FRAME_V1
            .iter()
            .filter(|role| role.is_writable())
        {
            assert_eq!(
                Some(role.name()),
                expected.get(seen).copied(),
                "the deadline walk acquired a write authority it does not have"
            );
            seen = seen.saturating_add(1);
        }
        assert_eq!(seen, expected.len(), "a writable position went missing");
        assert!(
            COMMIT_DEADLINE_FAILURE_FRAME_V1
                .iter()
                .any(|role| role.name() == RelayAccountNameV1::CapabilityManifest),
            "the bounty amount has to come from the market's own quote"
        );
    }

    /// A fold frame for `k` members: the fixed prefix, then `k` read-only seats
    /// and `k` writable captors, every key distinct. Thirty-nine is the widest
    /// this frame ever is, at `k = 5`.
    fn ensemble_fold_frame(members: u8) -> [RelayAccountPrivilegeV1; 39] {
        let mut built = [RelayAccountPrivilegeV1 {
            key: [0u8; 32],
            is_signer: false,
            is_writable: false,
        }; 39];
        let prefix = relay_frame_roles_v1(RelayFrameKindV1::EnsembleFold);
        for (index, slot) in built.iter_mut().enumerate() {
            let mut key = [0u8; 32];
            let first = key.get_mut(0).expect("first byte");
            *first = u8::try_from(index).expect("small") + 1;
            slot.key = key;
            let role = match prefix.get(index) {
                Some(role) => Some(*role),
                None => ensemble_fold_tail_v1(members, index.saturating_sub(prefix.len())),
            };
            if let Some(role) = role {
                slot.is_signer = role.is_signer();
                slot.is_writable = role.is_writable();
            }
        }
        built
    }

    fn fold_tail_len(members: u8) -> usize {
        usize::from(members).saturating_mul(2)
    }

    #[test]
    fn a_fold_frame_is_its_prefix_and_two_positions_per_declared_member() {
        // The one frame in this family whose WIDTH is a fact of a record: `k`
        // comes off the authenticated material, so the outer validates the
        // fixed prefix and a tail it computed rather than a table it looked up.
        for members in 1..=5_u8 {
            let built = ensemble_fold_frame(members);
            let tail = fold_tail_len(members);
            let width = ENSEMBLE_FOLD_FRAME_PREFIX_V1.len().saturating_add(tail);
            let exact = built.get(..width).expect("frame");
            assert_eq!(
                validate_relay_frame_with_tail_v1(
                    RelayFrameKindV1::EnsembleFold,
                    exact,
                    tail,
                    |index| ensemble_fold_tail_v1(members, index)
                ),
                Ok(())
            );
            let short = built.get(..width.saturating_sub(1)).expect("short");
            assert_eq!(
                validate_relay_frame_with_tail_v1(
                    RelayFrameKindV1::EnsembleFold,
                    short,
                    tail,
                    |index| ensemble_fold_tail_v1(members, index)
                ),
                Err(Error::InvalidAccountFrame),
                "a frame one position short of {members} members is not that ensemble's frame"
            );
            if let Some(grown) = built.get(..width.saturating_add(1)) {
                assert_eq!(
                    validate_relay_frame_with_tail_v1(
                        RelayFrameKindV1::EnsembleFold,
                        grown,
                        tail,
                        |index| ensemble_fold_tail_v1(members, index)
                    ),
                    Err(Error::InvalidAccountFrame)
                );
            }
        }
    }

    #[test]
    fn a_member_seat_passed_twice_cannot_answer_twice() {
        // The no-alias rule spans the WHOLE frame, tail included, which is what
        // stops one written seat from being presented at two member positions
        // and counted as two fragments toward the quorum.
        let members = 3_u8;
        let prefix = ENSEMBLE_FOLD_FRAME_PREFIX_V1.len();
        let tail = fold_tail_len(members);
        let width = prefix.saturating_add(tail);
        let mut built = ensemble_fold_frame(members);
        let seat = built.get(prefix).expect("member zero's seat").key;
        built
            .get_mut(prefix.saturating_add(1))
            .expect("member one's seat")
            .key = seat;
        let exact = built.get(..width).expect("frame");
        assert_eq!(
            validate_relay_frame_with_tail_v1(
                RelayFrameKindV1::EnsembleFold,
                exact,
                tail,
                |index| ensemble_fold_tail_v1(members, index)
            ),
            Err(Error::InvalidAccountFrame)
        );

        // And a captor cannot be the funding ledger it is paid out of.
        let mut built = ensemble_fold_frame(members);
        let funding = built.get(21).expect("resolution funding").key;
        built
            .get_mut(prefix.saturating_add(usize::from(members)))
            .expect("member zero's captor")
            .key = funding;
        let exact = built.get(..width).expect("frame");
        assert_eq!(
            validate_relay_frame_with_tail_v1(
                RelayFrameKindV1::EnsembleFold,
                exact,
                tail,
                |index| ensemble_fold_tail_v1(members, index)
            ),
            Err(Error::InvalidAccountFrame)
        );
    }

    #[test]
    fn a_writable_member_seat_and_a_read_only_captor_both_refuse() {
        // The fold READS every seat and WRITES every captor, and the frame says
        // so in both directions: a writable seat is asking for an authority
        // over a fragment the fold does not have, and a read-only captor is a
        // position no bounty could be paid into.
        let members = 2_u8;
        let prefix = ENSEMBLE_FOLD_FRAME_PREFIX_V1.len();
        let tail = fold_tail_len(members);
        let width = prefix.saturating_add(tail);

        let mut built = ensemble_fold_frame(members);
        built.get_mut(prefix).expect("seat").is_writable = true;
        assert_eq!(
            validate_relay_frame_with_tail_v1(
                RelayFrameKindV1::EnsembleFold,
                built.get(..width).expect("frame"),
                tail,
                |index| ensemble_fold_tail_v1(members, index)
            ),
            Err(Error::InvalidAccountFrame)
        );

        let mut built = ensemble_fold_frame(members);
        built
            .get_mut(prefix.saturating_add(usize::from(members)))
            .expect("captor")
            .is_writable = false;
        assert_eq!(
            validate_relay_frame_with_tail_v1(
                RelayFrameKindV1::EnsembleFold,
                built.get(..width).expect("frame"),
                tail,
                |index| ensemble_fold_tail_v1(members, index)
            ),
            Err(Error::InvalidAccountFrame)
        );
    }

    #[test]
    fn an_aliased_position_refuses_anywhere_in_the_frame() {
        let mut built = frame(RelayFrameKindV1::RetireRecord);
        let record_key = built.get(2).expect("record").key;
        built.get_mut(3).expect("beneficiary").key = record_key;
        let exact = built.get(..4).expect("prefix");
        assert_eq!(
            validate_relay_frame_v1(RelayFrameKindV1::RetireRecord, exact),
            Err(Error::InvalidAccountFrame)
        );
    }
}
