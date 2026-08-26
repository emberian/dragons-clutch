//! Canonical admission and reclamation contract for LiabilityBasisV2 Positions.
//!
//! A Position is always the ordinary runtime-width LiabilityBasisV2 account.
//! Its owner coordinate may be an authenticated Trading record (resting
//! inventory) or an ordinary user identity. The explicit owner-kind and
//! presence tags are authoritative; no all-zero public key is an absence
//! sentinel. Balance movement is deliberately absent from this ABI and remains
//! owned by [`crate::affine_batch_v2`].

use core::convert::TryInto;

/// Exact admission/close request width.
pub const PROTOCOL_POSITION_REQUEST_BYTES_V2: usize = 320;
/// Exact Claims-owned admission-state and admission-receipt width.
pub const PROTOCOL_POSITION_ADMISSION_BYTES_V2: usize = 512;
/// Exact terminal close receipt width.
pub const PROTOCOL_POSITION_CLOSE_RECEIPT_BYTES_V2: usize = 416;
/// Admission/close request magic.
pub const PROTOCOL_POSITION_REQUEST_MAGIC_V2: [u8; 8] = *b"DCLPPR02";
/// Persisted live-admission magic.
pub const PROTOCOL_POSITION_ADMISSION_MAGIC_V2: [u8; 8] = *b"DCLPPS02";
/// Immediate admission receipt magic.
pub const PROTOCOL_POSITION_RECEIPT_MAGIC_V2: [u8; 8] = *b"DCLPPC02";
/// Immediate terminal-close receipt magic.
pub const PROTOCOL_POSITION_CLOSE_RECEIPT_MAGIC_V2: [u8; 8] = *b"DCLPCL02";
/// Implemented protocol Position wire version.
pub const PROTOCOL_POSITION_WIRE_VERSION_V2: u16 = 2;
/// Claims-owned admission PDA seed domain.
pub const PROTOCOL_POSITION_ADMISSION_SEED_V2: &[u8] = b"dclutch:protocol-position:v2";

const ACTION_OFFSET: usize = 10;
const OWNER_KIND_OFFSET: usize = 11;
const PRESENCE_OFFSET: usize = 12;
const HEADER_RESERVED_OFFSET: usize = 13;
const RELEASE_SET_OFFSET: usize = 16;
const MARKET_OFFSET: usize = 48;
const POSITION_OWNER_OFFSET: usize = 80;
const PARENT_REQUEST_OFFSET: usize = 112;
const RENT_CREDIT_OFFSET: usize = 144;
const RENT_PROGRAM_OFFSET: usize = 176;
const GENERATION_OFFSET: usize = 208;
const EXPECTED_MARKET_REVISION_OFFSET: usize = 216;
const EXPECTED_POSITION_REVISION_OFFSET: usize = 224;
const OBSERVED_POSITION_LAMPORTS_OFFSET: usize = 232;
const OBSERVED_ADMISSION_LAMPORTS_OFFSET: usize = 240;
const POSITION_RENT_PRINCIPAL_OFFSET: usize = 248;
const ADMISSION_RENT_PRINCIPAL_OFFSET: usize = 256;
const REQUEST_RESERVED_OFFSET: usize = 264;

const EVIDENCE_STATUS_OFFSET: usize = 10;
const EVIDENCE_OWNER_KIND_OFFSET: usize = 11;
const EVIDENCE_RESERVED_HEADER_OFFSET: usize = 12;
const EVIDENCE_PRODUCT_RECORD_OFFSET: usize = 112;
const EVIDENCE_SEMANTIC_BASIS_OFFSET: usize = 144;
const EVIDENCE_LINKED_BASIS_OFFSET: usize = 176;
const EVIDENCE_PARENT_REQUEST_OFFSET: usize = 208;
const EVIDENCE_REQUEST_DIGEST_OFFSET: usize = 240;
const EVIDENCE_RENT_CREDIT_OFFSET: usize = 272;
const EVIDENCE_RENT_PROGRAM_OFFSET: usize = 304;
const EVIDENCE_CLAIMS_PROGRAM_OFFSET: usize = 336;
const EVIDENCE_TRADING_PROGRAM_OFFSET: usize = 368;
const EVIDENCE_POSITION_DIGEST_OFFSET: usize = 400;
const EVIDENCE_GENERATION_OFFSET: usize = 432;
const EVIDENCE_OUTCOME_COUNT_OFFSET: usize = 440;
const EVIDENCE_RESERVED_MIDDLE_OFFSET: usize = 444;
const EVIDENCE_POSITION_RENT_OFFSET: usize = 448;
const EVIDENCE_ADMISSION_RENT_OFFSET: usize = 456;
const EVIDENCE_MARKET_REVISION_BEFORE_OFFSET: usize = 464;
const EVIDENCE_MARKET_REVISION_AFTER_OFFSET: usize = 472;
const EVIDENCE_POSITION_REVISION_OFFSET: usize = 480;
const EVIDENCE_POSITION_LAMPORTS_OFFSET: usize = 488;
const EVIDENCE_ADMISSION_LAMPORTS_OFFSET: usize = 496;
const EVIDENCE_RESERVED_TAIL_OFFSET: usize = 504;

const CLOSE_REQUEST_DIGEST_OFFSET: usize = 144;
const CLOSE_ADMISSION_DIGEST_OFFSET: usize = 176;
const CLOSE_RENT_CREDIT_OFFSET: usize = 208;
const CLOSE_RENT_PROGRAM_OFFSET: usize = 240;
const CLOSE_CLAIMS_PROGRAM_OFFSET: usize = 272;
const CLOSE_POST_RESOURCE_DIGEST_OFFSET: usize = 304;
const CLOSE_POSITION_LAMPORTS_OFFSET: usize = 336;
const CLOSE_ADMISSION_LAMPORTS_OFFSET: usize = 344;
const CLOSE_RENT_CREDIT_BEFORE_OFFSET: usize = 352;
const CLOSE_RENT_CREDIT_AFTER_OFFSET: usize = 360;
const CLOSE_TOTAL_CREDIT_OFFSET: usize = 368;
const CLOSE_POSITION_REVISION_OFFSET: usize = 376;
const CLOSE_MARKET_REVISION_OFFSET: usize = 384;
const CLOSE_GENERATION_OFFSET: usize = 392;
const CLOSE_RESERVED_OFFSET: usize = 400;

