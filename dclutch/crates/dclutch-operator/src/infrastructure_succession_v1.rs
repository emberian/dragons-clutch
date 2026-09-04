//! Chain-derived unsigned Core infrastructure succession composition.
//!
//! `InitializeProtocolInfrastructureV2` is the lineage route's discipline
//! applied to the one selection a Core could not re-make: which Registry and
//! Rent deployments it reads. The V1 profile was write-once with no second
//! write route, so upgrading the Registry bricked the protocol (P-008); the
//! repair is a second write-once profile at its own PDA domain, created under
//! V1's whole gate plus the predecessor conjuncts
//! (`docs/design/PROFILE_UPGRADE_RULING_2026_08_31.md` §5, executed in
//! `dclutch-core-sbf/src/infrastructure_v2.rs`).
//!
//! The wire carries no arguments — sixteen fixed bytes of magic — so every
//! decision the ceremony makes is read out of its twenty-one accounts. A host
//! builder for it has exactly one job, presenting that frame, and the same two
//! temptations the lineage builder refused.
//!
//! It takes no authority. [`CoreInfrastructureSuccessionStateV1`] has no
//! authority field. The key that must stand in a consent slot is the upgrade
//! authority the PREDECESSOR record binds — the key the Loader already required
//! for the physical upgrade, now consenting to the re-selection — and this
//! builder reads it there. Note which side that is: the lineage route reads
//! consent out of the SUCCESSOR's activation cache, because what is being
//! claimed there is that the successor is fit to be moved to. Here what is
//! being consented to is the replacement of the predecessor's own selection, so
//! conjunct 5 asks the predecessor's bound key, and a builder that read the
//! successor's would compose a frame the chain refuses.
//!
//! It takes no `moved` knob either. Whether a binding moved is content:
//! `moved()` in the route hashes the presented successor record and compares
//! that digest against the artifact-release id V1 pinned, and this builder
//! computes the same comparison over the same bytes. A caller says only which
//! predecessor records it is holding; when that disagrees with the bytes it is
//! refused by name rather than quietly corrected, because the disagreement
//! means the caller is describing a different chain than the one it fetched.
//!
//! Conjunct 6 is ONE SUCCESSION PER DOMAIN, not one V2 per domain. It was raw
//! vacancy while this ceremony was the only writer of a V2; since `c60b25e8` a
//! genesis cohort writes its own V2 at initialization, and vacancy would refuse
//! the first real succession of every cohort that started clean — reinstating
//! P-008, the protocol-wide brick this ceremony exists to repair, for exactly
//! the cohorts that never carried the defect. The distinction needs no new
//! field: a profile naming the two genesis sentinels has not spent its
//! succession, and one naming two real artifact releases has.
//!
//! Like every other module in this crate it performs no RPC, holds no key,
//! signs nothing and submits nothing. [`CoreInfrastructureSuccessionReportV1`]
//! names the signatures the frame will require; obtaining them is the caller's
//! problem and deliberately not this crate's.

use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{
    ARTIFACT_RELEASE_BYTES_V1, ARTIFACT_RELEASE_SCHEMA_ID_V1, ArtifactReleaseV1,
    DeploymentObservationV1, require_slot_pinned_release_v1,
};
use dclutch_registry_svm::{ProgramDataV3View, ProgramV3View};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, ExecutionRoleBindingV1, InitializeProtocolInfrastructureV2,
    PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1, PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2,
    PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1, PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2,
    ProtocolInfrastructureProfileV1, ProtocolInfrastructureProfileV2,
};
use solana_program::{
    account_info::AccountInfo,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::{bpf_loader_upgradeable, native_loader, system_program, sysvar};

use crate::{Finality, Observation, ObservedAccount};

/// Exact account count of the succession frame.
///
/// This restates `dclutch_core_sbf::INITIALIZE_INFRASTRUCTURE_ACCOUNT_COUNT_V2`
/// rather than importing it: a host builder crate does not link the Core
/// program. The builder checks its own composed frame against this number, and
/// the campaign that drives the compiled program is what proves the two agree.
pub const INITIALIZE_INFRASTRUCTURE_ACCOUNT_COUNT_V2: usize = 21;
/// Frame index of the Registry binding's consent slot.
pub const REGISTRY_CONSENT_ACCOUNT_V2: usize = 15;
/// Frame index of the Rent binding's consent slot.
pub const RENT_CONSENT_ACCOUNT_V2: usize = 18;

/// The predecessor's own finalized artifact record, for a binding that moved.
///
/// Presented as evidence, never as a selection: the ceremony reads exactly two
/// facts out of it — the deployment slot the forward-only conjunct compares
/// against, and the upgrade authority whose signature conjunct 5 demands. The
/// deployment behind it is deliberately not observed anywhere, because it is
/// the superseded one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PredecessorRecordObservationV1 {
    /// Registry-owned headerless `ArtifactReleaseV1` record bytes.
    pub raw: ObservedAccount,
    /// The record's canonical staging cursor, vacant because it finalized.
    pub staging: ObservedAccount,
}

/// Same-finalized inputs for one infrastructure succession ceremony.
///
/// Fifteen accounts plus two optional predecessor records, not twenty-one. The
/// two consent slots are absent because they are derived, and the two
/// predecessor-evidence pairs are `Option`s rather than a `moved` flag: a
/// caller states what it holds, and the bytes state what moved. See the module
/// documentation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreInfrastructureSuccessionStateV1 {
    /// System wallet signing and paying the V2 profile's rent.
    pub payer: ObservedAccount,
    /// The derived V2 profile PDA, vacant or holding an unspent genesis V2.
    pub profile: ObservedAccount,
    /// The written V1 profile PDA this succession succeeds.
    pub predecessor_profile: ObservedAccount,
    /// Core's own Loader V3 ProgramData, read for its live upgrade authority.
    pub core_programdata: ObservedAccount,
    /// The key that ProgramData binds, which must sign.
    pub upgrade_authority: ObservedAccount,
    /// Successor Registry `ArtifactReleaseV1` record bytes.
    pub registry_artifact_raw: ObservedAccount,
    /// Successor Registry record's vacant staging cursor.
    pub registry_artifact_staging: ObservedAccount,
    /// Current executable Registry Program account.
    pub registry_program: ObservedAccount,
    /// Current Registry ProgramData account and complete ELF tail.
    pub registry_programdata: ObservedAccount,
    /// Successor Rent `ArtifactReleaseV1` record bytes.
    pub rent_artifact_raw: ObservedAccount,
    /// Successor Rent record's vacant staging cursor.
    pub rent_artifact_staging: ObservedAccount,
    /// Current executable Rent Program account.
    pub rent_program: ObservedAccount,
    /// Current Rent ProgramData account and complete ELF tail.
    pub rent_programdata: ObservedAccount,
    /// The Registry record V1 pinned, presented when the caller believes it moved.
    pub predecessor_registry_record: Option<PredecessorRecordObservationV1>,
    /// The Rent record V1 pinned, presented when the caller believes it moved.
    pub predecessor_rent_record: Option<PredecessorRecordObservationV1>,
    /// Canonical Rent sysvar.
    pub rent_sysvar: ObservedAccount,
    /// Canonical executable System Program.
    pub system_program: ObservedAccount,
}

impl CoreInfrastructureSuccessionStateV1 {
    /// Every observed input, in frame order, for the shared-observation checks.
    fn observed(&self) -> Vec<&ObservedAccount> {
        let mut accounts = Vec::with_capacity(INITIALIZE_INFRASTRUCTURE_ACCOUNT_COUNT_V2);
        accounts.extend([
            &self.payer,
            &self.profile,
            &self.predecessor_profile,
            &self.core_programdata,
            &self.upgrade_authority,
            &self.registry_artifact_raw,
            &self.registry_artifact_staging,
            &self.registry_program,
            &self.registry_programdata,
            &self.rent_artifact_raw,
            &self.rent_artifact_staging,
            &self.rent_program,
            &self.rent_programdata,
        ]);
        for record in [
            &self.predecessor_registry_record,
            &self.predecessor_rent_record,
        ] {
            if let Some(record) = record.as_ref() {
                accounts.extend([&record.raw, &record.staging]);
            }
        }
        accounts.extend([&self.rent_sysvar, &self.system_program]);
        accounts
    }
}

/// Which of the profile's two infrastructure bindings an arm speaks for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InfrastructureBindingV1 {
    /// The Registry program selection.
    Registry,
    /// The Rent program selection.
    Rent,
}

/// What one binding contributes to a succession frame.
///
/// The pair `(slot, must_sign)` is the whole of conjunct 5 for this binding,
/// and it is derived from the two records rather than chosen. An unmoved
/// binding's slot is `system_program::ID` and must NOT sign: its selection is
/// byte-identical to V1's, so nothing is being consented to and nothing may
/// stand where consent would go.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InfrastructureSuccessionConsentV1 {
    /// The binding this slot speaks for.
    pub binding: InfrastructureBindingV1,
    /// Whether this binding's artifact release id changed across the succession.
    pub moved: bool,
    /// The exact account the frame must carry in this binding's consent slot.
    pub slot: Pubkey,
    /// Whether that account must sign the transaction.
    pub must_sign: bool,
}

