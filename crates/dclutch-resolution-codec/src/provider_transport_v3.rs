//! Exact transport lifecycle for provider-owned Pyth update accounts.
//!
//! Posting, terminal consumption, and reclaim are deliberately separate
//! transitions. The Receiver update's write authority is a Resolution PDA so
//! no submitter can reclaim evidence before dClutch has consumed it, while any
//! resolver may trigger reclaim after the persisted lifecycle becomes
//! `Consumed`. Update rent always returns to the immutable submitter-selected
//! refund recipient.

use crate::{Error, Result};

/// Exact submitted/consumed lifecycle account width.
pub const PROVIDER_UPDATE_LIFECYCLE_BYTES_V3: usize = 528;
/// Exact provider submission request prefix width before `PostUpdateParams`.
pub const PROVIDER_SUBMIT_REQUEST_BYTES_V3: usize = 416;
/// Exact permissionless reclaim request width.
pub const PROVIDER_RECLAIM_REQUEST_BYTES_V3: usize = 288;
/// Exact provider submission return receipt width.
pub const PROVIDER_SUBMIT_RECEIPT_BYTES_V3: usize = 400;
/// Exact provider reclaim return receipt width.
pub const PROVIDER_RECLAIM_RECEIPT_BYTES_V3: usize = 304;
/// Lifecycle wire magic.
pub const PROVIDER_UPDATE_LIFECYCLE_MAGIC_V3: [u8; 8] = *b"DCLTPUL3";
/// Submission request magic.
pub const PROVIDER_SUBMIT_REQUEST_MAGIC_V3: [u8; 8] = *b"DCLTPSB3";
/// Reclaim request magic.
pub const PROVIDER_RECLAIM_REQUEST_MAGIC_V3: [u8; 8] = *b"DCLTPRL3";
/// Submission receipt magic.
pub const PROVIDER_SUBMIT_RECEIPT_MAGIC_V3: [u8; 8] = *b"DCLTPSR3";
/// Reclaim receipt magic.
pub const PROVIDER_RECLAIM_RECEIPT_MAGIC_V3: [u8; 8] = *b"DCLTPRR3";
/// Shared transport schema version.
pub const PROVIDER_TRANSPORT_VERSION_V3: u16 = 3;
/// Resolution-owned lifecycle PDA domain.
pub const PROVIDER_UPDATE_LIFECYCLE_PDA_DOMAIN_V3: &[u8] = b"dclutch/provider-life/v3";
/// Resolution-owned Receiver update-authority PDA domain.
pub const PROVIDER_UPDATE_AUTHORITY_PDA_DOMAIN_V3: &[u8] = b"dclutch/provider-update/v3";

const _: () = assert!(PROVIDER_UPDATE_LIFECYCLE_PDA_DOMAIN_V3.len() <= 32);
const _: () = assert!(PROVIDER_UPDATE_AUTHORITY_PDA_DOMAIN_V3.len() <= 32);

const IDENTITIES_OFFSET: usize = 32;
const LIFECYCLE_IDENTITY_COUNT: usize = 14;

/// Persisted update lifecycle phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ProviderUpdateStatusV3 {
    /// Receiver posted a fully verified update that has not been consumed.
    Submitted = 1,
    /// Resolution consumed the update into a terminal Source certificate.
    Consumed = 2,
}

impl ProviderUpdateStatusV3 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Submitted),
            2 => Ok(Self::Consumed),
            _ => Err(Error::UnknownAction),
        }
    }
}

