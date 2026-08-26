//! Canonical permit-authorized Claims founding contract.
//!
//! One accepted request creates the unique runtime-width LiabilityBasisV2
//! aggregate, admits the founder's canonical `ProtocolPositionV2` with User
//! owner semantics, and mints the same positive quantity at every claim
//! coordinate. The enclosing Claims adapter authenticates accounts, finalized
//! records, the selected Trading caller, a one-shot Core permit, PDA
//! derivations, rent, and the already-realized projected Custody
//! source-to-Hoard effect before constructing this allocation-free value.
//! Custody owns token movement and replay; Claims owns liabilities.

/// Exact fixed founding request width.
pub const CLAIMS_FOUNDING_REQUEST_BYTES_V5: usize = 832;
/// Exact fixed founding receipt width.
pub const CLAIMS_FOUNDING_RECEIPT_BYTES_V5: usize = 1008;
/// Founding request wire magic.
pub const CLAIMS_FOUNDING_REQUEST_MAGIC_V5: [u8; 8] = *b"DCLFDR05";
/// Founding receipt wire magic.
pub const CLAIMS_FOUNDING_RECEIPT_MAGIC_V5: [u8; 8] = *b"DCLFDC05";
/// Implemented founding ABI version.
pub const CLAIMS_FOUNDING_WIRE_VERSION_V5: u16 = 5;
/// Wire value fixing the admitted ProtocolPosition owner kind to `User`.
pub const CLAIMS_FOUNDING_USER_OWNER_KIND_V5: u8 = 1;
/// Canonical LiabilityBasisV2 aggregate PDA seed domain.
pub const CLAIMS_FOUNDING_AGGREGATE_SEED_V5: &[u8] = b"dclutch:lbv2:market";
/// Domain for the ordered post-resource transcript.
///
/// The adapter hashes this domain followed by the exact post aggregate,
/// Position, and admission bytes, in that order.
pub const CLAIMS_FOUNDING_POST_RESOURCE_DIGEST_DOMAIN_V5: &[u8] =
    b"dclutch/claims/founding/post-resources/v5";

const VERSION_OFFSET: usize = 8;
const OWNER_KIND_OFFSET: usize = 10;
const HEADER_RESERVED_OFFSET: usize = 11;
const RELEASE_SET_OFFSET: usize = 16;
const MARKET_OFFSET: usize = 48;
const PRODUCT_RECORD_OFFSET: usize = 80;
const PRODUCT_INSTANCE_OFFSET: usize = 112;
const LINKED_BASIS_OFFSET: usize = 144;
const SEMANTIC_BASIS_OFFSET: usize = 176;
const FOUNDER_OFFSET: usize = 208;
const FOUNDING_INTENT_OFFSET: usize = 240;
const AGGREGATE_OFFSET: usize = 272;
const POSITION_OFFSET: usize = 304;
const ADMISSION_OFFSET: usize = 336;
const FUNDING_SOURCE_OFFSET: usize = 368;
const HOARD_OFFSET: usize = 400;
const CUSTODY_REPLAY_OFFSET: usize = 432;
const RENT_CREDIT_OFFSET: usize = 464;
const RENT_PROGRAM_OFFSET: usize = 496;
const CLAIMS_PROGRAM_OFFSET: usize = 528;
const TRADING_PROGRAM_OFFSET: usize = 560;
const CUSTODY_REQUEST_DIGEST_OFFSET: usize = 592;
const CUSTODY_RECEIPT_DIGEST_OFFSET: usize = 624;
const GENERATION_OFFSET: usize = 656;
const CLAIM_COUNT_OFFSET: usize = 664;
const BODY_RESERVED_OFFSET: usize = 668;
const QUANTITY_OFFSET: usize = 672;
const BASIS_SCALE_OFFSET: usize = 680;
const PRE_SOURCE_AMOUNT_OFFSET: usize = 688;
const POST_SOURCE_AMOUNT_OFFSET: usize = 696;
const PRE_HOARD_AMOUNT_OFFSET: usize = 704;
const POST_HOARD_AMOUNT_OFFSET: usize = 712;
const PRE_CUSTODY_REVISION_OFFSET: usize = 720;
const POST_CUSTODY_REVISION_OFFSET: usize = 728;
const AGGREGATE_RENT_OFFSET: usize = 736;
const POSITION_RENT_OFFSET: usize = 744;
const ADMISSION_RENT_OFFSET: usize = 752;
const OBSERVED_AGGREGATE_LAMPORTS_OFFSET: usize = 760;
const OBSERVED_POSITION_LAMPORTS_OFFSET: usize = 768;
const OBSERVED_ADMISSION_LAMPORTS_OFFSET: usize = 776;
const PRE_AGGREGATE_REVISION_OFFSET: usize = 784;
const POST_AGGREGATE_REVISION_OFFSET: usize = 792;
const PRE_POSITION_REVISION_OFFSET: usize = 800;
const POST_POSITION_REVISION_OFFSET: usize = 808;
const TAIL_RESERVED_OFFSET: usize = 816;

const RECEIPT_REQUEST_OFFSET: usize = 16;
const RECEIPT_REQUEST_DIGEST_OFFSET: usize =
    RECEIPT_REQUEST_OFFSET + CLAIMS_FOUNDING_REQUEST_BYTES_V5;
const RECEIPT_AGGREGATE_DIGEST_OFFSET: usize = RECEIPT_REQUEST_DIGEST_OFFSET + 32;
const RECEIPT_POSITION_DIGEST_OFFSET: usize = RECEIPT_AGGREGATE_DIGEST_OFFSET + 32;
const RECEIPT_ADMISSION_DIGEST_OFFSET: usize = RECEIPT_POSITION_DIGEST_OFFSET + 32;
const RECEIPT_POST_RESOURCE_DIGEST_OFFSET: usize = RECEIPT_ADMISSION_DIGEST_OFFSET + 32;