/// Fully checked unsigned succession ceremony and the profile it would write.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreInfrastructureSuccessionReportV1 {
    /// Exact unsigned twenty-one-account Core instruction.
    pub instruction: Instruction,
    /// Shared finalized observation selecting every input.
    pub observation: Observation,
    /// Canonical derived V2 profile address, the domain's one succession.
    pub profile: Pubkey,
    /// Canonical bump Core will sign that account into existence with.
    pub profile_bump: u8,
    /// What conjunct 6 found standing at that address.
    ///
    /// A caller that prints "vacant" unconditionally is describing a chain it
    /// did not read: since `c60b25e8` a cohort writes its own genesis V2 at
    /// initialization, so `BornAtV2` is the ordinary standing and `Vacant` is
    /// the one only a pre-genesis-arm Core can present.
    pub profile_standing: SuccessionProfileStandingV1,
    /// The exact 224 bytes this ceremony would persist, composed locally.
    ///
    /// A caller can print this and byte-compare it against what lands. The
    /// profile carries no clock, so a succession composed long after the
    /// upgrade it records composes to exactly the bytes it would have then.
    pub record: ProtocolInfrastructureProfileV2,
    /// Per-binding consent projection, Registry then Rent, in frame order.
    pub consent: [InfrastructureSuccessionConsentV1; 2],
    /// Distinct keys whose signatures the frame requires, payer first.
    ///
    /// Deduplicated: on a cluster where one deployer key holds Core's Loader
    /// authority and consented to both upgrades, a succession that moved both
    /// bindings still needs exactly two signatures.
    pub required_signers: Vec<Pubkey>,
    /// Exact lamports the payer will spend on the 224-byte profile.
    ///
    /// On a vacant domain the route tops the PDA up to rent exemption rather
    /// than funding it outright (`create_profile_v2`), so lamports already
    /// sitting on the address reduce this debit to exactly what is still owed.
    /// On a `BornAtV2` domain it is **zero**: the account already exists at the
    /// exact width, already Core-owned and already rent-exempt, and the route
    /// overwrites its bytes without a transfer.
    pub profile_rent_debit_lamports: u64,
}

/// Stable refusal from a frame the succession ceremony would not accept.
///
/// Every arm of the route that this builder can see from a finalized snapshot
/// refuses here under its own name, so a caller learns which conjunct it
/// violated instead of reading a simulation log.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// At least one account was not observed at finalized commitment.
    ObservationNotFinalized,
    /// Inputs did not share one exact finalized observation.
    ObservationMismatch,
    /// Equal keys carried conflicting observed account facts.
    InconsistentAlias,
    /// Two frame slots shared a key where the frame's distinctness refuses it.
    ///
    /// The frame exempts exactly two aliasings: the natural-person slots (payer,
    /// Core's upgrade authority, the two consent slots) may share keys, and any
    /// two System-valued slots may share theirs.
    AliasedFrameAccount,
    /// The payer was not a usable System wallet.
    InvalidPayer,
    /// The System Program or Rent sysvar was not canonical.
    InvalidRuntimePlumbing,
    /// Conjunct 1: Core's ProgramData is not its own, or binds another key.
    InvalidCoreUpgradeAuthority,
    /// Conjunct 2: the V1 profile is not written at its derived PDA, or does
    /// not hostile-decode to a canonical V1 profile.
    PredecessorProfileAbsent,
    /// Conjunct 1: a presented successor artifact record refused its own
    /// bytes, digest, Registry PDAs, reserve, vacant cursor, or pinned shape.
    InvalidSuccessorRecord,
    /// Conjunct 1: a presented deployment did not authenticate against the
    /// release record that selects it.
    InvalidDeployment,
    /// Conjunct 1: the selected Registry or Rent program is Core itself.
    InfrastructureProgramIsCore,
    /// Conjunct 3: a binding's program identity differs across the succession.
    IdentityMoved,
    /// Conjunct 4: a moved binding's deployment slot did not strictly advance.
    NotForward,
    /// Conjunct 4's degenerate arm: a succession in which nothing moved selects
    /// nothing new and would spend the one vacancy this domain will ever have.
    NothingMoved,
    /// The bytes say this binding moved and no predecessor record was supplied.
    MovedBindingWithoutPredecessorRecord,
    /// A predecessor record was supplied for a binding whose bytes did not move.
    ///
    /// Nothing is being consented to, so nothing may stand in the slots that
    /// could look like predecessor evidence — the unmoved arm requires the
    /// System program in all three.
    UnmovedBindingWithPredecessorRecord,
    /// Conjunct 4 and 5's evidence: the presented predecessor record is not the
    /// one V1 pinned, or refused its own record checks.
    InvalidPredecessorRecord,
    /// Conjunct 5: a moved binding's predecessor record binds no upgrade
    /// authority to ask consent of.
    ///
    /// An `Immutable` predecessor could not have had its bytes moved, so a
    /// moved claim against it is a contradiction, not a missing signature.
    ConsentAuthorityMissing,
    /// A consent slot's key is the payer's, which no single message can serve.
    ///
    /// The frame refuses a writable consent slot, the payer's slot must be
    /// writable, and one key appearing twice in a message carries the union of
    /// its privileges. A deployer key that is also funding the ceremony has to
    /// hand the fee payment to a second wallet; sharing it with Core's upgrade
    /// authority slot, which the frame does permit, is the usual arrangement.
    ConsentAuthorityIsPayer,
    /// Conjunct 6: the presented profile account is not the derived V2 address.
    InvalidProfileAddress,
    /// Conjunct 6: the V2 domain's succession is spent — one succession per
    /// domain, ever, and this one already happened.
    ///
    /// A profile naming two real predecessor artifact releases has succeeded;
    /// one naming the two genesis sentinels has not, and is overwritten in
    /// place. Anything else at the address — a foreign owner, the wrong width,
    /// an executable, bytes that do not decode as a V2 — is this refusal too:
    /// an account the ceremony cannot read is never room to write.
    AlreadySucceeded,
    /// The payer could not cover the exact profile rent debit.
    InsufficientPayer,
    /// The composed V2 profile refused its own aliasing check.
    ///
    /// A belt: both aliasings it refuses on the successor side are already
    /// impossible under conjunct 3 and the V1 profile's own decode.
    ProfileIncoherent,
    /// Frame composition did not produce the exact account count.
    Encoding,
}

/// Build the exact twenty-one-account Core succession ceremony.
///
/// # Errors
///
/// Refuses locally on everything it can see the chain will refuse: the V2 PDA
/// at another address, or one whose succession is already spent
/// ([`Error::InvalidProfileAddress`], [`Error::AlreadySucceeded`]), an absent or undecodable predecessor profile
/// ([`Error::PredecessorProfileAbsent`]), conjunct 1's record and deployment
/// authentication ([`Error::InvalidSuccessorRecord`],
/// [`Error::InvalidDeployment`], [`Error::InfrastructureProgramIsCore`]),
/// conjunct 3 ([`Error::IdentityMoved`]), conjunct 4 in both its arms
/// ([`Error::NotForward`], [`Error::NothingMoved`]), conjunct 5's evidence and
/// consent ([`Error::InvalidPredecessorRecord`],
/// [`Error::ConsentAuthorityMissing`]), and a caller whose belief about what
/// moved disagrees with the bytes ([`Error::MovedBindingWithoutPredecessorRecord`],
/// [`Error::UnmovedBindingWithPredecessorRecord`]). Building a frame the chain
/// will refuse is not a service to the caller.
pub fn build_core_infrastructure_succession_v1(
    core_program: Pubkey,
    state: &CoreInfrastructureSuccessionStateV1,
) -> Result<CoreInfrastructureSuccessionReportV1, Error> {
    let accounts = state.observed();
    let observation = same_observation(&accounts)?;
    authenticate_aliases(&accounts)?;
    authenticate_payer(&state.payer)?;
    authenticate_system_program(&state.system_program)?;
    let rent = decode_rent(&state.rent_sysvar)?;

    // Conjunct 1: the party that could already replace the reader itself.
    authenticate_core_upgrade_authority(
        core_program,
        &state.core_programdata,
        &state.upgrade_authority,
    )?;

    // Conjunct 2: the predecessor stands written and decodes.
    let predecessor =
        authenticate_predecessor_profile(core_program, &state.predecessor_profile, &rent)?;

    // Whether a binding MOVED is decided by content, before any of the records
    // are authenticated and without consulting the caller: the presented
    // successor record's digest against the id V1 pinned.
    let registry_moved = moved(&state.registry_artifact_raw, predecessor.registry())?;
    let rent_moved = moved(&state.rent_artifact_raw, predecessor.rent())?;

    // Conjunct 1, per binding. Both records live under the CURRENT Registry,
    // the Rent record included.
    let registry = state.registry_program.key;
    let (registry_binding, registry_release) = authenticate_successor_record(
        registry,
        &state.registry_artifact_raw,
        &state.registry_artifact_staging,
        &state.registry_program,
        &state.registry_programdata,
        &rent,
    )?;
    let (rent_binding, rent_release) = authenticate_successor_record(
        registry,
        &state.rent_artifact_raw,
        &state.rent_artifact_staging,
        &state.rent_program,
        &state.rent_programdata,
        &rent,
    )?;
    if registry == core_program || state.rent_program.key == core_program {
        return Err(Error::InfrastructureProgramIsCore);
    }

    // Conjunct 3: bytes may move, identity never.
    if registry_binding.program() != predecessor.registry().program()
        || rent_binding.program() != predecessor.rent().program()
    {
        return Err(Error::IdentityMoved);
    }

    // Conjunct 4's degenerate arm: a succession that moves nothing selects
    // nothing new, and burns the one vacancy this domain will ever have.
    if !registry_moved && !rent_moved {
        return Err(Error::NothingMoved);
    }

    // Conjuncts 4 and 5, one arm per binding.
    let registry_arm = compose_arm(
        InfrastructureBindingV1::Registry,
        registry,
        predecessor.registry(),
        registry_release,
        registry_moved,
        state.predecessor_registry_record.as_ref(),
        &rent,
    )?;
    let rent_arm = compose_arm(
        InfrastructureBindingV1::Rent,
        registry,
        predecessor.rent(),
        rent_release,
        rent_moved,
        state.predecessor_rent_record.as_ref(),
        &rent,
    )?;

    let record = ProtocolInfrastructureProfileV2::new(
        registry_binding,
        rent_binding,
        predecessor.registry().artifact_release(),
        predecessor.rent().artifact_release(),
    )
    .map_err(|_| Error::ProfileIncoherent)?;

    // Conjunct 6: the address, and the ONE SUCCESSION PER DOMAIN that forbids a
    // second ceremony. Not one V2 per domain -- see `profile_standing`.
    let (profile, profile_bump) = Pubkey::find_program_address(
        &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2],
        &core_program,
    );
    if state.profile.key != profile {
        return Err(Error::InvalidProfileAddress);
    }
    let standing = profile_standing(core_program, &state.profile)?;
    // Exactly `create_profile_v2`'s two paths. A vacant domain is created and
    // topped up to rent exemption; a genesis profile is OVERWRITTEN IN PLACE,
    // and that path transfers nothing at all -- the account is already
    // Core-owned at the exact width, and the System program would refuse both
    // allocate and assign. Reporting a debit the ceremony will not spend would
    // be a forecast of a transfer that never happens.
    let profile_rent_debit_lamports = match standing {
        SuccessionProfileStandingV1::Vacant => rent
            .minimum_balance(PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2)
            .saturating_sub(state.profile.lamports),
        SuccessionProfileStandingV1::BornAtV2 => 0,
    };
    if state.payer.lamports < profile_rent_debit_lamports {
        return Err(Error::InsufficientPayer);
    }

    let arms = [registry_arm, rent_arm];
    for arm in &arms {
        if arm.consent.must_sign && arm.consent.slot == state.payer.key {
            return Err(Error::ConsentAuthorityIsPayer);
        }
    }

    let metas = compose_frame(state, profile, &arms)?;
    require_distinct_for_succession(&metas)?;

    let mut required_signers = vec![state.payer.key];
    if !required_signers.contains(&state.upgrade_authority.key) {
        required_signers.push(state.upgrade_authority.key);
    }
    for arm in &arms {
        if arm.consent.must_sign && !required_signers.contains(&arm.consent.slot) {
            required_signers.push(arm.consent.slot);
        }
    }

    Ok(CoreInfrastructureSuccessionReportV1 {
        instruction: Instruction {
            program_id: core_program,
            accounts: metas,
            data: InitializeProtocolInfrastructureV2.to_bytes().to_vec(),
        },
        observation,
        profile,
        profile_bump,
        profile_standing: standing,
        record,
        consent: [registry_arm.consent, rent_arm.consent],
        required_signers,
        profile_rent_debit_lamports,
    })
}

