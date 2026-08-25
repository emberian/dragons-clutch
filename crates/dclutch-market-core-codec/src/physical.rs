//! Canonical cross-program routing, replay, and acknowledgement bytes.
//!
//! Role-owned requests remain the sole semantic owner of Claims, Custody, and
//! Resolution facts. A Core envelope binds one exact request by length and
//! digest; it never repeats the request's token, funding, certificate, or
//! position fields. The full-effect digest is SHA-256 over
//! `CORE_EFFECT_DIGEST_DOMAIN_V1 || u32_le(280) || envelope ||
//! u32_le(role_request_bytes) || role_request`. The caller authority is the PDA
//! under `caller_program` with seeds
//! `CallerAuthoritySeedsV1`, always with caller role Core. Series is not an
//! execution-release role and calls Core through the separate
//! `SeriesCoreRequestV1` boundary.

use crate::{
    Error, Identity, Role,
    generated_physical::{
        ACK_ACTION_OFFSET, ACK_CONTEXT_OFFSET, ACK_EFFECT_DIGEST_OFFSET, ACK_MAGIC_OFFSET,
        ACK_MARKET_OFFSET, ACK_POST_RESOURCE_A_REVISION_OFFSET,
        ACK_POST_RESOURCE_B_REVISION_OFFSET, ACK_POST_RESOURCE_DIGEST_OFFSET,
        ACK_PRE_RESOURCE_A_REVISION_OFFSET, ACK_PRE_RESOURCE_B_REVISION_OFFSET,
        ACK_RELEASE_SET_OFFSET, ACK_RESERVED_OFFSET, ACK_ROLE_PROGRAM_OFFSET,
        ACK_TARGET_ROLE_OFFSET, ACK_VERSION_OFFSET, CORE_EFFECT_ACK_BYTES_V1,
        CORE_EFFECT_ACK_MAGIC_V1, CORE_EFFECT_ENVELOPE_BYTES_V1, CORE_EFFECT_MAGIC_V1,
        EFFECT_ACTION_OFFSET, EFFECT_CALLER_AUTHORITY_OFFSET, EFFECT_CALLER_PROGRAM_OFFSET,
        EFFECT_CONTEXT_OFFSET, EFFECT_EXPECTED_RESOURCE_A_REVISION_OFFSET,
        EFFECT_EXPECTED_RESOURCE_B_REVISION_OFFSET, EFFECT_GENERATION_OFFSET, EFFECT_MAGIC_OFFSET,
        EFFECT_MARKET_OFFSET, EFFECT_PARENT_STATE_DIGEST_OFFSET, EFFECT_RELEASE_SET_OFFSET,
        EFFECT_RESERVED_BODY_OFFSET, EFFECT_RESERVED_HEADER_OFFSET,
        EFFECT_ROLE_REQUEST_BYTES_OFFSET, EFFECT_ROLE_REQUEST_DIGEST_OFFSET,
        EFFECT_TARGET_ROLE_OFFSET, EFFECT_VERSION_OFFSET, PHYSICAL_ABI_VERSION_V1,
        SERIES_ACTION_OFFSET, SERIES_BENEFICIARY_OFFSET, SERIES_CAPABILITY_RENT_OFFSET,
        SERIES_CLOSE_RENT_OFFSET, SERIES_CORE_REQUEST_BYTES_V1, SERIES_CORE_REQUEST_MAGIC_V1,
        SERIES_EXPECTED_SERIES_REVISION_OFFSET, SERIES_EXPECTED_TICKET_REVISION_OFFSET,
        SERIES_FOUNDER_OFFSET, SERIES_HOARD_PRINCIPAL_OFFSET, SERIES_MAGIC_OFFSET,
        SERIES_MARKET_OFFSET, SERIES_MARKET_RENT_OFFSET, SERIES_OCCURRENCE_OFFSET,
        SERIES_PRODUCT_OFFSET, SERIES_REALM_OFFSET, SERIES_RELEASE_SET_OFFSET,
        SERIES_RESERVED_BODY_OFFSET, SERIES_RESERVED_HEADER_OFFSET, SERIES_TEMPLATE_OFFSET,
        SERIES_TICKET_OFFSET, SERIES_VERSION_OFFSET, SERIES_WORK_OFFSET,
    },
};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};

const IDENTITY_BYTES: usize = 32;
const ZERO_IDENTITY: [u8; IDENTITY_BYTES] = [0; IDENTITY_BYTES];

