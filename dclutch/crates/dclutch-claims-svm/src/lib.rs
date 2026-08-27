#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Exact runtime-width child ABI for the single Claims economic owner.
//!
//! A caller's domain packet remains owned by that caller. Its nonzero digest is
//! supplied as `request_id`; this ABI carries only Claims-owned economic facts.
//! The vector tail is borrowed, exact-width, and never allocated.

pub mod affine_batch_v2;
pub mod composition_v3;
pub mod custody_replay_v1;
pub mod founding_v4;
pub mod founding_v5;
pub mod frame_spec_v1;
pub mod lbv2_terminal_v2;
pub mod liability_basis_state_v2;
pub mod market_closure_v1;
pub mod product_basis_terminal_v3;
pub mod protocol_position_v2;
mod request_layout;
pub mod signed_delta_v3;
pub mod sparse_native_transfer_v1;
pub mod terminal_settlement_v3;

pub use request_layout::ClaimsPlanLayoutV1;

/// Bytes before the runtime-width `u64` quantity vector.
pub const CLAIMS_PLAN_HEADER_BYTES_V1: usize = 208;
/// Bytes in one exact claim quantity.
pub const CLAIM_QUANTITY_BYTES: usize = 8;
/// Bytes in one exact success receipt.
pub const CLAIMS_RECEIPT_BYTES_V1: usize = 256;
/// Sentinel for an intentionally absent Position revision.
pub const NO_POSITION_REVISION: u64 = u64::MAX;
/// Plan wire magic.
pub const CLAIMS_PLAN_MAGIC_V1: [u8; 8] = *b"DCLTCPK1";
/// Receipt wire magic.
pub const CLAIMS_RECEIPT_MAGIC_V1: [u8; 8] = *b"DCLTCAR1";
/// Implemented wire version.
pub const CLAIMS_WIRE_VERSION_V1: u16 = 1;
/// Canonical Claims aggregate PDA seed domain.
pub const CLAIMS_AGGREGATE_PDA_DOMAIN_V1: &[u8] = b"dclutch:claims-aggregate:v1";
/// Canonical Claims Position PDA seed domain.
pub const CLAIMS_POSITION_PDA_DOMAIN_V1: &[u8] = b"dclutch:claims-position:v1";

const VERSION_OFFSET: usize = 8;
const PLAN_KIND_OFFSET: usize = 10;
const PLAN_ROLE_OFFSET: usize = 11;
const PLAN_RESERVED_OFFSET: usize = 12;
const RELEASE_SET_OFFSET: usize = 16;
const MARKET_OFFSET: usize = 48;
const REQUEST_OFFSET: usize = 80;
const SOURCE_OWNER_OFFSET: usize = 112;
const DESTINATION_OWNER_OFFSET: usize = 144;
const EXPECTED_MARKET_REVISION_OFFSET: usize = 176;
const EXPECTED_SOURCE_REVISION_OFFSET: usize = 184;
const EXPECTED_DESTINATION_REVISION_OFFSET: usize = 192;
const OUTCOME_COUNT_OFFSET: usize = 200;
const PLAN_BODY_RESERVED_OFFSET: usize = 204;

const RECEIPT_ROLE_OFFSET: usize = 10;
const RECEIPT_KIND_OFFSET: usize = 11;
const RECEIPT_RESERVED_OFFSET: usize = 12;
const RECEIPT_PACKET_DIGEST_OFFSET: usize = 112;
const RECEIPT_CLAIMS_PROGRAM_OFFSET: usize = 144;
const RECEIPT_PRE_MARKET_REVISION_OFFSET: usize = 176;
const RECEIPT_POST_MARKET_REVISION_OFFSET: usize = 184;
const RECEIPT_POST_SOURCE_REVISION_OFFSET: usize = 192;
const RECEIPT_POST_DESTINATION_REVISION_OFFSET: usize = 200;
const RECEIPT_PAYOUT_OFFSET: usize = 208;
const RECEIPT_POST_RESOURCE_DIGEST_OFFSET: usize = 216;
const RECEIPT_TAIL_RESERVED_OFFSET: usize = 248;

