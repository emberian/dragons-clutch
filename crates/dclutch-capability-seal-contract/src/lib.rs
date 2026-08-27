//! Content-addressed validated-artifact seal for the Trading interpreter.
//!
//! Decision 0005 (`docs/decisions/0005-per-market-authentication-cache.md`) is
//! the sole owner of the argument this crate encodes. In one sentence: the
//! structural validity of a content-addressed artifact is a pure function of
//! the artifact bytes and the validator, so a Trading release may record its own
//! verdict once and honour it later, provided the bytes are re-pinned by their
//! own digest on every use and the validator's identity is part of the key.
//!
//! This crate owns three things and nothing else:
//!
//! 1. the persisted `SealedDescriptorClosureV1` byte layout and its hostile
//!    decoder;
//! 2. the canonical PDA seed projection for a seal account; and
//! 3. the invocation-scoped [`SealedArtifactV1`] token, which is the only way a
//!    validator's `from_sealed` constructor can be reached, and which cannot be
//!    constructed outside this crate.
//!
//! It deliberately depends on nothing. A seal is not a Market fact, not a
//! release-set fact, and not an account-model fact; it is a statement about
//! bytes.
//!
//! # What a seal does not do
//!
//! A seal never asserts that some account holds particular bytes. The consumer
//! recomputes `sha256(bytes)` and compares it with the digest the authenticated
//! descriptor names, exactly as it does without a seal, and only then may it
//! mint a token. A seal that named the wrong bytes would therefore be inert
//! rather than dangerous.

#![no_std]

/// Canonical PDA seed domain for one validated-artifact seal.
pub const CAPABILITY_SEAL_PDA_DOMAIN_V1: &[u8] = b"dclutch:capability-seal:v1";

// A Solana PDA seed is at most 32 bytes.
const _: () = assert!(CAPABILITY_SEAL_PDA_DOMAIN_V1.len() <= 32);

/// Exact persisted magic of one validated-artifact seal.
pub const CAPABILITY_SEAL_MAGIC_V1: [u8; 8] = *b"DCLTCSL1";
/// Exact persisted schema version.
pub const CAPABILITY_SEAL_SCHEMA_VERSION_V1: u16 = 1;
/// Exact persisted artifact profile.
pub const CAPABILITY_SEAL_PROFILE_V1: u16 = 1;

/// Offset of the persisted magic.
pub const CAPABILITY_SEAL_MAGIC_OFFSET_V1: usize = 0;
/// Offset of the persisted schema version.
pub const CAPABILITY_SEAL_SCHEMA_VERSION_OFFSET_V1: usize = 8;
/// Offset of the persisted artifact profile.
pub const CAPABILITY_SEAL_PROFILE_OFFSET_V1: usize = 10;
/// Offset of the persisted row count.
pub const CAPABILITY_SEAL_ROW_COUNT_OFFSET_V1: usize = 12;
/// Offset of the persisted verdict bitfield.
pub const CAPABILITY_SEAL_VERDICTS_OFFSET_V1: usize = 14;
/// Offset of the persisted action selector.
pub const CAPABILITY_SEAL_ACTION_OFFSET_V1: usize = 16;
/// Offset of the four canonical reserved bytes.
pub const CAPABILITY_SEAL_RESERVED_OFFSET_V1: usize = 20;
/// Offset of the persisted descriptor schema identity.
pub const CAPABILITY_SEAL_DESCRIPTOR_SCHEMA_OFFSET_V1: usize = 24;
/// Offset of the persisted descriptor content identity.
pub const CAPABILITY_SEAL_DESCRIPTOR_DIGEST_OFFSET_V1: usize = 56;
/// Offset of the persisted Trading interpreter semantic release.
pub const CAPABILITY_SEAL_TRADING_RELEASE_OFFSET_V1: usize = 88;
/// Exact persisted header width.
pub const CAPABILITY_SEAL_HEADER_BYTES_V1: usize = 120;

