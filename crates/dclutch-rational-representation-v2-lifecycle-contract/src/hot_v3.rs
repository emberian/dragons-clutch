//! Runtime-width Hot V3 family specialization for Rational resource lifecycle.
//!
//! The wallet-facing family bytes differ from the canonical Claims child only
//! in magic, version, and an all-zero parent-context slot. The authenticated
//! Hot outer hashes the complete family bytes and specializes that digest into
//! the sole [`LifecycleRequestV2`] ABI. No lifecycle or resource fact is
//! persisted in this projection layer.

use super::*;

/// Wallet-facing runtime-width family magic.
pub const RATIONAL_LIFECYCLE_HOT_MAGIC_V3: [u8; 8] = *b"DCRLHT03";
/// Wallet-facing family version.
pub const RATIONAL_LIFECYCLE_HOT_VERSION_V3: u16 = 3;
/// Canonical request-schema preimage.
pub const RATIONAL_LIFECYCLE_HOT_SCHEMA_PREIMAGE_V3: &[u8] =
    b"dclutch/schema/rational-lifecycle-hot-request-v3";
/// SHA-256 of [`RATIONAL_LIFECYCLE_HOT_SCHEMA_PREIMAGE_V3`].
pub const RATIONAL_LIFECYCLE_HOT_SCHEMA_RELEASE_ID_V3: [u8; 32] = [
    0xe1, 0x31, 0xe1, 0x90, 0xe3, 0xcc, 0x2f, 0x5f, 0x31, 0x42, 0xee, 0xe5, 0x58, 0x1d, 0xc1, 0x40,
    0x0d, 0x7f, 0x32, 0x6f, 0xf7, 0xb9, 0x15, 0xef, 0x36, 0x68, 0x7d, 0xc6, 0x2f, 0x97, 0x89, 0xcd,
];

/// Exact common identity-register width.
pub const RATIONAL_LIFECYCLE_HOT_COMMON_IDENTITIES_V3: usize = 10;
/// Exact common scalar-register width.
pub const RATIONAL_LIFECYCLE_HOT_COMMON_SCALARS_V3: usize = 11;
/// Exact identity registers per coordinate row.
pub const RATIONAL_LIFECYCLE_HOT_ITEM_IDENTITIES_V3: usize = 5;
/// Exact scalar registers per coordinate row.
pub const RATIONAL_LIFECYCLE_HOT_ITEM_SCALARS_V3: usize = 13;

/// Canonical byte layout for lifecycle Hot family requests.
///
/// Fixed-header offsets delegate the sole lifecycle request ABI. Coordinate
/// offsets are relative to one exact item row so RequestProfile and
/// EffectProgram artifacts can use fixed and item request banks without
/// copying numeric wire facts.
pub struct RationalLifecycleHotLayoutV3;

