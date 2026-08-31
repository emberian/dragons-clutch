//! Durable checkpoint for packet-bounded aggregate retirement.
//!
//! The checkpoint occupies the exact emptied Claims aggregate account. Claims
//! proves every liability is zero before handing that account to Core; its
//! original lamports remain in place and retain their original classification
//! as the Claims aggregate refund. No new rent source or refund class exists.

use dclutch_sha256_adapter::digestv;

/// Exact durable checkpoint width. This fits every valid LiabilityBasisV2
/// aggregate, including the minimum two-claim aggregate.
pub const AGGREGATE_RETIREMENT_CHECKPOINT_BYTES_V1: usize = 256;
/// Exact fixed prefix carried by each permissionless suffix instruction.
pub const AGGREGATE_RETIREMENT_SUFFIX_REQUEST_BYTES_V1: usize = 192;
/// Persisted checkpoint magic.
pub const AGGREGATE_RETIREMENT_CHECKPOINT_MAGIC_V1: [u8; 8] = *b"DCLTARC1";
/// Close the HoardPrincipal vault after Claims handoff.
pub const AGGREGATE_RETIREMENT_CLOSE_VAULT_MAGIC_V1: [u8; 8] = *b"DCLTARV1";
/// Close the normal Custody replay after the HoardPrincipal vault.
pub const AGGREGATE_RETIREMENT_CLOSE_REPLAY_MAGIC_V1: [u8; 8] = *b"DCLTARR1";
/// Finish Core and Rent closure after both child suffixes.
pub const AGGREGATE_RETIREMENT_FINISH_MAGIC_V1: [u8; 8] = *b"DCLTARF1";
/// Implemented wire version.
pub const AGGREGATE_RETIREMENT_CHECKPOINT_VERSION_V1: u16 = 1;
/// Receipt-history digest domain.
pub const AGGREGATE_RETIREMENT_HISTORY_DIGEST_DOMAIN_V1: &[u8] =
    b"dclutch/aggregate-retirement-history/v1";
/// Digest domain binding the fields shared by both Custody suffix requests.
pub const AGGREGATE_RETIREMENT_CUSTODY_JOIN_DIGEST_DOMAIN_V1: &[u8] =
    b"dclutch/aggregate-retirement-custody-join/v1";

const CORE_PRESTATE_OFFSET: usize = 16;
const BUNDLE_DIGEST_OFFSET: usize = 48;
const CLAIMS_CONTEXT_OFFSET: usize = 80;
const CLAIMS_RECEIPT_OFFSET: usize = 112;
const VAULT_RECEIPT_OFFSET: usize = 144;
const REPLAY_RECEIPT_OFFSET: usize = 176;
const CLAIMS_REFUND_OFFSET: usize = 208;
const CUSTODY_REFUND_OFFSET: usize = 216;
const GENERATION_OFFSET: usize = 224;
const CLAIMS_REVISION_OFFSET: usize = 232;
const CUSTODY_REVISION_OFFSET: usize = 240;
const PHASE_REVISION_OFFSET: usize = 248;

/// Exact ordered persisted partition of an in-progress aggregate retirement.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AggregateRetirementPhaseV1 {
    /// Claims proved zero liabilities and handed its aggregate to Core.
    ClaimsClosed = 1,
    /// The empty HoardPrincipal vault was closed next.
    HoardVaultClosed = 2,
    /// The normal Custody replay was closed next; only Core/Rent remain.
    CustodyReplayClosed = 3,
}

impl AggregateRetirementPhaseV1 {
    fn decode(value: u8) -> Result<Self, AggregateRetirementCheckpointErrorV1> {
        match value {
            1 => Ok(Self::ClaimsClosed),
            2 => Ok(Self::HoardVaultClosed),
            3 => Ok(Self::CustodyReplayClosed),
            _ => Err(AggregateRetirementCheckpointErrorV1::Phase),
        }
    }
}

