//! Pure General activation behind the common Trading admission boundary.
//!
//! The common Trading layer owns Registry/Core authentication, account
//! framing, root allocation/assignment, immutable-header creation, and final
//! writes. This module never invokes CPI and never mutates account memory. It
//! authenticates the General content profile, executes its admitted
//! TransitionVM program over an exact register projection, runs the shared
//! FundingState semantics, and returns one complete commit plan.

use dclutch_capability_contract::{
    CapabilityFundingDerivationV1, CapabilityManifestV1, ContentId, FundingCustodyObservationV1,
    FundingStateV1,
};
use dclutch_capability_program_contract::{
    CAPABILITY_ACTIVATION_EFFECT_SCHEMA_ID_V1, CAPABILITY_ROOT_DERIVATION_RELEASE_ID_V1,
    CapabilityRootHeaderV1, SupportedContentV1,
};
use dclutch_general_config_contract::{
    GENERAL_ACTIVATION_REQUEST_SCHEMA_ID_V2, GENERAL_CONFIG_SCHEMA_ID_V2, GENERAL_ROOT_BYTES_V2,
    GENERAL_ROOT_SCHEMA_ID_V2, GeneralActivationDispositionV2, GeneralActivationRequestV2,
    GeneralConfigV2, GeneralRootV2, activate_general_owned_v2,
};
use dclutch_market_core_codec::{CoreEffectActionV1, CoreEffectEnvelopeV1, Identity, Role};
use dclutch_transition_vm::Registers;
use solana_program::{
    hash::{hash, hashv},
    pubkey::Pubkey,
};

use crate::{
    TradingSbfError,
    dispatch::{
        TradingActivationRequestV1, TradingFamilyContextV1, dispatch_activation_authenticated,
    },
};

/// Physical-profile label for the exact General activation projection.
pub const GENERAL_ACTIVATION_ACCOUNT_PROFILE_PREIMAGE_V2: &[u8] =
    b"dclutch/account-profile/general-activation-v2";
/// SHA-256 of [`GENERAL_ACTIVATION_ACCOUNT_PROFILE_PREIMAGE_V2`].
pub const GENERAL_ACTIVATION_ACCOUNT_PROFILE_ID_V2: [u8; 32] = [
    0x59, 0x1f, 0xcb, 0xdc, 0x33, 0xc0, 0xce, 0x8e, 0xa3, 0xb3, 0xd7, 0xbe, 0x78, 0xfb, 0x87, 0xc5,
    0x74, 0x23, 0x15, 0xcf, 0x7b, 0x79, 0x98, 0x71, 0x19, 0x11, 0x42, 0x6c, 0x5f, 0x10, 0xbc, 0x9b,
];

const ACTIVATION_POSTSTATE_DOMAIN_V2: &[u8] = b"dclutch/general/activation-poststate/v2";
const GENERAL_FUNDING_COUNT_V2: u8 = 1;

/// Stable refusal from pure General activation preparation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralActivationError {
    /// Envelope, selector, funding header, or family request bytes refused.
    Instruction,
    /// Preauthenticated Market/release/root/request coordinates differed.
    Coordinates,
    /// Config, manifest, descriptor, or General schema content refused.
    Content,
    /// FundingState identity, custody, transition, or exact balance refused.
    Funding,
    /// A checked balance or length computation overflowed or underflowed.
    Arithmetic,
    /// The common descriptor/TransitionVM boundary refused.
    Common(TradingSbfError),
}

impl From<TradingSbfError> for GeneralActivationError {
    fn from(value: TradingSbfError) -> Self {
        Self::Common(value)
    }
}

/// Exact prestate observations authenticated by the future common creator.
///
/// The caller must already have established account owner, writable privilege,
/// and non-aliasing rules. Keys are semantic resources, not suffix indices.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralActivationResourcesV2 {
    funding_state_key: [u8; 32],
    funding_state: FundingStateV1,
    rent_credit_key: [u8; 32],
    composite_root_lamports: u64,
    funding_state_lamports: u64,
    rent_credit_lamports: u64,
    existing_root_state: Option<GeneralRootV2>,
}

