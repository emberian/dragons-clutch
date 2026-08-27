//! The signed attestation wire.
//!
//! Two message kinds are implemented, and they are exactly the two the record
//! transport profile uses: [`AccountObservationV1`] carried by
//! [`AttestationMessageV1`] (`DCLTRMA1`, one signer, one account), and
//! [`ObservationSetSealV1`] (`DCLTRSS1`, one signer, one completed set).
//!
//! The design document also draws a third kind, `RelayedObservationSetV1`
//! (`DCLTRMS1`), which carries an entire ordered set in one message for the
//! one-transaction profile.  **It is deliberately not implemented here.** The
//! only venue this family exists for does not qualify for that profile — its
//! four-account set needs 1,149 bytes against a 743-byte message budget — so an
//! implementation would be a second acceptance path with no consumer, which is
//! precisely the parallel-authority shape `AGENTS.md` forbids.  The transport
//! profile identity is still emitted, because the swap-path table in §4.10 is
//! about release identities rather than about implemented code, and
//! [`crate::release::RelayedAdapterConfigV1`] refuses it at admission.

use crate::{
    ADDRESS_BYTES, Error, RELAYED_ATTESTATION_ACCOUNT_SET_ID_OFFSET,
    RELAYED_ATTESTATION_DECODING_RULES_ID_OFFSET, RELAYED_ATTESTATION_HEAD_BYTES,
    RELAYED_ATTESTATION_MAGIC, RELAYED_ATTESTATION_MESSAGE_LEN_OFFSET,
    RELAYED_ATTESTATION_OBSERVED_CLUSTER_ID_OFFSET, RELAYED_ATTESTATION_OBSERVED_SLOT_OFFSET,
    RELAYED_ATTESTATION_RELAY_FAMILY_ID_OFFSET, RELAYED_ATTESTATION_RESERVED_OFFSET,
    RELAYED_ATTESTATION_SET_COUNT_OFFSET, RELAYED_ATTESTATION_SET_INDEX_OFFSET,
    RELAYED_OBSERVATION_DATA_LEN_OFFSET, RELAYED_OBSERVATION_EXECUTABLE_OFFSET,
    RELAYED_OBSERVATION_HEAD_BYTES, RELAYED_OBSERVATION_INLINE_LEN_OFFSET,
    RELAYED_OBSERVATION_KEY_OFFSET, RELAYED_OBSERVATION_LAMPORTS_OFFSET,
    RELAYED_OBSERVATION_OWNER_OFFSET, RELAYED_OBSERVATION_RESERVED_OFFSET,
    RELAYED_OBSERVATION_TAIL_DIGEST_OFFSET, RELAYED_SEAL_ACCOUNT_SET_ID_OFFSET, RELAYED_SEAL_BYTES,
    RELAYED_SEAL_MAGIC, RELAYED_SEAL_MESSAGE_LEN_OFFSET, RELAYED_SEAL_OBSERVED_CLUSTER_ID_OFFSET,
    RELAYED_SEAL_OBSERVED_SLOT_OFFSET, RELAYED_SEAL_RELAY_FAMILY_ID_OFFSET,
    RELAYED_SEAL_RESERVED_OFFSET, RELAYED_SEAL_RESERVED_TAIL_OFFSET, RELAYED_SEAL_SET_COUNT_OFFSET,
    RELAYED_SEAL_SET_DIGEST_OFFSET, Result, SHA256_EMPTY_DIGEST, array, base, header, one, put,
    require_nonzero, require_zero, slice, u16_at, u16_from, u32_at, u32_from, u64_at,
    variable_header,
};
use crate::{MAX_RELAYED_ACCOUNTS_V1, MAX_RELAYED_INLINE_BYTES_V1};

/// One account as the relayer read it, committing to the complete account.
///
/// `inline` is a release-pinned prefix and the digest covers `data[inline..]`,
/// so omitting bytes is a carriage decision and never a content decision.  A
/// fully inline body is not a variant: it is the case `inline.len() == data_len`
/// with the empty-string digest, which [`Self::expected_tail_digest_is_empty`]
/// reports and the adapter recomputes like any other.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountObservationV1<'a> {
    key: [u8; ADDRESS_BYTES],
    owner: [u8; ADDRESS_BYTES],
    lamports: u64,
    data_len: u32,
    inline: &'a [u8],
    executable: bool,
    tail_digest: [u8; 32],
}

