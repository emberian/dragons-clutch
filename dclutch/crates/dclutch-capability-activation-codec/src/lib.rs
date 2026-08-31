#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Family-neutral capability-activation artifact construction.
//!
//! # The wall this closes
//!
//! Activation is the one Core-signed action that CREATES a family's capability
//! root, and there is no other. `programs/dclutch-trading-sbf/src/outer.rs`
//! (`authenticate_set_descriptor`) selects the activation descriptor out of the
//! release's `CapabilityProgramSetV2` and requires it to be stamped
//! [`CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1`]; every hot-action entry every
//! family builds is stamped `v4::SCHEMA_RELEASE_ID` instead. A release that is
//! all-V4 therefore refuses activation with `UnsupportedContent` before any
//! width or effect check is reached, and its markets are foundable, admittable,
//! and permanently untradeable.
//!
//! That demand is not a seam defect and relaxing it would be a false fix. It is
//! what lets the activation outer stay family-neutral without acquiring a second
//! descriptor decoder, and a V4 action descriptor could not activate anything
//! even if it decoded: its effect performs the action's work, not the root's
//! creation, so it would refuse one statement later at
//! `effect.request_bytes() == descriptor.root_state_bytes()`. **The wall is a
//! missing artifact, one per family**: a V1 descriptor, an `AccountProfileV1`,
//! and an `EffectProgramV2` whose projected request buffer IS the family's exact
//! initial root tail.
//!
//! # Why the gate is in the constructor
//!
//! A wrong activation effect bricks every root it activates, permanently and
//! silently. The outer writes `CapabilityRootHeaderV1 || <projected request>`
//! and never decodes the family tail — it owns no family decoder by design — so
//! a tail of the right WIDTH and the wrong CONTENT is admitted and is
//! undecodable forever. Four hand-written bundles are four chances to do that.
//!
//! Here the family declares where its tail's bytes come from and nothing about
//! the artifacts, and [`build_activation_bundle_v1`] **runs the real effect
//! kernel over the effect it has just built and refuses to return a bundle whose
//! projected request buffer is not that tail, byte for byte, for probe seam
//! inputs no plausible mistake could coincide with.** The brick gate is a
//! precondition of obtaining a bundle at all, not a test a family may forget to
//! write.
//!
//! # What a family must be able to say, and when it cannot
//!
//! Every byte of an initial root tail must be either a **constant** the family
//! publishes, or a **seam-seeded register** the outer fills in before any
//! artifact runs (`activation_registers_v2`). There is no third source: the
//! effect kernel has no arithmetic and the activation frame holds only the root
//! and the funding ledgers.
//!
//! - Direct's 24-byte tail is entirely constant (magic, header word, a zero
//!   counter).
//! - General's 128-byte tail is constants PLUS the Market, the config identity
//!   and the generation — three [`ActivationTailFieldV1`]s reading identity and
//!   scalar registers 4, 8 and 1.
//! - Fractional's root cannot be composed at all: its PDA bump is derived after
//!   the effect runs, its `terms` digest covers Market-carrying bytes the config
//!   is deliberately free of, and its rent beneficiary has no author. That is an
//!   impossibility, and this module makes a family discover it here rather than
//!   on a root.
//!
//! # What varies by family, and what does not
//!
//! Family-varying, all of it in [`ActivationBundleInputV1`]: the constant tail,
//! the seam-supplied fields, the declared root width, the activation request
//! schema, the funding ledger's provisioned row count, and the five coordinates
//! inherited from the family's own action descriptor.
//!
//! Family-invariant, and fixed here: the two-account profile (the vacant root at
//! `ACTIVATION_ROOT_ACCOUNT_V2`, the selected Trading `FundingLedgerV2` at
//! `ACTIVATION_FIRST_FUNDING_ACCOUNT_V2`), the three profile operations, the
//! rent transfer, the register banks, the ascending write order, and the three
//! finalized schemas.
//!
//! The evidence that this is the REVIEWED shape rather than a fourth opinion of
//! it is a test, not a claim, and it lives in the crate that owns the reviewed
//! artifact: `dclutch-direct-codec`'s `activation_bundle_v1::tests::
//! the_family_neutral_template_reproduces_this_sealed_bundle_byte_for_byte`
//! rebuilds Direct's sealed triple out of this module and compares all four
//! records and all three digests. If it goes red the template has drifted from
//! the thing that was reviewed, and no family may use it until that is explained.

extern crate alloc;

use alloc::{vec, vec::Vec};

use dclutch_account_profile_contract::{
    ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V1, AccountProfileV1,
    encode_v1::{
        AccountAliasInputV1, AccountEffectPermissionsV1, AccountOperationInputV1,
        AccountPrivilegesV1, AccountRuleInputV1, RegisterGeometryV1, account_profile_v1_bytes,
        encode_account_profile_v1_atomic,
    },
};
use dclutch_capability_contract::{
    FundingCompartment, funding_ledger_bytes_v2, funding_ledger_remaining_offset_v2,
};
use dclutch_capability_program_contract::{
    CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1, CapabilityProgramV1,
    activation_registers_v2::{
        ACTIVATION_COMMON_IDENTITIES_V2, ACTIVATION_COMMON_SCALARS_V2,
        ACTIVATION_FIRST_FAMILY_SCALAR_V2, ACTIVATION_FIRST_FUNDING_ACCOUNT_V2,
        ACTIVATION_MAX_ROLE_REQUEST_BYTES_V2, ACTIVATION_MAX_RUNTIME_IDENTITIES_V2,
        ACTIVATION_MAX_RUNTIME_SCALARS_V2, ACTIVATION_ROOT_ACCOUNT_V2, ACTIVATION_ROOT_IDENTITY_V2,
        ACTIVATION_TRADING_PROGRAM_IDENTITY_V2,
    },
    encode_v1::{
        CapabilityProgramInputV1, capability_program_v1_bytes, encode_capability_program_v1_atomic,
    },
};
use dclutch_core_contract::ContentId;
use dclutch_effect_kernel::v2::{
    AccountInput, AccountPermission, ProgramV2 as EffectProgramV2,
    SCHEMA_RELEASE_ID as EFFECT_PROGRAM_SCHEMA_ID_V2,
    encode::{
        EffectGeometryV2, EffectInstructionV2, effect_program_v2_bytes,
        encode_effect_program_v2_atomic,
    },
    project_with_aliases_and_requests_atomic,
};
use dclutch_sha256_adapter::digest;
use dclutch_transition_vm::v2::{
    ProgramV2 as TransitionProgramV2, RegisterInput, RegisterOutput,
    encode::{
        RegisterGeometryV2 as TransitionRegisterGeometryV2, TransitionInstructionV2,
        encode_transition_program_v2_atomic, transition_program_v2_bytes,
    },
    execute_atomic,
};

