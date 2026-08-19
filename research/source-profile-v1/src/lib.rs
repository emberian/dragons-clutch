#![no_std]
#![forbid(unsafe_code)]
//! Offline model of a Pyth `PriceUpdateV2` boundary-price source profile.
//!
//! This crate is not wired into the Dragon's Clutch runtime.  It establishes
//! two narrow facts against the account/message layout reviewed in
//! `PROVENANCE.md`:
//!
//! 1. hostile bytes can be parsed without an oracle SDK or unchecked layout
//!    projection; and
//! 2. Pyth's `prev_publish_time < T <= publish_time` rule selects a point-price
//!    update for one immutable boundary without allowing the submitter to pick
//!    another otherwise-fresh update.
//!
//! [`auth_v2`] makes the deployment/config/adjacent-post/Clock/ownership join
//! executable as a typed contract. Its loader and Instructions-sysvar
//! projections still require exact production parsers; a raw parser result or
//! caller-asserted projection is not source authentication.

pub mod auth_v2;
pub mod crossing_v1;
pub mod spec_v2;

/// SHA-256(`"account:PriceUpdateV2"`)[0..8], Anchor's account discriminator.
pub const PRICE_UPDATE_V2_DISCRIMINATOR: [u8; 8] = [0x22, 0xf1, 0x23, 0x63, 0x9d, 0x7e, 0xf4, 0xcd];

/// Space allocated by the reviewed Pyth receiver SDK for `PriceUpdateV2`.
///
/// A fully verified Borsh enum occupies one byte, leaving one zero byte at the
/// end of this maximum-sized account.  Partial verification occupies both
/// bytes and is refused by this profile.
pub const PRICE_UPDATE_V2_ACCOUNT_LEN: usize = 134;

const FULL_VERIFICATION_VARIANT: u8 = 1;
const FULL_MESSAGE_END: usize = 133;

/// Refusals from hostile account bytes, metadata, selection, or normalization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    WrongOwner,
    ExecutableAccount,
    WrongLength,
    WrongDiscriminator,
    NotFullyVerified,
    NonCanonicalPadding,
    WrongFeed,
    InvalidPublishTime,
    NotBoundaryCrossing,
    InvalidPrice,
    InvalidConfidence,
    UnsupportedExponent,
    ArithmeticOverflow,
}

/// Metadata the Solana adapter must authenticate before treating bytes as a
/// receiver-owned account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountView<'a> {
    /// Ephemeral update-account key. It is bound by the immediate-post proof,
    /// never by the immutable SourceSpec identity.
    pub key: [u8; 32],
    pub owner: [u8; 32],
    pub executable: bool,
    pub data: &'a [u8],
}

/// Exact fields used by the boundary-price profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FullPriceUpdateV2 {
    pub write_authority: [u8; 32],
    pub feed_id: [u8; 32],
    pub price: i64,
    pub confidence: u64,
    pub exponent: i32,
    pub publish_time: i64,
    pub prev_publish_time: i64,
    pub ema_price: i64,
    pub ema_confidence: u64,
    /// Solana slot at which the receiver stored this account.  This is not a
    /// source-native price sequence or Pyth publish slot.
    pub posted_slot: u64,
}

/// A conservative integer interval at one normalized decimal scale.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NormalizedInterval {
    pub low: u128,
    pub high: u128,
}