/// Offset of a row's role tag.
pub const CAPABILITY_SEAL_ROW_ROLE_OFFSET_V1: usize = 0;
/// Offset of a row's two canonical reserved bytes.
pub const CAPABILITY_SEAL_ROW_RESERVED_OFFSET_V1: usize = 2;
/// Offset of a row's exact raw-record width.
pub const CAPABILITY_SEAL_ROW_WIDTH_OFFSET_V1: usize = 4;
/// Offset of a row's schema identity.
pub const CAPABILITY_SEAL_ROW_SCHEMA_OFFSET_V1: usize = 8;
/// Offset of a row's content identity.
pub const CAPABILITY_SEAL_ROW_DIGEST_OFFSET_V1: usize = 40;
/// Offset of a row's canonical raw-record address.
pub const CAPABILITY_SEAL_ROW_RAW_OFFSET_V1: usize = 72;
/// Offset of a row's canonical staging-cursor address.
pub const CAPABILITY_SEAL_ROW_STAGING_OFFSET_V1: usize = 104;
/// Exact persisted row width.
pub const CAPABILITY_SEAL_ROW_BYTES_V1: usize = 136;

/// Exact number of canonical rows in artifact profile 1.
pub const CAPABILITY_SEAL_ROW_COUNT_V1: usize = 6;

/// Exact whole-account width of one artifact-profile-1 seal.
pub const CAPABILITY_SEAL_BYTES_V1: usize =
    CAPABILITY_SEAL_HEADER_BYTES_V1 + CAPABILITY_SEAL_ROW_COUNT_V1 * CAPABILITY_SEAL_ROW_BYTES_V1;

/// The exact verdict set artifact profile 1 asserts.
///
/// A seal is written whole or not at all, so this is a canonicality check and
/// not a policy knob: a body carrying any other value is refused. A future
/// profile that asserts a different set of propositions is a different profile.
pub const CAPABILITY_SEAL_VERDICTS_V1: u16 = 0x00ff;

/// Canonical role of one sealed artifact, and its canonical row ordinal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum SealedRoleV1 {
    /// The selected `CapabilityProgramV4` descriptor itself.
    Descriptor = 0,
    /// The descriptor's selected state-lifecycle policy.
    LifecyclePolicy = 1,
    /// The descriptor's selected account/register projection profile.
    AccountProfile = 2,
    /// The descriptor's selected request projection profile.
    RequestProfile = 3,
    /// The descriptor's selected checked transition program.
    TransitionProgram = 4,
    /// The descriptor's selected allowed-effect program.
    EffectProgram = 5,
}

impl SealedRoleV1 {
    /// Canonical row ordinal of this role.
    pub const fn ordinal(self) -> usize {
        self as usize
    }

    /// Persisted tag of this role.
    pub const fn tag(self) -> u16 {
        self as u16
    }

    /// Decode one persisted role tag.
    pub const fn decode(tag: u16) -> Result<Self> {
        match tag {
            0 => Ok(Self::Descriptor),
            1 => Ok(Self::LifecyclePolicy),
            2 => Ok(Self::AccountProfile),
            3 => Ok(Self::RequestProfile),
            4 => Ok(Self::TransitionProgram),
            5 => Ok(Self::EffectProgram),
            _ => Err(Error::UnknownRole),
        }
    }

    /// Every canonical role in row order.
    pub const fn canonical_order() -> [Self; CAPABILITY_SEAL_ROW_COUNT_V1] {
        [
            Self::Descriptor,
            Self::LifecyclePolicy,
            Self::AccountProfile,
            Self::RequestProfile,
            Self::TransitionProgram,
            Self::EffectProgram,
        ]
    }
}