/// Hostile decode or canonicality refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// A packet did not have its exact header-derived width.
    InvalidLength,
    /// Magic bytes selected another wire family.
    InvalidMagic,
    /// The wire version is unsupported.
    UnsupportedVersion,
    /// Reserved bytes were nonzero.
    NonCanonicalReserved,
    /// A tag selected no implemented action or caller role.
    UnknownTag,
    /// A required identity was zero.
    ZeroIdentity,
    /// Source/destination Position identities were not canonical for the action.
    InvalidPositionShape,
    /// The runtime outcome width was zero or did not fit exact address arithmetic.
    InvalidOutcomeCount,
    /// The basket was empty or a complete set was not coordinate-equal.
    InvalidQuantityVector,
    /// An optimistic revision coordinate was absent or unexpectedly present.
    InvalidRevisionShape,
    /// A post-revision did not advance its exact present pre-revision once.
    InvalidPostRevision,
}

/// Result alias for this ABI.
pub type Result<T> = core::result::Result<T, Error>;

/// Registry role authorized to invoke the Claims child.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CallerRole {
    /// Market-Core orchestration such as Series founding.
    Core = 0,
    /// Trading orchestration such as Dealer and General clearing.
    Trading = 2,
}

impl CallerRole {
    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Core),
            2 => Ok(Self::Trading),
            _ => Err(Error::UnknownTag),
        }
    }
}

/// Canonical aggregate Claims identity under the current Claims program.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimsAggregateSeedsV1 {
    market: [u8; 32],
}

impl ClaimsAggregateSeedsV1 {
    /// Construct the unique aggregate coordinates for one logical Core Market.
    pub fn new(market: [u8; 32]) -> Result<Self> {
        require_nonzero(market)?;
        Ok(Self { market })
    }

    /// Borrow the exact ordered PDA seed slices, excluding the bump.
    pub fn as_slices(&self) -> [&[u8]; 2] {
        [CLAIMS_AGGREGATE_PDA_DOMAIN_V1, &self.market]
    }
}

/// Canonical dynamic Position identity under the current Claims program.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimsPositionSeedsV1 {
    market: [u8; 32],
    owner: [u8; 32],
}

impl ClaimsPositionSeedsV1 {
    /// Construct the unique Position coordinates for one logical Market/owner pair.
    pub fn new(market: [u8; 32], owner: [u8; 32]) -> Result<Self> {
        require_nonzero(market)?;
        require_nonzero(owner)?;
        Ok(Self { market, owner })
    }

    /// Borrow the exact ordered PDA seed slices, excluding the bump.
    pub fn as_slices(&self) -> [&[u8]; 3] {
        [CLAIMS_POSITION_PDA_DOMAIN_V1, &self.market, &self.owner]
    }
}

/// One capability-neutral Claims basket operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ClaimsAction {
    /// Move native claims from one Position to another.
    TransferNative = 0,
    /// Convert source native claims into destination materialized claims.
    Materialize = 1,
    /// Convert source materialized claims into destination native claims.
    Dematerialize = 2,
    /// Burn source native terminal claims and derive collateral payout.
    RedeemNativeTerminal = 3,
    /// Burn source materialized terminal claims and derive collateral payout.
    RedeemMaterializedTerminal = 4,
    /// Mint one or more equal complete sets into a destination Position.
    MintCompleteSet = 5,
    /// Merge equal complete sets from a source Position.
    MergeCompleteSet = 6,
    /// Initialize vacant canonical Claims accounts and mint the founding complete set.
    InitializeCompleteSet = 7,
}

impl ClaimsAction {
    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::TransferNative),
            1 => Ok(Self::Materialize),
            2 => Ok(Self::Dematerialize),
            3 => Ok(Self::RedeemNativeTerminal),
            4 => Ok(Self::RedeemMaterializedTerminal),
            5 => Ok(Self::MintCompleteSet),
            6 => Ok(Self::MergeCompleteSet),
            7 => Ok(Self::InitializeCompleteSet),
            _ => Err(Error::UnknownTag),
        }
    }
}

/// Borrowed exact Claims child plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimsPlanV1<'a> {
    action: ClaimsAction,
    caller_role: CallerRole,
    release_set_id: [u8; 32],
    market: [u8; 32],
    request_id: [u8; 32],
    source_owner: [u8; 32],
    destination_owner: [u8; 32],
    expected_market_revision: u64,
    expected_source_revision: u64,
    expected_destination_revision: u64,
    outcome_count: u32,
    quantities: &'a [u8],
}