impl GeneralActivationResourcesV2 {
    /// Construct one exact preauthenticated resource observation.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        funding_state_key: [u8; 32],
        funding_state: FundingStateV1,
        rent_credit_key: [u8; 32],
        composite_root_lamports: u64,
        funding_state_lamports: u64,
        rent_credit_lamports: u64,
        existing_root_state: Option<GeneralRootV2>,
    ) -> Result<Self, GeneralActivationError> {
        if is_zero(&funding_state_key) || is_zero(&rent_credit_key) {
            return Err(GeneralActivationError::Coordinates);
        }
        Ok(Self {
            funding_state_key,
            funding_state,
            rent_credit_key,
            composite_root_lamports,
            funding_state_lamports,
            rent_credit_lamports,
            existing_root_state,
        })
    }

    /// Exact Trading-owned FundingState account identity.
    #[must_use]
    pub const fn funding_state_key(self) -> [u8; 32] {
        self.funding_state_key
    }

    /// Exact typed FundingState prestate.
    #[must_use]
    pub const fn funding_state(self) -> FundingStateV1 {
        self.funding_state
    }

    /// Core-authenticated RentCredit destination.
    #[must_use]
    pub const fn rent_credit_key(self) -> [u8; 32] {
        self.rent_credit_key
    }

    /// Lamports observed on the composite root before activation.
    #[must_use]
    pub const fn composite_root_lamports(self) -> u64 {
        self.composite_root_lamports
    }

    /// Lamports observed on the FundingState before activation.
    #[must_use]
    pub const fn funding_state_lamports(self) -> u64 {
        self.funding_state_lamports
    }

    /// Lamports observed on RentCredit before activation.
    #[must_use]
    pub const fn rent_credit_lamports(self) -> u64 {
        self.rent_credit_lamports
    }

    /// Existing mutable tail on exact replay, or `None` for creation.
    #[must_use]
    pub const fn existing_root_state(self) -> Option<GeneralRootV2> {
        self.existing_root_state
    }
}

/// Exact family plan returned to the common atomic creator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedGeneralActivationV2 {
    disposition: GeneralActivationDispositionV2,
    root_state: GeneralRootV2,
    funding_after: FundingStateV1,
    composite_root_lamports_after: u64,
    funding_state_lamports_after: u64,
    rent_credit_lamports_after: u64,
    poststate_digest: Identity,
}

impl PreparedGeneralActivationV2 {
    /// Whether the common layer creates the composite account or proves replay.
    #[must_use]
    pub const fn disposition(self) -> GeneralActivationDispositionV2 {
        self.disposition
    }

    /// Exact initialized or replayed 128-byte General mutable tail.
    #[must_use]
    pub const fn root_state(self) -> GeneralRootV2 {
        self.root_state
    }

    /// Exact family-owned FundingState poststate.
    #[must_use]
    pub const fn funding_after(self) -> FundingStateV1 {
        self.funding_after
    }

    /// Exact final lamports on the one composite root.
    #[must_use]
    pub const fn composite_root_lamports_after(self) -> u64 {
        self.composite_root_lamports_after
    }

    /// Exact final lamports on the General-owned FundingState.
    #[must_use]
    pub const fn funding_state_lamports_after(self) -> u64 {
        self.funding_state_lamports_after
    }

    /// Exact final lamports on Core-authenticated RentCredit.
    #[must_use]
    pub const fn rent_credit_lamports_after(self) -> u64 {
        self.rent_credit_lamports_after
    }

    /// Digest of the complete expected header/tail/funding/rent poststate.
    ///
    /// The common layer compares this digest while applying the plan and alone
    /// constructs and emits the Core acknowledgement after commit succeeds.
    #[must_use]
    pub const fn poststate_digest(self) -> Identity {
        self.poststate_digest
    }
}

