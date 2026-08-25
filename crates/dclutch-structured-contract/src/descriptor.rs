//! Immutable descriptor and Product-owned backing authentication.

use core::convert::TryFrom;

use dclutch_bearer_contract::state::BEARER_SEMANTIC_RELEASE_ID;
use dclutch_capability_contract::{CapabilityEntryV1, FundingAssetClassV1};
use dclutch_market_contract::market::CategoricalMarketV1;
use dclutch_product_contract::{
    ContentId as ProductContentId,
    portfolio::{PORTFOLIO_TEMPLATE_CONTENT_DOMAIN_V1, PortfolioTemplateV1},
    product::InstanceV1,
};
use dclutch_token_svm::TOKEN_2022_PROGRAM_ID;

use crate::{Error, ID_BYTES, Result, array, byte, put, require_nonzero, require_zero};

/// Exact immutable structured-config width.
pub const STRUCTURED_CONFIG_BYTES: usize = 112;
/// Exact immutable structured-descriptor width.
pub const STRUCTURED_DESCRIPTOR_BYTES: usize = 352;
/// Canonical structured-config magic.
pub const STRUCTURED_CONFIG_MAGIC: [u8; 8] = *b"DCLTSTC1";
/// Canonical structured-descriptor magic.
pub const STRUCTURED_DESCRIPTOR_MAGIC: [u8; 8] = *b"DCLTSTD1";
/// Implemented config schema.
pub const STRUCTURED_CONFIG_SCHEMA_VERSION: u16 = 1;
/// Implemented descriptor schema.
pub const STRUCTURED_DESCRIPTOR_SCHEMA_VERSION: u16 = 1;
/// Current provisional exact-N descriptor profile.
pub const STRUCTURED_PROFILE_N2_N16_V1: u8 = 1;
/// Structured receipt Mints use integral raw units and no display decimals.
pub const STRUCTURED_RECEIPT_DECIMALS_V1: u8 = 0;
/// Mathematical minimum categorical width.
pub const MIN_STRUCTURED_OUTCOMES: usize = 2;
/// Provisional Product-artifact maximum inherited from PortfolioTemplate V1.
///
/// This is not a mathematical structured-portfolio restriction. The lifting
/// path is a wider exact-width or paged Product template release and matching
/// descriptor profile, without changing native categorical liabilities.
pub const MAX_STRUCTURED_OUTCOMES: usize = 16;

/// Exact capability-kind identity preimage.
pub const STRUCTURED_CAPABILITY_KIND_PREIMAGE_V1: &[u8] =
    b"dclutch:capability-kind:structured-portfolio:v1";
/// SHA-256 of [`STRUCTURED_CAPABILITY_KIND_PREIMAGE_V1`].
pub const STRUCTURED_CAPABILITY_KIND_ID_V1: [u8; ID_BYTES] = [
    0x5f, 0x02, 0x47, 0x2b, 0xbd, 0x21, 0xe5, 0x46, 0xa2, 0xf8, 0x6b, 0x4c, 0xc4, 0x5a, 0x53, 0x8b,
    0xb4, 0xf7, 0xe1, 0x8e, 0x6d, 0x97, 0x35, 0x06, 0xbb, 0x17, 0x7c, 0x49, 0x54, 0x4a, 0x71, 0x16,
];
/// Exact semantic-release identity preimage.
pub const STRUCTURED_SEMANTIC_RELEASE_PREIMAGE_V1: &[u8] =
    b"dclutch:structured-contract:semantic-release:v1";
/// SHA-256 of [`STRUCTURED_SEMANTIC_RELEASE_PREIMAGE_V1`].
pub const STRUCTURED_SEMANTIC_RELEASE_ID_V1: [u8; ID_BYTES] = [
    0x33, 0xa6, 0xeb, 0x59, 0x19, 0x94, 0x28, 0x7d, 0xd5, 0xa6, 0xa0, 0x9e, 0xbd, 0x37, 0xf3, 0x4d,
    0x6b, 0x2f, 0xf4, 0x2c, 0x29, 0x41, 0x67, 0xe6, 0x52, 0x68, 0xb1, 0x3b, 0x21, 0x69, 0xaa, 0x86,
];
/// Exact measured/provisional capacity-coordinate preimage.
pub const STRUCTURED_CAPACITY_PREIMAGE_V1: &[u8] =
    b"dclutch:structured-contract:capacity:n2-n16:v1";
/// SHA-256 of [`STRUCTURED_CAPACITY_PREIMAGE_V1`].
pub const STRUCTURED_CAPACITY_ID_V1: [u8; ID_BYTES] = [
    0x1b, 0xdf, 0x67, 0xea, 0x33, 0xcb, 0x6a, 0xc4, 0x80, 0xc3, 0x6f, 0x0e, 0xe1, 0xcc, 0xd7, 0x96,
    0x0c, 0x4d, 0xf9, 0xb1, 0x60, 0xd7, 0xe4, 0xdb, 0x9c, 0x11, 0xad, 0xc5, 0x52, 0x48, 0x62, 0x00,
];
/// Exact direct-child schema identity preimage.
pub const STRUCTURED_CHILD_SCHEMA_PREIMAGE_V1: &[u8] =
    b"dclutch:structured-contract:child-schema:v1";
