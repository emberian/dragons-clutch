//! Wire requests for the claim-check routes.
//!
//! Three of the four routes are described here; compaction's own request is
//! written against the terminal-settlement header it must reproduce and lands
//! with that route.
//!
//! The shape worth noticing is how little these carry. **No request names a
//! holder.** A claim-check's address is derived from `(aggregate, owner)` under
//! its own domain, exactly as the position it replaces is, so a caller naming
//! the wrong holder derives an address that is not the account they passed and
//! the route refuses before touching anything. A holder field on the wire would
//! be a second author for a fact the address already proves, and second authors
//! for one fact are how a route that pays the wrong person gets built.
//!
//! Redemption and escrow close share one magic and separate under an explicit
//! action tag at different exact widths, following the lifecycle-rent contract
//! rather than padding one width with a zeroed identity. That is deliberate:
//! this tree's rule is that no all-zero public key is an absence sentinel, so a
//! route with nothing to say about an identity must not have a field for it.

use core::convert::TryInto;

use crate::claim_check_v1::{
    CLAIM_CHECK_COMPACT_MAGIC_V1, CLAIM_CHECK_OPEN_MAGIC_V1, CLAIM_CHECK_REDEEM_MAGIC_V1,
    CLAIM_CHECK_WIRE_VERSION_V1, ClaimCheckErrorV1, ClaimCheckEscrowSeedsV1, ClaimCheckResultV1,
    ClaimCheckSeedsV1,
};

/// Exact width of an open-escrow request.
pub const OPEN_CLAIM_CHECK_ESCROW_BYTES_V1: usize = 128;
/// Exact width of a claim-check redemption request.
pub const REDEEM_CLAIM_CHECK_BYTES_V1: usize = 96;
/// Exact width of an escrow-close request.
pub const CLOSE_CLAIM_CHECK_ESCROW_BYTES_V1: usize = 64;

const VERSION_OFFSET: usize = 8;
const ACTION_OFFSET: usize = 10;
const RESERVED_HEADER_OFFSET: usize = 11;
const RESERVED_HEADER_BYTES: usize = 5;

const OPEN_RELEASE_SET_OFFSET: usize = 16;
const OPEN_MARKET_OFFSET: usize = 48;
const OPEN_REALM_OFFSET: usize = 80;
const OPEN_GENERATION_OFFSET: usize = 112;
const OPEN_RESERVED_BODY_OFFSET: usize = 120;
const OPEN_RESERVED_BODY_BYTES: usize = 8;

const REDEEM_AGGREGATE_OFFSET: usize = 16;
const REDEEM_OWNER_OFFSET: usize = 48;
const REDEEM_RESERVED_BODY_OFFSET: usize = 80;
const REDEEM_RESERVED_BODY_BYTES: usize = 16;

const CLOSE_AGGREGATE_OFFSET: usize = 16;
const CLOSE_RESERVED_BODY_OFFSET: usize = 48;
const CLOSE_RESERVED_BODY_BYTES: usize = 16;

/// Claim-check lifecycle action.
///
/// The discriminants are distinct across magics rather than restarting per
/// family, so a tag read out of a transaction identifies its route without also
/// needing the magic it arrived under.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ClaimCheckActionV1 {
    /// Create the per-market escrow and start the compaction clock.
    OpenEscrow = 1,
    /// Compact one sleeping position into a claim-check.
    Compact = 2,
    /// Redeem one claim-check, holder-signed, forever.
    Redeem = 3,
    /// Close a fully redeemed escrow and sweep it to the caller.
    CloseEscrow = 4,
}

impl ClaimCheckActionV1 {
    /// Return the wire magic this action always arrives under.
    #[must_use]
    pub const fn magic(self) -> [u8; 8] {
        match self {
            Self::OpenEscrow => CLAIM_CHECK_OPEN_MAGIC_V1,
            Self::Compact => CLAIM_CHECK_COMPACT_MAGIC_V1,
            Self::Redeem | Self::CloseEscrow => CLAIM_CHECK_REDEEM_MAGIC_V1,
        }
    }

