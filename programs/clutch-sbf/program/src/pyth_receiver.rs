//! Pinned Pyth receiver `PriceUpdateV2` account decode and price
//! normalization.
//!
//! ## Why this exists
//!
//! The R2 pull profile consumes a **caller-created, ephemeral** price-update
//! account written by the Pyth receiver program. `SourceSpecV1` cannot express
//! that shape: it pins one immutable source data-account key, and a pull update
//! account has no such key (`research/source-profile-v1/src/spec_v2.rs`, delta
//! 1). This module is the parser half of the v2 release triple — the piece that
//! turns 134 authenticated bytes into the ten typed fields
//! [`crate::source_v2::auth`] joins against identity, Clock, and the crossing
//! rule.
//!
//! It is the runtime form of `research/source-profile-v1/src/lib.rs`, ported
//! per `R2_PULL_PROMOTION_PLAN.md` P0.2 with `sha2` replaced by the runtime
//! SHA-256 syscall wrapper. The research crate remains the model; any
//! divergence between the two is a defect in this file, and
//! `the_pinned_research_fixture_parses_to_its_recorded_fields` pins the exact
//! byte vector both agree on.
//!
//! ## Primary source
//!
//! Account layout and the `VerificationLevel` Borsh encoding are as reviewed in
//! `research/source-profile-v1/PROVENANCE.md` (the `pyth_solana_receiver_sdk`
//! price-update module at the pinned revision). The discriminator is Anchor's
//! `SHA-256("account:PriceUpdateV2")[0..8]`.
//!
//! ## What this module does not decide
//!
//! Ownership proves only that the receiver wrote these bytes; it proves nothing
//! about *which* receiver deployment, *which* governance configuration, or
//! *whether this transaction* posted them. Those are
//! [`crate::source_v2::auth`]'s joins, and parsing without them is not source
//! authentication. This module therefore takes `expected_receiver` and
//! `expected_feed` as arguments rather than reading them from anywhere.
//!
//! The receiver `Config` account is deliberately **not** parsed. The profile
//! authenticates it by SHA-256 over its complete body
//! ([`config_byte_digest`]), so a governance change of any kind — fee,
//! `valid_data_sources`, router address, `minimum_signatures` — is a new feed
//! generation by construction and no field-level exception can exist. A codec
//! would create one.

/// Anchor account discriminator, `SHA-256("account:PriceUpdateV2")[0..8]`.
pub const PRICE_UPDATE_V2_DISCRIMINATOR: [u8; 8] = [0x22, 0xf1, 0x23, 0x63, 0x9d, 0x7e, 0xf4, 0xcd];

/// Space the reviewed receiver SDK allocates for a `PriceUpdateV2` account.
///
/// A fully verified `VerificationLevel` is a one-byte Borsh enum, leaving one
/// zero byte at the end of this maximum-sized account; partial verification
/// occupies both bytes and is refused by this profile.
pub const PRICE_UPDATE_V2_ACCOUNT_LEN: usize = 134;

/// Borsh variant index of `VerificationLevel::Full`.
///
/// `Partial { num_signatures }` is variant `0` followed by one payload byte;
/// `Full` is variant `1` with no payload.
pub const VERIFICATION_LEVEL_FULL: u8 = 1;

/// Offset of the 32-byte write authority.
pub const OFFSET_WRITE_AUTHORITY: usize = 8;
/// Offset of the one-byte `VerificationLevel` discriminant.
pub const OFFSET_VERIFICATION_LEVEL: usize = 40;
/// Offset of the 32-byte provider feed id.
pub const OFFSET_FEED_ID: usize = 41;
/// Offset of the `i64` price.
pub const OFFSET_PRICE: usize = 73;
/// Offset of the `u64` confidence.
pub const OFFSET_CONFIDENCE: usize = 81;
/// Offset of the `i32` decimal exponent.
pub const OFFSET_EXPONENT: usize = 89;
/// Offset of the `i64` publish time.
pub const OFFSET_PUBLISH_TIME: usize = 93;
/// Offset of the `i64` previous publish time.
pub const OFFSET_PREV_PUBLISH_TIME: usize = 101;
/// Offset of the `i64` EMA price.
pub const OFFSET_EMA_PRICE: usize = 109;
/// Offset of the `u64` EMA confidence.
pub const OFFSET_EMA_CONFIDENCE: usize = 117;
/// Offset of the `u64` receiver-write slot.
pub const OFFSET_POSTED_SLOT: usize = 125;
/// Offset of the trailing byte that a fully verified message leaves zero.
pub const OFFSET_TRAILING_PAD: usize = 133;

