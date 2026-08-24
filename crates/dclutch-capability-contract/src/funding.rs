//! Typed capability-funding quotes, custody observations, and transitions.
//!
//! Native lamports and immutable Realm collateral are distinct dimensions.
//! This module deliberately exposes no operation that sums or converts them.

use crate::{
    ActivationPolicy, CapabilityManifestV1, ContentId, Error, Result, copy_content_id,
    copy_infallible, put_byte, put_u16, put_u64, read_array, read_byte, read_content_id, read_u16,
    read_u64, require_nonzero_identifier, require_zero, subslice,
};

/// Exact width of one typed compartment allocation.
pub const FUNDING_ALLOCATION_BYTES: usize = 16;
/// Exact width of seven typed compartments plus two independent totals.
pub const FUNDING_AMOUNTS_BYTES: usize = 128;
/// Exact width of an optional Realm-collateral binding.
pub const REALM_COLLATERAL_BINDING_BYTES: usize = 160;
/// Exact immutable funding-quote width.
pub const FUNDING_QUOTE_BYTES: usize = 304;
/// Exact mutable funding-state width.
pub const FUNDING_STATE_BYTES: usize = 320;

/// Canonical funding-quote magic.
pub const FUNDING_QUOTE_MAGIC: [u8; 8] = *b"DCLTFQ01";
/// Implemented typed funding-quote schema.
pub const FUNDING_QUOTE_SCHEMA_VERSION: u16 = 1;
/// Canonical funding-state magic.
pub const FUNDING_STATE_MAGIC: [u8; 8] = *b"DCLTCFS1";
/// Implemented typed funding-state schema.
pub const FUNDING_STATE_SCHEMA_VERSION: u16 = 1;

/// Adapter PDA seed domain for a manifest-selected funding-state account.
pub const CAPABILITY_FUNDING_PDA_DOMAIN_V1: &[u8] = b"dclutch/cap-funding/v1";
/// Adapter PDA seed domain for its token-signing funding authority.
pub const CAPABILITY_FUNDING_AUTHORITY_PDA_DOMAIN_V1: &[u8] = b"dclutch/cap-fund-auth/v1";
/// Adapter PDA seed domain for its optional Realm-collateral vault.
pub const CAPABILITY_FUNDING_VAULT_PDA_DOMAIN_V1: &[u8] = b"dclutch/cap-fund-vault/v1";

const QUOTE_SCHEMA_OFFSET: usize = 8;
const QUOTE_COLLATERAL_KIND_OFFSET: usize = 10;
const QUOTE_RESERVED_OFFSET: usize = 11;
const QUOTE_RESERVED_BYTES: usize = 5;
const QUOTE_COLLATERAL_BINDING_OFFSET: usize = 16;
const QUOTE_AMOUNTS_OFFSET: usize =
    QUOTE_COLLATERAL_BINDING_OFFSET + REALM_COLLATERAL_BINDING_BYTES;

const ALLOCATION_CLASS_OFFSET: usize = 0;
const ALLOCATION_RESERVED_OFFSET: usize = 1;
const ALLOCATION_RESERVED_BYTES: usize = 7;
const ALLOCATION_AMOUNT_OFFSET: usize = 8;

const AMOUNTS_RENT_OFFSET: usize = 0;
const AMOUNTS_CREATION_OFFSET: usize = 16;
const AMOUNTS_WORK_OFFSET: usize = 32;
const AMOUNTS_PROVIDER_OFFSET: usize = 48;
const AMOUNTS_BOUNTY_OFFSET: usize = 64;
const AMOUNTS_LIQUIDITY_OFFSET: usize = 80;
const AMOUNTS_SERVICE_OFFSET: usize = 96;
const AMOUNTS_NATIVE_TOTAL_OFFSET: usize = 112;
const AMOUNTS_REALM_TOTAL_OFFSET: usize = 120;

const BINDING_REALM_ID_OFFSET: usize = 0;
const BINDING_RELEASE_ID_OFFSET: usize = 32;
const BINDING_TOKEN_PROGRAM_OFFSET: usize = 64;
const BINDING_MINT_OFFSET: usize = 96;
const BINDING_BENEFICIARY_OFFSET: usize = 128;

const STATE_SCHEMA_OFFSET: usize = 8;
const STATE_STATUS_OFFSET: usize = 10;
const STATE_HEADER_RESERVED_OFFSET: usize = 11;
const STATE_HEADER_RESERVED_BYTES: usize = 5;
const STATE_MANIFEST_ID_OFFSET: usize = 16;
const STATE_ENTRY_INDEX_OFFSET: usize = 48;
const STATE_BODY_RESERVED_OFFSET: usize = 50;
const STATE_BODY_RESERVED_BYTES: usize = 6;
const STATE_ACTIVATION_SLOT_OFFSET: usize = 56;
const STATE_REMAINING_OFFSET: usize = 64;
const STATE_RELEASED_OFFSET: usize = STATE_REMAINING_OFFSET + FUNDING_AMOUNTS_BYTES;

/// Asset class of one funding compartment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FundingAssetClassV1 {
    /// Canonical representation of a zero, inapplicable compartment.
    NotApplicable = 0,
    /// Native SVM lamports.
    NativeLamports = 1,
    /// Atomic units of the immutable Realm-selected collateral mint.
    RealmCollateral = 2,
}

impl FundingAssetClassV1 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::NotApplicable),
            1 => Ok(Self::NativeLamports),
            2 => Ok(Self::RealmCollateral),
            _ => Err(Error::UnknownFundingAssetClass),
        }
    }

    const fn byte(self) -> u8 {
        self as u8
    }
}

/// Whether a compartment's asset class is mathematical or capability-selected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FundingAssetPolicyV1 {
    /// Any nonzero amount is intrinsically native lamports.
    NativeLamportsOnly,
    /// The immutable capability quote selects lamports or Realm collateral.
    CapabilitySelected,
}

/// One canonical typed compartment amount.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompartmentFundingV1 {
    asset_class: FundingAssetClassV1,
    amount: u64,
}

impl Default for CompartmentFundingV1 {
    fn default() -> Self {
        Self::not_applicable()
    }
}

impl CompartmentFundingV1 {
    /// Construct the only canonical zero representation.
    pub const fn not_applicable() -> Self {
        Self {
            asset_class: FundingAssetClassV1::NotApplicable,
            amount: 0,
        }
    }

    /// Construct a positive native-lamport amount.
    pub fn native_lamports(amount: u64) -> Result<Self> {
        Self::new(FundingAssetClassV1::NativeLamports, amount)
    }

    /// Construct a positive immutable Realm-collateral amount.
    pub fn realm_collateral(amount: u64) -> Result<Self> {
        Self::new(FundingAssetClassV1::RealmCollateral, amount)
    }

    fn new(asset_class: FundingAssetClassV1, amount: u64) -> Result<Self> {
        match (asset_class, amount) {
            (FundingAssetClassV1::NotApplicable, 0) => Ok(Self::not_applicable()),
            (FundingAssetClassV1::NativeLamports | FundingAssetClassV1::RealmCollateral, 1..) => {
                Ok(Self {
                    asset_class,
                    amount,
                })
            }
            _ => Err(Error::NonCanonicalFundingAssetClass),
        }
    }

    fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != FUNDING_ALLOCATION_BYTES {
            return Err(Error::InvalidLength);
        }
        require_zero(bytes, ALLOCATION_RESERVED_OFFSET, ALLOCATION_RESERVED_BYTES)?;
        Self::new(
            FundingAssetClassV1::decode(read_byte(bytes, ALLOCATION_CLASS_OFFSET)?)?,
            read_u64(bytes, ALLOCATION_AMOUNT_OFFSET)?,
        )
    }

    fn to_bytes(self) -> [u8; FUNDING_ALLOCATION_BYTES] {
        let mut output = [0u8; FUNDING_ALLOCATION_BYTES];
        put_byte(
            &mut output,
            ALLOCATION_CLASS_OFFSET,
            self.asset_class.byte(),
        );
        put_u64(&mut output, ALLOCATION_AMOUNT_OFFSET, self.amount);
        output
    }

    /// Return the exact asset class.
    pub const fn asset_class(self) -> FundingAssetClassV1 {
        self.asset_class
    }

    /// Return the exact amount in that class's atomic units.
    pub const fn amount(self) -> u64 {
        self.amount
    }
}

