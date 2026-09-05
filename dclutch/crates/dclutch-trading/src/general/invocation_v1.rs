//! Durable caller intent and optimistic replay state for executable General V3.
//!
//! This wire covers the seven settlement actions whose complete artifact
//! triples and Trading caller already exist. It deliberately does not admit the
//! wider `ControllerRequestV3` GEN-SEVEN topology: collection, order, batch,
//! candidate, and escrow execution still need their coordinated artifact
//! triples and runtime dispatch before any durable caller may name them.

use crate::general_codec::Action;
use dclutch_sha256_adapter::digest;

/// Exact durable invocation width.
pub const GENERAL_INVOCATION_BYTES_V1: usize = 528;
/// Canonical invocation magic.
pub const GENERAL_INVOCATION_MAGIC_V1: [u8; 8] = *b"DCGINV01";
/// Implemented durable invocation version.
pub const GENERAL_INVOCATION_VERSION_V1: u16 = 1;
/// Exact optimistic replay-state width.
pub const GENERAL_INVOCATION_REPLAY_BYTES_V1: usize = 128;
/// Canonical optimistic replay-state magic.
pub const GENERAL_INVOCATION_REPLAY_MAGIC_V1: [u8; 8] = *b"DCGIRP01";
/// Domain for the ordered instruction-account commitment.
pub const GENERAL_INVOCATION_ACCOUNT_METAS_DOMAIN_V1: &[u8] =
    b"dclutch/general/invocation/account-metas/v1";
/// Domain for the sorted unique transaction-lock commitment.
pub const GENERAL_INVOCATION_LOCK_SET_DOMAIN_V1: &[u8] = b"dclutch/general/invocation/lock-set/v1";
/// Domain for the sorted unique signer-set commitment.
pub const GENERAL_INVOCATION_SIGNER_SET_DOMAIN_V1: &[u8] =
    b"dclutch/general/invocation/signer-set/v1";
/// Domain for the complete selected General artifact-graph commitment.
pub const GENERAL_INVOCATION_ARTIFACT_GRAPH_DOMAIN_V1: &[u8] =
    b"dclutch/general/invocation/artifact-graph/v1";
/// Devnet-compatible maximum unique transaction locks.
pub const GENERAL_INVOCATION_MAX_UNIQUE_LOCKS_V1: u16 = 64;

const ACTION_OFFSET: usize = 10;
const RESERVED_HEADER_OFFSET: usize = 11;
const MARKET_OFFSET: usize = 16;
const ROOT_OFFSET: usize = 48;
const ROOT_PRESTATE_OFFSET: usize = 80;
const RELEASE_SET_OFFSET: usize = 112;
const CHECKED_MANIFEST_OFFSET: usize = 144;
const TRADING_ARTIFACT_RELEASE_OFFSET: usize = 176;
const GENERAL_ARTIFACT_RELEASE_OFFSET: usize = 208;
const ARTIFACT_GRAPH_OFFSET: usize = 240;
const FAMILY_REQUEST_OFFSET: usize = 272;
const ACCOUNT_METAS_OFFSET: usize = 304;
const LOCK_SET_OFFSET: usize = 336;
const SIGNER_SET_OFFSET: usize = 368;
const TRADING_PROGRAM_OFFSET: usize = 400;
const PAYER_OFFSET: usize = 432;
const LOOKUP_TABLE_OFFSET: usize = 464;
const NONCE_OFFSET: usize = 496;
const GENERATION_OFFSET: usize = 504;
const ACCOUNT_META_COUNT_OFFSET: usize = 512;
const UNIQUE_LOCK_COUNT_OFFSET: usize = 514;
const SIGNER_COUNT_OFFSET: usize = 516;
const RESERVED_TAIL_OFFSET: usize = 518;

const REPLAY_MARKET_OFFSET: usize = 16;
const REPLAY_PAYER_OFFSET: usize = 48;
const REPLAY_LAST_INVOCATION_OFFSET: usize = 80;
const REPLAY_NEXT_NONCE_OFFSET: usize = 112;
const REPLAY_GENERATION_OFFSET: usize = 120;

