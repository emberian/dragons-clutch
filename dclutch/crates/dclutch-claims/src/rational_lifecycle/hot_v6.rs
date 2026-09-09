//! Market-neutral Hot V6 lifecycle family request.
//!
//! V6 keeps the Claims-owned V2 child layout but mints a distinct family
//! identity. The descriptor field is a body digest passed to the existing
//! native Claims finalized-record admission; Registry account keys never
//! substitute for content identities in the artifact register bank.

use super::*;

/// Reserved selectors in a unified Structured selected ProgramSet.
pub const STRUCTURED_ACTIVATE_RECEIPT_SELECTOR_V1: u32 = 6;
/// Reserved selectors in a unified Structured selected ProgramSet.
pub const STRUCTURED_ACTIVATE_COORDINATE_SELECTOR_V1: u32 = 7;
/// Selected single-coordinate retirement in the same Structured release.
pub const STRUCTURED_RETIRE_COORDINATE_SELECTOR_V1: u32 = 8;
/// Selected complete-support receipt retirement in the same Structured release.
pub const STRUCTURED_RETIRE_RECEIPT_SELECTOR_V1: u32 = 9;

/// Dynamic retirement transport magic; Claims V2 remains the semantic wire.
pub const DYNAMIC_RETIREMENT_MAGIC_V1: [u8; 8] = *b"DCRLDT01";
/// Exact transport prefix before the borrowed compact lifecycle header.
pub const DYNAMIC_RETIREMENT_PREFIX_BYTES_V1: usize = 24;

/// Validate the transport of one compact receipt-retirement header. The span
/// count is only a byte/account routing hint; Claims derives and checks support.
pub fn validate_dynamic_retirement_v1(
    input: &[u8],
) -> Result<compact_hot_v4::RationalLifecycleCompactHotRequestV4<'_>> {
    if input.get(..8) != Some(DYNAMIC_RETIREMENT_MAGIC_V1.as_slice())
        || read_u16(input, 8)? != 1
        || read_byte(input, 10)?
            != u8::try_from(STRUCTURED_RETIRE_RECEIPT_SELECTOR_V1)
                .map_err(|_| Error::InvalidHeader)?
        || read_byte(input, 11)? != 0
        || read_u32(input, 20)? != 0
    {
        return Err(Error::InvalidHeader);
    }
    let accounts = read_u32(input, 12)?;
    let stride =
        u32::try_from(LIFECYCLE_VACANCY_ACCOUNT_COUNT_V2).map_err(|_| Error::InvalidLength)?;
    if accounts == 0
        || accounts % stride != 0
        || usize::try_from(read_u32(input, 16)?).map_err(|_| Error::InvalidLength)?
            != LIFECYCLE_HEADER_BYTES_V2
    {
        return Err(Error::InvalidLength);
    }
    compact_hot_v4::RationalLifecycleCompactHotRequestV4::decode(
        input
            .get(DYNAMIC_RETIREMENT_PREFIX_BYTES_V1..)
            .ok_or(Error::InvalidLength)?,
    )
}

/// Encode a fixed compact header and an untrusted physical span-count hint.
pub fn encode_dynamic_retirement_v1(
    child: compact_hot_v4::RationalLifecycleCompactHotRequestV4<'_>,
    support_count: u32,
    output: &mut [u8],
) -> Result<()> {
    if output.len() != DYNAMIC_RETIREMENT_PREFIX_BYTES_V1 + LIFECYCLE_HEADER_BYTES_V2
        || support_count == 0
    {
        return Err(Error::InvalidLength);
    }
    let accounts = support_count
        .checked_mul(
            u32::try_from(LIFECYCLE_VACANCY_ACCOUNT_COUNT_V2).map_err(|_| Error::InvalidLength)?,
        )
        .ok_or(Error::InvalidLength)?;
    output.fill(0);
    put(output, 0, &DYNAMIC_RETIREMENT_MAGIC_V1)?;
    put(output, 8, &1_u16.to_le_bytes())?;
    put(
        output,
        10,
        &[
            u8::try_from(STRUCTURED_RETIRE_RECEIPT_SELECTOR_V1)
                .map_err(|_| Error::InvalidHeader)?,
        ],
    )?;
    put(output, 12, &accounts.to_le_bytes())?;
    put(
        output,
        16,
        &u32::try_from(LIFECYCLE_HEADER_BYTES_V2)
            .map_err(|_| Error::InvalidLength)?
            .to_le_bytes(),
    )?;
    put(output, DYNAMIC_RETIREMENT_PREFIX_BYTES_V1, child.as_bytes())?;
    validate_dynamic_retirement_v1(output)?;
    Ok(())
}

