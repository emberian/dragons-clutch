//! Typed capability-funding quotes, custody observations, and transitions.
//!
//! Native lamports and immutable Realm collateral are distinct dimensions.
//! This module deliberately exposes no operation that sums or converts them.

use crate::capability_manifest::{
    ActivationPolicy, CapabilityManifestV1, ContentId, Error, Result, copy_content_id,
    copy_infallible, put_byte, put_u16, put_u32, put_u64, read_array, read_byte, read_content_id,
    read_u16, read_u32, read_u64, require_nonzero_identifier, require_zero, subslice,
};

use crate::capability_manifest::funding_admission_v2::{
    FUNDING_LEDGER_ACTIVE_ADMISSIBLE_STATES_V2, FUNDING_LEDGER_MARKET_OPEN_ADMISSIBLE_STATES_V2,
    FUNDING_LEDGER_PENDING_ADMISSIBLE_STATES_V2,
};
use crate::capability_manifest::generated_abi;

/// Solana's fixed per-account storage overhead, in bytes. Chain-derived; the
/// one author is `formal/dclutch-semantics/DClutchSemantics/CapabilityManifestV1Abi.lean`.
pub const ACCOUNT_STORAGE_OVERHEAD_BYTES: u64 = generated_abi::ACCOUNT_STORAGE_OVERHEAD_BYTES;

/// The rent-exempt minimum an account of `account_bytes` was funded at, given
/// the exemption-scaled rate in force when its founding created it.
///
/// `Rent::minimum_balance` is affine in the account's length, so ONE rate
/// prices every account one founding created, at every width — including a
/// lookup table whose width grows between the transaction that funded it and
/// the transaction that reads it. That is why the persisted fact is the rate
/// and not any one length's minimum, and it is why four reserved header bytes
/// were enough for a fact a `u64` could not hold.
pub fn funded_rent_minimum_v2(funded_rent_rate: u32, account_bytes: usize) -> Result<u64> {
    if funded_rent_rate == 0 {
        return Err(Error::FundedRentRateMissing);
    }
    let bytes = u64::try_from(account_bytes).map_err(|_| Error::ArithmeticOverflow)?;
    ACCOUNT_STORAGE_OVERHEAD_BYTES
        .checked_add(bytes)
        .and_then(|span| span.checked_mul(u64::from(funded_rent_rate)))
        .ok_or(Error::ArithmeticOverflow)
}

/// Derive the rate to record from two readings of the cluster's own Rent, and
/// REFUSE a cluster whose rent-exempt minimum is not affine in the length.
///
/// Two readings pin an affine function; the caller supplies the zero-length one
/// and the one for the account it is about to fund, and both must agree with the
/// derived rate exactly. A cluster whose rent this cannot reproduce is refused
/// by name rather than approximated — the alternative is a recorded number that
/// silently prices some other account wrong.
pub fn derive_funded_rent_rate_v2(
    minimum_balance_zero: u64,
    account_bytes: usize,
    minimum_balance_for_account: u64,
) -> Result<u32> {
    let rate = minimum_balance_zero
        .checked_div(ACCOUNT_STORAGE_OVERHEAD_BYTES)
        .ok_or(Error::ArithmeticOverflow)?;
    let rate = u32::try_from(rate).map_err(|_| Error::UnrepresentableRentRate)?;
    if rate == 0 {
        return Err(Error::UnrepresentableRentRate);
    }
    if funded_rent_minimum_v2(rate, 0)? != minimum_balance_zero
        || funded_rent_minimum_v2(rate, account_bytes)? != minimum_balance_for_account
    {
        return Err(Error::UnrepresentableRentRate);
    }
    Ok(rate)
}

/// The exemption-scaled rate a RECORDED rent principal was funded at.
///
/// The exact inverse of [`funded_rent_minimum_v2`] for one reading, and the
/// instrument for a check the ruling otherwise leaves without one: a PERSISTED
/// principal that must be compared to something, over an account nobody is
/// creating. Comparing it to `Rent::minimum_balance` asks the cluster what a
/// byte costs today, which is the question [`funded_rent_persists_v1`] exists
/// to stop asking -- the principal was written when the account was funded and
/// prices that moment, so a rate that moves in EITHER direction refuses a
/// record that is telling the truth.
///
/// So recover the moment instead of quoting the present. `minimum_balance(len)`
/// is `(ACCOUNT_STORAGE_OVERHEAD + len) x rate`, and the division back out must
/// be EXACT: a principal no rate reproduces is a garbled record, refused by
/// name rather than rounded to the nearest plausible cluster. One founding is
/// one rate at every width, so a caller holding two principals one record wrote
/// recovers the rate from either and requires it to price the other -- a
/// statement the sysvar comparison never made, and true under every rate the
/// cluster later adopts.
///
/// The host recovery in `dclutch-resolution-core-v3-operator`'s
/// `funded_rent_recovery_v1` reads the same inverse off an account's BALANCE
/// rather than off a recorded principal; folding it onto this author is owed
/// and belongs to whoever next holds that file.
pub fn funded_rent_rate_from_minimum_v1(minimum: u64, account_bytes: usize) -> Result<u32> {
    let bytes = u64::try_from(account_bytes).map_err(|_| Error::ArithmeticOverflow)?;
    let span = ACCOUNT_STORAGE_OVERHEAD_BYTES
        .checked_add(bytes)
        .ok_or(Error::ArithmeticOverflow)?;
    let rate = u32::try_from(minimum.checked_div(span).ok_or(Error::ArithmeticOverflow)?)
        .map_err(|_| Error::UnrepresentableRentRate)?;
    if rate == 0 || funded_rent_minimum_v2(rate, account_bytes)? != minimum {
        return Err(Error::UnrepresentableRentRate);
    }
    Ok(rate)
}

/// Whether a PRE-EXISTING account's funded rent still holds it, without asking
/// the cluster what a byte costs today.
///
/// A floor written `lamports >= Rent::minimum_balance(len)` over an account
/// some EARLIER transaction funded is not a statement about that account. It is
/// a statement about the rate of the moment, and it refuses a live account the
/// instant that rate rises. Devnet fell 6,333 -> 5,080 at the epoch-1141
/// boundary with cohort-15 live on it and the fall broke every exactness check
/// (`c0a1586b1`); a RISE breaks every floor the same way, in the other
/// direction, and there is no direction a cluster is forbidden to move.
///
/// The runtime, not the program, is the authority on what a pre-existing
/// account's rent buys, and `solana-svm 4.3.0-beta.2` `src/rent_calculator.rs`
/// states it in three parts:
///
/// - **Rent is never collected.** The module carries no collection path at all,
///   and `RENT_EXEMPT_RENT_EPOCH` exists so the field can be deleted. Nothing
///   debits an account for its own storage, so no passage of time and no change
///   of rate can move a funded account's balance.
/// - **A raised rate cannot make a funded account rent-paying.** Under
///   SIMD-0392 `get_pre_exec_account_rent_state` reads a `RentPaying` account as
///   `RentExempt`, and `get_post_exec_account_rent_state` keeps it exempt for
///   any balance that did not fall. An account funded at yesterday's cheaper
///   rate is GRANDFATHERED by the runtime, not merely tolerated by it.
/// - **No transaction can leave a live account under-rented.**
///   `transition_allowed` refuses `RentExempt -> RentPaying` outright, so
///   neither this program nor a stranger's can drain a funded account to a
///   partial balance. The only exit the runtime permits is to zero.
///
/// So over a pre-existing account the rate-scaled floor decides exactly one
/// case the runtime has not already decided, and this predicate is that case:
/// `lamports == 0`, an account an earlier instruction of THIS transaction has
/// already drained, whose data is residue the runtime reaps at the
/// transaction's end. Rate-free, which is the whole point.
///
/// This is NOT the check for an account being created NOW. A creation must be
/// exempt at today's rate or the runtime refuses the transaction outright, so a
/// creating site reads the sysvar, funds against it, and records what it paid --
/// see [`derive_funded_rent_rate_v2`].
#[must_use]
pub const fn funded_rent_persists_v1(account_lamports: u64) -> bool {
    account_lamports != 0
}

/// Exact width of one typed compartment allocation.
pub const FUNDING_ALLOCATION_BYTES: usize = generated_abi::CAPABILITY_FUNDING_ALLOCATION_BYTES_V1;
/// Exact width of seven typed compartments plus two independent totals.
pub const FUNDING_AMOUNTS_BYTES: usize = generated_abi::CAPABILITY_FUNDING_AMOUNTS_BYTES_V1;
/// Exact width of an optional Realm-collateral binding.
pub const REALM_COLLATERAL_BINDING_BYTES: usize =
    generated_abi::CAPABILITY_FUNDING_BINDING_BYTES_V1;
/// Exact immutable funding-quote width.
pub const FUNDING_QUOTE_BYTES: usize = generated_abi::CAPABILITY_FUNDING_QUOTE_BYTES_V1;
/// Exact mutable funding-state width.
pub const FUNDING_STATE_BYTES: usize = generated_abi::CAPABILITY_FUNDING_STATE_BYTES_V1;
/// Exact FundingLedgerV2 header width before its ordered manifest slots.
pub const FUNDING_LEDGER_HEADER_BYTES_V2: usize =
    generated_abi::CAPABILITY_FUNDING_LEDGER_HEADER_BYTES_V2;
/// Exact width of one manifest-indexed FundingLedgerV2 slot.
pub const FUNDING_LEDGER_SLOT_BYTES_V2: usize =
    generated_abi::CAPABILITY_FUNDING_LEDGER_SLOT_BYTES_V2;
/// Canonical funding-quote magic.
pub const FUNDING_QUOTE_MAGIC: [u8; 8] = generated_abi::CAPABILITY_FUNDING_QUOTE_MAGIC_V1;
/// Implemented typed funding-quote schema.
pub const FUNDING_QUOTE_SCHEMA_VERSION: u16 =
    generated_abi::CAPABILITY_FUNDING_QUOTE_SCHEMA_VERSION_V1;
/// Byte offset of the remaining Rent-compartment lamport amount.
///
/// Published for one caller shape: a data-defined capability activation, whose
/// `AccountProfileV1` must project this exact scalar out of the live
/// `FundingStateV1` account with a `ProjectDataU64` operation so its
/// EffectProgram can move that many lamports into the root it is creating. An
/// interpreted artifact carries no decoder, so without this it would restate
/// the layout and become a second authority for it.
/// `the_published_rent_amount_offset_reads_the_rent_quote` requires it to read
/// back exactly what `remaining().rent().amount()` returns.
pub const FUNDING_STATE_REMAINING_RENT_AMOUNT_OFFSET_V1: usize =
    generated_abi::CAPABILITY_FUNDING_STATE_REMAINING_RENT_AMOUNT_OFFSET_V1;

/// Canonical funding-state magic.
pub const FUNDING_STATE_MAGIC: [u8; 8] = generated_abi::CAPABILITY_FUNDING_STATE_MAGIC_V1;
/// Implemented typed funding-state schema.
pub const FUNDING_STATE_SCHEMA_VERSION: u16 =
    generated_abi::CAPABILITY_FUNDING_STATE_SCHEMA_VERSION_V1;

/// Canonical FundingLedgerV2 magic.
pub const FUNDING_LEDGER_MAGIC_V2: [u8; 8] = generated_abi::CAPABILITY_FUNDING_LEDGER_MAGIC_V2;
/// Implemented FundingLedgerV2 schema.
pub const FUNDING_LEDGER_SCHEMA_VERSION_V2: u16 =
    generated_abi::CAPABILITY_FUNDING_LEDGER_SCHEMA_VERSION_V2;

/// Adapter PDA seed domain for a manifest-selected funding-state account.
pub const CAPABILITY_FUNDING_PDA_DOMAIN_V1: &[u8] = generated_abi::CAPABILITY_FUNDING_PDA_DOMAIN_V1;
/// Adapter PDA seed domain for its token-signing funding authority.
pub const CAPABILITY_FUNDING_AUTHORITY_PDA_DOMAIN_V1: &[u8] =
    generated_abi::CAPABILITY_FUNDING_AUTHORITY_PDA_DOMAIN_V1;
/// Adapter PDA seed domain for its optional Realm-collateral vault.
pub const CAPABILITY_FUNDING_VAULT_PDA_DOMAIN_V1: &[u8] =
    generated_abi::CAPABILITY_FUNDING_VAULT_PDA_DOMAIN_V1;
/// Adapter PDA seed domain for one controller-homogeneous subset ledger.
pub const CAPABILITY_FUNDING_LEDGER_PDA_DOMAIN_V2: &[u8] =
    generated_abi::CAPABILITY_FUNDING_LEDGER_PDA_DOMAIN_V2;
/// Per-entry token-signing authority domain below one FundingLedgerV2.
pub const CAPABILITY_FUNDING_LEDGER_AUTHORITY_PDA_DOMAIN_V2: &[u8] =
    generated_abi::CAPABILITY_FUNDING_LEDGER_AUTHORITY_PDA_DOMAIN_V2;
/// Optional per-entry Realm-vault domain below one FundingLedgerV2.
pub const CAPABILITY_FUNDING_LEDGER_VAULT_PDA_DOMAIN_V2: &[u8] =
    generated_abi::CAPABILITY_FUNDING_LEDGER_VAULT_PDA_DOMAIN_V2;

const QUOTE_SCHEMA_OFFSET: usize = generated_abi::CAPABILITY_FUNDING_QUOTE_SCHEMA_OFFSET_V1;
const QUOTE_COLLATERAL_KIND_OFFSET: usize =
    generated_abi::CAPABILITY_FUNDING_QUOTE_COLLATERAL_KIND_OFFSET_V1;