/// Prepare General activation from already authenticated common Trading facts.
///
/// This function is total over its bounded inputs and performs no writes or
/// CPI. The future common Trading creator must verify exact Rent for the
/// descriptor-sized composite root and FundingState before calling it, apply
/// every returned poststate atomically, and only then emit the acknowledgement.
#[allow(clippy::too_many_arguments)]
pub fn prepare_activation_authenticated(
    context: TradingFamilyContextV1,
    envelope: CoreEffectEnvelopeV1,
    role_request_bytes: &[u8],
    manifest_bytes: &[u8],
    descriptor_bytes: &[u8],
    config_bytes: &[u8],
    resources: GeneralActivationResourcesV2,
) -> Result<PreparedGeneralActivationV2, GeneralActivationError> {
    let activation = TradingActivationRequestV1::decode(role_request_bytes)?;
    let request = GeneralActivationRequestV2::decode(activation.family_request())
        .map_err(|_| GeneralActivationError::Instruction)?;
    let role_request_digest = identity(hash(role_request_bytes).to_bytes())?;
    envelope
        .validate_role_request(role_request_bytes.len(), role_request_digest)
        .map_err(|_| GeneralActivationError::Instruction)?;
    authenticate_coordinates(context, envelope, activation, request, resources)?;

    let config =
        GeneralConfigV2::decode(config_bytes).map_err(|_| GeneralActivationError::Content)?;
    if config.generation() != context.generation()
        || config.capability_program_id() != context.selection().capability_release().to_bytes()
    {
        return Err(GeneralActivationError::Content);
    }

    let mut registers = activation_registers(context, request, config, resources)?;
    let descriptor = dispatch_activation_authenticated(
        context,
        manifest_bytes,
        descriptor_bytes,
        config_bytes,
        supported_content()?,
        &mut registers,
    )?;
    if descriptor.root_state_bytes()
        != u32::try_from(GENERAL_ROOT_BYTES_V2).map_err(|_| GeneralActivationError::Arithmetic)?
    {
        return Err(GeneralActivationError::Content);
    }

    let manifest_id = content(request.manifest_id())?;
    let config_id = content(request.config_id())?;
    let manifest = CapabilityManifestV1::decode(manifest_bytes)
        .map_err(|_| GeneralActivationError::Content)?;
    let derivation = CapabilityFundingDerivationV1::new(
        context.market(),
        context.generation(),
        manifest_id,
        manifest,
        resources.funding_state(),
    )
    .map_err(|_| GeneralActivationError::Funding)?;
    let program_id = Pubkey::new_from_array(context.program_id());
    let expected_funding =
        Pubkey::find_program_address(&derivation.seed_components(), &program_id).0;
    if expected_funding.to_bytes() != resources.funding_state_key() {
        return Err(GeneralActivationError::Funding);
    }
    let custody = FundingCustodyObservationV1::native_only(
        resources.funding_state_lamports(),
        request.exact_funding_rent_lamports(),
    )
    .map_err(|_| GeneralActivationError::Funding)?;
    let owned = activate_general_owned_v2(
        context.market(),
        context.generation(),
        manifest_id,
        manifest,
        request.entry_index(),
        config_id,
        config,
        resources.funding_state(),
        custody,
        request.current_slot(),
        request.exact_root_rent_lamports(),
        resources.composite_root_lamports(),
        resources.existing_root_state(),
    )
    .map_err(|_| GeneralActivationError::Funding)?;

    let creation = owned.creation();
    let funding_debit = creation
        .funding_top_up_lamports()
        .checked_add(creation.displaced_prepaid_lamports())
        .ok_or(GeneralActivationError::Arithmetic)?;
    let funding_state_lamports_after = resources
        .funding_state_lamports()
        .checked_sub(funding_debit)
        .ok_or(GeneralActivationError::Arithmetic)?;
    let expected_funding_lamports_after = request
        .exact_funding_rent_lamports()
        .checked_add(owned.funding_after().remaining().native_lamports_total())
        .ok_or(GeneralActivationError::Arithmetic)?;
    if funding_state_lamports_after != expected_funding_lamports_after {
        return Err(GeneralActivationError::Funding);
    }
    let composite_root_lamports_after = resources
        .composite_root_lamports()
        .checked_add(creation.funding_top_up_lamports())
        .and_then(|value| value.checked_sub(creation.unsolicited_surplus_lamports()))
        .ok_or(GeneralActivationError::Arithmetic)?;
    if composite_root_lamports_after != request.exact_root_rent_lamports() {
        return Err(GeneralActivationError::Funding);
    }
    let rent_credit_lamports_after = resources
        .rent_credit_lamports()
        .checked_add(creation.displaced_prepaid_lamports())
        .and_then(|value| value.checked_add(creation.unsolicited_surplus_lamports()))
        .ok_or(GeneralActivationError::Arithmetic)?;
    let root_header = CapabilityRootHeaderV1::new(
        context.release_set(),
        context.market(),
        context.generation(),
        context.selection(),
    )
    .map_err(|_| GeneralActivationError::Content)?;
    let poststate_digest = activation_poststate_digest(
        root_header,
        owned.root_state(),
        owned.funding_after(),
        composite_root_lamports_after,
        funding_state_lamports_after,
        rent_credit_lamports_after,
    )?;

    Ok(PreparedGeneralActivationV2 {
        disposition: owned.disposition(),
        root_state: owned.root_state(),
        funding_after: owned.funding_after(),
        composite_root_lamports_after,
        funding_state_lamports_after,
        rent_credit_lamports_after,
        poststate_digest,
    })
}

fn authenticate_coordinates(
    context: TradingFamilyContextV1,
    envelope: CoreEffectEnvelopeV1,
    activation: TradingActivationRequestV1<'_>,
    request: GeneralActivationRequestV2,
    resources: GeneralActivationResourcesV2,
) -> Result<(), GeneralActivationError> {
    let selection = context.selection();
    if envelope.action() != CoreEffectActionV1::ActivateCapability
        || envelope.target_role() != Role::Trading
        || envelope.release_set().to_bytes() != context.release_set().to_bytes()
        || envelope.market().to_bytes() != context.market()
        || envelope.generation() != context.generation()
        || envelope.expected_resource_a_revision() != 0
        || envelope.expected_resource_b_revision() != 0
        || activation.selection() != selection
        || activation.funding().funding_count() != GENERAL_FUNDING_COUNT_V2
        || request.capability_root() != context.child_root_key()
        || request.manifest_id() != selection.manifest().to_bytes()
        || request.entry_index() != selection.entry_index()
        || request.config_id() != selection.config().to_bytes()
        || request.funding_state() != resources.funding_state_key()
        || request.rent_credit() != resources.rent_credit_key()
    {
        return Err(GeneralActivationError::Coordinates);
    }
    Ok(())
}