/// SHA-256 of [`STRUCTURED_CHILD_SCHEMA_PREIMAGE_V1`].
pub const STRUCTURED_CHILD_SCHEMA_ID_V1: [u8; ID_BYTES] = [
    0x87, 0xff, 0xcd, 0xbc, 0xf9, 0x5f, 0xab, 0x03, 0x86, 0x02, 0x17, 0xf4, 0x07, 0xe3, 0x4f, 0xa2,
    0x88, 0x92, 0x75, 0xfe, 0x6a, 0x79, 0x08, 0x4f, 0x6a, 0x5f, 0xac, 0x16, 0xfe, 0xf9, 0xc1, 0x5b,
];
/// Exact child-derivation identity preimage.
pub const STRUCTURED_CHILD_DERIVATION_PREIMAGE_V1: &[u8] =
    b"dclutch:structured-contract:child-derivation:v1";
/// SHA-256 of [`STRUCTURED_CHILD_DERIVATION_PREIMAGE_V1`].
pub const STRUCTURED_CHILD_DERIVATION_ID_V1: [u8; ID_BYTES] = [
    0xac, 0xcd, 0x51, 0x74, 0x7c, 0x3f, 0x2b, 0xa4, 0xb3, 0x20, 0xd0, 0x84, 0xac, 0x20, 0xd0, 0x26,
    0xaa, 0xff, 0x5a, 0x49, 0x7b, 0x58, 0xc3, 0xd5, 0x1d, 0x16, 0xfd, 0xf4, 0x23, 0x8d, 0x6a, 0x6b,
];

/// Content-address domain preceding exact descriptor bytes.
pub const STRUCTURED_DESCRIPTOR_CONTENT_DOMAIN_V1: &[u8] = b"dclutch.structured-descriptor.v1";
/// Content-address domain preceding exact config bytes.
pub const STRUCTURED_CONFIG_CONTENT_DOMAIN_V1: &[u8] = b"dclutch.structured-config.v1";
/// PDA domain for the descriptor; remaining seeds are exposed below.
pub const STRUCTURED_DESCRIPTOR_PDA_DOMAIN_V1: &[u8] = b"dclutch/structured/v1";
/// Receipt Mint PDA domain followed by descriptor account key.
pub const STRUCTURED_RECEIPT_MINT_PDA_DOMAIN_V1: &[u8] = b"dclutch/structured-mint/v1";
/// Receipt controller PDA domain followed by descriptor account key.
pub const STRUCTURED_RECEIPT_AUTHORITY_PDA_DOMAIN_V1: &[u8] = b"dclutch/structured-authority/v1";
/// Custody-owner PDA domain followed by descriptor account key.
pub const STRUCTURED_CUSTODY_OWNER_PDA_DOMAIN_V1: &[u8] = b"dclutch/structured-custody/v1";

const CONFIG_DECIMALS_OFFSET: usize = 10;
const CONFIG_PROFILE_OFFSET: usize = 11;
const CONFIG_RESERVED_OFFSET: usize = 12;
const CONFIG_RESERVED_BYTES: usize = 4;
const CONFIG_TOKEN_PROGRAM_OFFSET: usize = 16;
const CONFIG_RECEIPT_RELEASE_OFFSET: usize = 48;
const CONFIG_RENT_CREDIT_OFFSET: usize = 80;

const DESCRIPTOR_OUTCOME_COUNT_OFFSET: usize = 10;
const DESCRIPTOR_PROFILE_OFFSET: usize = 11;
const DESCRIPTOR_ENTRY_INDEX_OFFSET: usize = 12;
const DESCRIPTOR_DECIMALS_OFFSET: usize = 14;
const DESCRIPTOR_HEADER_RESERVED_OFFSET: usize = 15;
const DESCRIPTOR_MARKET_OFFSET: usize = 16;
const DESCRIPTOR_GENERATION_OFFSET: usize = 48;
const DESCRIPTOR_BODY_RESERVED_OFFSET: usize = 56;
const DESCRIPTOR_BODY_RESERVED_BYTES: usize = 8;
const DESCRIPTOR_TEMPLATE_ID_OFFSET: usize = 64;
const DESCRIPTOR_CONFIG_ID_OFFSET: usize = 96;
const DESCRIPTOR_RELEASE_ID_OFFSET: usize = 128;
const DESCRIPTOR_RECEIPT_RELEASE_OFFSET: usize = 160;
const DESCRIPTOR_RECEIPT_MINT_OFFSET: usize = 192;
const DESCRIPTOR_RECEIPT_AUTHORITY_OFFSET: usize = 224;
const DESCRIPTOR_CUSTODY_POSITION_OFFSET: usize = 256;
const DESCRIPTOR_CUSTODY_OWNER_OFFSET: usize = 288;
const DESCRIPTOR_RENT_CREDIT_OFFSET: usize = 320;

/// Immutable Structured configuration selected by the capability manifest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredConfigV1 {
    token_program: [u8; ID_BYTES],
    receipt_adapter_release_id: [u8; ID_BYTES],
    rent_credit: [u8; ID_BYTES],
}

