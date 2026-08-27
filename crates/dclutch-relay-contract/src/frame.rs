//! Exact ordered account frames for the six relay record routes.
//!
//! Counts, privileges and the complete no-alias policy are checked here; the
//! *semantic* identity of each position (that this really is the Market, that
//! this really is the pinned raw record) stays an adapter obligation and is
//! authenticated separately by PDA derivation and content identity.
//!
//! These frames are relay-owned rather than added to `dclutch-source-contract`.
//! The relay routes are their own instruction family with their own magic, they
//! name account classes that no Source route has (a relayer key set, an
//! observation record), and `SourceAccountRoleV1`'s constructor is private to
//! its crate.  The consumption frame is here too, for the same reason and one
//! more: it names the *venue's* pinned artifact release and the pinned account
//! set, neither of which is a Source concept, and it is dispatched behind this
//! family's own magic rather than the Source instruction wire.

use crate::{ADDRESS_BYTES, Error, Result};

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
    /// The raw immutable `SourceMaterialV2` record.
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
    /// The Resolution-owned explicit-failure funding compartment the walk debits.
    FailureFunding,
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
const FAILURE_FUNDING: RelayAccountRoleV1 = role(RelayAccountNameV1::FailureFunding, false, true);

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
pub const CONSUME_RECORD_FRAME_V1: [RelayAccountRoleV1; 28] = [
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
    FAILURE_FUNDING,
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
    /// [`COMMIT_DEADLINE_FAILURE_FRAME_V1`].
    CommitDeadlineFailure,
}

/// Return the exact ordered roles for one relay operation.
pub const fn relay_frame_roles_v1(kind: RelayFrameKindV1) -> &'static [RelayAccountRoleV1] {
    match kind {
        RelayFrameKindV1::CreateRecord => &CREATE_RECORD_FRAME_V1,
        RelayFrameKindV1::AppendObservation => &APPEND_OBSERVATION_FRAME_V1,
        RelayFrameKindV1::SealRecord => &SEAL_RECORD_FRAME_V1,
        RelayFrameKindV1::RetireRecord => &RETIRE_RECORD_FRAME_V1,
        RelayFrameKindV1::ConsumeRecord => &CONSUME_RECORD_FRAME_V1,
        RelayFrameKindV1::CommitDeadlineFailure => &COMMIT_DEADLINE_FAILURE_FRAME_V1,
    }
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

    fn frame(kind: RelayFrameKindV1) -> [RelayAccountPrivilegeV1; 28] {
        let roles = relay_frame_roles_v1(kind);
        let mut built = [RelayAccountPrivilegeV1 {
            key: [0; 32],
            is_signer: false,
            is_writable: false,
        }; 28];
        for (index, expected) in roles.iter().enumerate() {
            let slot = built.get_mut(index).expect("within twenty-eight");
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
            RelayFrameKindV1::CommitDeadlineFailure,
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
            RelayAccountNameV1::FailureFunding,
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
