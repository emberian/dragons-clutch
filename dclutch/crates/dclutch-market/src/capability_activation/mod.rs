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
//! Every byte of an initial root tail must be a **constant** the family
//! publishes, a **seam-seeded register** the outer fills in before any artifact
//! runs (`activation_registers_v2`), the profile-projected root rent quote, or
//! the canonical root bump inserted through the one reviewed checked-arithmetic
//! expression. There is no family-authored runtime value outside those sources.
//!
//! - Direct's 24-byte tail is entirely constant (magic, header word, a zero
//!   counter).
//! - General's 128-byte tail is constants PLUS the Market, the config identity
//!   and the generation — three [`ActivationTailFieldV1`]s reading identity and
//!   scalar registers 4, 8 and 1.
//! - Fractional's current root uses the append-only seam: the authenticated
//!   config and Core context identities, the Market, the profile-projected rent
//!   quote, and Trading's canonical pre-effect root bump. It does not smuggle
//!   Market-bound terms into the manifest-selected config coordinate.
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
//! artifact: `dclutch-trading`'s `activation_bundle_v1::tests::
//! the_family_neutral_template_reproduces_this_sealed_bundle_byte_for_byte`
//! rebuilds Direct's sealed triple out of this module and compares all four
//! records and all three digests. If it goes red the template has drifted from
//! the thing that was reviewed, and no family may use it until that is explained.

extern crate alloc;

use alloc::{vec, vec::Vec};

use crate::capability_manifest::{
    FundingCompartment, funding_ledger_bytes_v2, funding_ledger_remaining_offset_v2,
};
use crate::capability_program::{
    CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1, CapabilityProgramV1,
    activation_registers_v2::{
        ACTIVATION_COMMON_IDENTITIES_V2, ACTIVATION_COMMON_SCALARS_V2,
        ACTIVATION_FIRST_FAMILY_SCALAR_V2, ACTIVATION_FIRST_FUNDING_ACCOUNT_V2,
        ACTIVATION_MAX_ROLE_REQUEST_BYTES_V2, ACTIVATION_MAX_RUNTIME_IDENTITIES_V2,
        ACTIVATION_MAX_RUNTIME_SCALARS_V2, ACTIVATION_ROOT_ACCOUNT_V2, ACTIVATION_ROOT_IDENTITY_V2,
        ACTIVATION_RUNTIME_FUNDING_ACCOUNTS_V2, ACTIVATION_TRADING_PROGRAM_IDENTITY_V2,
        activation_account_count_v2,
    },
    activation_registers_v3::{
        ACTIVATION_COMMON_IDENTITIES_V3, ACTIVATION_COMMON_SCALARS_V3,
        ACTIVATION_FIRST_FAMILY_SCALAR_V3, ACTIVATION_ROOT_BUMP_SCALAR_V3,
    },
    encode_v1::{
        CapabilityProgramInputV1, capability_program_v1_bytes, encode_capability_program_v1_atomic,
    },
};
use dclutch_core_contract::ContentId;
use dclutch_sha256_adapter::digest;
use dclutch_vm::account_profile::{
    ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V1, AccountProfileV1, OperationKindV1,
    encode_v1::{
        AccountAliasInputV1, AccountEffectPermissionsV1, AccountOperationInputV1,
        AccountPrivilegesV1, AccountRuleInputV1, RegisterGeometryV1, account_profile_v1_bytes,
        encode_account_profile_v1_atomic,
    },
};
use dclutch_vm::effect::v2::{
    AccountInput, AccountPermission, ProgramV2 as EffectProgramV2,
    SCHEMA_RELEASE_ID as EFFECT_PROGRAM_SCHEMA_ID_V2,
    encode::{
        EffectGeometryV2, EffectInstructionV2, effect_program_v2_bytes,
        encode_effect_program_v2_atomic,
    },
    project_with_aliases_and_requests_atomic,
};
use dclutch_vm::v2::{
    ProgramV2 as TransitionProgramV2, RegisterInput, RegisterOutput,
    encode::{
        RegisterGeometryV2 as TransitionRegisterGeometryV2, TransitionInstructionV2,
        encode_transition_program_v2_atomic, transition_program_v2_bytes,
    },
    execute_atomic,
};