impl<'a> ClaimsPlanV1<'a> {
    /// Decode and fully canonicalize one exact runtime-width plan.
    pub fn decode(input: &'a [u8]) -> Result<Self> {
        if input.len() < CLAIMS_PLAN_HEADER_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        exact(input, 0, &CLAIMS_PLAN_MAGIC_V1)?;
        if u16_at(input, VERSION_OFFSET)? != CLAIMS_WIRE_VERSION_V1 {
            return Err(Error::UnsupportedVersion);
        }
        require_zero(input, PLAN_RESERVED_OFFSET, 4)?;
        require_zero(input, PLAN_BODY_RESERVED_OFFSET, 4)?;
        let outcome_count = u32_at(input, OUTCOME_COUNT_OFFSET)?;
        let tail_bytes = quantity_tail_bytes(outcome_count)?;
        let expected = CLAIMS_PLAN_HEADER_BYTES_V1
            .checked_add(tail_bytes)
            .ok_or(Error::InvalidLength)?;
        if input.len() != expected {
            return Err(Error::InvalidLength);
        }
        let value = Self {
            action: ClaimsAction::decode(byte_at(input, PLAN_KIND_OFFSET)?)?,
            caller_role: CallerRole::decode(byte_at(input, PLAN_ROLE_OFFSET)?)?,
            release_set_id: nonzero_array(input, RELEASE_SET_OFFSET)?,
            market: nonzero_array(input, MARKET_OFFSET)?,
            request_id: nonzero_array(input, REQUEST_OFFSET)?,
            source_owner: array_at(input, SOURCE_OWNER_OFFSET)?,
            destination_owner: array_at(input, DESTINATION_OWNER_OFFSET)?,
            expected_market_revision: u64_at(input, EXPECTED_MARKET_REVISION_OFFSET)?,
            expected_source_revision: u64_at(input, EXPECTED_SOURCE_REVISION_OFFSET)?,
            expected_destination_revision: u64_at(input, EXPECTED_DESTINATION_REVISION_OFFSET)?,
            outcome_count,
            quantities: slice(input, CLAIMS_PLAN_HEADER_BYTES_V1, tail_bytes)?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Construct and canonicalize one borrowed runtime-width plan.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        action: ClaimsAction,
        caller_role: CallerRole,
        release_set_id: [u8; 32],
        market: [u8; 32],
        request_id: [u8; 32],
        source_owner: [u8; 32],
        destination_owner: [u8; 32],
        expected_market_revision: u64,
        expected_source_revision: u64,
        expected_destination_revision: u64,
        outcome_count: u32,
        quantities: &'a [u8],
    ) -> Result<Self> {
        let value = Self {
            action,
            caller_role,
            release_set_id,
            market,
            request_id,
            source_owner,
            destination_owner,
            expected_market_revision,
            expected_source_revision,
            expected_destination_revision,
            outcome_count,
            quantities,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode into the exact caller-owned output slice.
    pub fn encode_into(self, output: &mut [u8]) -> Result<()> {
        let expected = CLAIMS_PLAN_HEADER_BYTES_V1
            .checked_add(self.quantities.len())
            .ok_or(Error::InvalidLength)?;
        if output.len() != expected {
            return Err(Error::InvalidLength);
        }
        self.validate()?;
        output.fill(0);
        put(output, 0, &CLAIMS_PLAN_MAGIC_V1)?;
        put(
            output,
            VERSION_OFFSET,
            &CLAIMS_WIRE_VERSION_V1.to_le_bytes(),
        )?;
        put_byte(output, PLAN_KIND_OFFSET, self.action as u8)?;
        put_byte(output, PLAN_ROLE_OFFSET, self.caller_role as u8)?;
        put(output, RELEASE_SET_OFFSET, &self.release_set_id)?;
        put(output, MARKET_OFFSET, &self.market)?;
        put(output, REQUEST_OFFSET, &self.request_id)?;
        put(output, SOURCE_OWNER_OFFSET, &self.source_owner)?;
        put(output, DESTINATION_OWNER_OFFSET, &self.destination_owner)?;
        put(
            output,
            EXPECTED_MARKET_REVISION_OFFSET,
            &self.expected_market_revision.to_le_bytes(),
        )?;
        put(
            output,
            EXPECTED_SOURCE_REVISION_OFFSET,
            &self.expected_source_revision.to_le_bytes(),
        )?;
        put(
            output,
            EXPECTED_DESTINATION_REVISION_OFFSET,
            &self.expected_destination_revision.to_le_bytes(),
        )?;
        put(
            output,
            OUTCOME_COUNT_OFFSET,
            &self.outcome_count.to_le_bytes(),
        )?;
        put(output, CLAIMS_PLAN_HEADER_BYTES_V1, self.quantities)
    }

    /// Action selected by this plan.
    pub const fn action(self) -> ClaimsAction {
        self.action
    }

    /// Registry role required from the caller.
    pub const fn caller_role(self) -> CallerRole {
        self.caller_role
    }

    /// Immutable selected execution release set.
    pub const fn release_set_id(self) -> [u8; 32] {
        self.release_set_id
    }

    /// Claims Market account identity.
    pub const fn market(self) -> [u8; 32] {
        self.market
    }

    /// Caller-owned domain request digest.
    pub const fn request_id(self) -> [u8; 32] {
        self.request_id
    }

    /// Source Position owner or the canonical zero sentinel.
    pub const fn source_owner(self) -> [u8; 32] {
        self.source_owner
    }

    /// Destination Position owner or the canonical zero sentinel.
    pub const fn destination_owner(self) -> [u8; 32] {
        self.destination_owner
    }

    /// Exact initial Claims Market revision.
    pub const fn expected_market_revision(self) -> u64 {
        self.expected_market_revision
    }

    /// Exact initial source Position revision or [`NO_POSITION_REVISION`].
    pub const fn expected_source_revision(self) -> u64 {
        self.expected_source_revision
    }

    /// Exact initial destination Position revision or [`NO_POSITION_REVISION`].
    pub const fn expected_destination_revision(self) -> u64 {
        self.expected_destination_revision
    }

    /// Exact Product-owned runtime outcome width.
    pub const fn outcome_count(self) -> u32 {
        self.outcome_count
    }

    /// Return one exact claim quantity.
    pub fn quantity(self, outcome: u32) -> Result<u64> {
        if outcome >= self.outcome_count {
            return Err(Error::InvalidOutcomeCount);
        }
        let index = usize::try_from(outcome).map_err(|_| Error::InvalidOutcomeCount)?;
        let offset = index
            .checked_mul(CLAIM_QUANTITY_BYTES)
            .ok_or(Error::InvalidOutcomeCount)?;
        u64_at(self.quantities, offset)
    }

    /// Borrow the exact little-endian `u64[outcome_count]` quantity tail.
    pub const fn quantities_bytes(self) -> &'a [u8] {
        self.quantities
    }

    fn validate(self) -> Result<()> {
        require_nonzero(self.release_set_id)?;
        require_nonzero(self.market)?;
        require_nonzero(self.request_id)?;
        if self.quantities.len() != quantity_tail_bytes(self.outcome_count)? {
            return Err(Error::InvalidLength);
        }
        let source = !is_zero(self.source_owner);
        let destination = !is_zero(self.destination_owner);
        let source_revision = self.expected_source_revision != NO_POSITION_REVISION;
        let destination_revision = self.expected_destination_revision != NO_POSITION_REVISION;
        let shape_valid = match self.action {
            ClaimsAction::TransferNative
            | ClaimsAction::Materialize
            | ClaimsAction::Dematerialize => {
                source
                    && destination
                    && self.source_owner != self.destination_owner
                    && source_revision
                    && destination_revision
            }
            ClaimsAction::RedeemNativeTerminal
            | ClaimsAction::RedeemMaterializedTerminal
            | ClaimsAction::MergeCompleteSet => {
                source && !destination && source_revision && !destination_revision
            }
            ClaimsAction::MintCompleteSet | ClaimsAction::InitializeCompleteSet => {
                !source && destination && !source_revision && destination_revision
            }
        };
        if !shape_valid {
            return Err(
                if source != source_revision || destination != destination_revision {
                    Error::InvalidRevisionShape
                } else {
                    Error::InvalidPositionShape
                },
            );
        }
        let first = self.quantity(0)?;
        let complete_set = matches!(
            self.action,
            ClaimsAction::MintCompleteSet
                | ClaimsAction::MergeCompleteSet
                | ClaimsAction::InitializeCompleteSet
        );
        let mut any_positive = false;
        let mut outcome = 0_u32;
        while outcome < self.outcome_count {
            let quantity = self.quantity(outcome)?;
            if quantity != 0 {
                any_positive = true;
            }
            if complete_set && quantity != first {
                return Err(Error::InvalidQuantityVector);
            }
            outcome = outcome.checked_add(1).ok_or(Error::InvalidOutcomeCount)?;
        }
        if !any_positive {
            return Err(Error::InvalidQuantityVector);
        }
        Ok(())
    }
}

/// Fixed success acknowledgement returned by the Claims program.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimsReceiptV1 {
    caller_role: CallerRole,
    action: ClaimsAction,
    release_set_id: [u8; 32],
    market: [u8; 32],
    request_id: [u8; 32],
    packet_digest: [u8; 32],
    claims_program: [u8; 32],
    pre_market_revision: u64,
    post_market_revision: u64,
    post_source_revision: u64,
    post_destination_revision: u64,
    payout: u64,
    post_resource_digest: [u8; 32],
}

impl ClaimsReceiptV1 {
    /// Construct and validate an exact post-state acknowledgement.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        plan: ClaimsPlanV1<'_>,
        packet_digest: [u8; 32],
        claims_program: [u8; 32],
        post_market_revision: u64,
        post_source_revision: u64,
        post_destination_revision: u64,
        payout: u64,
        post_resource_digest: [u8; 32],
    ) -> Result<Self> {
        require_nonzero(packet_digest)?;
        require_nonzero(claims_program)?;
        require_nonzero(post_resource_digest)?;
        require_advanced(plan.expected_market_revision, post_market_revision, true)?;
        require_advanced(
            plan.expected_source_revision,
            post_source_revision,
            !is_zero(plan.source_owner),
        )?;
        require_advanced(
            plan.expected_destination_revision,
            post_destination_revision,
            !is_zero(plan.destination_owner),
        )?;
        Ok(Self {
            caller_role: plan.caller_role,
            action: plan.action,
            release_set_id: plan.release_set_id,
            market: plan.market,
            request_id: plan.request_id,
            packet_digest,
            claims_program,
            pre_market_revision: plan.expected_market_revision,
            post_market_revision,
            post_source_revision,
            post_destination_revision,
            payout,
            post_resource_digest,
        })
    }