/// Stable hostile-decode or lifecycle refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtocolPositionErrorV2 {
    /// Bytes did not have the exact selected width.
    InvalidLength,
    /// Magic or version selected another protocol.
    InvalidHeader,
    /// Reserved bytes or inactive coordinates were noncanonical.
    NonCanonical,
    /// An action, owner-kind, status, or presence tag was unknown.
    UnknownTag,
    /// A required identity was zero or semantically aliased.
    InvalidIdentity,
    /// Presence did not match the selected lifecycle action.
    InvalidPresence,
    /// A revision overflowed or did not form the exact transition.
    InvalidRevision,
    /// Prepaid rent, observed lamports, or terminal credit did not balance.
    InvalidRent,
    /// Runtime Product outcome width was zero.
    InvalidOutcomeCount,
    /// Admission state did not join the current request.
    AdmissionMismatch,
    /// Receipt evidence was incomplete or internally inconsistent.
    ReceiptMismatch,
}

/// Result alias.
pub type Result<T> = core::result::Result<T, ProtocolPositionErrorV2>;

/// Protocol Position lifecycle action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ProtocolPositionActionV2 {
    /// Admit one vacant, prepaid canonical Position.
    Admit = 0,
    /// Reclaim one admitted, zero-vector Position and its admission record.
    Close = 1,
}

impl ProtocolPositionActionV2 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Admit),
            1 => Ok(Self::Close),
            _ => Err(ProtocolPositionErrorV2::UnknownTag),
        }
    }
}

/// Semantic kind of Position owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ProtocolPositionOwnerKindV2 {
    /// Immutable current-Trading-owned record holding resting inventory.
    TradingRecord = 0,
    /// Ordinary user identity receiving or holding claims.
    User = 1,
}

impl ProtocolPositionOwnerKindV2 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::TradingRecord),
            1 => Ok(Self::User),
            _ => Err(ProtocolPositionErrorV2::UnknownTag),
        }
    }
}

/// Explicit observed Position and admission-record presence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ProtocolPositionPresenceV2 {
    /// Both canonical PDAs are vacant, System-owned, and data-empty.
    Vacant = 0,
    /// Both canonical PDAs are existing Claims-owned state.
    Existing = 1,
}

impl ProtocolPositionPresenceV2 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Vacant),
            1 => Ok(Self::Existing),
            _ => Err(ProtocolPositionErrorV2::UnknownTag),
        }
    }
}

/// Exact immutable protocol Position request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtocolPositionRequestV2 {
    /// Selected admission/close action.
    pub action: ProtocolPositionActionV2,
    /// Explicit semantic owner kind.
    pub owner_kind: ProtocolPositionOwnerKindV2,
    /// Explicit pre-action account presence.
    pub presence: ProtocolPositionPresenceV2,
    /// Immutable selected execution release set.
    pub release_set: [u8; 32],
    /// Logical Core Market identity.
    pub market: [u8; 32],
    /// Exact Position owner: Trading record or ordinary user.
    pub position_owner: [u8; 32],
    /// Exact parent Trading request digest.
    pub parent_request_digest: [u8; 32],
    /// Permanent RentCredit account receiving both accounts on close.
    pub rent_credit: [u8; 32],
    /// Exact executable program owning the RentCredit.
    pub rent_program: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Claims aggregate revision, observed unchanged by this lifecycle action.
    pub expected_market_revision: u64,
    /// Existing Position revision on close; canonical zero on admission.
    pub expected_position_revision: u64,
    /// Exact current/prepaid Position lamports.
    pub observed_position_lamports: u64,
    /// Exact current/prepaid admission-record lamports.
    pub observed_admission_lamports: u64,
    /// Current rent minimum committed as Position rent principal.
    pub position_rent_principal: u64,
    /// Current rent minimum committed as admission-record rent principal.
    pub admission_rent_principal: u64,
}

impl ProtocolPositionRequestV2 {
    /// Construct and canonicalize one request.
    pub fn new(self) -> Result<Self> {
        self.validate()?;
        Ok(self)
    }