impl StructuredConfigV1 {
    /// Construct the closed Token-2022 integral-receipt profile.
    pub fn new(
        token_program: [u8; ID_BYTES],
        receipt_adapter_release_id: [u8; ID_BYTES],
        rent_credit: [u8; ID_BYTES],
    ) -> Result<Self> {
        require_nonzero(&token_program)?;
        require_nonzero(&receipt_adapter_release_id)?;
        require_nonzero(&rent_credit)?;
        if token_program != TOKEN_2022_PROGRAM_ID
            || receipt_adapter_release_id != BEARER_SEMANTIC_RELEASE_ID
        {
            return Err(Error::CapabilitySelectionMismatch);
        }
        Ok(Self {
            token_program,
            receipt_adapter_release_id,
            rent_credit,
        })
    }

    /// Decode one exact hostile config record.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != STRUCTURED_CONFIG_BYTES {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, 0)? != STRUCTURED_CONFIG_MAGIC {
            return Err(Error::InvalidMagic);
        }
        if u16::from_le_bytes(array(bytes, 8)?) != STRUCTURED_CONFIG_SCHEMA_VERSION {
            return Err(Error::UnsupportedSchema);
        }
        if byte(bytes, CONFIG_DECIMALS_OFFSET)? != STRUCTURED_RECEIPT_DECIMALS_V1
            || byte(bytes, CONFIG_PROFILE_OFFSET)? != STRUCTURED_PROFILE_N2_N16_V1
        {
            return Err(Error::UnsupportedSchema);
        }
        require_zero(bytes, CONFIG_RESERVED_OFFSET, CONFIG_RESERVED_BYTES)?;
        Self::new(
            array(bytes, CONFIG_TOKEN_PROGRAM_OFFSET)?,
            array(bytes, CONFIG_RECEIPT_RELEASE_OFFSET)?,
            array(bytes, CONFIG_RENT_CREDIT_OFFSET)?,
        )
    }

    /// Return the exact config content preimage.
    pub fn to_bytes(self) -> [u8; STRUCTURED_CONFIG_BYTES] {
        let mut output = [0; STRUCTURED_CONFIG_BYTES];
        put(&mut output, 0, &STRUCTURED_CONFIG_MAGIC);
        put(
            &mut output,
            8,
            &STRUCTURED_CONFIG_SCHEMA_VERSION.to_le_bytes(),
        );
        put(
            &mut output,
            CONFIG_DECIMALS_OFFSET,
            &[STRUCTURED_RECEIPT_DECIMALS_V1],
        );
        put(
            &mut output,
            CONFIG_PROFILE_OFFSET,
            &[STRUCTURED_PROFILE_N2_N16_V1],
        );
        put(
            &mut output,
            CONFIG_TOKEN_PROGRAM_OFFSET,
            &self.token_program,
        );
        put(
            &mut output,
            CONFIG_RECEIPT_RELEASE_OFFSET,
            &self.receipt_adapter_release_id,
        );
        put(&mut output, CONFIG_RENT_CREDIT_OFFSET, &self.rent_credit);
        output
    }

    /// Encode atomically into one exact caller-owned buffer.
    pub fn encode(self, output: &mut [u8]) -> Result<()> {
        if output.len() != STRUCTURED_CONFIG_BYTES {
            return Err(Error::OutputLength);
        }
        output.copy_from_slice(&self.to_bytes());
        Ok(())
    }

    /// Return the exact Token-2022 program address.
    pub const fn token_program(self) -> [u8; ID_BYTES] {
        self.token_program
    }

    /// Return the shared exact receipt-profile semantic release.
    pub const fn receipt_adapter_release_id(self) -> [u8; ID_BYTES] {
        self.receipt_adapter_release_id
    }

    /// Return the permanent recovered-rent beneficiary.
    pub const fn rent_credit(self) -> [u8; ID_BYTES] {
        self.rent_credit
    }
}

/// Inputs to one immutable structured receipt descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredDescriptorInputV1 {
    /// Canonical Market account key.
    pub market: [u8; ID_BYTES],
    /// Immutable Market generation.
    pub generation: u64,
    /// Selected manifest entry index.
    pub manifest_entry_index: u16,
    /// Product compiler's authenticated PortfolioTemplate content identity.
    pub portfolio_template_id: [u8; ID_BYTES],
    /// Exact immutable config content identity selected by the manifest.
    pub capability_config_id: [u8; ID_BYTES],
    /// Exact Structured semantic release.
    pub capability_release_id: [u8; ID_BYTES],
    /// Shared Bearer/Token receipt-profile semantic release.
    pub receipt_adapter_release_id: [u8; ID_BYTES],
    /// Canonical descriptor-derived Token-2022 receipt Mint.
    pub receipt_mint: [u8; ID_BYTES],
    /// Canonical descriptor-derived Mint/close/PermissionedBurn controller.
    pub receipt_authority: [u8; ID_BYTES],
    /// Canonical Market Position account used for native backing custody.
    pub custody_position: [u8; ID_BYTES],
    /// Canonical owner authority embedded in the custody Position.
    pub custody_owner: [u8; ID_BYTES],
    /// Permanent RentCredit beneficiary repeated from config.
    pub rent_credit: [u8; ID_BYTES],
}