/// Stable hostile-decode or founding-evidence refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClaimsFoundingErrorV5 {
    /// Request or receipt bytes did not have the exact selected width.
    InvalidLength,
    /// Magic bytes selected another ABI family.
    InvalidMagic,
    /// The wire version is unsupported.
    UnsupportedVersion,
    /// Reserved bytes or the fixed User owner tag were noncanonical.
    NonCanonical,
    /// A required account identity or transcript digest was zero.
    ZeroIdentity,
    /// Two distinct physical accounts or programs aliased.
    AccountAlias,
    /// Runtime claim width was zero.
    InvalidClaimCount,
    /// Complete-set quantity or LiabilityBasisV2 scale was zero.
    InvalidQuantity,
    /// Exact source debit, Hoard credit, or principal multiplication disagreed.
    InvalidCollateralTransfer,
    /// A required prepaid rent principal was zero.
    InvalidRent,
    /// Founding revisions were not the exact vacant-zero to live-one shape.
    InvalidRevision,
    /// Receipt evidence did not bind the exact accepted request.
    ReceiptMismatch,
}

/// Result alias for the founding ABI.
pub type Result<T> = core::result::Result<T, ClaimsFoundingErrorV5>;

/// Construction input for one atomic Claims founding request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimsFoundingRequestInputV5 {
    /// Immutable current execution release set.
    pub release_set: [u8; 32],
    /// Canonical logical Core Market identity.
    pub market: [u8; 32],
    /// Digest of the exact finalized Product Runtime V2 record.
    pub product_record_digest: [u8; 32],
    /// Exact Product instance content identity authenticated by that record.
    pub product_instance_id: [u8; 32],
    /// Digest of the exact finalized linked LiabilityBasisV2 record.
    pub linked_basis_record_digest: [u8; 32],
    /// Semantic LiabilityBasisV2 identity authenticated by the linked record.
    pub semantic_basis_id: [u8; 32],
    /// User identity admitted as the founding Position owner.
    pub founder: [u8; 32],
    /// SHA-256 of the canonical Core-owned `FoundingIntentV5` projection.
    pub founding_intent_digest: [u8; 32],
    /// Canonical Claims-owned LiabilityBasisV2 aggregate account.
    pub aggregate: [u8; 32],
    /// Canonical founder `ProtocolPositionV2` account.
    pub position: [u8; 32],
    /// Canonical Claims-owned `ProtocolPositionV2` admission account.
    pub admission: [u8; 32],
    /// Custody-owned Market Hoard account observed by the atomic adapter.
    pub hoard: [u8; 32],
    /// Exact prepaid RentCredit account consumed by founding.
    pub rent_credit: [u8; 32],
    /// Registry-selected Rent program owning that credit.
    pub rent_program: [u8; 32],
    /// Registry-selected Claims program that alone writes Claims state.
    pub claims_program: [u8; 32],
    /// Registry-selected Trading program signing the exact request.
    pub trading_program: [u8; 32],
    /// Generic Custody funding source debited by the accepted parent effect.
    pub funding_source: [u8; 32],
    /// Custody replay account advanced by the accepted parent effect.
    pub custody_replay: [u8; 32],
    /// SHA-256 of the exact accepted Custody request bytes.
    pub custody_request_digest: [u8; 32],
    /// SHA-256 of the exact producer-authenticated Custody receipt bytes.
    pub custody_receipt_digest: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Runtime Product claim count and aggregate/Position vector width.
    pub claim_count: u32,
    /// Equal complete-set quantity minted at every claim coordinate.
    pub quantity: u64,
    /// Positive LiabilityBasisV2 Hoard-principal units per complete set.
    pub basis_scale: u64,
    /// Exact funding-source token amount before the Custody transfer.
    pub pre_source_amount: u64,
    /// Exact funding-source token amount after the Custody transfer.
    pub post_source_amount: u64,
    /// Exact Hoard token amount before the Custody transfer.
    pub pre_hoard_amount: u64,
    /// Exact Hoard token amount after the Custody transfer.
    pub post_hoard_amount: u64,
    /// Exact Custody replay revision before the transfer.
    pub pre_custody_revision: u64,
    /// Exact Custody replay revision after the transfer.
    pub post_custody_revision: u64,
    /// Exact RentCredit principal consumed for the aggregate account.
    pub aggregate_rent_principal: u64,
    /// Exact RentCredit principal consumed for the Position account.
    pub position_rent_principal: u64,
    /// Exact RentCredit principal consumed for the admission account.
    pub admission_rent_principal: u64,
    /// Exact dust-tolerant aggregate lamports observed before allocation.
    pub observed_aggregate_lamports: u64,
    /// Exact dust-tolerant Position lamports observed before allocation.
    pub observed_position_lamports: u64,
    /// Exact dust-tolerant admission lamports observed before allocation.
    pub observed_admission_lamports: u64,
    /// Vacant aggregate founding revision, canonically zero.
    pub pre_aggregate_revision: u64,
    /// Live aggregate revision after the founding mint, canonically one.
    pub post_aggregate_revision: u64,
    /// Vacant Position founding revision, canonically zero.
    pub pre_position_revision: u64,
    /// Live Position revision after the founding mint, canonically one.
    pub post_position_revision: u64,
}

/// Canonical aggregate PDA coordinates for a logical Core Market.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimsFoundingAggregateSeedsV5 {
    market: [u8; 32],
}

impl ClaimsFoundingAggregateSeedsV5 {
    /// Construct the unique LiabilityBasisV2 aggregate coordinates.
    pub fn new(market: [u8; 32]) -> Result<Self> {
        require_nonzero(market)?;
        Ok(Self { market })
    }