/// One binding's contribution to the frame: its consent slot and its evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SuccessionArmV1 {
    consent: InfrastructureSuccessionConsentV1,
    /// The predecessor record's raw account, or System when nothing moved.
    predecessor_raw: Pubkey,
    /// The predecessor record's staging cursor, or System when nothing moved.
    predecessor_staging: Pubkey,
}

/// Whether the presented successor record differs from the V1-pinned one.
///
/// The sole definition of "this binding moved across the succession", computed
/// exactly as `moved()` in the route computes it: the raw record's bytes are
/// read only to hash them, and the digest IS the artifact-release id the
/// profile pinned. Every authentication of those bytes happens afterward.
fn moved(
    successor_raw: &ObservedAccount,
    predecessor_binding: ExecutionRoleBindingV1,
) -> Result<bool, Error> {
    if successor_raw.data.len() != ARTIFACT_RELEASE_BYTES_V1 {
        return Err(Error::InvalidSuccessorRecord);
    }
    let digest = ArtifactReleaseIdV1::new(hash(&successor_raw.data).to_bytes())
        .map_err(|_| Error::InvalidSuccessorRecord)?;
    Ok(digest != predecessor_binding.artifact_release())
}

/// Conjunct 1: Core's live Loader authority, and only Core's.
///
/// The ProgramData account is Core's own derived one and currently binds the
/// key presented in the signing slot. The frame also refuses an executable
/// account there, which no signer can be.
fn authenticate_core_upgrade_authority(
    core_program: Pubkey,
    programdata: &ObservedAccount,
    authority: &ObservedAccount,
) -> Result<(), Error> {
    let expected =
        Pubkey::find_program_address(&[core_program.as_ref()], &bpf_loader_upgradeable::ID).0;
    if programdata.key != expected
        || programdata.owner != bpf_loader_upgradeable::ID
        || programdata.executable
        || authority.executable
    {
        return Err(Error::InvalidCoreUpgradeAuthority);
    }
    let view = ProgramDataV3View::parse(&programdata.data)
        .map_err(|_| Error::InvalidCoreUpgradeAuthority)?;
    if view.upgrade_authority() != Some(authority.key.to_bytes()) {
        return Err(Error::InvalidCoreUpgradeAuthority);
    }
    Ok(())
}

/// Conjunct 2: the V1 profile, present and decodable — nothing more.
///
/// Deliberately not the inspection read: that one re-authenticates V1's pinned
/// deployments, and after the upgrade this ceremony repairs it refuses on the
/// superseded Registry, which is the whole situation the route exists for.
/// Presence means the exact derived V1 PDA, Core-owned, exact V1 width,
/// non-executable, rent-exempt, hostile-decoding to a canonical V1 profile.
fn authenticate_predecessor_profile(
    core_program: Pubkey,
    predecessor_profile: &ObservedAccount,
    rent: &Rent,
) -> Result<ProtocolInfrastructureProfileV1, Error> {
    let expected = Pubkey::find_program_address(
        &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1],
        &core_program,
    )
    .0;
    if predecessor_profile.key != expected
        || predecessor_profile.owner != core_program
        || predecessor_profile.executable
        || predecessor_profile.data.len() != PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1
        || !rent.is_exempt(
            predecessor_profile.lamports,
            PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1,
        )
    {
        return Err(Error::PredecessorProfileAbsent);
    }
    ProtocolInfrastructureProfileV1::decode(&predecessor_profile.data)
        .map_err(|_| Error::PredecessorProfileAbsent)
}

/// What the V2 domain holds when a succession is composed against it.
///
/// The two states conjunct 6 ADMITS. `Succeeded` is not a variant here because
/// it is not a standing a report can carry: the builder refuses it by name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SuccessionProfileStandingV1 {
    /// No V2 has ever been written. The route creates the account.
    Vacant,
    /// A genesis profile stands here, born at V2 with its succession unspent.
    ///
    /// The route overwrites it in place and transfers nothing.
    BornAtV2,
}

/// Conjunct 6's classification of the V2 domain, mirroring the route exactly.
///
/// This is `dclutch_core_sbf::infrastructure_v2::profile_succession_state_v2`
/// restated rather than imported — a host builder crate does not link the Core
/// program — and the campaign that drives the compiled program is what proves
/// the two agree. Restating it is why it drifted once: `c60b25e8` changed the
/// route's conjunct 6 from RAW VACANCY to ONE SUCCESSION PER DOMAIN and this
/// side was left behind, so the builder refused, before it built anything, the
/// first succession of every cohort born at V2 — which since that commit is
/// every cohort. The chain would have taken it.
///
/// Anything occupying the PDA that is not a decodable Core-owned V2 of the
/// exact width is `AlreadySucceeded`, never room to write: an account this
/// ceremony cannot read is not one it gets to overwrite. That is what makes a
/// V2 profile under a FOREIGN Core — a decodable profile at a PDA this Core did
/// not derive, or a profile this Core does not own — refuse by the no-fork name
/// rather than by a decode accident.
fn profile_standing(
    core_program: Pubkey,
    profile: &ObservedAccount,
) -> Result<SuccessionProfileStandingV1, Error> {
    if profile.owner == system_program::ID && profile.data.is_empty() && !profile.executable {
        return Ok(SuccessionProfileStandingV1::Vacant);
    }
    if profile.owner != core_program
        || profile.executable
        || profile.data.len() != PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2
    {
        return Err(Error::AlreadySucceeded);
    }
    match ProtocolInfrastructureProfileV2::decode(&profile.data) {
        Ok(standing) if standing.born_at_v2() => Ok(SuccessionProfileStandingV1::BornAtV2),
        _ => Err(Error::AlreadySucceeded),
    }
}

/// Conjunct 1 for one selected binding: its record, and the code it describes.
fn authenticate_successor_record(
    registry: Pubkey,
    raw: &ObservedAccount,
    staging: &ObservedAccount,
    program: &ObservedAccount,
    programdata: &ObservedAccount,
    rent: &Rent,
) -> Result<(ExecutionRoleBindingV1, ArtifactReleaseV1), Error> {
    let (release, artifact) =
        authenticate_artifact_record(registry, raw, staging, rent, Error::InvalidSuccessorRecord)?;
    if release.program().to_bytes() != program.key.to_bytes() {
        return Err(Error::InvalidSuccessorRecord);
    }
    require_slot_pinned_release_v1(release).map_err(|_| Error::InvalidSuccessorRecord)?;
    authenticate_deployment(release, program, programdata)?;
    Ok((
        ExecutionRoleBindingV1::new(release.program(), artifact),
        release,
    ))
}

