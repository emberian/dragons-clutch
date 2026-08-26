//! Exact register and TransitionVM contract for inline ordinary Direct V3.
//!
//! This is a successor bank, not an extension of the historical 41/4 map.
//! Native Ed25519 signer identities and request-carried maker identities occupy
//! distinct registers and the transition requires equality. The immutable
//! config is hostile-decoded elsewhere, but this projection additionally binds
//! its exact canonical bytes to the finalized content identity before placing
//! `price_scale`, `fee_basis_points`, and `fee_recipient` in the bank. Finalized
//! config records are immutable and therefore have no mutable revision field.

use dclutch_transition_vm::v3::{
    IdentityRegisterV3, InstructionV3, ProgramGeometryV3, ScalarRegisterV3, encode_program_atomic,
};
use sha2::{Digest, Sha256};

use crate::{
    execution_v3::DirectInlineOrdinaryRequestV3,
    successor::{DIRECT_FEE_DENOMINATOR_V1, DirectExecutionConfigV1},
};

/// Exact common scalar-bank width for inline ordinary V3.
pub const DIRECT_ORDINARY_COMMON_SCALARS_V3: usize = 64;
/// Exact common identity-bank width for inline ordinary V3.
pub const DIRECT_ORDINARY_COMMON_IDENTITIES_V3: usize = 32;
/// Per-Product-item scalar stride: canonical item index plus Claims quantity.
pub const DIRECT_ORDINARY_ITEM_SCALAR_STRIDE_V3: u16 = 2;
/// Ordinary has no Product-item identity tail.
pub const DIRECT_ORDINARY_ITEM_IDENTITY_STRIDE_V3: u16 = 0;
/// Exact prelude instruction count of the ordinary semantic program.
pub const DIRECT_ORDINARY_PRELUDE_INSTRUCTIONS_V3: usize = 64;
/// Exact per-Product-item instruction count of the ordinary semantic program.
pub const DIRECT_ORDINARY_ITEM_INSTRUCTIONS_V3: usize = 2;
/// Exact instruction count of the ordinary semantic program.
pub const DIRECT_ORDINARY_TRANSITION_INSTRUCTIONS_V3: usize =
    DIRECT_ORDINARY_PRELUDE_INSTRUCTIONS_V3 + DIRECT_ORDINARY_ITEM_INSTRUCTIONS_V3;
/// Exact encoded ordinary TransitionVM V3 width.
pub const DIRECT_ORDINARY_TRANSITION_BYTES_V3: usize = dclutch_transition_vm::v3::HEADER_BYTES
    + DIRECT_ORDINARY_TRANSITION_INSTRUCTIONS_V3 * dclutch_transition_vm::v3::INSTRUCTION_BYTES;

/// Scalar register: Direct root phase (`Open = 0`).
pub const SCALAR_ROOT_PHASE_V3: usize = 0;
/// Scalar register: trusted Clock slot.
pub const SCALAR_SLOT_V3: usize = 1;
/// Scalar register: seller inclusive validity start.
pub const SCALAR_SELLER_VALID_FROM_V3: usize = 2;
/// Scalar register: seller inclusive validity end.
pub const SCALAR_SELLER_VALID_THROUGH_V3: usize = 3;
/// Scalar register: buyer inclusive validity start.
pub const SCALAR_BUYER_VALID_FROM_V3: usize = 4;
/// Scalar register: buyer inclusive validity end.
pub const SCALAR_BUYER_VALID_THROUGH_V3: usize = 5;
/// Scalar register: seller side tag.
pub const SCALAR_SELLER_SIDE_V3: usize = 6;
/// Scalar register: buyer side tag.
pub const SCALAR_BUYER_SIDE_V3: usize = 7;
/// Scalar register: seller generation.
pub const SCALAR_SELLER_GENERATION_V3: usize = 8;
/// Scalar register: buyer generation.
pub const SCALAR_BUYER_GENERATION_V3: usize = 9;
/// Scalar register: authenticated Core Market generation.
pub const SCALAR_MARKET_GENERATION_V3: usize = 10;
/// Scalar register: seller Product outcome coordinate.
pub const SCALAR_SELLER_OUTCOME_V3: usize = 11;
/// Scalar register: buyer Product outcome coordinate.
pub const SCALAR_BUYER_OUTCOME_V3: usize = 12;
/// Scalar register: authenticated Product runtime outcome count.
pub const SCALAR_OUTCOME_COUNT_V3: usize = 13;
/// Scalar register: seller inline lifecycle tag.
pub const SCALAR_SELLER_LIFECYCLE_V3: usize = 14;
/// Scalar register: seller maximum fill.
pub const SCALAR_SELLER_MAXIMUM_V3: usize = 15;
/// Scalar register: buyer inline lifecycle tag.
pub const SCALAR_BUYER_LIFECYCLE_V3: usize = 16;
/// Scalar register: buyer maximum fill.
pub const SCALAR_BUYER_MAXIMUM_V3: usize = 17;
/// Scalar register: seller signed nonce.
pub const SCALAR_SELLER_NONCE_V3: usize = 18;
/// Scalar register: buyer signed nonce.
pub const SCALAR_BUYER_NONCE_V3: usize = 19;
/// Scalar register: seller replay next nonce.
pub const SCALAR_SELLER_NEXT_NONCE_V3: usize = 20;
/// Scalar register: buyer replay next nonce.
pub const SCALAR_BUYER_NEXT_NONCE_V3: usize = 21;
/// Scalar register: seller minimum price.
pub const SCALAR_SELLER_LIMIT_V3: usize = 22;
/// Scalar register: matcher execution price.
pub const SCALAR_EXECUTION_PRICE_V3: usize = 23;
/// Scalar register: buyer maximum price.
pub const SCALAR_BUYER_LIMIT_V3: usize = 24;
/// Scalar register: immutable config price scale.
pub const SCALAR_PRICE_SCALE_V3: usize = 25;
/// Scalar register: seller-signed fee basis points.
pub const SCALAR_SELLER_FEE_BPS_V3: usize = 26;
/// Scalar register: buyer-signed fee basis points.
pub const SCALAR_BUYER_FEE_BPS_V3: usize = 27;
/// Scalar register: immutable config fee basis points.
pub const SCALAR_POLICY_FEE_BPS_V3: usize = 28;
/// Scalar register: positive matcher-selected fill.
pub const SCALAR_FILL_V3: usize = 29;
/// Scalar register: Claims aggregate pre-revision.
pub const SCALAR_CLAIMS_MARKET_REVISION_V3: usize = 30;
/// Scalar register: seller Position pre-revision.
pub const SCALAR_SELLER_POSITION_REVISION_V3: usize = 31;
/// Scalar register: buyer Position pre-revision.
pub const SCALAR_BUYER_POSITION_REVISION_V3: usize = 32;
/// Scalar register: Custody replay pre-revision.
pub const SCALAR_CUSTODY_REVISION_V3: usize = 33;
/// Scalar register: exact pre-transition open-maker-root count.
pub const SCALAR_ROOT_OPEN_COUNT_V3: usize = 34;
/// Scalar register: exact post-transition open-maker-root count.
pub const SCALAR_ROOT_OPEN_COUNT_AFTER_V3: usize = 35;
/// Scalar register: lifecycle-owned seller first-use bit.
pub const SCALAR_SELLER_CREATED_V3: usize = 36;
/// Scalar register: seller persisted bump observation.
pub const SCALAR_SELLER_BUMP_OBSERVATION_V3: usize = 37;
/// Program-owned zero constant.
pub const SCALAR_ZERO_V3: usize = 38;
/// Program-owned one constant.
pub const SCALAR_ONE_V3: usize = 39;
/// Program-owned basis-point denominator.
pub const SCALAR_FEE_DENOMINATOR_V3: usize = 40;
/// Derived seller successor nonce.
pub const SCALAR_SELLER_NONCE_AFTER_V3: usize = 41;
/// Derived buyer successor nonce.
pub const SCALAR_BUYER_NONCE_AFTER_V3: usize = 42;
/// Derived exact gross collateral.
pub const SCALAR_GROSS_V3: usize = 43;
/// Derived one-side cumulative floor fee.
pub const SCALAR_FEE_V3: usize = 44;
/// Derived seller-net collateral transfer.
pub const SCALAR_SELLER_NET_V3: usize = 45;
/// Derived total buyer collateral debit.
pub const SCALAR_BUYER_DEBIT_V3: usize = 46;
/// Derived combined seller-plus-buyer fee transfer.
pub const SCALAR_COMBINED_FEE_V3: usize = 47;
/// Derived terminal seller-only Custody route enable bit.
pub const SCALAR_SELLER_TERMINAL_ROUTE_ENABLED_V3: usize = 48;
/// Buyer persisted historical-rent-principal observation.
pub const SCALAR_BUYER_RENT_PRINCIPAL_OBSERVATION_V3: usize = 49;
/// Lifecycle-owned buyer historical rent principal.
pub const SCALAR_BUYER_RENT_PRINCIPAL_V3: usize = 50;
/// Program-owned maker replay ABI version after Transition.
pub const SCALAR_MAKER_VERSION_V3: usize = 51;
/// Derived seller-intermediate plus fee-continuation route enable bit.
pub const SCALAR_SELLER_INTERMEDIATE_ROUTE_ENABLED_V3: usize = 52;
/// Derived nonzero combined-fee bit.
pub const SCALAR_FEE_NONZERO_V3: usize = 53;
/// Reserved for the replay revision after seller-net.
pub const SCALAR_CUSTODY_AFTER_SELLER_V3: usize = 54;
/// Reserved for the replay revision after combined fee.
pub const SCALAR_CUSTODY_AFTER_FEE_V3: usize = 55;
/// Lifecycle-owned seller canonical bump.
pub const SCALAR_SELLER_BUMP_V3: usize = 56;
/// Seller persisted historical-rent-principal observation.
pub const SCALAR_SELLER_RENT_PRINCIPAL_OBSERVATION_V3: usize = 57;
/// Lifecycle-owned seller historical rent principal.
pub const SCALAR_SELLER_RENT_PRINCIPAL_V3: usize = 58;
/// Scalar register: lifecycle-owned buyer first-use bit.
pub const SCALAR_BUYER_CREATED_V3: usize = 59;
/// Scalar register: buyer persisted bump observation.
pub const SCALAR_BUYER_BUMP_OBSERVATION_V3: usize = 60;
/// Lifecycle-owned buyer canonical bump.
pub const SCALAR_BUYER_BUMP_V3: usize = 61;
/// Reserved for exact Claims transfer quantity.
pub const SCALAR_CLAIM_TRANSFER_V3: usize = 62;
/// Derived terminal fee-only Custody route enable bit.
pub const SCALAR_FEE_SOLE_ROUTE_ENABLED_V3: usize = 63;
/// Program-owned maker replay magic word after fee arithmetic completes.
pub const SCALAR_MAKER_MAGIC_V3: usize = SCALAR_FEE_DENOMINATOR_V3;