/// One segregated capability-funding compartment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FundingCompartment {
    /// Child-account rent; intrinsically native lamports.
    Rent,
    /// Physical-creation funding; intrinsically native lamports.
    Creation,
    /// Ongoing work funding selected by the capability profile.
    Work,
    /// Provider funding selected by the capability profile.
    Provider,
    /// Bounty funding selected by the capability profile.
    Bounty,
    /// Liquidity funding selected by the capability profile.
    Liquidity,
    /// Service/liveness funding selected by the capability profile.
    Service,
}

impl FundingCompartment {
    /// Return whether this compartment's class is fixed or capability-selected.
    pub const fn asset_policy(self) -> FundingAssetPolicyV1 {
        match self {
            Self::Rent | Self::Creation => FundingAssetPolicyV1::NativeLamportsOnly,
            Self::Work | Self::Provider | Self::Bounty | Self::Liquidity | Self::Service => {
                FundingAssetPolicyV1::CapabilitySelected
            }
        }
    }
}

/// Seven typed compartments with separate checked totals per physical asset.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FundingAmountsV1 {
    rent: CompartmentFundingV1,
    creation: CompartmentFundingV1,
    work: CompartmentFundingV1,
    provider: CompartmentFundingV1,
    bounty: CompartmentFundingV1,
    liquidity: CompartmentFundingV1,
    service: CompartmentFundingV1,
    native_lamports_total: u64,
    realm_collateral_total: u64,
}

impl FundingAmountsV1 {
    /// Construct typed compartments and each class's independent checked total.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        rent: CompartmentFundingV1,
        creation: CompartmentFundingV1,
        work: CompartmentFundingV1,
        provider: CompartmentFundingV1,
        bounty: CompartmentFundingV1,
        liquidity: CompartmentFundingV1,
        service: CompartmentFundingV1,
    ) -> Result<Self> {
        for fixed in [rent, creation] {
            if !matches!(
                fixed.asset_class,
                FundingAssetClassV1::NotApplicable | FundingAssetClassV1::NativeLamports
            ) {
                return Err(Error::InvalidCompartmentAssetClass);
            }
        }
        let values = [rent, creation, work, provider, bounty, liquidity, service];
        let native_lamports_total =
            checked_asset_sum(&values, FundingAssetClassV1::NativeLamports)?;
        let realm_collateral_total =
            checked_asset_sum(&values, FundingAssetClassV1::RealmCollateral)?;
        Ok(Self {
            rent,
            creation,
            work,
            provider,
            bounty,
            liquidity,
            service,
            native_lamports_total,
            realm_collateral_total,
        })
    }

    /// Decode one exact canonical typed compartment record.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != FUNDING_AMOUNTS_BYTES {
            return Err(Error::InvalidLength);
        }
        let result = Self::new(
            decode_allocation(bytes, AMOUNTS_RENT_OFFSET)?,
            decode_allocation(bytes, AMOUNTS_CREATION_OFFSET)?,
            decode_allocation(bytes, AMOUNTS_WORK_OFFSET)?,
            decode_allocation(bytes, AMOUNTS_PROVIDER_OFFSET)?,
            decode_allocation(bytes, AMOUNTS_BOUNTY_OFFSET)?,
            decode_allocation(bytes, AMOUNTS_LIQUIDITY_OFFSET)?,
            decode_allocation(bytes, AMOUNTS_SERVICE_OFFSET)?,
        )?;
        if result.native_lamports_total != read_u64(bytes, AMOUNTS_NATIVE_TOTAL_OFFSET)?
            || result.realm_collateral_total != read_u64(bytes, AMOUNTS_REALM_TOTAL_OFFSET)?
        {
            return Err(Error::FundingAssetTotalMismatch);
        }
        Ok(result)
    }

    /// Return the exact canonical bytes.
    pub fn to_bytes(self) -> [u8; FUNDING_AMOUNTS_BYTES] {
        let mut output = [0u8; FUNDING_AMOUNTS_BYTES];
        encode_allocation(&mut output, AMOUNTS_RENT_OFFSET, self.rent);
        encode_allocation(&mut output, AMOUNTS_CREATION_OFFSET, self.creation);
        encode_allocation(&mut output, AMOUNTS_WORK_OFFSET, self.work);
        encode_allocation(&mut output, AMOUNTS_PROVIDER_OFFSET, self.provider);
        encode_allocation(&mut output, AMOUNTS_BOUNTY_OFFSET, self.bounty);
        encode_allocation(&mut output, AMOUNTS_LIQUIDITY_OFFSET, self.liquidity);
        encode_allocation(&mut output, AMOUNTS_SERVICE_OFFSET, self.service);
        put_u64(
            &mut output,
            AMOUNTS_NATIVE_TOTAL_OFFSET,
            self.native_lamports_total,
        );
        put_u64(
            &mut output,
            AMOUNTS_REALM_TOTAL_OFFSET,
            self.realm_collateral_total,
        );
        output
    }

    /// Return one typed compartment.
    pub const fn compartment(self, compartment: FundingCompartment) -> CompartmentFundingV1 {
        match compartment {
            FundingCompartment::Rent => self.rent,
            FundingCompartment::Creation => self.creation,
            FundingCompartment::Work => self.work,
            FundingCompartment::Provider => self.provider,
            FundingCompartment::Bounty => self.bounty,
            FundingCompartment::Liquidity => self.liquidity,
            FundingCompartment::Service => self.service,
        }
    }

    /// Return Rent funding.
    pub const fn rent(self) -> CompartmentFundingV1 {
        self.rent
    }
    /// Return Creation funding.
    pub const fn creation(self) -> CompartmentFundingV1 {
        self.creation
    }
    /// Return Work funding.
    pub const fn work(self) -> CompartmentFundingV1 {
        self.work
    }
    /// Return Provider funding.
    pub const fn provider(self) -> CompartmentFundingV1 {
        self.provider
    }
    /// Return Bounty funding.
    pub const fn bounty(self) -> CompartmentFundingV1 {
        self.bounty
    }
    /// Return Liquidity funding.
    pub const fn liquidity(self) -> CompartmentFundingV1 {
        self.liquidity
    }
    /// Return Service funding.
    pub const fn service(self) -> CompartmentFundingV1 {
        self.service
    }
    /// Return the checked lamport total without Realm collateral.
    pub const fn native_lamports_total(self) -> u64 {
        self.native_lamports_total
    }
    /// Return the checked Realm-collateral total without lamports.
    pub const fn realm_collateral_total(self) -> u64 {
        self.realm_collateral_total
    }

    fn with_compartment(
        self,
        compartment: FundingCompartment,
        value: CompartmentFundingV1,
    ) -> Result<Self> {
        Self::new(
            if compartment == FundingCompartment::Rent {
                value
            } else {
                self.rent
            },
            if compartment == FundingCompartment::Creation {
                value
            } else {
                self.creation
            },
            if compartment == FundingCompartment::Work {
                value
            } else {
                self.work
            },
            if compartment == FundingCompartment::Provider {
                value
            } else {
                self.provider
            },
            if compartment == FundingCompartment::Bounty {
                value
            } else {
                self.bounty
            },
            if compartment == FundingCompartment::Liquidity {
                value
            } else {
                self.liquidity
            },
            if compartment == FundingCompartment::Service {
                value
            } else {
                self.service
            },
        )
    }
}

/// Immutable binding for the one Realm-selected collateral asset.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RealmCollateralBindingV1 {
    realm_id: ContentId,
    collateral_release_id: ContentId,
    token_program: [u8; 32],
    mint: [u8; 32],
    refund_token_beneficiary: [u8; 32],
}

impl RealmCollateralBindingV1 {
    /// Construct one immutable Realm/release/token/mint/refund binding.
    pub fn new(
        realm_id: ContentId,
        collateral_release_id: ContentId,
        token_program: [u8; 32],
        mint: [u8; 32],
        refund_token_beneficiary: [u8; 32],
    ) -> Result<Self> {
        require_nonzero_identifier(&token_program)?;
        require_nonzero_identifier(&mint)?;
        require_nonzero_identifier(&refund_token_beneficiary)?;
        Ok(Self {
            realm_id,
            collateral_release_id,
            token_program,
            mint,
            refund_token_beneficiary,
        })
    }

    fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != REALM_COLLATERAL_BINDING_BYTES {
            return Err(Error::InvalidLength);
        }
        Self::new(
            read_content_id(bytes, BINDING_REALM_ID_OFFSET)?,
            read_content_id(bytes, BINDING_RELEASE_ID_OFFSET)?,
            read_array(bytes, BINDING_TOKEN_PROGRAM_OFFSET)?,
            read_array(bytes, BINDING_MINT_OFFSET)?,
            read_array(bytes, BINDING_BENEFICIARY_OFFSET)?,
        )
    }

    fn to_bytes(self) -> [u8; REALM_COLLATERAL_BINDING_BYTES] {
        let mut output = [0u8; REALM_COLLATERAL_BINDING_BYTES];
        copy_content_id(&mut output, BINDING_REALM_ID_OFFSET, self.realm_id);
        copy_content_id(
            &mut output,
            BINDING_RELEASE_ID_OFFSET,
            self.collateral_release_id,
        );
        copy_infallible(
            &mut output,
            BINDING_TOKEN_PROGRAM_OFFSET,
            &self.token_program,
        );
        copy_infallible(&mut output, BINDING_MINT_OFFSET, &self.mint);
        copy_infallible(
            &mut output,
            BINDING_BENEFICIARY_OFFSET,
            &self.refund_token_beneficiary,
        );
        output
    }

    /// Return the immutable Realm identity.
    pub const fn realm_id(self) -> ContentId {
        self.realm_id
    }
    /// Return the immutable Realm collateral-release identity.
    pub const fn collateral_release_id(self) -> ContentId {
        self.collateral_release_id
    }
    /// Return the immutable token-program key.
    pub const fn token_program(self) -> [u8; 32] {
        self.token_program
    }
    /// Return the immutable collateral-mint key.
    pub const fn mint(self) -> [u8; 32] {
        self.mint
    }
    /// Return the immutable token account receiving close refunds and donations.
    pub const fn refund_token_beneficiary(self) -> [u8; 32] {
        self.refund_token_beneficiary
    }
}

/// Immutable, canonically encoded typed funding quote.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FundingQuoteV1 {
    amounts: FundingAmountsV1,
    realm_collateral: Option<RealmCollateralBindingV1>,
}

impl FundingQuoteV1 {
    /// Construct a quote whose optional binding exactly matches collateral use.
    pub fn new(
        amounts: FundingAmountsV1,
        realm_collateral: Option<RealmCollateralBindingV1>,
    ) -> Result<Self> {
        match (amounts.realm_collateral_total(), realm_collateral) {
            (0, None) | (1.., Some(_)) => Ok(Self {
                amounts,
                realm_collateral,
            }),
            (0, Some(_)) => Err(Error::UnexpectedRealmCollateralBinding),
            (1.., None) => Err(Error::MissingRealmCollateralBinding),
        }
    }

    /// Decode one exact canonical quote preimage.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != FUNDING_QUOTE_BYTES {
            return Err(Error::InvalidLength);
        }
        if read_array::<8>(bytes, 0)? != FUNDING_QUOTE_MAGIC {
            return Err(Error::InvalidMagic);
        }
        if read_u16(bytes, QUOTE_SCHEMA_OFFSET)? != FUNDING_QUOTE_SCHEMA_VERSION {
            return Err(Error::UnsupportedSchema);
        }
        require_zero(bytes, QUOTE_RESERVED_OFFSET, QUOTE_RESERVED_BYTES)?;
        let binding_bytes = subslice(
            bytes,
            QUOTE_COLLATERAL_BINDING_OFFSET,
            REALM_COLLATERAL_BINDING_BYTES,
        )?;
        let realm_collateral = match read_byte(bytes, QUOTE_COLLATERAL_KIND_OFFSET)? {
            0 => {
                require_zero(binding_bytes, 0, REALM_COLLATERAL_BINDING_BYTES)?;
                None
            }
            1 => Some(RealmCollateralBindingV1::decode(binding_bytes)?),
            _ => return Err(Error::UnknownFundingAssetClass),
        };
        Self::new(
            FundingAmountsV1::decode(subslice(
                bytes,
                QUOTE_AMOUNTS_OFFSET,
                FUNDING_AMOUNTS_BYTES,
            )?)?,
            realm_collateral,
        )
    }

    /// Return the exact canonical quote content preimage.
    pub fn to_bytes(self) -> [u8; FUNDING_QUOTE_BYTES] {
        let mut output = [0u8; FUNDING_QUOTE_BYTES];
        copy_infallible(&mut output, 0, &FUNDING_QUOTE_MAGIC);
        put_u16(
            &mut output,
            QUOTE_SCHEMA_OFFSET,
            FUNDING_QUOTE_SCHEMA_VERSION,
        );
        if let Some(binding) = self.realm_collateral {
            put_byte(&mut output, QUOTE_COLLATERAL_KIND_OFFSET, 1);
            copy_infallible(
                &mut output,
                QUOTE_COLLATERAL_BINDING_OFFSET,
                &binding.to_bytes(),
            );
        }
        copy_infallible(&mut output, QUOTE_AMOUNTS_OFFSET, &self.amounts.to_bytes());
        output
    }

    /// Return all typed compartments and separate totals.
    pub const fn amounts(self) -> FundingAmountsV1 {
        self.amounts
    }
    /// Return the optional immutable Realm collateral binding.
    pub const fn realm_collateral(self) -> Option<RealmCollateralBindingV1> {
        self.realm_collateral
    }
    /// Return the independent native-lamport total.
    pub const fn native_lamports_total(self) -> u64 {
        self.amounts.native_lamports_total()
    }
    /// Return the independent Realm-collateral total.
    pub const fn realm_collateral_total(self) -> u64 {
        self.amounts.realm_collateral_total()
    }
}

/// Mutable lifecycle status for one capability's prepaid funding state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FundingStatus {
    /// Exact typed quote custody is present; physical activation has not run.
    Pending = 0,
    /// Physical activation completed with exact native Rent/Creation release.
    Active = 1,
}

impl FundingStatus {
    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Pending),
            1 => Ok(Self::Active),
            _ => Err(Error::UnknownFundingStatus),
        }
    }

    const fn byte(self) -> u8 {
        self as u8
    }
}

/// Exact observed Realm token-vault account state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RealmCollateralVaultObservationV1 {
    vault: [u8; 32],
    authority: [u8; 32],
    token_program: [u8; 32],
    mint: [u8; 32],
    token_amount: u64,
    account_lamports: u64,
    exact_rent_lamports: u64,
}

impl RealmCollateralVaultObservationV1 {
    /// Construct one adapter-observed token vault without interpreting units.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        vault: [u8; 32],
        authority: [u8; 32],
        token_program: [u8; 32],
        mint: [u8; 32],
        token_amount: u64,
        account_lamports: u64,
        exact_rent_lamports: u64,
    ) -> Result<Self> {
        for key in [vault, authority, token_program, mint] {
            require_nonzero_identifier(&key)?;
        }
        if account_lamports < exact_rent_lamports {
            return Err(Error::UnderfundedPhysicalCustody);
        }
        Ok(Self {
            vault,
            authority,
            token_program,
            mint,
            token_amount,
            account_lamports,
            exact_rent_lamports,
        })
    }

    /// Return the observed vault key.
    pub const fn vault(self) -> [u8; 32] {
        self.vault
    }
    /// Return the observed token authority.
    pub const fn authority(self) -> [u8; 32] {
        self.authority
    }
    /// Return the observed token program.
    pub const fn token_program(self) -> [u8; 32] {
        self.token_program
    }
    /// Return the observed mint.
    pub const fn mint(self) -> [u8; 32] {
        self.mint
    }
    /// Return exact observed Realm-collateral atomic units.
    pub const fn token_amount(self) -> u64 {
        self.token_amount
    }
    /// Return all observed vault-account lamports.
    pub const fn account_lamports(self) -> u64 {
        self.account_lamports
    }
    /// Return the adapter-calculated exact vault rent reserve.
    pub const fn exact_rent_lamports(self) -> u64 {
        self.exact_rent_lamports
    }
}

/// Adapter-authenticated Realm and canonical funding-PDA observations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RealmCollateralCustodyV1 {
    realm_id: ContentId,
    collateral_release_id: ContentId,
    canonical_funding_authority: [u8; 32],
    canonical_vault: [u8; 32],
    observation: RealmCollateralVaultObservationV1,
}