    /// Borrow the exact ordered PDA seed slices, excluding the bump.
    pub fn as_slices(&self) -> [&[u8]; 2] {
        [CLAIMS_FOUNDING_AGGREGATE_SEED_V5, &self.market]
    }
}

/// One fully validated atomic Claims founding request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimsFoundingRequestV5(ClaimsFoundingRequestInputV5);

impl ClaimsFoundingRequestV5 {
    /// Construct and fully validate one request.
    pub fn new(input: ClaimsFoundingRequestInputV5) -> Result<Self> {
        for identity in [
            input.release_set,
            input.market,
            input.product_record_digest,
            input.product_instance_id,
            input.linked_basis_record_digest,
            input.semantic_basis_id,
            input.founder,
            input.founding_intent_digest,
            input.aggregate,
            input.position,
            input.admission,
            input.funding_source,
            input.hoard,
            input.custody_replay,
            input.rent_credit,
            input.rent_program,
            input.claims_program,
            input.trading_program,
            input.custody_request_digest,
            input.custody_receipt_digest,
        ] {
            require_nonzero(identity)?;
        }
        require_distinct_accounts(&[
            input.market,
            input.founder,
            input.aggregate,
            input.position,
            input.admission,
            input.funding_source,
            input.hoard,
            input.custody_replay,
            input.rent_credit,
            input.rent_program,
            input.claims_program,
            input.trading_program,
        ])?;
        if input.claim_count == 0 {
            return Err(ClaimsFoundingErrorV5::InvalidClaimCount);
        }
        if input.quantity == 0 || input.basis_scale == 0 {
            return Err(ClaimsFoundingErrorV5::InvalidQuantity);
        }
        let collateral = input
            .quantity
            .checked_mul(input.basis_scale)
            .ok_or(ClaimsFoundingErrorV5::InvalidCollateralTransfer)?;
        if collateral == 0
            || input.post_source_amount != 0
            || input.pre_source_amount != collateral
            || input
                .pre_source_amount
                .checked_sub(input.post_source_amount)
                != Some(collateral)
            || input.post_hoard_amount.checked_sub(input.pre_hoard_amount) != Some(collateral)
        {
            return Err(ClaimsFoundingErrorV5::InvalidCollateralTransfer);
        }
        require_next_revision(input.pre_custody_revision, input.post_custody_revision)?;
        if input.aggregate_rent_principal == 0
            || input.position_rent_principal == 0
            || input.admission_rent_principal == 0
            || input.observed_aggregate_lamports < input.aggregate_rent_principal
            || input.observed_position_lamports < input.position_rent_principal
            || input.observed_admission_lamports < input.admission_rent_principal
        {
            return Err(ClaimsFoundingErrorV5::InvalidRent);
        }
        if input.pre_aggregate_revision != 0
            || input.post_aggregate_revision != 1
            || input.pre_position_revision != 0
            || input.post_position_revision != 1
        {
            return Err(ClaimsFoundingErrorV5::InvalidRevision);
        }
        Ok(Self(input))
    }

