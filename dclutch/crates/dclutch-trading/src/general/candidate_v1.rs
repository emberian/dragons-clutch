//! Candidate submission and page verification: the half nothing could reach.
//!
//! Decision 0009 §6 item 2 states the gap this module closes, and it is the
//! sharpest one General had. `evaluate_runtime_consider_row_with_manifest_v2`
//! is 1,987 lines of streamed candidate verification with **no caller outside
//! tests**, and `Consider` reads a `SubmittedVerifiedCandidate` that **no
//! action writes**. The settlement half was reachable only by handing it a
//! certificate produced off-chain, which means the one thing General's whole
//! design rests on -- that a candidate is verified by the protocol rather than
//! asserted by whoever submits it -- had never been executed as a protocol
//! step.
//!
//! Three records and two transitions close it:
//!
//! ```text
//! SubmitCandidate    GeneralCandidateV1 created, bound to a CLOSED batch
//!                      candidate_id proved to be the record's own digest
//!                          |
//! VerifyCandidateRow  x (rows)   -- permissionless, streamed, one row a step
//!                      RuntimeCandidateVerifierV2 cursor advances
//!                      each row authenticated against its ESCROWED order
//!                          |
//!                      terminal row writes VerifiedCandidateV2
//!                          |
//! Consider            (already exists) compares it against the incumbent
//! Freeze              (already exists) closes selection
//! ```
//!
//! **There is no challenge verb, and that is a verdict rather than an
//! omission.** Gen-2 had no way to displace a submitted candidate either; what
//! it had, and what gen-3 already has in `consider_verified_candidate_v2`, is a
//! running maximum under an immutable interpreted policy. Any solver may submit
//! a better candidate at any time while selection is open, and the better one
//! wins by the policy's own lexicographic comparison. A separate challenge verb
//! would add a second way to decide the same question.

use dclutch_sha256_adapter::{digest as sha256, digestv as sha256v};

use crate::general::collection_v1::{
    BatchStatusV1, GeneralBatchV1, GeneralCollectionErrorV1, GeneralOrderV1,
    authenticate_batch_candidate_v1, authenticate_batch_verified_candidate_v1,
    authenticate_order_execution_v1,
};
use crate::general::runtime_verify::{
    RuntimeConsiderRowBuffersV2, RuntimeConsiderRowViewV2, RuntimeManifestBuffersV2,
    RuntimeVerifyErrorV2, evaluate_runtime_consider_row_with_manifest_v2,
    evaluate_runtime_consider_row_with_manifest_workspace_v2, runtime_manifest_orders_for_row_v2,
    runtime_verifier_len_v2,
};
use crate::general::runtime_width::{
    CandidateV2, PageV2, VerifiedCandidateV2, verified_candidate_len,
};

/// Exact width of one General candidate submission record.
pub const GENERAL_CANDIDATE_BYTES_V1: usize = 224;

// The candidate and candidate-page seed domains are the crate root's to state.
//
// Each was declared twice with DIFFERENT BYTES: the root said
// `dclutch:general-candidate:v1` and this module said
// `dclutch-general-candidate-v1`, and for the page the two definitions did not
// even share a name -- root `GENERAL_PAGE_PDA_DOMAIN_V1` against a local
// `GENERAL_CANDIDATE_PAGE_PDA_DOMAIN_V1` -- which is why the collision survived
// the grep that found the first one. Neither had a consumer, so the divergence
// cost nothing yet and was free to remove; the moment the submission route and
// the settlement half each picked a definition, the two halves would have
// derived DIFFERENT addresses for the same record and every cross-half
// authentication would have refused an account that was, by its own module's
// account, canonical. Re-exported rather than restated so this module keeps its
// import path while the root remains the single author of the bytes.
pub use crate::general::{
    GENERAL_CANDIDATE_PDA_DOMAIN_V1, GENERAL_PAGE_PDA_DOMAIN_V1 as GENERAL_CANDIDATE_PAGE_DOMAIN_V1,
};

/// Canonical PDA seed domain for one candidate's streamed verifier cursor.
///
/// Unlike the two above this one has no counterpart in the root table, so it is
/// a sole statement rather than a second one and stays here with its own
/// assertion.
pub const GENERAL_VERIFIER_PDA_DOMAIN_V1: &[u8] = b"dclutch-general-verifier-v1";

const _: () = assert!(
    GENERAL_VERIFIER_PDA_DOMAIN_V1.len() <= 32,
    "GENERAL_VERIFIER_PDA_DOMAIN_V1 must fit one Solana PDA seed"
);

/// Sole byte-layout authority for one canonical candidate submission body.
pub struct GeneralCandidateLayoutV1;

impl GeneralCandidateLayoutV1 {
    /// Canonical body magic.
    pub const MAGIC: [u8; 8] = *b"DCGSUB01";
    /// Canonical layout version.
    pub const VERSION: u16 = 1;
    /// Candidate local-state phase tag.
    pub const PHASE: u8 = 22;
    /// Magic field offset.
    pub const MAGIC_OFFSET: usize = 0;
    /// Version field offset.
    pub const VERSION_OFFSET: usize = 8;
    /// Phase field offset.
    pub const PHASE_OFFSET: usize = 10;
    /// Header reserved-byte offset.
    pub const HEADER_RESERVED_OFFSET: usize = 11;
    /// Outcome-count field offset.
    pub const OUTCOME_COUNT_OFFSET: usize = 12;
    /// Page-count field offset.
    pub const PAGE_COUNT_OFFSET: usize = 16;
    /// Status field offset.
    pub const STATUS_OFFSET: usize = 20;
    /// Status padding offset.
    pub const STATUS_RESERVED_OFFSET: usize = 21;
    /// Page-revision field offset.
    pub const PAGE_REVISION_OFFSET: usize = 24;
    /// Candidate-identity field offset.
    pub const CANDIDATE_ID_OFFSET: usize = 32;
    /// Batch-identity field offset.
    pub const BATCH_ID_OFFSET: usize = 64;
    /// Solver-identity field offset.
    pub const SOLVER_ID_OFFSET: usize = 96;
    /// Verified-certificate digest field offset.
    pub const VERIFIED_DIGEST_OFFSET: usize = 128;
    /// Submission-slot field offset.
    pub const SUBMITTED_SLOT_OFFSET: usize = 160;
    /// Verified-revision field offset.
    pub const VERIFIED_REVISION_OFFSET: usize = 168;
    /// Execution-row count field offset.
    pub const ROW_COUNT_OFFSET: usize = 176;
    /// Execution-row padding offset.
    pub const ROW_RESERVED_OFFSET: usize = 180;
    /// Per-crank reward-rate field offset.
    pub const REWARD_RATE_OFFSET: usize = 184;
    /// Remaining verification-compartment field offset.
    pub const VERIFICATION_REMAINING_OFFSET: usize = 192;
    /// Remaining cleanup-compartment field offset.
    pub const CLEANUP_REMAINING_OFFSET: usize = 200;
    /// Canonical trailing-reserved range offset.
    pub const TAIL_RESERVED_OFFSET: usize = 208;
}