/// Canonical cross-program effect selected by Market Core.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CoreEffectActionV1 {
    /// Create the manifest-selected Resolution Fund.
    CreateFund = 0,
    /// Verify that the exact Fund reached readiness.
    VerifyFundReady = 1,
    /// Create and initialize the Realm-selected collateral custody.
    OpenCustody = 2,
    /// Mint one complete native claim set.
    SplitClaims = 3,
    /// Move collateral principal for a complete-set split.
    SplitCustody = 4,
    /// Admit one exact terminal Resolution receipt.
    AdmitTerminal = 5,
    /// Burn a terminal claim and derive its exact payout.
    RedeemClaims = 6,
    /// Release collateral for an authenticated terminal payout.
    RedeemCustody = 7,
    /// Close the consumed Resolution Fund to its persisted destination.
    CloseFund = 8,
    /// Close zero-balance custody to its persisted RentCredit destination.
    CloseCustody = 9,
}

impl CoreEffectActionV1 {
    fn decode(tag: u8) -> Result<Self, Error> {
        match tag {
            0 => Ok(Self::CreateFund),
            1 => Ok(Self::VerifyFundReady),
            2 => Ok(Self::OpenCustody),
            3 => Ok(Self::SplitClaims),
            4 => Ok(Self::SplitCustody),
            5 => Ok(Self::AdmitTerminal),
            6 => Ok(Self::RedeemClaims),
            7 => Ok(Self::RedeemCustody),
            8 => Ok(Self::CloseFund),
            9 => Ok(Self::CloseCustody),
            _ => Err(Error::InvalidTag),
        }
    }

    /// Return the single target release-set role that may execute the effect.
    #[must_use]
    pub const fn target_role(self) -> Role {
        match self {
            Self::CreateFund | Self::VerifyFundReady | Self::AdmitTerminal | Self::CloseFund => {
                Role::Resolution
            }
            Self::OpenCustody | Self::SplitCustody | Self::RedeemCustody | Self::CloseCustody => {
                Role::Custody
            }
            Self::SplitClaims | Self::RedeemClaims => Role::Claims,
        }
    }
}

/// Fixed routing and replay prefix for one exact role-owned request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CoreEffectEnvelopeV1 {
    action: CoreEffectActionV1,
    target_role: Role,
    caller_program: Identity,
    caller_authority: Identity,
    release_set: Identity,
    market: Identity,
    context: Identity,
    parent_state_digest: Identity,
    role_request_digest: Identity,
    generation: u64,
    expected_resource_a_revision: u64,
    expected_resource_b_revision: u64,
    role_request_bytes: u32,
}