impl RationalLifecycleHotLayoutV3 {
    /// Exact fixed request-bank width.
    pub const FIXED_BYTES: usize = LIFECYCLE_HEADER_BYTES_V2;
    /// Exact item request-bank stride.
    pub const ITEM_BYTES: usize = LIFECYCLE_COORDINATE_BYTES_V2;
    /// Magic offset in the fixed request bank.
    pub const MAGIC: usize = 0;
    /// Version offset in the fixed request bank.
    pub const VERSION: usize = 8;
    /// Action offset in the fixed request bank.
    pub const ACTION: usize = ACTION_OFFSET;
    /// Reserved-header offset in the fixed request bank.
    pub const RESERVED_HEADER: usize = HEADER_RESERVED_OFFSET;
    /// Selected release-set offset.
    pub const RELEASE_SET: usize = RELEASE_SET_OFFSET;
    /// Logical Market offset.
    pub const MARKET: usize = MARKET_OFFSET;
    /// Representation-graph offset.
    pub const GRAPH_ID: usize = GRAPH_ID_OFFSET;
    /// Finalized descriptor offset.
    pub const DESCRIPTOR_ID: usize = DESCRIPTOR_ID_OFFSET;
    /// Zero family parent-context offset.
    pub const PARENT_CONTEXT: usize = PARENT_CONTEXT_OFFSET;
    /// Derived representation-authority offset.
    pub const REPRESENTATION_AUTHORITY: usize = REPRESENTATION_AUTHORITY_OFFSET;
    /// Closeable receipt-Mint offset.
    pub const RECEIPT_MINT: usize = RECEIPT_MINT_OFFSET;
    /// Token-2022 program offset.
    pub const TOKEN_PROGRAM: usize = TOKEN_PROGRAM_OFFSET;
    /// RentCredit offset.
    pub const RENT_CREDIT: usize = RENT_CREDIT_OFFSET;
    /// RentCredit owner-program offset.
    pub const RENT_PROGRAM: usize = RENT_PROGRAM_OFFSET;
    /// Market-generation offset.
    pub const GENERATION: usize = GENERATION_OFFSET;
    /// Expected Claims Market-revision offset.
    pub const EXPECTED_MARKET_REVISION: usize = EXPECTED_MARKET_REVISION_OFFSET;
    /// Observed receipt-Mint lamports offset.
    pub const OBSERVED_RECEIPT_LAMPORTS: usize = OBSERVED_RECEIPT_LAMPORTS_OFFSET;
    /// Receipt-Mint rent-principal offset.
    pub const RECEIPT_RENT_PRINCIPAL: usize = RECEIPT_RENT_PRINCIPAL_OFFSET;
    /// Expected receipt-Mint supply offset.
    pub const EXPECTED_RECEIPT_SUPPLY: usize = EXPECTED_RECEIPT_SUPPLY_OFFSET;
    /// Runtime outcome-count offset.
    pub const OUTCOME_COUNT: usize = OUTCOME_COUNT_OFFSET;
    /// Runtime coordinate-count offset.
    pub const COORDINATE_COUNT: usize = COORDINATE_COUNT_OFFSET;
    /// RentCredit pre-lamports offset.
    pub const RENT_CREDIT_BEFORE: usize = RENT_CREDIT_BEFORE_OFFSET;
    /// RentCredit post-lamports offset.
    pub const RENT_CREDIT_AFTER: usize = RENT_CREDIT_AFTER_OFFSET;

    /// Coordinate outcome offset within one item request bank.
    pub const ITEM_OUTCOME: usize = ROW_OUTCOME_OFFSET;
    /// Coordinate reserved-head offset within one item request bank.
    pub const ITEM_RESERVED_HEAD: usize = ROW_RESERVED_HEAD_OFFSET;
    /// Coordinate coefficient offset within one item request bank.
    pub const ITEM_COEFFICIENT: usize = ROW_COEFFICIENT_OFFSET;
    /// Coordinate shard-Mint offset within one item request bank.
    pub const ITEM_SHARD_MINT: usize = ROW_SHARD_MINT_OFFSET;
    /// Coordinate Structured-custody offset within one item request bank.
    pub const ITEM_STRUCTURED_CUSTODY: usize = ROW_STRUCTURED_CUSTODY_OFFSET;
    /// Coordinate Claims custody-owner offset within one item request bank.
    pub const ITEM_CUSTODY_OWNER: usize = ROW_CUSTODY_OWNER_OFFSET;
    /// Coordinate Claims Position offset within one item request bank.
    pub const ITEM_CUSTODY_POSITION: usize = ROW_CUSTODY_POSITION_OFFSET;
    /// Coordinate admission offset within one item request bank.
    pub const ITEM_POSITION_ADMISSION: usize = ROW_POSITION_ADMISSION_OFFSET;
    /// Observed shard-Mint lamports offset within one item request bank.
    pub const ITEM_SHARD_LAMPORTS: usize = ROW_SHARD_LAMPORTS_OFFSET;
    /// Observed Structured-custody lamports offset within one item request bank.
    pub const ITEM_STRUCTURED_LAMPORTS: usize = ROW_STRUCTURED_LAMPORTS_OFFSET;
    /// Observed Position lamports offset within one item request bank.
    pub const ITEM_POSITION_LAMPORTS: usize = ROW_POSITION_LAMPORTS_OFFSET;
    /// Observed admission lamports offset within one item request bank.
    pub const ITEM_ADMISSION_LAMPORTS: usize = ROW_ADMISSION_LAMPORTS_OFFSET;
    /// Shard-Mint rent-principal offset within one item request bank.
    pub const ITEM_SHARD_RENT: usize = ROW_SHARD_RENT_OFFSET;
    /// Structured-custody rent-principal offset within one item request bank.
    pub const ITEM_STRUCTURED_RENT: usize = ROW_STRUCTURED_RENT_OFFSET;
    /// Position rent-principal offset within one item request bank.
    pub const ITEM_POSITION_RENT: usize = ROW_POSITION_RENT_OFFSET;
    /// Admission rent-principal offset within one item request bank.
    pub const ITEM_ADMISSION_RENT: usize = ROW_ADMISSION_RENT_OFFSET;
    /// Expected shard supply offset within one item request bank.
    pub const ITEM_SHARD_SUPPLY: usize = ROW_SHARD_SUPPLY_OFFSET;
    /// Expected Structured-custody amount offset within one item request bank.
    pub const ITEM_STRUCTURED_AMOUNT: usize = ROW_STRUCTURED_AMOUNT_OFFSET;
    /// Expected Position revision offset within one item request bank.
    pub const ITEM_POSITION_REVISION: usize = ROW_POSITION_REVISION_OFFSET;
    /// Reserved-tail offset within one item request bank.
    pub const ITEM_RESERVED_TAIL: usize = ROW_RESERVED_TAIL_OFFSET;