const STATUS_SUBMITTED: u8 = 1;
const STATUS_VERIFIED: u8 = 2;
const STATUS_CONSIDERED: u8 = 3;

/// Byte range of the `candidate_id` field inside a `CandidateV2` record.
const CANDIDATE_IDENTITY_OFFSET: usize = 32;
const CANDIDATE_IDENTITY_END: usize = 64;

/// Stable refusal from candidate submission or page verification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralCandidateErrorV1 {
    /// A record or bank had another exact width.
    InvalidLength,
    /// Magic, version, phase, or reserved bytes were noncanonical.
    InvalidHeader,
    /// The status byte named no canonical submission status.
    InvalidStatus,
    /// The submission phase did not admit this transition.
    InvalidPhaseTransition,
    /// A required identity or count was zero.
    ZeroIdentity,
    /// A checked count or coordinate calculation overflowed.
    ArithmeticOverflow,
    /// The declared `candidate_id` was not the record's own digest.
    NonCanonicalIdentity,
    /// A record belonged to another batch, candidate, page, or width.
    Substitution,
    /// The submission window had closed for this slot.
    OutsideWindow,
    /// The work escrow does not cover exactly the work still owed.
    Uncapitalized,
    /// The batch refused the candidate or one of its execution rows.
    Collection(GeneralCollectionErrorV1),
    /// The streamed verifier refused this row.
    Verify(RuntimeVerifyErrorV2),
}

impl From<GeneralCollectionErrorV1> for GeneralCandidateErrorV1 {
    fn from(value: GeneralCollectionErrorV1) -> Self {
        Self::Collection(value)
    }
}

impl From<RuntimeVerifyErrorV2> for GeneralCandidateErrorV1 {
    fn from(value: RuntimeVerifyErrorV2) -> Self {
        Self::Verify(value)
    }
}

/// Result alias for candidate submission and verification.
pub type GeneralCandidateResultV1<T> = core::result::Result<T, GeneralCandidateErrorV1>;

/// Canonical submission status.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum GeneralCandidateStatusV1 {
    /// Submitted; its rows have not all been verified.
    Submitted = STATUS_SUBMITTED,
    /// Every row verified; a certificate exists and names this candidate.
    Verified = STATUS_VERIFIED,
    /// The certificate has been offered to selection.
    Considered = STATUS_CONSIDERED,
}

impl GeneralCandidateStatusV1 {
    fn decode(value: u8) -> GeneralCandidateResultV1<Self> {
        match value {
            STATUS_SUBMITTED => Ok(Self::Submitted),
            STATUS_VERIFIED => Ok(Self::Verified),
            STATUS_CONSIDERED => Ok(Self::Considered),
            _ => Err(GeneralCandidateErrorV1::InvalidStatus),
        }
    }

    /// Return the canonical one-byte status tag.
    #[must_use]
    pub const fn tag(self) -> u8 {
        self as u8
    }
}

/// Immutable submission coordinates fixed when a candidate is submitted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralCandidateOpeningV1 {
    /// Runtime outcome width shared with the batch and every page.
    pub outcome_count: u32,
    /// Number of immutable pages this candidate declares.
    pub page_count: u32,
    /// Exact revision every one of this candidate's pages must carry.
    pub page_revision: u64,
    /// Slot the candidate was submitted at.
    pub submitted_slot: u64,
    /// Candidate content identity, proved against the record's own bytes.
    pub candidate_id: [u8; 32],
    /// Closed batch this candidate settles.
    pub batch_id: [u8; 32],
    /// Solver who submitted it and who owns its rent.
    pub solver_id: [u8; 32],
    /// Exact number of execution rows across every declared page.
    ///
    /// Declared at submission because it is what the work escrow is sized
    /// against, and checked by construction: the verifier cursor advances its
    /// revision exactly once per row, so a candidate whose real row count
    /// differs from this cannot complete.
    pub row_count: u32,
    /// Exact lamports one crank of this candidate's work pays its caller.
    pub reward_rate_lamports: u64,
}

impl GeneralCandidateOpeningV1 {
    /// Exact lamports that must be escrowed for verification and selection.
    ///
    /// One reward per execution row, plus one for the single consideration.
    pub fn verification_capacity(self) -> GeneralCandidateResultV1<u64> {
        u64::from(self.row_count)
            .checked_add(1)
            .and_then(|cranks| cranks.checked_mul(self.reward_rate_lamports))
            .ok_or(GeneralCandidateErrorV1::ArithmeticOverflow)
    }

    /// Exact lamports that must be escrowed to close this candidate out.
    #[must_use]
    pub const fn cleanup_capacity(self) -> u64 {
        self.reward_rate_lamports
    }