impl CoreEffectEnvelopeV1 {
    /// Construct one envelope. The adapter supplies the exact request digest.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        action: CoreEffectActionV1,
        target_role: Role,
        caller_program: Identity,
        caller_authority: Identity,
        release_set: Identity,
        market: Identity,
        context: Identity,
        parent_state_digest: Identity,
        role_request_digest: Identity,
        generation: u64,
        expected_resource_a_revision: u64,
        expected_resource_b_revision: u64,
        role_request_bytes: u32,
    ) -> Result<Self, Error> {
        if target_role != action.target_role() || role_request_bytes == 0 {
            return Err(Error::InvalidCoordinates);
        }
        Ok(Self {
            action,
            target_role,
            caller_program,
            caller_authority,
            release_set,
            market,
            context,
            parent_state_digest,
            role_request_digest,
            generation,
            expected_resource_a_revision,
            expected_resource_b_revision,
            role_request_bytes,
        })
    }

    /// Hostile-decode one exact envelope with zero reserved bytes.
    pub fn decode(input: &[u8]) -> Result<Self, Error> {
        exact_len(input, CORE_EFFECT_ENVELOPE_BYTES_V1)?;
        exact_magic(input, EFFECT_MAGIC_OFFSET, &CORE_EFFECT_MAGIC_V1)?;
        if read_u16(input, EFFECT_VERSION_OFFSET)? != PHYSICAL_ABI_VERSION_V1 {
            return Err(Error::UnsupportedVersion);
        }
        require_zero(input, EFFECT_RESERVED_HEADER_OFFSET, 4)?;
        require_zero(input, EFFECT_RESERVED_BODY_OFFSET, 12)?;
        let action = CoreEffectActionV1::decode(read_u8(input, EFFECT_ACTION_OFFSET)?)?;
        let target_role = decode_role(read_u8(input, EFFECT_TARGET_ROLE_OFFSET)?)?;
        Self::new(
            action,
            target_role,
            read_identity(input, EFFECT_CALLER_PROGRAM_OFFSET)?,
            read_identity(input, EFFECT_CALLER_AUTHORITY_OFFSET)?,
            read_identity(input, EFFECT_RELEASE_SET_OFFSET)?,
            read_identity(input, EFFECT_MARKET_OFFSET)?,
            read_identity(input, EFFECT_CONTEXT_OFFSET)?,
            read_identity(input, EFFECT_PARENT_STATE_DIGEST_OFFSET)?,
            read_identity(input, EFFECT_ROLE_REQUEST_DIGEST_OFFSET)?,
            read_u64(input, EFFECT_GENERATION_OFFSET)?,
            read_u64(input, EFFECT_EXPECTED_RESOURCE_A_REVISION_OFFSET)?,
            read_u64(input, EFFECT_EXPECTED_RESOURCE_B_REVISION_OFFSET)?,
            read_u32(input, EFFECT_ROLE_REQUEST_BYTES_OFFSET)?,
        )
    }

    /// Encode the exact fixed prefix.
    pub fn encode(self) -> Result<[u8; CORE_EFFECT_ENVELOPE_BYTES_V1], Error> {
        let mut output = [0; CORE_EFFECT_ENVELOPE_BYTES_V1];
        put(&mut output, EFFECT_MAGIC_OFFSET, &CORE_EFFECT_MAGIC_V1)?;
        put_u16(&mut output, EFFECT_VERSION_OFFSET, PHYSICAL_ABI_VERSION_V1)?;
        put_u8(&mut output, EFFECT_ACTION_OFFSET, self.action as u8)?;
        put_u8(
            &mut output,
            EFFECT_TARGET_ROLE_OFFSET,
            role_tag(self.target_role),
        )?;
        put_identity(
            &mut output,
            EFFECT_CALLER_PROGRAM_OFFSET,
            self.caller_program,
        )?;
        put_identity(
            &mut output,
            EFFECT_CALLER_AUTHORITY_OFFSET,
            self.caller_authority,
        )?;
        put_identity(&mut output, EFFECT_RELEASE_SET_OFFSET, self.release_set)?;
        put_identity(&mut output, EFFECT_MARKET_OFFSET, self.market)?;
        put_identity(&mut output, EFFECT_CONTEXT_OFFSET, self.context)?;
        put_identity(
            &mut output,
            EFFECT_PARENT_STATE_DIGEST_OFFSET,
            self.parent_state_digest,
        )?;
        put_identity(
            &mut output,
            EFFECT_ROLE_REQUEST_DIGEST_OFFSET,
            self.role_request_digest,
        )?;
        put_u64(&mut output, EFFECT_GENERATION_OFFSET, self.generation)?;
        put_u64(
            &mut output,
            EFFECT_EXPECTED_RESOURCE_A_REVISION_OFFSET,
            self.expected_resource_a_revision,
        )?;
        put_u64(
            &mut output,
            EFFECT_EXPECTED_RESOURCE_B_REVISION_OFFSET,
            self.expected_resource_b_revision,
        )?;
        put_u32(
            &mut output,
            EFFECT_ROLE_REQUEST_BYTES_OFFSET,
            self.role_request_bytes,
        )?;
        Ok(output)
    }

    /// Validate the adapter-observed request length and SHA-256 digest.
    pub fn validate_role_request(
        self,
        observed_bytes: usize,
        observed_digest: Identity,
    ) -> Result<(), Error> {
        let expected =
            usize::try_from(self.role_request_bytes).map_err(|_| Error::InvalidLength)?;
        if observed_bytes != expected || observed_digest != self.role_request_digest {
            return Err(Error::InvalidCoordinates);
        }
        Ok(())
    }

    /// Project the only allowed release-pinned caller PDA seed sequence.
    pub fn caller_authority_seeds(self) -> Result<CallerAuthoritySeedsV1, Error> {
        CallerAuthoritySeedsV1::from_bytes(
            self.release_set.to_bytes(),
            self.market.to_bytes(),
            ExecutionRoleV1::Core,
            self.context.to_bytes(),
            self.role_request_digest.to_bytes(),
        )
        .map_err(|_| Error::InvalidRelease)
    }

    /// Selected effect.
    #[must_use]
    pub const fn action(self) -> CoreEffectActionV1 {
        self.action
    }
    /// Selected target role. The caller role is always Core.
    #[must_use]
    pub const fn target_role(self) -> Role {
        self.target_role
    }
    /// Exact calling program.
    #[must_use]
    pub const fn caller_program(self) -> Identity {
        self.caller_program
    }
    /// Release-pinned calling PDA authority.
    #[must_use]
    pub const fn caller_authority(self) -> Identity {
        self.caller_authority
    }
    /// Immutable release-set identity.
    #[must_use]
    pub const fn release_set(self) -> Identity {
        self.release_set
    }
    /// Exact Market identity.
    #[must_use]
    pub const fn market(self) -> Identity {
        self.market
    }
    /// Nonzero action replay coordinate.
    #[must_use]
    pub const fn context(self) -> Identity {
        self.context
    }
    /// Digest of the exact parent state observed before the effect.
    #[must_use]
    pub const fn parent_state_digest(self) -> Identity {
        self.parent_state_digest
    }
    /// Digest of the exact role-owned request bytes.
    #[must_use]
    pub const fn role_request_digest(self) -> Identity {
        self.role_request_digest
    }
    /// Market generation.
    #[must_use]
    pub const fn generation(self) -> u64 {
        self.generation
    }
    /// First role-owned replay revision.
    #[must_use]
    pub const fn expected_resource_a_revision(self) -> u64 {
        self.expected_resource_a_revision
    }
    /// Second role-owned replay revision.
    #[must_use]
    pub const fn expected_resource_b_revision(self) -> u64 {
        self.expected_resource_b_revision
    }
    /// Exact trailing role request width.
    #[must_use]
    pub const fn role_request_bytes(self) -> u32 {
        self.role_request_bytes
    }
}