const QUOTE_RESERVED_OFFSET: usize = generated_abi::CAPABILITY_FUNDING_QUOTE_RESERVED_OFFSET_V1;
const QUOTE_RESERVED_BYTES: usize = generated_abi::CAPABILITY_FUNDING_QUOTE_RESERVED_BYTES_V1;
const QUOTE_COLLATERAL_BINDING_OFFSET: usize =
    generated_abi::CAPABILITY_FUNDING_QUOTE_BINDING_OFFSET_V1;
const QUOTE_AMOUNTS_OFFSET: usize = generated_abi::CAPABILITY_FUNDING_QUOTE_AMOUNTS_OFFSET_V1;

const ALLOCATION_CLASS_OFFSET: usize = generated_abi::CAPABILITY_FUNDING_ALLOCATION_CLASS_OFFSET_V1;
const ALLOCATION_RESERVED_OFFSET: usize =
    generated_abi::CAPABILITY_FUNDING_ALLOCATION_RESERVED_OFFSET_V1;
const ALLOCATION_RESERVED_BYTES: usize =
    generated_abi::CAPABILITY_FUNDING_ALLOCATION_RESERVED_BYTES_V1;
const ALLOCATION_AMOUNT_OFFSET: usize =
    generated_abi::CAPABILITY_FUNDING_ALLOCATION_AMOUNT_OFFSET_V1;

const AMOUNTS_RENT_OFFSET: usize = generated_abi::CAPABILITY_FUNDING_AMOUNTS_RENT_OFFSET_V1;
const AMOUNTS_CREATION_OFFSET: usize = generated_abi::CAPABILITY_FUNDING_AMOUNTS_CREATION_OFFSET_V1;
const AMOUNTS_WORK_OFFSET: usize = generated_abi::CAPABILITY_FUNDING_AMOUNTS_WORK_OFFSET_V1;
const AMOUNTS_PROVIDER_OFFSET: usize = generated_abi::CAPABILITY_FUNDING_AMOUNTS_PROVIDER_OFFSET_V1;
const AMOUNTS_BOUNTY_OFFSET: usize = generated_abi::CAPABILITY_FUNDING_AMOUNTS_BOUNTY_OFFSET_V1;
const AMOUNTS_LIQUIDITY_OFFSET: usize =
    generated_abi::CAPABILITY_FUNDING_AMOUNTS_LIQUIDITY_OFFSET_V1;
const AMOUNTS_SERVICE_OFFSET: usize = generated_abi::CAPABILITY_FUNDING_AMOUNTS_SERVICE_OFFSET_V1;
const AMOUNTS_NATIVE_TOTAL_OFFSET: usize =
    generated_abi::CAPABILITY_FUNDING_AMOUNTS_NATIVE_TOTAL_OFFSET_V1;
const AMOUNTS_REALM_TOTAL_OFFSET: usize =
    generated_abi::CAPABILITY_FUNDING_AMOUNTS_REALM_TOTAL_OFFSET_V1;

const BINDING_REALM_ID_OFFSET: usize = generated_abi::CAPABILITY_FUNDING_BINDING_REALM_ID_OFFSET_V1;
const BINDING_RELEASE_ID_OFFSET: usize =
    generated_abi::CAPABILITY_FUNDING_BINDING_RELEASE_ID_OFFSET_V1;
const BINDING_TOKEN_PROGRAM_OFFSET: usize =
    generated_abi::CAPABILITY_FUNDING_BINDING_TOKEN_PROGRAM_OFFSET_V1;
const BINDING_MINT_OFFSET: usize = generated_abi::CAPABILITY_FUNDING_BINDING_MINT_OFFSET_V1;
const BINDING_BENEFICIARY_OFFSET: usize =
    generated_abi::CAPABILITY_FUNDING_BINDING_BENEFICIARY_OFFSET_V1;

const STATE_SCHEMA_OFFSET: usize = generated_abi::CAPABILITY_FUNDING_STATE_SCHEMA_OFFSET_V1;
const STATE_STATUS_OFFSET: usize = generated_abi::CAPABILITY_FUNDING_STATE_STATUS_OFFSET_V1;
const STATE_HEADER_RESERVED_OFFSET: usize =
    generated_abi::CAPABILITY_FUNDING_STATE_HEADER_RESERVED_OFFSET_V1;
const STATE_HEADER_RESERVED_BYTES: usize =
    generated_abi::CAPABILITY_FUNDING_STATE_HEADER_RESERVED_BYTES_V1;
const STATE_MANIFEST_ID_OFFSET: usize =
    generated_abi::CAPABILITY_FUNDING_STATE_MANIFEST_ID_OFFSET_V1;
const STATE_ENTRY_INDEX_OFFSET: usize =
    generated_abi::CAPABILITY_FUNDING_STATE_ENTRY_INDEX_OFFSET_V1;
const STATE_BODY_RESERVED_OFFSET: usize =
    generated_abi::CAPABILITY_FUNDING_STATE_BODY_RESERVED_OFFSET_V1;
const STATE_BODY_RESERVED_BYTES: usize =
    generated_abi::CAPABILITY_FUNDING_STATE_BODY_RESERVED_BYTES_V1;
const STATE_ACTIVATION_SLOT_OFFSET: usize =
    generated_abi::CAPABILITY_FUNDING_STATE_ACTIVATION_SLOT_OFFSET_V1;
const STATE_REMAINING_OFFSET: usize = generated_abi::CAPABILITY_FUNDING_STATE_REMAINING_OFFSET_V1;
const STATE_RELEASED_OFFSET: usize = generated_abi::CAPABILITY_FUNDING_STATE_RELEASED_OFFSET_V1;

const LEDGER_SCHEMA_OFFSET_V2: usize = generated_abi::CAPABILITY_FUNDING_LEDGER_SCHEMA_OFFSET_V2;
const LEDGER_SELECTED_MASK_OFFSET_V2: usize =
    generated_abi::CAPABILITY_FUNDING_LEDGER_SELECTED_MASK_OFFSET_V2;
const LEDGER_FUNDED_RENT_RATE_OFFSET_V2: usize =
    generated_abi::CAPABILITY_FUNDING_LEDGER_FUNDED_RENT_RATE_OFFSET_V2;
const LEDGER_MANIFEST_ID_OFFSET_V2: usize =
    generated_abi::CAPABILITY_FUNDING_LEDGER_MANIFEST_ID_OFFSET_V2;
const LEDGER_SLOT_STATUS_OFFSET_V2: usize =
    generated_abi::CAPABILITY_FUNDING_LEDGER_SLOT_STATUS_OFFSET_V2;
const LEDGER_SLOT_RESERVED_OFFSET_V2: usize =
    generated_abi::CAPABILITY_FUNDING_LEDGER_SLOT_RESERVED_OFFSET_V2;
const LEDGER_SLOT_RESERVED_BYTES_V2: usize =
    generated_abi::CAPABILITY_FUNDING_LEDGER_SLOT_RESERVED_BYTES_V2;
const LEDGER_SLOT_ACTIVATION_SLOT_OFFSET_V2: usize =
    generated_abi::CAPABILITY_FUNDING_LEDGER_SLOT_ACTIVATION_SLOT_OFFSET_V2;
const LEDGER_SLOT_REMAINING_OFFSETS_V2: [usize; 7] = [
    generated_abi::CAPABILITY_FUNDING_LEDGER_SLOT_REMAINING_RENT_OFFSET_V2,
    generated_abi::CAPABILITY_FUNDING_LEDGER_SLOT_REMAINING_CREATION_OFFSET_V2,
    generated_abi::CAPABILITY_FUNDING_LEDGER_SLOT_REMAINING_WORK_OFFSET_V2,
    generated_abi::CAPABILITY_FUNDING_LEDGER_SLOT_REMAINING_PROVIDER_OFFSET_V2,
    generated_abi::CAPABILITY_FUNDING_LEDGER_SLOT_REMAINING_BOUNTY_OFFSET_V2,
    generated_abi::CAPABILITY_FUNDING_LEDGER_SLOT_REMAINING_LIQUIDITY_OFFSET_V2,
    generated_abi::CAPABILITY_FUNDING_LEDGER_SLOT_REMAINING_SERVICE_OFFSET_V2,
];

const FUNDING_COMPARTMENTS: [FundingCompartment; 7] = [
    FundingCompartment::Rent,
    FundingCompartment::Creation,
    FundingCompartment::Work,
    FundingCompartment::Provider,
    FundingCompartment::Bounty,
    FundingCompartment::Liquidity,
    FundingCompartment::Service,
];
const FUNDING_LEDGER_MASK_BITS_V2: u16 = 16;

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

    /// Validate terminal physical custody, admitting only explicit donations.
    ///
    /// Unlike [`Self::validate_against`], this accepts native lamports and
    /// Realm tokens above semantic remaining principal.  Callers must consume
    /// the resulting surplus only through [`Self::close`], which classifies
    /// every such excess for the immutable refund beneficiary.
    pub fn validate_close_custody(
        self,
        manifest_content_id: ContentId,
        manifest: CapabilityManifestV1<'_>,
        custody: FundingCustodyObservationV1,
    ) -> Result<()> {
        let quote = self.validate_semantics(manifest_content_id, manifest)?;
        validate_custody_binding(quote, custody)?;
        if custody.present_native_lamports()? < self.remaining.native_lamports_total() {
            return Err(Error::UnderfundedPhysicalCustody);
        }
        match (quote.realm_collateral(), custody.realm_collateral()) {
            (None, None) => Ok(()),
            (Some(_), Some(observed))
                if observed.observation().token_amount()
                    >= self.remaining.realm_collateral_total() =>
            {
                Ok(())
            }
            (Some(_), Some(_)) => Err(Error::UnderfundedPhysicalCustody),
            (Some(_), None) => Err(Error::MissingRealmCollateralVault),
            (None, Some(_)) => Err(Error::UnexpectedRealmCollateralVault),
        }
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
        self.validate_close_custody(manifest_content_id, manifest, custody)?;
        let quote = self.validate_semantics(manifest_content_id, manifest)?;
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

/// Return one exact FundingLedgerV2 account width for a selected-row count.
///
/// The bound is provisional profile-1 policy. The checked multiplication and
/// addition are part of hostile decoding; callers never infer a partial final
/// slot from a trailing byte span.
pub fn funding_ledger_bytes_v2(slot_count: u16) -> Result<usize> {
    if slot_count == 0 || usize::from(slot_count) > crate::capability_manifest::MAX_CAPABILITIES {
        return Err(Error::TooManyCapabilities);
    }
    FUNDING_LEDGER_HEADER_BYTES_V2
        .checked_add(
            usize::from(slot_count)
                .checked_mul(FUNDING_LEDGER_SLOT_BYTES_V2)
                .ok_or(Error::ArithmeticOverflow)?,
        )
        .ok_or(Error::ArithmeticOverflow)
}

/// Return the exact number of physical rows selected by a nonzero mask.
pub fn funding_ledger_slot_count_v2(selected_mask: u16) -> Result<u16> {
    if selected_mask == 0 {
        return Err(Error::InvalidDependency);
    }
    u16::try_from(selected_mask.count_ones()).map_err(|_| Error::ArithmeticOverflow)
}

/// Validate a required union and a canonical disjoint partition of it.
///
/// Each mask selects manifest indices, each selected index occurs in exactly
/// one ledger, masks are ordered by their lowest selected index, and their union
/// equals `required_union` exactly. Controller homogeneity is an adapter check
/// because controller programs are owned by authenticated release/program-set
/// artifacts rather than the manifest ABI.
pub fn validate_funding_ledger_masks_v2(
    manifest_entry_count: u16,
    required_union: u16,
    ledger_masks: &[u16],
) -> Result<()> {
    let valid_mask = manifest_valid_mask_v2(manifest_entry_count)?;
    if required_union == 0 || required_union & !valid_mask != 0 || ledger_masks.is_empty() {
        return Err(Error::InvalidDependency);
    }
    let mut observed_union = 0_u16;
    let mut previous_lowest = None;
    for &mask in ledger_masks {
        if mask == 0 || mask & !valid_mask != 0 || mask & observed_union != 0 {
            return Err(Error::InvalidDependency);
        }
        let lowest = mask.trailing_zeros();
        if previous_lowest.is_some_and(|previous| previous >= lowest) {
            return Err(Error::InvalidDependency);
        }
        previous_lowest = Some(lowest);
        observed_union |= mask;
    }
    if observed_union != required_union {
        return Err(Error::InvalidDependency);
    }
    Ok(())
}

/// Lifecycle state of one manifest-indexed FundingLedgerV2 slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FundingLedgerStatusV2 {
    /// The exact quote remains prepaid and activation has not run.
    Pending = generated_abi::CAPABILITY_FUNDING_LEDGER_STATUS_PENDING_V2,
    /// Activation ran once; Rent and Creation have been released.
    Active = generated_abi::CAPABILITY_FUNDING_LEDGER_STATUS_ACTIVE_V2,
    /// This logical entry has closed and retains no principal.
    ///
    /// The shared account remains until every slot is Closed, so closing one
    /// entry cannot refund or destroy another entry's ledger rent.
    Closed = generated_abi::CAPABILITY_FUNDING_LEDGER_STATUS_CLOSED_V2,
}