impl<'a> AccountObservationV1<'a> {
    /// Construct one observation body from values read off a foreign cluster.
    pub fn new(
        key: [u8; ADDRESS_BYTES],
        owner: [u8; ADDRESS_BYTES],
        lamports: u64,
        data_len: u32,
        inline: &'a [u8],
        executable: bool,
        tail_digest: [u8; 32],
    ) -> Result<Self> {
        require_nonzero(&key)?;
        require_nonzero(&owner)?;
        if inline.len() > MAX_RELAYED_INLINE_BYTES_V1 || u32_from(inline.len())? > data_len {
            return Err(Error::InvalidInlineWidth);
        }
        Ok(Self {
            key,
            owner,
            lamports,
            data_len,
            inline,
            executable,
            tail_digest,
        })
    }

    /// Decode one observation body whose length is exactly its declared width.
    pub fn decode(bytes: &'a [u8]) -> Result<Self> {
        let inline_len = usize::from(u16_at(bytes, RELAYED_OBSERVATION_INLINE_LEN_OFFSET)?);
        let expected = RELAYED_OBSERVATION_HEAD_BYTES
            .checked_add(inline_len)
            .ok_or(Error::ArithmeticOverflow)?;
        if bytes.len() != expected {
            return Err(Error::InvalidLength);
        }
        Self::decode_prefix(bytes).map(|(body, _)| body)
    }

    /// Decode one observation body from the front of a longer buffer, returning
    /// the body and the number of bytes it consumed.
    pub fn decode_prefix(bytes: &'a [u8]) -> Result<(Self, usize)> {
        require_zero(bytes, RELAYED_OBSERVATION_RESERVED_OFFSET, 1)?;
        let executable = match one(bytes, RELAYED_OBSERVATION_EXECUTABLE_OFFSET)? {
            0 => false,
            1 => true,
            _ => return Err(Error::NonCanonicalReservedBytes),
        };
        let inline_len = usize::from(u16_at(bytes, RELAYED_OBSERVATION_INLINE_LEN_OFFSET)?);
        let consumed = RELAYED_OBSERVATION_HEAD_BYTES
            .checked_add(inline_len)
            .ok_or(Error::ArithmeticOverflow)?;
        let body = Self::new(
            array(bytes, RELAYED_OBSERVATION_KEY_OFFSET)?,
            array(bytes, RELAYED_OBSERVATION_OWNER_OFFSET)?,
            u64_at(bytes, RELAYED_OBSERVATION_LAMPORTS_OFFSET)?,
            u32_at(bytes, RELAYED_OBSERVATION_DATA_LEN_OFFSET)?,
            slice(bytes, RELAYED_OBSERVATION_HEAD_BYTES, inline_len)?,
            executable,
            array(bytes, RELAYED_OBSERVATION_TAIL_DIGEST_OFFSET)?,
        )?;
        Ok((body, consumed))
    }

    /// Exact encoded width of this body.
    pub fn encoded_len(self) -> usize {
        RELAYED_OBSERVATION_HEAD_BYTES.saturating_add(self.inline.len())
    }

    /// Encode this body into the front of a caller-owned buffer.
    pub fn encode_into(self, output: &mut [u8]) -> Result<usize> {
        let width = self.encoded_len();
        if output.len() < width {
            return Err(Error::OutputLength);
        }
        put(output, RELAYED_OBSERVATION_KEY_OFFSET, &self.key)?;
        put(output, RELAYED_OBSERVATION_OWNER_OFFSET, &self.owner)?;
        put(
            output,
            RELAYED_OBSERVATION_LAMPORTS_OFFSET,
            &self.lamports.to_le_bytes(),
        )?;
        put(
            output,
            RELAYED_OBSERVATION_DATA_LEN_OFFSET,
            &self.data_len.to_le_bytes(),
        )?;
        put(
            output,
            RELAYED_OBSERVATION_INLINE_LEN_OFFSET,
            &u16_from(self.inline.len())?.to_le_bytes(),
        )?;
        put(
            output,
            RELAYED_OBSERVATION_EXECUTABLE_OFFSET,
            &[u8::from(self.executable)],
        )?;
        put(output, RELAYED_OBSERVATION_RESERVED_OFFSET, &[0])?;
        put(
            output,
            RELAYED_OBSERVATION_TAIL_DIGEST_OFFSET,
            &self.tail_digest,
        )?;
        put(output, RELAYED_OBSERVATION_HEAD_BYTES, self.inline)?;
        Ok(width)
    }

