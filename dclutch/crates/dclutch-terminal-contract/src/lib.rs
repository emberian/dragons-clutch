#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Provider-neutral same-PDA terminal categorical Market representation.
//!
//! The active categorical Market may compact to this exact 312-byte record
//! only after its root is retired, every direct child is gone, and its Hoard
//! and complete native supply vector are empty. The compaction constructor
//! checks those conditions against the semantic owner before discarding the
//! economic fields. The terminal record retains only the universal root and
//! the same canonical 64-byte categorical settlement summary used by the
//! active Market. It stores no second Market identity, Product identity,
//! generation, outcome count, provider policy, or evidence body.
//!
//! Active profile V1 is `320 + 8N` bytes, so same-PDA compaction reclaims
//! exactly `8 + 8N` bytes: 24 bytes for `N=2` through 136 bytes for `N=16`.
//! The SVM adapter must use its authenticated Rent sysvar to compute
//! `minimum_balance(320 + 8N) - minimum_balance(312)` with checked arithmetic
//! and transfer exactly that lamport delta to [`MarketRoot::rent_refund`] in
//! the same atomic instruction. This crate contains no Rent or account-memory
//! policy. The layouts are distinct; bytewise reinterpretation is invalid.

use core::convert::TryInto;

use dclutch_core_contract::{MARKET_ROOT_BYTES, MarketRoot, Phase};
use dclutch_market_contract::{
    Error as MarketError,
    market::{
        CategoricalMarketV1, CategoricalSettlementSummaryV1, MARKET_BASE_BYTES, MAX_MARKET_BYTES,
        PROVISIONAL_CATEGORICAL_PROFILE_V1, SETTLEMENT_SUMMARY_BYTES,
    },
};

/// Canonical terminal categorical Market magic.
pub const TERMINAL_CATEGORICAL_MARKET_MAGIC: [u8; 8] = *b"DCLTCTM1";
/// Implemented terminal categorical Market schema.
pub const TERMINAL_CATEGORICAL_MARKET_SCHEMA_VERSION: u16 = 1;
/// Exact byte width of every provisional-profile terminal categorical Market.
pub const TERMINAL_CATEGORICAL_MARKET_BYTES: usize =
    16 + MARKET_ROOT_BYTES + SETTLEMENT_SUMMARY_BYTES;
/// Byte offset of the exact categorical outcome count.
pub const TERMINAL_MARKET_OUTCOME_COUNT_OFFSET: usize = 10;
/// Byte offset of the categorical profile discriminator.
pub const TERMINAL_MARKET_PROFILE_OFFSET: usize = 11;
/// Byte offset of the retained universal Market root.
pub const TERMINAL_MARKET_ROOT_OFFSET: usize = 16;
/// Byte offset of the retained canonical settlement summary.
pub const TERMINAL_MARKET_SETTLEMENT_OFFSET: usize =
    TERMINAL_MARKET_ROOT_OFFSET + MARKET_ROOT_BYTES;

const HEADER_RESERVED_OFFSET: usize = 12;
const HEADER_RESERVED_BYTES: usize = 4;

/// Explicit refusal returned by the terminal categorical contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Input did not have the one exact canonical width.
    InvalidLength,
    /// Output did not have the one exact canonical width.
    OutputLength,
    /// Magic bytes did not name this contract.
    InvalidMagic,
    /// The schema version is not implemented.
    UnsupportedSchema,
    /// The categorical profile discriminator is not implemented.
    UnsupportedProfile,
    /// The exact categorical width is outside the selected profile.
    InvalidOutcomeCount,
    /// Reserved header bytes were not zero.
    NonCanonicalReservedBytes,
    /// The retained Market root was invalid.
    InvalidMarketRoot {
        /// Exact root-contract refusal.
        error: dclutch_core_contract::Error,
    },
    /// The retained categorical settlement summary was invalid.
    InvalidSettlementSummary {
        /// Exact Market-contract refusal.
        error: MarketError,
    },
    /// The active Market had not reached its final retired phase.
    MarketNotRetired,
    /// Hoard atoms or aggregate native supplies remained at compaction.
    NonemptyEconomicState,
    /// Checked exact integer layout arithmetic overflowed.
    ArithmeticOverflow,
}

/// Result alias for this contract.
pub type Result<T> = core::result::Result<T, Error>;

