//! Dealer hot request with an explicit canonical Claims Position revision.

use super::{
    array_at, byte_at, is_zero, put, put_byte, put_u64, require_zero, u16_at, u64_at, Action,
    Error, Identity, Request, Result, Side,
};
use crate::generated_dealer_trading_profile as generated;

/// Exact canonical Trading Dealer request width.
pub const TRADING_REQUEST_BYTES: usize = generated::TRADING_REQUEST_BYTES;

/// Exact Dealer request admitted by the canonical Trading profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TradingRequest {
    /// Semantic action interpreted by the Dealer kernel.
    pub action: Action,
    /// Fill direction; canonical buy tag for non-fill actions.
    pub side: Side,
    /// Fill/unwind outcome or terminal winner.
    pub outcome: u8,
    /// Expected mutable Dealer tail revision.
    pub expected_state_revision: u64,
    /// Expected canonical Dealer Claims Position revision.
    pub expected_position_revision: u64,
    /// Current timestamp for time-sensitive actions.
    pub now: u64,
    /// Fill or unwind quantity.
    pub quantity: u64,
    /// Expected active Candidate identity.
    pub expected_candidate_id: Identity,
    /// Action-shaped actor identity.
    pub actor_id: Identity,
    /// Proposed or pending Candidate identity.
    pub replacement_candidate_id: Identity,
    /// Expected active or proposed Candidate revision.
    pub expected_candidate_revision: u64,
}

impl TradingRequest {
    /// Hostile-decode one exact action-shaped request.
    pub fn decode(input: &[u8]) -> Result<Self> {
        require_header(input)?;
        require_zero(input, generated::TRADING_REQUEST_RESERVED_OFFSET, 3)?;
        let request = Self {
            action: Action::decode(byte_at(input, generated::TRADING_REQUEST_ACTION_OFFSET)?)?,
            side: Side::decode(byte_at(input, generated::TRADING_REQUEST_SIDE_OFFSET)?)?,
            outcome: byte_at(input, generated::TRADING_REQUEST_OUTCOME_OFFSET)?,
            expected_state_revision: u64_at(
                input,
                generated::TRADING_REQUEST_EXPECTED_STATE_REVISION_OFFSET,
            )?,
            expected_position_revision: u64_at(
                input,
                generated::TRADING_REQUEST_EXPECTED_POSITION_REVISION_OFFSET,
            )?,
            now: u64_at(input, generated::TRADING_REQUEST_NOW_OFFSET)?,
            quantity: u64_at(input, generated::TRADING_REQUEST_QUANTITY_OFFSET)?,
            expected_candidate_id: array_at(
                input,
                generated::TRADING_REQUEST_EXPECTED_CANDIDATE_ID_OFFSET,
            )?,
            actor_id: array_at(input, generated::TRADING_REQUEST_ACTOR_ID_OFFSET)?,
            replacement_candidate_id: array_at(
                input,
                generated::TRADING_REQUEST_REPLACEMENT_CANDIDATE_ID_OFFSET,
            )?,
            expected_candidate_revision: u64_at(
                input,
                generated::TRADING_REQUEST_EXPECTED_CANDIDATE_REVISION_OFFSET,
            )?,
        };
        request.semantic_request()?.validate_shape()?;
        Ok(request)
    }