    /// The observed account address.
    pub const fn key(self) -> [u8; ADDRESS_BYTES] {
        self.key
    }
    /// The observed owning program.
    pub const fn owner(self) -> [u8; ADDRESS_BYTES] {
        self.owner
    }
    /// The observed lamport balance.
    pub const fn lamports(self) -> u64 {
        self.lamports
    }
    /// The complete on-chain data length as read.
    pub const fn data_len(self) -> u32 {
        self.data_len
    }
    /// The release-pinned inline prefix.
    pub const fn inline(self) -> &'a [u8] {
        self.inline
    }
    /// Whether the observed account was executable.
    pub const fn executable(self) -> bool {
        self.executable
    }
    /// The attested SHA-256 over `data[inline.len()..data_len]`.
    pub const fn tail_digest(self) -> [u8; 32] {
        self.tail_digest
    }

    /// Whether the body carries the account in full.
    pub fn is_fully_inline(self) -> bool {
        u32_from(self.inline.len()).is_ok_and(|inline| inline == self.data_len)
    }

    /// Whether the attested tail digest must be the empty-string digest.
    ///
    /// The adapter still recomputes; this only names the case so a test can
    /// assert that a fully inline body carrying a *different* digest refuses.
    pub fn expected_tail_digest_is_empty(self) -> bool {
        self.is_fully_inline()
    }

    /// Compare the attested tail digest against one the adapter recomputed.
    ///
    /// This crate hashes nothing.  A fully inline body is checked against the
    /// pinned empty-string digest without the caller having to know that rule.
    pub fn require_tail_digest(self, recomputed: [u8; 32]) -> Result<()> {
        let expected = if self.is_fully_inline() {
            SHA256_EMPTY_DIGEST
        } else {
            recomputed
        };
        if self.tail_digest != expected {
            return Err(Error::TailDigestMismatch);
        }
        Ok(())
    }

    /// Require the observed position to match the founding-time pin.
    pub fn require_pinned_position(
        self,
        expected_key: [u8; ADDRESS_BYTES],
        expected_owner: [u8; ADDRESS_BYTES],
        expected_inline_len: u16,
    ) -> Result<()> {
        if self.key != expected_key {
            return Err(Error::ObservedKeyMismatch);
        }
        if self.owner != expected_owner {
            return Err(Error::ObservedOwnerMismatch);
        }
        if u16_from(self.inline.len())? != expected_inline_len {
            return Err(Error::InvalidInlineWidth);
        }
        Ok(())
    }
}

/// One signer's attestation of one account in an ordered set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AttestationMessageV1<'a> {
    observed_cluster_id: [u8; 32],
    relay_family_id: [u8; 32],
    decoding_rules_id: [u8; 32],
    account_set_id: [u8; 32],
    observed_slot: u64,
    set_index: u16,
    set_count: u16,
    body: AccountObservationV1<'a>,
}

