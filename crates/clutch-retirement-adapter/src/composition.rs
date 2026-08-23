// SPDX-License-Identifier: AGPL-3.0-or-later

use clutch_retirement::{
    ChildGenerationV1, EpochRetirementTailV1, MarketEpochCursorV1, PositionRetirementTailV1,
    ReservationCountTailV1, RetirementErrorV1, DIRECT_RESERVATION_ACCOUNT_VERSION_V6,
    DIRECT_RESERVATION_V2_BYTES, DIRECT_RESERVATION_V6_BYTES, EPOCH_ACCOUNT_TAG,
    EPOCH_ACCOUNT_VERSION_V5, EPOCH_V2_BYTES, EPOCH_V5_BYTES, MARKET_ACCOUNT_TAG,
    MARKET_ACCOUNT_VERSION_V2, MARKET_V1_BYTES, MARKET_V2_BYTES, POSITION_ACCOUNT_TAG,
    POSITION_ACCOUNT_VERSION_V2, POSITION_V1_BYTES, POSITION_V2_BYTES, RESERVATION_ACCOUNT_TAG,
    RESERVATION_ACCOUNT_VERSION_V5, RESERVATION_V4_BYTES, RESERVATION_V5_BYTES,
};
use clutch_solana_layout::{
    account_len, account_version,
    direct_selection_v3::DirectReservationV2Account,
    reservation::{ReservationAccount, RESERVATION_STATE_ACTIVE, RESERVATION_STATE_ENTITLED},
    EpochAccount, MarketAccount, PositionAccount,
};

use crate::{CountedChildSchemaV1, RetirementAdapterErrorV1};

fn exact(input: &[u8], expected: usize) -> Result<(), RetirementAdapterErrorV1> {
    if input.len() < expected {
        Err(RetirementErrorV1::Truncated.into())
    } else if input.len() > expected {
        Err(RetirementErrorV1::TrailingBytes.into())
    } else {
        Ok(())
    }
}

fn header(input: &[u8], tag: u8, version: u8) -> Result<(), RetirementAdapterErrorV1> {
    if input[0] != tag {
        return Err(RetirementErrorV1::WrongTag.into());
    }
    if input[1] != version {
        return Err(RetirementErrorV1::WrongVersion.into());
    }
    Ok(())
}

fn checked_base(
    input: &[u8],
    full_len: usize,
    base_len: usize,
    tag: u8,
    promoted_version: u8,
    legacy_version: u8,
    output: &mut [u8],
) -> Result<(), RetirementAdapterErrorV1> {
    exact(input, full_len)?;
    exact(output, base_len)?;
    header(input, tag, promoted_version)?;
    output.copy_from_slice(&input[..base_len]);
    output[1] = legacy_version;
    Ok(())
}

fn validate_position_count_marker(
    base_state: u8,
    count: ReservationCountTailV1,
) -> Result<(), RetirementAdapterErrorV1> {
    count.validate()?;
    let expected = matches!(
        base_state,
        RESERVATION_STATE_ACTIVE | RESERVATION_STATE_ENTITLED
    );
    if count.position_counted != expected {
        return Err(RetirementErrorV1::NonCanonicalState.into());
    }
    Ok(())
}

/// Exact Position V2 composition: authoritative Position V1 body plus tail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionAccountV2 {
    /// Base value decoded and validated by `clutch-solana-layout`.
    pub base: PositionAccount,
    /// Counted-retirement facts owned by `clutch-retirement`.
    pub retirement: PositionRetirementTailV1,
}

impl PositionAccountV2 {
    /// Encode exactly 280 bytes under Position tag/version 2.
    pub fn encode(self) -> Result<[u8; POSITION_V2_BYTES], RetirementAdapterErrorV1> {
        let mut output = [0u8; POSITION_V2_BYTES];
        let written = self.base.encode(&mut output[..POSITION_V1_BYTES])?;
        if written != POSITION_V1_BYTES {
            return Err(RetirementAdapterErrorV1::BaseLengthMismatch);
        }
        output[1] = POSITION_ACCOUNT_VERSION_V2;
        output[POSITION_V1_BYTES..].copy_from_slice(&self.retirement.encode()?);
        Ok(output)
    }

