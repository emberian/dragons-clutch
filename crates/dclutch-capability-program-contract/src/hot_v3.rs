//! Family-neutral Trading V3 hot execution ABI.
//!
//! The fixed prefix authenticates the Market, immutable capability root, and
//! every content-selection authority before any family runtime account. A
//! disposition-derived ExecutionStrategy suffix follows the prefix; the
//! remaining accounts are the exact AccountProfile/EffectProgram address
//! space. The adapter injects the already-present root, immutable config raw,
//! Product graph-root raw, Product-selected portfolio raw, and the exact
//! Product-linked basis raw accounts as logical coordinates zero through four
//! without duplicating physical metas. Config and Product records are
//! read-only projection evidence and no child route may borrow any injected
//! coordinate. No family discriminator or dummy account is part of the ABI.

/// Canonical hot instruction magic.
pub const HOT_EXECUTION_MAGIC_V3: [u8; 8] = *b"DCLTHOT3";
/// Canonical hot instruction schema version.
pub const HOT_EXECUTION_VERSION_V3: u16 = 3;
/// Canonical family-neutral hot instruction physical profile.
pub const HOT_EXECUTION_PROFILE_V3: u16 = 1;
/// Exact fixed hot envelope width before the family request.
pub const HOT_EXECUTION_ENVELOPE_BYTES_V3: usize = 128;
/// Absolute byte offset of the exact family request in the current instruction.
///
/// RequestProfile V2 native-evidence ranges are absolute, so generators add
/// this constant to family-relative signed-message coordinates.
pub const HOT_FAMILY_REQUEST_OFFSET_V3: usize = HOT_EXECUTION_ENVELOPE_BYTES_V3;

/// Canonical hot acknowledgment magic.
pub const HOT_EXECUTION_ACK_MAGIC_V3: [u8; 8] = *b"DCLTHAK3";
/// Exact hot acknowledgment width.
pub const HOT_EXECUTION_ACK_BYTES_V3: usize = 280;

