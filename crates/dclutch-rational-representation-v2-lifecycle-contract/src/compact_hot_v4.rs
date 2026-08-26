//! Compact descriptor-support-derived receipt-retirement request.
//!
//! The wallet carries only the fixed lifecycle header. It neither asserts the
//! descriptor's sparse support width nor repeats coordinate DTOs already owned
//! by the finalized descriptor. The authenticated Hot adapter derives the
//! positive support width and ordered nonzero rows before specializing the sole
//! Claims [`LifecycleRequestV2`] child.

use super::*;

/// Exact compact family request width.
pub const RATIONAL_LIFECYCLE_COMPACT_HOT_REQUEST_BYTES_V4: usize = LIFECYCLE_HEADER_BYTES_V2;
/// Compact RetireReceipt family magic.
pub const RATIONAL_LIFECYCLE_COMPACT_HOT_MAGIC_V4: [u8; 8] = *b"DCRLHC04";
/// Compact RetireReceipt family version.
pub const RATIONAL_LIFECYCLE_COMPACT_HOT_VERSION_V4: u16 = 4;
/// Canonical compact request-schema preimage.
pub const RATIONAL_LIFECYCLE_COMPACT_HOT_SCHEMA_PREIMAGE_V4: &[u8] =
    b"dclutch/schema/rational-lifecycle-compact-hot-request-v4";
/// SHA-256 of [`RATIONAL_LIFECYCLE_COMPACT_HOT_SCHEMA_PREIMAGE_V4`].
pub const RATIONAL_LIFECYCLE_COMPACT_HOT_SCHEMA_RELEASE_ID_V4: [u8; 32] = [
    0xb8, 0x38, 0x14, 0x7c, 0x37, 0x47, 0xa7, 0x75, 0x10, 0x67, 0x56, 0xc4, 0xa6, 0x53, 0xa6, 0xc3,
    0xa8, 0x48, 0x01, 0x4c, 0xad, 0x77, 0x87, 0x60, 0xb8, 0x9a, 0x5a, 0x16, 0x95, 0xa2, 0x83, 0x74,
];

/// Canonical field coordinates shared with the sole Claims child header.
pub struct RationalLifecycleCompactHotLayoutV4;

impl RationalLifecycleCompactHotLayoutV4 {
    /// Magic offset.
    pub const MAGIC: usize = 0;
    /// Version offset.
    pub const VERSION: usize = 8;
    /// Action offset.
    pub const ACTION: usize = ACTION_OFFSET;
    /// Reserved fixed-header offset.
    pub const RESERVED_HEADER: usize = HEADER_RESERVED_OFFSET;
    /// Selected release-set offset.
    pub const RELEASE_SET: usize = RELEASE_SET_OFFSET;
    /// Logical Market offset.
    pub const MARKET: usize = MARKET_OFFSET;
    /// Representation graph offset.
    pub const GRAPH_ID: usize = GRAPH_ID_OFFSET;
    /// Finalized descriptor offset.
    pub const DESCRIPTOR_ID: usize = DESCRIPTOR_ID_OFFSET;
    /// Family-zero/child-digest parent-context offset.
    pub const PARENT_CONTEXT: usize = PARENT_CONTEXT_OFFSET;
    /// Representation-authority offset.
    pub const REPRESENTATION_AUTHORITY: usize = REPRESENTATION_AUTHORITY_OFFSET;
    /// Receipt-Mint offset.
    pub const RECEIPT_MINT: usize = RECEIPT_MINT_OFFSET;
    /// Token-2022 program offset.
    pub const TOKEN_PROGRAM: usize = TOKEN_PROGRAM_OFFSET;
    /// RentCredit offset.
    pub const RENT_CREDIT: usize = RENT_CREDIT_OFFSET;
    /// RentCredit owner-program offset.
    pub const RENT_PROGRAM: usize = RENT_PROGRAM_OFFSET;
    /// Market-generation offset.
    pub const GENERATION: usize = GENERATION_OFFSET;
    /// Expected Claims Market revision offset.
    pub const EXPECTED_MARKET_REVISION: usize = EXPECTED_MARKET_REVISION_OFFSET;
    /// Observed receipt-Mint lamports offset.
    pub const OBSERVED_RECEIPT_LAMPORTS: usize = OBSERVED_RECEIPT_LAMPORTS_OFFSET;
    /// Receipt-Mint rent-principal offset.
    pub const RECEIPT_RENT_PRINCIPAL: usize = RECEIPT_RENT_PRINCIPAL_OFFSET;
    /// Expected receipt-Mint supply offset.
    pub const EXPECTED_RECEIPT_SUPPLY: usize = EXPECTED_RECEIPT_SUPPLY_OFFSET;
    /// Product-owned outcome width offset.
    pub const OUTCOME_COUNT: usize = OUTCOME_COUNT_OFFSET;
    /// Canonical zero coordinate-count offset on the compact family wire.
    pub const COORDINATE_COUNT: usize = COORDINATE_COUNT_OFFSET;
    /// RentCredit pre-lamports offset.
    pub const RENT_CREDIT_BEFORE: usize = RENT_CREDIT_BEFORE_OFFSET;
    /// Exact RentCredit post-lamports offset.
    pub const RENT_CREDIT_AFTER: usize = RENT_CREDIT_AFTER_OFFSET;
}

