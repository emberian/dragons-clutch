use crate::{Error, Result, generated_series as generated};

/// Exact physical account or content identity.
pub type Identity = [u8; 32];

/// Series lifecycle phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Phase {
    /// More occurrence tickets may be consumed or expired.
    Active,
    /// Every configured occurrence is final; close is permitted.
    Terminal,
    /// Series close rent has been returned.
    Closed,
}

impl Phase {
    pub(crate) fn decode(tag: u8) -> Result<Self> {
        match tag {
            generated::PHASE_ACTIVE => Ok(Self::Active),
            generated::PHASE_TERMINAL => Ok(Self::Terminal),
            generated::PHASE_CLOSED => Ok(Self::Closed),
            _ => Err(Error::UnknownTag),
        }
    }

    pub(crate) const fn tag(self) -> u8 {
        match self {
            Self::Active => generated::PHASE_ACTIVE,
            Self::Terminal => generated::PHASE_TERMINAL,
            Self::Closed => generated::PHASE_CLOSED,
        }
    }
}

/// Occurrence-ticket phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TicketPhase {
    /// Exact prepaid compartments remain available.
    Ready,
    /// Ticket was atomically consumed into its committed Market.
    Consumed,
    /// Retry window elapsed and all funds were returned.
    Expired,
}

impl TicketPhase {
    pub(crate) fn decode(tag: u8) -> Result<Self> {
        match tag {
            generated::TICKET_READY => Ok(Self::Ready),
            generated::TICKET_CONSUMED => Ok(Self::Consumed),
            generated::TICKET_EXPIRED => Ok(Self::Expired),
            _ => Err(Error::UnknownTag),
        }
    }

    pub(crate) const fn tag(self) -> u8 {
        match self {
            Self::Ready => generated::TICKET_READY,
            Self::Consumed => generated::TICKET_CONSUMED,
            Self::Expired => generated::TICKET_EXPIRED,
        }
    }

    pub(crate) const fn is_final(self) -> bool {
        matches!(self, Self::Consumed | Self::Expired)
    }
}

/// Transition action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    /// Atomically consume a ticket into one Market.
    Consume,
    /// Refund a ticket after its retry window.
    Expire,
    /// Return separately funded Series close rent.
    Close,
}

impl Action {
    fn decode(tag: u8) -> Result<Self> {
        match tag {
            generated::ACTION_CONSUME => Ok(Self::Consume),
            generated::ACTION_EXPIRE => Ok(Self::Expire),
            generated::ACTION_CLOSE => Ok(Self::Close),
            _ => Err(Error::UnknownTag),
        }
    }

    const fn tag(self) -> u8 {
        match self {
            Self::Consume => generated::ACTION_CONSUME,
            Self::Expire => generated::ACTION_EXPIRE,
            Self::Close => generated::ACTION_CLOSE,
        }
    }
}

/// Immutable recurring-Market template.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TemplateV1 {
    /// Content identity of this exact template.
    pub template_id: Identity,
    /// Immutable Realm selecting collateral and policy.
    pub realm_id: Identity,
    /// Product result-domain identity.
    pub product_id: Identity,
    /// Selected execution release set.
    pub release_set_id: Identity,
    /// Recipient of separately funded terminal close rent.
    pub series_refund_owner: Identity,
    /// Product outcome width.
    pub outcome_count: u32,
    /// Finite number of scheduled occurrences.
    pub occurrence_count: u32,
    /// Slot of occurrence zero.
    pub first_occurrence_slot: u64,
    /// Exact positive slot period.
    pub period_slots: u64,
    /// Inclusive retry window after each due slot.
    pub retry_window_slots: u64,
    /// Initial complete-set quantity and Hoard principal.
    pub seed_quantity: u64,
    /// Exact Market account rent compartment.
    pub market_rent_lamports: u64,
    /// Exact capability-account rent compartment.
    pub capability_rent_lamports: u64,
    /// Permissionless founding-work compartment.
    pub founding_work_lamports: u64,
    /// Separately funded Series close-rent compartment.
    pub series_close_rent_lamports: u64,
}