/// Exact persistent state after active categorical economics are reclaimed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalCategoricalMarketV1<const N: usize> {
    root: MarketRoot,
    settlement: CategoricalSettlementSummaryV1,
}

impl<const N: usize> TerminalCategoricalMarketV1<N> {
    /// Compact one validated, economically empty active Market.
    ///
    /// This is the sole public construction boundary from live state. The SVM
    /// adapter must perform same-PDA reallocation and the exact Rent refund in
    /// the same instruction after this pure contract accepts the projection.
    pub fn from_reclaimed_active(active: &CategoricalMarketV1<N>) -> Result<Self> {
        if active.root().phase() != Phase::Retired || active.root().outstanding_children() != 0 {
            return Err(Error::MarketNotRetired);
        }
        if active.hoard_atoms() != 0 || active.supply().iter().any(|amount| *amount != 0) {
            return Err(Error::NonemptyEconomicState);
        }
        Self::from_terminal_parts(active.root(), active.settlement())
    }

    /// Return the checked exact terminal width of 312 bytes.
    pub fn encoded_len() -> Result<usize> {
        validate_outcome_count::<N>()?;
        Ok(TERMINAL_CATEGORICAL_MARKET_BYTES)
    }

    /// Return the exact active categorical outcome count retained for dispatch.
    pub fn outcome_count() -> Result<u8> {
        validate_outcome_count::<N>()
    }

    /// Hostile-decode one exact terminal categorical Market.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let stored_outcome_count = decode_terminal_outcome_count(bytes)?;
        if stored_outcome_count != validate_outcome_count::<N>()? {
            return Err(Error::InvalidOutcomeCount);
        }
        let root_end = TERMINAL_MARKET_ROOT_OFFSET
            .checked_add(MARKET_ROOT_BYTES)
            .ok_or(Error::ArithmeticOverflow)?;
        let root = MarketRoot::decode(
            bytes
                .get(TERMINAL_MARKET_ROOT_OFFSET..root_end)
                .ok_or(Error::InvalidLength)?,
        )
        .map_err(|error| Error::InvalidMarketRoot { error })?;
        let settlement_end = TERMINAL_MARKET_SETTLEMENT_OFFSET
            .checked_add(SETTLEMENT_SUMMARY_BYTES)
            .ok_or(Error::ArithmeticOverflow)?;
        let settlement = CategoricalSettlementSummaryV1::decode::<N>(
            bytes
                .get(TERMINAL_MARKET_SETTLEMENT_OFFSET..settlement_end)
                .ok_or(Error::InvalidLength)?,
        )
        .map_err(|error| Error::InvalidSettlementSummary { error })?;
        Self::from_terminal_parts(root, settlement)
    }

    /// Return exact canonical terminal bytes.
    pub fn to_bytes(self) -> Result<[u8; TERMINAL_CATEGORICAL_MARKET_BYTES]> {
        let terminal = Self::from_terminal_parts(self.root, self.settlement)?;
        let outcome_count = validate_outcome_count::<N>()?;
        let settlement = terminal
            .settlement
            .to_bytes::<N>()
            .map_err(|error| Error::InvalidSettlementSummary { error })?;
        let mut output = [0u8; TERMINAL_CATEGORICAL_MARKET_BYTES];
        put(&mut output, 0, &TERMINAL_CATEGORICAL_MARKET_MAGIC);
        put(
            &mut output,
            8,
            &TERMINAL_CATEGORICAL_MARKET_SCHEMA_VERSION.to_le_bytes(),
        );
        put(
            &mut output,
            TERMINAL_MARKET_OUTCOME_COUNT_OFFSET,
            &[outcome_count],
        );
        put(
            &mut output,
            TERMINAL_MARKET_PROFILE_OFFSET,
            &[PROVISIONAL_CATEGORICAL_PROFILE_V1],
        );
        put(
            &mut output,
            TERMINAL_MARKET_ROOT_OFFSET,
            &terminal.root.to_bytes(),
        );
        put(&mut output, TERMINAL_MARKET_SETTLEMENT_OFFSET, &settlement);
        Ok(output)
    }

    /// Encode atomically into an exact caller-owned output buffer.
    pub fn encode(&self, output: &mut [u8]) -> Result<()> {
        if output.len() != TERMINAL_CATEGORICAL_MARKET_BYTES {
            return Err(Error::OutputLength);
        }
        let canonical = self.to_bytes()?;
        output.copy_from_slice(&canonical);
        Ok(())
    }

    /// Return the retained universal root.
    pub const fn root(self) -> MarketRoot {
        self.root
    }

    /// Return the retained canonical categorical settlement summary.
    pub const fn settlement(self) -> CategoricalSettlementSummaryV1 {
        self.settlement
    }

    fn from_terminal_parts(
        root: MarketRoot,
        settlement: CategoricalSettlementSummaryV1,
    ) -> Result<Self> {
        validate_outcome_count::<N>()?;
        root.validate()
            .map_err(|error| Error::InvalidMarketRoot { error })?;
        if root.phase() != Phase::Retired || root.outstanding_children() != 0 {
            return Err(Error::MarketNotRetired);
        }
        settlement
            .to_bytes::<N>()
            .map_err(|error| Error::InvalidSettlementSummary { error })?;
        Ok(Self { root, settlement })
    }
}