/// Borrowed exact compact RetireReceipt family request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RationalLifecycleCompactHotRequestV4<'a> {
    bytes: &'a [u8],
}

impl<'a> RationalLifecycleCompactHotRequestV4<'a> {
    /// Hostile-decode one exact fixed-width compact family request.
    pub fn decode(input: &'a [u8]) -> Result<Self> {
        validate_compact(input)?;
        Ok(Self { bytes: input })
    }

    /// Project one canonical Claims child header after authenticated K derivation.
    ///
    /// `coordinate_count` must be the positive descriptor support width
    /// protected by AccountProfile/Transition; it never comes from these
    /// family bytes. The caller-owned output changes only after every check
    /// succeeds.
    pub fn specialize_child_header_into(
        self,
        family_digest: [u8; 32],
        coordinate_count: u32,
        output: &mut [u8; LIFECYCLE_HEADER_BYTES_V2],
    ) -> Result<LifecycleHeaderV2> {
        if is_zero(&family_digest) || coordinate_count == 0 {
            return Err(Error::InvalidSupport);
        }
        let header = specialized_header(self.bytes, family_digest, coordinate_count)?;
        LifecycleRequestV2 {
            header,
            coordinate_bytes: &[],
        }
        .validate_header()?;
        let mut candidate = [0_u8; LIFECYCLE_HEADER_BYTES_V2];
        header.encode_into(&mut candidate)?;
        output.copy_from_slice(&candidate);
        Ok(header)
    }

    /// Encode one chain-observed fixed header into the compact family form.
    ///
    /// The input must already select RetireReceipt with no caller-owned row
    /// count. Parent context is erased because Hot binds the digest of these
    /// exact output bytes into the child.
    pub fn from_header_into(
        header: LifecycleHeaderV2,
        output: &'a mut [u8; LIFECYCLE_HEADER_BYTES_V2],
    ) -> Result<Self> {
        if header.action != LifecycleActionV2::RetireReceipt || header.coordinate_count != 0 {
            return Err(Error::NonCanonical);
        }
        let mut candidate = [0_u8; LIFECYCLE_HEADER_BYTES_V2];
        header.encode_into(&mut candidate)?;
        put(&mut candidate, 0, &RATIONAL_LIFECYCLE_COMPACT_HOT_MAGIC_V4)?;
        put(
            &mut candidate,
            8,
            &RATIONAL_LIFECYCLE_COMPACT_HOT_VERSION_V4.to_le_bytes(),
        )?;
        candidate
            .get_mut(PARENT_CONTEXT_OFFSET..PARENT_CONTEXT_OFFSET + 32)
            .ok_or(Error::InvalidLength)?
            .fill(0);
        validate_compact(&candidate)?;
        output.copy_from_slice(&candidate);
        Ok(Self { bytes: output })
    }

    /// Exact family bytes whose SHA-256 becomes child parent context.
    pub const fn as_bytes(self) -> &'a [u8] {
        self.bytes
    }
}

fn validate_compact(input: &[u8]) -> Result<()> {
    if input.len() != RATIONAL_LIFECYCLE_COMPACT_HOT_REQUEST_BYTES_V4 {
        return Err(Error::InvalidLength);
    }
    exact(input, 0, &RATIONAL_LIFECYCLE_COMPACT_HOT_MAGIC_V4)?;
    if read_u16(input, 8)? != RATIONAL_LIFECYCLE_COMPACT_HOT_VERSION_V4 {
        return Err(Error::InvalidHeader);
    }
    require_zero(input, PARENT_CONTEXT_OFFSET, 32)?;
    if read_byte(input, ACTION_OFFSET)? != LifecycleActionV2::RetireReceipt.tag()
        || read_u32(input, COORDINATE_COUNT_OFFSET)? != 0
    {
        return Err(Error::NonCanonical);
    }
    let header = specialized_header(input, [1; 32], 0)?;
    LifecycleRequestV2 {
        header,
        coordinate_bytes: &[],
    }
    .validate_header()?;
    Ok(())
}

