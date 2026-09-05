//! Request, transition, and interpreted strategy for registered ordinary fills.
//!
//! The matcher request is intentionally unsigned. Authority comes from two
//! previously authenticated GTC records and their maker replay coordinates.
//! The transition re-proves both reservations, charges cumulative-difference
//! fees, derives partial/terminal candidates, and exposes only checked child
//! effect quantities.
//!
//! The register schema, the admission program, and its exact encoded bytes are
//! authored in
//! `formal/dclutch-semantics/DClutchSemantics/DirectRegisteredFillV4.lean` and
//! emitted into `generated_registered_fill_v4.rs`. This module republishes the
//! emitted program after hostile decoding; it holds no admission relation of
//! its own. The unsigned RequestProfileV1 below is a projection, not an
//! admission relation: every value it places in the bank is re-proved by the
//! authored program.

#[cfg(not(target_os = "solana"))]
use dclutch_core_contract::ContentId;
#[cfg(not(target_os = "solana"))]
use dclutch_market::execution_strategy::v2::{
    ACCELERATOR_ACK_SCHEMA_ID_V2, ACCELERATOR_REQUEST_SCHEMA_ID_V2,
    EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2, EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2,
    EXECUTION_STRATEGY_PROGRAM_BYTES_V2, ExecutionStrategyProgramV2, StrategyDispositionV2,
};
#[cfg(not(target_os = "solana"))]
use dclutch_vm::request_profile::encode::{
    RequestCoordinateV1, RequestGeometryV1, RequestInstructionV1, ScalarRegisterV1,
    encode_request_profile_v1_atomic,
};
#[cfg(not(target_os = "solana"))]
use dclutch_vm::v3::ProgramV3;

#[cfg(not(target_os = "solana"))]
use crate::execution_v3::{
    DIRECT_EXECUTION_REQUEST_MAGIC_V3, DIRECT_EXECUTION_REQUEST_VERSION_V3,
    DIRECT_REGISTERED_FILL_REQUEST_BYTES_V3, DirectExecutionActionV3,
};
// Visible to the SBF target like its six sibling emissions: a gated register
// schema is a schema no program can address.
pub use crate::generated_registered_fill_v4::*;

#[cfg(not(target_os = "solana"))]
mod host {
    use super::*;

    const REQUEST_OPERATIONS: usize = 8;

    /// Exact unsigned RequestProfileV1 width.
    pub const DIRECT_REGISTERED_FILL_REQUEST_PROFILE_BYTES_V4: usize =
        dclutch_vm::request_profile::HEADER_BYTES
            + REQUEST_OPERATIONS * dclutch_vm::request_profile::OPERATION_BYTES;
    /// Exact interpreted ExecutionStrategy width.
    pub const DIRECT_REGISTERED_FILL_STRATEGY_BYTES_V4: usize = EXECUTION_STRATEGY_PROGRAM_BYTES_V2;