    /// Exact total work escrow one submission must carry.
    pub fn work_capacity(self) -> GeneralCandidateResultV1<u64> {
        self.verification_capacity()?
            .checked_add(self.cleanup_capacity())
            .ok_or(GeneralCandidateErrorV1::ArithmeticOverflow)
    }
}

/// Mutable submission state advanced by verification and by selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralCandidateStateV1 {
    /// Current canonical status.
    pub status: GeneralCandidateStatusV1,
    /// SHA-256 of the exact certificate bytes; zero until verified.
    pub verified_digest: [u8; 32],
    /// Verification revision the certificate carries; zero until verified.
    pub verified_revision: u64,
    /// Unspent lamports in the verification-and-selection compartment.
    pub verification_remaining: u64,
    /// Unspent lamports in the cleanup compartment.
    pub cleanup_remaining: u64,
}

/// One crank's exact reward and the actor it is owed to.
///
/// Every permissionless transition on a candidate returns one of these. It is
/// not advisory: the compartment has already been debited by the transition
/// that produced it, so a caller that drops it has taken work and withheld the
/// payment its own record now says was made.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkRewardV1 {
    /// Exact lamports this crank earned.
    pub lamports: u64,
    /// Which compartment it came out of.
    pub compartment: WorkCompartmentV1,
}

/// The two funded compartments of one candidate's work escrow.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkCompartmentV1 {
    /// Pays one crank per execution row, and the single consideration.
    Verification,
    /// Pays the one crank that closes a spent candidate out.
    Cleanup,
}

/// One complete candidate submission record.
///
/// **This is the account `Consider` reads and nothing wrote.** Its existence is
/// what turns "a certificate someone handed us" into "a certificate this
/// protocol produced from pages this protocol authenticated".
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralCandidateV1 {
    opening: GeneralCandidateOpeningV1,
    state: GeneralCandidateStateV1,
}

impl GeneralCandidateV1 {
    /// Submit one candidate against a closed batch.
    ///
    /// Permissionless and UNBONDED, which is gen-2's answer and the family's:
    /// every verb in its collection half was permissionless, gated on windows
    /// and counters rather than on identity, and gen-2 carried no bond
    /// anywhere. A bond is a fee on being right as much as on being wrong -- a
    /// solver whose valid candidate simply loses the comparison has done the
    /// protocol a service -- and slashing the honest case is what makes an open
    /// solver set close.
    ///
    /// What replaces it is gen-2's real invention: a COMPARTMENTALIZED, FULLY
    /// REFUNDABLE WORK ESCROW. The submission funds exactly the cranks its own
    /// life requires -- one per execution row, one for the single
    /// consideration, one to close it out -- each crank draws exactly one
    /// reward to whoever performs it, and everything unspent goes back to the
    /// solver. That is what lets submission be unbounded and permissionless
    /// without a candidate cap and without drawing on the Market's Hoard.
    ///
    /// **And it closes a liveness gap gen-2 had.** Gen-2's consideration was
    /// permissionless and UNPAID, which makes a verb permissible rather than
    /// live: a valid candidate nobody cranked before the selection window
    /// closed never competed at all, and a submitter whose consideration was
    /// censored had no recourse. Here the consideration is the last crank the
    /// verification compartment was sized for.
    ///
    /// The solver signs, but only to own the escrow and its refund -- not to be
    /// authorized. Anyone may submit.
    #[allow(clippy::too_many_arguments)]
    pub fn submit(
        batch: GeneralBatchV1,
        candidate: CandidateV2<'_>,
        page_revision: u64,
        row_count: u32,
        reward_rate_lamports: u64,
        solver_id: [u8; 32],
        funded_lamports: u64,
        current_slot: u64,
    ) -> GeneralCandidateResultV1<Self> {
        // The identity a submission carries must be the record's own digest.
        // `CandidateV2::decode` treats `candidate_id` as a declared field and
        // checks nothing about it, so before this a candidate could name ANY
        // identity -- including one already verified under different prices.
        authenticate_candidate_identity_v1(candidate)?;
        authenticate_batch_candidate_v1(batch, candidate.header())?;
        let opening = batch.opening();
        if current_slot < opening.collection_close_slot {
            // Submitting before the close would let a solver build against an
            // order set that can still grow, and would let them close it.
            return Err(GeneralCandidateErrorV1::OutsideWindow);
        }
        if current_slot >= opening.settlement_close_slot {
            return Err(GeneralCandidateErrorV1::OutsideWindow);
        }
        let header = candidate.header();
        if page_revision == 0 || is_zero(&solver_id) {
            return Err(GeneralCandidateErrorV1::ZeroIdentity);
        }
        if row_count == 0 || row_count < header.page_count || reward_rate_lamports == 0 {
            return Err(GeneralCandidateErrorV1::ZeroIdentity);
        }
        let opening = GeneralCandidateOpeningV1 {
            outcome_count: header.outcome_count,
            page_count: header.page_count,
            page_revision,
            submitted_slot: current_slot,
            candidate_id: header.candidate_id,
            batch_id: header.batch_id,
            solver_id,
            row_count,
            reward_rate_lamports,
        };
        // The escrow is exact in both directions. Underfunding buys work nobody
        // is paid for; overfunding leaves lamports with no rule for who gets
        // them, which is the same hole facing the other way.
        if funded_lamports != opening.work_capacity()? {
            return Err(GeneralCandidateErrorV1::Uncapitalized);
        }
        let value = Self {
            opening,
            state: GeneralCandidateStateV1 {
                status: GeneralCandidateStatusV1::Submitted,
                verified_digest: [0; 32],
                verified_revision: 0,
                verification_remaining: opening.verification_capacity()?,
                cleanup_remaining: opening.cleanup_capacity(),
            },
        };
        value.validate_capitalization(0)?;
        Ok(value)
    }