/// Stable refusal from a hostile durable invocation or replay observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralInvocationErrorV1 {
    /// The wire had another exact width.
    InvalidLength,
    /// Magic or version selected another schema.
    InvalidHeader,
    /// Reserved bytes or a count relation was noncanonical.
    NonCanonical,
    /// A required account or content identity was zero.
    ZeroIdentity,
    /// This action has no caller-backed General V3 artifact triple.
    UnexecutableAction,
    /// Nonce, generation, or replay coordinates did not join.
    Replay,
    /// Advancing the replay nonce overflowed.
    Arithmetic,
}

/// Result alias for durable General invocation operations.
pub type Result<T> = core::result::Result<T, GeneralInvocationErrorV1>;

/// Fields of one exact content-addressed General invocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralInvocationFieldsV1 {
    /// Caller-backed settlement action.
    pub action: Action,
    /// Exact Core Market account.
    pub market: [u8; 32],
    /// Exact General capability root account.
    pub root: [u8; 32],
    /// SHA-256 of the complete root prestate.
    pub root_prestate_digest: [u8; 32],
    /// Immutable capability execution release set selected by the Market.
    pub release_set: [u8; 32],
    /// Digest of the independently checked multiprogram manifest.
    pub checked_manifest_digest: [u8; 32],
    /// Selected immutable Trading artifact release.
    pub trading_artifact_release: [u8; 32],
    /// Selected immutable General accelerator artifact release.
    pub general_artifact_release: [u8; 32],
    /// Commitment to every selected General artifact identity in semantic order.
    pub artifact_graph_digest: [u8; 32],
    /// Digest of the exact canonical `ControllerRequestV2` bytes.
    pub family_request_digest: [u8; 32],
    /// Commitment to every ordered instruction meta and privilege bit.
    pub account_metas_digest: [u8; 32],
    /// Commitment to the sorted unique transaction-lock set.
    pub lock_set_digest: [u8; 32],
    /// Commitment to the sorted unique signer set.
    pub signer_set_digest: [u8; 32],
    /// Checked Trading program which receives the top-level instruction.
    pub trading_program: [u8; 32],
    /// Fee payer and first transaction signer.
    pub payer: [u8; 32],
    /// Sole exact finalized address lookup table used by the packet.
    pub lookup_table: [u8; 32],
    /// Market-and-payer scoped optimistic replay nonce.
    pub nonce: u64,
    /// Immutable Market generation.
    pub generation: u64,
    /// Raw top-level instruction meta count before alias collapse.
    pub account_meta_count: u16,
    /// Exact unique transaction lock count, including payer, program, and LUT.
    pub unique_lock_count: u16,
    /// Exact unique transaction signer count.
    pub signer_count: u16,
}

/// One exact content-addressed, message-independent General call intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralInvocationV1 {
    fields: GeneralInvocationFieldsV1,
}

impl GeneralInvocationV1 {
    /// Validate and construct one exact durable invocation.
    pub fn new(fields: GeneralInvocationFieldsV1) -> Result<Self> {
        require_executable_action(fields.action)?;
        for identity in [
            fields.market,
            fields.root,
            fields.root_prestate_digest,
            fields.release_set,
            fields.checked_manifest_digest,
            fields.trading_artifact_release,
            fields.general_artifact_release,
            fields.artifact_graph_digest,
            fields.family_request_digest,
            fields.account_metas_digest,
            fields.lock_set_digest,
            fields.signer_set_digest,
            fields.trading_program,
            fields.payer,
            fields.lookup_table,
        ] {
            require_nonzero(identity)?;
        }
        if fields.nonce == 0
            || fields.generation == 0
            || fields.account_meta_count == 0
            || fields.unique_lock_count == 0
            || fields.unique_lock_count > GENERAL_INVOCATION_MAX_UNIQUE_LOCKS_V1
            || fields.signer_count == 0
            || fields.signer_count > fields.unique_lock_count
        {
            return Err(GeneralInvocationErrorV1::NonCanonical);
        }
        Ok(Self { fields })
    }