/// One immutable transferable structured-receipt descriptor.
///
/// Coefficients, denominator, claim basis, and result domain are deliberately
/// absent. They are already owned by the authenticated Product template. The
/// descriptor binds that exact template content identity, the Market, and all
/// physical receipt/custody identities. Persisting recipe or receipt supply
/// here would create parallel semantic truth.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredDescriptorV1 {
    outcome_count: u8,
    manifest_entry_index: u16,
    market: [u8; ID_BYTES],
    generation: u64,
    portfolio_template_id: [u8; ID_BYTES],
    capability_config_id: [u8; ID_BYTES],
    capability_release_id: [u8; ID_BYTES],
    receipt_adapter_release_id: [u8; ID_BYTES],
    receipt_mint: [u8; ID_BYTES],
    receipt_authority: [u8; ID_BYTES],
    custody_position: [u8; ID_BYTES],
    custody_owner: [u8; ID_BYTES],
    rent_credit: [u8; ID_BYTES],
}

impl StructuredDescriptorV1 {
    /// Construct one descriptor for exact Product width `N`.
    pub fn new<const N: usize>(input: StructuredDescriptorInputV1) -> Result<Self> {
        validate_width::<N>()?;
        for value in [
            input.market,
            input.portfolio_template_id,
            input.capability_config_id,
            input.capability_release_id,
            input.receipt_adapter_release_id,
            input.receipt_mint,
            input.receipt_authority,
            input.custody_position,
            input.custody_owner,
            input.rent_credit,
        ] {
            require_nonzero(&value)?;
        }
        if input.capability_release_id != STRUCTURED_SEMANTIC_RELEASE_ID_V1 {
            return Err(Error::CapabilityReleaseMismatch);
        }
        if input.receipt_adapter_release_id != BEARER_SEMANTIC_RELEASE_ID {
            return Err(Error::CapabilitySelectionMismatch);
        }
        require_distinct_physical_identities(&input)?;
        Ok(Self {
            outcome_count: u8::try_from(N).map_err(|_| Error::InvalidOutcomeCount)?,
            manifest_entry_index: input.manifest_entry_index,
            market: input.market,
            generation: input.generation,
            portfolio_template_id: input.portfolio_template_id,
            capability_config_id: input.capability_config_id,
            capability_release_id: input.capability_release_id,
            receipt_adapter_release_id: input.receipt_adapter_release_id,
            receipt_mint: input.receipt_mint,
            receipt_authority: input.receipt_authority,
            custody_position: input.custody_position,
            custody_owner: input.custody_owner,
            rent_credit: input.rent_credit,
        })
    }