/// Per-item scalar slot containing the canonical Product item index.
pub const ITEM_SCALAR_INDEX_V3: u16 = 0;
/// Per-item scalar slot containing the exact Claims transfer quantity.
pub const ITEM_SCALAR_CLAIM_QUANTITY_V3: u16 = 1;

/// Identity register: SHA-256 of the complete family request.
pub const IDENTITY_PARENT_REQUEST_DIGEST_V3: usize = 0;
/// Lifecycle-owned seller immutable rent beneficiary.
pub const IDENTITY_SELLER_RENT_BENEFICIARY_V3: usize = 1;
/// Identity register: immutable config fee recipient.
pub const IDENTITY_FEE_RECIPIENT_V3: usize = 2;
/// Identity register: authenticated logical Core Market.
pub const IDENTITY_MARKET_V3: usize = 3;
/// Identity register: native-Ed25519 seller signer.
pub const IDENTITY_SELLER_NATIVE_SIGNER_V3: usize = 4;
/// Identity register: native-Ed25519 buyer signer.
pub const IDENTITY_BUYER_NATIVE_SIGNER_V3: usize = 5;
/// Identity register: request-carried seller maker.
pub const IDENTITY_SELLER_REQUEST_MAKER_V3: usize = 6;
/// Identity register: request-carried buyer maker.
pub const IDENTITY_BUYER_REQUEST_MAKER_V3: usize = 7;
/// Identity register: seller signed-intent Market.
pub const IDENTITY_SELLER_INTENT_MARKET_V3: usize = 8;
/// Identity register: buyer signed-intent Market.
pub const IDENTITY_BUYER_INTENT_MARKET_V3: usize = 9;
/// Identity register: immutable execution release set.
pub const IDENTITY_RELEASE_SET_V3: usize = 10;
/// Identity register: finalized Product record digest.
pub const IDENTITY_PRODUCT_RECORD_DIGEST_V3: usize = 11;
/// Identity register: semantic LiabilityBasis identity.
pub const IDENTITY_SEMANTIC_BASIS_V3: usize = 12;
/// Identity register: finalized linked-basis record digest.
pub const IDENTITY_LINKED_BASIS_RECORD_V3: usize = 13;
/// Identity register: current Registry-selected Trading program.
pub const IDENTITY_TRADING_PROGRAM_V3: usize = 14;
/// Lifecycle-owned seller state owner, equal to current Trading.
pub const IDENTITY_SELLER_STATE_OWNER_V3: usize = 15;
/// Lifecycle-owned buyer state owner, equal to current Trading.
pub const IDENTITY_BUYER_STATE_OWNER_V3: usize = 16;
/// Identity register: immutable Realm record identity.
pub const IDENTITY_REALM_V3: usize = 17;
/// Identity register: Realm-selected collateral mint.
pub const IDENTITY_MINT_V3: usize = 18;
/// Identity register: Realm-selected token program.
pub const IDENTITY_TOKEN_PROGRAM_V3: usize = 19;
/// Lifecycle-owned buyer immutable rent beneficiary.
pub const IDENTITY_BUYER_RENT_BENEFICIARY_V3: usize = 20;
/// Lifecycle-owned seller maker replay root.
pub const IDENTITY_SELLER_MAKER_ROOT_V3: usize = 21;
/// Lifecycle-owned buyer maker replay root and Custody context.
pub const IDENTITY_BUYER_MAKER_ROOT_V3: usize = 22;
/// Identity register: independently observed System Program.
pub const IDENTITY_SYSTEM_PROGRAM_V3: usize = 23;
/// Identity register: canonical Custody transfer authority.
pub const IDENTITY_CUSTODY_AUTHORITY_V3: usize = 24;
/// Seller persisted rent-beneficiary observation.
pub const IDENTITY_SELLER_RENT_BENEFICIARY_OBSERVATION_V3: usize = 25;
/// Buyer persisted rent-beneficiary observation.
pub const IDENTITY_BUYER_RENT_BENEFICIARY_OBSERVATION_V3: usize = 26;
/// Identity register: fee recipient's exact collateral token account.
pub const IDENTITY_FEE_TOKEN_ACCOUNT_V3: usize = 27;
/// Identity register: seller-signed collateral token account.
pub const IDENTITY_SELLER_COLLATERAL_REQUEST_V3: usize = 28;
/// Identity register: buyer-signed collateral token account.
pub const IDENTITY_BUYER_COLLATERAL_REQUEST_V3: usize = 29;
/// Identity register: authenticated seller destination token account.
pub const IDENTITY_SELLER_TOKEN_ACCOUNT_V3: usize = 30;
/// Identity register: authenticated buyer source token account.
pub const IDENTITY_BUYER_TOKEN_ACCOUNT_V3: usize = 31;

