// SPDX-License-Identifier: AGPL-3.0-or-later

//! Canonical purpose-owned Replay envelope paired with global Position V3.
//!
//! Retirement authenticates only this common prefix and the hash of the full
//! purpose extension. General, Dealer, Series, and structured-claim handlers
//! remain the sole interpreters of their extensions and the sole authorities
//! allowed to move a live envelope to `Terminal` after exhausting their own
//! child graphs. A terminal envelope is necessary but never sufficient runtime
//! authority: the adapter must still authenticate program owner, exact PDA,
//! writable role, full account length, and the purpose owner's terminal join.

use crate::{
    retirement_error_v2_from_v1, DeletableRentOwnerV1, Identity32V1, PositionPurposeV3,
    RetirementErrorV2, DELETABLE_RENT_OWNER_V1_BYTES, IDENTITY_BYTES, PURPOSE_REPLAY_ACCOUNT_TAG,
    PURPOSE_REPLAY_ACCOUNT_VERSION_V3,
};

/// Canonical stable Replay PDA domain paired with Position V3.
pub const PURPOSE_REPLAY_V3_PDA_PREFIX: &[u8] = b"dc-purpose-replay-v3";
/// Domain for hashing an exact purpose extension.
pub const PURPOSE_REPLAY_V3_EXTENSION_HASH_DOMAIN: &[u8] =
    b"dragons-clutch/purpose-replay-v3/extension\0";
/// Domain for the semantic identity of the full envelope and extension.
pub const PURPOSE_REPLAY_V3_SEMANTIC_DOMAIN: &[u8] = b"dragons-clutch/purpose-replay-v3/account\0";

/// Exact common prefix before the purpose-owned extension.
///
/// Layout: 32-byte header, four full identities, and the 48-byte deletable
/// rent owner. The extension begins at byte 208 and its exact length is stored
/// in the header.
pub const PURPOSE_REPLAY_V3_PREFIX_BYTES: usize =
    32 + (4 * IDENTITY_BYTES) + DELETABLE_RENT_OWNER_V1_BYTES;

const POSITION_ACCOUNT_OFFSET: usize = 32;
const REPLAY_ACCOUNT_OFFSET: usize = POSITION_ACCOUNT_OFFSET + IDENTITY_BYTES;
const PURPOSE_BINDING_OFFSET: usize = REPLAY_ACCOUNT_OFFSET + IDENTITY_BYTES;
const EXTENSION_HASH_OFFSET: usize = PURPOSE_BINDING_OFFSET + IDENTITY_BYTES;
const RENT_OFFSET: usize = EXTENSION_HASH_OFFSET + IDENTITY_BYTES;

const _: () = assert!(PURPOSE_REPLAY_V3_PREFIX_BYTES == 208);
const _: () = assert!(RENT_OFFSET == 160);

/// Injected allocation-free hash boundary used by Replay V3 codecs.
pub trait ReplayV3HashBackend {
    /// Compute SHA-256 over the exact ordered byte slices.
    fn sha256_parts(&self, parts: &[&[u8]]) -> [u8; 32];
}

/// Exact purpose-extension schema coordinate.
///
/// Schema zero is permanently invalid. Purpose owners allocate a nonzero
/// coordinate and must reject any other schema or length before interpreting
/// their extension.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct ReplayV3ExtensionSchema(u32);

impl ReplayV3ExtensionSchema {
    /// Construct one nonzero exact extension schema coordinate.
    pub const fn new(value: u32) -> Result<Self, RetirementErrorV2> {
        if value == 0 {
            Err(RetirementErrorV2::NonCanonicalState)
        } else {
            Ok(Self(value))
        }
    }