const _: () = assert!(OFFSET_TRAILING_PAD + 1 == PRICE_UPDATE_V2_ACCOUNT_LEN);
const _: () = assert!(OFFSET_POSTED_SLOT + 8 == OFFSET_TRAILING_PAD);

/// Refusals from hostile update bytes, metadata, or normalization.
///
/// Deliberately a module-local vocabulary in the style of
/// [`crate::source::SourceError`] and [`crate::loader_state::LoaderStateError`].
/// Projection onto stable numeric codes is
/// [`crate::instructions::source_ingest_v2`]'s job, and the R2 plan's P0.8
/// error-granularity decision owns how coarse that projection is allowed to be.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PythReceiverError {
    /// The account is not owned by the pinned receiver program.
    WrongOwner,
    /// A data account was presented as executable.
    ExecutableAccount,
    /// The body is not exactly [`PRICE_UPDATE_V2_ACCOUNT_LEN`] bytes.
    WrongLength,
    /// The leading eight bytes are not [`PRICE_UPDATE_V2_DISCRIMINATOR`].
    WrongDiscriminator,
    /// The `VerificationLevel` discriminant is not `Full`.
    NotFullyVerified,
    /// The trailing byte of a fully verified message was not zero.
    NonCanonicalPadding,
    /// The update names a different provider feed than the spec pins.
    WrongFeed,
    /// A publish time was negative and cannot own an unsigned archive field.
    InvalidPublishTime,
    /// The price is zero or negative.
    InvalidPrice,
    /// The confidence policy is unsatisfiable for this update.
    InvalidConfidence,
    /// The decimal exponent or target scale is outside the integer envelope.
    UnsupportedExponent,
    /// Interval arithmetic left the `i128`/`u128` envelope.
    ArithmeticOverflow,
}

/// Metadata a caller must present alongside update bytes.
///
/// `key` is the **ephemeral** update-account address. It is bound by the
/// immediate-post join, never by the immutable SourceSpec identity, and this
/// module never compares it against anything — see
/// [`crate::source_v2::auth`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PriceUpdateAccountViewV1<'a> {
    key: [u8; 32],
    owner: [u8; 32],
    executable: bool,
    data: &'a [u8],
}

impl<'a> PriceUpdateAccountViewV1<'a> {
    /// Wrap one runtime account's metadata and body.
    pub const fn new(key: [u8; 32], owner: [u8; 32], executable: bool, data: &'a [u8]) -> Self {
        Self {
            key,
            owner,
            executable,
            data,
        }
    }

    /// The ephemeral update-account address.
    pub const fn key(self) -> [u8; 32] {
        self.key
    }
}

/// The exact fields the boundary-price profile consumes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FullPriceUpdateV2 {
    /// Authority the receiver recorded as permitted to rewrite this account.
    pub write_authority: [u8; 32],
    /// Provider feed id carried by the message.
    pub feed_id: [u8; 32],
    /// Signed price at [`Self::exponent`] decimal scale.
    pub price: i64,
    /// Confidence half-width at the same scale.
    pub confidence: u64,
    /// Decimal exponent; the price is `price * 10^exponent`.
    pub exponent: i32,
    /// Aggregate publish time of this message.
    pub publish_time: i64,
    /// Publish time of the preceding successful aggregate.
    pub prev_publish_time: i64,
    /// Exponential-moving-average price; carried, never admitted.
    pub ema_price: i64,
    /// Exponential-moving-average confidence; carried, never admitted.
    pub ema_confidence: u64,
    /// Solana slot at which the **receiver** stored this account.
    ///
    /// This is a receiver-write slot. It is explicitly not a source-native
    /// price sequence and not a Pyth publish slot, and the archive's
    /// `publish_slot` field takes exactly this value with that meaning.
    pub posted_slot: u64,
}