    fn decode(value: u8) -> ClaimCheckResultV1<Self> {
        match value {
            1 => Ok(Self::OpenEscrow),
            2 => Ok(Self::Compact),
            3 => Ok(Self::Redeem),
            4 => Ok(Self::CloseEscrow),
            _ => Err(ClaimCheckErrorV1::UnknownTag),
        }
    }
}

/// Permissionless request to open one market's claim-check escrow.
///
/// This is the act that establishes the compaction deadline's origin, and it
/// can only ever be generous: the route refuses any phase before terminal, so
/// the earliest origin is the market going terminal, and a later one simply
/// lengthens every holder's grace period. Being permissionless, no actor can
/// withhold the start from anyone else.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OpenClaimCheckEscrowRequestV1 {
    /// Immutable selected execution release set.
    pub release_set: [u8; 32],
    /// Logical Core Market identity.
    pub market: [u8; 32],
    /// Collateral Realm identity, naming the mint and its token program.
    pub realm: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
}

impl OpenClaimCheckEscrowRequestV1 {
    /// Construct and canonicalize one open request.
    pub fn new(self) -> ClaimCheckResultV1<Self> {
        self.validate()?;
        Ok(self)
    }

    /// Hostile-decode one exact open request.
    pub fn decode(input: &[u8]) -> ClaimCheckResultV1<Self> {
        require_header(
            input,
            OPEN_CLAIM_CHECK_ESCROW_BYTES_V1,
            &CLAIM_CHECK_OPEN_MAGIC_V1,
            ClaimCheckActionV1::OpenEscrow,
        )?;
        require_zero(input, OPEN_RESERVED_BODY_OFFSET, OPEN_RESERVED_BODY_BYTES)?;
        Self {
            release_set: read_array(input, OPEN_RELEASE_SET_OFFSET)?,
            market: read_array(input, OPEN_MARKET_OFFSET)?,
            realm: read_array(input, OPEN_REALM_OFFSET)?,
            generation: read_u64(input, OPEN_GENERATION_OFFSET)?,
        }
        .new()
    }

    /// Encode one exact canonical open request.
    pub fn to_bytes(self) -> ClaimCheckResultV1<[u8; OPEN_CLAIM_CHECK_ESCROW_BYTES_V1]> {
        self.validate()?;
        let mut output = [0; OPEN_CLAIM_CHECK_ESCROW_BYTES_V1];
        write_header(&mut output, ClaimCheckActionV1::OpenEscrow)?;
        for (offset, value) in [
            (OPEN_RELEASE_SET_OFFSET, self.release_set),
            (OPEN_MARKET_OFFSET, self.market),
            (OPEN_REALM_OFFSET, self.realm),
        ] {
            write(&mut output, offset, &value)?;
        }
        write(
            &mut output,
            OPEN_GENERATION_OFFSET,
            &self.generation.to_le_bytes(),
        )?;
        Ok(output)
    }

    fn validate(self) -> ClaimCheckResultV1<()> {
        require_distinct(&[self.release_set, self.market, self.realm])?;
        if self.generation == 0 {
            return Err(ClaimCheckErrorV1::InvalidEntitlement);
        }
        Ok(())
    }
}

/// Holder-signed request to redeem one claim-check.
///
/// The coordinates must derive the record account the frame passes. They are
/// not how the holder is identified -- the signature is -- they are how a
/// caller's intent is bound to the account they supplied, so a mismatched pair
/// refuses rather than acting on whichever record happened to be in the frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RedeemClaimCheckRequestV1 {
    /// Claims aggregate the claim-check was minted against.
    pub aggregate: [u8; 32],
    /// The sole entitled holder, who must also be the transaction's signer.
    pub owner: [u8; 32],
}

impl RedeemClaimCheckRequestV1 {
    /// Construct and canonicalize one redemption request.
    pub fn new(self) -> ClaimCheckResultV1<Self> {
        self.seeds()?;
        Ok(self)
    }

