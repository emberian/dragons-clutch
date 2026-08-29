//! Durable preparation state for a lock-bounded Dealer scenario commit.
//!
//! The existing selector-9 Hot frame is useful topology evidence, but its
//! account set cannot execute on a cluster whose transaction lock limit is 64.
//! This checkpoint is the semantic bridge for a split route: bounded prepare
//! transactions append authenticated page receipts in canonical order, one
//! selected accelerator evaluation seals the best valid submitted candidate,
//! and a final transaction reauthenticates every mutable prestate before it
//! performs Claims/Custody effects and closes the checkpoint atomically.
//!
//! This crate owns only hostile fixed-layout decoding and the total phase
//! machine. A Solana adapter must derive the PDA, compute every domain-separated
//! digest from observed bytes, authenticate the receipt producer, enforce the
//! cluster lock census, execute children, and close/refund the account.

use super::{Error as CodecError, array_at, byte_at, put, put_byte, put_u64, require_zero, u64_at};

/// Maximum canonical preparation pages for one Dealer scenario.
pub const DEALER_SCENARIO_PREPARATION_PAGES_V1: usize = 6;
/// Exact checkpoint account-data width.
pub const DEALER_SCENARIO_CHECKPOINT_BYTES_V1: usize = 752;
/// Canonical checkpoint magic.
pub const DEALER_SCENARIO_CHECKPOINT_MAGIC_V1: [u8; 8] = *b"DCLTDSC1";
/// Implemented checkpoint schema version.
pub const DEALER_SCENARIO_CHECKPOINT_VERSION_V1: u16 = 1;
/// Trading PDA domain for one request-scoped checkpoint.
pub const DEALER_SCENARIO_CHECKPOINT_PDA_DOMAIN_V1: &[u8] = b"dclutch:dealer-checkpoint:v1";
/// Domain for one page receipt over an exact checkpoint prestate.
pub const DEALER_SCENARIO_PAGE_RECEIPT_DOMAIN_V1: &[u8] = b"dclutch:dealer-scenario-page:v1";
/// Domain for the joined Claims prestate used by preparation and commit.
pub const DEALER_SCENARIO_CLAIMS_PRESTATE_DOMAIN_V1: &[u8] =
    b"dclutch:dealer-scenario-claims-prestate:v1";
/// Domain for the joined Custody prestate used by preparation and commit.
pub const DEALER_SCENARIO_CUSTODY_PRESTATE_DOMAIN_V1: &[u8] =
    b"dclutch:dealer-scenario-custody-prestate:v1";
/// Domain for the selected accelerator evaluation receipt.
pub const DEALER_SCENARIO_EVALUATION_RECEIPT_DOMAIN_V1: &[u8] =
    b"dclutch:dealer-scenario-evaluation:v1";
/// Domain for the ordered active Custody effect commitment.
pub const DEALER_SCENARIO_EFFECTS_DOMAIN_V1: &[u8] = b"dclutch:dealer-scenario-effects:v1";

const VERSION_OFFSET: usize = 8;
const PHASE_OFFSET: usize = 10;
const PAGE_COUNT_OFFSET: usize = 11;
const NEXT_PAGE_OFFSET: usize = 12;
const RESERVED_OFFSET: usize = 13;
const RESERVED_BYTES: usize = 3;
const REVISION_OFFSET: usize = 16;
const GENERATION_OFFSET: usize = 24;
const CREATED_SLOT_OFFSET: usize = 32;
const EXPIRES_AT_OFFSET: usize = 40;
const RELEASE_SET_OFFSET: usize = 48;
const MARKET_OFFSET: usize = 80;
const CHILD_ROOT_OFFSET: usize = 112;
const OBLIGATION_OFFSET: usize = 144;
const REFUND_BENEFICIARY_OFFSET: usize = 176;
const REQUEST_DIGEST_OFFSET: usize = 208;
const ROOT_PRESTATE_DIGEST_OFFSET: usize = 240;
const CLAIMS_PRESTATE_DIGEST_OFFSET: usize = 272;
const OBLIGATION_PRESTATE_DIGEST_OFFSET: usize = 304;
const CUSTODY_PRESTATE_DIGEST_OFFSET: usize = 336;
const LAST_CHECKPOINT_PRESTATE_DIGEST_OFFSET: usize = 368;
const PAGE_RECEIPT_DIGESTS_OFFSET: usize = 400;
const EVALUATION_RECEIPT_DIGEST_OFFSET: usize = 592;
const CANDIDATE_BANK_DIGEST_OFFSET: usize = 624;
const CANDIDATE_OBLIGATION_DIGEST_OFFSET: usize = 656;
const CLAIMS_DELTA_DIGEST_OFFSET: usize = 688;
const EFFECTS_DIGEST_OFFSET: usize = 720;