/// Parse one fully verified receiver account with exact owner, feed, length,
/// discriminator, enum variant, and padding checks.
///
/// The caller must additionally prove that `expected_receiver` is the pinned
/// deployment and that this account was posted under the pinned receiver
/// configuration.  Ownership alone cannot prove either fact.
pub fn parse_full_price_update_v2(
    account: AccountView<'_>,
    expected_receiver: [u8; 32],
    expected_feed: [u8; 32],
) -> Result<FullPriceUpdateV2, Error> {
    if account.owner != expected_receiver {
        return Err(Error::WrongOwner);
    }
    if account.executable {
        return Err(Error::ExecutableAccount);
    }
    if account.data.len() != PRICE_UPDATE_V2_ACCOUNT_LEN {
        return Err(Error::WrongLength);
    }
    if account.data[..8] != PRICE_UPDATE_V2_DISCRIMINATOR {
        return Err(Error::WrongDiscriminator);
    }

    // `VerificationLevel` is Borsh encoded.  `Partial { num_signatures }` is
    // variant 0 followed by one byte; `Full` is variant 1 with no payload.
    if account.data[40] != FULL_VERIFICATION_VARIANT {
        return Err(Error::NotFullyVerified);
    }
    if account.data[FULL_MESSAGE_END] != 0 {
        return Err(Error::NonCanonicalPadding);
    }

    let mut write_authority = [0_u8; 32];
    write_authority.copy_from_slice(&account.data[8..40]);
    let mut feed_id = [0_u8; 32];
    feed_id.copy_from_slice(&account.data[41..73]);
    if feed_id != expected_feed {
        return Err(Error::WrongFeed);
    }

    let update = FullPriceUpdateV2 {
        write_authority,
        feed_id,
        price: i64_at(account.data, 73),
        confidence: u64_at(account.data, 81),
        exponent: i32_at(account.data, 89),
        publish_time: i64_at(account.data, 93),
        prev_publish_time: i64_at(account.data, 101),
        ema_price: i64_at(account.data, 109),
        ema_confidence: u64_at(account.data, 117),
        posted_slot: u64_at(account.data, 125),
    };
    if update.publish_time < 0 || update.prev_publish_time < 0 {
        return Err(Error::InvalidPublishTime);
    }
    Ok(update)
}

/// Apply Pyth's point-in-time selection rule.
///
/// The reviewed upstream message definition states that, for any instant `T`,
/// the unique update is the one satisfying this strict/non-strict pair.  An
/// unsuccessful aggregation may have equal previous and current times; such a
/// message selects no boundary here.
pub fn selects_boundary(update: FullPriceUpdateV2, boundary_unix_seconds: u64) -> bool {
    let Ok(boundary) = i64::try_from(boundary_unix_seconds) else {
        return false;
    };
    update.prev_publish_time < boundary && boundary <= update.publish_time
}

/// Require the exact crossing update for a canonical boundary.
pub fn require_boundary(
    update: FullPriceUpdateV2,
    boundary_unix_seconds: u64,
) -> Result<FullPriceUpdateV2, Error> {
    if !selects_boundary(update, boundary_unix_seconds) {
        return Err(Error::NotBoundaryCrossing);
    }
    Ok(update)
}

/// Normalize `(price +/- multiplier*confidence) * 10^exponent` into positive
/// atoms with `target_decimals` decimal places.
///
/// Division rounds the low endpoint down and the high endpoint up.  No center
/// plus radius is returned because decimal division can make the conservative
/// interval asymmetric by one atom.
pub fn normalize_interval(
    update: FullPriceUpdateV2,
    target_decimals: u8,
    confidence_multiplier: u16,
) -> Result<NormalizedInterval, Error> {
    if update.price <= 0 {
        return Err(Error::InvalidPrice);
    }
    if confidence_multiplier == 0 {
        return Err(Error::InvalidConfidence);
    }
    if target_decimals > 18 || !(-38..=38).contains(&update.exponent) {
        return Err(Error::UnsupportedExponent);
    }

    let price = i128::from(update.price);
    let widened_confidence = i128::from(update.confidence)
        .checked_mul(i128::from(confidence_multiplier))
        .ok_or(Error::ArithmeticOverflow)?;
    let low_source = price
        .checked_sub(widened_confidence)
        .filter(|value| *value > 0)
        .ok_or(Error::InvalidConfidence)?;
    let high_source = price
        .checked_add(widened_confidence)
        .ok_or(Error::ArithmeticOverflow)?;

    let shift = update
        .exponent
        .checked_add(i32::from(target_decimals))
        .ok_or(Error::ArithmeticOverflow)?;
    let (low, high) = if shift >= 0 {
        let factor = pow10(u32::try_from(shift).map_err(|_| Error::UnsupportedExponent)?)?;
        (
            low_source
                .checked_mul(factor)
                .ok_or(Error::ArithmeticOverflow)?,
            high_source
                .checked_mul(factor)
                .ok_or(Error::ArithmeticOverflow)?,
        )
    } else {
        let magnitude = shift.checked_neg().ok_or(Error::ArithmeticOverflow)?;
        let divisor = pow10(u32::try_from(magnitude).map_err(|_| Error::UnsupportedExponent)?)?;
        let low = low_source / divisor;
        let high = high_source
            .checked_add(divisor - 1)
            .ok_or(Error::ArithmeticOverflow)?
            / divisor;
        (low, high)
    };

    Ok(NormalizedInterval {
        low: u128::try_from(low).map_err(|_| Error::InvalidPrice)?,
        high: u128::try_from(high).map_err(|_| Error::InvalidPrice)?,
    })
}