/// Normalized role-program acknowledgement consumed before Core commits state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CoreEffectAckV1 {
    action: CoreEffectActionV1,
    target_role: Role,
    role_program: Identity,
    release_set: Identity,
    market: Identity,
    context: Identity,
    effect_digest: Identity,
    post_resource_digest: Identity,
    pre_resource_a_revision: u64,
    post_resource_a_revision: u64,
    pre_resource_b_revision: u64,
    post_resource_b_revision: u64,
}

impl CoreEffectAckV1 {
    /// Construct one monotonic acknowledgment.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        action: CoreEffectActionV1,
        target_role: Role,
        role_program: Identity,
        release_set: Identity,
        market: Identity,
        context: Identity,
        effect_digest: Identity,
        post_resource_digest: Identity,
        pre_resource_a_revision: u64,
        post_resource_a_revision: u64,
        pre_resource_b_revision: u64,
        post_resource_b_revision: u64,
    ) -> Result<Self, Error> {
        if target_role != action.target_role()
            || post_resource_a_revision < pre_resource_a_revision
            || post_resource_b_revision < pre_resource_b_revision
        {
            return Err(Error::InvalidCoordinates);
        }
        Ok(Self {
            action,
            target_role,
            role_program,
            release_set,
            market,
            context,
            effect_digest,
            post_resource_digest,
            pre_resource_a_revision,
            post_resource_a_revision,
            pre_resource_b_revision,
            post_resource_b_revision,
        })
    }

    /// Hostile-decode one exact acknowledgment.
    pub fn decode(input: &[u8]) -> Result<Self, Error> {
        exact_len(input, CORE_EFFECT_ACK_BYTES_V1)?;
        exact_magic(input, ACK_MAGIC_OFFSET, &CORE_EFFECT_ACK_MAGIC_V1)?;
        if read_u16(input, ACK_VERSION_OFFSET)? != PHYSICAL_ABI_VERSION_V1 {
            return Err(Error::UnsupportedVersion);
        }
        require_zero(input, ACK_RESERVED_OFFSET, 4)?;
        Self::new(
            CoreEffectActionV1::decode(read_u8(input, ACK_ACTION_OFFSET)?)?,
            decode_role(read_u8(input, ACK_TARGET_ROLE_OFFSET)?)?,
            read_identity(input, ACK_ROLE_PROGRAM_OFFSET)?,
            read_identity(input, ACK_RELEASE_SET_OFFSET)?,
            read_identity(input, ACK_MARKET_OFFSET)?,
            read_identity(input, ACK_CONTEXT_OFFSET)?,
            read_identity(input, ACK_EFFECT_DIGEST_OFFSET)?,
            read_identity(input, ACK_POST_RESOURCE_DIGEST_OFFSET)?,
            read_u64(input, ACK_PRE_RESOURCE_A_REVISION_OFFSET)?,
            read_u64(input, ACK_POST_RESOURCE_A_REVISION_OFFSET)?,
            read_u64(input, ACK_PRE_RESOURCE_B_REVISION_OFFSET)?,
            read_u64(input, ACK_POST_RESOURCE_B_REVISION_OFFSET)?,
        )
    }

    /// Encode the exact fixed acknowledgment.
    pub fn encode(self) -> Result<[u8; CORE_EFFECT_ACK_BYTES_V1], Error> {
        let mut output = [0; CORE_EFFECT_ACK_BYTES_V1];
        put(&mut output, ACK_MAGIC_OFFSET, &CORE_EFFECT_ACK_MAGIC_V1)?;
        put_u16(&mut output, ACK_VERSION_OFFSET, PHYSICAL_ABI_VERSION_V1)?;
        put_u8(&mut output, ACK_ACTION_OFFSET, self.action as u8)?;
        put_u8(
            &mut output,
            ACK_TARGET_ROLE_OFFSET,
            role_tag(self.target_role),
        )?;
        put_identity(&mut output, ACK_ROLE_PROGRAM_OFFSET, self.role_program)?;
        put_identity(&mut output, ACK_RELEASE_SET_OFFSET, self.release_set)?;
        put_identity(&mut output, ACK_MARKET_OFFSET, self.market)?;
        put_identity(&mut output, ACK_CONTEXT_OFFSET, self.context)?;
        put_identity(&mut output, ACK_EFFECT_DIGEST_OFFSET, self.effect_digest)?;
        put_identity(
            &mut output,
            ACK_POST_RESOURCE_DIGEST_OFFSET,
            self.post_resource_digest,
        )?;
        put_u64(
            &mut output,
            ACK_PRE_RESOURCE_A_REVISION_OFFSET,
            self.pre_resource_a_revision,
        )?;
        put_u64(
            &mut output,
            ACK_POST_RESOURCE_A_REVISION_OFFSET,
            self.post_resource_a_revision,
        )?;
        put_u64(
            &mut output,
            ACK_PRE_RESOURCE_B_REVISION_OFFSET,
            self.pre_resource_b_revision,
        )?;
        put_u64(
            &mut output,
            ACK_POST_RESOURCE_B_REVISION_OFFSET,
            self.post_resource_b_revision,
        )?;
        Ok(output)
    }

    /// Authenticate this acknowledgment against its exact parent effect.
    pub fn validate_for(
        self,
        envelope: CoreEffectEnvelopeV1,
        expected_role_program: Identity,
        full_effect_digest: Identity,
    ) -> Result<(), Error> {
        if self.action != envelope.action
            || self.target_role != envelope.target_role
            || self.role_program != expected_role_program
            || self.release_set != envelope.release_set
            || self.market != envelope.market
            || self.context != envelope.context
            || self.effect_digest != full_effect_digest
            || self.pre_resource_a_revision != envelope.expected_resource_a_revision
            || self.pre_resource_b_revision != envelope.expected_resource_b_revision
        {
            return Err(Error::InvalidRelease);
        }
        Ok(())
    }

    /// Selected effect.
    #[must_use]
    pub const fn action(self) -> CoreEffectActionV1 {
        self.action
    }
    /// Selected target role.
    #[must_use]
    pub const fn target_role(self) -> Role {
        self.target_role
    }
    /// Exact current role program.
    #[must_use]
    pub const fn role_program(self) -> Identity {
        self.role_program
    }
    /// Immutable release set.
    #[must_use]
    pub const fn release_set(self) -> Identity {
        self.release_set
    }
    /// Exact Market.
    #[must_use]
    pub const fn market(self) -> Identity {
        self.market
    }
    /// Exact replay context.
    #[must_use]
    pub const fn context(self) -> Identity {
        self.context
    }
    /// Digest of envelope plus exact role-owned request.
    #[must_use]
    pub const fn effect_digest(self) -> Identity {
        self.effect_digest
    }
    /// Role-owned normalized poststate digest.
    #[must_use]
    pub const fn post_resource_digest(self) -> Identity {
        self.post_resource_digest
    }
    /// First observed pre-revision.
    #[must_use]
    pub const fn pre_resource_a_revision(self) -> u64 {
        self.pre_resource_a_revision
    }
    /// First observed post-revision.
    #[must_use]
    pub const fn post_resource_a_revision(self) -> u64 {
        self.post_resource_a_revision
    }
    /// Second observed pre-revision.
    #[must_use]
    pub const fn pre_resource_b_revision(self) -> u64 {
        self.pre_resource_b_revision
    }
    /// Second observed post-revision.
    #[must_use]
    pub const fn post_resource_b_revision(self) -> u64 {
        self.post_resource_b_revision
    }
}

