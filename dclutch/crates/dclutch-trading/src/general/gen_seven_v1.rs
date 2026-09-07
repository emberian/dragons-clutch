//! Exact GEN-SEVEN request and mutable-state topology.
//!
//! The collection and candidate contracts already define seven pure
//! transitions. The admitted artifact family could not call them for a reason
//! narrower than "the triples are missing": the V2 controller request cannot
//! encode any of their action tags, and its two bump bytes cannot name the
//! three independently derived states touched by one terminal verification
//! step. This module is the caller-backed rung beneath those triples. It
//! hostile-decodes the width-preserving V3 request and returns the one state,
//! signer, trusted-environment, escrow and root-write topology for its action.
//!
//! It deliberately does not mark a wider release admissible. StateLifecycle,
//! AccountProfile, TransitionVM and EffectProgram still have to consume this
//! topology together, and release admission must continue refusing the
//! fourteen-action profile until that complete join exists.

use crate::general_codec::Error as GeneralCodecError;
use crate::general_codec::successor_request_v3::{ControllerActionV3, ControllerRequestV3};

use crate::general::{
    candidate_v1::{GeneralCandidateErrorV1, GeneralCandidateStatusV1, GeneralCandidateV1},
    collection_v1::GeneralBatchV1,
    escrow_v1::{
        ActionCustodyTransferV1, GeneralEscrowErrorV1, WorkEscrowClosePlanV1,
        WorkEscrowObservationV1, general_action_custody_transfer_v1,
    },
};

/// One mutable state role in a GEN-SEVEN action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralSevenStateRoleV1 {
    /// Trading-owned best-valid-submitted selection cursor.
    Selection,
    /// Trading-owned General batch envelope.
    Batch,
    /// Trading-owned General order envelope.
    Order,
    /// Trading-owned General candidate-submission envelope.
    Candidate,
    /// Trading-owned streamed verifier envelope.
    Verifier,
    /// Raw immutable `VerifiedCandidateV2` certificate produced only by the
    /// terminal verifier row. It is not a `GeneralLocalStateV3` envelope.
    VerifiedCandidate,
}

/// How one state role participates in the atomic transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralSevenStateOperationV1 {
    /// Authenticate an existing canonical state and write its successor.
    Authenticate,
    /// Create one vacant canonical PDA in this action.
    Create,
    /// Authenticate the cursor when present or create it on the first row.
    AuthenticateOrCreate,
    /// Create only when the semantic evaluator proves the terminal row.
    CreateIfTerminal,
    /// Close the canonical state after all credits are accounted exactly.
    Close,
}

/// One ordered mutable-state coordinate and its request-provided bump witness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralSevenStateCoordinateV1 {
    /// Semantic owner of the state bytes.
    pub role: GeneralSevenStateRoleV1,
    /// Lifecycle operation selected for this action.
    pub operation: GeneralSevenStateOperationV1,
    /// Untrusted bump witness. Zero is a valid Solana PDA bump and therefore
    /// must never be used as an absence sentinel.
    pub bump: u8,
}

/// The sole signer role, when the pure transition requires one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralSevenSignerV1 {
    /// Permissionless transition.
    None,
    /// Maker who owns the admitted order.
    OrderOwner,
    /// Solver who owns the candidate work escrow and refund.
    Solver,
}

/// Lamport effect that is part of the transition rather than account rent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralSevenLamportEffectV1 {
    /// No General-owned lamport compartment moves.
    None,
    /// Submission funds the exact verification plus cleanup compartments.
    FundCandidateWorkEscrow,
    /// Verification pays one exact crank reward from the Candidate account.
    PayVerificationCrank,
    /// Consideration pays its final verification-compartment crank.
    PayConsiderationCrank,
    /// Candidate close pays the cleanup crank, returns unspent verification
    /// work and rent to the solver, and leaves the Candidate account empty.
    CloseCandidateWorkEscrow,
}

/// Close guard preventing a permissionless actor from censoring a live candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralCandidateCloseGuardV1 {
    /// This action does not close a Candidate.
    None,
    /// Close only after successful consideration, or after the batch's whole
    /// settlement window has ended for an abandoned candidate.
    ConsideredOrSettlementWindowEnded,
}