    /// Decode one exact request.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != PROTOCOL_POSITION_REQUEST_BYTES_V2 {
            return Err(ProtocolPositionErrorV2::InvalidLength);
        }
        exact(input, 0, &PROTOCOL_POSITION_REQUEST_MAGIC_V2)?;
        if read_u16(input, 8)? != PROTOCOL_POSITION_WIRE_VERSION_V2 {
            return Err(ProtocolPositionErrorV2::InvalidHeader);
        }
        require_zero(input, HEADER_RESERVED_OFFSET, 3)?;
        require_zero(input, REQUEST_RESERVED_OFFSET, 56)?;
        Self {
            action: ProtocolPositionActionV2::decode(read_byte(input, ACTION_OFFSET)?)?,
            owner_kind: ProtocolPositionOwnerKindV2::decode(read_byte(input, OWNER_KIND_OFFSET)?)?,
            presence: ProtocolPositionPresenceV2::decode(read_byte(input, PRESENCE_OFFSET)?)?,
            release_set: read_array(input, RELEASE_SET_OFFSET)?,
            market: read_array(input, MARKET_OFFSET)?,
            position_owner: read_array(input, POSITION_OWNER_OFFSET)?,
            parent_request_digest: read_array(input, PARENT_REQUEST_OFFSET)?,
            rent_credit: read_array(input, RENT_CREDIT_OFFSET)?,
            rent_program: read_array(input, RENT_PROGRAM_OFFSET)?,
            generation: read_u64(input, GENERATION_OFFSET)?,
            expected_market_revision: read_u64(input, EXPECTED_MARKET_REVISION_OFFSET)?,
            expected_position_revision: read_u64(input, EXPECTED_POSITION_REVISION_OFFSET)?,
            observed_position_lamports: read_u64(input, OBSERVED_POSITION_LAMPORTS_OFFSET)?,
            observed_admission_lamports: read_u64(input, OBSERVED_ADMISSION_LAMPORTS_OFFSET)?,
            position_rent_principal: read_u64(input, POSITION_RENT_PRINCIPAL_OFFSET)?,
            admission_rent_principal: read_u64(input, ADMISSION_RENT_PRINCIPAL_OFFSET)?,
        }
        .new()
    }

    /// Encode one exact canonical request.
    pub fn to_bytes(self) -> Result<[u8; PROTOCOL_POSITION_REQUEST_BYTES_V2]> {
        self.validate()?;
        let mut output = [0; PROTOCOL_POSITION_REQUEST_BYTES_V2];
        write(&mut output, 0, &PROTOCOL_POSITION_REQUEST_MAGIC_V2)?;
        write(
            &mut output,
            8,
            &PROTOCOL_POSITION_WIRE_VERSION_V2.to_le_bytes(),
        )?;
        write(&mut output, ACTION_OFFSET, &[self.action as u8])?;
        write(&mut output, OWNER_KIND_OFFSET, &[self.owner_kind as u8])?;
        write(&mut output, PRESENCE_OFFSET, &[self.presence as u8])?;
        for (offset, value) in [
            (RELEASE_SET_OFFSET, self.release_set),
            (MARKET_OFFSET, self.market),
            (POSITION_OWNER_OFFSET, self.position_owner),
            (PARENT_REQUEST_OFFSET, self.parent_request_digest),
            (RENT_CREDIT_OFFSET, self.rent_credit),
            (RENT_PROGRAM_OFFSET, self.rent_program),
        ] {
            write(&mut output, offset, &value)?;
        }
        for (offset, value) in [
            (GENERATION_OFFSET, self.generation),
            (
                EXPECTED_MARKET_REVISION_OFFSET,
                self.expected_market_revision,
            ),
            (
                EXPECTED_POSITION_REVISION_OFFSET,
                self.expected_position_revision,
            ),
            (
                OBSERVED_POSITION_LAMPORTS_OFFSET,
                self.observed_position_lamports,
            ),
            (
                OBSERVED_ADMISSION_LAMPORTS_OFFSET,
                self.observed_admission_lamports,
            ),
            (POSITION_RENT_PRINCIPAL_OFFSET, self.position_rent_principal),
            (
                ADMISSION_RENT_PRINCIPAL_OFFSET,
                self.admission_rent_principal,
            ),
        ] {
            write(&mut output, offset, &value.to_le_bytes())?;
        }
        Ok(output)
    }

    fn validate(self) -> Result<()> {
        for identity in [
            self.release_set,
            self.market,
            self.position_owner,
            self.parent_request_digest,
            self.rent_credit,
            self.rent_program,
        ] {
            require_nonzero(identity)?;
        }
        if self.position_owner == self.market
            || self.position_owner == self.rent_credit
            || self.rent_credit == self.rent_program
        {
            return Err(ProtocolPositionErrorV2::InvalidIdentity);
        }
        if self.position_rent_principal == 0
            || self.admission_rent_principal == 0
            || self.observed_position_lamports < self.position_rent_principal
            || self.observed_admission_lamports < self.admission_rent_principal
        {
            return Err(ProtocolPositionErrorV2::InvalidRent);
        }
        match (self.action, self.presence) {
            (ProtocolPositionActionV2::Admit, ProtocolPositionPresenceV2::Vacant) => {
                if self.expected_position_revision != 0 {
                    return Err(ProtocolPositionErrorV2::InvalidRevision);
                }
            }
            (ProtocolPositionActionV2::Close, ProtocolPositionPresenceV2::Existing) => {
                if self.expected_position_revision == u64::MAX {
                    return Err(ProtocolPositionErrorV2::InvalidRevision);
                }
            }
            _ => return Err(ProtocolPositionErrorV2::InvalidPresence),
        }
        Ok(())
    }
}

/// Authenticated Product/LBV2 evidence persisted by Claims at admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtocolPositionAdmissionEvidenceV2 {
    /// Exact finalized Product Runtime V2 graph-root digest.
    pub product_record_digest: [u8; 32],
    /// Product-owned semantic LiabilityBasisV2 identity.
    pub semantic_basis_id: [u8; 32],
    /// Exact finalized linked-basis record digest.
    pub linked_basis_record_digest: [u8; 32],
    /// Digest of the exact admission request.
    pub request_digest: [u8; 32],
    /// Registry-selected current Claims program.
    pub claims_program: [u8; 32],
    /// Registry-selected current Trading program.
    pub trading_program: [u8; 32],
    /// Digest of the exact initialized zero LBV2 Position bytes.
    pub position_state_digest: [u8; 32],
    /// Product Runtime V2 outcome width.
    pub outcome_count: u32,
}

/// Claims-owned immutable admission state and immediate admission receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtocolPositionAdmissionV2 {
    request: ProtocolPositionRequestV2,
    evidence: ProtocolPositionAdmissionEvidenceV2,
}