    /// Return the exact persisted coordinate.
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Common Replay lifecycle. Purpose handlers alone authorize Terminal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ReplayV3Lifecycle {
    /// Purpose transitions may still advance the exact ordinal.
    Live = 1,
    /// Purpose child graphs are exhausted and economic mutation is disabled.
    Terminal = 2,
}

impl ReplayV3Lifecycle {
    fn decode(value: u8) -> Result<Self, RetirementErrorV2> {
        match value {
            1 => Ok(Self::Live),
            2 => Ok(Self::Terminal),
            _ => Err(RetirementErrorV2::InvalidEnum),
        }
    }
}

/// Immutable and monotone common fields supplied at founding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReplayV3EnvelopeFields {
    /// Exact canonical Position V3 account paired with this Replay.
    pub position_account: Identity32V1,
    /// Exact Replay V3 account key. It is retained in its own authenticated body.
    pub replay_account: Identity32V1,
    /// Position purpose selected by the semantic owner.
    pub purpose: PositionPurposeV3,
    /// Exact purpose binding, equal to the paired Position V3 field.
    pub purpose_binding_id: Identity32V1,
    /// Nonzero current Position generation.
    pub position_generation: u64,
    /// Exact ordinal expected by the next purpose transition.
    pub next_sequence: u64,
    /// Canonical stored Replay PDA bump.
    pub stored_bump: u8,
    /// Independently paid Replay rent principal and donation floor.
    pub rent: DeletableRentOwnerV1,
}

impl ReplayV3EnvelopeFields {
    fn validate(self) -> Result<(), RetirementErrorV2> {
        if self.position_account == self.replay_account || self.position_generation == 0 {
            return Err(RetirementErrorV2::NonCanonicalState);
        }
        self.rent.validate()
    }
}

/// Owned, authenticated common prefix for one exact purpose extension.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReplayV3EnvelopeHeader {
    fields: ReplayV3EnvelopeFields,
    lifecycle: ReplayV3Lifecycle,
    extension_schema: ReplayV3ExtensionSchema,
    extension_len: u32,
    extension_hash: Identity32V1,
}

impl ReplayV3EnvelopeHeader {
    /// Construct the unique live common prefix for exact extension bytes.
    pub fn new_live<B: ReplayV3HashBackend>(
        fields: ReplayV3EnvelopeFields,
        extension_schema: ReplayV3ExtensionSchema,
        extension: &[u8],
        backend: &B,
    ) -> Result<Self, RetirementErrorV2> {
        fields.validate()?;
        let extension_len = canonical_extension_len(extension)?;
        let extension_hash = extension_hash(
            fields.purpose,
            extension_schema,
            extension_len,
            extension,
            backend,
        )?;
        let value = Self {
            fields,
            lifecycle: ReplayV3Lifecycle::Live,
            extension_schema,
            extension_len,
            extension_hash,
        };
        value.validate()?;
        Ok(value)
    }

    fn validate(self) -> Result<(), RetirementErrorV2> {
        self.fields.validate()?;
        if self.extension_len == 0
            || (self.lifecycle == ReplayV3Lifecycle::Terminal && self.fields.next_sequence == 0)
        {
            return Err(RetirementErrorV2::NonCanonicalState);
        }
        Ok(())
    }

    /// Advance one purpose-owned transition and retain a live envelope.
    ///
    /// The purpose handler must first authenticate its extension and all
    /// external semantic owners. The common codec independently enforces an
    /// exact ordinal increment, a stable schema/length, and a Position
    /// generation that either stays fixed or advances by exactly one.
    pub fn advanced_live<B: ReplayV3HashBackend>(
        self,
        next_position_generation: u64,
        next_extension: &[u8],
        backend: &B,
    ) -> Result<Self, RetirementErrorV2> {
        self.transitioned(
            ReplayV3Lifecycle::Live,
            next_position_generation,
            next_extension,
            backend,
        )
    }

    /// Advance one exact transition and seal the envelope Terminal.
    ///
    /// Calling this pure transition does not authenticate a Dealer/General/
    /// Series/claim terminal state. The purpose handler is the sole authority
    /// for that prerequisite and must expose a private-field capability before
    /// a runtime adapter commits the returned bytes.
    pub fn terminalized<B: ReplayV3HashBackend>(
        self,
        next_position_generation: u64,
        terminal_extension: &[u8],
        backend: &B,
    ) -> Result<Self, RetirementErrorV2> {
        self.transitioned(
            ReplayV3Lifecycle::Terminal,
            next_position_generation,
            terminal_extension,
            backend,
        )
    }