impl TemplateV1 {
    /// Decode one exact canonical template.
    pub fn decode(input: &[u8]) -> Result<Self> {
        decode_header(input, generated::TEMPLATE_BYTES, &generated::TEMPLATE_MAGIC)?;
        require_zero(input, generated::HEADER_TAG_OFFSET, 2)?;
        let value = Self {
            template_id: identity_at(input, generated::TEMPLATE_TEMPLATE_ID_OFFSET)?,
            realm_id: identity_at(input, generated::TEMPLATE_REALM_ID_OFFSET)?,
            product_id: identity_at(input, generated::TEMPLATE_PRODUCT_ID_OFFSET)?,
            release_set_id: identity_at(input, generated::TEMPLATE_RELEASE_SET_ID_OFFSET)?,
            series_refund_owner: identity_at(
                input,
                generated::TEMPLATE_SERIES_REFUND_OWNER_OFFSET,
            )?,
            outcome_count: u32_at(input, generated::TEMPLATE_OUTCOME_COUNT_OFFSET)?,
            occurrence_count: u32_at(input, generated::TEMPLATE_OCCURRENCE_COUNT_OFFSET)?,
            first_occurrence_slot: u64_at(input, generated::TEMPLATE_FIRST_SLOT_OFFSET)?,
            period_slots: u64_at(input, generated::TEMPLATE_PERIOD_SLOTS_OFFSET)?,
            retry_window_slots: u64_at(input, generated::TEMPLATE_RETRY_WINDOW_OFFSET)?,
            seed_quantity: u64_at(input, generated::TEMPLATE_SEED_QUANTITY_OFFSET)?,
            market_rent_lamports: u64_at(input, generated::TEMPLATE_MARKET_RENT_OFFSET)?,
            capability_rent_lamports: u64_at(input, generated::TEMPLATE_CAPABILITY_RENT_OFFSET)?,
            founding_work_lamports: u64_at(input, generated::TEMPLATE_FOUNDING_WORK_OFFSET)?,
            series_close_rent_lamports: u64_at(input, generated::TEMPLATE_CLOSE_RENT_OFFSET)?,
        };
        value.validate_basic()?;
        Ok(value)
    }

    /// Encode one exact canonical template.
    pub fn to_bytes(self) -> Result<[u8; generated::TEMPLATE_BYTES]> {
        self.validate_basic()?;
        let mut out = [0; generated::TEMPLATE_BYTES];
        encode_header(&mut out, &generated::TEMPLATE_MAGIC, 0)?;
        put(
            &mut out,
            generated::TEMPLATE_TEMPLATE_ID_OFFSET,
            &self.template_id,
        )?;
        put(
            &mut out,
            generated::TEMPLATE_REALM_ID_OFFSET,
            &self.realm_id,
        )?;
        put(
            &mut out,
            generated::TEMPLATE_PRODUCT_ID_OFFSET,
            &self.product_id,
        )?;
        put(
            &mut out,
            generated::TEMPLATE_RELEASE_SET_ID_OFFSET,
            &self.release_set_id,
        )?;
        put(
            &mut out,
            generated::TEMPLATE_SERIES_REFUND_OWNER_OFFSET,
            &self.series_refund_owner,
        )?;
        put_u32(
            &mut out,
            generated::TEMPLATE_OUTCOME_COUNT_OFFSET,
            self.outcome_count,
        )?;
        put_u32(
            &mut out,
            generated::TEMPLATE_OCCURRENCE_COUNT_OFFSET,
            self.occurrence_count,
        )?;
        put_u64(
            &mut out,
            generated::TEMPLATE_FIRST_SLOT_OFFSET,
            self.first_occurrence_slot,
        )?;
        put_u64(
            &mut out,
            generated::TEMPLATE_PERIOD_SLOTS_OFFSET,
            self.period_slots,
        )?;
        put_u64(
            &mut out,
            generated::TEMPLATE_RETRY_WINDOW_OFFSET,
            self.retry_window_slots,
        )?;
        put_u64(
            &mut out,
            generated::TEMPLATE_SEED_QUANTITY_OFFSET,
            self.seed_quantity,
        )?;
        put_u64(
            &mut out,
            generated::TEMPLATE_MARKET_RENT_OFFSET,
            self.market_rent_lamports,
        )?;
        put_u64(
            &mut out,
            generated::TEMPLATE_CAPABILITY_RENT_OFFSET,
            self.capability_rent_lamports,
        )?;
        put_u64(
            &mut out,
            generated::TEMPLATE_FOUNDING_WORK_OFFSET,
            self.founding_work_lamports,
        )?;
        put_u64(
            &mut out,
            generated::TEMPLATE_CLOSE_RENT_OFFSET,
            self.series_close_rent_lamports,
        )?;
        Ok(out)
    }

