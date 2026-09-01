//! One canonical four-action selected Fractional release.
//!
//! [`build_fractional_selected_bundle_v4`] compiles the artifacts for a single
//! exposure action and is consumed only by its own tests. This module is the
//! missing caller: it derives the four public Claims frame specs from the
//! Claims-owned frame contract, compiles one bundle per supported action, joins
//! them into one `CapabilityProgramSetV2` selected by the request action byte,
//! and emits the canonical publication a Market manifest binds.
//!
//! Two properties are deliberate.
//!
//! The frame specs are **derived, never supplied**. A release tool cannot pair
//! a selector with a frame the Claims child will not actually demand, because
//! every privilege comes from [`SignedDeltaFrameSpecV3`] and the Fractional
//! terminal contract rather than from an argument. This is the same rule Direct
//! applies when it refuses to pair a selector with an arbitrary descriptor.
//!
//! Every external byte width the profile depends on is **named**. Claims frames
//! contain coordinates whose width no release compiler can know: wallets, Mints,
//! Token accounts, sysvars, and deployment-sized ProgramData. Those become
//! `AuthenticatedOpaqueReadonlyData`. The widths that a release genuinely does
//! select — Core Market, activation cache, RentCredit, and the four Product and
//! basis records — are supplied through [`FractionalFrameWidthsV4`] and refused
//! when zero.

extern crate alloc;

use alloc::{vec, vec::Vec};

use dclutch_capability_activation_codec::{
    ActivationBundleErrorV1, ActivationBundleInputV1, ActivationBundleV1, ActivationTailFieldV1,
    activation_account_profile_schema_v1, activation_descriptor_schema_v1,
    activation_effect_schema_v1, build_activation_bundle_v1, validate_activation_bundle_v1,
};
use dclutch_capability_program_contract::{
    activation_registers_v2::{
        ACTIVATION_CONFIG_IDENTITY_V2, ACTIVATION_CONTEXT_IDENTITY_V2,
        ACTIVATION_MARKET_IDENTITY_V2,
    },
    set_v2::{
        CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2, CapabilityDescriptorReferenceV2,
        CapabilityProgramSetEntryV2, CapabilityProgramSetV2, SelectorWidthV2,
        encode_program_set_v2, encoded_program_set_bytes_v2,
    },
    v4::{CapabilityProgramV4, SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_ID_V4},
};
use dclutch_claims_svm::{
    frame_spec_v1::{ClaimsFrameDataV1, SignedDeltaFrameSpecV3},
    terminal_settlement_v3::{
        TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3, TERMINAL_SETTLEMENT_CERTIFICATE_ACCOUNT_V3,
        TERMINAL_SETTLEMENT_COLLATERAL_MINT_ACCOUNT_V3,
        TERMINAL_SETTLEMENT_CUSTODY_AUTHORITY_ACCOUNT_V3,
        TERMINAL_SETTLEMENT_CUSTODY_CALLER_ACCOUNT_V3,
        TERMINAL_SETTLEMENT_CUSTODY_PROGRAM_ACCOUNT_V3,
        TERMINAL_SETTLEMENT_CUSTODY_REPLAY_ACCOUNT_V3, TERMINAL_SETTLEMENT_EXPOSURE_RAW_ACCOUNT_V3,
        TERMINAL_SETTLEMENT_EXPOSURE_STAGING_ACCOUNT_V3, TERMINAL_SETTLEMENT_HOARD_ACCOUNT_V3,
        TERMINAL_SETTLEMENT_REALM_ACCOUNT_V3, TERMINAL_SETTLEMENT_REALM_STAGING_ACCOUNT_V3,
        TERMINAL_SETTLEMENT_RECIPIENT_ACCOUNT_V3,
        TERMINAL_SETTLEMENT_RESOLUTION_PROGRAM_ACCOUNT_V3,
        TERMINAL_SETTLEMENT_RESOLUTION_PROGRAMDATA_ACCOUNT_V3,
        TERMINAL_SETTLEMENT_TOKEN_PROGRAM_ACCOUNT_V3,
    },
};
use dclutch_core_contract::ContentId;
use dclutch_fractional_claim_contract::{
    FRACTIONAL_ATOMIC_ACCOUNT_COUNT_V3, FRACTIONAL_ATOMIC_ACTOR_V3,
    FRACTIONAL_ATOMIC_HOLDER_TOKEN_V3, FRACTIONAL_ATOMIC_ROOT_V3, FRACTIONAL_ATOMIC_SHARD_MINT_V3,
    FRACTIONAL_ATOMIC_SIGNED_DELTA_ACCOUNT_COUNT_V3, FRACTIONAL_ATOMIC_TERMS_RAW_V3,
    FRACTIONAL_ATOMIC_TERMS_STAGING_V3, FRACTIONAL_ATOMIC_TOKEN_BEHAVIOR_RAW_V3,
    FRACTIONAL_ATOMIC_TOKEN_BEHAVIOR_STAGING_V3, FRACTIONAL_ATOMIC_TOKEN_PROGRAM_V3,
    FRACTIONAL_CAPABILITY_KIND_ID_V1, FRACTIONAL_CAPABILITY_ROOT_BYTES_V4,
    FRACTIONAL_EXPOSURE_REQUEST_ACTION_OFFSET_V2, FRACTIONAL_EXPOSURE_REQUEST_BYTES_V2,
    FRACTIONAL_ROOT_BYTES_V1, FRACTIONAL_ROOT_MARKET_OFFSET_V1,
    FRACTIONAL_ROOT_RENT_BENEFICIARY_OFFSET_V1, FRACTIONAL_ROOT_RENT_PRINCIPAL_OFFSET_V1,
    FRACTIONAL_ROOT_SCHEMA_ID_V2, FRACTIONAL_ROOT_SELECTION_CONFIG_OFFSET_V2,
    FRACTIONAL_TERMINAL_ACCOUNT_COUNT_V3, FRACTIONAL_TERMINAL_ACTOR_V3,
    FRACTIONAL_TERMINAL_ROOT_V3, FRACTIONAL_TERMINAL_SHARD_MINT_V3,
    FRACTIONAL_TERMINAL_SOURCE_TOKEN_V3, FRACTIONAL_TERMINAL_TERMS_RAW_V3,
    FRACTIONAL_TERMINAL_TERMS_STAGING_V3, FRACTIONAL_TERMINAL_TOKEN_BEHAVIOR_RAW_V3,
    FRACTIONAL_TERMINAL_TOKEN_BEHAVIOR_STAGING_V3, FractionalExposureActionV2,
    FractionalRootInputV2, fractional_root_creation_tail_v2,
};
use dclutch_fractional_claim_kernel::{
    FRACTIONAL_SELECTION_CONFIG_BYTES_V1, FRACTIONAL_SELECTION_CONFIG_SCHEMA_ID_V1,
    FractionalExposureTermsV2, FractionalSelectionConfigV1, encode_fractional_selection_config_v1,
    fractional_selection_config_from_terms_v1, join_fractional_selection_config_v1,
};
use sha2::{Digest, Sha256};

use crate::{
    FractionalClaimsAccountRuleV1, FractionalSelectedBundleInputV4, FractionalSelectedBundleV4,
    FractionalSelectedProfileInputV4, build_fractional_current_selected_bundle_v4,
    build_fractional_selected_bundle_v4, validate_fractional_current_selected_bundle_v4,
    validate_fractional_selected_bundle_v4,
};

/// Exact executable action count published by one Fractional release.
pub const FRACTIONAL_SELECTED_ACTION_COUNT_V4: usize = 4;

/// The only four actions a Fractional release publishes, in ascending selector
/// order.
///
/// `Transfer` is absent because it is an ordinary Token-2022 `TransferChecked`
/// that never enters the family caller. `Terminalize` and `ZeroSupplyRetire`
/// are absent because neither has a production Claims handler on this rung.
pub const FRACTIONAL_SELECTED_ACTIONS_V4: [FractionalExposureActionV2;
    FRACTIONAL_SELECTED_ACTION_COUNT_V4] = [
    FractionalExposureActionV2::Wrap,
    FractionalExposureActionV2::WholeUnwrap,
    FractionalExposureActionV2::TerminalRedeem,
    FractionalExposureActionV2::TerminalZeroBurn,
];

/// Reserved selector that reaches the sole Fractional root activation
/// descriptor. No historical exposure action uses this byte.
pub const FRACTIONAL_ACTIVATION_SELECTOR_V1: u32 = 255;
/// Exact activation selector-request width.
pub const FRACTIONAL_ACTIVATION_REQUEST_BYTES_V1: usize = 16;
/// Domain-separating activation selector-request magic.
pub const FRACTIONAL_ACTIVATION_REQUEST_MAGIC_V1: [u8; 8] = *b"DCFRACT1";
/// Activation selector-request schema preimage.
pub const FRACTIONAL_ACTIVATION_REQUEST_SCHEMA_PREIMAGE_V1: &[u8] =
    b"dclutch/schema/fractional-activation-request-v1";