    /// Decode one exact request and refuse every noncanonical byte.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != CLAIMS_FOUNDING_REQUEST_BYTES_V5 {
            return Err(ClaimsFoundingErrorV5::InvalidLength);
        }
        if array_at::<8>(input, 0)? != CLAIMS_FOUNDING_REQUEST_MAGIC_V5 {
            return Err(ClaimsFoundingErrorV5::InvalidMagic);
        }
        if u16_at(input, VERSION_OFFSET)? != CLAIMS_FOUNDING_WIRE_VERSION_V5 {
            return Err(ClaimsFoundingErrorV5::UnsupportedVersion);
        }
        if byte_at(input, OWNER_KIND_OFFSET)? != CLAIMS_FOUNDING_USER_OWNER_KIND_V5 {
            return Err(ClaimsFoundingErrorV5::NonCanonical);
        }
        require_zero(input, HEADER_RESERVED_OFFSET, 5)?;
        require_zero(input, BODY_RESERVED_OFFSET, 4)?;
        require_zero(input, TAIL_RESERVED_OFFSET, 16)?;
        Self::new(ClaimsFoundingRequestInputV5 {
            release_set: array_at(input, RELEASE_SET_OFFSET)?,
            market: array_at(input, MARKET_OFFSET)?,
            product_record_digest: array_at(input, PRODUCT_RECORD_OFFSET)?,
            product_instance_id: array_at(input, PRODUCT_INSTANCE_OFFSET)?,
            linked_basis_record_digest: array_at(input, LINKED_BASIS_OFFSET)?,
            semantic_basis_id: array_at(input, SEMANTIC_BASIS_OFFSET)?,
            founder: array_at(input, FOUNDER_OFFSET)?,
            founding_intent_digest: array_at(input, FOUNDING_INTENT_OFFSET)?,
            aggregate: array_at(input, AGGREGATE_OFFSET)?,
            position: array_at(input, POSITION_OFFSET)?,
            admission: array_at(input, ADMISSION_OFFSET)?,
            funding_source: array_at(input, FUNDING_SOURCE_OFFSET)?,
            hoard: array_at(input, HOARD_OFFSET)?,
            custody_replay: array_at(input, CUSTODY_REPLAY_OFFSET)?,
            rent_credit: array_at(input, RENT_CREDIT_OFFSET)?,
            rent_program: array_at(input, RENT_PROGRAM_OFFSET)?,
            claims_program: array_at(input, CLAIMS_PROGRAM_OFFSET)?,
            trading_program: array_at(input, TRADING_PROGRAM_OFFSET)?,
            custody_request_digest: array_at(input, CUSTODY_REQUEST_DIGEST_OFFSET)?,
            custody_receipt_digest: array_at(input, CUSTODY_RECEIPT_DIGEST_OFFSET)?,
            generation: u64_at(input, GENERATION_OFFSET)?,
            claim_count: u32_at(input, CLAIM_COUNT_OFFSET)?,
            quantity: u64_at(input, QUANTITY_OFFSET)?,
            basis_scale: u64_at(input, BASIS_SCALE_OFFSET)?,
            pre_source_amount: u64_at(input, PRE_SOURCE_AMOUNT_OFFSET)?,
            post_source_amount: u64_at(input, POST_SOURCE_AMOUNT_OFFSET)?,
            pre_hoard_amount: u64_at(input, PRE_HOARD_AMOUNT_OFFSET)?,
            post_hoard_amount: u64_at(input, POST_HOARD_AMOUNT_OFFSET)?,
            pre_custody_revision: u64_at(input, PRE_CUSTODY_REVISION_OFFSET)?,
            post_custody_revision: u64_at(input, POST_CUSTODY_REVISION_OFFSET)?,
            aggregate_rent_principal: u64_at(input, AGGREGATE_RENT_OFFSET)?,
            position_rent_principal: u64_at(input, POSITION_RENT_OFFSET)?,
            admission_rent_principal: u64_at(input, ADMISSION_RENT_OFFSET)?,
            observed_aggregate_lamports: u64_at(input, OBSERVED_AGGREGATE_LAMPORTS_OFFSET)?,
            observed_position_lamports: u64_at(input, OBSERVED_POSITION_LAMPORTS_OFFSET)?,
            observed_admission_lamports: u64_at(input, OBSERVED_ADMISSION_LAMPORTS_OFFSET)?,
            pre_aggregate_revision: u64_at(input, PRE_AGGREGATE_REVISION_OFFSET)?,
            post_aggregate_revision: u64_at(input, POST_AGGREGATE_REVISION_OFFSET)?,
            pre_position_revision: u64_at(input, PRE_POSITION_REVISION_OFFSET)?,
            post_position_revision: u64_at(input, POST_POSITION_REVISION_OFFSET)?,
        })
    }

    /// Encode the exact canonical request bytes.
    pub fn to_bytes(&self) -> [u8; CLAIMS_FOUNDING_REQUEST_BYTES_V5] {
        let mut output = [0_u8; CLAIMS_FOUNDING_REQUEST_BYTES_V5];
        put_infallible(&mut output, 0, &CLAIMS_FOUNDING_REQUEST_MAGIC_V5);
        put_infallible(
            &mut output,
            VERSION_OFFSET,
            &CLAIMS_FOUNDING_WIRE_VERSION_V5.to_le_bytes(),
        );
        put_infallible(
            &mut output,
            OWNER_KIND_OFFSET,
            &[CLAIMS_FOUNDING_USER_OWNER_KIND_V5],
        );
        for (offset, identity) in [
            (RELEASE_SET_OFFSET, self.0.release_set),
            (MARKET_OFFSET, self.0.market),
            (PRODUCT_RECORD_OFFSET, self.0.product_record_digest),
            (PRODUCT_INSTANCE_OFFSET, self.0.product_instance_id),
            (LINKED_BASIS_OFFSET, self.0.linked_basis_record_digest),
            (SEMANTIC_BASIS_OFFSET, self.0.semantic_basis_id),
            (FOUNDER_OFFSET, self.0.founder),
            (FOUNDING_INTENT_OFFSET, self.0.founding_intent_digest),
            (AGGREGATE_OFFSET, self.0.aggregate),
            (POSITION_OFFSET, self.0.position),
            (ADMISSION_OFFSET, self.0.admission),
            (FUNDING_SOURCE_OFFSET, self.0.funding_source),
            (HOARD_OFFSET, self.0.hoard),
            (CUSTODY_REPLAY_OFFSET, self.0.custody_replay),
            (RENT_CREDIT_OFFSET, self.0.rent_credit),
            (RENT_PROGRAM_OFFSET, self.0.rent_program),
            (CLAIMS_PROGRAM_OFFSET, self.0.claims_program),
            (TRADING_PROGRAM_OFFSET, self.0.trading_program),
            (CUSTODY_REQUEST_DIGEST_OFFSET, self.0.custody_request_digest),
            (CUSTODY_RECEIPT_DIGEST_OFFSET, self.0.custody_receipt_digest),
        ] {
            put_infallible(&mut output, offset, &identity);
        }
        put_infallible(
            &mut output,
            GENERATION_OFFSET,
            &self.0.generation.to_le_bytes(),
        );
        put_infallible(
            &mut output,
            CLAIM_COUNT_OFFSET,
            &self.0.claim_count.to_le_bytes(),
        );
        for (offset, value) in [
            (QUANTITY_OFFSET, self.0.quantity),
            (BASIS_SCALE_OFFSET, self.0.basis_scale),
            (PRE_SOURCE_AMOUNT_OFFSET, self.0.pre_source_amount),
            (POST_SOURCE_AMOUNT_OFFSET, self.0.post_source_amount),
            (PRE_HOARD_AMOUNT_OFFSET, self.0.pre_hoard_amount),
            (POST_HOARD_AMOUNT_OFFSET, self.0.post_hoard_amount),
            (PRE_CUSTODY_REVISION_OFFSET, self.0.pre_custody_revision),
            (POST_CUSTODY_REVISION_OFFSET, self.0.post_custody_revision),
            (AGGREGATE_RENT_OFFSET, self.0.aggregate_rent_principal),
            (POSITION_RENT_OFFSET, self.0.position_rent_principal),
            (ADMISSION_RENT_OFFSET, self.0.admission_rent_principal),
            (
                OBSERVED_AGGREGATE_LAMPORTS_OFFSET,
                self.0.observed_aggregate_lamports,
            ),
            (
                OBSERVED_POSITION_LAMPORTS_OFFSET,
                self.0.observed_position_lamports,
            ),
            (
                OBSERVED_ADMISSION_LAMPORTS_OFFSET,
                self.0.observed_admission_lamports,
            ),
            (PRE_AGGREGATE_REVISION_OFFSET, self.0.pre_aggregate_revision),
            (
                POST_AGGREGATE_REVISION_OFFSET,
                self.0.post_aggregate_revision,
            ),
            (PRE_POSITION_REVISION_OFFSET, self.0.pre_position_revision),
            (POST_POSITION_REVISION_OFFSET, self.0.post_position_revision),
        ] {
            put_infallible(&mut output, offset, &value.to_le_bytes());
        }
        output
    }

    /// Return the complete validated input value.
    pub const fn input(&self) -> ClaimsFoundingRequestInputV5 {
        self.0
    }

    /// Return the exact collateral moved from source into Hoard.
    pub const fn collateral_transferred(&self) -> u64 {
        self.0.quantity * self.0.basis_scale
    }

    /// Return the immutable current release set.
    pub const fn release_set(&self) -> [u8; 32] {
        self.0.release_set
    }
    /// Return the logical Core Market identity.
    pub const fn market(&self) -> [u8; 32] {
        self.0.market
    }
    /// Return the Product Runtime V2 record digest.
    pub const fn product_record_digest(&self) -> [u8; 32] {
        self.0.product_record_digest
    }
    /// Return the Product instance identity.
    pub const fn product_instance_id(&self) -> [u8; 32] {
        self.0.product_instance_id
    }
    /// Return the linked-basis record digest.
    pub const fn linked_basis_record_digest(&self) -> [u8; 32] {
        self.0.linked_basis_record_digest
    }
    /// Return the semantic basis identity.
    pub const fn semantic_basis_id(&self) -> [u8; 32] {
        self.0.semantic_basis_id
    }
    /// Return the founding User owner identity.
    pub const fn founder(&self) -> [u8; 32] {
        self.0.founder
    }
    /// Return the canonical founding-intent digest.
    pub const fn founding_intent_digest(&self) -> [u8; 32] {
        self.0.founding_intent_digest
    }
    /// Return the canonical aggregate account.
    pub const fn aggregate(&self) -> [u8; 32] {
        self.0.aggregate
    }
    /// Return the canonical founder Position account.
    pub const fn position(&self) -> [u8; 32] {
        self.0.position
    }
    /// Return the canonical Position admission account.
    pub const fn admission(&self) -> [u8; 32] {
        self.0.admission
    }
    /// Return the generic Custody funding source account.
    pub const fn funding_source(&self) -> [u8; 32] {
        self.0.funding_source
    }
    /// Return the observed Custody-owned Hoard account.
    pub const fn hoard(&self) -> [u8; 32] {
        self.0.hoard
    }
    /// Return the exact Custody replay account advanced by funding.
    pub const fn custody_replay(&self) -> [u8; 32] {
        self.0.custody_replay
    }
    /// Return the prepaid RentCredit account.
    pub const fn rent_credit(&self) -> [u8; 32] {
        self.0.rent_credit
    }
    /// Return the selected Rent program.
    pub const fn rent_program(&self) -> [u8; 32] {
        self.0.rent_program
    }
    /// Return the sole Claims writer program.
    pub const fn claims_program(&self) -> [u8; 32] {
        self.0.claims_program
    }
    /// Return the selected Trading caller program.
    pub const fn trading_program(&self) -> [u8; 32] {
        self.0.trading_program
    }
    /// Return the digest of the exact accepted Custody request.
    pub const fn custody_request_digest(&self) -> [u8; 32] {
        self.0.custody_request_digest
    }
    /// Return the digest of the exact producer-authenticated Custody receipt.
    pub const fn custody_receipt_digest(&self) -> [u8; 32] {
        self.0.custody_receipt_digest
    }
    /// Return the immutable Market generation.
    pub const fn generation(&self) -> u64 {
        self.0.generation
    }
    /// Return the runtime claim-vector width.
    pub const fn claim_count(&self) -> u32 {
        self.0.claim_count
    }
    /// Return the equal quantity minted at every claim coordinate.
    pub const fn quantity(&self) -> u64 {
        self.0.quantity
    }
    /// Return the positive Hoard-principal scale per complete set.
    pub const fn basis_scale(&self) -> u64 {
        self.0.basis_scale
    }

    /// Return the exact funding-source amount before transfer.
    pub const fn pre_source_amount(&self) -> u64 {
        self.0.pre_source_amount
    }
    /// Return the exact funding-source amount after transfer.
    pub const fn post_source_amount(&self) -> u64 {
        self.0.post_source_amount
    }
    /// Return the exact Hoard amount before transfer.
    pub const fn pre_hoard_amount(&self) -> u64 {
        self.0.pre_hoard_amount
    }
    /// Return the exact Hoard amount after transfer.
    pub const fn post_hoard_amount(&self) -> u64 {
        self.0.post_hoard_amount
    }
    /// Return the exact Custody replay revision before transfer.
    pub const fn pre_custody_revision(&self) -> u64 {
        self.0.pre_custody_revision
    }
    /// Return the exact Custody replay revision after transfer.
    pub const fn post_custody_revision(&self) -> u64 {
        self.0.post_custody_revision
    }

    /// Return the exact aggregate RentCredit consumption.
    pub const fn aggregate_rent_principal(&self) -> u64 {
        self.0.aggregate_rent_principal
    }

    /// Return the exact Position RentCredit consumption.
    pub const fn position_rent_principal(&self) -> u64 {
        self.0.position_rent_principal
    }

    /// Return the exact admission RentCredit consumption.
    pub const fn admission_rent_principal(&self) -> u64 {
        self.0.admission_rent_principal
    }
    /// Return the observed dust-tolerant aggregate lamports.
    pub const fn observed_aggregate_lamports(&self) -> u64 {
        self.0.observed_aggregate_lamports
    }
    /// Return the observed dust-tolerant Position lamports.
    pub const fn observed_position_lamports(&self) -> u64 {
        self.0.observed_position_lamports
    }
    /// Return the observed dust-tolerant admission lamports.
    pub const fn observed_admission_lamports(&self) -> u64 {
        self.0.observed_admission_lamports
    }

    /// Return the vacant aggregate pre-revision.
    pub const fn pre_aggregate_revision(&self) -> u64 {
        self.0.pre_aggregate_revision
    }

    /// Return the live aggregate post-revision.
    pub const fn post_aggregate_revision(&self) -> u64 {
        self.0.post_aggregate_revision
    }

    /// Return the vacant Position pre-revision.
    pub const fn pre_position_revision(&self) -> u64 {
        self.0.pre_position_revision
    }

    /// Return the live Position post-revision.
    pub const fn post_position_revision(&self) -> u64 {
        self.0.post_position_revision
    }
}