/// Core Market account.
pub const HOT_MARKET_ACCOUNT_V3: usize = 0;
/// Mutable Trading capability root, committed after every effect/child route.
pub const HOT_ROOT_ACCOUNT_V3: usize = 1;
/// Finalized CapabilityManifest raw record.
pub const HOT_MANIFEST_RAW_ACCOUNT_V3: usize = 2;
/// Vacant CapabilityManifest staging cursor.
pub const HOT_MANIFEST_STAGING_ACCOUNT_V3: usize = 3;
/// Finalized CapabilityProgramSet raw record.
pub const HOT_PROGRAM_SET_RAW_ACCOUNT_V3: usize = 4;
/// Vacant CapabilityProgramSet staging cursor.
pub const HOT_PROGRAM_SET_STAGING_ACCOUNT_V3: usize = 5;
/// Action-selected finalized CapabilityProgramV3 raw record.
pub const HOT_DESCRIPTOR_RAW_ACCOUNT_V3: usize = 6;
/// Vacant CapabilityProgramV3 staging cursor.
pub const HOT_DESCRIPTOR_STAGING_ACCOUNT_V3: usize = 7;
/// Manifest-selected finalized immutable config raw record.
pub const HOT_CONFIG_RAW_ACCOUNT_V3: usize = 8;
/// Vacant immutable config staging cursor.
pub const HOT_CONFIG_STAGING_ACCOUNT_V3: usize = 9;
/// Finalized AccountProfile raw record.
pub const HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3: usize = 10;
/// Vacant AccountProfile staging cursor.
pub const HOT_ACCOUNT_PROFILE_STAGING_ACCOUNT_V3: usize = 11;
/// Finalized RequestProfile raw record.
pub const HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3: usize = 12;
/// Vacant RequestProfile staging cursor.
pub const HOT_REQUEST_PROFILE_STAGING_ACCOUNT_V3: usize = 13;
/// Finalized interpreted TransitionVM raw record.
pub const HOT_TRANSITION_RAW_ACCOUNT_V3: usize = 14;
/// Vacant TransitionVM staging cursor.
pub const HOT_TRANSITION_STAGING_ACCOUNT_V3: usize = 15;
/// Finalized EffectProgram raw record.
pub const HOT_EFFECT_RAW_ACCOUNT_V3: usize = 16;
/// Vacant EffectProgram staging cursor.
pub const HOT_EFFECT_STAGING_ACCOUNT_V3: usize = 17;
/// Finalized descriptor-selected StateLifecyclePolicy raw record.
pub const HOT_LIFECYCLE_RAW_ACCOUNT_V3: usize = 18;
/// Vacant StateLifecyclePolicy staging cursor.
pub const HOT_LIFECYCLE_STAGING_ACCOUNT_V3: usize = 19;
/// Finalized ExecutionStrategy raw record.
pub const HOT_STRATEGY_RAW_ACCOUNT_V3: usize = 20;
/// Vacant ExecutionStrategy staging cursor.
pub const HOT_STRATEGY_STAGING_ACCOUNT_V3: usize = 21;
/// Registry activation cache for the Market-selected release set.
pub const HOT_ACTIVATION_CACHE_ACCOUNT_V3: usize = 22;
/// Current Registry-selected Core Program.
pub const HOT_CORE_PROGRAM_ACCOUNT_V3: usize = 23;
/// Current Core ProgramData.
pub const HOT_CORE_PROGRAMDATA_ACCOUNT_V3: usize = 24;
/// Current Registry-selected Trading Program.
pub const HOT_TRADING_PROGRAM_ACCOUNT_V3: usize = 25;
/// Current Trading ProgramData.
pub const HOT_TRADING_PROGRAMDATA_ACCOUNT_V3: usize = 26;
/// Immutable Registry Program selected by the Market.
pub const HOT_REGISTRY_PROGRAM_ACCOUNT_V3: usize = 27;
/// Rent sysvar used for finalized-record and root observations.
pub const HOT_RENT_SYSVAR_ACCOUNT_V3: usize = 28;
/// Instructions sysvar used only when RequestProfile V2 requires native evidence.
pub const HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3: usize = 29;
/// Registry-finalized Product Runtime V2 graph-root raw record selected by Market.
pub const HOT_PRODUCT_RAW_ACCOUNT_V3: usize = 30;
/// Vacant Product Runtime V2 graph-root staging cursor.
pub const HOT_PRODUCT_STAGING_ACCOUNT_V3: usize = 31;
/// Registry-finalized Product-selected result-domain raw record.
pub const HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3: usize = 32;
/// Vacant Product-selected result-domain staging cursor.
pub const HOT_RESULT_DOMAIN_STAGING_ACCOUNT_V3: usize = 33;
/// Registry-finalized Product-selected portfolio raw record.
pub const HOT_PORTFOLIO_RAW_ACCOUNT_V3: usize = 34;
/// Vacant Product-selected portfolio staging cursor.
pub const HOT_PORTFOLIO_STAGING_ACCOUNT_V3: usize = 35;
/// Registry-finalized Product-linked basis raw record.
pub const HOT_LINKED_BASIS_RAW_ACCOUNT_V3: usize = 36;
/// Vacant Product-linked basis staging cursor.
pub const HOT_LINKED_BASIS_STAGING_ACCOUNT_V3: usize = 37;
/// Exact family-neutral account prefix width.
pub const HOT_FIXED_ACCOUNT_COUNT_V3: usize = 38;
/// First disposition-derived ExecutionStrategy account.
pub const HOT_STRATEGY_EXTRA_ACCOUNTS_START_V3: usize = HOT_FIXED_ACCOUNT_COUNT_V3;
/// Runtime AccountProfile coordinate occupied by the prefix root account.
pub const HOT_RUNTIME_ROOT_COORDINATE_V3: usize = 0;
/// Runtime AccountProfile coordinate occupied by authenticated immutable config.
pub const HOT_RUNTIME_CONFIG_COORDINATE_V3: usize = 1;
/// Runtime AccountProfile coordinate occupied by the Product graph-root body.
pub const HOT_RUNTIME_PRODUCT_COORDINATE_V3: usize = 2;
/// Runtime AccountProfile coordinate occupied by the Product portfolio body.
pub const HOT_RUNTIME_PORTFOLIO_COORDINATE_V3: usize = 3;
/// Runtime AccountProfile coordinate occupied by the Product-linked basis body.
pub const HOT_RUNTIME_LINKED_BASIS_COORDINATE_V3: usize = 4;
/// Number of fixed-prefix accounts injected into the logical runtime vector.
pub const HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3: usize = 5;
/// Common identity-register input containing SHA-256 of the exact family request.
///
/// This is the sole adapter-computed register fact. AccountProfile and
/// RequestProfile own every other register coordinate, while EffectProgram
/// may copy this digest into canonical child `parent_request_digest` fields.
pub const HOT_PARENT_REQUEST_DIGEST_IDENTITY_V3: usize = 0;