    fn transitioned<B: ReplayV3HashBackend>(
        self,
        lifecycle: ReplayV3Lifecycle,
        next_position_generation: u64,
        extension: &[u8],
        backend: &B,
    ) -> Result<Self, RetirementErrorV2> {
        self.validate()?;
        if self.lifecycle != ReplayV3Lifecycle::Live {
            return Err(RetirementErrorV2::AlreadyTerminal);
        }
        let generation_plus_one = self
            .fields
            .position_generation
            .checked_add(1)
            .ok_or(RetirementErrorV2::ArithmeticOverflow)?;
        if next_position_generation != self.fields.position_generation
            && next_position_generation != generation_plus_one
        {
            return Err(RetirementErrorV2::WrongGeneration);
        }
        let extension_len = canonical_extension_len(extension)?;
        if extension_len != self.extension_len {
            return Err(RetirementErrorV2::NonCanonicalState);
        }
        let mut fields = self.fields;
        fields.position_generation = next_position_generation;
        fields.next_sequence = fields
            .next_sequence
            .checked_add(1)
            .ok_or(RetirementErrorV2::ArithmeticOverflow)?;
        let next = Self {
            fields,
            lifecycle,
            extension_schema: self.extension_schema,
            extension_len,
            extension_hash: extension_hash(
                fields.purpose,
                self.extension_schema,
                extension_len,
                extension,
                backend,
            )?,
        };
        next.validate()?;
        Ok(next)
    }

    /// Exact canonical Position account.
    pub const fn position_account(self) -> Identity32V1 {
        self.fields.position_account
    }

    /// Exact canonical Replay account retained in the body.
    pub const fn replay_account(self) -> Identity32V1 {
        self.fields.replay_account
    }

    /// Position purpose selecting the extension interpreter.
    pub const fn purpose(self) -> PositionPurposeV3 {
        self.fields.purpose
    }

    /// Exact purpose binding shared with Position V3.
    pub const fn purpose_binding_id(self) -> Identity32V1 {
        self.fields.purpose_binding_id
    }

    /// Current nonzero Position generation.
    pub const fn position_generation(self) -> u64 {
        self.fields.position_generation
    }

    /// Exact ordinal expected by the next purpose transition.
    pub const fn next_sequence(self) -> u64 {
        self.fields.next_sequence
    }

    /// Canonical stored Replay bump.
    pub const fn stored_bump(self) -> u8 {
        self.fields.stored_bump
    }

    /// Common Replay lifecycle.
    pub const fn lifecycle(self) -> ReplayV3Lifecycle {
        self.lifecycle
    }

    /// Exact purpose-extension schema coordinate.
    pub const fn extension_schema(self) -> ReplayV3ExtensionSchema {
        self.extension_schema
    }

    /// Exact purpose-extension byte length.
    pub const fn extension_len(self) -> u32 {
        self.extension_len
    }

    /// Hash of the full domain-separated purpose extension.
    pub const fn extension_hash(self) -> Identity32V1 {
        self.extension_hash
    }

    /// Independently funded Replay rent owner.
    pub const fn rent(self) -> DeletableRentOwnerV1 {
        self.fields.rent
    }

    /// Stable PDA seed facts plus the stored bump.
    pub const fn pda_seeds(self) -> ReplayV3PdaSeeds {
        ReplayV3PdaSeeds {
            position_account: self.fields.position_account,
            purpose: self.fields.purpose,
            purpose_binding_id: self.fields.purpose_binding_id,
            stored_bump: self.fields.stored_bump,
        }
    }
}

/// Borrowed exact envelope after full prefix, length, and extension-hash checks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReplayV3Envelope<'a> {
    header: ReplayV3EnvelopeHeader,
    extension: &'a [u8],
}