impl<'a> AttestationMessageV1<'a> {
    /// Construct one attestation message.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        observed_cluster_id: [u8; 32],
        relay_family_id: [u8; 32],
        decoding_rules_id: [u8; 32],
        account_set_id: [u8; 32],
        observed_slot: u64,
        set_index: u16,
        set_count: u16,
        body: AccountObservationV1<'a>,
    ) -> Result<Self> {
        require_nonzero(&observed_cluster_id)?;
        require_nonzero(&relay_family_id)?;
        require_nonzero(&decoding_rules_id)?;
        require_nonzero(&account_set_id)?;
        require_set_geometry(set_index, set_count)?;
        Ok(Self {
            observed_cluster_id,
            relay_family_id,
            decoding_rules_id,
            account_set_id,
            observed_slot,
            set_index,
            set_count,
            body,
        })
    }

    /// Hostile-decode one attestation message whose declared `message_len` must
    /// equal the verified message length exactly.
    pub fn decode(bytes: &'a [u8]) -> Result<Self> {
        variable_header(bytes, RELAYED_ATTESTATION_MAGIC)?;
        require_zero(bytes, RELAYED_ATTESTATION_RESERVED_OFFSET, 2)?;
        if u32_at(bytes, RELAYED_ATTESTATION_MESSAGE_LEN_OFFSET)? != u32_from(bytes.len())? {
            return Err(Error::MessageLengthMismatch);
        }
        let tail = slice(
            bytes,
            RELAYED_ATTESTATION_HEAD_BYTES,
            bytes
                .len()
                .checked_sub(RELAYED_ATTESTATION_HEAD_BYTES)
                .ok_or(Error::InvalidLength)?,
        )?;
        let body = AccountObservationV1::decode(tail)?;
        Self::new(
            array(bytes, RELAYED_ATTESTATION_OBSERVED_CLUSTER_ID_OFFSET)?,
            array(bytes, RELAYED_ATTESTATION_RELAY_FAMILY_ID_OFFSET)?,
            array(bytes, RELAYED_ATTESTATION_DECODING_RULES_ID_OFFSET)?,
            array(bytes, RELAYED_ATTESTATION_ACCOUNT_SET_ID_OFFSET)?,
            u64_at(bytes, RELAYED_ATTESTATION_OBSERVED_SLOT_OFFSET)?,
            u16_at(bytes, RELAYED_ATTESTATION_SET_INDEX_OFFSET)?,
            u16_at(bytes, RELAYED_ATTESTATION_SET_COUNT_OFFSET)?,
            body,
        )
    }

    /// Exact encoded width of this message.
    pub fn encoded_len(self) -> usize {
        RELAYED_ATTESTATION_HEAD_BYTES.saturating_add(self.body.encoded_len())
    }

    /// Encode this message into a caller-owned buffer of its exact width.
    pub fn encode_into(self, output: &mut [u8]) -> Result<usize> {
        let width = self.encoded_len();
        if output.len() != width {
            return Err(Error::OutputLength);
        }
        let head = base::<RELAYED_ATTESTATION_HEAD_BYTES>(RELAYED_ATTESTATION_MAGIC)?;
        put(output, 0, &head)?;
        put(
            output,
            RELAYED_ATTESTATION_MESSAGE_LEN_OFFSET,
            &u32_from(width)?.to_le_bytes(),
        )?;
        put(
            output,
            RELAYED_ATTESTATION_OBSERVED_CLUSTER_ID_OFFSET,
            &self.observed_cluster_id,
        )?;
        put(
            output,
            RELAYED_ATTESTATION_RELAY_FAMILY_ID_OFFSET,
            &self.relay_family_id,
        )?;
        put(
            output,
            RELAYED_ATTESTATION_DECODING_RULES_ID_OFFSET,
            &self.decoding_rules_id,
        )?;
        put(
            output,
            RELAYED_ATTESTATION_ACCOUNT_SET_ID_OFFSET,
            &self.account_set_id,
        )?;
        put(
            output,
            RELAYED_ATTESTATION_OBSERVED_SLOT_OFFSET,
            &self.observed_slot.to_le_bytes(),
        )?;
        put(
            output,
            RELAYED_ATTESTATION_SET_INDEX_OFFSET,
            &self.set_index.to_le_bytes(),
        )?;
        put(
            output,
            RELAYED_ATTESTATION_SET_COUNT_OFFSET,
            &self.set_count.to_le_bytes(),
        )?;
        let tail = output
            .get_mut(RELAYED_ATTESTATION_HEAD_BYTES..)
            .ok_or(Error::OutputLength)?;
        self.body.encode_into(tail)?;
        Ok(width)
    }

    /// The signed genesis hash of the cluster the relayer read.
    pub const fn observed_cluster_id(self) -> [u8; 32] {
        self.observed_cluster_id
    }
    /// The provider family this message claims to belong to.
    pub const fn relay_family_id(self) -> [u8; 32] {
        self.relay_family_id
    }
    /// The decoding-rules identity this message echoes.
    pub const fn decoding_rules_id(self) -> [u8; 32] {
        self.decoding_rules_id
    }
    /// The founding-time pinned ordered account set.
    pub const fn account_set_id(self) -> [u8; 32] {
        self.account_set_id
    }
    /// The finalized slot the read was taken at.
    pub const fn observed_slot(self) -> u64 {
        self.observed_slot
    }
    /// The position of this account in the canonical ordered set.
    pub const fn set_index(self) -> u16 {
        self.set_index
    }
    /// The cardinality of the canonical ordered set.
    pub const fn set_count(self) -> u16 {
        self.set_count
    }
    /// The observation body.
    pub const fn body(self) -> AccountObservationV1<'a> {
        self.body
    }
}