    /// Re-prove that the work escrow still covers exactly the work still owed.
    ///
    /// **This is the invariant gen-2 got right and gen-3 had not carried.** A
    /// funded escrow that is only checked when it is created decays into a
    /// balance nobody can reason about after the first crank. Re-proving it at
    /// every transition means the remaining lamports are always exactly the
    /// remaining cranks, so an over-draw is caught at the draw rather than at
    /// the last crank that finds the compartment empty.
    pub fn validate_capitalization(self, rows_verified: u32) -> GeneralCandidateResultV1<()> {
        let cranks_left = match self.state.status {
            GeneralCandidateStatusV1::Submitted => u64::from(
                self.opening
                    .row_count
                    .checked_sub(rows_verified)
                    .ok_or(GeneralCandidateErrorV1::Uncapitalized)?,
            )
            .checked_add(1)
            .ok_or(GeneralCandidateErrorV1::ArithmeticOverflow)?,
            // Verified but not yet considered: the single consideration remains.
            GeneralCandidateStatusV1::Verified => 1,
            GeneralCandidateStatusV1::Considered => 0,
        };
        let owed = cranks_left
            .checked_mul(self.opening.reward_rate_lamports)
            .ok_or(GeneralCandidateErrorV1::ArithmeticOverflow)?;
        if self.state.verification_remaining != owed
            || self.state.cleanup_remaining > self.opening.cleanup_capacity()
        {
            return Err(GeneralCandidateErrorV1::Uncapitalized);
        }
        Ok(())
    }

    /// Draw exactly one verification crank's reward.
    fn draw_verification(&mut self) -> GeneralCandidateResultV1<WorkRewardV1> {
        let rate = self.opening.reward_rate_lamports;
        self.state.verification_remaining = self
            .state
            .verification_remaining
            .checked_sub(rate)
            .ok_or(GeneralCandidateErrorV1::Uncapitalized)?;
        Ok(WorkRewardV1 {
            lamports: rate,
            compartment: WorkCompartmentV1::Verification,
        })
    }

    /// Draw the single cleanup crank's reward, closing this candidate out.
    ///
    /// Permissionless, and the residual verification compartment goes back to
    /// the solver rather than to the caller: a candidate that lost, or that
    /// nobody finished verifying, must not pay a stranger for work not done.
    pub fn close_out(&mut self) -> GeneralCandidateResultV1<(WorkRewardV1, u64)> {
        let rate = self.opening.reward_rate_lamports;
        if self.state.cleanup_remaining != rate {
            return Err(GeneralCandidateErrorV1::Uncapitalized);
        }
        self.state.cleanup_remaining = 0;
        let solver_refund = self.state.verification_remaining;
        self.state.verification_remaining = 0;
        Ok((
            WorkRewardV1 {
                lamports: rate,
                compartment: WorkCompartmentV1::Cleanup,
            },
            solver_refund,
        ))
    }

    /// Hostile-decode one exact 224-byte submission record.
    pub fn decode(bytes: &[u8]) -> GeneralCandidateResultV1<Self> {
        if bytes.len() != GENERAL_CANDIDATE_BYTES_V1 {
            return Err(GeneralCandidateErrorV1::InvalidLength);
        }
        if bytes.get(..8) != Some(GeneralCandidateLayoutV1::MAGIC.as_slice())
            || read_u16(bytes, GeneralCandidateLayoutV1::VERSION_OFFSET)?
                != GeneralCandidateLayoutV1::VERSION
            || read_u8(bytes, GeneralCandidateLayoutV1::PHASE_OFFSET)?
                != GeneralCandidateLayoutV1::PHASE
            || read_u8(bytes, GeneralCandidateLayoutV1::HEADER_RESERVED_OFFSET)? != 0
        {
            return Err(GeneralCandidateErrorV1::InvalidHeader);
        }
        require_zero(bytes, GeneralCandidateLayoutV1::STATUS_RESERVED_OFFSET, 3)?;
        require_zero(bytes, GeneralCandidateLayoutV1::ROW_RESERVED_OFFSET, 4)?;
        require_zero(bytes, GeneralCandidateLayoutV1::TAIL_RESERVED_OFFSET, 16)?;
        let opening = GeneralCandidateOpeningV1 {
            outcome_count: read_u32(bytes, GeneralCandidateLayoutV1::OUTCOME_COUNT_OFFSET)?,
            page_count: read_u32(bytes, GeneralCandidateLayoutV1::PAGE_COUNT_OFFSET)?,
            page_revision: read_u64(bytes, GeneralCandidateLayoutV1::PAGE_REVISION_OFFSET)?,
            submitted_slot: read_u64(bytes, GeneralCandidateLayoutV1::SUBMITTED_SLOT_OFFSET)?,
            candidate_id: read_array(bytes, GeneralCandidateLayoutV1::CANDIDATE_ID_OFFSET)?,
            batch_id: read_array(bytes, GeneralCandidateLayoutV1::BATCH_ID_OFFSET)?,
            solver_id: read_array(bytes, GeneralCandidateLayoutV1::SOLVER_ID_OFFSET)?,
            row_count: read_u32(bytes, GeneralCandidateLayoutV1::ROW_COUNT_OFFSET)?,
            reward_rate_lamports: read_u64(bytes, GeneralCandidateLayoutV1::REWARD_RATE_OFFSET)?,
        };
        let state = GeneralCandidateStateV1 {
            status: GeneralCandidateStatusV1::decode(read_u8(
                bytes,
                GeneralCandidateLayoutV1::STATUS_OFFSET,
            )?)?,
            verified_digest: read_array(bytes, GeneralCandidateLayoutV1::VERIFIED_DIGEST_OFFSET)?,
            verified_revision: read_u64(bytes, GeneralCandidateLayoutV1::VERIFIED_REVISION_OFFSET)?,
            verification_remaining: read_u64(
                bytes,
                GeneralCandidateLayoutV1::VERIFICATION_REMAINING_OFFSET,
            )?,
            cleanup_remaining: read_u64(bytes, GeneralCandidateLayoutV1::CLEANUP_REMAINING_OFFSET)?,
        };
        let value = Self { opening, state };
        value.validate()?;
        Ok(value)
    }