/// A conservative integer interval at one normalized decimal scale.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NormalizedInterval {
    /// Low endpoint, rounded down.
    pub low: u128,
    /// High endpoint, rounded up.
    pub high: u128,
}

/// SHA-256 over the **complete** receiver `Config` account body.
///
/// Any byte change is a new immutable source generation. There is deliberately
/// no field-level governance exception and no partial-body digest.
pub fn config_byte_digest(data: &[u8]) -> [u8; 32] {
    solana_sha256_hasher::hashv(&[data]).to_bytes()
}

/// Parse one fully verified receiver account with exact owner, length,
/// discriminator, verification-level, padding, and feed checks.
///
/// The caller must separately prove that `expected_receiver` is the pinned
/// deployment and that these bytes were posted by *this* transaction under the
/// pinned configuration. Ownership alone establishes neither.
pub fn parse_full_price_update_v2(
    account: PriceUpdateAccountViewV1<'_>,
    expected_receiver: [u8; 32],
    expected_feed: [u8; 32],
) -> Result<FullPriceUpdateV2, PythReceiverError> {
    if account.owner != expected_receiver {
        return Err(PythReceiverError::WrongOwner);
    }
    if account.executable {
        return Err(PythReceiverError::ExecutableAccount);
    }
    if account.data.len() != PRICE_UPDATE_V2_ACCOUNT_LEN {
        return Err(PythReceiverError::WrongLength);
    }
    if account.data[..8] != PRICE_UPDATE_V2_DISCRIMINATOR {
        return Err(PythReceiverError::WrongDiscriminator);
    }
    if account.data[OFFSET_VERIFICATION_LEVEL] != VERIFICATION_LEVEL_FULL {
        return Err(PythReceiverError::NotFullyVerified);
    }
    if account.data[OFFSET_TRAILING_PAD] != 0 {
        return Err(PythReceiverError::NonCanonicalPadding);
    }

    let feed_id = array_32(account.data, OFFSET_FEED_ID);
    if feed_id != expected_feed {
        return Err(PythReceiverError::WrongFeed);
    }

    let update = FullPriceUpdateV2 {
        write_authority: array_32(account.data, OFFSET_WRITE_AUTHORITY),
        feed_id,
        price: i64_at(account.data, OFFSET_PRICE),
        confidence: u64_at(account.data, OFFSET_CONFIDENCE),
        exponent: i32_at(account.data, OFFSET_EXPONENT),
        publish_time: i64_at(account.data, OFFSET_PUBLISH_TIME),
        prev_publish_time: i64_at(account.data, OFFSET_PREV_PUBLISH_TIME),
        ema_price: i64_at(account.data, OFFSET_EMA_PRICE),
        ema_confidence: u64_at(account.data, OFFSET_EMA_CONFIDENCE),
        posted_slot: u64_at(account.data, OFFSET_POSTED_SLOT),
    };
    if update.publish_time < 0 || update.prev_publish_time < 0 {
        return Err(PythReceiverError::InvalidPublishTime);
    }
    Ok(update)
}

/// Apply the provider's point-in-time selection predicate at one instant.
///
/// For any instant `T` the unique qualifying update is the one satisfying
/// `prev_publish_time < T <= publish_time`. A failed aggregation may carry
/// `prev == publish`; such a message satisfies the predicate for no `T` at all
/// and therefore witnesses no boundary.
pub fn selects_boundary(update: FullPriceUpdateV2, boundary_unix_seconds: u64) -> bool {
    let Ok(boundary) = i64::try_from(boundary_unix_seconds) else {
        return false;
    };
    update.prev_publish_time < boundary && boundary <= update.publish_time
}

