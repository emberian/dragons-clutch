//! Runtime-width best-valid-submitted-candidate selection.
//!
//! This module owns the one canonical General successor selection cursor. It
//! is a stateless, allocation-free semantic transition: Generic Trading
//! authenticates the selected policy and candidate accounts, evaluates this
//! transition into scratch, and persists the exact candidate bytes through
//! the common lifecycle/effect machinery. The evaluator owns no accounts and
//! performs no CPI.

use sha2::{Digest, Sha256};

use dclutch_general_codec::SelectionPolicyV1;

use crate::{
    runtime_verify::{RuntimeVerifyErrorV2, runtime_candidate_better_v2},
    runtime_width::{VerifiedCandidateV2, verified_candidate_len},
};

/// Exact byte width of a successor General selection cursor.
pub const RUNTIME_SELECTION_CURSOR_BYTES_V2: usize = 208;

const MAGIC: [u8; 8] = *b"DCGSEL02";
const VERSION: u16 = 2;
const PHASE_OPEN: u8 = 1;
const PHASE_FROZEN: u8 = 2;

/// Typed canonical offsets consumed by the generic EffectProgram artifact.
///
/// Keeping these coordinates beside the hostile decoder prevents an artifact
/// generator from becoming a second owner of the persisted wire layout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeSelectionLayoutV2;

impl RuntimeSelectionLayoutV2 {
    /// Cursor magic interpreted as one little-endian scalar.
    pub const fn magic_u64() -> u64 {
        u64::from_le_bytes(MAGIC)
    }

    /// Exact cursor ABI version.
    pub const fn version_value() -> u16 {
        VERSION
    }

    /// Magic byte offset.
    pub const fn magic() -> u32 {
        0
    }

    /// Version byte offset.
    pub const fn version() -> u32 {
        8
    }

    /// Open/frozen phase byte offset.
    pub const fn phase() -> u32 {
        10
    }

    /// Product-derived outcome-count byte offset.
    pub const fn outcome_count() -> u32 {
        12
    }

    /// Optimistic selection revision byte offset.
    pub const fn revision() -> u32 {
        16
    }

    /// Count of distinct submitted certificates considered.
    pub const fn submitted_count() -> u32 {
        24
    }

    /// Coordinate of the selected Candidate in its immutable Batch.
    pub const fn best_candidate_coordinate() -> u32 {
        28
    }

    /// Verification revision of the selected certificate.
    pub const fn best_verified_revision() -> u32 {
        32
    }

    /// Exact price denominator shared by the comparison domain.
    pub const fn price_scale() -> u32 {
        40
    }

    /// Product content identity byte offset.
    pub const fn product_id() -> u32 {
        48
    }

    /// Batch content identity byte offset.
    pub const fn batch_id() -> u32 {
        80
    }

    /// Interpreted selection-policy content identity byte offset.
    pub const fn policy_id() -> u32 {
        112
    }

    /// Best valid submitted Candidate identity byte offset.
    pub const fn best_candidate_id() -> u32 {
        144
    }

    /// Digest of the exact selected verified-candidate record.
    pub const fn best_verified_digest() -> u32 {
        176
    }
}

/// Selection progress phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeSelectionPhaseV2 {
    /// More verified candidates may be considered.
    Open,
    /// Permissionless freeze selected the final best submitted candidate.
    Frozen,
}

impl RuntimeSelectionPhaseV2 {
    const fn tag(self) -> u8 {
        match self {
            Self::Open => PHASE_OPEN,
            Self::Frozen => PHASE_FROZEN,
        }
    }

    fn decode(tag: u8) -> Result<Self> {
        match tag {
            PHASE_OPEN => Ok(Self::Open),
            PHASE_FROZEN => Ok(Self::Frozen),
            _ => Err(RuntimeSelectionErrorV2::InvalidPhase),
        }
    }
}