/// Stable register projection or program-emission refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectOrdinaryRegisterErrorV3 {
    /// Caller-owned register or program buffers had another exact width.
    InvalidLength,
    /// Finalized config bytes did not match the authenticated content identity.
    ConfigContentMismatch,
    /// An authenticated semantic, release, program, account, or PDA identity was zero.
    ZeroIdentity,
    /// Typed TransitionVM program emission refused.
    TransitionProgram,
}

/// Result alias for the ordinary V3 register contract.
pub type Result<T> = core::result::Result<T, DirectOrdinaryRegisterErrorV3>;

/// Chain-authenticated facts projected outside the untrusted Direct request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectOrdinaryAuthenticatedContextV3 {
    /// SHA-256 of the complete canonical family request.
    pub parent_request_digest: [u8; 32],
    /// Finalized config content ID selected by the descriptor.
    pub config_content_id: [u8; 32],
    /// Exact hostile-decoded immutable Direct config.
    pub config: DirectExecutionConfigV1,
    /// Logical Core Market account.
    pub market: [u8; 32],
    /// Current Market generation.
    pub generation: u64,
    /// Product Runtime V2-authenticated outcome count.
    pub outcome_count: u32,
    /// Trusted current Clock slot.
    pub slot: u64,
    /// Direct root phase tag; open is zero.
    pub root_phase: u8,
    /// Seller maker replay next nonce.
    pub seller_next_nonce: u64,
    /// Buyer maker replay next nonce.
    pub buyer_next_nonce: u64,
    /// Exact pre-transition count of live maker replay roots.
    pub root_open_maker_count: u64,
    /// Lifecycle-owned seller first-use bit.
    pub seller_created: bool,
    /// Seller live-state bump observation (zero when vacant).
    pub seller_bump_observation: u8,
    /// Lifecycle-owned canonical seller bump.
    pub seller_bump: u8,
    /// Seller live-state historical rent observation (zero when vacant).
    pub seller_rent_principal_observation: u64,
    /// Lifecycle-owned seller historical rent principal.
    pub seller_rent_principal: u64,
    /// Lifecycle-owned buyer first-use bit.
    pub buyer_created: bool,
    /// Buyer live-state bump observation (zero when vacant).
    pub buyer_bump_observation: u8,
    /// Lifecycle-owned canonical buyer bump.
    pub buyer_bump: u8,
    /// Buyer live-state historical rent observation (zero when vacant).
    pub buyer_rent_principal_observation: u64,
    /// Lifecycle-owned buyer historical rent principal.
    pub buyer_rent_principal: u64,
    /// Claims aggregate pre-revision.
    pub claims_market_revision: u64,
    /// Seller Position pre-revision.
    pub seller_position_revision: u64,
    /// Buyer Position pre-revision.
    pub buyer_position_revision: u64,
    /// Custody replay pre-revision.
    pub custody_revision: u64,
    /// Current release-set identity.
    pub release_set: [u8; 32],
    /// Finalized Product record digest.
    pub product_record_digest: [u8; 32],
    /// Exact semantic LiabilityBasis identity.
    pub semantic_basis: [u8; 32],
    /// Finalized linked-basis record digest.
    pub linked_basis_record_digest: [u8; 32],
    /// Registry-selected Trading program.
    pub trading_program: [u8; 32],
    /// Immutable Realm identity.
    pub realm: [u8; 32],
    /// Realm-selected collateral mint.
    pub mint: [u8; 32],
    /// Realm-selected token program.
    pub token_program: [u8; 32],
    /// Seller maker replay root.
    pub seller_maker_root: [u8; 32],
    /// Buyer maker replay root and Custody context.
    pub buyer_maker_root: [u8; 32],
    /// Canonical System Program account used to anchor lifecycle payers.
    pub system_program: [u8; 32],
    /// Canonical Custody authority.
    pub custody_authority: [u8; 32],
    /// Lifecycle-owned seller immutable rent beneficiary.
    pub seller_rent_beneficiary: [u8; 32],
    /// Seller live-state rent-beneficiary observation (zero when vacant).
    pub seller_rent_beneficiary_observation: [u8; 32],
    /// Lifecycle-owned buyer immutable rent beneficiary.
    pub buyer_rent_beneficiary: [u8; 32],
    /// Buyer live-state rent-beneficiary observation (zero when vacant).
    pub buyer_rent_beneficiary_observation: [u8; 32],
    /// Exact fee collateral token account.
    pub fee_token_account: [u8; 32],
    /// Authenticated seller destination token account.
    pub seller_token_account: [u8; 32],
    /// Authenticated buyer source token account.
    pub buyer_token_account: [u8; 32],
    /// Native signature adapter's seller identity.
    pub seller_native_signer: [u8; 32],
    /// Native signature adapter's buyer identity.
    pub buyer_native_signer: [u8; 32],
}