impl FundingLedgerStatusV2 {
    /// Hostile-decode one persisted status byte.
    ///
    /// `pub(crate)` for `funding_admission_v2`'s
    /// `the_bit_index_is_the_wire_encoding`, which pins the admission bit
    /// index against this pair rather than against a second numbering.
    pub(crate) fn decode(value: u8) -> Result<Self> {
        match value {
            generated_abi::CAPABILITY_FUNDING_LEDGER_STATUS_PENDING_V2 => Ok(Self::Pending),
            generated_abi::CAPABILITY_FUNDING_LEDGER_STATUS_ACTIVE_V2 => Ok(Self::Active),
            generated_abi::CAPABILITY_FUNDING_LEDGER_STATUS_CLOSED_V2 => Ok(Self::Closed),
            _ => Err(Error::UnknownFundingLedgerStatus),
        }
    }

    /// The byte this status is persisted as.
    pub(crate) const fn byte(self) -> u8 {
        self as u8
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FundingLedgerRawSlotV2 {
    status: FundingLedgerStatusV2,
    activation_slot: u64,
    remaining: [u64; 7],
}

impl FundingLedgerRawSlotV2 {
    fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != FUNDING_LEDGER_SLOT_BYTES_V2 {
            return Err(Error::InvalidLength);
        }
        require_zero(
            bytes,
            LEDGER_SLOT_RESERVED_OFFSET_V2,
            LEDGER_SLOT_RESERVED_BYTES_V2,
        )?;
        let status =
            FundingLedgerStatusV2::decode(read_byte(bytes, LEDGER_SLOT_STATUS_OFFSET_V2)?)?;
        let activation_slot = read_u64(bytes, LEDGER_SLOT_ACTIVATION_SLOT_OFFSET_V2)?;
        if status == FundingLedgerStatusV2::Pending && activation_slot != 0 {
            return Err(Error::InvalidFundingStatus);
        }
        let mut remaining = [0_u64; 7];
        for (value, offset) in remaining.iter_mut().zip(LEDGER_SLOT_REMAINING_OFFSETS_V2) {
            *value = read_u64(bytes, offset)?;
        }
        Ok(Self {
            status,
            activation_slot,
            remaining,
        })
    }

    fn to_bytes(self) -> [u8; FUNDING_LEDGER_SLOT_BYTES_V2] {
        let mut output = [0_u8; FUNDING_LEDGER_SLOT_BYTES_V2];
        put_byte(
            &mut output,
            LEDGER_SLOT_STATUS_OFFSET_V2,
            self.status.byte(),
        );
        put_u64(
            &mut output,
            LEDGER_SLOT_ACTIVATION_SLOT_OFFSET_V2,
            self.activation_slot,
        );
        for (value, offset) in self
            .remaining
            .into_iter()
            .zip(LEDGER_SLOT_REMAINING_OFFSETS_V2)
        {
            put_u64(&mut output, offset, value);
        }
        output
    }
}

/// One fully authenticated logical funding slot.
///
/// `remaining` and `released` are typed values derived only after the exact
/// ledger/manifest identity and sparse ascending-index mapping authenticate. Neither
/// asset classes nor released totals are duplicated in mutable account data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedFundingSlotV2 {
    entry_index: u16,
    status: FundingLedgerStatusV2,
    activation_slot: u64,
    remaining: FundingAmountsV1,
    released: FundingAmountsV1,
}

impl AuthenticatedFundingSlotV2 {
    /// Return the dense manifest entry index that owns this slot.
    pub const fn entry_index(self) -> u16 {
        self.entry_index
    }

    /// Return this slot's lifecycle status.
    pub const fn status(self) -> FundingLedgerStatusV2 {
        self.status
    }

    /// Return the accepted activation slot, or zero while Pending.
    pub const fn activation_slot(self) -> u64 {
        self.activation_slot
    }

    /// Return presently held typed semantic principal.
    pub const fn remaining(self) -> FundingAmountsV1 {
        self.remaining
    }

    /// Return typed released principal derived as immutable quote minus remaining.
    pub const fn released(self) -> FundingAmountsV1 {
        self.released
    }
}

/// Hostile-decoded manifest-keyed ordered funding ledger.
///
/// This view deliberately exposes no slot values. Call [`Self::authenticate`]
/// with the adapter-authenticated immutable manifest first; only the returned
/// [`AuthenticatedFundingLedgerV2`] can derive asset classes or released
/// principal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FundingLedgerV2<'ledger> {
    bytes: &'ledger [u8],
    manifest_content_id: ContentId,
    selected_mask: u16,
    slot_count: u16,
    funded_rent_rate: u32,
}

impl<'ledger> FundingLedgerV2<'ledger> {
    /// Decode exact count, width, reserved bytes, statuses, and slot boundaries.
    pub fn decode(bytes: &'ledger [u8]) -> Result<Self> {
        if bytes.len() < FUNDING_LEDGER_HEADER_BYTES_V2 {
            return Err(Error::InvalidLength);
        }
        if read_array::<8>(bytes, 0)? != FUNDING_LEDGER_MAGIC_V2 {
            return Err(Error::InvalidMagic);
        }
        if read_u16(bytes, LEDGER_SCHEMA_OFFSET_V2)? != FUNDING_LEDGER_SCHEMA_VERSION_V2 {
            return Err(Error::UnsupportedSchema);
        }
        let funded_rent_rate = read_u32(bytes, LEDGER_FUNDED_RENT_RATE_OFFSET_V2)?;
        if funded_rent_rate == 0 {
            return Err(Error::FundedRentRateMissing);
        }
        let selected_mask = read_u16(bytes, LEDGER_SELECTED_MASK_OFFSET_V2)?;
        let slot_count = funding_ledger_slot_count_v2(selected_mask)?;
        if bytes.len() != funding_ledger_bytes_v2(slot_count)? {
            return Err(Error::InvalidLength);
        }
        let result = Self {
            bytes,
            manifest_content_id: read_content_id(bytes, LEDGER_MANIFEST_ID_OFFSET_V2)?,
            selected_mask,
            slot_count,
            funded_rent_rate,
        };
        let mut row_index = 0;
        while row_index < slot_count {
            result.raw_row(row_index)?;
            row_index = row_index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        Ok(result)
    }

    /// Initialize exact Pending rows in selected manifest-index order.
    ///
    /// The composing adapter must already have authenticated
    /// `manifest_content_id` as the content identity of `manifest.as_bytes()`.
    /// The method refuses a zero or out-of-range mask and a buffer not exactly
    /// `48 + 72 * popcount(selected_mask)` bytes.
    pub fn initialize(
        output: &mut [u8],
        manifest_content_id: ContentId,
        manifest: CapabilityManifestV1<'_>,
        selected_mask: u16,
        funded_rent_rate: u32,
    ) -> Result<()> {
        if funded_rent_rate == 0 {
            return Err(Error::FundedRentRateMissing);
        }
        validate_selected_mask_v2(manifest.entry_count(), selected_mask)?;
        let slot_count = funding_ledger_slot_count_v2(selected_mask)?;
        if output.len() != funding_ledger_bytes_v2(slot_count)? {
            return Err(Error::InvalidLength);
        }
        output.fill(0);
        copy_infallible(output, 0, &FUNDING_LEDGER_MAGIC_V2);
        put_u16(
            output,
            LEDGER_SCHEMA_OFFSET_V2,
            FUNDING_LEDGER_SCHEMA_VERSION_V2,
        );
        put_u16(output, LEDGER_SELECTED_MASK_OFFSET_V2, selected_mask);
        put_u32(output, LEDGER_FUNDED_RENT_RATE_OFFSET_V2, funded_rent_rate);
        copy_content_id(output, LEDGER_MANIFEST_ID_OFFSET_V2, manifest_content_id);
        let mut row_index = 0;
        while row_index < slot_count {
            let entry_index = manifest_entry_for_ledger_row_v2(selected_mask, row_index)?;
            let entry = manifest.entry(entry_index)?;
            let quote = entry.funding_quote().amounts();
            let mut remaining = [0_u64; 7];
            for (value, compartment) in remaining.iter_mut().zip(FUNDING_COMPARTMENTS) {
                *value = quote.compartment(compartment).amount();
            }
            write_ledger_row(
                output,
                row_index,
                FundingLedgerRawSlotV2 {
                    status: FundingLedgerStatusV2::Pending,
                    activation_slot: 0,
                    remaining,
                },
            )?;
            row_index = row_index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        decode_funding_ledger_v2(output)?.authenticate(manifest_content_id, manifest)?;
        Ok(())
    }

    /// Bind the decoded ledger to the exact immutable manifest and validate
    /// every selected slot pointwise.
    pub fn authenticate<'manifest>(
        self,
        manifest_content_id: ContentId,
        manifest: CapabilityManifestV1<'manifest>,
    ) -> Result<AuthenticatedFundingLedgerV2<'ledger, 'manifest>> {
        if self.manifest_content_id != manifest_content_id {
            return Err(Error::FundingBindingMismatch);
        }
        validate_selected_mask_v2(manifest.entry_count(), self.selected_mask)?;
        let authenticated = AuthenticatedFundingLedgerV2 {
            ledger: self,
            manifest_content_id,
            manifest,
        };
        let mut row_index = 0;
        while row_index < self.slot_count {
            authenticated.slot_by_row(row_index)?;
            row_index = row_index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        Ok(authenticated)
    }

    /// Return the manifest content identity persisted in the header.
    pub const fn manifest_content_id(self) -> ContentId {
        self.manifest_content_id
    }

    /// Return the exemption-scaled rent rate this ledger was funded at.
    ///
    /// Nonzero by decode. This is the CLUSTER parameter in force when the
    /// founding created and funded the account, not a reading of today's.
    pub const fn funded_rent_rate(self) -> u32 {
        self.funded_rent_rate
    }

    /// Rederive the rent-exempt minimum this ledger was funded at, for a width.
    ///
    /// Every exactness check over an account this founding created asks THIS,
    /// never `Rent::minimum_balance`: the sysvar answers what an account created
    /// now would cost, and the account was not created now.
    pub fn funded_rent_minimum(self, account_bytes: usize) -> Result<u64> {
        funded_rent_minimum_v2(self.funded_rent_rate, account_bytes)
    }

    /// Return the exact nonzero manifest-index selection persisted in the header.
    pub const fn selected_mask(self) -> u16 {
        self.selected_mask
    }

    /// Return the exact physical row count derived from the selected mask.
    pub const fn slot_count(self) -> u16 {
        self.slot_count
    }

    /// Borrow the exact canonical account bytes.
    pub const fn as_bytes(self) -> &'ledger [u8] {
        self.bytes
    }

    fn raw_slot(self, entry_index: u16) -> Result<FundingLedgerRawSlotV2> {
        let row_index = funding_ledger_row_for_manifest_entry_v2(self.selected_mask, entry_index)?;
        self.raw_row(row_index)
    }

    fn raw_row(self, row_index: u16) -> Result<FundingLedgerRawSlotV2> {
        FundingLedgerRawSlotV2::decode(self.row_bytes(row_index)?)
    }

    fn row_bytes(self, row_index: u16) -> Result<&'ledger [u8]> {
        if row_index >= self.slot_count {
            return Err(Error::InvalidDependency);
        }
        let start = funding_ledger_slot_offset_v2(row_index)?;
        subslice(self.bytes, start, FUNDING_LEDGER_SLOT_BYTES_V2)
    }