/// SHA-256 of [`FRACTIONAL_ACTIVATION_REQUEST_SCHEMA_PREIMAGE_V1`].
pub const FRACTIONAL_ACTIVATION_REQUEST_SCHEMA_ID_V1: [u8; 32] = [
    0x5b, 0xac, 0x28, 0xad, 0x53, 0x99, 0x7f, 0xae, 0xdb, 0x69, 0xf9, 0x3a, 0xd5, 0x32, 0xa2, 0x33,
    0x7d, 0x5c, 0x7e, 0x63, 0xb9, 0xf3, 0x54, 0xb7, 0x98, 0x58, 0x61, 0x87, 0x6d, 0x02, 0x46, 0xbd,
];

/// The one funding-ledger row a current Fractional activation projects.
///
/// Chain-derived: Fractional currently has one selected capability entry and
/// therefore one pre-paid root-rent row. A wider funding shape needs a
/// deliberately wider family activation profile, not an unchecked argument.
pub const FRACTIONAL_SELECTED_FUNDING_LEDGER_SLOTS_V1: u16 = 1;

/// Exact current activation tail fields, in ascending, disjoint root order.
///
/// The generic activation codec owns the bump expression and the rent quote;
/// this family merely names the V2 seam coordinates the state decoder needs.
pub const FRACTIONAL_ACTIVATION_TAIL_FIELDS_V1: [ActivationTailFieldV1; 5] = [
    ActivationTailFieldV1::RootBumpWord { offset: 8 },
    ActivationTailFieldV1::SeamIdentity {
        offset: FRACTIONAL_ROOT_SELECTION_CONFIG_OFFSET_V2 as u32,
        register: ACTIVATION_CONFIG_IDENTITY_V2,
    },
    ActivationTailFieldV1::SeamIdentity {
        offset: FRACTIONAL_ROOT_MARKET_OFFSET_V1 as u32,
        register: ACTIVATION_MARKET_IDENTITY_V2,
    },
    ActivationTailFieldV1::SeamIdentity {
        offset: FRACTIONAL_ROOT_RENT_BENEFICIARY_OFFSET_V1 as u32,
        register: ACTIVATION_CONTEXT_IDENTITY_V2,
    },
    ActivationTailFieldV1::RentQuoteWord {
        offset: FRACTIONAL_ROOT_RENT_PRINCIPAL_OFFSET_V1 as u32,
    },
];

/// The widest representation a published Fractional capability may name.
///
/// This is not the index space's bound. The `U8` action selector, the terms
/// codec, and `MAX_COMPOSITION_REPRESENTATION_WIDTH_V3` all admit 256, and none
/// of them is wrong. The bound here is narrower because a *terminal settlement*
/// translates every Product result coordinate onto every Claims representation
/// root, and that work grows until it exhausts a transaction:
///
/// ```text
///   width   8   16   32   48   64    96      98      99
///   units 463k 519k 593k 731k 897k 1356k   1393k   exhausted
/// ```
///
/// The arithmetic maximum is 98, and this constant is deliberately not 98. At
/// that width the margin is 6,672 units out of 1,400,000 -- under half a
/// percent -- which is inside build-to-build variation: width 98 settles
/// against one build of the same committed source and exhausts the budget
/// against another. A published bound may not depend on which machine compiled
/// Claims, so the supported width keeps roughly a third of the budget in
/// reserve instead.
///
/// Measured against real ELFs by
/// `the_terminal_settlement_has_headroom_at_the_supported_width_and_none_far_above_it`
/// in `programs/dclutch-claims-sbf/program-test/fractional-atomic/`.
///
/// A release publishes `TerminalRedeem` and `TerminalZeroBurn` among its four
/// actions, so publishing above this width would publish a capability that can
/// be wrapped into and never redeemed from -- the open-market actions do no
/// Product evaluation and stay cheap, so nothing would refuse until holders
/// tried to exit. Refusing here is the earliest point that trap can be closed.
///
/// This constant and the campaign that measures it must move together: if the
/// terminal path's compute cost changes, that test fails rather than this
/// number silently becoming a lie.
pub const FRACTIONAL_MAX_SETTLEABLE_WIDTH_V4: u32 = 64;

/// Canonical publication magic.
pub const FRACTIONAL_SELECTED_PUBLICATION_MAGIC_V4: [u8; 8] = *b"DCFRPB04";
/// Exact canonical publication width.
pub const FRACTIONAL_SELECTED_PUBLICATION_BYTES_V4: usize = 512;

/// Publication layout version.
///
/// Bumped from 4 to 5 by the config split: the publication gained a fifteenth
/// identity (the market-free selection config the manifest entry now names).
/// The magic still names the family; the version names the layout, which is
/// exactly the distinction the version field exists for. This is a
/// PRE-RELEASE wire change -- no Fractional descriptor exists in any published
/// release set on any chain -- and not a migration.
const PUBLICATION_VERSION: u16 = 5;
const IDENTITY_START: usize = 16;
const IDENTITY_COUNT: usize = 15;
const SCALAR_START: usize = IDENTITY_START + IDENTITY_COUNT * 32;

/// Exact selection-config record width as a frame width.
///
/// Welded to the kernel's constant at compile time rather than restated: if
/// the selection config ever widens, this fails to build instead of silently
/// compiling a release whose config slot expects the old width.
const SELECTION_CONFIG_BYTES: u32 = 128;
const _: () = assert!(FRACTIONAL_SELECTION_CONFIG_BYTES_V1 == 128);

/// Byte widths a release selects but the Claims frame contract cannot supply.
///
/// Every field is an authenticated observation of one finalized account. A zero
/// is refused rather than silently compiled into a rule that no account can
/// satisfy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalFrameWidthsV4 {
    /// Finalized linked-basis Record checked prefix.
    pub linked_basis_record: u32,
    /// Finalized Product root Record.
    pub product_record: u32,
    /// Finalized Product ResultDomain Record.
    pub result_domain_record: u32,
    /// Finalized Product portfolio Record.
    pub portfolio_record: u32,
    /// Canonical Core Market state.
    pub core_market: u32,
    /// Current Registry activation cache.
    pub activation_cache: u32,
    /// Canonical Claims Position RentCredit state.
    pub rent_credit: u32,
}

impl FractionalFrameWidthsV4 {
    fn checked(self) -> Result<Self, FractionalSelectedReleaseErrorV4> {
        if [
            self.linked_basis_record,
            self.product_record,
            self.result_domain_record,
            self.portfolio_record,
            self.core_market,
            self.activation_cache,
            self.rent_credit,
        ]
        .contains(&0)
        {
            return Err(FractionalSelectedReleaseErrorV4::Widths);
        }
        Ok(self)
    }

    fn profile(self) -> FractionalSelectedProfileInputV4 {
        FractionalSelectedProfileInputV4 {
            // DERIVED, never supplied. The manifest-named config is the
            // market-free selection config, whose width is a constant this
            // release compiler knows. Taking it as a caller argument would let
            // a release pair its config slot with a width no selection config
            // can ever have -- the same trap the frame specs already refuse.
            selected_config_bytes: SELECTION_CONFIG_BYTES,
            product_record_bytes: self.product_record,
            portfolio_record_bytes: self.portfolio_record,
            linked_basis_bytes: self.linked_basis_record,
        }
    }
}

/// Complete authenticated input for one selected Fractional release.
#[derive(Clone, Copy, Debug)]
pub struct FractionalSelectedReleaseInputV4<'a> {
    /// Sole durable owner of the shard Mints, denominator, and bases.
    pub terms: FractionalExposureTermsV2<'a>,
    /// Manifest-selected physical capacity profile.
    pub capacity_profile: [u8; 32],
    /// Release-selected external widths.
    pub widths: FractionalFrameWidthsV4,
}

/// Canonical Market-bindable publication for one selected Fractional release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalSelectedPublicationV4 {
    /// Execution release set that admits the four capabilities.
    pub release_set: [u8; 32],
    /// Logical Core Market.
    pub market: [u8; 32],
    /// Finalized Product graph-root digest.
    pub product_record: [u8; 32],
    /// Product-owned ResultDomain digest.
    pub result_domain: [u8; 32],
    /// Exact Fractional exposure terms digest.
    ///
    /// The EXECUTION record. Not the identity the manifest entry names -- see
    /// [`Self::selection_config`]. It binds the Core Market, so an entry
    /// naming it would be the fixed point the split exists to remove.
    pub terms: [u8; 32],
    /// Exact market-free selection config digest.
    ///
    /// THIS is the identity a capability manifest entry carries as its
    /// `config_id`. It is derivable before the Market address exists, which is
    /// the whole property that makes a Fractional-selected market foundable.
    pub selection_config: [u8; 32],
    /// Finalized TokenBehaviorV2 selection digest.
    pub token_behavior: [u8; 32],
    /// Finalized composition exposure digest.
    pub exposure: [u8; 32],
    /// Terms-selected Token-2022 program.
    pub token_program: [u8; 32],
    /// Manifest-selected physical capacity profile.
    pub capacity_profile: [u8; 32],
    /// SHA-256 identity of the four-entry ProgramSetV2 bytes.
    pub program_set_id: [u8; 32],
    /// Descriptor identities in [`FRACTIONAL_SELECTED_ACTIONS_V4`] order.
    pub descriptors: [[u8; 32]; FRACTIONAL_SELECTED_ACTION_COUNT_V4],
    /// Exact integer denominator.
    pub denominator: u64,
    /// Exact Product outcome width.
    pub product_width: u32,
    /// Exact representation shard width.
    pub representation_width: u32,
}