/// The vacant composite root and the one selected Trading `FundingLedgerV2`.
///
/// Read from the seam's own author rather than written down: the frame is the
/// root followed by [`ACTIVATION_RUNTIME_FUNDING_ACCOUNTS_V2`] ledgers, and
/// `outer.rs::RuntimeFrameV2::new` composes exactly that for every
/// descriptor-owned route.
///
/// It does not widen with a market's physical ledger count, and that is a fact
/// about `AccountProfileV1` rather than a template decision: a dependency
/// ledger's rule would be `UnanchoredAccount`, because the requirement
/// operations available here compare against the seam-seeded identity bank and
/// it publishes no foreign controller. The seam authenticates dependency
/// ledgers outside the interpreted frame instead.
pub const ACTIVATION_ACCOUNT_COUNT_V1: u16 =
    match activation_account_count_v2(ACTIVATION_RUNTIME_FUNDING_ACCOUNTS_V2) {
        Some(count) => count,
        None => panic!("the descriptor-owned activation frame is in bounds"),
    };

/// Scalar holding the rent quote projected out of the funding ledger.
///
/// The first family-owned scalar. The bank below it is seam-seeded, and a family
/// artifact that writes there clobbers what the outer relies on downstream.
pub const ACTIVATION_RENT_QUOTE_SCALAR_V1: u16 = ACTIVATION_FIRST_FAMILY_SCALAR_V2;

/// Scalar holding the rent quote when the append-only root-bump bank is used.
pub const ACTIVATION_RENT_QUOTE_SCALAR_V2: u16 = ACTIVATION_FIRST_FAMILY_SCALAR_V3;

/// First scalar holding a constant root-tail word loaded by the transition.
pub const ACTIVATION_FIRST_CONSTANT_SCALAR_V1: u16 = ACTIVATION_RENT_QUOTE_SCALAR_V1 + 1;

/// Scalar holding the Creation compartment projected out of the funding ledger.
///
/// Allocated only by a family that declares
/// [`ActivationBundleInputV1::delivers_creation_principal`]; a family that does
/// not keeps the exact scalar bank, artifact bytes and digests it had.
pub const ACTIVATION_CREATION_QUOTE_SCALAR_V1: u16 = ACTIVATION_RENT_QUOTE_SCALAR_V1 + 1;

/// Scalar holding the exact total one activation transfers into its root.
pub const ACTIVATION_ROOT_TRANSFER_SCALAR_V1: u16 = ACTIVATION_CREATION_QUOTE_SCALAR_V1 + 1;

const ACTIVATION_FIRST_CONSTANT_SCALAR_WITH_CREATION_V1: u16 =
    ACTIVATION_ROOT_TRANSFER_SCALAR_V1 + 1;

const ACTIVATION_ROOT_BUMP_MULTIPLIER_SCALAR_V2: u16 = ACTIVATION_RENT_QUOTE_SCALAR_V2 + 1;
const ACTIVATION_ROOT_BUMP_BASE_SCALAR_V2: u16 = ACTIVATION_ROOT_BUMP_MULTIPLIER_SCALAR_V2 + 1;
const ACTIVATION_ROOT_BUMP_MAX_SCALAR_V2: u16 = ACTIVATION_ROOT_BUMP_BASE_SCALAR_V2 + 1;
const ACTIVATION_ROOT_BUMP_WORD_SCALAR_V2: u16 = ACTIVATION_ROOT_BUMP_MAX_SCALAR_V2 + 1;
const ACTIVATION_FIRST_CONSTANT_SCALAR_V2: u16 = ACTIVATION_ROOT_BUMP_WORD_SCALAR_V2 + 1;
const ACTIVATION_ROOT_BUMP_SHIFT_V2: u64 = 1 << 16;

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
    /// One aligned root header word whose low bytes are family constants and
    /// whose PDA-bump byte is supplied by Trading's derived V3 register.
    ///
    /// This is deliberately not a general arithmetic expression. The only
    /// newly admitted source is the one canonical bump register, shifted into
    /// byte two and checked-added to the family-authored base word.
    RootBumpWord {
        /// Byte offset within the family root tail.
        offset: u32,
    },
    /// Eight little-endian bytes carrying the account-profile-projected root
    /// rent quote. The family names only the tail offset; this codec owns which
    /// scalar receives the quote in each register generation.
    RentQuoteWord {
        /// Byte offset within the family root tail.
        offset: u32,
    },
}

impl ActivationTailFieldV1 {
    /// Byte offset within the family root tail.
    #[must_use]
    pub const fn offset(self) -> u32 {
        match self {
            Self::SeamScalar { offset, .. }
            | Self::SeamIdentity { offset, .. }
            | Self::RootBumpWord { offset }
            | Self::RentQuoteWord { offset } => offset,
        }
    }