    /// Decode one exact descriptor while refusing unknown widths and releases.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != STRUCTURED_DESCRIPTOR_BYTES {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, 0)? != STRUCTURED_DESCRIPTOR_MAGIC {
            return Err(Error::InvalidMagic);
        }
        if u16::from_le_bytes(array(bytes, 8)?) != STRUCTURED_DESCRIPTOR_SCHEMA_VERSION {
            return Err(Error::UnsupportedSchema);
        }
        if byte(bytes, DESCRIPTOR_PROFILE_OFFSET)? != STRUCTURED_PROFILE_N2_N16_V1
            || byte(bytes, DESCRIPTOR_DECIMALS_OFFSET)? != STRUCTURED_RECEIPT_DECIMALS_V1
        {
            return Err(Error::UnsupportedSchema);
        }
        require_zero(bytes, DESCRIPTOR_HEADER_RESERVED_OFFSET, 1)?;
        require_zero(
            bytes,
            DESCRIPTOR_BODY_RESERVED_OFFSET,
            DESCRIPTOR_BODY_RESERVED_BYTES,
        )?;
        let outcome_count = byte(bytes, DESCRIPTOR_OUTCOME_COUNT_OFFSET)?;
        validate_dynamic_width(outcome_count)?;
        let input = StructuredDescriptorInputV1 {
            market: array(bytes, DESCRIPTOR_MARKET_OFFSET)?,
            generation: u64::from_le_bytes(array(bytes, DESCRIPTOR_GENERATION_OFFSET)?),
            manifest_entry_index: u16::from_le_bytes(array(bytes, DESCRIPTOR_ENTRY_INDEX_OFFSET)?),
            portfolio_template_id: array(bytes, DESCRIPTOR_TEMPLATE_ID_OFFSET)?,
            capability_config_id: array(bytes, DESCRIPTOR_CONFIG_ID_OFFSET)?,
            capability_release_id: array(bytes, DESCRIPTOR_RELEASE_ID_OFFSET)?,
            receipt_adapter_release_id: array(bytes, DESCRIPTOR_RECEIPT_RELEASE_OFFSET)?,
            receipt_mint: array(bytes, DESCRIPTOR_RECEIPT_MINT_OFFSET)?,
            receipt_authority: array(bytes, DESCRIPTOR_RECEIPT_AUTHORITY_OFFSET)?,
            custody_position: array(bytes, DESCRIPTOR_CUSTODY_POSITION_OFFSET)?,
            custody_owner: array(bytes, DESCRIPTOR_CUSTODY_OWNER_OFFSET)?,
            rent_credit: array(bytes, DESCRIPTOR_RENT_CREDIT_OFFSET)?,
        };
        match outcome_count {
            2 => Self::new::<2>(input),
            3 => Self::new::<3>(input),
            4 => Self::new::<4>(input),
            5 => Self::new::<5>(input),
            6 => Self::new::<6>(input),
            7 => Self::new::<7>(input),
            8 => Self::new::<8>(input),
            9 => Self::new::<9>(input),
            10 => Self::new::<10>(input),
            11 => Self::new::<11>(input),
            12 => Self::new::<12>(input),
            13 => Self::new::<13>(input),
            14 => Self::new::<14>(input),
            15 => Self::new::<15>(input),
            16 => Self::new::<16>(input),
            _ => Err(Error::InvalidOutcomeCount),
        }
    }

    /// Return the exact descriptor content preimage.
    pub fn to_bytes(self) -> [u8; STRUCTURED_DESCRIPTOR_BYTES] {
        let mut output = [0; STRUCTURED_DESCRIPTOR_BYTES];
        put(&mut output, 0, &STRUCTURED_DESCRIPTOR_MAGIC);
        put(
            &mut output,
            8,
            &STRUCTURED_DESCRIPTOR_SCHEMA_VERSION.to_le_bytes(),
        );
        put(
            &mut output,
            DESCRIPTOR_OUTCOME_COUNT_OFFSET,
            &[self.outcome_count],
        );
        put(
            &mut output,
            DESCRIPTOR_PROFILE_OFFSET,
            &[STRUCTURED_PROFILE_N2_N16_V1],
        );
        put(
            &mut output,
            DESCRIPTOR_ENTRY_INDEX_OFFSET,
            &self.manifest_entry_index.to_le_bytes(),
        );
        put(
            &mut output,
            DESCRIPTOR_DECIMALS_OFFSET,
            &[STRUCTURED_RECEIPT_DECIMALS_V1],
        );
        put(&mut output, DESCRIPTOR_MARKET_OFFSET, &self.market);
        put(
            &mut output,
            DESCRIPTOR_GENERATION_OFFSET,
            &self.generation.to_le_bytes(),
        );
        put(
            &mut output,
            DESCRIPTOR_TEMPLATE_ID_OFFSET,
            &self.portfolio_template_id,
        );
        put(
            &mut output,
            DESCRIPTOR_CONFIG_ID_OFFSET,
            &self.capability_config_id,
        );
        put(
            &mut output,
            DESCRIPTOR_RELEASE_ID_OFFSET,
            &self.capability_release_id,
        );
        put(
            &mut output,
            DESCRIPTOR_RECEIPT_RELEASE_OFFSET,
            &self.receipt_adapter_release_id,
        );
        put(
            &mut output,
            DESCRIPTOR_RECEIPT_MINT_OFFSET,
            &self.receipt_mint,
        );
        put(
            &mut output,
            DESCRIPTOR_RECEIPT_AUTHORITY_OFFSET,
            &self.receipt_authority,
        );
        put(
            &mut output,
            DESCRIPTOR_CUSTODY_POSITION_OFFSET,
            &self.custody_position,
        );
        put(
            &mut output,
            DESCRIPTOR_CUSTODY_OWNER_OFFSET,
            &self.custody_owner,
        );
        put(
            &mut output,
            DESCRIPTOR_RENT_CREDIT_OFFSET,
            &self.rent_credit,
        );
        output
    }

    /// Encode atomically into one exact caller-owned buffer.
    pub fn encode(self, output: &mut [u8]) -> Result<()> {
        if output.len() != STRUCTURED_DESCRIPTOR_BYTES {
            return Err(Error::OutputLength);
        }
        output.copy_from_slice(&self.to_bytes());
        Ok(())
    }

    /// Return the exact Product outcome width.
    pub const fn outcome_count(self) -> u8 {
        self.outcome_count
    }

    /// Return the selected manifest entry index.
    pub const fn manifest_entry_index(self) -> u16 {
        self.manifest_entry_index
    }

    /// Return the canonical Market key.
    pub const fn market(self) -> [u8; ID_BYTES] {
        self.market
    }

    /// Return the immutable Market generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Return the authenticated Product PortfolioTemplate identity.
    pub const fn portfolio_template_id(self) -> [u8; ID_BYTES] {
        self.portfolio_template_id
    }

    /// Return the immutable capability config identity.
    pub const fn capability_config_id(self) -> [u8; ID_BYTES] {
        self.capability_config_id
    }

    /// Return the exact Structured semantic release identity.
    pub const fn capability_release_id(self) -> [u8; ID_BYTES] {
        self.capability_release_id
    }

    /// Return the shared exact receipt adapter release identity.
    pub const fn receipt_adapter_release_id(self) -> [u8; ID_BYTES] {
        self.receipt_adapter_release_id
    }

    /// Return the canonical Token-2022 receipt Mint.
    pub const fn receipt_mint(self) -> [u8; ID_BYTES] {
        self.receipt_mint
    }

    /// Return the canonical Mint/close/PermissionedBurn authority.
    pub const fn receipt_authority(self) -> [u8; ID_BYTES] {
        self.receipt_authority
    }

    /// Return the canonical native custody Position account.
    pub const fn custody_position(self) -> [u8; ID_BYTES] {
        self.custody_position
    }

    /// Return the owner embedded in the custody Position.
    pub const fn custody_owner(self) -> [u8; ID_BYTES] {
        self.custody_owner
    }

    /// Return the permanent descriptor/Mint/custody RentCredit beneficiary.
    pub const fn rent_credit(self) -> [u8; ID_BYTES] {
        self.rent_credit
    }
}