const ENVELOPE_REQUEST_BYTES_OFFSET: usize = 12;
const ENVELOPE_RELEASE_SET_OFFSET: usize = 16;
const ENVELOPE_MARKET_OFFSET: usize = 48;
const ENVELOPE_GENERATION_OFFSET: usize = 80;
const ENVELOPE_ROOT_PRESTATE_DIGEST_OFFSET: usize = 88;
const ENVELOPE_RESERVED_OFFSET: usize = 120;

const ACK_RELEASE_SET_OFFSET: usize = 16;
const ACK_MARKET_OFFSET: usize = 48;
const ACK_GENERATION_OFFSET: usize = 80;
const ACK_ROOT_OFFSET: usize = 88;
const ACK_REQUEST_DIGEST_OFFSET: usize = 120;
const ACK_SELECTED_PROGRAM_OFFSET: usize = 152;
const ACK_ROOT_PRESTATE_DIGEST_OFFSET: usize = 184;
const ACK_ROOT_POSTSTATE_DIGEST_OFFSET: usize = 216;
const ACK_EXECUTION_DIGEST_OFFSET: usize = 248;

/// Stable hot-envelope or acknowledgment refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HotExecutionErrorV3 {
    /// The exact fixed width or count-derived instruction width differed.
    InvalidLength,
    /// Magic selected another instruction or receipt.
    InvalidMagic,
    /// Schema version, physical profile, or reserved bytes were noncanonical.
    UnsupportedProfile,
    /// A required content/account/digest identity was zero.
    ZeroIdentity,
}

/// Result alias for the common hot ABI.
pub type HotExecutionResultV3<T> = core::result::Result<T, HotExecutionErrorV3>;

/// Exact immutable envelope preceding one family request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HotExecutionEnvelopeV3 {
    request_bytes: u32,
    release_set: [u8; 32],
    market: [u8; 32],
    generation: u64,
    root_prestate_digest: [u8; 32],
}

impl HotExecutionEnvelopeV3 {
    /// Construct an exact envelope for one nonempty family request.
    pub fn new(
        request_bytes: u32,
        release_set: [u8; 32],
        market: [u8; 32],
        generation: u64,
        root_prestate_digest: [u8; 32],
    ) -> HotExecutionResultV3<Self> {
        if request_bytes == 0 {
            return Err(HotExecutionErrorV3::InvalidLength);
        }
        for identity in [release_set, market, root_prestate_digest] {
            require_nonzero(identity)?;
        }
        Ok(Self {
            request_bytes,
            release_set,
            market,
            generation,
            root_prestate_digest,
        })
    }