/// The vacant composite root and the one selected Trading `FundingLedgerV2`.
///
/// The seam requires `effect.account_count() == 1 + funding_count`, so this
/// template is the single-selected-ledger shape every founding provisions
/// today. A release whose activation must debit two ledgers needs its own
/// template rather than a wider parameter here: "which ledger holds the root
/// quote" would stop being derivable from the profile alone.
pub const ACTIVATION_ACCOUNT_COUNT_V1: u16 = 2;

/// Scalar holding the rent quote projected out of the funding ledger.
///
/// The first family-owned scalar. The bank below it is seam-seeded, and a family
/// artifact that writes there clobbers what the outer relies on downstream.
pub const ACTIVATION_RENT_QUOTE_SCALAR_V1: u16 = ACTIVATION_FIRST_FAMILY_SCALAR_V2;

/// First scalar holding a constant root-tail word loaded by the transition.
pub const ACTIVATION_FIRST_CONSTANT_SCALAR_V1: u16 = ACTIVATION_RENT_QUOTE_SCALAR_V1 + 1;

/// Exact byte width of one composed root-tail scalar word.
pub const ACTIVATION_TAIL_WORD_BYTES_V1: usize = 8;

/// Exact byte width of one composed root-tail identity.
pub const ACTIVATION_TAIL_IDENTITY_BYTES_V1: usize = 32;

/// One region of the initial root tail the seam supplies at runtime.
///
/// A field's register must live in the seam-seeded common bank. A family-owned
/// register would hold whatever this bundle's own transition put there, and this
/// transition loads only constants — so such a field would compose a silent zero
/// into the root and be refused here rather than discovered on chain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivationTailFieldV1 {
    /// Eight little-endian bytes read from a seam-seeded scalar register.
    SeamScalar {
        /// Byte offset within the family root tail.
        offset: u32,
        /// Seam-seeded scalar coordinate, below `ACTIVATION_COMMON_SCALARS_V2`.
        register: u16,
    },
    /// Thirty-two bytes read from a seam-seeded identity register.
    SeamIdentity {
        /// Byte offset within the family root tail.
        offset: u32,
        /// Seam-seeded identity coordinate, below
        /// `ACTIVATION_COMMON_IDENTITIES_V2`.
        register: u16,
    },
}

impl ActivationTailFieldV1 {
    /// Byte offset within the family root tail.
    #[must_use]
    pub const fn offset(self) -> u32 {
        match self {
            Self::SeamScalar { offset, .. } | Self::SeamIdentity { offset, .. } => offset,
        }
    }

    /// Exact byte width this field composes.
    #[must_use]
    pub const fn width(self) -> usize {
        match self {
            Self::SeamScalar { .. } => ACTIVATION_TAIL_WORD_BYTES_V1,
            Self::SeamIdentity { .. } => ACTIVATION_TAIL_IDENTITY_BYTES_V1,
        }
    }
}

/// Chain-selected release facts an activation descriptor inherits, plus the
/// family-owned facts no other artifact can supply.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActivationBundleInputV1<'a> {
    /// Capability kind the manifest entry selected, as the family's own action
    /// descriptor states it.
    pub kind: ContentId,
    /// Finalized config-record schema the action descriptor states.
    pub config_schema: ContentId,
    /// Family request schema this activation descriptor interprets.
    ///
    /// The one descriptor coordinate activation does not inherit: the activation
    /// selector request is its own grammar, and reusing the action request
    /// schema would let an ordinary action request select the activation
    /// descriptor.
    pub request_schema: ContentId,
    /// Mutable child-root tail schema the action descriptor states.
    pub root_schema: ContentId,
    /// Manifest-selected child derivation policy the action descriptor states.
    pub derivation_policy: ContentId,
    /// Manifest-selected capacity profile the action descriptor states.
    pub capacity_profile: ContentId,
    /// Exact root width the action descriptor declares.
    ///
    /// Checked against `constant_root_tail.len()`; a family whose tail constant
    /// has drifted from its descriptor refuses here rather than composing a
    /// short root the seam would refuse at its own width check.
    pub root_state_bytes: u32,
    /// The family's canonical initial root tail with every seam-supplied region
    /// zeroed.
    ///
    /// Derive it from `<Family>Root::…` and zero the seam regions
    /// programmatically; never write a literal. Every nonzero eight-byte word
    /// becomes one transition constant and one request write. Every zero word is
    /// left to the zero-initialised request buffer, which is already its initial
    /// value.
    pub constant_root_tail: &'a [u8],
    /// Regions the seam supplies, strictly ascending and non-overlapping.
    pub seam_fields: &'a [ActivationTailFieldV1],
    /// Compartment rows the founding provisions in the selected funding ledger.
    pub funding_ledger_slot_count: u16,
}

/// Three finalized activation records and their exact identities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActivationBundleV1 {
    /// Exact two-account `AccountProfileV1` record.
    pub account_profile: Vec<u8>,
    /// Embedded TransitionVM V2 bytes, published for audit evidence.
    pub transition: Vec<u8>,
    /// Exact request-composing `EffectProgramV2` record.
    pub effect: Vec<u8>,
    /// Exact `CapabilityProgramV1` activation descriptor record.
    pub descriptor: Vec<u8>,
    /// SHA-256 identity of `account_profile`.
    pub account_profile_id: [u8; 32],
    /// SHA-256 identity of `effect`.
    pub effect_id: [u8; 32],
    /// SHA-256 identity of `descriptor`.
    pub descriptor_id: [u8; 32],
}