/// Fixed facts from one canonical runtime selection cursor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeSelectionHeaderV2 {
    /// Product-derived runtime outcome count.
    pub outcome_count: u32,
    /// Exact optimistic revision.
    pub revision: u64,
    /// Number of distinct submitted certificates considered.
    pub submitted_count: u32,
    /// Immutable Candidate coordinate of the current best submission.
    pub best_candidate_coordinate: u32,
    /// Verification revision of the current best certificate.
    pub best_verified_revision: u64,
    /// Exact price denominator shared by the comparison domain.
    pub price_scale: u64,
    /// Product content identity.
    pub product_id: [u8; 32],
    /// Batch content identity.
    pub batch_id: [u8; 32],
    /// Immutable interpreted selection-policy identity.
    pub policy_id: [u8; 32],
    /// Best valid submitted Candidate identity.
    pub best_candidate_id: [u8; 32],
    /// SHA-256 digest of the exact selected verified-candidate bytes.
    pub best_verified_digest: [u8; 32],
    /// Whether selection is still open or frozen.
    pub phase: RuntimeSelectionPhaseV2,
}

/// Borrowed hostile-decoded successor selection cursor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeSelectionCursorV2<'a> {
    bytes: &'a [u8],
    header: RuntimeSelectionHeaderV2,
}

impl<'a> RuntimeSelectionCursorV2<'a> {
    /// Decode one exact canonical selection cursor.
    pub fn decode(bytes: &'a [u8]) -> Result<Self> {
        if bytes.len() != RUNTIME_SELECTION_CURSOR_BYTES_V2
            || bytes.get(..8) != Some(MAGIC.as_slice())
            || read_u16(bytes, 8)? != VERSION
            || byte(bytes, 11)? != 0
        {
            return Err(RuntimeSelectionErrorV2::InvalidEncoding);
        }
        let header = RuntimeSelectionHeaderV2 {
            outcome_count: read_u32(bytes, 12)?,
            revision: read_u64(bytes, 16)?,
            submitted_count: read_u32(bytes, 24)?,
            best_candidate_coordinate: read_u32(bytes, 28)?,
            best_verified_revision: read_u64(bytes, 32)?,
            price_scale: read_u64(bytes, 40)?,
            product_id: read_array32(bytes, 48)?,
            batch_id: read_array32(bytes, 80)?,
            policy_id: read_array32(bytes, 112)?,
            best_candidate_id: read_array32(bytes, 144)?,
            best_verified_digest: read_array32(bytes, 176)?,
            phase: RuntimeSelectionPhaseV2::decode(byte(bytes, 10)?)?,
        };
        validate_header(header)?;
        Ok(Self { bytes, header })
    }

    /// Return the exact fixed selection facts.
    pub const fn header(self) -> RuntimeSelectionHeaderV2 {
        self.header
    }

    /// Return the canonical hostile-decoded bytes.
    pub const fn as_bytes(self) -> &'a [u8] {
        self.bytes
    }
}

/// Stable refusal from successor selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeSelectionErrorV2 {
    /// A state or candidate bank had another exact byte width.
    InvalidLength,
    /// Magic, version, phase, or canonical padding refused.
    InvalidEncoding,
    /// The cursor phase did not admit this transition.
    InvalidPhase,
    /// A required count, revision, or content identity was zero.
    ZeroCoordinate,
    /// Candidate, Product, Batch, policy, or certificate facts were substituted.
    Substitution,
    /// Optimistic revision differed from the persisted cursor.
    RevisionMismatch,
    /// The exact same submitted certificate was replayed.
    DuplicateCandidate,
    /// Checked revision or submission-count arithmetic overflowed.
    ArithmeticOverflow,
    /// Runtime candidate comparison refused.
    Comparison,
}

/// Result alias for successor selection transitions.
pub type Result<T> = core::result::Result<T, RuntimeSelectionErrorV2>;