    /// Encode the exact canonical submission layout.
    #[must_use]
    pub fn to_bytes(self) -> [u8; GENERAL_CANDIDATE_BYTES_V1] {
        let mut output = [0_u8; GENERAL_CANDIDATE_BYTES_V1];
        put(
            &mut output,
            GeneralCandidateLayoutV1::MAGIC_OFFSET,
            &GeneralCandidateLayoutV1::MAGIC,
        );
        put(
            &mut output,
            GeneralCandidateLayoutV1::VERSION_OFFSET,
            &GeneralCandidateLayoutV1::VERSION.to_le_bytes(),
        );
        output[GeneralCandidateLayoutV1::PHASE_OFFSET] = GeneralCandidateLayoutV1::PHASE;
        put(
            &mut output,
            GeneralCandidateLayoutV1::OUTCOME_COUNT_OFFSET,
            &self.opening.outcome_count.to_le_bytes(),
        );
        put(
            &mut output,
            GeneralCandidateLayoutV1::PAGE_COUNT_OFFSET,
            &self.opening.page_count.to_le_bytes(),
        );
        output[GeneralCandidateLayoutV1::STATUS_OFFSET] = self.state.status.tag();
        put(
            &mut output,
            GeneralCandidateLayoutV1::PAGE_REVISION_OFFSET,
            &self.opening.page_revision.to_le_bytes(),
        );
        put(
            &mut output,
            GeneralCandidateLayoutV1::CANDIDATE_ID_OFFSET,
            &self.opening.candidate_id,
        );
        put(
            &mut output,
            GeneralCandidateLayoutV1::BATCH_ID_OFFSET,
            &self.opening.batch_id,
        );
        put(
            &mut output,
            GeneralCandidateLayoutV1::SOLVER_ID_OFFSET,
            &self.opening.solver_id,
        );
        put(
            &mut output,
            GeneralCandidateLayoutV1::VERIFIED_DIGEST_OFFSET,
            &self.state.verified_digest,
        );
        put(
            &mut output,
            GeneralCandidateLayoutV1::SUBMITTED_SLOT_OFFSET,
            &self.opening.submitted_slot.to_le_bytes(),
        );
        put(
            &mut output,
            GeneralCandidateLayoutV1::VERIFIED_REVISION_OFFSET,
            &self.state.verified_revision.to_le_bytes(),
        );
        put(
            &mut output,
            GeneralCandidateLayoutV1::ROW_COUNT_OFFSET,
            &self.opening.row_count.to_le_bytes(),
        );
        put(
            &mut output,
            GeneralCandidateLayoutV1::REWARD_RATE_OFFSET,
            &self.opening.reward_rate_lamports.to_le_bytes(),
        );
        put(
            &mut output,
            GeneralCandidateLayoutV1::VERIFICATION_REMAINING_OFFSET,
            &self.state.verification_remaining.to_le_bytes(),
        );
        put(
            &mut output,
            GeneralCandidateLayoutV1::CLEANUP_REMAINING_OFFSET,
            &self.state.cleanup_remaining.to_le_bytes(),
        );
        output
    }

    /// Validate every cross-field invariant of one submission.
    pub fn validate(self) -> GeneralCandidateResultV1<()> {
        if is_zero(&self.opening.candidate_id)
            || is_zero(&self.opening.batch_id)
            || is_zero(&self.opening.solver_id)
        {
            return Err(GeneralCandidateErrorV1::ZeroIdentity);
        }
        if self.opening.outcome_count == 0
            || self.opening.page_count == 0
            || self.opening.page_revision == 0
            || self.opening.row_count < self.opening.page_count
            || self.opening.reward_rate_lamports == 0
        {
            return Err(GeneralCandidateErrorV1::ZeroIdentity);
        }
        if self.state.verification_remaining > self.opening.verification_capacity()?
            || self.state.cleanup_remaining > self.opening.cleanup_capacity()
        {
            return Err(GeneralCandidateErrorV1::Uncapitalized);
        }
        match self.state.status {
            GeneralCandidateStatusV1::Submitted => {
                if !is_zero(&self.state.verified_digest) || self.state.verified_revision != 0 {
                    return Err(GeneralCandidateErrorV1::InvalidStatus);
                }
            }
            GeneralCandidateStatusV1::Verified | GeneralCandidateStatusV1::Considered => {
                if is_zero(&self.state.verified_digest) || self.state.verified_revision == 0 {
                    return Err(GeneralCandidateErrorV1::InvalidStatus);
                }
            }
        }
        Ok(())
    }

    /// Immutable submission coordinates.
    #[must_use]
    pub const fn opening(self) -> GeneralCandidateOpeningV1 {
        self.opening
    }

    /// Mutable submission state.
    #[must_use]
    pub const fn state(self) -> GeneralCandidateStateV1 {
        self.state
    }

    /// Record the certificate one completed verification produced.
    ///
    /// The certificate is bound three ways before the status moves: to this
    /// submission's candidate identity, to the batch whose escrow must cover
    /// it, and to its own bytes by digest. `Consider` then reads a certificate
    /// that a chain-executed verification wrote, which is exactly the property
    /// the settlement half assumed and nothing supplied.
    pub fn record_verified(
        &mut self,
        batch: GeneralBatchV1,
        verified_bytes: &[u8],
    ) -> GeneralCandidateResultV1<()> {
        if self.state.status != GeneralCandidateStatusV1::Submitted {
            return Err(GeneralCandidateErrorV1::InvalidPhaseTransition);
        }
        let verified = VerifiedCandidateV2::decode(verified_bytes)
            .map_err(|_| GeneralCandidateErrorV1::Substitution)?;
        let header = verified.header();
        if header.candidate_id != self.opening.candidate_id
            || header.batch_id != self.opening.batch_id
            || header.outcome_count != self.opening.outcome_count
            || header.page_count != self.opening.page_count
        {
            return Err(GeneralCandidateErrorV1::Substitution);
        }
        if header.revision == 0 {
            return Err(GeneralCandidateErrorV1::ZeroIdentity);
        }
        authenticate_batch_verified_candidate_v1(batch, header)?;
        self.state.status = GeneralCandidateStatusV1::Verified;
        self.state.verified_digest = sha256(verified_bytes);
        self.state.verified_revision = header.revision;
        // Recording the certificate spends no crank: the row that produced it
        // was already paid. What it must not do is leave the compartment
        // holding more than the one consideration still owed.
        self.validate_capitalization(self.opening.row_count)?;
        Ok(())
    }