    fn slot_bytes(self, entry_index: u16) -> Result<&'ledger [u8]> {
        let row_index = funding_ledger_row_for_manifest_entry_v2(self.selected_mask, entry_index)?;
        self.row_bytes(row_index)
    }

    /// Activate exactly one Pending slot without touching any other slot.
    pub fn activate_in_place(
        bytes: &mut [u8],
        manifest_content_id: ContentId,
        manifest: CapabilityManifestV1<'_>,
        entry_index: u16,
        current_slot: u64,
    ) -> Result<ActivationDebitV1> {
        let (mut raw, debit) = {
            let authenticated =
                decode_funding_ledger_v2(bytes)?.authenticate(manifest_content_id, manifest)?;
            let slot = authenticated.slot(entry_index)?;
            if !FUNDING_LEDGER_PENDING_ADMISSIBLE_STATES_V2.admits(slot.status) {
                return Err(Error::InvalidFundingStatus);
            }
            let entry = manifest.entry(entry_index)?;
            if entry.activation_policy() == ActivationPolicy::PrepaidLazy
                && current_slot > entry.activation_deadline_slot()
            {
                return Err(Error::ActivationDeadlineElapsed);
            }
            let quote = entry.funding_quote().amounts();
            (
                authenticated.ledger.raw_slot(entry_index)?,
                ActivationDebitV1 {
                    rent_lamports: quote.rent().amount(),
                    creation_lamports: quote.creation().amount(),
                },
            )
        };
        raw.status = FundingLedgerStatusV2::Active;
        raw.activation_slot = current_slot;
        raw.remaining[0] = 0;
        raw.remaining[1] = 0;
        write_ledger_slot(bytes, entry_index, raw)?;
        let post = decode_funding_ledger_v2(bytes)?.authenticate(manifest_content_id, manifest)?;
        if post.slot(entry_index)?.status != FundingLedgerStatusV2::Active {
            return Err(Error::InvalidFundingStatus);
        }
        Ok(debit)
    }

    /// Release one exact non-activation compartment from one Active slot.
    pub fn release_in_place(
        bytes: &mut [u8],
        manifest_content_id: ContentId,
        manifest: CapabilityManifestV1<'_>,
        entry_index: u16,
        compartment: FundingCompartment,
        amount: u64,
    ) -> Result<FundingReleasePlanV1> {
        if amount == 0 {
            return Err(Error::ZeroPrincipalRelease);
        }
        if matches!(
            compartment,
            FundingCompartment::Rent | FundingCompartment::Creation
        ) {
            return Err(Error::ActivationCompartmentRequired);
        }
        let (mut raw, asset_class, compartment_index) = {
            let authenticated =
                decode_funding_ledger_v2(bytes)?.authenticate(manifest_content_id, manifest)?;
            let slot = authenticated.slot(entry_index)?;
            if !FUNDING_LEDGER_ACTIVE_ADMISSIBLE_STATES_V2.admits(slot.status) {
                return Err(Error::InvalidFundingStatus);
            }
            let compartment_index = funding_compartment_index(compartment);
            (
                authenticated.ledger.raw_slot(entry_index)?,
                slot.remaining().compartment(compartment).asset_class(),
                compartment_index,
            )
        };
        let remaining = raw
            .remaining
            .get_mut(compartment_index)
            .ok_or(Error::InvalidDependency)?;
        *remaining = remaining
            .checked_sub(amount)
            .ok_or(Error::InsufficientCompartmentPrincipal)?;
        write_ledger_slot(bytes, entry_index, raw)?;
        decode_funding_ledger_v2(bytes)?.authenticate(manifest_content_id, manifest)?;
        Ok(FundingReleasePlanV1 {
            compartment,
            asset_class,
            amount,
        })
    }

    /// Close one Active logical slot with exact physical refund classification.
    ///
    /// The caller authenticates the immutable Market RentCredit and, when the
    /// selected quote uses Realm collateral, the row's independently derived
    /// authority and vault before constructing `custody`. One slot can never
    /// authorize or discharge another row's token vault. Shared ledger Rent and
    /// unsolicited lamports remain until the final selected slot closes.
    pub fn close_slot_in_place(
        bytes: &mut [u8],
        manifest_content_id: ContentId,
        manifest: CapabilityManifestV1<'_>,
        entry_index: u16,
        custody: FundingLedgerCloseCustodyV2,
    ) -> Result<FundingLedgerEntryClosePlanV2> {
        require_nonzero_identifier(&custody.native_rent_credit)?;
        let (mut raw, slot, quote, aggregate_native_before) = {
            let authenticated =
                decode_funding_ledger_v2(bytes)?.authenticate(manifest_content_id, manifest)?;
            let slot = authenticated.slot(entry_index)?;
            if !FUNDING_LEDGER_ACTIVE_ADMISSIBLE_STATES_V2.admits(slot.status) {
                return Err(Error::InvalidFundingStatus);
            }
            (
                authenticated.ledger.raw_slot(entry_index)?,
                slot,
                manifest.entry(entry_index)?.funding_quote(),
                authenticated.remaining_native_lamports_total()?,
            )
        };
        let required_ledger_lamports = custody
            .exact_ledger_rent_lamports
            .checked_add(aggregate_native_before)
            .ok_or(Error::ArithmeticOverflow)?;
        let ledger_lamport_surplus = custody
            .ledger_account_lamports
            .checked_sub(required_ledger_lamports)
            .ok_or(Error::UnderfundedPhysicalCustody)?;
        let realm_close =
            validate_ledger_slot_realm_close_v2(quote, slot.remaining(), custody.realm_collateral)?;
        raw.status = FundingLedgerStatusV2::Closed;
        raw.remaining = [0; 7];
        write_ledger_slot(bytes, entry_index, raw)?;
        let post = decode_funding_ledger_v2(bytes)?.authenticate(manifest_content_id, manifest)?;
        let ledger_can_close = post.all_closed();
        let remaining_native_lamports = slot.remaining().native_lamports_total();
        let expected_post_ledger_lamports = if ledger_can_close {
            0
        } else {
            custody
                .ledger_account_lamports
                .checked_sub(remaining_native_lamports)
                .ok_or(Error::UnderfundedPhysicalCustody)?
        };
        Ok(FundingLedgerEntryClosePlanV2 {
            native_rent_credit: custody.native_rent_credit,
            remaining_native_lamports,
            realm_token_beneficiary: realm_close.beneficiary,
            remaining_realm_collateral: realm_close.remaining,
            realm_collateral_donation: realm_close.donation,
            vault_rent_lamports: realm_close.vault_rent,
            vault_lamport_donation: realm_close.vault_donation,
            ledger_rent_lamports: if ledger_can_close {
                custody.exact_ledger_rent_lamports
            } else {
                0
            },
            ledger_lamport_donation: if ledger_can_close {
                ledger_lamport_surplus
            } else {
                0
            },
            expected_post_ledger_lamports,
            ledger_can_close,
            // Carved only from rent this close LIBERATED, and so nonzero only
            // on the final row close, where those two buckets are nonzero.
            crank_reward: if ledger_can_close {
                let liberated = custody
                    .exact_ledger_rent_lamports
                    .checked_add(ledger_lamport_surplus)
                    .ok_or(Error::ArithmeticOverflow)?;
                // `min`, never a guarded subtraction: a close that liberates
                // little pays little and is still admitted. A crank that can
                // refuse for money is an unturned crank.
                custody.crank_reward_cap.min(liberated)
            } else {
                0
            },
        })
    }
}

/// Ledger view whose exact immutable manifest binding has authenticated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedFundingLedgerV2<'ledger, 'manifest> {
    ledger: FundingLedgerV2<'ledger>,
    manifest_content_id: ContentId,
    manifest: CapabilityManifestV1<'manifest>,
}

impl<'ledger, 'manifest> AuthenticatedFundingLedgerV2<'ledger, 'manifest> {
    /// Derive one typed slot selected at the requested manifest index.
    pub fn slot(self, entry_index: u16) -> Result<AuthenticatedFundingSlotV2> {
        let raw = self.ledger.raw_slot(entry_index)?;
        let quote = self.manifest.entry(entry_index)?.funding_quote().amounts();
        let (remaining, released) = derive_funding_amounts_v2(raw.remaining, quote)?;
        match raw.status {
            FundingLedgerStatusV2::Pending => {
                if raw.activation_slot != 0
                    || remaining != quote
                    || released != FundingAmountsV1::default()
                {
                    return Err(Error::InvalidFundingStatus);
                }
            }
            FundingLedgerStatusV2::Active => {
                if remaining.rent().amount() != 0
                    || remaining.creation().amount() != 0
                    || released.rent() != quote.rent()
                    || released.creation() != quote.creation()
                {
                    return Err(Error::FundingConservationMismatch);
                }
            }
            FundingLedgerStatusV2::Closed => {
                if remaining != FundingAmountsV1::default() || released != quote {
                    return Err(Error::FundingConservationMismatch);
                }
            }
        }
        Ok(AuthenticatedFundingSlotV2 {
            entry_index,
            status: raw.status,
            activation_slot: raw.activation_slot,
            remaining,
            released,
        })
    }

    fn slot_by_row(self, row_index: u16) -> Result<AuthenticatedFundingSlotV2> {
        let entry_index = manifest_entry_for_ledger_row_v2(self.ledger.selected_mask, row_index)?;
        self.slot(entry_index)
    }

    /// Require one slot's state permits Market opening at `current_slot`.
    pub fn validate_market_open(self, entry_index: u16, current_slot: u64) -> Result<()> {
        let slot = self.slot(entry_index)?;
        let entry = self.manifest.entry(entry_index)?;
        // The set stands BESIDE the match rather than swallowing it: only one
        // conjunct of this refusal is over the ledger machine, and it is the
        // one that holds whatever the policy says. What remains is two policy
        // cases with their own refusals.
        if !FUNDING_LEDGER_MARKET_OPEN_ADMISSIBLE_STATES_V2.admits(slot.status()) {
            return Err(Error::InvalidFundingStatus);
        }
        match (entry.activation_policy(), slot.status()) {
            (ActivationPolicy::RequiredAtFounding, FundingLedgerStatusV2::Pending) => {
                Err(Error::FoundingCapabilityInactive)
            }
            (ActivationPolicy::PrepaidLazy, FundingLedgerStatusV2::Pending)
                if current_slot > entry.activation_deadline_slot() =>
            {
                Err(Error::ActivationDeadlineElapsed)
            }
            _ => Ok(()),
        }
    }

    /// Return the checked sum of remaining native principal across selected rows.
    pub fn remaining_native_lamports_total(self) -> Result<u64> {
        let mut total = 0_u64;
        let mut row_index = 0;
        while row_index < self.ledger.slot_count {
            total = total
                .checked_add(
                    self.slot_by_row(row_index)?
                        .remaining()
                        .native_lamports_total(),
                )
                .ok_or(Error::ArithmeticOverflow)?;
            row_index = row_index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        Ok(total)
    }

    /// Validate the one ledger account's Rent plus aggregate native custody.
    ///
    /// Per-entry semantic conservation was already checked pointwise by
    /// authentication, so an equal-total cross-slot substitution still
    /// refuses before this aggregate physical check.
    pub fn validate_native_custody(
        self,
        account_lamports: u64,
        exact_ledger_rent_lamports: u64,
        admit_donations: bool,
    ) -> Result<()> {
        let expected = exact_ledger_rent_lamports
            .checked_add(self.remaining_native_lamports_total()?)
            .ok_or(Error::ArithmeticOverflow)?;
        if (admit_donations && account_lamports < expected)
            || (!admit_donations && account_lamports != expected)
        {
            return Err(Error::PresentNativeLamportsMismatch);
        }
        Ok(())
    }

    /// Return true only after every selected entry has closed.
    pub fn all_closed(self) -> bool {
        let mut row_index = 0;
        while row_index < self.ledger.slot_count {
            if self
                .slot_by_row(row_index)
                .map(|slot| slot.status() != FundingLedgerStatusV2::Closed)
                .unwrap_or(true)
            {
                return false;
            }
            row_index += 1;
        }
        true
    }

    /// Borrow one exact slot for selected-only mutation postconditions.
    pub fn slot_bytes(self, entry_index: u16) -> Result<&'ledger [u8]> {
        self.ledger.slot_bytes(entry_index)
    }

    /// Return the hostile-decoded ledger view.
    pub const fn ledger(self) -> FundingLedgerV2<'ledger> {
        self.ledger
    }

    /// Return the exact authenticated manifest content identity.
    pub const fn manifest_content_id(self) -> ContentId {
        self.manifest_content_id
    }

    /// Rederive the rent-exempt minimum this ledger's account was funded at.
    pub fn funded_rent_minimum(self, account_bytes: usize) -> Result<u64> {
        self.ledger.funded_rent_minimum(account_bytes)
    }

    /// Validate custody against the rent the account was FUNDED at.
    ///
    /// This is `validate_native_custody` with its rent term supplied by the
    /// ledger's own record instead of by the caller's reading of the Rent
    /// sysvar, and it is the form every check over a pre-existing account
    /// should take. The refusal is its own code: when the arithmetic fails
    /// here, the term a reader must look at first is the PERSISTED rate.
    pub fn validate_recorded_native_custody(
        self,
        account_lamports: u64,
        account_bytes: usize,
        admit_donations: bool,
    ) -> Result<()> {
        let funded = self.funded_rent_minimum(account_bytes)?;
        self.validate_native_custody(account_lamports, funded, admit_donations)
            .map_err(|error| match error {
                Error::PresentNativeLamportsMismatch => Error::FundedRentNotEvidenced,
                other => other,
            })
    }
}

/// Exact physical custody presented while closing one subset-ledger row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FundingLedgerCloseCustodyV2 {
    ledger_account_lamports: u64,
    exact_ledger_rent_lamports: u64,
    native_rent_credit: [u8; 32],
    realm_collateral: Option<RealmCollateralCustodyV1>,
    crank_reward_cap: u64,
}

impl FundingLedgerCloseCustodyV2 {
    /// Observe one native-only row and its shared physical ledger.
    ///
    /// Pays no crank. This is the unchanged shape every existing caller uses.
    pub fn native_only(
        ledger_account_lamports: u64,
        exact_ledger_rent_lamports: u64,
        native_rent_credit: [u8; 32],
    ) -> Result<Self> {
        Self::new(
            ledger_account_lamports,
            exact_ledger_rent_lamports,
            native_rent_credit,
            None,
        )
    }

    /// Observe the same row while offering a capped reward to the crank.
    ///
    /// `crank_reward_cap` is chain-derived by the adapter -- a Rent minimum,
    /// never a source literal (`docs/design/FUNDED_CRANK_V1.md` §3). Passing
    /// zero is exactly [`Self::native_only`].
    ///
    /// **The reward is carved only from rent the crank LIBERATED**, never from
    /// anyone's principal: see [`FundingLedgerEntryClosePlanV2::crank_reward`].
    pub fn native_with_crank(
        ledger_account_lamports: u64,
        exact_ledger_rent_lamports: u64,
        native_rent_credit: [u8; 32],
        crank_reward_cap: u64,
    ) -> Result<Self> {
        let mut value = Self::new(
            ledger_account_lamports,
            exact_ledger_rent_lamports,
            native_rent_credit,
            None,
        )?;
        value.crank_reward_cap = crank_reward_cap;
        Ok(value)
    }

    /// Observe one row's independently derived Realm vault.
    pub fn with_realm_collateral(
        ledger_account_lamports: u64,
        exact_ledger_rent_lamports: u64,
        native_rent_credit: [u8; 32],
        realm_collateral: RealmCollateralCustodyV1,
    ) -> Result<Self> {
        Self::new(
            ledger_account_lamports,
            exact_ledger_rent_lamports,
            native_rent_credit,
            Some(realm_collateral),
        )
    }