/// Hostile decode or non-canonical transition refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AggregateRetirementCheckpointErrorV1 {
    /// Input width was not exact.
    Length,
    /// Magic or version selected another family.
    Header,
    /// Reserved or phase-inactive bytes were nonzero.
    NonCanonical,
    /// A required digest or identity was zero.
    ZeroIdentity,
    /// The persisted phase or requested successor was wrong.
    Phase,
    /// A revision or refund coordinate overflowed or did not advance once.
    Coordinate,
}

/// Result alias for the aggregate-retirement checkpoint.
pub type AggregateRetirementCheckpointResultV1<T> =
    core::result::Result<T, AggregateRetirementCheckpointErrorV1>;

/// Exact input after Claims has atomically handed its empty aggregate to Core.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimsClosedCheckpointInputV1 {
    /// SHA-256 of the unchanged Retiring Core state.
    pub core_prestate_digest: [u8; 32],
    /// SHA-256 of the complete original retirement bundle.
    pub bundle_digest: [u8; 32],
    /// Claims-owned custody context recovered before the aggregate was erased.
    pub claims_context: [u8; 32],
    /// SHA-256 of the exact Claims handoff receipt.
    pub claims_receipt_digest: [u8; 32],
    /// Exact Claims aggregate lamports retained by this checkpoint.
    pub claims_refund_lamports: u64,
    /// Immutable Market generation.
    pub generation: u64,
    /// Claims revision after zero-liability handoff.
    pub claims_revision: u64,
    /// Custody replay revision before either Custody suffix.
    pub custody_revision: u64,
}

/// One exact durable aggregate-retirement checkpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AggregateRetirementCheckpointV1 {
    phase: AggregateRetirementPhaseV1,
    core_prestate_digest: [u8; 32],
    bundle_digest: [u8; 32],
    phase_join_digest: [u8; 32],
    claims_receipt_digest: [u8; 32],
    close_vault_receipt_digest: [u8; 32],
    close_replay_receipt_digest: [u8; 32],
    claims_refund_lamports: u64,
    custody_refund_lamports: u64,
    generation: u64,
    claims_revision: u64,
    custody_revision: u64,
    phase_revision: u64,
}

impl AggregateRetirementCheckpointV1 {
    /// Construct the first persisted phase after the zero-liability handoff.
    pub fn claims_closed(
        input: ClaimsClosedCheckpointInputV1,
    ) -> AggregateRetirementCheckpointResultV1<Self> {
        require_nonzero(&[
            input.core_prestate_digest,
            input.bundle_digest,
            input.claims_context,
            input.claims_receipt_digest,
        ])?;
        if input.claims_refund_lamports == 0
            || input.generation == 0
            || input.claims_revision == 0
            || input.custody_revision == 0
        {
            return Err(AggregateRetirementCheckpointErrorV1::Coordinate);
        }
        Ok(Self {
            phase: AggregateRetirementPhaseV1::ClaimsClosed,
            core_prestate_digest: input.core_prestate_digest,
            bundle_digest: input.bundle_digest,
            phase_join_digest: input.claims_context,
            claims_receipt_digest: input.claims_receipt_digest,
            close_vault_receipt_digest: [0; 32],
            close_replay_receipt_digest: [0; 32],
            claims_refund_lamports: input.claims_refund_lamports,
            custody_refund_lamports: 0,
            generation: input.generation,
            claims_revision: input.claims_revision,
            custody_revision: input.custody_revision,
            phase_revision: 1,
        })
    }