    /// Hostile-decode one exact receipt.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != CLAIMS_RECEIPT_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        exact(input, 0, &CLAIMS_RECEIPT_MAGIC_V1)?;
        if u16_at(input, VERSION_OFFSET)? != CLAIMS_WIRE_VERSION_V1 {
            return Err(Error::UnsupportedVersion);
        }
        require_zero(input, RECEIPT_RESERVED_OFFSET, 4)?;
        require_zero(input, RECEIPT_TAIL_RESERVED_OFFSET, 8)?;
        let value = Self {
            caller_role: CallerRole::decode(byte_at(input, RECEIPT_ROLE_OFFSET)?)?,
            action: ClaimsAction::decode(byte_at(input, RECEIPT_KIND_OFFSET)?)?,
            release_set_id: nonzero_array(input, RELEASE_SET_OFFSET)?,
            market: nonzero_array(input, MARKET_OFFSET)?,
            request_id: nonzero_array(input, REQUEST_OFFSET)?,
            packet_digest: nonzero_array(input, RECEIPT_PACKET_DIGEST_OFFSET)?,
            claims_program: nonzero_array(input, RECEIPT_CLAIMS_PROGRAM_OFFSET)?,
            pre_market_revision: u64_at(input, RECEIPT_PRE_MARKET_REVISION_OFFSET)?,
            post_market_revision: u64_at(input, RECEIPT_POST_MARKET_REVISION_OFFSET)?,
            post_source_revision: u64_at(input, RECEIPT_POST_SOURCE_REVISION_OFFSET)?,
            post_destination_revision: u64_at(input, RECEIPT_POST_DESTINATION_REVISION_OFFSET)?,
            payout: u64_at(input, RECEIPT_PAYOUT_OFFSET)?,
            post_resource_digest: nonzero_array(input, RECEIPT_POST_RESOURCE_DIGEST_OFFSET)?,
        };
        if value.post_market_revision
            != value
                .pre_market_revision
                .checked_add(1)
                .ok_or(Error::InvalidPostRevision)?
        {
            return Err(Error::InvalidPostRevision);
        }
        let (source_present, destination_present) = match value.action {
            ClaimsAction::TransferNative
            | ClaimsAction::Materialize
            | ClaimsAction::Dematerialize => (true, true),
            ClaimsAction::RedeemNativeTerminal
            | ClaimsAction::RedeemMaterializedTerminal
            | ClaimsAction::MergeCompleteSet => (true, false),
            ClaimsAction::MintCompleteSet | ClaimsAction::InitializeCompleteSet => (false, true),
        };
        validate_post_revision_presence(value.post_source_revision, source_present)?;
        validate_post_revision_presence(value.post_destination_revision, destination_present)?;
        Ok(value)
    }

    /// Encode into the exact receipt wire.
    pub fn to_bytes(self) -> [u8; CLAIMS_RECEIPT_BYTES_V1] {
        let mut output = [0_u8; CLAIMS_RECEIPT_BYTES_V1];
        copy(&mut output, 0, &CLAIMS_RECEIPT_MAGIC_V1);
        copy(
            &mut output,
            VERSION_OFFSET,
            &CLAIMS_WIRE_VERSION_V1.to_le_bytes(),
        );
        set(&mut output, RECEIPT_ROLE_OFFSET, self.caller_role as u8);
        set(&mut output, RECEIPT_KIND_OFFSET, self.action as u8);
        copy(&mut output, RELEASE_SET_OFFSET, &self.release_set_id);
        copy(&mut output, MARKET_OFFSET, &self.market);
        copy(&mut output, REQUEST_OFFSET, &self.request_id);
        copy(
            &mut output,
            RECEIPT_PACKET_DIGEST_OFFSET,
            &self.packet_digest,
        );
        copy(
            &mut output,
            RECEIPT_CLAIMS_PROGRAM_OFFSET,
            &self.claims_program,
        );
        copy(
            &mut output,
            RECEIPT_PRE_MARKET_REVISION_OFFSET,
            &self.pre_market_revision.to_le_bytes(),
        );
        copy(
            &mut output,
            RECEIPT_POST_MARKET_REVISION_OFFSET,
            &self.post_market_revision.to_le_bytes(),
        );
        copy(
            &mut output,
            RECEIPT_POST_SOURCE_REVISION_OFFSET,
            &self.post_source_revision.to_le_bytes(),
        );
        copy(
            &mut output,
            RECEIPT_POST_DESTINATION_REVISION_OFFSET,
            &self.post_destination_revision.to_le_bytes(),
        );
        copy(
            &mut output,
            RECEIPT_PAYOUT_OFFSET,
            &self.payout.to_le_bytes(),
        );
        copy(
            &mut output,
            RECEIPT_POST_RESOURCE_DIGEST_OFFSET,
            &self.post_resource_digest,
        );
        output
    }

    /// Caller role reauthenticated for this transition.
    pub const fn caller_role(self) -> CallerRole {
        self.caller_role
    }

    /// Exact Claims action acknowledged.
    pub const fn action(self) -> ClaimsAction {
        self.action
    }

    /// Selected execution release set.
    pub const fn release_set_id(self) -> [u8; 32] {
        self.release_set_id
    }

    /// Exact Market identity.
    pub const fn market(self) -> [u8; 32] {
        self.market
    }

    /// Caller domain request digest.
    pub const fn request_id(self) -> [u8; 32] {
        self.request_id
    }

    /// SHA-256 of the complete Claims plan bytes.
    pub const fn packet_digest(self) -> [u8; 32] {
        self.packet_digest
    }

    /// Registry-authenticated current Claims program.
    pub const fn claims_program(self) -> [u8; 32] {
        self.claims_program
    }

    /// Exact initial Market revision.
    pub const fn pre_market_revision(self) -> u64 {
        self.pre_market_revision
    }

    /// Exact resulting Market revision.
    pub const fn post_market_revision(self) -> u64 {
        self.post_market_revision
    }

    /// Exact resulting source Position revision or the absent sentinel.
    pub const fn post_source_revision(self) -> u64 {
        self.post_source_revision
    }

    /// Exact resulting destination Position revision or the absent sentinel.
    pub const fn post_destination_revision(self) -> u64 {
        self.post_destination_revision
    }

    /// Exact collateral payout derived by Claims economics.
    pub const fn payout(self) -> u64 {
        self.payout
    }

    /// SHA-256 of the exact resulting Market and participating Position bytes.
    pub const fn post_resource_digest(self) -> [u8; 32] {
        self.post_resource_digest
    }
}