/// Fully authenticated execution topology for one GEN-SEVEN request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralSevenPlanV1 {
    request: ControllerRequestV3,
    states: [Option<GeneralSevenStateCoordinateV1>; 3],
    signer: GeneralSevenSignerV1,
    current_slot: bool,
    writable_root_tail: bool,
    lamport_effect: GeneralSevenLamportEffectV1,
    custody_transfer: ActionCustodyTransferV1,
    claims_position_lifecycle: bool,
    candidate_close_guard: GeneralCandidateCloseGuardV1,
}

impl GeneralSevenPlanV1 {
    /// Canonical hostile-decoded request.
    #[must_use]
    pub const fn request(self) -> ControllerRequestV3 {
        self.request
    }

    /// Action subject identity.
    #[must_use]
    pub const fn subject_id(self) -> [u8; 32] {
        // Every GEN-SEVEN grammar requires a subject. Construction below is the
        // sole path to this type and returns an error if the codec disagrees.
        match self.request.subject_id {
            Some(value) => value,
            None => [0; 32],
        }
    }

    /// Ordered state coordinates. `None` is a topology fact; a bump value of
    /// zero inside `Some` still names a present coordinate.
    #[must_use]
    pub const fn states(self) -> [Option<GeneralSevenStateCoordinateV1>; 3] {
        self.states
    }

    /// Sole required signer role.
    #[must_use]
    pub const fn signer(self) -> GeneralSevenSignerV1 {
        self.signer
    }

    /// Whether AccountProfile must select trusted `CurrentSlot` projection.
    #[must_use]
    pub const fn requires_current_slot(self) -> bool {
        self.current_slot
    }

    /// Whether the Transition/AccountProfile pair must permit a write to the
    /// `GeneralRootV2` tail behind the immutable capability-root header.
    #[must_use]
    pub const fn writes_root_tail(self) -> bool {
        self.writable_root_tail
    }

    /// General-owned lamport effect for this transition.
    #[must_use]
    pub const fn lamport_effect(self) -> GeneralSevenLamportEffectV1 {
        self.lamport_effect
    }

    /// Canonical Custody movement, from the escrow module's sole table.
    #[must_use]
    pub const fn custody_transfer(self) -> ActionCustodyTransferV1 {
        self.custody_transfer
    }

    /// Whether the still-open Claims Position lifecycle must be supplied by
    /// the eventual artifact triple.
    #[must_use]
    pub const fn requires_claims_position_lifecycle(self) -> bool {
        self.claims_position_lifecycle
    }

    /// Candidate-close censorship guard.
    #[must_use]
    pub const fn candidate_close_guard(self) -> GeneralCandidateCloseGuardV1 {
        self.candidate_close_guard
    }
}

/// Stable refusal from GEN-SEVEN request planning.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralSevenPlanErrorV1 {
    /// The exact V3 request codec refused.
    Request(GeneralCodecError),
    /// A settlement-half action was offered to the front-half planner.
    SettlementAction,
    /// The codec admitted no subject for a subject-bearing action.
    MissingSubject,
    /// A candidate-close executor was given another authenticated action plan.
    CloseActionRequired,
    /// Candidate and batch identities did not join.
    CandidateSubstitution,
    /// A live unconsidered Candidate was offered before its batch expired.
    CandidateStillLive,
    /// Candidate record transition refused.
    Candidate(GeneralCandidateErrorV1),
    /// Exact work-escrow movement refused.
    Escrow(GeneralEscrowErrorV1),
}

impl From<GeneralCodecError> for GeneralSevenPlanErrorV1 {
    fn from(value: GeneralCodecError) -> Self {
        Self::Request(value)
    }
}

impl From<GeneralCandidateErrorV1> for GeneralSevenPlanErrorV1 {
    fn from(value: GeneralCandidateErrorV1) -> Self {
        Self::Candidate(value)
    }
}

impl From<GeneralEscrowErrorV1> for GeneralSevenPlanErrorV1 {
    fn from(value: GeneralEscrowErrorV1) -> Self {
        Self::Escrow(value)
    }
}

/// Result alias for GEN-SEVEN request planning.
pub type Result<T> = core::result::Result<T, GeneralSevenPlanErrorV1>;