fn supported_content() -> Result<SupportedContentV1, GeneralActivationError> {
    Ok(SupportedContentV1 {
        config_schema: content(GENERAL_CONFIG_SCHEMA_ID_V2)?,
        request_schema: content(GENERAL_ACTIVATION_REQUEST_SCHEMA_ID_V2)?,
        root_schema: content(GENERAL_ROOT_SCHEMA_ID_V2)?,
        account_profile: content(GENERAL_ACTIVATION_ACCOUNT_PROFILE_ID_V2)?,
        derivation_policy: content(CAPABILITY_ROOT_DERIVATION_RELEASE_ID_V1)?,
        effect_schema: content(CAPABILITY_ACTIVATION_EFFECT_SCHEMA_ID_V1)?,
    })
}

fn activation_registers(
    context: TradingFamilyContextV1,
    request: GeneralActivationRequestV2,
    config: GeneralConfigV2,
    resources: GeneralActivationResourcesV2,
) -> Result<Registers, GeneralActivationError> {
    let mut registers = Registers::zeroed();
    for (index, value) in [
        (0, 1),
        (1, context.generation()),
        (2, u64::from(request.entry_index())),
        (3, u64::from(GENERAL_FUNDING_COUNT_V2)),
        (
            4,
            u64::try_from(GENERAL_ROOT_BYTES_V2).map_err(|_| GeneralActivationError::Arithmetic)?,
        ),
        (5, request.current_slot()),
        (6, request.exact_root_rent_lamports()),
        (7, request.exact_funding_rent_lamports()),
        (8, u64::from(config.outcome_count())),
        (9, u64::from(config.max_orders_per_candidate())),
        (10, u64::from(config.max_pages_per_candidate())),
        (11, config.price_scale()),
        (12, resources.composite_root_lamports()),
        (13, resources.funding_state_lamports()),
        (14, resources.rent_credit_lamports()),
    ] {
        registers
            .set_scalar(index, value)
            .map_err(|_| GeneralActivationError::Content)?;
    }
    for (index, value) in [
        (0, context.market()),
        (1, context.release_set().to_bytes()),
        (2, context.selection().capability_release().to_bytes()),
        (3, context.selection().config().to_bytes()),
        (4, context.child_root_key()),
        (5, resources.funding_state_key()),
        (6, resources.rent_credit_key()),
        (7, context.selection().manifest().to_bytes()),
    ] {
        registers
            .set_identity(index, value)
            .map_err(|_| GeneralActivationError::Content)?;
    }
    Ok(registers)
}

fn activation_poststate_digest(
    root_header: CapabilityRootHeaderV1,
    root_state: GeneralRootV2,
    funding_after: FundingStateV1,
    root_lamports_after: u64,
    funding_lamports_after: u64,
    rent_credit_lamports_after: u64,
) -> Result<Identity, GeneralActivationError> {
    let header_bytes = root_header.to_bytes();
    let state_bytes = root_state.to_bytes();
    let funding_bytes = funding_after.to_bytes();
    let root_lamports = root_lamports_after.to_le_bytes();
    let funding_lamports = funding_lamports_after.to_le_bytes();
    let rent_credit_lamports = rent_credit_lamports_after.to_le_bytes();
    identity(
        hashv(&[
            ACTIVATION_POSTSTATE_DOMAIN_V2,
            &header_bytes,
            &state_bytes,
            &funding_bytes,
            &root_lamports,
            &funding_lamports,
            &rent_credit_lamports,
        ])
        .to_bytes(),
    )
}

fn content(bytes: [u8; 32]) -> Result<ContentId, GeneralActivationError> {
    ContentId::new(bytes).map_err(|_| GeneralActivationError::Content)
}

fn identity(bytes: [u8; 32]) -> Result<Identity, GeneralActivationError> {
    Identity::new(bytes).map_err(|_| GeneralActivationError::Coordinates)
}