/// Exact Product-derived integral backing for one structured receipt atom.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BackingRecipeV1<const N: usize> {
    minimum_realization_lot: u64,
    coefficients: [u64; N],
}

impl<const N: usize> BackingRecipeV1<N> {
    fn from_template(template: PortfolioTemplateV1<N>) -> Result<Self> {
        validate_width::<N>()?;
        let minimum_realization_lot = derive_minimum_realization_lot(template)?;
        if minimum_realization_lot != template.denominator() {
            return Err(Error::NonCanonicalRealizationLot);
        }
        let mut coefficients = [0; N];
        template
            .materialize(minimum_realization_lot, &mut coefficients)
            .map_err(product_error)?;
        Ok(Self {
            minimum_realization_lot,
            coefficients,
        })
    }

    /// Return the least positive Product scale represented by one receipt atom.
    pub const fn minimum_realization_lot(self) -> u64 {
        self.minimum_realization_lot
    }

    /// Borrow actual native claims backing one receipt atom.
    pub const fn coefficients(&self) -> &[u64; N] {
        &self.coefficients
    }
}

/// Adapter-authenticated Product instance and PortfolioTemplate projection.
///
/// The pure crate deliberately does not hash. Before construction, the adapter
/// must hash exact Instance and template preimages under their documented
/// Product domains. This constructor then owns every semantic cross-link and
/// exact realization check.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductBindingV1<const N: usize> {
    instance_id: ProductContentId,
    instance: InstanceV1,
    portfolio_template_id: ProductContentId,
    template: PortfolioTemplateV1<N>,
    recipe: BackingRecipeV1<N>,
}

impl<const N: usize> ProductBindingV1<N> {
    /// Join one exact-width Product instance and portfolio template.
    pub fn new(
        instance_id: ProductContentId,
        instance: InstanceV1,
        portfolio_template_id: ProductContentId,
        template: PortfolioTemplateV1<N>,
    ) -> Result<Self> {
        validate_width::<N>()?;
        let width = u32::try_from(N).map_err(|_| Error::InvalidOutcomeCount)?;
        if instance.partition_cell_count() != width {
            return Err(Error::InvalidOutcomeCount);
        }
        if instance.claim_basis_id() != template.claim_basis_id() {
            return Err(Error::ClaimBasisMismatch);
        }
        if instance.result_domain_id() != template.result_domain_id() {
            return Err(Error::ResultDomainMismatch);
        }
        let recipe = BackingRecipeV1::from_template(template)?;
        Ok(Self {
            instance_id,
            instance,
            portfolio_template_id,
            template,
            recipe,
        })
    }

    /// Return the authenticated Product-instance content identity.
    pub const fn instance_id(self) -> ProductContentId {
        self.instance_id
    }

    /// Return the hostile-decoded Product instance.
    pub const fn instance(self) -> InstanceV1 {
        self.instance
    }

    /// Return the authenticated Product PortfolioTemplate identity.
    pub const fn portfolio_template_id(self) -> ProductContentId {
        self.portfolio_template_id
    }

    /// Return Product's sole canonical template content-address namespace.
    pub const fn portfolio_template_content_domain(self) -> &'static [u8] {
        PORTFOLIO_TEMPLATE_CONTENT_DOMAIN_V1
    }

    /// Return the hostile-decoded canonical PortfolioTemplate.
    pub const fn template(self) -> PortfolioTemplateV1<N> {
        self.template
    }

    /// Return the exact integral backing derived from Product truth.
    pub const fn recipe(self) -> BackingRecipeV1<N> {
        self.recipe
    }
}

/// Fully joined immutable descriptor context consumed by pure transitions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredContextV1<const N: usize> {
    descriptor_key: [u8; ID_BYTES],
    descriptor: StructuredDescriptorV1,
    product_instance_id: [u8; ID_BYTES],
    claim_basis_id: [u8; ID_BYTES],
    result_domain_id: [u8; ID_BYTES],
    recipe: BackingRecipeV1<N>,
}