/// Core-owned Series lifecycle action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SeriesCoreActionV1 {
    /// Prepay exact immutable child resources.
    Prepare = 0,
    /// Consume the ticket and found its exact Market.
    Consume = 1,
    /// Expire and refund an unconsumed ticket.
    Expire = 2,
    /// Close a terminal Series to its persisted refund owner.
    Close = 3,
}

impl SeriesCoreActionV1 {
    fn decode(tag: u8) -> Result<Self, Error> {
        match tag {
            0 => Ok(Self::Prepare),
            1 => Ok(Self::Consume),
            2 => Ok(Self::Expire),
            3 => Ok(Self::Close),
            _ => Err(Error::InvalidTag),
        }
    }
}

/// Canonical Series-to-Core request; occurrence and close shapes are disjoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesCoreRequestV1 {
    action: SeriesCoreActionV1,
    release_set: Identity,
    template: Identity,
    ticket: Option<Identity>,
    market: Option<Identity>,
    realm: Option<Identity>,
    product: Option<Identity>,
    beneficiary: Identity,
    founder: Option<Identity>,
    occurrence: u32,
    expected_series_revision: u64,
    expected_ticket_revision: u64,
    market_rent: u64,
    capability_rent: u64,
    work: u64,
    hoard_principal: u64,
    series_close_rent: u64,
}