fn require_advanced(pre: u64, post: u64, present: bool) -> Result<()> {
    if present {
        if pre == NO_POSITION_REVISION
            || post != pre.checked_add(1).ok_or(Error::InvalidPostRevision)?
        {
            return Err(Error::InvalidPostRevision);
        }
    } else if pre != NO_POSITION_REVISION || post != NO_POSITION_REVISION {
        return Err(Error::InvalidPostRevision);
    }
    Ok(())
}

fn validate_post_revision_presence(revision: u64, present: bool) -> Result<()> {
    if (revision == NO_POSITION_REVISION) == present {
        Err(Error::InvalidPostRevision)
    } else {
        Ok(())
    }
}

fn quantity_tail_bytes(outcome_count: u32) -> Result<usize> {
    if outcome_count == 0 {
        return Err(Error::InvalidOutcomeCount);
    }
    usize::try_from(outcome_count)
        .map_err(|_| Error::InvalidOutcomeCount)?
        .checked_mul(CLAIM_QUANTITY_BYTES)
        .ok_or(Error::InvalidOutcomeCount)
}

fn require_nonzero(identity: [u8; 32]) -> Result<()> {
    if is_zero(identity) {
        Err(Error::ZeroIdentity)
    } else {
        Ok(())
    }
}

fn is_zero(identity: [u8; 32]) -> bool {
    identity.iter().all(|byte| *byte == 0)
}

