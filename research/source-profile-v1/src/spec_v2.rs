//! PROPOSED `SourceSpecV2` pull-profile canonical body codec (MODEL-ONLY).
//!
//! This module is research code for the spec revision described in
//! `docs/design/SOURCE_PROVIDER_V1_SELECTION.md` §3.  It is not a runtime
//! transition, not a registry entry, and freezes no identity; the default ELF
//! keeps refusing `SourceReleaseUnavailable` (`0x79`).
//!
//! `SourceSpecV1` binds one exact immutable source data-account key.  Pyth
//! pull-update accounts are caller-created, ephemeral, and closable, so no
//! such key exists.  The V2 body is a **new spec generation under a new
//! domain** (`dragons-clutch/feed/v2`), not an update — old feeds fail closed
//! by construction.  Field-level deltas from V1:
//!
//! 1. the exact source data-account key is **replaced** by the receiver
//!    `Config` PDA key plus a canonical config byte digest pinned at spec
//!    creation (governance can mutate `Config` without a program upgrade, so
//!    a governance change is a new feed generation);
//! 2. the provider feed id (Pyth 32-byte `feed_id`) is **added** and must be
//!    checked equal on every admitted update;
//! 3. the deployment-generation pin (receiver program key plus generation) is
//!    **kept** and rechecked on every use, now alongside the config digest;
//! 4. the canonical selection rule id registers the `CROSSING_V1` rules from
//!    [`crate::crossing_v1`] — the V1 finalized-bucket rule id is **not**
//!    admissible under this domain, because admission check 12's
//!    `publish_time / bucket_seconds == cursor` equation does not apply to a
//!    crossing-rule feed.
//!
//! The v2 feed identity is `digest(FEED_DOMAIN_V2, canonical body)`.  This
//! crate deliberately ships no hash implementation: the domain string is
//! registered here and the digest computation remains an adapter obligation,
//! exactly as `canonical_feed_id` is for V1.

use crate::crossing_v1::{SELECTION_CROSSING_V1_CLOSING, SELECTION_CROSSING_V1_OPENING};

/// Digest domain separating v2 pull-profile feed identities from every V1
/// feed.  A body hashed under any other domain is a different identity space.
pub const FEED_DOMAIN_V2: &[u8; 22] = b"dragons-clutch/feed/v2";

/// Exact canonical byte length of a [`SourceSpecV2`] body.
pub const SOURCE_SPEC_V2_BYTES: usize = 320;

/// Leading magic of the canonical v2 body.  V1 bodies start `DCSRCV1\0` and
/// are refused here byte-for-byte.
pub const SOURCE_SPEC_V2_MAGIC: [u8; 8] = *b"DCSRCV2\0";

/// Canonical source-spec schema version encoded after the magic.
pub const SOURCE_SPEC_V2_SCHEMA: u16 = 2;

/// Quote-units-per-one-base-unit orientation (the only registered value).
pub const ORIENTATION_QUOTE_PER_BASE: u8 = 1;

/// Model copy of the accumulator grid envelope (`clutch_accumulator`
/// `MAX_BUCKET_SECONDS`).  Pinned here because this research crate is
/// dependency-free; drift against the runtime constant must be re-audited
/// before any promotion.
pub const MODEL_MAX_BUCKET_SECONDS: u64 = 86_400;

/// Model copy of the accumulator value envelope (`clutch_accumulator`
/// `MAX_VALUE`), bounding the absolute confidence cap exactly as V1 does.
pub const MODEL_MAX_CONFIDENCE_ATOMS: u128 = 1_000_000_000_000_000_000_000_000;

/// Offset of the six reserved zero bytes closing the canonical body.
const RESERVED_AT: usize = 314;