impl SeriesCoreRequestV1 {
    /// Construct Prepare, Consume, or Expire for one exact ticket and Market.
    #[allow(clippy::too_many_arguments)]
    pub fn occurrence(
        action: SeriesCoreActionV1,
        release_set: Identity,
        template: Identity,
        ticket: Identity,
        market: Identity,
        realm: Identity,
        product: Identity,
        beneficiary: Identity,
        founder: Identity,
        occurrence: u32,
        expected_series_revision: u64,
        expected_ticket_revision: u64,
        market_rent: u64,
        capability_rent: u64,
        work: u64,
        hoard_principal: u64,
    ) -> Result<Self, Error> {
        if action == SeriesCoreActionV1::Close || hoard_principal == 0 {
            return Err(Error::InvalidCoordinates);
        }
        Ok(Self {
            action,
            release_set,
            template,
            ticket: Some(ticket),
            market: Some(market),
            realm: Some(realm),
            product: Some(product),
            beneficiary,
            founder: Some(founder),
            occurrence,
            expected_series_revision,
            expected_ticket_revision,
            market_rent,
            capability_rent,
            work,
            hoard_principal,
            series_close_rent: 0,
        })
    }

    /// Construct terminal Series close without occurrence-only coordinates.
    pub fn close(
        release_set: Identity,
        template: Identity,
        beneficiary: Identity,
        expected_series_revision: u64,
        series_close_rent: u64,
    ) -> Result<Self, Error> {
        Ok(Self {
            action: SeriesCoreActionV1::Close,
            release_set,
            template,
            ticket: None,
            market: None,
            realm: None,
            product: None,
            beneficiary,
            founder: None,
            occurrence: 0,
            expected_series_revision,
            expected_ticket_revision: 0,
            market_rent: 0,
            capability_rent: 0,
            work: 0,
            hoard_principal: 0,
            series_close_rent,
        })
    }

    /// Hostile-decode one exact action-specific request.
    pub fn decode(input: &[u8]) -> Result<Self, Error> {
        exact_len(input, SERIES_CORE_REQUEST_BYTES_V1)?;
        exact_magic(input, SERIES_MAGIC_OFFSET, &SERIES_CORE_REQUEST_MAGIC_V1)?;
        if read_u16(input, SERIES_VERSION_OFFSET)? != PHYSICAL_ABI_VERSION_V1 {
            return Err(Error::UnsupportedVersion);
        }
        require_zero(input, SERIES_RESERVED_HEADER_OFFSET, 5)?;
        require_zero(input, SERIES_RESERVED_BODY_OFFSET, 4)?;
        let action = SeriesCoreActionV1::decode(read_u8(input, SERIES_ACTION_OFFSET)?)?;
        let release_set = read_identity(input, SERIES_RELEASE_SET_OFFSET)?;
        let template = read_identity(input, SERIES_TEMPLATE_OFFSET)?;
        let beneficiary = read_identity(input, SERIES_BENEFICIARY_OFFSET)?;
        let expected_series_revision = read_u64(input, SERIES_EXPECTED_SERIES_REVISION_OFFSET)?;
        let close_rent = read_u64(input, SERIES_CLOSE_RENT_OFFSET)?;
        if action == SeriesCoreActionV1::Close {
            require_zero(input, SERIES_TICKET_OFFSET, IDENTITY_BYTES * 4)?;
            require_zero(input, SERIES_FOUNDER_OFFSET, IDENTITY_BYTES)?;
            require_zero(input, SERIES_OCCURRENCE_OFFSET, 8)?;
            require_zero(input, SERIES_EXPECTED_TICKET_REVISION_OFFSET, 40)?;
            return Self::close(
                release_set,
                template,
                beneficiary,
                expected_series_revision,
                close_rent,
            );
        }
        if close_rent != 0 {
            return Err(Error::NonzeroReserved);
        }
        Self::occurrence(
            action,
            release_set,
            template,
            read_identity(input, SERIES_TICKET_OFFSET)?,
            read_identity(input, SERIES_MARKET_OFFSET)?,
            read_identity(input, SERIES_REALM_OFFSET)?,
            read_identity(input, SERIES_PRODUCT_OFFSET)?,
            beneficiary,
            read_identity(input, SERIES_FOUNDER_OFFSET)?,
            read_u32(input, SERIES_OCCURRENCE_OFFSET)?,
            expected_series_revision,
            read_u64(input, SERIES_EXPECTED_TICKET_REVISION_OFFSET)?,
            read_u64(input, SERIES_MARKET_RENT_OFFSET)?,
            read_u64(input, SERIES_CAPABILITY_RENT_OFFSET)?,
            read_u64(input, SERIES_WORK_OFFSET)?,
            read_u64(input, SERIES_HOARD_PRINCIPAL_OFFSET)?,
        )
    }