    fn new(
        ledger_account_lamports: u64,
        exact_ledger_rent_lamports: u64,
        native_rent_credit: [u8; 32],
        realm_collateral: Option<RealmCollateralCustodyV1>,
    ) -> Result<Self> {
        require_nonzero_identifier(&native_rent_credit)?;
        if ledger_account_lamports < exact_ledger_rent_lamports {
            return Err(Error::UnderfundedPhysicalCustody);
        }
        Ok(Self {
            ledger_account_lamports,
            exact_ledger_rent_lamports,
            native_rent_credit,
            realm_collateral,
            crank_reward_cap: 0,
        })
    }

    /// Return the chain-derived ceiling on this close's crank reward.
    pub const fn crank_reward_cap(self) -> u64 {
        self.crank_reward_cap
    }

    /// Return all lamports observed in the shared physical ledger.
    pub const fn ledger_account_lamports(self) -> u64 {
        self.ledger_account_lamports
    }

    /// Return the exact current Rent reserve for the ledger width.
    pub const fn exact_ledger_rent_lamports(self) -> u64 {
        self.exact_ledger_rent_lamports
    }

    /// Return the Market-authenticated native RentCredit.
    pub const fn native_rent_credit(self) -> [u8; 32] {
        self.native_rent_credit
    }

    /// Return the optional row-specific Realm custody observation.
    pub const fn realm_collateral(self) -> Option<RealmCollateralCustodyV1> {
        self.realm_collateral
    }
}

/// Per-entry close plan. The adapter moves this principal, closes any
/// per-entry Realm vault, and closes/refunds ledger rent only when
/// [`Self::ledger_can_close`] is true.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FundingLedgerEntryClosePlanV2 {
    native_rent_credit: [u8; 32],
    remaining_native_lamports: u64,
    remaining_realm_collateral: u64,
    realm_token_beneficiary: Option<[u8; 32]>,
    realm_collateral_donation: u64,
    vault_rent_lamports: u64,
    vault_lamport_donation: u64,
    ledger_rent_lamports: u64,
    ledger_lamport_donation: u64,
    expected_post_ledger_lamports: u64,
    ledger_can_close: bool,
    crank_reward: u64,
}

impl FundingLedgerEntryClosePlanV2 {
    /// Return the Market-authenticated RentCredit receiving every native
    /// lamport this plan does not owe the crank.
    pub const fn native_rent_credit(self) -> [u8; 32] {
        self.native_rent_credit
    }

    /// Return the lamports owed to whoever turned this crank; zero unless a
    /// reward cap was offered AND this close is the one that frees the ledger.
    ///
    /// **Carved only from rent this close liberated** -- the ledger's own Rent
    /// reserve plus its surplus -- and never from `remaining_native_lamports`,
    /// which is a depositor's principal. The crank is paid out of value its own
    /// act released, so no participant is worse off than if it had never run:
    /// unturned, that rent stays locked in the ledger and reaches nobody.
    pub const fn crank_reward(self) -> u64 {
        self.crank_reward
    }

    /// Return every native lamport the RentCredit is owed, net of the crank.
    pub fn native_refund_total(self) -> Result<u64> {
        self.native_gross_total()?
            .checked_sub(self.crank_reward)
            .ok_or(Error::ArithmeticOverflow)
    }

    /// Return every native lamport leaving, before the crank is paid.
    fn native_gross_total(self) -> Result<u64> {
        self.ledger_sourced_total()?
            .checked_add(self.vault_lamport_donation)
            .ok_or(Error::ArithmeticOverflow)
    }

    /// Return the native lamports sourced from the shared physical ledger.
    ///
    /// Deliberately excludes `vault_lamport_donation`, which leaves a *different
    /// account* -- the row's Realm vault. Summing the two would make the ledger
    /// conservation below off by exactly that donation.
    fn ledger_sourced_total(self) -> Result<u64> {
        self.remaining_native_lamports
            .checked_add(self.ledger_rent_lamports)
            .and_then(|value| value.checked_add(self.ledger_lamport_donation))
            .ok_or(Error::ArithmeticOverflow)
    }

    /// Return the rent this close liberated, which is the crank's only source.
    fn liberated_total(self) -> Result<u64> {
        self.ledger_rent_lamports
            .checked_add(self.ledger_lamport_donation)
            .ok_or(Error::ArithmeticOverflow)
    }

    /// Refuse unless every native lamport leaving reaches exactly one recipient.
    ///
    /// **Not a tautology, and this is the one check worth having here.**
    /// `close_slot_in_place` derives the buckets independently -- principal
    /// from the slot's own remaining, rent from the observed Rent minimum,
    /// surplus from a subtraction against the observed account balance -- so
    /// this genuinely relates quantities that were computed apart. It also
    /// pins the property a second recipient puts at risk: that the crank's
    /// reward is *carved from* the refund rather than *added to* it, so the
    /// close can never pay out more than the ledger actually held.
    pub fn validate_native_conservation(self, observed_ledger_lamports: u64) -> Result<()> {
        // The crank is paid only out of rent it liberated, never principal.
        if self.crank_reward > self.liberated_total()? {
            return Err(Error::UnderfundedPhysicalCustody);
        }
        // The reward is carved FROM the refund, never added TO it.
        if self
            .native_refund_total()?
            .checked_add(self.crank_reward)
            .ok_or(Error::ArithmeticOverflow)?
            != self.native_gross_total()?
        {
            return Err(Error::UnderfundedPhysicalCustody);
        }
        // Every lamport the shared ledger held is either leaving or staying.
        let accounted = self
            .ledger_sourced_total()?
            .checked_add(self.expected_post_ledger_lamports)
            .ok_or(Error::ArithmeticOverflow)?;
        if accounted != observed_ledger_lamports {
            return Err(Error::UnderfundedPhysicalCustody);
        }
        Ok(())
    }

    /// Return this entry's remaining native principal.
    pub const fn remaining_native_lamports(self) -> u64 {
        self.remaining_native_lamports
    }

    /// Return this entry's remaining Realm collateral.
    pub const fn remaining_realm_collateral(self) -> u64 {
        self.remaining_realm_collateral
    }

    /// Return the quote-authenticated Realm-token beneficiary, if applicable.
    pub const fn realm_token_beneficiary(self) -> Option<[u8; 32]> {
        self.realm_token_beneficiary
    }

    /// Return unsolicited same-mint tokens, classified only as a refund gift.
    pub const fn realm_collateral_donation(self) -> u64 {
        self.realm_collateral_donation
    }

    /// Return the row vault's exact Rent reserve, or zero without a vault.
    pub const fn vault_rent_lamports(self) -> u64 {
        self.vault_rent_lamports
    }

    /// Return unsolicited vault lamports routed to the native RentCredit.
    pub const fn vault_lamport_donation(self) -> u64 {
        self.vault_lamport_donation
    }

    /// Return physical ledger Rent refunded only by the final row close.
    pub const fn ledger_rent_lamports(self) -> u64 {
        self.ledger_rent_lamports
    }

    /// Return shared-ledger surplus classified only on final physical close.
    pub const fn ledger_lamport_donation(self) -> u64 {
        self.ledger_lamport_donation
    }

    /// Return exact shared-ledger lamports expected after this close.
    pub const fn expected_post_ledger_lamports(self) -> u64 {
        self.expected_post_ledger_lamports
    }

    /// Return whether every slot is now Closed and ledger rent may be refunded.
    pub const fn ledger_can_close(self) -> bool {
        self.ledger_can_close
    }
}

/// Canonical PDA seed projection for one controller-owned subset ledger.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityFundingLedgerDerivationV2 {
    controller_program: [u8; 32],
    market: [u8; 32],
    generation_le: [u8; 8],
    manifest_content_id: [u8; 32],
    selected_mask_le: [u8; 2],
}

impl CapabilityFundingLedgerDerivationV2 {
    /// Bind controller, Market generation, manifest identity, and subset mask.
    pub fn new(
        controller_program: [u8; 32],
        market: [u8; 32],
        generation: u64,
        manifest_content_id: ContentId,
        ledger: FundingLedgerV2<'_>,
    ) -> Result<Self> {
        require_nonzero_identifier(&controller_program)?;
        require_nonzero_identifier(&market)?;
        if ledger.manifest_content_id() != manifest_content_id {
            return Err(Error::FundingBindingMismatch);
        }
        Ok(Self {
            controller_program,
            market,
            generation_le: generation.to_le_bytes(),
            manifest_content_id: manifest_content_id.to_bytes(),
            selected_mask_le: ledger.selected_mask().to_le_bytes(),
        })
    }

    /// Return exact ordered PDA seed components.
    pub fn seed_components(&self) -> [&[u8]; 6] {
        [
            CAPABILITY_FUNDING_LEDGER_PDA_DOMAIN_V2,
            self.controller_program.as_slice(),
            self.market.as_slice(),
            self.generation_le.as_slice(),
            self.manifest_content_id.as_slice(),
            self.selected_mask_le.as_slice(),
        ]
    }
}

/// Per-entry token-signing authority below one FundingLedgerV2 account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityFundingLedgerAuthorityDerivationV2 {
    ledger: [u8; 32],
    entry_index_le: [u8; 2],
}

impl CapabilityFundingLedgerAuthorityDerivationV2 {
    /// Bind authority to the ledger key and exact selected manifest index.
    pub fn new(ledger: [u8; 32], entry_index: u16) -> Result<Self> {
        require_nonzero_identifier(&ledger)?;
        if usize::from(entry_index) >= crate::capability_manifest::MAX_CAPABILITIES {
            return Err(Error::InvalidDependency);
        }
        Ok(Self {
            ledger,
            entry_index_le: entry_index.to_le_bytes(),
        })
    }

    /// Return exact ordered authority PDA seeds.
    pub fn seed_components(&self) -> [&[u8]; 3] {
        [
            CAPABILITY_FUNDING_LEDGER_AUTHORITY_PDA_DOMAIN_V2,
            self.ledger.as_slice(),
            self.entry_index_le.as_slice(),
        ]
    }
}

/// Optional Realm-collateral vault below one per-entry ledger authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityFundingLedgerVaultDerivationV2 {
    funding_authority: [u8; 32],
    token_program: [u8; 32],
    mint: [u8; 32],
}

impl CapabilityFundingLedgerVaultDerivationV2 {
    /// Construct exact vault seeds from one entry authority and quote binding.
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
            CAPABILITY_FUNDING_LEDGER_VAULT_PDA_DOMAIN_V2,
            self.funding_authority.as_slice(),
            self.token_program.as_slice(),
            self.mint.as_slice(),
        ]
    }
}

fn decode_funding_ledger_v2(bytes: &[u8]) -> Result<FundingLedgerV2<'_>> {
    FundingLedgerV2::decode(bytes)
}

/// Return the checked absolute byte offset of one dense ledger slot.
pub fn funding_ledger_slot_offset_v2(entry_index: u16) -> Result<usize> {
    FUNDING_LEDGER_HEADER_BYTES_V2
        .checked_add(
            usize::from(entry_index)
                .checked_mul(FUNDING_LEDGER_SLOT_BYTES_V2)
                .ok_or(Error::ArithmeticOverflow)?,
        )
        .ok_or(Error::ArithmeticOverflow)
}

/// Map a selected manifest index to its ascending physical row.
pub fn funding_ledger_row_for_manifest_entry_v2(
    selected_mask: u16,
    entry_index: u16,
) -> Result<u16> {
    if entry_index >= FUNDING_LEDGER_MASK_BITS_V2 {
        return Err(Error::InvalidDependency);
    }
    let entry_bit = 1_u16
        .checked_shl(u32::from(entry_index))
        .ok_or(Error::InvalidDependency)?;
    if selected_mask & entry_bit == 0 {
        return Err(Error::InvalidDependency);
    }
    let lower_mask = entry_bit.checked_sub(1).ok_or(Error::ArithmeticOverflow)?;
    u16::try_from((selected_mask & lower_mask).count_ones()).map_err(|_| Error::ArithmeticOverflow)
}

/// Map one physical row to its selected manifest index.
pub fn manifest_entry_for_ledger_row_v2(selected_mask: u16, row_index: u16) -> Result<u16> {
    let slot_count = funding_ledger_slot_count_v2(selected_mask)?;
    if row_index >= slot_count {
        return Err(Error::InvalidDependency);
    }
    let mut manifest_index = 0_u16;
    let mut observed_row = 0_u16;
    while manifest_index < FUNDING_LEDGER_MASK_BITS_V2 {
        let bit = 1_u16
            .checked_shl(u32::from(manifest_index))
            .ok_or(Error::ArithmeticOverflow)?;
        if selected_mask & bit != 0 {
            if observed_row == row_index {
                return Ok(manifest_index);
            }
            observed_row = observed_row
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?;
        }
        manifest_index = manifest_index
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
    }
    Err(Error::InvalidDependency)
}

/// Return the checked absolute offset of one slot's remaining amount.
pub fn funding_ledger_remaining_offset_v2(
    entry_index: u16,
    compartment: FundingCompartment,
) -> Result<usize> {
    let compartment_offset = LEDGER_SLOT_REMAINING_OFFSETS_V2
        .get(funding_compartment_index(compartment))
        .copied()
        .ok_or(Error::InvalidDependency)?;
    funding_ledger_slot_offset_v2(entry_index)?
        .checked_add(compartment_offset)
        .ok_or(Error::ArithmeticOverflow)
}