/// One signer's seal over a completed ordered set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObservationSetSealV1 {
    observed_cluster_id: [u8; 32],
    relay_family_id: [u8; 32],
    account_set_id: [u8; 32],
    observed_slot: u64,
    set_count: u16,
    set_digest: [u8; 32],
}

impl ObservationSetSealV1 {
    /// Construct one seal message.
    pub fn new(
        observed_cluster_id: [u8; 32],
        relay_family_id: [u8; 32],
        account_set_id: [u8; 32],
        observed_slot: u64,
        set_count: u16,
        set_digest: [u8; 32],
    ) -> Result<Self> {
        require_nonzero(&observed_cluster_id)?;
        require_nonzero(&relay_family_id)?;
        require_nonzero(&account_set_id)?;
        require_nonzero(&set_digest)?;
        require_set_geometry(0, set_count)?;
        Ok(Self {
            observed_cluster_id,
            relay_family_id,
            account_set_id,
            observed_slot,
            set_count,
            set_digest,
        })
    }

    /// Hostile-decode one exactly 156-byte seal message.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        header(bytes, RELAYED_SEAL_BYTES, RELAYED_SEAL_MAGIC)?;
        require_zero(bytes, RELAYED_SEAL_RESERVED_OFFSET, 2)?;
        require_zero(bytes, RELAYED_SEAL_RESERVED_TAIL_OFFSET, 2)?;
        if u32_at(bytes, RELAYED_SEAL_MESSAGE_LEN_OFFSET)? != u32_from(RELAYED_SEAL_BYTES)? {
            return Err(Error::MessageLengthMismatch);
        }
        Self::new(
            array(bytes, RELAYED_SEAL_OBSERVED_CLUSTER_ID_OFFSET)?,
            array(bytes, RELAYED_SEAL_RELAY_FAMILY_ID_OFFSET)?,
            array(bytes, RELAYED_SEAL_ACCOUNT_SET_ID_OFFSET)?,
            u64_at(bytes, RELAYED_SEAL_OBSERVED_SLOT_OFFSET)?,
            u16_at(bytes, RELAYED_SEAL_SET_COUNT_OFFSET)?,
            array(bytes, RELAYED_SEAL_SET_DIGEST_OFFSET)?,
        )
    }

    /// Encode the exact canonical seal bytes.
    pub fn to_bytes(self) -> Result<[u8; RELAYED_SEAL_BYTES]> {
        let mut out = base::<RELAYED_SEAL_BYTES>(RELAYED_SEAL_MAGIC)?;
        put(
            &mut out,
            RELAYED_SEAL_MESSAGE_LEN_OFFSET,
            &u32_from(RELAYED_SEAL_BYTES)?.to_le_bytes(),
        )?;
        put(
            &mut out,
            RELAYED_SEAL_OBSERVED_CLUSTER_ID_OFFSET,
            &self.observed_cluster_id,
        )?;
        put(
            &mut out,
            RELAYED_SEAL_RELAY_FAMILY_ID_OFFSET,
            &self.relay_family_id,
        )?;
        put(
            &mut out,
            RELAYED_SEAL_ACCOUNT_SET_ID_OFFSET,
            &self.account_set_id,
        )?;
        put(
            &mut out,
            RELAYED_SEAL_OBSERVED_SLOT_OFFSET,
            &self.observed_slot.to_le_bytes(),
        )?;
        put(
            &mut out,
            RELAYED_SEAL_SET_COUNT_OFFSET,
            &self.set_count.to_le_bytes(),
        )?;
        put(&mut out, RELAYED_SEAL_SET_DIGEST_OFFSET, &self.set_digest)?;
        Ok(out)
    }

    /// The signed genesis hash of the cluster the relayer read.
    pub const fn observed_cluster_id(self) -> [u8; 32] {
        self.observed_cluster_id
    }
    /// The provider family this seal claims to belong to.
    pub const fn relay_family_id(self) -> [u8; 32] {
        self.relay_family_id
    }
    /// The founding-time pinned ordered account set.
    pub const fn account_set_id(self) -> [u8; 32] {
        self.account_set_id
    }
    /// The finalized slot the sealed reads were taken at.
    pub const fn observed_slot(self) -> u64 {
        self.observed_slot
    }
    /// The cardinality of the sealed set.
    pub const fn set_count(self) -> u16 {
        self.set_count
    }
    /// The running fold over the accepted bodies.
    pub const fn set_digest(self) -> [u8; 32] {
        self.set_digest
    }
}