/// Project one exact inline ordinary input bank atomically.
pub fn project_direct_ordinary_registers_v3(
    request: DirectInlineOrdinaryRequestV3,
    context: DirectOrdinaryAuthenticatedContextV3,
    scalar_scratch: &mut [u64],
    identity_scratch: &mut [[u8; 32]],
    scalar_output: &mut [u64],
    identity_output: &mut [[u8; 32]],
) -> Result<()> {
    let tail_count = usize::try_from(context.outcome_count)
        .map_err(|_| DirectOrdinaryRegisterErrorV3::InvalidLength)?;
    let scalar_width = tail_count
        .checked_mul(usize::from(DIRECT_ORDINARY_ITEM_SCALAR_STRIDE_V3))
        .and_then(|tail| DIRECT_ORDINARY_COMMON_SCALARS_V3.checked_add(tail))
        .ok_or(DirectOrdinaryRegisterErrorV3::InvalidLength)?;
    if scalar_scratch.len() != scalar_width
        || scalar_output.len() != scalar_width
        || identity_scratch.len() != DIRECT_ORDINARY_COMMON_IDENTITIES_V3
        || identity_output.len() != DIRECT_ORDINARY_COMMON_IDENTITIES_V3
    {
        return Err(DirectOrdinaryRegisterErrorV3::InvalidLength);
    }
    let encoded_config = context.config.encode();
    let encoded_config_id: [u8; 32] = Sha256::digest(encoded_config).into();
    if context.config_content_id == [0; 32] || context.config_content_id != encoded_config_id {
        return Err(DirectOrdinaryRegisterErrorV3::ConfigContentMismatch);
    }
    let identities = [
        context.parent_request_digest,
        context.seller_rent_beneficiary,
        context.config.fee_recipient(),
        context.market,
        context.seller_native_signer,
        context.buyer_native_signer,
        request.seller.maker,
        request.buyer.maker,
        request.seller.intent.market,
        request.buyer.intent.market,
        context.release_set,
        context.product_record_digest,
        context.semantic_basis,
        context.linked_basis_record_digest,
        context.trading_program,
        context.trading_program,
        context.trading_program,
        context.realm,
        context.mint,
        context.token_program,
        context.buyer_rent_beneficiary,
        context.seller_maker_root,
        context.buyer_maker_root,
        context.system_program,
        context.custody_authority,
        context.seller_rent_beneficiary_observation,
        context.buyer_rent_beneficiary_observation,
        context.fee_token_account,
        request.seller.intent.collateral_account,
        request.buyer.intent.collateral_account,
        context.seller_token_account,
        context.buyer_token_account,
    ];
    if identities
        .iter()
        .enumerate()
        .any(|(index, value)| *value == [0; 32] && index != 25 && index != 26)
    {
        return Err(DirectOrdinaryRegisterErrorV3::ZeroIdentity);
    }
    scalar_scratch.fill(0);
    for (index, value) in [
        u64::from(context.root_phase),
        context.slot,
        request.seller.intent.valid_from,
        request.seller.intent.valid_through,
        request.buyer.intent.valid_from,
        request.buyer.intent.valid_through,
        u64::from(request.seller.intent.side),
        u64::from(request.buyer.intent.side),
        request.seller.intent.generation,
        request.buyer.intent.generation,
        context.generation,
        u64::from(request.seller.intent.outcome),
        u64::from(request.buyer.intent.outcome),
        u64::from(context.outcome_count),
        u64::from(request.seller.intent.lifecycle),
        request.seller.intent.maximum_fill,
        u64::from(request.buyer.intent.lifecycle),
        request.buyer.intent.maximum_fill,
        request.seller.intent.nonce,
        request.buyer.intent.nonce,
        context.seller_next_nonce,
        context.buyer_next_nonce,
        request.seller.intent.limit_price,
        request.execution_price,
        request.buyer.intent.limit_price,
        context.config.price_scale(),
        u64::from(request.seller.intent.fee_basis_points),
        u64::from(request.buyer.intent.fee_basis_points),
        u64::from(context.config.fee_basis_points()),
        request.fill,
        context.claims_market_revision,
        context.seller_position_revision,
        context.buyer_position_revision,
        context.custody_revision,
        context.root_open_maker_count,
        0,
        u64::from(context.seller_created),
        u64::from(context.seller_bump_observation),
    ]
    .into_iter()
    .enumerate()
    {
        *scalar_scratch
            .get_mut(index)
            .ok_or(DirectOrdinaryRegisterErrorV3::InvalidLength)? = value;
    }
    for (index, value) in [
        (
            SCALAR_BUYER_RENT_PRINCIPAL_OBSERVATION_V3,
            context.buyer_rent_principal_observation,
        ),
        (SCALAR_BUYER_RENT_PRINCIPAL_V3, context.buyer_rent_principal),
        (SCALAR_SELLER_BUMP_V3, u64::from(context.seller_bump)),
        (
            SCALAR_SELLER_RENT_PRINCIPAL_OBSERVATION_V3,
            context.seller_rent_principal_observation,
        ),
        (
            SCALAR_SELLER_RENT_PRINCIPAL_V3,
            context.seller_rent_principal,
        ),
        (SCALAR_BUYER_CREATED_V3, u64::from(context.buyer_created)),
        (
            SCALAR_BUYER_BUMP_OBSERVATION_V3,
            u64::from(context.buyer_bump_observation),
        ),
        (SCALAR_BUYER_BUMP_V3, u64::from(context.buyer_bump)),
    ] {
        *scalar_scratch
            .get_mut(index)
            .ok_or(DirectOrdinaryRegisterErrorV3::InvalidLength)? = value;
    }
    let mut item = 0_usize;
    while item < tail_count {
        let offset = item
            .checked_mul(usize::from(DIRECT_ORDINARY_ITEM_SCALAR_STRIDE_V3))
            .and_then(|value| DIRECT_ORDINARY_COMMON_SCALARS_V3.checked_add(value))
            .ok_or(DirectOrdinaryRegisterErrorV3::InvalidLength)?;
        *scalar_scratch
            .get_mut(offset + usize::from(ITEM_SCALAR_INDEX_V3))
            .ok_or(DirectOrdinaryRegisterErrorV3::InvalidLength)? =
            u64::try_from(item).map_err(|_| DirectOrdinaryRegisterErrorV3::InvalidLength)?;
        item = item
            .checked_add(1)
            .ok_or(DirectOrdinaryRegisterErrorV3::InvalidLength)?;
    }
    identity_scratch.copy_from_slice(&identities);
    scalar_output.copy_from_slice(scalar_scratch);
    identity_output.copy_from_slice(identity_scratch);
    Ok(())
}

/// Emit the exact ordinary TransitionVM V3 program atomically.
pub fn encode_direct_ordinary_transition_v3(scratch: &mut [u8], output: &mut [u8]) -> Result<()> {
    encode_program_atomic(
        ProgramGeometryV3 {
            common_scalars: u16::try_from(DIRECT_ORDINARY_COMMON_SCALARS_V3)
                .map_err(|_| DirectOrdinaryRegisterErrorV3::TransitionProgram)?,
            item_scalar_stride: DIRECT_ORDINARY_ITEM_SCALAR_STRIDE_V3,
            common_identities: u16::try_from(DIRECT_ORDINARY_COMMON_IDENTITIES_V3)
                .map_err(|_| DirectOrdinaryRegisterErrorV3::TransitionProgram)?,
            item_identity_stride: DIRECT_ORDINARY_ITEM_IDENTITY_STRIDE_V3,
        },
        &DIRECT_ORDINARY_PRELUDE_V3,
        &DIRECT_ORDINARY_ITEM_V3,
        &[],
        scratch,
        output,
    )
    .map_err(|_| DirectOrdinaryRegisterErrorV3::TransitionProgram)
}