    /// Encode the exact action-specific request.
    pub fn encode(self) -> Result<[u8; SERIES_CORE_REQUEST_BYTES_V1], Error> {
        let mut output = [0; SERIES_CORE_REQUEST_BYTES_V1];
        put(
            &mut output,
            SERIES_MAGIC_OFFSET,
            &SERIES_CORE_REQUEST_MAGIC_V1,
        )?;
        put_u16(&mut output, SERIES_VERSION_OFFSET, PHYSICAL_ABI_VERSION_V1)?;
        put_u8(&mut output, SERIES_ACTION_OFFSET, self.action as u8)?;
        put_identity(&mut output, SERIES_RELEASE_SET_OFFSET, self.release_set)?;
        put_identity(&mut output, SERIES_TEMPLATE_OFFSET, self.template)?;
        put_optional_identity(&mut output, SERIES_TICKET_OFFSET, self.ticket)?;
        put_optional_identity(&mut output, SERIES_MARKET_OFFSET, self.market)?;
        put_optional_identity(&mut output, SERIES_REALM_OFFSET, self.realm)?;
        put_optional_identity(&mut output, SERIES_PRODUCT_OFFSET, self.product)?;
        put_identity(&mut output, SERIES_BENEFICIARY_OFFSET, self.beneficiary)?;
        put_optional_identity(&mut output, SERIES_FOUNDER_OFFSET, self.founder)?;
        put_u32(&mut output, SERIES_OCCURRENCE_OFFSET, self.occurrence)?;
        put_u64(
            &mut output,
            SERIES_EXPECTED_SERIES_REVISION_OFFSET,
            self.expected_series_revision,
        )?;
        put_u64(
            &mut output,
            SERIES_EXPECTED_TICKET_REVISION_OFFSET,
            self.expected_ticket_revision,
        )?;
        put_u64(&mut output, SERIES_MARKET_RENT_OFFSET, self.market_rent)?;
        put_u64(
            &mut output,
            SERIES_CAPABILITY_RENT_OFFSET,
            self.capability_rent,
        )?;
        put_u64(&mut output, SERIES_WORK_OFFSET, self.work)?;
        put_u64(
            &mut output,
            SERIES_HOARD_PRINCIPAL_OFFSET,
            self.hoard_principal,
        )?;
        put_u64(
            &mut output,
            SERIES_CLOSE_RENT_OFFSET,
            self.series_close_rent,
        )?;
        Ok(output)
    }

    /// Selected Series action.
    #[must_use]
    pub const fn action(self) -> SeriesCoreActionV1 {
        self.action
    }
    /// Immutable release set.
    #[must_use]
    pub const fn release_set(self) -> Identity {
        self.release_set
    }
    /// Exact Series template.
    #[must_use]
    pub const fn template(self) -> Identity {
        self.template
    }
    /// Exact occurrence ticket, absent only for Close.
    #[must_use]
    pub const fn ticket(self) -> Option<Identity> {
        self.ticket
    }
    /// Derived Market, absent only for Close.
    #[must_use]
    pub const fn market(self) -> Option<Identity> {
        self.market
    }
    /// Immutable Realm, absent only for Close.
    #[must_use]
    pub const fn realm(self) -> Option<Identity> {
        self.realm
    }
    /// Immutable Product, absent only for Close.
    #[must_use]
    pub const fn product(self) -> Option<Identity> {
        self.product
    }
    /// Persisted refund or RentCredit beneficiary.
    #[must_use]
    pub const fn beneficiary(self) -> Identity {
        self.beneficiary
    }
    /// Persisted Market founder, absent only for Close.
    #[must_use]
    pub const fn founder(self) -> Option<Identity> {
        self.founder
    }
    /// Selected occurrence index.
    #[must_use]
    pub const fn occurrence_index(self) -> u32 {
        self.occurrence
    }
    /// Expected Series replay revision.
    #[must_use]
    pub const fn expected_series_revision(self) -> u64 {
        self.expected_series_revision
    }
    /// Expected ticket replay revision, zero only for Close.
    #[must_use]
    pub const fn expected_ticket_revision(self) -> u64 {
        self.expected_ticket_revision
    }
    /// Exact prepaid Market rent.
    #[must_use]
    pub const fn market_rent(self) -> u64 {
        self.market_rent
    }
    /// Exact prepaid capability-account rent.
    #[must_use]
    pub const fn capability_rent(self) -> u64 {
        self.capability_rent
    }
    /// Exact prepaid initial work capital.
    #[must_use]
    pub const fn work(self) -> u64 {
        self.work
    }
    /// Exact positive Ticket-owned collateral routed only to Hoard principal.
    #[must_use]
    pub const fn hoard_principal(self) -> u64 {
        self.hoard_principal
    }
    /// Exact terminal Series close rent.
    #[must_use]
    pub const fn series_close_rent(self) -> u64 {
        self.series_close_rent
    }
}