impl RealmCollateralCustodyV1 {
    /// Bind an observed vault to authenticated Realm and PDA derivations.
    pub fn new(
        realm_id: ContentId,
        collateral_release_id: ContentId,
        canonical_funding_authority: [u8; 32],
        canonical_vault: [u8; 32],
        observation: RealmCollateralVaultObservationV1,
    ) -> Result<Self> {
        require_nonzero_identifier(&canonical_funding_authority)?;
        require_nonzero_identifier(&canonical_vault)?;
        if observation.authority != canonical_funding_authority {
            return Err(Error::FundingAuthorityMismatch);
        }
        if observation.vault != canonical_vault {
            return Err(Error::FundingVaultMismatch);
        }
        Ok(Self {
            realm_id,
            collateral_release_id,
            canonical_funding_authority,
            canonical_vault,
            observation,
        })
    }

    /// Return the authenticated Realm identity.
    pub const fn realm_id(self) -> ContentId {
        self.realm_id
    }
    /// Return the authenticated collateral release identity.
    pub const fn collateral_release_id(self) -> ContentId {
        self.collateral_release_id
    }
    /// Return the canonical funding-authority PDA key.
    pub const fn canonical_funding_authority(self) -> [u8; 32] {
        self.canonical_funding_authority
    }
    /// Return the canonical token-vault PDA key.
    pub const fn canonical_vault(self) -> [u8; 32] {
        self.canonical_vault
    }
    /// Return the exact observed vault state.
    pub const fn observation(self) -> RealmCollateralVaultObservationV1 {
        self.observation
    }
}

/// Exact physical custody observation for both nonconvertible asset dimensions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FundingCustodyObservationV1 {
    state_account_lamports: u64,
    exact_state_rent_lamports: u64,
    realm_collateral: Option<RealmCollateralCustodyV1>,
}

impl FundingCustodyObservationV1 {
    /// Observe a native-only program-owned funding-state account.
    pub fn native_only(
        state_account_lamports: u64,
        exact_state_rent_lamports: u64,
    ) -> Result<Self> {
        Self::new(state_account_lamports, exact_state_rent_lamports, None)
    }

    /// Observe the funding-state account and its Realm token vault.
    pub fn with_realm_collateral(
        state_account_lamports: u64,
        exact_state_rent_lamports: u64,
        realm_collateral: RealmCollateralCustodyV1,
    ) -> Result<Self> {
        Self::new(
            state_account_lamports,
            exact_state_rent_lamports,
            Some(realm_collateral),
        )
    }

    fn new(
        state_account_lamports: u64,
        exact_state_rent_lamports: u64,
        realm_collateral: Option<RealmCollateralCustodyV1>,
    ) -> Result<Self> {
        if state_account_lamports < exact_state_rent_lamports {
            return Err(Error::UnderfundedPhysicalCustody);
        }
        Ok(Self {
            state_account_lamports,
            exact_state_rent_lamports,
            realm_collateral,
        })
    }

    /// Return all lamports observed in the program-owned state account.
    pub const fn state_account_lamports(self) -> u64 {
        self.state_account_lamports
    }
    /// Return the exact current Rent minimum for the state width.
    pub const fn exact_state_rent_lamports(self) -> u64 {
        self.exact_state_rent_lamports
    }
    /// Return lamports held above the state account's own Rent reserve.
    pub fn present_native_lamports(self) -> Result<u64> {
        self.state_account_lamports
            .checked_sub(self.exact_state_rent_lamports)
            .ok_or(Error::UnderfundedPhysicalCustody)
    }
    /// Return optional authenticated Realm-collateral custody.
    pub const fn realm_collateral(self) -> Option<RealmCollateralCustodyV1> {
        self.realm_collateral
    }
}

/// Exact activation transfer plan; both fields are intrinsically lamports.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActivationDebitV1 {
    rent_lamports: u64,
    creation_lamports: u64,
}

impl ActivationDebitV1 {
    /// Return child-account rent lamports released atomically at activation.
    pub const fn rent_lamports(self) -> u64 {
        self.rent_lamports
    }
    /// Return physical-creation lamports released atomically at activation.
    pub const fn creation_lamports(self) -> u64 {
        self.creation_lamports
    }
}

/// One exact, typed non-activation release plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FundingReleasePlanV1 {
    compartment: FundingCompartment,
    asset_class: FundingAssetClassV1,
    amount: u64,
}

impl FundingReleasePlanV1 {
    /// Return the released semantic compartment.
    pub const fn compartment(self) -> FundingCompartment {
        self.compartment
    }
    /// Return the physical asset class; callers cannot reinterpret its units.
    pub const fn asset_class(self) -> FundingAssetClassV1 {
        self.asset_class
    }
    /// Return the amount in that asset class's atomic units.
    pub const fn amount(self) -> u64 {
        self.amount
    }
}

/// Complete close distribution with no cross-asset total and no stranded funds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FundingClosePlanV1 {
    native_rent_credit: [u8; 32],
    remaining_native_lamports: u64,
    state_rent_lamports: u64,
    state_lamport_donation: u64,
    realm_token_beneficiary: Option<[u8; 32]>,
    remaining_realm_collateral: u64,
    realm_collateral_donation: u64,
    vault_rent_lamports: u64,
    vault_lamport_donation: u64,
}

impl FundingClosePlanV1 {
    /// Return the canonical pre-existing RentCredit receiving every lamport.
    pub const fn native_rent_credit(self) -> [u8; 32] {
        self.native_rent_credit
    }
    /// Return remaining native funding principal.
    pub const fn remaining_native_lamports(self) -> u64 {
        self.remaining_native_lamports
    }
    /// Return the funding-state account's exact Rent reserve.
    pub const fn state_rent_lamports(self) -> u64 {
        self.state_rent_lamports
    }
    /// Return unsolicited state-account lamports, routed to the same RentCredit.
    pub const fn state_lamport_donation(self) -> u64 {
        self.state_lamport_donation
    }
    /// Return the immutable Realm-token refund beneficiary, if applicable.
    pub const fn realm_token_beneficiary(self) -> Option<[u8; 32]> {
        self.realm_token_beneficiary
    }
    /// Return remaining Realm collateral principal.
    pub const fn remaining_realm_collateral(self) -> u64 {
        self.remaining_realm_collateral
    }
    /// Return unsolicited same-mint tokens, classified only as a refund gift.
    pub const fn realm_collateral_donation(self) -> u64 {
        self.realm_collateral_donation
    }
    /// Return the collateral vault's exact Rent reserve.
    pub const fn vault_rent_lamports(self) -> u64 {
        self.vault_rent_lamports
    }
    /// Return unsolicited vault lamports, routed to the same RentCredit.
    pub const fn vault_lamport_donation(self) -> u64 {
        self.vault_lamport_donation
    }
}

/// Program-owned, manifest-bound typed capability funding ledger.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FundingStateV1 {
    manifest_content_id: ContentId,
    entry_index: u16,
    status: FundingStatus,
    activation_slot: u64,
    remaining: FundingAmountsV1,
    released: FundingAmountsV1,
}

impl FundingStateV1 {
    /// Construct an exactly prepaid pending state for one manifest entry.
    pub fn new(
        manifest_content_id: ContentId,
        manifest: CapabilityManifestV1<'_>,
        entry_index: u16,
        custody: FundingCustodyObservationV1,
    ) -> Result<Self> {
        let entry = manifest.entry(entry_index)?;
        let result = Self {
            manifest_content_id,
            entry_index,
            status: FundingStatus::Pending,
            activation_slot: 0,
            remaining: entry.funding_quote().amounts(),
            released: FundingAmountsV1::default(),
        };
        result.validate_against(manifest_content_id, manifest, custody)?;
        Ok(result)
    }