impl FractionalSelectedPublicationV4 {
    /// Exact canonical publication bytes.
    #[must_use]
    pub fn to_bytes(&self) -> [u8; FRACTIONAL_SELECTED_PUBLICATION_BYTES_V4] {
        let mut output = [0_u8; FRACTIONAL_SELECTED_PUBLICATION_BYTES_V4];
        output[..8].copy_from_slice(&FRACTIONAL_SELECTED_PUBLICATION_MAGIC_V4);
        output[8..10].copy_from_slice(&PUBLICATION_VERSION.to_le_bytes());
        // SCALAR_START is defined as IDENTITY_START + IDENTITY_COUNT * 32 and
        // `identities` returns exactly IDENTITY_COUNT of them, so the
        // identities tile IDENTITY_START..SCALAR_START by construction and the
        // per-field offset was only ever that fact restated. Both halves of the
        // tiling are type-level here, so there is nothing left for an assert to
        // catch.
        for (slot, identity) in output[IDENTITY_START..SCALAR_START]
            .chunks_exact_mut(32)
            .zip(self.identities().iter())
        {
            slot.copy_from_slice(identity);
        }
        output[SCALAR_START..SCALAR_START + 8].copy_from_slice(&self.denominator.to_le_bytes());
        output[SCALAR_START + 8..SCALAR_START + 12]
            .copy_from_slice(&self.product_width.to_le_bytes());
        output[SCALAR_START + 12..SCALAR_START + 16]
            .copy_from_slice(&self.representation_width.to_le_bytes());
        output
    }

    /// SHA-256 identity of [`Self::to_bytes`] with no extra domain prefix.
    #[must_use]
    pub fn publication_id(&self) -> [u8; 32] {
        digest(&self.to_bytes())
    }

    fn identities(&self) -> [[u8; 32]; IDENTITY_COUNT] {
        [
            self.release_set,
            self.market,
            self.product_record,
            self.result_domain,
            self.terms,
            self.selection_config,
            self.token_behavior,
            self.exposure,
            self.token_program,
            self.capacity_profile,
            self.program_set_id,
            self.descriptors[0],
            self.descriptors[1],
            self.descriptors[2],
            self.descriptors[3],
        ]
    }
}

/// One complete selected Fractional release.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FractionalSelectedReleaseV4 {
    /// Action bundles in [`FRACTIONAL_SELECTED_ACTIONS_V4`] order.
    pub bundles: Vec<FractionalSelectedBundleV4>,
    /// Exact four-entry CapabilityProgramSetV2 bytes.
    pub program_set: Vec<u8>,
    /// SHA-256 identity of `program_set`.
    pub program_set_id: [u8; 32],
    /// Exact market-free selection config record bytes.
    ///
    /// These are the bytes a Registry finalizes under
    /// `FRACTIONAL_SELECTION_CONFIG_SCHEMA_ID_V1` and the bytes the selection
    /// seam hashes into the manifest entry's config identity. They contain no
    /// Market and nothing derived from one.
    pub selection_config: Vec<u8>,
    /// SHA-256 identity of `selection_config`.
    pub selection_config_id: [u8; 32],
    /// Canonical Market-bindable publication.
    pub publication: FractionalSelectedPublicationV4,
}

/// The append-only current Fractional release: four historical V4 action
/// artifacts plus the one V1 activation coordinate that creates their V2 root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FractionalCurrentReleaseV4 {
    /// Current V2-root action bundles in canonical action order.
    pub bundles: Vec<FractionalSelectedBundleV4>,
    /// Exact market-free config the current actions and activation inherit.
    pub selection_config: Vec<u8>,
    /// SHA-256 identity of `selection_config`.
    pub selection_config_id: [u8; 32],
    /// Family-neutral activation artifacts specialized to the Fractional V2 tail.
    pub activation: ActivationBundleV1,
    /// Exact five-entry current ProgramSetV2 bytes.
    pub program_set: Vec<u8>,
    /// SHA-256 identity of the current activation-capable program set.
    pub program_set_id: [u8; 32],
}

/// Stable selected-release refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FractionalSelectedReleaseErrorV4 {
    /// Authenticated terms were not usable as a release identity source.
    Terms,
    /// A release-selected external byte width was zero.
    Widths,
    /// The representation is wider than a terminal settlement can execute.
    ///
    /// See [`FRACTIONAL_MAX_SETTLEABLE_WIDTH_V4`]. Such a capability would
    /// admit wraps it could never settle.
    Unsettleable,
    /// Canonical Claims frame derivation refused.
    Frame,
    /// One action bundle did not compile or rejoin under its own compiler.
    Bundle,
    /// ProgramSetV2 encoding, decoding, or selection refused.
    ProgramSet,
    /// The market-free selection config did not encode, decode, or join.
    ///
    /// A join failure here means the release's own config and terms describe
    /// different instruments -- caught at compile time rather than left for
    /// the chain to discover at execution.
    SelectionConfig,
    /// Publication identities or scalars were not exact.
    Publication,
    /// The append-only current activation descriptor/profile/effect refused.
    Activation,
}

/// Complete input for the current activation-capable Fractional release.
#[derive(Clone, Copy, Debug)]
pub struct FractionalActivationBundleInputV1<'a> {
    /// One current V4 action descriptor from the selected release.
    ///
    /// All selected action descriptors must agree on inherited coordinates;
    /// this constructor reads them from this exact byte body instead of
    /// restating a second source of truth.
    pub action_descriptor: &'a [u8],
}

/// Stable Fractional activation construction or validation refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FractionalActivationBundleErrorV1 {
    /// The supplied action descriptor did not decode as CapabilityProgramV4.
    ActionDescriptor,
    /// The action did not name the current Fractional V2 root schema and width.
    ForeignRoot,
    /// The family creation oracle or its declared seam coverage refused.
    CreationTail,
    /// Activation selector request bytes were not canonical.
    Request,
    /// The generic activation codec refused the derived template.
    Template(ActivationBundleErrorV1),
}

/// Encode the sole canonical Fractional activation selector request.
#[must_use]
pub fn fractional_activation_request_v1() -> [u8; FRACTIONAL_ACTIVATION_REQUEST_BYTES_V1] {
    let mut output = [0_u8; FRACTIONAL_ACTIVATION_REQUEST_BYTES_V1];
    output[..8].copy_from_slice(&FRACTIONAL_ACTIVATION_REQUEST_MAGIC_V1);
    output[8..10].copy_from_slice(&1_u16.to_le_bytes());
    output[FRACTIONAL_EXPOSURE_REQUEST_ACTION_OFFSET_V2] = u8::MAX;
    output
}

/// Hostile-check one Fractional activation selector request.
pub fn validate_fractional_activation_request_v1(
    bytes: &[u8],
) -> core::result::Result<(), FractionalActivationBundleErrorV1> {
    if bytes != fractional_activation_request_v1() {
        return Err(FractionalActivationBundleErrorV1::Request);
    }
    Ok(())
}

/// Build the canonical V2-root activation descriptor/profile/effect bundle.
///
/// The generic codec remains the sole semantic owner of activation artifacts.
/// This wrapper derives a Fractional V2 root tail from its creation oracle and
/// supplies the family-specific seam field contract; it never writes an
/// activation effect, profile, or transition itself.
pub fn build_fractional_activation_bundle_v1(
    input: FractionalActivationBundleInputV1<'_>,
) -> core::result::Result<ActivationBundleV1, FractionalActivationBundleErrorV1> {
    let (template, tail) = fractional_activation_template_v1(input)?;
    build_activation_bundle_v1(template.as_input(&tail))
        .map_err(FractionalActivationBundleErrorV1::Template)
}

/// Rejoin one activation bundle to its inherited current action descriptor.
pub fn validate_fractional_activation_bundle_v1(
    bundle: &ActivationBundleV1,
    input: FractionalActivationBundleInputV1<'_>,
) -> core::result::Result<(), FractionalActivationBundleErrorV1> {
    let (template, tail) = fractional_activation_template_v1(input)?;
    validate_activation_bundle_v1(bundle, template.as_input(&tail))
        .map_err(FractionalActivationBundleErrorV1::Template)
}