impl<const N: usize> StructuredContextV1<N> {
    /// Authenticate descriptor, Market, Product instance/template, and config.
    ///
    /// Content IDs and physical PDA keys must be recomputed by the adapter
    /// before this pure boundary.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        descriptor_key: [u8; ID_BYTES],
        descriptor: StructuredDescriptorV1,
        market_key: [u8; ID_BYTES],
        market: &CategoricalMarketV1<N>,
        product: ProductBindingV1<N>,
        config_id: [u8; ID_BYTES],
        config: StructuredConfigV1,
    ) -> Result<Self> {
        validate_width::<N>()?;
        require_nonzero(&descriptor_key)?;
        require_nonzero(&market_key)?;
        require_nonzero(&config_id)?;
        if descriptor.outcome_count != u8::try_from(N).map_err(|_| Error::InvalidOutcomeCount)? {
            return Err(Error::InvalidOutcomeCount);
        }
        if descriptor.market != market_key {
            return Err(Error::MarketMismatch);
        }
        let identity = market.root().identity();
        if descriptor.generation != identity.generation() {
            return Err(Error::GenerationMismatch);
        }
        if identity.product_instance_id().to_bytes() != product.instance_id().to_bytes() {
            return Err(Error::ProductInstanceMismatch);
        }
        if identity.claim_basis_id().to_bytes() != product.instance().claim_basis_id().to_bytes()
            || product.instance().claim_basis_id() != product.template().claim_basis_id()
        {
            return Err(Error::ClaimBasisMismatch);
        }
        if product.instance().result_domain_id() != product.template().result_domain_id() {
            return Err(Error::ResultDomainMismatch);
        }
        if descriptor.portfolio_template_id != product.portfolio_template_id().to_bytes() {
            return Err(Error::PortfolioTemplateMismatch);
        }
        if descriptor.capability_config_id != config_id {
            return Err(Error::CapabilityConfigMismatch);
        }
        if descriptor.capability_release_id != STRUCTURED_SEMANTIC_RELEASE_ID_V1 {
            return Err(Error::CapabilityReleaseMismatch);
        }
        if descriptor.receipt_adapter_release_id != config.receipt_adapter_release_id() {
            return Err(Error::CapabilitySelectionMismatch);
        }
        if descriptor.rent_credit != config.rent_credit() {
            return Err(Error::RentCreditMismatch);
        }
        if config.token_program() != TOKEN_2022_PROGRAM_ID {
            return Err(Error::CapabilitySelectionMismatch);
        }
        Ok(Self {
            descriptor_key,
            descriptor,
            product_instance_id: product.instance_id().to_bytes(),
            claim_basis_id: product.instance().claim_basis_id().to_bytes(),
            result_domain_id: product.instance().result_domain_id().to_bytes(),
            recipe: product.recipe(),
        })
    }

    /// Recheck immutable Market key, generation, and Product coordinates.
    pub fn validate_market(
        self,
        market_key: [u8; ID_BYTES],
        market: &CategoricalMarketV1<N>,
    ) -> Result<()> {
        if self.descriptor.market != market_key {
            return Err(Error::MarketMismatch);
        }
        let identity = market.root().identity();
        if self.descriptor.generation != identity.generation() {
            return Err(Error::GenerationMismatch);
        }
        if self.product_instance_id != identity.product_instance_id().to_bytes() {
            return Err(Error::ProductInstanceMismatch);
        }
        if self.claim_basis_id != identity.claim_basis_id().to_bytes() {
            return Err(Error::ClaimBasisMismatch);
        }
        Ok(())
    }

    /// Return the authenticated descriptor account key.
    pub const fn descriptor_key(self) -> [u8; ID_BYTES] {
        self.descriptor_key
    }

    /// Return the immutable descriptor.
    pub const fn descriptor(self) -> StructuredDescriptorV1 {
        self.descriptor
    }

    /// Return the bound Product result-domain identity.
    pub const fn result_domain_id(self) -> [u8; ID_BYTES] {
        self.result_domain_id
    }

    /// Return exact backing for one structured receipt atom.
    pub const fn recipe(self) -> BackingRecipeV1<N> {
        self.recipe
    }
}

/// Ordered descriptor PDA seed projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredDescriptorDerivationV1 {
    market: [u8; ID_BYTES],
    generation_le: [u8; 8],
    template_id: [u8; ID_BYTES],
    config_id: [u8; ID_BYTES],
    release_id: [u8; ID_BYTES],
}

impl StructuredDescriptorDerivationV1 {
    /// Construct from the complete immutable descriptor coordinate.
    pub fn new(descriptor: StructuredDescriptorV1) -> Result<Self> {
        require_nonzero(&descriptor.market)?;
        Ok(Self {
            market: descriptor.market,
            generation_le: descriptor.generation.to_le_bytes(),
            template_id: descriptor.portfolio_template_id,
            config_id: descriptor.capability_config_id,
            release_id: descriptor.capability_release_id,
        })
    }

    /// Return exact PDA seed components in canonical order.
    pub fn seeds(&self) -> [&[u8]; 6] {
        [
            STRUCTURED_DESCRIPTOR_PDA_DOMAIN_V1,
            &self.market,
            &self.generation_le,
            &self.template_id,
            &self.config_id,
            &self.release_id,
        ]
    }
}

/// Ordered one-domain-plus-descriptor-key PDA seed projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredChildDerivationV1 {
    domain: &'static [u8],
    descriptor_key: [u8; ID_BYTES],
}

