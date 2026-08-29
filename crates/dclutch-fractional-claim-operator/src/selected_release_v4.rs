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

use dclutch_capability_program_contract::{
    set_v2::{
        CapabilityDescriptorReferenceV2, CapabilityProgramSetEntryV2, CapabilityProgramSetV2,
        SelectorWidthV2, encode_program_set_v2, encoded_program_set_bytes_v2,
    },
    v4::SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_ID_V4,
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
use dclutch_fractional_claim_contract::{
    FRACTIONAL_ATOMIC_ACCOUNT_COUNT_V3, FRACTIONAL_ATOMIC_ACTOR_V3,
    FRACTIONAL_ATOMIC_HOLDER_TOKEN_V3, FRACTIONAL_ATOMIC_ROOT_V3, FRACTIONAL_ATOMIC_SHARD_MINT_V3,
    FRACTIONAL_ATOMIC_SIGNED_DELTA_ACCOUNT_COUNT_V3, FRACTIONAL_ATOMIC_TERMS_RAW_V3,
    FRACTIONAL_ATOMIC_TERMS_STAGING_V3, FRACTIONAL_ATOMIC_TOKEN_BEHAVIOR_RAW_V3,
    FRACTIONAL_ATOMIC_TOKEN_BEHAVIOR_STAGING_V3, FRACTIONAL_ATOMIC_TOKEN_PROGRAM_V3,
    FRACTIONAL_CAPABILITY_ROOT_BYTES_V4, FRACTIONAL_EXPOSURE_REQUEST_ACTION_OFFSET_V2,
    FRACTIONAL_EXPOSURE_REQUEST_BYTES_V2, FRACTIONAL_TERMINAL_ACCOUNT_COUNT_V3,
    FRACTIONAL_TERMINAL_ACTOR_V3, FRACTIONAL_TERMINAL_ROOT_V3, FRACTIONAL_TERMINAL_SHARD_MINT_V3,
    FRACTIONAL_TERMINAL_SOURCE_TOKEN_V3, FRACTIONAL_TERMINAL_TERMS_RAW_V3,
    FRACTIONAL_TERMINAL_TERMS_STAGING_V3, FRACTIONAL_TERMINAL_TOKEN_BEHAVIOR_RAW_V3,
    FRACTIONAL_TERMINAL_TOKEN_BEHAVIOR_STAGING_V3, FractionalExposureActionV2,
};
use dclutch_fractional_claim_kernel::FractionalExposureTermsV2;
use sha2::{Digest, Sha256};

use crate::{
    FractionalClaimsAccountRuleV1, FractionalSelectedBundleInputV4, FractionalSelectedBundleV4,
    FractionalSelectedProfileInputV4, build_fractional_selected_bundle_v4,
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
pub const FRACTIONAL_SELECTED_PUBLICATION_BYTES_V4: usize = 480;

const PUBLICATION_VERSION: u16 = 4;
const IDENTITY_START: usize = 16;
const IDENTITY_COUNT: usize = 14;
const SCALAR_START: usize = IDENTITY_START + IDENTITY_COUNT * 32;

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
    /// Independently authenticated Fractional terms Record checked prefix.
    pub selected_config: u32,
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
            self.selected_config,
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
            selected_config_bytes: self.selected_config,
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
    pub terms: [u8; 32],
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
        for (index, identity) in self.identities().iter().enumerate() {
            let start = IDENTITY_START + index * 32;
            output[start..start + 32].copy_from_slice(identity);
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
    /// Canonical Market-bindable publication.
    pub publication: FractionalSelectedPublicationV4,
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
    /// Publication identities or scalars were not exact.
    Publication,
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
    for (index, action) in FRACTIONAL_SELECTED_ACTIONS_V4.iter().copied().enumerate() {
        let frame = fractional_claims_frame_spec_v4(action, input.terms, widths)?;
        let bundle = build_fractional_selected_bundle_v4(FractionalSelectedBundleInputV4 {
            action,
            capacity_profile: input.capacity_profile,
            profile,
            claims_frame: &frame,
        })
        .map_err(|_| FractionalSelectedReleaseErrorV4::Bundle)?;
        descriptors[index] = digest(&bundle.descriptor);
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
    let publication = FractionalSelectedPublicationV4 {
        release_set: input.terms.release_set(),
        market: input.terms.market(),
        product_record: input.terms.product_record(),
        result_domain: input.terms.result_domain(),
        terms: input.terms.terms_id(),
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
        publication,
    };
    validate_fractional_selected_release_v4(&release, input)?;
    Ok(release)
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
    for (index, action) in FRACTIONAL_SELECTED_ACTIONS_V4.iter().copied().enumerate() {
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
        descriptors[index] = digest(&bundle.descriptor);
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
    let expected_publication = FractionalSelectedPublicationV4 {
        release_set: input.terms.release_set(),
        market: input.terms.market(),
        product_record: input.terms.product_record(),
        result_domain: input.terms.result_domain(),
        terms: input.terms.terms_id(),
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
    for (index, action) in FRACTIONAL_SELECTED_ACTIONS_V4.iter().copied().enumerate() {
        let selector = u32::from(action.byte());
        if prior.is_some_and(|value| value >= selector) {
            return Err(FractionalSelectedReleaseErrorV4::ProgramSet);
        }
        prior = Some(selector);
        entries[index] = CapabilityProgramSetEntryV2::new(
            selector,
            CapabilityDescriptorReferenceV2::new(schema, content(descriptors[index])?),
        );
    }
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