const fn scalar(index: usize) -> ScalarRegisterV3 {
    assert!(index <= u16::MAX as usize);
    #[allow(clippy::cast_possible_truncation)]
    ScalarRegisterV3::common(index as u16)
}

const fn identity(index: usize) -> IdentityRegisterV3 {
    assert!(index <= u16::MAX as usize);
    #[allow(clippy::cast_possible_truncation)]
    IdentityRegisterV3::common(index as u16)
}

const fn item_scalar(index: u16) -> ScalarRegisterV3 {
    ScalarRegisterV3::item(index)
}

const DIRECT_ORDINARY_PRELUDE_V3: [InstructionV3; DIRECT_ORDINARY_PRELUDE_INSTRUCTIONS_V3] = [
    InstructionV3::load_const(scalar(SCALAR_ZERO_V3), 0),
    InstructionV3::load_const(scalar(SCALAR_ONE_V3), 1),
    InstructionV3::load_const(
        scalar(SCALAR_FEE_DENOMINATOR_V3),
        DIRECT_FEE_DENOMINATOR_V1 as u64,
    ),
    InstructionV3::scalar_eq(scalar(SCALAR_ROOT_PHASE_V3), scalar(SCALAR_ZERO_V3)),
    InstructionV3::nonzero(scalar(SCALAR_FILL_V3)),
    InstructionV3::scalar_le(scalar(SCALAR_SELLER_VALID_FROM_V3), scalar(SCALAR_SLOT_V3)),
    InstructionV3::scalar_le(
        scalar(SCALAR_SLOT_V3),
        scalar(SCALAR_SELLER_VALID_THROUGH_V3),
    ),
    InstructionV3::scalar_le(scalar(SCALAR_BUYER_VALID_FROM_V3), scalar(SCALAR_SLOT_V3)),
    InstructionV3::scalar_le(
        scalar(SCALAR_SLOT_V3),
        scalar(SCALAR_BUYER_VALID_THROUGH_V3),
    ),
    InstructionV3::scalar_eq(scalar(SCALAR_SELLER_SIDE_V3), scalar(SCALAR_ZERO_V3)),
    InstructionV3::scalar_eq(scalar(SCALAR_BUYER_SIDE_V3), scalar(SCALAR_ONE_V3)),
    InstructionV3::identity_eq(
        identity(IDENTITY_SELLER_INTENT_MARKET_V3),
        identity(IDENTITY_BUYER_INTENT_MARKET_V3),
    ),
    InstructionV3::identity_eq(
        identity(IDENTITY_SELLER_INTENT_MARKET_V3),
        identity(IDENTITY_MARKET_V3),
    ),
    InstructionV3::scalar_eq(
        scalar(SCALAR_SELLER_GENERATION_V3),
        scalar(SCALAR_BUYER_GENERATION_V3),
    ),
    InstructionV3::scalar_eq(
        scalar(SCALAR_SELLER_GENERATION_V3),
        scalar(SCALAR_MARKET_GENERATION_V3),
    ),
    InstructionV3::scalar_eq(
        scalar(SCALAR_SELLER_OUTCOME_V3),
        scalar(SCALAR_BUYER_OUTCOME_V3),
    ),
    InstructionV3::identity_eq(
        identity(IDENTITY_SELLER_NATIVE_SIGNER_V3),
        identity(IDENTITY_SELLER_REQUEST_MAKER_V3),
    ),
    InstructionV3::identity_eq(
        identity(IDENTITY_BUYER_NATIVE_SIGNER_V3),
        identity(IDENTITY_BUYER_REQUEST_MAKER_V3),
    ),
    InstructionV3::identity_ne(
        identity(IDENTITY_SELLER_REQUEST_MAKER_V3),
        identity(IDENTITY_BUYER_REQUEST_MAKER_V3),
    ),
    InstructionV3::identity_eq(
        identity(IDENTITY_SELLER_COLLATERAL_REQUEST_V3),
        identity(IDENTITY_SELLER_TOKEN_ACCOUNT_V3),
    ),
    InstructionV3::identity_eq(
        identity(IDENTITY_BUYER_COLLATERAL_REQUEST_V3),
        identity(IDENTITY_BUYER_TOKEN_ACCOUNT_V3),
    ),
    InstructionV3::scalar_lt(
        scalar(SCALAR_SELLER_OUTCOME_V3),
        scalar(SCALAR_OUTCOME_COUNT_V3),
    ),
    InstructionV3::nonzero(scalar(SCALAR_PRICE_SCALE_V3)),
    InstructionV3::lifecycle_accepts(
        scalar(SCALAR_SELLER_LIFECYCLE_V3),
        scalar(SCALAR_SELLER_MAXIMUM_V3),
        scalar(SCALAR_FILL_V3),
    ),
    InstructionV3::lifecycle_accepts(
        scalar(SCALAR_BUYER_LIFECYCLE_V3),
        scalar(SCALAR_BUYER_MAXIMUM_V3),
        scalar(SCALAR_FILL_V3),
    ),
    InstructionV3::scalar_eq(
        scalar(SCALAR_SELLER_NONCE_V3),
        scalar(SCALAR_SELLER_NEXT_NONCE_V3),
    ),
    InstructionV3::scalar_eq(
        scalar(SCALAR_BUYER_NONCE_V3),
        scalar(SCALAR_BUYER_NEXT_NONCE_V3),
    ),
    InstructionV3::increment_into(
        scalar(SCALAR_SELLER_NEXT_NONCE_V3),
        scalar(SCALAR_SELLER_NONCE_AFTER_V3),
    ),
    InstructionV3::increment_into(
        scalar(SCALAR_BUYER_NEXT_NONCE_V3),
        scalar(SCALAR_BUYER_NONCE_AFTER_V3),
    ),
    InstructionV3::scalar_le(
        scalar(SCALAR_SELLER_LIMIT_V3),
        scalar(SCALAR_EXECUTION_PRICE_V3),
    ),
    InstructionV3::scalar_le(
        scalar(SCALAR_EXECUTION_PRICE_V3),
        scalar(SCALAR_BUYER_LIMIT_V3),
    ),
    InstructionV3::scalar_le(
        scalar(SCALAR_EXECUTION_PRICE_V3),
        scalar(SCALAR_PRICE_SCALE_V3),
    ),
    InstructionV3::scalar_eq(
        scalar(SCALAR_SELLER_FEE_BPS_V3),
        scalar(SCALAR_POLICY_FEE_BPS_V3),
    ),
    InstructionV3::scalar_eq(
        scalar(SCALAR_BUYER_FEE_BPS_V3),
        scalar(SCALAR_POLICY_FEE_BPS_V3),
    ),
    InstructionV3::mul_div_exact(
        scalar(SCALAR_FILL_V3),
        scalar(SCALAR_EXECUTION_PRICE_V3),
        scalar(SCALAR_PRICE_SCALE_V3),
        scalar(SCALAR_GROSS_V3),
    ),
    InstructionV3::mul_div_floor(
        scalar(SCALAR_GROSS_V3),
        scalar(SCALAR_POLICY_FEE_BPS_V3),
        scalar(SCALAR_FEE_DENOMINATOR_V3),
        scalar(SCALAR_FEE_V3),
    ),
    InstructionV3::sub_into(
        scalar(SCALAR_GROSS_V3),
        scalar(SCALAR_FEE_V3),
        scalar(SCALAR_SELLER_NET_V3),
    ),
    InstructionV3::checked_add_into(
        scalar(SCALAR_GROSS_V3),
        scalar(SCALAR_FEE_V3),
        scalar(SCALAR_BUYER_DEBIT_V3),
    ),
    InstructionV3::checked_add_into(
        scalar(SCALAR_FEE_V3),
        scalar(SCALAR_FEE_V3),
        scalar(SCALAR_COMBINED_FEE_V3),
    ),
    InstructionV3::checked_add_into(
        scalar(SCALAR_SELLER_NET_V3),
        scalar(SCALAR_COMBINED_FEE_V3),
        scalar(SCALAR_SELLER_TERMINAL_ROUTE_ENABLED_V3),
    ),
    InstructionV3::scalar_eq(
        scalar(SCALAR_SELLER_TERMINAL_ROUTE_ENABLED_V3),
        scalar(SCALAR_BUYER_DEBIT_V3),
    ),
    InstructionV3::scalar_le(scalar(SCALAR_SELLER_CREATED_V3), scalar(SCALAR_ONE_V3)),
    InstructionV3::scalar_le(scalar(SCALAR_BUYER_CREATED_V3), scalar(SCALAR_ONE_V3)),
    InstructionV3::checked_add_into(
        scalar(SCALAR_ROOT_OPEN_COUNT_V3),
        scalar(SCALAR_SELLER_CREATED_V3),
        scalar(SCALAR_ROOT_OPEN_COUNT_AFTER_V3),
    ),
    InstructionV3::checked_add_into(
        scalar(SCALAR_ROOT_OPEN_COUNT_AFTER_V3),
        scalar(SCALAR_BUYER_CREATED_V3),
        scalar(SCALAR_ROOT_OPEN_COUNT_AFTER_V3),
    ),
    InstructionV3::identity_eq(
        identity(IDENTITY_SELLER_STATE_OWNER_V3),
        identity(IDENTITY_TRADING_PROGRAM_V3),
    ),
    InstructionV3::identity_eq(
        identity(IDENTITY_BUYER_STATE_OWNER_V3),
        identity(IDENTITY_TRADING_PROGRAM_V3),
    ),
    InstructionV3::load_const(scalar(SCALAR_SELLER_INTERMEDIATE_ROUTE_ENABLED_V3), 1),
    InstructionV3::select_zero(
        scalar(SCALAR_SELLER_NET_V3),
        scalar(SCALAR_ZERO_V3),
        scalar(SCALAR_SELLER_INTERMEDIATE_ROUTE_ENABLED_V3),
    ),
    InstructionV3::load_const(scalar(SCALAR_FEE_NONZERO_V3), 1),
    InstructionV3::select_zero(
        scalar(SCALAR_COMBINED_FEE_V3),
        scalar(SCALAR_ZERO_V3),
        scalar(SCALAR_FEE_NONZERO_V3),
    ),
    InstructionV3::load_const(scalar(SCALAR_SELLER_TERMINAL_ROUTE_ENABLED_V3), 0),
    InstructionV3::select_zero(
        scalar(SCALAR_COMBINED_FEE_V3),
        scalar(SCALAR_SELLER_INTERMEDIATE_ROUTE_ENABLED_V3),
        scalar(SCALAR_SELLER_TERMINAL_ROUTE_ENABLED_V3),
    ),
    InstructionV3::checked_add_into(
        scalar(SCALAR_FEE_NONZERO_V3),
        scalar(SCALAR_ZERO_V3),
        scalar(SCALAR_SELLER_INTERMEDIATE_ROUTE_ENABLED_V3),
    ),
    InstructionV3::select_zero(
        scalar(SCALAR_SELLER_NET_V3),
        scalar(SCALAR_ZERO_V3),
        scalar(SCALAR_SELLER_INTERMEDIATE_ROUTE_ENABLED_V3),
    ),
    InstructionV3::load_const(scalar(SCALAR_FEE_SOLE_ROUTE_ENABLED_V3), 0),
    InstructionV3::select_zero(
        scalar(SCALAR_SELLER_NET_V3),
        scalar(SCALAR_FEE_NONZERO_V3),
        scalar(SCALAR_FEE_SOLE_ROUTE_ENABLED_V3),
    ),
    InstructionV3::checked_add_into(
        scalar(SCALAR_SELLER_TERMINAL_ROUTE_ENABLED_V3),
        scalar(SCALAR_SELLER_INTERMEDIATE_ROUTE_ENABLED_V3),
        scalar(SCALAR_CUSTODY_AFTER_SELLER_V3),
    ),
    InstructionV3::checked_add_into(
        scalar(SCALAR_CUSTODY_REVISION_V3),
        scalar(SCALAR_CUSTODY_AFTER_SELLER_V3),
        scalar(SCALAR_CUSTODY_AFTER_SELLER_V3),
    ),
    InstructionV3::checked_add_into(
        scalar(SCALAR_CUSTODY_AFTER_SELLER_V3),
        scalar(SCALAR_SELLER_INTERMEDIATE_ROUTE_ENABLED_V3),
        scalar(SCALAR_CUSTODY_AFTER_FEE_V3),
    ),
    InstructionV3::checked_add_into(
        scalar(SCALAR_CUSTODY_AFTER_FEE_V3),
        scalar(SCALAR_FEE_SOLE_ROUTE_ENABLED_V3),
        scalar(SCALAR_CUSTODY_AFTER_FEE_V3),
    ),
    InstructionV3::checked_add_into(
        scalar(SCALAR_FILL_V3),
        scalar(SCALAR_ZERO_V3),
        scalar(SCALAR_CLAIM_TRANSFER_V3),
    ),
    InstructionV3::load_const(
        scalar(SCALAR_MAKER_VERSION_V3),
        crate::successor::DirectMakerReplayLayoutV1::ABI_VERSION as u64,
    ),
    InstructionV3::load_const(
        scalar(SCALAR_MAKER_MAGIC_V3),
        crate::successor::DirectMakerReplayLayoutV1::MAGIC_WORD,
    ),
];