    pub(crate) fn validate_basic(self) -> Result<()> {
        require_identities(&[
            self.template_id,
            self.realm_id,
            self.product_id,
            self.release_set_id,
            self.series_refund_owner,
        ])?;
        if self.outcome_count == 0
            || self.occurrence_count == 0
            || self.period_slots == 0
            || self.seed_quantity == 0
        {
            return Err(Error::ZeroQuantity);
        }
        self.seed_quantity
            .checked_add(self.market_rent_lamports)
            .and_then(|v| v.checked_add(self.capability_rent_lamports))
            .and_then(|v| v.checked_add(self.founding_work_lamports))
            .ok_or(Error::ArithmeticOverflow)?;
        Ok(())
    }
}

/// Replay-owned Series cursor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesStateV1 {
    /// Series account identity.
    pub series_id: Identity,
    /// Immutable template identity.
    pub template_id: Identity,
    /// Lifecycle phase.
    pub phase: Phase,
    /// Next occurrence that may be consumed or expired.
    pub next_occurrence: u32,
    /// Optimistic replay revision.
    pub revision: u64,
    /// Separately funded close rent remaining.
    pub close_rent_lamports: u64,
}

impl SeriesStateV1 {
    /// Decode one canonical Series cursor.
    pub fn decode(input: &[u8]) -> Result<Self> {
        decode_header(input, generated::SERIES_BYTES, &generated::SERIES_MAGIC)?;
        require_zero(input, generated::HEADER_RESERVED_OFFSET, 1)?;
        require_zero(input, generated::SERIES_RESERVED_BODY_OFFSET, 4)?;
        let value = Self {
            series_id: identity_at(input, generated::SERIES_SERIES_ID_OFFSET)?,
            template_id: identity_at(input, generated::SERIES_TEMPLATE_ID_OFFSET)?,
            phase: Phase::decode(byte_at(input, generated::HEADER_TAG_OFFSET)?)?,
            next_occurrence: u32_at(input, generated::SERIES_NEXT_OCCURRENCE_OFFSET)?,
            revision: u64_at(input, generated::SERIES_REVISION_OFFSET)?,
            close_rent_lamports: u64_at(input, generated::SERIES_CLOSE_RENT_OFFSET)?,
        };
        require_identities(&[value.series_id, value.template_id])?;
        Ok(value)
    }

    /// Encode one canonical Series cursor.
    pub fn to_bytes(self) -> Result<[u8; generated::SERIES_BYTES]> {
        require_identities(&[self.series_id, self.template_id])?;
        let mut out = [0; generated::SERIES_BYTES];
        encode_header(&mut out, &generated::SERIES_MAGIC, self.phase.tag())?;
        put(
            &mut out,
            generated::SERIES_SERIES_ID_OFFSET,
            &self.series_id,
        )?;
        put(
            &mut out,
            generated::SERIES_TEMPLATE_ID_OFFSET,
            &self.template_id,
        )?;
        put_u32(
            &mut out,
            generated::SERIES_NEXT_OCCURRENCE_OFFSET,
            self.next_occurrence,
        )?;
        put_u64(&mut out, generated::SERIES_REVISION_OFFSET, self.revision)?;
        put_u64(
            &mut out,
            generated::SERIES_CLOSE_RENT_OFFSET,
            self.close_rent_lamports,
        )?;
        Ok(out)
    }
}

/// Four exact occurrence-ticket compartments.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TicketFundsV1 {
    /// Initial Market Hoard principal.
    pub hoard_principal: u64,
    /// Market account rent.
    pub market_rent: u64,
    /// Capability account rent.
    pub capability_rent: u64,
    /// Permissionless founding work.
    pub founding_work: u64,
}

impl TicketFundsV1 {
    pub(crate) const fn is_zero(self) -> bool {
        self.hoard_principal == 0
            && self.market_rent == 0
            && self.capability_rent == 0
            && self.founding_work == 0
    }
}

