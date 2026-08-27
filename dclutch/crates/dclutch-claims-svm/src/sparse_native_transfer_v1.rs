//! Fixed-width sparse native-claim transfer over LiabilityBasisV2.
//!
//! This is the canonical O(1) transfer primitive for one selected outcome. It
//! carries no runtime-width neutral vector: Claims independently authenticates
//! Product Runtime V3 and the LiabilityBasisV2 aggregate/Position widths, then
//! debits one source coordinate and credits the same quantity to one distinct
//! destination coordinate atomically.

use crate::CallerRole;

/// Exact sparse-transfer request bytes.
pub const SPARSE_NATIVE_TRANSFER_BYTES_V1: usize = 320;
/// Exact sparse-transfer receipt bytes.
pub const SPARSE_NATIVE_TRANSFER_RECEIPT_BYTES_V1: usize = 448;
/// Canonical request magic.
pub const SPARSE_NATIVE_TRANSFER_MAGIC_V1: [u8; 8] = *b"DCLSPT01";
/// Canonical receipt magic.
pub const SPARSE_NATIVE_TRANSFER_RECEIPT_MAGIC_V1: [u8; 8] = *b"DCLSPR01";
/// Implemented sparse-transfer wire version.
pub const SPARSE_NATIVE_TRANSFER_VERSION_V1: u16 = 1;

const VERSION: usize = 8;
const ROLE: usize = 10;
const RESERVED: usize = 11;

/// Public typed patch coordinates for the fixed request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SparseNativeTransferLayoutV1;

impl SparseNativeTransferLayoutV1 {
    /// Current execution release set.
    pub const RELEASE_SET: usize = 16;
    /// Logical Core Market.
    pub const MARKET: usize = 48;
    /// Nonzero parent request identity.
    pub const REQUEST_ID: usize = 80;
    /// Finalized Product graph-root digest.
    pub const PRODUCT_RECORD: usize = 112;
    /// Semantic LiabilityBasisV2 identity.
    pub const SEMANTIC_BASIS: usize = 144;
    /// Finalized canonical ProductBasisV3 raw digest.
    pub const LINKED_BASIS_RECORD: usize = 176;
    /// Source Position owner.
    pub const SOURCE_OWNER: usize = 208;
    /// Destination Position owner.
    pub const DESTINATION_OWNER: usize = 240;
    /// Aggregate optimistic pre-revision.
    pub const MARKET_REVISION: usize = 272;
    /// Source Position optimistic pre-revision.
    pub const SOURCE_REVISION: usize = 280;
    /// Destination Position optimistic pre-revision.
    pub const DESTINATION_REVISION: usize = 288;
    /// Immutable Market generation.
    pub const GENERATION: usize = 296;
    /// Selected Product outcome.
    pub const OUTCOME: usize = 304;
    /// Product-authenticated runtime claim count.
    pub const CLAIM_COUNT: usize = 308;
    /// Exact positive native quantity.
    pub const QUANTITY: usize = 312;
}

const RECEIPT_PACKET_DIGEST: usize = 272;
const RECEIPT_CLAIMS_PROGRAM: usize = 304;
const RECEIPT_RESOURCE_DIGEST: usize = 336;
const RECEIPT_PRE_MARKET_REVISION: usize = 368;
const RECEIPT_PRE_SOURCE_REVISION: usize = 376;
const RECEIPT_PRE_DESTINATION_REVISION: usize = 384;
const RECEIPT_POST_MARKET_REVISION: usize = 392;
const RECEIPT_POST_SOURCE_REVISION: usize = 400;
const RECEIPT_POST_DESTINATION_REVISION: usize = 408;
const RECEIPT_GENERATION: usize = 416;
const RECEIPT_OUTCOME: usize = 424;
const RECEIPT_CLAIM_COUNT: usize = 428;
const RECEIPT_QUANTITY: usize = 432;
const RECEIPT_RESERVED: usize = 440;

