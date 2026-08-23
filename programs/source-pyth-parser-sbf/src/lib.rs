#![no_std]
#![deny(missing_docs)]

//! Read-only Pyth `PriceUpdateV2` parser for SourceSeries 77/v2 action 4.
//!
//! This SBF release is deliberately distinct from the Pyth receiver. The
//! receiver owns and posts the update; this program owns only an immutable
//! route config and returns the canonical 120-byte [`ParserOutputV1`]. It
//! performs no CPI and mutates no account.

use clutch_pyth_parser_v1::{PythParserConfigV1, PythParserRequestV1};
use clutch_source_plane_v3::ContentId;
use clutch_source_plane_v3_runtime::{account_data_id, ParserOutputV1, RuntimeKey};
use clutch_source_profile_v1::{
    normalize_interval, parse_full_price_update_v2, require_boundary, AccountView,
};
use solana_account_info::AccountInfo;
use solana_clock::Clock;
use solana_get_sysvar::GetSysvar;
use solana_program_entrypoint::{entrypoint, ProgramResult};
use solana_program_error::ProgramError;
use solana_pubkey::Pubkey;

const CONFIG_POSITION: usize = 0;
const FEED_POSITION: usize = 1;
const CLOCK_POSITION: usize = 2;
const ACCOUNT_COUNT: usize = 3;

/// Stable parser refusal codes. These codes describe this parser artifact,
/// not Source V3 core or Clutch dispatcher errors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ParserError {
    /// Instruction bytes did not match the fixed parser request.
    InvalidRequest = 1,
    /// Account count or privileges did not match the fixed role table.
    WrongAccounts = 2,
    /// Parser config bytes, owner, or self identity did not authenticate.
    InvalidConfig = 3,
    /// Feed identity, owner, verification status, or Pyth body refused.
    InvalidFeed = 4,
    /// Canonical Clock identity or runtime Clock read refused.
    InvalidClock = 5,
    /// The update did not cross the state-derived boundary.
    NotBoundaryCrossing = 6,
    /// Publish time or posted slot lay outside the config freshness bounds.
    StaleOrFuture = 7,
    /// Checked conservative integer normalization refused.
    InvalidNormalization = 8,
    /// Canonical output digest or fixed-width encoding refused.
    InvalidOutput = 9,
}

impl From<ParserError> for ProgramError {
    fn from(value: ParserError) -> Self {
        Self::Custom(value as u32)
    }
}

/// Runtime Clock facts consumed by the pure parser evaluation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParserClockV1 {
    /// Canonical current slot.
    pub slot: u64,
    /// Canonical nonnegative Unix timestamp.
    pub unix_timestamp: u64,
}

entrypoint!(process_instruction);

/// Authenticate config, feed, Clock, boundary and normalization, then return
/// exactly the canonical 120-byte `ParserOutputV1` body.
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if accounts.len() != ACCOUNT_COUNT {
        return Err(ParserError::WrongAccounts.into());
    }
    let config_account = &accounts[CONFIG_POSITION];
    let feed = &accounts[FEED_POSITION];
    let clock_account = &accounts[CLOCK_POSITION];
    if config_account.is_signer
        || config_account.is_writable
        || config_account.executable
        || config_account.owner != program_id
        || feed.is_signer
        || feed.is_writable
        || feed.executable
        || clock_account.is_signer
        || clock_account.is_writable
        || clock_account.executable
    {
        return Err(ParserError::WrongAccounts.into());
    }
    if *clock_account.key != solana_sdk_ids::sysvar::clock::ID
        || *clock_account.owner != solana_sdk_ids::sysvar::ID
    {
        return Err(ParserError::InvalidClock.into());
    }
    let request = PythParserRequestV1::decode(instruction_data)
        .map_err(|_| ProgramError::from(ParserError::InvalidRequest))?;
    let config_data = config_account
        .try_borrow_data()
        .map_err(|_| ProgramError::from(ParserError::InvalidConfig))?;
    let config = PythParserConfigV1::decode(&config_data)
        .map_err(|_| ProgramError::from(ParserError::InvalidConfig))?;
    if config.config_account != config_account.key.to_bytes() {
        return Err(ParserError::InvalidConfig.into());
    }
    let runtime_clock = Clock::get().map_err(|_| ParserError::InvalidClock)?;
    let unix_timestamp =
        u64::try_from(runtime_clock.unix_timestamp).map_err(|_| ParserError::InvalidClock)?;
    let feed_data = feed
        .try_borrow_data()
        .map_err(|_| ProgramError::from(ParserError::InvalidFeed))?;
    let output = evaluate(
        config,
        request,
        ParserClockV1 {
            slot: runtime_clock.slot,
            unix_timestamp,
        },
        feed.key.to_bytes(),
        feed.owner.to_bytes(),
        &feed_data,
    )?;
    let return_bytes = output.encode().map_err(|_| ParserError::InvalidOutput)?;
    solana_cpi::set_return_data(&return_bytes);
    Ok(())
}

