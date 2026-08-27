//! Market-neutral Hot V6 lifecycle family request.
//!
//! V6 keeps the Claims-owned V2 child layout but mints a distinct family
//! identity. Its artifact geometry reserves separate identities for the
//! authenticated descriptor account and the request-carried descriptor field;
//! the transition must require them equal before any child effect executes.

use super::*;

/// Wallet-facing V6 family magic.
pub const RATIONAL_LIFECYCLE_HOT_MAGIC_V6: [u8; 8] = *b"DCRLHT06";
/// Wallet-facing V6 family version.
pub const RATIONAL_LIFECYCLE_HOT_VERSION_V6: u16 = 6;
/// Canonical V6 request-schema preimage.
pub const RATIONAL_LIFECYCLE_HOT_SCHEMA_PREIMAGE_V6: &[u8] =
    b"dclutch/schema/rational-lifecycle-hot-request-v6";
/// SHA-256 of [`RATIONAL_LIFECYCLE_HOT_SCHEMA_PREIMAGE_V6`].
pub const RATIONAL_LIFECYCLE_HOT_SCHEMA_RELEASE_ID_V6: [u8; 32] = [
    0xbe, 0xa7, 0x2d, 0x39, 0x52, 0x24, 0xd3, 0x60, 0x53, 0xc2, 0x5c, 0x79, 0xb3, 0x88, 0x45, 0xf0,
    0xd2, 0x8e, 0x97, 0x9d, 0x5f, 0xf5, 0xbd, 0xb8, 0xa5, 0xbe, 0x18, 0x39, 0x76, 0xa3, 0x8f, 0x4b,
];

/// Common identity containing the descriptor ID carried by the family request.
pub const RATIONAL_LIFECYCLE_IDENTITY_REQUEST_DESCRIPTOR_V6: usize = 10;
/// V6 common identity width: ten V3 identities plus request descriptor evidence.
pub const RATIONAL_LIFECYCLE_HOT_COMMON_IDENTITIES_V6: usize = 11;

/// V6 flat register geometry with a separate request-descriptor identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RationalLifecycleHotRegisterLayoutV6 {
    coordinate_count: usize,
}

impl RationalLifecycleHotRegisterLayoutV6 {
    /// Construct exact V6 geometry.
    pub const fn new(coordinate_count: usize) -> Self {
        Self { coordinate_count }
    }

    /// Exact common plus flattened coordinate identity width.
    pub const fn identity_count(self) -> Option<usize> {
        match self
            .coordinate_count
            .checked_mul(hot_v3::RATIONAL_LIFECYCLE_HOT_ITEM_IDENTITIES_V3)
        {
            Some(items) => RATIONAL_LIFECYCLE_HOT_COMMON_IDENTITIES_V6.checked_add(items),
            None => None,
        }
    }

    /// Exact common plus flattened coordinate scalar width.
    pub const fn scalar_count(self) -> Option<usize> {
        hot_v3::RationalLifecycleHotRegisterLayoutV3::new(self.coordinate_count).scalar_count()
    }

    /// V6 identity coordinate for one row field.
    pub const fn coordinate_identity(self, row: usize, field: usize) -> Option<usize> {
        if row >= self.coordinate_count
            || field >= hot_v3::RATIONAL_LIFECYCLE_HOT_ITEM_IDENTITIES_V3
        {
            return None;
        }
        match row.checked_mul(hot_v3::RATIONAL_LIFECYCLE_HOT_ITEM_IDENTITIES_V3) {
            Some(start) => match RATIONAL_LIFECYCLE_HOT_COMMON_IDENTITIES_V6.checked_add(start) {
                Some(base) => base.checked_add(field),
                None => None,
            },
            None => None,
        }
    }

    /// V6 scalar coordinate for one row field.
    pub const fn coordinate_scalar(self, row: usize, field: usize) -> Option<usize> {
        hot_v3::RationalLifecycleHotRegisterLayoutV3::new(self.coordinate_count)
            .coordinate_scalar(row, field)
    }
}

/// Borrowed canonical V6 family request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RationalLifecycleHotRequestV6<'a> {
    bytes: &'a [u8],
}

impl<'a> RationalLifecycleHotRequestV6<'a> {
    /// Hostile-decode exact V6 family bytes.
    pub fn decode(input: &'a [u8]) -> Result<Self> {
        if input.len() < LIFECYCLE_HEADER_BYTES_V2
            || input.get(..8) != Some(RATIONAL_LIFECYCLE_HOT_MAGIC_V6.as_slice())
            || read_u16(input, 8)? != RATIONAL_LIFECYCLE_HOT_VERSION_V6
            || input
                .get(PARENT_CONTEXT_OFFSET..PARENT_CONTEXT_OFFSET + 32)
                .ok_or(Error::InvalidLength)?
                .iter()
                .any(|byte| *byte != 0)
        {
            return Err(Error::NonCanonical);
        }
        let rows = usize::try_from(read_u32(input, COORDINATE_COUNT_OFFSET)?)
            .map_err(|_| Error::InvalidLength)?;
        let expected = LIFECYCLE_HEADER_BYTES_V2
            .checked_add(
                rows.checked_mul(LIFECYCLE_COORDINATE_BYTES_V2)
                    .ok_or(Error::InvalidLength)?,
            )
            .ok_or(Error::InvalidLength)?;
        if input.len() != expected {
            return Err(Error::InvalidLength);
        }
        let mut header = [0_u8; LIFECYCLE_HEADER_BYTES_V2];
        header.copy_from_slice(
            input
                .get(..LIFECYCLE_HEADER_BYTES_V2)
                .ok_or(Error::InvalidLength)?,
        );
        put(&mut header, 0, &LIFECYCLE_REQUEST_MAGIC_V2)?;
        put(&mut header, 8, &LIFECYCLE_VERSION_V2.to_le_bytes())?;
        put(&mut header, PARENT_CONTEXT_OFFSET, &[1; 32])?;
        let decoded = LifecycleHeaderV2::decode(&header)?;
        if decoded.parent_context != [1; 32] {
            return Err(Error::NonCanonical);
        }
        Ok(Self { bytes: input })
    }