    /// Exact byte width this field composes.
    #[must_use]
    pub const fn width(self) -> usize {
        match self {
            Self::SeamScalar { .. } | Self::RootBumpWord { .. } | Self::RentQuoteWord { .. } => {
                ACTIVATION_TAIL_WORD_BYTES_V1
            }
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
    /// Whether this activation delivers the manifest-declared `Creation`
    /// compartment into the root alongside its exact Rent reserve.
    ///
    /// `Rent` and `Creation` are the funding model's only two
    /// `NativeLamportsOnly` compartments, and
    /// `FundingLedgerV2::activate_in_place` already releases BOTH and reports
    /// them as `ActivationDebitV1`. A family that leaves this `false` keeps the
    /// exact artifacts, bytes and digests it has today; a family that sets it
    /// true is stating that its root's opening balance is
    /// `rent_lamports + creation_lamports`, which is the only family-varying
    /// lamport quantity the seam admits and which the manifest — not any family
    /// config record — authenticates.
    pub delivers_creation_principal: bool,
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
    /// ledger's parked rent quote plus any declared creation principal.
    ProjectedRentMismatch,
    /// A family asked for both the append-only root-bump bank and a delivered
    /// creation principal. No family needs both, and composing the two scalar
    /// layouts blind would silently move a constant word; this refuses instead.
    CreationPrincipalWithRootBump,
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

/// Seam image for an activation that also delivers a creation principal.
///
/// Append-only beside [`ActivationSeamImageV1`]: `rent_quote` keeps its exact
/// meaning and `creation_quote` is the manifest's `Creation` compartment, which
/// `FundingLedgerV2::activate_in_place` releases in the same statement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActivationSeamImageV3<'a> {
    /// Seam-seeded scalar bank, exactly `ACTIVATION_COMMON_SCALARS_V2` wide.
    pub scalars: &'a [u64],
    /// Unchanged common identity bank.
    pub identities: &'a [[u8; 32]],
    /// Lamports the founding parked in the ledger's Rent compartment.
    pub rent_quote: u64,
    /// Lamports the founding parked in the ledger's Creation compartment.
    pub creation_quote: u64,
}

/// Append-only seam image for roots which persist their canonical PDA bump.
///
/// The first eight scalars retain their exact V1 meanings. Scalar eight is the
/// child-root bump derived by Trading from the authenticated root header; no
/// instruction byte or family artifact authors it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActivationSeamImageV2<'a> {
    /// Seam-seeded scalar bank, exactly `ACTIVATION_COMMON_SCALARS_V3` wide.
    pub scalars: &'a [u64],
    /// Seam-seeded identity bank, exactly `ACTIVATION_COMMON_IDENTITIES_V3` wide.
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
    let account_profile = build_account_profile(input, &plan)?;
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
    if bundle.account_profile != build_account_profile(input, &plan)? {
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
    require_declared_creation_projection(profile, &plan)?;
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
    project_activation_root_tail(
        bundle,
        seam.scalars,
        seam.identities,
        seam.rent_quote,
        0,
        ACTIVATION_RENT_QUOTE_SCALAR_V1,
    )
}

/// Run the activation seam over a bundle that also delivers a creation principal.
///
/// Append-only beside [`project_activation_root_tail_v1`]: the returned lamports
/// are `[root, ledger]`, and a correct bundle leaves the root holding
/// `rent_quote + creation_quote` with the ledger row at zero.
pub fn project_activation_root_tail_v3(
    bundle: &ActivationBundleV1,
    seam: ActivationSeamImageV3<'_>,
) -> ActivationResultV1<(Vec<u8>, [u64; 2])> {
    if seam.scalars.len() != ACTIVATION_COMMON_SCALARS_V2
        || seam.identities.len() != ACTIVATION_COMMON_IDENTITIES_V2
    {
        return Err(ActivationBundleErrorV1::RegisterGeometry);
    }
    project_activation_root_tail(
        bundle,
        seam.scalars,
        seam.identities,
        seam.rent_quote,
        seam.creation_quote,
        ACTIVATION_RENT_QUOTE_SCALAR_V1,
    )
}

/// Run the append-only root-bump activation seam over one built bundle.
pub fn project_activation_root_tail_v2(
    bundle: &ActivationBundleV1,
    seam: ActivationSeamImageV2<'_>,
) -> ActivationResultV1<(Vec<u8>, [u64; 2])> {
    let bump = seam
        .scalars
        .get(usize::from(ACTIVATION_ROOT_BUMP_SCALAR_V3))
        .copied()
        .ok_or(ActivationBundleErrorV1::RegisterGeometry)?;
    if seam.scalars.len() != ACTIVATION_COMMON_SCALARS_V3
        || seam.identities.len() != ACTIVATION_COMMON_IDENTITIES_V3
        || bump > u64::from(u8::MAX)
    {
        return Err(ActivationBundleErrorV1::RegisterGeometry);
    }
    project_activation_root_tail(
        bundle,
        seam.scalars,
        seam.identities,
        seam.rent_quote,
        0,
        ACTIVATION_RENT_QUOTE_SCALAR_V2,
    )
}

fn project_activation_root_tail(
    bundle: &ActivationBundleV1,
    seam_scalars: &[u64],
    seam_identities: &[[u8; 32]],
    rent_quote: u64,
    creation_quote: u64,
    rent_quote_scalar: u16,
) -> ActivationResultV1<(Vec<u8>, [u64; 2])> {
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
    copy_bank_u64(&mut scalars, seam_scalars)?;
    copy_bank_identity(&mut identities, seam_identities)?;
    // The profile's one projection: the ledger's parked rent quote into the
    // family scalar the transfer reads. Emulated rather than re-run, because a
    // faithful `AccountObservationV1` image would restate the ledger layout this
    // module deliberately never restates.
    set_scalar(&mut scalars, rent_quote_scalar, rent_quote)?;
    // The second projected compartment, when the family declared one. For a
    // bundle that declared none this coordinate is its first CONSTANT scalar,
    // which the transition's own `load_const` overwrites before any write reads
    // it -- so a caller passing a stray creation quote cannot alter the tail.
    if creation_quote != 0 {
        set_scalar(
            &mut scalars,
            ACTIVATION_CREATION_QUOTE_SCALAR_V1,
            creation_quote,
        )?;
    }
    run_transition(transition, &mut scalars, &mut identities)?;
    let ledger_bytes = ledger_data_length(&profile)?;
    let request_bytes = usize::from(effect.request_bytes());
    let accounts = [
        AccountInput {
            lamports: 0,
            data_len: 0,
        },
        AccountInput {
            lamports: rent_quote
                .checked_add(creation_quote)
                .ok_or(ActivationBundleErrorV1::Geometry)?,
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
    /// A family-authored base word with Trading's derived root bump inserted
    /// into byte two through checked TransitionVM arithmetic.
    RootBumpWord {
        offset: u32,
        base_word: u64,
        bump_register: u16,
        multiplier_scalar: u16,
        base_scalar: u16,
        max_scalar: u16,
        output_scalar: u16,
    },
    /// Account-profile-projected rent quote written at the family offset.
    RentQuoteWord { offset: u32, scalar: u16 },
}

impl WriteV1 {
    const fn offset(self) -> u32 {
        match self {
            Self::Constant { offset, .. }
            | Self::SeamScalar { offset, .. }
            | Self::SeamIdentity { offset, .. }
            | Self::RootBumpWord { offset, .. }
            | Self::RentQuoteWord { offset, .. } => offset,
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
    common_scalars: usize,
    rent_quote_scalar: u16,
    /// Present exactly when the family declared a delivered creation principal.
    creation_quote_scalar: Option<u16>,
    /// Scalar the single transfer instruction reads. Equal to
    /// `rent_quote_scalar` unless a creation principal is delivered, in which
    /// case the transition sums the two projected compartments into it.
    root_transfer_scalar: u16,
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
        let root_bump_fields = input
            .seam_fields
            .iter()
            .filter(|field| matches!(field, ActivationTailFieldV1::RootBumpWord { .. }))
            .count();
        if root_bump_fields > 1 {
            return Err(ActivationBundleErrorV1::TailFieldGeometry);
        }
        let uses_root_bump = root_bump_fields == 1;
        if uses_root_bump && input.delivers_creation_principal {
            return Err(ActivationBundleErrorV1::CreationPrincipalWithRootBump);
        }
        let common_scalars = if uses_root_bump {
            ACTIVATION_COMMON_SCALARS_V3
        } else {
            ACTIVATION_COMMON_SCALARS_V2
        };
        let rent_quote_scalar = if uses_root_bump {
            ACTIVATION_RENT_QUOTE_SCALAR_V2
        } else {
            ACTIVATION_RENT_QUOTE_SCALAR_V1
        };
        let creation_quote_scalar = input
            .delivers_creation_principal
            .then_some(ACTIVATION_CREATION_QUOTE_SCALAR_V1);
        let root_transfer_scalar = if input.delivers_creation_principal {
            ACTIVATION_ROOT_TRANSFER_SCALAR_V1
        } else {
            rent_quote_scalar
        };
        let first_constant_scalar = if uses_root_bump {
            ACTIVATION_FIRST_CONSTANT_SCALAR_V2
        } else if input.delivers_creation_principal {
            ACTIVATION_FIRST_CONSTANT_SCALAR_WITH_CREATION_V1
        } else {
            ACTIVATION_FIRST_CONSTANT_SCALAR_V1
        };
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
            if !matches!(field, ActivationTailFieldV1::RootBumpWord { .. })
                && region.iter().any(|byte| *byte != 0)
            {
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
                ActivationTailFieldV1::RootBumpWord { offset } => {
                    let base_word = u64::from_le_bytes(
                        region
                            .try_into()
                            .map_err(|_| ActivationBundleErrorV1::TailFieldGeometry)?,
                    );
                    if base_word & (0xff_u64 << 16) != 0 {
                        return Err(ActivationBundleErrorV1::TailFieldOverwritesConstant);
                    }
                    WriteV1::RootBumpWord {
                        offset,
                        base_word,
                        bump_register: ACTIVATION_ROOT_BUMP_SCALAR_V3,
                        multiplier_scalar: ACTIVATION_ROOT_BUMP_MULTIPLIER_SCALAR_V2,
                        base_scalar: ACTIVATION_ROOT_BUMP_BASE_SCALAR_V2,
                        max_scalar: ACTIVATION_ROOT_BUMP_MAX_SCALAR_V2,
                        output_scalar: ACTIVATION_ROOT_BUMP_WORD_SCALAR_V2,
                    }
                }
                ActivationTailFieldV1::RentQuoteWord { offset } => WriteV1::RentQuoteWord {
                    offset,
                    scalar: rent_quote_scalar,
                },
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
                let scalar = first_constant_scalar
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
        let scalars = usize::from(first_constant_scalar)
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
            common_scalars,
            rent_quote_scalar,
            creation_quote_scalar,
            root_transfer_scalar,
        })
    }
}

/// Require the profile to declare the Creation projection exactly when the
/// family declared a delivered principal — read off the DECODED artifact.
///
/// The byte-equality check above rebuilds the profile with
/// [`build_account_profile`], so a builder that stopped emitting this operation
/// would agree with itself and the emulated projection would still succeed:
/// `project_activation_root_tail` seeds the compartment scalars directly rather
/// than re-running an `AccountObservationV1`. On chain that bundle would read a
/// zero scalar, transfer the rent alone, and strand a principal
/// `release_in_place` can never release. This reads the operation list the outer
/// will actually interpret, so the builder cannot be its own witness.
fn require_declared_creation_projection(
    profile: AccountProfileV1<'_>,
    plan: &CompositionPlanV1,
) -> ActivationResultV1<()> {
    let creation_offset = u32::try_from(
        funding_ledger_remaining_offset_v2(0, FundingCompartment::Creation)
            .map_err(|_| ActivationBundleErrorV1::Geometry)?,
    )
    .map_err(|_| ActivationBundleErrorV1::Geometry)?;
    let mut declared = None;
    let mut index = 0_u16;
    while index < profile.operation_count() {
        let operation = profile
            .operation(index)
            .map_err(|_| ActivationBundleErrorV1::AccountProfile)?;
        if operation.kind() == OperationKindV1::ProjectDataU64
            && operation.account() == ACTIVATION_FIRST_FUNDING_ACCOUNT_V2
            && operation.data_offset() == creation_offset
        {
            // Two projections of the same compartment into different scalars
            // would make the transferred total depend on operation order.
            if declared.is_some() {
                return Err(ActivationBundleErrorV1::AccountProfile);
            }
            declared = Some(operation.register());
        }
        index = index
            .checked_add(1)
            .ok_or(ActivationBundleErrorV1::Geometry)?;
    }
    if declared != plan.creation_quote_scalar {
        return Err(ActivationBundleErrorV1::AccountProfile);
    }
    Ok(())
}

fn build_account_profile(
    input: ActivationBundleInputV1<'_>,
    plan: &CompositionPlanV1,
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
            destination: plan.rent_quote_scalar,
        },
    ];
    // The declared creation principal is projected from the SAME ledger row, one
    // compartment along, so the two halves of `ActivationDebitV1` reach the
    // artifacts from the account the outer already authenticated. A family that
    // declares none appends nothing and keeps its exact profile bytes.
    let mut operations = Vec::from(operations);
    if let Some(destination) = plan.creation_quote_scalar {
        operations.push(AccountOperationInputV1::ProjectDataU64 {
            account: ACTIVATION_FIRST_FUNDING_ACCOUNT_V2,
            data_offset: u32::try_from(
                funding_ledger_remaining_offset_v2(0, FundingCompartment::Creation)
                    .map_err(|_| ActivationBundleErrorV1::Geometry)?,
            )
            .map_err(|_| ActivationBundleErrorV1::Geometry)?,
            destination,
        });
    }
    let width = account_profile_v1_bytes(rules.len(), operations.len())
        .map_err(|_| ActivationBundleErrorV1::AccountProfile)?;
    let mut scratch = vec![0_u8; width];
    let mut output = vec![0_u8; width];
    encode_account_profile_v1_atomic(
        &rules,
        &operations,
        RegisterGeometryV1 {
            scalars: plan.geometry.scalars,
            identities: plan.geometry.identities,
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
    let mut instructions = Vec::new();
    // The one arithmetic statement a delivered creation principal needs: the
    // total the transfer moves is the checked sum of the two native compartments
    // the profile projected, so an overflowing manifest quote refuses inside the
    // transition instead of wrapping into a short root.
    if let Some(creation_scalar) = plan.creation_quote_scalar {
        instructions.push(TransitionInstructionV2::checked_add_into(
            plan.rent_quote_scalar,
            creation_scalar,
            plan.root_transfer_scalar,
        ));
    }
    for write in &plan.writes {
        match *write {
            WriteV1::Constant { scalar, value, .. } => {
                instructions.push(TransitionInstructionV2::load_const(scalar, value));
            }
            WriteV1::RootBumpWord {
                base_word,
                bump_register,
                multiplier_scalar,
                base_scalar,
                max_scalar,
                output_scalar,
                ..
            } => {
                instructions.push(TransitionInstructionV2::load_const(
                    multiplier_scalar,
                    ACTIVATION_ROOT_BUMP_SHIFT_V2,
                ));
                instructions.push(TransitionInstructionV2::load_const(base_scalar, base_word));
                instructions.push(TransitionInstructionV2::load_const(
                    max_scalar,
                    u64::from(u8::MAX),
                ));
                instructions.push(TransitionInstructionV2::scalar_le(
                    bump_register,
                    max_scalar,
                ));
                instructions.push(TransitionInstructionV2::checked_mul_into(
                    bump_register,
                    multiplier_scalar,
                    output_scalar,
                ));
                instructions.push(TransitionInstructionV2::checked_add_into(
                    output_scalar,
                    base_scalar,
                    output_scalar,
                ));
            }
            WriteV1::SeamScalar { .. } | WriteV1::SeamIdentity { .. } => {}
            WriteV1::RentQuoteWord { .. } => {}
        }
    }
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
    // Move the ledger's parked activation quote into the vacant root. Without a
    // declared creation principal that is the rent quote alone and the outer
    // requires the root to end at exactly its rent-exempt minimum; with one it
    // is the checked sum, and the outer requires rent plus that principal.
    instructions.push(EffectInstructionV2::transfer_lamports(
        ACTIVATION_FIRST_FUNDING_ACCOUNT_V2,
        ACTIVATION_ROOT_ACCOUNT_V2,
        plan.root_transfer_scalar,
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
            WriteV1::RootBumpWord {
                offset,
                output_scalar,
                ..
            } => EffectInstructionV2::write_request_u64(offset, output_scalar),
            WriteV1::RentQuoteWord { offset, scalar } => {
                EffectInstructionV2::write_request_u64(offset, scalar)
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
    let mut scalars = vec![0_u64; plan.common_scalars];
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
    if plan.common_scalars == ACTIVATION_COMMON_SCALARS_V3 {
        *scalars
            .get_mut(usize::from(ACTIVATION_ROOT_BUMP_SCALAR_V3))
            .ok_or(ActivationBundleErrorV1::RegisterGeometry)? = 0xfe;
    }
    let expected = expected_tail(input, plan, &scalars, &identities, PROBE_RENT_QUOTE)?;
    let (projected, lamports) = if plan.common_scalars == ACTIVATION_COMMON_SCALARS_V3 {
        project_activation_root_tail_v2(
            bundle,
            ActivationSeamImageV2 {
                scalars: &scalars,
                identities: &identities,
                rent_quote: PROBE_RENT_QUOTE,
            },
        )?
    } else {
        project_activation_root_tail_v1(
            bundle,
            ActivationSeamImageV1 {
                scalars: &scalars,
                identities: &identities,
                rent_quote: PROBE_RENT_QUOTE,
            },
        )?
    };
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
    rent_quote: u64,
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
            WriteV1::RootBumpWord {
                offset,
                base_word,
                bump_register,
                ..
            } => {
                let bump = scalars
                    .get(usize::from(bump_register))
                    .copied()
                    .filter(|value| u8::try_from(*value).is_ok())
                    .ok_or(ActivationBundleErrorV1::TailFieldRegisterOutOfBank)?;
                let value = bump
                    .checked_mul(ACTIVATION_ROOT_BUMP_SHIFT_V2)
                    .and_then(|shifted| base_word.checked_add(shifted))
                    .ok_or(ActivationBundleErrorV1::Geometry)?;
                write_region(&mut expected, offset, &value.to_le_bytes())?;
            }
            WriteV1::RentQuoteWord { offset, .. } => {
                write_region(&mut expected, offset, &rent_quote.to_le_bytes())?;
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
            delivers_creation_principal: false,
        }
    }

    fn input_with_creation<'a>(
        tail: &'a [u8],
        fields: &'a [ActivationTailFieldV1],
    ) -> ActivationBundleInputV1<'a> {
        ActivationBundleInputV1 {
            delivers_creation_principal: true,
            ..input(tail, fields)
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

    #[test]
    fn a_root_bump_bundle_projects_only_the_derived_u8_and_exact_rent_quote() {
        let mut tail = [0_u8; 128];
        tail.get_mut(..8)
            .expect("magic")
            .copy_from_slice(b"DCLTFRA1");
        tail.get_mut(8..10)
            .expect("version")
            .copy_from_slice(&2_u16.to_le_bytes());
        let fields = [
            ActivationTailFieldV1::RootBumpWord { offset: 8 },
            ActivationTailFieldV1::SeamIdentity {
                offset: 16,
                register: 8,
            },
            ActivationTailFieldV1::SeamIdentity {
                offset: 48,
                register: 4,
            },
            ActivationTailFieldV1::SeamIdentity {
                offset: 80,
                register: 5,
            },
            ActivationTailFieldV1::RentQuoteWord { offset: 120 },
        ];
        let bundle = build_activation_bundle_v1(input(&tail, &fields)).expect("V2 bundle");

        let mut scalars = [0_u64; ACTIVATION_COMMON_SCALARS_V3];
        *scalars
            .get_mut(usize::from(ACTIVATION_ROOT_BUMP_SCALAR_V3))
            .expect("bump scalar") = 0xfe;
        let mut identities = [[0_u8; 32]; ACTIVATION_COMMON_IDENTITIES_V2];
        *identities.get_mut(8).expect("config") = [0xc8; 32];
        *identities.get_mut(4).expect("market") = [0x4d; 32];
        *identities.get_mut(5).expect("context") = [0xbe; 32];
        let rent_quote = 2_672_640_u64;
        let (projected, lamports) = project_activation_root_tail_v2(
            &bundle,
            ActivationSeamImageV2 {
                scalars: &scalars,
                identities: &identities,
                rent_quote,
            },
        )
        .expect("V2 projection");

        let mut expected = tail;
        *expected.get_mut(10).expect("bump byte") = 0xfe;
        expected
            .get_mut(16..48)
            .expect("config")
            .copy_from_slice(&[0xc8; 32]);
        expected
            .get_mut(48..80)
            .expect("market")
            .copy_from_slice(&[0x4d; 32]);
        expected
            .get_mut(80..112)
            .expect("beneficiary")
            .copy_from_slice(&[0xbe; 32]);
        expected
            .get_mut(120..128)
            .expect("rent")
            .copy_from_slice(&rent_quote.to_le_bytes());
        assert_eq!(projected, expected);
        assert_eq!(lamports, [rent_quote, 0]);

        *scalars
            .get_mut(usize::from(ACTIVATION_ROOT_BUMP_SCALAR_V3))
            .expect("bump scalar") = 256;
        assert_eq!(
            project_activation_root_tail_v2(
                &bundle,
                ActivationSeamImageV2 {
                    scalars: &scalars,
                    identities: &identities,
                    rent_quote,
                },
            ),
            Err(ActivationBundleErrorV1::RegisterGeometry)
        );

        // The V3 seam's identity width is now named in V3 terms, not borrowed
        // from V2 on the same line as the V3 scalar width. The two are equal
        // today, so this hostile is what keeps the NAME load-bearing: if the
        // V3 identity bank ever widens, one bank short must still refuse
        // RegisterGeometry rather than project a short read as a valid tail.
        *scalars
            .get_mut(usize::from(ACTIVATION_ROOT_BUMP_SCALAR_V3))
            .expect("bump scalar") = 0xfe;
        let short = [[0_u8; 32]; ACTIVATION_COMMON_IDENTITIES_V3 - 1];
        assert_eq!(
            project_activation_root_tail_v2(
                &bundle,
                ActivationSeamImageV2 {
                    scalars: &scalars,
                    identities: &short,
                    rent_quote,
                },
            ),
            Err(ActivationBundleErrorV1::RegisterGeometry),
            "an identity bank narrower than ACTIVATION_COMMON_IDENTITIES_V3 is not the V3 seam"
        );
        let wide = [[0_u8; 32]; ACTIVATION_COMMON_IDENTITIES_V3 + 1];
        assert_eq!(
            project_activation_root_tail_v2(
                &bundle,
                ActivationSeamImageV2 {
                    scalars: &scalars,
                    identities: &wide,
                    rent_quote,
                },
            ),
            Err(ActivationBundleErrorV1::RegisterGeometry),
            "a wider identity bank is not the V3 seam either: the width is exact"
        );
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
    fn a_declared_creation_principal_is_projected_summed_and_delivered_whole() {
        const RENT: u64 = 2_672_640;
        const CREATION: u64 = 5_000;
        let tail = mixed_tail();
        let plain = build_activation_bundle_v1(input(&tail, &[])).expect("plain bundle");
        let bundle =
            build_activation_bundle_v1(input_with_creation(&tail, &[])).expect("creation bundle");

        // Declaring the principal changes the artifacts; not declaring it must
        // leave every family that does not byte-identical.
        assert_ne!(plain.account_profile, bundle.account_profile);
        assert_ne!(plain.transition, bundle.transition);
        assert_ne!(plain.effect, bundle.effect);

        // The root opens holding BOTH native compartments and the ledger empties.
        let (composed, lamports) = project_activation_root_tail_v3(
            &bundle,
            ActivationSeamImageV3 {
                scalars: &[0_u64; ACTIVATION_COMMON_SCALARS_V2],
                identities: &[[0_u8; 32]; ACTIVATION_COMMON_IDENTITIES_V2],
                rent_quote: RENT,
                creation_quote: CREATION,
            },
        )
        .expect("projection");
        assert_eq!(lamports, [RENT + CREATION, 0]);
        // The tail is unchanged by the funding: a creation principal funds the
        // root, it does not compose a byte of family state.
        let (plain_tail, plain_lamports) = project_activation_root_tail_v1(
            &plain,
            ActivationSeamImageV1 {
                scalars: &[0_u64; ACTIVATION_COMMON_SCALARS_V2],
                identities: &[[0_u8; 32]; ACTIVATION_COMMON_IDENTITIES_V2],
                rent_quote: RENT,
            },
        )
        .expect("plain projection");
        assert_eq!(composed, plain_tail);
        assert_eq!(plain_lamports, [RENT, 0]);
    }

    #[test]
    fn a_creation_principal_cannot_be_combined_with_the_root_bump_bank() {
        let mut tail = [0_u8; 56];
        tail.get_mut(0..8)
            .expect("magic")
            .copy_from_slice(&0x4443_4c54_4d49_5831_u64.to_le_bytes());
        assert_eq!(
            build_activation_bundle_v1(input_with_creation(
                &tail,
                &[ActivationTailFieldV1::RootBumpWord { offset: 8 }],
            )),
            Err(ActivationBundleErrorV1::CreationPrincipalWithRootBump)
        );
    }

    #[test]
    fn a_bundle_does_not_rejoin_across_the_creation_declaration() {
        let tail = mixed_tail();
        let plain = build_activation_bundle_v1(input(&tail, &[])).expect("plain bundle");
        let bundle =
            build_activation_bundle_v1(input_with_creation(&tail, &[])).expect("creation bundle");
        // Each rejoins only to the declaration it was built under, so a release
        // cannot publish a rent-only triple against a creation-funded manifest.
        assert!(validate_activation_bundle_v1(&plain, input_with_creation(&tail, &[])).is_err());
        assert!(validate_activation_bundle_v1(&bundle, input(&tail, &[])).is_err());
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