/// One independently prepaid occurrence ticket.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TicketV1 {
    /// Ticket account identity.
    pub ticket_id: Identity,
    /// Immutable template identity.
    pub template_id: Identity,
    /// Initial complete-set claimant.
    pub founder: Identity,
    /// Recipient after expiry.
    pub refund_owner: Identity,
    /// Precommitted Market account identity.
    pub committed_market_id: Identity,
    /// Scheduled zero-based occurrence.
    pub occurrence: u32,
    /// Optimistic replay revision.
    pub revision: u64,
    /// Ticket lifecycle phase.
    pub phase: TicketPhase,
    /// Remaining exact funding compartments.
    pub funds: TicketFundsV1,
}

impl TicketV1 {
    /// Decode one canonical occurrence ticket.
    pub fn decode(input: &[u8]) -> Result<Self> {
        decode_header(input, generated::TICKET_BYTES, &generated::TICKET_MAGIC)?;
        require_zero(input, generated::HEADER_RESERVED_OFFSET, 1)?;
        require_zero(input, generated::TICKET_RESERVED_BODY_OFFSET, 4)?;
        let value = Self {
            ticket_id: identity_at(input, generated::TICKET_TICKET_ID_OFFSET)?,
            template_id: identity_at(input, generated::TICKET_TEMPLATE_ID_OFFSET)?,
            founder: identity_at(input, generated::TICKET_FOUNDER_OFFSET)?,
            refund_owner: identity_at(input, generated::TICKET_REFUND_OWNER_OFFSET)?,
            committed_market_id: identity_at(input, generated::TICKET_MARKET_ID_OFFSET)?,
            occurrence: u32_at(input, generated::TICKET_OCCURRENCE_OFFSET)?,
            revision: u64_at(input, generated::TICKET_REVISION_OFFSET)?,
            phase: TicketPhase::decode(byte_at(input, generated::HEADER_TAG_OFFSET)?)?,
            funds: TicketFundsV1 {
                hoard_principal: u64_at(input, generated::TICKET_HOARD_OFFSET)?,
                market_rent: u64_at(input, generated::TICKET_MARKET_RENT_OFFSET)?,
                capability_rent: u64_at(input, generated::TICKET_CAPABILITY_RENT_OFFSET)?,
                founding_work: u64_at(input, generated::TICKET_FOUNDING_WORK_OFFSET)?,
            },
        };
        require_identities(&[
            value.ticket_id,
            value.template_id,
            value.founder,
            value.refund_owner,
            value.committed_market_id,
        ])?;
        Ok(value)
    }

    /// Encode one canonical occurrence ticket.
    pub fn to_bytes(self) -> Result<[u8; generated::TICKET_BYTES]> {
        require_identities(&[
            self.ticket_id,
            self.template_id,
            self.founder,
            self.refund_owner,
            self.committed_market_id,
        ])?;
        let mut out = [0; generated::TICKET_BYTES];
        encode_header(&mut out, &generated::TICKET_MAGIC, self.phase.tag())?;
        put(
            &mut out,
            generated::TICKET_TICKET_ID_OFFSET,
            &self.ticket_id,
        )?;
        put(
            &mut out,
            generated::TICKET_TEMPLATE_ID_OFFSET,
            &self.template_id,
        )?;
        put(&mut out, generated::TICKET_FOUNDER_OFFSET, &self.founder)?;
        put(
            &mut out,
            generated::TICKET_REFUND_OWNER_OFFSET,
            &self.refund_owner,
        )?;
        put(
            &mut out,
            generated::TICKET_MARKET_ID_OFFSET,
            &self.committed_market_id,
        )?;
        put_u32(
            &mut out,
            generated::TICKET_OCCURRENCE_OFFSET,
            self.occurrence,
        )?;
        put_u64(&mut out, generated::TICKET_REVISION_OFFSET, self.revision)?;
        put_u64(
            &mut out,
            generated::TICKET_HOARD_OFFSET,
            self.funds.hoard_principal,
        )?;
        put_u64(
            &mut out,
            generated::TICKET_MARKET_RENT_OFFSET,
            self.funds.market_rent,
        )?;
        put_u64(
            &mut out,
            generated::TICKET_CAPABILITY_RENT_OFFSET,
            self.funds.capability_rent,
        )?;
        put_u64(
            &mut out,
            generated::TICKET_FOUNDING_WORK_OFFSET,
            self.funds.founding_work,
        )?;
        Ok(out)
    }
}