    /// Advance exactly once after closing the HoardPrincipal vault.
    pub fn close_vault(
        self,
        receipt_digest: [u8; 32],
        custody_join_digest: [u8; 32],
        refund_lamports: u64,
        post_custody_revision: u64,
    ) -> AggregateRetirementCheckpointResultV1<Self> {
        if self.phase != AggregateRetirementPhaseV1::ClaimsClosed {
            return Err(AggregateRetirementCheckpointErrorV1::Phase);
        }
        require_nonzero(&[receipt_digest, custody_join_digest])?;
        let expected_revision = self
            .custody_revision
            .checked_add(1)
            .ok_or(AggregateRetirementCheckpointErrorV1::Coordinate)?;
        if post_custody_revision != expected_revision || refund_lamports == 0 {
            return Err(AggregateRetirementCheckpointErrorV1::Coordinate);
        }
        Ok(Self {
            phase: AggregateRetirementPhaseV1::HoardVaultClosed,
            phase_join_digest: custody_join_digest,
            close_vault_receipt_digest: receipt_digest,
            custody_refund_lamports: refund_lamports,
            custody_revision: post_custody_revision,
            phase_revision: self
                .phase_revision
                .checked_add(1)
                .ok_or(AggregateRetirementCheckpointErrorV1::Coordinate)?,
            ..self
        })
    }

    /// Advance exactly once after closing the normal Custody replay.
    pub fn close_replay(
        self,
        receipt_digest: [u8; 32],
        refund_lamports: u64,
        post_custody_revision: u64,
    ) -> AggregateRetirementCheckpointResultV1<Self> {
        if self.phase != AggregateRetirementPhaseV1::HoardVaultClosed {
            return Err(AggregateRetirementCheckpointErrorV1::Phase);
        }
        require_nonzero(&[receipt_digest])?;
        let expected_revision = self
            .custody_revision
            .checked_add(1)
            .ok_or(AggregateRetirementCheckpointErrorV1::Coordinate)?;
        let custody_refund_lamports = self
            .custody_refund_lamports
            .checked_add(refund_lamports)
            .ok_or(AggregateRetirementCheckpointErrorV1::Coordinate)?;
        if post_custody_revision != expected_revision || refund_lamports == 0 {
            return Err(AggregateRetirementCheckpointErrorV1::Coordinate);
        }
        Ok(Self {
            phase: AggregateRetirementPhaseV1::CustodyReplayClosed,
            close_replay_receipt_digest: receipt_digest,
            custody_refund_lamports,
            custody_revision: post_custody_revision,
            phase_revision: self
                .phase_revision
                .checked_add(1)
                .ok_or(AggregateRetirementCheckpointErrorV1::Coordinate)?,
            ..self
        })
    }

    /// Decode an exact canonical checkpoint.
    pub fn decode(input: &[u8]) -> AggregateRetirementCheckpointResultV1<Self> {
        if input.len() != AGGREGATE_RETIREMENT_CHECKPOINT_BYTES_V1 {
            return Err(AggregateRetirementCheckpointErrorV1::Length);
        }
        if input.get(..8) != Some(AGGREGATE_RETIREMENT_CHECKPOINT_MAGIC_V1.as_slice())
            || u16_at(input, 8)? != AGGREGATE_RETIREMENT_CHECKPOINT_VERSION_V1
        {
            return Err(AggregateRetirementCheckpointErrorV1::Header);
        }
        require_zero(input, 11, 5)?;
        let phase = AggregateRetirementPhaseV1::decode(byte(input, 10)?)?;
        let checkpoint = Self {
            phase,
            core_prestate_digest: array(input, CORE_PRESTATE_OFFSET)?,
            bundle_digest: array(input, BUNDLE_DIGEST_OFFSET)?,
            phase_join_digest: array(input, CLAIMS_CONTEXT_OFFSET)?,
            claims_receipt_digest: array(input, CLAIMS_RECEIPT_OFFSET)?,
            close_vault_receipt_digest: array(input, VAULT_RECEIPT_OFFSET)?,
            close_replay_receipt_digest: array(input, REPLAY_RECEIPT_OFFSET)?,
            claims_refund_lamports: u64_at(input, CLAIMS_REFUND_OFFSET)?,
            custody_refund_lamports: u64_at(input, CUSTODY_REFUND_OFFSET)?,
            generation: u64_at(input, GENERATION_OFFSET)?,
            claims_revision: u64_at(input, CLAIMS_REVISION_OFFSET)?,
            custody_revision: u64_at(input, CUSTODY_REVISION_OFFSET)?,
            phase_revision: u64_at(input, PHASE_REVISION_OFFSET)?,
        };
        checkpoint.validate()?;
        Ok(checkpoint)
    }