    /// Decode an exact Position V2, delegating every base-field invariant to
    /// the authoritative Position V1 decoder after restoring its header.
    pub fn decode(input: &[u8]) -> Result<Self, RetirementAdapterErrorV1> {
        let mut base_bytes = [0u8; POSITION_V1_BYTES];
        checked_base(
            input,
            POSITION_V2_BYTES,
            POSITION_V1_BYTES,
            POSITION_ACCOUNT_TAG,
            POSITION_ACCOUNT_VERSION_V2,
            account_version::POSITION,
            &mut base_bytes,
        )?;
        Ok(Self {
            base: PositionAccount::decode(&base_bytes)?,
            retirement: PositionRetirementTailV1::decode(&input[POSITION_V1_BYTES..])?,
        })
    }
}

/// Exact Market V2 composition: authoritative Market V1 body plus cursor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketAccountV2 {
    /// Base value decoded and validated by `clutch-solana-layout`.
    pub base: MarketAccount,
    /// Monotone general-Epoch cursor.
    pub cursor: MarketEpochCursorV1,
}

impl MarketAccountV2 {
    /// Encode exactly 734 bytes under Market tag/version 2.
    pub fn encode(self) -> Result<[u8; MARKET_V2_BYTES], RetirementAdapterErrorV1> {
        let mut output = [0u8; MARKET_V2_BYTES];
        let written = self.base.encode(&mut output[..MARKET_V1_BYTES])?;
        if written != MARKET_V1_BYTES {
            return Err(RetirementAdapterErrorV1::BaseLengthMismatch);
        }
        output[1] = MARKET_ACCOUNT_VERSION_V2;
        output[MARKET_V1_BYTES..].copy_from_slice(&self.cursor.encode());
        Ok(output)
    }

    /// Decode an exact Market V2 through the authoritative Market V1 decoder.
    pub fn decode(input: &[u8]) -> Result<Self, RetirementAdapterErrorV1> {
        let mut base_bytes = [0u8; MARKET_V1_BYTES];
        checked_base(
            input,
            MARKET_V2_BYTES,
            MARKET_V1_BYTES,
            MARKET_ACCOUNT_TAG,
            MARKET_ACCOUNT_VERSION_V2,
            account_version::MARKET,
            &mut base_bytes,
        )?;
        Ok(Self {
            base: MarketAccount::decode(&base_bytes)?,
            cursor: MarketEpochCursorV1::decode(&input[MARKET_V1_BYTES..])?,
        })
    }
}

/// Exact general Epoch V5 composition: authoritative Epoch V2 body plus tail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralEpochAccountV5 {
    /// Base value decoded and validated by `clutch-solana-layout`.
    pub base: EpochAccount,
    /// Counted child, generation, and rent facts.
    pub retirement: EpochRetirementTailV1,
}

impl GeneralEpochAccountV5 {
    /// Encode exactly 429 bytes under Epoch tag/version 5.
    pub fn encode(self) -> Result<[u8; EPOCH_V5_BYTES], RetirementAdapterErrorV1> {
        let mut output = [0u8; EPOCH_V5_BYTES];
        let written = self.base.encode(&mut output[..EPOCH_V2_BYTES])?;
        if written != EPOCH_V2_BYTES {
            return Err(RetirementAdapterErrorV1::BaseLengthMismatch);
        }
        output[1] = EPOCH_ACCOUNT_VERSION_V5;
        output[EPOCH_V2_BYTES..].copy_from_slice(&self.retirement.encode()?);
        Ok(output)
    }