fn exact_len(input: &[u8], expected: usize) -> Result<(), Error> {
    if input.len() != expected {
        return Err(Error::InvalidLength);
    }
    Ok(())
}

fn exact_magic(input: &[u8], offset: usize, expected: &[u8]) -> Result<(), Error> {
    if input.get(offset..offset.saturating_add(expected.len())) != Some(expected) {
        return Err(Error::InvalidMagic);
    }
    Ok(())
}

fn require_zero(input: &[u8], offset: usize, width: usize) -> Result<(), Error> {
    let bytes = input
        .get(offset..offset.saturating_add(width))
        .ok_or(Error::InvalidLength)?;
    if bytes.iter().any(|byte| *byte != 0) {
        return Err(Error::NonzeroReserved);
    }
    Ok(())
}

fn read_u8(input: &[u8], offset: usize) -> Result<u8, Error> {
    input.get(offset).copied().ok_or(Error::InvalidLength)
}

fn read_u16(input: &[u8], offset: usize) -> Result<u16, Error> {
    Ok(u16::from_le_bytes(read_array(input, offset)?))
}

fn read_u32(input: &[u8], offset: usize) -> Result<u32, Error> {
    Ok(u32::from_le_bytes(read_array(input, offset)?))
}

fn read_u64(input: &[u8], offset: usize) -> Result<u64, Error> {
    Ok(u64::from_le_bytes(read_array(input, offset)?))
}

fn read_identity(input: &[u8], offset: usize) -> Result<Identity, Error> {
    Identity::new(read_array(input, offset)?)
}

fn read_array<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N], Error> {
    input
        .get(offset..offset.saturating_add(N))
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn put(output: &mut [u8], offset: usize, bytes: &[u8]) -> Result<(), Error> {
    output
        .get_mut(offset..offset.saturating_add(bytes.len()))
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(bytes);
    Ok(())
}

fn put_u8(output: &mut [u8], offset: usize, value: u8) -> Result<(), Error> {
    let destination = output.get_mut(offset).ok_or(Error::InvalidLength)?;
    *destination = value;
    Ok(())
}

fn put_u16(output: &mut [u8], offset: usize, value: u16) -> Result<(), Error> {
    put(output, offset, &value.to_le_bytes())
}

fn put_u32(output: &mut [u8], offset: usize, value: u32) -> Result<(), Error> {
    put(output, offset, &value.to_le_bytes())
}

fn put_u64(output: &mut [u8], offset: usize, value: u64) -> Result<(), Error> {
    put(output, offset, &value.to_le_bytes())
}

fn put_identity(output: &mut [u8], offset: usize, value: Identity) -> Result<(), Error> {
    put(output, offset, &value.to_bytes())
}

fn put_optional_identity(
    output: &mut [u8],
    offset: usize,
    value: Option<Identity>,
) -> Result<(), Error> {
    let bytes = value.map_or(ZERO_IDENTITY, Identity::to_bytes);
    put(output, offset, &bytes)
}

const fn role_tag(role: Role) -> u8 {
    match role {
        Role::Core => 0,
        Role::Claims => 1,
        Role::Trading => 2,
        Role::Resolution => 3,
        Role::Custody => 4,
    }
}

fn decode_role(tag: u8) -> Result<Role, Error> {
    match tag {
        0 => Ok(Role::Core),
        1 => Ok(Role::Claims),
        2 => Ok(Role::Trading),
        3 => Ok(Role::Resolution),
        4 => Ok(Role::Custody),
        _ => Err(Error::InvalidTag),
    }
}