impl ProtocolPositionAdmissionV2 {
    /// Construct one exact admitted-live state.
    pub fn new(
        request: ProtocolPositionRequestV2,
        evidence: ProtocolPositionAdmissionEvidenceV2,
    ) -> Result<Self> {
        request.validate()?;
        if request.action != ProtocolPositionActionV2::Admit
            || request.presence != ProtocolPositionPresenceV2::Vacant
            || evidence.outcome_count == 0
        {
            return Err(ProtocolPositionErrorV2::AdmissionMismatch);
        }
        for identity in [
            evidence.product_record_digest,
            evidence.semantic_basis_id,
            evidence.linked_basis_record_digest,
            evidence.request_digest,
            evidence.claims_program,
            evidence.trading_program,
            evidence.position_state_digest,
        ] {
            require_nonzero(identity)?;
        }
        Ok(Self { request, evidence })
    }

    /// Decode exact persisted admission bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        decode_admission(input, PROTOCOL_POSITION_ADMISSION_MAGIC_V2)
    }

    /// Decode one exact immediate admission receipt.
    pub fn decode_receipt(input: &[u8]) -> Result<Self> {
        decode_admission(input, PROTOCOL_POSITION_RECEIPT_MAGIC_V2)
    }

    /// Encode exact persisted admission bytes.
    pub fn to_state_bytes(self) -> Result<[u8; PROTOCOL_POSITION_ADMISSION_BYTES_V2]> {
        self.encode_with_magic(PROTOCOL_POSITION_ADMISSION_MAGIC_V2)
    }

    /// Encode the exact immediate admission receipt.
    pub fn to_receipt_bytes(self) -> Result<[u8; PROTOCOL_POSITION_ADMISSION_BYTES_V2]> {
        self.encode_with_magic(PROTOCOL_POSITION_RECEIPT_MAGIC_V2)
    }

    fn encode_with_magic(
        self,
        magic: [u8; 8],
    ) -> Result<[u8; PROTOCOL_POSITION_ADMISSION_BYTES_V2]> {
        let mut output = [0; PROTOCOL_POSITION_ADMISSION_BYTES_V2];
        write(&mut output, 0, &magic)?;
        write(
            &mut output,
            8,
            &PROTOCOL_POSITION_WIRE_VERSION_V2.to_le_bytes(),
        )?;
        write(&mut output, EVIDENCE_STATUS_OFFSET, &[1])?;
        write(
            &mut output,
            EVIDENCE_OWNER_KIND_OFFSET,
            &[self.request.owner_kind as u8],
        )?;
        for (offset, value) in [
            (RELEASE_SET_OFFSET, self.request.release_set),
            (MARKET_OFFSET, self.request.market),
            (POSITION_OWNER_OFFSET, self.request.position_owner),
            (
                EVIDENCE_PRODUCT_RECORD_OFFSET,
                self.evidence.product_record_digest,
            ),
            (
                EVIDENCE_SEMANTIC_BASIS_OFFSET,
                self.evidence.semantic_basis_id,
            ),
            (
                EVIDENCE_LINKED_BASIS_OFFSET,
                self.evidence.linked_basis_record_digest,
            ),
            (
                EVIDENCE_PARENT_REQUEST_OFFSET,
                self.request.parent_request_digest,
            ),
            (EVIDENCE_REQUEST_DIGEST_OFFSET, self.evidence.request_digest),
            (EVIDENCE_RENT_CREDIT_OFFSET, self.request.rent_credit),
            (EVIDENCE_RENT_PROGRAM_OFFSET, self.request.rent_program),
            (EVIDENCE_CLAIMS_PROGRAM_OFFSET, self.evidence.claims_program),
            (
                EVIDENCE_TRADING_PROGRAM_OFFSET,
                self.evidence.trading_program,
            ),
            (
                EVIDENCE_POSITION_DIGEST_OFFSET,
                self.evidence.position_state_digest,
            ),
        ] {
            write(&mut output, offset, &value)?;
        }
        write(
            &mut output,
            EVIDENCE_GENERATION_OFFSET,
            &self.request.generation.to_le_bytes(),
        )?;
        write(
            &mut output,
            EVIDENCE_OUTCOME_COUNT_OFFSET,
            &self.evidence.outcome_count.to_le_bytes(),
        )?;
        for (offset, value) in [
            (
                EVIDENCE_POSITION_RENT_OFFSET,
                self.request.position_rent_principal,
            ),
            (
                EVIDENCE_ADMISSION_RENT_OFFSET,
                self.request.admission_rent_principal,
            ),
            (
                EVIDENCE_MARKET_REVISION_BEFORE_OFFSET,
                self.request.expected_market_revision,
            ),
            (
                EVIDENCE_MARKET_REVISION_AFTER_OFFSET,
                self.request.expected_market_revision,
            ),
            (EVIDENCE_POSITION_REVISION_OFFSET, 0),
            (
                EVIDENCE_POSITION_LAMPORTS_OFFSET,
                self.request.observed_position_lamports,
            ),
            (
                EVIDENCE_ADMISSION_LAMPORTS_OFFSET,
                self.request.observed_admission_lamports,
            ),
        ] {
            write(&mut output, offset, &value.to_le_bytes())?;
        }
        Ok(output)
    }

    /// Return the admitted owner kind.
    pub const fn owner_kind(self) -> ProtocolPositionOwnerKindV2 {
        self.request.owner_kind
    }

    /// Return the logical Market.
    pub const fn market(self) -> [u8; 32] {
        self.request.market
    }

    /// Return the Position owner coordinate.
    pub const fn position_owner(self) -> [u8; 32] {
        self.request.position_owner
    }

    /// Return the selected release set.
    pub const fn release_set(self) -> [u8; 32] {
        self.request.release_set
    }

    /// Return the immutable generation.
    pub const fn generation(self) -> u64 {
        self.request.generation
    }

    /// Return the immutable parent request digest.
    pub const fn parent_request_digest(self) -> [u8; 32] {
        self.request.parent_request_digest
    }

    /// Return the digest of the exact admission request.
    pub const fn request_digest(self) -> [u8; 32] {
        self.evidence.request_digest
    }

    /// Return the exact Product graph-root digest.
    pub const fn product_record_digest(self) -> [u8; 32] {
        self.evidence.product_record_digest
    }

    /// Return the semantic LiabilityBasisV2 identity.
    pub const fn semantic_basis_id(self) -> [u8; 32] {
        self.evidence.semantic_basis_id
    }

    /// Return the exact linked basis-record digest.
    pub const fn linked_basis_record_digest(self) -> [u8; 32] {
        self.evidence.linked_basis_record_digest
    }

    /// Return the immutable RentCredit.
    pub const fn rent_credit(self) -> [u8; 32] {
        self.request.rent_credit
    }

    /// Return the immutable RentCredit program.
    pub const fn rent_program(self) -> [u8; 32] {
        self.request.rent_program
    }

    /// Return the Claims program that admitted this Position.
    pub const fn claims_program(self) -> [u8; 32] {
        self.evidence.claims_program
    }

    /// Return the Trading program that authorized admission.
    pub const fn trading_program(self) -> [u8; 32] {
        self.evidence.trading_program
    }

    /// Return the initialized Position digest.
    pub const fn position_state_digest(self) -> [u8; 32] {
        self.evidence.position_state_digest
    }

    /// Return runtime Product width.
    pub const fn outcome_count(self) -> u32 {
        self.evidence.outcome_count
    }

    /// Return the immutable Position rent principal.
    pub const fn position_rent_principal(self) -> u64 {
        self.request.position_rent_principal
    }

    /// Return the immutable admission rent principal.
    pub const fn admission_rent_principal(self) -> u64 {
        self.request.admission_rent_principal
    }

    /// Return the aggregate revision observed at admission.
    pub const fn market_revision(self) -> u64 {
        self.request.expected_market_revision
    }

    /// Return exact prepaid Position lamports, including permitted dust.
    pub const fn position_lamports(self) -> u64 {
        self.request.observed_position_lamports
    }

    /// Return exact prepaid admission lamports, including permitted dust.
    pub const fn admission_lamports(self) -> u64 {
        self.request.observed_admission_lamports
    }
}