#[derive(Clone, Copy)]
struct FractionalActivationInheritedV1 {
    kind: ContentId,
    config_schema: ContentId,
    root_schema: ContentId,
    derivation_policy: ContentId,
    capacity_profile: ContentId,
    root_state_bytes: u32,
    request_schema: ContentId,
}

impl FractionalActivationInheritedV1 {
    fn as_input<'a>(self, tail: &'a [u8]) -> ActivationBundleInputV1<'a> {
        ActivationBundleInputV1 {
            kind: self.kind,
            config_schema: self.config_schema,
            request_schema: self.request_schema,
            root_schema: self.root_schema,
            derivation_policy: self.derivation_policy,
            capacity_profile: self.capacity_profile,
            root_state_bytes: self.root_state_bytes,
            constant_root_tail: tail,
            seam_fields: &FRACTIONAL_ACTIVATION_TAIL_FIELDS_V1,
            funding_ledger_slot_count: FRACTIONAL_SELECTED_FUNDING_LEDGER_SLOTS_V1,
        }
    }
}

fn fractional_activation_template_v1(
    input: FractionalActivationBundleInputV1<'_>,
) -> core::result::Result<
    (
        FractionalActivationInheritedV1,
        [u8; FRACTIONAL_ROOT_BYTES_V1],
    ),
    FractionalActivationBundleErrorV1,
> {
    let action = CapabilityProgramV4::decode(input.action_descriptor)
        .map_err(|_| FractionalActivationBundleErrorV1::ActionDescriptor)?;
    if action.kind().to_bytes() != FRACTIONAL_CAPABILITY_KIND_ID_V1
        || action.config_schema().to_bytes() != FRACTIONAL_SELECTION_CONFIG_SCHEMA_ID_V1
        || action.root_schema().to_bytes() != FRACTIONAL_ROOT_SCHEMA_ID_V2
        || action.root_state_bytes()
            != u32::try_from(FRACTIONAL_ROOT_BYTES_V1)
                .map_err(|_| FractionalActivationBundleErrorV1::ForeignRoot)?
    {
        return Err(FractionalActivationBundleErrorV1::ForeignRoot);
    }
    let mut tail = fractional_root_creation_tail_v2(FractionalRootInputV2 {
        bump: 0,
        selection_config: [0x11; 32],
        market: [0x22; 32],
        rent_beneficiary: [0x33; 32],
        revision: 1,
        historical_rent_principal: 1,
    })
    .ok_or(FractionalActivationBundleErrorV1::CreationTail)?;
    // The bump field retains the V2 header word (version plus a blank bump),
    // which `RootBumpWord` reads as its family-authored base. Every other seam
    // field is blanked, proving it has exactly one runtime author.
    for field in FRACTIONAL_ACTIVATION_TAIL_FIELDS_V1 {
        if matches!(field, ActivationTailFieldV1::RootBumpWord { .. }) {
            continue;
        }
        let start = usize::try_from(field.offset())
            .map_err(|_| FractionalActivationBundleErrorV1::CreationTail)?;
        let end = start
            .checked_add(field.width())
            .ok_or(FractionalActivationBundleErrorV1::CreationTail)?;
        tail.get_mut(start..end)
            .ok_or(FractionalActivationBundleErrorV1::CreationTail)?
            .fill(0);
    }
    Ok((
        FractionalActivationInheritedV1 {
            kind: action.kind(),
            config_schema: action.config_schema(),
            root_schema: action.root_schema(),
            derivation_policy: action.derivation_policy(),
            capacity_profile: action.capacity_profile(),
            root_state_bytes: action.root_state_bytes(),
            request_schema: ContentId::new(FRACTIONAL_ACTIVATION_REQUEST_SCHEMA_ID_V1)
                .map_err(|_| FractionalActivationBundleErrorV1::CreationTail)?,
        },
        tail,
    ))
}

/// Derive the exact public Claims child frame one action will actually demand.
///
/// Privileges come from the Claims-owned frame contract and the Fractional
/// terminal layout. Widths come from the Claims data contract, the Product
/// width carried by the authenticated terms, and the named release widths.
/// Nothing here is caller-selected.
pub fn fractional_claims_frame_spec_v4(
    action: FractionalExposureActionV2,
    terms: FractionalExposureTermsV2<'_>,
    widths: FractionalFrameWidthsV4,
) -> Result<Vec<FractionalClaimsAccountRuleV1>, FractionalSelectedReleaseErrorV4> {
    let widths = widths.checked()?;
    match action {
        FractionalExposureActionV2::Wrap | FractionalExposureActionV2::WholeUnwrap => {
            atomic_frame(terms, widths)
        }
        FractionalExposureActionV2::TerminalRedeem
        | FractionalExposureActionV2::TerminalZeroBurn => terminal_frame(terms, widths),
        _ => Err(FractionalSelectedReleaseErrorV4::Frame),
    }
}

/// Compile, join, and hostile-rejoin one complete four-action Fractional release.
pub fn fractional_selected_release_v4(
    input: FractionalSelectedReleaseInputV4<'_>,
) -> Result<FractionalSelectedReleaseV4, FractionalSelectedReleaseErrorV4> {
    let widths = input.widths.checked()?;
    if input.capacity_profile == [0; 32]
        || input.terms.terms_id() == [0; 32]
        || input.terms.market() == [0; 32]
        || input.terms.release_set() == [0; 32]
        || input.terms.denominator() <= 1
        || input.terms.product_width() == 0
        || input.terms.representation_width() == 0
    {
        return Err(FractionalSelectedReleaseErrorV4::Terms);
    }
    if input.terms.representation_width() > FRACTIONAL_MAX_SETTLEABLE_WIDTH_V4 {
        return Err(FractionalSelectedReleaseErrorV4::Unsettleable);
    }
    let profile = widths.profile();
    let mut bundles = Vec::with_capacity(FRACTIONAL_SELECTED_ACTION_COUNT_V4);
    let mut descriptors = [[0_u8; 32]; FRACTIONAL_SELECTED_ACTION_COUNT_V4];
    // `descriptors` and FRACTIONAL_SELECTED_ACTIONS_V4 are both
    // [_; FRACTIONAL_SELECTED_ACTION_COUNT_V4], so walking them together visits
    // exactly the same pairs the index did, and the lengths cannot disagree.
    for (slot, action) in descriptors
        .iter_mut()
        .zip(FRACTIONAL_SELECTED_ACTIONS_V4.iter().copied())
    {
        let frame = fractional_claims_frame_spec_v4(action, input.terms, widths)?;
        let bundle = build_fractional_selected_bundle_v4(FractionalSelectedBundleInputV4 {
            action,
            capacity_profile: input.capacity_profile,
            profile,
            claims_frame: &frame,
        })
        .map_err(|_| FractionalSelectedReleaseErrorV4::Bundle)?;
        *slot = digest(&bundle.descriptor);
        bundles.push(bundle);
    }
    let entries = program_set_entries(&descriptors)?;
    let width = encoded_program_set_bytes_v2(entries.len())
        .map_err(|_| FractionalSelectedReleaseErrorV4::ProgramSet)?;
    let mut program_set = vec![0_u8; width];
    encode_program_set_v2(
        u32::try_from(FRACTIONAL_EXPOSURE_REQUEST_ACTION_OFFSET_V2)
            .map_err(|_| FractionalSelectedReleaseErrorV4::ProgramSet)?,
        SelectorWidthV2::U8,
        &entries,
        &mut program_set,
    )
    .map_err(|_| FractionalSelectedReleaseErrorV4::ProgramSet)?;
    let program_set_id = digest(&program_set);
    let selection_config = selection_config_bytes(input.terms)?;
    let selection_config_id = digest(&selection_config);
    let publication = FractionalSelectedPublicationV4 {
        release_set: input.terms.release_set(),
        market: input.terms.market(),
        product_record: input.terms.product_record(),
        result_domain: input.terms.result_domain(),
        terms: input.terms.terms_id(),
        selection_config: selection_config_id,
        token_behavior: input.terms.token_behavior(),
        exposure: input.terms.exposure_id(),
        token_program: input.terms.token_program(),
        capacity_profile: input.capacity_profile,
        program_set_id,
        descriptors,
        denominator: input.terms.denominator(),
        product_width: input.terms.product_width(),
        representation_width: input.terms.representation_width(),
    };
    let release = FractionalSelectedReleaseV4 {
        bundles,
        program_set,
        program_set_id,
        selection_config,
        selection_config_id,
        publication,
    };
    validate_fractional_selected_release_v4(&release, input)?;
    Ok(release)
}