fn is_zero(value: &[u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::{vec, vec::Vec};

    use dclutch_capability_contract::{
        ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CompartmentFundingV1,
        FundingAmountsV1, FundingQuoteV1, FundingStatus, MANIFEST_HEADER_BYTES,
        MAX_DEPENDENCIES_PER_CAPABILITY,
    };
    use dclutch_capability_program_contract::{
        CAPABILITY_PROGRAM_ACCOUNT_PROFILE_OFFSET, CAPABILITY_PROGRAM_CAPACITY_PROFILE_OFFSET,
        CAPABILITY_PROGRAM_CONFIG_SCHEMA_OFFSET, CAPABILITY_PROGRAM_DERIVATION_POLICY_OFFSET,
        CAPABILITY_PROGRAM_EFFECT_SCHEMA_OFFSET, CAPABILITY_PROGRAM_HEADER_BYTES_V1,
        CAPABILITY_PROGRAM_KIND_OFFSET, CAPABILITY_PROGRAM_MAGIC_V1,
        CAPABILITY_PROGRAM_PROFILE_OFFSET, CAPABILITY_PROGRAM_PROFILE_V1,
        CAPABILITY_PROGRAM_REQUEST_SCHEMA_OFFSET, CAPABILITY_PROGRAM_ROOT_SCHEMA_OFFSET,
        CAPABILITY_PROGRAM_ROOT_STATE_BYTES_OFFSET, CAPABILITY_PROGRAM_SCHEMA_VERSION_OFFSET,
        CAPABILITY_PROGRAM_SCHEMA_VERSION_V1, CAPABILITY_ROOT_HEADER_BYTES_V1,
    };
    use dclutch_general_config_contract::{GENERAL_CAPABILITY_KIND_ID_V1, GeneralConfigV2Input};
    use dclutch_market_core_codec::CapabilityFundingHeaderV1;
    use dclutch_registry_svm::AuthenticatedRoleReceiptV1;
    use dclutch_release_set_contract::{
        ArtifactReleaseIdV1, CapabilityExecutionSelectionV1, ExecutionRoleV1, ProgramIdentityV1,
    };
    use dclutch_transition_vm::{HEADER_BYTES, INSTRUCTION_BYTES, MAGIC, VERSION};

    use super::*;

    const GENERATION: u64 = 7;
    const ROOT_RENT: u64 = 100;
    const FUNDING_RENT: u64 = 20;
    const ACTIVATION_SLOT: u64 = 9;

    struct Fixture {
        context: TradingFamilyContextV1,
        envelope: CoreEffectEnvelopeV1,
        role_request: Vec<u8>,
        manifest: Vec<u8>,
        descriptor: Vec<u8>,
        config: [u8; dclutch_general_config_contract::GENERAL_CONFIG_BYTES_V2],
        resources: GeneralActivationResourcesV2,
    }

    fn bytes(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn cid(byte: u8) -> ContentId {
        ContentId::new(bytes(byte)).expect("content identity")
    }

    fn core_id(value: [u8; 32]) -> Identity {
        Identity::new(value).expect("Core identity")
    }

    fn write(output: &mut [u8], offset: usize, value: &[u8]) {
        let end = offset.checked_add(value.len()).expect("fixture width");
        output
            .get_mut(offset..end)
            .expect("fixture destination")
            .copy_from_slice(value);
    }

    fn instruction(opcode: u8, a: u8, b: u8, immediate: u64) -> [u8; INSTRUCTION_BYTES] {
        let mut encoded = [0_u8; INSTRUCTION_BYTES];
        encoded[0] = opcode;
        encoded[1] = a;
        encoded[2] = b;
        encoded[8..].copy_from_slice(&immediate.to_le_bytes());
        encoded
    }

    fn transition_program(accepts_generation: bool) -> Vec<u8> {
        let expected_generation = if accepts_generation {
            GENERATION
        } else {
            GENERATION + 1
        };
        let instructions = [
            instruction(0, 16, 0, 1),
            instruction(1, 0, 16, 0),
            instruction(0, 17, 0, GENERAL_ROOT_BYTES_V2 as u64),
            instruction(1, 4, 17, 0),
            instruction(0, 18, 0, 1),
            instruction(1, 3, 18, 0),
            instruction(0, 19, 0, expected_generation),
            instruction(1, 1, 19, 0),
            instruction(6, 6, 0, 0),
            instruction(6, 7, 0, 0),
            instruction(6, 8, 0, 0),
            instruction(6, 11, 0, 0),
        ];
        let mut program = Vec::with_capacity(HEADER_BYTES + instructions.len() * INSTRUCTION_BYTES);
        program.extend_from_slice(&MAGIC);
        program.push(VERSION);
        program.push(u8::try_from(instructions.len()).expect("instruction count"));
        program.extend_from_slice(&[0, 0]);
        for encoded in instructions {
            program.extend_from_slice(&encoded);
        }
        program
    }

    fn descriptor_bytes(capacity: ContentId, accepts_generation: bool) -> Vec<u8> {
        let transition = transition_program(accepts_generation);
        let mut descriptor = vec![0_u8; CAPABILITY_PROGRAM_HEADER_BYTES_V1 + transition.len()];
        write(&mut descriptor, 0, &CAPABILITY_PROGRAM_MAGIC_V1);
        write(
            &mut descriptor,
            CAPABILITY_PROGRAM_SCHEMA_VERSION_OFFSET,
            &CAPABILITY_PROGRAM_SCHEMA_VERSION_V1.to_le_bytes(),
        );
        write(
            &mut descriptor,
            CAPABILITY_PROGRAM_PROFILE_OFFSET,
            &CAPABILITY_PROGRAM_PROFILE_V1.to_le_bytes(),
        );
        for (offset, value) in [
            (
                CAPABILITY_PROGRAM_KIND_OFFSET,
                GENERAL_CAPABILITY_KIND_ID_V1,
            ),
            (
                CAPABILITY_PROGRAM_CONFIG_SCHEMA_OFFSET,
                GENERAL_CONFIG_SCHEMA_ID_V2,
            ),
            (
                CAPABILITY_PROGRAM_REQUEST_SCHEMA_OFFSET,
                GENERAL_ACTIVATION_REQUEST_SCHEMA_ID_V2,
            ),
            (
                CAPABILITY_PROGRAM_ROOT_SCHEMA_OFFSET,
                GENERAL_ROOT_SCHEMA_ID_V2,
            ),
            (
                CAPABILITY_PROGRAM_ACCOUNT_PROFILE_OFFSET,
                GENERAL_ACTIVATION_ACCOUNT_PROFILE_ID_V2,
            ),
            (
                CAPABILITY_PROGRAM_DERIVATION_POLICY_OFFSET,
                CAPABILITY_ROOT_DERIVATION_RELEASE_ID_V1,
            ),
            (
                CAPABILITY_PROGRAM_CAPACITY_PROFILE_OFFSET,
                capacity.to_bytes(),
            ),
            (
                CAPABILITY_PROGRAM_EFFECT_SCHEMA_OFFSET,
                CAPABILITY_ACTIVATION_EFFECT_SCHEMA_ID_V1,
            ),
        ] {
            write(&mut descriptor, offset, &value);
        }
        write(
            &mut descriptor,
            CAPABILITY_PROGRAM_ROOT_STATE_BYTES_OFFSET,
            &u32::try_from(GENERAL_ROOT_BYTES_V2)
                .expect("root width")
                .to_le_bytes(),
        );
        write(
            &mut descriptor,
            CAPABILITY_PROGRAM_HEADER_BYTES_V1,
            &transition,
        );
        descriptor
    }

    fn funding_quote() -> FundingQuoteV1 {
        let rent = CompartmentFundingV1::native_lamports(ROOT_RENT).expect("root rent");
        let none = CompartmentFundingV1::not_applicable();
        let amounts = FundingAmountsV1::new(rent, none, none, none, none, none, none)
            .expect("funding amounts");
        FundingQuoteV1::new(amounts, None).expect("funding quote")
    }

    fn fixture(accepts_generation: bool) -> Fixture {
        let program_id = Pubkey::new_from_array(bytes(0x90));
        let market = bytes(0x11);
        let release_set = cid(0x70);
        let capacity = cid(0x41);
        let descriptor = descriptor_bytes(capacity, accepts_generation);
        let descriptor_id = ContentId::new(hash(&descriptor).to_bytes()).expect("descriptor ID");
        let config_value = GeneralConfigV2::new(GeneralConfigV2Input {
            capacity_profile_id: capacity.to_bytes(),
            claim_basis_id: bytes(0x42),
            capability_program_id: descriptor_id.to_bytes(),
            generation: GENERATION,
            price_scale: 100,
            collection_slots: 10,
            selection_slots: 11,
            settlement_slots: 12,
            max_orders_per_candidate: 64,
            max_pages_per_candidate: 2,
            continuation_reward_lamports: 5,
            selection_policy_id: bytes(0x43),
            outcome_count: 2,
            quote_surplus_beneficiary: bytes(0x44),
        })
        .expect("General config");
        let config = config_value.to_bytes();
        let config_id = ContentId::new(hash(&config).to_bytes()).expect("config ID");
        let entry = CapabilityEntryV1::new(
            ContentId::new(GENERAL_CAPABILITY_KIND_ID_V1).expect("General kind"),
            descriptor_id,
            config_id,
            capacity,
            ContentId::new(GENERAL_ROOT_SCHEMA_ID_V2).expect("root schema"),
            ContentId::new(CAPABILITY_ROOT_DERIVATION_RELEASE_ID_V1).expect("root derivation"),
            ActivationPolicy::RequiredAtFounding,
            0,
            0,
            [0; MAX_DEPENDENCIES_PER_CAPABILITY],
            funding_quote(),
        )
        .expect("manifest entry");
        let mut manifest = vec![0_u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
        CapabilityManifestV1::encode_into(&[entry], &mut manifest).expect("manifest encoding");
        let manifest_id = ContentId::new(hash(&manifest).to_bytes()).expect("manifest ID");
        let manifest_view = CapabilityManifestV1::decode(&manifest).expect("manifest view");
        let pending_custody =
            FundingCustodyObservationV1::native_only(ROOT_RENT + FUNDING_RENT, FUNDING_RENT)
                .expect("pending custody");
        let funding = FundingStateV1::new(manifest_id, manifest_view, 0, pending_custody)
            .expect("pending funding");
        let funding_derivation = CapabilityFundingDerivationV1::new(
            market,
            GENERATION,
            manifest_id,
            manifest_view,
            funding,
        )
        .expect("funding derivation");
        let funding_key =
            Pubkey::find_program_address(&funding_derivation.seed_components(), &program_id).0;
        let selection = CapabilityExecutionSelectionV1::new(
            0,
            manifest_id,
            ContentId::new(GENERAL_CAPABILITY_KIND_ID_V1).expect("General kind"),
            descriptor_id,
            config_id,
        )
        .expect("selection");
        let root_header = CapabilityRootHeaderV1::new(release_set, market, GENERATION, selection)
            .expect("root header");
        let root_seeds = root_header.seeds();
        let root_key = Pubkey::find_program_address(&root_seeds.as_slices(), &program_id).0;
        let rent_credit_key = bytes(0x66);
        let request = GeneralActivationRequestV2::new(
            root_key.to_bytes(),
            config_id.to_bytes(),
            manifest_id.to_bytes(),
            funding_key.to_bytes(),
            rent_credit_key,
            0,
            ACTIVATION_SLOT,
            ROOT_RENT,
            FUNDING_RENT,
        )
        .expect("activation request");
        let funding_header = CapabilityFundingHeaderV1::new(1).expect("funding header");
        let mut role_request = Vec::with_capacity(
            dclutch_release_set_contract::CAPABILITY_EXECUTION_SELECTION_BYTES_V1
                + dclutch_market_core_codec::CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1
                + dclutch_general_config_contract::GENERAL_ACTIVATION_REQUEST_BYTES_V2,
        );
        role_request.extend_from_slice(&selection.to_bytes());
        role_request.extend_from_slice(&funding_header.encode());
        role_request.extend_from_slice(&request.to_bytes());
        let role_request_digest = core_id(hash(&role_request).to_bytes());
        let envelope = CoreEffectEnvelopeV1::new(
            CoreEffectActionV1::ActivateCapability,
            Role::Trading,
            core_id(bytes(0xa0)),
            core_id(bytes(0xa1)),
            core_id(release_set.to_bytes()),
            core_id(market),
            core_id(bytes(0xa2)),
            core_id(bytes(0xa3)),
            role_request_digest,
            GENERATION,
            0,
            0,
            u32::try_from(role_request.len()).expect("role request width"),
        )
        .expect("Core envelope");
        let receipt = AuthenticatedRoleReceiptV1::new(
            ExecutionRoleV1::Trading,
            release_set,
            ProgramIdentityV1::new(program_id.to_bytes()).expect("Trading program"),
            ArtifactReleaseIdV1::new(bytes(0xa4)).expect("artifact release"),
            cid(0xa5),
        );
        let context = TradingFamilyContextV1::authenticate_activation(
            &program_id,
            &root_key,
            root_header,
            CAPABILITY_ROOT_HEADER_BYTES_V1 + GENERAL_ROOT_BYTES_V2,
            receipt,
        )
        .expect("authenticated family context");
        let resources = GeneralActivationResourcesV2::new(
            funding_key.to_bytes(),
            funding,
            rent_credit_key,
            40,
            ROOT_RENT + FUNDING_RENT,
            7,
            None,
        )
        .expect("activation resources");
        Fixture {
            context,
            envelope,
            role_request,
            manifest,
            descriptor,
            config,
            resources,
        }
    }

    fn prepare(fixture: &Fixture) -> Result<PreparedGeneralActivationV2, GeneralActivationError> {
        prepare_activation_authenticated(
            fixture.context,
            fixture.envelope,
            &fixture.role_request,
            &fixture.manifest,
            &fixture.descriptor,
            &fixture.config,
            fixture.resources,
        )
    }

    #[test]
    fn account_profile_label_has_exact_content_identity() {
        assert_eq!(
            hash(GENERAL_ACTIVATION_ACCOUNT_PROFILE_PREIMAGE_V2).to_bytes(),
            GENERAL_ACTIVATION_ACCOUNT_PROFILE_ID_V2
        );
    }

    #[test]
    fn prepare_create_conserves_dust_prepaid_rent_and_returns_complete_plan() {
        let fixture = fixture(true);
        let before_total = fixture.resources.composite_root_lamports()
            + fixture.resources.funding_state_lamports()
            + fixture.resources.rent_credit_lamports();
        let prepared = prepare(&fixture).expect("pure activation plan");
        assert_eq!(
            prepared.disposition(),
            GeneralActivationDispositionV2::Create
        );
        assert_eq!(prepared.root_state().market(), fixture.context.market());
        assert_eq!(prepared.root_state().revision(), 1);
        assert_eq!(prepared.funding_after().status(), FundingStatus::Active);
        assert_eq!(prepared.funding_after().activation_slot(), ACTIVATION_SLOT);
        assert_eq!(prepared.composite_root_lamports_after(), ROOT_RENT);
        assert_eq!(prepared.funding_state_lamports_after(), FUNDING_RENT);
        assert_eq!(prepared.rent_credit_lamports_after(), 47);
        assert_eq!(
            prepared.composite_root_lamports_after()
                + prepared.funding_state_lamports_after()
                + prepared.rent_credit_lamports_after(),
            before_total
        );
        assert_ne!(prepared.poststate_digest().to_bytes(), [0; 32]);
    }

    #[test]
    fn exact_replay_is_idempotent_and_preserves_every_poststate() {
        let mut fixture = fixture(true);
        let created = prepare(&fixture).expect("creation plan");
        fixture.resources = GeneralActivationResourcesV2::new(
            fixture.resources.funding_state_key(),
            created.funding_after(),
            fixture.resources.rent_credit_key(),
            created.composite_root_lamports_after(),
            created.funding_state_lamports_after(),
            created.rent_credit_lamports_after(),
            Some(created.root_state()),
        )
        .expect("replay resources");
        let replay = prepare(&fixture).expect("idempotent replay");
        assert_eq!(
            replay.disposition(),
            GeneralActivationDispositionV2::Idempotent
        );
        assert_eq!(replay.root_state(), created.root_state());
        assert_eq!(replay.funding_after(), created.funding_after());
        assert_eq!(
            replay.composite_root_lamports_after(),
            created.composite_root_lamports_after()
        );
        assert_eq!(
            replay.funding_state_lamports_after(),
            created.funding_state_lamports_after()
        );
        assert_eq!(
            replay.rent_credit_lamports_after(),
            created.rent_credit_lamports_after()
        );
        assert_eq!(replay.poststate_digest(), created.poststate_digest());
    }

    #[test]
    fn hostile_content_and_late_funding_refuse_without_input_mutation() {
        let fixture = fixture(true);
        let resources_before = fixture.resources;

        let mut descriptor = fixture.descriptor.clone();
        *descriptor
            .get_mut(CAPABILITY_PROGRAM_EFFECT_SCHEMA_OFFSET)
            .expect("descriptor effect byte") ^= 1;
        assert_eq!(
            prepare_activation_authenticated(
                fixture.context,
                fixture.envelope,
                &fixture.role_request,
                &fixture.manifest,
                &descriptor,
                &fixture.config,
                fixture.resources,
            ),
            Err(GeneralActivationError::Common(TradingSbfError::Content))
        );

        let mut role_request = fixture.role_request.clone();
        let last = role_request.last_mut().expect("request byte");
        *last ^= 1;
        assert_eq!(
            prepare_activation_authenticated(
                fixture.context,
                fixture.envelope,
                &role_request,
                &fixture.manifest,
                &fixture.descriptor,
                &fixture.config,
                fixture.resources,
            ),
            Err(GeneralActivationError::Instruction)
        );

        let late_resources = GeneralActivationResourcesV2::new(
            fixture.resources.funding_state_key(),
            fixture.resources.funding_state(),
            fixture.resources.rent_credit_key(),
            fixture.resources.composite_root_lamports(),
            fixture.resources.funding_state_lamports() - 1,
            fixture.resources.rent_credit_lamports(),
            None,
        )
        .expect("late hostile resources");
        assert_eq!(
            prepare_activation_authenticated(
                fixture.context,
                fixture.envelope,
                &fixture.role_request,
                &fixture.manifest,
                &fixture.descriptor,
                &fixture.config,
                late_resources,
            ),
            Err(GeneralActivationError::Funding)
        );
        assert_eq!(fixture.resources, resources_before);
        assert_eq!(
            late_resources.funding_state_lamports(),
            ROOT_RENT + FUNDING_RENT - 1
        );
    }

    #[test]
    fn admitted_transition_refusal_is_atomic() {
        let fixture = fixture(false);
        let resources_before = fixture.resources;
        assert_eq!(
            prepare(&fixture),
            Err(GeneralActivationError::Common(TradingSbfError::Transition))
        );
        assert_eq!(fixture.resources, resources_before);
    }
}