/// Stable sparse-transfer refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SparseNativeTransferErrorV1 {
    /// Request or receipt width differed.
    InvalidLength,
    /// Magic selected another wire family.
    InvalidMagic,
    /// Version was unsupported.
    UnsupportedVersion,
    /// Reserved bytes were nonzero.
    NonCanonical,
    /// A required identity was zero or source and destination aliased.
    InvalidIdentity,
    /// Quantity, outcome, or runtime width was invalid.
    InvalidQuantity,
    /// An optimistic revision could not advance exactly once.
    InvalidRevision,
    /// Receipt facts differed from the request.
    ReceiptMismatch,
}

/// Result alias for sparse transfers.
pub type Result<T> = core::result::Result<T, SparseNativeTransferErrorV1>;

/// Immutable fields of one sparse native transfer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SparseNativeTransferInputV1 {
    /// Registry role of the selected caller program.
    pub caller_role: CallerRole,
    /// Current immutable execution release set.
    pub release_set: [u8; 32],
    /// Logical Core Market identity.
    pub market: [u8; 32],
    /// Nonzero parent request identity.
    pub request_id: [u8; 32],
    /// Finalized Product graph-root digest.
    pub product_record_digest: [u8; 32],
    /// Semantic LiabilityBasisV2 identity.
    pub semantic_basis_id: [u8; 32],
    /// Finalized canonical ProductBasisV3 raw-record digest.
    pub linked_basis_record_digest: [u8; 32],
    /// Source Position owner.
    pub source_owner: [u8; 32],
    /// Destination Position owner.
    pub destination_owner: [u8; 32],
    /// Aggregate optimistic pre-revision.
    pub expected_market_revision: u64,
    /// Source Position optimistic pre-revision.
    pub expected_source_revision: u64,
    /// Destination Position optimistic pre-revision.
    pub expected_destination_revision: u64,
    /// Immutable Market generation.
    pub generation: u64,
    /// Selected Product outcome.
    pub outcome: u32,
    /// Product-authenticated runtime claim count.
    pub claim_count: u32,
    /// Exact positive native quantity.
    pub quantity: u64,
}

/// Canonical fixed-width sparse transfer request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SparseNativeTransferV1(SparseNativeTransferInputV1);

impl SparseNativeTransferV1 {
    /// Construct and fully validate one request.
    pub fn new(input: SparseNativeTransferInputV1) -> Result<Self> {
        require_nonzero(&[
            input.release_set,
            input.market,
            input.request_id,
            input.product_record_digest,
            input.semantic_basis_id,
            input.linked_basis_record_digest,
            input.source_owner,
            input.destination_owner,
        ])?;
        if input.source_owner == input.destination_owner {
            return Err(SparseNativeTransferErrorV1::InvalidIdentity);
        }
        if input.quantity == 0 || input.claim_count == 0 || input.outcome >= input.claim_count {
            return Err(SparseNativeTransferErrorV1::InvalidQuantity);
        }
        if input.expected_market_revision == u64::MAX
            || input.expected_source_revision == u64::MAX
            || input.expected_destination_revision == u64::MAX
        {
            return Err(SparseNativeTransferErrorV1::InvalidRevision);
        }
        Ok(Self(input))
    }