    /// Record that this certificate has been offered to selection.
    ///
    /// `consider_verified_candidate_v2` refuses an exact replay of the same
    /// certificate digest, so this is not the sole replay guard; it is what
    /// makes the submission record say, on its own, whether its work is spent.
    pub fn record_considered(&mut self) -> GeneralCandidateResultV1<WorkRewardV1> {
        if self.state.status != GeneralCandidateStatusV1::Verified {
            return Err(GeneralCandidateErrorV1::InvalidPhaseTransition);
        }
        // THE LIVENESS GAP GEN-2 LEFT, closed. In gen-2 a valid candidate that
        // nobody cranked before the selection window closed simply never
        // competed, and a submitter whose consideration was censored had no
        // recourse at all -- the verb was permissionless and unpaid, which
        // makes it permissible rather than live. Here the consideration is the
        // last crank the verification compartment was sized for, so whoever
        // performs it is paid out of the candidate's own escrow.
        let reward = self.draw_verification()?;
        self.state.status = GeneralCandidateStatusV1::Considered;
        self.validate_capitalization(self.opening.row_count)?;
        Ok(reward)
    }
}

/// Return the canonical identity of one Candidate record.
///
/// The digest covers every byte EXCEPT the 32 that carry the identity itself,
/// which is the only way a self-describing record can be content-addressed.
/// The masked bytes are not skipped silently: they are excluded by construction
/// here and required to equal this value by
/// [`authenticate_candidate_identity_v1`].
pub fn general_candidate_identity_v1(candidate_bytes: &[u8]) -> GeneralCandidateResultV1<[u8; 32]> {
    let head = candidate_bytes
        .get(..CANDIDATE_IDENTITY_OFFSET)
        .ok_or(GeneralCandidateErrorV1::InvalidLength)?;
    let tail = candidate_bytes
        .get(CANDIDATE_IDENTITY_END..)
        .ok_or(GeneralCandidateErrorV1::InvalidLength)?;
    Ok(sha256v(&[head, tail]))
}

/// Require one Candidate record to carry its own digest as its identity.
pub fn authenticate_candidate_identity_v1(
    candidate: CandidateV2<'_>,
) -> GeneralCandidateResultV1<()> {
    if candidate.header().candidate_id != general_candidate_identity_v1(candidate.as_bytes())? {
        return Err(GeneralCandidateErrorV1::NonCanonicalIdentity);
    }
    Ok(())
}

/// Readonly inputs for one permissionless candidate verification step.
pub struct CandidateVerifyRowViewV1<'a> {
    /// The closed batch this candidate settles.
    pub batch: GeneralBatchV1,
    /// The submission record naming this candidate.
    pub submission: GeneralCandidateV1,
    /// Immutable runtime-width Candidate record.
    pub candidate: &'a [u8],
    /// Immutable runtime-width Page holding the next row.
    pub page: &'a [u8],
    /// Immutable order record the next row names.
    pub order: &'a [u8],
    /// Empty vacant state, all-zero initial state, or canonical persisted
    /// verifier cursor.
    pub cursor_before: &'a [u8],
    /// Empty vacant state or one exact all-zero certificate destination.
    pub verified_before: &'a [u8],
    /// Zero-based optimistic page index.
    pub expected_page_index: u32,
    /// Zero-based optimistic row index.
    pub expected_row_index: u32,
    /// Exact optimistic verifier revision.
    pub expected_revision: u64,
}

/// Scratch and candidate banks for one failure-atomic verification step.
pub struct CandidateVerifyRowBuffersV1<'a> {
    /// Non-authoritative verifier scratch; may change on refusal.
    pub cursor_scratch: &'a mut [u8],
    /// Complete verifier candidate; unchanged on refusal.
    pub cursor_output: &'a mut [u8],
    /// Non-authoritative certificate scratch; may change on refusal.
    pub verified_scratch: &'a mut [u8],
    /// Complete certificate candidate; unchanged on refusal.
    pub verified_output: &'a mut [u8],
    /// Non-authoritative manifest scratch; may change on refusal.
    pub manifest_scratch: &'a mut [u8],
    /// Complete manifest chunk; unchanged on refusal.
    pub manifest_output: &'a mut [u8],
}

enum CandidateVerifyRowExecutionBuffersV1<'a> {
    StateLast(CandidateVerifyRowBuffersV1<'a>),
    Workspace {
        cursor: &'a mut [u8],
        verified: &'a mut [u8],
        manifest: &'a mut [u8],
    },
}

/// Accepted summary for one candidate verification step.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateVerifyRowSummaryV1 {
    /// Whether this step completed the candidate's last declared page.
    pub complete: bool,
    /// Exact distinct globally grouped order count so far.
    pub order_count: u32,
    /// Exact successor verifier revision.
    pub revision: u64,
    /// Number of manifest order rows this step emitted.
    pub manifest_order_count: u32,
    /// The exact reward this crank earned its caller.
    pub reward: WorkRewardV1,
    /// The submission record this step advanced.
    pub submission: GeneralCandidateV1,
}

/// Return the exact manifest capacity one verification step will require.
///
/// A caller has to size the manifest bank before it can evaluate, and the count
/// is a function of the cursor's open order and whether this is the last row.
pub fn candidate_verify_manifest_orders_v1(
    view: &CandidateVerifyRowViewV1<'_>,
) -> GeneralCandidateResultV1<u32> {
    let (execution, terminal_step) = select_row(view)?;
    Ok(runtime_manifest_orders_for_row_v2(
        view.cursor_before,
        execution.header().order_id,
        terminal_step,
    )?)
}