const DIRECT_ORDINARY_ITEM_V3: [InstructionV3; DIRECT_ORDINARY_ITEM_INSTRUCTIONS_V3] = [
    InstructionV3::load_const(item_scalar(ITEM_SCALAR_CLAIM_QUANTITY_V3), 0),
    InstructionV3::select_eq(
        item_scalar(ITEM_SCALAR_INDEX_V3),
        scalar(SCALAR_SELLER_OUTCOME_V3),
        scalar(SCALAR_CLAIM_TRANSFER_V3),
        item_scalar(ITEM_SCALAR_CLAIM_QUANTITY_V3),
    ),
];

#[cfg(test)]
#[allow(clippy::indexing_slicing)]
mod tests {
    extern crate std;

    use dclutch_transition_vm::v3::{
        ProgramV3, RegisterInput, RegisterOutput, execute_fold_atomic,
    };

    use super::*;
    use crate::{
        execution_v3::{DirectInlineOrdinaryRequestV3, DirectSignedParticipantV3},
        intent_v2::CompactIntentV2,
        successor::{
            AuthenticatedCompactIntentV2, DirectRootStateV1, InlineExecutionV2,
            InlineOrdinaryInputV2, InlineParticipantV2, MakerReplayFirstUseV1,
            MakerReplayObservationV1, MakerReplayVacancyV1, settle_inline_ordinary_v2,
        },
    };

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn request() -> DirectInlineOrdinaryRequestV3 {
        DirectInlineOrdinaryRequestV3 {
            seller: DirectSignedParticipantV3 {
                maker: id(2),
                intent: CompactIntentV2 {
                    side: 0,
                    lifecycle: 1,
                    outcome: 2,
                    market: id(1),
                    generation: 7,
                    nonce: 4,
                    valid_from: 10,
                    valid_through: 30,
                    maximum_fill: 25,
                    limit_price: 40,
                    fee_basis_points: 1_000,
                    collateral_account: id(20),
                },
            },
            buyer: DirectSignedParticipantV3 {
                maker: id(3),
                intent: CompactIntentV2 {
                    side: 1,
                    lifecycle: 1,
                    outcome: 2,
                    market: id(1),
                    generation: 7,
                    nonce: 9,
                    valid_from: 5,
                    valid_through: 40,
                    maximum_fill: 30,
                    limit_price: 60,
                    fee_basis_points: 1_000,
                    collateral_account: id(21),
                },
            },
            fill: 20,
            execution_price: 50,
        }
    }