    /// Hostile-decode one exact durable invocation.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != GENERAL_INVOCATION_BYTES_V1 {
            return Err(GeneralInvocationErrorV1::InvalidLength);
        }
        if array::<8>(input, 0)? != GENERAL_INVOCATION_MAGIC_V1
            || u16_at(input, 8)? != GENERAL_INVOCATION_VERSION_V1
        {
            return Err(GeneralInvocationErrorV1::InvalidHeader);
        }
        require_zero(input, RESERVED_HEADER_OFFSET, 5)?;
        require_zero(input, RESERVED_TAIL_OFFSET, 10)?;
        Self::new(GeneralInvocationFieldsV1 {
            action: decode_executable_action(byte_at(input, ACTION_OFFSET)?)?,
            market: array(input, MARKET_OFFSET)?,
            root: array(input, ROOT_OFFSET)?,
            root_prestate_digest: array(input, ROOT_PRESTATE_OFFSET)?,
            release_set: array(input, RELEASE_SET_OFFSET)?,
            checked_manifest_digest: array(input, CHECKED_MANIFEST_OFFSET)?,
            trading_artifact_release: array(input, TRADING_ARTIFACT_RELEASE_OFFSET)?,
            general_artifact_release: array(input, GENERAL_ARTIFACT_RELEASE_OFFSET)?,
            artifact_graph_digest: array(input, ARTIFACT_GRAPH_OFFSET)?,
            family_request_digest: array(input, FAMILY_REQUEST_OFFSET)?,
            account_metas_digest: array(input, ACCOUNT_METAS_OFFSET)?,
            lock_set_digest: array(input, LOCK_SET_OFFSET)?,
            signer_set_digest: array(input, SIGNER_SET_OFFSET)?,
            trading_program: array(input, TRADING_PROGRAM_OFFSET)?,
            payer: array(input, PAYER_OFFSET)?,
            lookup_table: array(input, LOOKUP_TABLE_OFFSET)?,
            nonce: u64_at(input, NONCE_OFFSET)?,
            generation: u64_at(input, GENERATION_OFFSET)?,
            account_meta_count: u16_at(input, ACCOUNT_META_COUNT_OFFSET)?,
            unique_lock_count: u16_at(input, UNIQUE_LOCK_COUNT_OFFSET)?,
            signer_count: u16_at(input, SIGNER_COUNT_OFFSET)?,
        })
    }

    /// Encode one canonical durable invocation.
    pub fn to_bytes(self) -> [u8; GENERAL_INVOCATION_BYTES_V1] {
        let fields = self.fields;
        let mut output = [0_u8; GENERAL_INVOCATION_BYTES_V1];
        output[..8].copy_from_slice(&GENERAL_INVOCATION_MAGIC_V1);
        output[8..10].copy_from_slice(&GENERAL_INVOCATION_VERSION_V1.to_le_bytes());
        output[ACTION_OFFSET] = fields.action as u8;
        for (offset, value) in [
            (MARKET_OFFSET, fields.market),
            (ROOT_OFFSET, fields.root),
            (ROOT_PRESTATE_OFFSET, fields.root_prestate_digest),
            (RELEASE_SET_OFFSET, fields.release_set),
            (CHECKED_MANIFEST_OFFSET, fields.checked_manifest_digest),
            (
                TRADING_ARTIFACT_RELEASE_OFFSET,
                fields.trading_artifact_release,
            ),
            (
                GENERAL_ARTIFACT_RELEASE_OFFSET,
                fields.general_artifact_release,
            ),
            (ARTIFACT_GRAPH_OFFSET, fields.artifact_graph_digest),
            (FAMILY_REQUEST_OFFSET, fields.family_request_digest),
            (ACCOUNT_METAS_OFFSET, fields.account_metas_digest),
            (LOCK_SET_OFFSET, fields.lock_set_digest),
            (SIGNER_SET_OFFSET, fields.signer_set_digest),
            (TRADING_PROGRAM_OFFSET, fields.trading_program),
            (PAYER_OFFSET, fields.payer),
            (LOOKUP_TABLE_OFFSET, fields.lookup_table),
        ] {
            output[offset..offset + 32].copy_from_slice(&value);
        }
        output[NONCE_OFFSET..NONCE_OFFSET + 8].copy_from_slice(&fields.nonce.to_le_bytes());
        output[GENERATION_OFFSET..GENERATION_OFFSET + 8]
            .copy_from_slice(&fields.generation.to_le_bytes());
        output[ACCOUNT_META_COUNT_OFFSET..ACCOUNT_META_COUNT_OFFSET + 2]
            .copy_from_slice(&fields.account_meta_count.to_le_bytes());
        output[UNIQUE_LOCK_COUNT_OFFSET..UNIQUE_LOCK_COUNT_OFFSET + 2]
            .copy_from_slice(&fields.unique_lock_count.to_le_bytes());
        output[SIGNER_COUNT_OFFSET..SIGNER_COUNT_OFFSET + 2]
            .copy_from_slice(&fields.signer_count.to_le_bytes());
        output
    }

    /// SHA-256 content identity of the complete canonical invocation bytes.
    #[must_use]
    pub fn content_id(self) -> [u8; 32] {
        digest(&self.to_bytes())
    }

    /// Return every authenticated invocation field.
    #[must_use]
    pub const fn fields(self) -> GeneralInvocationFieldsV1 {
        self.fields
    }
}