    /// Decode an exact general Epoch V5 through the Epoch V2 semantic owner.
    pub fn decode(input: &[u8]) -> Result<Self, RetirementAdapterErrorV1> {
        let mut base_bytes = [0u8; EPOCH_V2_BYTES];
        checked_base(
            input,
            EPOCH_V5_BYTES,
            EPOCH_V2_BYTES,
            EPOCH_ACCOUNT_TAG,
            EPOCH_ACCOUNT_VERSION_V5,
            account_version::EPOCH,
            &mut base_bytes,
        )?;
        Ok(Self {
            base: EpochAccount::decode(&base_bytes)?,
            retirement: EpochRetirementTailV1::decode(&input[EPOCH_V2_BYTES..])?,
        })
    }
}

/// Exact general Reservation V5 composition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralReservationAccountV5 {
    /// General Reservation V4 decoded by its authoritative owner.
    pub base: ReservationAccount,
    /// Parent generation and exact-once Position count marker.
    pub count: ReservationCountTailV1,
}

impl GeneralReservationAccountV5 {
    /// Encode exactly 627 bytes under shared Reservation tag/version 5.
    pub fn encode(self) -> Result<[u8; RESERVATION_V5_BYTES], RetirementAdapterErrorV1> {
        validate_position_count_marker(self.base.state, self.count)?;
        let mut output = [0u8; RESERVATION_V5_BYTES];
        let written = self.base.encode(&mut output[..RESERVATION_V4_BYTES])?;
        if written != RESERVATION_V4_BYTES {
            return Err(RetirementAdapterErrorV1::BaseLengthMismatch);
        }
        output[1] = RESERVATION_ACCOUNT_VERSION_V5;
        output[RESERVATION_V4_BYTES..].copy_from_slice(&self.count.encode()?);
        Ok(output)
    }

    /// Decode an exact general Reservation V5 through the V4 semantic owner.
    pub fn decode(input: &[u8]) -> Result<Self, RetirementAdapterErrorV1> {
        let mut base_bytes = [0u8; RESERVATION_V4_BYTES];
        checked_base(
            input,
            RESERVATION_V5_BYTES,
            RESERVATION_V4_BYTES,
            RESERVATION_ACCOUNT_TAG,
            RESERVATION_ACCOUNT_VERSION_V5,
            clutch_solana_layout::reservation::RESERVATION_ACCOUNT_VERSION,
            &mut base_bytes,
        )?;
        let base = ReservationAccount::decode(&base_bytes)?;
        let count = ReservationCountTailV1::decode(&input[RESERVATION_V4_BYTES..])?;
        validate_position_count_marker(base.state, count)?;
        Ok(Self { base, count })
    }
}

/// Exact direct Reservation V6 composition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectReservationAccountV6 {
    /// Direct Reservation V2 decoded by its authoritative owner.
    pub base: DirectReservationV2Account,
    /// Parent generation and exact-once Position count marker.
    pub count: ReservationCountTailV1,
}

impl DirectReservationAccountV6 {
    /// Encode exactly 627 bytes under shared Reservation tag/version 6.
    pub fn encode(
        self,
        neutral_sink: clutch_solana_layout::Hash32,
    ) -> Result<[u8; DIRECT_RESERVATION_V6_BYTES], RetirementAdapterErrorV1> {
        validate_position_count_marker(self.base.reservation.state, self.count)?;
        let mut output = [0u8; DIRECT_RESERVATION_V6_BYTES];
        let written = self
            .base
            .encode(neutral_sink, &mut output[..DIRECT_RESERVATION_V2_BYTES])?;
        if written != DIRECT_RESERVATION_V2_BYTES {
            return Err(RetirementAdapterErrorV1::BaseLengthMismatch);
        }
        output[1] = DIRECT_RESERVATION_ACCOUNT_VERSION_V6;
        output[DIRECT_RESERVATION_V2_BYTES..].copy_from_slice(&self.count.encode()?);
        Ok(output)
    }