/// Stable activation construction or validation refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivationBundleErrorV1 {
    /// The declared root width was zero, exceeded the seam's role-request bound,
    /// or disagreed with the supplied constant tail.
    RootWidth,
    /// A nonzero constant byte fell where no aligned write can place it.
    TailAlignment,
    /// A seam field left the tail, overlapped another, or was out of order.
    TailFieldGeometry,
    /// A seam field named a register outside the seam-seeded common bank, which
    /// would compose a silent zero into the root.
    TailFieldRegisterOutOfBank,
    /// A seam field's region carried nonzero constant bytes, so the tail
    /// declared two different sources for the same byte.
    TailFieldOverwritesConstant,
    /// The composed register geometry exceeded the seam's runtime bank.
    RegisterGeometry,
    /// AccountProfile construction, geometry, or exact content refused.
    AccountProfile,
    /// Transition construction, geometry, or exact content refused.
    Transition,
    /// Effect construction, geometry, or exact content refused.
    Effect,
    /// Descriptor construction, inherited coordinates, or exact content refused.
    Descriptor,
    /// A fixed width or content identity was invalid.
    Geometry,
    /// The real effect kernel refused to run the built effect at all.
    Projection,
    /// **The brick gate.** The real effect kernel ran and its projected request
    /// buffer was not the tail the input declares, byte for byte. A bundle that
    /// reaches this would have created an undecodable root.
    ProjectedTailMismatch,
    /// The real effect kernel did not leave the vacant root holding exactly the
    /// ledger's parked rent quote.
    ProjectedRentMismatch,
}

/// Result alias for family-neutral activation construction.
pub type ActivationResultV1<T> = core::result::Result<T, ActivationBundleErrorV1>;

/// The register image the seam presents to an activation effect.
///
/// Published so a family can ask [`project_activation_root_tail_v1`] what its
/// root will contain for a given Market, config and generation, and decode the
/// answer with its own decoder. That is how an activation is reviewed: by
/// reading its OUTPUT, not its instruction list.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActivationSeamImageV1<'a> {
    /// Seam-seeded scalar bank, exactly `ACTIVATION_COMMON_SCALARS_V2` wide.
    pub scalars: &'a [u64],
    /// Seam-seeded identity bank, exactly `ACTIVATION_COMMON_IDENTITIES_V2`
    /// wide.
    pub identities: &'a [[u8; 32]],
    /// Lamports the founding parked in the ledger's Rent compartment.
    pub rent_quote: u64,
}

/// Schema used to finalize an activation `AccountProfile` record.
#[must_use]
pub const fn activation_account_profile_schema_v1() -> [u8; 32] {
    ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V1
}

/// Schema used to finalize an activation `EffectProgram` record.
#[must_use]
pub const fn activation_effect_schema_v1() -> [u8; 32] {
    EFFECT_PROGRAM_SCHEMA_ID_V2
}

/// Schema used to finalize an activation descriptor record.
///
/// The constant the whole wall is about: the value a family's
/// `CapabilityProgramSetV2` activation entry must carry in its descriptor schema
/// field, where every action entry carries `v4::SCHEMA_RELEASE_ID`.
#[must_use]
pub const fn activation_descriptor_schema_v1() -> [u8; 32] {
    CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1
}

/// Build one family's canonical activation descriptor/profile/effect bundle.
///
/// Returns only after the real effect kernel has run over the built effect and
/// agreed, byte for byte, that its projected request buffer is the declared
/// tail under probe seam inputs.
pub fn build_activation_bundle_v1(
    input: ActivationBundleInputV1<'_>,
) -> ActivationResultV1<ActivationBundleV1> {
    let plan = CompositionPlanV1::derive(input)?;
    let account_profile = build_account_profile(input, plan.geometry)?;
    let transition = build_transition(&plan)?;
    let effect = build_effect(input, &plan)?;
    let account_profile_id = digest(&account_profile);
    let effect_id = digest(&effect);
    let descriptor_width = capability_program_v1_bytes(transition.len())
        .map_err(|_| ActivationBundleErrorV1::Descriptor)?;
    let mut scratch = vec![0_u8; descriptor_width];
    let mut descriptor = vec![0_u8; descriptor_width];
    encode_capability_program_v1_atomic(
        CapabilityProgramInputV1 {
            kind: input.kind,
            config_schema: input.config_schema,
            request_schema: input.request_schema,
            root_schema: input.root_schema,
            account_profile: content(account_profile_id)?,
            derivation_policy: input.derivation_policy,
            capacity_profile: input.capacity_profile,
            effect_schema: content(effect_id)?,
            root_state_bytes: input.root_state_bytes,
            transition_program: &transition,
        },
        &mut scratch,
        &mut descriptor,
    )
    .map_err(|_| ActivationBundleErrorV1::Descriptor)?;
    let bundle = ActivationBundleV1 {
        account_profile,
        transition,
        effect,
        descriptor_id: digest(&descriptor),
        descriptor,
        account_profile_id,
        effect_id,
    };
    validate_activation_bundle_v1(&bundle, input)?;
    Ok(bundle)
}

/// Hostile-decode one activation bundle and rejoin it to its release facts.
///
/// Rebuilds all three records from `input` and requires exact equality, decodes
/// each under its own hostile decoder, requires the descriptor's ten
/// coordinates, and re-runs the brick gate. A bundle that passes this is one
/// this module would have produced.
pub fn validate_activation_bundle_v1(
    bundle: &ActivationBundleV1,
    input: ActivationBundleInputV1<'_>,
) -> ActivationResultV1<()> {
    let plan = CompositionPlanV1::derive(input)?;
    if bundle.account_profile_id != digest(&bundle.account_profile)
        || bundle.effect_id != digest(&bundle.effect)
        || bundle.descriptor_id != digest(&bundle.descriptor)
    {
        return Err(ActivationBundleErrorV1::Descriptor);
    }
    if bundle.account_profile != build_account_profile(input, plan.geometry)? {
        return Err(ActivationBundleErrorV1::AccountProfile);
    }
    let profile = AccountProfileV1::decode_selected(
        bundle.account_profile_id,
        digest(&bundle.account_profile),
        &bundle.account_profile,
    )
    .map_err(|_| ActivationBundleErrorV1::AccountProfile)?;
    if profile.account_count() != ACTIVATION_ACCOUNT_COUNT_V1
        || profile.scalar_count() != plan.geometry.scalars
        || profile.identity_count() != plan.geometry.identities
    {
        return Err(ActivationBundleErrorV1::AccountProfile);
    }
    if bundle.transition != build_transition(&plan)? {
        return Err(ActivationBundleErrorV1::Transition);
    }
    let transition = TransitionProgramV2::decode(&bundle.transition)
        .map_err(|_| ActivationBundleErrorV1::Transition)?;
    if transition.scalar_count() != plan.geometry.scalars
        || transition.identity_count() != plan.geometry.identities
    {
        return Err(ActivationBundleErrorV1::Transition);
    }
    if bundle.effect != build_effect(input, &plan)? {
        return Err(ActivationBundleErrorV1::Effect);
    }
    let effect =
        EffectProgramV2::decode(&bundle.effect).map_err(|_| ActivationBundleErrorV1::Effect)?;
    if effect.account_count() != ACTIVATION_ACCOUNT_COUNT_V1
        || effect.scalar_count() != plan.geometry.scalars
        || effect.identity_count() != plan.geometry.identities
        || u32::from(effect.request_bytes()) != input.root_state_bytes
    {
        return Err(ActivationBundleErrorV1::Effect);
    }
    let descriptor = CapabilityProgramV1::decode(&bundle.descriptor)
        .map_err(|_| ActivationBundleErrorV1::Descriptor)?;
    if descriptor.kind() != input.kind
        || descriptor.config_schema() != input.config_schema
        || descriptor.request_schema() != input.request_schema
        || descriptor.root_schema() != input.root_schema
        || descriptor.account_profile().to_bytes() != bundle.account_profile_id
        || descriptor.derivation_policy() != input.derivation_policy
        || descriptor.capacity_profile() != input.capacity_profile
        || descriptor.effect_schema().to_bytes() != bundle.effect_id
        || descriptor.root_state_bytes() != input.root_state_bytes
        || descriptor.transition_program().bytes() != bundle.transition
    {
        return Err(ActivationBundleErrorV1::Descriptor);
    }
    require_probe_projection_is_the_declared_tail(bundle, input, &plan)
}