/// Map one typed lifecycle action under the authenticated Structured kind to
/// its reserved selected-table selector.
#[must_use]
pub fn structured_lifecycle_action_selector_v1(
    capability_kind: [u8; 32],
    action: LifecycleActionV2,
) -> Option<u32> {
    if capability_kind != crate::structured_kernel::STRUCTURED_CAPABILITY_KIND_ID_V2 {
        return None;
    }
    match action {
        LifecycleActionV2::ActivateReceipt => Some(STRUCTURED_ACTIVATE_RECEIPT_SELECTOR_V1),
        LifecycleActionV2::ActivateCoordinate => Some(STRUCTURED_ACTIVATE_COORDINATE_SELECTOR_V1),
        LifecycleActionV2::RetireCoordinate => Some(STRUCTURED_RETIRE_COORDINATE_SELECTOR_V1),
        LifecycleActionV2::RetireReceipt => Some(STRUCTURED_RETIRE_RECEIPT_SELECTOR_V1),
    }
}

/// Map a typed V6 lifecycle request under the authenticated
/// Structured capability kind to its ProgramSet selector.  The family and
/// Claims wires retain their canonical action tags.
#[must_use]
pub fn structured_lifecycle_selector_v1(
    capability_kind: [u8; 32],
    family_request: &[u8],
) -> Option<u32> {
    if capability_kind != crate::structured_kernel::STRUCTURED_CAPABILITY_KIND_ID_V2
        || family_request.get(..8) != Some(RATIONAL_LIFECYCLE_HOT_MAGIC_V6.as_slice())
    {
        return None;
    }
    let request = RationalLifecycleHotRequestV6::decode(family_request).ok()?;
    structured_lifecycle_action_selector_v1(
        capability_kind,
        LifecycleActionV2::decode(read_byte(request.bytes, ACTION_OFFSET).ok()?).ok()?,
    )
}

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

/// V6 transports the canonical Claims register geometry. The request carries
/// the descriptor body digest; Claims authenticates its finalized raw record,
/// staging cursor and body. A raw Registry PDA is not that digest.
pub type RationalLifecycleHotRegisterLayoutV6 = hot_v3::RationalLifecycleHotRegisterLayoutV3;

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

    #[test]
    fn structured_selector_normalizes_authenticated_lifecycle_wires() {
        let child_bytes = child();
        let child = LifecycleRequestV2::decode(&child_bytes).expect("child");
        let mut receipt = [0_u8; LIFECYCLE_HEADER_BYTES_V2];
        RationalLifecycleHotRequestV6::from_child_into(child, &mut receipt).expect("receipt");
        assert_eq!(
            structured_lifecycle_selector_v1(
                crate::structured_kernel::STRUCTURED_CAPABILITY_KIND_ID_V2,
                &receipt,
            ),
            Some(STRUCTURED_ACTIVATE_RECEIPT_SELECTOR_V1)
        );

        let mut coordinate = receipt;
        coordinate[ACTION_OFFSET] = LifecycleActionV2::ActivateCoordinate.tag();
        assert_eq!(
            structured_lifecycle_selector_v1(
                crate::structured_kernel::STRUCTURED_CAPABILITY_KIND_ID_V2,
                &coordinate,
            ),
            Some(STRUCTURED_ACTIVATE_COORDINATE_SELECTOR_V1)
        );

        assert_eq!(structured_lifecycle_selector_v1(id(99), &receipt), None);
        let mut wrong_magic = receipt;
        wrong_magic[0] ^= 1;
        assert_eq!(
            structured_lifecycle_selector_v1(
                crate::structured_kernel::STRUCTURED_CAPABILITY_KIND_ID_V2,
                &wrong_magic,
            ),
            None
        );
        let mut retirement = receipt;
        retirement[ACTION_OFFSET] = LifecycleActionV2::RetireReceipt.tag();
        assert_eq!(
            structured_lifecycle_selector_v1(
                crate::structured_kernel::STRUCTURED_CAPABILITY_KIND_ID_V2,
                &retirement,
            ),
            Some(STRUCTURED_RETIRE_RECEIPT_SELECTOR_V1)
        );
        retirement[ACTION_OFFSET] = LifecycleActionV2::RetireCoordinate.tag();
        assert_eq!(
            structured_lifecycle_selector_v1(
                crate::structured_kernel::STRUCTURED_CAPABILITY_KIND_ID_V2,
                &retirement,
            ),
            Some(STRUCTURED_RETIRE_COORDINATE_SELECTOR_V1)
        );
        let mut unknown = receipt;
        unknown[ACTION_OFFSET] = 255;
        assert_eq!(
            structured_lifecycle_selector_v1(
                crate::structured_kernel::STRUCTURED_CAPABILITY_KIND_ID_V2,
                &unknown,
            ),
            None
        );
    }
}