    /// Decode an exact direct Reservation V6 through the V2 semantic owner.
    pub fn decode(
        input: &[u8],
        neutral_sink: clutch_solana_layout::Hash32,
    ) -> Result<Self, RetirementAdapterErrorV1> {
        let mut base_bytes = [0u8; DIRECT_RESERVATION_V2_BYTES];
        checked_base(
            input,
            DIRECT_RESERVATION_V6_BYTES,
            DIRECT_RESERVATION_V2_BYTES,
            RESERVATION_ACCOUNT_TAG,
            DIRECT_RESERVATION_ACCOUNT_VERSION_V6,
            clutch_solana_layout::direct_selection_v3::DIRECT_RESERVATION_V2_VERSION,
            &mut base_bytes,
        )?;
        let base = DirectReservationV2Account::decode(&base_bytes, neutral_sink)?;
        let count = ReservationCountTailV1::decode(&input[DIRECT_RESERVATION_V2_BYTES..])?;
        validate_position_count_marker(base.reservation.state, count)?;
        Ok(Self { base, count })
    }
}

/// Promote one already-authoritatively-decoded child base and append its
/// parent-generation tail into an exact caller-provided output buffer.
pub fn encode_counted_child_after_base_validation(
    schema: CountedChildSchemaV1,
    legacy_base: &[u8],
    generation: ChildGenerationV1,
    output: &mut [u8],
) -> Result<(), RetirementAdapterErrorV1> {
    exact(legacy_base, schema.legacy_len())?;
    exact(output, schema.counted_len())?;
    header(legacy_base, schema.tag(), schema.legacy_version())?;
    output[..schema.legacy_len()].copy_from_slice(legacy_base);
    output[1] = schema.counted_version();
    output[schema.legacy_len()..].copy_from_slice(&generation.encode()?);
    Ok(())
}

/// Split one exact counted child into legacy-version base bytes for its
/// authoritative decoder and a validated parent-generation tail.
pub fn decode_counted_child(
    schema: CountedChildSchemaV1,
    input: &[u8],
    legacy_base_output: &mut [u8],
) -> Result<ChildGenerationV1, RetirementAdapterErrorV1> {
    checked_base(
        input,
        schema.counted_len(),
        schema.legacy_len(),
        schema.tag(),
        schema.counted_version(),
        schema.legacy_version(),
        legacy_base_output,
    )?;
    Ok(ChildGenerationV1::decode(&input[schema.legacy_len()..])?)
}

const _: () = assert!(POSITION_V1_BYTES == account_len::POSITION);
const _: () = assert!(MARKET_V1_BYTES == account_len::MARKET);
const _: () = assert!(EPOCH_V2_BYTES == account_len::EPOCH);
const _: () =
    assert!(RESERVATION_V4_BYTES == clutch_solana_layout::reservation::RESERVATION_ACCOUNT_BYTES);
const _: () = assert!(
    DIRECT_RESERVATION_V2_BYTES
        == clutch_solana_layout::direct_selection_v3::DIRECT_RESERVATION_V2_BYTES
);
const _: () = assert!(
    EPOCH_ACCOUNT_VERSION_V5 != clutch_solana_layout::direct_selection::DIRECT_EPOCH_VERSION
);
const _: () = assert!(
    EPOCH_ACCOUNT_VERSION_V5 != clutch_solana_layout::direct_selection_v3::DIRECT_EPOCH_V4_VERSION
);
const _: () = assert!(
    RESERVATION_ACCOUNT_VERSION_V5
        != clutch_solana_layout::reservation::RESERVATION_ACCOUNT_VERSION
);
const _: () = assert!(
    RESERVATION_ACCOUNT_VERSION_V5
        != clutch_solana_layout::direct_selection_v3::DIRECT_RESERVATION_V2_VERSION
);
const _: () = assert!(
    DIRECT_RESERVATION_ACCOUNT_VERSION_V6
        != clutch_solana_layout::reservation::RESERVATION_ACCOUNT_VERSION
);
const _: () = assert!(
    DIRECT_RESERVATION_ACCOUNT_VERSION_V6
        != clutch_solana_layout::direct_selection_v3::DIRECT_RESERVATION_V2_VERSION
);
const _: () = assert!(DIRECT_RESERVATION_ACCOUNT_VERSION_V6 != RESERVATION_ACCOUNT_VERSION_V5);