    /// Exact complete request width for `coordinate_count` rows.
    pub const fn request_bytes(coordinate_count: usize) -> Option<usize> {
        match coordinate_count.checked_mul(Self::ITEM_BYTES) {
            Some(items) => Self::FIXED_BYTES.checked_add(items),
            None => None,
        }
    }
}

/// Common identity containing SHA-256 of the complete family request.
pub const RATIONAL_LIFECYCLE_IDENTITY_PARENT_DIGEST_V3: usize = 0;
/// Common identity containing the selected release set.
pub const RATIONAL_LIFECYCLE_IDENTITY_RELEASE_SET_V3: usize = 1;
/// Common identity containing the logical Core Market.
pub const RATIONAL_LIFECYCLE_IDENTITY_MARKET_V3: usize = 2;
/// Common identity containing the semantic representation graph.
pub const RATIONAL_LIFECYCLE_IDENTITY_GRAPH_V3: usize = 3;
/// Common identity containing the finalized descriptor digest.
pub const RATIONAL_LIFECYCLE_IDENTITY_DESCRIPTOR_V3: usize = 4;
/// Common identity containing the derived representation authority.
pub const RATIONAL_LIFECYCLE_IDENTITY_REPRESENTATION_AUTHORITY_V3: usize = 5;
/// Common identity containing the closeable receipt Mint.
pub const RATIONAL_LIFECYCLE_IDENTITY_RECEIPT_MINT_V3: usize = 6;
/// Common identity containing Token-2022.
pub const RATIONAL_LIFECYCLE_IDENTITY_TOKEN_PROGRAM_V3: usize = 7;
/// Common identity containing the permanent RentCredit.
pub const RATIONAL_LIFECYCLE_IDENTITY_RENT_CREDIT_V3: usize = 8;
/// Common identity containing the RentCredit owner program.
pub const RATIONAL_LIFECYCLE_IDENTITY_RENT_PROGRAM_V3: usize = 9;

/// Common scalar containing the lifecycle action tag.
pub const RATIONAL_LIFECYCLE_SCALAR_ACTION_V3: usize = 0;
/// Common scalar containing immutable Market generation.
pub const RATIONAL_LIFECYCLE_SCALAR_GENERATION_V3: usize = 1;
/// Common scalar containing the expected Claims Market revision.
pub const RATIONAL_LIFECYCLE_SCALAR_MARKET_REVISION_V3: usize = 2;
/// Common scalar containing observed receipt-Mint lamports.
pub const RATIONAL_LIFECYCLE_SCALAR_RECEIPT_LAMPORTS_V3: usize = 3;
/// Common scalar containing the exact receipt-Mint rent principal.
pub const RATIONAL_LIFECYCLE_SCALAR_RECEIPT_RENT_V3: usize = 4;
/// Common scalar containing observed receipt-Mint supply.
pub const RATIONAL_LIFECYCLE_SCALAR_RECEIPT_SUPPLY_V3: usize = 5;
/// Common scalar containing request-owned runtime outcome width.
pub const RATIONAL_LIFECYCLE_SCALAR_OUTCOME_COUNT_V3: usize = 6;
/// Common scalar containing the runtime coordinate-row count.
pub const RATIONAL_LIFECYCLE_SCALAR_COORDINATE_COUNT_V3: usize = 7;
/// Common scalar containing RentCredit pre-lamports.
pub const RATIONAL_LIFECYCLE_SCALAR_RENT_BEFORE_V3: usize = 8;
/// Common scalar containing exact RentCredit post-lamports.
pub const RATIONAL_LIFECYCLE_SCALAR_RENT_AFTER_V3: usize = 9;
/// Common scalar containing independently observed Product outcome width.
pub const RATIONAL_LIFECYCLE_SCALAR_PRODUCT_OUTCOME_COUNT_V3: usize = 10;