/// Exact request to post one real Receiver update under protocol custody.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderSubmitRequestV3 {
    /// Immutable Market generation.
    pub generation: u64,
    /// Earliest Unix second at which reclaim is allowed after consumption.
    pub reclaim_after_unix_seconds: i64,
    /// Core Market.
    pub market: [u8; 32],
    /// Current Source state.
    pub source_state: [u8; 32],
    /// Deterministic lifecycle destination.
    pub lifecycle: [u8; 32],
    /// Market-selected SourceMaterialV2 identity.
    pub source_material: [u8; 32],
    /// Source-selected Pyth release identity.
    pub provider_release: [u8; 32],
    /// Vacant signing Receiver PriceUpdate destination.
    pub update_account: [u8; 32],
    /// Provider submitter paying update rent and provider fee.
    pub provider_submitter: [u8; 32],
    /// Immutable rent-refund recipient.
    pub refund_recipient: [u8; 32],
    /// Market-selected release set.
    pub release_set: [u8; 32],
    /// Immutable Registry program authenticated when the provider post landed.
    pub registry_program: [u8; 32],
    /// Router-owned verified EncodedVaa account.
    pub encoded_vaa: [u8; 32],
    /// SHA-256 of the exact appended PostUpdateParams body.
    pub post_body_digest: [u8; 32],
}

impl ProviderSubmitRequestV3 {
    /// Decode one exact canonical submission request prefix.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        require_header(
            bytes,
            PROVIDER_SUBMIT_REQUEST_BYTES_V3,
            PROVIDER_SUBMIT_REQUEST_MAGIC_V3,
            1,
        )?;
        let value = Self {
            generation: read_u64(bytes, 16)?,
            reclaim_after_unix_seconds: read_i64(bytes, 24)?,
            market: array(bytes, 32)?,
            source_state: array(bytes, 64)?,
            lifecycle: array(bytes, 96)?,
            source_material: array(bytes, 128)?,
            provider_release: array(bytes, 160)?,
            update_account: array(bytes, 192)?,
            provider_submitter: array(bytes, 224)?,
            refund_recipient: array(bytes, 256)?,
            release_set: array(bytes, 288)?,
            registry_program: array(bytes, 320)?,
            encoded_vaa: array(bytes, 352)?,
            post_body_digest: array(bytes, 384)?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode one exact canonical submission request prefix.
    pub fn to_bytes(self) -> Result<[u8; PROVIDER_SUBMIT_REQUEST_BYTES_V3]> {
        self.validate()?;
        let mut bytes = [0; PROVIDER_SUBMIT_REQUEST_BYTES_V3];
        write_header(&mut bytes, PROVIDER_SUBMIT_REQUEST_MAGIC_V3, 1)?;
        put(&mut bytes, 16, &self.generation.to_le_bytes())?;
        put(
            &mut bytes,
            24,
            &self.reclaim_after_unix_seconds.to_le_bytes(),
        )?;
        for (index, identity) in self.identities().iter().enumerate() {
            put(&mut bytes, 32 + index * 32, identity)?;
        }
        Ok(bytes)
    }

    fn identities(self) -> [[u8; 32]; 12] {
        [
            self.market,
            self.source_state,
            self.lifecycle,
            self.source_material,
            self.provider_release,
            self.update_account,
            self.provider_submitter,
            self.refund_recipient,
            self.release_set,
            self.registry_program,
            self.encoded_vaa,
            self.post_body_digest,
        ]
    }

    fn validate(self) -> Result<()> {
        if self.generation == 0
            || self.reclaim_after_unix_seconds <= 0
            || self.identities().iter().any(is_zero)
            || self.provider_submitter == self.refund_recipient
            || self.provider_submitter == self.update_account
            || self.update_account == self.lifecycle
        {
            return Err(Error::ZeroCoordinate);
        }
        Ok(())
    }
}

/// Exact request to reclaim a consumed Receiver update.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderReclaimRequestV3 {
    /// Immutable Market generation.
    pub generation: u64,
    /// Exact terminal sequence already stored in lifecycle and certificate.
    pub terminal_sequence: u64,
    /// Core Market.
    pub market: [u8; 32],
    /// Terminal Source state.
    pub source_state: [u8; 32],
    /// Consumed lifecycle account.
    pub lifecycle: [u8; 32],
    /// Terminal certificate.
    pub certificate: [u8; 32],
    /// Receiver update to close.
    pub update_account: [u8; 32],
    /// Permissionless reclaim transaction signer.
    pub resolver: [u8; 32],
    /// Immutable lifecycle refund recipient.
    pub refund_recipient: [u8; 32],
    /// Market-selected release set.
    pub release_set: [u8; 32],
}