    /// Encode exact canonical checkpoint bytes.
    pub fn to_bytes(self) -> [u8; AGGREGATE_RETIREMENT_CHECKPOINT_BYTES_V1] {
        let mut output = [0; AGGREGATE_RETIREMENT_CHECKPOINT_BYTES_V1];
        put(&mut output, 0, &AGGREGATE_RETIREMENT_CHECKPOINT_MAGIC_V1);
        put_u16(&mut output, 8, AGGREGATE_RETIREMENT_CHECKPOINT_VERSION_V1);
        output[10] = self.phase as u8;
        for (offset, value) in [
            (CORE_PRESTATE_OFFSET, self.core_prestate_digest),
            (BUNDLE_DIGEST_OFFSET, self.bundle_digest),
            (CLAIMS_CONTEXT_OFFSET, self.phase_join_digest),
            (CLAIMS_RECEIPT_OFFSET, self.claims_receipt_digest),
            (VAULT_RECEIPT_OFFSET, self.close_vault_receipt_digest),
            (REPLAY_RECEIPT_OFFSET, self.close_replay_receipt_digest),
        ] {
            put(&mut output, offset, &value);
        }
        for (offset, value) in [
            (CLAIMS_REFUND_OFFSET, self.claims_refund_lamports),
            (CUSTODY_REFUND_OFFSET, self.custody_refund_lamports),
            (GENERATION_OFFSET, self.generation),
            (CLAIMS_REVISION_OFFSET, self.claims_revision),
            (CUSTODY_REVISION_OFFSET, self.custody_revision),
            (PHASE_REVISION_OFFSET, self.phase_revision),
        ] {
            put_u64(&mut output, offset, value);
        }
        output
    }

    /// Current exhaustive persisted phase.
    pub const fn phase(self) -> AggregateRetirementPhaseV1 {
        self.phase
    }

    /// Immutable digest of the complete original plan.
    pub const fn bundle_digest(self) -> [u8; 32] {
        self.bundle_digest
    }

    /// Digest of the unchanged Retiring Core state.
    pub const fn core_prestate_digest(self) -> [u8; 32] {
        self.core_prestate_digest
    }

    /// Claims-owned custody context retained across the handoff.
    pub const fn phase_join_digest(self) -> [u8; 32] {
        self.phase_join_digest
    }

    /// Exact Claims handoff receipt digest.
    pub const fn claims_receipt_digest(self) -> [u8; 32] {
        self.claims_receipt_digest
    }

    /// Exact HoardPrincipal close receipt digest, zero before that phase.
    pub const fn close_vault_receipt_digest(self) -> [u8; 32] {
        self.close_vault_receipt_digest
    }

    /// Exact Custody replay close receipt digest, zero before that phase.
    pub const fn close_replay_receipt_digest(self) -> [u8; 32] {
        self.close_replay_receipt_digest
    }

    /// Exact Claims refund retained by this account.
    pub const fn claims_refund_lamports(self) -> u64 {
        self.claims_refund_lamports
    }

    /// Exact cumulative Custody refund already credited to RentCredit.
    pub const fn custody_refund_lamports(self) -> u64 {
        self.custody_refund_lamports
    }

    /// Exact current Custody revision.
    pub const fn custody_revision(self) -> u64 {
        self.custody_revision
    }

    /// Immutable Market generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Exact post-handoff Claims revision.
    pub const fn claims_revision(self) -> u64 {
        self.claims_revision
    }

    /// Exact checkpoint revision.
    pub const fn phase_revision(self) -> u64 {
        self.phase_revision
    }