/// Evaluate one verified submission into an exact selection candidate.
///
/// `cursor_before` is either the exact all-zero vacant state or a canonical
/// open cursor. Existing cursors require the exact incumbent certificate whose
/// digest they persist. Scratch may change on refusal; output never does.
pub fn consider_verified_candidate_v2(
    policy: SelectionPolicyV1,
    cursor_before: &[u8],
    incumbent_verified: Option<&[u8]>,
    submitted_verified: &[u8],
    expected_revision: u64,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<()> {
    exact_state_widths(cursor_before, scratch, output)?;
    let submitted = VerifiedCandidateV2::decode(submitted_verified)
        .map_err(|_| RuntimeSelectionErrorV2::Substitution)?;
    let submitted_header = submitted.header();
    if submitted_verified.len()
        != verified_candidate_len(submitted_header.outcome_count)
            .map_err(|_| RuntimeSelectionErrorV2::InvalidLength)?
    {
        return Err(RuntimeSelectionErrorV2::InvalidLength);
    }
    let submitted_digest = digest(submitted_verified);
    let vacant = cursor_before.iter().all(|value| *value == 0);
    let (revision, submitted_count, selected_header, selected_digest) = if vacant {
        if expected_revision != 0 || incumbent_verified.is_some() {
            return Err(RuntimeSelectionErrorV2::RevisionMismatch);
        }
        (0, 0, submitted_header, submitted_digest)
    } else {
        let cursor = RuntimeSelectionCursorV2::decode(cursor_before)?;
        let header = cursor.header();
        if header.phase != RuntimeSelectionPhaseV2::Open {
            return Err(RuntimeSelectionErrorV2::InvalidPhase);
        }
        if header.revision != expected_revision {
            return Err(RuntimeSelectionErrorV2::RevisionMismatch);
        }
        if header.policy_id != policy.policy_id
            || header.outcome_count != submitted_header.outcome_count
            || header.product_id != submitted_header.product_id
            || header.batch_id != submitted_header.batch_id
            || header.price_scale != submitted_header.price_scale
        {
            return Err(RuntimeSelectionErrorV2::Substitution);
        }
        let incumbent_bytes = incumbent_verified.ok_or(RuntimeSelectionErrorV2::Substitution)?;
        let incumbent = VerifiedCandidateV2::decode(incumbent_bytes)
            .map_err(|_| RuntimeSelectionErrorV2::Substitution)?;
        let incumbent_header = incumbent.header();
        if digest(incumbent_bytes) != header.best_verified_digest
            || incumbent_header.candidate_id != header.best_candidate_id
            || incumbent_header.candidate_coordinate != header.best_candidate_coordinate
            || incumbent_header.revision != header.best_verified_revision
        {
            return Err(RuntimeSelectionErrorV2::Substitution);
        }
        if submitted_digest == header.best_verified_digest {
            return Err(RuntimeSelectionErrorV2::DuplicateCandidate);
        }
        let better = runtime_candidate_better_v2(&policy, submitted_verified, incumbent_bytes)
            .map_err(map_comparison)?;
        let (selected, selected_digest) = if better {
            (submitted_header, submitted_digest)
        } else {
            (incumbent_header, header.best_verified_digest)
        };
        (
            header.revision,
            header.submitted_count,
            selected,
            selected_digest,
        )
    };
    let next_revision = revision
        .checked_add(1)
        .ok_or(RuntimeSelectionErrorV2::ArithmeticOverflow)?;
    let next_submitted = submitted_count
        .checked_add(1)
        .ok_or(RuntimeSelectionErrorV2::ArithmeticOverflow)?;
    encode_into(
        RuntimeSelectionHeaderV2 {
            outcome_count: selected_header.outcome_count,
            revision: next_revision,
            submitted_count: next_submitted,
            best_candidate_coordinate: selected_header.candidate_coordinate,
            best_verified_revision: selected_header.revision,
            price_scale: selected_header.price_scale,
            product_id: selected_header.product_id,
            batch_id: selected_header.batch_id,
            policy_id: policy.policy_id,
            best_candidate_id: selected_header.candidate_id,
            best_verified_digest: selected_digest,
            phase: RuntimeSelectionPhaseV2::Open,
        },
        scratch,
    )?;
    output.copy_from_slice(scratch);
    Ok(())
}

/// Permissionlessly freeze one nonempty selection at an exact revision.
///
/// Scratch may change on refusal; output remains byte-for-byte unchanged.
pub fn freeze_selection_v2(
    cursor_before: &[u8],
    expected_revision: u64,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<()> {
    exact_state_widths(cursor_before, scratch, output)?;
    let cursor = RuntimeSelectionCursorV2::decode(cursor_before)?;
    let mut header = cursor.header();
    if header.phase != RuntimeSelectionPhaseV2::Open {
        return Err(RuntimeSelectionErrorV2::InvalidPhase);
    }
    if header.revision != expected_revision {
        return Err(RuntimeSelectionErrorV2::RevisionMismatch);
    }
    header.revision = header
        .revision
        .checked_add(1)
        .ok_or(RuntimeSelectionErrorV2::ArithmeticOverflow)?;
    header.phase = RuntimeSelectionPhaseV2::Frozen;
    encode_into(header, scratch)?;
    output.copy_from_slice(scratch);
    Ok(())
}

fn encode_into(header: RuntimeSelectionHeaderV2, output: &mut [u8]) -> Result<()> {
    if output.len() != RUNTIME_SELECTION_CURSOR_BYTES_V2 {
        return Err(RuntimeSelectionErrorV2::InvalidLength);
    }
    validate_header(header)?;
    output.fill(0);
    put(output, 0, &MAGIC)?;
    put(output, 8, &VERSION.to_le_bytes())?;
    put_byte(output, 10, header.phase.tag())?;
    put(output, 12, &header.outcome_count.to_le_bytes())?;
    put(output, 16, &header.revision.to_le_bytes())?;
    put(output, 24, &header.submitted_count.to_le_bytes())?;
    put(output, 28, &header.best_candidate_coordinate.to_le_bytes())?;
    put(output, 32, &header.best_verified_revision.to_le_bytes())?;
    put(output, 40, &header.price_scale.to_le_bytes())?;
    put(output, 48, &header.product_id)?;
    put(output, 80, &header.batch_id)?;
    put(output, 112, &header.policy_id)?;
    put(output, 144, &header.best_candidate_id)?;
    put(output, 176, &header.best_verified_digest)
}

fn validate_header(header: RuntimeSelectionHeaderV2) -> Result<()> {
    if header.outcome_count == 0
        || header.revision == 0
        || header.submitted_count == 0
        || header.best_candidate_coordinate == 0
        || header.best_verified_revision == 0
        || header.price_scale == 0
        || zero(&header.product_id)
        || zero(&header.batch_id)
        || zero(&header.policy_id)
        || zero(&header.best_candidate_id)
        || zero(&header.best_verified_digest)
    {
        return Err(RuntimeSelectionErrorV2::ZeroCoordinate);
    }
    Ok(())
}

fn exact_state_widths(before: &[u8], scratch: &[u8], output: &[u8]) -> Result<()> {
    if before.len() != RUNTIME_SELECTION_CURSOR_BYTES_V2
        || scratch.len() != RUNTIME_SELECTION_CURSOR_BYTES_V2
        || output.len() != RUNTIME_SELECTION_CURSOR_BYTES_V2
    {
        Err(RuntimeSelectionErrorV2::InvalidLength)
    } else {
        Ok(())
    }
}

fn map_comparison(_error: RuntimeVerifyErrorV2) -> RuntimeSelectionErrorV2 {
    RuntimeSelectionErrorV2::Comparison
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn zero(value: &[u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}

fn byte(input: &[u8], offset: usize) -> Result<u8> {
    input
        .get(offset)
        .copied()
        .ok_or(RuntimeSelectionErrorV2::InvalidLength)
}

fn read_u16(input: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(read_array(input, offset)?))
}

fn read_u32(input: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(read_array(input, offset)?))
}

fn read_u64(input: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(read_array(input, offset)?))
}

fn read_array32(input: &[u8], offset: usize) -> Result<[u8; 32]> {
    read_array(input, offset)
}

fn read_array<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N]> {
    input
        .get(
            offset
                ..offset
                    .checked_add(N)
                    .ok_or(RuntimeSelectionErrorV2::InvalidLength)?,
        )
        .ok_or(RuntimeSelectionErrorV2::InvalidLength)?
        .try_into()
        .map_err(|_| RuntimeSelectionErrorV2::InvalidLength)
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<()> {
    output
        .get_mut(
            offset
                ..offset
                    .checked_add(value.len())
                    .ok_or(RuntimeSelectionErrorV2::InvalidLength)?,
        )
        .ok_or(RuntimeSelectionErrorV2::InvalidLength)?
        .copy_from_slice(value);
    Ok(())
}

fn put_byte(output: &mut [u8], offset: usize, value: u8) -> Result<()> {
    *output
        .get_mut(offset)
        .ok_or(RuntimeSelectionErrorV2::InvalidLength)? = value;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::vec;

    use dclutch_general_codec::{MAX_SELECTION_CRITERIA, SelectionCriterion};

    use super::*;
    use crate::runtime_width::{VerifiedCandidateHeaderV2, VerifiedCandidateV2};

    const PRODUCT: [u8; 32] = [1; 32];
    const BATCH: [u8; 32] = [2; 32];
    const POLICY: [u8; 32] = [3; 32];

    fn policy() -> SelectionPolicyV1 {
        let mut criteria = [SelectionCriterion::MaximizeFilledLots; MAX_SELECTION_CRITERIA];
        criteria[1] = SelectionCriterion::MinimizeQuoteSurplus;
        criteria[2] = SelectionCriterion::MinimizeCandidateId;
        SelectionPolicyV1 {
            policy_id: POLICY,
            criterion_count: 3,
            criteria,
        }
    }

    fn verified(width: u32, candidate: u8, coordinate: u32, filled: u64) -> std::vec::Vec<u8> {
        let mut output = vec![0; verified_candidate_len(width).expect("verified width")];
        VerifiedCandidateV2::encode_into(
            VerifiedCandidateHeaderV2 {
                outcome_count: width,
                page_count: 1,
                candidate_coordinate: coordinate,
                revision: 1,
                candidate_id: [candidate; 32],
                product_id: PRODUCT,
                batch_id: BATCH,
                filled_lots: filled,
                quote_debit: filled,
                quote_credit: 0,
                price_scale: 1,
            },
            &vec![filled; usize::try_from(width).expect("width")],
            &vec![filled; usize::try_from(width).expect("width")],
            &mut output,
        )
        .expect("verified encode");
        output
    }

    fn vacant() -> [u8; RUNTIME_SELECTION_CURSOR_BYTES_V2] {
        [0; RUNTIME_SELECTION_CURSOR_BYTES_V2]
    }

    #[test]
    fn selects_best_valid_submitted_candidate_for_runtime_widths() {
        for width in [1_u32, 258] {
            let first = verified(width, 8, 1, 2);
            let better = verified(width, 7, 2, 3);
            let mut scratch = vacant();
            let mut selection = vacant();
            consider_verified_candidate_v2(
                policy(),
                &vacant(),
                None,
                &first,
                0,
                &mut scratch,
                &mut selection,
            )
            .expect("first submission");
            let first_cursor = RuntimeSelectionCursorV2::decode(&selection).expect("selection");
            assert_eq!(first_cursor.header().best_candidate_id, [8; 32]);
            let before = selection;
            consider_verified_candidate_v2(
                policy(),
                &before,
                Some(&first),
                &better,
                1,
                &mut scratch,
                &mut selection,
            )
            .expect("better submission");
            let cursor = RuntimeSelectionCursorV2::decode(&selection).expect("selection");
            assert_eq!(cursor.header().best_candidate_id, [7; 32]);
            assert_eq!(cursor.header().submitted_count, 2);
            assert_eq!(cursor.header().revision, 2);
        }
    }

    #[test]
    fn inferior_submission_advances_once_without_replacing_best() {
        let best = verified(1, 5, 1, 5);
        let inferior = verified(1, 4, 2, 4);
        let mut scratch = vacant();
        let mut selection = vacant();
        consider_verified_candidate_v2(
            policy(),
            &vacant(),
            None,
            &best,
            0,
            &mut scratch,
            &mut selection,
        )
        .expect("first");
        let before = selection;
        consider_verified_candidate_v2(
            policy(),
            &before,
            Some(&best),
            &inferior,
            1,
            &mut scratch,
            &mut selection,
        )
        .expect("inferior remains valid submission");
        let header = RuntimeSelectionCursorV2::decode(&selection)
            .expect("selection")
            .header();
        assert_eq!(header.best_candidate_id, [5; 32]);
        assert_eq!(header.submitted_count, 2);
    }

    #[test]
    fn freeze_is_permissionless_exact_revision_and_failure_atomic() {
        let candidate = verified(1, 7, 1, 3);
        let mut scratch = vacant();
        let mut open = vacant();
        consider_verified_candidate_v2(
            policy(),
            &vacant(),
            None,
            &candidate,
            0,
            &mut scratch,
            &mut open,
        )
        .expect("open");
        let mut frozen = [0x55; RUNTIME_SELECTION_CURSOR_BYTES_V2];
        let before = frozen;
        assert_eq!(
            freeze_selection_v2(&open, 0, &mut scratch, &mut frozen),
            Err(RuntimeSelectionErrorV2::RevisionMismatch)
        );
        assert_eq!(frozen, before);
        freeze_selection_v2(&open, 1, &mut scratch, &mut frozen).expect("freeze");
        let cursor = RuntimeSelectionCursorV2::decode(&frozen).expect("frozen");
        assert_eq!(cursor.header().phase, RuntimeSelectionPhaseV2::Frozen);
        assert_eq!(cursor.header().revision, 2);
        let before = frozen;
        assert_eq!(
            freeze_selection_v2(&before, 2, &mut scratch, &mut frozen),
            Err(RuntimeSelectionErrorV2::InvalidPhase)
        );
        assert_eq!(frozen, before);
    }

    #[test]
    fn substitution_duplicate_and_noncanonical_padding_refuse_atomically() {
        let first = verified(1, 7, 1, 3);
        let second = verified(1, 6, 2, 4);
        let mut scratch = vacant();
        let mut selection = vacant();
        consider_verified_candidate_v2(
            policy(),
            &vacant(),
            None,
            &first,
            0,
            &mut scratch,
            &mut selection,
        )
        .expect("first");
        let open = selection;
        let output_before = [0x66; RUNTIME_SELECTION_CURSOR_BYTES_V2];
        let mut output = output_before;
        assert_eq!(
            consider_verified_candidate_v2(
                policy(),
                &open,
                Some(&first),
                &first,
                1,
                &mut scratch,
                &mut output,
            ),
            Err(RuntimeSelectionErrorV2::DuplicateCandidate)
        );
        assert_eq!(output, output_before);

        let mut substituted = first.clone();
        *substituted.last_mut().expect("tail") ^= 1;
        assert_eq!(
            consider_verified_candidate_v2(
                policy(),
                &open,
                Some(&substituted),
                &second,
                1,
                &mut scratch,
                &mut output,
            ),
            Err(RuntimeSelectionErrorV2::Substitution)
        );
        assert_eq!(output, output_before);

        let mut hostile = open;
        hostile[11] = 1;
        assert_eq!(
            RuntimeSelectionCursorV2::decode(&hostile),
            Err(RuntimeSelectionErrorV2::InvalidEncoding)
        );
    }

    #[test]
    fn typed_layout_offsets_roundtrip_through_decoder() {
        let candidate = verified(1, 7, 9, 3);
        let mut scratch = vacant();
        let mut selection = vacant();
        consider_verified_candidate_v2(
            policy(),
            &vacant(),
            None,
            &candidate,
            0,
            &mut scratch,
            &mut selection,
        )
        .expect("selection");
        assert_eq!(RuntimeSelectionLayoutV2::magic(), 0);
        assert_eq!(RuntimeSelectionLayoutV2::version(), 8);
        assert_eq!(
            selection[RuntimeSelectionLayoutV2::phase() as usize],
            PHASE_OPEN
        );
        assert_eq!(
            read_u32(
                &selection,
                RuntimeSelectionLayoutV2::best_candidate_coordinate() as usize,
            )
            .expect("coordinate"),
            9
        );
        assert_eq!(
            read_array32(
                &selection,
                RuntimeSelectionLayoutV2::best_candidate_id() as usize,
            )
            .expect("identity"),
            [7; 32]
        );
        RuntimeSelectionCursorV2::decode(&selection).expect("canonical decoder");
    }
}