/// Exact terminal-close evidence supplied by the adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtocolPositionCloseEvidenceV2 {
    /// Digest of the exact close request.
    pub request_digest: [u8; 32],
    /// Digest of the exact persisted admission bytes.
    pub admission_digest: [u8; 32],
    /// Registry-selected current Claims program.
    pub claims_program: [u8; 32],
    /// Digest committing both closed resources and credited RentCredit poststate.
    pub post_resource_digest: [u8; 32],
    /// RentCredit lamports before close.
    pub rent_credit_before: u64,
    /// RentCredit lamports after close.
    pub rent_credit_after: u64,
}

/// Exact terminal receipt for one reclaimed Position/admission pair.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtocolPositionCloseReceiptV2 {
    owner_kind: ProtocolPositionOwnerKindV2,
    release_set: [u8; 32],
    market: [u8; 32],
    position_owner: [u8; 32],
    parent_request_digest: [u8; 32],
    request_digest: [u8; 32],
    admission_digest: [u8; 32],
    rent_credit: [u8; 32],
    rent_program: [u8; 32],
    claims_program: [u8; 32],
    post_resource_digest: [u8; 32],
    position_lamports: u64,
    admission_lamports: u64,
    rent_credit_before: u64,
    rent_credit_after: u64,
    total_credit: u64,
    position_revision: u64,
    market_revision: u64,
    generation: u64,
}

impl ProtocolPositionCloseReceiptV2 {
    /// Construct one close receipt with exact conserved native credit.
    pub fn new(
        request: ProtocolPositionRequestV2,
        evidence: ProtocolPositionCloseEvidenceV2,
    ) -> Result<Self> {
        request.validate()?;
        for identity in [
            evidence.request_digest,
            evidence.admission_digest,
            evidence.claims_program,
            evidence.post_resource_digest,
        ] {
            require_nonzero(identity)?;
        }
        let total = request
            .observed_position_lamports
            .checked_add(request.observed_admission_lamports)
            .ok_or(ProtocolPositionErrorV2::InvalidRent)?;
        if request.action != ProtocolPositionActionV2::Close
            || request.presence != ProtocolPositionPresenceV2::Existing
            || evidence.rent_credit_before.checked_add(total) != Some(evidence.rent_credit_after)
        {
            return Err(ProtocolPositionErrorV2::ReceiptMismatch);
        }
        Ok(Self {
            owner_kind: request.owner_kind,
            release_set: request.release_set,
            market: request.market,
            position_owner: request.position_owner,
            parent_request_digest: request.parent_request_digest,
            request_digest: evidence.request_digest,
            admission_digest: evidence.admission_digest,
            rent_credit: request.rent_credit,
            rent_program: request.rent_program,
            claims_program: evidence.claims_program,
            post_resource_digest: evidence.post_resource_digest,
            position_lamports: request.observed_position_lamports,
            admission_lamports: request.observed_admission_lamports,
            rent_credit_before: evidence.rent_credit_before,
            rent_credit_after: evidence.rent_credit_after,
            total_credit: total,
            position_revision: request.expected_position_revision,
            market_revision: request.expected_market_revision,
            generation: request.generation,
        })
    }