/// Every refusal this contract can raise.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// The seal body is not exactly `CAPABILITY_SEAL_BYTES_V1` bytes.
    InvalidLength,
    /// The persisted magic is not `CAPABILITY_SEAL_MAGIC_V1`.
    InvalidMagic,
    /// The persisted schema version is not supported.
    UnsupportedSchema,
    /// The persisted artifact profile is not supported.
    UnsupportedArtifactProfile,
    /// A canonical reserved field is nonzero.
    NonCanonicalReserved,
    /// The persisted row count is not the artifact profile's exact count.
    InvalidRowCount,
    /// The persisted verdict set is not the artifact profile's exact set.
    InvalidVerdicts,
    /// A persisted role tag is not a canonical role.
    UnknownRole,
    /// A row does not carry its canonical role in its canonical position.
    NonCanonicalRowOrder,
    /// A persisted identity or address is the zero value.
    ZeroIdentity,
    /// A row's exact raw-record width is zero.
    ZeroRecordWidth,
    /// A row's schema or content identity is not the expected one.
    ArtifactIdentityMismatch,
    /// The seal does not describe the expected descriptor.
    DescriptorMismatch,
    /// The seal was written under a different Trading interpreter release.
    InterpreterReleaseMismatch,
    /// The seal was written for a different action selector.
    ActionMismatch,
    /// The observed record bytes are not the width the seal names.
    RecordWidthMismatch,
    /// The supplied token does not name the bytes it is being used for.
    TokenRangeMismatch,
    /// The supplied token names a different artifact role.
    TokenRoleMismatch,
}

/// Result of a seal-contract operation.
pub type Result<T> = core::result::Result<T, Error>;

/// One canonical persisted seal row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SealedRecordRowV1 {
    role: SealedRoleV1,
    exact_data_length: u32,
    schema: [u8; 32],
    content_digest: [u8; 32],
    raw_record_account: [u8; 32],
    staging_account: [u8; 32],
}

impl SealedRecordRowV1 {
    /// Construct one canonical row.
    pub fn new(
        role: SealedRoleV1,
        exact_data_length: u32,
        schema: [u8; 32],
        content_digest: [u8; 32],
        raw_record_account: [u8; 32],
        staging_account: [u8; 32],
    ) -> Result<Self> {
        if exact_data_length == 0 {
            return Err(Error::ZeroRecordWidth);
        }
        for identity in [
            &schema,
            &content_digest,
            &raw_record_account,
            &staging_account,
        ] {
            if identity.iter().all(|byte| *byte == 0) {
                return Err(Error::ZeroIdentity);
            }
        }
        if raw_record_account == staging_account {
            return Err(Error::ZeroIdentity);
        }
        Ok(Self {
            role,
            exact_data_length,
            schema,
            content_digest,
            raw_record_account,
            staging_account,
        })
    }

    /// Canonical role of this row.
    pub const fn role(self) -> SealedRoleV1 {
        self.role
    }
    /// Exact raw-record width the seal writer observed.
    pub const fn exact_data_length(self) -> u32 {
        self.exact_data_length
    }
    /// Schema identity the seal writer validated under.
    pub const fn schema(self) -> [u8; 32] {
        self.schema
    }
    /// Complete-body content identity the seal writer validated.
    pub const fn content_digest(self) -> [u8; 32] {
        self.content_digest
    }
    /// Canonical Registry raw-record address for this row.
    pub const fn raw_record_account(self) -> [u8; 32] {
        self.raw_record_account
    }
    /// Canonical Registry staging-cursor address for this row.
    pub const fn staging_account(self) -> [u8; 32] {
        self.staging_account
    }

    fn encode_into(self, output: &mut [u8]) -> Result<()> {
        let row = output
            .get_mut(..CAPABILITY_SEAL_ROW_BYTES_V1)
            .ok_or(Error::InvalidLength)?;
        put_u16(row, CAPABILITY_SEAL_ROW_ROLE_OFFSET_V1, self.role.tag())?;
        put_u32(
            row,
            CAPABILITY_SEAL_ROW_WIDTH_OFFSET_V1,
            self.exact_data_length,
        )?;
        copy(row, CAPABILITY_SEAL_ROW_SCHEMA_OFFSET_V1, &self.schema)?;
        copy(
            row,
            CAPABILITY_SEAL_ROW_DIGEST_OFFSET_V1,
            &self.content_digest,
        )?;
        copy(
            row,
            CAPABILITY_SEAL_ROW_RAW_OFFSET_V1,
            &self.raw_record_account,
        )?;
        copy(
            row,
            CAPABILITY_SEAL_ROW_STAGING_OFFSET_V1,
            &self.staging_account,
        )?;
        Ok(())
    }