    /// Decode one exact canonical typed funding-state record.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != FUNDING_STATE_BYTES {
            return Err(Error::InvalidLength);
        }
        if read_array::<8>(bytes, 0)? != FUNDING_STATE_MAGIC {
            return Err(Error::InvalidMagic);
        }
        if read_u16(bytes, STATE_SCHEMA_OFFSET)? != FUNDING_STATE_SCHEMA_VERSION {
            return Err(Error::UnsupportedSchema);
        }
        require_zero(
            bytes,
            STATE_HEADER_RESERVED_OFFSET,
            STATE_HEADER_RESERVED_BYTES,
        )?;
        require_zero(bytes, STATE_BODY_RESERVED_OFFSET, STATE_BODY_RESERVED_BYTES)?;
        let result = Self {
            manifest_content_id: read_content_id(bytes, STATE_MANIFEST_ID_OFFSET)?,
            entry_index: read_u16(bytes, STATE_ENTRY_INDEX_OFFSET)?,
            status: FundingStatus::decode(read_byte(bytes, STATE_STATUS_OFFSET)?)?,
            activation_slot: read_u64(bytes, STATE_ACTIVATION_SLOT_OFFSET)?,
            remaining: FundingAmountsV1::decode(subslice(
                bytes,
                STATE_REMAINING_OFFSET,
                FUNDING_AMOUNTS_BYTES,
            )?)?,
            released: FundingAmountsV1::decode(subslice(
                bytes,
                STATE_RELEASED_OFFSET,
                FUNDING_AMOUNTS_BYTES,
            )?)?,
        };
        if result.status == FundingStatus::Pending
            && (result.activation_slot != 0
                || result.released.native_lamports_total() != 0
                || result.released.realm_collateral_total() != 0)
        {
            return Err(Error::InvalidFundingStatus);
        }
        Ok(result)
    }

    /// Return exact canonical bytes.
    pub fn to_bytes(self) -> [u8; FUNDING_STATE_BYTES] {
        let mut output = [0u8; FUNDING_STATE_BYTES];
        copy_infallible(&mut output, 0, &FUNDING_STATE_MAGIC);
        put_u16(
            &mut output,
            STATE_SCHEMA_OFFSET,
            FUNDING_STATE_SCHEMA_VERSION,
        );
        put_byte(&mut output, STATE_STATUS_OFFSET, self.status.byte());
        copy_content_id(
            &mut output,
            STATE_MANIFEST_ID_OFFSET,
            self.manifest_content_id,
        );
        put_u16(&mut output, STATE_ENTRY_INDEX_OFFSET, self.entry_index);
        put_u64(
            &mut output,
            STATE_ACTIVATION_SLOT_OFFSET,
            self.activation_slot,
        );
        copy_infallible(
            &mut output,
            STATE_REMAINING_OFFSET,
            &self.remaining.to_bytes(),
        );
        copy_infallible(
            &mut output,
            STATE_RELEASED_OFFSET,
            &self.released.to_bytes(),
        );
        output
    }

    /// Validate quote conservation and exact physical custody per asset class.
    pub fn validate_against(
        self,
        manifest_content_id: ContentId,
        manifest: CapabilityManifestV1<'_>,
        custody: FundingCustodyObservationV1,
    ) -> Result<()> {
        let quote = self.validate_semantics(manifest_content_id, manifest)?;
        validate_custody_binding(quote, custody)?;
        if custody.present_native_lamports()? != self.remaining.native_lamports_total() {
            return Err(Error::PresentNativeLamportsMismatch);
        }
        match (quote.realm_collateral(), custody.realm_collateral()) {
            (None, None) => {}
            (Some(_), Some(observed))
                if observed.observation().token_amount()
                    == self.remaining.realm_collateral_total() => {}
            (Some(_), Some(_)) => return Err(Error::PresentRealmCollateralMismatch),
            (Some(_), None) => return Err(Error::MissingRealmCollateralVault),
            (None, Some(_)) => return Err(Error::UnexpectedRealmCollateralVault),
        }
        Ok(())
    }

    /// Determine whether this exact typed custody permits Market opening.
    pub fn validate_market_open(
        self,
        manifest_content_id: ContentId,
        manifest: CapabilityManifestV1<'_>,
        custody: FundingCustodyObservationV1,
        current_slot: u64,
    ) -> Result<()> {
        self.validate_against(manifest_content_id, manifest, custody)?;
        let entry = manifest.entry(self.entry_index)?;
        match (entry.activation_policy(), self.status) {
            (ActivationPolicy::RequiredAtFounding, FundingStatus::Pending) => {
                Err(Error::FoundingCapabilityInactive)
            }
            (ActivationPolicy::PrepaidLazy, FundingStatus::Pending)
                if current_slot > entry.activation_deadline_slot() =>
            {
                Err(Error::ActivationDeadlineElapsed)
            }
            _ => Ok(()),
        }
    }

    /// Activate and release only the intrinsically native Rent/Creation amounts.
    pub fn activate(
        &mut self,
        manifest_content_id: ContentId,
        manifest: CapabilityManifestV1<'_>,
        custody: FundingCustodyObservationV1,
        current_slot: u64,
    ) -> Result<ActivationDebitV1> {
        self.validate_against(manifest_content_id, manifest, custody)?;
        if self.status != FundingStatus::Pending {
            return Err(Error::InvalidFundingStatus);
        }
        let entry = manifest.entry(self.entry_index)?;
        if entry.activation_policy() == ActivationPolicy::PrepaidLazy
            && current_slot > entry.activation_deadline_slot()
        {
            return Err(Error::ActivationDeadlineElapsed);
        }
        let quote = entry.funding_quote().amounts();
        let debit = ActivationDebitV1 {
            rent_lamports: quote.rent().amount(),
            creation_lamports: quote.creation().amount(),
        };
        let mut next_remaining = self.remaining;
        let mut next_released = self.released;
        move_compartment(
            &mut next_remaining,
            &mut next_released,
            FundingCompartment::Rent,
            debit.rent_lamports,
        )?;
        move_compartment(
            &mut next_remaining,
            &mut next_released,
            FundingCompartment::Creation,
            debit.creation_lamports,
        )?;
        self.remaining = next_remaining;
        self.released = next_released;
        self.status = FundingStatus::Active;
        self.activation_slot = current_slot;
        Ok(debit)
    }

    /// Release one exact non-activation compartment and return its typed CPI plan.
    pub fn release(
        &mut self,
        manifest_content_id: ContentId,
        manifest: CapabilityManifestV1<'_>,
        custody: FundingCustodyObservationV1,
        compartment: FundingCompartment,
        amount: u64,
    ) -> Result<FundingReleasePlanV1> {
        self.validate_against(manifest_content_id, manifest, custody)?;
        if self.status != FundingStatus::Active {
            return Err(Error::InvalidFundingStatus);
        }
        if amount == 0 {
            return Err(Error::ZeroPrincipalRelease);
        }
        if matches!(
            compartment,
            FundingCompartment::Rent | FundingCompartment::Creation
        ) {
            return Err(Error::ActivationCompartmentRequired);
        }
        let allocation = self.remaining.compartment(compartment);
        move_compartment(&mut self.remaining, &mut self.released, compartment, amount)?;
        Ok(FundingReleasePlanV1 {
            compartment,
            asset_class: allocation.asset_class(),
            amount,
        })
    }

    /// Plan terminal/abandonment close, classifying every donation explicitly.
    ///
    /// The adapter must authenticate `native_rent_credit` from the immutable
    /// Market root, execute all returned transfers and account closes, and
    /// remove this state atomically. No plan field is protocol revenue.
    pub fn close(
        self,
        manifest_content_id: ContentId,
        manifest: CapabilityManifestV1<'_>,
        custody: FundingCustodyObservationV1,
        native_rent_credit: [u8; 32],
    ) -> Result<FundingClosePlanV1> {
        require_nonzero_identifier(&native_rent_credit)?;
        let quote = self.validate_semantics(manifest_content_id, manifest)?;
        validate_custody_binding(quote, custody)?;
        let observed_native = custody.present_native_lamports()?;
        let required_native = self.remaining.native_lamports_total();
        let state_lamport_donation = observed_native
            .checked_sub(required_native)
            .ok_or(Error::UnderfundedPhysicalCustody)?;
        let (
            realm_token_beneficiary,
            remaining_realm_collateral,
            realm_collateral_donation,
            vault_rent_lamports,
            vault_lamport_donation,
        ) = match (quote.realm_collateral(), custody.realm_collateral()) {
            (None, None) => (None, 0, 0, 0, 0),
            (Some(binding), Some(realm)) => {
                let observed = realm.observation();
                let required = self.remaining.realm_collateral_total();
                let donation = observed
                    .token_amount()
                    .checked_sub(required)
                    .ok_or(Error::UnderfundedPhysicalCustody)?;
                let vault_lamport_donation = observed
                    .account_lamports()
                    .checked_sub(observed.exact_rent_lamports())
                    .ok_or(Error::UnderfundedPhysicalCustody)?;
                (
                    Some(binding.refund_token_beneficiary()),
                    required,
                    donation,
                    observed.exact_rent_lamports(),
                    vault_lamport_donation,
                )
            }
            (Some(_), None) => return Err(Error::MissingRealmCollateralVault),
            (None, Some(_)) => return Err(Error::UnexpectedRealmCollateralVault),
        };
        Ok(FundingClosePlanV1 {
            native_rent_credit,
            remaining_native_lamports: required_native,
            state_rent_lamports: custody.exact_state_rent_lamports(),
            state_lamport_donation,
            realm_token_beneficiary,
            remaining_realm_collateral,
            realm_collateral_donation,
            vault_rent_lamports,
            vault_lamport_donation,
        })
    }

    fn validate_semantics(
        self,
        manifest_content_id: ContentId,
        manifest: CapabilityManifestV1<'_>,
    ) -> Result<FundingQuoteV1> {
        if self.manifest_content_id != manifest_content_id {
            return Err(Error::FundingBindingMismatch);
        }
        let entry = manifest.entry(self.entry_index)?;
        let quote = entry.funding_quote();
        validate_conservation(self.remaining, self.released, quote.amounts())?;
        match self.status {
            FundingStatus::Pending => {
                if self.activation_slot != 0
                    || self.released.native_lamports_total() != 0
                    || self.released.realm_collateral_total() != 0
                {
                    return Err(Error::InvalidFundingStatus);
                }
            }
            FundingStatus::Active => {
                if self.remaining.rent().amount() != 0
                    || self.remaining.creation().amount() != 0
                    || self.released.rent() != quote.amounts().rent()
                    || self.released.creation() != quote.amounts().creation()
                {
                    return Err(Error::FundingConservationMismatch);
                }
            }
        }
        Ok(quote)
    }

    /// Return the bound manifest content identity.
    pub const fn manifest_content_id(self) -> ContentId {
        self.manifest_content_id
    }
    /// Return the bound manifest entry index.
    pub const fn entry_index(self) -> u16 {
        self.entry_index
    }
    /// Return activation status.
    pub const fn status(self) -> FundingStatus {
        self.status
    }
    /// Return activation slot, or zero while pending.
    pub const fn activation_slot(self) -> u64 {
        self.activation_slot
    }
    /// Return typed presently held semantic principal.
    pub const fn remaining(self) -> FundingAmountsV1 {
        self.remaining
    }
    /// Return typed previously released semantic principal.
    pub const fn released(self) -> FundingAmountsV1 {
        self.released
    }
}