    /// Encode one exact close receipt.
    pub fn to_bytes(self) -> Result<[u8; PROTOCOL_POSITION_CLOSE_RECEIPT_BYTES_V2]> {
        let mut output = [0; PROTOCOL_POSITION_CLOSE_RECEIPT_BYTES_V2];
        write(&mut output, 0, &PROTOCOL_POSITION_CLOSE_RECEIPT_MAGIC_V2)?;
        write(
            &mut output,
            8,
            &PROTOCOL_POSITION_WIRE_VERSION_V2.to_le_bytes(),
        )?;
        write(
            &mut output,
            ACTION_OFFSET,
            &[ProtocolPositionActionV2::Close as u8],
        )?;
        write(&mut output, OWNER_KIND_OFFSET, &[self.owner_kind as u8])?;
        write(
            &mut output,
            PRESENCE_OFFSET,
            &[ProtocolPositionPresenceV2::Vacant as u8],
        )?;
        for (offset, value) in [
            (RELEASE_SET_OFFSET, self.release_set),
            (MARKET_OFFSET, self.market),
            (POSITION_OWNER_OFFSET, self.position_owner),
            (PARENT_REQUEST_OFFSET, self.parent_request_digest),
            (CLOSE_REQUEST_DIGEST_OFFSET, self.request_digest),
            (CLOSE_ADMISSION_DIGEST_OFFSET, self.admission_digest),
            (CLOSE_RENT_CREDIT_OFFSET, self.rent_credit),
            (CLOSE_RENT_PROGRAM_OFFSET, self.rent_program),
            (CLOSE_CLAIMS_PROGRAM_OFFSET, self.claims_program),
            (CLOSE_POST_RESOURCE_DIGEST_OFFSET, self.post_resource_digest),
        ] {
            write(&mut output, offset, &value)?;
        }
        for (offset, value) in [
            (CLOSE_POSITION_LAMPORTS_OFFSET, self.position_lamports),
            (CLOSE_ADMISSION_LAMPORTS_OFFSET, self.admission_lamports),
            (CLOSE_RENT_CREDIT_BEFORE_OFFSET, self.rent_credit_before),
            (CLOSE_RENT_CREDIT_AFTER_OFFSET, self.rent_credit_after),
            (CLOSE_TOTAL_CREDIT_OFFSET, self.total_credit),
            (CLOSE_POSITION_REVISION_OFFSET, self.position_revision),
            (CLOSE_MARKET_REVISION_OFFSET, self.market_revision),
            (CLOSE_GENERATION_OFFSET, self.generation),
        ] {
            write(&mut output, offset, &value.to_le_bytes())?;
        }
        Ok(output)
    }

    /// Decode and canonicalize one exact close receipt.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != PROTOCOL_POSITION_CLOSE_RECEIPT_BYTES_V2 {
            return Err(ProtocolPositionErrorV2::InvalidLength);
        }
        exact(input, 0, &PROTOCOL_POSITION_CLOSE_RECEIPT_MAGIC_V2)?;
        if read_u16(input, 8)? != PROTOCOL_POSITION_WIRE_VERSION_V2
            || ProtocolPositionActionV2::decode(read_byte(input, ACTION_OFFSET)?)?
                != ProtocolPositionActionV2::Close
            || ProtocolPositionPresenceV2::decode(read_byte(input, PRESENCE_OFFSET)?)?
                != ProtocolPositionPresenceV2::Vacant
        {
            return Err(ProtocolPositionErrorV2::InvalidHeader);
        }
        require_zero(input, HEADER_RESERVED_OFFSET, 3)?;
        require_zero(input, CLOSE_RESERVED_OFFSET, 16)?;
        let total = read_u64(input, CLOSE_TOTAL_CREDIT_OFFSET)?;
        let position_lamports = read_u64(input, CLOSE_POSITION_LAMPORTS_OFFSET)?;
        let admission_lamports = read_u64(input, CLOSE_ADMISSION_LAMPORTS_OFFSET)?;
        if position_lamports.checked_add(admission_lamports) != Some(total) {
            return Err(ProtocolPositionErrorV2::ReceiptMismatch);
        }
        let value = Self {
            owner_kind: ProtocolPositionOwnerKindV2::decode(read_byte(input, OWNER_KIND_OFFSET)?)?,
            release_set: read_array(input, RELEASE_SET_OFFSET)?,
            market: read_array(input, MARKET_OFFSET)?,
            position_owner: read_array(input, POSITION_OWNER_OFFSET)?,
            parent_request_digest: read_array(input, PARENT_REQUEST_OFFSET)?,
            request_digest: read_array(input, CLOSE_REQUEST_DIGEST_OFFSET)?,
            admission_digest: read_array(input, CLOSE_ADMISSION_DIGEST_OFFSET)?,
            rent_credit: read_array(input, CLOSE_RENT_CREDIT_OFFSET)?,
            rent_program: read_array(input, CLOSE_RENT_PROGRAM_OFFSET)?,
            claims_program: read_array(input, CLOSE_CLAIMS_PROGRAM_OFFSET)?,
            post_resource_digest: read_array(input, CLOSE_POST_RESOURCE_DIGEST_OFFSET)?,
            position_lamports,
            admission_lamports,
            rent_credit_before: read_u64(input, CLOSE_RENT_CREDIT_BEFORE_OFFSET)?,
            rent_credit_after: read_u64(input, CLOSE_RENT_CREDIT_AFTER_OFFSET)?,
            total_credit: total,
            position_revision: read_u64(input, CLOSE_POSITION_REVISION_OFFSET)?,
            market_revision: read_u64(input, CLOSE_MARKET_REVISION_OFFSET)?,
            generation: read_u64(input, CLOSE_GENERATION_OFFSET)?,
        };
        for identity in [
            value.release_set,
            value.market,
            value.position_owner,
            value.parent_request_digest,
            value.request_digest,
            value.admission_digest,
            value.rent_credit,
            value.rent_program,
            value.claims_program,
            value.post_resource_digest,
        ] {
            require_nonzero(identity)?;
        }
        if value.rent_credit_before.checked_add(total) != Some(value.rent_credit_after) {
            return Err(ProtocolPositionErrorV2::ReceiptMismatch);
        }
        Ok(value)
    }
}