impl ProviderReclaimRequestV3 {
    /// Decode one exact canonical reclaim request.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        require_header(
            bytes,
            PROVIDER_RECLAIM_REQUEST_BYTES_V3,
            PROVIDER_RECLAIM_REQUEST_MAGIC_V3,
            2,
        )?;
        let value = Self {
            generation: read_u64(bytes, 16)?,
            terminal_sequence: read_u64(bytes, 24)?,
            market: array(bytes, 32)?,
            source_state: array(bytes, 64)?,
            lifecycle: array(bytes, 96)?,
            certificate: array(bytes, 128)?,
            update_account: array(bytes, 160)?,
            resolver: array(bytes, 192)?,
            refund_recipient: array(bytes, 224)?,
            release_set: array(bytes, 256)?,
        };
        if value.generation == 0
            || value.terminal_sequence == 0
            || value.identities().iter().any(is_zero)
            || value.resolver == value.update_account
        {
            return Err(Error::ZeroCoordinate);
        }
        Ok(value)
    }

    /// Encode one exact canonical reclaim request.
    pub fn to_bytes(self) -> Result<[u8; PROVIDER_RECLAIM_REQUEST_BYTES_V3]> {
        if self.generation == 0
            || self.terminal_sequence == 0
            || self.identities().iter().any(is_zero)
            || self.resolver == self.update_account
        {
            return Err(Error::ZeroCoordinate);
        }
        let mut bytes = [0; PROVIDER_RECLAIM_REQUEST_BYTES_V3];
        write_header(&mut bytes, PROVIDER_RECLAIM_REQUEST_MAGIC_V3, 2)?;
        put(&mut bytes, 16, &self.generation.to_le_bytes())?;
        put(&mut bytes, 24, &self.terminal_sequence.to_le_bytes())?;
        for (index, identity) in self.identities().iter().enumerate() {
            put(&mut bytes, 32 + index * 32, identity)?;
        }
        Ok(bytes)
    }

    fn identities(self) -> [[u8; 32]; 8] {
        [
            self.market,
            self.source_state,
            self.lifecycle,
            self.certificate,
            self.update_account,
            self.resolver,
            self.refund_recipient,
            self.release_set,
        ]
    }
}

/// Persisted custody and replay state for one provider update.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderUpdateLifecycleV3 {
    /// Current lifecycle status.
    pub status: ProviderUpdateStatusV3,
    /// Lifecycle PDA bump.
    pub bump: u8,
    /// Immutable Market generation.
    pub generation: u64,
    /// Zero before consumption; exact positive terminal sequence afterward.
    pub terminal_sequence: u64,
    /// Core Market.
    pub market: [u8; 32],
    /// Source state.
    pub source_state: [u8; 32],
    /// Source material identity.
    pub source_material: [u8; 32],
    /// Pyth release identity.
    pub provider_release: [u8; 32],
    /// Receiver update account.
    pub update_account: [u8; 32],
    /// SHA-256 of exact Receiver update bytes.
    pub update_digest: [u8; 32],
    /// SHA-256 of exact PostUpdateParams body.
    pub post_body_digest: [u8; 32],
    /// Provider submitter.
    pub provider_submitter: [u8; 32],
    /// Resolution PDA retained by Receiver as write authority.
    pub update_authority: [u8; 32],
    /// Immutable update-rent refund recipient.
    pub refund_recipient: [u8; 32],
    /// Market-selected release set.
    pub release_set: [u8; 32],
    /// Immutable Registry program authenticated at submission.
    pub registry_program: [u8; 32],
    /// Zero before consumption; terminal provider evidence afterward.
    pub provider_evidence: [u8; 32],
    /// Zero before consumption; terminal certificate account afterward.
    pub certificate: [u8; 32],
    /// Provider publication time.
    pub publish_time: i64,
    /// Provider-posted slot.
    pub posted_slot: u64,
    /// Earliest permitted reclaim time.
    pub reclaim_after_unix_seconds: i64,
    /// Exact rent held by the update after submission.
    pub update_rent_lamports: u64,
    /// Exact provider fee paid at submission.
    pub provider_fee_lamports: u64,
}