fn specialized_header(
    input: &[u8],
    family_digest: [u8; 32],
    coordinate_count: u32,
) -> Result<LifecycleHeaderV2> {
    let mut candidate = [0_u8; LIFECYCLE_HEADER_BYTES_V2];
    candidate.copy_from_slice(input);
    put(&mut candidate, 0, &LIFECYCLE_REQUEST_MAGIC_V2)?;
    put(&mut candidate, 8, &LIFECYCLE_VERSION_V2.to_le_bytes())?;
    put(&mut candidate, PARENT_CONTEXT_OFFSET, &family_digest)?;
    put(
        &mut candidate,
        COORDINATE_COUNT_OFFSET,
        &coordinate_count.to_le_bytes(),
    )?;
    LifecycleHeaderV2::decode(&candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn header() -> LifecycleHeaderV2 {
        LifecycleHeaderV2 {
            action: LifecycleActionV2::RetireReceipt,
            release_set: id(1),
            market: id(2),
            graph_id: id(3),
            descriptor_id: id(4),
            parent_context: id(5),
            representation_authority: id(6),
            receipt_mint: id(7),
            token_program: TOKEN_2022_PROGRAM_ID,
            rent_credit: id(9),
            rent_program: id(10),
            generation: 11,
            expected_claims_market_revision: 12,
            observed_receipt_lamports: 20,
            receipt_rent_principal: 10,
            expected_receipt_supply: 0,
            outcome_count: 258,
            coordinate_count: 0,
            rent_credit_before: 100,
            rent_credit_after: 110,
        }
    }

    fn family_bytes() -> [u8; LIFECYCLE_HEADER_BYTES_V2] {
        let mut bytes = [0_u8; LIFECYCLE_HEADER_BYTES_V2];
        RationalLifecycleCompactHotRequestV4::from_header_into(header(), &mut bytes)
            .expect("compact family");
        bytes
    }

    #[test]
    fn compact_family_specializes_only_after_positive_support_derivation() {
        let bytes = family_bytes();
        let request = RationalLifecycleCompactHotRequestV4::decode(&bytes).expect("decode");
        let mut child = [0_u8; LIFECYCLE_HEADER_BYTES_V2];
        let specialized = request
            .specialize_child_header_into(id(21), 3, &mut child)
            .expect("specialized child");
        assert_eq!(specialized.action, LifecycleActionV2::RetireReceipt);
        assert_eq!(specialized.parent_context, id(21));
        assert_eq!(specialized.coordinate_count, 3);
        assert_eq!(&child[..8], &LIFECYCLE_REQUEST_MAGIC_V2);
        assert_eq!(read_u32(&child, COORDINATE_COUNT_OFFSET), Ok(3));
        assert_eq!(
            request.specialize_child_header_into(id(21), 0, &mut child),
            Err(Error::InvalidSupport),
        );
    }

    #[test]
    fn caller_cannot_assert_support_width_parent_or_action() {
        let canonical = family_bytes();
        for (offset, value) in [
            (COORDINATE_COUNT_OFFSET, 1_u8),
            (PARENT_CONTEXT_OFFSET, 1_u8),
            (ACTION_OFFSET, LifecycleActionV2::ActivateReceipt.tag()),
        ] {
            let mut hostile = canonical;
            assert!(hostile.get_mut(offset).map(|byte| *byte = value).is_some());
            assert!(RationalLifecycleCompactHotRequestV4::decode(&hostile).is_err());
        }
        let mut padded = [0_u8; LIFECYCLE_HEADER_BYTES_V2 + 1];
        padded[..LIFECYCLE_HEADER_BYTES_V2].copy_from_slice(&canonical);
        assert_eq!(
            RationalLifecycleCompactHotRequestV4::decode(&padded),
            Err(Error::InvalidLength),
        );
    }

    #[test]
    fn schema_id_is_pinned_to_exact_preimage() {
        use sha2::{Digest, Sha256};

        assert_eq!(
            Sha256::digest(RATIONAL_LIFECYCLE_COMPACT_HOT_SCHEMA_PREIMAGE_V4).as_slice(),
            RATIONAL_LIFECYCLE_COMPACT_HOT_SCHEMA_RELEASE_ID_V4,
        );
    }
}