/// Hostile-decode one exact V3 request and select its closed execution topology.
pub fn authenticate_general_seven_request_v1(bytes: &[u8]) -> Result<GeneralSevenPlanV1> {
    let request = ControllerRequestV3::decode(bytes)?;
    if matches!(
        request.action,
        ControllerActionV3::Freeze
            | ControllerActionV3::InitializeSettlement
            | ControllerActionV3::Collect
            | ControllerActionV3::Materialize
            | ControllerActionV3::Distribute
            | ControllerActionV3::Close
    ) {
        return Err(GeneralSevenPlanErrorV1::SettlementAction);
    }
    if request.subject_id.is_none() {
        return Err(GeneralSevenPlanErrorV1::MissingSubject);
    }
    let bumps = [
        request.primary_state_bump,
        request.secondary_state_bump,
        request.result_state_bump,
    ];
    let state = |role, operation, index: usize| {
        Some(GeneralSevenStateCoordinateV1 {
            role,
            operation,
            bump: bumps[index],
        })
    };
    let (states, signer, current_slot, writable_root_tail, lamport_effect) = match request.action {
        ControllerActionV3::Consider => (
            [
                state(
                    GeneralSevenStateRoleV1::Selection,
                    GeneralSevenStateOperationV1::AuthenticateOrCreate,
                    0,
                ),
                state(
                    GeneralSevenStateRoleV1::Candidate,
                    GeneralSevenStateOperationV1::Authenticate,
                    1,
                ),
                None,
            ],
            GeneralSevenSignerV1::None,
            false,
            false,
            GeneralSevenLamportEffectV1::PayConsiderationCrank,
        ),
        ControllerActionV3::OpenBatch => (
            [
                state(
                    GeneralSevenStateRoleV1::Batch,
                    GeneralSevenStateOperationV1::Create,
                    0,
                ),
                None,
                None,
            ],
            GeneralSevenSignerV1::None,
            true,
            true,
            GeneralSevenLamportEffectV1::None,
        ),
        ControllerActionV3::PlaceOrder => (
            [
                state(
                    GeneralSevenStateRoleV1::Batch,
                    GeneralSevenStateOperationV1::Authenticate,
                    0,
                ),
                state(
                    GeneralSevenStateRoleV1::Order,
                    GeneralSevenStateOperationV1::Create,
                    1,
                ),
                None,
            ],
            GeneralSevenSignerV1::OrderOwner,
            true,
            false,
            GeneralSevenLamportEffectV1::None,
        ),
        ControllerActionV3::CancelOrder => (
            [
                state(
                    GeneralSevenStateRoleV1::Batch,
                    GeneralSevenStateOperationV1::Authenticate,
                    0,
                ),
                state(
                    GeneralSevenStateRoleV1::Order,
                    GeneralSevenStateOperationV1::Authenticate,
                    1,
                ),
                None,
            ],
            GeneralSevenSignerV1::OrderOwner,
            true,
            false,
            GeneralSevenLamportEffectV1::None,
        ),
        ControllerActionV3::CloseBatch => (
            [
                state(
                    GeneralSevenStateRoleV1::Batch,
                    GeneralSevenStateOperationV1::Authenticate,
                    0,
                ),
                None,
                None,
            ],
            GeneralSevenSignerV1::None,
            true,
            true,
            GeneralSevenLamportEffectV1::None,
        ),
        ControllerActionV3::SubmitCandidate => (
            [
                state(
                    GeneralSevenStateRoleV1::Candidate,
                    GeneralSevenStateOperationV1::Create,
                    0,
                ),
                None,
                None,
            ],
            GeneralSevenSignerV1::Solver,
            true,
            false,
            GeneralSevenLamportEffectV1::FundCandidateWorkEscrow,
        ),
        ControllerActionV3::VerifyCandidateRow => (
            [
                state(
                    GeneralSevenStateRoleV1::Candidate,
                    GeneralSevenStateOperationV1::Authenticate,
                    0,
                ),
                state(
                    GeneralSevenStateRoleV1::Verifier,
                    GeneralSevenStateOperationV1::AuthenticateOrCreate,
                    1,
                ),
                state(
                    GeneralSevenStateRoleV1::VerifiedCandidate,
                    GeneralSevenStateOperationV1::CreateIfTerminal,
                    2,
                ),
            ],
            GeneralSevenSignerV1::None,
            false,
            false,
            GeneralSevenLamportEffectV1::PayVerificationCrank,
        ),
        ControllerActionV3::ReleaseOrder => (
            [
                state(
                    GeneralSevenStateRoleV1::Order,
                    GeneralSevenStateOperationV1::Authenticate,
                    0,
                ),
                None,
                None,
            ],
            GeneralSevenSignerV1::None,
            true,
            false,
            GeneralSevenLamportEffectV1::None,
        ),
        ControllerActionV3::CloseCandidate => (
            [
                state(
                    GeneralSevenStateRoleV1::Candidate,
                    GeneralSevenStateOperationV1::Close,
                    0,
                ),
                None,
                None,
            ],
            GeneralSevenSignerV1::None,
            true,
            false,
            GeneralSevenLamportEffectV1::CloseCandidateWorkEscrow,
        ),
        ControllerActionV3::Freeze
        | ControllerActionV3::InitializeSettlement
        | ControllerActionV3::Collect
        | ControllerActionV3::Materialize
        | ControllerActionV3::Distribute
        | ControllerActionV3::Close => return Err(GeneralSevenPlanErrorV1::SettlementAction),
    };
    let claims_position_lifecycle = matches!(
        request.action,
        ControllerActionV3::PlaceOrder
            | ControllerActionV3::CancelOrder
            | ControllerActionV3::ReleaseOrder
    );
    let custody_transfer = request
        .action
        .legacy()
        .map(general_action_custody_transfer_v1)
        .unwrap_or(ActionCustodyTransferV1::None);
    let candidate_close_guard = if request.action == ControllerActionV3::CloseCandidate {
        GeneralCandidateCloseGuardV1::ConsideredOrSettlementWindowEnded
    } else {
        GeneralCandidateCloseGuardV1::None
    };
    Ok(GeneralSevenPlanV1 {
        request,
        states,
        signer,
        current_slot,
        writable_root_tail,
        lamport_effect,
        custody_transfer,
        claims_position_lifecycle,
        candidate_close_guard,
    })
}