fn exact(input: &[u8], offset: usize, expected: &[u8]) -> Result<()> {
    if slice(input, offset, expected.len())? == expected {
        Ok(())
    } else {
        Err(Error::InvalidMagic)
    }
}

fn require_zero(input: &[u8], offset: usize, width: usize) -> Result<()> {
    if slice(input, offset, width)?.iter().all(|byte| *byte == 0) {
        Ok(())
    } else {
        Err(Error::NonCanonicalReserved)
    }
}

fn nonzero_array(input: &[u8], offset: usize) -> Result<[u8; 32]> {
    let value = array_at(input, offset)?;
    require_nonzero(value)?;
    Ok(value)
}

fn slice(input: &[u8], offset: usize, width: usize) -> Result<&[u8]> {
    input
        .get(offset..offset.checked_add(width).ok_or(Error::InvalidLength)?)
        .ok_or(Error::InvalidLength)
}

fn byte_at(input: &[u8], offset: usize) -> Result<u8> {
    input.get(offset).copied().ok_or(Error::InvalidLength)
}

fn array_at(input: &[u8], offset: usize) -> Result<[u8; 32]> {
    slice(input, offset, 32)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn u16_at(input: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(
        slice(input, offset, 2)?
            .try_into()
            .map_err(|_| Error::InvalidLength)?,
    ))
}

