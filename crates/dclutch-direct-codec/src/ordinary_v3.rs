//! Exact register and TransitionVM contract for inline ordinary Direct V3.
//!
//! This is a successor bank, not an extension of the historical 41/4 map.
//! Native Ed25519 signer identities and request-carried maker identities occupy
//! distinct registers and the transition requires equality. The immutable
//! config is hostile-decoded elsewhere, but this projection additionally binds
//! its exact canonical bytes to the finalized content identity before placing
//! `price_scale`, `fee_basis_points`, and `fee_recipient` in the bank. Finalized
//! config records are immutable and therefore have no mutable revision field.
//!
//! The register schema, the admission program, and its exact encoded bytes are
//! authored in `formal/dclutch-semantics/DClutchSemantics/DirectOrdinaryV3.lean`
//! and emitted into `generated_ordinary_v3.rs`. This module projects an
//! authenticated request into that bank and republishes the emitted program
//! after hostile decoding; it holds no admission relation of its own.

use dclutch_transition_vm::v3::ProgramV3;
use sha2::{Digest, Sha256};

pub use crate::generated_ordinary_v3::*;
use crate::{execution_v3::DirectInlineOrdinaryRequestV3, successor::DirectExecutionConfigV1};

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

/// Republish the exact Lean-emitted ordinary TransitionVM V3 program.
///
/// The program is authored in Lean and emitted into `generated_ordinary_v3.rs`.
/// This function hostile-decodes those bytes in caller-owned scratch and copies
/// them to `output` only after the decoder accepts the complete result, so a
/// caller never observes a partially written or undecodable program.
pub fn encode_direct_ordinary_transition_v3(scratch: &mut [u8], output: &mut [u8]) -> Result<()> {
    if scratch.len() != DIRECT_ORDINARY_TRANSITION_BYTES_V3
        || output.len() != DIRECT_ORDINARY_TRANSITION_BYTES_V3
    {
        return Err(DirectOrdinaryRegisterErrorV3::TransitionProgram);
    }
    scratch.copy_from_slice(&DIRECT_ORDINARY_TRANSITION_V3);
    ProgramV3::decode(scratch).map_err(|_| DirectOrdinaryRegisterErrorV3::TransitionProgram)?;
    output.copy_from_slice(scratch);
    Ok(())
}

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

    /// The emitted width and geometry are literals from Lean. This restates the
    /// arithmetic the Rust side used to carry, so a regenerated program that
    /// disagrees with its own header or section counts cannot land silently.
    #[test]
    fn the_emitted_program_carries_its_own_derived_width_and_geometry() {
        assert_eq!(
            DIRECT_ORDINARY_TRANSITION_INSTRUCTIONS_V3,
            DIRECT_ORDINARY_PRELUDE_INSTRUCTIONS_V3
                + DIRECT_ORDINARY_ITEM_INSTRUCTIONS_V3
                + DIRECT_ORDINARY_EPILOGUE_INSTRUCTIONS_V3
        );
        assert_eq!(
            DIRECT_ORDINARY_TRANSITION_BYTES_V3,
            dclutch_transition_vm::v3::HEADER_BYTES
                + DIRECT_ORDINARY_TRANSITION_INSTRUCTIONS_V3
                    * dclutch_transition_vm::v3::INSTRUCTION_BYTES
        );
        let program = ProgramV3::decode(&DIRECT_ORDINARY_TRANSITION_V3).expect("program decode");
        assert_eq!(
            usize::from(program.common_scalar_count()),
            DIRECT_ORDINARY_COMMON_SCALARS_V3
        );
        assert_eq!(
            usize::from(program.common_identity_count()),
            DIRECT_ORDINARY_COMMON_IDENTITIES_V3
        );
        assert_eq!(
            program.item_scalar_stride(),
            DIRECT_ORDINARY_ITEM_SCALAR_STRIDE_V3
        );
        assert_eq!(
            program.item_identity_stride(),
            DIRECT_ORDINARY_ITEM_IDENTITY_STRIDE_V3
        );
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