fn require_set_geometry(set_index: u16, set_count: u16) -> Result<()> {
    let count = usize::from(set_count);
    if count == 0 || count > MAX_RELAYED_ACCOUNTS_V1 || set_index >= set_count {
        return Err(Error::InvalidSetGeometry);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        RELAYED_ATTESTATION_EXAMPLE, RELAYED_ATTESTATION_REFUSAL_CORPUS,
        RELAYED_ATTESTATION_REFUSAL_CORPUS_CANONICAL_WIDTH,
        RELAYED_ATTESTATION_REFUSAL_CORPUS_COUNT, RELAYED_SEAL_EXAMPLE,
        RELAYED_SEAL_REFUSAL_CORPUS, RELAYED_SEAL_REFUSAL_CORPUS_CANONICAL_WIDTH,
        RELAYED_SEAL_REFUSAL_CORPUS_COUNT,
    };

    fn example_body() -> AccountObservationV1<'static> {
        AccountObservationV1::new(
            [7; 32],
            [9; 32],
            1_000_000,
            4,
            &[0xaa, 0xbb, 0xcc, 0xdd],
            false,
            [11; 32],
        )
        .expect("example body")
    }

    #[test]
    fn the_lean_example_decodes_and_reencodes_byte_for_byte() {
        let decoded = AttestationMessageV1::decode(&RELAYED_ATTESTATION_EXAMPLE)
            .expect("the generated example must decode");
        let mut out = [0u8; RELAYED_ATTESTATION_EXAMPLE.len()];
        decoded.encode_into(&mut out).expect("re-encode");
        assert_eq!(out, RELAYED_ATTESTATION_EXAMPLE);
    }

    #[test]
    fn the_lean_refusal_corpus_is_refused_by_the_rust_decoder() {
        assert_eq!(
            RELAYED_ATTESTATION_REFUSAL_CORPUS.len(),
            RELAYED_ATTESTATION_REFUSAL_CORPUS_COUNT,
            "the Rust side is iterating fewer entries than Lean emitted"
        );
        assert_eq!(
            RELAYED_ATTESTATION_REFUSAL_CORPUS_CANONICAL_WIDTH,
            RELAYED_ATTESTATION_EXAMPLE.len()
        );
        for (index, candidate) in RELAYED_ATTESTATION_REFUSAL_CORPUS.iter().enumerate() {
            assert!(
                AttestationMessageV1::decode(candidate).is_err(),
                "attestation corpus entry {index} was accepted"
            );
        }
    }

    #[test]
    fn every_truncation_of_the_example_refuses() {
        for width in 0..RELAYED_ATTESTATION_EXAMPLE.len() {
            let candidate = RELAYED_ATTESTATION_EXAMPLE
                .get(..width)
                .expect("prefix within the example");
            assert!(
                AttestationMessageV1::decode(candidate).is_err(),
                "a {width}-byte truncation was accepted"
            );
        }
    }

    #[test]
    fn a_trailing_byte_refuses_because_message_len_is_exact() {
        let mut extended = [0u8; RELAYED_ATTESTATION_EXAMPLE.len() + 1];
        extended
            .get_mut(..RELAYED_ATTESTATION_EXAMPLE.len())
            .expect("prefix")
            .copy_from_slice(&RELAYED_ATTESTATION_EXAMPLE);
        assert_eq!(
            AttestationMessageV1::decode(&extended),
            Err(Error::MessageLengthMismatch)
        );
    }

    #[test]
    fn the_seal_example_round_trips_and_its_corpus_refuses() {
        let decoded = ObservationSetSealV1::decode(&RELAYED_SEAL_EXAMPLE).expect("seal decodes");
        assert_eq!(decoded.to_bytes().expect("encode"), RELAYED_SEAL_EXAMPLE);
        assert_eq!(
            RELAYED_SEAL_REFUSAL_CORPUS.len(),
            RELAYED_SEAL_REFUSAL_CORPUS_COUNT
        );
        assert_eq!(
            RELAYED_SEAL_REFUSAL_CORPUS_CANONICAL_WIDTH,
            RELAYED_SEAL_BYTES
        );
        for (index, candidate) in RELAYED_SEAL_REFUSAL_CORPUS.iter().enumerate() {
            assert!(
                ObservationSetSealV1::decode(candidate).is_err(),
                "seal corpus entry {index} was accepted"
            );
        }
    }

    #[test]
    fn an_inline_prefix_wider_than_the_account_refuses() {
        assert_eq!(
            AccountObservationV1::new([7; 32], [9; 32], 0, 2, &[1, 2, 3], false, [0; 32]),
            Err(Error::InvalidInlineWidth)
        );
    }

    #[test]
    fn an_inline_prefix_above_the_release_ceiling_refuses() {
        let wide = [0u8; MAX_RELAYED_INLINE_BYTES_V1 + 1];
        assert_eq!(
            AccountObservationV1::new([7; 32], [9; 32], 0, 4_096, &wide, false, [0; 32]),
            Err(Error::InvalidInlineWidth)
        );
    }

    #[test]
    fn a_fully_inline_body_must_carry_the_empty_string_digest() {
        let full = AccountObservationV1::new(
            [7; 32],
            [9; 32],
            0,
            4,
            &[1, 2, 3, 4],
            false,
            SHA256_EMPTY_DIGEST,
        )
        .expect("body");
        assert!(full.is_fully_inline());
        // The recomputed argument is deliberately wrong: a fully inline body is
        // compared against the pinned empty-string digest, never against it.
        assert_eq!(full.require_tail_digest([0xff; 32]), Ok(()));

        let lying =
            AccountObservationV1::new([7; 32], [9; 32], 0, 4, &[1, 2, 3, 4], false, [0x11; 32])
                .expect("body");
        assert_eq!(
            lying.require_tail_digest(SHA256_EMPTY_DIGEST),
            Err(Error::TailDigestMismatch)
        );
    }

    #[test]
    fn a_partial_body_is_compared_against_the_adapters_recomputation() {
        let partial =
            AccountObservationV1::new([7; 32], [9; 32], 0, 64, &[1, 2, 3, 4], false, [0x22; 32])
                .expect("body");
        assert!(!partial.is_fully_inline());
        assert_eq!(partial.require_tail_digest([0x22; 32]), Ok(()));
        assert_eq!(
            partial.require_tail_digest([0x23; 32]),
            Err(Error::TailDigestMismatch)
        );
    }

    #[test]
    fn a_substituted_owner_refuses_on_the_owner_and_a_substituted_key_on_the_key() {
        let body = example_body();
        assert_eq!(
            body.require_pinned_position([8; 32], [9; 32], 4),
            Err(Error::ObservedKeyMismatch)
        );
        assert_eq!(
            body.require_pinned_position([7; 32], [10; 32], 4),
            Err(Error::ObservedOwnerMismatch)
        );
        assert_eq!(
            body.require_pinned_position([7; 32], [9; 32], 5),
            Err(Error::InvalidInlineWidth)
        );
        assert_eq!(body.require_pinned_position([7; 32], [9; 32], 4), Ok(()));
    }

    #[test]
    fn a_set_index_at_or_past_the_count_refuses() {
        let body = example_body();
        assert_eq!(
            AttestationMessageV1::new([3; 32], [4; 32], [5; 32], [6; 32], 1, 4, 4, body),
            Err(Error::InvalidSetGeometry)
        );
        assert_eq!(
            AttestationMessageV1::new(
                [3; 32],
                [4; 32],
                [5; 32],
                [6; 32],
                1,
                0,
                u16_from(MAX_RELAYED_ACCOUNTS_V1 + 1).expect("fits"),
                body
            ),
            Err(Error::InvalidSetGeometry)
        );
    }
}