    fn context(config: DirectExecutionConfigV1) -> DirectOrdinaryAuthenticatedContextV3 {
        DirectOrdinaryAuthenticatedContextV3 {
            parent_request_digest: id(30),
            config_content_id: Sha256::digest(config.encode()).into(),
            config,
            market: id(1),
            generation: 7,
            outcome_count: 4,
            slot: 20,
            root_phase: 0,
            seller_next_nonce: 4,
            buyer_next_nonce: 9,
            root_open_maker_count: 2,
            seller_created: false,
            seller_bump_observation: 1,
            seller_bump: 1,
            seller_rent_principal_observation: 100,
            seller_rent_principal: 100,
            buyer_created: false,
            buyer_bump_observation: 2,
            buyer_bump: 2,
            buyer_rent_principal_observation: 100,
            buyer_rent_principal: 100,
            claims_market_revision: 11,
            seller_position_revision: 12,
            buyer_position_revision: 13,
            custody_revision: 14,
            release_set: id(31),
            product_record_digest: id(32),
            semantic_basis: id(33),
            linked_basis_record_digest: id(34),
            trading_program: id(35),
            realm: id(38),
            mint: id(39),
            token_program: id(40),
            seller_maker_root: id(42),
            buyer_maker_root: id(43),
            system_program: id(44),
            custody_authority: id(45),
            seller_rent_beneficiary: id(71),
            seller_rent_beneficiary_observation: id(71),
            buyer_rent_beneficiary: id(72),
            buyer_rent_beneficiary_observation: id(72),
            fee_token_account: id(48),
            seller_token_account: id(20),
            buyer_token_account: id(21),
            seller_native_signer: id(2),
            buyer_native_signer: id(3),
        }
    }

    fn execute(
        request: DirectInlineOrdinaryRequestV3,
        context: DirectOrdinaryAuthenticatedContextV3,
        output: &mut [u64],
    ) -> core::result::Result<(), dclutch_transition_vm::v3::Error> {
        let tail_count = context.outcome_count;
        let scalar_width = DIRECT_ORDINARY_COMMON_SCALARS_V3
            + usize::try_from(tail_count).expect("tail count")
                * usize::from(DIRECT_ORDINARY_ITEM_SCALAR_STRIDE_V3);
        let mut scalar_input = std::vec![0_u64; scalar_width];
        let mut identity_input = [[0_u8; 32]; DIRECT_ORDINARY_COMMON_IDENTITIES_V3];
        let mut projected_scalars = std::vec![0_u64; scalar_width];
        let mut projected_identities = [[0_u8; 32]; DIRECT_ORDINARY_COMMON_IDENTITIES_V3];
        project_direct_ordinary_registers_v3(
            request,
            context,
            &mut scalar_input,
            &mut identity_input,
            &mut projected_scalars,
            &mut projected_identities,
        )
        .expect("register projection");
        let mut program_scratch = [0_u8; DIRECT_ORDINARY_TRANSITION_BYTES_V3];
        let mut program_bytes = [0_u8; DIRECT_ORDINARY_TRANSITION_BYTES_V3];
        encode_direct_ordinary_transition_v3(&mut program_scratch, &mut program_bytes)
            .expect("program emission");
        let program = ProgramV3::decode(&program_bytes).expect("program decode");
        let mut scratch_scalars = std::vec![0_u64; scalar_width];
        let mut scratch_identities = [[0_u8; 32]; DIRECT_ORDINARY_COMMON_IDENTITIES_V3];
        let mut output_identities = [[9_u8; 32]; DIRECT_ORDINARY_COMMON_IDENTITIES_V3];
        execute_fold_atomic(
            program,
            tail_count,
            RegisterInput {
                scalars: &projected_scalars,
                identities: &projected_identities,
            },
            RegisterOutput {
                scalars: &mut scratch_scalars,
                identities: &mut scratch_identities,
            },
            RegisterOutput {
                scalars: output,
                identities: &mut output_identities,
            },
        )
    }

    #[test]
    fn exact_program_admits_price_improved_ioc_and_conserves_two_fee_routes() {
        let config = DirectExecutionConfigV1::new(100, 1_000, id(60)).expect("config");
        let scalar_width = DIRECT_ORDINARY_COMMON_SCALARS_V3
            + 4 * usize::from(DIRECT_ORDINARY_ITEM_SCALAR_STRIDE_V3);
        let mut output = std::vec![99_u64; scalar_width];
        execute(request(), context(config), &mut output).expect("ordinary transition");
        assert_eq!(DIRECT_ORDINARY_TRANSITION_BYTES_V3, 1_616);
        assert_eq!(output[SCALAR_SELLER_NONCE_AFTER_V3], 5);
        assert_eq!(output[SCALAR_BUYER_NONCE_AFTER_V3], 10);
        assert_eq!(output[SCALAR_GROSS_V3], 10);
        assert_eq!(output[SCALAR_FEE_V3], 1);
        assert_eq!(output[SCALAR_SELLER_NET_V3], 9);
        assert_eq!(output[SCALAR_BUYER_DEBIT_V3], 11);
        assert_eq!(output[SCALAR_COMBINED_FEE_V3], 2);
        assert_eq!(
            output[SCALAR_SELLER_NET_V3] + output[SCALAR_COMBINED_FEE_V3],
            output[SCALAR_BUYER_DEBIT_V3]
        );
        assert_eq!(output[SCALAR_ROOT_OPEN_COUNT_AFTER_V3], 2);
        assert_eq!(output[SCALAR_SELLER_TERMINAL_ROUTE_ENABLED_V3], 0);
        assert_eq!(output[SCALAR_SELLER_INTERMEDIATE_ROUTE_ENABLED_V3], 1);
        assert_eq!(output[SCALAR_FEE_NONZERO_V3], 1);
        assert_eq!(output[SCALAR_FEE_SOLE_ROUTE_ENABLED_V3], 0);
        assert_eq!(
            output[SCALAR_MAKER_MAGIC_V3],
            crate::successor::DirectMakerReplayLayoutV1::MAGIC_WORD
        );
        assert_eq!(
            output[SCALAR_MAKER_VERSION_V3],
            u64::from(crate::successor::DirectMakerReplayLayoutV1::ABI_VERSION)
        );
        assert_eq!(output[SCALAR_CUSTODY_AFTER_SELLER_V3], 15);
        assert_eq!(output[SCALAR_CUSTODY_AFTER_FEE_V3], 16);
        assert_eq!(output[SCALAR_CLAIM_TRANSFER_V3], 20);
        for item in 0..4 {
            let base = DIRECT_ORDINARY_COMMON_SCALARS_V3
                + item * usize::from(DIRECT_ORDINARY_ITEM_SCALAR_STRIDE_V3);
            assert_eq!(
                output[base + usize::from(ITEM_SCALAR_INDEX_V3)],
                item as u64
            );
            assert_eq!(
                output[base + usize::from(ITEM_SCALAR_CLAIM_QUANTITY_V3)],
                if item == 2 { 20 } else { 0 }
            );
        }
    }