    /// Digest of the ordered, phase-tagged receipt history.
    pub fn history_digest(self) -> [u8; 32] {
        let phase = [self.phase as u8];
        let scalars = [
            self.claims_refund_lamports,
            self.custody_refund_lamports,
            self.generation,
            self.claims_revision,
            self.custody_revision,
            self.phase_revision,
        ];
        // Six little-endian u64s tile 48 bytes exactly, so `chunks_exact_mut`
        // yields exactly six slots and the zip is total — same bytes as the
        // computed-index write it replaces, with no index to get wrong. The
        // assert is what stops the tiling from going quiet: zip truncates, so
        // adding a scalar without widening the buffer would silently drop it
        // out of the digest rather than panic. It is compiled out of the SBF
        // release build, so this costs the ELF nothing.
        let mut scalar_bytes = [0_u8; 48];
        debug_assert!(scalar_bytes.len() == scalars.len().saturating_mul(8));
        for (slot, value) in scalar_bytes.chunks_exact_mut(8).zip(scalars.iter()) {
            slot.copy_from_slice(&value.to_le_bytes());
        }
        digestv(&[
            AGGREGATE_RETIREMENT_HISTORY_DIGEST_DOMAIN_V1,
            &phase,
            &self.bundle_digest,
            &self.claims_receipt_digest,
            &self.close_vault_receipt_digest,
            &self.close_replay_receipt_digest,
            &scalar_bytes,
        ])
    }

    fn validate(self) -> AggregateRetirementCheckpointResultV1<()> {
        require_nonzero(&[
            self.core_prestate_digest,
            self.bundle_digest,
            self.phase_join_digest,
            self.claims_receipt_digest,
        ])?;
        if self.claims_refund_lamports == 0
            || self.generation == 0
            || self.claims_revision == 0
            || self.custody_revision == 0
            || self.phase_revision != self.phase as u64
        {
            return Err(AggregateRetirementCheckpointErrorV1::Coordinate);
        }
        match self.phase {
            AggregateRetirementPhaseV1::ClaimsClosed => {
                if self.close_vault_receipt_digest != [0; 32]
                    || self.close_replay_receipt_digest != [0; 32]
                    || self.custody_refund_lamports != 0
                {
                    return Err(AggregateRetirementCheckpointErrorV1::NonCanonical);
                }
            }
            AggregateRetirementPhaseV1::HoardVaultClosed => {
                require_nonzero(&[self.close_vault_receipt_digest])?;
                if self.close_replay_receipt_digest != [0; 32] || self.custody_refund_lamports == 0
                {
                    return Err(AggregateRetirementCheckpointErrorV1::NonCanonical);
                }
            }
            AggregateRetirementPhaseV1::CustodyReplayClosed => {
                require_nonzero(&[
                    self.close_vault_receipt_digest,
                    self.close_replay_receipt_digest,
                ])?;
                if self.custody_refund_lamports == 0 {
                    return Err(AggregateRetirementCheckpointErrorV1::NonCanonical);
                }
            }
        }
        Ok(())
    }
}

/// One fixed suffix request. Its action is selected by its distinct magic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AggregateRetirementSuffixRequestV1 {
    magic: [u8; 8],
    /// Core Market identity.
    pub market: [u8; 32],
    /// Reused Claims aggregate / Core checkpoint identity.
    pub checkpoint: [u8; 32],
    /// Digest of the complete original retirement bundle.
    pub bundle_digest: [u8; 32],
    /// Digest of the exact immutable Resolution closure receipt.
    pub source_receipt_digest: [u8; 32],
    /// Digest of this phase's exact child request; zero only for Finish.
    pub child_request_digest: [u8; 32],
    /// Exact expected persisted phase revision.
    pub expected_phase_revision: u64,
    /// Exact expected live Custody revision.
    pub expected_custody_revision: u64,
}