/// Normalized exact current Registry/Core release receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReleaseReceiptV1 {
    /// Registry/Core program that authenticated the activation.
    pub registry_program: Identity,
    /// Selected release-set content identity.
    pub release_set_id: Identity,
    /// Currently observed Core Program.
    pub observed_program: Identity,
    /// Current checked artifact-release identity.
    pub artifact_release: Identity,
    /// Current semantic-release identity.
    pub semantic_release: Identity,
}

impl ReleaseReceiptV1 {
    /// Decode the only accepted Core receipt shape.
    pub fn decode(input: &[u8]) -> Result<Self> {
        decode_header(input, generated::RECEIPT_BYTES, &generated::RECEIPT_MAGIC)?;
        if byte_at(input, generated::HEADER_TAG_OFFSET)? != generated::CORE_ROLE_TAG
            || byte_at(input, generated::HEADER_RESERVED_OFFSET)?
                != generated::RECEIPT_AUTHENTICATED_FLAGS
        {
            return Err(Error::UnknownTag);
        }
        let value = Self {
            registry_program: identity_at(input, generated::RECEIPT_REGISTRY_PROGRAM_OFFSET)?,
            release_set_id: identity_at(input, generated::RECEIPT_RELEASE_SET_ID_OFFSET)?,
            observed_program: identity_at(input, generated::RECEIPT_OBSERVED_PROGRAM_OFFSET)?,
            artifact_release: identity_at(input, generated::RECEIPT_ARTIFACT_RELEASE_OFFSET)?,
            semantic_release: identity_at(input, generated::RECEIPT_SEMANTIC_RELEASE_OFFSET)?,
        };
        require_identities(&[
            value.registry_program,
            value.release_set_id,
            value.observed_program,
            value.artifact_release,
            value.semantic_release,
        ])?;
        Ok(value)
    }

    /// Encode one normalized authenticated Core receipt.
    pub fn to_bytes(self) -> Result<[u8; generated::RECEIPT_BYTES]> {
        require_identities(&[
            self.registry_program,
            self.release_set_id,
            self.observed_program,
            self.artifact_release,
            self.semantic_release,
        ])?;
        let mut out = [0; generated::RECEIPT_BYTES];
        encode_header(
            &mut out,
            &generated::RECEIPT_MAGIC,
            generated::CORE_ROLE_TAG,
        )?;
        out[generated::HEADER_RESERVED_OFFSET] = generated::RECEIPT_AUTHENTICATED_FLAGS;
        put(
            &mut out,
            generated::RECEIPT_REGISTRY_PROGRAM_OFFSET,
            &self.registry_program,
        )?;
        put(
            &mut out,
            generated::RECEIPT_RELEASE_SET_ID_OFFSET,
            &self.release_set_id,
        )?;
        put(
            &mut out,
            generated::RECEIPT_OBSERVED_PROGRAM_OFFSET,
            &self.observed_program,
        )?;
        put(
            &mut out,
            generated::RECEIPT_ARTIFACT_RELEASE_OFFSET,
            &self.artifact_release,
        )?;
        put(
            &mut out,
            generated::RECEIPT_SEMANTIC_RELEASE_OFFSET,
            &self.semantic_release,
        )?;
        Ok(out)
    }
}

/// Optimistic transition request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RequestV1 {
    /// Consume, expire, or close.
    pub action: Action,
    /// Current observed slot.
    pub now_slot: u64,
    /// Exact Series revision expected.
    pub expected_series_revision: u64,
    /// Exact Ticket revision expected.
    pub expected_ticket_revision: u64,
    /// Permissionless work recipient; zero for expire and close.
    pub work_recipient: Identity,
}

impl RequestV1 {
    /// Decode one canonical transition request.
    pub fn decode(input: &[u8]) -> Result<Self> {
        decode_header(input, generated::REQUEST_BYTES, &generated::REQUEST_MAGIC)?;
        require_zero(input, generated::HEADER_RESERVED_OFFSET, 1)?;
        let value = Self {
            action: Action::decode(byte_at(input, generated::HEADER_TAG_OFFSET)?)?,
            now_slot: u64_at(input, generated::REQUEST_NOW_SLOT_OFFSET)?,
            expected_series_revision: u64_at(input, generated::REQUEST_SERIES_REVISION_OFFSET)?,
            expected_ticket_revision: u64_at(input, generated::REQUEST_TICKET_REVISION_OFFSET)?,
            work_recipient: identity_at(input, generated::REQUEST_WORK_RECIPIENT_OFFSET)?,
        };
        value.validate_recipient()?;
        Ok(value)
    }