/// Per-item identity containing the closeable shard Mint.
pub const RATIONAL_LIFECYCLE_ITEM_IDENTITY_SHARD_MINT_V3: usize = 0;
/// Per-item identity containing Structured custody.
pub const RATIONAL_LIFECYCLE_ITEM_IDENTITY_STRUCTURED_CUSTODY_V3: usize = 1;
/// Per-item identity containing the Claims custody owner.
pub const RATIONAL_LIFECYCLE_ITEM_IDENTITY_CUSTODY_OWNER_V3: usize = 2;
/// Per-item identity containing the Claims custody Position.
pub const RATIONAL_LIFECYCLE_ITEM_IDENTITY_CUSTODY_POSITION_V3: usize = 3;
/// Per-item identity containing the Position admission.
pub const RATIONAL_LIFECYCLE_ITEM_IDENTITY_POSITION_ADMISSION_V3: usize = 4;

/// Per-item scalar containing the Product outcome.
pub const RATIONAL_LIFECYCLE_ITEM_SCALAR_OUTCOME_V3: usize = 0;
/// Per-item scalar containing the descriptor coefficient.
pub const RATIONAL_LIFECYCLE_ITEM_SCALAR_COEFFICIENT_V3: usize = 1;
/// Per-item scalar containing observed shard-Mint lamports.
pub const RATIONAL_LIFECYCLE_ITEM_SCALAR_SHARD_LAMPORTS_V3: usize = 2;
/// Per-item scalar containing observed Structured-custody lamports.
pub const RATIONAL_LIFECYCLE_ITEM_SCALAR_STRUCTURED_LAMPORTS_V3: usize = 3;
/// Per-item scalar containing observed Position lamports.
pub const RATIONAL_LIFECYCLE_ITEM_SCALAR_POSITION_LAMPORTS_V3: usize = 4;
/// Per-item scalar containing observed admission lamports.
pub const RATIONAL_LIFECYCLE_ITEM_SCALAR_ADMISSION_LAMPORTS_V3: usize = 5;
/// Per-item scalar containing shard-Mint rent principal.
pub const RATIONAL_LIFECYCLE_ITEM_SCALAR_SHARD_RENT_V3: usize = 6;
/// Per-item scalar containing Structured-custody rent principal.
pub const RATIONAL_LIFECYCLE_ITEM_SCALAR_STRUCTURED_RENT_V3: usize = 7;
/// Per-item scalar containing Position rent principal.
pub const RATIONAL_LIFECYCLE_ITEM_SCALAR_POSITION_RENT_V3: usize = 8;
/// Per-item scalar containing admission rent principal.
pub const RATIONAL_LIFECYCLE_ITEM_SCALAR_ADMISSION_RENT_V3: usize = 9;
/// Per-item scalar containing shard-Mint supply.
pub const RATIONAL_LIFECYCLE_ITEM_SCALAR_SHARD_SUPPLY_V3: usize = 10;
/// Per-item scalar containing Structured-custody token amount.
pub const RATIONAL_LIFECYCLE_ITEM_SCALAR_STRUCTURED_AMOUNT_V3: usize = 11;
/// Per-item scalar containing Claims Position revision.
pub const RATIONAL_LIFECYCLE_ITEM_SCALAR_POSITION_REVISION_V3: usize = 12;

/// Borrowed wallet-facing lifecycle family request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RationalLifecycleHotRequestV3<'a> {
    bytes: &'a [u8],
}