/// Durable checkpoint phase.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerScenarioCheckpointPhaseV1 {
    /// Authenticated page receipts are still being collected.
    Collecting = 1,
    /// One admitted evaluation sealed the candidate and effect commitments.
    Evaluated = 2,
}

impl DealerScenarioCheckpointPhaseV1 {
    fn decode(value: u8) -> CheckpointResultV1<Self> {
        match value {
            1 => Ok(Self::Collecting),
            2 => Ok(Self::Evaluated),
            _ => Err(DealerScenarioCheckpointErrorV1::Phase),
        }
    }
}

/// Immutable facts for a new request-scoped preparation checkpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerScenarioCheckpointInputV1 {
    /// Selected immutable release set.
    pub release_set: [u8; 32],
    /// Logical Core Market.
    pub market: [u8; 32],
    /// Trading child root.
    pub child_root: [u8; 32],
    /// Trading-owned obligation PDA.
    pub obligation: [u8; 32],
    /// Sole account which receives checkpoint rent on commit or cleanup.
    pub refund_beneficiary: [u8; 32],
    /// Digest of the exact family request bytes.
    pub request_digest: [u8; 32],
    /// Digest of the root bytes observed before preparation.
    pub root_prestate_digest: [u8; 32],
    /// Domain-separated digest of every Claims page admitted for evaluation.
    ///
    /// This is zero while the checkpoint is collecting and is written exactly
    /// once by [`DealerScenarioCheckpointV1::finish_evaluation`] from the
    /// adapter's ordered page transcript.
    pub claims_prestate_digest: [u8; 32],
    /// Digest of the exact current obligation account bytes.
    pub obligation_prestate_digest: [u8; 32],
    /// Domain-separated digest of every Custody page admitted for evaluation.
    ///
    /// This is zero while the checkpoint is collecting and is written exactly
    /// once by [`DealerScenarioCheckpointV1::finish_evaluation`] from the
    /// adapter's ordered page transcript.
    pub custody_prestate_digest: [u8; 32],
    /// Current Core Market generation.
    pub generation: u64,
    /// Finalized slot which created the checkpoint.
    pub created_slot: u64,
    /// Last slot at which evaluation or commit may succeed.
    pub expires_at: u64,
}

/// Commit-last evaluation commitments from the selected admitted accelerator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerScenarioEvaluationV1 {
    /// Typed receipt for the best valid submitted candidate.
    pub evaluation_receipt_digest: [u8; 32],
    /// Digest of the exact returned scalar-then-identity candidate bank.
    pub candidate_bank_digest: [u8; 32],
    /// Digest of the exact candidate obligation account bytes.
    pub candidate_obligation_digest: [u8; 32],
    /// Expected family-neutral Claims delta digest.
    pub claims_delta_digest: [u8; 32],
    /// Ordered commitment to all active canonical Custody effects.
    pub effects_digest: [u8; 32],
}

/// Exact live evidence which the final atomic commit must reauthenticate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerScenarioCommitEvidenceV1 {
    /// Exact family request digest.
    pub request_digest: [u8; 32],
    /// Current root body digest; it must still equal preparation prestate.
    pub root_prestate_digest: [u8; 32],
    /// Current joined Claims prestate digest.
    pub claims_prestate_digest: [u8; 32],
    /// Current obligation account-data digest.
    pub obligation_prestate_digest: [u8; 32],
    /// Current joined Custody prestate digest.
    pub custody_prestate_digest: [u8; 32],
    /// Receipt from the selected admitted evaluator.
    pub evaluation_receipt_digest: [u8; 32],
    /// Candidate bank selected by that receipt.
    pub candidate_bank_digest: [u8; 32],
    /// Candidate obligation bytes to write last.
    pub candidate_obligation_digest: [u8; 32],
    /// Claims delta which the immediate child receipt must confirm.
    pub claims_delta_digest: [u8; 32],
    /// Ordered active Custody effect commitment.
    pub effects_digest: [u8; 32],
}