    /// Hostile-decode one exact request.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != SPARSE_NATIVE_TRANSFER_BYTES_V1 {
            return Err(SparseNativeTransferErrorV1::InvalidLength);
        }
        exact(bytes, 0, &SPARSE_NATIVE_TRANSFER_MAGIC_V1)?;
        if read_u16(bytes, VERSION)? != SPARSE_NATIVE_TRANSFER_VERSION_V1 {
            return Err(SparseNativeTransferErrorV1::UnsupportedVersion);
        }
        require_zero(bytes, RESERVED, 5)?;
        Self::new(SparseNativeTransferInputV1 {
            caller_role: decode_role(read_u8(bytes, ROLE)?)?,
            release_set: read_array(bytes, SparseNativeTransferLayoutV1::RELEASE_SET)?,
            market: read_array(bytes, SparseNativeTransferLayoutV1::MARKET)?,
            request_id: read_array(bytes, SparseNativeTransferLayoutV1::REQUEST_ID)?,
            product_record_digest: read_array(bytes, SparseNativeTransferLayoutV1::PRODUCT_RECORD)?,
            semantic_basis_id: read_array(bytes, SparseNativeTransferLayoutV1::SEMANTIC_BASIS)?,
            linked_basis_record_digest: read_array(
                bytes,
                SparseNativeTransferLayoutV1::LINKED_BASIS_RECORD,
            )?,
            source_owner: read_array(bytes, SparseNativeTransferLayoutV1::SOURCE_OWNER)?,
            destination_owner: read_array(bytes, SparseNativeTransferLayoutV1::DESTINATION_OWNER)?,
            expected_market_revision: read_u64(
                bytes,
                SparseNativeTransferLayoutV1::MARKET_REVISION,
            )?,
            expected_source_revision: read_u64(
                bytes,
                SparseNativeTransferLayoutV1::SOURCE_REVISION,
            )?,
            expected_destination_revision: read_u64(
                bytes,
                SparseNativeTransferLayoutV1::DESTINATION_REVISION,
            )?,
            generation: read_u64(bytes, SparseNativeTransferLayoutV1::GENERATION)?,
            outcome: read_u32(bytes, SparseNativeTransferLayoutV1::OUTCOME)?,
            claim_count: read_u32(bytes, SparseNativeTransferLayoutV1::CLAIM_COUNT)?,
            quantity: read_u64(bytes, SparseNativeTransferLayoutV1::QUANTITY)?,
        })
    }

    /// Encode the canonical fixed request.
    pub fn to_bytes(self) -> [u8; SPARSE_NATIVE_TRANSFER_BYTES_V1] {
        let mut output = [0_u8; SPARSE_NATIVE_TRANSFER_BYTES_V1];
        put(&mut output, 0, &SPARSE_NATIVE_TRANSFER_MAGIC_V1);
        put(
            &mut output,
            VERSION,
            &SPARSE_NATIVE_TRANSFER_VERSION_V1.to_le_bytes(),
        );
        output[ROLE] = self.0.caller_role as u8;
        for (offset, value) in [
            (
                SparseNativeTransferLayoutV1::RELEASE_SET,
                self.0.release_set,
            ),
            (SparseNativeTransferLayoutV1::MARKET, self.0.market),
            (SparseNativeTransferLayoutV1::REQUEST_ID, self.0.request_id),
            (
                SparseNativeTransferLayoutV1::PRODUCT_RECORD,
                self.0.product_record_digest,
            ),
            (
                SparseNativeTransferLayoutV1::SEMANTIC_BASIS,
                self.0.semantic_basis_id,
            ),
            (
                SparseNativeTransferLayoutV1::LINKED_BASIS_RECORD,
                self.0.linked_basis_record_digest,
            ),
            (
                SparseNativeTransferLayoutV1::SOURCE_OWNER,
                self.0.source_owner,
            ),
            (
                SparseNativeTransferLayoutV1::DESTINATION_OWNER,
                self.0.destination_owner,
            ),
        ] {
            put(&mut output, offset, &value);
        }
        for (offset, value) in [
            (
                SparseNativeTransferLayoutV1::MARKET_REVISION,
                self.0.expected_market_revision,
            ),
            (
                SparseNativeTransferLayoutV1::SOURCE_REVISION,
                self.0.expected_source_revision,
            ),
            (
                SparseNativeTransferLayoutV1::DESTINATION_REVISION,
                self.0.expected_destination_revision,
            ),
            (SparseNativeTransferLayoutV1::GENERATION, self.0.generation),
            (SparseNativeTransferLayoutV1::QUANTITY, self.0.quantity),
        ] {
            put(&mut output, offset, &value.to_le_bytes());
        }
        put(
            &mut output,
            SparseNativeTransferLayoutV1::OUTCOME,
            &self.0.outcome.to_le_bytes(),
        );
        put(
            &mut output,
            SparseNativeTransferLayoutV1::CLAIM_COUNT,
            &self.0.claim_count.to_le_bytes(),
        );
        output
    }

    /// Return immutable request fields.
    pub const fn input(self) -> SparseNativeTransferInputV1 {
        self.0
    }
}