impl StructuredChildDerivationV1 {
    fn new(domain: &'static [u8], descriptor_key: [u8; ID_BYTES]) -> Result<Self> {
        require_nonzero(&descriptor_key)?;
        Ok(Self {
            domain,
            descriptor_key,
        })
    }

    /// Return exact PDA seed components in canonical order.
    pub fn seeds(&self) -> [&[u8]; 2] {
        [self.domain, &self.descriptor_key]
    }
}

/// Return the canonical receipt-Mint PDA seed projection.
pub fn receipt_mint_derivation_v1(
    descriptor_key: [u8; ID_BYTES],
) -> Result<StructuredChildDerivationV1> {
    StructuredChildDerivationV1::new(STRUCTURED_RECEIPT_MINT_PDA_DOMAIN_V1, descriptor_key)
}

/// Return the canonical receipt-controller PDA seed projection.
pub fn receipt_authority_derivation_v1(
    descriptor_key: [u8; ID_BYTES],
) -> Result<StructuredChildDerivationV1> {
    StructuredChildDerivationV1::new(STRUCTURED_RECEIPT_AUTHORITY_PDA_DOMAIN_V1, descriptor_key)
}

/// Return the canonical custody-owner PDA seed projection.
pub fn custody_owner_derivation_v1(
    descriptor_key: [u8; ID_BYTES],
) -> Result<StructuredChildDerivationV1> {
    StructuredChildDerivationV1::new(STRUCTURED_CUSTODY_OWNER_PDA_DOMAIN_V1, descriptor_key)
}

/// Validate one manifest entry as this exact Structured V1 capability.
///
/// Rent and creation may be prepaid native lamports. Every economic/work
/// compartment is zero, and Realm collateral is forbidden, so neither Hoard
/// principal nor future fee revenue can capitalize this child.
pub fn validate_structured_capability_entry_v1(
    entry: CapabilityEntryV1,
    expected_config_id: [u8; ID_BYTES],
) -> Result<()> {
    if entry.kind_id().to_bytes() != STRUCTURED_CAPABILITY_KIND_ID_V1
        || entry.release_id().to_bytes() != STRUCTURED_SEMANTIC_RELEASE_ID_V1
        || entry.config_id().to_bytes() != expected_config_id
        || entry.capacity_profile_id().to_bytes() != STRUCTURED_CAPACITY_ID_V1
        || entry.child_schema_id().to_bytes() != STRUCTURED_CHILD_SCHEMA_ID_V1
        || entry.child_derivation_id().to_bytes() != STRUCTURED_CHILD_DERIVATION_ID_V1
        || entry.dependency_count() != 0
    {
        return Err(Error::CapabilitySelectionMismatch);
    }
    let quote = entry.funding_quote();
    let amounts = quote.amounts();
    if quote.realm_collateral().is_some()
        || !matches!(
            amounts.rent().asset_class(),
            FundingAssetClassV1::NotApplicable | FundingAssetClassV1::NativeLamports
        )
        || !matches!(
            amounts.creation().asset_class(),
            FundingAssetClassV1::NotApplicable | FundingAssetClassV1::NativeLamports
        )
        || amounts.work().amount() != 0
        || amounts.provider().amount() != 0
        || amounts.bounty().amount() != 0
        || amounts.liquidity().amount() != 0
        || amounts.service().amount() != 0
    {
        return Err(Error::CapabilitySelectionMismatch);
    }
    Ok(())
}

pub(crate) fn validate_width<const N: usize>() -> Result<()> {
    if !(MIN_STRUCTURED_OUTCOMES..=MAX_STRUCTURED_OUTCOMES).contains(&N) {
        Err(Error::InvalidOutcomeCount)
    } else {
        Ok(())
    }
}

fn validate_dynamic_width(width: u8) -> Result<()> {
    if !(MIN_STRUCTURED_OUTCOMES..=MAX_STRUCTURED_OUTCOMES).contains(&usize::from(width)) {
        Err(Error::InvalidOutcomeCount)
    } else {
        Ok(())
    }
}

fn require_distinct_physical_identities(input: &StructuredDescriptorInputV1) -> Result<()> {
    let physical = [
        input.market,
        input.receipt_mint,
        input.receipt_authority,
        input.custody_position,
        input.custody_owner,
    ];
    for (index, value) in physical.iter().enumerate() {
        if physical.iter().take(index).any(|prior| prior == value) {
            return Err(Error::AccountAlias);
        }
    }
    Ok(())
}

fn derive_minimum_realization_lot<const N: usize>(template: PortfolioTemplateV1<N>) -> Result<u64> {
    let denominator = template.denominator();
    let mut lot = 1u64;
    for coefficient in template.coefficients() {
        let divisor = denominator
            .checked_div(gcd(denominator, *coefficient))
            .ok_or(Error::ArithmeticOverflow)?;
        lot = lcm(lot, divisor)?;
    }
    Ok(lot)
}

fn gcd(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

fn lcm(left: u64, right: u64) -> Result<u64> {
    left.checked_div(gcd(left, right))
        .and_then(|reduced| reduced.checked_mul(right))
        .ok_or(Error::ArithmeticOverflow)
}

fn product_error(error: dclutch_product_contract::Error) -> Error {
    Error::ProductContract { error }
}