fn pow10(exponent: u32) -> Result<i128, Error> {
    let mut value = 1_i128;
    let mut index = 0_u32;
    while index < exponent {
        value = value.checked_mul(10).ok_or(Error::ArithmeticOverflow)?;
        index += 1;
    }
    Ok(value)
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
    const FIXTURE_HEX: &str = include_str!("../fixtures/price-update-v2-full.hex");

    fn fixture() -> [u8; PRICE_UPDATE_V2_ACCOUNT_LEN] {
        let source = FIXTURE_HEX.trim().as_bytes();
        assert_eq!(source.len(), PRICE_UPDATE_V2_ACCOUNT_LEN * 2);
        let mut out = [0_u8; PRICE_UPDATE_V2_ACCOUNT_LEN];
        let mut index = 0_usize;
        while index < out.len() {
            out[index] = (nibble(source[index * 2]) << 4) | nibble(source[index * 2 + 1]);
            index += 1;
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

    fn parse(bytes: &[u8]) -> Result<FullPriceUpdateV2, Error> {
        parse_full_price_update_v2(
            AccountView {
                key: [0x44; 32],
                owner: RECEIVER,
                executable: false,
                data: bytes,
            },
            RECEIVER,
            FEED,
        )
    }

    #[test]
    fn exact_official_layout_fixture_parses() {
        let bytes = fixture();
        let update = parse(&bytes).unwrap();
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
    fn only_the_crossing_update_selects_the_boundary() {
        let update = parse(&fixture()).unwrap();
        assert!(selects_boundary(update, 1_700_000_000));
        assert!(!selects_boundary(update, 1_699_999_999));
        assert!(!selects_boundary(update, 1_700_000_012));

        let mut unsuccessful = update;
        unsuccessful.prev_publish_time = unsuccessful.publish_time;
        assert!(!selects_boundary(unsuccessful, 1_700_000_011));
    }

    #[test]
    fn hostile_metadata_and_bytes_fail_closed() {
        let bytes = fixture();
        let wrong_owner = AccountView {
            key: [0x44; 32],
            owner: [0x44; 32],
            executable: false,
            data: &bytes,
        };
        assert_eq!(
            parse_full_price_update_v2(wrong_owner, RECEIVER, FEED),
            Err(Error::WrongOwner)
        );

        let executable = AccountView {
            key: [0x44; 32],
            owner: RECEIVER,
            executable: true,
            data: &bytes,
        };
        assert_eq!(
            parse_full_price_update_v2(executable, RECEIVER, FEED),
            Err(Error::ExecutableAccount)
        );
        assert_eq!(parse(&bytes[..133]), Err(Error::WrongLength));

        let mut bad = bytes;
        bad[0] ^= 1;
        assert_eq!(parse(&bad), Err(Error::WrongDiscriminator));
        bad = bytes;
        bad[40] = 0;
        assert_eq!(parse(&bad), Err(Error::NotFullyVerified));
        bad = bytes;
        bad[133] = 1;
        assert_eq!(parse(&bad), Err(Error::NonCanonicalPadding));
        bad = bytes;
        bad[41] ^= 1;
        assert_eq!(parse(&bad), Err(Error::WrongFeed));
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
    fn normalization_rounds_interval_outward() {
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
        assert_eq!(normalize_interval(update, 8, 1), Err(Error::InvalidPrice));
        update.price = 10;
        update.confidence = 10;
        assert_eq!(
            normalize_interval(update, 8, 1),
            Err(Error::InvalidConfidence)
        );
        update.price = i64::MAX;
        update.confidence = 0;
        update.exponent = 38;
        assert_eq!(
            normalize_interval(update, 18, 1),
            Err(Error::ArithmeticOverflow)
        );
    }
}