/// Refusals from hostile v2 body bytes or invalid construction fields.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpecV2Error {
    /// The byte string is not exactly [`SOURCE_SPEC_V2_BYTES`] long
    /// (truncation and trailing bytes both land here).
    WrongLength,
    /// The body does not begin with [`SOURCE_SPEC_V2_MAGIC`]; a V1 body is
    /// this refusal, not a downgrade path.
    WrongMagic,
    /// The encoded schema version is not [`SOURCE_SPEC_V2_SCHEMA`].
    WrongSchema,
    /// An identity that participates in the feed digest was zero.
    ZeroIdentity,
    /// Base and quote asset identities were equal.
    SameAsset,
    /// A version, generation, or registry identifier was zero.
    Unversioned,
    /// The requested asset orientation is not registered.
    UnknownOrientation,
    /// The selection rule is not a registered `CROSSING_V1` rule id.  The V1
    /// finalized-bucket rule id (1) is deliberately not admissible here.
    UnknownSelectionRule,
    /// The normalized decimal scale is outside the integer envelope.
    InvalidScale,
    /// The observation grid is structurally invalid.
    InvalidGrid,
    /// A freshness rule has no finite positive bound.
    InvalidFreshnessPolicy,
    /// The confidence policy is empty, unbounded, or outside its envelope.
    InvalidConfidencePolicy,
    /// The reserved tail bytes were not all zero.
    NonCanonicalPadding,
}

/// Public construction fields for a proposed immutable V2 pull-profile spec.
///
/// [`SourceSpecV2::new`] validates these fields.  The field struct is not
/// itself evidence of a valid spec.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceSpecFieldsV2 {
    /// Digest identifying the reviewed adapter implementation.
    pub source_adapter_id: [u8; 32],
    /// Version of that adapter; immutable terms bind this as `source_version`.
    pub source_adapter_version: u32,
    /// Closed parser-registry identifier.
    pub parser_id: u16,
    /// Version of the selected parser.
    pub parser_version: u16,
    /// Receiver program key that must own every admitted update account.
    pub receiver_program: [u8; 32],
    /// Receiver `Config` PDA key (stable), replacing the V1 exact source
    /// data-account key.
    pub receiver_config: [u8; 32],
    /// Canonical receiver `Config` byte digest pinned at spec creation and
    /// rechecked byte-exact on every append.  A governance config change is
    /// a new feed generation.
    pub config_digest: [u8; 32],
    /// Provider feed id (Pyth 32-byte `feed_id`), checked equal on every
    /// admitted update.
    pub provider_feed_id: [u8; 32],
    /// Pinned receiver deployment generation (program + ProgramData
    /// identity), rechecked on every use.
    pub deployment_generation: u64,
    /// Canonical base-asset identity.
    pub base_asset_id: [u8; 32],
    /// Canonical quote-asset identity.
    pub quote_asset_id: [u8; 32],
    /// Registered orientation; V2 accepts [`ORIENTATION_QUOTE_PER_BASE`].
    pub orientation: u8,
    /// Decimal places in the normalized positive integer price.
    pub normalized_decimals: u8,
    /// Observation grid family.
    pub grid_family_id: u32,
    /// Observation grid version.
    pub grid_version: u16,
    /// Exact duration of one observation bucket in seconds.
    pub bucket_seconds: u64,
    /// Maximum age of an admitted update in cluster slots.
    pub max_staleness_slots: u64,
    /// Maximum age of an admitted update in seconds.
    pub max_staleness_seconds: u64,
    /// Allowed positive source-time skew in seconds.
    pub max_future_seconds: u64,
    /// Maximum admitted half-width after confidence widening.
    pub max_confidence_atoms: u128,
    /// Maximum admitted half-width relative to price, in basis points.
    pub max_confidence_bps: u32,
    /// Integer multiplier applied to the parser's confidence value.
    pub confidence_multiplier: u16,
    /// Registered `CROSSING_V1` selection rule id (boundary variant is
    /// deliberately not frozen; both ids are registered).
    pub selection_rule: u16,
}

/// Validated proposed V2 pull-profile source specification (MODEL-ONLY).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceSpecV2 {
    fields: SourceSpecFieldsV2,
}