/// Hostile-decode the exact categorical width from a complete terminal header.
///
/// This validates the exact 312-byte envelope, magic, schema, provisional
/// profile, reserved bytes, and `2..=16` width. Callers must then dispatch to
/// [`TerminalCategoricalMarketV1::decode`] for root and summary validation.
pub fn decode_terminal_outcome_count(bytes: &[u8]) -> Result<u8> {
    if bytes.len() != TERMINAL_CATEGORICAL_MARKET_BYTES {
        return Err(Error::InvalidLength);
    }
    if read_array::<8>(bytes, 0)? != TERMINAL_CATEGORICAL_MARKET_MAGIC {
        return Err(Error::InvalidMagic);
    }
    if u16::from_le_bytes(read_array(bytes, 8)?) != TERMINAL_CATEGORICAL_MARKET_SCHEMA_VERSION {
        return Err(Error::UnsupportedSchema);
    }
    if read_byte(bytes, TERMINAL_MARKET_PROFILE_OFFSET)? != PROVISIONAL_CATEGORICAL_PROFILE_V1 {
        return Err(Error::UnsupportedProfile);
    }
    require_zero(bytes, HEADER_RESERVED_OFFSET, HEADER_RESERVED_BYTES)?;
    let outcome_count = read_byte(bytes, TERMINAL_MARKET_OUTCOME_COUNT_OFFSET)?;
    if !(2..=16).contains(&outcome_count) {
        return Err(Error::InvalidOutcomeCount);
    }
    Ok(outcome_count)
}

/// Return the exact active-to-terminal byte reduction for width `N`.
pub fn reclaimed_bytes<const N: usize>() -> Result<usize> {
    validate_outcome_count::<N>()?;
    let active = MARKET_BASE_BYTES
        .checked_add(N.checked_mul(8).ok_or(Error::ArithmeticOverflow)?)
        .ok_or(Error::ArithmeticOverflow)?;
    active
        .checked_sub(TERMINAL_CATEGORICAL_MARKET_BYTES)
        .ok_or(Error::ArithmeticOverflow)
}

/// Return the maximum provisional-profile active-to-terminal byte reduction.
pub const MAX_RECLAIMED_BYTES: usize = MAX_MARKET_BYTES - TERMINAL_CATEGORICAL_MARKET_BYTES;

fn validate_outcome_count<const N: usize>() -> Result<u8> {
    if !(2..=16).contains(&N) {
        return Err(Error::InvalidOutcomeCount);
    }
    u8::try_from(N).map_err(|_| Error::InvalidOutcomeCount)
}

fn read_byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}

fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = offset.checked_add(N).ok_or(Error::InvalidLength)?;
    bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn require_zero(bytes: &[u8], offset: usize, width: usize) -> Result<()> {
    let end = offset.checked_add(width).ok_or(Error::InvalidLength)?;
    if bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(Error::NonCanonicalReservedBytes);
    }
    Ok(())
}

fn put(output: &mut [u8], offset: usize, input: &[u8]) {
    for (destination, source) in output.iter_mut().skip(offset).zip(input) {
        *destination = *source;
    }
}