impl<'a> ReplayV3Envelope<'a> {
    /// Join an owned header to the exact extension it commits.
    pub fn from_header<B: ReplayV3HashBackend>(
        header: ReplayV3EnvelopeHeader,
        extension: &'a [u8],
        backend: &B,
    ) -> Result<Self, RetirementErrorV2> {
        header.validate()?;
        require_extension(header, extension, backend)?;
        Ok(Self { header, extension })
    }

    /// Decode an exact hostile account body and authenticate its full extension hash.
    pub fn decode<B: ReplayV3HashBackend>(
        input: &'a [u8],
        backend: &B,
    ) -> Result<Self, RetirementErrorV2> {
        if input.len() < PURPOSE_REPLAY_V3_PREFIX_BYTES {
            return Err(RetirementErrorV2::Truncated);
        }
        if input[0] != PURPOSE_REPLAY_ACCOUNT_TAG {
            return Err(RetirementErrorV2::WrongTag);
        }
        if input[1] != PURPOSE_REPLAY_ACCOUNT_VERSION_V3 {
            return Err(RetirementErrorV2::WrongVersion);
        }
        if input[5..8] != [0; 3] {
            return Err(RetirementErrorV2::NonCanonicalState);
        }
        let extension_len = read_u32(input, 24);
        let exact_len = PURPOSE_REPLAY_V3_PREFIX_BYTES
            .checked_add(
                usize::try_from(extension_len)
                    .map_err(|_| RetirementErrorV2::ArithmeticOverflow)?,
            )
            .ok_or(RetirementErrorV2::ArithmeticOverflow)?;
        if input.len() < exact_len {
            return Err(RetirementErrorV2::Truncated);
        }
        if input.len() > exact_len {
            return Err(RetirementErrorV2::TrailingBytes);
        }
        let header = ReplayV3EnvelopeHeader {
            fields: ReplayV3EnvelopeFields {
                position_account: read_identity(input, POSITION_ACCOUNT_OFFSET)?,
                replay_account: read_identity(input, REPLAY_ACCOUNT_OFFSET)?,
                purpose: PositionPurposeV3::decode(input[2])?,
                purpose_binding_id: read_identity(input, PURPOSE_BINDING_OFFSET)?,
                position_generation: read_u64(input, 8),
                next_sequence: read_u64(input, 16),
                stored_bump: input[4],
                rent: DeletableRentOwnerV1::decode(
                    &input[RENT_OFFSET..PURPOSE_REPLAY_V3_PREFIX_BYTES],
                )?,
            },
            lifecycle: ReplayV3Lifecycle::decode(input[3])?,
            extension_schema: ReplayV3ExtensionSchema::new(read_u32(input, 28))?,
            extension_len,
            extension_hash: read_identity(input, EXTENSION_HASH_OFFSET)?,
        };
        let extension = &input[PURPOSE_REPLAY_V3_PREFIX_BYTES..];
        Self::from_header(header, extension, backend)
    }