/// Authenticated durable Dealer scenario preparation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerScenarioCheckpointV1 {
    input: DealerScenarioCheckpointInputV1,
    phase: DealerScenarioCheckpointPhaseV1,
    next_page: u8,
    revision: u64,
    last_checkpoint_prestate_digest: [u8; 32],
    page_receipt_digests: [[u8; 32]; DEALER_SCENARIO_PREPARATION_PAGES_V1],
    evaluation: DealerScenarioEvaluationV1,
}

/// Stable refusal from hostile decoding or the preparation phase machine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerScenarioCheckpointErrorV1 {
    /// Fixed-layout bytes were malformed.
    Codec(CodecError),
    /// A required identity, digest, count, or slot was not canonical.
    Coordinate,
    /// Phase bytes or phase-specific fields differed.
    Phase,
    /// A page was repeated, skipped, reordered, or appended after evaluation.
    Replay,
    /// Evaluation or commit occurred after expiry, or cleanup occurred early.
    Expiry,
    /// A live prestate or candidate commitment differed from the checkpoint.
    Substitution,
    /// Checked revision arithmetic overflowed.
    Arithmetic,
}

impl From<CodecError> for DealerScenarioCheckpointErrorV1 {
    fn from(value: CodecError) -> Self {
        Self::Codec(value)
    }
}

/// Result alias for the preparation checkpoint.
pub type CheckpointResultV1<T> = core::result::Result<T, DealerScenarioCheckpointErrorV1>;

impl DealerScenarioCheckpointV1 {
    /// Create one canonical collecting checkpoint.
    pub fn new(input: DealerScenarioCheckpointInputV1) -> CheckpointResultV1<Self> {
        validate_input(input)?;
        Ok(Self {
            input,
            phase: DealerScenarioCheckpointPhaseV1::Collecting,
            next_page: 0,
            revision: 0,
            last_checkpoint_prestate_digest: [0; 32],
            page_receipt_digests: [[0; 32]; DEALER_SCENARIO_PREPARATION_PAGES_V1],
            evaluation: empty_evaluation(),
        })
    }

    /// Hostile-decode one exact checkpoint body.
    pub fn decode(bytes: &[u8]) -> CheckpointResultV1<Self> {
        if bytes.len() != DEALER_SCENARIO_CHECKPOINT_BYTES_V1 {
            return Err(DealerScenarioCheckpointErrorV1::Codec(
                CodecError::InvalidLength,
            ));
        }
        if bytes.get(..8) != Some(DEALER_SCENARIO_CHECKPOINT_MAGIC_V1.as_slice()) {
            return Err(DealerScenarioCheckpointErrorV1::Codec(
                CodecError::InvalidMagic,
            ));
        }
        if read_u16(bytes, VERSION_OFFSET)? != DEALER_SCENARIO_CHECKPOINT_VERSION_V1 {
            return Err(DealerScenarioCheckpointErrorV1::Codec(
                CodecError::UnsupportedVersion,
            ));
        }
        require_zero(bytes, RESERVED_OFFSET, RESERVED_BYTES)?;
        let mut page_receipt_digests = [[0; 32]; DEALER_SCENARIO_PREPARATION_PAGES_V1];
        for (index, digest) in page_receipt_digests.iter_mut().enumerate() {
            *digest = array_at(bytes, PAGE_RECEIPT_DIGESTS_OFFSET + index * 32)?;
        }
        if byte_at(bytes, PAGE_COUNT_OFFSET)?
            != u8::try_from(DEALER_SCENARIO_PREPARATION_PAGES_V1)
                .map_err(|_| DealerScenarioCheckpointErrorV1::Arithmetic)?
        {
            return Err(DealerScenarioCheckpointErrorV1::Coordinate);
        }
        let checkpoint = Self {
            input: DealerScenarioCheckpointInputV1 {
                release_set: array_at(bytes, RELEASE_SET_OFFSET)?,
                market: array_at(bytes, MARKET_OFFSET)?,
                child_root: array_at(bytes, CHILD_ROOT_OFFSET)?,
                obligation: array_at(bytes, OBLIGATION_OFFSET)?,
                refund_beneficiary: array_at(bytes, REFUND_BENEFICIARY_OFFSET)?,
                request_digest: array_at(bytes, REQUEST_DIGEST_OFFSET)?,
                root_prestate_digest: array_at(bytes, ROOT_PRESTATE_DIGEST_OFFSET)?,
                claims_prestate_digest: array_at(bytes, CLAIMS_PRESTATE_DIGEST_OFFSET)?,
                obligation_prestate_digest: array_at(bytes, OBLIGATION_PRESTATE_DIGEST_OFFSET)?,
                custody_prestate_digest: array_at(bytes, CUSTODY_PRESTATE_DIGEST_OFFSET)?,
                generation: u64_at(bytes, GENERATION_OFFSET)?,
                created_slot: u64_at(bytes, CREATED_SLOT_OFFSET)?,
                expires_at: u64_at(bytes, EXPIRES_AT_OFFSET)?,
            },
            phase: DealerScenarioCheckpointPhaseV1::decode(byte_at(bytes, PHASE_OFFSET)?)?,
            next_page: byte_at(bytes, NEXT_PAGE_OFFSET)?,
            revision: u64_at(bytes, REVISION_OFFSET)?,
            last_checkpoint_prestate_digest: array_at(
                bytes,
                LAST_CHECKPOINT_PRESTATE_DIGEST_OFFSET,
            )?,
            page_receipt_digests,
            evaluation: DealerScenarioEvaluationV1 {
                evaluation_receipt_digest: array_at(bytes, EVALUATION_RECEIPT_DIGEST_OFFSET)?,
                candidate_bank_digest: array_at(bytes, CANDIDATE_BANK_DIGEST_OFFSET)?,
                candidate_obligation_digest: array_at(bytes, CANDIDATE_OBLIGATION_DIGEST_OFFSET)?,
                claims_delta_digest: array_at(bytes, CLAIMS_DELTA_DIGEST_OFFSET)?,
                effects_digest: array_at(bytes, EFFECTS_DIGEST_OFFSET)?,
            },
        };
        checkpoint.validate()?;
        Ok(checkpoint)
    }