impl ProviderUpdateLifecycleV3 {
    /// Construct canonical submitted state after Receiver postconditions pass.
    #[allow(clippy::too_many_arguments)]
    pub fn submitted(
        request: ProviderSubmitRequestV3,
        bump: u8,
        update_authority: [u8; 32],
        registry_program: [u8; 32],
        update_digest: [u8; 32],
        publish_time: i64,
        posted_slot: u64,
        update_rent_lamports: u64,
        provider_fee_lamports: u64,
    ) -> Result<Self> {
        let value = Self {
            status: ProviderUpdateStatusV3::Submitted,
            bump,
            generation: request.generation,
            terminal_sequence: 0,
            market: request.market,
            source_state: request.source_state,
            source_material: request.source_material,
            provider_release: request.provider_release,
            update_account: request.update_account,
            update_digest,
            post_body_digest: request.post_body_digest,
            provider_submitter: request.provider_submitter,
            update_authority,
            refund_recipient: request.refund_recipient,
            release_set: request.release_set,
            registry_program,
            provider_evidence: [0; 32],
            certificate: [0; 32],
            publish_time,
            posted_slot,
            reclaim_after_unix_seconds: request.reclaim_after_unix_seconds,
            update_rent_lamports,
            provider_fee_lamports,
        };
        value.validate()?;
        Ok(value)
    }

    /// Consume this exact update into one terminal certificate.
    pub fn consume(
        &mut self,
        terminal_sequence: u64,
        provider_evidence: [u8; 32],
        certificate: [u8; 32],
    ) -> Result<()> {
        if self.status != ProviderUpdateStatusV3::Submitted
            || terminal_sequence == 0
            || is_zero(&provider_evidence)
            || is_zero(&certificate)
        {
            return Err(Error::InvalidReceiptShape);
        }
        self.status = ProviderUpdateStatusV3::Consumed;
        self.terminal_sequence = terminal_sequence;
        self.provider_evidence = provider_evidence;
        self.certificate = certificate;
        self.validate()
    }