/// Compile the current activation-capable Fractional release.
///
/// [`fractional_selected_release_v4`] remains the historical four-action
/// constructor and retains its exact program-set bytes. This constructor is
/// append-only: it keeps those four artifacts and appends the reserved
/// activation selector under the V1 descriptor schema the Trading seam admits.
pub fn fractional_current_release_v4(
    input: FractionalSelectedReleaseInputV4<'_>,
) -> Result<FractionalCurrentReleaseV4, FractionalSelectedReleaseErrorV4> {
    let widths = input.widths.checked()?;
    if input.capacity_profile == [0; 32]
        || input.terms.terms_id() == [0; 32]
        || input.terms.market() == [0; 32]
        || input.terms.release_set() == [0; 32]
        || input.terms.denominator() <= 1
        || input.terms.product_width() == 0
        || input.terms.representation_width() == 0
    {
        return Err(FractionalSelectedReleaseErrorV4::Terms);
    }
    if input.terms.representation_width() > FRACTIONAL_MAX_SETTLEABLE_WIDTH_V4 {
        return Err(FractionalSelectedReleaseErrorV4::Unsettleable);
    }
    let mut bundles = Vec::with_capacity(FRACTIONAL_SELECTED_ACTION_COUNT_V4);
    let mut descriptors = [[0_u8; 32]; FRACTIONAL_SELECTED_ACTION_COUNT_V4];
    for (slot, action) in descriptors
        .iter_mut()
        .zip(FRACTIONAL_SELECTED_ACTIONS_V4.iter().copied())
    {
        let frame = fractional_claims_frame_spec_v4(action, input.terms, widths)?;
        let bundle = build_fractional_current_selected_bundle_v4(FractionalSelectedBundleInputV4 {
            action,
            capacity_profile: input.capacity_profile,
            profile: widths.profile(),
            claims_frame: &frame,
        })
        .map_err(|_| FractionalSelectedReleaseErrorV4::Bundle)?;
        *slot = digest(&bundle.descriptor);
        bundles.push(bundle);
    }
    let first = bundles
        .first()
        .ok_or(FractionalSelectedReleaseErrorV4::Activation)?;
    let activation = build_fractional_activation_bundle_v1(FractionalActivationBundleInputV1 {
        action_descriptor: &first.descriptor,
    })
    .map_err(|_| FractionalSelectedReleaseErrorV4::Activation)?;
    let entries = current_program_set_entries(&descriptors, activation.descriptor_id)?;
    let width = encoded_program_set_bytes_v2(entries.len())
        .map_err(|_| FractionalSelectedReleaseErrorV4::ProgramSet)?;
    let mut program_set = vec![0_u8; width];
    encode_program_set_v2(
        u32::try_from(FRACTIONAL_EXPOSURE_REQUEST_ACTION_OFFSET_V2)
            .map_err(|_| FractionalSelectedReleaseErrorV4::ProgramSet)?,
        SelectorWidthV2::U8,
        &entries,
        &mut program_set,
    )
    .map_err(|_| FractionalSelectedReleaseErrorV4::ProgramSet)?;
    let selection_config = selection_config_bytes(input.terms)?;
    let release = FractionalCurrentReleaseV4 {
        program_set_id: digest(&program_set),
        selection_config_id: digest(&selection_config),
        selection_config,
        bundles,
        activation,
        program_set,
    };
    validate_fractional_current_release_v4(&release, input)?;
    Ok(release)
}

/// Hostile-rejoin the current activation-capable release.
pub fn validate_fractional_current_release_v4(
    release: &FractionalCurrentReleaseV4,
    input: FractionalSelectedReleaseInputV4<'_>,
) -> Result<(), FractionalSelectedReleaseErrorV4> {
    let widths = input.widths.checked()?;
    let expected_config = selection_config_bytes(input.terms)?;
    if release.selection_config != expected_config
        || release.selection_config_id != digest(&release.selection_config)
    {
        return Err(FractionalSelectedReleaseErrorV4::SelectionConfig);
    }
    join_fractional_selection_config_v1(
        FractionalSelectionConfigV1::decode(&release.selection_config)
            .map_err(|_| FractionalSelectedReleaseErrorV4::SelectionConfig)?,
        input.terms,
    )
    .map_err(|_| FractionalSelectedReleaseErrorV4::SelectionConfig)?;
    if release.bundles.len() != FRACTIONAL_SELECTED_ACTION_COUNT_V4 {
        return Err(FractionalSelectedReleaseErrorV4::Bundle);
    }
    let first = release
        .bundles
        .first()
        .ok_or(FractionalSelectedReleaseErrorV4::Activation)?;
    validate_fractional_activation_bundle_v1(
        &release.activation,
        FractionalActivationBundleInputV1 {
            action_descriptor: &first.descriptor,
        },
    )
    .map_err(|_| FractionalSelectedReleaseErrorV4::Activation)?;
    if release.program_set_id != digest(&release.program_set) {
        return Err(FractionalSelectedReleaseErrorV4::ProgramSet);
    }
    let set = CapabilityProgramSetV2::decode(&release.program_set)
        .map_err(|_| FractionalSelectedReleaseErrorV4::ProgramSet)?;
    let mut descriptors = [[0_u8; 32]; FRACTIONAL_SELECTED_ACTION_COUNT_V4];
    for ((slot, action), bundle) in descriptors
        .iter_mut()
        .zip(FRACTIONAL_SELECTED_ACTIONS_V4.iter().copied())
        .zip(&release.bundles)
    {
        let frame = fractional_claims_frame_spec_v4(action, input.terms, widths)?;
        let accounts =
            u16::try_from(frame.len()).map_err(|_| FractionalSelectedReleaseErrorV4::Frame)?;
        if bundle.action != action {
            return Err(FractionalSelectedReleaseErrorV4::Bundle);
        }
        validate_fractional_current_selected_bundle_v4(bundle, input.capacity_profile, accounts)
            .map_err(|_| FractionalSelectedReleaseErrorV4::Bundle)?;
        *slot = digest(&bundle.descriptor);
    }
    let expected = current_program_set_entries(&descriptors, release.activation.descriptor_id)?;
    if usize::from(set.entry_count()) != expected.len() {
        return Err(FractionalSelectedReleaseErrorV4::ProgramSet);
    }
    for (index, entry) in expected.iter().copied().enumerate() {
        if set
            .entry(u16::try_from(index).map_err(|_| FractionalSelectedReleaseErrorV4::ProgramSet)?)
            .map_err(|_| FractionalSelectedReleaseErrorV4::ProgramSet)?
            != entry
        {
            return Err(FractionalSelectedReleaseErrorV4::ProgramSet);
        }
    }
    let selected = set
        .select_descriptor(&fractional_activation_request_v1())
        .map_err(|_| FractionalSelectedReleaseErrorV4::ProgramSet)?;
    if selected.schema().to_bytes() != activation_descriptor_schema_v1()
        || selected.program().to_bytes() != release.activation.descriptor_id
    {
        return Err(FractionalSelectedReleaseErrorV4::ProgramSet);
    }
    Ok(())
}