/// Authorize and plan one exact candidate/work-escrow close.
///
/// A considered Candidate is spent and may close immediately. An unconsidered
/// Candidate remains live until its batch's settlement deadline; before then a
/// permissionless close would be censorship. After the deadline its remaining
/// verification compartment is abandoned work and returns to the solver. The
/// cleanup crank is always paid to the caller, and the Candidate account's rent
/// is part of the solver credit rather than a fee.
pub fn plan_candidate_work_escrow_close_v1(
    action_plan: GeneralSevenPlanV1,
    batch: GeneralBatchV1,
    mut submission: GeneralCandidateV1,
    current_slot: u64,
    observation: WorkEscrowObservationV1,
    solver_lamports_before: u64,
) -> Result<WorkEscrowClosePlanV1> {
    if action_plan.request().action != ControllerActionV3::CloseCandidate {
        return Err(GeneralSevenPlanErrorV1::CloseActionRequired);
    }
    if action_plan.subject_id() != submission.opening().candidate_id
        || submission.opening().batch_id != batch.batch_id()
    {
        return Err(GeneralSevenPlanErrorV1::CandidateSubstitution);
    }
    if submission.state().status != GeneralCandidateStatusV1::Considered
        && current_slot < batch.opening().settlement_close_slot
    {
        return Err(GeneralSevenPlanErrorV1::CandidateStillLive);
    }
    let (cleanup, solver_refund) = submission.close_out()?;
    Ok(WorkEscrowClosePlanV1::new(
        observation,
        cleanup,
        solver_refund,
        solver_lamports_before,
    )?)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use crate::general_codec::successor_request_v3::{
        CONTROLLER_REQUEST_ACTION_OFFSET_V3, CONTROLLER_REQUEST_RESULT_BUMP_OFFSET_V3,
    };
    use crate::general_config::root::GeneralRootV2;
    use std::vec;

    use super::*;
    use crate::general::{
        candidate_v1::{GeneralCandidateOpeningV1, general_candidate_identity_v1},
        collection_v1::GeneralBatchOpeningV1,
        runtime_width::{CandidateHeaderV2, CandidateV2, candidate_len},
    };

    const ACTIONS: [ControllerActionV3; 9] = [
        ControllerActionV3::Consider,
        ControllerActionV3::OpenBatch,
        ControllerActionV3::PlaceOrder,
        ControllerActionV3::CancelOrder,
        ControllerActionV3::CloseBatch,
        ControllerActionV3::SubmitCandidate,
        ControllerActionV3::VerifyCandidateRow,
        ControllerActionV3::ReleaseOrder,
        ControllerActionV3::CloseCandidate,
    ];

    fn request(action: ControllerActionV3) -> ControllerRequestV3 {
        ControllerRequestV3 {
            action,
            expected_revision: if matches!(
                action,
                ControllerActionV3::Consider
                    | ControllerActionV3::OpenBatch
                    | ControllerActionV3::CloseBatch
            ) {
                7
            } else if action == ControllerActionV3::VerifyCandidateRow {
                3
            } else {
                0
            },
            subject_id: Some([action.tag().wrapping_add(1); 32]),
            page_index: u32::from(matches!(
                action,
                ControllerActionV3::Consider | ControllerActionV3::VerifyCandidateRow
            )) * 2,
            execution_index: u8::from(action == ControllerActionV3::VerifyCandidateRow) * 3,
            manifest_order_index: 0,
            primary_state_bump: 0,
            secondary_state_bump: if matches!(
                action,
                ControllerActionV3::Consider
                    | ControllerActionV3::PlaceOrder
                    | ControllerActionV3::CancelOrder
                    | ControllerActionV3::VerifyCandidateRow
            ) {
                42
            } else {
                0
            },
            result_state_bump: if action == ControllerActionV3::VerifyCandidateRow {
                43
            } else {
                0
            },
        }
    }

    #[test]
    fn all_revised_actions_have_one_exact_caller_visible_topology() {
        let expected_counts = [2_usize, 1, 2, 2, 1, 1, 3, 1, 1];
        for (action, expected_count) in ACTIONS.into_iter().zip(expected_counts) {
            let bytes = request(action).to_bytes().expect("canonical request");
            let plan = authenticate_general_seven_request_v1(&bytes).expect("front-half plan");
            assert_eq!(plan.request().action, action);
            assert_eq!(plan.subject_id(), [action.tag().wrapping_add(1); 32]);
            assert_eq!(
                plan.states().into_iter().flatten().count(),
                expected_count,
                "{action:?} state count",
            );
        }
    }

    #[test]
    fn signer_root_environment_and_escrow_obligations_are_closed() {
        for action in ACTIONS {
            let bytes = request(action).to_bytes().expect("canonical request");
            let plan = authenticate_general_seven_request_v1(&bytes).expect("front-half plan");
            assert_eq!(
                plan.signer(),
                match action {
                    ControllerActionV3::PlaceOrder | ControllerActionV3::CancelOrder => {
                        GeneralSevenSignerV1::OrderOwner
                    }
                    ControllerActionV3::SubmitCandidate => GeneralSevenSignerV1::Solver,
                    _ => GeneralSevenSignerV1::None,
                },
            );
            assert_eq!(
                plan.writes_root_tail(),
                matches!(
                    action,
                    ControllerActionV3::OpenBatch | ControllerActionV3::CloseBatch
                ),
            );
            assert_eq!(
                plan.requires_current_slot(),
                !matches!(
                    action,
                    ControllerActionV3::Consider | ControllerActionV3::VerifyCandidateRow
                ),
            );
            assert_eq!(
                plan.requires_claims_position_lifecycle(),
                matches!(
                    action,
                    ControllerActionV3::PlaceOrder
                        | ControllerActionV3::CancelOrder
                        | ControllerActionV3::ReleaseOrder
                ),
            );
            assert_eq!(
                matches!(plan.custody_transfer(), ActionCustodyTransferV1::Fixed(_)),
                matches!(
                    action,
                    ControllerActionV3::PlaceOrder
                        | ControllerActionV3::CancelOrder
                        | ControllerActionV3::ReleaseOrder
                ),
            );
            assert_eq!(
                plan.candidate_close_guard(),
                if action == ControllerActionV3::CloseCandidate {
                    GeneralCandidateCloseGuardV1::ConsideredOrSettlementWindowEnded
                } else {
                    GeneralCandidateCloseGuardV1::None
                },
            );
        }
    }

    #[test]
    fn verification_keeps_zero_bump_as_a_present_coordinate() {
        let value = request(ControllerActionV3::VerifyCandidateRow);
        assert_eq!(value.primary_state_bump, 0);
        let bytes = value.to_bytes().expect("canonical request");
        let plan = authenticate_general_seven_request_v1(&bytes).expect("verification plan");
        let states = plan.states();
        assert_eq!(
            states[0],
            Some(GeneralSevenStateCoordinateV1 {
                role: GeneralSevenStateRoleV1::Candidate,
                operation: GeneralSevenStateOperationV1::Authenticate,
                bump: 0,
            }),
        );
        assert_eq!(
            states[2].map(|state| state.operation),
            Some(GeneralSevenStateOperationV1::CreateIfTerminal),
        );
    }

    #[test]
    fn hostile_settlement_and_cross_action_substitutions_refuse_before_planning() {
        let settlement = ControllerRequestV3 {
            action: ControllerActionV3::Freeze,
            expected_revision: 7,
            subject_id: None,
            page_index: 0,
            execution_index: 0,
            manifest_order_index: 0,
            primary_state_bump: 1,
            secondary_state_bump: 0,
            result_state_bump: 0,
        }
        .to_bytes()
        .expect("canonical settlement request");
        assert_eq!(
            authenticate_general_seven_request_v1(&settlement),
            Err(GeneralSevenPlanErrorV1::SettlementAction),
        );

        let mut verification = request(ControllerActionV3::VerifyCandidateRow)
            .to_bytes()
            .expect("canonical verification");
        verification[CONTROLLER_REQUEST_ACTION_OFFSET_V3] =
            ControllerActionV3::SubmitCandidate.tag();
        assert!(matches!(
            authenticate_general_seven_request_v1(&verification),
            Err(GeneralSevenPlanErrorV1::Request(_))
        ));

        let mut place = request(ControllerActionV3::PlaceOrder)
            .to_bytes()
            .expect("canonical placement");
        place[CONTROLLER_REQUEST_RESULT_BUMP_OFFSET_V3] = 1;
        assert!(matches!(
            authenticate_general_seven_request_v1(&place),
            Err(GeneralSevenPlanErrorV1::Request(_))
        ));
    }

    #[test]
    fn candidate_lamport_effects_are_not_silently_treated_as_rent() {
        for (action, expected) in [
            (
                ControllerActionV3::SubmitCandidate,
                GeneralSevenLamportEffectV1::FundCandidateWorkEscrow,
            ),
            (
                ControllerActionV3::VerifyCandidateRow,
                GeneralSevenLamportEffectV1::PayVerificationCrank,
            ),
            (
                ControllerActionV3::Consider,
                GeneralSevenLamportEffectV1::PayConsiderationCrank,
            ),
            (
                ControllerActionV3::CloseCandidate,
                GeneralSevenLamportEffectV1::CloseCandidateWorkEscrow,
            ),
            (
                ControllerActionV3::OpenBatch,
                GeneralSevenLamportEffectV1::None,
            ),
        ] {
            let bytes = request(action).to_bytes().expect("canonical request");
            let plan = authenticate_general_seven_request_v1(&bytes).expect("front-half plan");
            assert_eq!(plan.lamport_effect(), expected);
        }
    }

    fn id(low: u8) -> [u8; 32] {
        let mut output = [0_u8; 32];
        output[0] = low;
        output
    }

    fn closed_batch_and_submission() -> (GeneralBatchV1, GeneralCandidateV1) {
        const COLLECTION_CLOSE: u64 = 100;
        const SETTLEMENT_CLOSE: u64 = 200;
        let mut root = GeneralRootV2::active(id(1), id(2), 7).expect("active root");
        let revision = root.revision();
        let mut batch = GeneralBatchV1::open(
            &mut root,
            GeneralBatchOpeningV1 {
                outcome_count: 1,
                sequence: 0,
                generation: 7,
                market: id(1),
                product_id: id(3),
                config_id: id(2),
                price_scale: 100,
                collection_close_slot: COLLECTION_CLOSE,
                settlement_close_slot: SETTLEMENT_CLOSE,
                max_orders: 1,
            },
            revision,
            10,
        )
        .expect("open batch");
        let revision = root.revision();
        batch.close(&mut root, revision).expect("close batch");
        let mut candidate_bytes = vec![0_u8; candidate_len(1).expect("candidate width")];
        let header = CandidateHeaderV2 {
            outcome_count: 1,
            page_count: 1,
            candidate_coordinate: 1,
            price_scale: 100,
            candidate_id: id(9),
            product_id: id(3),
            batch_id: batch.batch_id(),
        };
        CandidateV2::encode_into(header, &[100], &mut candidate_bytes).expect("draft candidate");
        let candidate_id = general_candidate_identity_v1(&candidate_bytes).expect("identity");
        CandidateV2::encode_into(
            CandidateHeaderV2 {
                candidate_id,
                ..header
            },
            &[100],
            &mut candidate_bytes,
        )
        .expect("addressed candidate");
        let opening = GeneralCandidateOpeningV1 {
            outcome_count: 1,
            page_count: 1,
            page_revision: 1,
            submitted_slot: COLLECTION_CLOSE,
            candidate_id,
            batch_id: batch.batch_id(),
            solver_id: id(4),
            row_count: 1,
            reward_rate_lamports: 10,
        };
        let submission = GeneralCandidateV1::submit(
            batch,
            CandidateV2::decode(&candidate_bytes).expect("candidate"),
            opening.page_revision,
            opening.row_count,
            opening.reward_rate_lamports,
            opening.solver_id,
            opening.work_capacity().expect("capacity"),
            opening.submitted_slot,
        )
        .expect("submission");
        (batch, submission)
    }

    fn candidate_close_action_plan(submission: GeneralCandidateV1) -> GeneralSevenPlanV1 {
        let mut value = request(ControllerActionV3::CloseCandidate);
        value.subject_id = Some(submission.opening().candidate_id);
        authenticate_general_seven_request_v1(&value.to_bytes().expect("candidate-close request"))
            .expect("candidate-close topology")
    }

    #[test]
    fn hostile_permissionless_close_cannot_censor_a_live_candidate() {
        let (batch, submission) = closed_batch_and_submission();
        let observation = WorkEscrowObservationV1 {
            escrow_lamports: 35,
            rent_floor: 5,
            beneficiary_lamports: 100,
        };
        assert_eq!(
            plan_candidate_work_escrow_close_v1(
                candidate_close_action_plan(submission),
                batch,
                submission,
                199,
                observation,
                200,
            ),
            Err(GeneralSevenPlanErrorV1::CandidateStillLive),
        );
        let plan = plan_candidate_work_escrow_close_v1(
            candidate_close_action_plan(submission),
            batch,
            submission,
            200,
            observation,
            200,
        )
        .expect("expired candidate closes");
        assert_eq!(plan.cleanup_reward(), 10);
        assert_eq!(plan.solver_credit(), 25);
        assert_eq!(plan.cranker_after(), 110);
        assert_eq!(plan.solver_after(), 225);
        plan.validate_post(0, 110, 225).expect("exact poststate");
    }

    #[test]
    fn hostile_candidate_close_refuses_a_substituted_batch() {
        let (batch, submission) = closed_batch_and_submission();
        let mut foreign_root = GeneralRootV2::active(id(5), id(2), 7).expect("foreign root");
        let revision = foreign_root.revision();
        let mut foreign_batch = GeneralBatchV1::open(
            &mut foreign_root,
            GeneralBatchOpeningV1 {
                market: id(5),
                ..batch.opening()
            },
            revision,
            10,
        )
        .expect("foreign batch");
        let revision = foreign_root.revision();
        foreign_batch
            .close(&mut foreign_root, revision)
            .expect("foreign close");
        assert_eq!(
            plan_candidate_work_escrow_close_v1(
                candidate_close_action_plan(submission),
                foreign_batch,
                submission,
                200,
                WorkEscrowObservationV1 {
                    escrow_lamports: 35,
                    rent_floor: 5,
                    beneficiary_lamports: 100,
                },
                200,
            ),
            Err(GeneralSevenPlanErrorV1::CandidateSubstitution),
        );
    }

    #[test]
    fn hostile_candidate_close_refuses_another_action_or_candidate_identity() {
        let (batch, submission) = closed_batch_and_submission();
        let observation = WorkEscrowObservationV1 {
            escrow_lamports: 35,
            rent_floor: 5,
            beneficiary_lamports: 100,
        };
        let open = authenticate_general_seven_request_v1(
            &request(ControllerActionV3::OpenBatch)
                .to_bytes()
                .expect("open request"),
        )
        .expect("open topology");
        assert_eq!(
            plan_candidate_work_escrow_close_v1(open, batch, submission, 200, observation, 200,),
            Err(GeneralSevenPlanErrorV1::CloseActionRequired),
        );

        let mut substituted = request(ControllerActionV3::CloseCandidate);
        substituted.subject_id = Some(id(99));
        let substituted = authenticate_general_seven_request_v1(
            &substituted.to_bytes().expect("substituted close request"),
        )
        .expect("substituted close topology");
        assert_eq!(
            plan_candidate_work_escrow_close_v1(
                substituted,
                batch,
                submission,
                200,
                observation,
                200,
            ),
            Err(GeneralSevenPlanErrorV1::CandidateSubstitution),
        );
    }
}