/// Run the real profile, transition and effect over a built bundle and return
/// what the outer would write as the family root tail, and the two accounts'
/// resulting lamports.
///
/// The register image is emulated in the seam's own order: seed, project the
/// ledger's parked rent, execute the transition, project the effect
/// (`process_activation` in `programs/dclutch-trading-sbf/src/outer.rs`).
pub fn project_activation_root_tail_v1(
    bundle: &ActivationBundleV1,
    seam: ActivationSeamImageV1<'_>,
) -> ActivationResultV1<(Vec<u8>, [u64; 2])> {
    if seam.scalars.len() != ACTIVATION_COMMON_SCALARS_V2
        || seam.identities.len() != ACTIVATION_COMMON_IDENTITIES_V2
    {
        return Err(ActivationBundleErrorV1::RegisterGeometry);
    }
    let effect =
        EffectProgramV2::decode(&bundle.effect).map_err(|_| ActivationBundleErrorV1::Effect)?;
    let profile = AccountProfileV1::decode_selected(
        bundle.account_profile_id,
        digest(&bundle.account_profile),
        &bundle.account_profile,
    )
    .map_err(|_| ActivationBundleErrorV1::AccountProfile)?;
    let transition = TransitionProgramV2::decode(&bundle.transition)
        .map_err(|_| ActivationBundleErrorV1::Transition)?;
    if profile.scalar_count() != effect.scalar_count()
        || transition.scalar_count() != effect.scalar_count()
        || profile.identity_count() != effect.identity_count()
        || transition.identity_count() != effect.identity_count()
    {
        return Err(ActivationBundleErrorV1::RegisterGeometry);
    }
    let scalar_count = usize::from(effect.scalar_count());
    let identity_count = usize::from(effect.identity_count());
    let mut scalars = vec![0_u64; scalar_count];
    let mut identities = vec![[0_u8; 32]; identity_count];
    copy_bank_u64(&mut scalars, seam.scalars)?;
    copy_bank_identity(&mut identities, seam.identities)?;
    // The profile's one projection: the ledger's parked rent quote into the
    // family scalar the transfer reads. Emulated rather than re-run, because a
    // faithful `AccountObservationV1` image would restate the ledger layout this
    // module deliberately never restates.
    set_scalar(
        &mut scalars,
        ACTIVATION_RENT_QUOTE_SCALAR_V1,
        seam.rent_quote,
    )?;
    run_transition(transition, &mut scalars, &mut identities)?;
    let ledger_bytes = ledger_data_length(&profile)?;
    let request_bytes = usize::from(effect.request_bytes());
    let accounts = [
        AccountInput {
            lamports: 0,
            data_len: 0,
        },
        AccountInput {
            lamports: seam.rent_quote,
            data_len: ledger_bytes,
        },
    ];
    let aliases = [
        ACTIVATION_ROOT_ACCOUNT_V2,
        ACTIVATION_FIRST_FUNDING_ACCOUNT_V2,
    ];
    let permissions = [
        AccountPermission::new(false, true, false),
        AccountPermission::new(true, false, true),
    ];
    let mut scratch_lamports = [0_u64; 2];
    let mut output_lamports = [0_u64; 2];
    let mut scratch_request = vec![0_u8; request_bytes];
    let mut output_request = vec![0_u8; request_bytes];
    project_with_aliases_and_requests_atomic(
        effect,
        &scalars,
        &identities,
        &aliases,
        &accounts,
        &permissions,
        &mut scratch_lamports,
        &mut output_lamports,
        &mut scratch_request,
        &mut output_request,
    )
    .map_err(|_| ActivationBundleErrorV1::Projection)?;
    Ok((output_request, output_lamports))
}

/// One composed region and the register it is composed from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WriteV1 {
    /// A constant word, loaded by the transition into a family scalar.
    Constant {
        offset: u32,
        scalar: u16,
        value: u64,
    },
    /// A seam-seeded scalar read straight out of the common bank.
    SeamScalar { offset: u32, register: u16 },
    /// A seam-seeded identity read straight out of the common bank.
    SeamIdentity { offset: u32, register: u16 },
}