fn decode_admission(input: &[u8], magic: [u8; 8]) -> Result<ProtocolPositionAdmissionV2> {
    if input.len() != PROTOCOL_POSITION_ADMISSION_BYTES_V2 {
        return Err(ProtocolPositionErrorV2::InvalidLength);
    }
    exact(input, 0, &magic)?;
    if read_u16(input, 8)? != PROTOCOL_POSITION_WIRE_VERSION_V2
        || read_byte(input, EVIDENCE_STATUS_OFFSET)? != 1
    {
        return Err(ProtocolPositionErrorV2::InvalidHeader);
    }
    require_zero(input, EVIDENCE_RESERVED_HEADER_OFFSET, 4)?;
    require_zero(input, EVIDENCE_RESERVED_MIDDLE_OFFSET, 4)?;
    require_zero(input, EVIDENCE_RESERVED_TAIL_OFFSET, 8)?;
    let request = ProtocolPositionRequestV2 {
        action: ProtocolPositionActionV2::Admit,
        owner_kind: ProtocolPositionOwnerKindV2::decode(read_byte(
            input,
            EVIDENCE_OWNER_KIND_OFFSET,
        )?)?,
        presence: ProtocolPositionPresenceV2::Vacant,
        release_set: read_array(input, RELEASE_SET_OFFSET)?,
        market: read_array(input, MARKET_OFFSET)?,
        position_owner: read_array(input, POSITION_OWNER_OFFSET)?,
        parent_request_digest: read_array(input, EVIDENCE_PARENT_REQUEST_OFFSET)?,
        rent_credit: read_array(input, EVIDENCE_RENT_CREDIT_OFFSET)?,
        rent_program: read_array(input, EVIDENCE_RENT_PROGRAM_OFFSET)?,
        generation: read_u64(input, EVIDENCE_GENERATION_OFFSET)?,
        expected_market_revision: read_u64(input, EVIDENCE_MARKET_REVISION_BEFORE_OFFSET)?,
        expected_position_revision: 0,
        observed_position_lamports: read_u64(input, EVIDENCE_POSITION_LAMPORTS_OFFSET)?,
        observed_admission_lamports: read_u64(input, EVIDENCE_ADMISSION_LAMPORTS_OFFSET)?,
        position_rent_principal: read_u64(input, EVIDENCE_POSITION_RENT_OFFSET)?,
        admission_rent_principal: read_u64(input, EVIDENCE_ADMISSION_RENT_OFFSET)?,
    };
    if read_u64(input, EVIDENCE_MARKET_REVISION_AFTER_OFFSET)? != request.expected_market_revision
        || read_u64(input, EVIDENCE_POSITION_REVISION_OFFSET)? != 0
    {
        return Err(ProtocolPositionErrorV2::AdmissionMismatch);
    }
    ProtocolPositionAdmissionV2::new(
        request,
        ProtocolPositionAdmissionEvidenceV2 {
            product_record_digest: read_array(input, EVIDENCE_PRODUCT_RECORD_OFFSET)?,
            semantic_basis_id: read_array(input, EVIDENCE_SEMANTIC_BASIS_OFFSET)?,
            linked_basis_record_digest: read_array(input, EVIDENCE_LINKED_BASIS_OFFSET)?,
            request_digest: read_array(input, EVIDENCE_REQUEST_DIGEST_OFFSET)?,
            claims_program: read_array(input, EVIDENCE_CLAIMS_PROGRAM_OFFSET)?,
            trading_program: read_array(input, EVIDENCE_TRADING_PROGRAM_OFFSET)?,
            position_state_digest: read_array(input, EVIDENCE_POSITION_DIGEST_OFFSET)?,
            outcome_count: read_u32(input, EVIDENCE_OUTCOME_COUNT_OFFSET)?,
        },
    )
}

fn require_nonzero(value: [u8; 32]) -> Result<()> {
    if value.iter().all(|byte| *byte == 0) {
        Err(ProtocolPositionErrorV2::InvalidIdentity)
    } else {
        Ok(())
    }
}

fn exact(input: &[u8], offset: usize, expected: &[u8]) -> Result<()> {
    let end = offset
        .checked_add(expected.len())
        .ok_or(ProtocolPositionErrorV2::InvalidLength)?;
    if input.get(offset..end) != Some(expected) {
        return Err(ProtocolPositionErrorV2::InvalidHeader);
    }
    Ok(())
}

fn require_zero(input: &[u8], offset: usize, width: usize) -> Result<()> {
    let end = offset
        .checked_add(width)
        .ok_or(ProtocolPositionErrorV2::InvalidLength)?;
    if input
        .get(offset..end)
        .ok_or(ProtocolPositionErrorV2::InvalidLength)?
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(ProtocolPositionErrorV2::NonCanonical);
    }
    Ok(())
}

fn read_byte(input: &[u8], offset: usize) -> Result<u8> {
    input
        .get(offset)
        .copied()
        .ok_or(ProtocolPositionErrorV2::InvalidLength)
}

fn read_u16(input: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(read_array(input, offset)?))
}

fn read_u32(input: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(read_array(input, offset)?))
}

fn read_u64(input: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(read_array(input, offset)?))
}

fn read_array<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = offset
        .checked_add(N)
        .ok_or(ProtocolPositionErrorV2::InvalidLength)?;
    input
        .get(offset..end)
        .ok_or(ProtocolPositionErrorV2::InvalidLength)?
        .try_into()
        .map_err(|_| ProtocolPositionErrorV2::InvalidLength)
}