    /// Hostile-decode the exact fixed envelope without accepting request bytes.
    pub fn decode(bytes: &[u8]) -> HotExecutionResultV3<Self> {
        if bytes.len() != HOT_EXECUTION_ENVELOPE_BYTES_V3 {
            return Err(HotExecutionErrorV3::InvalidLength);
        }
        if bytes.get(..8) != Some(HOT_EXECUTION_MAGIC_V3.as_slice()) {
            return Err(HotExecutionErrorV3::InvalidMagic);
        }
        if read_u16(bytes, 8)? != HOT_EXECUTION_VERSION_V3
            || read_u16(bytes, 10)? != HOT_EXECUTION_PROFILE_V3
            || !all_zero(slice(bytes, ENVELOPE_RESERVED_OFFSET, 8)?)
        {
            return Err(HotExecutionErrorV3::UnsupportedProfile);
        }
        Self::new(
            read_u32(bytes, ENVELOPE_REQUEST_BYTES_OFFSET)?,
            read_array(bytes, ENVELOPE_RELEASE_SET_OFFSET)?,
            read_array(bytes, ENVELOPE_MARKET_OFFSET)?,
            read_u64(bytes, ENVELOPE_GENERATION_OFFSET)?,
            read_array(bytes, ENVELOPE_ROOT_PRESTATE_DIGEST_OFFSET)?,
        )
    }

    /// Split a complete instruction into its exact envelope and family request.
    pub fn split_instruction(bytes: &[u8]) -> HotExecutionResultV3<(Self, &[u8])> {
        let envelope = Self::decode(
            bytes
                .get(..HOT_EXECUTION_ENVELOPE_BYTES_V3)
                .ok_or(HotExecutionErrorV3::InvalidLength)?,
        )?;
        let request_bytes = usize::try_from(envelope.request_bytes)
            .map_err(|_| HotExecutionErrorV3::InvalidLength)?;
        let expected = HOT_EXECUTION_ENVELOPE_BYTES_V3
            .checked_add(request_bytes)
            .ok_or(HotExecutionErrorV3::InvalidLength)?;
        if bytes.len() != expected {
            return Err(HotExecutionErrorV3::InvalidLength);
        }
        Ok((
            envelope,
            bytes
                .get(HOT_FAMILY_REQUEST_OFFSET_V3..)
                .ok_or(HotExecutionErrorV3::InvalidLength)?,
        ))
    }

    /// Exact nonzero family request width.
    pub const fn request_bytes(self) -> u32 {
        self.request_bytes
    }

    /// Current immutable execution release set.
    pub const fn release_set(self) -> [u8; 32] {
        self.release_set
    }

    /// Exact Core Market identity.
    pub const fn market(self) -> [u8; 32] {
        self.market
    }

    /// Exact Core Market generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Optimistic prestate commitment of the complete capability root.
    pub const fn root_prestate_digest(self) -> [u8; 32] {
        self.root_prestate_digest
    }

    /// Encode the exact canonical fixed envelope.
    pub fn to_bytes(self) -> [u8; HOT_EXECUTION_ENVELOPE_BYTES_V3] {
        let mut output = [0_u8; HOT_EXECUTION_ENVELOPE_BYTES_V3];
        put(&mut output, 0, &HOT_EXECUTION_MAGIC_V3);
        put(&mut output, 8, &HOT_EXECUTION_VERSION_V3.to_le_bytes());
        put(&mut output, 10, &HOT_EXECUTION_PROFILE_V3.to_le_bytes());
        put(
            &mut output,
            ENVELOPE_REQUEST_BYTES_OFFSET,
            &self.request_bytes.to_le_bytes(),
        );
        put(&mut output, ENVELOPE_RELEASE_SET_OFFSET, &self.release_set);
        put(&mut output, ENVELOPE_MARKET_OFFSET, &self.market);
        put(
            &mut output,
            ENVELOPE_GENERATION_OFFSET,
            &self.generation.to_le_bytes(),
        );
        put(
            &mut output,
            ENVELOPE_ROOT_PRESTATE_DIGEST_OFFSET,
            &self.root_prestate_digest,
        );
        output
    }
}

/// Exact commit-last evidence returned by the common hot outer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HotExecutionAckV3 {
    /// Current immutable execution release set.
    pub release_set: [u8; 32],
    /// Exact Core Market identity.
    pub market: [u8; 32],
    /// Exact Core Market generation.
    pub generation: u64,
    /// Exact mutable capability-root account identity.
    pub root: [u8; 32],
    /// SHA-256 of the complete exact family request.
    pub request_digest: [u8; 32],
    /// Action-selected exact CapabilityProgramV3 content identity.
    pub selected_program: [u8; 32],
    /// Complete root commitment before execution.
    pub root_prestate_digest: [u8; 32],
    /// Complete root commitment after every accepted effect/child route.
    pub root_poststate_digest: [u8; 32],
    /// Domain-separated commitment to selected artifacts and child receipts.
    pub execution_digest: [u8; 32],
}