    /// Decode one exact canonical lifecycle account.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != PROVIDER_UPDATE_LIFECYCLE_BYTES_V3
            || array::<8>(bytes, 0)? != PROVIDER_UPDATE_LIFECYCLE_MAGIC_V3
            || read_u16(bytes, 8)? != PROVIDER_TRANSPORT_VERSION_V3
        {
            return Err(Error::InvalidLength);
        }
        require_zero(bytes, 12, 4)?;
        require_zero(bytes, 520, 8)?;
        let mut identities = [[0; 32]; LIFECYCLE_IDENTITY_COUNT];
        for (index, identity) in identities.iter_mut().enumerate() {
            *identity = array(bytes, IDENTITIES_OFFSET + index * 32)?;
        }
        let value = Self {
            status: ProviderUpdateStatusV3::decode(byte(bytes, 10)?)?,
            bump: byte(bytes, 11)?,
            generation: read_u64(bytes, 16)?,
            terminal_sequence: read_u64(bytes, 24)?,
            market: identities[0],
            source_state: identities[1],
            source_material: identities[2],
            provider_release: identities[3],
            update_account: identities[4],
            update_digest: identities[5],
            post_body_digest: identities[6],
            provider_submitter: identities[7],
            update_authority: identities[8],
            refund_recipient: identities[9],
            release_set: identities[10],
            registry_program: identities[11],
            provider_evidence: identities[12],
            certificate: identities[13],
            publish_time: read_i64(bytes, 480)?,
            posted_slot: read_u64(bytes, 488)?,
            reclaim_after_unix_seconds: read_i64(bytes, 496)?,
            update_rent_lamports: read_u64(bytes, 504)?,
            provider_fee_lamports: read_u64(bytes, 512)?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode one exact canonical lifecycle account.
    pub fn to_bytes(self) -> Result<[u8; PROVIDER_UPDATE_LIFECYCLE_BYTES_V3]> {
        self.validate()?;
        let mut bytes = [0; PROVIDER_UPDATE_LIFECYCLE_BYTES_V3];
        put(&mut bytes, 0, &PROVIDER_UPDATE_LIFECYCLE_MAGIC_V3)?;
        put(&mut bytes, 8, &PROVIDER_TRANSPORT_VERSION_V3.to_le_bytes())?;
        bytes[10] = self.status as u8;
        bytes[11] = self.bump;
        put(&mut bytes, 16, &self.generation.to_le_bytes())?;
        put(&mut bytes, 24, &self.terminal_sequence.to_le_bytes())?;
        for (index, identity) in self.identities().iter().enumerate() {
            put(&mut bytes, IDENTITIES_OFFSET + index * 32, identity)?;
        }
        put(&mut bytes, 480, &self.publish_time.to_le_bytes())?;
        put(&mut bytes, 488, &self.posted_slot.to_le_bytes())?;
        put(
            &mut bytes,
            496,
            &self.reclaim_after_unix_seconds.to_le_bytes(),
        )?;
        put(&mut bytes, 504, &self.update_rent_lamports.to_le_bytes())?;
        put(&mut bytes, 512, &self.provider_fee_lamports.to_le_bytes())?;
        Ok(bytes)
    }

    fn identities(self) -> [[u8; 32]; LIFECYCLE_IDENTITY_COUNT] {
        [
            self.market,
            self.source_state,
            self.source_material,
            self.provider_release,
            self.update_account,
            self.update_digest,
            self.post_body_digest,
            self.provider_submitter,
            self.update_authority,
            self.refund_recipient,
            self.release_set,
            self.registry_program,
            self.provider_evidence,
            self.certificate,
        ]
    }

    fn validate(self) -> Result<()> {
        if self.generation == 0
            || self.publish_time <= 0
            || self.posted_slot == 0
            || self.reclaim_after_unix_seconds < self.publish_time
            || self.update_rent_lamports == 0
            || self.identities().iter().take(12).any(is_zero)
        {
            return Err(Error::ZeroCoordinate);
        }
        match self.status {
            ProviderUpdateStatusV3::Submitted
                if self.terminal_sequence != 0
                    || !is_zero(&self.provider_evidence)
                    || !is_zero(&self.certificate) =>
            {
                Err(Error::InvalidReceiptShape)
            }
            ProviderUpdateStatusV3::Consumed
                if self.terminal_sequence == 0
                    || is_zero(&self.provider_evidence)
                    || is_zero(&self.certificate) =>
            {
                Err(Error::InvalidReceiptShape)
            }
            _ => Ok(()),
        }
    }
}

/// Typed submission receipt returned by Resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderSubmitReceiptV3 {
    /// Exact request-prefix digest.
    pub request_digest: [u8; 32],
    /// Lifecycle account.
    pub lifecycle: [u8; 32],
    /// Receiver update account.
    pub update_account: [u8; 32],
    /// Exact update digest.
    pub update_digest: [u8; 32],
    /// Provider submitter.
    pub provider_submitter: [u8; 32],
    /// Resolution-owned update authority.
    pub update_authority: [u8; 32],
    /// Refund recipient.
    pub refund_recipient: [u8; 32],
    /// Pyth release identity.
    pub provider_release: [u8; 32],
    /// Post body digest.
    pub post_body_digest: [u8; 32],
    /// Market.
    pub market: [u8; 32],
    /// Generation.
    pub generation: u64,
    /// Posted slot.
    pub posted_slot: u64,
    /// Publish time.
    pub publish_time: i64,
    /// Update rent.
    pub update_rent_lamports: u64,
    /// Provider fee.
    pub provider_fee_lamports: u64,
}