    /// Encode one exact checkpoint body.
    pub fn to_bytes(self) -> CheckpointResultV1<[u8; DEALER_SCENARIO_CHECKPOINT_BYTES_V1]> {
        self.validate()?;
        let mut bytes = [0_u8; DEALER_SCENARIO_CHECKPOINT_BYTES_V1];
        put(&mut bytes, 0, &DEALER_SCENARIO_CHECKPOINT_MAGIC_V1)?;
        put(
            &mut bytes,
            VERSION_OFFSET,
            &DEALER_SCENARIO_CHECKPOINT_VERSION_V1.to_le_bytes(),
        )?;
        put_byte(&mut bytes, PHASE_OFFSET, self.phase as u8)?;
        put_byte(
            &mut bytes,
            PAGE_COUNT_OFFSET,
            u8::try_from(DEALER_SCENARIO_PREPARATION_PAGES_V1)
                .map_err(|_| DealerScenarioCheckpointErrorV1::Arithmetic)?,
        )?;
        put_byte(&mut bytes, NEXT_PAGE_OFFSET, self.next_page)?;
        put_u64(&mut bytes, REVISION_OFFSET, self.revision)?;
        put_u64(&mut bytes, GENERATION_OFFSET, self.input.generation)?;
        put_u64(&mut bytes, CREATED_SLOT_OFFSET, self.input.created_slot)?;
        put_u64(&mut bytes, EXPIRES_AT_OFFSET, self.input.expires_at)?;
        for (offset, value) in [
            (RELEASE_SET_OFFSET, self.input.release_set),
            (MARKET_OFFSET, self.input.market),
            (CHILD_ROOT_OFFSET, self.input.child_root),
            (OBLIGATION_OFFSET, self.input.obligation),
            (REFUND_BENEFICIARY_OFFSET, self.input.refund_beneficiary),
            (REQUEST_DIGEST_OFFSET, self.input.request_digest),
            (ROOT_PRESTATE_DIGEST_OFFSET, self.input.root_prestate_digest),
            (
                CLAIMS_PRESTATE_DIGEST_OFFSET,
                self.input.claims_prestate_digest,
            ),
            (
                OBLIGATION_PRESTATE_DIGEST_OFFSET,
                self.input.obligation_prestate_digest,
            ),
            (
                CUSTODY_PRESTATE_DIGEST_OFFSET,
                self.input.custody_prestate_digest,
            ),
            (
                LAST_CHECKPOINT_PRESTATE_DIGEST_OFFSET,
                self.last_checkpoint_prestate_digest,
            ),
            (
                EVALUATION_RECEIPT_DIGEST_OFFSET,
                self.evaluation.evaluation_receipt_digest,
            ),
            (
                CANDIDATE_BANK_DIGEST_OFFSET,
                self.evaluation.candidate_bank_digest,
            ),
            (
                CANDIDATE_OBLIGATION_DIGEST_OFFSET,
                self.evaluation.candidate_obligation_digest,
            ),
            (
                CLAIMS_DELTA_DIGEST_OFFSET,
                self.evaluation.claims_delta_digest,
            ),
            (EFFECTS_DIGEST_OFFSET, self.evaluation.effects_digest),
        ] {
            put(&mut bytes, offset, &value)?;
        }
        for (index, digest) in self.page_receipt_digests.iter().enumerate() {
            put(&mut bytes, PAGE_RECEIPT_DIGESTS_OFFSET + index * 32, digest)?;
        }
        Ok(bytes)
    }