/// The four fields every suffix request of one retirement must carry alike.
///
/// A retirement issues THREE suffix requests -- close vault, close replay,
/// finish -- and these four fields are identical in all three: they say which
/// retirement of which market this is. Only the magic, the child request digest
/// and the two revisions differ per phase.
///
/// Grouping them is not tidying. Passed positionally they are four adjacent
/// `[u8; 32]` arguments, restated at each of the three call sites, and any two
/// of them transposed type-checks and encodes a request that names a real
/// market and the wrong bundle. Built once and reused, the "all three agree"
/// property is a fact about the program text rather than a thing three call
/// sites have to keep saying the same way.
///
/// It carries no validation of its own on purpose: the checks these fields face
/// live in [`AggregateRetirementSuffixRequestV1::new`] in the order that
/// function has always run them, and a wrong request must keep refusing with
/// the error it already refuses with.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AggregateRetirementSuffixBindingV1 {
    /// Core Market identity.
    pub market: [u8; 32],
    /// Reused Claims aggregate / Core checkpoint identity.
    pub checkpoint: [u8; 32],
    /// Digest of the complete original retirement bundle.
    pub bundle_digest: [u8; 32],
    /// Digest of the exact immutable Resolution closure receipt.
    pub source_receipt_digest: [u8; 32],
}

impl AggregateRetirementSuffixRequestV1 {
    /// Construct a suffix request for one of the three named actions.
    ///
    /// Arguments are in wire order: the magic at offset 0, the binding's four
    /// digests at 16 through 143, then the child request digest and the two
    /// revisions.
    pub fn new(
        magic: [u8; 8],
        binding: AggregateRetirementSuffixBindingV1,
        child_request_digest: [u8; 32],
        expected_phase_revision: u64,
        expected_custody_revision: u64,
    ) -> AggregateRetirementCheckpointResultV1<Self> {
        let AggregateRetirementSuffixBindingV1 {
            market,
            checkpoint,
            bundle_digest,
            source_receipt_digest,
        } = binding;
        if !matches!(
            magic,
            AGGREGATE_RETIREMENT_CLOSE_VAULT_MAGIC_V1
                | AGGREGATE_RETIREMENT_CLOSE_REPLAY_MAGIC_V1
                | AGGREGATE_RETIREMENT_FINISH_MAGIC_V1
        ) {
            return Err(AggregateRetirementCheckpointErrorV1::Header);
        }
        require_nonzero(&[market, checkpoint, bundle_digest, source_receipt_digest])?;
        if magic == AGGREGATE_RETIREMENT_FINISH_MAGIC_V1 {
            if child_request_digest != [0; 32] {
                return Err(AggregateRetirementCheckpointErrorV1::NonCanonical);
            }
        } else {
            require_nonzero(&[child_request_digest])?;
        }
        if market == checkpoint || expected_phase_revision == 0 || expected_custody_revision == 0 {
            return Err(AggregateRetirementCheckpointErrorV1::Coordinate);
        }
        Ok(Self {
            magic,
            market,
            checkpoint,
            bundle_digest,
            source_receipt_digest,
            child_request_digest,
            expected_phase_revision,
            expected_custody_revision,
        })
    }

    /// Encode exact canonical request bytes.
    pub fn to_bytes(self) -> [u8; AGGREGATE_RETIREMENT_SUFFIX_REQUEST_BYTES_V1] {
        let mut output = [0; AGGREGATE_RETIREMENT_SUFFIX_REQUEST_BYTES_V1];
        put(&mut output, 0, &self.magic);
        put_u16(&mut output, 8, AGGREGATE_RETIREMENT_CHECKPOINT_VERSION_V1);
        for (offset, value) in [
            (16, self.market),
            (48, self.checkpoint),
            (80, self.bundle_digest),
            (112, self.source_receipt_digest),
            (144, self.child_request_digest),
        ] {
            put(&mut output, offset, &value);
        }
        put_u64(&mut output, 176, self.expected_phase_revision);
        put_u64(&mut output, 184, self.expected_custody_revision);
        output
    }

    /// Hostile-decode an exact suffix request.
    pub fn decode(input: &[u8]) -> AggregateRetirementCheckpointResultV1<Self> {
        if input.len() != AGGREGATE_RETIREMENT_SUFFIX_REQUEST_BYTES_V1 {
            return Err(AggregateRetirementCheckpointErrorV1::Length);
        }
        require_zero(input, 10, 6)?;
        Self::new(
            array(input, 0)?,
            AggregateRetirementSuffixBindingV1 {
                market: array(input, 16)?,
                checkpoint: array(input, 48)?,
                bundle_digest: array(input, 80)?,
                source_receipt_digest: array(input, 112)?,
            },
            array(input, 144)?,
            u64_at(input, 176)?,
            u64_at(input, 184)?,
        )
    }