impl ProviderSubmitReceiptV3 {
    /// Decode one exact fixed submission receipt.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        require_header(
            bytes,
            PROVIDER_SUBMIT_RECEIPT_BYTES_V3,
            PROVIDER_SUBMIT_RECEIPT_MAGIC_V3,
            1,
        )?;
        require_zero(bytes, 24, 8)?;
        require_zero(bytes, 384, 16)?;
        let value = Self {
            generation: read_u64(bytes, 16)?,
            request_digest: array(bytes, 32)?,
            lifecycle: array(bytes, 64)?,
            update_account: array(bytes, 96)?,
            update_digest: array(bytes, 128)?,
            provider_submitter: array(bytes, 160)?,
            update_authority: array(bytes, 192)?,
            refund_recipient: array(bytes, 224)?,
            provider_release: array(bytes, 256)?,
            post_body_digest: array(bytes, 288)?,
            market: array(bytes, 320)?,
            posted_slot: read_u64(bytes, 352)?,
            publish_time: read_i64(bytes, 360)?,
            update_rent_lamports: read_u64(bytes, 368)?,
            provider_fee_lamports: read_u64(bytes, 376)?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode the fixed submission receipt.
    pub fn to_bytes(self) -> Result<[u8; PROVIDER_SUBMIT_RECEIPT_BYTES_V3]> {
        self.validate()?;
        let mut bytes = [0; PROVIDER_SUBMIT_RECEIPT_BYTES_V3];
        write_header(&mut bytes, PROVIDER_SUBMIT_RECEIPT_MAGIC_V3, 1)?;
        put(&mut bytes, 16, &self.generation.to_le_bytes())?;
        for (index, identity) in self.identities().iter().enumerate() {
            put(&mut bytes, 32 + index * 32, identity)?;
        }
        put(&mut bytes, 352, &self.posted_slot.to_le_bytes())?;
        put(&mut bytes, 360, &self.publish_time.to_le_bytes())?;
        put(&mut bytes, 368, &self.update_rent_lamports.to_le_bytes())?;
        put(&mut bytes, 376, &self.provider_fee_lamports.to_le_bytes())?;
        Ok(bytes)
    }

    fn validate(self) -> Result<()> {
        if self.generation == 0
            || self.posted_slot == 0
            || self.publish_time <= 0
            || self.update_rent_lamports == 0
            || self.identities().iter().any(is_zero)
        {
            return Err(Error::InvalidReceiptShape);
        }
        Ok(())
    }

    fn identities(self) -> [[u8; 32]; 10] {
        [
            self.request_digest,
            self.lifecycle,
            self.update_account,
            self.update_digest,
            self.provider_submitter,
            self.update_authority,
            self.refund_recipient,
            self.provider_release,
            self.post_body_digest,
            self.market,
        ]
    }
}

/// Typed reclaim receipt returned before the lifecycle account is closed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderReclaimReceiptV3 {
    /// Exact reclaim request digest.
    pub request_digest: [u8; 32],
    /// Closed lifecycle account.
    pub lifecycle: [u8; 32],
    /// Closed update account.
    pub update_account: [u8; 32],
    /// Terminal certificate.
    pub certificate: [u8; 32],
    /// Permissionless resolver.
    pub resolver: [u8; 32],
    /// Refund recipient.
    pub refund_recipient: [u8; 32],
    /// Update digest consumed by terminal resolution.
    pub update_digest: [u8; 32],
    /// Provider evidence in the terminal certificate.
    pub provider_evidence: [u8; 32],
    /// Generation.
    pub generation: u64,
    /// Terminal sequence.
    pub terminal_sequence: u64,
    /// Exact refunded update rent.
    pub refunded_lamports: u64,
}