/// Canonical PDA seed projection for one program-owned funding-state account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityFundingDerivationV1 {
    market: [u8; 32],
    generation_le: [u8; 8],
    entry_index_le: [u8; 2],
    config_id: [u8; 32],
    release_id: [u8; 32],
}

impl CapabilityFundingDerivationV1 {
    /// Validate funding-to-manifest binding and construct exact ordered seeds.
    pub fn new(
        market: [u8; 32],
        generation: u64,
        manifest_content_id: ContentId,
        manifest: CapabilityManifestV1<'_>,
        funding: FundingStateV1,
    ) -> Result<Self> {
        require_nonzero_identifier(&market)?;
        if funding.manifest_content_id() != manifest_content_id {
            return Err(Error::FundingBindingMismatch);
        }
        let entry = manifest.entry(funding.entry_index())?;
        Ok(Self {
            market,
            generation_le: generation.to_le_bytes(),
            entry_index_le: funding.entry_index().to_le_bytes(),
            config_id: entry.config_id().to_bytes(),
            release_id: entry.release_id().to_bytes(),
        })
    }

    /// Return the exact ordered PDA seed components.
    pub fn seed_components(&self) -> [&[u8]; 6] {
        [
            CAPABILITY_FUNDING_PDA_DOMAIN_V1,
            self.market.as_slice(),
            self.generation_le.as_slice(),
            self.entry_index_le.as_slice(),
            self.config_id.as_slice(),
            self.release_id.as_slice(),
        ]
    }

    /// Return the authenticated Market key seed.
    pub const fn market(self) -> [u8; 32] {
        self.market
    }
    /// Return the immutable Market generation seed.
    pub const fn generation(self) -> u64 {
        u64::from_le_bytes(self.generation_le)
    }
    /// Return the selected manifest entry index seed.
    pub const fn entry_index(self) -> u16 {
        u16::from_le_bytes(self.entry_index_le)
    }
    /// Return the selected immutable capability config identity seed.
    pub const fn config_id(self) -> [u8; 32] {
        self.config_id
    }
    /// Return the selected immutable capability release identity seed.
    pub const fn release_id(self) -> [u8; 32] {
        self.release_id
    }
}

/// PDA seeds for the token-signing authority derived from a funding-state PDA.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityFundingAuthorityDerivationV1 {
    funding_state: [u8; 32],
}

impl CapabilityFundingAuthorityDerivationV1 {
    /// Construct from the adapter-derived canonical funding-state PDA key.
    pub fn new(funding_state: [u8; 32]) -> Result<Self> {
        require_nonzero_identifier(&funding_state)?;
        Ok(Self { funding_state })
    }

    /// Return exact ordered authority PDA seeds.
    pub fn seed_components(&self) -> [&[u8]; 2] {
        [
            CAPABILITY_FUNDING_AUTHORITY_PDA_DOMAIN_V1,
            self.funding_state.as_slice(),
        ]
    }
}

/// PDA seeds for the optional Realm-collateral token vault.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityFundingVaultDerivationV1 {
    funding_authority: [u8; 32],
    token_program: [u8; 32],
    mint: [u8; 32],
}

impl CapabilityFundingVaultDerivationV1 {
    /// Construct exact vault seeds from canonical authority and quote binding.
    pub fn new(funding_authority: [u8; 32], binding: RealmCollateralBindingV1) -> Result<Self> {
        require_nonzero_identifier(&funding_authority)?;
        Ok(Self {
            funding_authority,
            token_program: binding.token_program(),
            mint: binding.mint(),
        })
    }

    /// Return exact ordered vault PDA seeds.
    pub fn seed_components(&self) -> [&[u8]; 4] {
        [
            CAPABILITY_FUNDING_VAULT_PDA_DOMAIN_V1,
            self.funding_authority.as_slice(),
            self.token_program.as_slice(),
            self.mint.as_slice(),
        ]
    }
}

/// Canonical physical roles an adapter must authenticate when applicable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FundingCustodyRoleV1 {
    /// Program-owned funding-state PDA holding its Rent plus native principal.
    FundingState,
    /// Canonical PDA signing optional Realm-token movement.
    CapabilityFundingAuthority,
    /// Canonical token-account PDA holding Realm collateral.
    RealmCollateralVault,
    /// Immutable Realm-selected collateral mint.
    RealmCollateralMint,
    /// Immutable Realm-selected token program.
    RealmTokenProgram,
    /// Pre-existing immutable beneficiary RentCredit for all close lamports.
    NativeRentCredit,
    /// Immutable same-mint token account for close principal and gifts.
    RealmTokenBeneficiary,
}

fn validate_custody_binding(
    quote: FundingQuoteV1,
    custody: FundingCustodyObservationV1,
) -> Result<()> {
    match (quote.realm_collateral(), custody.realm_collateral()) {
        (None, None) => Ok(()),
        (Some(binding), Some(realm)) => {
            let observed = realm.observation();
            if realm.realm_id() != binding.realm_id()
                || realm.collateral_release_id() != binding.collateral_release_id()
                || observed.token_program() != binding.token_program()
                || observed.mint() != binding.mint()
            {
                return Err(Error::RealmCollateralBindingMismatch);
            }
            Ok(())
        }
        (Some(_), None) => Err(Error::MissingRealmCollateralVault),
        (None, Some(_)) => Err(Error::UnexpectedRealmCollateralVault),
    }
}

fn validate_conservation(
    remaining: FundingAmountsV1,
    released: FundingAmountsV1,
    quote: FundingAmountsV1,
) -> Result<()> {
    for compartment in [
        FundingCompartment::Rent,
        FundingCompartment::Creation,
        FundingCompartment::Work,
        FundingCompartment::Provider,
        FundingCompartment::Bounty,
        FundingCompartment::Liquidity,
        FundingCompartment::Service,
    ] {
        let quoted = quote.compartment(compartment);
        let left = remaining.compartment(compartment);
        let right = released.compartment(compartment);
        validate_conserved_allocation(quoted, left, right)?;
    }
    if remaining
        .native_lamports_total()
        .checked_add(released.native_lamports_total())
        .ok_or(Error::ArithmeticOverflow)?
        != quote.native_lamports_total()
        || remaining
            .realm_collateral_total()
            .checked_add(released.realm_collateral_total())
            .ok_or(Error::ArithmeticOverflow)?
            != quote.realm_collateral_total()
    {
        return Err(Error::FundingConservationMismatch);
    }
    Ok(())
}