/// Canonical caller-owned optimistic replay cursor.
///
/// A future onchain caller must own and advance this state only after the
/// General Hot instruction succeeds. The host operator can validate and
/// project the transition, but its projection is not onchain authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralInvocationReplayV1 {
    market: [u8; 32],
    payer: [u8; 32],
    last_invocation: [u8; 32],
    next_nonce: u64,
    generation: u64,
}

impl GeneralInvocationReplayV1 {
    /// Construct the canonical fresh cursor for one Market generation and payer.
    pub fn fresh(market: [u8; 32], payer: [u8; 32], generation: u64) -> Result<Self> {
        require_nonzero(market)?;
        require_nonzero(payer)?;
        if generation == 0 {
            return Err(GeneralInvocationErrorV1::Replay);
        }
        Ok(Self {
            market,
            payer,
            last_invocation: [0; 32],
            next_nonce: 1,
            generation,
        })
    }

    /// Hostile-decode one exact replay observation.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != GENERAL_INVOCATION_REPLAY_BYTES_V1 {
            return Err(GeneralInvocationErrorV1::InvalidLength);
        }
        if array::<8>(input, 0)? != GENERAL_INVOCATION_REPLAY_MAGIC_V1
            || u16_at(input, 8)? != GENERAL_INVOCATION_VERSION_V1
        {
            return Err(GeneralInvocationErrorV1::InvalidHeader);
        }
        require_zero(input, 10, 6)?;
        let value = Self {
            market: array(input, REPLAY_MARKET_OFFSET)?,
            payer: array(input, REPLAY_PAYER_OFFSET)?,
            last_invocation: array(input, REPLAY_LAST_INVOCATION_OFFSET)?,
            next_nonce: u64_at(input, REPLAY_NEXT_NONCE_OFFSET)?,
            generation: u64_at(input, REPLAY_GENERATION_OFFSET)?,
        };
        require_nonzero(value.market)?;
        require_nonzero(value.payer)?;
        if value.generation == 0
            || value.next_nonce == 0
            || (is_zero(value.last_invocation) && value.next_nonce != 1)
            || (!is_zero(value.last_invocation) && value.next_nonce == 1)
        {
            return Err(GeneralInvocationErrorV1::Replay);
        }
        Ok(value)
    }

    /// Encode one canonical replay state.
    pub fn to_bytes(self) -> [u8; GENERAL_INVOCATION_REPLAY_BYTES_V1] {
        let mut output = [0_u8; GENERAL_INVOCATION_REPLAY_BYTES_V1];
        output[..8].copy_from_slice(&GENERAL_INVOCATION_REPLAY_MAGIC_V1);
        output[8..10].copy_from_slice(&GENERAL_INVOCATION_VERSION_V1.to_le_bytes());
        output[REPLAY_MARKET_OFFSET..REPLAY_MARKET_OFFSET + 32].copy_from_slice(&self.market);
        output[REPLAY_PAYER_OFFSET..REPLAY_PAYER_OFFSET + 32].copy_from_slice(&self.payer);
        output[REPLAY_LAST_INVOCATION_OFFSET..REPLAY_LAST_INVOCATION_OFFSET + 32]
            .copy_from_slice(&self.last_invocation);
        output[REPLAY_NEXT_NONCE_OFFSET..REPLAY_NEXT_NONCE_OFFSET + 8]
            .copy_from_slice(&self.next_nonce.to_le_bytes());
        output[REPLAY_GENERATION_OFFSET..REPLAY_GENERATION_OFFSET + 8]
            .copy_from_slice(&self.generation.to_le_bytes());
        output
    }

    /// Validate this cursor and project the unique post-success replay state.
    pub fn advance(self, invocation: GeneralInvocationV1) -> Result<Self> {
        let fields = invocation.fields();
        if fields.market != self.market
            || fields.payer != self.payer
            || fields.generation != self.generation
            || fields.nonce != self.next_nonce
        {
            return Err(GeneralInvocationErrorV1::Replay);
        }
        Ok(Self {
            last_invocation: invocation.content_id(),
            next_nonce: self
                .next_nonce
                .checked_add(1)
                .ok_or(GeneralInvocationErrorV1::Arithmetic)?,
            ..self
        })
    }

    /// Exact Market coordinate.
    #[must_use]
    pub const fn market(self) -> [u8; 32] {
        self.market
    }

    /// Exact payer coordinate.
    #[must_use]
    pub const fn payer(self) -> [u8; 32] {
        self.payer
    }

    /// Content identity of the last completed invocation, or zero when fresh.
    #[must_use]
    pub const fn last_invocation(self) -> [u8; 32] {
        self.last_invocation
    }

    /// Sole nonce admitted by the next call.
    #[must_use]
    pub const fn next_nonce(self) -> u64 {
        self.next_nonce
    }

    /// Immutable Market generation.
    #[must_use]
    pub const fn generation(self) -> u64 {
        self.generation
    }
}

