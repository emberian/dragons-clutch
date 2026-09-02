//! Transient request for atomic expiry of a still-unallocated Series permit.
//!
//! Unlike [`crate::SeriesPermitExpiryRequestV1`], this route-local request does
//! not carry a caller-supplied permit body. The selected atomic Series route
//! reaches Core before the deterministic permit PDA has ever been allocated,
//! so Core derives its address, RentCredit, and refund owner from finalized
//! Series records and the authenticated account frame. Only the two replay
//! revisions originate in the family request and therefore cross this wire.

use crate::{Error, PHYSICAL_ABI_VERSION_V1};

/// Distinct transient instruction magic; never accepted as ordinary V1 expiry.
pub const SERIES_UNALLOCATED_PERMIT_EXPIRY_REQUEST_MAGIC_V1: [u8; 8] = *b"DCLSUPE1";
/// Exact fixed width of [`SeriesUnallocatedPermitExpiryRequestV1`].
pub const SERIES_UNALLOCATED_PERMIT_EXPIRY_REQUEST_BYTES_V1: usize = 32;

const VERSION_OFFSET: usize = 8;
const _: () = assert!(SERIES_UNALLOCATED_PERMIT_EXPIRY_REQUEST_MAGIC_V1.len() == VERSION_OFFSET);
const RESERVED_OFFSET: usize = 10;
/// Byte offset of the expected Series-root revision.
pub const SERIES_UNALLOCATED_PERMIT_EXPIRY_EXPECTED_SERIES_REVISION_OFFSET_V1: usize = 16;
/// Byte offset of the expected Ticket revision.
pub const SERIES_UNALLOCATED_PERMIT_EXPIRY_EXPECTED_TICKET_REVISION_OFFSET_V1: usize = 24;

/// Root-independent replay coordinates for atomic unallocated-permit expiry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesUnallocatedPermitExpiryRequestV1 {
    expected_series_revision: u64,
    expected_ticket_revision: u64,
}

impl SeriesUnallocatedPermitExpiryRequestV1 {
    /// Construct one exact pair of family-authenticated replay revisions.
    #[must_use]
    pub const fn new(expected_series_revision: u64, expected_ticket_revision: u64) -> Self {
        Self {
            expected_series_revision,
            expected_ticket_revision,
        }
    }

    /// Hostile-decode the exact transient wire.
    pub fn decode(input: &[u8]) -> Result<Self, Error> {
        if input.len() != SERIES_UNALLOCATED_PERMIT_EXPIRY_REQUEST_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        if input.get(..SERIES_UNALLOCATED_PERMIT_EXPIRY_REQUEST_MAGIC_V1.len())
            != Some(SERIES_UNALLOCATED_PERMIT_EXPIRY_REQUEST_MAGIC_V1.as_slice())
        {
            return Err(Error::InvalidMagic);
        }
        let version = u16::from_le_bytes(
            input
                .get(VERSION_OFFSET..VERSION_OFFSET + 2)
                .ok_or(Error::InvalidLength)?
                .try_into()
                .map_err(|_| Error::InvalidLength)?,
        );
        if version != PHYSICAL_ABI_VERSION_V1 {
            return Err(Error::UnsupportedVersion);
        }
        if input
            .get(
                RESERVED_OFFSET
                    ..SERIES_UNALLOCATED_PERMIT_EXPIRY_EXPECTED_SERIES_REVISION_OFFSET_V1,
            )
            .ok_or(Error::InvalidLength)?
            .iter()
            .any(|byte| *byte != 0)
        {
            return Err(Error::NonzeroReserved);
        }
        Ok(Self::new(
            read_u64(
                input,
                SERIES_UNALLOCATED_PERMIT_EXPIRY_EXPECTED_SERIES_REVISION_OFFSET_V1,
            )?,
            read_u64(
                input,
                SERIES_UNALLOCATED_PERMIT_EXPIRY_EXPECTED_TICKET_REVISION_OFFSET_V1,
            )?,
        ))
    }

    /// Encode the exact transient wire.
    #[must_use]
    pub fn encode(self) -> [u8; SERIES_UNALLOCATED_PERMIT_EXPIRY_REQUEST_BYTES_V1] {
        let mut output = [0_u8; SERIES_UNALLOCATED_PERMIT_EXPIRY_REQUEST_BYTES_V1];
        output[..VERSION_OFFSET]
            .copy_from_slice(&SERIES_UNALLOCATED_PERMIT_EXPIRY_REQUEST_MAGIC_V1);
        output[VERSION_OFFSET..VERSION_OFFSET + 2]
            .copy_from_slice(&PHYSICAL_ABI_VERSION_V1.to_le_bytes());
        output[SERIES_UNALLOCATED_PERMIT_EXPIRY_EXPECTED_SERIES_REVISION_OFFSET_V1
            ..SERIES_UNALLOCATED_PERMIT_EXPIRY_EXPECTED_SERIES_REVISION_OFFSET_V1 + 8]
            .copy_from_slice(&self.expected_series_revision.to_le_bytes());
        output[SERIES_UNALLOCATED_PERMIT_EXPIRY_EXPECTED_TICKET_REVISION_OFFSET_V1
            ..SERIES_UNALLOCATED_PERMIT_EXPIRY_EXPECTED_TICKET_REVISION_OFFSET_V1 + 8]
            .copy_from_slice(&self.expected_ticket_revision.to_le_bytes());
        output
    }

    /// Expected persistent Series-root revision before Expire.
    #[must_use]
    pub const fn expected_series_revision(self) -> u64 {
        self.expected_series_revision
    }

    /// Expected prepared Ticket revision before Expire.
    #[must_use]
    pub const fn expected_ticket_revision(self) -> u64 {
        self.expected_ticket_revision
    }
}

fn read_u64(input: &[u8], offset: usize) -> Result<u64, Error> {
    input
        .get(offset..offset + 8)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map(u64::from_le_bytes)
        .map_err(|_| Error::InvalidLength)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]

    use super::*;

    #[test]
    fn exact_roundtrip_and_hostile_header_refusals() {
        let request = SeriesUnallocatedPermitExpiryRequestV1::new(7, 9);
        let bytes = request.encode();
        assert_eq!(
            SeriesUnallocatedPermitExpiryRequestV1::decode(&bytes),
            Ok(request),
        );
        assert_eq!(
            SeriesUnallocatedPermitExpiryRequestV1::decode(&bytes[..bytes.len() - 1]),
            Err(Error::InvalidLength),
        );

        let mut hostile = bytes;
        hostile[0] ^= 1;
        assert_eq!(
            SeriesUnallocatedPermitExpiryRequestV1::decode(&hostile),
            Err(Error::InvalidMagic),
        );
        let mut hostile = bytes;
        hostile[RESERVED_OFFSET] = 1;
        assert_eq!(
            SeriesUnallocatedPermitExpiryRequestV1::decode(&hostile),
            Err(Error::NonzeroReserved),
        );
    }
}