fn funding_compartment_index(compartment: FundingCompartment) -> usize {
    match compartment {
        FundingCompartment::Rent => 0,
        FundingCompartment::Creation => 1,
        FundingCompartment::Work => 2,
        FundingCompartment::Provider => 3,
        FundingCompartment::Bounty => 4,
        FundingCompartment::Liquidity => 5,
        FundingCompartment::Service => 6,
    }
}

fn write_ledger_row(output: &mut [u8], row_index: u16, slot: FundingLedgerRawSlotV2) -> Result<()> {
    let selected_mask = read_u16(output, LEDGER_SELECTED_MASK_OFFSET_V2)?;
    let slot_count = funding_ledger_slot_count_v2(selected_mask)?;
    if row_index >= slot_count || output.len() != funding_ledger_bytes_v2(slot_count)? {
        return Err(Error::InvalidLength);
    }
    let start = funding_ledger_slot_offset_v2(row_index)?;
    let end = start
        .checked_add(FUNDING_LEDGER_SLOT_BYTES_V2)
        .ok_or(Error::ArithmeticOverflow)?;
    output
        .get_mut(start..end)
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(&slot.to_bytes());
    Ok(())
}

fn write_ledger_slot(
    output: &mut [u8],
    entry_index: u16,
    slot: FundingLedgerRawSlotV2,
) -> Result<()> {
    let selected_mask = read_u16(output, LEDGER_SELECTED_MASK_OFFSET_V2)?;
    let row_index = funding_ledger_row_for_manifest_entry_v2(selected_mask, entry_index)?;
    write_ledger_row(output, row_index, slot)
}

fn manifest_valid_mask_v2(entry_count: u16) -> Result<u16> {
    if entry_count == 0 || usize::from(entry_count) > crate::capability_manifest::MAX_CAPABILITIES {
        return Err(Error::TooManyCapabilities);
    }
    if entry_count == FUNDING_LEDGER_MASK_BITS_V2 {
        return Ok(u16::MAX);
    }
    1_u16
        .checked_shl(u32::from(entry_count))
        .and_then(|exclusive_limit| exclusive_limit.checked_sub(1))
        .ok_or(Error::ArithmeticOverflow)
}

fn validate_selected_mask_v2(entry_count: u16, selected_mask: u16) -> Result<()> {
    let valid_mask = manifest_valid_mask_v2(entry_count)?;
    if selected_mask == 0 || selected_mask & !valid_mask != 0 {
        return Err(Error::InvalidDependency);
    }
    Ok(())
}

struct LedgerRealmCloseV2 {
    beneficiary: Option<[u8; 32]>,
    remaining: u64,
    donation: u64,
    vault_rent: u64,
    vault_donation: u64,
}

fn validate_ledger_slot_realm_close_v2(
    quote: FundingQuoteV1,
    remaining: FundingAmountsV1,
    realm_collateral: Option<RealmCollateralCustodyV1>,
) -> Result<LedgerRealmCloseV2> {
    match (quote.realm_collateral(), realm_collateral) {
        (None, None) => Ok(LedgerRealmCloseV2 {
            beneficiary: None,
            remaining: 0,
            donation: 0,
            vault_rent: 0,
            vault_donation: 0,
        }),
        (Some(binding), Some(realm)) => {
            let observed = realm.observation();
            if realm.realm_id() != binding.realm_id()
                || realm.collateral_release_id() != binding.collateral_release_id()
                || observed.token_program() != binding.token_program()
                || observed.mint() != binding.mint()
            {
                return Err(Error::RealmCollateralBindingMismatch);
            }
            let required = remaining.realm_collateral_total();
            let donation = observed
                .token_amount()
                .checked_sub(required)
                .ok_or(Error::UnderfundedPhysicalCustody)?;
            let vault_lamport_donation = observed
                .account_lamports()
                .checked_sub(observed.exact_rent_lamports())
                .ok_or(Error::UnderfundedPhysicalCustody)?;
            Ok(LedgerRealmCloseV2 {
                beneficiary: Some(binding.refund_token_beneficiary()),
                remaining: required,
                donation,
                vault_rent: observed.exact_rent_lamports(),
                vault_donation: vault_lamport_donation,
            })
        }
        (Some(_), None) => Err(Error::MissingRealmCollateralVault),
        (None, Some(_)) => Err(Error::UnexpectedRealmCollateralVault),
    }
}