    /// Encode one canonical transition request.
    pub fn to_bytes(self) -> Result<[u8; generated::REQUEST_BYTES]> {
        self.validate_recipient()?;
        let mut out = [0; generated::REQUEST_BYTES];
        encode_header(&mut out, &generated::REQUEST_MAGIC, self.action.tag())?;
        put_u64(&mut out, generated::REQUEST_NOW_SLOT_OFFSET, self.now_slot)?;
        put_u64(
            &mut out,
            generated::REQUEST_SERIES_REVISION_OFFSET,
            self.expected_series_revision,
        )?;
        put_u64(
            &mut out,
            generated::REQUEST_TICKET_REVISION_OFFSET,
            self.expected_ticket_revision,
        )?;
        put(
            &mut out,
            generated::REQUEST_WORK_RECIPIENT_OFFSET,
            &self.work_recipient,
        )?;
        Ok(out)
    }

    fn validate_recipient(self) -> Result<()> {
        match self.action {
            Action::Consume if is_zero(&self.work_recipient) => Err(Error::RecipientRefusal),
            Action::Expire | Action::Close if !is_zero(&self.work_recipient) => {
                Err(Error::RecipientRefusal)
            }
            _ => Ok(()),
        }
    }
}

fn decode_header(input: &[u8], width: usize, magic: &[u8; 4]) -> Result<()> {
    if input.len() != width {
        return Err(Error::InvalidLength);
    }
    if input.get(..4) != Some(magic.as_slice()) {
        return Err(Error::InvalidMagic);
    }
    if u16_at(input, generated::HEADER_VERSION_OFFSET)? != generated::ABI_VERSION {
        return Err(Error::UnsupportedVersion);
    }
    Ok(())
}

fn encode_header<const N: usize>(out: &mut [u8; N], magic: &[u8; 4], tag: u8) -> Result<()> {
    put(out, 0, magic)?;
    put(
        out,
        generated::HEADER_VERSION_OFFSET,
        &generated::ABI_VERSION.to_le_bytes(),
    )?;
    out[generated::HEADER_TAG_OFFSET] = tag;
    Ok(())
}

pub(crate) const fn is_zero(identity: &Identity) -> bool {
    let mut index = 0;
    while index < identity.len() {
        if identity[index] != 0 {
            return false;
        }
        index += 1;
    }
    true
}

fn require_identities(identities: &[Identity]) -> Result<()> {
    if identities.iter().any(is_zero) {
        Err(Error::ZeroIdentity)
    } else {
        Ok(())
    }
}

fn byte_at(input: &[u8], offset: usize) -> Result<u8> {
    input.get(offset).copied().ok_or(Error::InvalidLength)
}

fn identity_at(input: &[u8], offset: usize) -> Result<Identity> {
    input
        .get(offset..offset + 32)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or(Error::InvalidLength)
}

fn u16_at(input: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(
        input
            .get(offset..offset + 2)
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or(Error::InvalidLength)?,
    ))
}

fn u32_at(input: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        input
            .get(offset..offset + 4)
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or(Error::InvalidLength)?,
    ))
}

fn u64_at(input: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(
        input
            .get(offset..offset + 8)
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or(Error::InvalidLength)?,
    ))
}

fn require_zero(input: &[u8], offset: usize, width: usize) -> Result<()> {
    let bytes = input
        .get(offset..offset + width)
        .ok_or(Error::InvalidLength)?;
    if bytes.iter().any(|byte| *byte != 0) {
        Err(Error::NonCanonicalReserved)
    } else {
        Ok(())
    }
}

fn put<const N: usize>(output: &mut [u8; N], offset: usize, value: &[u8]) -> Result<()> {
    output
        .get_mut(offset..offset + value.len())
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(value);
    Ok(())
}

fn put_u32<const N: usize>(output: &mut [u8; N], offset: usize, value: u32) -> Result<()> {
    put(output, offset, &value.to_le_bytes())
}

fn put_u64<const N: usize>(output: &mut [u8; N], offset: usize, value: u64) -> Result<()> {
    put(output, offset, &value.to_le_bytes())
}