/// Canonical fixed success receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SparseNativeTransferReceiptV1 {
    request: SparseNativeTransferV1,
    packet_digest: [u8; 32],
    claims_program: [u8; 32],
    post_resource_digest: [u8; 32],
    post_market_revision: u64,
    post_source_revision: u64,
    post_destination_revision: u64,
}

impl SparseNativeTransferReceiptV1 {
    /// Construct an exact success receipt.
    pub fn new(
        request: SparseNativeTransferV1,
        packet_digest: [u8; 32],
        claims_program: [u8; 32],
        post_resource_digest: [u8; 32],
        post_market_revision: u64,
        post_source_revision: u64,
        post_destination_revision: u64,
    ) -> Result<Self> {
        require_nonzero(&[packet_digest, claims_program, post_resource_digest])?;
        let input = request.input();
        if input.expected_market_revision.checked_add(1) != Some(post_market_revision)
            || input.expected_source_revision.checked_add(1) != Some(post_source_revision)
            || input.expected_destination_revision.checked_add(1) != Some(post_destination_revision)
        {
            return Err(SparseNativeTransferErrorV1::InvalidRevision);
        }
        Ok(Self {
            request,
            packet_digest,
            claims_program,
            post_resource_digest,
            post_market_revision,
            post_source_revision,
            post_destination_revision,
        })
    }