fn require_executable_action(action: Action) -> Result<()> {
    if matches!(
        action,
        Action::Consider
            | Action::Freeze
            | Action::InitializeSettlement
            | Action::Collect
            | Action::Materialize
            | Action::Distribute
            | Action::Close
    ) {
        Ok(())
    } else {
        Err(GeneralInvocationErrorV1::UnexecutableAction)
    }
}

fn decode_executable_action(tag: u8) -> Result<Action> {
    for action in [
        Action::Consider,
        Action::Freeze,
        Action::InitializeSettlement,
        Action::Collect,
        Action::Materialize,
        Action::Distribute,
        Action::Close,
    ] {
        if action as u8 == tag {
            return Ok(action);
        }
    }
    Err(GeneralInvocationErrorV1::UnexecutableAction)
}

fn require_nonzero(value: [u8; 32]) -> Result<()> {
    if is_zero(value) {
        Err(GeneralInvocationErrorV1::ZeroIdentity)
    } else {
        Ok(())
    }
}

fn is_zero(value: [u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}

fn byte_at(input: &[u8], offset: usize) -> Result<u8> {
    input
        .get(offset)
        .copied()
        .ok_or(GeneralInvocationErrorV1::InvalidLength)
}

fn array<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N]> {
    input
        .get(offset..offset + N)
        .ok_or(GeneralInvocationErrorV1::InvalidLength)?
        .try_into()
        .map_err(|_| GeneralInvocationErrorV1::InvalidLength)
}

fn u16_at(input: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(array(input, offset)?))
}

fn u64_at(input: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(array(input, offset)?))
}