impl SourceSpecV2 {
    /// Validate and construct a v2 specification.
    pub fn new(fields: SourceSpecFieldsV2) -> Result<Self, SpecV2Error> {
        if is_zero(&fields.source_adapter_id)
            || is_zero(&fields.receiver_program)
            || is_zero(&fields.receiver_config)
            || is_zero(&fields.config_digest)
            || is_zero(&fields.provider_feed_id)
            || is_zero(&fields.base_asset_id)
            || is_zero(&fields.quote_asset_id)
        {
            return Err(SpecV2Error::ZeroIdentity);
        }
        if fields.base_asset_id == fields.quote_asset_id {
            return Err(SpecV2Error::SameAsset);
        }
        if fields.source_adapter_version == 0
            || fields.parser_id == 0
            || fields.parser_version == 0
            || fields.deployment_generation == 0
        {
            return Err(SpecV2Error::Unversioned);
        }
        if fields.orientation != ORIENTATION_QUOTE_PER_BASE {
            return Err(SpecV2Error::UnknownOrientation);
        }
        if fields.selection_rule != SELECTION_CROSSING_V1_CLOSING
            && fields.selection_rule != SELECTION_CROSSING_V1_OPENING
        {
            return Err(SpecV2Error::UnknownSelectionRule);
        }
        if fields.normalized_decimals > 18 {
            return Err(SpecV2Error::InvalidScale);
        }
        if fields.bucket_seconds == 0 || fields.bucket_seconds > MODEL_MAX_BUCKET_SECONDS {
            return Err(SpecV2Error::InvalidGrid);
        }
        if fields.max_staleness_slots == 0 || fields.max_staleness_seconds == 0 {
            return Err(SpecV2Error::InvalidFreshnessPolicy);
        }
        if fields.max_confidence_atoms == 0
            || fields.max_confidence_atoms > MODEL_MAX_CONFIDENCE_ATOMS
            || fields.max_confidence_bps == 0
            || fields.max_confidence_bps > 10_000
            || fields.confidence_multiplier == 0
            || fields.confidence_multiplier > 32
        {
            return Err(SpecV2Error::InvalidConfidencePolicy);
        }
        Ok(Self { fields })
    }

    /// Validated construction fields.
    pub const fn fields(self) -> SourceSpecFieldsV2 {
        self.fields
    }

    /// Encode the exact 320-byte v2 feed-spec preimage.
    ///
    /// | Offset | Bytes | Field |
    /// | ---: | ---: | --- |
    /// | 0 | 8 | magic `DCSRCV2\0` |
    /// | 8 | 2 | source-spec schema = 2 |
    /// | 10 | 32 | audited `source_adapter_id` |
    /// | 42 | 4 | source adapter version |
    /// | 46 | 2 | closed parser id |
    /// | 48 | 2 | parser version |
    /// | 50 | 32 | receiver program key |
    /// | 82 | 32 | receiver `Config` PDA key |
    /// | 114 | 32 | canonical config byte digest |
    /// | 146 | 32 | provider feed id |
    /// | 178 | 8 | deployment generation |
    /// | 186 | 32 | base-asset identity |
    /// | 218 | 32 | quote-asset identity |
    /// | 250 | 1 | orientation = quote per base |
    /// | 251 | 1 | normalized decimals |
    /// | 252 | 4 | grid family id |
    /// | 256 | 2 | grid version |
    /// | 258 | 8 | bucket seconds |
    /// | 266 | 8 | maximum staleness slots |
    /// | 274 | 8 | maximum staleness seconds |
    /// | 282 | 8 | maximum future-time skew seconds |
    /// | 290 | 16 | maximum widened confidence atoms |
    /// | 306 | 4 | maximum widened confidence basis points |
    /// | 310 | 2 | integer confidence multiplier |
    /// | 312 | 2 | canonical selection rule id |
    /// | 314 | 6 | zero reserved |
    pub fn encode_canonical(self) -> [u8; SOURCE_SPEC_V2_BYTES] {
        let mut out = [0_u8; SOURCE_SPEC_V2_BYTES];
        let mut at = 0_usize;
        put(&mut out, &mut at, &SOURCE_SPEC_V2_MAGIC);
        put(&mut out, &mut at, &SOURCE_SPEC_V2_SCHEMA.to_le_bytes());
        put(&mut out, &mut at, &self.fields.source_adapter_id);
        put(
            &mut out,
            &mut at,
            &self.fields.source_adapter_version.to_le_bytes(),
        );
        put(&mut out, &mut at, &self.fields.parser_id.to_le_bytes());
        put(&mut out, &mut at, &self.fields.parser_version.to_le_bytes());
        put(&mut out, &mut at, &self.fields.receiver_program);
        put(&mut out, &mut at, &self.fields.receiver_config);
        put(&mut out, &mut at, &self.fields.config_digest);
        put(&mut out, &mut at, &self.fields.provider_feed_id);
        put(
            &mut out,
            &mut at,
            &self.fields.deployment_generation.to_le_bytes(),
        );
        put(&mut out, &mut at, &self.fields.base_asset_id);
        put(&mut out, &mut at, &self.fields.quote_asset_id);
        put(&mut out, &mut at, &[self.fields.orientation]);
        put(&mut out, &mut at, &[self.fields.normalized_decimals]);
        put(&mut out, &mut at, &self.fields.grid_family_id.to_le_bytes());
        put(&mut out, &mut at, &self.fields.grid_version.to_le_bytes());
        put(&mut out, &mut at, &self.fields.bucket_seconds.to_le_bytes());
        put(
            &mut out,
            &mut at,
            &self.fields.max_staleness_slots.to_le_bytes(),
        );
        put(
            &mut out,
            &mut at,
            &self.fields.max_staleness_seconds.to_le_bytes(),
        );
        put(
            &mut out,
            &mut at,
            &self.fields.max_future_seconds.to_le_bytes(),
        );
        put(
            &mut out,
            &mut at,
            &self.fields.max_confidence_atoms.to_le_bytes(),
        );
        put(
            &mut out,
            &mut at,
            &self.fields.max_confidence_bps.to_le_bytes(),
        );
        put(
            &mut out,
            &mut at,
            &self.fields.confidence_multiplier.to_le_bytes(),
        );
        put(&mut out, &mut at, &self.fields.selection_rule.to_le_bytes());
        debug_assert_eq!(at, RESERVED_AT);
        out
    }