    /// Decode one exact receipt.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != SPARSE_NATIVE_TRANSFER_RECEIPT_BYTES_V1 {
            return Err(SparseNativeTransferErrorV1::InvalidLength);
        }
        exact(bytes, 0, &SPARSE_NATIVE_TRANSFER_RECEIPT_MAGIC_V1)?;
        if read_u16(bytes, VERSION)? != SPARSE_NATIVE_TRANSFER_VERSION_V1 {
            return Err(SparseNativeTransferErrorV1::UnsupportedVersion);
        }
        require_zero(bytes, RESERVED, 5)?;
        require_zero(bytes, RECEIPT_RESERVED, 8)?;
        let request = SparseNativeTransferV1::new(SparseNativeTransferInputV1 {
            caller_role: decode_role(read_u8(bytes, ROLE)?)?,
            release_set: read_array(bytes, SparseNativeTransferLayoutV1::RELEASE_SET)?,
            market: read_array(bytes, SparseNativeTransferLayoutV1::MARKET)?,
            request_id: read_array(bytes, SparseNativeTransferLayoutV1::REQUEST_ID)?,
            product_record_digest: read_array(bytes, SparseNativeTransferLayoutV1::PRODUCT_RECORD)?,
            semantic_basis_id: read_array(bytes, SparseNativeTransferLayoutV1::SEMANTIC_BASIS)?,
            linked_basis_record_digest: read_array(
                bytes,
                SparseNativeTransferLayoutV1::LINKED_BASIS_RECORD,
            )?,
            source_owner: read_array(bytes, SparseNativeTransferLayoutV1::SOURCE_OWNER)?,
            destination_owner: read_array(bytes, SparseNativeTransferLayoutV1::DESTINATION_OWNER)?,
            expected_market_revision: read_u64(bytes, RECEIPT_PRE_MARKET_REVISION)?,
            expected_source_revision: read_u64(bytes, RECEIPT_PRE_SOURCE_REVISION)?,
            expected_destination_revision: read_u64(bytes, RECEIPT_PRE_DESTINATION_REVISION)?,
            generation: read_u64(bytes, RECEIPT_GENERATION)?,
            outcome: read_u32(bytes, RECEIPT_OUTCOME)?,
            claim_count: read_u32(bytes, RECEIPT_CLAIM_COUNT)?,
            quantity: read_u64(bytes, RECEIPT_QUANTITY)?,
        })?;
        Self::new(
            request,
            read_array(bytes, RECEIPT_PACKET_DIGEST)?,
            read_array(bytes, RECEIPT_CLAIMS_PROGRAM)?,
            read_array(bytes, RECEIPT_RESOURCE_DIGEST)?,
            read_u64(bytes, RECEIPT_POST_MARKET_REVISION)?,
            read_u64(bytes, RECEIPT_POST_SOURCE_REVISION)?,
            read_u64(bytes, RECEIPT_POST_DESTINATION_REVISION)?,
        )
    }

    /// Encode this exact receipt.
    pub fn to_bytes(self) -> [u8; SPARSE_NATIVE_TRANSFER_RECEIPT_BYTES_V1] {
        let mut output = [0_u8; SPARSE_NATIVE_TRANSFER_RECEIPT_BYTES_V1];
        let input = self.request.input();
        put(&mut output, 0, &SPARSE_NATIVE_TRANSFER_RECEIPT_MAGIC_V1);
        put(
            &mut output,
            VERSION,
            &SPARSE_NATIVE_TRANSFER_VERSION_V1.to_le_bytes(),
        );
        output[ROLE] = input.caller_role as u8;
        let request = self.request.to_bytes();
        output[16..272].copy_from_slice(&request[16..272]);
        for (offset, value) in [
            (RECEIPT_PACKET_DIGEST, self.packet_digest),
            (RECEIPT_CLAIMS_PROGRAM, self.claims_program),
            (RECEIPT_RESOURCE_DIGEST, self.post_resource_digest),
        ] {
            put(&mut output, offset, &value);
        }
        for (offset, value) in [
            (RECEIPT_PRE_MARKET_REVISION, input.expected_market_revision),
            (RECEIPT_PRE_SOURCE_REVISION, input.expected_source_revision),
            (
                RECEIPT_PRE_DESTINATION_REVISION,
                input.expected_destination_revision,
            ),
            (RECEIPT_POST_MARKET_REVISION, self.post_market_revision),
            (RECEIPT_POST_SOURCE_REVISION, self.post_source_revision),
            (
                RECEIPT_POST_DESTINATION_REVISION,
                self.post_destination_revision,
            ),
            (RECEIPT_GENERATION, input.generation),
            (RECEIPT_QUANTITY, input.quantity),
        ] {
            put(&mut output, offset, &value.to_le_bytes());
        }
        put(&mut output, RECEIPT_OUTCOME, &input.outcome.to_le_bytes());
        put(
            &mut output,
            RECEIPT_CLAIM_COUNT,
            &input.claim_count.to_le_bytes(),
        );
        output
    }

    /// Require exact request binding.
    pub fn validate_request(self, request: SparseNativeTransferV1) -> Result<()> {
        if self.request != request {
            return Err(SparseNativeTransferErrorV1::ReceiptMismatch);
        }
        Ok(())
    }

    /// Return the exact request whose successful transition this receipt binds.
    pub const fn request(self) -> SparseNativeTransferV1 {
        self.request
    }

    /// Exact request packet digest.
    pub const fn packet_digest(self) -> [u8; 32] {
        self.packet_digest
    }
    /// Claims program producing the receipt.
    pub const fn claims_program(self) -> [u8; 32] {
        self.claims_program
    }
    /// Digest of aggregate, source, then destination poststates.
    pub const fn post_resource_digest(self) -> [u8; 32] {
        self.post_resource_digest
    }
    /// Aggregate post revision.
    pub const fn post_market_revision(self) -> u64 {
        self.post_market_revision
    }
    /// Source Position post revision.
    pub const fn post_source_revision(self) -> u64 {
        self.post_source_revision
    }
    /// Destination Position post revision.
    pub const fn post_destination_revision(self) -> u64 {
        self.post_destination_revision
    }
}

fn decode_role(value: u8) -> Result<CallerRole> {
    match value {
        0 => Ok(CallerRole::Core),
        2 => Ok(CallerRole::Trading),
        _ => Err(SparseNativeTransferErrorV1::NonCanonical),
    }
}

fn require_nonzero(values: &[[u8; 32]]) -> Result<()> {
    if values.contains(&[0; 32]) {
        Err(SparseNativeTransferErrorV1::InvalidIdentity)
    } else {
        Ok(())
    }
}

fn exact(bytes: &[u8], offset: usize, value: &[u8]) -> Result<()> {
    if bytes.get(offset..offset.saturating_add(value.len())) == Some(value) {
        Ok(())
    } else {
        Err(SparseNativeTransferErrorV1::InvalidMagic)
    }
}