impl ProviderReclaimReceiptV3 {
    /// Decode one exact reclaim receipt.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        require_header(
            bytes,
            PROVIDER_RECLAIM_RECEIPT_BYTES_V3,
            PROVIDER_RECLAIM_RECEIPT_MAGIC_V3,
            2,
        )?;
        require_zero(bytes, 296, 8)?;
        let value = Self {
            generation: read_u64(bytes, 16)?,
            terminal_sequence: read_u64(bytes, 24)?,
            request_digest: array(bytes, 32)?,
            lifecycle: array(bytes, 64)?,
            update_account: array(bytes, 96)?,
            certificate: array(bytes, 128)?,
            resolver: array(bytes, 160)?,
            refund_recipient: array(bytes, 192)?,
            update_digest: array(bytes, 224)?,
            provider_evidence: array(bytes, 256)?,
            refunded_lamports: read_u64(bytes, 288)?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode one exact reclaim receipt.
    pub fn to_bytes(self) -> Result<[u8; PROVIDER_RECLAIM_RECEIPT_BYTES_V3]> {
        self.validate()?;
        let mut bytes = [0; PROVIDER_RECLAIM_RECEIPT_BYTES_V3];
        write_header(&mut bytes, PROVIDER_RECLAIM_RECEIPT_MAGIC_V3, 2)?;
        put(&mut bytes, 16, &self.generation.to_le_bytes())?;
        put(&mut bytes, 24, &self.terminal_sequence.to_le_bytes())?;
        for (index, identity) in self.identities().iter().enumerate() {
            put(&mut bytes, 32 + index * 32, identity)?;
        }
        put(&mut bytes, 288, &self.refunded_lamports.to_le_bytes())?;
        Ok(bytes)
    }

    fn validate(self) -> Result<()> {
        if self.generation == 0
            || self.terminal_sequence == 0
            || self.refunded_lamports == 0
            || self.identities().iter().any(is_zero)
        {
            return Err(Error::InvalidReceiptShape);
        }
        Ok(())
    }

    fn identities(self) -> [[u8; 32]; 8] {
        [
            self.request_digest,
            self.lifecycle,
            self.update_account,
            self.certificate,
            self.resolver,
            self.refund_recipient,
            self.update_digest,
            self.provider_evidence,
        ]
    }
}

fn require_header(bytes: &[u8], width: usize, magic: [u8; 8], action: u8) -> Result<()> {
    if bytes.len() != width || array::<8>(bytes, 0)? != magic {
        return Err(Error::InvalidLength);
    }
    if read_u16(bytes, 8)? != PROVIDER_TRANSPORT_VERSION_V3 {
        return Err(Error::UnsupportedVersion);
    }
    if byte(bytes, 10)? != action {
        return Err(Error::UnknownAction);
    }
    if byte(bytes, 11)? != 0 {
        return Err(Error::NonCanonicalReserved);
    }
    require_zero(bytes, 12, 4)
}

fn write_header(bytes: &mut [u8], magic: [u8; 8], action: u8) -> Result<()> {
    put(bytes, 0, &magic)?;
    put(bytes, 8, &PROVIDER_TRANSPORT_VERSION_V3.to_le_bytes())?;
    put(bytes, 10, &[action])?;
    Ok(())
}

fn is_zero(value: &[u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}

fn byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}

fn array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = offset.checked_add(N).ok_or(Error::InvalidLength)?;
    bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(array(bytes, offset)?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(array(bytes, offset)?))
}

fn read_i64(bytes: &[u8], offset: usize) -> Result<i64> {
    Ok(i64::from_le_bytes(array(bytes, offset)?))
}

fn require_zero(bytes: &[u8], offset: usize, width: usize) -> Result<()> {
    let end = offset.checked_add(width).ok_or(Error::InvalidLength)?;
    if bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(Error::NonCanonicalReserved);
    }
    Ok(())
}