impl HotExecutionAckV3 {
    /// Construct exact nonzero commit-last evidence.
    pub fn new(value: Self) -> HotExecutionResultV3<Self> {
        for identity in [
            value.release_set,
            value.market,
            value.root,
            value.request_digest,
            value.selected_program,
            value.root_prestate_digest,
            value.root_poststate_digest,
            value.execution_digest,
        ] {
            require_nonzero(identity)?;
        }
        Ok(value)
    }

    /// Hostile-decode one exact acknowledgment.
    pub fn decode(bytes: &[u8]) -> HotExecutionResultV3<Self> {
        if bytes.len() != HOT_EXECUTION_ACK_BYTES_V3 {
            return Err(HotExecutionErrorV3::InvalidLength);
        }
        if bytes.get(..8) != Some(HOT_EXECUTION_ACK_MAGIC_V3.as_slice()) {
            return Err(HotExecutionErrorV3::InvalidMagic);
        }
        if read_u16(bytes, 8)? != HOT_EXECUTION_VERSION_V3
            || read_u16(bytes, 10)? != HOT_EXECUTION_PROFILE_V3
            || !all_zero(slice(bytes, 12, 4)?)
        {
            return Err(HotExecutionErrorV3::UnsupportedProfile);
        }
        Self::new(Self {
            release_set: read_array(bytes, ACK_RELEASE_SET_OFFSET)?,
            market: read_array(bytes, ACK_MARKET_OFFSET)?,
            generation: read_u64(bytes, ACK_GENERATION_OFFSET)?,
            root: read_array(bytes, ACK_ROOT_OFFSET)?,
            request_digest: read_array(bytes, ACK_REQUEST_DIGEST_OFFSET)?,
            selected_program: read_array(bytes, ACK_SELECTED_PROGRAM_OFFSET)?,
            root_prestate_digest: read_array(bytes, ACK_ROOT_PRESTATE_DIGEST_OFFSET)?,
            root_poststate_digest: read_array(bytes, ACK_ROOT_POSTSTATE_DIGEST_OFFSET)?,
            execution_digest: read_array(bytes, ACK_EXECUTION_DIGEST_OFFSET)?,
        })
    }

    /// Encode exact canonical acknowledgment bytes.
    pub fn to_bytes(self) -> [u8; HOT_EXECUTION_ACK_BYTES_V3] {
        let mut output = [0_u8; HOT_EXECUTION_ACK_BYTES_V3];
        put(&mut output, 0, &HOT_EXECUTION_ACK_MAGIC_V3);
        put(&mut output, 8, &HOT_EXECUTION_VERSION_V3.to_le_bytes());
        put(&mut output, 10, &HOT_EXECUTION_PROFILE_V3.to_le_bytes());
        for (offset, value) in [
            (ACK_RELEASE_SET_OFFSET, self.release_set),
            (ACK_MARKET_OFFSET, self.market),
            (ACK_ROOT_OFFSET, self.root),
            (ACK_REQUEST_DIGEST_OFFSET, self.request_digest),
            (ACK_SELECTED_PROGRAM_OFFSET, self.selected_program),
            (ACK_ROOT_PRESTATE_DIGEST_OFFSET, self.root_prestate_digest),
            (ACK_ROOT_POSTSTATE_DIGEST_OFFSET, self.root_poststate_digest),
            (ACK_EXECUTION_DIGEST_OFFSET, self.execution_digest),
        ] {
            put(&mut output, offset, &value);
        }
        put(
            &mut output,
            ACK_GENERATION_OFFSET,
            &self.generation.to_le_bytes(),
        );
        output
    }
}