/// Typed success receipt embedding the exact accepted founding request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimsFoundingReceiptV5 {
    request: ClaimsFoundingRequestV5,
    request_digest: [u8; 32],
    aggregate_digest: [u8; 32],
    position_digest: [u8; 32],
    admission_digest: [u8; 32],
    post_resource_digest: [u8; 32],
}

impl ClaimsFoundingReceiptV5 {
    /// Construct a receipt after the atomic Claims and Hoard postconditions hold.
    pub fn new(
        request: ClaimsFoundingRequestV5,
        request_digest: [u8; 32],
        aggregate_digest: [u8; 32],
        position_digest: [u8; 32],
        admission_digest: [u8; 32],
        post_resource_digest: [u8; 32],
    ) -> Result<Self> {
        for digest in [
            request_digest,
            aggregate_digest,
            position_digest,
            admission_digest,
            post_resource_digest,
        ] {
            require_nonzero(digest)?;
        }
        Ok(Self {
            request,
            request_digest,
            aggregate_digest,
            position_digest,
            admission_digest,
            post_resource_digest,
        })
    }

    /// Decode one exact receipt and its embedded canonical request.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != CLAIMS_FOUNDING_RECEIPT_BYTES_V5 {
            return Err(ClaimsFoundingErrorV5::InvalidLength);
        }
        if array_at::<8>(input, 0)? != CLAIMS_FOUNDING_RECEIPT_MAGIC_V5 {
            return Err(ClaimsFoundingErrorV5::InvalidMagic);
        }
        if u16_at(input, VERSION_OFFSET)? != CLAIMS_FOUNDING_WIRE_VERSION_V5 {
            return Err(ClaimsFoundingErrorV5::UnsupportedVersion);
        }
        require_zero(input, 10, 6)?;
        Self::new(
            ClaimsFoundingRequestV5::decode(subslice(
                input,
                RECEIPT_REQUEST_OFFSET,
                CLAIMS_FOUNDING_REQUEST_BYTES_V5,
            )?)?,
            array_at(input, RECEIPT_REQUEST_DIGEST_OFFSET)?,
            array_at(input, RECEIPT_AGGREGATE_DIGEST_OFFSET)?,
            array_at(input, RECEIPT_POSITION_DIGEST_OFFSET)?,
            array_at(input, RECEIPT_ADMISSION_DIGEST_OFFSET)?,
            array_at(input, RECEIPT_POST_RESOURCE_DIGEST_OFFSET)?,
        )
    }

    /// Encode the exact canonical receipt bytes.
    pub fn to_bytes(&self) -> [u8; CLAIMS_FOUNDING_RECEIPT_BYTES_V5] {
        let mut output = [0_u8; CLAIMS_FOUNDING_RECEIPT_BYTES_V5];
        put_infallible(&mut output, 0, &CLAIMS_FOUNDING_RECEIPT_MAGIC_V5);
        put_infallible(
            &mut output,
            VERSION_OFFSET,
            &CLAIMS_FOUNDING_WIRE_VERSION_V5.to_le_bytes(),
        );
        put_infallible(
            &mut output,
            RECEIPT_REQUEST_OFFSET,
            &self.request.to_bytes(),
        );
        for (offset, digest) in [
            (RECEIPT_REQUEST_DIGEST_OFFSET, self.request_digest),
            (RECEIPT_AGGREGATE_DIGEST_OFFSET, self.aggregate_digest),
            (RECEIPT_POSITION_DIGEST_OFFSET, self.position_digest),
            (RECEIPT_ADMISSION_DIGEST_OFFSET, self.admission_digest),
            (
                RECEIPT_POST_RESOURCE_DIGEST_OFFSET,
                self.post_resource_digest,
            ),
        ] {
            put_infallible(&mut output, offset, &digest);
        }
        output
    }

    /// Require this receipt to bind the exact accepted request and request digest.
    pub fn verify_for(
        &self,
        request: &ClaimsFoundingRequestV5,
        request_digest: [u8; 32],
    ) -> Result<()> {
        if &self.request != request || self.request_digest != request_digest {
            return Err(ClaimsFoundingErrorV5::ReceiptMismatch);
        }
        Ok(())
    }

    /// Return the exact embedded request.
    pub const fn request(&self) -> ClaimsFoundingRequestV5 {
        self.request
    }
    /// Return the digest of exact request bytes.
    pub const fn request_digest(&self) -> [u8; 32] {
        self.request_digest
    }
    /// Return the digest of exact post aggregate bytes.
    pub const fn aggregate_digest(&self) -> [u8; 32] {
        self.aggregate_digest
    }
    /// Return the digest of exact post Position bytes.
    pub const fn position_digest(&self) -> [u8; 32] {
        self.position_digest
    }
    /// Return the digest of exact post admission bytes.
    pub const fn admission_digest(&self) -> [u8; 32] {
        self.admission_digest
    }
    /// Return the ordered aggregate+Position+admission post-resource digest.
    pub const fn post_resource_digest(&self) -> [u8; 32] {
        self.post_resource_digest
    }
}