/// Pure, allocation-free parser semantics below the Solana account adapter.
pub fn evaluate(
    config: PythParserConfigV1,
    request: PythParserRequestV1,
    clock: ParserClockV1,
    feed_key: [u8; 32],
    feed_owner: [u8; 32],
    feed_data: &[u8],
) -> Result<ParserOutputV1, ParserError> {
    config.validate().map_err(|_| ParserError::InvalidConfig)?;
    request
        .validate()
        .map_err(|_| ParserError::InvalidRequest)?;
    if feed_key != config.feed_account || feed_owner != config.receiver_program {
        return Err(ParserError::InvalidFeed);
    }
    let update = parse_full_price_update_v2(
        AccountView {
            key: feed_key,
            owner: feed_owner,
            executable: false,
            data: feed_data,
        },
        config.receiver_program,
        config.pyth_feed_id,
    )
    .map_err(|_| ParserError::InvalidFeed)?;
    let update = require_boundary(update, request.boundary_unix_seconds)
        .map_err(|_| ParserError::NotBoundaryCrossing)?;
    let publish_time = u64::try_from(update.publish_time).map_err(|_| ParserError::InvalidFeed)?;
    if publish_time > clock.unix_timestamp
        || clock.unix_timestamp - publish_time > config.maximum_source_age_seconds
        || update.posted_slot > clock.slot
        || clock.slot - update.posted_slot > config.maximum_source_slot_lag
    {
        return Err(ParserError::StaleOrFuture);
    }
    // The sole named rounding boundary is inside `normalize_interval`: decimal
    // downscaling floors the low endpoint and ceilings the high endpoint.
    let interval = normalize_interval(update, config.target_decimals, config.confidence_multiplier)
        .map_err(|_| ParserError::InvalidNormalization)?;
    let feed_account_data_id = account_data_id(RuntimeKey::from_bytes(feed_key), feed_data)
        .map_err(|_| ParserError::InvalidOutput)?;
    let output = ParserOutputV1 {
        source_spec_id: ContentId::from_bytes(config.source_spec_id),
        low: interval.low,
        high: interval.high,
        // `PriceUpdateV2` exposes no source-native sequence. Its authenticated
        // monotone-or-equal publish time is the frozen Source profile sequence;
        // receiver `posted_slot` remains separately represented as a slot.
        source_sequence: publish_time,
        publish_slot: update.posted_slot,
        publish_time,
        feed_account_data_id,
    };
    output.validate().map_err(|_| ParserError::InvalidOutput)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_source_profile_v1::PRICE_UPDATE_V2_DISCRIMINATOR;

    fn config() -> PythParserConfigV1 {
        PythParserConfigV1 {
            config_account: [1; 32],
            source_spec_id: [2; 32],
            receiver_program: [3; 32],
            feed_account: [4; 32],
            pyth_feed_id: [5; 32],
            target_decimals: 8,
            confidence_multiplier: 2,
            maximum_source_age_seconds: 30,
            maximum_source_slot_lag: 150,
        }
    }

    fn feed() -> [u8; 134] {
        let mut data = [0_u8; 134];
        data[..8].copy_from_slice(&PRICE_UPDATE_V2_DISCRIMINATOR);
        data[8..40].copy_from_slice(&[9; 32]);
        data[40] = 1;
        data[41..73].copy_from_slice(&[5; 32]);
        data[73..81].copy_from_slice(&10_000_i64.to_le_bytes());
        data[81..89].copy_from_slice(&100_u64.to_le_bytes());
        data[89..93].copy_from_slice(&(-2_i32).to_le_bytes());
        data[93..101].copy_from_slice(&1_700_000_010_i64.to_le_bytes());
        data[101..109].copy_from_slice(&1_700_000_000_i64.to_le_bytes());
        data[109..117].copy_from_slice(&10_000_i64.to_le_bytes());
        data[117..125].copy_from_slice(&100_u64.to_le_bytes());
        data[125..133].copy_from_slice(&9_900_u64.to_le_bytes());
        data
    }

    #[test]
    fn exact_crossing_returns_canonical_output() {
        let output = evaluate(
            config(),
            PythParserRequestV1 {
                boundary_unix_seconds: 1_700_000_005,
            },
            ParserClockV1 {
                slot: 10_000,
                unix_timestamp: 1_700_000_020,
            },
            [4; 32],
            [3; 32],
            &feed(),
        )
        .unwrap();
        assert_eq!(output.source_sequence, 1_700_000_010);
        assert_eq!(output.publish_slot, 9_900);
        assert_eq!(output.encode().unwrap().len(), 120);
    }

    #[test]
    fn noncrossing_update_refuses() {
        assert_eq!(
            evaluate(
                config(),
                PythParserRequestV1 {
                    boundary_unix_seconds: 1_700_000_011,
                },
                ParserClockV1 {
                    slot: 10_000,
                    unix_timestamp: 1_700_000_020,
                },
                [4; 32],
                [3; 32],
                &feed(),
            ),
            Err(ParserError::NotBoundaryCrossing)
        );
    }

    #[test]
    fn future_or_stale_post_refuses() {
        assert_eq!(
            evaluate(
                config(),
                PythParserRequestV1 {
                    boundary_unix_seconds: 1_700_000_005,
                },
                ParserClockV1 {
                    slot: 9_899,
                    unix_timestamp: 1_700_000_020,
                },
                [4; 32],
                [3; 32],
                &feed(),
            ),
            Err(ParserError::StaleOrFuture)
        );
    }

    #[test]
    fn hostile_owner_and_partial_verification_refuse() {
        assert_eq!(
            evaluate(
                config(),
                PythParserRequestV1 {
                    boundary_unix_seconds: 1_700_000_005,
                },
                ParserClockV1 {
                    slot: 10_000,
                    unix_timestamp: 1_700_000_020,
                },
                [4; 32],
                [8; 32],
                &feed(),
            ),
            Err(ParserError::InvalidFeed)
        );
        let mut partial = feed();
        partial[40] = 0;
        assert_eq!(
            evaluate(
                config(),
                PythParserRequestV1 {
                    boundary_unix_seconds: 1_700_000_005,
                },
                ParserClockV1 {
                    slot: 10_000,
                    unix_timestamp: 1_700_000_020,
                },
                [4; 32],
                [3; 32],
                &partial,
            ),
            Err(ParserError::InvalidFeed)
        );
    }
}