    fn decode(bytes: &[u8], ordinal: usize) -> Result<Self> {
        let start = CAPABILITY_SEAL_HEADER_BYTES_V1
            .checked_add(
                ordinal
                    .checked_mul(CAPABILITY_SEAL_ROW_BYTES_V1)
                    .ok_or(Error::InvalidLength)?,
            )
            .ok_or(Error::InvalidLength)?;
        let row = slice(bytes, start, CAPABILITY_SEAL_ROW_BYTES_V1)?;
        require_zero(row, CAPABILITY_SEAL_ROW_RESERVED_OFFSET_V1, 2)?;
        let role = SealedRoleV1::decode(read_u16(row, CAPABILITY_SEAL_ROW_ROLE_OFFSET_V1)?)?;
        if role.ordinal() != ordinal {
            return Err(Error::NonCanonicalRowOrder);
        }
        Self::new(
            role,
            read_u32(row, CAPABILITY_SEAL_ROW_WIDTH_OFFSET_V1)?,
            read_array(row, CAPABILITY_SEAL_ROW_SCHEMA_OFFSET_V1)?,
            read_array(row, CAPABILITY_SEAL_ROW_DIGEST_OFFSET_V1)?,
            read_array(row, CAPABILITY_SEAL_ROW_RAW_OFFSET_V1)?,
            read_array(row, CAPABILITY_SEAL_ROW_STAGING_OFFSET_V1)?,
        )
    }
}

/// The four coordinates that address one seal account.
///
/// Every one of them is content: the descriptor names its own closure, the
/// action selects which of the policy's plan sets the ownership verdict covers,
/// and the interpreter release identifies the validator whose verdict this is.
/// No Market, generation, release set or capability root appears here, because
/// none of them appears in the proposition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilitySealKeyV1 {
    descriptor_schema: [u8; 32],
    descriptor_digest: [u8; 32],
    action: u32,
    trading_semantic_release: [u8; 32],
}

impl CapabilitySealKeyV1 {
    /// Construct one exact seal key.
    pub fn new(
        descriptor_schema: [u8; 32],
        descriptor_digest: [u8; 32],
        action: u32,
        trading_semantic_release: [u8; 32],
    ) -> Result<Self> {
        for identity in [
            &descriptor_schema,
            &descriptor_digest,
            &trading_semantic_release,
        ] {
            if identity.iter().all(|byte| *byte == 0) {
                return Err(Error::ZeroIdentity);
            }
        }
        Ok(Self {
            descriptor_schema,
            descriptor_digest,
            action,
            trading_semantic_release,
        })
    }

    /// Selected descriptor schema identity.
    pub const fn descriptor_schema(self) -> [u8; 32] {
        self.descriptor_schema
    }
    /// Selected descriptor content identity.
    pub const fn descriptor_digest(self) -> [u8; 32] {
        self.descriptor_digest
    }
    /// Selected action selector.
    pub const fn action(self) -> u32 {
        self.action
    }
    /// Trading interpreter semantic release whose verdict this seal is.
    pub const fn trading_semantic_release(self) -> [u8; 32] {
        self.trading_semantic_release
    }

    /// Return the sole canonical seal PDA seed projection.
    pub fn seeds(self) -> CapabilitySealSeedsV1 {
        CapabilitySealSeedsV1 {
            descriptor_schema: self.descriptor_schema,
            descriptor_digest: self.descriptor_digest,
            action: self.action.to_le_bytes(),
            trading_semantic_release: self.trading_semantic_release,
        }
    }
}