fn write(output: &mut [u8], offset: usize, value: &[u8]) -> Result<()> {
    let end = offset
        .checked_add(value.len())
        .ok_or(ProtocolPositionErrorV2::InvalidLength)?;
    output
        .get_mut(offset..end)
        .ok_or(ProtocolPositionErrorV2::InvalidLength)?
        .copy_from_slice(value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(action: ProtocolPositionActionV2) -> ProtocolPositionRequestV2 {
        ProtocolPositionRequestV2 {
            action,
            owner_kind: ProtocolPositionOwnerKindV2::TradingRecord,
            presence: if action == ProtocolPositionActionV2::Admit {
                ProtocolPositionPresenceV2::Vacant
            } else {
                ProtocolPositionPresenceV2::Existing
            },
            release_set: [1; 32],
            market: [2; 32],
            position_owner: [3; 32],
            parent_request_digest: [4; 32],
            rent_credit: [5; 32],
            rent_program: [6; 32],
            generation: 7,
            expected_market_revision: 8,
            expected_position_revision: if action == ProtocolPositionActionV2::Admit {
                0
            } else {
                9
            },
            observed_position_lamports: 12,
            observed_admission_lamports: 13,
            position_rent_principal: 10,
            admission_rent_principal: 11,
        }
        .new()
        .expect("request")
    }

    fn evidence() -> ProtocolPositionAdmissionEvidenceV2 {
        ProtocolPositionAdmissionEvidenceV2 {
            product_record_digest: [7; 32],
            semantic_basis_id: [8; 32],
            linked_basis_record_digest: [9; 32],
            request_digest: [10; 32],
            claims_program: [11; 32],
            trading_program: [12; 32],
            position_state_digest: [13; 32],
            outcome_count: 70_001,
        }
    }

    #[test]
    fn explicit_presence_and_owner_kind_roundtrip_without_sentinels() {
        let admit = request(ProtocolPositionActionV2::Admit);
        let bytes = admit.to_bytes().expect("bytes");
        assert_eq!(ProtocolPositionRequestV2::decode(&bytes), Ok(admit));
        let mut user = admit;
        user.owner_kind = ProtocolPositionOwnerKindV2::User;
        assert_eq!(
            ProtocolPositionRequestV2::decode(&user.to_bytes().expect("user bytes")),
            Ok(user)
        );

        let mut wrong = admit;
        wrong.presence = ProtocolPositionPresenceV2::Existing;
        assert_eq!(
            wrong.to_bytes(),
            Err(ProtocolPositionErrorV2::InvalidPresence)
        );
        let mut zero = admit;
        zero.position_owner = [0; 32];
        assert_eq!(
            zero.to_bytes(),
            Err(ProtocolPositionErrorV2::InvalidIdentity)
        );
    }

    #[test]
    fn admission_commits_runtime_width_dust_and_exact_rent_principals() {
        let request = request(ProtocolPositionActionV2::Admit);
        let admission = ProtocolPositionAdmissionV2::new(request, evidence()).expect("admission");
        let state = admission.to_state_bytes().expect("state");
        let receipt = admission.to_receipt_bytes().expect("receipt");
        assert_eq!(ProtocolPositionAdmissionV2::decode(&state), Ok(admission));
        assert_eq!(
            ProtocolPositionAdmissionV2::decode_receipt(&receipt),
            Ok(admission)
        );

        let mut dirty = state;
        dirty[EVIDENCE_RESERVED_TAIL_OFFSET] = 1;
        assert_eq!(
            ProtocolPositionAdmissionV2::decode(&dirty),
            Err(ProtocolPositionErrorV2::NonCanonical)
        );
    }

    #[test]
    fn close_receipt_conserves_both_accounts_into_exact_rent_credit() {
        let request = request(ProtocolPositionActionV2::Close);
        let close = ProtocolPositionCloseReceiptV2::new(
            request,
            ProtocolPositionCloseEvidenceV2 {
                request_digest: [14; 32],
                admission_digest: [15; 32],
                claims_program: [16; 32],
                post_resource_digest: [17; 32],
                rent_credit_before: 20,
                rent_credit_after: 45,
            },
        )
        .expect("close");
        let bytes = close.to_bytes().expect("bytes");
        assert_eq!(ProtocolPositionCloseReceiptV2::decode(&bytes), Ok(close));

        let mut bad = bytes;
        bad[CLOSE_RENT_CREDIT_AFTER_OFFSET] = 44;
        assert_eq!(
            ProtocolPositionCloseReceiptV2::decode(&bad),
            Err(ProtocolPositionErrorV2::ReceiptMismatch)
        );
    }

    #[test]
    fn prepaid_creation_is_dust_tolerant_but_never_underfunded() {
        let mut request = request(ProtocolPositionActionV2::Admit);
        request.observed_position_lamports = request.position_rent_principal + 99;
        request.observed_admission_lamports = request.admission_rent_principal + 101;
        request.to_bytes().expect("donations are preserved");
        request.observed_position_lamports = request.position_rent_principal - 1;
        assert_eq!(
            request.to_bytes(),
            Err(ProtocolPositionErrorV2::InvalidRent)
        );
    }

    #[test]
    fn hostile_alias_reserved_tag_and_terminal_revision_refuse() {
        let mut aliased = request(ProtocolPositionActionV2::Admit);
        aliased.rent_credit = aliased.position_owner;
        assert_eq!(
            aliased.to_bytes(),
            Err(ProtocolPositionErrorV2::InvalidIdentity)
        );

        let mut bytes = request(ProtocolPositionActionV2::Admit)
            .to_bytes()
            .expect("bytes");
        bytes[REQUEST_RESERVED_OFFSET] = 1;
        assert_eq!(
            ProtocolPositionRequestV2::decode(&bytes),
            Err(ProtocolPositionErrorV2::NonCanonical)
        );
        bytes[REQUEST_RESERVED_OFFSET] = 0;
        bytes[OWNER_KIND_OFFSET] = 2;
        assert_eq!(
            ProtocolPositionRequestV2::decode(&bytes),
            Err(ProtocolPositionErrorV2::UnknownTag)
        );

        let mut close = request(ProtocolPositionActionV2::Close);
        close.expected_position_revision = u64::MAX;
        assert_eq!(
            close.to_bytes(),
            Err(ProtocolPositionErrorV2::InvalidRevision)
        );
    }
}