fn derive_funding_amounts_v2(
    raw_remaining: [u64; 7],
    quote: FundingAmountsV1,
) -> Result<(FundingAmountsV1, FundingAmountsV1)> {
    let mut remaining = [CompartmentFundingV1::not_applicable(); 7];
    let mut released = [CompartmentFundingV1::not_applicable(); 7];
    for (((remaining_value, released_value), raw_value), compartment) in remaining
        .iter_mut()
        .zip(released.iter_mut())
        .zip(raw_remaining)
        .zip(FUNDING_COMPARTMENTS)
    {
        let quoted = quote.compartment(compartment);
        let released_amount = quoted
            .amount()
            .checked_sub(raw_value)
            .ok_or(Error::FundingConservationMismatch)?;
        *remaining_value = allocation_or_na(quoted.asset_class(), raw_value)?;
        *released_value = allocation_or_na(quoted.asset_class(), released_amount)?;
    }
    Ok((
        FundingAmountsV1::new(
            remaining[0],
            remaining[1],
            remaining[2],
            remaining[3],
            remaining[4],
            remaining[5],
            remaining[6],
        )?,
        FundingAmountsV1::new(
            released[0],
            released[1],
            released[2],
            released[3],
            released[4],
            released[5],
            released[6],
        )?,
    ))
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
    use crate::capability_manifest::{
        CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, MANIFEST_HEADER_BYTES,
        MAX_DEPENDENCIES_PER_CAPABILITY,
    };

    fn id(value: u8) -> ContentId {
        ContentId::new([value; 32]).expect("nonzero fixture")
    }

    /// `solana_program::rent::Rent::default()`'s exemption-scaled rate: 3480
    /// lamports per byte-year at an exemption threshold of 2.0. Stated as a
    /// literal because this crate is `no_std` and depends on no Solana types.
    const DEFAULT_FUNDED_RENT_RATE: u32 = 6960;

    /// The published offset is a projection of the encoder, not a second layout.
    ///
    /// A data-defined activation reads this offset out of the live account with
    /// one `ProjectDataU64`, so it has to name the same eight bytes
    /// `remaining().rent().amount()` does, for any value.
    #[test]
    fn the_published_rent_amount_offset_reads_the_rent_quote() {
        for lamports in [1_u64, 7, 1_234_567, u64::from(u32::MAX)] {
            let amounts = FundingAmountsV1::new(
                native(lamports),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
            )
            .expect("amounts");
            let mut storage = [0_u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
            let decoded = manifest(
                &mut storage,
                FundingQuoteV1::new(amounts, None).expect("quote"),
            );
            let state = FundingStateV1::new(
                id(1),
                decoded,
                0,
                FundingCustodyObservationV1::native_only(lamports, 0).expect("custody"),
            )
            .expect("state");
            assert_eq!(state.remaining().rent().amount(), lamports);
            let bytes = state.to_bytes();
            let offset = FUNDING_STATE_REMAINING_RENT_AMOUNT_OFFSET_V1;
            let projected = u64::from_le_bytes(
                bytes
                    .get(offset..offset + 8)
                    .expect("rent amount span")
                    .try_into()
                    .expect("eight bytes"),
            );
            assert_eq!(projected, lamports);
        }
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

    fn native_quote(
        rent: u64,
        creation: u64,
        work: u64,
        bounty: u64,
        service: u64,
    ) -> FundingQuoteV1 {
        FundingQuoteV1::new(
            FundingAmountsV1::new(
                native(rent),
                native(creation),
                if work == 0 {
                    CompartmentFundingV1::not_applicable()
                } else {
                    native(work)
                },
                CompartmentFundingV1::not_applicable(),
                if bounty == 0 {
                    CompartmentFundingV1::not_applicable()
                } else {
                    native(bounty)
                },
                CompartmentFundingV1::not_applicable(),
                if service == 0 {
                    CompartmentFundingV1::not_applicable()
                } else {
                    native(service)
                },
            )
            .expect("native amounts"),
            None,
        )
        .expect("native quote")
    }

    fn ledger_manifest<'a>(
        storage: &'a mut [u8; MANIFEST_HEADER_BYTES + 3 * CAPABILITY_ENTRY_BYTES],
    ) -> CapabilityManifestV1<'a> {
        let quotes = [
            native_quote(10, 20, 30, 0, 0),
            native_quote(11, 21, 0, 40, 0),
            native_quote(12, 22, 0, 0, 50),
        ];
        let mut entries = [
            CapabilityEntryV1::new(
                id(1),
                id(11),
                id(21),
                id(31),
                id(41),
                id(51),
                ActivationPolicy::RequiredAtFounding,
                0,
                0,
                [0; MAX_DEPENDENCIES_PER_CAPABILITY],
                quotes[0],
            )
            .expect("entry zero"),
            CapabilityEntryV1::new(
                id(2),
                id(12),
                id(22),
                id(32),
                id(42),
                id(52),
                ActivationPolicy::RequiredAtFounding,
                0,
                0,
                [0; MAX_DEPENDENCIES_PER_CAPABILITY],
                quotes[1],
            )
            .expect("entry one"),
            CapabilityEntryV1::new(
                id(3),
                id(13),
                id(23),
                id(33),
                id(43),
                id(53),
                ActivationPolicy::RequiredAtFounding,
                0,
                0,
                [0; MAX_DEPENDENCIES_PER_CAPABILITY],
                quotes[2],
            )
            .expect("entry two"),
        ];
        // The manifest constructor owns canonical kind order. Keep the local
        // fixture explicit so a future quote edit cannot hide a reordered row.
        entries.sort_by_key(|entry| entry.kind_id().to_bytes());
        CapabilityManifestV1::encode_into(&entries, storage).expect("ledger manifest")
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
        assert!(
            CAPABILITY_FUNDING_PDA_DOMAIN_V1.len()
                <= crate::capability_manifest::SVM_MAX_PDA_SEED_BYTES
        );
        assert!(
            CAPABILITY_FUNDING_AUTHORITY_PDA_DOMAIN_V1.len()
                <= crate::capability_manifest::SVM_MAX_PDA_SEED_BYTES
        );
        assert!(
            CAPABILITY_FUNDING_VAULT_PDA_DOMAIN_V1.len()
                <= crate::capability_manifest::SVM_MAX_PDA_SEED_BYTES
        );
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

    #[test]
    fn funding_ledger_v2_has_exact_dynamic_width_and_manifest_derived_views() {
        assert_eq!(funding_ledger_bytes_v2(1), Ok(120));
        assert_eq!(funding_ledger_bytes_v2(3), Ok(264));
        assert_eq!(funding_ledger_bytes_v2(4), Ok(336));
        assert_eq!(funding_ledger_bytes_v2(16), Ok(1_200));
        assert_eq!(funding_ledger_bytes_v2(0), Err(Error::TooManyCapabilities));
        assert_eq!(funding_ledger_bytes_v2(17), Err(Error::TooManyCapabilities));

        let mut manifest_storage = [0_u8; MANIFEST_HEADER_BYTES + 3 * CAPABILITY_ENTRY_BYTES];
        let manifest = ledger_manifest(&mut manifest_storage);
        let manifest_id = id(70);
        let mut ledger_bytes = [0_u8; 264];
        FundingLedgerV2::initialize(
            &mut ledger_bytes,
            manifest_id,
            manifest,
            0b111,
            DEFAULT_FUNDED_RENT_RATE,
        )
        .expect("initialize");
        let ledger = FundingLedgerV2::decode(&ledger_bytes).expect("decode");
        assert_eq!(ledger.selected_mask(), 0b111);
        assert_eq!(ledger.slot_count(), 3);
        assert_eq!(ledger.manifest_content_id(), manifest_id);
        let authenticated = ledger
            .authenticate(manifest_id, manifest)
            .expect("authenticate");
        for index in 0..3 {
            let slot = authenticated.slot(index).expect("slot");
            assert_eq!(slot.entry_index(), index);
            assert_eq!(slot.status(), FundingLedgerStatusV2::Pending);
            assert_eq!(slot.activation_slot(), 0);
            assert_eq!(
                slot.remaining(),
                manifest
                    .entry(index)
                    .expect("entry")
                    .funding_quote()
                    .amounts()
            );
            assert_eq!(slot.released(), FundingAmountsV1::default());
        }
        assert_eq!(authenticated.remaining_native_lamports_total(), Ok(216));
        assert_eq!(
            authenticated.validate_native_custody(316, 100, false),
            Ok(())
        );
        assert_eq!(
            authenticated.validate_native_custody(317, 100, false),
            Err(Error::PresentNativeLamportsMismatch)
        );
        assert_eq!(
            authenticated.validate_native_custody(317, 100, true),
            Ok(())
        );
        assert_eq!(
            ledger.authenticate(id(71), manifest),
            Err(Error::FundingBindingMismatch)
        );
    }

    #[test]
    fn funding_ledger_v2_sparse_subset_rows_are_ordered_and_disjoint() {
        let mut manifest_storage = [0_u8; MANIFEST_HEADER_BYTES + 3 * CAPABILITY_ENTRY_BYTES];
        let manifest = ledger_manifest(&mut manifest_storage);
        let manifest_id = id(70);
        let mut bytes = [0_u8; 192];
        FundingLedgerV2::initialize(
            &mut bytes,
            manifest_id,
            manifest,
            0b101,
            DEFAULT_FUNDED_RENT_RATE,
        )
        .expect("sparse subset");
        let ledger = FundingLedgerV2::decode(&bytes).expect("decode sparse");
        assert_eq!(ledger.selected_mask(), 0b101);
        assert_eq!(ledger.slot_count(), 2);
        assert_eq!(funding_ledger_row_for_manifest_entry_v2(0b101, 0), Ok(0));
        assert_eq!(funding_ledger_row_for_manifest_entry_v2(0b101, 2), Ok(1));
        assert_eq!(manifest_entry_for_ledger_row_v2(0b101, 0), Ok(0));
        assert_eq!(manifest_entry_for_ledger_row_v2(0b101, 1), Ok(2));
        let authenticated = ledger
            .authenticate(manifest_id, manifest)
            .expect("authenticate");
        assert_eq!(authenticated.slot(0).expect("entry zero").entry_index(), 0);
        assert_eq!(authenticated.slot(2).expect("entry two").entry_index(), 2);
        assert_eq!(authenticated.slot(1), Err(Error::InvalidDependency));

        let mut out_of_range = bytes;
        put_u16(&mut out_of_range, LEDGER_SELECTED_MASK_OFFSET_V2, 0b1001);
        let structurally_valid = FundingLedgerV2::decode(&out_of_range).expect("same-width mask");
        assert_eq!(
            structurally_valid.authenticate(manifest_id, manifest),
            Err(Error::InvalidDependency)
        );

        assert_eq!(
            validate_funding_ledger_masks_v2(3, 0b111, &[0b011, 0b100]),
            Ok(())
        );
        assert_eq!(
            validate_funding_ledger_masks_v2(3, 0b111, &[0b100, 0b011]),
            Err(Error::InvalidDependency)
        );
        assert_eq!(
            validate_funding_ledger_masks_v2(3, 0b111, &[0b101, 0b011]),
            Err(Error::InvalidDependency)
        );
        assert_eq!(
            validate_funding_ledger_masks_v2(3, 0b111, &[0b011]),
            Err(Error::InvalidDependency)
        );
        assert_eq!(
            validate_funding_ledger_masks_v2(3, 0b111, &[0b011, 0b1000]),
            Err(Error::InvalidDependency)
        );

        let derivation =
            CapabilityFundingLedgerDerivationV2::new([6; 32], [7; 32], 9, manifest_id, ledger)
                .expect("derive sparse ledger");
        assert_eq!(derivation.seed_components()[1], [6; 32].as_slice());
        assert_eq!(
            derivation.seed_components()[5],
            0b101_u16.to_le_bytes().as_slice()
        );
    }

    #[test]
    fn funding_ledger_v2_hostile_structure_and_pointwise_quotes_refuse() {
        let mut manifest_storage = [0_u8; MANIFEST_HEADER_BYTES + 3 * CAPABILITY_ENTRY_BYTES];
        let manifest = ledger_manifest(&mut manifest_storage);
        let manifest_id = id(70);
        let mut bytes = [0_u8; 264];
        FundingLedgerV2::initialize(
            &mut bytes,
            manifest_id,
            manifest,
            0b111,
            DEFAULT_FUNDED_RENT_RATE,
        )
        .expect("initialize");

        assert_eq!(
            FundingLedgerV2::decode(bytes.get(..bytes.len() - 1).expect("short ledger")),
            Err(Error::InvalidLength)
        );
        for (offset, value, expected) in [
            (0, 0, Error::InvalidMagic),
            (LEDGER_SCHEMA_OFFSET_V2, 1, Error::UnsupportedSchema),
            (
                funding_ledger_slot_offset_v2(1).expect("slot") + LEDGER_SLOT_RESERVED_OFFSET_V2,
                1,
                Error::NonCanonicalReservedBytes,
            ),
            (
                funding_ledger_slot_offset_v2(1).expect("slot") + LEDGER_SLOT_STATUS_OFFSET_V2,
                9,
                Error::UnknownFundingLedgerStatus,
            ),
        ] {
            let mut hostile = bytes;
            *hostile.get_mut(offset).expect("hostile coordinate") = value;
            assert_eq!(FundingLedgerV2::decode(&hostile), Err(expected));
        }

        // The four bytes this header once reserved now carry the founding's
        // exemption-scaled rent rate, so a zeroed rate is what a hostile header
        // gets refused for at that coordinate.
        let mut missing_rate = bytes;
        put_u32(&mut missing_rate, LEDGER_FUNDED_RENT_RATE_OFFSET_V2, 0);
        assert_eq!(
            FundingLedgerV2::decode(&missing_rate),
            Err(Error::FundedRentRateMissing)
        );

        let mut wrong_count = bytes;
        put_u16(&mut wrong_count, LEDGER_SELECTED_MASK_OFFSET_V2, 0b1111);
        assert_eq!(
            FundingLedgerV2::decode(&wrong_count),
            Err(Error::InvalidLength)
        );

        let mut pending_debit = bytes;
        let work =
            funding_ledger_remaining_offset_v2(0, FundingCompartment::Work).expect("work offset");
        put_u64(&mut pending_debit, work, 29);
        let decoded = FundingLedgerV2::decode(&pending_debit).expect("structural decode");
        assert_eq!(
            decoded.authenticate(manifest_id, manifest),
            Err(Error::InvalidFundingStatus)
        );

        let mut above_quote = bytes;
        put_u64(&mut above_quote, work, 31);
        let decoded = FundingLedgerV2::decode(&above_quote).expect("structural decode");
        assert_eq!(
            decoded.authenticate(manifest_id, manifest),
            Err(Error::FundingConservationMismatch)
        );

        let mut swapped = bytes;
        let first: [u8; FUNDING_LEDGER_SLOT_BYTES_V2] = FundingLedgerV2::decode(&bytes)
            .expect("ledger")
            .slot_bytes(0)
            .expect("first")
            .try_into()
            .expect("slot bytes");
        let second: [u8; FUNDING_LEDGER_SLOT_BYTES_V2] = FundingLedgerV2::decode(&bytes)
            .expect("ledger")
            .slot_bytes(1)
            .expect("second")
            .try_into()
            .expect("slot bytes");
        let first_start = funding_ledger_slot_offset_v2(0).expect("first offset");
        let second_start = funding_ledger_slot_offset_v2(1).expect("second offset");
        swapped
            .get_mut(first_start..first_start + FUNDING_LEDGER_SLOT_BYTES_V2)
            .expect("first destination")
            .copy_from_slice(&second);
        swapped
            .get_mut(second_start..second_start + FUNDING_LEDGER_SLOT_BYTES_V2)
            .expect("second destination")
            .copy_from_slice(&first);
        let decoded = FundingLedgerV2::decode(&swapped).expect("structural decode");
        assert!(decoded.authenticate(manifest_id, manifest).is_err());
    }

    #[test]
    fn funding_ledger_v2_transitions_touch_only_the_selected_slot_and_refuse_replay() {
        let mut manifest_storage = [0_u8; MANIFEST_HEADER_BYTES + 3 * CAPABILITY_ENTRY_BYTES];
        let manifest = ledger_manifest(&mut manifest_storage);
        let manifest_id = id(70);
        let mut bytes = [0_u8; 264];
        FundingLedgerV2::initialize(
            &mut bytes,
            manifest_id,
            manifest,
            0b111,
            DEFAULT_FUNDED_RENT_RATE,
        )
        .expect("initialize");
        let before = bytes;
        let debit = FundingLedgerV2::activate_in_place(&mut bytes, manifest_id, manifest, 1, 9)
            .expect("activate selected slot");
        assert_eq!(debit.rent_lamports(), 11);
        assert_eq!(debit.creation_lamports(), 21);
        let ledger = FundingLedgerV2::decode(&bytes).expect("post ledger");
        let before_ledger = FundingLedgerV2::decode(&before).expect("pre ledger");
        assert_eq!(ledger.slot_bytes(0), before_ledger.slot_bytes(0));
        assert_eq!(ledger.slot_bytes(2), before_ledger.slot_bytes(2));
        let activated = ledger
            .authenticate(manifest_id, manifest)
            .expect("post authenticate")
            .slot(1)
            .expect("slot");
        assert_eq!(activated.status(), FundingLedgerStatusV2::Active);
        assert_eq!(activated.activation_slot(), 9);
        assert_eq!(activated.remaining().rent().amount(), 0);
        assert_eq!(activated.released().rent().amount(), 11);

        let replay_prestate = bytes;
        assert_eq!(
            FundingLedgerV2::activate_in_place(&mut bytes, manifest_id, manifest, 1, 10),
            Err(Error::InvalidFundingStatus)
        );
        assert_eq!(bytes, replay_prestate);

        let release_prestate = bytes;
        let release = FundingLedgerV2::release_in_place(
            &mut bytes,
            manifest_id,
            manifest,
            1,
            FundingCompartment::Bounty,
            7,
        )
        .expect("release");
        assert_eq!(release.amount(), 7);
        assert_eq!(release.asset_class(), FundingAssetClassV1::NativeLamports);
        let ledger = FundingLedgerV2::decode(&bytes).expect("release ledger");
        let release_before = FundingLedgerV2::decode(&release_prestate).expect("release prestate");
        assert_eq!(ledger.slot_bytes(0), release_before.slot_bytes(0));
        assert_eq!(ledger.slot_bytes(2), release_before.slot_bytes(2));
        let slot = ledger
            .authenticate(manifest_id, manifest)
            .expect("authenticate release")
            .slot(1)
            .expect("slot");
        assert_eq!(slot.remaining().bounty().amount(), 33);
        assert_eq!(slot.released().bounty().amount(), 7);

        let insufficient_prestate = bytes;
        assert_eq!(
            FundingLedgerV2::release_in_place(
                &mut bytes,
                manifest_id,
                manifest,
                1,
                FundingCompartment::Bounty,
                34,
            ),
            Err(Error::InsufficientCompartmentPrincipal)
        );
        assert_eq!(bytes, insufficient_prestate);
    }

    /// Replay every Lean-decided activation case through `activate_in_place`.
    ///
    /// `CapabilityFundingLedgerV2.lean` was an import-graph island: nothing
    /// imported it while this function shipped on chain. The corpus is the
    /// bridge, and the module is now imported by the root.
    ///
    /// Two conjuncts are deliberately outside the corpus and named in the Lean
    /// docstring: the `PrepaidLazy` activation deadline, which Lean has no
    /// notion of, and the post-write re-authentication, which is atomicity
    /// rather than rule.
    #[test]
    fn lean_activation_corpus_replays_through_funding_ledger_v2() {
        use crate::capability_manifest::generated_funding_activation_corpus::{
            FUNDING_ACTIVATION_COMPARTMENTS, FUNDING_ACTIVATION_VECTORS_V1,
            FUNDING_ACTIVATION_ZEROED,
        };

        const ORDER: [FundingCompartment; 7] = [
            FundingCompartment::Rent,
            FundingCompartment::Creation,
            FundingCompartment::Work,
            FundingCompartment::Provider,
            FundingCompartment::Bounty,
            FundingCompartment::Liquidity,
            FundingCompartment::Service,
        ];
        assert_eq!(FUNDING_ACTIVATION_COMPARTMENTS, ORDER.len());

        let mut manifest_storage = [0_u8; MANIFEST_HEADER_BYTES + 3 * CAPABILITY_ENTRY_BYTES];
        let manifest = ledger_manifest(&mut manifest_storage);
        let manifest_id = id(70);

        for vector in FUNDING_ACTIVATION_VECTORS_V1 {
            let mut bytes = [0_u8; 264];
            FundingLedgerV2::initialize(
                &mut bytes,
                manifest_id,
                manifest,
                0b111,
                DEFAULT_FUNDED_RENT_RATE,
            )
            .expect("initialize");

            // Drive slot 1 to this vector's status through the real transitions;
            // writing a status byte by hand would bypass the authentication the
            // rule is stated over.
            if vector.status > 0 {
                FundingLedgerV2::activate_in_place(&mut bytes, manifest_id, manifest, 1, 9)
                    .expect("reach Active");
            }
            if vector.status == 2 {
                let exact_rent = 100;
                let ledger_lamports = exact_rent
                    + FundingLedgerV2::decode(&bytes)
                        .expect("ledger")
                        .authenticate(manifest_id, manifest)
                        .expect("manifest")
                        .remaining_native_lamports_total()
                        .expect("native total");
                let custody =
                    FundingLedgerCloseCustodyV2::native_only(ledger_lamports, exact_rent, [99; 32])
                        .expect("close custody");
                FundingLedgerV2::close_slot_in_place(&mut bytes, manifest_id, manifest, 1, custody)
                    .expect("reach Closed");
            }

            let before = FundingLedgerV2::decode(&bytes)
                .expect("pre ledger")
                .authenticate(manifest_id, manifest)
                .expect("pre manifest")
                .slot(1)
                .expect("pre slot")
                .remaining();

            let result =
                FundingLedgerV2::activate_in_place(&mut bytes, manifest_id, manifest, 1, 11);
            assert_eq!(result.is_ok(), vector.admits, "{}", vector.name);

            if !vector.admits {
                assert_eq!(
                    result.err(),
                    Some(Error::InvalidFundingStatus),
                    "{}",
                    vector.name
                );
                continue;
            }

            let after = FundingLedgerV2::decode(&bytes)
                .expect("post ledger")
                .authenticate(manifest_id, manifest)
                .expect("post manifest")
                .slot(1)
                .expect("post slot");
            assert_eq!(
                after.status(),
                FundingLedgerStatusV2::Active,
                "{}",
                vector.name
            );
            assert_eq!(after.activation_slot(), 11, "{}", vector.name);

            // Lean owns which compartments the release zeroes; the Rust writes
            // remaining[0] and remaining[1] by hand, and this is what says those
            // are the right two and that the other five are untouched.
            for (index, compartment) in ORDER.into_iter().enumerate() {
                let observed = after.remaining().compartment(compartment).amount();
                if *FUNDING_ACTIVATION_ZEROED.get(index).expect("zeroed flag") {
                    assert_eq!(observed, 0, "{} compartment {index}", vector.name);
                } else {
                    assert_eq!(
                        observed,
                        before.compartment(compartment).amount(),
                        "{} compartment {index}",
                        vector.name
                    );
                }
            }
        }
    }

    #[test]
    fn funding_ledger_v2_close_tombstones_one_slot_and_refunds_rent_only_last() {
        let mut manifest_storage = [0_u8; MANIFEST_HEADER_BYTES + 3 * CAPABILITY_ENTRY_BYTES];
        let manifest = ledger_manifest(&mut manifest_storage);
        let manifest_id = id(70);
        let mut bytes = [0_u8; 264];
        FundingLedgerV2::initialize(
            &mut bytes,
            manifest_id,
            manifest,
            0b111,
            DEFAULT_FUNDED_RENT_RATE,
        )
        .expect("initialize");

        for entry_index in 0..3 {
            FundingLedgerV2::activate_in_place(
                &mut bytes,
                manifest_id,
                manifest,
                entry_index,
                9 + u64::from(entry_index),
            )
            .expect("activate");
        }
        let exact_rent = 100;
        let donation = 5;
        let native_rent_credit = [99; 32];
        let mut ledger_lamports = exact_rent
            + FundingLedgerV2::decode(&bytes)
                .expect("activated ledger")
                .authenticate(manifest_id, manifest)
                .expect("activated manifest")
                .remaining_native_lamports_total()
                .expect("native total")
            + donation;
        for entry_index in 0..3 {
            let prestate = bytes;
            let custody = FundingLedgerCloseCustodyV2::native_only(
                ledger_lamports,
                exact_rent,
                native_rent_credit,
            )
            .expect("close custody");
            let close = FundingLedgerV2::close_slot_in_place(
                &mut bytes,
                manifest_id,
                manifest,
                entry_index,
                custody,
            )
            .expect("close slot");
            assert_eq!(close.ledger_can_close(), entry_index == 2);
            assert_eq!(close.native_rent_credit(), native_rent_credit);
            assert_eq!(
                close.ledger_rent_lamports(),
                if entry_index == 2 { 100 } else { 0 }
            );
            assert_eq!(
                close.ledger_lamport_donation(),
                if entry_index == 2 { 5 } else { 0 }
            );
            ledger_lamports = close.expected_post_ledger_lamports();
            let before = FundingLedgerV2::decode(&prestate).expect("before");
            let after = FundingLedgerV2::decode(&bytes).expect("after");
            for other in 0..3 {
                if other != entry_index {
                    assert_eq!(after.slot_bytes(other), before.slot_bytes(other));
                }
            }
            assert_eq!(
                after
                    .authenticate(manifest_id, manifest)
                    .expect("authenticate")
                    .slot(entry_index)
                    .expect("closed slot")
                    .status(),
                FundingLedgerStatusV2::Closed
            );
        }
        assert_eq!(ledger_lamports, 0);
    }

    #[test]
    fn funding_ledger_v2_close_pays_a_capped_crank_only_from_rent_it_liberated() {
        let mut manifest_storage = [0_u8; MANIFEST_HEADER_BYTES + 3 * CAPABILITY_ENTRY_BYTES];
        let manifest = ledger_manifest(&mut manifest_storage);
        let manifest_id = id(70);
        let mut bytes = [0_u8; 264];
        FundingLedgerV2::initialize(
            &mut bytes,
            manifest_id,
            manifest,
            0b111,
            DEFAULT_FUNDED_RENT_RATE,
        )
        .expect("initialize");
        for entry_index in 0..3 {
            FundingLedgerV2::activate_in_place(
                &mut bytes,
                manifest_id,
                manifest,
                entry_index,
                9 + u64::from(entry_index),
            )
            .expect("activate");
        }
        let exact_rent = 100_u64;
        let donation = 5_u64;
        let native_rent_credit = [99; 32];
        let mut ledger_lamports = exact_rent
            + FundingLedgerV2::decode(&bytes)
                .expect("activated ledger")
                .authenticate(manifest_id, manifest)
                .expect("activated manifest")
                .remaining_native_lamports_total()
                .expect("native total")
            + donation;
        // A cap far above what any close can liberate, to prove the `min` binds
        // on the liberated total rather than on the cap.
        let reward_cap = 10_000_u64;

        for entry_index in 0..3 {
            let observed = ledger_lamports;
            let custody = FundingLedgerCloseCustodyV2::native_with_crank(
                observed,
                exact_rent,
                native_rent_credit,
                reward_cap,
            )
            .expect("close custody");
            let close = FundingLedgerV2::close_slot_in_place(
                &mut bytes,
                manifest_id,
                manifest,
                entry_index,
                custody,
            )
            .expect("close slot");

            // *** Paid only on the close that actually frees the ledger. ***
            let final_close = entry_index == 2;
            assert_eq!(close.ledger_can_close(), final_close);
            if final_close {
                // Liberated is rent + surplus == 105, under the 10_000 cap, so
                // the crank takes all of it and NONE of anyone's principal.
                assert_eq!(close.crank_reward(), exact_rent + donation);
                assert!(
                    close.crank_reward()
                        <= close.ledger_rent_lamports() + close.ledger_lamport_donation(),
                    "the reward must never reach principal"
                );
            } else {
                assert_eq!(
                    close.crank_reward(),
                    0,
                    "a non-final close liberates no rent and earns nothing"
                );
            }

            // Conservation against the observed account, every close.
            close
                .validate_native_conservation(observed)
                .expect("every ledger lamport is either leaving or staying");

            // The reward is carved FROM the refund, never added TO it.
            assert_eq!(
                close.native_refund_total().expect("refund") + close.crank_reward(),
                close.remaining_native_lamports()
                    + close.vault_lamport_donation()
                    + close.ledger_rent_lamports()
                    + close.ledger_lamport_donation()
            );

            // *** NEGATIVE CONTROL: the conservation must actually discriminate.
            // One lamport more or less in the observed account has to refuse,
            // or the check is inert.
            assert_eq!(
                close.validate_native_conservation(observed + 1),
                Err(Error::UnderfundedPhysicalCustody)
            );
            assert_eq!(
                close.validate_native_conservation(observed - 1),
                Err(Error::UnderfundedPhysicalCustody)
            );

            ledger_lamports = close.expected_post_ledger_lamports();
        }
        assert_eq!(ledger_lamports, 0);
    }

    #[test]
    fn funding_ledger_v2_close_without_a_crank_is_byte_identical_to_what_it_was() {
        // The compatibility control for the optional reward: `native_only` and
        // `native_with_crank(.., 0)` must produce the same plan, so no existing
        // caller observes any change at all.
        let mut manifest_storage = [0_u8; MANIFEST_HEADER_BYTES + 3 * CAPABILITY_ENTRY_BYTES];
        let manifest = ledger_manifest(&mut manifest_storage);
        let manifest_id = id(70);
        let mut unpaid = [0_u8; 264];
        FundingLedgerV2::initialize(
            &mut unpaid,
            manifest_id,
            manifest,
            0b111,
            DEFAULT_FUNDED_RENT_RATE,
        )
        .expect("initialize");
        for entry_index in 0..3 {
            FundingLedgerV2::activate_in_place(
                &mut unpaid,
                manifest_id,
                manifest,
                entry_index,
                9 + u64::from(entry_index),
            )
            .expect("activate");
        }
        let mut zero_cap = unpaid;
        let exact_rent = 100_u64;
        let ledger_lamports = exact_rent
            + FundingLedgerV2::decode(&unpaid)
                .expect("ledger")
                .authenticate(manifest_id, manifest)
                .expect("manifest")
                .remaining_native_lamports_total()
                .expect("native total")
            + 5;

        for entry_index in 0..3 {
            let a = FundingLedgerV2::close_slot_in_place(
                &mut unpaid,
                manifest_id,
                manifest,
                entry_index,
                FundingLedgerCloseCustodyV2::native_only(ledger_lamports, exact_rent, [99; 32])
                    .expect("custody"),
            )
            .expect("close");
            let b = FundingLedgerV2::close_slot_in_place(
                &mut zero_cap,
                manifest_id,
                manifest,
                entry_index,
                FundingLedgerCloseCustodyV2::native_with_crank(
                    ledger_lamports,
                    exact_rent,
                    [99; 32],
                    0,
                )
                .expect("custody"),
            )
            .expect("close");
            assert_eq!(a, b, "a zero cap must be exactly the unpaid plan");
            assert_eq!(a.crank_reward(), 0);
            assert_eq!(
                a.native_refund_total().expect("refund"),
                a.remaining_native_lamports()
                    + a.vault_lamport_donation()
                    + a.ledger_rent_lamports()
                    + a.ledger_lamport_donation(),
                "with no crank the RentCredit is still owed every lamport"
            );
        }
        assert_eq!(unpaid, zero_cap, "and the ledger bytes must match too");
    }

    #[test]
    fn funding_ledger_v2_realm_close_requires_its_exact_row_vault() {
        let entry = CapabilityEntryV1::new(
            id(1),
            id(11),
            id(21),
            id(31),
            id(41),
            id(51),
            ActivationPolicy::RequiredAtFounding,
            0,
            0,
            [0; MAX_DEPENDENCIES_PER_CAPABILITY],
            quote(),
        )
        .expect("realm entry");
        let mut manifest_storage = [0_u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
        let manifest = CapabilityManifestV1::encode_into(&[entry], &mut manifest_storage)
            .expect("realm manifest");
        let manifest_id = id(70);
        let mut bytes = [0_u8; 120];
        FundingLedgerV2::initialize(
            &mut bytes,
            manifest_id,
            manifest,
            1,
            DEFAULT_FUNDED_RENT_RATE,
        )
        .expect("initialize");
        FundingLedgerV2::activate_in_place(&mut bytes, manifest_id, manifest, 0, 9)
            .expect("activate");
        let authenticated = FundingLedgerV2::decode(&bytes)
            .expect("ledger")
            .authenticate(manifest_id, manifest)
            .expect("manifest");
        let remaining_native = authenticated
            .remaining_native_lamports_total()
            .expect("native total");
        let realm_remaining = authenticated
            .slot(0)
            .expect("realm row")
            .remaining()
            .realm_collateral_total();
        let ledger_lamports = 100 + remaining_native + 5;
        let missing_prestate = bytes;
        assert_eq!(
            FundingLedgerV2::close_slot_in_place(
                &mut bytes,
                manifest_id,
                manifest,
                0,
                FundingLedgerCloseCustodyV2::native_only(ledger_lamports, 100, [99; 32])
                    .expect("native observation"),
            ),
            Err(Error::MissingRealmCollateralVault)
        );
        assert_eq!(bytes, missing_prestate);

        let realm = realm_custody(realm_remaining + 7, ledger_lamports)
            .realm_collateral()
            .expect("realm observation");
        let close = FundingLedgerV2::close_slot_in_place(
            &mut bytes,
            manifest_id,
            manifest,
            0,
            FundingLedgerCloseCustodyV2::with_realm_collateral(
                ledger_lamports,
                100,
                [99; 32],
                realm,
            )
            .expect("realm custody"),
        )
        .expect("realm close");
        assert_eq!(close.realm_token_beneficiary(), Some([44; 32]));
        assert_eq!(close.remaining_realm_collateral(), realm_remaining);
        assert_eq!(close.realm_collateral_donation(), 7);
        assert_eq!(close.vault_rent_lamports(), 150);
        assert_eq!(close.vault_lamport_donation(), 50);
        assert_eq!(close.ledger_rent_lamports(), 100);
        assert_eq!(close.ledger_lamport_donation(), 5);
        assert_eq!(close.expected_post_ledger_lamports(), 0);
    }

    #[test]
    fn funding_ledger_v2_entry_authorities_do_not_collide_for_equal_mints() {
        assert!(
            CAPABILITY_FUNDING_LEDGER_PDA_DOMAIN_V2.len()
                <= crate::capability_manifest::SVM_MAX_PDA_SEED_BYTES
        );
        assert!(
            CAPABILITY_FUNDING_LEDGER_AUTHORITY_PDA_DOMAIN_V2.len()
                <= crate::capability_manifest::SVM_MAX_PDA_SEED_BYTES
        );
        assert!(
            CAPABILITY_FUNDING_LEDGER_VAULT_PDA_DOMAIN_V2.len()
                <= crate::capability_manifest::SVM_MAX_PDA_SEED_BYTES
        );
        let first =
            CapabilityFundingLedgerAuthorityDerivationV2::new([7; 32], 0).expect("first authority");
        let second = CapabilityFundingLedgerAuthorityDerivationV2::new([7; 32], 1)
            .expect("second authority");
        assert_ne!(first.seed_components()[2], second.seed_components()[2]);
        assert_ne!(
            CAPABILITY_FUNDING_LEDGER_PDA_DOMAIN_V2,
            CAPABILITY_FUNDING_PDA_DOMAIN_V1
        );
    }
}