fn require_next_revision(before: u64, after: u64) -> Result<()> {
    if before == u64::MAX || before.checked_add(1) != Some(after) {
        Err(ClaimsFoundingErrorV5::InvalidRevision)
    } else {
        Ok(())
    }
}

fn require_distinct_accounts(accounts: &[[u8; 32]]) -> Result<()> {
    let mut outer = 0_usize;
    while outer < accounts.len() {
        let current = accounts
            .get(outer)
            .ok_or(ClaimsFoundingErrorV5::InvalidLength)?;
        let mut inner = outer
            .checked_add(1)
            .ok_or(ClaimsFoundingErrorV5::InvalidLength)?;
        while inner < accounts.len() {
            if current
                == accounts
                    .get(inner)
                    .ok_or(ClaimsFoundingErrorV5::InvalidLength)?
            {
                return Err(ClaimsFoundingErrorV5::AccountAlias);
            }
            inner = inner
                .checked_add(1)
                .ok_or(ClaimsFoundingErrorV5::InvalidLength)?;
        }
        outer = outer
            .checked_add(1)
            .ok_or(ClaimsFoundingErrorV5::InvalidLength)?;
    }
    Ok(())
}

fn require_nonzero(value: [u8; 32]) -> Result<()> {
    if value.iter().all(|byte| *byte == 0) {
        Err(ClaimsFoundingErrorV5::ZeroIdentity)
    } else {
        Ok(())
    }
}