    /// Hostile-decode one exact redemption request.
    pub fn decode(input: &[u8]) -> ClaimCheckResultV1<Self> {
        require_header(
            input,
            REDEEM_CLAIM_CHECK_BYTES_V1,
            &CLAIM_CHECK_REDEEM_MAGIC_V1,
            ClaimCheckActionV1::Redeem,
        )?;
        require_zero(
            input,
            REDEEM_RESERVED_BODY_OFFSET,
            REDEEM_RESERVED_BODY_BYTES,
        )?;
        Self {
            aggregate: read_array(input, REDEEM_AGGREGATE_OFFSET)?,
            owner: read_array(input, REDEEM_OWNER_OFFSET)?,
        }
        .new()
    }

    /// Encode one exact canonical redemption request.
    pub fn to_bytes(self) -> ClaimCheckResultV1<[u8; REDEEM_CLAIM_CHECK_BYTES_V1]> {
        self.seeds()?;
        let mut output = [0; REDEEM_CLAIM_CHECK_BYTES_V1];
        write_header(&mut output, ClaimCheckActionV1::Redeem)?;
        write(&mut output, REDEEM_AGGREGATE_OFFSET, &self.aggregate)?;
        write(&mut output, REDEEM_OWNER_OFFSET, &self.owner)?;
        Ok(output)
    }

    /// Return the claim-check coordinates this request names.
    pub fn seeds(self) -> ClaimCheckResultV1<ClaimCheckSeedsV1> {
        ClaimCheckSeedsV1::new(self.aggregate, self.owner)
    }
}

/// Permissionless request to close one fully redeemed escrow.
///
/// The gate is the escrow's own outstanding count, not a deadline: an escrow
/// with a live claim-check is holding collateral for a holder who has not
/// come back, and that is the ruling working as intended rather than a leak.
/// Both accounts' rent, and any residue the vault still holds, fund the caller
/// -- which is why this crank needs no escrow of its own.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CloseClaimCheckEscrowRequestV1 {
    /// Claims aggregate whose escrow is being closed.
    pub aggregate: [u8; 32],
}

impl CloseClaimCheckEscrowRequestV1 {
    /// Construct and canonicalize one escrow-close request.
    pub fn new(self) -> ClaimCheckResultV1<Self> {
        self.seeds()?;
        Ok(self)
    }

    /// Hostile-decode one exact escrow-close request.
    pub fn decode(input: &[u8]) -> ClaimCheckResultV1<Self> {
        require_header(
            input,
            CLOSE_CLAIM_CHECK_ESCROW_BYTES_V1,
            &CLAIM_CHECK_REDEEM_MAGIC_V1,
            ClaimCheckActionV1::CloseEscrow,
        )?;
        require_zero(input, CLOSE_RESERVED_BODY_OFFSET, CLOSE_RESERVED_BODY_BYTES)?;
        Self {
            aggregate: read_array(input, CLOSE_AGGREGATE_OFFSET)?,
        }
        .new()
    }

    /// Encode one exact canonical escrow-close request.
    pub fn to_bytes(self) -> ClaimCheckResultV1<[u8; CLOSE_CLAIM_CHECK_ESCROW_BYTES_V1]> {
        self.seeds()?;
        let mut output = [0; CLOSE_CLAIM_CHECK_ESCROW_BYTES_V1];
        write_header(&mut output, ClaimCheckActionV1::CloseEscrow)?;
        write(&mut output, CLOSE_AGGREGATE_OFFSET, &self.aggregate)?;
        Ok(output)
    }

    /// Return the escrow coordinates this request names.
    pub fn seeds(self) -> ClaimCheckResultV1<ClaimCheckEscrowSeedsV1> {
        ClaimCheckEscrowSeedsV1::new(self.aggregate)
    }
}