    /// Encode the exact common prefix and full purpose extension.
    pub fn encode_into<B: ReplayV3HashBackend>(
        self,
        output: &mut [u8],
        backend: &B,
    ) -> Result<(), RetirementErrorV2> {
        require_extension(self.header, self.extension, backend)?;
        let exact_len = self.encoded_len()?;
        if output.len() < exact_len {
            return Err(RetirementErrorV2::Truncated);
        }
        if output.len() > exact_len {
            return Err(RetirementErrorV2::TrailingBytes);
        }
        output.fill(0);
        output[0] = PURPOSE_REPLAY_ACCOUNT_TAG;
        output[1] = PURPOSE_REPLAY_ACCOUNT_VERSION_V3;
        output[2] = u8::from(self.header.fields.purpose);
        output[3] = self.header.lifecycle as u8;
        output[4] = self.header.fields.stored_bump;
        output[8..16].copy_from_slice(&self.header.fields.position_generation.to_le_bytes());
        output[16..24].copy_from_slice(&self.header.fields.next_sequence.to_le_bytes());
        output[24..28].copy_from_slice(&self.header.extension_len.to_le_bytes());
        output[28..32].copy_from_slice(&self.header.extension_schema.get().to_le_bytes());
        for (offset, identity) in [
            (POSITION_ACCOUNT_OFFSET, self.header.fields.position_account),
            (REPLAY_ACCOUNT_OFFSET, self.header.fields.replay_account),
            (
                PURPOSE_BINDING_OFFSET,
                self.header.fields.purpose_binding_id,
            ),
            (EXTENSION_HASH_OFFSET, self.header.extension_hash),
        ] {
            output[offset..offset + IDENTITY_BYTES].copy_from_slice(&identity.bytes());
        }
        output[RENT_OFFSET..PURPOSE_REPLAY_V3_PREFIX_BYTES]
            .copy_from_slice(&self.header.fields.rent.encode()?);
        output[PURPOSE_REPLAY_V3_PREFIX_BYTES..].copy_from_slice(self.extension);
        Ok(())
    }

    /// Return the owned common prefix.
    pub const fn header(self) -> ReplayV3EnvelopeHeader {
        self.header
    }

    /// Return the exact full purpose extension.
    pub const fn extension(self) -> &'a [u8] {
        self.extension
    }

    /// Exact total account bytes for this schema variant.
    pub fn encoded_len(self) -> Result<usize, RetirementErrorV2> {
        PURPOSE_REPLAY_V3_PREFIX_BYTES
            .checked_add(self.extension.len())
            .ok_or(RetirementErrorV2::ArithmeticOverflow)
    }

    /// Semantic identity of the complete exact envelope and extension.
    pub fn semantic_id<B: ReplayV3HashBackend>(
        self,
        backend: &B,
    ) -> Result<Identity32V1, RetirementErrorV2> {
        let mut prefix = [0u8; PURPOSE_REPLAY_V3_PREFIX_BYTES];
        encode_prefix(self.header, &mut prefix)?;
        Identity32V1::new(backend.sha256_parts(&[
            PURPOSE_REPLAY_V3_SEMANTIC_DOMAIN,
            &prefix,
            self.extension,
        ]))
        .map_err(retirement_error_v2_from_v1)
    }

    /// Mint the only retirement-facing terminal projection.
    pub fn terminal_projection(self) -> Result<ReplayV3TerminalProjection<'a>, RetirementErrorV2> {
        if self.header.lifecycle != ReplayV3Lifecycle::Terminal
            || self.header.fields.next_sequence == 0
        {
            return Err(RetirementErrorV2::WrongPhase);
        }
        Ok(ReplayV3TerminalProjection { envelope: self })
    }
}

/// Opaque proof that the common envelope is terminal and its extension hash matched.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReplayV3TerminalProjection<'a> {
    envelope: ReplayV3Envelope<'a>,
}

impl<'a> ReplayV3TerminalProjection<'a> {
    /// Exact terminal common prefix.
    pub const fn header(self) -> ReplayV3EnvelopeHeader {
        self.envelope.header
    }

    /// Exact full terminal extension, still interpreted only by its purpose owner.
    pub const fn extension(self) -> &'a [u8] {
        self.envelope.extension
    }
}

/// Stable canonical Replay V3 PDA seed facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReplayV3PdaSeeds {
    position_account: Identity32V1,
    purpose: PositionPurposeV3,
    purpose_binding_id: Identity32V1,
    stored_bump: u8,
}

impl ReplayV3PdaSeeds {
    /// Exact paired Position account seed.
    pub const fn position_account(self) -> Identity32V1 {
        self.position_account
    }

    /// One-byte purpose seed.
    pub const fn purpose(self) -> PositionPurposeV3 {
        self.purpose
    }

    /// Exact purpose binding seed.
    pub const fn purpose_binding_id(self) -> Identity32V1 {
        self.purpose_binding_id
    }

    /// Canonical stored bump.
    pub const fn stored_bump(self) -> u8 {
        self.stored_bump
    }
}