#[cfg(test)]
mod tests {
    use dclutch_core_contract::{ContentId as CoreContentId, MarketIdentity};
    use dclutch_market_contract::market::{BINARY_MARKET_BYTES, CategoricalSettlementSummaryV1};

    use super::*;

    const GENERATION: u64 = 9;

    fn core_id(value: u8) -> CoreContentId {
        CoreContentId::new([value; 32]).expect("nonzero core ID")
    }

    fn identity() -> MarketIdentity {
        MarketIdentity::new(
            core_id(1),
            core_id(2),
            core_id(3),
            core_id(4),
            core_id(5),
            GENERATION,
        )
    }

    fn founding_market<const N: usize>() -> CategoricalMarketV1<N> {
        CategoricalMarketV1::new(
            MarketRoot::founding(identity(), [6; 32]).expect("founding root"),
            0,
            [0; N],
            CategoricalSettlementSummaryV1::empty(),
        )
        .expect("founding market")
    }

    fn canceled_retired_market<const N: usize>() -> CategoricalMarketV1<N> {
        let mut market = founding_market::<N>();
        market
            .transition_phase(GENERATION, Phase::Retiring)
            .expect("retiring");
        market
            .transition_phase(GENERATION, Phase::Retired)
            .expect("retired");
        market
    }

    fn settled_retired_market<const N: usize>(winner: usize) -> CategoricalMarketV1<N> {
        let mut market = founding_market::<N>();
        market
            .transition_phase(GENERATION, Phase::Open)
            .expect("open");
        market.split_complete_set(10).expect("split");
        let summary = resolved_summary::<N>(winner);
        market
            .resolve_with_summary(GENERATION, summary)
            .expect("resolve");
        let mut outcome = 0usize;
        while outcome < N {
            market.redeem_outcome(outcome, 10).expect("redeem");
            outcome = outcome.checked_add(1).expect("bounded outcome");
        }
        market
            .transition_phase(GENERATION, Phase::Retiring)
            .expect("retiring");
        market
            .transition_phase(GENERATION, Phase::Retired)
            .expect("retired");
        market
    }

    fn resolved_summary<const N: usize>(winner: usize) -> CategoricalSettlementSummaryV1 {
        let winner = u8::try_from(winner).expect("bounded winner");
        let mut bytes = [0u8; SETTLEMENT_SUMMARY_BYTES];
        *bytes.get_mut(0).expect("status") = 1;
        *bytes.get_mut(1).expect("occurrence route") = 0;
        *bytes.get_mut(2).expect("winner") = winner;
        bytes
            .get_mut(8..16)
            .expect("terminal sequence")
            .copy_from_slice(&1u64.to_le_bytes());
        bytes.get_mut(16..48).expect("evidence").fill(7);
        CategoricalSettlementSummaryV1::decode::<N>(&bytes).expect("summary")
    }

    #[test]
    fn canceled_and_settled_terminal_records_round_trip_at_exact_width() {
        let canceled_active = canceled_retired_market::<2>();
        let canceled = TerminalCategoricalMarketV1::from_reclaimed_active(&canceled_active)
            .expect("canceled terminal");
        let canceled_bytes = canceled.to_bytes().expect("encode canceled");
        assert_eq!(TERMINAL_CATEGORICAL_MARKET_BYTES, 312);
        assert_eq!(canceled_bytes.len(), 312);
        assert_eq!(
            canceled_bytes.get(0..8),
            Some(&TERMINAL_CATEGORICAL_MARKET_MAGIC[..])
        );
        assert_eq!(decode_terminal_outcome_count(&canceled_bytes), Ok(2));
        assert_eq!(
            TerminalCategoricalMarketV1::<2>::decode(&canceled_bytes),
            Ok(canceled)
        );
        assert!(canceled.settlement().is_empty());

        let settled_active = settled_retired_market::<16>(15);
        let settled = TerminalCategoricalMarketV1::from_reclaimed_active(&settled_active)
            .expect("settled terminal");
        let settled_bytes = settled.to_bytes().expect("encode settled");
        assert_eq!(
            TerminalCategoricalMarketV1::<16>::decode(&settled_bytes),
            Ok(settled)
        );
        let resolution = settled.settlement().resolution().expect("resolution");
        assert_eq!(resolution.winner(), 15);
        assert_eq!(resolution.terminal_sequence(), 1);
        assert_eq!(settled.root().rent_refund(), [6; 32]);
    }