/// Owned exact seed projection for one seal account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilitySealSeedsV1 {
    descriptor_schema: [u8; 32],
    descriptor_digest: [u8; 32],
    action: [u8; 4],
    trading_semantic_release: [u8; 32],
}

impl CapabilitySealSeedsV1 {
    /// Return the exact seed order interpreted under the Trading Program ID.
    pub fn as_slices(&self) -> [&[u8]; 5] {
        [
            CAPABILITY_SEAL_PDA_DOMAIN_V1,
            &self.descriptor_schema,
            &self.descriptor_digest,
            &self.action,
            &self.trading_semantic_release,
        ]
    }
}

/// Borrowed exact view of one persisted seal account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SealedDescriptorClosureV1 {
    key: CapabilitySealKeyV1,
    rows: [SealedRecordRowV1; CAPABILITY_SEAL_ROW_COUNT_V1],
}

impl SealedDescriptorClosureV1 {
    /// Hostile-decode one exact persisted seal body.
    ///
    /// This validates only that the bytes are a canonical seal. It says nothing
    /// about whether the seal is the right seal; that is
    /// [`SealedDescriptorClosureV1::require_key`] and
    /// [`SealedDescriptorClosureV1::require_artifact`], and the caller must
    /// have derived the account's address from the same key.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != CAPABILITY_SEAL_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        if slice(
            bytes,
            CAPABILITY_SEAL_MAGIC_OFFSET_V1,
            CAPABILITY_SEAL_MAGIC_V1.len(),
        )? != CAPABILITY_SEAL_MAGIC_V1
        {
            return Err(Error::InvalidMagic);
        }
        if read_u16(bytes, CAPABILITY_SEAL_SCHEMA_VERSION_OFFSET_V1)?
            != CAPABILITY_SEAL_SCHEMA_VERSION_V1
        {
            return Err(Error::UnsupportedSchema);
        }
        if read_u16(bytes, CAPABILITY_SEAL_PROFILE_OFFSET_V1)? != CAPABILITY_SEAL_PROFILE_V1 {
            return Err(Error::UnsupportedArtifactProfile);
        }
        require_zero(bytes, CAPABILITY_SEAL_RESERVED_OFFSET_V1, 4)?;
        if usize::from(read_u16(bytes, CAPABILITY_SEAL_ROW_COUNT_OFFSET_V1)?)
            != CAPABILITY_SEAL_ROW_COUNT_V1
        {
            return Err(Error::InvalidRowCount);
        }
        if read_u16(bytes, CAPABILITY_SEAL_VERDICTS_OFFSET_V1)? != CAPABILITY_SEAL_VERDICTS_V1 {
            return Err(Error::InvalidVerdicts);
        }
        let key = CapabilitySealKeyV1::new(
            read_array(bytes, CAPABILITY_SEAL_DESCRIPTOR_SCHEMA_OFFSET_V1)?,
            read_array(bytes, CAPABILITY_SEAL_DESCRIPTOR_DIGEST_OFFSET_V1)?,
            read_u32(bytes, CAPABILITY_SEAL_ACTION_OFFSET_V1)?,
            read_array(bytes, CAPABILITY_SEAL_TRADING_RELEASE_OFFSET_V1)?,
        )?;
        let rows = [
            SealedRecordRowV1::decode(bytes, 0)?,
            SealedRecordRowV1::decode(bytes, 1)?,
            SealedRecordRowV1::decode(bytes, 2)?,
            SealedRecordRowV1::decode(bytes, 3)?,
            SealedRecordRowV1::decode(bytes, 4)?,
            SealedRecordRowV1::decode(bytes, 5)?,
        ];
        let descriptor = rows.first().ok_or(Error::NonCanonicalRowOrder)?;
        if descriptor.schema != key.descriptor_schema
            || descriptor.content_digest != key.descriptor_digest
        {
            return Err(Error::DescriptorMismatch);
        }
        Ok(Self { key, rows })
    }

    /// Encode one exact canonical seal body.
    pub fn encode(
        key: CapabilitySealKeyV1,
        rows: [SealedRecordRowV1; CAPABILITY_SEAL_ROW_COUNT_V1],
        output: &mut [u8],
    ) -> Result<()> {
        if output.len() != CAPABILITY_SEAL_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        for byte in output.iter_mut() {
            *byte = 0;
        }
        copy(
            output,
            CAPABILITY_SEAL_MAGIC_OFFSET_V1,
            &CAPABILITY_SEAL_MAGIC_V1,
        )?;
        put_u16(
            output,
            CAPABILITY_SEAL_SCHEMA_VERSION_OFFSET_V1,
            CAPABILITY_SEAL_SCHEMA_VERSION_V1,
        )?;
        put_u16(
            output,
            CAPABILITY_SEAL_PROFILE_OFFSET_V1,
            CAPABILITY_SEAL_PROFILE_V1,
        )?;
        put_u16(
            output,
            CAPABILITY_SEAL_ROW_COUNT_OFFSET_V1,
            u16::try_from(CAPABILITY_SEAL_ROW_COUNT_V1).map_err(|_| Error::InvalidRowCount)?,
        )?;
        put_u16(
            output,
            CAPABILITY_SEAL_VERDICTS_OFFSET_V1,
            CAPABILITY_SEAL_VERDICTS_V1,
        )?;
        put_u32(output, CAPABILITY_SEAL_ACTION_OFFSET_V1, key.action)?;
        copy(
            output,
            CAPABILITY_SEAL_DESCRIPTOR_SCHEMA_OFFSET_V1,
            &key.descriptor_schema,
        )?;
        copy(
            output,
            CAPABILITY_SEAL_DESCRIPTOR_DIGEST_OFFSET_V1,
            &key.descriptor_digest,
        )?;
        copy(
            output,
            CAPABILITY_SEAL_TRADING_RELEASE_OFFSET_V1,
            &key.trading_semantic_release,
        )?;
        for (ordinal, row) in rows.iter().enumerate() {
            if row.role.ordinal() != ordinal {
                return Err(Error::NonCanonicalRowOrder);
            }
            let start = CAPABILITY_SEAL_HEADER_BYTES_V1
                .checked_add(
                    ordinal
                        .checked_mul(CAPABILITY_SEAL_ROW_BYTES_V1)
                        .ok_or(Error::InvalidLength)?,
                )
                .ok_or(Error::InvalidLength)?;
            row.encode_into(output.get_mut(start..).ok_or(Error::InvalidLength)?)?;
        }
        if rows.first().is_some_and(|row| {
            row.schema != key.descriptor_schema || row.content_digest != key.descriptor_digest
        }) {
            return Err(Error::DescriptorMismatch);
        }
        Ok(())
    }

    /// The four coordinates this seal was written under.
    pub const fn key(self) -> CapabilitySealKeyV1 {
        self.key
    }

    /// Borrow the canonical row for one role.
    pub fn row(self, role: SealedRoleV1) -> Result<SealedRecordRowV1> {
        self.rows
            .get(role.ordinal())
            .copied()
            .ok_or(Error::NonCanonicalRowOrder)
    }

    /// Refuse unless this seal is exactly the one the consumer derived.
    ///
    /// The caller must independently have required the seal account's address
    /// to be the canonical PDA for `expected`. This check makes the persisted
    /// body agree with that derivation rather than trusting either alone.
    pub fn require_key(self, expected: CapabilitySealKeyV1) -> Result<()> {
        if self.key.descriptor_schema != expected.descriptor_schema
            || self.key.descriptor_digest != expected.descriptor_digest
        {
            return Err(Error::DescriptorMismatch);
        }
        if self.key.action != expected.action {
            return Err(Error::ActionMismatch);
        }
        if self.key.trading_semantic_release != expected.trading_semantic_release {
            return Err(Error::InterpreterReleaseMismatch);
        }
        Ok(())
    }

    /// Mint the invocation-scoped token for this seal's policy/profile join.
    pub fn authenticate_profile_join<'a>(
        self,
        policy: SealedArtifactV1<'a>,
        profile: SealedArtifactV1<'a>,
    ) -> Result<SealedProfileJoinV1<'a>> {
        self.require_own_token(policy, SealedRoleV1::LifecyclePolicy)?;
        self.require_own_token(profile, SealedRoleV1::AccountProfile)?;
        Ok(SealedProfileJoinV1 {
            policy: policy.bytes,
            profile: profile.bytes,
        })
    }

    /// Mint the invocation-scoped token for this seal's ownership conjunction.
    pub fn authenticate_static_ownership<'a>(
        self,
        profile: SealedArtifactV1<'a>,
        policy: SealedArtifactV1<'a>,
        request: SealedArtifactV1<'a>,
        transition: SealedArtifactV1<'a>,
    ) -> Result<SealedStaticOwnershipV1<'a>> {
        self.require_own_token(profile, SealedRoleV1::AccountProfile)?;
        self.require_own_token(policy, SealedRoleV1::LifecyclePolicy)?;
        self.require_own_token(request, SealedRoleV1::RequestProfile)?;
        self.require_own_token(transition, SealedRoleV1::TransitionProgram)?;
        Ok(SealedStaticOwnershipV1 {
            action: self.key.action,
            profile: profile.bytes,
            policy: policy.bytes,
            request: request.bytes,
            transition: transition.bytes,
        })
    }

    /// Refuse unless `token` is this seal's own token for exactly `role`.
    fn require_own_token(self, token: SealedArtifactV1<'_>, role: SealedRoleV1) -> Result<()> {
        if token.role != role {
            return Err(Error::TokenRoleMismatch);
        }
        if token.seal != self.key.descriptor_digest {
            return Err(Error::DescriptorMismatch);
        }
        Ok(())
    }

    /// Mint the invocation-scoped token for one sealed artifact.
    ///
    /// `schema` and `content_digest` must be the identities the *authenticated
    /// descriptor* names for `role`, never identities taken from the seal or
    /// supplied by a caller, and `bytes` must be the record body whose live
    /// `sha256` the caller has just compared against `content_digest`. Those
    /// two obligations are what make the returned token mean what it says; this
    /// crate cannot check either, so it names them here and keeps the token's
    /// constructor private so no other path can mint one.
    pub fn authenticate_artifact<'a>(
        self,
        role: SealedRoleV1,
        schema: [u8; 32],
        content_digest: [u8; 32],
        bytes: &'a [u8],
    ) -> Result<SealedArtifactV1<'a>> {
        let row = self.row(role)?;
        if row.schema != schema || row.content_digest != content_digest {
            return Err(Error::ArtifactIdentityMismatch);
        }
        if usize::try_from(row.exact_data_length).map_err(|_| Error::RecordWidthMismatch)?
            != bytes.len()
        {
            return Err(Error::RecordWidthMismatch);
        }
        Ok(SealedArtifactV1 {
            seal: self.key.descriptor_digest,
            role,
            bytes,
        })
    }
}