    /// Project a canonical Claims child into V6 family form.
    pub fn from_child_into<'b>(
        child: LifecycleRequestV2<'_>,
        output: &'b mut [u8],
    ) -> Result<RationalLifecycleHotRequestV6<'b>> {
        child.encode_into(output)?;
        put(output, 0, &RATIONAL_LIFECYCLE_HOT_MAGIC_V6)?;
        put(output, 8, &RATIONAL_LIFECYCLE_HOT_VERSION_V6.to_le_bytes())?;
        output
            .get_mut(PARENT_CONTEXT_OFFSET..PARENT_CONTEXT_OFFSET + 32)
            .ok_or(Error::InvalidLength)?
            .fill(0);
        RationalLifecycleHotRequestV6::decode(output)
    }

    /// Specialize V6 into the sole Claims lifecycle child.
    pub fn specialize_child_into<'b>(
        self,
        family_digest: [u8; 32],
        output: &'b mut [u8],
    ) -> Result<LifecycleRequestV2<'b>> {
        if is_zero(&family_digest) || output.len() != self.bytes.len() {
            return Err(Error::InvalidIdentity);
        }
        output.copy_from_slice(self.bytes);
        put(output, 0, &LIFECYCLE_REQUEST_MAGIC_V2)?;
        put(output, 8, &LIFECYCLE_VERSION_V2.to_le_bytes())?;
        put(output, PARENT_CONTEXT_OFFSET, &family_digest)?;
        LifecycleRequestV2::decode(output)
    }

    /// Exact borrowed family bytes.
    pub const fn as_bytes(self) -> &'a [u8] {
        self.bytes
    }
}

fn read_u16(input: &[u8], offset: usize) -> Result<u16> {
    let bytes: [u8; 2] = input
        .get(offset..offset + 2)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)?;
    Ok(u16::from_le_bytes(bytes))
}

fn read_u32(input: &[u8], offset: usize) -> Result<u32> {
    let bytes: [u8; 4] = input
        .get(offset..offset + 4)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)?;
    Ok(u32::from_le_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn child() -> [u8; LIFECYCLE_HEADER_BYTES_V2] {
        let request = LifecycleRequestV2::new(
            LifecycleHeaderV2 {
                action: LifecycleActionV2::ActivateReceipt,
                release_set: id(1),
                market: id(2),
                graph_id: id(3),
                descriptor_id: id(4),
                parent_context: id(5),
                representation_authority: id(6),
                receipt_mint: id(7),
                token_program: TOKEN_2022_PROGRAM_ID,
                rent_credit: id(8),
                rent_program: id(9),
                generation: 1,
                expected_claims_market_revision: 0,
                observed_receipt_lamports: 1,
                receipt_rent_principal: 1,
                expected_receipt_supply: 0,
                outcome_count: 3,
                coordinate_count: 0,
                rent_credit_before: 1,
                rent_credit_after: 1,
            },
            &[],
        )
        .expect("child");
        let mut bytes = [0_u8; LIFECYCLE_HEADER_BYTES_V2];
        request.encode_into(&mut bytes).expect("encode child");
        bytes
    }

    #[test]
    fn v6_is_distinct_and_specializes_exactly() {
        let child_bytes = child();
        let child = LifecycleRequestV2::decode(&child_bytes).expect("child");
        let mut family_bytes = [0_u8; LIFECYCLE_HEADER_BYTES_V2];
        let family = RationalLifecycleHotRequestV6::from_child_into(child, &mut family_bytes)
            .expect("V6 family");
        assert_eq!(
            family.as_bytes().get(..8),
            Some(RATIONAL_LIFECYCLE_HOT_MAGIC_V6.as_slice())
        );
        assert_ne!(
            RATIONAL_LIFECYCLE_HOT_SCHEMA_RELEASE_ID_V6,
            hot_v3::RATIONAL_LIFECYCLE_HOT_SCHEMA_RELEASE_ID_V3
        );
        let mut specialized = [0_u8; LIFECYCLE_HEADER_BYTES_V2];
        let specialized = family
            .specialize_child_into(id(10), &mut specialized)
            .expect("specialized child");
        assert_eq!(specialized.header().descriptor_id, id(4));
        assert_eq!(specialized.header().parent_context, id(10));
        let mut hostile = family_bytes;
        hostile[7] = hot_v3::RATIONAL_LIFECYCLE_HOT_MAGIC_V3[7];
        assert!(RationalLifecycleHotRequestV6::decode(&hostile).is_err());
    }
}