fn require_nonzero(value: [u8; 32]) -> HotExecutionResultV3<()> {
    if value == [0; 32] {
        Err(HotExecutionErrorV3::ZeroIdentity)
    } else {
        Ok(())
    }
}

fn slice(bytes: &[u8], offset: usize, width: usize) -> HotExecutionResultV3<&[u8]> {
    bytes
        .get(
            offset
                ..offset
                    .checked_add(width)
                    .ok_or(HotExecutionErrorV3::InvalidLength)?,
        )
        .ok_or(HotExecutionErrorV3::InvalidLength)
}

fn read_u16(bytes: &[u8], offset: usize) -> HotExecutionResultV3<u16> {
    Ok(u16::from_le_bytes(
        slice(bytes, offset, 2)?
            .try_into()
            .map_err(|_| HotExecutionErrorV3::InvalidLength)?,
    ))
}

fn read_u32(bytes: &[u8], offset: usize) -> HotExecutionResultV3<u32> {
    Ok(u32::from_le_bytes(
        slice(bytes, offset, 4)?
            .try_into()
            .map_err(|_| HotExecutionErrorV3::InvalidLength)?,
    ))
}

fn read_u64(bytes: &[u8], offset: usize) -> HotExecutionResultV3<u64> {
    Ok(u64::from_le_bytes(
        slice(bytes, offset, 8)?
            .try_into()
            .map_err(|_| HotExecutionErrorV3::InvalidLength)?,
    ))
}

fn read_array(bytes: &[u8], offset: usize) -> HotExecutionResultV3<[u8; 32]> {
    slice(bytes, offset, 32)?
        .try_into()
        .map_err(|_| HotExecutionErrorV3::InvalidLength)
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) {
    for (destination, source) in output.iter_mut().skip(offset).zip(value) {
        *destination = *source;
    }
}