    /// Append the next authenticated page receipt.
    ///
    /// `checkpoint_prestate_digest` must be computed by the adapter from the
    /// exact current account bytes, never accepted from instruction data.
    pub fn append_page(
        self,
        current_slot: u64,
        page_index: u8,
        checkpoint_prestate_digest: [u8; 32],
        page_receipt_digest: [u8; 32],
    ) -> CheckpointResultV1<Self> {
        self.require_live_collecting(current_slot)?;
        if page_index != self.next_page
            || usize::from(page_index) >= DEALER_SCENARIO_PREPARATION_PAGES_V1
        {
            return Err(DealerScenarioCheckpointErrorV1::Replay);
        }
        if checkpoint_prestate_digest == [0; 32] || page_receipt_digest == [0; 32] {
            return Err(DealerScenarioCheckpointErrorV1::Coordinate);
        }
        let mut next = self;
        let destination = next
            .page_receipt_digests
            .get_mut(usize::from(page_index))
            .ok_or(DealerScenarioCheckpointErrorV1::Replay)?;
        if *destination != [0; 32] {
            return Err(DealerScenarioCheckpointErrorV1::Replay);
        }
        *destination = page_receipt_digest;
        next.last_checkpoint_prestate_digest = checkpoint_prestate_digest;
        next.next_page = next
            .next_page
            .checked_add(1)
            .ok_or(DealerScenarioCheckpointErrorV1::Arithmetic)?;
        next.revision = next
            .revision
            .checked_add(1)
            .ok_or(DealerScenarioCheckpointErrorV1::Arithmetic)?;
        next.validate()?;
        Ok(next)
    }

    /// Seal the selected admitted evaluation after all page receipts exist.
    pub fn finish_evaluation(
        self,
        current_slot: u64,
        checkpoint_prestate_digest: [u8; 32],
        claims_prestate_digest: [u8; 32],
        custody_prestate_digest: [u8; 32],
        evaluation: DealerScenarioEvaluationV1,
    ) -> CheckpointResultV1<Self> {
        self.require_live_collecting(current_slot)?;
        if usize::from(self.next_page) != DEALER_SCENARIO_PREPARATION_PAGES_V1 {
            return Err(DealerScenarioCheckpointErrorV1::Phase);
        }
        if checkpoint_prestate_digest == [0; 32]
            || claims_prestate_digest == [0; 32]
            || custody_prestate_digest == [0; 32]
            || !evaluation_is_complete(evaluation)
        {
            return Err(DealerScenarioCheckpointErrorV1::Coordinate);
        }
        let mut next = self;
        next.last_checkpoint_prestate_digest = checkpoint_prestate_digest;
        next.input.claims_prestate_digest = claims_prestate_digest;
        next.input.custody_prestate_digest = custody_prestate_digest;
        next.evaluation = evaluation;
        next.phase = DealerScenarioCheckpointPhaseV1::Evaluated;
        next.revision = next
            .revision
            .checked_add(1)
            .ok_or(DealerScenarioCheckpointErrorV1::Arithmetic)?;
        next.validate()?;
        Ok(next)
    }