/// Classify one instruction payload arriving under a claim-check magic.
///
/// Redemption and escrow close share a magic, so a dispatcher must read the
/// action rather than the magic alone. Returning the action without decoding
/// the body keeps that decision in one place instead of duplicating a width
/// test at each route's door.
pub fn claim_check_action_of(input: &[u8]) -> ClaimCheckResultV1<ClaimCheckActionV1> {
    let action = ClaimCheckActionV1::decode(read_byte(input, ACTION_OFFSET)?)?;
    exact(input, 0, &action.magic())?;
    if read_u16(input, VERSION_OFFSET)? != CLAIM_CHECK_WIRE_VERSION_V1 {
        return Err(ClaimCheckErrorV1::InvalidHeader);
    }
    Ok(action)
}

fn require_header(
    input: &[u8],
    width: usize,
    magic: &[u8; 8],
    action: ClaimCheckActionV1,
) -> ClaimCheckResultV1<()> {
    if input.len() != width {
        return Err(ClaimCheckErrorV1::InvalidLength);
    }
    exact(input, 0, magic)?;
    if read_u16(input, VERSION_OFFSET)? != CLAIM_CHECK_WIRE_VERSION_V1 {
        return Err(ClaimCheckErrorV1::InvalidHeader);
    }
    if ClaimCheckActionV1::decode(read_byte(input, ACTION_OFFSET)?)? != action {
        return Err(ClaimCheckErrorV1::UnknownTag);
    }
    require_zero(input, RESERVED_HEADER_OFFSET, RESERVED_HEADER_BYTES)
}

fn write_header(output: &mut [u8], action: ClaimCheckActionV1) -> ClaimCheckResultV1<()> {
    write(output, 0, &action.magic())?;
    write(
        output,
        VERSION_OFFSET,
        &CLAIM_CHECK_WIRE_VERSION_V1.to_le_bytes(),
    )?;
    write(output, ACTION_OFFSET, &[action as u8])
}

fn require_distinct(identities: &[[u8; 32]]) -> ClaimCheckResultV1<()> {
    for (index, left) in identities.iter().enumerate() {
        if left.iter().all(|byte| *byte == 0) {
            return Err(ClaimCheckErrorV1::InvalidIdentity);
        }
        let rest = index.checked_add(1).ok_or(ClaimCheckErrorV1::Arithmetic)?;
        if identities.iter().skip(rest).any(|right| right == left) {
            return Err(ClaimCheckErrorV1::InvalidIdentity);
        }
    }
    Ok(())
}

fn exact(input: &[u8], offset: usize, expected: &[u8]) -> ClaimCheckResultV1<()> {
    let end = offset
        .checked_add(expected.len())
        .ok_or(ClaimCheckErrorV1::InvalidLength)?;
    if input.get(offset..end) != Some(expected) {
        return Err(ClaimCheckErrorV1::InvalidHeader);
    }
    Ok(())
}

fn require_zero(input: &[u8], offset: usize, width: usize) -> ClaimCheckResultV1<()> {
    let end = offset
        .checked_add(width)
        .ok_or(ClaimCheckErrorV1::InvalidLength)?;
    if input
        .get(offset..end)
        .ok_or(ClaimCheckErrorV1::InvalidLength)?
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(ClaimCheckErrorV1::NonCanonical);
    }
    Ok(())
}

fn read_byte(input: &[u8], offset: usize) -> ClaimCheckResultV1<u8> {
    input
        .get(offset)
        .copied()
        .ok_or(ClaimCheckErrorV1::InvalidLength)
}

fn read_u16(input: &[u8], offset: usize) -> ClaimCheckResultV1<u16> {
    Ok(u16::from_le_bytes(read_array(input, offset)?))
}

fn read_u64(input: &[u8], offset: usize) -> ClaimCheckResultV1<u64> {
    Ok(u64::from_le_bytes(read_array(input, offset)?))
}

fn read_array<const N: usize>(input: &[u8], offset: usize) -> ClaimCheckResultV1<[u8; N]> {
    let end = offset
        .checked_add(N)
        .ok_or(ClaimCheckErrorV1::InvalidLength)?;
    input
        .get(offset..end)
        .ok_or(ClaimCheckErrorV1::InvalidLength)?
        .try_into()
        .map_err(|_| ClaimCheckErrorV1::InvalidLength)
}