    /// Encode one exact action-shaped request.
    pub fn to_bytes(self) -> Result<[u8; TRADING_REQUEST_BYTES]> {
        self.semantic_request()?.validate_shape()?;
        let mut output = [0_u8; TRADING_REQUEST_BYTES];
        put(&mut output, 0, &generated::TRADING_REQUEST_MAGIC)?;
        put(
            &mut output,
            generated::TRADING_REQUEST_VERSION_OFFSET,
            &generated::ROOT_TAIL_ABI_VERSION.to_le_bytes(),
        )?;
        put_byte(
            &mut output,
            generated::TRADING_REQUEST_ACTION_OFFSET,
            self.action.tag(),
        )?;
        put_byte(
            &mut output,
            generated::TRADING_REQUEST_SIDE_OFFSET,
            self.side.tag(),
        )?;
        put_byte(
            &mut output,
            generated::TRADING_REQUEST_OUTCOME_OFFSET,
            self.outcome,
        )?;
        for (offset, value) in [
            (
                generated::TRADING_REQUEST_EXPECTED_STATE_REVISION_OFFSET,
                self.expected_state_revision,
            ),
            (
                generated::TRADING_REQUEST_EXPECTED_POSITION_REVISION_OFFSET,
                self.expected_position_revision,
            ),
            (generated::TRADING_REQUEST_NOW_OFFSET, self.now),
            (generated::TRADING_REQUEST_QUANTITY_OFFSET, self.quantity),
            (
                generated::TRADING_REQUEST_EXPECTED_CANDIDATE_REVISION_OFFSET,
                self.expected_candidate_revision,
            ),
        ] {
            put_u64(&mut output, offset, value)?;
        }
        for (offset, value) in [
            (
                generated::TRADING_REQUEST_EXPECTED_CANDIDATE_ID_OFFSET,
                self.expected_candidate_id,
            ),
            (generated::TRADING_REQUEST_ACTOR_ID_OFFSET, self.actor_id),
            (
                generated::TRADING_REQUEST_REPLACEMENT_CANDIDATE_ID_OFFSET,
                self.replacement_candidate_id,
            ),
        ] {
            put(&mut output, offset, &value)?;
        }
        Ok(output)
    }

    /// Project the existing total semantic-machine request.
    pub fn semantic_request(self) -> Result<Request> {
        if is_zero(&self.expected_candidate_id) {
            return Err(Error::ZeroCoordinate);
        }
        Ok(Request {
            action: self.action,
            side: self.side,
            outcome: self.outcome,
            expected_state_revision: self.expected_state_revision,
            now: self.now,
            quantity: self.quantity,
            expected_candidate_id: self.expected_candidate_id,
            actor_id: self.actor_id,
            replacement_candidate_id: self.replacement_candidate_id,
            expected_candidate_revision: self.expected_candidate_revision,
        })
    }
}

fn require_header(input: &[u8]) -> Result<()> {
    if input.len() != TRADING_REQUEST_BYTES {
        return Err(Error::InvalidLength);
    }
    if input.get(..generated::TRADING_REQUEST_MAGIC.len())
        != Some(&generated::TRADING_REQUEST_MAGIC)
    {
        return Err(Error::InvalidMagic);
    }
    if u16_at(input, generated::TRADING_REQUEST_VERSION_OFFSET)? != generated::ROOT_TAIL_ABI_VERSION
    {
        return Err(Error::UnsupportedVersion);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fill(position_revision: u64) -> TradingRequest {
        TradingRequest {
            action: Action::Fill,
            side: Side::TakerBuys,
            outcome: 1,
            expected_state_revision: 7,
            expected_position_revision: position_revision,
            now: 11,
            quantity: 13,
            expected_candidate_id: [1; 32],
            actor_id: [0; 32],
            replacement_candidate_id: [0; 32],
            expected_candidate_revision: 9,
        }
    }

    #[test]
    fn explicit_position_revision_round_trips() {
        let request = fill(17);
        let bytes = request.to_bytes().expect("request bytes");
        assert_eq!(bytes.len(), 152);
        assert_eq!(TradingRequest::decode(&bytes), Ok(request));
        assert_eq!(
            TradingRequest::decode(&bytes)
                .expect("decoded")
                .expected_position_revision,
            17
        );
    }

    #[test]
    fn hostile_version_padding_and_truncation_refuse() {
        let canonical = fill(0).to_bytes().expect("canonical");
        for hostile in [
            canonical.get(..canonical.len() - 1).expect("truncated"),
            canonical.get(1..).expect("shifted"),
        ] {
            assert!(TradingRequest::decode(hostile).is_err());
        }
        let mut version = canonical;
        version[generated::TRADING_REQUEST_VERSION_OFFSET] ^= 1;
        assert_eq!(
            TradingRequest::decode(&version),
            Err(Error::UnsupportedVersion)
        );
        let mut padding = canonical;
        padding[generated::TRADING_REQUEST_RESERVED_OFFSET] = 1;
        assert_eq!(
            TradingRequest::decode(&padding),
            Err(Error::NonCanonicalPadding)
        );
    }
}