    /// Require that a final atomic commit still observes the prepared world.
    ///
    /// Success does not persist a `Committed` phase. The adapter must execute
    /// every child effect, verify immediate receipts and poststates, write the
    /// obligation last, then close this checkpoint in the same transaction.
    pub fn admit_commit(
        self,
        current_slot: u64,
        evidence: DealerScenarioCommitEvidenceV1,
    ) -> CheckpointResultV1<()> {
        if self.phase != DealerScenarioCheckpointPhaseV1::Evaluated {
            return Err(DealerScenarioCheckpointErrorV1::Phase);
        }
        self.require_live_slot(current_slot)?;
        if evidence.request_digest != self.input.request_digest
            || evidence.root_prestate_digest != self.input.root_prestate_digest
            || evidence.claims_prestate_digest != self.input.claims_prestate_digest
            || evidence.obligation_prestate_digest != self.input.obligation_prestate_digest
            || evidence.custody_prestate_digest != self.input.custody_prestate_digest
            || evidence.evaluation_receipt_digest != self.evaluation.evaluation_receipt_digest
            || evidence.candidate_bank_digest != self.evaluation.candidate_bank_digest
            || evidence.candidate_obligation_digest != self.evaluation.candidate_obligation_digest
            || evidence.claims_delta_digest != self.evaluation.claims_delta_digest
            || evidence.effects_digest != self.evaluation.effects_digest
        {
            return Err(DealerScenarioCheckpointErrorV1::Substitution);
        }
        Ok(())
    }

    /// Admit permissionless cleanup only after the checkpoint expires.
    ///
    /// The returned beneficiary is immutable; a cleanup caller cannot redirect
    /// rent or any other lamports.
    pub fn cleanup_beneficiary(self, current_slot: u64) -> CheckpointResultV1<[u8; 32]> {
        if current_slot <= self.input.expires_at {
            return Err(DealerScenarioCheckpointErrorV1::Expiry);
        }
        Ok(self.input.refund_beneficiary)
    }

    /// Immutable checkpoint facts.
    pub const fn input(self) -> DealerScenarioCheckpointInputV1 {
        self.input
    }

    /// Current phase.
    pub const fn phase(self) -> DealerScenarioCheckpointPhaseV1 {
        self.phase
    }

    /// Next canonical page ordinal.
    pub const fn next_page(self) -> u8 {
        self.next_page
    }

    /// Monotone checkpoint revision.
    pub const fn revision(self) -> u64 {
        self.revision
    }

    /// Page receipt digest at one canonical ordinal.
    pub fn page_receipt_digest(self, index: u8) -> CheckpointResultV1<[u8; 32]> {
        self.page_receipt_digests
            .get(usize::from(index))
            .copied()
            .ok_or(DealerScenarioCheckpointErrorV1::Coordinate)
    }

    /// Sealed evaluation commitments, zero while collecting.
    pub const fn evaluation(self) -> DealerScenarioEvaluationV1 {
        self.evaluation
    }

    fn require_live_collecting(self, current_slot: u64) -> CheckpointResultV1<()> {
        if self.phase != DealerScenarioCheckpointPhaseV1::Collecting {
            return Err(DealerScenarioCheckpointErrorV1::Phase);
        }
        self.require_live_slot(current_slot)
    }

    fn require_live_slot(self, current_slot: u64) -> CheckpointResultV1<()> {
        if current_slot < self.input.created_slot || current_slot > self.input.expires_at {
            Err(DealerScenarioCheckpointErrorV1::Expiry)
        } else {
            Ok(())
        }
    }

    fn validate(self) -> CheckpointResultV1<()> {
        validate_immutable_input(self.input)?;
        if usize::from(self.next_page) > DEALER_SCENARIO_PREPARATION_PAGES_V1 {
            return Err(DealerScenarioCheckpointErrorV1::Phase);
        }
        for (index, digest) in self.page_receipt_digests.iter().enumerate() {
            let populated = index < usize::from(self.next_page);
            if populated == (*digest == [0; 32]) {
                return Err(DealerScenarioCheckpointErrorV1::Phase);
            }
        }
        match self.phase {
            DealerScenarioCheckpointPhaseV1::Collecting => {
                if self.revision != u64::from(self.next_page)
                    || self.input.claims_prestate_digest != [0; 32]
                    || self.input.custody_prestate_digest != [0; 32]
                    || evaluation_is_complete(self.evaluation)
                    || self.evaluation != empty_evaluation()
                    || (self.next_page == 0 && self.last_checkpoint_prestate_digest != [0; 32])
                    || (self.next_page != 0 && self.last_checkpoint_prestate_digest == [0; 32])
                {
                    return Err(DealerScenarioCheckpointErrorV1::Phase);
                }
            }
            DealerScenarioCheckpointPhaseV1::Evaluated => {
                if usize::from(self.next_page) != DEALER_SCENARIO_PREPARATION_PAGES_V1
                    || self.revision
                        != u64::try_from(DEALER_SCENARIO_PREPARATION_PAGES_V1)
                            .map_err(|_| DealerScenarioCheckpointErrorV1::Arithmetic)?
                            + 1
                    || self.last_checkpoint_prestate_digest == [0; 32]
                    || self.input.claims_prestate_digest == [0; 32]
                    || self.input.custody_prestate_digest == [0; 32]
                    || !evaluation_is_complete(self.evaluation)
                {
                    return Err(DealerScenarioCheckpointErrorV1::Phase);
                }
            }
        }
        Ok(())
    }
}