    /// Stable registered fill artifact refusal.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum DirectRegisteredFillArtifactErrorV4 {
        /// A register or byte coordinate did not fit.
        Coordinate,
        /// RequestProfile construction or hostile decoding refused.
        RequestProfile,
        /// Transition republication or hostile decoding refused.
        Transition,
        /// Interpreted strategy construction refused.
        Strategy,
    }

    /// Emit the exact unsigned registered-fill RequestProfileV1 atomically.
    pub fn encode_direct_registered_fill_request_profile_v4_atomic(
        scratch: &mut [u8],
        output: &mut [u8],
    ) -> Result<(), DirectRegisteredFillArtifactErrorV4> {
        if scratch.len() != DIRECT_REGISTERED_FILL_REQUEST_PROFILE_BYTES_V4
            || output.len() != DIRECT_REGISTERED_FILL_REQUEST_PROFILE_BYTES_V4
        {
            return Err(DirectRegisteredFillArtifactErrorV4::Coordinate);
        }
        let instructions = [
            RequestInstructionV1::require_u64(
                RequestCoordinateV1::fixed(0),
                u64::from_le_bytes(DIRECT_EXECUTION_REQUEST_MAGIC_V3),
            ),
            RequestInstructionV1::require_u16(
                RequestCoordinateV1::fixed(8),
                DIRECT_EXECUTION_REQUEST_VERSION_V3,
            ),
            RequestInstructionV1::require_zero(RequestCoordinateV1::fixed(10), 2),
            RequestInstructionV1::require_u32(
                RequestCoordinateV1::fixed(12),
                DirectExecutionActionV3::FillRegisteredOrdinary as u32,
            ),
            RequestInstructionV1::require_u32(RequestCoordinateV1::fixed(16), 16),
            RequestInstructionV1::require_zero(RequestCoordinateV1::fixed(20), 12),
            RequestInstructionV1::project_u64(
                RequestCoordinateV1::fixed(32),
                scalar_request(FILL_SCALAR_QUANTITY_V4)?,
            ),
            RequestInstructionV1::project_u64(
                RequestCoordinateV1::fixed(40),
                scalar_request(FILL_SCALAR_EXECUTION_PRICE_V4)?,
            ),
        ];
        encode_request_profile_v1_atomic(
            RequestGeometryV1::new(
                width32(DIRECT_REGISTERED_FILL_REQUEST_BYTES_V3)?,
                0,
                width16(DIRECT_REGISTERED_FILL_COMMON_SCALARS_V4)?,
                DIRECT_REGISTERED_FILL_ITEM_SCALAR_STRIDE_V4,
                width16(DIRECT_REGISTERED_FILL_COMMON_IDENTITIES_V4)?,
                DIRECT_REGISTERED_FILL_ITEM_IDENTITY_STRIDE_V4,
            ),
            &instructions,
            &[],
            scratch,
            output,
        )
        .map_err(|_| DirectRegisteredFillArtifactErrorV4::RequestProfile)
    }

    /// Republish the exact Lean-emitted registered-fill TransitionVM V3 program.
    ///
    /// The program is authored in
    /// `formal/dclutch-semantics/DClutchSemantics/DirectRegisteredFillV4.lean`
    /// and emitted into `generated_registered_fill_v4.rs`. This function
    /// hostile-decodes those bytes in caller-owned scratch and copies them to
    /// `output` only after the decoder accepts the complete result, so a caller
    /// never observes a partially written or undecodable program.
    pub fn encode_direct_registered_fill_transition_v4_atomic(
        scratch: &mut [u8],
        output: &mut [u8],
    ) -> Result<(), DirectRegisteredFillArtifactErrorV4> {
        if scratch.len() != DIRECT_REGISTERED_FILL_TRANSITION_BYTES_V4
            || output.len() != DIRECT_REGISTERED_FILL_TRANSITION_BYTES_V4
        {
            return Err(DirectRegisteredFillArtifactErrorV4::Coordinate);
        }
        scratch.copy_from_slice(&DIRECT_REGISTERED_FILL_TRANSITION_V4);
        ProgramV3::decode(scratch).map_err(|_| DirectRegisteredFillArtifactErrorV4::Transition)?;
        output.copy_from_slice(scratch);
        Ok(())
    }

    /// Construct the canonical interpreted strategy selecting `transition_id`.
    pub fn direct_registered_fill_strategy_v4(
        transition_id: [u8; 32],
    ) -> Result<[u8; DIRECT_REGISTERED_FILL_STRATEGY_BYTES_V4], DirectRegisteredFillArtifactErrorV4>
    {
        let transition = ContentId::new(transition_id)
            .map_err(|_| DirectRegisteredFillArtifactErrorV4::Strategy)?;
        let strategy = ExecutionStrategyProgramV2::new(
            StrategyDispositionV2::Interpreted,
            ContentId::new(dclutch_vm::v3::SCHEMA_RELEASE_ID)
                .map_err(|_| DirectRegisteredFillArtifactErrorV4::Strategy)?,
            transition,
            ContentId::new(EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2)
                .map_err(|_| DirectRegisteredFillArtifactErrorV4::Strategy)?,
            None,
            ContentId::new(EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2)
                .map_err(|_| DirectRegisteredFillArtifactErrorV4::Strategy)?,
            None,
            ContentId::new(ACCELERATOR_REQUEST_SCHEMA_ID_V2)
                .map_err(|_| DirectRegisteredFillArtifactErrorV4::Strategy)?,
            ContentId::new(ACCELERATOR_ACK_SCHEMA_ID_V2)
                .map_err(|_| DirectRegisteredFillArtifactErrorV4::Strategy)?,
        )
        .map_err(|_| DirectRegisteredFillArtifactErrorV4::Strategy)?;
        Ok(strategy.to_bytes())
    }

    fn scalar_request(
        value: usize,
    ) -> Result<ScalarRegisterV1, DirectRegisteredFillArtifactErrorV4> {
        Ok(ScalarRegisterV1::common(width16(value)?))
    }

    fn width16(value: usize) -> Result<u16, DirectRegisteredFillArtifactErrorV4> {
        u16::try_from(value).map_err(|_| DirectRegisteredFillArtifactErrorV4::Coordinate)
    }

    fn width32(value: usize) -> Result<u32, DirectRegisteredFillArtifactErrorV4> {
        u32::try_from(value).map_err(|_| DirectRegisteredFillArtifactErrorV4::Coordinate)
    }

    #[cfg(test)]
    mod tests {
        extern crate std;

        use dclutch_vm::request_profile::{
            ProjectionRegistersV1, RequestProfileV1, project_atomic,
        };
        use dclutch_vm::v3::{
            RegisterInput, RegisterKindV3, RegisterOutput, RegisterSpaceV3, RegisterWriteTargetV3,
            execute_fold_atomic,
        };

        use super::*;
        use crate::{
            execution_v3::{DirectExecutionQuantityV3, DirectExecutionRequestV3},
            registered_requests_v4::encode_direct_registered_execution_request_v3_atomic,
        };

        /// The emitted width and geometry are literals from Lean. This restates
        /// the arithmetic the Rust side used to carry, so a regenerated program
        /// that disagrees with its own header or section counts cannot land
        /// silently.
        #[test]
        fn the_emitted_program_carries_its_own_derived_width_and_geometry() {
            assert_eq!(
                DIRECT_REGISTERED_FILL_TRANSITION_INSTRUCTIONS_V4,
                DIRECT_REGISTERED_FILL_PRELUDE_INSTRUCTIONS_V4
                    + DIRECT_REGISTERED_FILL_ITEM_INSTRUCTIONS_V4
                    + DIRECT_REGISTERED_FILL_EPILOGUE_INSTRUCTIONS_V4
            );
            assert_eq!(
                DIRECT_REGISTERED_FILL_TRANSITION_BYTES_V4,
                dclutch_vm::v3::HEADER_BYTES
                    + DIRECT_REGISTERED_FILL_TRANSITION_INSTRUCTIONS_V4
                        * dclutch_vm::v3::INSTRUCTION_BYTES
            );
            let program =
                ProgramV3::decode(&DIRECT_REGISTERED_FILL_TRANSITION_V4).expect("program decode");
            assert_eq!(
                usize::from(program.common_scalar_count()),
                DIRECT_REGISTERED_FILL_COMMON_SCALARS_V4
            );
            assert_eq!(
                usize::from(program.common_identity_count()),
                DIRECT_REGISTERED_FILL_COMMON_IDENTITIES_V4
            );
            assert_eq!(
                program.item_scalar_stride(),
                DIRECT_REGISTERED_FILL_ITEM_SCALAR_STRIDE_V4
            );
            assert_eq!(
                program.item_identity_stride(),
                DIRECT_REGISTERED_FILL_ITEM_IDENTITY_STRIDE_V4
            );
        }

        fn valid_scalars() -> [u64; DIRECT_REGISTERED_FILL_COMMON_SCALARS_V4] {
            let mut scalars = [0_u64; DIRECT_REGISTERED_FILL_COMMON_SCALARS_V4];
            scalars[FILL_SCALAR_SLOT_V4] = 100;
            scalars[FILL_SCALAR_OUTCOME_COUNT_V4] = 3;
            scalars[FILL_SCALAR_MARKET_GENERATION_V4] = 7;
            scalars[FILL_SCALAR_PRICE_SCALE_V4] = 100;
            scalars[FILL_SCALAR_POLICY_FEE_BPS_V4] = 100;
            scalars[FILL_SCALAR_ROOT_OPEN_COUNT_V4] = 2;
            for (base, side, limit) in [(13, 0, 40), (32, 1, 60)] {
                scalars
                    .get_mut(base..base + 19)
                    .expect("participant register span")
                    .copy_from_slice(&[
                        side, 2, 1, 7, 0, 90, 110, 20, limit, 100, 0, 0, 0, 0, 0, 1, 1, 0, 7,
                    ]);
            }
            scalars[FILL_SCALAR_SELLER_RESERVED_CLAIMS_V4] = 20;
            scalars[FILL_SCALAR_BUYER_RESERVED_COLLATERAL_V4] = 12;
            scalars[FILL_SCALAR_CLAIM_SOURCE_REVISION_V4] = 4;
            scalars[FILL_SCALAR_CLAIM_DESTINATION_REVISION_V4] = 9;
            scalars[FILL_SCALAR_CUSTODY_REVISION_V4] = 3;
            scalars[FILL_SCALAR_SELLER_MAKER_RENT_PRINCIPAL_V4] = 1;
            scalars[FILL_SCALAR_SELLER_RECORD_RENT_PRINCIPAL_V4] = 1;
            scalars[FILL_SCALAR_BUYER_MAKER_RENT_PRINCIPAL_V4] = 1;
            scalars[FILL_SCALAR_BUYER_RECORD_RENT_PRINCIPAL_V4] = 1;
            scalars
        }

        fn valid_identities() -> [[u8; 32]; DIRECT_REGISTERED_FILL_COMMON_IDENTITIES_V4] {
            let mut identities = [[1_u8; 32]; DIRECT_REGISTERED_FILL_COMMON_IDENTITIES_V4];
            identities[FILL_IDENTITY_MARKET_V4] = [2; 32];
            identities[FILL_IDENTITY_SELLER_INTENT_MARKET_V4] = [2; 32];
            identities[FILL_IDENTITY_BUYER_INTENT_MARKET_V4] = [2; 32];
            identities[FILL_IDENTITY_SELLER_MAKER_MARKET_V4] = [2; 32];
            identities[FILL_IDENTITY_BUYER_MAKER_MARKET_V4] = [2; 32];
            identities[FILL_IDENTITY_SELLER_MAKER_V4] = [3; 32];
            identities[FILL_IDENTITY_BUYER_MAKER_V4] = [4; 32];
            identities[FILL_IDENTITY_SELLER_MAKER_REPLAY_OWNER_V4] = [3; 32];
            identities[FILL_IDENTITY_BUYER_MAKER_REPLAY_OWNER_V4] = [4; 32];
            identities
        }

        #[test]
        fn request_and_transition_derive_conserving_partial_fill() {
            let mut request = [0_u8; DIRECT_REGISTERED_FILL_REQUEST_BYTES_V3];
            encode_direct_registered_execution_request_v3_atomic(
                DirectExecutionActionV3::FillRegisteredOrdinary,
                DirectExecutionQuantityV3 {
                    fill: 10,
                    execution_price: 50,
                },
                3,
                &mut request,
            )
            .expect("request");
            assert!(matches!(
                DirectExecutionRequestV3::decode(&request, 3),
                Ok(DirectExecutionRequestV3::FillRegisteredOrdinary(_))
            ));
            let mut profile_scratch = [0_u8; DIRECT_REGISTERED_FILL_REQUEST_PROFILE_BYTES_V4];
            let mut profile_bytes = [0_u8; DIRECT_REGISTERED_FILL_REQUEST_PROFILE_BYTES_V4];
            encode_direct_registered_fill_request_profile_v4_atomic(
                &mut profile_scratch,
                &mut profile_bytes,
            )
            .expect("profile");
            let profile = RequestProfileV1::decode(&profile_bytes).expect("decode profile");
            let input_scalars = valid_scalars();
            let input_identities = valid_identities();
            let mut projection_scratch_scalars = input_scalars;
            let mut projection_scratch_identities = input_identities;
            let mut scalars = input_scalars;
            let mut identities = input_identities;
            project_atomic(
                profile,
                3,
                &request,
                ProjectionRegistersV1 {
                    input_scalars: &input_scalars,
                    input_identities: &input_identities,
                    scratch_scalars: &mut projection_scratch_scalars,
                    scratch_identities: &mut projection_scratch_identities,
                    output_scalars: &mut scalars,
                    output_identities: &mut identities,
                },
            )
            .expect("project");
            assert_eq!(scalars[FILL_SCALAR_QUANTITY_V4], 10);
            assert_eq!(scalars[FILL_SCALAR_EXECUTION_PRICE_V4], 50);

            let mut transition_scratch = [0_u8; DIRECT_REGISTERED_FILL_TRANSITION_BYTES_V4];
            let mut transition_bytes = [0_u8; DIRECT_REGISTERED_FILL_TRANSITION_BYTES_V4];
            encode_direct_registered_fill_transition_v4_atomic(
                &mut transition_scratch,
                &mut transition_bytes,
            )
            .expect("transition");
            let transition = ProgramV3::decode(&transition_bytes).expect("decode transition");
            let input = scalars;
            let mut scalar_scratch = input;
            let mut output = input;
            let mut identity_scratch = identities;
            let mut identity_output = identities;
            execute_fold_atomic(
                transition,
                3,
                RegisterInput {
                    scalars: &input,
                    identities: &identities,
                },
                RegisterOutput {
                    scalars: &mut scalar_scratch,
                    identities: &mut identity_scratch,
                },
                RegisterOutput {
                    scalars: &mut output,
                    identities: &mut identity_output,
                },
            )
            .expect("execute");
            assert_eq!(output[FILL_SCALAR_GROSS_V4], 5);
            assert_eq!(output[FILL_SCALAR_SELLER_FILLED_AFTER_V4], 10);
            assert_eq!(output[FILL_SCALAR_BUYER_FILLED_AFTER_V4], 10);
            assert_eq!(output[FILL_SCALAR_SELLER_RESERVED_CLAIMS_AFTER_V4], 10);
            assert_eq!(output[FILL_SCALAR_BUYER_RESERVED_COLLATERAL_AFTER_V4], 7);
            assert_eq!(output[FILL_SCALAR_SELLER_TERMINAL_V4], 0);
            assert_eq!(output[FILL_SCALAR_BUYER_TERMINAL_V4], 0);
            assert_eq!(output[FILL_SCALAR_CLAIM_SOURCE_REVISION_AFTER_V4], 5);
            assert_eq!(output[FILL_SCALAR_CLAIM_DESTINATION_REVISION_AFTER_V4], 10);
            // One Custody transfer, so one Custody revision: the canonical fill
            // takes a zero fee and enables the seller leg alone.
            assert_eq!(output[FILL_SCALAR_CUSTODY_REVISION_AFTER_SELLER_V4], 4);
            assert_eq!(output[FILL_SCALAR_CUSTODY_REVISION_AFTER_FEE_V4], 4);
        }

        fn refuses_without_output_commit(
            scalars: [u64; DIRECT_REGISTERED_FILL_COMMON_SCALARS_V4],
            identities: [[u8; 32]; DIRECT_REGISTERED_FILL_COMMON_IDENTITIES_V4],
        ) {
            let mut scratch = [0_u8; DIRECT_REGISTERED_FILL_TRANSITION_BYTES_V4];
            let mut bytes = [0_u8; DIRECT_REGISTERED_FILL_TRANSITION_BYTES_V4];
            encode_direct_registered_fill_transition_v4_atomic(&mut scratch, &mut bytes)
                .expect("transition");
            let transition = ProgramV3::decode(&bytes).expect("decode");
            let mut scalar_scratch = scalars;
            let mut output = [0x55_u64; DIRECT_REGISTERED_FILL_COMMON_SCALARS_V4];
            let before = output;
            let mut identity_scratch = identities;
            let mut identity_output = identities;
            assert!(
                execute_fold_atomic(
                    transition,
                    3,
                    RegisterInput {
                        scalars: &scalars,
                        identities: &identities,
                    },
                    RegisterOutput {
                        scalars: &mut scalar_scratch,
                        identities: &mut identity_scratch,
                    },
                    RegisterOutput {
                        scalars: &mut output,
                        identities: &mut identity_output,
                    },
                )
                .is_err()
            );
            assert_eq!(output, before);
        }

        #[test]
        fn substituted_market_or_nonintegral_quote_refuses_without_output_commit() {
            let mut input = valid_scalars();
            input[FILL_SCALAR_QUANTITY_V4] = 3;
            input[FILL_SCALAR_EXECUTION_PRICE_V4] = 50;
            let mut identities = valid_identities();
            identities[FILL_IDENTITY_BUYER_INTENT_MARKET_V4] = [9; 32];
            refuses_without_output_commit(input, identities);
        }

        /// The clause `73f0793` landed on the ordinary program and this one did
        /// not have. Above the denominator both fee legs can exceed the quote
        /// they are taken from; the conservation clause is an identity in the
        /// fee deltas and never noticed.
        #[test]
        fn a_venue_rate_above_the_denominator_refuses_without_output_commit() {
            let mut input = valid_scalars();
            input[FILL_SCALAR_QUANTITY_V4] = 10;
            input[FILL_SCALAR_EXECUTION_PRICE_V4] = 50;
            input[FILL_SCALAR_POLICY_FEE_BPS_V4] = 10_001;
            input[FILL_SCALAR_SELLER_FEE_BPS_V4] = 10_001;
            input[FILL_SCALAR_BUYER_FEE_BPS_V4] = 10_001;
            input[FILL_SCALAR_BUYER_RESERVED_COLLATERAL_V4] = 24;
            refuses_without_output_commit(input, valid_identities());
        }

        /// THE ZERO-FEE ROUTE SELECTION, executed rather than described.
        ///
        /// The fill's `EffectProgramV4` routes one Claims transfer and up to two
        /// Custody transfers -- the seller's net and the combined fee -- out of
        /// the buyer record's Vault. Neither Custody leg is unconditional:
        /// `CustodyRequestV1::validate` refuses `amount == 0` for
        /// `OperationV1::Transfer`, and on the CANONICAL admitted fill the
        /// combined fee is exactly zero, because both cumulative-difference legs
        /// floor to nothing at a hundred basis points. An Effect that routed the
        /// fee leg unconditionally would refuse the ordinary case.
        ///
        /// So the TRANSITION derives the enable bits, exactly as the
        /// inline-ordinary family does with
        /// `SCALAR_SELLER_TERMINAL_ROUTE_ENABLED_V3` and its two siblings, and
        /// advances the Custody replay revision by one per enabled route. Both
        /// are authored in `DirectRegisteredFillV4.lean`'s `routeOps`; this test
        /// is the Rust-side execution of the case that forced them.
        ///
        /// Ruling, taken from the ordinary family's fee semantics: a zero
        /// combined fee is a NO-TRANSFER PATH, not a refusal. Refusing it would
        /// refuse the ordinary small fill at a realistic venue rate, and every
        /// mid-order fill whose cumulative-difference delta is zero while the
        /// order as a whole pays.
        #[test]
        fn the_canonical_zero_fee_fill_enables_the_seller_route_alone() {
            let mut input = valid_scalars();
            input[FILL_SCALAR_QUANTITY_V4] = 10;
            input[FILL_SCALAR_EXECUTION_PRICE_V4] = 50;
            let identities = valid_identities();
            let output = execute(input, identities);

            // The canonical admitted fill quotes five and charges nothing.
            assert_eq!(output[FILL_SCALAR_SELLER_NET_V4], 5);
            assert_eq!(output[FILL_SCALAR_TOTAL_FEE_V4], 0);

            // A Custody Transfer carrying that fee refuses on its own terms --
            // which is why the fee route must be disabled, not routed empty.
            let fee_leg = fee_transfer_template(output[FILL_SCALAR_TOTAL_FEE_V4]);
            assert_eq!(
                fee_leg.to_bytes().map(|_| ()),
                Err(dclutch_custody::Error::InvalidOperationShape)
            );
            // The seller leg, which is nonzero here, is well formed.
            assert!(
                fee_transfer_template(output[FILL_SCALAR_SELLER_NET_V4])
                    .to_bytes()
                    .is_ok()
            );

            // Exactly one route is enabled, and it is the terminal seller leg.
            assert_eq!(output[FILL_SCALAR_SELLER_TERMINAL_ROUTE_ENABLED_V4], 1);
            assert_eq!(output[FILL_SCALAR_SELLER_INTERMEDIATE_ROUTE_ENABLED_V4], 0);
            assert_eq!(output[FILL_SCALAR_FEE_SOLE_ROUTE_ENABLED_V4], 0);
            assert_eq!(output[FILL_SCALAR_FEE_NONZERO_V4], 0);

            // And the ladder claims exactly that one transfer.
            assert_eq!(
                output[FILL_SCALAR_CUSTODY_REVISION_AFTER_SELLER_V4],
                output[FILL_SCALAR_CUSTODY_REVISION_V4] + 1
            );
            assert_eq!(
                output[FILL_SCALAR_CUSTODY_REVISION_AFTER_FEE_V4],
                output[FILL_SCALAR_CUSTODY_REVISION_V4] + 1
            );
        }

        /// The other three corners of `(sellerNet != 0, totalFee != 0)`, and the
        /// Custody envelope each one implies. Every enabled route carries a
        /// nonzero amount; every disabled route is one whose amount would have
        /// been zero.
        #[test]
        fn every_enabled_custody_route_carries_a_nonzero_amount() {
            let identities = valid_identities();

            // Both legs move: a rate that clears the floor on each side.
            let mut both = matched_scalars();
            both[FILL_SCALAR_POLICY_FEE_BPS_V4] = 2_000;
            both[FILL_SCALAR_SELLER_FEE_BPS_V4] = 2_000;
            both[FILL_SCALAR_BUYER_FEE_BPS_V4] = 2_000;
            both[FILL_SCALAR_BUYER_RESERVED_COLLATERAL_V4] = 14;
            let output = execute(both, identities);
            assert_eq!(output[FILL_SCALAR_SELLER_NET_V4], 4);
            assert_eq!(output[FILL_SCALAR_TOTAL_FEE_V4], 2);
            assert_eq!(output[FILL_SCALAR_SELLER_TERMINAL_ROUTE_ENABLED_V4], 0);
            assert_eq!(output[FILL_SCALAR_SELLER_INTERMEDIATE_ROUTE_ENABLED_V4], 1);
            assert_eq!(output[FILL_SCALAR_FEE_SOLE_ROUTE_ENABLED_V4], 0);
            assert_eq!(
                output[FILL_SCALAR_CUSTODY_REVISION_AFTER_FEE_V4],
                output[FILL_SCALAR_CUSTODY_REVISION_V4] + 2
            );
            assert!(
                fee_transfer_template(output[FILL_SCALAR_TOTAL_FEE_V4])
                    .to_bytes()
                    .is_ok()
            );

            // The seller nets nothing: the fee leg alone, and it is terminal.
            let mut fee_only = matched_scalars();
            fee_only[FILL_SCALAR_POLICY_FEE_BPS_V4] = 10_000;
            fee_only[FILL_SCALAR_SELLER_FEE_BPS_V4] = 10_000;
            fee_only[FILL_SCALAR_BUYER_FEE_BPS_V4] = 10_000;
            fee_only[FILL_SCALAR_BUYER_RESERVED_COLLATERAL_V4] = 24;
            let output = execute(fee_only, identities);
            assert_eq!(output[FILL_SCALAR_SELLER_NET_V4], 0);
            assert_eq!(output[FILL_SCALAR_TOTAL_FEE_V4], 10);
            assert_eq!(output[FILL_SCALAR_SELLER_TERMINAL_ROUTE_ENABLED_V4], 0);
            assert_eq!(output[FILL_SCALAR_SELLER_INTERMEDIATE_ROUTE_ENABLED_V4], 0);
            assert_eq!(output[FILL_SCALAR_FEE_SOLE_ROUTE_ENABLED_V4], 1);
            assert_eq!(
                output[FILL_SCALAR_CUSTODY_REVISION_AFTER_FEE_V4],
                output[FILL_SCALAR_CUSTODY_REVISION_V4] + 1
            );

            // Nothing moves: a fill at an execution price of zero quotes
            // nothing and charges nothing, and still transfers Claims.
            let mut free = matched_scalars();
            free[FILL_SCALAR_SELLER_LIMIT_V4] = 0;
            free[FILL_SCALAR_EXECUTION_PRICE_V4] = 0;
            let output = execute(free, identities);
            assert_eq!(output[FILL_SCALAR_GROSS_V4], 0);
            assert_eq!(output[FILL_SCALAR_SELLER_TERMINAL_ROUTE_ENABLED_V4], 0);
            assert_eq!(output[FILL_SCALAR_SELLER_INTERMEDIATE_ROUTE_ENABLED_V4], 0);
            assert_eq!(output[FILL_SCALAR_FEE_SOLE_ROUTE_ENABLED_V4], 0);
            assert_eq!(
                output[FILL_SCALAR_CUSTODY_REVISION_AFTER_FEE_V4],
                output[FILL_SCALAR_CUSTODY_REVISION_V4]
            );
            assert_eq!(
                output[FILL_SCALAR_CLAIM_SOURCE_REVISION_AFTER_V4],
                output[FILL_SCALAR_CLAIM_SOURCE_REVISION_V4] + 1
            );
            assert_eq!(output[FILL_SCALAR_SELLER_RESERVED_CLAIMS_AFTER_V4], 10);
        }

        /// `valid_scalars` with the matcher-selected pair the RequestProfile
        /// projects, which is otherwise supplied by the request.
        fn matched_scalars() -> [u64; DIRECT_REGISTERED_FILL_COMMON_SCALARS_V4] {
            let mut scalars = valid_scalars();
            scalars[FILL_SCALAR_QUANTITY_V4] = 10;
            scalars[FILL_SCALAR_EXECUTION_PRICE_V4] = 50;
            scalars
        }

        /// Run the emitted program over one bank and return the output.
        fn execute(
            input: [u64; DIRECT_REGISTERED_FILL_COMMON_SCALARS_V4],
            identities: [[u8; 32]; DIRECT_REGISTERED_FILL_COMMON_IDENTITIES_V4],
        ) -> [u64; DIRECT_REGISTERED_FILL_COMMON_SCALARS_V4] {
            let mut scratch = [0_u8; DIRECT_REGISTERED_FILL_TRANSITION_BYTES_V4];
            let mut bytes = [0_u8; DIRECT_REGISTERED_FILL_TRANSITION_BYTES_V4];
            encode_direct_registered_fill_transition_v4_atomic(&mut scratch, &mut bytes)
                .expect("transition");
            let transition = ProgramV3::decode(&bytes).expect("decode");
            let mut scalar_scratch = input;
            let mut output = input;
            let mut identity_scratch = identities;
            let mut identity_output = identities;
            execute_fold_atomic(
                transition,
                3,
                RegisterInput {
                    scalars: &input,
                    identities: &identities,
                },
                RegisterOutput {
                    scalars: &mut scalar_scratch,
                    identities: &mut identity_scratch,
                },
                RegisterOutput {
                    scalars: &mut output,
                    identities: &mut identity_output,
                },
            )
            .expect("execute");
            output
        }

        /// One Custody `Transfer` envelope of the shape the fill's fee leg
        /// needs, parameterised only by the amount under test.
        fn fee_transfer_template(amount: u64) -> dclutch_custody::CustodyRequestV1 {
            use dclutch_custody::{
                CallerRoleV1, CompartmentV1, ContextV1, CustodyRequestV1, OperationV1,
            };
            let id = |value: u8| [value; 32];
            CustodyRequestV1 {
                operation: OperationV1::Transfer,
                caller_role: CallerRoleV1::Trading,
                source_compartment: CompartmentV1::TradingPrincipal,
                destination_compartment: CompartmentV1::External,
                release_set: id(1),
                market: id(2),
                realm: id(3),
                context: id(4),
                caller_program: id(5),
                semantic: ContextV1 {
                    candidate: [0; 32],
                    source_owner: [0; 32],
                    destination_owner: id(6),
                    order: id(7),
                    parent_request_digest: id(8),
                    order_nonce: 1,
                    generation: 1,
                    page_index: 0,
                    execution_index: 0,
                    transfer_index: 1,
                },
                source: id(9),
                destination: id(10),
                source_vault_context: id(4),
                destination_vault_context: [0; 32],
                mint: id(11),
                token_program: id(12),
                payer: [0; 32],
                rent_refund: [0; 32],
                expected_revision: 1,
                resulting_revision: 2,
                amount,
                rent_lamports: 0,
            }
        }

        /// A REGISTERED SELL RECORD, AS THIS FAMILY WOULD WRITE ONE, IS UNFILLABLE.
        ///
        /// The fill re-proves both reservations rather than trusting them:
        /// `sellerMaximum - sellerFilled` must equal `sellerReservedClaims`
        /// exactly. A freshly registered record has `filled == 0`, so its claim
        /// reserve has to be its signed maximum.
        ///
        /// The family's ONLY EffectProgramV4 --
        /// `encode_direct_register_buy_effect_v4_atomic` -- writes
        /// `DirectRegisteredRecordLayoutV2::RESERVED_CLAIMS` from
        /// `REGISTERED_SCALAR_ZERO_V4`, which is right for the Buy it was written
        /// for and wrong for every Sell. A Sell Effect copied from it produces a
        /// record no fill can ever admit, at any maximum. That is what this test
        /// decides, over the whole family of records such an Effect can write.
        ///
        /// The fix costs no schema, which is worth recording because the
        /// neighbouring gaps do: the value a Sell Effect must write is the signed
        /// maximum, and the shared creation bank already carries it at
        /// `REGISTERED_SCALAR_MAXIMUM_V4`. It is one instruction operand, and it
        /// is exactly the kind of one-operand difference that copying RegisterBuy
        /// hides -- the record decodes, the creation admits, and nothing refuses
        /// until a fill that no test in the family runs.
        #[test]
        fn a_seller_record_carrying_the_reserve_this_family_writes_can_never_be_filled() {
            let identities = valid_identities();
            for maximum in [2_u64, 20, 1_000] {
                // One bank, one difference: the seller record's claim reserve.
                let mut bank = matched_scalars();
                bank[FILL_SCALAR_QUANTITY_V4] = 2;
                bank[FILL_SCALAR_SELLER_MAXIMUM_V4] = maximum;
                bank[FILL_SCALAR_SELLER_FILLED_V4] = 0;

                // What `REGISTERED_SCALAR_ZERO_V4` puts in the record.
                let mut written_by_the_buy_effect = bank;
                written_by_the_buy_effect[FILL_SCALAR_SELLER_RESERVED_CLAIMS_V4] = 0;
                refuses_without_output_commit(written_by_the_buy_effect, identities);

                // What a Sell Effect must write instead: the signed maximum.
                let mut corrected = bank;
                corrected[FILL_SCALAR_SELLER_RESERVED_CLAIMS_V4] = maximum;
                let output = execute(corrected, identities);
                assert_eq!(
                    output[FILL_SCALAR_SELLER_RESERVED_CLAIMS_AFTER_V4],
                    maximum - 2,
                    "a record reserving its own maximum fills at maximum {maximum}"
                );
            }
        }

        /// THE FILL'S READ SET, MEASURED -- and the writers it is still owed.
        ///
        /// `registered_bundle_v4`'s
        /// `every_register_the_effect_reads_has_a_declared_writer` measures an
        /// EFFECT's reads by perturbation and joins them to the artifacts' own
        /// static write declarations. The fill has no Effect, no AccountProfile
        /// and no LifecycleV5, so the same method runs one artifact earlier: each
        /// common register is perturbed in isolation and counted as READ exactly
        /// when it moves the shipped transition's decision or one of the
        /// registers the transition DECLARES it writes. The observation is
        /// restricted to those writes deliberately -- `execute_fold_atomic`
        /// copies the whole scratch bank to the output bank, so a whole-bank
        /// comparison reports every carried register as read.
        ///
        /// Two writers exist in this family today, and this test names them: the
        /// RequestProfile above projects the matcher's quantity and execution
        /// price, and the program owns the five constants it loads. Everything
        /// else the measurement finds is read by an object that ships and written
        /// by nothing -- an AccountProfile projection or a LifecycleV5 protected
        /// output has to supply it, and neither artifact exists.
        ///
        /// MEASURED: fifty-four scalars and nine identities are read; two of the
        /// scalars have a writer. The other fifty-two group cleanly, and the
        /// grouping is the specification for whoever authors the missing
        /// artifacts:
        ///
        /// * seven (0..5, 12) are the authenticated root phase, the Clock slot,
        ///   the Product outcome count, the Core Market generation, the immutable
        ///   config price scale and fee rate, and the root's live-maker count --
        ///   AccountProfile projections, exactly as the inline-ordinary profile
        ///   already projects its own.
        /// * thirty-eight (13..50) are the two persisted record-plus-replay
        ///   spans, whole and symmetric: nineteen coordinates per side, read out
        ///   of `DirectRegisteredRecordLayoutV2` and `DirectMakerReplayLayoutV1`.
        /// * three (80, 81, 84) are the child pre-revisions the settlement's
        ///   `expected_revision` fields commit to: two Claims Positions and the
        ///   buyer record's Custody replay.
        /// * four (88..91) are the rent principals the transition requires
        ///   NONZERO, and they are a DECISION rather than a lookup. Either the
        ///   AccountProfile projects them from each account's persisted
        ///   `RENT_PRINCIPAL` field, or a LifecycleV5 declares them as protected
        ///   outputs. The second costs schema: a
        ///   `LifecycleProtectedOutputsInputV3` names seven coordinates per plan
        ///   and this schema has only `historical_rent_principal` and `state` for
        ///   all four accounts, so four authenticate plans would want twelve more
        ///   scalars (created, bump observation, bump) and six more identities
        ///   (two maker beneficiaries, four owners) -- and the identity bank
        ///   carries eight unaddressed registers, which is enough for six. The
        ///   two guarded `Close` plans a terminal record needs cost nothing:
        ///   `LifecycleGuardInputV3::ScalarEq` reads `sellerTerminal` /
        ///   `buyerTerminal`, and `sellerRentOwner` / `buyerRentOwner` are
        ///   already the record RentCredit beneficiaries a Close requires.
        ///
        /// The nine identities are the Market, both makers, both intent Markets,
        /// both maker-replay Markets, and both replay-stored owners. The other
        /// twenty-three named coordinates are read by no instruction in the
        /// program: they exist for the Effect and the AccountProfile.
        #[test]
        fn the_transition_reads_registers_no_artifact_in_the_family_writes() {
            let (read_scalars, read_identities) = measured_read_set();

            // The measurement discriminates: a register the program writes
            // before it reads does not move the result when it is perturbed.
            for derived in [
                FILL_SCALAR_GROSS_V4,
                FILL_SCALAR_SELLER_NET_V4,
                FILL_SCALAR_TOTAL_FEE_V4,
                FILL_SCALAR_SELLER_TERMINAL_ROUTE_ENABLED_V4,
                FILL_SCALAR_CUSTODY_REVISION_AFTER_FEE_V4,
            ] {
                assert!(
                    !read_scalars.contains(&derived),
                    "scalar {derived} is derived, not read"
                );
            }
            // Same for the five constants: each is loaded before any use.
            for constant in [
                FILL_SCALAR_ZERO_V4,
                FILL_SCALAR_ONE_V4,
                FILL_SCALAR_GTC_V4,
                FILL_SCALAR_FEE_DENOMINATOR_V4,
                FILL_SCALAR_TERMINAL_V4,
            ] {
                assert!(
                    !read_scalars.contains(&constant),
                    "scalar {constant} is program-owned"
                );
            }

            // The whole of what this family writes into the fill's bank today.
            let projected = [FILL_SCALAR_QUANTITY_V4, FILL_SCALAR_EXECUTION_PRICE_V4];
            for index in projected {
                assert!(read_scalars.contains(&index));
            }
            let unwritten: std::vec::Vec<usize> = read_scalars
                .iter()
                .copied()
                .filter(|index| !projected.contains(index))
                .collect();

            assert_eq!(read_scalars.len(), 54);
            assert_eq!(unwritten.len(), 52);
            assert_eq!(read_identities.len(), 9);

            // Seven come from the authenticated root, config, Product and Clock.
            for index in [
                FILL_SCALAR_ROOT_PHASE_V4,
                FILL_SCALAR_SLOT_V4,
                FILL_SCALAR_OUTCOME_COUNT_V4,
                FILL_SCALAR_MARKET_GENERATION_V4,
                FILL_SCALAR_PRICE_SCALE_V4,
                FILL_SCALAR_POLICY_FEE_BPS_V4,
                FILL_SCALAR_ROOT_OPEN_COUNT_V4,
            ] {
                assert!(unwritten.contains(&index));
            }
            // Thirty-eight are the two persisted record-plus-replay spans, whole.
            for index in FILL_SCALAR_SELLER_SIDE_V4..=FILL_SCALAR_BUYER_MAKER_GENERATION_V4 {
                assert!(unwritten.contains(&index), "persisted scalar {index}");
            }
            // Three are the child pre-revisions each settlement leg commits to.
            for index in [
                FILL_SCALAR_CLAIM_SOURCE_REVISION_V4,
                FILL_SCALAR_CLAIM_DESTINATION_REVISION_V4,
                FILL_SCALAR_CUSTODY_REVISION_V4,
            ] {
                assert!(unwritten.contains(&index));
            }
            // And four are the rent principals, the decision named above.
            for index in [
                FILL_SCALAR_SELLER_MAKER_RENT_PRINCIPAL_V4,
                FILL_SCALAR_SELLER_RECORD_RENT_PRINCIPAL_V4,
                FILL_SCALAR_BUYER_MAKER_RENT_PRINCIPAL_V4,
                FILL_SCALAR_BUYER_RECORD_RENT_PRINCIPAL_V4,
            ] {
                assert!(unwritten.contains(&index));
            }

            // The nine identities are exactly the ones the transition
            // authenticates against; the other twenty-three named coordinates
            // are the Effect's and the AccountProfile's, not this program's.
            assert_eq!(
                read_identities,
                std::vec![
                    FILL_IDENTITY_MARKET_V4,
                    FILL_IDENTITY_SELLER_MAKER_V4,
                    FILL_IDENTITY_BUYER_MAKER_V4,
                    FILL_IDENTITY_SELLER_INTENT_MARKET_V4,
                    FILL_IDENTITY_BUYER_INTENT_MARKET_V4,
                    FILL_IDENTITY_SELLER_MAKER_MARKET_V4,
                    FILL_IDENTITY_BUYER_MAKER_MARKET_V4,
                    FILL_IDENTITY_SELLER_MAKER_REPLAY_OWNER_V4,
                    FILL_IDENTITY_BUYER_MAKER_REPLAY_OWNER_V4,
                ]
            );

            // THE SCHEMA HAS NO SLACK, and now that is a measurement rather
            // than a coincidence. The read set is the EXACT complement of the
            // transition's declared write set: every one of the ninety-six
            // scalars is either derived by this program or read by it, and none
            // is merely carried. That was invisible while the measurement
            // compared whole output banks, because `execute_fold_atomic` copies
            // scratch to output and a carried register is indistinguishable
            // from a read one there.
            let (written_scalars, written_identities) = transition_write_set();
            assert!(written_identities.is_empty(), "the fill writes no identity");
            let mut union = read_scalars.clone();
            union.extend(written_scalars.iter().copied());
            union.sort_unstable();
            union.dedup();
            assert_eq!(union.len(), DIRECT_REGISTERED_FILL_COMMON_SCALARS_V4);
            assert_eq!(written_scalars.len(), 42);
            assert!(
                read_scalars
                    .iter()
                    .all(|index| !written_scalars.contains(index))
            );
        }

        /// Perturb every common register in isolation against the canonical
        /// admitted bank and keep the ones that move the result.
        fn measured_read_set() -> (std::vec::Vec<usize>, std::vec::Vec<usize>) {
            let base_scalars = matched_scalars();
            let base_identities = valid_identities();
            let writes = transition_write_set();
            let base = resolve(&writes, base_scalars, base_identities);
            assert!(base.is_some(), "the baseline fill must be admitted");

            let mut scalars = std::vec::Vec::new();
            for index in 0..DIRECT_REGISTERED_FILL_COMMON_SCALARS_V4 {
                let original = *base_scalars.get(index).expect("scalar coordinate");
                let moved = [0, 1, 2, u64::MAX, original.wrapping_add(1)]
                    .into_iter()
                    .filter(|candidate| *candidate != original)
                    .any(|candidate| {
                        let mut perturbed = base_scalars;
                        *perturbed.get_mut(index).expect("scalar coordinate") = candidate;
                        resolve(&writes, perturbed, base_identities) != base
                    });
                if moved {
                    scalars.push(index);
                }
            }

            let mut identities = std::vec::Vec::new();
            for index in 0..DIRECT_REGISTERED_FILL_COMMON_IDENTITIES_V4 {
                let original = *base_identities.get(index).expect("identity coordinate");
                let moved = [[9_u8; 32], [0_u8; 32], [7_u8; 32]]
                    .into_iter()
                    .filter(|candidate| *candidate != original)
                    .any(|candidate| {
                        let mut perturbed = base_identities;
                        *perturbed.get_mut(index).expect("identity coordinate") = candidate;
                        resolve(&writes, base_scalars, perturbed) != base
                    });
                if moved {
                    identities.push(index);
                }
            }
            (scalars, identities)
        }

        /// The registers the shipped transition declares it writes -- the only
        /// output coordinates a read measurement may observe.
        ///
        /// Both item strides are zero, so the common bank is the whole bank.
        #[allow(clippy::type_complexity)]
        fn transition_write_set() -> (std::vec::Vec<usize>, std::vec::Vec<usize>) {
            let mut scratch = [0_u8; DIRECT_REGISTERED_FILL_TRANSITION_BYTES_V4];
            let mut bytes = [0_u8; DIRECT_REGISTERED_FILL_TRANSITION_BYTES_V4];
            encode_direct_registered_fill_transition_v4_atomic(&mut scratch, &mut bytes)
                .expect("transition");
            let transition = ProgramV3::decode(&bytes).expect("decode");
            assert_eq!(DIRECT_REGISTERED_FILL_ITEM_SCALAR_STRIDE_V4, 0);
            assert_eq!(DIRECT_REGISTERED_FILL_ITEM_IDENTITY_STRIDE_V4, 0);
            let scalars = (0..DIRECT_REGISTERED_FILL_COMMON_SCALARS_V4)
                .filter(|index| {
                    transition
                        .writes_register(RegisterWriteTargetV3 {
                            kind: RegisterKindV3::Scalar,
                            space: RegisterSpaceV3::Common,
                            index: u16::try_from(*index).expect("scalar register"),
                        })
                        .expect("writes")
                })
                .collect();
            let identities = (0..DIRECT_REGISTERED_FILL_COMMON_IDENTITIES_V4)
                .filter(|index| {
                    transition
                        .writes_register(RegisterWriteTargetV3 {
                            kind: RegisterKindV3::Identity,
                            space: RegisterSpaceV3::Common,
                            index: u16::try_from(*index).expect("identity register"),
                        })
                        .expect("writes")
                })
                .collect();
            (scalars, identities)
        }

        /// Execute the emitted program and return what a read is OBSERVABLE IN:
        /// its admission, and the registers it writes.
        ///
        /// `execute_fold_atomic` copies the whole scratch bank to the output
        /// bank, so every register the program never touches appears in the
        /// output VERBATIM. Comparing whole output banks therefore reports
        /// "read" for pass-through, which is not a read; restricting the
        /// observation to the program's own declared writes is what makes the
        /// measurement a measurement rather than an enumeration of the bank.
        #[allow(clippy::type_complexity)]
        fn resolve(
            writes: &(std::vec::Vec<usize>, std::vec::Vec<usize>),
            input: [u64; DIRECT_REGISTERED_FILL_COMMON_SCALARS_V4],
            identities: [[u8; 32]; DIRECT_REGISTERED_FILL_COMMON_IDENTITIES_V4],
        ) -> Option<(std::vec::Vec<u64>, std::vec::Vec<[u8; 32]>)> {
            let mut scratch = [0_u8; DIRECT_REGISTERED_FILL_TRANSITION_BYTES_V4];
            let mut bytes = [0_u8; DIRECT_REGISTERED_FILL_TRANSITION_BYTES_V4];
            encode_direct_registered_fill_transition_v4_atomic(&mut scratch, &mut bytes)
                .expect("transition");
            let transition = ProgramV3::decode(&bytes).expect("decode");
            let mut scalar_scratch = input;
            let mut output = input;
            let mut identity_scratch = identities;
            let mut identity_output = identities;
            execute_fold_atomic(
                transition,
                3,
                RegisterInput {
                    scalars: &input,
                    identities: &identities,
                },
                RegisterOutput {
                    scalars: &mut scalar_scratch,
                    identities: &mut identity_scratch,
                },
                RegisterOutput {
                    scalars: &mut output,
                    identities: &mut identity_output,
                },
            )
            .ok()?;
            Some((
                writes
                    .0
                    .iter()
                    .map(|index| *output.get(*index).expect("scalar"))
                    .collect(),
                writes
                    .1
                    .iter()
                    .map(|index| *identity_output.get(*index).expect("identity"))
                    .collect(),
            ))
        }

        /// The boundary itself stays admissible: a venue rate exactly at the
        /// denominator takes the whole quote as fee, which is a policy the
        /// makers may sign.
        #[test]
        fn a_venue_rate_at_the_denominator_admits() {
            let mut input = valid_scalars();
            input[FILL_SCALAR_QUANTITY_V4] = 10;
            input[FILL_SCALAR_EXECUTION_PRICE_V4] = 50;
            input[FILL_SCALAR_POLICY_FEE_BPS_V4] = 10_000;
            input[FILL_SCALAR_SELLER_FEE_BPS_V4] = 10_000;
            input[FILL_SCALAR_BUYER_FEE_BPS_V4] = 10_000;
            input[FILL_SCALAR_BUYER_RESERVED_COLLATERAL_V4] = 24;
            let identities = valid_identities();
            let mut scratch = [0_u8; DIRECT_REGISTERED_FILL_TRANSITION_BYTES_V4];
            let mut bytes = [0_u8; DIRECT_REGISTERED_FILL_TRANSITION_BYTES_V4];
            encode_direct_registered_fill_transition_v4_atomic(&mut scratch, &mut bytes)
                .expect("transition");
            let transition = ProgramV3::decode(&bytes).expect("decode");
            let mut scalar_scratch = input;
            let mut output = input;
            let mut identity_scratch = identities;
            let mut identity_output = identities;
            execute_fold_atomic(
                transition,
                3,
                RegisterInput {
                    scalars: &input,
                    identities: &identities,
                },
                RegisterOutput {
                    scalars: &mut scalar_scratch,
                    identities: &mut identity_scratch,
                },
                RegisterOutput {
                    scalars: &mut output,
                    identities: &mut identity_output,
                },
            )
            .expect("execute");
            assert_eq!(output[FILL_SCALAR_GROSS_V4], 5);
            assert_eq!(output[FILL_SCALAR_SELLER_NET_V4], 0);
            assert_eq!(output[FILL_SCALAR_BUYER_DEBIT_V4], 10);
        }
    }
}

#[cfg(not(target_os = "solana"))]
pub use host::*;