    #[test]
    fn exact_shrink_profile_is_explicit_and_liftable() {
        assert_eq!(BINARY_MARKET_BYTES, 336);
        assert_eq!(MAX_MARKET_BYTES, 448);
        assert_eq!(reclaimed_bytes::<2>(), Ok(24));
        assert_eq!(reclaimed_bytes::<16>(), Ok(136));
        assert_eq!(MAX_RECLAIMED_BYTES, 136);
        assert_eq!(reclaimed_bytes::<1>(), Err(Error::InvalidOutcomeCount));
        assert_eq!(reclaimed_bytes::<17>(), Err(Error::InvalidOutcomeCount));
    }

    #[test]
    fn compaction_refuses_live_phase() {
        let founding = founding_market::<2>();
        assert_eq!(
            TerminalCategoricalMarketV1::from_reclaimed_active(&founding),
            Err(Error::MarketNotRetired)
        );

        let mut resolved = founding_market::<2>();
        resolved
            .transition_phase(GENERATION, Phase::Open)
            .expect("open");
        resolved.split_complete_set(3).expect("split");
        resolved
            .resolve_with_summary(GENERATION, resolved_summary::<2>(0))
            .expect("resolved");
        assert_eq!(
            TerminalCategoricalMarketV1::from_reclaimed_active(&resolved),
            Err(Error::MarketNotRetired)
        );
    }

    #[test]
    fn hostile_envelope_profile_width_root_and_summary_refuse() {
        let terminal =
            TerminalCategoricalMarketV1::from_reclaimed_active(&settled_retired_market::<3>(1))
                .expect("terminal");
        let canonical = terminal.to_bytes().expect("canonical");
        for length in 0..TERMINAL_CATEGORICAL_MARKET_BYTES {
            assert_eq!(
                TerminalCategoricalMarketV1::<3>::decode(canonical.get(..length).expect("prefix"),),
                Err(Error::InvalidLength)
            );
        }
        for (offset, expected) in [
            (0, Error::InvalidMagic),
            (8, Error::UnsupportedSchema),
            (TERMINAL_MARKET_PROFILE_OFFSET, Error::UnsupportedProfile),
            (HEADER_RESERVED_OFFSET, Error::NonCanonicalReservedBytes),
        ] {
            let mut changed = canonical;
            *changed.get_mut(offset).expect("field") ^= 1;
            assert_eq!(
                TerminalCategoricalMarketV1::<3>::decode(&changed),
                Err(expected)
            );
        }

        let mut invalid_width = canonical;
        *invalid_width
            .get_mut(TERMINAL_MARKET_OUTCOME_COUNT_OFFSET)
            .expect("outcome count") = 17;
        assert_eq!(
            TerminalCategoricalMarketV1::<3>::decode(&invalid_width),
            Err(Error::InvalidOutcomeCount)
        );
        assert_eq!(
            TerminalCategoricalMarketV1::<2>::decode(&canonical),
            Err(Error::InvalidOutcomeCount)
        );

        let mut live_root = canonical;
        *live_root
            .get_mut(TERMINAL_MARKET_ROOT_OFFSET + 184)
            .expect("root phase") = 1;
        assert_eq!(
            TerminalCategoricalMarketV1::<3>::decode(&live_root),
            Err(Error::MarketNotRetired)
        );

        let mut invalid_summary = canonical;
        *invalid_summary
            .get_mut(TERMINAL_MARKET_SETTLEMENT_OFFSET + 3)
            .expect("summary reserved") = 1;
        assert_eq!(
            TerminalCategoricalMarketV1::<3>::decode(&invalid_summary),
            Err(Error::InvalidSettlementSummary {
                error: MarketError::NonCanonicalReservedBytes,
            })
        );
    }

    #[test]
    fn output_length_refusal_is_atomic() {
        let terminal =
            TerminalCategoricalMarketV1::from_reclaimed_active(&canceled_retired_market::<2>())
                .expect("terminal");
        let mut output = [0xa5; TERMINAL_CATEGORICAL_MARKET_BYTES - 1];
        assert_eq!(terminal.encode(&mut output), Err(Error::OutputLength));
        assert!(output.iter().all(|byte| *byte == 0xa5));
    }
}