fn validate_conserved_allocation(
    quote: CompartmentFundingV1,
    remaining: CompartmentFundingV1,
    released: CompartmentFundingV1,
) -> Result<()> {
    for value in [remaining, released] {
        if value.amount() != 0 && value.asset_class() != quote.asset_class() {
            return Err(Error::FundingConservationMismatch);
        }
    }
    if remaining
        .amount()
        .checked_add(released.amount())
        .ok_or(Error::ArithmeticOverflow)?
        != quote.amount()
    {
        return Err(Error::FundingConservationMismatch);
    }
    Ok(())
}

fn move_compartment(
    remaining: &mut FundingAmountsV1,
    released: &mut FundingAmountsV1,
    compartment: FundingCompartment,
    amount: u64,
) -> Result<()> {
    if amount == 0 {
        return Ok(());
    }
    let source = remaining.compartment(compartment);
    let destination = released.compartment(compartment);
    let next_source_amount = source
        .amount()
        .checked_sub(amount)
        .ok_or(Error::InsufficientCompartmentPrincipal)?;
    let next_destination_amount = destination
        .amount()
        .checked_add(amount)
        .ok_or(Error::ArithmeticOverflow)?;
    let next_source = allocation_or_na(source.asset_class(), next_source_amount)?;
    let destination_class = if destination.asset_class() == FundingAssetClassV1::NotApplicable {
        source.asset_class()
    } else {
        destination.asset_class()
    };
    if destination_class != source.asset_class() {
        return Err(Error::FundingConservationMismatch);
    }
    let next_destination = allocation_or_na(destination_class, next_destination_amount)?;
    *remaining = remaining.with_compartment(compartment, next_source)?;
    *released = released.with_compartment(compartment, next_destination)?;
    Ok(())
}

fn allocation_or_na(asset_class: FundingAssetClassV1, amount: u64) -> Result<CompartmentFundingV1> {
    if amount == 0 {
        Ok(CompartmentFundingV1::not_applicable())
    } else {
        CompartmentFundingV1::new(asset_class, amount)
    }
}

fn checked_asset_sum(
    values: &[CompartmentFundingV1],
    asset_class: FundingAssetClassV1,
) -> Result<u64> {
    let mut total = 0u64;
    for value in values {
        if value.asset_class() == asset_class {
            total = total
                .checked_add(value.amount())
                .ok_or(Error::ArithmeticOverflow)?;
        }
    }
    Ok(total)
}

fn decode_allocation(bytes: &[u8], offset: usize) -> Result<CompartmentFundingV1> {
    CompartmentFundingV1::decode(subslice(bytes, offset, FUNDING_ALLOCATION_BYTES)?)
}