fn validate_input(input: DealerScenarioCheckpointInputV1) -> CheckpointResultV1<()> {
    validate_immutable_input(input)?;
    if input.claims_prestate_digest != [0; 32] || input.custody_prestate_digest != [0; 32] {
        return Err(DealerScenarioCheckpointErrorV1::Coordinate);
    }
    Ok(())
}

fn validate_immutable_input(input: DealerScenarioCheckpointInputV1) -> CheckpointResultV1<()> {
    if input.created_slot >= input.expires_at
        || [
            input.release_set,
            input.market,
            input.child_root,
            input.obligation,
            input.refund_beneficiary,
            input.request_digest,
            input.root_prestate_digest,
            input.obligation_prestate_digest,
        ]
        .contains(&[0; 32])
    {
        Err(DealerScenarioCheckpointErrorV1::Coordinate)
    } else {
        Ok(())
    }
}

const fn empty_evaluation() -> DealerScenarioEvaluationV1 {
    DealerScenarioEvaluationV1 {
        evaluation_receipt_digest: [0; 32],
        candidate_bank_digest: [0; 32],
        candidate_obligation_digest: [0; 32],
        claims_delta_digest: [0; 32],
        effects_digest: [0; 32],
    }
}

fn evaluation_is_complete(value: DealerScenarioEvaluationV1) -> bool {
    ![
        value.evaluation_receipt_digest,
        value.candidate_bank_digest,
        value.candidate_obligation_digest,
        value.claims_delta_digest,
        value.effects_digest,
    ]
    .contains(&[0; 32])
}