/// Caller-owned register banks populated from one exact family request.
pub struct RationalLifecycleHotRegisterOutputV3<'a> {
    /// Exact common identity bank.
    pub common_identities: &'a mut [[u8; 32]],
    /// Exact common scalar bank.
    pub common_scalars: &'a mut [u64],
    /// Flat row-major item identity banks.
    pub item_identities: &'a mut [[u8; 32]],
    /// Flat row-major item scalar banks.
    pub item_scalars: &'a mut [u64],
}

impl<'a> RationalLifecycleHotRequestV3<'a> {
    /// Hostile-decode one exact runtime-width Hot family request.
    pub fn decode(input: &'a [u8]) -> Result<Self> {
        let request = decode_specialized(input, [1; 32])?;
        if request.header().parent_context != [1; 32] {
            return Err(Error::NonCanonical);
        }
        Ok(Self { bytes: input })
    }

    /// Project a canonical Claims child into the wallet-facing family form.
    pub fn from_child_into<'b>(
        child: LifecycleRequestV2<'_>,
        output: &'b mut [u8],
    ) -> Result<RationalLifecycleHotRequestV3<'b>> {
        child.encode_into(output)?;
        put(output, 0, &RATIONAL_LIFECYCLE_HOT_MAGIC_V3)?;
        put(output, 8, &RATIONAL_LIFECYCLE_HOT_VERSION_V3.to_le_bytes())?;
        output
            .get_mut(PARENT_CONTEXT_OFFSET..PARENT_CONTEXT_OFFSET + 32)
            .ok_or(Error::InvalidLength)?
            .fill(0);
        RationalLifecycleHotRequestV3::decode(output)
    }

    /// Specialize this family request into the sole Claims lifecycle child.
    pub fn specialize_child_into<'b>(
        self,
        family_digest: [u8; 32],
        output: &'b mut [u8],
    ) -> Result<LifecycleRequestV2<'b>> {
        if is_zero(&family_digest) {
            return Err(Error::InvalidIdentity);
        }
        if output.len() != self.bytes.len() {
            return Err(Error::InvalidLength);
        }
        output.copy_from_slice(self.bytes);
        put(output, 0, &LIFECYCLE_REQUEST_MAGIC_V2)?;
        put(output, 8, &LIFECYCLE_VERSION_V2.to_le_bytes())?;
        put(output, PARENT_CONTEXT_OFFSET, &family_digest)?;
        LifecycleRequestV2::decode(output)
    }

    /// Project exact common and per-coordinate register banks atomically.
    ///
    /// `product_outcome_count` is supplied by independently authenticated
    /// Product state and intentionally remains distinct from the request-owned
    /// width. Transition policy must require equality.
    pub fn project_registers(
        self,
        family_digest: [u8; 32],
        product_outcome_count: u32,
        output: RationalLifecycleHotRegisterOutputV3<'_>,
    ) -> Result<()> {
        if product_outcome_count == 0 {
            return Err(Error::InvalidPhysicalState);
        }
        let mut child_bytes_header = [0_u8; LIFECYCLE_HEADER_BYTES_V2];
        let header_source = self
            .bytes
            .get(..LIFECYCLE_HEADER_BYTES_V2)
            .ok_or(Error::InvalidLength)?;
        child_bytes_header.copy_from_slice(header_source);
        put(&mut child_bytes_header, 0, &LIFECYCLE_REQUEST_MAGIC_V2)?;
        put(
            &mut child_bytes_header,
            8,
            &LIFECYCLE_VERSION_V2.to_le_bytes(),
        )?;
        put(
            &mut child_bytes_header,
            PARENT_CONTEXT_OFFSET,
            &family_digest,
        )?;
        let header = LifecycleHeaderV2::decode(&child_bytes_header)?;
        let coordinate_count =
            usize::try_from(header.coordinate_count).map_err(|_| Error::InvalidLength)?;
        let expected_item_identities = coordinate_count
            .checked_mul(RATIONAL_LIFECYCLE_HOT_ITEM_IDENTITIES_V3)
            .ok_or(Error::InvalidLength)?;
        let expected_item_scalars = coordinate_count
            .checked_mul(RATIONAL_LIFECYCLE_HOT_ITEM_SCALARS_V3)
            .ok_or(Error::InvalidLength)?;
        if output.common_identities.len() != RATIONAL_LIFECYCLE_HOT_COMMON_IDENTITIES_V3
            || output.common_scalars.len() != RATIONAL_LIFECYCLE_HOT_COMMON_SCALARS_V3
            || output.item_identities.len() != expected_item_identities
            || output.item_scalars.len() != expected_item_scalars
        {
            return Err(Error::InvalidLength);
        }
        let child = LifecycleRequestV2::new(
            header,
            self.bytes
                .get(LIFECYCLE_HEADER_BYTES_V2..)
                .ok_or(Error::InvalidLength)?,
        )?;
        let common_identities = [
            family_digest,
            header.release_set,
            header.market,
            header.graph_id,
            header.descriptor_id,
            header.representation_authority,
            header.receipt_mint,
            header.token_program,
            header.rent_credit,
            header.rent_program,
        ];
        let common_scalars = [
            u64::from(header.action.tag()),
            header.generation,
            header.expected_claims_market_revision,
            header.observed_receipt_lamports,
            header.receipt_rent_principal,
            header.expected_receipt_supply,
            u64::from(header.outcome_count),
            u64::from(header.coordinate_count),
            header.rent_credit_before,
            header.rent_credit_after,
            u64::from(product_outcome_count),
        ];
        output.common_identities.copy_from_slice(&common_identities);
        output.common_scalars.copy_from_slice(&common_scalars);
        for ((identity_row, scalar_row), coordinate) in output
            .item_identities
            .chunks_exact_mut(RATIONAL_LIFECYCLE_HOT_ITEM_IDENTITIES_V3)
            .zip(
                output
                    .item_scalars
                    .chunks_exact_mut(RATIONAL_LIFECYCLE_HOT_ITEM_SCALARS_V3),
            )
            .zip(child.coordinates())
        {
            let coordinate = coordinate?;
            identity_row.copy_from_slice(&[
                coordinate.shard_mint,
                coordinate.structured_custody_account,
                coordinate.claims_custody_owner,
                coordinate.claims_custody_position,
                coordinate.position_admission,
            ]);
            scalar_row.copy_from_slice(&[
                u64::from(coordinate.outcome),
                coordinate.coefficient,
                coordinate.observed_shard_lamports,
                coordinate.observed_structured_lamports,
                coordinate.observed_position_lamports,
                coordinate.observed_admission_lamports,
                coordinate.shard_rent_principal,
                coordinate.structured_rent_principal,
                coordinate.position_rent_principal,
                coordinate.admission_rent_principal,
                coordinate.expected_shard_supply,
                coordinate.expected_structured_amount,
                coordinate.expected_position_revision,
            ]);
        }
        Ok(())
    }

    /// Exact family bytes hashed into the Claims child parent context.
    pub const fn as_bytes(self) -> &'a [u8] {
        self.bytes
    }
}