fn canonical_extension_len(extension: &[u8]) -> Result<u32, RetirementErrorV2> {
    if extension.is_empty() {
        return Err(RetirementErrorV2::NonCanonicalState);
    }
    u32::try_from(extension.len()).map_err(|_| RetirementErrorV2::ArithmeticOverflow)
}

fn extension_hash<B: ReplayV3HashBackend>(
    purpose: PositionPurposeV3,
    schema: ReplayV3ExtensionSchema,
    extension_len: u32,
    extension: &[u8],
    backend: &B,
) -> Result<Identity32V1, RetirementErrorV2> {
    let purpose_byte = [u8::from(purpose)];
    let schema_bytes = schema.get().to_le_bytes();
    let length_bytes = extension_len.to_le_bytes();
    Identity32V1::new(backend.sha256_parts(&[
        PURPOSE_REPLAY_V3_EXTENSION_HASH_DOMAIN,
        &purpose_byte,
        &schema_bytes,
        &length_bytes,
        extension,
    ]))
    .map_err(retirement_error_v2_from_v1)
}

fn require_extension<B: ReplayV3HashBackend>(
    header: ReplayV3EnvelopeHeader,
    extension: &[u8],
    backend: &B,
) -> Result<(), RetirementErrorV2> {
    header.validate()?;
    let extension_len = canonical_extension_len(extension)?;
    if extension_len != header.extension_len
        || extension_hash(
            header.fields.purpose,
            header.extension_schema,
            extension_len,
            extension,
            backend,
        )? != header.extension_hash
    {
        return Err(RetirementErrorV2::ReplayMismatch);
    }
    Ok(())
}

fn encode_prefix(
    header: ReplayV3EnvelopeHeader,
    output: &mut [u8; PURPOSE_REPLAY_V3_PREFIX_BYTES],
) -> Result<(), RetirementErrorV2> {
    output.fill(0);
    output[0] = PURPOSE_REPLAY_ACCOUNT_TAG;
    output[1] = PURPOSE_REPLAY_ACCOUNT_VERSION_V3;
    output[2] = u8::from(header.fields.purpose);
    output[3] = header.lifecycle as u8;
    output[4] = header.fields.stored_bump;
    output[8..16].copy_from_slice(&header.fields.position_generation.to_le_bytes());
    output[16..24].copy_from_slice(&header.fields.next_sequence.to_le_bytes());
    output[24..28].copy_from_slice(&header.extension_len.to_le_bytes());
    output[28..32].copy_from_slice(&header.extension_schema.get().to_le_bytes());
    for (offset, identity) in [
        (POSITION_ACCOUNT_OFFSET, header.fields.position_account),
        (REPLAY_ACCOUNT_OFFSET, header.fields.replay_account),
        (PURPOSE_BINDING_OFFSET, header.fields.purpose_binding_id),
        (EXTENSION_HASH_OFFSET, header.extension_hash),
    ] {
        output[offset..offset + IDENTITY_BYTES].copy_from_slice(&identity.bytes());
    }
    output[RENT_OFFSET..PURPOSE_REPLAY_V3_PREFIX_BYTES]
        .copy_from_slice(&header.fields.rent.encode()?);
    Ok(())
}

fn read_u32(input: &[u8], offset: usize) -> u32 {
    let mut bytes = [0u8; 4];
    bytes.copy_from_slice(&input[offset..offset + 4]);
    u32::from_le_bytes(bytes)
}

fn read_u64(input: &[u8], offset: usize) -> u64 {
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&input[offset..offset + 8]);
    u64::from_le_bytes(bytes)
}

fn read_identity(input: &[u8], offset: usize) -> Result<Identity32V1, RetirementErrorV2> {
    let mut bytes = [0u8; IDENTITY_BYTES];
    bytes.copy_from_slice(&input[offset..offset + IDENTITY_BYTES]);
    Identity32V1::new(bytes).map_err(retirement_error_v2_from_v1)
}