impl WriteV1 {
    const fn offset(self) -> u32 {
        match self {
            Self::Constant { offset, .. }
            | Self::SeamScalar { offset, .. }
            | Self::SeamIdentity { offset, .. } => offset,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct GeometryV1 {
    scalars: u16,
    identities: u16,
}

/// The complete ascending composition of one family root tail.
struct CompositionPlanV1 {
    writes: Vec<WriteV1>,
    geometry: GeometryV1,
}

impl CompositionPlanV1 {
    /// Decompose the declared tail into the writes an activation must perform.
    ///
    /// A zero constant word is not composed: the request buffer the seam hands
    /// the effect is zero-initialised, so a zero word is already correct and an
    /// instruction for it would be dead weight in a program the outer interprets
    /// on chain. A nonzero constant byte that no aligned eight-byte write can
    /// place is refused rather than truncated — such a tail cannot be composed
    /// at all, and a family must learn that here, not on a bricked root.
    fn derive(input: ActivationBundleInputV1<'_>) -> ActivationResultV1<Self> {
        let tail = input.constant_root_tail;
        if tail.is_empty()
            || tail.len() > ACTIVATION_MAX_ROLE_REQUEST_BYTES_V2
            || u32::try_from(tail.len()).map_err(|_| ActivationBundleErrorV1::RootWidth)?
                != input.root_state_bytes
        {
            return Err(ActivationBundleErrorV1::RootWidth);
        }
        let mut writes = Vec::new();
        // Seam fields first, so the constant walk can refuse a word that
        // overlaps one. They are required strictly ascending and disjoint so
        // that reading the effect top to bottom reads the tail left to right.
        let mut covered = vec![false; tail.len()];
        let mut boundary = 0_usize;
        for field in input.seam_fields {
            let start = usize::try_from(field.offset())
                .map_err(|_| ActivationBundleErrorV1::TailFieldGeometry)?;
            let end = start
                .checked_add(field.width())
                .ok_or(ActivationBundleErrorV1::TailFieldGeometry)?;
            if start < boundary || end > tail.len() || start % ACTIVATION_TAIL_WORD_BYTES_V1 != 0 {
                return Err(ActivationBundleErrorV1::TailFieldGeometry);
            }
            let region = tail
                .get(start..end)
                .ok_or(ActivationBundleErrorV1::TailFieldGeometry)?;
            if region.iter().any(|byte| *byte != 0) {
                return Err(ActivationBundleErrorV1::TailFieldOverwritesConstant);
            }
            for flag in covered
                .get_mut(start..end)
                .ok_or(ActivationBundleErrorV1::TailFieldGeometry)?
            {
                *flag = true;
            }
            boundary = end;
            writes.push(match *field {
                ActivationTailFieldV1::SeamScalar { offset, register } => {
                    if usize::from(register) >= ACTIVATION_COMMON_SCALARS_V2 {
                        return Err(ActivationBundleErrorV1::TailFieldRegisterOutOfBank);
                    }
                    WriteV1::SeamScalar { offset, register }
                }
                ActivationTailFieldV1::SeamIdentity { offset, register } => {
                    if usize::from(register) >= ACTIVATION_COMMON_IDENTITIES_V2 {
                        return Err(ActivationBundleErrorV1::TailFieldRegisterOutOfBank);
                    }
                    WriteV1::SeamIdentity { offset, register }
                }
            });
        }
        let mut constant_count = 0_u16;
        let mut offset = 0_usize;
        while offset < tail.len() {
            let end = offset
                .checked_add(ACTIVATION_TAIL_WORD_BYTES_V1)
                .ok_or(ActivationBundleErrorV1::RootWidth)?;
            let Some(window) = tail.get(offset..end) else {
                let remainder = tail
                    .get(offset..)
                    .ok_or(ActivationBundleErrorV1::RootWidth)?;
                if remainder.iter().any(|byte| *byte != 0) {
                    return Err(ActivationBundleErrorV1::TailAlignment);
                }
                break;
            };
            let word_covered = covered
                .get(offset..end)
                .ok_or(ActivationBundleErrorV1::TailFieldGeometry)?
                .iter()
                .any(|flag| *flag);
            let value = u64::from_le_bytes(
                window
                    .try_into()
                    .map_err(|_| ActivationBundleErrorV1::RootWidth)?,
            );
            if value != 0 && !word_covered {
                let scalar = ACTIVATION_FIRST_CONSTANT_SCALAR_V1
                    .checked_add(constant_count)
                    .ok_or(ActivationBundleErrorV1::RegisterGeometry)?;
                constant_count = constant_count
                    .checked_add(1)
                    .ok_or(ActivationBundleErrorV1::RegisterGeometry)?;
                writes.push(WriteV1::Constant {
                    offset: u32::try_from(offset)
                        .map_err(|_| ActivationBundleErrorV1::RootWidth)?,
                    scalar,
                    value,
                });
            }
            offset = end;
        }
        writes.sort_by_key(|write| write.offset());
        let scalars = usize::from(ACTIVATION_FIRST_CONSTANT_SCALAR_V1)
            .checked_add(usize::from(constant_count))
            .ok_or(ActivationBundleErrorV1::RegisterGeometry)?;
        if scalars > ACTIVATION_MAX_RUNTIME_SCALARS_V2
            || ACTIVATION_COMMON_IDENTITIES_V2 > ACTIVATION_MAX_RUNTIME_IDENTITIES_V2
        {
            return Err(ActivationBundleErrorV1::RegisterGeometry);
        }
        Ok(Self {
            writes,
            geometry: GeometryV1 {
                scalars: u16::try_from(scalars)
                    .map_err(|_| ActivationBundleErrorV1::RegisterGeometry)?,
                identities: u16::try_from(ACTIVATION_COMMON_IDENTITIES_V2)
                    .map_err(|_| ActivationBundleErrorV1::RegisterGeometry)?,
            },
        })
    }
}

fn build_account_profile(
    input: ActivationBundleInputV1<'_>,
    geometry: GeometryV1,
) -> ActivationResultV1<Vec<u8>> {
    let rules = [
        // The composite root: vacant and System-owned at activation, credited
        // by the funding transfer and allocated/assigned by the outer's commit.
        // A vacant account has zero data, so the rule declares length zero.
        AccountRuleInputV1 {
            privileges: AccountPrivilegesV1::new(false, true, false),
            effect_permissions: AccountEffectPermissionsV1::new(false, true, false),
            alias: AccountAliasInputV1::SelfRepresentative,
            data_length: 0,
        },
        // The selected Trading FundingLedger: debited of its parked rent quote,
        // and rewritten in place by the outer's own activation commit. The
        // write-data permission is what the outer reads back to recognise this
        // as the ledger it may activate.
        AccountRuleInputV1 {
            privileges: AccountPrivilegesV1::new(false, true, false),
            effect_permissions: AccountEffectPermissionsV1::new(true, false, true),
            alias: AccountAliasInputV1::SelfRepresentative,
            data_length: u32::try_from(
                funding_ledger_bytes_v2(input.funding_ledger_slot_count)
                    .map_err(|_| ActivationBundleErrorV1::Geometry)?,
            )
            .map_err(|_| ActivationBundleErrorV1::Geometry)?,
        },
    ];
    let rent_quote_offset = u32::try_from(
        funding_ledger_remaining_offset_v2(0, FundingCompartment::Rent)
            .map_err(|_| ActivationBundleErrorV1::Geometry)?,
    )
    .map_err(|_| ActivationBundleErrorV1::Geometry)?;
    let operations = [
        AccountOperationInputV1::RequireKey {
            account: ACTIVATION_ROOT_ACCOUNT_V2,
            expected: ACTIVATION_ROOT_IDENTITY_V2,
        },
        AccountOperationInputV1::RequireOwner {
            account: ACTIVATION_FIRST_FUNDING_ACCOUNT_V2,
            expected: ACTIVATION_TRADING_PROGRAM_IDENTITY_V2,
        },
        // An EffectProgram has no arithmetic over account data, so the parked
        // rent the transfer must move is projected here out of the ledger's
        // Rent compartment into the family scalar the effect reads.
        AccountOperationInputV1::ProjectDataU64 {
            account: ACTIVATION_FIRST_FUNDING_ACCOUNT_V2,
            data_offset: rent_quote_offset,
            destination: ACTIVATION_RENT_QUOTE_SCALAR_V1,
        },
    ];
    let width = account_profile_v1_bytes(rules.len(), operations.len())
        .map_err(|_| ActivationBundleErrorV1::AccountProfile)?;
    let mut scratch = vec![0_u8; width];
    let mut output = vec![0_u8; width];
    encode_account_profile_v1_atomic(
        &rules,
        &operations,
        RegisterGeometryV1 {
            scalars: geometry.scalars,
            identities: geometry.identities,
        },
        &mut scratch,
        &mut output,
    )
    .map_err(|_| ActivationBundleErrorV1::AccountProfile)?;
    Ok(output)
}

fn build_transition(plan: &CompositionPlanV1) -> ActivationResultV1<Vec<u8>> {
    // Every word the initial tail needs that no account carries and no seam
    // register seeds. They are read out of the family's own canonical initial
    // state, so a layout change moves this artifact with it or refuses.
    let instructions = plan
        .writes
        .iter()
        .filter_map(|write| match *write {
            WriteV1::Constant { scalar, value, .. } => {
                Some(TransitionInstructionV2::load_const(scalar, value))
            }
            WriteV1::SeamScalar { .. } | WriteV1::SeamIdentity { .. } => None,
        })
        .collect::<Vec<_>>();
    let width = transition_program_v2_bytes(instructions.len())
        .map_err(|_| ActivationBundleErrorV1::Transition)?;
    let mut scratch = vec![0_u8; width];
    let mut output = vec![0_u8; width];
    encode_transition_program_v2_atomic(
        TransitionRegisterGeometryV2 {
            scalars: plan.geometry.scalars,
            identities: plan.geometry.identities,
        },
        &instructions,
        &mut scratch,
        &mut output,
    )
    .map_err(|_| ActivationBundleErrorV1::Transition)?;
    Ok(output)
}

fn build_effect(
    input: ActivationBundleInputV1<'_>,
    plan: &CompositionPlanV1,
) -> ActivationResultV1<Vec<u8>> {
    let mut instructions = Vec::with_capacity(
        plan.writes
            .len()
            .checked_add(1)
            .ok_or(ActivationBundleErrorV1::Geometry)?,
    );
    // Move the ledger's parked rent quote into the vacant root; the outer
    // requires the root to end at exactly its rent-exempt minimum.
    instructions.push(EffectInstructionV2::transfer_lamports(
        ACTIVATION_FIRST_FUNDING_ACCOUNT_V2,
        ACTIVATION_ROOT_ACCOUNT_V2,
        ACTIVATION_RENT_QUOTE_SCALAR_V1,
    ));
    // Compose the initial root tail into the request buffer, ascending, so the
    // instruction list reads as the tail itself.
    for write in &plan.writes {
        instructions.push(match *write {
            WriteV1::Constant { offset, scalar, .. } => {
                EffectInstructionV2::write_request_u64(offset, scalar)
            }
            WriteV1::SeamScalar { offset, register } => {
                EffectInstructionV2::write_request_u64(offset, register)
            }
            WriteV1::SeamIdentity { offset, register } => {
                EffectInstructionV2::write_request_identity(offset, register)
            }
        });
    }
    let width =
        effect_program_v2_bytes(instructions.len()).map_err(|_| ActivationBundleErrorV1::Effect)?;
    let mut scratch = vec![0_u8; width];
    let mut output = vec![0_u8; width];
    encode_effect_program_v2_atomic(
        EffectGeometryV2 {
            accounts: ACTIVATION_ACCOUNT_COUNT_V1,
            scalars: plan.geometry.scalars,
            identities: plan.geometry.identities,
            request_bytes: u16::try_from(input.root_state_bytes)
                .map_err(|_| ActivationBundleErrorV1::RootWidth)?,
        },
        &instructions,
        &mut scratch,
        &mut output,
    )
    .map_err(|_| ActivationBundleErrorV1::Effect)?;
    Ok(output)
}

/// The brick gate, run on every build and every validate.
///
/// The probe values are deliberately unlike anything a mistake produces: a
/// projection that ignores a register, reads the neighbouring one, or writes at
/// the wrong offset cannot coincide with the expected image by accident.
fn require_probe_projection_is_the_declared_tail(
    bundle: &ActivationBundleV1,
    input: ActivationBundleInputV1<'_>,
    plan: &CompositionPlanV1,
) -> ActivationResultV1<()> {
    const PROBE_RENT_QUOTE: u64 = 2_672_640;
    let mut scalars = [0_u64; ACTIVATION_COMMON_SCALARS_V2];
    for (index, slot) in scalars.iter_mut().enumerate() {
        let ordinal = u64::try_from(index).map_err(|_| ActivationBundleErrorV1::Geometry)?;
        *slot = 0xa53c_0000_0000_0001_u64
            .checked_add(ordinal.wrapping_mul(0x0001_0000_0000_0000))
            .ok_or(ActivationBundleErrorV1::Geometry)?;
    }
    let mut identities = [[0_u8; 32]; ACTIVATION_COMMON_IDENTITIES_V2];
    for (index, slot) in identities.iter_mut().enumerate() {
        let ordinal = u8::try_from(index).map_err(|_| ActivationBundleErrorV1::Geometry)?;
        for (position, byte) in slot.iter_mut().enumerate() {
            let step =
                u8::try_from(position % 251).map_err(|_| ActivationBundleErrorV1::Geometry)?;
            *byte = ordinal
                .wrapping_mul(17)
                .wrapping_add(step)
                .wrapping_add(0x5b);
        }
    }
    let expected = expected_tail(input, plan, &scalars, &identities)?;
    let (projected, lamports) = project_activation_root_tail_v1(
        bundle,
        ActivationSeamImageV1 {
            scalars: &scalars,
            identities: &identities,
            rent_quote: PROBE_RENT_QUOTE,
        },
    )?;
    if projected != expected {
        return Err(ActivationBundleErrorV1::ProjectedTailMismatch);
    }
    let root = lamports
        .first()
        .copied()
        .ok_or(ActivationBundleErrorV1::Geometry)?;
    let ledger = lamports
        .get(1)
        .copied()
        .ok_or(ActivationBundleErrorV1::Geometry)?;
    if root != PROBE_RENT_QUOTE || ledger != 0 {
        return Err(ActivationBundleErrorV1::ProjectedRentMismatch);
    }
    Ok(())
}

/// The tail the declared composition says a seam image must produce.
fn expected_tail(
    input: ActivationBundleInputV1<'_>,
    plan: &CompositionPlanV1,
    scalars: &[u64],
    identities: &[[u8; 32]],
) -> ActivationResultV1<Vec<u8>> {
    let mut expected = input.constant_root_tail.to_vec();
    for write in &plan.writes {
        match *write {
            WriteV1::Constant { .. } => {}
            WriteV1::SeamScalar { offset, register } => {
                let value = scalars
                    .get(usize::from(register))
                    .copied()
                    .ok_or(ActivationBundleErrorV1::TailFieldRegisterOutOfBank)?;
                write_region(&mut expected, offset, &value.to_le_bytes())?;
            }
            WriteV1::SeamIdentity { offset, register } => {
                let value = identities
                    .get(usize::from(register))
                    .copied()
                    .ok_or(ActivationBundleErrorV1::TailFieldRegisterOutOfBank)?;
                write_region(&mut expected, offset, &value)?;
            }
        }
    }
    Ok(expected)
}

fn write_region(output: &mut [u8], offset: u32, source: &[u8]) -> ActivationResultV1<()> {
    let start = usize::try_from(offset).map_err(|_| ActivationBundleErrorV1::TailFieldGeometry)?;
    let end = start
        .checked_add(source.len())
        .ok_or(ActivationBundleErrorV1::TailFieldGeometry)?;
    output
        .get_mut(start..end)
        .ok_or(ActivationBundleErrorV1::TailFieldGeometry)?
        .copy_from_slice(source);
    Ok(())
}

fn run_transition(
    transition: TransitionProgramV2<'_>,
    scalars: &mut [u64],
    identities: &mut [[u8; 32]],
) -> ActivationResultV1<()> {
    let input_scalars = scalars.to_vec();
    let input_identities = identities.to_vec();
    let mut scratch_scalars = input_scalars.clone();
    let mut scratch_identities = input_identities.clone();
    execute_atomic(
        transition,
        RegisterInput {
            scalars: &input_scalars,
            identities: &input_identities,
        },
        RegisterOutput {
            scalars: &mut scratch_scalars,
            identities: &mut scratch_identities,
        },
        RegisterOutput {
            scalars,
            identities,
        },
    )
    .map_err(|_| ActivationBundleErrorV1::Transition)?;
    Ok(())
}

fn ledger_data_length(profile: &AccountProfileV1<'_>) -> ActivationResultV1<usize> {
    let rule = profile
        .rule(ACTIVATION_FIRST_FUNDING_ACCOUNT_V2)
        .map_err(|_| ActivationBundleErrorV1::AccountProfile)?;
    usize::try_from(rule.data_length()).map_err(|_| ActivationBundleErrorV1::AccountProfile)
}

fn copy_bank_u64(destination: &mut [u64], source: &[u64]) -> ActivationResultV1<()> {
    destination
        .get_mut(..source.len())
        .ok_or(ActivationBundleErrorV1::RegisterGeometry)?
        .copy_from_slice(source);
    Ok(())
}

fn copy_bank_identity(destination: &mut [[u8; 32]], source: &[[u8; 32]]) -> ActivationResultV1<()> {
    destination
        .get_mut(..source.len())
        .ok_or(ActivationBundleErrorV1::RegisterGeometry)?
        .copy_from_slice(source);
    Ok(())
}

fn set_scalar(scalars: &mut [u64], index: u16, value: u64) -> ActivationResultV1<()> {
    *scalars
        .get_mut(usize::from(index))
        .ok_or(ActivationBundleErrorV1::RegisterGeometry)? = value;
    Ok(())
}

fn content(bytes: [u8; 32]) -> ActivationResultV1<ContentId> {
    ContentId::new(bytes).map_err(|_| ActivationBundleErrorV1::Geometry)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;

    fn id(value: u8) -> ContentId {
        ContentId::new([value; 32]).expect("content identity")
    }

    fn input<'a>(
        tail: &'a [u8],
        fields: &'a [ActivationTailFieldV1],
    ) -> ActivationBundleInputV1<'a> {
        ActivationBundleInputV1 {
            kind: id(0x11),
            config_schema: id(0x22),
            request_schema: id(0x33),
            root_schema: id(0x44),
            derivation_policy: id(0x55),
            capacity_profile: id(0x66),
            root_state_bytes: u32::try_from(tail.len()).expect("width"),
            constant_root_tail: tail,
            seam_fields: fields,
            funding_ledger_slot_count: 1,
        }
    }

    /// A tail with a constant word, a seam identity, a seam scalar and a zero
    /// word -- one of each thing a family can declare.
    fn mixed_tail() -> [u8; 56] {
        let mut tail = [0_u8; 56];
        tail.get_mut(0..8)
            .expect("magic")
            .copy_from_slice(&0x4443_4c54_4d49_5831_u64.to_le_bytes());
        // 8..40 is a seam identity, 40..48 a seam scalar, 48..56 zero.
        tail
    }

    fn mixed_fields() -> [ActivationTailFieldV1; 2] {
        [
            ActivationTailFieldV1::SeamIdentity {
                offset: 8,
                register: 4,
            },
            ActivationTailFieldV1::SeamScalar {
                offset: 40,
                register: 1,
            },
        ]
    }

    #[test]
    fn a_mixed_constant_and_seam_tail_projects_exactly_what_it_declares() {
        let tail = mixed_tail();
        let fields = mixed_fields();
        let bundle = build_activation_bundle_v1(input(&tail, &fields)).expect("bundle");

        let mut scalars = [0_u64; ACTIVATION_COMMON_SCALARS_V2];
        *scalars.get_mut(1).expect("generation slot") = 0x0123_4567_89ab_cdef;
        let mut identities = [[0_u8; 32]; ACTIVATION_COMMON_IDENTITIES_V2];
        *identities.get_mut(4).expect("market slot") = [0x9e; 32];
        let (projected, lamports) = project_activation_root_tail_v1(
            &bundle,
            ActivationSeamImageV1 {
                scalars: &scalars,
                identities: &identities,
                rent_quote: 2_672_640,
            },
        )
        .expect("projection");

        let mut expected = tail;
        expected
            .get_mut(8..40)
            .expect("identity region")
            .copy_from_slice(&[0x9e; 32]);
        expected
            .get_mut(40..48)
            .expect("scalar region")
            .copy_from_slice(&0x0123_4567_89ab_cdef_u64.to_le_bytes());
        assert_eq!(projected.as_slice(), expected.as_slice());
        assert_eq!(lamports, [2_672_640, 0]);

        // The trailing zero word costs no instruction: the request buffer is
        // already zero there.
        let effect = EffectProgramV2::decode(&bundle.effect).expect("effect");
        assert_eq!(effect.instruction_count(), 4);
    }

    /// A field reading a family-owned register would compose a silent zero,
    /// because this bundle's transition only ever loads constants.
    #[test]
    fn a_field_reading_outside_the_seam_bank_refuses() {
        let tail = mixed_tail();
        for field in [
            ActivationTailFieldV1::SeamScalar {
                offset: 8,
                register: u16::try_from(ACTIVATION_COMMON_SCALARS_V2).expect("bank"),
            },
            ActivationTailFieldV1::SeamIdentity {
                offset: 8,
                register: u16::try_from(ACTIVATION_COMMON_IDENTITIES_V2).expect("bank"),
            },
        ] {
            assert_eq!(
                build_activation_bundle_v1(input(&tail, &[field])).err(),
                Some(ActivationBundleErrorV1::TailFieldRegisterOutOfBank)
            );
        }
    }

    #[test]
    fn a_field_over_a_constant_byte_refuses_rather_than_choosing_a_source() {
        let mut tail = mixed_tail();
        *tail.get_mut(8).expect("first field byte") = 0x01;
        assert_eq!(
            build_activation_bundle_v1(input(&tail, &mixed_fields())).err(),
            Some(ActivationBundleErrorV1::TailFieldOverwritesConstant)
        );
    }

    #[test]
    fn descending_overlapping_unaligned_and_overrunning_fields_refuse() {
        let tail = mixed_tail();
        let cases: [&[ActivationTailFieldV1]; 4] = [
            // Descending.
            &[
                ActivationTailFieldV1::SeamScalar {
                    offset: 40,
                    register: 1,
                },
                ActivationTailFieldV1::SeamIdentity {
                    offset: 8,
                    register: 4,
                },
            ],
            // Overlapping.
            &[
                ActivationTailFieldV1::SeamIdentity {
                    offset: 8,
                    register: 4,
                },
                ActivationTailFieldV1::SeamScalar {
                    offset: 32,
                    register: 1,
                },
            ],
            // Unaligned.
            &[ActivationTailFieldV1::SeamScalar {
                offset: 12,
                register: 1,
            }],
            // Overrunning the tail.
            &[ActivationTailFieldV1::SeamIdentity {
                offset: 48,
                register: 4,
            }],
        ];
        for fields in cases {
            assert_eq!(
                build_activation_bundle_v1(input(&tail, fields)).err(),
                Some(ActivationBundleErrorV1::TailFieldGeometry)
            );
        }
    }

    /// There is no request write narrower than eight bytes, so a nonzero byte in
    /// a trailing partial word cannot be composed at all.
    #[test]
    fn a_nonzero_byte_no_aligned_write_can_place_refuses() {
        let mut tail = [0_u8; 20];
        tail.get_mut(0..8)
            .expect("word")
            .copy_from_slice(&7_u64.to_le_bytes());
        *tail.get_mut(17).expect("trailing byte") = 1;
        assert_eq!(
            build_activation_bundle_v1(input(&tail, &[])).err(),
            Some(ActivationBundleErrorV1::TailAlignment)
        );
        // The same tail with that byte zero is composable.
        *tail.get_mut(17).expect("trailing byte") = 0;
        build_activation_bundle_v1(input(&tail, &[])).expect("composable tail");
    }

    #[test]
    fn a_declared_width_that_disagrees_with_the_tail_refuses() {
        let tail = mixed_tail();
        let fields = mixed_fields();
        let mut wrong = input(&tail, &fields);
        wrong.root_state_bytes = u32::try_from(tail.len()).expect("width") + 8;
        assert_eq!(
            build_activation_bundle_v1(wrong).err(),
            Some(ActivationBundleErrorV1::RootWidth)
        );
        assert_eq!(
            build_activation_bundle_v1(input(&[], &[])).err(),
            Some(ActivationBundleErrorV1::RootWidth)
        );
    }

    /// A bundle built for one tail does not rejoin to another, and the artifacts
    /// are all three load-bearing.
    #[test]
    fn a_bundle_does_not_rejoin_to_a_different_tail_or_a_substituted_record() {
        let tail = mixed_tail();
        let fields = mixed_fields();
        let bundle = build_activation_bundle_v1(input(&tail, &fields)).expect("bundle");
        validate_activation_bundle_v1(&bundle, input(&tail, &fields)).expect("rejoin");

        let mut other = tail;
        *other.get_mut(0).expect("magic byte") ^= 1;
        assert!(validate_activation_bundle_v1(&bundle, input(&other, &fields)).is_err());

        for index in 0..3_usize {
            let mut hostile = bundle.clone();
            let record = match index {
                0 => &mut hostile.account_profile,
                1 => &mut hostile.effect,
                _ => &mut hostile.transition,
            };
            *record.last_mut().expect("record byte") ^= 1;
            assert!(validate_activation_bundle_v1(&hostile, input(&tail, &fields)).is_err());
        }
    }

    #[test]
    fn the_three_published_schemas_are_the_ones_the_seam_authenticates_under() {
        assert_eq!(
            activation_descriptor_schema_v1(),
            CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1
        );
        assert_eq!(
            activation_account_profile_schema_v1(),
            ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V1
        );
        assert_eq!(activation_effect_schema_v1(), EFFECT_PROGRAM_SCHEMA_ID_V2);
    }
}