    /// Decode and validate one exact canonical body.
    ///
    /// Structural refusals come first (length, magic, schema, reserved
    /// padding), then every [`SourceSpecV2::new`] field refusal.  Any
    /// accepted byte string re-encodes byte-identically, so the canonical
    /// form is unique.
    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, SpecV2Error> {
        if bytes.len() != SOURCE_SPEC_V2_BYTES {
            return Err(SpecV2Error::WrongLength);
        }
        if bytes[..8] != SOURCE_SPEC_V2_MAGIC {
            return Err(SpecV2Error::WrongMagic);
        }
        if u16_at(bytes, 8) != SOURCE_SPEC_V2_SCHEMA {
            return Err(SpecV2Error::WrongSchema);
        }
        if bytes[RESERVED_AT..].iter().any(|byte| *byte != 0) {
            return Err(SpecV2Error::NonCanonicalPadding);
        }
        Self::new(SourceSpecFieldsV2 {
            source_adapter_id: key_at(bytes, 10),
            source_adapter_version: u32_at(bytes, 42),
            parser_id: u16_at(bytes, 46),
            parser_version: u16_at(bytes, 48),
            receiver_program: key_at(bytes, 50),
            receiver_config: key_at(bytes, 82),
            config_digest: key_at(bytes, 114),
            provider_feed_id: key_at(bytes, 146),
            deployment_generation: u64_at(bytes, 178),
            base_asset_id: key_at(bytes, 186),
            quote_asset_id: key_at(bytes, 218),
            orientation: bytes[250],
            normalized_decimals: bytes[251],
            grid_family_id: u32_at(bytes, 252),
            grid_version: u16_at(bytes, 256),
            bucket_seconds: u64_at(bytes, 258),
            max_staleness_slots: u64_at(bytes, 266),
            max_staleness_seconds: u64_at(bytes, 274),
            max_future_seconds: u64_at(bytes, 282),
            max_confidence_atoms: u128_at(bytes, 290),
            max_confidence_bps: u32_at(bytes, 306),
            confidence_multiplier: u16_at(bytes, 310),
            selection_rule: u16_at(bytes, 312),
        })
    }
}

fn is_zero(bytes: &[u8; 32]) -> bool {
    bytes.iter().all(|byte| *byte == 0)
}

fn put<const N: usize>(out: &mut [u8; N], at: &mut usize, bytes: &[u8]) {
    let end = *at + bytes.len();
    out[*at..end].copy_from_slice(bytes);
    *at = end;
}

fn key_at(bytes: &[u8], at: usize) -> [u8; 32] {
    let mut value = [0_u8; 32];
    value.copy_from_slice(&bytes[at..at + 32]);
    value
}

fn u16_at(bytes: &[u8], at: usize) -> u16 {
    let mut value = [0_u8; 2];
    value.copy_from_slice(&bytes[at..at + 2]);
    u16::from_le_bytes(value)
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    let mut value = [0_u8; 4];
    value.copy_from_slice(&bytes[at..at + 4]);
    u32::from_le_bytes(value)
}