/// One content-addressed Registry record, finalized and unstaged.
///
/// The digest of the bytes is the record's whole address: both PDAs are
/// re-derived from it, so a substituted record is a different account rather
/// than a different opinion. The caller supplies the name its own conjunct
/// refuses under, because a successor record and a predecessor record failing
/// this check mean different things to an operator.
fn authenticate_artifact_record(
    registry: Pubkey,
    raw: &ObservedAccount,
    staging: &ObservedAccount,
    rent: &Rent,
    refusal: Error,
) -> Result<(ArtifactReleaseV1, ArtifactReleaseIdV1), Error> {
    if raw.data.len() != ARTIFACT_RELEASE_BYTES_V1 {
        return Err(refusal);
    }
    let digest = hash(&raw.data).to_bytes();
    let expected_raw = Pubkey::find_program_address(
        &[
            RAW_RECORD_PDA_SEED_V1,
            &ARTIFACT_RELEASE_SCHEMA_ID_V1,
            &digest,
        ],
        &registry,
    )
    .0;
    let expected_staging = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            &ARTIFACT_RELEASE_SCHEMA_ID_V1,
            &digest,
        ],
        &registry,
    )
    .0;
    // Nonzero lamports on the vacant cursor are unclassified dust, exactly as
    // the record authority treats them; ownership and emptiness are the check.
    if raw.key != expected_raw
        || raw.owner != registry
        || raw.executable
        || !rent.is_exempt(raw.lamports, raw.data.len())
        || staging.key != expected_staging
        || staging.owner != system_program::ID
        || staging.executable
        || !staging.data.is_empty()
    {
        return Err(refusal);
    }
    let release = ArtifactReleaseV1::decode(&raw.data).map_err(|_| refusal)?;
    let artifact = ArtifactReleaseIdV1::new(digest).map_err(|_| refusal)?;
    Ok((release, artifact))
}

/// The observed Loader V3 deployment, authenticated against its own record.
///
/// The host hashes the complete observed ELF on both arms. On chain the
/// unmoved binding rides V1's first admission and skips that hash to save
/// compute; a host has no such budget to spend, and a snapshot whose ELF
/// disagrees with the pin is one an operator wants named before it composes a
/// ceremony around it.
fn authenticate_deployment(
    release: ArtifactReleaseV1,
    program: &ObservedAccount,
    programdata: &ObservedAccount,
) -> Result<(), Error> {
    if release.loader_program().to_bytes() != bpf_loader_upgradeable::ID.to_bytes()
        || release.programdata() != programdata.key.to_bytes()
        || program.owner != bpf_loader_upgradeable::ID
        || programdata.owner != bpf_loader_upgradeable::ID
        || !program.executable
        || programdata.executable
    {
        return Err(Error::InvalidDeployment);
    }
    let program_view = ProgramV3View::parse(&program.data).map_err(|_| Error::InvalidDeployment)?;
    let programdata_view =
        ProgramDataV3View::parse(&programdata.data).map_err(|_| Error::InvalidDeployment)?;
    let derived =
        Pubkey::find_program_address(&[program.key.as_ref()], &bpf_loader_upgradeable::ID).0;
    if program_view.programdata() != programdata.key.to_bytes() || programdata.key != derived {
        return Err(Error::InvalidDeployment);
    }
    let observation = DeploymentObservationV1::new(
        program.key.to_bytes(),
        program.owner.to_bytes(),
        program.executable,
        programdata.key.to_bytes(),
        programdata.owner.to_bytes(),
        programdata.executable,
        program_view.programdata(),
        bpf_loader_upgradeable::ID.to_bytes(),
        programdata_view.deployment_slot(),
        hash(programdata_view.elf()).to_bytes(),
        programdata_view.upgrade_authority(),
    )
    .map_err(|_| Error::InvalidDeployment)?;
    release
        .authenticate_deployment(observation)
        .map_err(|_| Error::InvalidDeployment)
}

/// Conjuncts 4 and 5 for one binding, projected from the records alone.
///
/// This is deliberately the same shape as the route's own
/// `authenticate_succession_arm`, in the same order, reading the same fields —
/// so an arm this function admits is one the program admits, and an arm the
/// program would refuse is refused here by the analogous name.
fn compose_arm(
    binding: InfrastructureBindingV1,
    registry: Pubkey,
    predecessor_binding: ExecutionRoleBindingV1,
    successor_release: ArtifactReleaseV1,
    moved: bool,
    record: Option<&PredecessorRecordObservationV1>,
    rent: &Rent,
) -> Result<SuccessionArmV1, Error> {
    let Some(record) = record else {
        if moved {
            return Err(Error::MovedBindingWithoutPredecessorRecord);
        }
        return Ok(SuccessionArmV1 {
            consent: InfrastructureSuccessionConsentV1 {
                binding,
                moved: false,
                slot: system_program::ID,
                must_sign: false,
            },
            predecessor_raw: system_program::ID,
            predecessor_staging: system_program::ID,
        });
    };
    if !moved {
        return Err(Error::UnmovedBindingWithPredecessorRecord);
    }
    let (predecessor_release, artifact) = authenticate_artifact_record(
        registry,
        &record.raw,
        &record.staging,
        rent,
        Error::InvalidPredecessorRecord,
    )?;
    // The record digest IS the artifact-release id, so pinning the presented
    // bytes to V1's id leaves an attacker no record to substitute.
    if artifact != predecessor_binding.artifact_release()
        || predecessor_release.program() != predecessor_binding.program()
    {
        return Err(Error::InvalidPredecessorRecord);
    }
    require_slot_pinned_release_v1(predecessor_release)
        .map_err(|_| Error::InvalidPredecessorRecord)?;
    // Conjunct 4: under Loader V3 a ProgramData slot only moves forward, so
    // strictly greater is exactly "was upgraded after".
    if successor_release.deployment_slot() <= predecessor_release.deployment_slot() {
        return Err(Error::NotForward);
    }
    // Conjunct 5: the key that moved the bytes consents to the re-selection.
    let bound = predecessor_release
        .upgrade_authority()
        .ok_or(Error::ConsentAuthorityMissing)?;
    Ok(SuccessionArmV1 {
        consent: InfrastructureSuccessionConsentV1 {
            binding,
            moved: true,
            slot: Pubkey::new_from_array(bound),
            must_sign: true,
        },
        predecessor_raw: record.raw.key,
        predecessor_staging: record.staging.key,
    })
}

/// The twenty-one metas, in the order the frame destructures them.
///
/// Exactly two accounts are writable — the payer and the profile the ceremony
/// creates — and exactly the payer, Core's upgrade authority, and each moved
/// binding's consent slot sign.
fn compose_frame(
    state: &CoreInfrastructureSuccessionStateV1,
    profile: Pubkey,
    arms: &[SuccessionArmV1; 2],
) -> Result<Vec<AccountMeta>, Error> {
    let mut metas = Vec::with_capacity(INITIALIZE_INFRASTRUCTURE_ACCOUNT_COUNT_V2);
    metas.push(AccountMeta::new(state.payer.key, true));
    metas.push(AccountMeta::new(profile, false));
    metas.push(AccountMeta::new_readonly(
        state.predecessor_profile.key,
        false,
    ));
    metas.push(AccountMeta::new_readonly(state.core_programdata.key, false));
    metas.push(AccountMeta::new_readonly(state.upgrade_authority.key, true));
    for account in [
        &state.registry_artifact_raw,
        &state.registry_artifact_staging,
        &state.registry_program,
        &state.registry_programdata,
        &state.rent_artifact_raw,
        &state.rent_artifact_staging,
        &state.rent_program,
        &state.rent_programdata,
    ] {
        metas.push(AccountMeta::new_readonly(account.key, false));
    }
    for (arm, base) in arms
        .iter()
        .zip([REGISTRY_CONSENT_ACCOUNT_V2, RENT_CONSENT_ACCOUNT_V2])
    {
        if metas.len() != base.saturating_sub(2) {
            return Err(Error::Encoding);
        }
        metas.push(AccountMeta::new_readonly(arm.predecessor_raw, false));
        metas.push(AccountMeta::new_readonly(arm.predecessor_staging, false));
        metas.push(AccountMeta::new_readonly(
            arm.consent.slot,
            arm.consent.must_sign,
        ));
    }
    metas.push(AccountMeta::new_readonly(state.rent_sysvar.key, false));
    metas.push(AccountMeta::new_readonly(state.system_program.key, false));
    if metas.len() != INITIALIZE_INFRASTRUCTURE_ACCOUNT_COUNT_V2 {
        return Err(Error::Encoding);
    }
    Ok(metas)
}