/// Normalize `(price ± multiplier·confidence)·10^exponent` into positive atoms
/// at `target_decimals` decimal places.
///
/// Division rounds the low endpoint down and the high endpoint up, so the
/// admitted interval always contains the true one. No centre-plus-radius form
/// is returned: decimal division can make the conservative interval asymmetric
/// by one atom, and a radius would have to round one side inward.
pub fn normalize_interval(
    update: FullPriceUpdateV2,
    target_decimals: u8,
    confidence_multiplier: u16,
) -> Result<NormalizedInterval, PythReceiverError> {
    if update.price <= 0 {
        return Err(PythReceiverError::InvalidPrice);
    }
    if confidence_multiplier == 0 {
        return Err(PythReceiverError::InvalidConfidence);
    }
    if target_decimals > 18 || !(-38..=38).contains(&update.exponent) {
        return Err(PythReceiverError::UnsupportedExponent);
    }

    let price = i128::from(update.price);
    let widened_confidence = i128::from(update.confidence)
        .checked_mul(i128::from(confidence_multiplier))
        .ok_or(PythReceiverError::ArithmeticOverflow)?;
    let low_source = price
        .checked_sub(widened_confidence)
        .filter(|value| *value > 0)
        .ok_or(PythReceiverError::InvalidConfidence)?;
    let high_source = price
        .checked_add(widened_confidence)
        .ok_or(PythReceiverError::ArithmeticOverflow)?;

    let shift = update
        .exponent
        .checked_add(i32::from(target_decimals))
        .ok_or(PythReceiverError::ArithmeticOverflow)?;
    let (low, high) = if shift >= 0 {
        let factor =
            pow10_i128(u32::try_from(shift).map_err(|_| PythReceiverError::UnsupportedExponent)?)?;
        (
            low_source
                .checked_mul(factor)
                .ok_or(PythReceiverError::ArithmeticOverflow)?,
            high_source
                .checked_mul(factor)
                .ok_or(PythReceiverError::ArithmeticOverflow)?,
        )
    } else {
        let magnitude = shift
            .checked_neg()
            .ok_or(PythReceiverError::ArithmeticOverflow)?;
        let divisor = pow10_i128(
            u32::try_from(magnitude).map_err(|_| PythReceiverError::UnsupportedExponent)?,
        )?;
        let low = low_source / divisor;
        let high = high_source
            .checked_add(divisor - 1)
            .ok_or(PythReceiverError::ArithmeticOverflow)?
            / divisor;
        (low, high)
    };

    Ok(NormalizedInterval {
        low: u128::try_from(low).map_err(|_| PythReceiverError::InvalidPrice)?,
        high: u128::try_from(high).map_err(|_| PythReceiverError::InvalidPrice)?,
    })
}

/// Round `value·10^exponent` up to an unsigned integer at `target_decimals`.
///
/// Used for the absolute confidence cap, which must never be understated by
/// truncation: an inward-rounded half-width could pass a cap the true one
/// fails.
pub fn normalize_unsigned_ceil(
    value: u128,
    exponent: i32,
    target_decimals: u8,
) -> Result<u128, PythReceiverError> {
    let shift = exponent
        .checked_add(i32::from(target_decimals))
        .ok_or(PythReceiverError::ArithmeticOverflow)?;
    if shift >= 0 {
        let factor =
            pow10_u128(u32::try_from(shift).map_err(|_| PythReceiverError::ArithmeticOverflow)?)?;
        value
            .checked_mul(factor)
            .ok_or(PythReceiverError::ArithmeticOverflow)
    } else {
        let magnitude = shift
            .checked_neg()
            .ok_or(PythReceiverError::ArithmeticOverflow)?;
        let divisor = pow10_u128(
            u32::try_from(magnitude).map_err(|_| PythReceiverError::ArithmeticOverflow)?,
        )?;
        value
            .checked_add(divisor - 1)
            .ok_or(PythReceiverError::ArithmeticOverflow)
            .map(|rounded| rounded / divisor)
    }
}

fn pow10_i128(exponent: u32) -> Result<i128, PythReceiverError> {
    let mut value = 1_i128;
    let mut index = 0_u32;
    while index < exponent {
        value = value
            .checked_mul(10)
            .ok_or(PythReceiverError::ArithmeticOverflow)?;
        index += 1;
    }
    Ok(value)
}