fn require_zero(bytes: &[u8], offset: usize, width: usize) -> Result<()> {
    if bytes
        .get(offset..offset.saturating_add(width))
        .is_some_and(|value| value.iter().all(|byte| *byte == 0))
    {
        Ok(())
    } else {
        Err(SparseNativeTransferErrorV1::NonCanonical)
    }
}

fn read_u8(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes
        .get(offset)
        .copied()
        .ok_or(SparseNativeTransferErrorV1::InvalidLength)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(read_array(bytes, offset)?))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(read_array(bytes, offset)?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(read_array(bytes, offset)?))
}

fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = offset
        .checked_add(N)
        .ok_or(SparseNativeTransferErrorV1::InvalidLength)?;
    bytes
        .get(offset..end)
        .ok_or(SparseNativeTransferErrorV1::InvalidLength)?
        .try_into()
        .map_err(|_| SparseNativeTransferErrorV1::InvalidLength)
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) {
    if let Some(end) = offset.checked_add(value.len())
        && let Some(destination) = output.get_mut(offset..end)
    {
        destination.copy_from_slice(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn request() -> SparseNativeTransferV1 {
        SparseNativeTransferV1::new(SparseNativeTransferInputV1 {
            caller_role: CallerRole::Trading,
            release_set: id(1),
            market: id(2),
            request_id: id(3),
            product_record_digest: id(4),
            semantic_basis_id: id(5),
            linked_basis_record_digest: id(6),
            source_owner: id(7),
            destination_owner: id(8),
            expected_market_revision: 9,
            expected_source_revision: 10,
            expected_destination_revision: 11,
            generation: 12,
            outcome: 2,
            claim_count: 4,
            quantity: 13,
        })
        .expect("request")
    }

    #[test]
    fn exact_request_and_receipt_round_trip() {
        let request = request();
        let bytes = request.to_bytes();
        assert_eq!(SparseNativeTransferV1::decode(&bytes), Ok(request));
        assert_eq!(
            bytes.get(
                SparseNativeTransferLayoutV1::QUANTITY..SparseNativeTransferLayoutV1::QUANTITY + 8
            ),
            Some(13_u64.to_le_bytes().as_slice())
        );
        let receipt =
            SparseNativeTransferReceiptV1::new(request, id(9), id(10), id(11), 10, 11, 12)
                .expect("receipt");
        assert_eq!(
            SparseNativeTransferReceiptV1::decode(&receipt.to_bytes()),
            Ok(receipt)
        );
        assert_eq!(receipt.validate_request(request), Ok(()));
    }

    #[test]
    fn hostile_alias_width_outcome_and_revision_refuse() {
        let mut input = request().input();
        input.destination_owner = input.source_owner;
        assert_eq!(
            SparseNativeTransferV1::new(input),
            Err(SparseNativeTransferErrorV1::InvalidIdentity)
        );
        input = request().input();
        input.outcome = input.claim_count;
        assert_eq!(
            SparseNativeTransferV1::new(input),
            Err(SparseNativeTransferErrorV1::InvalidQuantity)
        );
        let bytes = request().to_bytes();
        assert_eq!(
            SparseNativeTransferV1::decode(
                bytes
                    .get(..bytes.len() - 1)
                    .expect("one-byte-short request"),
            ),
            Err(SparseNativeTransferErrorV1::InvalidLength)
        );
        assert_eq!(
            SparseNativeTransferReceiptV1::new(request(), id(9), id(10), id(11), 11, 11, 12),
            Err(SparseNativeTransferErrorV1::InvalidRevision)
        );
    }

    #[test]
    fn reserved_and_receipt_substitution_refuse() {
        let mut bytes = request().to_bytes();
        bytes[RESERVED] = 1;
        assert_eq!(
            SparseNativeTransferV1::decode(&bytes),
            Err(SparseNativeTransferErrorV1::NonCanonical)
        );
        let receipt =
            SparseNativeTransferReceiptV1::new(request(), id(9), id(10), id(11), 10, 11, 12)
                .expect("receipt");
        let mut other = request().input();
        other.quantity = 14;
        assert_eq!(
            receipt.validate_request(SparseNativeTransferV1::new(other).expect("other")),
            Err(SparseNativeTransferErrorV1::ReceiptMismatch)
        );
    }
}