/// The frame's own distinctness policy, with its two named exemptions.
///
/// 1. The natural-person slots — payer, Core's upgrade authority, and the two
///    consent authorities — may share keys freely: on a real cluster they are
///    frequently one key, and conjunct 5 constrains what each must SIGN, not
///    that the humans be distinct people.
/// 2. Any two slots holding the System program may alias: an unmoved binding
///    stands it in three slots beside the frame's own System account.
fn require_distinct_for_succession(metas: &[AccountMeta]) -> Result<(), Error> {
    const PERSON_SLOTS: [usize; 4] = [0, 4, REGISTRY_CONSENT_ACCOUNT_V2, RENT_CONSENT_ACCOUNT_V2];
    for (left_index, left) in metas.iter().enumerate() {
        for (right_index, right) in metas.iter().enumerate().skip(left_index.saturating_add(1)) {
            if left.pubkey != right.pubkey {
                continue;
            }
            let persons = PERSON_SLOTS.contains(&left_index) && PERSON_SLOTS.contains(&right_index);
            let both_system = left.pubkey == system_program::ID;
            if !persons && !both_system {
                return Err(Error::AliasedFrameAccount);
            }
        }
    }
    Ok(())
}

fn authenticate_payer(payer: &ObservedAccount) -> Result<(), Error> {
    if payer.owner != system_program::ID || payer.executable || !payer.data.is_empty() {
        return Err(Error::InvalidPayer);
    }
    Ok(())
}

/// Require the canonical executable System Program.
///
/// The identity triple is the whole check, and it deliberately does not require
/// the account to be dataless: a native program account carries its own name as
/// data, so an emptiness requirement admits only hand-synthesized fictions and
/// refuses every account read off a real chain.
fn authenticate_system_program(system: &ObservedAccount) -> Result<(), Error> {
    if system.key != system_program::ID || system.owner != native_loader::ID || !system.executable {
        return Err(Error::InvalidRuntimePlumbing);
    }
    Ok(())
}

fn decode_rent(account: &ObservedAccount) -> Result<Rent, Error> {
    if account.key != sysvar::rent::ID
        || account.owner != sysvar::ID
        || account.executable
        || account.data.len() != Rent::size_of()
    {
        return Err(Error::InvalidRuntimePlumbing);
    }
    let mut lamports = account.lamports;
    let mut data = account.data.clone();
    let info = AccountInfo::new(
        &account.key,
        false,
        false,
        &mut lamports,
        &mut data,
        &account.owner,
        false,
    );
    Rent::from_account_info(&info).map_err(|_| Error::InvalidRuntimePlumbing)
}

fn same_observation(accounts: &[&ObservedAccount]) -> Result<Observation, Error> {
    let observation = accounts
        .first()
        .map(|account| account.observation)
        .ok_or(Error::ObservationMismatch)?;
    if accounts
        .iter()
        .any(|account| account.observation.finality != Finality::Finalized)
    {
        return Err(Error::ObservationNotFinalized);
    }
    if accounts
        .iter()
        .any(|account| account.observation != observation)
    {
        return Err(Error::ObservationMismatch);
    }
    Ok(observation)
}

fn authenticate_aliases(accounts: &[&ObservedAccount]) -> Result<(), Error> {
    for (left_index, left) in accounts.iter().enumerate() {
        for right in accounts.iter().skip(left_index.saturating_add(1)) {
            if left.key == right.key && left != right {
                return Err(Error::InconsistentAlias);
            }
        }
    }
    Ok(())
}

const _: () = assert!(
    PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2 == 224,
    "the succession profile is the frozen 224-byte wire"
);

#[cfg(test)]
mod tests {
    //! Fixtures shaped like the succession the repair will actually run: the
    //! Registry moved, the Rent program did not, and one deployer key stands
    //! behind Core's Loader authority and both infrastructure upgrades.

    use dclutch_core_contract::ContentId;
    use dclutch_registry_contract::ArtifactUpgradePolicyV1;
    use dclutch_release_set_contract::{
        PROTOCOL_INFRASTRUCTURE_GENESIS_REGISTRY_ARTIFACT_V2,
        PROTOCOL_INFRASTRUCTURE_GENESIS_RENT_ARTIFACT_V2, ProgramIdentityV1,
    };

    use super::*;

    const CORE_SEED: u8 = 20;
    const REGISTRY_SEED: u8 = 30;
    const RENT_SEED: u8 = 40;
    const DEPLOYER_SEED: u8 = 50;
    /// Deliberately not the key the successor records bind, so a test that
    /// moves it proves consent followed the predecessor rather than a constant.
    const OTHER_DEPLOYER_SEED: u8 = 51;
    const PAYER_SEED: u8 = 90;

    fn seeded(seed: u8) -> Pubkey {
        Pubkey::new_from_array([seed; 32])
    }

    fn observation() -> Observation {
        Observation {
            slot: 491_018_122,
            unix_timestamp: 1_800_000_000,
            finality: Finality::Finalized,
        }
    }

    fn observed(
        key: Pubkey,
        owner: Pubkey,
        lamports: u64,
        executable: bool,
        data: Vec<u8>,
    ) -> ObservedAccount {
        ObservedAccount {
            observation: observation(),
            key,
            owner,
            lamports,
            executable,
            data,
        }
    }

    fn put(output: &mut [u8], offset: usize, source: &[u8]) {
        let Some(end) = offset.checked_add(source.len()) else {
            return;
        };
        let Some(destination) = output.get_mut(offset..end) else {
            return;
        };
        destination.copy_from_slice(source);
    }

    fn loader_program_bytes(programdata: Pubkey) -> Vec<u8> {
        let mut output = vec![0; 36];
        put(&mut output, 0, &2_u32.to_le_bytes());
        put(&mut output, 4, programdata.as_ref());
        output
    }

    /// Loader V3 ProgramData: variant, slot, then the 33-byte authority option.
    fn programdata_bytes(slot: u64, authority: Option<Pubkey>, elf: &[u8]) -> Vec<u8> {
        let mut output = vec![0; 45 + elf.len()];
        put(&mut output, 0, &3_u32.to_le_bytes());
        put(&mut output, 4, &slot.to_le_bytes());
        if let Some(authority) = authority {
            put(&mut output, 12, &[1]);
            put(&mut output, 13, authority.as_ref());
        }
        put(&mut output, 45, elf);
        output
    }

    fn rent_account(rent: &Rent) -> ObservedAccount {
        let mut lamports = 1;
        let mut data = vec![0; Rent::size_of()];
        let key = sysvar::rent::ID;
        let owner = sysvar::ID;
        let mut info =
            AccountInfo::new(&key, false, false, &mut lamports, &mut data, &owner, false);
        assert_eq!(rent.clone().to_account_info(&mut info), Some(()));
        observed(key, owner, 1, false, data)
    }

    /// One deployment and the release record that describes it.
    ///
    /// Holding `program_seed` fixed while `elf_seed` and `slot` move is the
    /// shape a succession arm has: one program identity on both sides of the
    /// profile, a different artifact release id.
    #[derive(Clone, Copy)]
    struct ReleaseSpec {
        program_seed: u8,
        elf_seed: u8,
        slot: u64,
        authority: Option<u8>,
    }

    impl ReleaseSpec {
        const fn at(program_seed: u8, elf_seed: u8, slot: u64) -> Self {
            Self {
                program_seed,
                elf_seed,
                slot,
                authority: Some(DEPLOYER_SEED),
            }
        }
    }

    struct Deployment {
        program: ObservedAccount,
        programdata: ObservedAccount,
        release: ArtifactReleaseV1,
    }

    fn deployment(spec: ReleaseSpec) -> Deployment {
        let program = seeded(spec.program_seed);
        let programdata =
            Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0;
        let elf = vec![spec.elf_seed; 96];
        let authority = spec.authority.map(seeded);
        let release = ArtifactReleaseV1::new(
            ProgramIdentityV1::new(program.to_bytes()).expect("program"),
            ProgramIdentityV1::new(bpf_loader_upgradeable::ID.to_bytes()).expect("loader"),
            programdata.to_bytes(),
            ContentId::new([spec.elf_seed; 32]).expect("semantic release"),
            hash(&elf).to_bytes(),
            spec.slot,
            match authority {
                Some(_) => ArtifactUpgradePolicyV1::ExactAuthority,
                None => ArtifactUpgradePolicyV1::Immutable,
            },
            authority.map(|key| key.to_bytes()),
        )
        .expect("release");
        Deployment {
            program: observed(
                program,
                bpf_loader_upgradeable::ID,
                1,
                true,
                loader_program_bytes(programdata),
            ),
            programdata: observed(
                programdata,
                bpf_loader_upgradeable::ID,
                1,
                false,
                programdata_bytes(spec.slot, authority, &elf),
            ),
            release,
        }
    }

    struct Record {
        raw: ObservedAccount,
        staging: ObservedAccount,
        binding: ExecutionRoleBindingV1,
    }

    fn record(registry: Pubkey, release: ArtifactReleaseV1, rent: &Rent) -> Record {
        let data = release.to_bytes().to_vec();
        let width = data.len();
        let digest = hash(&data).to_bytes();
        let raw = Pubkey::find_program_address(
            &[
                RAW_RECORD_PDA_SEED_V1,
                &ARTIFACT_RELEASE_SCHEMA_ID_V1,
                &digest,
            ],
            &registry,
        )
        .0;
        let staging = Pubkey::find_program_address(
            &[
                STAGING_CURSOR_PDA_SEED_V1,
                &ARTIFACT_RELEASE_SCHEMA_ID_V1,
                &digest,
            ],
            &registry,
        )
        .0;
        Record {
            raw: observed(raw, registry, rent.minimum_balance(width), false, data),
            staging: observed(staging, system_program::ID, 0, false, Vec::new()),
            binding: ExecutionRoleBindingV1::new(
                release.program(),
                ArtifactReleaseIdV1::new(digest).expect("artifact"),
            ),
        }
    }