fn u32_at(input: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        slice(input, offset, 4)?
            .try_into()
            .map_err(|_| Error::InvalidLength)?,
    ))
}

fn u64_at(input: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(
        slice(input, offset, 8)?
            .try_into()
            .map_err(|_| Error::InvalidLength)?,
    ))
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<()> {
    let end = offset
        .checked_add(value.len())
        .ok_or(Error::InvalidLength)?;
    let target = output.get_mut(offset..end).ok_or(Error::InvalidLength)?;
    target.copy_from_slice(value);
    Ok(())
}

fn put_byte(output: &mut [u8], offset: usize, value: u8) -> Result<()> {
    *output.get_mut(offset).ok_or(Error::InvalidLength)? = value;
    Ok(())
}

fn copy(output: &mut [u8; CLAIMS_RECEIPT_BYTES_V1], offset: usize, value: &[u8]) {
    if let Some(target) = output.get_mut(offset..offset.saturating_add(value.len())) {
        target.copy_from_slice(value);
    }
}

fn set(output: &mut [u8; CLAIMS_RECEIPT_BYTES_V1], offset: usize, value: u8) {
    if let Some(target) = output.get_mut(offset) {
        *target = value;
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use std::vec;

    fn quantities(values: &[u64]) -> std::vec::Vec<u8> {
        let mut output = vec![0_u8; values.len() * 8];
        for (index, value) in values.iter().enumerate() {
            let start = index * 8;
            if let Some(target) = output.get_mut(start..start + 8) {
                target.copy_from_slice(&value.to_le_bytes());
            }
        }
        output
    }

    #[test]
    fn runtime_width_plan_roundtrips_without_family_context() -> Result<()> {
        let vector = quantities(&[1; 257]);
        let plan = ClaimsPlanV1::new(
            ClaimsAction::TransferNative,
            CallerRole::Trading,
            [1; 32],
            [2; 32],
            [3; 32],
            [4; 32],
            [5; 32],
            9,
            10,
            11,
            257,
            &vector,
        )?;
        let mut encoded = vec![0_u8; CLAIMS_PLAN_HEADER_BYTES_V1 + vector.len()];
        plan.encode_into(&mut encoded)?;
        assert_eq!(ClaimsPlanV1::decode(&encoded), Ok(plan));
        assert_eq!(plan.quantity(256), Ok(1));
        Ok(())
    }

    #[test]
    fn complete_set_and_position_sentinels_are_canonical() {
        let unequal = quantities(&[5, 4, 5]);
        assert_eq!(
            ClaimsPlanV1::new(
                ClaimsAction::MintCompleteSet,
                CallerRole::Core,
                [1; 32],
                [2; 32],
                [3; 32],
                [0; 32],
                [5; 32],
                0,
                NO_POSITION_REVISION,
                0,
                3,
                &unequal,
            ),
            Err(Error::InvalidQuantityVector)
        );
        let equal = quantities(&[5, 5, 5]);
        assert_eq!(
            ClaimsPlanV1::new(
                ClaimsAction::MintCompleteSet,
                CallerRole::Core,
                [1; 32],
                [2; 32],
                [3; 32],
                [0; 32],
                [5; 32],
                0,
                0,
                0,
                3,
                &equal,
            ),
            Err(Error::InvalidRevisionShape)
        );
    }

    #[test]
    fn foundational_complete_set_has_a_nonaliasing_action_tag() -> Result<()> {
        let equal = quantities(&[5, 5, 5]);
        let plan = ClaimsPlanV1::new(
            ClaimsAction::InitializeCompleteSet,
            CallerRole::Core,
            [1; 32],
            [2; 32],
            [3; 32],
            [0; 32],
            [5; 32],
            0,
            NO_POSITION_REVISION,
            0,
            3,
            &equal,
        )?;
        let mut encoded = vec![0_u8; CLAIMS_PLAN_HEADER_BYTES_V1 + equal.len()];
        plan.encode_into(&mut encoded)?;
        assert_eq!(encoded.get(PLAN_KIND_OFFSET), Some(&7));
        assert_eq!(ClaimsPlanV1::decode(&encoded), Ok(plan));
        assert_ne!(plan.action(), ClaimsAction::MintCompleteSet);
        Ok(())
    }

    #[test]
    fn hostile_bytes_and_receipt_revisions_refuse() -> Result<()> {
        let vector = quantities(&[7, 0, 2]);
        let plan = ClaimsPlanV1::new(
            ClaimsAction::RedeemNativeTerminal,
            CallerRole::Trading,
            [1; 32],
            [2; 32],
            [3; 32],
            [4; 32],
            [0; 32],
            8,
            12,
            NO_POSITION_REVISION,
            3,
            &vector,
        )?;
        let mut encoded = vec![0_u8; CLAIMS_PLAN_HEADER_BYTES_V1 + vector.len()];
        plan.encode_into(&mut encoded)?;
        let mut hostile = encoded.clone();
        if let Some(byte) = hostile.get_mut(PLAN_RESERVED_OFFSET) {
            *byte = 1;
        }
        assert_eq!(
            ClaimsPlanV1::decode(&hostile),
            Err(Error::NonCanonicalReserved)
        );
        assert_eq!(
            ClaimsReceiptV1::new(
                plan,
                [6; 32],
                [7; 32],
                8,
                13,
                NO_POSITION_REVISION,
                7,
                [8; 32]
            ),
            Err(Error::InvalidPostRevision)
        );
        let receipt = ClaimsReceiptV1::new(
            plan,
            [6; 32],
            [7; 32],
            9,
            13,
            NO_POSITION_REVISION,
            7,
            [8; 32],
        )?;
        assert_eq!(ClaimsReceiptV1::decode(&receipt.to_bytes()), Ok(receipt));
        Ok(())
    }

    #[test]
    fn dynamic_position_identity_is_unique_and_nonzero() -> Result<()> {
        let aggregate = ClaimsAggregateSeedsV1::new([2; 32])?;
        assert_eq!(
            aggregate.as_slices(),
            [CLAIMS_AGGREGATE_PDA_DOMAIN_V1, [2; 32].as_slice()]
        );
        assert_eq!(
            ClaimsAggregateSeedsV1::new([0; 32]),
            Err(Error::ZeroIdentity)
        );
        let seeds = ClaimsPositionSeedsV1::new([2; 32], [4; 32])?;
        assert_eq!(
            seeds.as_slices(),
            [
                CLAIMS_POSITION_PDA_DOMAIN_V1,
                [2; 32].as_slice(),
                [4; 32].as_slice(),
            ]
        );
        assert_eq!(
            ClaimsPositionSeedsV1::new([0; 32], [4; 32]),
            Err(Error::ZeroIdentity)
        );
        assert_eq!(
            ClaimsPositionSeedsV1::new([2; 32], [0; 32]),
            Err(Error::ZeroIdentity)
        );
        Ok(())
    }
}