fn write(output: &mut [u8], offset: usize, value: &[u8]) -> ClaimCheckResultV1<()> {
    let end = offset
        .checked_add(value.len())
        .ok_or(ClaimCheckErrorV1::InvalidLength)?;
    output
        .get_mut(offset..end)
        .ok_or(ClaimCheckErrorV1::InvalidLength)?
        .copy_from_slice(value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claim_check_v1::CLAIM_CHECK_ESCROW_MAGIC_V1;

    fn open() -> OpenClaimCheckEscrowRequestV1 {
        OpenClaimCheckEscrowRequestV1 {
            release_set: [1; 32],
            market: [2; 32],
            realm: [3; 32],
            generation: 9,
        }
        .new()
        .expect("open")
    }

    fn redeem() -> RedeemClaimCheckRequestV1 {
        RedeemClaimCheckRequestV1 {
            aggregate: [4; 32],
            owner: [5; 32],
        }
        .new()
        .expect("redeem")
    }

    fn close() -> CloseClaimCheckEscrowRequestV1 {
        CloseClaimCheckEscrowRequestV1 { aggregate: [4; 32] }
            .new()
            .expect("close")
    }

    #[test]
    fn every_request_round_trips_at_its_one_exact_width() {
        let open_bytes = open().to_bytes().expect("open bytes");
        assert_eq!(open_bytes.len(), OPEN_CLAIM_CHECK_ESCROW_BYTES_V1);
        assert_eq!(
            OpenClaimCheckEscrowRequestV1::decode(&open_bytes),
            Ok(open())
        );

        let redeem_bytes = redeem().to_bytes().expect("redeem bytes");
        assert_eq!(redeem_bytes.len(), REDEEM_CLAIM_CHECK_BYTES_V1);
        assert_eq!(
            RedeemClaimCheckRequestV1::decode(&redeem_bytes),
            Ok(redeem())
        );

        let close_bytes = close().to_bytes().expect("close bytes");
        assert_eq!(close_bytes.len(), CLOSE_CLAIM_CHECK_ESCROW_BYTES_V1);
        assert_eq!(
            CloseClaimCheckEscrowRequestV1::decode(&close_bytes),
            Ok(close())
        );
    }

    #[test]
    fn a_request_decoded_at_another_requests_width_is_refused() {
        // The three widths are distinct, which is what lets a dispatcher route
        // on the action without a body decode ever succeeding by accident.
        let open_bytes = open().to_bytes().expect("open bytes");
        assert_eq!(
            RedeemClaimCheckRequestV1::decode(&open_bytes),
            Err(ClaimCheckErrorV1::InvalidLength)
        );
        let redeem_bytes = redeem().to_bytes().expect("redeem bytes");
        assert_eq!(
            OpenClaimCheckEscrowRequestV1::decode(&redeem_bytes),
            Err(ClaimCheckErrorV1::InvalidLength)
        );
        assert_ne!(
            OPEN_CLAIM_CHECK_ESCROW_BYTES_V1,
            REDEEM_CLAIM_CHECK_BYTES_V1
        );
        assert_ne!(
            REDEEM_CLAIM_CHECK_BYTES_V1,
            CLOSE_CLAIM_CHECK_ESCROW_BYTES_V1
        );
    }

    #[test]
    fn redemption_and_escrow_close_share_a_magic_and_separate_by_action() {
        let redeem_bytes = redeem().to_bytes().expect("redeem bytes");
        let close_bytes = close().to_bytes().expect("close bytes");
        assert_eq!(
            redeem_bytes.get(..8),
            close_bytes.get(..8),
            "one magic serves both"
        );
        assert_eq!(
            claim_check_action_of(&redeem_bytes),
            Ok(ClaimCheckActionV1::Redeem)
        );
        assert_eq!(
            claim_check_action_of(&close_bytes),
            Ok(ClaimCheckActionV1::CloseEscrow)
        );
        assert_eq!(
            claim_check_action_of(&open().to_bytes().expect("open bytes")),
            Ok(ClaimCheckActionV1::OpenEscrow)
        );
    }

    #[test]
    fn an_action_under_the_wrong_magic_is_refused() {
        // A redeem tag arriving under the open magic must not dispatch as
        // either route: the pair is checked, never just the tag.
        let mut forged = redeem().to_bytes().expect("bytes");
        write(&mut forged, 0, &CLAIM_CHECK_OPEN_MAGIC_V1).expect("forge magic");
        assert_eq!(
            claim_check_action_of(&forged),
            Err(ClaimCheckErrorV1::InvalidHeader)
        );
        assert_eq!(
            RedeemClaimCheckRequestV1::decode(&forged),
            Err(ClaimCheckErrorV1::InvalidHeader)
        );

        let mut swapped = redeem().to_bytes().expect("bytes");
        write(
            &mut swapped,
            ACTION_OFFSET,
            &[ClaimCheckActionV1::CloseEscrow as u8],
        )
        .expect("forge action");
        assert_eq!(
            RedeemClaimCheckRequestV1::decode(&swapped),
            Err(ClaimCheckErrorV1::UnknownTag)
        );
    }

    #[test]
    fn an_unknown_action_or_version_is_refused() {
        for tag in [0_u8, 5, 200, 255] {
            let mut bytes = redeem().to_bytes().expect("bytes");
            write(&mut bytes, ACTION_OFFSET, &[tag]).expect("tag");
            assert_eq!(
                claim_check_action_of(&bytes),
                Err(ClaimCheckErrorV1::UnknownTag)
            );
        }
        let mut versioned = open().to_bytes().expect("bytes");
        write(&mut versioned, VERSION_OFFSET, &2_u16.to_le_bytes()).expect("version");
        assert_eq!(
            OpenClaimCheckEscrowRequestV1::decode(&versioned),
            Err(ClaimCheckErrorV1::InvalidHeader)
        );
        assert_eq!(
            claim_check_action_of(&versioned),
            Err(ClaimCheckErrorV1::InvalidHeader)
        );
    }

    #[test]
    fn a_persisted_record_magic_never_decodes_as_a_request() {
        // The record magics and the request magics live in one namespace, and
        // an account's bytes must never be mistaken for an instruction's.
        let mut bytes = open().to_bytes().expect("bytes");
        write(&mut bytes, 0, &CLAIM_CHECK_ESCROW_MAGIC_V1).expect("record magic");
        assert_eq!(
            OpenClaimCheckEscrowRequestV1::decode(&bytes),
            Err(ClaimCheckErrorV1::InvalidHeader)
        );
        assert_eq!(
            claim_check_action_of(&bytes),
            Err(ClaimCheckErrorV1::InvalidHeader)
        );
    }

    #[test]
    fn every_reserved_byte_of_every_request_must_be_zero() {
        for offset in RESERVED_HEADER_OFFSET..(RESERVED_HEADER_OFFSET + RESERVED_HEADER_BYTES) {
            let mut bytes = open().to_bytes().expect("bytes");
            write(&mut bytes, offset, &[0xFF]).expect("dirty");
            assert_eq!(
                OpenClaimCheckEscrowRequestV1::decode(&bytes),
                Err(ClaimCheckErrorV1::NonCanonical)
            );
        }
        for offset in OPEN_RESERVED_BODY_OFFSET..OPEN_CLAIM_CHECK_ESCROW_BYTES_V1 {
            let mut bytes = open().to_bytes().expect("bytes");
            write(&mut bytes, offset, &[1]).expect("dirty");
            assert_eq!(
                OpenClaimCheckEscrowRequestV1::decode(&bytes),
                Err(ClaimCheckErrorV1::NonCanonical)
            );
        }
        for offset in REDEEM_RESERVED_BODY_OFFSET..REDEEM_CLAIM_CHECK_BYTES_V1 {
            let mut bytes = redeem().to_bytes().expect("bytes");
            write(&mut bytes, offset, &1_u8.to_le_bytes()).expect("dirty");
            assert_eq!(
                RedeemClaimCheckRequestV1::decode(&bytes),
                Err(ClaimCheckErrorV1::NonCanonical)
            );
        }
        for offset in CLOSE_RESERVED_BODY_OFFSET..CLOSE_CLAIM_CHECK_ESCROW_BYTES_V1 {
            let mut bytes = close().to_bytes().expect("bytes");
            write(&mut bytes, offset, &1_u8.to_le_bytes()).expect("dirty");
            assert_eq!(
                CloseClaimCheckEscrowRequestV1::decode(&bytes),
                Err(ClaimCheckErrorV1::NonCanonical)
            );
        }
    }

    #[test]
    fn a_truncated_request_is_refused_before_any_field_is_read() {
        for bytes in [
            open().to_bytes().expect("open").as_slice(),
            redeem().to_bytes().expect("redeem").as_slice(),
            close().to_bytes().expect("close").as_slice(),
        ] {
            let short = bytes.get(..bytes.len() - 1).expect("truncate");
            assert_eq!(
                OpenClaimCheckEscrowRequestV1::decode(short),
                Err(ClaimCheckErrorV1::InvalidLength)
            );
            assert_eq!(
                RedeemClaimCheckRequestV1::decode(short),
                Err(ClaimCheckErrorV1::InvalidLength)
            );
            assert_eq!(
                CloseClaimCheckEscrowRequestV1::decode(short),
                Err(ClaimCheckErrorV1::InvalidLength)
            );
        }
        assert_eq!(
            claim_check_action_of(&[]),
            Err(ClaimCheckErrorV1::InvalidLength)
        );
    }

    #[test]
    fn a_request_naming_a_zero_or_aliased_identity_is_refused() {
        for mutate in [
            |value: &mut OpenClaimCheckEscrowRequestV1| value.release_set = [0; 32],
            |value: &mut OpenClaimCheckEscrowRequestV1| value.market = [0; 32],
            |value: &mut OpenClaimCheckEscrowRequestV1| value.realm = [0; 32],
            |value: &mut OpenClaimCheckEscrowRequestV1| value.market = value.release_set,
        ] {
            let mut value = open();
            mutate(&mut value);
            assert_eq!(value.new(), Err(ClaimCheckErrorV1::InvalidIdentity));
        }
        let mut ungenerated = open();
        ungenerated.generation = 0;
        assert_eq!(
            ungenerated.new(),
            Err(ClaimCheckErrorV1::InvalidEntitlement)
        );

        // Redemption's coordinates refuse exactly what the position's do.
        let mut aliased = redeem();
        aliased.owner = aliased.aggregate;
        assert_eq!(aliased.new(), Err(ClaimCheckErrorV1::InvalidIdentity));
        let mut zeroed = redeem();
        zeroed.owner = [0; 32];
        assert_eq!(zeroed.new(), Err(ClaimCheckErrorV1::InvalidIdentity));
        assert_eq!(
            CloseClaimCheckEscrowRequestV1 { aggregate: [0; 32] }.new(),
            Err(ClaimCheckErrorV1::InvalidIdentity)
        );
    }

    #[test]
    fn there_is_no_holder_field_a_caller_could_forge() {
        // Redemption names the holder only as a PDA coordinate, and the
        // coordinate is checked against the account it derives. A route reading
        // a holder from anywhere else would be a second author for a fact the
        // address already proves.
        let value = redeem();
        let seeds = value.seeds().expect("seeds");
        assert_eq!(seeds.owner(), value.owner);
        assert_eq!(seeds.aggregate(), value.aggregate);
        // And the open request carries no owner at all: the escrow is per
        // market, never per holder.
        let open_bytes = open().to_bytes().expect("bytes");
        assert!(!open_bytes.windows(32).any(|window| window == [5_u8; 32]));
    }
}