    /// Selected distinct action magic.
    pub const fn magic(self) -> [u8; 8] {
        self.magic
    }
}

fn require_nonzero(values: &[[u8; 32]]) -> AggregateRetirementCheckpointResultV1<()> {
    if values.contains(&[0; 32]) {
        return Err(AggregateRetirementCheckpointErrorV1::ZeroIdentity);
    }
    Ok(())
}

fn byte(input: &[u8], offset: usize) -> AggregateRetirementCheckpointResultV1<u8> {
    input
        .get(offset)
        .copied()
        .ok_or(AggregateRetirementCheckpointErrorV1::Length)
}

fn array<const N: usize>(
    input: &[u8],
    offset: usize,
) -> AggregateRetirementCheckpointResultV1<[u8; N]> {
    input
        .get(offset..offset.saturating_add(N))
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or(AggregateRetirementCheckpointErrorV1::Length)
}

fn u16_at(input: &[u8], offset: usize) -> AggregateRetirementCheckpointResultV1<u16> {
    Ok(u16::from_le_bytes(array(input, offset)?))
}

fn u64_at(input: &[u8], offset: usize) -> AggregateRetirementCheckpointResultV1<u64> {
    Ok(u64::from_le_bytes(array(input, offset)?))
}

fn require_zero(
    input: &[u8],
    offset: usize,
    len: usize,
) -> AggregateRetirementCheckpointResultV1<()> {
    if input
        .get(offset..offset.saturating_add(len))
        .ok_or(AggregateRetirementCheckpointErrorV1::Length)?
        .iter()
        .any(|value| *value != 0)
    {
        return Err(AggregateRetirementCheckpointErrorV1::NonCanonical);
    }
    Ok(())
}

/// Write one fixed-width field into the encoder's own exactly-sized buffer.
///
/// The slicing panic here is deliberate and is kept as a panic.
///
/// These three take no caller data. `output` is the buffer this module just
/// allocated at the checkpoint's exact encoded width, and `offset` is one of
/// this file's own layout constants. An out-of-range write is therefore not a
/// malformed input to refuse — it is this encoder disagreeing with its own
/// layout, which would mean every digest it produced was already wrong.
///
/// So there is no refusal to convert to. The two alternatives are both worse
/// than the lint: `get_mut(..)` with the write skipped emits a short, partly
/// zero record that hashes to a plausible-looking identity, and a fabricated
/// `Err` variant would add a refusal path no caller can trigger and no test can
/// reach. Panicking stops the transaction, which is the correct response to an
/// encoder that cannot encode.
#[allow(clippy::indexing_slicing)]
fn put<const N: usize>(output: &mut [u8], offset: usize, value: &[u8; N]) {
    output[offset..offset + N].copy_from_slice(value);
}