/// Invocation-scoped proof that one exact byte range is a validated artifact.
///
/// This is the whole trust interface of the seal. A validator's `from_sealed`
/// constructor accepts it only for the very bytes it names -- same address, same
/// length -- and only for its own role, so a token proved for one artifact can
/// never carry into a view over another. It has no wire encoding, cannot be
/// constructed outside this crate, and cannot outlive the borrow of the record
/// body whose digest pinned it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SealedArtifactV1<'a> {
    seal: [u8; 32],
    role: SealedRoleV1,
    bytes: &'a [u8],
}

impl<'a> SealedArtifactV1<'a> {
    /// The descriptor digest of the seal that minted this token.
    ///
    /// Two tokens from two different seals are two verdicts about two different
    /// closures. Every join below requires its operands to carry the same seal
    /// identity, so no caller can assemble a join out of halves.
    pub const fn seal(self) -> [u8; 32] {
        self.seal
    }

    /// The role this token was minted for.
    pub const fn role(self) -> SealedRoleV1 {
        self.role
    }

    /// Borrow the exact byte range this token was minted for.
    pub const fn bytes(self) -> &'a [u8] {
        self.bytes
    }

    /// Refuse unless this token names exactly `bytes` under exactly `role`.
    ///
    /// Pointer identity, not equality: a byte-identical artifact at another
    /// address is a different artifact for this purpose, because the caller
    /// pinned only the range it hashed.
    pub fn require(self, role: SealedRoleV1, bytes: &[u8]) -> Result<()> {
        if self.role != role {
            return Err(Error::TokenRoleMismatch);
        }
        if !core::ptr::eq(self.bytes.as_ptr(), bytes.as_ptr()) || self.bytes.len() != bytes.len() {
            return Err(Error::TokenRangeMismatch);
        }
        Ok(())
    }
}