fn read_u16(bytes: &[u8], offset: usize) -> CheckpointResultV1<u16> {
    let end = offset
        .checked_add(2)
        .ok_or(DealerScenarioCheckpointErrorV1::Arithmetic)?;
    let source = bytes
        .get(offset..end)
        .ok_or(DealerScenarioCheckpointErrorV1::Codec(
            CodecError::InvalidLength,
        ))?;
    let mut value = [0_u8; 2];
    value.copy_from_slice(source);
    Ok(u16::from_le_bytes(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn id(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    const fn input() -> DealerScenarioCheckpointInputV1 {
        DealerScenarioCheckpointInputV1 {
            release_set: id(1),
            market: id(2),
            child_root: id(3),
            obligation: id(4),
            refund_beneficiary: id(5),
            request_digest: id(6),
            root_prestate_digest: id(7),
            claims_prestate_digest: [0; 32],
            obligation_prestate_digest: id(9),
            custody_prestate_digest: [0; 32],
            generation: 11,
            created_slot: 20,
            expires_at: 40,
        }
    }

    const fn evaluation() -> DealerScenarioEvaluationV1 {
        DealerScenarioEvaluationV1 {
            evaluation_receipt_digest: id(21),
            candidate_bank_digest: id(22),
            candidate_obligation_digest: id(23),
            claims_delta_digest: id(24),
            effects_digest: id(25),
        }
    }

    fn evaluated() -> DealerScenarioCheckpointV1 {
        let mut checkpoint = DealerScenarioCheckpointV1::new(input()).expect("new");
        for page in 0..6 {
            checkpoint = checkpoint
                .append_page(21 + u64::from(page), page, id(40 + page), id(50 + page))
                .expect("canonical page");
        }
        checkpoint
            .finish_evaluation(30, id(60), id(8), id(10), evaluation())
            .expect("evaluation")
    }

    fn evidence() -> DealerScenarioCommitEvidenceV1 {
        DealerScenarioCommitEvidenceV1 {
            request_digest: id(6),
            root_prestate_digest: id(7),
            claims_prestate_digest: id(8),
            obligation_prestate_digest: id(9),
            custody_prestate_digest: id(10),
            evaluation_receipt_digest: id(21),
            candidate_bank_digest: id(22),
            candidate_obligation_digest: id(23),
            claims_delta_digest: id(24),
            effects_digest: id(25),
        }
    }

    #[test]
    fn six_pages_resume_then_commit_against_exact_prestates() {
        let checkpoint = evaluated();
        assert_eq!(
            checkpoint.phase(),
            DealerScenarioCheckpointPhaseV1::Evaluated
        );
        assert_eq!(checkpoint.revision(), 7);
        assert_eq!(checkpoint.next_page(), 6);
        assert_eq!(checkpoint.admit_commit(40, evidence()), Ok(()));
        let bytes = checkpoint.to_bytes().expect("bytes");
        assert_eq!(DealerScenarioCheckpointV1::decode(&bytes), Ok(checkpoint));
    }

    #[test]
    fn reorder_replay_and_mixed_page_receipts_refuse() {
        let checkpoint = DealerScenarioCheckpointV1::new(input()).expect("new");
        assert_eq!(
            checkpoint.append_page(21, 1, id(30), id(31)),
            Err(DealerScenarioCheckpointErrorV1::Replay)
        );
        let checkpoint = checkpoint
            .append_page(21, 0, id(30), id(31))
            .expect("page zero");
        assert_eq!(
            checkpoint.append_page(22, 0, id(32), id(33)),
            Err(DealerScenarioCheckpointErrorV1::Replay)
        );
        let mut bytes = checkpoint.to_bytes().expect("bytes");
        bytes[PAGE_RECEIPT_DIGESTS_OFFSET + 32] = 1;
        assert_eq!(
            DealerScenarioCheckpointV1::decode(&bytes),
            Err(DealerScenarioCheckpointErrorV1::Phase)
        );
    }

    #[test]
    fn evaluation_requires_every_page_and_is_commit_last() {
        let checkpoint = DealerScenarioCheckpointV1::new(input()).expect("new");
        assert_eq!(
            checkpoint.finish_evaluation(21, id(30), id(8), id(10), evaluation()),
            Err(DealerScenarioCheckpointErrorV1::Phase)
        );
        let evaluated = evaluated();
        assert_eq!(
            evaluated.append_page(31, 0, id(30), id(31)),
            Err(DealerScenarioCheckpointErrorV1::Phase)
        );
        assert_eq!(
            evaluated.finish_evaluation(31, id(32), id(8), id(10), evaluation()),
            Err(DealerScenarioCheckpointErrorV1::Phase)
        );
    }

    #[test]
    fn every_commit_digest_is_substitution_sensitive() {
        let checkpoint = evaluated();
        for index in 0..10 {
            let mut hostile = evidence();
            match index {
                0 => hostile.request_digest = id(90),
                1 => hostile.root_prestate_digest = id(90),
                2 => hostile.claims_prestate_digest = id(90),
                3 => hostile.obligation_prestate_digest = id(90),
                4 => hostile.custody_prestate_digest = id(90),
                5 => hostile.evaluation_receipt_digest = id(90),
                6 => hostile.candidate_bank_digest = id(90),
                7 => hostile.candidate_obligation_digest = id(90),
                8 => hostile.claims_delta_digest = id(90),
                9 => hostile.effects_digest = id(90),
                _ => unreachable!(),
            }
            assert_eq!(
                checkpoint.admit_commit(31, hostile),
                Err(DealerScenarioCheckpointErrorV1::Substitution)
            );
        }
    }

    #[test]
    fn expiry_blocks_forward_progress_but_enables_fixed_refund_cleanup() {
        let checkpoint = DealerScenarioCheckpointV1::new(input()).expect("new");
        assert_eq!(
            checkpoint.append_page(41, 0, id(30), id(31)),
            Err(DealerScenarioCheckpointErrorV1::Expiry)
        );
        assert_eq!(
            checkpoint.cleanup_beneficiary(40),
            Err(DealerScenarioCheckpointErrorV1::Expiry)
        );
        assert_eq!(checkpoint.cleanup_beneficiary(41), Ok(id(5)));

        let evaluated = evaluated();
        assert_eq!(
            evaluated.admit_commit(41, evidence()),
            Err(DealerScenarioCheckpointErrorV1::Expiry)
        );
        assert_eq!(evaluated.cleanup_beneficiary(41), Ok(id(5)));
    }
}