/// Verify one candidate execution row, permissionlessly and on chain.
///
/// **This is the caller `evaluate_runtime_consider_row_with_manifest_v2` has
/// never had.** The evaluator is complete and adversarially tested; what it
/// lacked was anything that fed it authenticated inputs instead of fixtures.
/// The join is exactly two facts this module supplies and the evaluator cannot:
///
/// 1. The page belongs to THIS submission's candidate, at the revision the
///    submission pinned, so a solver cannot swap a page mid-stream.
/// 2. The [`crate::general::runtime_verify::AuthenticatedOrderTermsV2`] the evaluator
///    trusts comes from [`authenticate_order_execution_v1`] over a real,
///    ESCROWED order record -- not from a caller's assertion. Decision 0009 §1
///    called this hole B and named it the sharpest of the three: the verifier
///    enforced `ExcessLots` and `QuoteLimit` faithfully against limits the
///    caller made up.
///
/// Permissionless: verification is work anyone may do and nobody may withhold.
/// A solver who submits and then declines to verify would otherwise be able to
/// occupy a batch's selection window with a candidate nobody can evaluate.
pub fn verify_candidate_row_v1(
    view: CandidateVerifyRowViewV1<'_>,
    buffers: CandidateVerifyRowBuffersV1<'_>,
) -> GeneralCandidateResultV1<CandidateVerifyRowSummaryV1> {
    verify_candidate_row_inner_v1(
        view,
        CandidateVerifyRowExecutionBuffersV1::StateLast(buffers),
    )
}

/// Verify one candidate row directly in non-authoritative accelerator
/// workspaces.
///
/// The authenticated batch/page/order join and all capitalization rules are
/// identical to [`verify_candidate_row_v1`]. This form omits the state-last
/// buffer copies and is only suitable when every workspace is discarded on
/// refusal before any external effect is published.
pub fn verify_candidate_row_workspace_v1(
    view: CandidateVerifyRowViewV1<'_>,
    cursor_workspace: &mut [u8],
    verified_workspace: &mut [u8],
    manifest_workspace: &mut [u8],
) -> GeneralCandidateResultV1<CandidateVerifyRowSummaryV1> {
    verify_candidate_row_inner_v1(
        view,
        CandidateVerifyRowExecutionBuffersV1::Workspace {
            cursor: cursor_workspace,
            verified: verified_workspace,
            manifest: manifest_workspace,
        },
    )
}

fn verify_candidate_row_inner_v1(
    view: CandidateVerifyRowViewV1<'_>,
    buffers: CandidateVerifyRowExecutionBuffersV1<'_>,
) -> GeneralCandidateResultV1<CandidateVerifyRowSummaryV1> {
    if view.submission.state().status != GeneralCandidateStatusV1::Submitted {
        return Err(GeneralCandidateErrorV1::InvalidPhaseTransition);
    }
    // The revision counts rows consumed, so it is also the count of verification
    // cranks already paid. Re-proving the capitalization against it before the
    // work happens is what keeps "remaining lamports" and "remaining cranks"
    // the same number at every step.
    let rows_verified = u32::try_from(view.expected_revision)
        .map_err(|_| GeneralCandidateErrorV1::ArithmeticOverflow)?;
    view.submission.validate_capitalization(rows_verified)?;
    let (execution, terminal_step) = select_row(&view)?;
    let manifest_order_count = runtime_manifest_orders_for_row_v2(
        view.cursor_before,
        execution.header().order_id,
        terminal_step,
    )?;

    // The order record is the authority for the row's terms, and it must be an
    // order this batch actually admitted and still holds escrow for.
    let order = GeneralOrderV1::decode(view.order)?;
    let authenticated_order = authenticate_order_execution_v1(view.batch, order, execution)?;

    let runtime_view = RuntimeConsiderRowViewV2 {
        candidate: view.candidate,
        page: view.page,
        cursor_before: view.cursor_before,
        verified_before: view.verified_before,
        authenticated_order,
        expected_page_index: view.expected_page_index,
        expected_row_index: view.expected_row_index,
        expected_page_revision: view.submission.opening().page_revision,
        expected_revision: view.expected_revision,
        max_orders: view.batch.opening().max_orders,
    };
    let (summary, verified_result) = match buffers {
        CandidateVerifyRowExecutionBuffersV1::StateLast(buffers) => {
            let CandidateVerifyRowBuffersV1 {
                cursor_scratch,
                cursor_output,
                verified_scratch,
                verified_output,
                manifest_scratch,
                manifest_output,
            } = buffers;
            let summary = evaluate_runtime_consider_row_with_manifest_v2(
                runtime_view,
                RuntimeConsiderRowBuffersV2 {
                    cursor_scratch,
                    cursor_output,
                    verified_scratch,
                    verified_output: &mut *verified_output,
                },
                RuntimeManifestBuffersV2 {
                    manifest_scratch,
                    manifest_output,
                },
            )?;
            (summary, &*verified_output)
        }
        CandidateVerifyRowExecutionBuffersV1::Workspace {
            cursor,
            verified,
            manifest,
        } => {
            let summary = evaluate_runtime_consider_row_with_manifest_workspace_v2(
                runtime_view,
                cursor,
                verified,
                manifest,
            )?;
            (summary, &*verified)
        }
    };
    // The evaluator's own terminal test and ours must agree. They are derived
    // independently -- it from the cursor it just advanced, this module from
    // the page geometry it selected the row out of -- so a disagreement means
    // one of the two read a substituted page.
    if summary.complete != terminal_step {
        return Err(GeneralCandidateErrorV1::Substitution);
    }
    // The terminal row is the candidate's declared last row, or the declared
    // count was wrong and the escrow was sized against a lie.
    if terminal_step && summary.revision != u64::from(view.submission.opening().row_count) {
        return Err(GeneralCandidateErrorV1::Uncapitalized);
    }
    let mut submission = view.submission;
    let reward = submission.draw_verification()?;
    if summary.complete {
        // The terminal row is the sole producer of the certificate and the
        // same transition must make the submission name those exact bytes.
        // Leaving the submission in `Submitted` here made the result account
        // physically materializable but forever ineligible for Consider.
        submission.record_verified(view.batch, verified_result)?;
    }
    submission.validate_capitalization(
        rows_verified
            .checked_add(1)
            .ok_or(GeneralCandidateErrorV1::ArithmeticOverflow)?,
    )?;
    Ok(CandidateVerifyRowSummaryV1 {
        complete: summary.complete,
        order_count: summary.order_count,
        revision: summary.revision,
        manifest_order_count,
        reward,
        submission,
    })
}