fn put(output: &mut [u8], offset: usize, source: &[u8]) -> Result<()> {
    let end = offset
        .checked_add(source.len())
        .ok_or(Error::InvalidLength)?;
    output
        .get_mut(offset..end)
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(source);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn submit() -> ProviderSubmitRequestV3 {
        ProviderSubmitRequestV3 {
            generation: 7,
            reclaim_after_unix_seconds: 100,
            market: [1; 32],
            source_state: [2; 32],
            lifecycle: [3; 32],
            source_material: [4; 32],
            provider_release: [5; 32],
            update_account: [6; 32],
            provider_submitter: [7; 32],
            refund_recipient: [8; 32],
            release_set: [9; 32],
            registry_program: [10; 32],
            encoded_vaa: [11; 32],
            post_body_digest: [12; 32],
        }
    }

    #[test]
    fn submitted_consumed_partition_round_trips() {
        let request = submit();
        assert_eq!(
            ProviderSubmitRequestV3::decode(&request.to_bytes().expect("submit request")),
            Ok(request)
        );
        let mut lifecycle = ProviderUpdateLifecycleV3::submitted(
            request, 4, [12; 32], [18; 32], [13; 32], 100, 9, 500, 1,
        )
        .expect("submitted lifecycle");
        assert_eq!(
            ProviderUpdateLifecycleV3::decode(&lifecycle.to_bytes().expect("submitted bytes")),
            Ok(lifecycle)
        );
        assert_eq!(lifecycle.consume(10, [14; 32], [15; 32]), Ok(()));
        assert_eq!(lifecycle.status, ProviderUpdateStatusV3::Consumed);
        assert_eq!(
            ProviderUpdateLifecycleV3::decode(&lifecycle.to_bytes().expect("consumed bytes")),
            Ok(lifecycle)
        );
        assert!(lifecycle.consume(11, [16; 32], [17; 32]).is_err());
    }

    #[test]
    fn hostile_aliases_and_early_consumed_state_refuse() {
        let mut request = submit();
        request.refund_recipient = request.provider_submitter;
        assert!(request.to_bytes().is_err());
        let request = submit();
        let mut lifecycle = ProviderUpdateLifecycleV3::submitted(
            request, 4, [12; 32], [18; 32], [13; 32], 100, 9, 500, 1,
        )
        .expect("submitted lifecycle");
        lifecycle.status = ProviderUpdateStatusV3::Consumed;
        assert!(lifecycle.to_bytes().is_err());
    }

    #[test]
    fn action_bytes_and_typed_receipts_are_exact() {
        let request = submit();
        let mut submit_bytes = request.to_bytes().expect("submit request");
        submit_bytes[10] = 2;
        assert_eq!(
            ProviderSubmitRequestV3::decode(&submit_bytes),
            Err(Error::UnknownAction)
        );
        let submit_receipt = ProviderSubmitReceiptV3 {
            request_digest: [1; 32],
            lifecycle: [2; 32],
            update_account: [3; 32],
            update_digest: [4; 32],
            provider_submitter: [5; 32],
            update_authority: [6; 32],
            refund_recipient: [7; 32],
            provider_release: [8; 32],
            post_body_digest: [9; 32],
            market: [10; 32],
            generation: 11,
            posted_slot: 12,
            publish_time: 13,
            update_rent_lamports: 14,
            provider_fee_lamports: 15,
        };
        assert_eq!(
            ProviderSubmitReceiptV3::decode(&submit_receipt.to_bytes().expect("submit receipt")),
            Ok(submit_receipt)
        );
        let reclaim_receipt = ProviderReclaimReceiptV3 {
            request_digest: [1; 32],
            lifecycle: [2; 32],
            update_account: [3; 32],
            certificate: [4; 32],
            resolver: [5; 32],
            refund_recipient: [6; 32],
            update_digest: [7; 32],
            provider_evidence: [8; 32],
            generation: 9,
            terminal_sequence: 10,
            refunded_lamports: 11,
        };
        assert_eq!(
            ProviderReclaimReceiptV3::decode(&reclaim_receipt.to_bytes().expect("reclaim receipt")),
            Ok(reclaim_receipt)
        );
    }
}