/// Verify the sole 320-byte lifecycle receipt against the exact child.
///
/// The Hot adapter must additionally require that return-data producer equals
/// the current Registry-selected Claims program.
pub fn verify_rational_lifecycle_hot_receipt_v3(
    child: LifecycleRequestV2<'_>,
    child_digest: [u8; 32],
    receipt_bytes: &[u8],
) -> Result<LifecycleReceiptV2> {
    let receipt = LifecycleReceiptV2::decode(receipt_bytes)?;
    receipt.verify_for(child, child_digest)?;
    Ok(receipt)
}

fn decode_specialized(input: &[u8], parent_marker: [u8; 32]) -> Result<LifecycleRequestV2<'_>> {
    if input.len() < LIFECYCLE_HEADER_BYTES_V2 {
        return Err(Error::InvalidLength);
    }
    exact(input, 0, &RATIONAL_LIFECYCLE_HOT_MAGIC_V3)?;
    if read_u16(input, 8)? != RATIONAL_LIFECYCLE_HOT_VERSION_V3 {
        return Err(Error::InvalidHeader);
    }
    require_zero(input, PARENT_CONTEXT_OFFSET, 32)?;
    let mut header = [0_u8; LIFECYCLE_HEADER_BYTES_V2];
    header.copy_from_slice(
        input
            .get(..LIFECYCLE_HEADER_BYTES_V2)
            .ok_or(Error::InvalidLength)?,
    );
    put(&mut header, 0, &LIFECYCLE_REQUEST_MAGIC_V2)?;
    put(&mut header, 8, &LIFECYCLE_VERSION_V2.to_le_bytes())?;
    put(&mut header, PARENT_CONTEXT_OFFSET, &parent_marker)?;
    let decoded_header = LifecycleHeaderV2::decode(&header)?;
    LifecycleRequestV2::new(
        decoded_header,
        input
            .get(LIFECYCLE_HEADER_BYTES_V2..)
            .ok_or(Error::InvalidLength)?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn header(action: LifecycleActionV2, coordinate_count: u32) -> LifecycleHeaderV2 {
        LifecycleHeaderV2 {
            action,
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
            coordinate_count,
            rent_credit_before: 100,
            rent_credit_after: 100,
        }
    }

    fn coordinate(outcome: u32) -> LifecycleCoordinateV2 {
        LifecycleCoordinateV2 {
            outcome,
            coefficient: 3,
            shard_mint: id(20),
            structured_custody_account: id(21),
            claims_custody_owner: id(22),
            claims_custody_position: id(23),
            position_admission: id(24),
            observed_shard_lamports: 10,
            observed_structured_lamports: 11,
            observed_position_lamports: 12,
            observed_admission_lamports: 13,
            shard_rent_principal: 10,
            structured_rent_principal: 11,
            position_rent_principal: 12,
            admission_rent_principal: 13,
            expected_shard_supply: 0,
            expected_structured_amount: 0,
            expected_position_revision: 0,
        }
    }

    fn child_bytes() -> [u8; LIFECYCLE_HEADER_BYTES_V2 + LIFECYCLE_COORDINATE_BYTES_V2] {
        let mut row = [0_u8; LIFECYCLE_COORDINATE_BYTES_V2];
        coordinate(257).encode_into(&mut row).expect("coordinate");
        let request =
            LifecycleRequestV2::new(header(LifecycleActionV2::ActivateCoordinate, 1), &row)
                .expect("child");
        let mut bytes = [0_u8; LIFECYCLE_HEADER_BYTES_V2 + LIFECYCLE_COORDINATE_BYTES_V2];
        request.encode_into(&mut bytes).expect("encode");
        bytes
    }

    #[test]
    fn schema_and_layout_are_fresh() {
        assert_eq!(
            Sha256::digest(RATIONAL_LIFECYCLE_HOT_SCHEMA_PREIMAGE_V3).as_slice(),
            RATIONAL_LIFECYCLE_HOT_SCHEMA_RELEASE_ID_V3
        );
        assert_eq!(
            RationalLifecycleHotLayoutV3::request_bytes(1),
            Some(LIFECYCLE_HEADER_BYTES_V2 + LIFECYCLE_COORDINATE_BYTES_V2)
        );
        assert_eq!(
            RationalLifecycleHotLayoutV3::PARENT_CONTEXT,
            PARENT_CONTEXT_OFFSET
        );
        assert_eq!(
            RationalLifecycleHotLayoutV3::ITEM_POSITION_REVISION,
            ROW_POSITION_REVISION_OFFSET
        );
    }

    #[test]
    fn family_specializes_exact_child_and_registers() {
        let child_bytes = child_bytes();
        let child = LifecycleRequestV2::decode(&child_bytes).expect("child");
        let mut family_bytes = [0_u8; LIFECYCLE_HEADER_BYTES_V2 + LIFECYCLE_COORDINATE_BYTES_V2];
        let family = RationalLifecycleHotRequestV3::from_child_into(child, &mut family_bytes)
            .expect("family");
        let digest = id(31);
        let mut specialized = [0_u8; LIFECYCLE_HEADER_BYTES_V2 + LIFECYCLE_COORDINATE_BYTES_V2];
        let child = family
            .specialize_child_into(digest, &mut specialized)
            .expect("specialized");
        assert_eq!(child.header().parent_context, digest);
        assert_eq!(child.header().outcome_count, 258);

        let mut common_identities = [[0_u8; 32]; RATIONAL_LIFECYCLE_HOT_COMMON_IDENTITIES_V3];
        let mut common_scalars = [0_u64; RATIONAL_LIFECYCLE_HOT_COMMON_SCALARS_V3];
        let mut item_identities = [[0_u8; 32]; RATIONAL_LIFECYCLE_HOT_ITEM_IDENTITIES_V3];
        let mut item_scalars = [0_u64; RATIONAL_LIFECYCLE_HOT_ITEM_SCALARS_V3];
        family
            .project_registers(
                digest,
                258,
                RationalLifecycleHotRegisterOutputV3 {
                    common_identities: &mut common_identities,
                    common_scalars: &mut common_scalars,
                    item_identities: &mut item_identities,
                    item_scalars: &mut item_scalars,
                },
            )
            .expect("registers");
        assert_eq!(common_identities[0], digest);
        assert_eq!(
            common_scalars[RATIONAL_LIFECYCLE_SCALAR_OUTCOME_COUNT_V3],
            258
        );
        assert_eq!(
            common_scalars[RATIONAL_LIFECYCLE_SCALAR_PRODUCT_OUTCOME_COUNT_V3],
            258
        );
        assert_eq!(item_identities[0], id(20));
        assert_eq!(item_scalars[RATIONAL_LIFECYCLE_ITEM_SCALAR_OUTCOME_V3], 257);
    }

    #[test]
    fn hostile_parent_and_atomic_width_refuse() {
        let child_bytes = child_bytes();
        let child = LifecycleRequestV2::decode(&child_bytes).expect("child");
        let mut family_bytes = [0_u8; LIFECYCLE_HEADER_BYTES_V2 + LIFECYCLE_COORDINATE_BYTES_V2];
        RationalLifecycleHotRequestV3::from_child_into(child, &mut family_bytes).expect("family");
        family_bytes[PARENT_CONTEXT_OFFSET] = 1;
        assert_eq!(
            RationalLifecycleHotRequestV3::decode(&family_bytes),
            Err(Error::NonCanonical)
        );
        family_bytes[PARENT_CONTEXT_OFFSET] = 0;
        let family = RationalLifecycleHotRequestV3::decode(&family_bytes).expect("family");
        let mut common_identities = [[9_u8; 32]; RATIONAL_LIFECYCLE_HOT_COMMON_IDENTITIES_V3];
        let before = common_identities;
        let mut wrong_common_scalars = [9_u64; RATIONAL_LIFECYCLE_HOT_COMMON_SCALARS_V3 - 1];
        let mut item_identities = [[9_u8; 32]; RATIONAL_LIFECYCLE_HOT_ITEM_IDENTITIES_V3];
        let mut item_scalars = [9_u64; RATIONAL_LIFECYCLE_HOT_ITEM_SCALARS_V3];
        assert_eq!(
            family.project_registers(
                id(31),
                258,
                RationalLifecycleHotRegisterOutputV3 {
                    common_identities: &mut common_identities,
                    common_scalars: &mut wrong_common_scalars,
                    item_identities: &mut item_identities,
                    item_scalars: &mut item_scalars,
                },
            ),
            Err(Error::InvalidLength)
        );
        assert_eq!(common_identities, before);
    }

    #[test]
    fn receipt_verifier_refuses_substitution() {
        let child_bytes = child_bytes();
        let child = LifecycleRequestV2::decode(&child_bytes).expect("child");
        let digest = id(31);
        let receipt = LifecycleReceiptV2 {
            action: LifecycleActionV2::ActivateCoordinate,
            request_digest: digest,
            descriptor_id: child.header().descriptor_id,
            market: child.header().market,
            post_resource_digest: id(32),
            position_lifecycle_receipt_digest: id(33),
            rent_credit: child.header().rent_credit,
            rent_program: child.header().rent_program,
            generation: child.header().generation,
            outcome: 257,
            rent_credit_before: 100,
            rent_credit_after: 100,
            credited_lamports: 0,
            coordinate_count: 1,
        };
        let bytes = receipt.to_bytes().expect("receipt");
        assert_eq!(
            verify_rational_lifecycle_hot_receipt_v3(child, digest, &bytes),
            Ok(receipt)
        );
        assert_eq!(
            verify_rational_lifecycle_hot_receipt_v3(child, id(30), &bytes),
            Err(Error::InvalidCompletion)
        );
    }
}