/// See [`put`]: same buffer, same layout constants, same deliberate panic.
#[allow(clippy::indexing_slicing)]
fn put_u16(output: &mut [u8], offset: usize, value: u16) {
    output[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

/// See [`put`]: same buffer, same layout constants, same deliberate panic.
#[allow(clippy::indexing_slicing)]
fn put_u64(output: &mut [u8], offset: usize, value: u64) {
    output[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checkpoint() -> AggregateRetirementCheckpointV1 {
        AggregateRetirementCheckpointV1::claims_closed(ClaimsClosedCheckpointInputV1 {
            core_prestate_digest: [1; 32],
            bundle_digest: [2; 32],
            claims_context: [3; 32],
            claims_receipt_digest: [4; 32],
            claims_refund_lamports: 91,
            generation: 7,
            claims_revision: 12,
            custody_revision: 21,
        })
        .expect("claims-closed checkpoint")
    }

    #[test]
    fn phases_are_exhaustive_ordered_and_round_trip() {
        let claims = checkpoint();
        assert_eq!(claims.phase(), AggregateRetirementPhaseV1::ClaimsClosed);
        assert_eq!(
            AggregateRetirementCheckpointV1::decode(&claims.to_bytes()),
            Ok(claims)
        );
        let vault = claims
            .close_vault([5; 32], [7; 32], 17, 22)
            .expect("vault close");
        assert_eq!(vault.phase(), AggregateRetirementPhaseV1::HoardVaultClosed);
        let replay = vault.close_replay([6; 32], 19, 23).expect("replay close");
        assert_eq!(
            replay.phase(),
            AggregateRetirementPhaseV1::CustodyReplayClosed
        );
        assert_eq!(replay.claims_refund_lamports(), 91);
        assert_eq!(replay.custody_refund_lamports(), 36);
        assert_eq!(
            AggregateRetirementCheckpointV1::decode(&replay.to_bytes()),
            Ok(replay)
        );
    }

    #[test]
    fn replay_skip_and_phase_substitution_refuse() {
        let claims = checkpoint();
        assert_eq!(
            claims.close_replay([6; 32], 19, 22),
            Err(AggregateRetirementCheckpointErrorV1::Phase)
        );
        let vault = claims
            .close_vault([5; 32], [7; 32], 17, 22)
            .expect("vault close");
        assert_eq!(
            vault.close_vault([5; 32], [7; 32], 17, 23),
            Err(AggregateRetirementCheckpointErrorV1::Phase)
        );
        let replay = vault.close_replay([6; 32], 19, 23).expect("replay close");
        assert_eq!(
            replay.close_replay([6; 32], 19, 24),
            Err(AggregateRetirementCheckpointErrorV1::Phase)
        );
    }

    #[test]
    fn inactive_receipts_and_refund_overflow_refuse() {
        let mut bytes = checkpoint().to_bytes();
        bytes[VAULT_RECEIPT_OFFSET] = 1;
        assert_eq!(
            AggregateRetirementCheckpointV1::decode(&bytes),
            Err(AggregateRetirementCheckpointErrorV1::NonCanonical)
        );
        let vault = checkpoint()
            .close_vault([5; 32], [7; 32], u64::MAX, 22)
            .expect("max first custody refund is representable");
        assert_eq!(
            vault.close_replay([6; 32], 1, 23),
            Err(AggregateRetirementCheckpointErrorV1::Coordinate)
        );
    }

    #[test]
    fn suffix_actions_are_distinct_and_finish_has_no_child() {
        let binding = AggregateRetirementSuffixBindingV1 {
            market: [1; 32],
            checkpoint: [2; 32],
            bundle_digest: [3; 32],
            source_receipt_digest: [4; 32],
        };
        let vault = AggregateRetirementSuffixRequestV1::new(
            AGGREGATE_RETIREMENT_CLOSE_VAULT_MAGIC_V1,
            binding,
            [5; 32],
            1,
            21,
        )
        .expect("vault request");
        assert_eq!(
            AggregateRetirementSuffixRequestV1::decode(&vault.to_bytes()),
            Ok(vault)
        );
        assert_eq!(
            AggregateRetirementSuffixRequestV1::new(
                AGGREGATE_RETIREMENT_FINISH_MAGIC_V1,
                binding,
                [5; 32],
                3,
                23,
            ),
            Err(AggregateRetirementCheckpointErrorV1::NonCanonical)
        );
    }

    #[test]
    fn receipt_history_changes_at_every_authenticated_boundary() {
        let claims = checkpoint();
        let vault = claims
            .close_vault([5; 32], [7; 32], 17, 22)
            .expect("vault close");
        let replay = vault.close_replay([6; 32], 19, 23).expect("replay close");
        assert_ne!(claims.history_digest(), vault.history_digest());
        assert_ne!(vault.history_digest(), replay.history_digest());
    }
}