    /// The five releases one succession snapshot is built out of.
    ///
    /// Predecessor releases are what V1 pinned; successor releases are what V2
    /// selects and what the observed deployments currently run. A binding whose
    /// two specs are identical did not move.
    #[derive(Clone, Copy)]
    struct Plan {
        core: ReleaseSpec,
        registry_predecessor: ReleaseSpec,
        registry_successor: ReleaseSpec,
        rent_predecessor: ReleaseSpec,
        rent_successor: ReleaseSpec,
    }

    fn plan() -> Plan {
        Plan {
            core: ReleaseSpec::at(CORE_SEED, 100, 490_000_000),
            registry_predecessor: ReleaseSpec::at(REGISTRY_SEED, 101, 490_697_000),
            registry_successor: ReleaseSpec::at(REGISTRY_SEED, 201, 490_849_793),
            rent_predecessor: ReleaseSpec::at(RENT_SEED, 104, 490_693_331),
            rent_successor: ReleaseSpec::at(RENT_SEED, 104, 490_693_331),
        }
    }

    struct Fixture {
        core_program: Pubkey,
        rent: Rent,
        state: CoreInfrastructureSuccessionStateV1,
        /// The V1-pinned Registry record, for tests that attach it by hand.
        predecessor_registry: PredecessorRecordObservationV1,
        v1_profile: ProtocolInfrastructureProfileV1,
        registry_binding: ExecutionRoleBindingV1,
        rent_binding: ExecutionRoleBindingV1,
    }