fn require_zero(input: &[u8], offset: usize, width: usize) -> Result<()> {
    if subslice(input, offset, width)?
        .iter()
        .all(|byte| *byte == 0)
    {
        Ok(())
    } else {
        Err(ClaimsFoundingErrorV5::NonCanonical)
    }
}

fn byte_at(input: &[u8], offset: usize) -> Result<u8> {
    input
        .get(offset)
        .copied()
        .ok_or(ClaimsFoundingErrorV5::InvalidLength)
}

fn u16_at(input: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(array_at(input, offset)?))
}

fn u32_at(input: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(array_at(input, offset)?))
}

fn u64_at(input: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(array_at(input, offset)?))
}

fn array_at<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N]> {
    subslice(input, offset, N)?
        .try_into()
        .map_err(|_| ClaimsFoundingErrorV5::InvalidLength)
}

fn subslice(input: &[u8], offset: usize, width: usize) -> Result<&[u8]> {
    input
        .get(
            offset
                ..offset
                    .checked_add(width)
                    .ok_or(ClaimsFoundingErrorV5::InvalidLength)?,
        )
        .ok_or(ClaimsFoundingErrorV5::InvalidLength)
}

fn put_infallible(output: &mut [u8], offset: usize, value: &[u8]) {
    if let Some(target) = output.get_mut(offset..offset.saturating_add(value.len())) {
        target.copy_from_slice(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn input() -> ClaimsFoundingRequestInputV5 {
        ClaimsFoundingRequestInputV5 {
            release_set: id(1),
            market: id(2),
            product_record_digest: id(3),
            product_instance_id: id(4),
            linked_basis_record_digest: id(5),
            semantic_basis_id: id(6),
            founder: id(7),
            founding_intent_digest: id(8),
            aggregate: id(9),
            position: id(10),
            admission: id(11),
            funding_source: id(12),
            hoard: id(13),
            custody_replay: id(14),
            rent_credit: id(15),
            rent_program: id(16),
            claims_program: id(17),
            trading_program: id(18),
            custody_request_digest: id(19),
            custody_receipt_digest: id(20),
            generation: 21,
            claim_count: 5,
            quantity: 7,
            basis_scale: 11,
            pre_source_amount: 77,
            post_source_amount: 0,
            pre_hoard_amount: 23,
            post_hoard_amount: 100,
            pre_custody_revision: 24,
            post_custody_revision: 25,
            aggregate_rent_principal: 30,
            position_rent_principal: 31,
            admission_rent_principal: 32,
            observed_aggregate_lamports: 33,
            observed_position_lamports: 34,
            observed_admission_lamports: 35,
            pre_aggregate_revision: 0,
            post_aggregate_revision: 1,
            pre_position_revision: 0,
            post_position_revision: 1,
        }
    }

    fn request() -> ClaimsFoundingRequestV5 {
        ClaimsFoundingRequestV5::new(input()).expect("valid founding")
    }

    fn mutate(bytes: &mut [u8], offset: usize, value: u8) {
        *bytes.get_mut(offset).expect("test offset") = value;
    }

    #[test]
    fn exact_request_and_receipt_roundtrip_bind_atomic_founding() {
        let request = request();
        assert_eq!(
            ClaimsFoundingRequestV5::decode(&request.to_bytes()),
            Ok(request)
        );
        assert_eq!(request.release_set(), id(1));
        assert_eq!(request.market(), id(2));
        assert_eq!(request.product_record_digest(), id(3));
        assert_eq!(request.product_instance_id(), id(4));
        assert_eq!(request.linked_basis_record_digest(), id(5));
        assert_eq!(request.semantic_basis_id(), id(6));
        assert_eq!(request.founder(), id(7));
        assert_eq!(request.founding_intent_digest(), id(8));
        assert_eq!(
            (request.aggregate(), request.position(), request.admission()),
            (id(9), id(10), id(11))
        );
        assert_eq!(
            (
                request.funding_source(),
                request.hoard(),
                request.custody_replay(),
                request.rent_credit()
            ),
            (id(12), id(13), id(14), id(15))
        );
        assert_eq!(
            (
                request.rent_program(),
                request.claims_program(),
                request.trading_program()
            ),
            (id(16), id(17), id(18))
        );
        assert_eq!(
            (
                request.custody_request_digest(),
                request.custody_receipt_digest()
            ),
            (id(19), id(20))
        );
        assert_eq!((request.generation(), request.claim_count()), (21, 5));
        assert_eq!(
            (
                request.quantity(),
                request.basis_scale(),
                request.collateral_transferred()
            ),
            (7, 11, 77)
        );
        let receipt = ClaimsFoundingReceiptV5::new(request, id(40), id(41), id(42), id(43), id(44))
            .expect("receipt");
        assert_eq!(
            ClaimsFoundingReceiptV5::decode(&receipt.to_bytes()),
            Ok(receipt)
        );
        receipt.verify_for(&request, id(40)).expect("join");
        assert_eq!(receipt.request(), request);
        assert_eq!(
            (receipt.request_digest(), receipt.aggregate_digest()),
            (id(40), id(41))
        );
        assert_eq!(
            (
                receipt.position_digest(),
                receipt.admission_digest(),
                receipt.post_resource_digest()
            ),
            (id(42), id(43), id(44))
        );
        let seeds = ClaimsFoundingAggregateSeedsV5::new(id(2)).expect("seeds");
        assert_eq!(
            seeds.as_slices(),
            [CLAIMS_FOUNDING_AGGREGATE_SEED_V5, id(2).as_slice()]
        );
    }

    #[test]
    fn hostile_header_identity_alias_and_width_refuse() {
        let bytes = request().to_bytes();
        assert_eq!(
            ClaimsFoundingRequestV5::decode(bytes.get(..831).expect("slice")),
            Err(ClaimsFoundingErrorV5::InvalidLength)
        );
        for (offset, expected) in [
            (0, ClaimsFoundingErrorV5::InvalidMagic),
            (VERSION_OFFSET, ClaimsFoundingErrorV5::UnsupportedVersion),
            (OWNER_KIND_OFFSET, ClaimsFoundingErrorV5::NonCanonical),
            (HEADER_RESERVED_OFFSET, ClaimsFoundingErrorV5::NonCanonical),
            (BODY_RESERVED_OFFSET, ClaimsFoundingErrorV5::NonCanonical),
            (TAIL_RESERVED_OFFSET, ClaimsFoundingErrorV5::NonCanonical),
        ] {
            let mut hostile = bytes;
            mutate(&mut hostile, offset, 99);
            assert_eq!(ClaimsFoundingRequestV5::decode(&hostile), Err(expected));
        }
        assert_eq!(
            ClaimsFoundingRequestV5::new(ClaimsFoundingRequestInputV5 {
                product_instance_id: [0; 32],
                ..input()
            }),
            Err(ClaimsFoundingErrorV5::ZeroIdentity)
        );
        assert_eq!(
            ClaimsFoundingRequestV5::new(ClaimsFoundingRequestInputV5 {
                position: id(9),
                ..input()
            }),
            Err(ClaimsFoundingErrorV5::AccountAlias)
        );
    }

    #[test]
    fn quantity_width_and_exact_source_to_hoard_transfer_refuse() {
        for hostile in [
            ClaimsFoundingRequestInputV5 {
                claim_count: 0,
                ..input()
            },
            ClaimsFoundingRequestInputV5 {
                quantity: 0,
                ..input()
            },
            ClaimsFoundingRequestInputV5 {
                basis_scale: 0,
                ..input()
            },
        ] {
            assert!(ClaimsFoundingRequestV5::new(hostile).is_err());
        }
        for hostile in [
            ClaimsFoundingRequestInputV5 {
                post_source_amount: 99,
                ..input()
            },
            ClaimsFoundingRequestInputV5 {
                post_hoard_amount: 99,
                ..input()
            },
            ClaimsFoundingRequestInputV5 {
                pre_source_amount: 76,
                post_source_amount: 77,
                ..input()
            },
            ClaimsFoundingRequestInputV5 {
                quantity: u64::MAX,
                basis_scale: 2,
                ..input()
            },
        ] {
            assert_eq!(
                ClaimsFoundingRequestV5::new(hostile),
                Err(ClaimsFoundingErrorV5::InvalidCollateralTransfer)
            );
        }
    }

    #[test]
    fn hostile_rent_revision_and_receipt_evidence_refuse() {
        assert_eq!(
            ClaimsFoundingRequestV5::new(ClaimsFoundingRequestInputV5 {
                admission_rent_principal: 0,
                ..input()
            }),
            Err(ClaimsFoundingErrorV5::InvalidRent)
        );
        for hostile in [
            ClaimsFoundingRequestInputV5 {
                post_custody_revision: 26,
                ..input()
            },
            ClaimsFoundingRequestInputV5 {
                pre_aggregate_revision: 1,
                ..input()
            },
            ClaimsFoundingRequestInputV5 {
                post_position_revision: 2,
                ..input()
            },
        ] {
            assert_eq!(
                ClaimsFoundingRequestV5::new(hostile),
                Err(ClaimsFoundingErrorV5::InvalidRevision)
            );
        }
        let request = request();
        assert_eq!(
            ClaimsFoundingReceiptV5::new(request, [0; 32], id(21), id(22), id(23), id(24),),
            Err(ClaimsFoundingErrorV5::ZeroIdentity)
        );
        let receipt = ClaimsFoundingReceiptV5::new(request, id(20), id(21), id(22), id(23), id(24))
            .expect("receipt");
        assert_eq!(
            receipt.verify_for(&request, id(99)),
            Err(ClaimsFoundingErrorV5::ReceiptMismatch)
        );
        let mut bytes = receipt.to_bytes();
        mutate(&mut bytes, 10, 1);
        assert_eq!(
            ClaimsFoundingReceiptV5::decode(&bytes),
            Err(ClaimsFoundingErrorV5::NonCanonical)
        );
    }
}