fn u64_at(bytes: &[u8], at: usize) -> u64 {
    let mut value = [0_u8; 8];
    value.copy_from_slice(&bytes[at..at + 8]);
    u64::from_le_bytes(value)
}

fn u128_at(bytes: &[u8], at: usize) -> u128 {
    let mut value = [0_u8; 16];
    value.copy_from_slice(&bytes[at..at + 16]);
    u128::from_le_bytes(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields() -> SourceSpecFieldsV2 {
        SourceSpecFieldsV2 {
            source_adapter_id: [0xa1; 32],
            source_adapter_version: 3,
            parser_id: 6,
            parser_version: 5,
            receiver_program: [0xb2; 32],
            receiver_config: [0xc3; 32],
            config_digest: [0xd4; 32],
            provider_feed_id: [0xe5; 32],
            deployment_generation: 7,
            base_asset_id: [0x11; 32],
            quote_asset_id: [0x22; 32],
            orientation: ORIENTATION_QUOTE_PER_BASE,
            normalized_decimals: 8,
            grid_family_id: 4,
            grid_version: 9,
            bucket_seconds: 300,
            max_staleness_slots: 400,
            max_staleness_seconds: 240,
            max_future_seconds: 15,
            max_confidence_atoms: 1_000_000_000_000,
            max_confidence_bps: 500,
            confidence_multiplier: 3,
            selection_rule: SELECTION_CROSSING_V1_CLOSING,
        }
    }

    fn spec() -> SourceSpecV2 {
        SourceSpecV2::new(fields()).expect("valid v2 source spec")
    }

    #[test]
    fn exact_layout_offsets_are_pinned() {
        let bytes = spec().encode_canonical();
        assert_eq!(bytes[..8], *b"DCSRCV2\0");
        assert_eq!(bytes[8..10], 2_u16.to_le_bytes());
        assert_eq!(bytes[10..42], [0xa1; 32]);
        assert_eq!(bytes[42..46], 3_u32.to_le_bytes());
        assert_eq!(bytes[46..48], 6_u16.to_le_bytes());
        assert_eq!(bytes[48..50], 5_u16.to_le_bytes());
        assert_eq!(bytes[50..82], [0xb2; 32]);
        assert_eq!(bytes[82..114], [0xc3; 32]);
        assert_eq!(bytes[114..146], [0xd4; 32]);
        assert_eq!(bytes[146..178], [0xe5; 32]);
        assert_eq!(bytes[178..186], 7_u64.to_le_bytes());
        assert_eq!(bytes[186..218], [0x11; 32]);
        assert_eq!(bytes[218..250], [0x22; 32]);
        assert_eq!(bytes[250], ORIENTATION_QUOTE_PER_BASE);
        assert_eq!(bytes[251], 8);
        assert_eq!(bytes[252..256], 4_u32.to_le_bytes());
        assert_eq!(bytes[256..258], 9_u16.to_le_bytes());
        assert_eq!(bytes[258..266], 300_u64.to_le_bytes());
        assert_eq!(bytes[266..274], 400_u64.to_le_bytes());
        assert_eq!(bytes[274..282], 240_u64.to_le_bytes());
        assert_eq!(bytes[282..290], 15_u64.to_le_bytes());
        assert_eq!(bytes[290..306], 1_000_000_000_000_u128.to_le_bytes());
        assert_eq!(bytes[306..310], 500_u32.to_le_bytes());
        assert_eq!(bytes[310..312], 3_u16.to_le_bytes());
        assert_eq!(
            bytes[312..314],
            SELECTION_CROSSING_V1_CLOSING.to_le_bytes()
        );
        assert_eq!(bytes[RESERVED_AT..], [0; 6]);
    }

    #[test]
    fn canonical_round_trip_is_exact_both_ways() {
        let original = spec();
        let bytes = original.encode_canonical();
        let decoded = SourceSpecV2::decode_canonical(&bytes).expect("canonical bytes decode");
        assert_eq!(decoded, original);
        assert_eq!(decoded.fields(), fields());
        assert_eq!(decoded.encode_canonical(), bytes);

        let mut opening = fields();
        opening.selection_rule = SELECTION_CROSSING_V1_OPENING;
        let opening = SourceSpecV2::new(opening).expect("opening variant is registered");
        let opening_bytes = opening.encode_canonical();
        assert_ne!(opening_bytes, bytes);
        assert_eq!(
            SourceSpecV2::decode_canonical(&opening_bytes),
            Ok(opening)
        );
    }

    #[test]
    fn truncated_and_extended_bytes_are_refused() {
        let bytes = spec().encode_canonical();
        assert_eq!(
            SourceSpecV2::decode_canonical(&[]),
            Err(SpecV2Error::WrongLength)
        );
        for cut in [1_usize, 8, 134, 256, SOURCE_SPEC_V2_BYTES - 1] {
            assert_eq!(
                SourceSpecV2::decode_canonical(&bytes[..cut]),
                Err(SpecV2Error::WrongLength)
            );
        }
        let mut trailing = [0_u8; SOURCE_SPEC_V2_BYTES + 1];
        trailing[..SOURCE_SPEC_V2_BYTES].copy_from_slice(&bytes);
        assert_eq!(
            SourceSpecV2::decode_canonical(&trailing),
            Err(SpecV2Error::WrongLength)
        );
    }

    #[test]
    fn wrong_domain_magic_and_schema_are_refused() {
        let bytes = spec().encode_canonical();
        for at in 0..8 {
            let mut hostile = bytes;
            hostile[at] ^= 1;
            assert_eq!(
                SourceSpecV2::decode_canonical(&hostile),
                Err(SpecV2Error::WrongMagic)
            );
        }

        // A V1 body prefix is a wrong domain, never a downgrade path.
        let mut v1 = bytes;
        v1[..8].copy_from_slice(b"DCSRCV1\0");
        assert_eq!(
            SourceSpecV2::decode_canonical(&v1),
            Err(SpecV2Error::WrongMagic)
        );

        for schema in [0_u16, 1, 3, u16::MAX] {
            let mut hostile = bytes;
            hostile[8..10].copy_from_slice(&schema.to_le_bytes());
            assert_eq!(
                SourceSpecV2::decode_canonical(&hostile),
                Err(SpecV2Error::WrongSchema)
            );
        }
    }

    #[test]
    fn non_canonical_padding_is_refused() {
        let bytes = spec().encode_canonical();
        for at in RESERVED_AT..SOURCE_SPEC_V2_BYTES {
            let mut hostile = bytes;
            hostile[at] = 1;
            assert_eq!(
                SourceSpecV2::decode_canonical(&hostile),
                Err(SpecV2Error::NonCanonicalPadding)
            );
        }
    }

    #[test]
    fn zero_identities_are_refused_in_fields_and_bytes() {
        let zero = [0_u8; 32];
        let mut case = fields();
        case.source_adapter_id = zero;
        assert_eq!(SourceSpecV2::new(case), Err(SpecV2Error::ZeroIdentity));
        case = fields();
        case.receiver_program = zero;
        assert_eq!(SourceSpecV2::new(case), Err(SpecV2Error::ZeroIdentity));
        case = fields();
        case.receiver_config = zero;
        assert_eq!(SourceSpecV2::new(case), Err(SpecV2Error::ZeroIdentity));
        case = fields();
        case.config_digest = zero;
        assert_eq!(SourceSpecV2::new(case), Err(SpecV2Error::ZeroIdentity));
        case = fields();
        case.provider_feed_id = zero;
        assert_eq!(SourceSpecV2::new(case), Err(SpecV2Error::ZeroIdentity));
        case = fields();
        case.base_asset_id = zero;
        assert_eq!(SourceSpecV2::new(case), Err(SpecV2Error::ZeroIdentity));
        case = fields();
        case.quote_asset_id = zero;
        assert_eq!(SourceSpecV2::new(case), Err(SpecV2Error::ZeroIdentity));

        // The same refusals hold on the byte path for every key field.
        let bytes = spec().encode_canonical();
        for key_start in [10_usize, 50, 82, 114, 146, 186, 218] {
            let mut hostile = bytes;
            hostile[key_start..key_start + 32].copy_from_slice(&zero);
            assert_eq!(
                SourceSpecV2::decode_canonical(&hostile),
                Err(SpecV2Error::ZeroIdentity)
            );
        }

        let mut same_asset = bytes;
        let base: [u8; 32] = key_at(&bytes, 186);
        same_asset[218..250].copy_from_slice(&base);
        assert_eq!(
            SourceSpecV2::decode_canonical(&same_asset),
            Err(SpecV2Error::SameAsset)
        );
    }

    #[test]
    fn unversioned_and_unregistered_values_are_refused() {
        let mut case = fields();
        case.source_adapter_version = 0;
        assert_eq!(SourceSpecV2::new(case), Err(SpecV2Error::Unversioned));
        case = fields();
        case.parser_id = 0;
        assert_eq!(SourceSpecV2::new(case), Err(SpecV2Error::Unversioned));
        case = fields();
        case.parser_version = 0;
        assert_eq!(SourceSpecV2::new(case), Err(SpecV2Error::Unversioned));
        case = fields();
        case.deployment_generation = 0;
        assert_eq!(SourceSpecV2::new(case), Err(SpecV2Error::Unversioned));

        case = fields();
        case.orientation = 0;
        assert_eq!(
            SourceSpecV2::new(case),
            Err(SpecV2Error::UnknownOrientation)
        );
        case.orientation = 2;
        assert_eq!(
            SourceSpecV2::new(case),
            Err(SpecV2Error::UnknownOrientation)
        );

        // The V1 finalized-bucket rule id (1) is not registered under v2.
        for rule in [0_u16, 1, 4, u16::MAX] {
            case = fields();
            case.selection_rule = rule;
            assert_eq!(
                SourceSpecV2::new(case),
                Err(SpecV2Error::UnknownSelectionRule)
            );
        }
        let mut hostile = spec().encode_canonical();
        hostile[312..314].copy_from_slice(&1_u16.to_le_bytes());
        assert_eq!(
            SourceSpecV2::decode_canonical(&hostile),
            Err(SpecV2Error::UnknownSelectionRule)
        );
    }

    #[test]
    fn policy_envelope_violations_are_refused() {
        let mut case = fields();
        case.normalized_decimals = 19;
        assert_eq!(SourceSpecV2::new(case), Err(SpecV2Error::InvalidScale));

        case = fields();
        case.bucket_seconds = 0;
        assert_eq!(SourceSpecV2::new(case), Err(SpecV2Error::InvalidGrid));
        case.bucket_seconds = MODEL_MAX_BUCKET_SECONDS + 1;
        assert_eq!(SourceSpecV2::new(case), Err(SpecV2Error::InvalidGrid));

        case = fields();
        case.max_staleness_slots = 0;
        assert_eq!(
            SourceSpecV2::new(case),
            Err(SpecV2Error::InvalidFreshnessPolicy)
        );
        case = fields();
        case.max_staleness_seconds = 0;
        assert_eq!(
            SourceSpecV2::new(case),
            Err(SpecV2Error::InvalidFreshnessPolicy)
        );

        for hostile in [
            {
                let mut c = fields();
                c.max_confidence_atoms = 0;
                c
            },
            {
                let mut c = fields();
                c.max_confidence_atoms = MODEL_MAX_CONFIDENCE_ATOMS + 1;
                c
            },
            {
                let mut c = fields();
                c.max_confidence_bps = 0;
                c
            },
            {
                let mut c = fields();
                c.max_confidence_bps = 10_001;
                c
            },
            {
                let mut c = fields();
                c.confidence_multiplier = 0;
                c
            },
            {
                let mut c = fields();
                c.confidence_multiplier = 33;
                c
            },
        ] {
            assert_eq!(
                SourceSpecV2::new(hostile),
                Err(SpecV2Error::InvalidConfidencePolicy)
            );
        }

        let mut hostile = spec().encode_canonical();
        hostile[258..266].copy_from_slice(&0_u64.to_le_bytes());
        assert_eq!(
            SourceSpecV2::decode_canonical(&hostile),
            Err(SpecV2Error::InvalidGrid)
        );
    }

    #[test]
    fn v2_domain_is_distinct_from_v1() {
        assert_eq!(FEED_DOMAIN_V2, b"dragons-clutch/feed/v2");
        assert_ne!(&FEED_DOMAIN_V2[..], b"dragons-clutch/feed/v1");
        assert_ne!(SOURCE_SPEC_V2_MAGIC, *b"DCSRCV1\0");
    }
}