/// Return the exact verifier cursor width for one submission.
pub fn candidate_verifier_len_v1(
    submission: GeneralCandidateV1,
) -> GeneralCandidateResultV1<usize> {
    Ok(runtime_verifier_len_v2(submission.opening().outcome_count)?)
}

/// Return the exact certificate width for one submission.
pub fn candidate_certificate_len_v1(
    submission: GeneralCandidateV1,
) -> GeneralCandidateResultV1<usize> {
    verified_candidate_len(submission.opening().outcome_count)
        .map_err(|_| GeneralCandidateErrorV1::InvalidLength)
}

/// The one row a step consumes, and whether it is the candidate's last.
type SelectedRow<'a> = (crate::general::runtime_width::ExecutionV2<'a>, bool);

fn select_row<'a>(
    view: &CandidateVerifyRowViewV1<'a>,
) -> GeneralCandidateResultV1<SelectedRow<'a>> {
    if view.batch.state().status != BatchStatusV1::Closed {
        return Err(GeneralCollectionErrorV1::NotClosed.into());
    }
    let candidate =
        CandidateV2::decode(view.candidate).map_err(|_| GeneralCandidateErrorV1::Substitution)?;
    authenticate_candidate_identity_v1(candidate)?;
    let opening = view.submission.opening();
    let header = candidate.header();
    if header.candidate_id != opening.candidate_id
        || header.batch_id != opening.batch_id
        || header.outcome_count != opening.outcome_count
        || header.page_count != opening.page_count
    {
        return Err(GeneralCandidateErrorV1::Substitution);
    }
    let page = PageV2::decode(view.page).map_err(|_| GeneralCandidateErrorV1::Substitution)?;
    let page_header = page.header();
    let expected_coordinate = view
        .expected_page_index
        .checked_add(1)
        .ok_or(GeneralCandidateErrorV1::ArithmeticOverflow)?;
    // The page revision is the submission's, not the caller's. Without this pin
    // a solver could publish a second page at the same coordinate and feed
    // whichever one suited the step.
    if page_header.candidate_id != opening.candidate_id
        || page_header.page_count != opening.page_count
        || page_header.outcome_count != opening.outcome_count
        || page_header.page_coordinate != expected_coordinate
        || page_header.revision != opening.page_revision
    {
        return Err(GeneralCandidateErrorV1::Substitution);
    }
    if view.expected_row_index >= page.row_count() {
        return Err(GeneralCandidateErrorV1::Substitution);
    }
    let execution = page
        .execution(view.expected_row_index)
        .map_err(|_| GeneralCandidateErrorV1::Substitution)?;
    let terminal_step = view.expected_page_index.checked_add(1) == Some(opening.page_count)
        && view.expected_row_index.checked_add(1) == Some(page.row_count());
    Ok((execution, terminal_step))
}

fn is_zero(value: &[u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) {
    if let Some(target) = output.get_mut(offset..offset.saturating_add(value.len())) {
        target.copy_from_slice(value);
    }
}

fn require_zero(bytes: &[u8], offset: usize, length: usize) -> GeneralCandidateResultV1<()> {
    let end = offset
        .checked_add(length)
        .ok_or(GeneralCandidateErrorV1::ArithmeticOverflow)?;
    if bytes
        .get(offset..end)
        .ok_or(GeneralCandidateErrorV1::InvalidLength)?
        .iter()
        .all(|value| *value == 0)
    {
        Ok(())
    } else {
        Err(GeneralCandidateErrorV1::InvalidHeader)
    }
}

fn read_u8(bytes: &[u8], offset: usize) -> GeneralCandidateResultV1<u8> {
    bytes
        .get(offset)
        .copied()
        .ok_or(GeneralCandidateErrorV1::InvalidLength)
}

fn read_u16(bytes: &[u8], offset: usize) -> GeneralCandidateResultV1<u16> {
    Ok(u16::from_le_bytes(read_fixed::<2>(bytes, offset)?))
}

fn read_u32(bytes: &[u8], offset: usize) -> GeneralCandidateResultV1<u32> {
    Ok(u32::from_le_bytes(read_fixed::<4>(bytes, offset)?))
}

fn read_u64(bytes: &[u8], offset: usize) -> GeneralCandidateResultV1<u64> {
    Ok(u64::from_le_bytes(read_fixed::<8>(bytes, offset)?))
}

fn read_array(bytes: &[u8], offset: usize) -> GeneralCandidateResultV1<[u8; 32]> {
    read_fixed::<32>(bytes, offset)
}

fn read_fixed<const WIDTH: usize>(
    bytes: &[u8],
    offset: usize,
) -> GeneralCandidateResultV1<[u8; WIDTH]> {
    let end = offset
        .checked_add(WIDTH)
        .ok_or(GeneralCandidateErrorV1::ArithmeticOverflow)?;
    let slice = bytes
        .get(offset..end)
        .ok_or(GeneralCandidateErrorV1::InvalidLength)?;
    <[u8; WIDTH]>::try_from(slice).map_err(|_| GeneralCandidateErrorV1::InvalidLength)
}

#[cfg(test)]
mod tests;