/// Hostile-decode and rebind one complete selected Fractional release.
///
/// This recompiles every canonical frame from the same authenticated terms, so
/// a substituted frame, descriptor, selector, or publication identity refuses
/// even when the supplied bytes are individually well formed.
pub fn validate_fractional_selected_release_v4(
    release: &FractionalSelectedReleaseV4,
    input: FractionalSelectedReleaseInputV4<'_>,
) -> Result<(), FractionalSelectedReleaseErrorV4> {
    let widths = input.widths.checked()?;
    if release.bundles.len() != FRACTIONAL_SELECTED_ACTION_COUNT_V4
        || release.program_set_id != digest(&release.program_set)
    {
        return Err(FractionalSelectedReleaseErrorV4::ProgramSet);
    }
    let mut descriptors = [[0_u8; 32]; FRACTIONAL_SELECTED_ACTION_COUNT_V4];
    // Same fixed pairing as the builder. `index` stays because the bundles are
    // a Vec that this function still refuses by position rather than by type.
    for (index, (slot, action)) in descriptors
        .iter_mut()
        .zip(FRACTIONAL_SELECTED_ACTIONS_V4.iter().copied())
        .enumerate()
    {
        let bundle = release
            .bundles
            .get(index)
            .ok_or(FractionalSelectedReleaseErrorV4::Bundle)?;
        if bundle.action != action {
            return Err(FractionalSelectedReleaseErrorV4::Bundle);
        }
        let frame = fractional_claims_frame_spec_v4(action, input.terms, widths)?;
        let claims_accounts =
            u16::try_from(frame.len()).map_err(|_| FractionalSelectedReleaseErrorV4::Frame)?;
        validate_fractional_selected_bundle_v4(bundle, input.capacity_profile, claims_accounts)
            .map_err(|_| FractionalSelectedReleaseErrorV4::Bundle)?;
        let expected = build_fractional_selected_bundle_v4(FractionalSelectedBundleInputV4 {
            action,
            capacity_profile: input.capacity_profile,
            profile: widths.profile(),
            claims_frame: &frame,
        })
        .map_err(|_| FractionalSelectedReleaseErrorV4::Bundle)?;
        if expected != *bundle {
            return Err(FractionalSelectedReleaseErrorV4::Bundle);
        }
        *slot = digest(&bundle.descriptor);
    }
    let expected_entries = program_set_entries(&descriptors)?;
    let set = CapabilityProgramSetV2::decode(&release.program_set)
        .map_err(|_| FractionalSelectedReleaseErrorV4::ProgramSet)?;
    if usize::try_from(set.selector_offset()).ok()
        != Some(FRACTIONAL_EXPOSURE_REQUEST_ACTION_OFFSET_V2)
        || set.selector_width() != SelectorWidthV2::U8
        || usize::from(set.entry_count()) != FRACTIONAL_SELECTED_ACTION_COUNT_V4
    {
        return Err(FractionalSelectedReleaseErrorV4::ProgramSet);
    }
    for (index, entry) in expected_entries.iter().copied().enumerate() {
        let position =
            u16::try_from(index).map_err(|_| FractionalSelectedReleaseErrorV4::ProgramSet)?;
        if set
            .entry(position)
            .map_err(|_| FractionalSelectedReleaseErrorV4::ProgramSet)?
            != entry
        {
            return Err(FractionalSelectedReleaseErrorV4::ProgramSet);
        }
        let selected = set
            .select_descriptor(&action_selector_probe(entry.selector())?)
            .map_err(|_| FractionalSelectedReleaseErrorV4::ProgramSet)?;
        if selected != entry.descriptor() {
            return Err(FractionalSelectedReleaseErrorV4::ProgramSet);
        }
    }
    // Recompile the selection config from the same authenticated terms and
    // re-run the runtime join against it, so a substituted config refuses here
    // exactly as it would on chain. The join is the KERNEL's -- this compiler
    // does not restate which fields are market-free.
    let expected_selection_config = selection_config_bytes(input.terms)?;
    if release.selection_config != expected_selection_config
        || release.selection_config_id != digest(&release.selection_config)
    {
        return Err(FractionalSelectedReleaseErrorV4::SelectionConfig);
    }
    let decoded = FractionalSelectionConfigV1::decode(&release.selection_config)
        .map_err(|_| FractionalSelectedReleaseErrorV4::SelectionConfig)?;
    join_fractional_selection_config_v1(decoded, input.terms)
        .map_err(|_| FractionalSelectedReleaseErrorV4::SelectionConfig)?;
    let expected_publication = FractionalSelectedPublicationV4 {
        release_set: input.terms.release_set(),
        market: input.terms.market(),
        product_record: input.terms.product_record(),
        result_domain: input.terms.result_domain(),
        terms: input.terms.terms_id(),
        selection_config: release.selection_config_id,
        token_behavior: input.terms.token_behavior(),
        exposure: input.terms.exposure_id(),
        token_program: input.terms.token_program(),
        capacity_profile: input.capacity_profile,
        program_set_id: release.program_set_id,
        descriptors,
        denominator: input.terms.denominator(),
        product_width: input.terms.product_width(),
        representation_width: input.terms.representation_width(),
    };
    if release.publication != expected_publication {
        return Err(FractionalSelectedReleaseErrorV4::Publication);
    }
    Ok(())
}

/// Encode the market-free selection config the manifest entry will name.
///
/// The projection is the kernel's single author of "which fields are
/// market-free"; this function only encodes what that projection returns. No
/// Market coordinate is reachable from here, which is what makes the resulting
/// identity constructible before the Market address exists.
fn selection_config_bytes(
    terms: FractionalExposureTermsV2<'_>,
) -> Result<Vec<u8>, FractionalSelectedReleaseErrorV4> {
    let mut bytes = vec![0_u8; FRACTIONAL_SELECTION_CONFIG_BYTES_V1];
    encode_fractional_selection_config_v1(
        fractional_selection_config_from_terms_v1(terms),
        &mut bytes,
    )
    .map_err(|_| FractionalSelectedReleaseErrorV4::SelectionConfig)?;
    Ok(bytes)
}

/// One record a Registry must finalize for a published Fractional release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalPublicationRecordV1<'a> {
    /// Stable operator-facing label.
    pub label: &'static str,
    /// Schema the record is finalized under.
    pub schema: [u8; 32],
    /// Exact semantic bytes.
    pub body: &'a [u8],
}

impl FractionalPublicationRecordV1<'_> {
    /// Content identity of the exact bytes.
    #[must_use]
    pub fn content_id(&self) -> [u8; 32] {
        digest(self.body)
    }
}

impl FractionalSelectedReleaseV4 {
    /// Enumerate every record the Registry must hold for this release.
    ///
    /// Each record's schema is READ OFF the artifact that names it -- the
    /// descriptor's own `ArtifactReferenceV4` schemas, the descriptor's own
    /// `config_schema`, and the set entry's own descriptor schema. Nothing here
    /// restates a schema constant, so a publication plan cannot finalize a
    /// record under a schema this release does not actually select.
    ///
    /// Note what the config record IS after the split: the market-free
    /// selection config, published under the schema the descriptor selects.
    /// The exposure TERMS are deliberately absent -- they are the execution
    /// record, they bind the Market, and nothing a founding publishes may
    /// depend on the Market it is founding.
    pub fn publication_records(
        &self,
    ) -> Result<Vec<FractionalPublicationRecordV1<'_>>, FractionalSelectedReleaseErrorV4> {
        let set = CapabilityProgramSetV2::decode(&self.program_set)
            .map_err(|_| FractionalSelectedReleaseErrorV4::ProgramSet)?;
        let first = CapabilityProgramV4::decode(
            &self
                .bundles
                .first()
                .ok_or(FractionalSelectedReleaseErrorV4::Bundle)?
                .descriptor,
        )
        .map_err(|_| FractionalSelectedReleaseErrorV4::Bundle)?;

        let mut records = Vec::with_capacity(2 + FRACTIONAL_SELECTED_ACTION_COUNT_V4 * 7);
        records.push(FractionalPublicationRecordV1 {
            label: "program-set",
            schema: CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
            body: &self.program_set,
        });
        records.push(FractionalPublicationRecordV1 {
            label: "selection-config",
            schema: first.config_schema().to_bytes(),
            body: &self.selection_config,
        });

        for (index, bundle) in self.bundles.iter().enumerate() {
            let entry = set
                .entry(
                    u16::try_from(index)
                        .map_err(|_| FractionalSelectedReleaseErrorV4::ProgramSet)?,
                )
                .map_err(|_| FractionalSelectedReleaseErrorV4::ProgramSet)?;
            let descriptor = CapabilityProgramV4::decode(&bundle.descriptor)
                .map_err(|_| FractionalSelectedReleaseErrorV4::Bundle)?;
            let artifacts = descriptor.artifacts();
            for (label, schema, body) in [
                (
                    "descriptor",
                    entry.descriptor().schema().to_bytes(),
                    bundle.descriptor.as_slice(),
                ),
                (
                    "account-profile",
                    artifacts.account_profile.schema().to_bytes(),
                    bundle.account_profile.as_slice(),
                ),
                (
                    "lifecycle-policy",
                    artifacts.lifecycle.schema().to_bytes(),
                    bundle.lifecycle_policy.as_slice(),
                ),
                (
                    "request-profile",
                    artifacts.request_profile.schema().to_bytes(),
                    bundle.request_profile.as_slice(),
                ),
                (
                    "strategy",
                    artifacts.strategy.schema().to_bytes(),
                    bundle.strategy.as_slice(),
                ),
                (
                    "transition",
                    artifacts.transition.schema().to_bytes(),
                    bundle.transition.as_slice(),
                ),
                (
                    "effect",
                    artifacts.effect.schema().to_bytes(),
                    bundle.effect.as_slice(),
                ),
            ] {
                records.push(FractionalPublicationRecordV1 {
                    label,
                    schema,
                    body,
                });
            }
        }
        Ok(records)
    }
}

