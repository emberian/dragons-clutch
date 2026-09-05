//! Request wire for first-use creation of a Market's Claims-role Custody replay.
//!
//! The wire is small on purpose. Every coordinate of the Custody
//! `InitializeReplay` this route forwards is DERIVED — the namespace from the
//! aggregate's persisted `custody_context`, the role from the route itself, the
//! rent from the Rent sysvar, the payer from the account that signs for it — so
//! there is nothing left for a caller to state except which Market it means.
//!
//! Carrying the Market is not redundant with the aggregate account: it is what
//! ADDRESSES the aggregate. The adapter derives the aggregate PDA from this
//! field and refuses any account that is not it, so a substituted aggregate is
//! caught by derivation rather than by trusting what the substituted account
//! says about itself.
//!
//! Forty-eight bytes also keeps the whole instruction inside a legacy packet.
//! Creating this cursor is a plain user action that stands ahead of a
//! redemption; requiring a published address-lookup table to perform it would
//! make the first redeemer of every Market do two transactions before the one
//! they came for.

/// Exact request bytes.
pub const CLAIMS_CUSTODY_REPLAY_REQUEST_BYTES_V1: usize = 48;
/// Canonical request magic.
pub const CLAIMS_CUSTODY_REPLAY_REQUEST_MAGIC_V1: [u8; 8] = *b"DCLCCR01";
/// Implemented wire version.
pub const CLAIMS_CUSTODY_REPLAY_VERSION_V1: u16 = 1;

const VERSION_OFFSET: usize = 8;
const RESERVED_OFFSET: usize = 10;
const RESERVED_BYTES: usize = 6;
const MARKET_OFFSET: usize = 16;

/// Stable hostile-decode refusal for this wire.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClaimsCustodyReplayErrorV1 {
    /// Input width was not exact.
    InvalidLength,
    /// Magic or version selected another wire family.
    InvalidHeader,
    /// Reserved bytes were nonzero.
    NonCanonical,
    /// The named Market identity was zero.
    ZeroIdentity,
}

/// Result alias for this wire.
pub type ClaimsCustodyReplayResultV1<T> = core::result::Result<T, ClaimsCustodyReplayErrorV1>;

/// One exact hostile-decodable replay-creation request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimsCustodyReplayRequestV1 {
    market: [u8; 32],
}

impl ClaimsCustodyReplayRequestV1 {
    /// Construct a request naming one logical Core Market.
    pub fn new(market: [u8; 32]) -> ClaimsCustodyReplayResultV1<Self> {
        if market.iter().all(|byte| *byte == 0) {
            return Err(ClaimsCustodyReplayErrorV1::ZeroIdentity);
        }
        Ok(Self { market })
    }

    /// The logical Core Market whose Claims-role replay is being created.
    #[must_use]
    pub const fn market(self) -> [u8; 32] {
        self.market
    }

    /// Hostile-decode an exact request.
    pub fn decode(input: &[u8]) -> ClaimsCustodyReplayResultV1<Self> {
        if input.len() != CLAIMS_CUSTODY_REPLAY_REQUEST_BYTES_V1 {
            return Err(ClaimsCustodyReplayErrorV1::InvalidLength);
        }
        if input.get(..8) != Some(CLAIMS_CUSTODY_REPLAY_REQUEST_MAGIC_V1.as_slice()) {
            return Err(ClaimsCustodyReplayErrorV1::InvalidHeader);
        }
        let version = u16::from_le_bytes([
            *input
                .get(VERSION_OFFSET)
                .ok_or(ClaimsCustodyReplayErrorV1::InvalidLength)?,
            *input
                .get(VERSION_OFFSET + 1)
                .ok_or(ClaimsCustodyReplayErrorV1::InvalidLength)?,
        ]);
        if version != CLAIMS_CUSTODY_REPLAY_VERSION_V1 {
            return Err(ClaimsCustodyReplayErrorV1::InvalidHeader);
        }
        let reserved = input
            .get(RESERVED_OFFSET..RESERVED_OFFSET + RESERVED_BYTES)
            .ok_or(ClaimsCustodyReplayErrorV1::InvalidLength)?;
        if reserved.iter().any(|byte| *byte != 0) {
            return Err(ClaimsCustodyReplayErrorV1::NonCanonical);
        }
        let market: [u8; 32] = input
            .get(MARKET_OFFSET..MARKET_OFFSET + 32)
            .ok_or(ClaimsCustodyReplayErrorV1::InvalidLength)?
            .try_into()
            .map_err(|_| ClaimsCustodyReplayErrorV1::InvalidLength)?;
        Self::new(market)
    }

    /// Encode the exact canonical request.
    pub fn to_bytes(self) -> [u8; CLAIMS_CUSTODY_REPLAY_REQUEST_BYTES_V1] {
        let mut output = [0; CLAIMS_CUSTODY_REPLAY_REQUEST_BYTES_V1];
        let (magic, rest) = output.split_at_mut(8);
        magic.copy_from_slice(&CLAIMS_CUSTODY_REPLAY_REQUEST_MAGIC_V1);
        let (version, rest) = rest.split_at_mut(2);
        version.copy_from_slice(&CLAIMS_CUSTODY_REPLAY_VERSION_V1.to_le_bytes());
        let (_reserved, market) = rest.split_at_mut(RESERVED_BYTES);
        market.copy_from_slice(&self.market);
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_canonical_request_round_trips() {
        let request = ClaimsCustodyReplayRequestV1::new([7; 32]).expect("request");
        let bytes = request.to_bytes();
        assert_eq!(bytes.len(), CLAIMS_CUSTODY_REPLAY_REQUEST_BYTES_V1);
        assert_eq!(ClaimsCustodyReplayRequestV1::decode(&bytes), Ok(request));
        assert_eq!(request.market(), [7; 32]);
    }

    #[test]
    fn a_zero_market_is_not_a_request() {
        assert_eq!(
            ClaimsCustodyReplayRequestV1::new([0; 32]),
            Err(ClaimsCustodyReplayErrorV1::ZeroIdentity)
        );
        let mut bytes = ClaimsCustodyReplayRequestV1::new([7; 32])
            .expect("request")
            .to_bytes();
        for byte in bytes.iter_mut().skip(MARKET_OFFSET) {
            *byte = 0;
        }
        assert_eq!(
            ClaimsCustodyReplayRequestV1::decode(&bytes),
            Err(ClaimsCustodyReplayErrorV1::ZeroIdentity)
        );
    }

    #[test]
    fn every_header_byte_is_load_bearing() {
        let good = ClaimsCustodyReplayRequestV1::new([7; 32])
            .expect("request")
            .to_bytes();
        assert_eq!(
            ClaimsCustodyReplayRequestV1::decode(good.get(..47).expect("short request")),
            Err(ClaimsCustodyReplayErrorV1::InvalidLength)
        );
        for (offset, expected) in [
            (0, ClaimsCustodyReplayErrorV1::InvalidHeader),
            (VERSION_OFFSET, ClaimsCustodyReplayErrorV1::InvalidHeader),
            (RESERVED_OFFSET, ClaimsCustodyReplayErrorV1::NonCanonical),
        ] {
            let mut hostile = good;
            *hostile.get_mut(offset).expect("sampled header byte") ^= 1;
            assert_eq!(
                ClaimsCustodyReplayRequestV1::decode(&hostile),
                Err(expected)
            );
        }
    }
}