fn require_zero(input: &[u8], offset: usize, count: usize) -> Result<()> {
    if input
        .get(offset..offset + count)
        .ok_or(GeneralInvocationErrorV1::InvalidLength)?
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(GeneralInvocationErrorV1::NonCanonical);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn fields() -> GeneralInvocationFieldsV1 {
        GeneralInvocationFieldsV1 {
            action: Action::Collect,
            market: id(1),
            root: id(2),
            root_prestate_digest: id(3),
            release_set: id(4),
            checked_manifest_digest: id(5),
            trading_artifact_release: id(6),
            general_artifact_release: id(7),
            artifact_graph_digest: id(8),
            family_request_digest: id(9),
            account_metas_digest: id(10),
            lock_set_digest: id(11),
            signer_set_digest: id(12),
            trading_program: id(13),
            payer: id(14),
            lookup_table: id(15),
            nonce: 1,
            generation: 7,
            account_meta_count: 52,
            unique_lock_count: 48,
            signer_count: 2,
        }
    }

    #[test]
    fn invocation_roundtrip_and_content_identity_are_exact() {
        let invocation = GeneralInvocationV1::new(fields()).expect("invocation");
        let bytes = invocation.to_bytes();
        assert_eq!(GeneralInvocationV1::decode(&bytes), Ok(invocation));
        assert_eq!(invocation.content_id(), digest(&bytes));
    }

    #[test]
    fn wider_gen_seven_actions_remain_explicitly_unexecutable() {
        for action in [
            Action::OpenBatch,
            Action::PlaceOrder,
            Action::CancelOrder,
            Action::CloseBatch,
            Action::SubmitCandidate,
            Action::VerifyCandidateRow,
            Action::ReleaseOrder,
        ] {
            assert_eq!(
                GeneralInvocationV1::new(GeneralInvocationFieldsV1 { action, ..fields() }),
                Err(GeneralInvocationErrorV1::UnexecutableAction)
            );
        }
    }

    #[test]
    fn replay_is_single_use_and_market_payer_scoped() {
        let invocation = GeneralInvocationV1::new(fields()).expect("invocation");
        let fresh = GeneralInvocationReplayV1::fresh(id(1), id(14), 7).expect("fresh");
        let advanced = fresh.advance(invocation).expect("first use");
        assert_eq!(advanced.next_nonce(), 2);
        assert_eq!(advanced.last_invocation(), invocation.content_id());
        assert_eq!(
            advanced.advance(invocation),
            Err(GeneralInvocationErrorV1::Replay)
        );

        for replay in [
            GeneralInvocationReplayV1::fresh(id(16), id(14), 7).expect("wrong market"),
            GeneralInvocationReplayV1::fresh(id(1), id(17), 7).expect("wrong payer"),
            GeneralInvocationReplayV1::fresh(id(1), id(14), 8).expect("wrong generation"),
        ] {
            assert_eq!(
                replay.advance(invocation),
                Err(GeneralInvocationErrorV1::Replay)
            );
        }
    }

    #[test]
    fn hostile_headers_padding_identities_and_geometry_refuse() {
        let canonical = GeneralInvocationV1::new(fields())
            .expect("invocation")
            .to_bytes();
        for offset in [0_usize, 8, RESERVED_HEADER_OFFSET, RESERVED_TAIL_OFFSET] {
            let mut mutated = canonical;
            mutated[offset] ^= 1;
            assert!(
                GeneralInvocationV1::decode(&mutated).is_err(),
                "offset {offset}"
            );
        }
        for offset in [MARKET_OFFSET, FAMILY_REQUEST_OFFSET, LOOKUP_TABLE_OFFSET] {
            let mut mutated = canonical;
            mutated[offset..offset + 32].fill(0);
            assert_eq!(
                GeneralInvocationV1::decode(&mutated),
                Err(GeneralInvocationErrorV1::ZeroIdentity)
            );
        }
        let mut too_many_locks = canonical;
        too_many_locks[UNIQUE_LOCK_COUNT_OFFSET..UNIQUE_LOCK_COUNT_OFFSET + 2]
            .copy_from_slice(&65_u16.to_le_bytes());
        assert_eq!(
            GeneralInvocationV1::decode(&too_many_locks),
            Err(GeneralInvocationErrorV1::NonCanonical)
        );
    }

    #[test]
    fn replay_roundtrip_and_hostile_shape_are_exact() {
        let invocation = GeneralInvocationV1::new(fields()).expect("invocation");
        let fresh = GeneralInvocationReplayV1::fresh(id(1), id(14), 7).expect("fresh");
        let advanced = fresh.advance(invocation).expect("advanced");
        for replay in [fresh, advanced] {
            assert_eq!(
                GeneralInvocationReplayV1::decode(&replay.to_bytes()),
                Ok(replay)
            );
        }
        let mut impossible = fresh.to_bytes();
        impossible[REPLAY_NEXT_NONCE_OFFSET..REPLAY_NEXT_NONCE_OFFSET + 8]
            .copy_from_slice(&2_u64.to_le_bytes());
        assert_eq!(
            GeneralInvocationReplayV1::decode(&impossible),
            Err(GeneralInvocationErrorV1::Replay)
        );
    }
}