impl FractionalCurrentReleaseV4 {
    /// Enumerate every current record the Registry must finalize.
    ///
    /// The historical action record closure stays untouched. The three
    /// activation records are appended because the activation transition is
    /// embedded in its descriptor and therefore has no fourth Record owner.
    pub fn publication_records(
        &self,
    ) -> Result<Vec<FractionalPublicationRecordV1<'_>>, FractionalSelectedReleaseErrorV4> {
        let set = CapabilityProgramSetV2::decode(&self.program_set)
            .map_err(|_| FractionalSelectedReleaseErrorV4::ProgramSet)?;
        let first = CapabilityProgramV4::decode(
            &self
                .bundles
                .first()
                .ok_or(FractionalSelectedReleaseErrorV4::Bundle)?
                .descriptor,
        )
        .map_err(|_| FractionalSelectedReleaseErrorV4::Bundle)?;
        let mut records = Vec::with_capacity(5 + FRACTIONAL_SELECTED_ACTION_COUNT_V4 * 7);
        records.push(FractionalPublicationRecordV1 {
            label: "program-set",
            schema: CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
            body: &self.program_set,
        });
        records.push(FractionalPublicationRecordV1 {
            label: "selection-config",
            schema: first.config_schema().to_bytes(),
            body: &self.selection_config,
        });
        for (index, bundle) in self.bundles.iter().enumerate() {
            let entry = set
                .entry(
                    u16::try_from(index)
                        .map_err(|_| FractionalSelectedReleaseErrorV4::ProgramSet)?,
                )
                .map_err(|_| FractionalSelectedReleaseErrorV4::ProgramSet)?;
            let descriptor = CapabilityProgramV4::decode(&bundle.descriptor)
                .map_err(|_| FractionalSelectedReleaseErrorV4::Bundle)?;
            let artifacts = descriptor.artifacts();
            for (label, schema, body) in [
                (
                    "descriptor",
                    entry.descriptor().schema().to_bytes(),
                    bundle.descriptor.as_slice(),
                ),
                (
                    "account-profile",
                    artifacts.account_profile.schema().to_bytes(),
                    bundle.account_profile.as_slice(),
                ),
                (
                    "lifecycle-policy",
                    artifacts.lifecycle.schema().to_bytes(),
                    bundle.lifecycle_policy.as_slice(),
                ),
                (
                    "request-profile",
                    artifacts.request_profile.schema().to_bytes(),
                    bundle.request_profile.as_slice(),
                ),
                (
                    "strategy",
                    artifacts.strategy.schema().to_bytes(),
                    bundle.strategy.as_slice(),
                ),
                (
                    "transition",
                    artifacts.transition.schema().to_bytes(),
                    bundle.transition.as_slice(),
                ),
                (
                    "effect",
                    artifacts.effect.schema().to_bytes(),
                    bundle.effect.as_slice(),
                ),
            ] {
                records.push(FractionalPublicationRecordV1 {
                    label,
                    schema,
                    body,
                });
            }
        }
        let activation_entry = set
            .entry(
                u16::try_from(FRACTIONAL_SELECTED_ACTION_COUNT_V4)
                    .map_err(|_| FractionalSelectedReleaseErrorV4::ProgramSet)?,
            )
            .map_err(|_| FractionalSelectedReleaseErrorV4::ProgramSet)?;
        for (label, schema, body) in [
            (
                "activation-account-profile",
                activation_account_profile_schema_v1(),
                self.activation.account_profile.as_slice(),
            ),
            (
                "activation-effect",
                activation_effect_schema_v1(),
                self.activation.effect.as_slice(),
            ),
            (
                "activation-descriptor",
                activation_entry.descriptor().schema().to_bytes(),
                self.activation.descriptor.as_slice(),
            ),
        ] {
            records.push(FractionalPublicationRecordV1 {
                label,
                schema,
                body,
            });
        }
        Ok(records)
    }
}

fn program_set_entries(
    descriptors: &[[u8; 32]; FRACTIONAL_SELECTED_ACTION_COUNT_V4],
) -> Result<
    [CapabilityProgramSetEntryV2; FRACTIONAL_SELECTED_ACTION_COUNT_V4],
    FractionalSelectedReleaseErrorV4,
> {
    let schema = content(CAPABILITY_PROGRAM_SCHEMA_ID_V4)?;
    let mut entries =
        [CapabilityProgramSetEntryV2::new(0, CapabilityDescriptorReferenceV2::new(schema, schema));
            FRACTIONAL_SELECTED_ACTION_COUNT_V4];
    let mut prior: Option<u32> = None;
    // All three of `entries`, FRACTIONAL_SELECTED_ACTIONS_V4 and `descriptors`
    // are [_; FRACTIONAL_SELECTED_ACTION_COUNT_V4], so the one index served
    // only to re-derive a correspondence the types already fix.
    for ((slot, action), descriptor) in entries
        .iter_mut()
        .zip(FRACTIONAL_SELECTED_ACTIONS_V4.iter().copied())
        .zip(descriptors.iter().copied())
    {
        let selector = u32::from(action.byte());
        if prior.is_some_and(|value| value >= selector) {
            return Err(FractionalSelectedReleaseErrorV4::ProgramSet);
        }
        prior = Some(selector);
        *slot = CapabilityProgramSetEntryV2::new(
            selector,
            CapabilityDescriptorReferenceV2::new(schema, content(descriptor)?),
        );
    }
    Ok(entries)
}

fn current_program_set_entries(
    descriptors: &[[u8; 32]; FRACTIONAL_SELECTED_ACTION_COUNT_V4],
    activation_descriptor: [u8; 32],
) -> Result<
    [CapabilityProgramSetEntryV2; FRACTIONAL_SELECTED_ACTION_COUNT_V4 + 1],
    FractionalSelectedReleaseErrorV4,
> {
    let actions = program_set_entries(descriptors)?;
    let mut entries = [actions[0]; FRACTIONAL_SELECTED_ACTION_COUNT_V4 + 1];
    entries[..FRACTIONAL_SELECTED_ACTION_COUNT_V4].copy_from_slice(&actions);
    entries[FRACTIONAL_SELECTED_ACTION_COUNT_V4] = CapabilityProgramSetEntryV2::new(
        FRACTIONAL_ACTIVATION_SELECTOR_V1,
        CapabilityDescriptorReferenceV2::new(
            content(activation_descriptor_schema_v1())?,
            content(activation_descriptor)?,
        ),
    );
    Ok(entries)
}

fn action_selector_probe(
    selector: u32,
) -> Result<[u8; FRACTIONAL_EXPOSURE_REQUEST_BYTES_V2], FractionalSelectedReleaseErrorV4> {
    let mut request = [0_u8; FRACTIONAL_EXPOSURE_REQUEST_BYTES_V2];
    let byte = u8::try_from(selector).map_err(|_| FractionalSelectedReleaseErrorV4::ProgramSet)?;
    *request
        .get_mut(FRACTIONAL_EXPOSURE_REQUEST_ACTION_OFFSET_V2)
        .ok_or(FractionalSelectedReleaseErrorV4::ProgramSet)? = byte;
    Ok(request)
}

fn atomic_frame(
    terms: FractionalExposureTermsV2<'_>,
    widths: FractionalFrameWidthsV4,
) -> Result<Vec<FractionalClaimsAccountRuleV1>, FractionalSelectedReleaseErrorV4> {
    let mut rules = signed_delta_prefix(
        2,
        FRACTIONAL_ATOMIC_SIGNED_DELTA_ACCOUNT_COUNT_V3,
        terms,
        widths,
    )?;
    for (index, signer, writable, executable, data) in [
        (
            FRACTIONAL_ATOMIC_TERMS_RAW_V3,
            false,
            false,
            false,
            Data::Opaque,
        ),
        (
            FRACTIONAL_ATOMIC_TERMS_STAGING_V3,
            false,
            false,
            false,
            Data::Exact(0),
        ),
        (
            FRACTIONAL_ATOMIC_TOKEN_BEHAVIOR_RAW_V3,
            false,
            false,
            false,
            Data::Opaque,
        ),
        (
            FRACTIONAL_ATOMIC_TOKEN_BEHAVIOR_STAGING_V3,
            false,
            false,
            false,
            Data::Exact(0),
        ),
        (FRACTIONAL_ATOMIC_ROOT_V3, true, true, false, Data::Root),
        (FRACTIONAL_ATOMIC_ACTOR_V3, true, false, false, Data::Opaque),
        (
            FRACTIONAL_ATOMIC_SHARD_MINT_V3,
            false,
            true,
            false,
            Data::Opaque,
        ),
        (
            FRACTIONAL_ATOMIC_HOLDER_TOKEN_V3,
            false,
            true,
            false,
            Data::Opaque,
        ),
        (
            FRACTIONAL_ATOMIC_TOKEN_PROGRAM_V3,
            false,
            false,
            true,
            Data::Opaque,
        ),
    ] {
        push(&mut rules, index, signer, writable, executable, data)?;
    }
    if rules.len() != FRACTIONAL_ATOMIC_ACCOUNT_COUNT_V3 {
        return Err(FractionalSelectedReleaseErrorV4::Frame);
    }
    Ok(rules)
}