fn pow10_u128(exponent: u32) -> Result<u128, PythReceiverError> {
    let mut value = 1_u128;
    let mut index = 0_u32;
    while index < exponent {
        value = value
            .checked_mul(10)
            .ok_or(PythReceiverError::ArithmeticOverflow)?;
        index += 1;
    }
    Ok(value)
}

fn array_32(bytes: &[u8], at: usize) -> [u8; 32] {
    let mut value = [0_u8; 32];
    value.copy_from_slice(&bytes[at..at + 32]);
    value
}

fn i64_at(bytes: &[u8], at: usize) -> i64 {
    let mut value = [0_u8; 8];
    value.copy_from_slice(&bytes[at..at + 8]);
    i64::from_le_bytes(value)
}

fn u64_at(bytes: &[u8], at: usize) -> u64 {
    let mut value = [0_u8; 8];
    value.copy_from_slice(&bytes[at..at + 8]);
    u64::from_le_bytes(value)
}

fn i32_at(bytes: &[u8], at: usize) -> i32 {
    let mut value = [0_u8; 4];
    value.copy_from_slice(&bytes[at..at + 4]);
    i32::from_le_bytes(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RECEIVER: [u8; 32] = [0x33; 32];
    const FEED: [u8; 32] = [0x22; 32];
    const UPDATE_KEY: [u8; 32] = [0x44; 32];

    /// The exact byte vector pinned by
    /// `research/source-profile-v1/fixtures/price-update-v2-full.hex`.
    ///
    /// Copied rather than `include_str!`'d across the workspace boundary: the
    /// program crate must not depend on a research crate's file tree, and a
    /// silent divergence is caught by
    /// `the_pinned_research_fixture_parses_to_its_recorded_fields`, whose
    /// expected field values are the research crate's own assertions.
    const FIXTURE_HEX: &str = concat!(
        "22f123639d7ef4cd",
        "1111111111111111111111111111111111111111111111111111111111111111",
        "01",
        "2222222222222222222222222222222222222222222222222222222222222222",
        "15cd5b0700000000",
        "3930000000000000",
        "f8ffffff",
        "0bf1536500000000",
        "fff0536500000000",
        "90b25b0700000000",
        "204e000000000000",
        "80b2e60e00000000",
        "00",
    );

    fn fixture() -> [u8; PRICE_UPDATE_V2_ACCOUNT_LEN] {
        let source = FIXTURE_HEX.as_bytes();
        assert_eq!(source.len(), PRICE_UPDATE_V2_ACCOUNT_LEN * 2);
        let mut out = [0_u8; PRICE_UPDATE_V2_ACCOUNT_LEN];
        for (index, byte) in out.iter_mut().enumerate() {
            *byte = (nibble(source[index * 2]) << 4) | nibble(source[index * 2 + 1]);
        }
        out
    }

    fn nibble(byte: u8) -> u8 {
        match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            _ => panic!("fixture is lowercase hexadecimal"),
        }
    }

    fn parse(bytes: &[u8]) -> Result<FullPriceUpdateV2, PythReceiverError> {
        parse_full_price_update_v2(
            PriceUpdateAccountViewV1::new(UPDATE_KEY, RECEIVER, false, bytes),
            RECEIVER,
            FEED,
        )
    }

    #[test]
    fn the_pinned_research_fixture_parses_to_its_recorded_fields() {
        let update = parse(&fixture()).expect("pinned fixture parses");
        assert_eq!(update.write_authority, [0x11; 32]);
        assert_eq!(update.feed_id, FEED);
        assert_eq!(update.price, 123_456_789);
        assert_eq!(update.confidence, 12_345);
        assert_eq!(update.exponent, -8);
        assert_eq!(update.prev_publish_time, 1_699_999_999);
        assert_eq!(update.publish_time, 1_700_000_011);
        assert_eq!(update.ema_price, 123_450_000);
        assert_eq!(update.ema_confidence, 20_000);
        assert_eq!(update.posted_slot, 250_000_000);
    }

    #[test]
    fn the_ephemeral_key_is_carried_and_never_compared() {
        // The parser must not develop an opinion about the update address:
        // binding it is the immediate-post join's job, and a parser-side pin
        // would resurrect exactly the V1 assumption v2 exists to remove.
        let bytes = fixture();
        for key in [[0_u8; 32], [0xff; 32], RECEIVER, FEED] {
            let view = PriceUpdateAccountViewV1::new(key, RECEIVER, false, &bytes);
            assert_eq!(view.key(), key);
            assert!(parse_full_price_update_v2(view, RECEIVER, FEED).is_ok());
        }
    }

    #[test]
    fn hostile_metadata_fails_closed() {
        let bytes = fixture();
        assert_eq!(
            parse_full_price_update_v2(
                PriceUpdateAccountViewV1::new(UPDATE_KEY, [0x44; 32], false, &bytes),
                RECEIVER,
                FEED,
            ),
            Err(PythReceiverError::WrongOwner)
        );
        assert_eq!(
            parse_full_price_update_v2(
                PriceUpdateAccountViewV1::new(UPDATE_KEY, RECEIVER, true, &bytes),
                RECEIVER,
                FEED,
            ),
            Err(PythReceiverError::ExecutableAccount)
        );
    }

    #[test]
    fn truncation_at_every_byte_boundary_refuses() {
        let bytes = fixture();
        for cut in 0..PRICE_UPDATE_V2_ACCOUNT_LEN {
            assert_eq!(
                parse(&bytes[..cut]),
                Err(PythReceiverError::WrongLength),
                "truncation to {cut} bytes must refuse"
            );
        }
        let mut extended = [0_u8; PRICE_UPDATE_V2_ACCOUNT_LEN + 1];
        extended[..PRICE_UPDATE_V2_ACCOUNT_LEN].copy_from_slice(&bytes);
        assert_eq!(parse(&extended), Err(PythReceiverError::WrongLength));
    }

    #[test]
    fn every_discriminator_byte_is_load_bearing() {
        let bytes = fixture();
        for at in 0..8 {
            let mut hostile = bytes;
            hostile[at] ^= 1;
            assert_eq!(parse(&hostile), Err(PythReceiverError::WrongDiscriminator));
        }
    }

    #[test]
    fn partial_verification_and_non_canonical_padding_refuse() {
        let bytes = fixture();
        // Variant 0 is `Partial { num_signatures }`; every non-`Full`
        // discriminant refuses, including values the enum does not define.
        for level in [0_u8, 2, 3, 0xff] {
            let mut hostile = bytes;
            hostile[OFFSET_VERIFICATION_LEVEL] = level;
            assert_eq!(parse(&hostile), Err(PythReceiverError::NotFullyVerified));
        }
        for pad in [1_u8, 0x0f, 0xff] {
            let mut hostile = bytes;
            hostile[OFFSET_TRAILING_PAD] = pad;
            assert_eq!(parse(&hostile), Err(PythReceiverError::NonCanonicalPadding));
        }
    }

    #[test]
    fn every_feed_id_byte_is_load_bearing() {
        let bytes = fixture();
        for at in OFFSET_FEED_ID..OFFSET_PRICE {
            let mut hostile = bytes;
            hostile[at] ^= 1;
            assert_eq!(parse(&hostile), Err(PythReceiverError::WrongFeed));
        }
    }

    #[test]
    fn negative_publish_times_cannot_own_unsigned_archive_fields() {
        let bytes = fixture();
        let mut hostile = bytes;
        hostile[OFFSET_PUBLISH_TIME..OFFSET_PUBLISH_TIME + 8]
            .copy_from_slice(&(-1_i64).to_le_bytes());
        assert_eq!(parse(&hostile), Err(PythReceiverError::InvalidPublishTime));

        hostile = bytes;
        hostile[OFFSET_PREV_PUBLISH_TIME..OFFSET_PREV_PUBLISH_TIME + 8]
            .copy_from_slice(&i64::MIN.to_le_bytes());
        assert_eq!(parse(&hostile), Err(PythReceiverError::InvalidPublishTime));
    }

    #[test]
    fn only_the_crossing_update_selects_a_boundary() {
        let update = parse(&fixture()).unwrap();
        assert!(selects_boundary(update, 1_700_000_000));
        assert!(selects_boundary(update, 1_700_000_011));
        assert!(!selects_boundary(update, 1_699_999_999));
        assert!(!selects_boundary(update, 1_700_000_012));

        // A failed aggregation carries `prev == publish` and witnesses no
        // instant at all, not even its own publish time.
        let mut degenerate = update;
        degenerate.prev_publish_time = degenerate.publish_time;
        for boundary in [1_700_000_010_u64, 1_700_000_011, 1_700_000_012] {
            assert!(!selects_boundary(degenerate, boundary));
        }
    }

    #[test]
    fn normalization_is_exact_when_scales_match() {
        let update = parse(&fixture()).unwrap();
        assert_eq!(
            normalize_interval(update, 8, 2),
            Ok(NormalizedInterval {
                low: 123_432_099,
                high: 123_481_479,
            })
        );
    }

    #[test]
    fn normalization_rounds_the_interval_outward() {
        let mut update = parse(&fixture()).unwrap();
        update.price = 12_345;
        update.confidence = 1;
        update.exponent = -3;
        assert_eq!(
            normalize_interval(update, 2, 1),
            Ok(NormalizedInterval {
                low: 1_234,
                high: 1_235,
            })
        );
    }

    #[test]
    fn invalid_numeric_states_are_refused() {
        let mut update = parse(&fixture()).unwrap();
        update.price = -1;
        assert_eq!(
            normalize_interval(update, 8, 1),
            Err(PythReceiverError::InvalidPrice)
        );
        update.price = 10;
        update.confidence = 10;
        assert_eq!(
            normalize_interval(update, 8, 1),
            Err(PythReceiverError::InvalidConfidence)
        );
        update.price = i64::MAX;
        update.confidence = 0;
        assert_eq!(
            normalize_interval(update, 8, 0),
            Err(PythReceiverError::InvalidConfidence)
        );
        update.confidence = 1;
        update.exponent = 39;
        assert_eq!(
            normalize_interval(update, 8, 1),
            Err(PythReceiverError::UnsupportedExponent)
        );
        update.exponent = 38;
        assert_eq!(
            normalize_interval(update, 18, 1),
            Err(PythReceiverError::ArithmeticOverflow)
        );
    }

    #[test]
    fn the_config_digest_covers_the_whole_body_with_no_field_exception() {
        let body = b"post-cutover-config-generation".as_slice();
        let pinned = config_byte_digest(body);
        assert_ne!(pinned, [0_u8; 32]);
        assert_eq!(pinned, config_byte_digest(body));

        // Every single-byte mutation is a different generation, and so is any
        // length change: there is no prefix, suffix, or field-level exception.
        for at in 0..body.len() {
            let mut hostile = body.to_vec();
            hostile[at] ^= 1;
            assert_ne!(config_byte_digest(&hostile), pinned);
        }
        assert_ne!(config_byte_digest(&body[..body.len() - 1]), pinned);
        let mut extended = body.to_vec();
        extended.push(0);
        assert_ne!(config_byte_digest(&extended), pinned);
        assert_ne!(config_byte_digest(&[]), pinned);
    }

    #[test]
    fn unsigned_ceil_normalization_never_understates_a_half_width() {
        // 1 confidence atom at 10^-3, normalized to 2 decimals, is 0.01 of a
        // unit and must round up to one atom rather than truncate to zero.
        assert_eq!(normalize_unsigned_ceil(1, -3, 2), Ok(1));
        assert_eq!(normalize_unsigned_ceil(0, -3, 2), Ok(0));
        assert_eq!(normalize_unsigned_ceil(12_345, -8, 8), Ok(12_345));
        assert_eq!(normalize_unsigned_ceil(1, 2, 0), Ok(100));
        assert_eq!(
            normalize_unsigned_ceil(u128::MAX, 1, 0),
            Err(PythReceiverError::ArithmeticOverflow)
        );
    }
}