fn all_zero(bytes: &[u8]) -> bool {
    bytes.iter().all(|value| *value == 0)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::vec::Vec;

    use super::*;

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    #[test]
    fn envelope_splits_one_exact_family_request() {
        let envelope = HotExecutionEnvelopeV3::new(3, id(1), id(2), 7, id(3)).expect("envelope");
        let mut instruction = Vec::from(envelope.to_bytes());
        instruction.extend_from_slice(b"hot");
        assert_eq!(
            HotExecutionEnvelopeV3::split_instruction(&instruction),
            Ok((envelope, b"hot".as_slice()))
        );
        instruction.push(0);
        assert_eq!(
            HotExecutionEnvelopeV3::split_instruction(&instruction),
            Err(HotExecutionErrorV3::InvalidLength)
        );
    }

    #[test]
    fn reserved_zero_identities_and_width_substitution_refuse() {
        let envelope = HotExecutionEnvelopeV3::new(1, id(1), id(2), 0, id(3)).expect("envelope");
        let bytes = envelope.to_bytes();
        for offset in [0, 8, 10, ENVELOPE_RESERVED_OFFSET] {
            let mut hostile = bytes;
            *hostile.get_mut(offset).expect("hostile offset") ^= 1;
            assert!(HotExecutionEnvelopeV3::decode(&hostile).is_err());
        }
        assert_eq!(
            HotExecutionEnvelopeV3::new(1, [0; 32], id(2), 0, id(3)),
            Err(HotExecutionErrorV3::ZeroIdentity)
        );
        assert_eq!(
            HotExecutionEnvelopeV3::decode(
                bytes
                    .get(..bytes.len() - 1)
                    .expect("one-byte-short envelope"),
            ),
            Err(HotExecutionErrorV3::InvalidLength)
        );
    }

    #[test]
    fn commit_last_ack_binds_program_request_root_and_execution() {
        let ack = HotExecutionAckV3::new(HotExecutionAckV3 {
            release_set: id(1),
            market: id(2),
            generation: 3,
            root: id(4),
            request_digest: id(5),
            selected_program: id(6),
            root_prestate_digest: id(7),
            root_poststate_digest: id(8),
            execution_digest: id(9),
        })
        .expect("ack");
        let bytes = ack.to_bytes();
        assert_eq!(HotExecutionAckV3::decode(&bytes), Ok(ack));
        let mut bad_magic = bytes;
        bad_magic[0] = 0;
        assert!(HotExecutionAckV3::decode(&bad_magic).is_err());
        let mut bad_reserved = bytes;
        bad_reserved[12] = 1;
        assert!(HotExecutionAckV3::decode(&bad_reserved).is_err());
        for offset in [ACK_SELECTED_PROGRAM_OFFSET, ACK_EXECUTION_DIGEST_OFFSET] {
            let mut zero_identity = bytes;
            zero_identity
                .get_mut(offset..offset + 32)
                .expect("identity field")
                .fill(0);
            assert!(HotExecutionAckV3::decode(&zero_identity).is_err());
        }
    }

    #[test]
    fn frame_prefix_is_contiguous_and_runtime_root_is_unique() {
        assert_eq!(HOT_MARKET_ACCOUNT_V3, 0);
        assert_eq!(HOT_ROOT_ACCOUNT_V3, 1);
        assert_eq!(
            HOT_EFFECT_STAGING_ACCOUNT_V3 + 1,
            HOT_LIFECYCLE_RAW_ACCOUNT_V3
        );
        assert_eq!(
            HOT_LIFECYCLE_STAGING_ACCOUNT_V3 + 1,
            HOT_STRATEGY_RAW_ACCOUNT_V3
        );
        assert_eq!(
            HOT_STRATEGY_STAGING_ACCOUNT_V3 + 1,
            HOT_ACTIVATION_CACHE_ACCOUNT_V3
        );
        assert_eq!(
            HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3 + 1,
            HOT_PRODUCT_RAW_ACCOUNT_V3
        );
        assert_eq!(
            HOT_PRODUCT_RAW_ACCOUNT_V3 + 1,
            HOT_PRODUCT_STAGING_ACCOUNT_V3
        );
        assert_eq!(
            HOT_PRODUCT_STAGING_ACCOUNT_V3 + 1,
            HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3
        );
        assert_eq!(
            HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3 + 1,
            HOT_RESULT_DOMAIN_STAGING_ACCOUNT_V3
        );
        assert_eq!(
            HOT_RESULT_DOMAIN_STAGING_ACCOUNT_V3 + 1,
            HOT_PORTFOLIO_RAW_ACCOUNT_V3
        );
        assert_eq!(
            HOT_PORTFOLIO_RAW_ACCOUNT_V3 + 1,
            HOT_PORTFOLIO_STAGING_ACCOUNT_V3
        );
        assert_eq!(
            HOT_PORTFOLIO_STAGING_ACCOUNT_V3 + 1,
            HOT_LINKED_BASIS_RAW_ACCOUNT_V3
        );
        assert_eq!(
            HOT_LINKED_BASIS_RAW_ACCOUNT_V3 + 1,
            HOT_LINKED_BASIS_STAGING_ACCOUNT_V3
        );
        assert_eq!(
            HOT_LINKED_BASIS_STAGING_ACCOUNT_V3 + 1,
            HOT_FIXED_ACCOUNT_COUNT_V3
        );
        assert_eq!(
            HOT_STRATEGY_EXTRA_ACCOUNTS_START_V3,
            HOT_FIXED_ACCOUNT_COUNT_V3
        );
        assert_eq!(HOT_RUNTIME_ROOT_COORDINATE_V3, 0);
        assert_eq!(HOT_RUNTIME_CONFIG_COORDINATE_V3, 1);
        assert_eq!(HOT_RUNTIME_PRODUCT_COORDINATE_V3, 2);
        assert_eq!(HOT_RUNTIME_PORTFOLIO_COORDINATE_V3, 3);
        assert_eq!(HOT_RUNTIME_LINKED_BASIS_COORDINATE_V3, 4);
        assert_eq!(HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3, 5);
        assert_eq!(
            HOT_FAMILY_REQUEST_OFFSET_V3,
            HOT_EXECUTION_ENVELOPE_BYTES_V3
        );
    }
}