fn terminal_frame(
    terms: FractionalExposureTermsV2<'_>,
    widths: FractionalFrameWidthsV4,
) -> Result<Vec<FractionalClaimsAccountRuleV1>, FractionalSelectedReleaseErrorV4> {
    let mut rules = signed_delta_prefix(
        1,
        TERMINAL_SETTLEMENT_EXPOSURE_RAW_ACCOUNT_V3,
        terms,
        widths,
    )?;
    for (index, signer, writable, executable, data) in [
        (
            TERMINAL_SETTLEMENT_EXPOSURE_RAW_ACCOUNT_V3,
            false,
            false,
            false,
            Data::Opaque,
        ),
        (
            TERMINAL_SETTLEMENT_EXPOSURE_STAGING_ACCOUNT_V3,
            false,
            false,
            false,
            Data::Exact(0),
        ),
        (
            TERMINAL_SETTLEMENT_CUSTODY_CALLER_ACCOUNT_V3,
            false,
            false,
            false,
            Data::Opaque,
        ),
        (
            TERMINAL_SETTLEMENT_CUSTODY_PROGRAM_ACCOUNT_V3,
            false,
            false,
            true,
            Data::Opaque,
        ),
        (
            TERMINAL_SETTLEMENT_CERTIFICATE_ACCOUNT_V3,
            false,
            false,
            false,
            Data::Opaque,
        ),
        (
            TERMINAL_SETTLEMENT_RESOLUTION_PROGRAM_ACCOUNT_V3,
            false,
            false,
            true,
            Data::Opaque,
        ),
        (
            TERMINAL_SETTLEMENT_RESOLUTION_PROGRAMDATA_ACCOUNT_V3,
            false,
            false,
            false,
            Data::Opaque,
        ),
        (
            TERMINAL_SETTLEMENT_REALM_ACCOUNT_V3,
            false,
            false,
            false,
            Data::Opaque,
        ),
        (
            TERMINAL_SETTLEMENT_REALM_STAGING_ACCOUNT_V3,
            false,
            false,
            false,
            Data::Exact(0),
        ),
        (
            TERMINAL_SETTLEMENT_CUSTODY_REPLAY_ACCOUNT_V3,
            false,
            true,
            false,
            Data::Opaque,
        ),
        (
            TERMINAL_SETTLEMENT_COLLATERAL_MINT_ACCOUNT_V3,
            false,
            false,
            false,
            Data::Opaque,
        ),
        (
            TERMINAL_SETTLEMENT_HOARD_ACCOUNT_V3,
            false,
            true,
            false,
            Data::Opaque,
        ),
        (
            TERMINAL_SETTLEMENT_RECIPIENT_ACCOUNT_V3,
            false,
            true,
            false,
            Data::Opaque,
        ),
        (
            TERMINAL_SETTLEMENT_CUSTODY_AUTHORITY_ACCOUNT_V3,
            false,
            false,
            false,
            Data::Opaque,
        ),
        (
            TERMINAL_SETTLEMENT_TOKEN_PROGRAM_ACCOUNT_V3,
            false,
            false,
            true,
            Data::Opaque,
        ),
        (
            FRACTIONAL_TERMINAL_TERMS_RAW_V3,
            false,
            false,
            false,
            Data::Opaque,
        ),
        (
            FRACTIONAL_TERMINAL_TERMS_STAGING_V3,
            false,
            false,
            false,
            Data::Exact(0),
        ),
        (
            FRACTIONAL_TERMINAL_TOKEN_BEHAVIOR_RAW_V3,
            false,
            false,
            false,
            Data::Opaque,
        ),
        (
            FRACTIONAL_TERMINAL_TOKEN_BEHAVIOR_STAGING_V3,
            false,
            false,
            false,
            Data::Exact(0),
        ),
        (FRACTIONAL_TERMINAL_ROOT_V3, true, true, false, Data::Root),
        (
            FRACTIONAL_TERMINAL_ACTOR_V3,
            true,
            false,
            false,
            Data::Opaque,
        ),
        (
            FRACTIONAL_TERMINAL_SHARD_MINT_V3,
            false,
            true,
            false,
            Data::Opaque,
        ),
        (
            FRACTIONAL_TERMINAL_SOURCE_TOKEN_V3,
            false,
            true,
            false,
            Data::Opaque,
        ),
    ] {
        push(&mut rules, index, signer, writable, executable, data)?;
    }
    if rules.len() != FRACTIONAL_TERMINAL_ACCOUNT_COUNT_V3
        || TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3 != FRACTIONAL_TERMINAL_TERMS_RAW_V3
    {
        return Err(FractionalSelectedReleaseErrorV4::Frame);
    }
    Ok(rules)
}

#[derive(Clone, Copy)]
enum Data {
    Exact(u32),
    Opaque,
    Root,
}

fn signed_delta_prefix(
    position_count: u32,
    expected: usize,
    terms: FractionalExposureTermsV2<'_>,
    widths: FractionalFrameWidthsV4,
) -> Result<Vec<FractionalClaimsAccountRuleV1>, FractionalSelectedReleaseErrorV4> {
    let spec = SignedDeltaFrameSpecV3::new(position_count)
        .map_err(|_| FractionalSelectedReleaseErrorV4::Frame)?;
    let count = spec
        .account_count()
        .map_err(|_| FractionalSelectedReleaseErrorV4::Frame)?;
    if usize::from(count) != expected {
        return Err(FractionalSelectedReleaseErrorV4::Frame);
    }
    let mut rules = Vec::with_capacity(expected);
    for index in 0..count {
        let account = spec
            .account(index)
            .map_err(|_| FractionalSelectedReleaseErrorV4::Frame)?;
        let privileges = account.privileges();
        let data = resolve_width(
            spec.data(index)
                .map_err(|_| FractionalSelectedReleaseErrorV4::Frame)?,
            terms,
            widths,
        )?;
        push(
            &mut rules,
            usize::from(index),
            privileges.signer(),
            privileges.writable(),
            privileges.executable(),
            data,
        )?;
    }
    Ok(rules)
}

/// Resolve one Claims data obligation through its named semantic owner.
///
/// The variants that name an owner outside Claims are resolved from the
/// authenticated terms or the release widths. The remainder are deployment
/// facts no release can pin -- sysvars, Loader-v3 Program and ProgramData
/// accounts, and wallet identities -- and become opaque.
fn resolve_width(
    data: ClaimsFrameDataV1,
    terms: FractionalExposureTermsV2<'_>,
    widths: FractionalFrameWidthsV4,
) -> Result<Data, FractionalSelectedReleaseErrorV4> {
    Ok(match data {
        ClaimsFrameDataV1::Exact(bytes) => Data::Exact(bytes),
        ClaimsFrameDataV1::OpaqueData => Data::Opaque,
        ClaimsFrameDataV1::ProductTail { base, item_stride } => Data::Exact(
            item_stride
                .checked_mul(terms.product_width())
                .and_then(|tail| base.checked_add(tail))
                .ok_or(FractionalSelectedReleaseErrorV4::Frame)?,
        ),
        ClaimsFrameDataV1::LinkedBasisRecord => Data::Exact(widths.linked_basis_record),
        ClaimsFrameDataV1::ProductRecord => Data::Exact(widths.product_record),
        ClaimsFrameDataV1::ResultDomainRecord => Data::Exact(widths.result_domain_record),
        ClaimsFrameDataV1::PortfolioRecord => Data::Exact(widths.portfolio_record),
        ClaimsFrameDataV1::CoreMarket => Data::Exact(widths.core_market),
        ClaimsFrameDataV1::ActivationCache => Data::Exact(widths.activation_cache),
        ClaimsFrameDataV1::RentCredit => Data::Exact(widths.rent_credit),
        ClaimsFrameDataV1::RentSysvar
        | ClaimsFrameDataV1::UpgradeableProgram
        | ClaimsFrameDataV1::ProgramData(_)
        | ClaimsFrameDataV1::PositionOwnerIdentity => Data::Opaque,
    })
}

fn push(
    rules: &mut Vec<FractionalClaimsAccountRuleV1>,
    index: usize,
    signer: bool,
    writable: bool,
    executable: bool,
    data: Data,
) -> Result<(), FractionalSelectedReleaseErrorV4> {
    if rules.len() != index {
        return Err(FractionalSelectedReleaseErrorV4::Frame);
    }
    let (data_length, opaque_data) = match data {
        Data::Exact(bytes) => (bytes, false),
        Data::Opaque => (0, true),
        Data::Root => (
            u32::try_from(FRACTIONAL_CAPABILITY_ROOT_BYTES_V4)
                .map_err(|_| FractionalSelectedReleaseErrorV4::Frame)?,
            false,
        ),
    };
    rules.push(FractionalClaimsAccountRuleV1 {
        signer,
        writable,
        executable,
        data_length,
        opaque_data,
    });
    Ok(())
}

fn content(
    bytes: [u8; 32],
) -> Result<dclutch_core_contract::ContentId, FractionalSelectedReleaseErrorV4> {
    dclutch_core_contract::ContentId::new(bytes)
        .map_err(|_| FractionalSelectedReleaseErrorV4::ProgramSet)
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