fn encode_allocation(output: &mut [u8], offset: usize, allocation: CompartmentFundingV1) {
    copy_infallible(output, offset, &allocation.to_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, MANIFEST_HEADER_BYTES,
        MAX_DEPENDENCIES_PER_CAPABILITY,
    };

    fn id(value: u8) -> ContentId {
        ContentId::new([value; 32]).expect("nonzero fixture")
    }

    fn native(value: u64) -> CompartmentFundingV1 {
        CompartmentFundingV1::native_lamports(value).expect("positive lamports")
    }

    fn realm(value: u64) -> CompartmentFundingV1 {
        CompartmentFundingV1::realm_collateral(value).expect("positive collateral")
    }

    fn binding() -> RealmCollateralBindingV1 {
        RealmCollateralBindingV1::new(id(40), id(41), [42; 32], [43; 32], [44; 32])
            .expect("binding")
    }

    fn amounts() -> FundingAmountsV1 {
        FundingAmountsV1::new(
            native(10),
            native(20),
            native(30),
            CompartmentFundingV1::not_applicable(),
            native(40),
            realm(50),
            realm(60),
        )
        .expect("amounts")
    }

    fn quote() -> FundingQuoteV1 {
        FundingQuoteV1::new(amounts(), Some(binding())).expect("quote")
    }

    fn manifest<'a>(
        storage: &'a mut [u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES],
        funding_quote: FundingQuoteV1,
    ) -> CapabilityManifestV1<'a> {
        let entry = CapabilityEntryV1::new(
            id(1),
            id(2),
            id(3),
            id(4),
            id(5),
            id(6),
            ActivationPolicy::RequiredAtFounding,
            0,
            0,
            [0; MAX_DEPENDENCIES_PER_CAPABILITY],
            funding_quote,
        )
        .expect("entry");
        CapabilityManifestV1::encode_into(&[entry], storage).expect("manifest")
    }

    fn realm_custody(token_amount: u64, state_lamports: u64) -> FundingCustodyObservationV1 {
        let observation = RealmCollateralVaultObservationV1::new(
            [50; 32],
            [51; 32],
            [42; 32],
            [43; 32],
            token_amount,
            200,
            150,
        )
        .expect("vault observation");
        let custody =
            RealmCollateralCustodyV1::new(id(40), id(41), [51; 32], [50; 32], observation)
                .expect("realm custody");
        FundingCustodyObservationV1::with_realm_collateral(state_lamports, 100, custody)
            .expect("funding custody")
    }

    #[test]
    fn typed_amounts_never_expose_a_cross_unit_total() {
        let value = amounts();
        assert_eq!(value.native_lamports_total(), 100);
        assert_eq!(value.realm_collateral_total(), 110);
        assert_eq!(
            value.liquidity().asset_class(),
            FundingAssetClassV1::RealmCollateral
        );
        assert_eq!(FundingAmountsV1::decode(&value.to_bytes()), Ok(value));
        assert_eq!(FundingQuoteV1::decode(&quote().to_bytes()), Ok(quote()));
    }

    #[test]
    fn cross_asset_values_never_share_an_overflow_domain() {
        let independent_maxima = FundingAmountsV1::new(
            native(u64::MAX),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            realm(u64::MAX),
            CompartmentFundingV1::not_applicable(),
        )
        .expect("independent asset maxima");
        assert_eq!(independent_maxima.native_lamports_total(), u64::MAX);
        assert_eq!(independent_maxima.realm_collateral_total(), u64::MAX);

        assert_eq!(
            FundingAmountsV1::new(
                native(u64::MAX),
                native(1),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
            ),
            Err(Error::ArithmeticOverflow)
        );
        assert_eq!(
            FundingAmountsV1::new(
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
                realm(u64::MAX),
                realm(1),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
            ),
            Err(Error::ArithmeticOverflow)
        );
    }

    #[test]
    fn quote_and_state_hostile_encodings_are_refused() {
        let quote = quote();
        let quote_bytes = quote.to_bytes();
        assert_eq!(
            FundingQuoteV1::decode(&quote_bytes[..FUNDING_QUOTE_BYTES - 1]),
            Err(Error::InvalidLength)
        );

        for (offset, value, expected) in [
            (0, 0, Error::InvalidMagic),
            (QUOTE_SCHEMA_OFFSET, 2, Error::UnsupportedSchema),
            (QUOTE_RESERVED_OFFSET, 1, Error::NonCanonicalReservedBytes),
            (
                QUOTE_AMOUNTS_OFFSET + AMOUNTS_RENT_OFFSET + ALLOCATION_CLASS_OFFSET,
                9,
                Error::UnknownFundingAssetClass,
            ),
        ] {
            let mut hostile = quote_bytes;
            *hostile.get_mut(offset).expect("fixture offset") = value;
            assert_eq!(FundingQuoteV1::decode(&hostile), Err(expected));
        }

        let mut noncanonical_zero = quote_bytes;
        let amount_offset = QUOTE_AMOUNTS_OFFSET + AMOUNTS_RENT_OFFSET + ALLOCATION_AMOUNT_OFFSET;
        noncanonical_zero
            .get_mut(amount_offset..amount_offset + 8)
            .expect("rent amount")
            .fill(0);
        assert_eq!(
            FundingQuoteV1::decode(&noncanonical_zero),
            Err(Error::NonCanonicalFundingAssetClass)
        );

        let mut false_total = quote_bytes;
        let total_offset = QUOTE_AMOUNTS_OFFSET + AMOUNTS_NATIVE_TOTAL_OFFSET;
        false_total
            .get_mut(total_offset..total_offset + 8)
            .expect("native total")
            .fill(0);
        assert_eq!(
            FundingQuoteV1::decode(&false_total),
            Err(Error::FundingAssetTotalMismatch)
        );

        let mut storage = [0u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
        let manifest = manifest(&mut storage, quote);
        let state = FundingStateV1::new(id(70), manifest, 0, realm_custody(110, 200))
            .expect("pending funding");
        let state_bytes = state.to_bytes();
        assert_eq!(FundingStateV1::decode(&state_bytes), Ok(state));
        assert_eq!(
            FundingStateV1::decode(&state_bytes[..FUNDING_STATE_BYTES - 1]),
            Err(Error::InvalidLength)
        );
        for (offset, value, expected) in [
            (0, 0, Error::InvalidMagic),
            (STATE_SCHEMA_OFFSET, 2, Error::UnsupportedSchema),
            (
                STATE_HEADER_RESERVED_OFFSET,
                1,
                Error::NonCanonicalReservedBytes,
            ),
            (
                STATE_BODY_RESERVED_OFFSET,
                1,
                Error::NonCanonicalReservedBytes,
            ),
            (STATE_STATUS_OFFSET, 9, Error::UnknownFundingStatus),
            (STATE_ACTIVATION_SLOT_OFFSET, 1, Error::InvalidFundingStatus),
            (
                STATE_REMAINING_OFFSET + AMOUNTS_RENT_OFFSET + ALLOCATION_CLASS_OFFSET,
                FundingAssetClassV1::RealmCollateral.byte(),
                Error::InvalidCompartmentAssetClass,
            ),
        ] {
            let mut hostile = state_bytes;
            *hostile.get_mut(offset).expect("fixture offset") = value;
            assert_eq!(FundingStateV1::decode(&hostile), Err(expected));
        }

        let mut false_state_total = state_bytes;
        let total_offset = STATE_REMAINING_OFFSET + AMOUNTS_NATIVE_TOTAL_OFFSET;
        false_state_total
            .get_mut(total_offset..total_offset + 8)
            .expect("remaining native total")
            .fill(0);
        assert_eq!(
            FundingStateV1::decode(&false_state_total),
            Err(Error::FundingAssetTotalMismatch)
        );
    }

    #[test]
    fn zero_is_only_not_applicable_and_fixed_compartments_are_lamports() {
        assert_eq!(
            CompartmentFundingV1::native_lamports(0),
            Err(Error::NonCanonicalFundingAssetClass)
        );
        assert_eq!(
            CompartmentFundingV1::realm_collateral(0),
            Err(Error::NonCanonicalFundingAssetClass)
        );
        assert_eq!(
            FundingAmountsV1::new(
                realm(1),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
            ),
            Err(Error::InvalidCompartmentAssetClass)
        );
    }

    #[test]
    fn quote_binding_is_exactly_present_when_realm_collateral_is_nonzero() {
        assert_eq!(
            FundingQuoteV1::new(amounts(), None),
            Err(Error::MissingRealmCollateralBinding)
        );
        let native_only = FundingAmountsV1::new(
            native(1),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
        )
        .expect("native only");
        assert_eq!(
            FundingQuoteV1::new(native_only, Some(binding())),
            Err(Error::UnexpectedRealmCollateralBinding)
        );
    }

    #[test]
    fn wrong_realm_release_program_mint_authority_and_vault_refuse() {
        let observed = RealmCollateralVaultObservationV1::new(
            [50; 32], [51; 32], [42; 32], [43; 32], 110, 200, 150,
        )
        .expect("observed");
        assert_eq!(
            RealmCollateralCustodyV1::new(id(40), id(41), [52; 32], [50; 32], observed),
            Err(Error::FundingAuthorityMismatch)
        );
        assert_eq!(
            RealmCollateralCustodyV1::new(id(40), id(41), [51; 32], [52; 32], observed),
            Err(Error::FundingVaultMismatch)
        );
        for bad in [
            RealmCollateralCustodyV1::new(id(99), id(41), [51; 32], [50; 32], observed),
            RealmCollateralCustodyV1::new(id(40), id(99), [51; 32], [50; 32], observed),
        ] {
            let bad = bad.expect("structural custody");
            let custody =
                FundingCustodyObservationV1::with_realm_collateral(200, 100, bad).expect("custody");
            let mut storage = [0u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
            let manifest = manifest(&mut storage, quote());
            assert_eq!(
                FundingStateV1::new(id(70), manifest, 0, custody),
                Err(Error::RealmCollateralBindingMismatch)
            );
        }
        for (program, mint) in [([99; 32], [43; 32]), ([42; 32], [99; 32])] {
            let observed = RealmCollateralVaultObservationV1::new(
                [50; 32], [51; 32], program, mint, 110, 200, 150,
            )
            .expect("observed");
            let realm = RealmCollateralCustodyV1::new(id(40), id(41), [51; 32], [50; 32], observed)
                .expect("realm");
            let custody = FundingCustodyObservationV1::with_realm_collateral(200, 100, realm)
                .expect("custody");
            let mut storage = [0u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
            let manifest = manifest(&mut storage, quote());
            assert_eq!(
                FundingStateV1::new(id(70), manifest, 0, custody),
                Err(Error::RealmCollateralBindingMismatch)
            );
        }
    }

    #[test]
    fn missing_vault_and_donations_refuse_ordinary_validation() {
        let mut storage = [0u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
        let manifest = manifest(&mut storage, quote());
        let missing = FundingCustodyObservationV1::native_only(200, 100).expect("native custody");
        assert_eq!(
            FundingStateV1::new(id(70), manifest, 0, missing),
            Err(Error::MissingRealmCollateralVault)
        );
        assert_eq!(
            FundingStateV1::new(id(70), manifest, 0, realm_custody(111, 200)),
            Err(Error::PresentRealmCollateralMismatch)
        );
        assert_eq!(
            FundingStateV1::new(id(70), manifest, 0, realm_custody(110, 201)),
            Err(Error::PresentNativeLamportsMismatch)
        );
    }

    #[test]
    fn activation_partial_release_and_close_preserve_each_asset() {
        let mut storage = [0u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
        let manifest = manifest(&mut storage, quote());
        let mut state =
            FundingStateV1::new(id(70), manifest, 0, realm_custody(110, 200)).expect("funding");
        let activation = state
            .activate(id(70), manifest, realm_custody(110, 200), 9)
            .expect("activate");
        assert_eq!(activation.rent_lamports(), 10);
        assert_eq!(activation.creation_lamports(), 20);
        let release = state
            .release(
                id(70),
                manifest,
                realm_custody(110, 170),
                FundingCompartment::Liquidity,
                25,
            )
            .expect("partial release");
        assert_eq!(release.asset_class(), FundingAssetClassV1::RealmCollateral);
        assert_eq!(release.amount(), 25);
        assert_eq!(state.remaining().realm_collateral_total(), 85);

        let close = state
            .close(id(70), manifest, realm_custody(90, 175), [80; 32])
            .expect("close with explicit donations");
        assert_eq!(close.remaining_native_lamports(), 70);
        assert_eq!(close.state_rent_lamports(), 100);
        assert_eq!(close.state_lamport_donation(), 5);
        assert_eq!(close.remaining_realm_collateral(), 85);
        assert_eq!(close.realm_collateral_donation(), 5);
        assert_eq!(close.vault_rent_lamports(), 150);
        assert_eq!(close.vault_lamport_donation(), 50);
        assert_eq!(close.realm_token_beneficiary(), Some([44; 32]));
    }

    #[test]
    fn derivation_domains_and_exact_seed_inputs_are_distinct() {
        assert!(CAPABILITY_FUNDING_PDA_DOMAIN_V1.len() <= crate::SVM_MAX_PDA_SEED_BYTES);
        assert!(CAPABILITY_FUNDING_AUTHORITY_PDA_DOMAIN_V1.len() <= crate::SVM_MAX_PDA_SEED_BYTES);
        assert!(CAPABILITY_FUNDING_VAULT_PDA_DOMAIN_V1.len() <= crate::SVM_MAX_PDA_SEED_BYTES);
        let authority = CapabilityFundingAuthorityDerivationV1::new([7; 32]).expect("authority");
        assert_eq!(
            authority.seed_components()[0],
            CAPABILITY_FUNDING_AUTHORITY_PDA_DOMAIN_V1
        );
        let vault = CapabilityFundingVaultDerivationV1::new([8; 32], binding()).expect("vault");
        assert_eq!(
            vault.seed_components()[0],
            CAPABILITY_FUNDING_VAULT_PDA_DOMAIN_V1
        );
        assert_eq!(vault.seed_components()[2], [42; 32].as_slice());
        assert_eq!(vault.seed_components()[3], [43; 32].as_slice());
    }
}