    impl Fixture {
        fn with(plan: Plan) -> Self {
            let core_program = seeded(CORE_SEED);
            let rent = Rent::default();
            let core = deployment(plan.core);
            let registry = deployment(plan.registry_successor);
            let rent_program = deployment(plan.rent_successor);
            // Every record lives under the CURRENT Registry, the Rent record
            // and both predecessor records included.
            let registry_key = registry.program.key;
            let registry_successor = record(registry_key, registry.release, &rent);
            let rent_successor = record(registry_key, rent_program.release, &rent);
            let registry_predecessor = record(
                registry_key,
                deployment(plan.registry_predecessor).release,
                &rent,
            );
            let rent_predecessor = record(
                registry_key,
                deployment(plan.rent_predecessor).release,
                &rent,
            );
            let v1_profile = ProtocolInfrastructureProfileV1::new(
                registry_predecessor.binding,
                rent_predecessor.binding,
            )
            .expect("V1 profile");
            let v1_key = Pubkey::find_program_address(
                &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1],
                &core_program,
            )
            .0;
            let v2_key = Pubkey::find_program_address(
                &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2],
                &core_program,
            )
            .0;
            let predecessor_registry = PredecessorRecordObservationV1 {
                raw: registry_predecessor.raw,
                staging: registry_predecessor.staging,
            };
            let predecessor_rent = PredecessorRecordObservationV1 {
                raw: rent_predecessor.raw,
                staging: rent_predecessor.staging,
            };
            // What the caller believes it holds, which the builder then checks
            // against the bytes rather than trusting.
            let registry_moved = registry_successor.binding.artifact_release()
                != registry_predecessor.binding.artifact_release();
            let rent_moved = rent_successor.binding.artifact_release()
                != rent_predecessor.binding.artifact_release();
            let state = CoreInfrastructureSuccessionStateV1 {
                payer: observed(
                    seeded(PAYER_SEED),
                    system_program::ID,
                    2_645_351_216,
                    false,
                    Vec::new(),
                ),
                profile: observed(v2_key, system_program::ID, 0, false, Vec::new()),
                predecessor_profile: observed(
                    v1_key,
                    core_program,
                    rent.minimum_balance(PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1),
                    false,
                    v1_profile.to_bytes().to_vec(),
                ),
                core_programdata: core.programdata,
                upgrade_authority: observed(
                    seeded(DEPLOYER_SEED),
                    system_program::ID,
                    1,
                    false,
                    Vec::new(),
                ),
                registry_artifact_raw: registry_successor.raw,
                registry_artifact_staging: registry_successor.staging,
                registry_program: registry.program,
                registry_programdata: registry.programdata,
                rent_artifact_raw: rent_successor.raw,
                rent_artifact_staging: rent_successor.staging,
                rent_program: rent_program.program,
                rent_programdata: rent_program.programdata,
                predecessor_registry_record: registry_moved.then(|| predecessor_registry.clone()),
                predecessor_rent_record: rent_moved.then_some(predecessor_rent),
                rent_sysvar: rent_account(&rent),
                system_program: observed(
                    system_program::ID,
                    native_loader::ID,
                    1,
                    true,
                    Vec::new(),
                ),
            };
            Self {
                core_program,
                rent,
                state,
                predecessor_registry,
                v1_profile,
                registry_binding: registry_successor.binding,
                rent_binding: rent_successor.binding,
            }
        }

        fn new() -> Self {
            Self::with(plan())
        }

        fn build(&self) -> Result<CoreInfrastructureSuccessionReportV1, Error> {
            build_core_infrastructure_succession_v1(self.core_program, &self.state)
        }

        fn report(&self) -> CoreInfrastructureSuccessionReportV1 {
            self.build().expect("succession")
        }

        /// Plant the genesis V2 this cohort was born with at the V2 domain.
        ///
        /// Exactly what `InitializeProtocolInfrastructureV1` leaves there since
        /// `c60b25e8`: the same two bindings V1 names, with the two genesis
        /// sentinels standing in for predecessor ids it has none of.
        fn born_at_v2(&mut self) -> ProtocolInfrastructureProfileV2 {
            let genesis = ProtocolInfrastructureProfileV2::genesis(
                self.v1_profile.registry(),
                self.v1_profile.rent(),
            )
            .expect("genesis V2");
            self.occupy(self.core_program, genesis.to_bytes().to_vec());
            genesis
        }

        /// Stand `data` at the V2 domain under `owner`, rent-exempt for its width.
        fn occupy(&mut self, owner: Pubkey, data: Vec<u8>) {
            self.state.profile.owner = owner;
            self.state.profile.lamports = self.rent.minimum_balance(data.len());
            self.state.profile.data = data;
        }
    }

    #[test]
    fn the_repair_shaped_succession_projects_one_signing_arm_and_one_system_arm() {
        let fixture = Fixture::new();
        let report = fixture.report();
        let state = &fixture.state;
        let deployer = seeded(DEPLOYER_SEED);

        assert_eq!(report.instruction.program_id, fixture.core_program);
        assert_eq!(
            report.instruction.accounts.len(),
            INITIALIZE_INFRASTRUCTURE_ACCOUNT_COUNT_V2
        );
        assert_eq!(
            report.instruction.data,
            InitializeProtocolInfrastructureV2.to_bytes()
        );

        // The frame, slot by slot. Exactly two accounts are writable — the
        // payer and the profile being created — and exactly three sign.
        let expected = [
            (state.payer.key, true, true),
            (report.profile, false, true),
            (state.predecessor_profile.key, false, false),
            (state.core_programdata.key, false, false),
            (state.upgrade_authority.key, true, false),
            (state.registry_artifact_raw.key, false, false),
            (state.registry_artifact_staging.key, false, false),
            (state.registry_program.key, false, false),
            (state.registry_programdata.key, false, false),
            (state.rent_artifact_raw.key, false, false),
            (state.rent_artifact_staging.key, false, false),
            (state.rent_program.key, false, false),
            (state.rent_programdata.key, false, false),
            (fixture.predecessor_registry.raw.key, false, false),
            (fixture.predecessor_registry.staging.key, false, false),
            (deployer, true, false),
            (system_program::ID, false, false),
            (system_program::ID, false, false),
            (system_program::ID, false, false),
            (sysvar::rent::ID, false, false),
            (system_program::ID, false, false),
        ];
        for (index, (key, signer, writable)) in expected.into_iter().enumerate() {
            let meta = report.instruction.accounts.get(index).expect("meta");
            assert_eq!(meta.pubkey, key, "slot {index} key");
            assert_eq!(meta.is_signer, signer, "slot {index} signer");
            assert_eq!(meta.is_writable, writable, "slot {index} writable");
        }

        // The moved binding asks the predecessor's deployer to consent; the
        // unmoved one stands the System program and must not sign. That
        // asymmetry is the whole of conjunct 5 in one frame.
        let [registry, rent_consent] = report.consent;
        assert_eq!(registry.binding, InfrastructureBindingV1::Registry);
        assert!(registry.moved && registry.must_sign);
        assert_eq!(registry.slot, deployer);
        assert_eq!(rent_consent.binding, InfrastructureBindingV1::Rent);
        assert!(!rent_consent.moved && !rent_consent.must_sign);
        assert_eq!(rent_consent.slot, system_program::ID);

        // One deployer behind Core and the Registry: two signatures, not three.
        assert_eq!(report.required_signers, vec![state.payer.key, deployer]);
        assert_eq!(
            report.profile_rent_debit_lamports,
            fixture
                .rent
                .minimum_balance(PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2)
        );
        assert_eq!(
            report.profile,
            Pubkey::find_program_address(
                &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2],
                &fixture.core_program,
            )
            .0
        );

        // The exact bytes that will land, walkable back to the V1 profile.
        assert_eq!(
            report.record.to_bytes().len(),
            PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2
        );
        assert_eq!(
            ProtocolInfrastructureProfileV2::decode(&report.record.to_bytes()),
            Ok(report.record)
        );
        assert_eq!(report.record.registry(), fixture.registry_binding);
        assert_eq!(report.record.rent(), fixture.rent_binding);
        assert_eq!(
            report.record.predecessor_registry_artifact(),
            fixture.v1_profile.registry().artifact_release()
        );
        // The unmoved binding carries the same artifact id on both sides.
        assert_eq!(
            report.record.predecessor_rent_artifact(),
            fixture.rent_binding.artifact_release()
        );
    }

    /// O-016's twin, mirrored: the consenting key is read out of the
    /// PREDECESSOR record, so it moves when that record moves and there is no
    /// caller input that could hold it still — nor any successor fact that
    /// could substitute for it.
    #[test]
    fn the_consenting_key_is_read_from_the_predecessor_record_alone() {
        let mut plan = plan();
        plan.registry_predecessor.authority = Some(OTHER_DEPLOYER_SEED);
        let fixture = Fixture::with(plan);
        let report = fixture.report();
        let other = seeded(OTHER_DEPLOYER_SEED);

        let [registry, _] = report.consent;
        assert_eq!(registry.slot, other);
        assert_ne!(registry.slot, seeded(DEPLOYER_SEED));
        // The successor record still binds the ordinary deployer, and Core's
        // Loader authority is still that key: three signatures, not two.
        assert_eq!(
            report.required_signers,
            vec![fixture.state.payer.key, seeded(DEPLOYER_SEED), other]
        );
        let slot = report
            .instruction
            .accounts
            .get(REGISTRY_CONSENT_ACCOUNT_V2)
            .expect("consent slot");
        assert_eq!(slot.pubkey, other);
        assert!(slot.is_signer && !slot.is_writable);
    }

    /// Moved-ness is derived from the bytes; a caller that believes otherwise
    /// is told which binding it is wrong about rather than silently corrected.
    #[test]
    fn a_caller_whose_belief_disagrees_with_the_bytes_refuses_both_ways() {
        let mut fixture = Fixture::new();
        fixture.state.predecessor_registry_record = None;
        assert_eq!(
            fixture.build(),
            Err(Error::MovedBindingWithoutPredecessorRecord)
        );

        let mut fixture = Fixture::new();
        fixture.state.predecessor_rent_record = Some(fixture.predecessor_registry.clone());
        assert_eq!(
            fixture.build(),
            Err(Error::UnmovedBindingWithPredecessorRecord)
        );
    }

    #[test]
    fn a_succession_that_moves_nothing_refuses_before_it_spends_the_vacancy() {
        let mut plan = plan();
        plan.registry_successor = plan.registry_predecessor;
        let fixture = Fixture::with(plan);
        assert!(fixture.state.predecessor_registry_record.is_none());
        assert_eq!(fixture.build(), Err(Error::NothingMoved));
    }

    #[test]
    fn a_binding_whose_program_identity_moved_refuses() {
        let mut plan = plan();
        plan.registry_successor.program_seed = 99;
        let fixture = Fixture::with(plan);
        assert_eq!(fixture.build(), Err(Error::IdentityMoved));
    }

    #[test]
    fn a_moved_binding_whose_slot_did_not_advance_refuses() {
        let mut plan = plan();
        // New bytes, older slot: an upgrade that ran backwards.
        plan.registry_successor.slot = 1;
        let fixture = Fixture::with(plan);
        assert_eq!(fixture.build(), Err(Error::NotForward));
    }

    /// An `Immutable` predecessor binds no authority and its bytes could not
    /// have moved, so a moved claim against it is a contradiction.
    #[test]
    fn a_moved_binding_whose_predecessor_binds_no_authority_refuses() {
        let mut plan = plan();
        plan.registry_predecessor.authority = None;
        let fixture = Fixture::with(plan);
        assert_eq!(fixture.build(), Err(Error::ConsentAuthorityMissing));
    }

    /// The record digest IS the artifact-release id, so a substituted
    /// predecessor record is a different account rather than a different
    /// opinion — including a perfectly valid record of the wrong release.
    #[test]
    fn a_substituted_or_unowned_predecessor_record_refuses() {
        let mut fixture = Fixture::new();
        fixture.state.predecessor_registry_record = Some(PredecessorRecordObservationV1 {
            raw: fixture.state.registry_artifact_raw.clone(),
            staging: fixture.state.registry_artifact_staging.clone(),
        });
        assert_eq!(fixture.build(), Err(Error::InvalidPredecessorRecord));

        let mut fixture = Fixture::new();
        if let Some(record) = fixture.state.predecessor_registry_record.as_mut() {
            record.raw.owner = seeded(66);
        }
        assert_eq!(fixture.build(), Err(Error::InvalidPredecessorRecord));
    }

    #[test]
    fn a_successor_record_that_is_not_the_registrys_own_refuses() {
        let mut fixture = Fixture::new();
        fixture.state.registry_artifact_raw.owner = seeded(66);
        assert_eq!(fixture.build(), Err(Error::InvalidSuccessorRecord));

        let mut fixture = Fixture::new();
        fixture.state.rent_artifact_staging.data = vec![0; 1];
        assert_eq!(fixture.build(), Err(Error::InvalidSuccessorRecord));

        // Bytes of the wrong width never reach a digest comparison at all.
        let mut fixture = Fixture::new();
        fixture.state.registry_artifact_raw.data.truncate(8);
        assert_eq!(fixture.build(), Err(Error::InvalidSuccessorRecord));
    }

    #[test]
    fn a_deployment_that_has_moved_under_its_own_record_refuses() {
        let mut fixture = Fixture::new();
        put(
            &mut fixture.state.registry_programdata.data,
            4,
            &999_u64.to_le_bytes(),
        );
        assert_eq!(fixture.build(), Err(Error::InvalidDeployment));
    }

    /// Conjunct 1's aliasing arm: the reader may not select itself.
    #[test]
    fn an_infrastructure_binding_that_names_core_refuses() {
        let mut plan = plan();
        plan.rent_successor = plan.core;
        let fixture = Fixture::with(plan);
        assert_eq!(fixture.build(), Err(Error::InfrastructureProgramIsCore));
    }

    #[test]
    fn a_predecessor_profile_that_is_absent_or_undecodable_refuses() {
        let mut fixture = Fixture::new();
        fixture.state.predecessor_profile.owner = seeded(67);
        assert_eq!(fixture.build(), Err(Error::PredecessorProfileAbsent));

        let mut fixture = Fixture::new();
        fixture.state.predecessor_profile.data.truncate(143);
        assert_eq!(fixture.build(), Err(Error::PredecessorProfileAbsent));

        let mut fixture = Fixture::new();
        put(&mut fixture.state.predecessor_profile.data, 0, &[0xff]);
        assert_eq!(fixture.build(), Err(Error::PredecessorProfileAbsent));
    }

    #[test]
    fn a_core_programdata_that_binds_another_key_refuses() {
        let mut fixture = Fixture::new();
        fixture.state.upgrade_authority.key = seeded(77);
        assert_eq!(fixture.build(), Err(Error::InvalidCoreUpgradeAuthority));

        let mut fixture = Fixture::new();
        fixture.state.core_programdata.key = seeded(78);
        assert_eq!(fixture.build(), Err(Error::InvalidCoreUpgradeAuthority));
    }

    #[test]
    fn a_second_ceremony_finds_the_domain_already_succeeded() {
        let mut fixture = Fixture::new();
        fixture.state.profile.owner = fixture.core_program;
        fixture.state.profile.lamports = fixture
            .rent
            .minimum_balance(PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2);
        fixture.state.profile.data = vec![0; PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2];
        assert_eq!(fixture.build(), Err(Error::AlreadySucceeded));
    }

    /// The regression `c60b25e8` left behind, and the reason it was invisible:
    /// the route changed conjunct 6 and this restatement did not, so the
    /// builder refused — before it built anything — the first succession of
    /// every cohort born at V2, which since that commit is every cohort. The
    /// chain would have taken it.
    #[test]
    fn the_genesis_v2_a_cohort_is_born_with_is_superseded_not_refused() {
        let vacant = Fixture::new().report();

        let mut fixture = Fixture::new();
        let genesis = fixture.born_at_v2();
        assert!(genesis.born_at_v2());
        let report = fixture.report();

        assert_eq!(
            report.profile_standing,
            SuccessionProfileStandingV1::BornAtV2
        );
        // The account already exists, Core-owned, at the exact width and
        // rent-exempt, so `create_profile_v2` overwrites it and transfers
        // nothing. A forecast debit here would be a transfer that never happens.
        assert_eq!(report.profile_rent_debit_lamports, 0);
        assert_ne!(vacant.profile_rent_debit_lamports, 0);

        // The standing changes the debit and nothing else. Same address, same
        // frame, same bytes — and those bytes name the two REAL predecessors
        // read out of the live V1, never a sentinel, which is the whole of the
        // soundness argument for reading the ids instead of the vacancy.
        assert_eq!(report.profile, vacant.profile);
        assert_eq!(report.instruction, vacant.instruction);
        assert_eq!(report.record, vacant.record);
        assert!(!report.record.born_at_v2());
        assert_eq!(
            report.record.predecessor_registry_artifact(),
            fixture.v1_profile.registry().artifact_release()
        );
    }

    /// One succession per domain, ever — and an account this ceremony cannot
    /// read is never room to write.
    ///
    /// Each arm stands something at the exact derived address that a decode
    /// alone might have accepted: a genesis profile belonging to a FOREIGN
    /// Core, an executable, the predecessor V1 width, and a profile that has
    /// already spent its succession. All four are the no-fork refusal, and none
    /// of them reaches the encoder.
    #[test]
    fn a_v2_domain_this_ceremony_cannot_claim_refuses_as_succeeded() {
        let genesis = {
            let mut fixture = Fixture::new();
            fixture.born_at_v2().to_bytes().to_vec()
        };

        // A perfectly well-formed genesis V2 under someone else's Core.
        let mut fixture = Fixture::new();
        fixture.occupy(seeded(211), genesis.clone());
        assert_eq!(fixture.build(), Err(Error::AlreadySucceeded));

        // Core-owned, right width, right bytes, but executable.
        let mut fixture = Fixture::new();
        fixture.occupy(fixture.core_program, genesis.clone());
        fixture.state.profile.executable = true;
        assert_eq!(fixture.build(), Err(Error::AlreadySucceeded));

        // Core-owned but the predecessor V1 width, which decodes as nothing.
        let mut fixture = Fixture::new();
        let narrow = genesis[..PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1].to_vec();
        fixture.occupy(fixture.core_program, narrow);
        assert_eq!(fixture.build(), Err(Error::AlreadySucceeded));

        // Exact width and owner, one byte of magic wrong: undecodable.
        let mut fixture = Fixture::new();
        let mut corrupt = genesis.clone();
        put(&mut corrupt, 0, &[0xff]);
        fixture.occupy(fixture.core_program, corrupt);
        assert_eq!(fixture.build(), Err(Error::AlreadySucceeded));

        // The one this rule exists to still refuse: a profile naming two real
        // predecessor artifact releases has spent its succession.
        let mut fixture = Fixture::new();
        let spent = ProtocolInfrastructureProfileV2::new(
            fixture.registry_binding,
            fixture.rent_binding,
            fixture.v1_profile.registry().artifact_release(),
            fixture.v1_profile.rent().artifact_release(),
        )
        .expect("spent V2");
        assert!(!spent.born_at_v2());
        fixture.occupy(fixture.core_program, spent.to_bytes().to_vec());
        assert_eq!(fixture.build(), Err(Error::AlreadySucceeded));
    }

    /// Half a forgery is still a forgery.
    ///
    /// One sentinel and one real predecessor id is a shape neither writer can
    /// produce — genesis writes both sentinels, the ceremony writes two ids
    /// read out of the live V1 — so it must not buy a second succession. The
    /// route requires BOTH; so does this side.
    #[test]
    fn a_half_sentinel_profile_does_not_buy_a_second_succession() {
        let genesis_registry =
            ArtifactReleaseIdV1::new(PROTOCOL_INFRASTRUCTURE_GENESIS_REGISTRY_ARTIFACT_V2)
                .expect("sentinel");
        let genesis_rent = ArtifactReleaseIdV1::new(PROTOCOL_INFRASTRUCTURE_GENESIS_RENT_ARTIFACT_V2)
            .expect("sentinel");

        for (registry_predecessor, rent_predecessor) in [(true, false), (false, true)] {
            let mut fixture = Fixture::new();
            let forged = ProtocolInfrastructureProfileV2::new(
                fixture.v1_profile.registry(),
                fixture.v1_profile.rent(),
                if registry_predecessor {
                    fixture.v1_profile.registry().artifact_release()
                } else {
                    genesis_registry
                },
                if rent_predecessor {
                    fixture.v1_profile.rent().artifact_release()
                } else {
                    genesis_rent
                },
            )
            .expect("half-sentinel V2");
            assert!(!forged.born_at_v2());
            fixture.occupy(fixture.core_program, forged.to_bytes().to_vec());
            assert_eq!(fixture.build(), Err(Error::AlreadySucceeded));
        }
    }

    #[test]
    fn a_profile_account_at_any_other_address_refuses() {
        let mut fixture = Fixture::new();
        fixture.state.profile.key = seeded(200);
        assert_eq!(fixture.build(), Err(Error::InvalidProfileAddress));
    }

    /// Lamports already sitting on the vacant address are the domain's, not the
    /// payer's problem: the route tops up rather than funding outright.
    #[test]
    fn dust_on_the_vacant_profile_reduces_the_debit_it_does_not_refuse() {
        let mut fixture = Fixture::new();
        fixture.state.profile.lamports = 7;
        let report = fixture.report();
        assert_eq!(
            report.profile_rent_debit_lamports,
            fixture
                .rent
                .minimum_balance(PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2)
                - 7
        );
    }

    #[test]
    fn a_payer_that_is_not_a_usable_wallet_or_cannot_cover_the_profile_refuses() {
        let mut fixture = Fixture::new();
        fixture.state.payer.owner = seeded(68);
        assert_eq!(fixture.build(), Err(Error::InvalidPayer));

        let mut fixture = Fixture::new();
        fixture.state.payer.lamports = 1;
        assert_eq!(fixture.build(), Err(Error::InsufficientPayer));
    }

    #[test]
    fn noncanonical_runtime_plumbing_refuses() {
        let mut fixture = Fixture::new();
        fixture.state.system_program.owner = seeded(69);
        assert_eq!(fixture.build(), Err(Error::InvalidRuntimePlumbing));

        let mut fixture = Fixture::new();
        fixture.state.rent_sysvar.data.truncate(1);
        assert_eq!(fixture.build(), Err(Error::InvalidRuntimePlumbing));
    }

    /// The deployer key may fund nothing here. A consent slot cannot be
    /// writable, the payer's slot must be, and one key in one message carries
    /// the union of its privileges — so the fee payment goes to another wallet.
    #[test]
    fn a_consent_key_that_is_also_the_payer_refuses() {
        let mut fixture = Fixture::new();
        fixture.state.payer.key = seeded(DEPLOYER_SEED);
        fixture.state.upgrade_authority = fixture.state.payer.clone();
        assert_eq!(fixture.build(), Err(Error::ConsentAuthorityIsPayer));
    }

    /// Sharing the deployer key between the payer slot and Core's upgrade
    /// authority is exactly what a one-key cluster does, and the frame permits
    /// it: both are natural-person slots.
    #[test]
    fn the_payer_may_stand_in_cores_upgrade_authority_slot() {
        let mut plan = plan();
        plan.core.authority = Some(PAYER_SEED);
        let mut fixture = Fixture::with(plan);
        fixture.state.upgrade_authority = fixture.state.payer.clone();
        let report = fixture.report();
        assert_eq!(
            report.required_signers,
            vec![fixture.state.payer.key, seeded(DEPLOYER_SEED)]
        );
    }

    /// A consent key that lands on a slot the frame keeps distinct is refused
    /// before a frame is built, not discovered by the runtime.
    #[test]
    fn a_consent_key_aliasing_a_non_person_slot_refuses() {
        let mut plan = plan();
        plan.registry_predecessor.authority = Some(REGISTRY_SEED);
        let fixture = Fixture::with(plan);
        assert_eq!(fixture.build(), Err(Error::AliasedFrameAccount));
    }

    #[test]
    fn an_unfinalized_split_or_inconsistent_observation_refuses_first() {
        let mut fixture = Fixture::new();
        fixture.state.registry_program.observation.finality = Finality::Confirmed;
        assert_eq!(fixture.build(), Err(Error::ObservationNotFinalized));

        let mut fixture = Fixture::new();
        fixture.state.registry_program.observation.slot = 491_018_123;
        assert_eq!(fixture.build(), Err(Error::ObservationMismatch));

        // One key, two stories: the snapshot contradicts itself.
        let mut fixture = Fixture::new();
        fixture.state.upgrade_authority.key = fixture.state.payer.key;
        assert_eq!(fixture.build(), Err(Error::InconsistentAlias));
    }

    /// The profile carries no clock, so a succession composed months after the
    /// upgrade it records composes to exactly the bytes it would have then.
    #[test]
    fn a_succession_composed_late_writes_the_same_bytes_as_a_timely_one() {
        let fixture = Fixture::new();
        let timely = fixture.report();

        let mut later = Fixture::new();
        {
            let state = &mut later.state;
            let mut accounts = vec![
                &mut state.payer,
                &mut state.profile,
                &mut state.predecessor_profile,
                &mut state.core_programdata,
                &mut state.upgrade_authority,
                &mut state.registry_artifact_raw,
                &mut state.registry_artifact_staging,
                &mut state.registry_program,
                &mut state.registry_programdata,
                &mut state.rent_artifact_raw,
                &mut state.rent_artifact_staging,
                &mut state.rent_program,
                &mut state.rent_programdata,
                &mut state.rent_sysvar,
                &mut state.system_program,
            ];
            for record in [
                &mut state.predecessor_registry_record,
                &mut state.predecessor_rent_record,
            ] {
                if let Some(record) = record.as_mut() {
                    accounts.extend([&mut record.raw, &mut record.staging]);
                }
            }
            for account in accounts {
                account.observation.slot = 512_000_000;
                account.observation.unix_timestamp = 1_805_000_000;
            }
        }
        let late = later.report();

        assert_eq!(timely.record.to_bytes(), late.record.to_bytes());
        assert_eq!(timely.instruction, late.instruction);
    }
}