    #[test]
    fn signer_config_fee_and_late_exact_quote_substitutions_refuse_atomically() {
        let config = DirectExecutionConfigV1::new(100, 1_000, id(60)).expect("config");
        let mut wrong_signer = context(config);
        wrong_signer.seller_native_signer = id(90);
        let mut wrong_fee = request();
        wrong_fee.buyer.intent.fee_basis_points = 999;
        let mut inexact = request();
        inexact.fill = 19;
        let cases = [
            (request(), wrong_signer),
            (wrong_fee, context(config)),
            (inexact, context(config)),
        ];
        for (request, context) in cases {
            let mut output = std::vec![
                0x55_u64;
                DIRECT_ORDINARY_COMMON_SCALARS_V3
                    + 4 * usize::from(DIRECT_ORDINARY_ITEM_SCALAR_STRIDE_V3)
            ];
            let before = output.clone();
            assert!(execute(request, context, &mut output).is_err());
            assert_eq!(output, before);
        }

        let mut hostile_token = context(config);
        hostile_token.buyer_token_account = id(90);
        let mut output = std::vec![
            0x55_u64;
            DIRECT_ORDINARY_COMMON_SCALARS_V3
                + 4 * usize::from(DIRECT_ORDINARY_ITEM_SCALAR_STRIDE_V3)
        ];
        let before = output.clone();
        assert!(execute(request(), hostile_token, &mut output).is_err());
        assert_eq!(output, before);

        let mut wrong_content = context(config);
        wrong_content.config_content_id[0] ^= 1;
        let mut scalar_scratch = std::vec![
            0_u64;
            DIRECT_ORDINARY_COMMON_SCALARS_V3
                + 4 * usize::from(DIRECT_ORDINARY_ITEM_SCALAR_STRIDE_V3)
        ];
        let mut identity_scratch = [[0_u8; 32]; DIRECT_ORDINARY_COMMON_IDENTITIES_V3];
        let mut scalar_output = std::vec![
            0x77_u64;
            DIRECT_ORDINARY_COMMON_SCALARS_V3
                + 4 * usize::from(DIRECT_ORDINARY_ITEM_SCALAR_STRIDE_V3)
        ];
        let mut identity_output = [[0x77_u8; 32]; DIRECT_ORDINARY_COMMON_IDENTITIES_V3];
        let scalar_before = scalar_output.clone();
        let identity_before = identity_output;
        assert_eq!(
            project_direct_ordinary_registers_v3(
                request(),
                wrong_content,
                &mut scalar_scratch,
                &mut identity_scratch,
                &mut scalar_output,
                &mut identity_output,
            ),
            Err(DirectOrdinaryRegisterErrorV3::ConfigContentMismatch)
        );
        assert_eq!(scalar_output, scalar_before);
        assert_eq!(identity_output, identity_before);
    }

    #[test]
    fn transition_effects_equal_the_successor_semantic_owner() {
        let config = DirectExecutionConfigV1::new(100, 1_000, id(60)).expect("config");
        let mut request = request();
        request.seller.intent.nonce = 0;
        request.buyer.intent.nonce = 0;
        let participant = |value: DirectSignedParticipantV3, bump: u8| InlineParticipantV2 {
            authenticated: AuthenticatedCompactIntentV2::from_adjacent_ed25519(
                value.maker,
                value.intent,
            )
            .expect("authenticated intent"),
            maker_replay: MakerReplayObservationV1::Vacant(MakerReplayVacancyV1::new(bump, 7)),
            first_use: Some(MakerReplayFirstUseV1 {
                rent_owner: id(70 + bump),
                rent_principal: 100,
            }),
        };
        let semantic = settle_inline_ordinary_v2(InlineOrdinaryInputV2 {
            root: DirectRootStateV1::new(),
            seller: participant(request.seller, 1),
            buyer: participant(request.buyer, 2),
            execution: InlineExecutionV2 {
                config,
                outcome_count: 4,
                slot: 20,
                fill: request.fill,
                execution_price: request.execution_price,
            },
        })
        .expect("semantic owner accepts");
        let mut context = context(config);
        context.seller_next_nonce = 0;
        context.buyer_next_nonce = 0;
        context.root_open_maker_count = 0;
        context.seller_created = true;
        context.seller_bump_observation = 0;
        context.seller_rent_principal_observation = 0;
        context.seller_rent_beneficiary_observation = [0; 32];
        context.buyer_created = true;
        context.buyer_bump_observation = 0;
        context.buyer_rent_principal_observation = 0;
        context.buyer_rent_beneficiary_observation = [0; 32];
        let mut output = std::vec![
            99_u64;
            DIRECT_ORDINARY_COMMON_SCALARS_V3
                + 4 * usize::from(DIRECT_ORDINARY_ITEM_SCALAR_STRIDE_V3)
        ];
        execute(request, context, &mut output).expect("compiled transition accepts");
        assert_eq!(output[SCALAR_SELLER_NONCE_AFTER_V3], 1);
        assert_eq!(output[SCALAR_BUYER_NONCE_AFTER_V3], 1);
        assert_eq!(output[SCALAR_ROOT_OPEN_COUNT_AFTER_V3], 2);
        assert_eq!(output[SCALAR_GROSS_V3], semantic.effects.gross_collateral);
        assert_eq!(
            output[SCALAR_SELLER_NET_V3],
            semantic.effects.seller_net_collateral_credit
        );
        assert_eq!(
            output[SCALAR_BUYER_DEBIT_V3],
            semantic.effects.buyer_collateral_debit
        );
        assert_eq!(
            output[SCALAR_COMBINED_FEE_V3],
            semantic.effects.total_fee_transfer
        );
    }
}