/// Invocation-scoped proof that one policy's join to one account profile holds.
///
/// The lifecycle policy's join to the account profile is a fact about a *pair*
/// of artifacts, so it cannot live in either artifact's own row. It is minted
/// only from two tokens of one seal, which is what keeps a join proved for one
/// closure out of a plan over another.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SealedProfileJoinV1<'a> {
    policy: &'a [u8],
    profile: &'a [u8],
}

impl<'a> SealedProfileJoinV1<'a> {
    /// Borrow the exact policy byte range this join was proved for.
    pub const fn policy(self) -> &'a [u8] {
        self.policy
    }

    /// Borrow the exact account-profile byte range this join was proved for.
    pub const fn profile(self) -> &'a [u8] {
        self.profile
    }
}

/// Invocation-scoped proof of the static register-ownership conjunction.
///
/// This one is a fact about four artifacts *and* an action selector, which is
/// why the action is a seed of the seal rather than a field a caller supplies.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SealedStaticOwnershipV1<'a> {
    action: u32,
    profile: &'a [u8],
    policy: &'a [u8],
    request: &'a [u8],
    transition: &'a [u8],
}

impl SealedStaticOwnershipV1<'_> {
    /// Refuse unless this verdict covers exactly these four artifacts and this
    /// action.
    pub fn require(
        self,
        action: u32,
        profile: &[u8],
        policy: &[u8],
        request: &[u8],
        transition: &[u8],
    ) -> Result<()> {
        if self.action != action {
            return Err(Error::ActionMismatch);
        }
        for (proved, observed) in [
            (self.profile, profile),
            (self.policy, policy),
            (self.request, request),
            (self.transition, transition),
        ] {
            if !core::ptr::eq(proved.as_ptr(), observed.as_ptr()) || proved.len() != observed.len()
            {
                return Err(Error::TokenRangeMismatch);
            }
        }
        Ok(())
    }
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(read_array(bytes, offset)?))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(read_array(bytes, offset)?))
}

fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    slice(bytes, offset, N)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn slice(bytes: &[u8], offset: usize, width: usize) -> Result<&[u8]> {
    let end = offset.checked_add(width).ok_or(Error::InvalidLength)?;
    bytes.get(offset..end).ok_or(Error::InvalidLength)
}

fn require_zero(bytes: &[u8], offset: usize, width: usize) -> Result<()> {
    if slice(bytes, offset, width)?.iter().any(|byte| *byte != 0) {
        return Err(Error::NonCanonicalReserved);
    }
    Ok(())
}

fn copy(output: &mut [u8], offset: usize, value: &[u8]) -> Result<()> {
    let end = offset
        .checked_add(value.len())
        .ok_or(Error::InvalidLength)?;
    output
        .get_mut(offset..end)
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(value);
    Ok(())
}

fn put_u16(output: &mut [u8], offset: usize, value: u16) -> Result<()> {
    copy(output, offset, &value.to_le_bytes())
}

fn put_u32(output: &mut [u8], offset: usize, value: u32) -> Result<()> {
    copy(output, offset, &value.to_le_bytes())
}

#[cfg(test)]
mod tests;
