//! Fixed-layout Custody state for staged Dealer scenario value movement.
//!
//! Evaluation commits one manifest and up to four exact Custody request
//! envelopes. Custody then moves each source amount into a dedicated
//! checkpoint-scoped token escrow, recording the transition in one batch and
//! one effect state. Final activation drains every escrow to its original
//! destination. Expiry rolls the escrows back to their original sources in
//! reverse order. No fee, Hoard, liveness, rent, or recovery label is inferred
//! from balances: the original canonical Custody request remains the semantic
//! owner of both compartments.

use super::{Error, Result, array_at, byte_at, put, put_byte, put_u64, require_zero, u64_at};
use crate::scenario_reservation_receipt_v1::{
    DEALER_SCENARIO_MAX_RESERVATIONS_V1, DealerScenarioReservationActionV1,
};

/// Exact Reserve or Rollback selector plus one ordinal.
pub const DEALER_SCENARIO_RESERVATION_INSTRUCTION_BYTES_V1: usize = 9;
/// Reserve one ordered effect into Custody escrow.
pub const DEALER_SCENARIO_RESERVE_MAGIC_V1: [u8; 8] = *b"DCLTCRS1";
/// Roll one expired escrow back to its original source.
pub const DEALER_SCENARIO_ROLLBACK_MAGIC_V1: [u8; 8] = *b"DCLTCRB1";
/// Activate every ordered escrow in one bounded Custody transaction.
pub const DEALER_SCENARIO_ACTIVATE_MAGIC_V1: [u8; 8] = *b"DCLTCAC1";

/// Encode one exact reserve/rollback instruction.
pub fn encode_dealer_scenario_reservation_instruction_v1(
    action: DealerScenarioReservationActionV1,
    ordinal: u8,
) -> Result<[u8; DEALER_SCENARIO_RESERVATION_INSTRUCTION_BYTES_V1]> {
    if usize::from(ordinal) >= DEALER_SCENARIO_MAX_RESERVATIONS_V1 {
        return Err(Error::InvalidPhase);
    }
    let mut output = [0_u8; DEALER_SCENARIO_RESERVATION_INSTRUCTION_BYTES_V1];
    output[..8].copy_from_slice(match action {
        DealerScenarioReservationActionV1::Reserve => &DEALER_SCENARIO_RESERVE_MAGIC_V1,
        DealerScenarioReservationActionV1::Rollback => &DEALER_SCENARIO_ROLLBACK_MAGIC_V1,
    });
    output[8] = ordinal;
    Ok(output)
}

/// Hostile-decode one exact reserve/rollback instruction.
pub fn decode_dealer_scenario_reservation_instruction_v1(
    input: &[u8],
) -> Result<(DealerScenarioReservationActionV1, u8)> {
    if input.len() != DEALER_SCENARIO_RESERVATION_INSTRUCTION_BYTES_V1 {
        return Err(Error::InvalidLength);
    }
    let action = match input.get(..8) {
        Some(magic) if magic == DEALER_SCENARIO_RESERVE_MAGIC_V1 => {
            DealerScenarioReservationActionV1::Reserve
        }
        Some(magic) if magic == DEALER_SCENARIO_ROLLBACK_MAGIC_V1 => {
            DealerScenarioReservationActionV1::Rollback
        }
        _ => return Err(Error::InvalidMagic),
    };
    let ordinal = byte_at(input, 8)?;
    if usize::from(ordinal) >= DEALER_SCENARIO_MAX_RESERVATIONS_V1 {
        return Err(Error::InvalidPhase);
    }
    Ok((action, ordinal))
}

/// Encode one exact all-effect activation instruction.
pub fn encode_dealer_scenario_activation_instruction_v1(
    effect_count: u8,
) -> Result<[u8; DEALER_SCENARIO_RESERVATION_INSTRUCTION_BYTES_V1]> {
    if effect_count == 0 || usize::from(effect_count) > DEALER_SCENARIO_MAX_RESERVATIONS_V1 {
        return Err(Error::InvalidPhase);
    }
    let mut output = [0_u8; DEALER_SCENARIO_RESERVATION_INSTRUCTION_BYTES_V1];
    output[..8].copy_from_slice(&DEALER_SCENARIO_ACTIVATE_MAGIC_V1);
    output[8] = effect_count;
    Ok(output)
}

/// Hostile-decode one exact all-effect activation instruction.
pub fn decode_dealer_scenario_activation_instruction_v1(input: &[u8]) -> Result<u8> {
    if input.len() != DEALER_SCENARIO_RESERVATION_INSTRUCTION_BYTES_V1 {
        return Err(Error::InvalidLength);
    }
    if input.get(..8) != Some(DEALER_SCENARIO_ACTIVATE_MAGIC_V1.as_slice()) {
        return Err(Error::InvalidMagic);
    }
    let effect_count = byte_at(input, 8)?;
    if effect_count == 0 || usize::from(effect_count) > DEALER_SCENARIO_MAX_RESERVATIONS_V1 {
        return Err(Error::InvalidPhase);
    }
    Ok(effect_count)
}

/// Canonical Custody V1 request width carried by an effect envelope.
pub const DEALER_SCENARIO_CANONICAL_CUSTODY_REQUEST_BYTES_V1: usize = 672;
/// Canonical delegated Custody V2 request width carried by an effect envelope.
pub const DEALER_SCENARIO_DELEGATED_CUSTODY_REQUEST_BYTES_V1: usize = 776;
/// Exact evaluator-owned effect-envelope width.
pub const DEALER_SCENARIO_CUSTODY_EFFECT_BYTES_V1: usize = 912;
/// Exact evaluator-owned effect-manifest width.
pub const DEALER_SCENARIO_CUSTODY_EFFECT_MANIFEST_BYTES_V1: usize = 384;
/// Exact Custody-owned reservation-batch width.
pub const DEALER_SCENARIO_RESERVATION_BATCH_BYTES_V1: usize = 640;
/// Exact Custody-owned per-effect reservation-state width.
///
/// Re-exported from the Lean emission rather than restated: this record's
/// magic, width, version, three wire tags and every coordinate now have one
/// author, `DClutchSemantics.DealerScenarioReservationStateV1Abi`.
pub use crate::generated_scenario_reservation_state_v1::DEALER_SCENARIO_RESERVATION_STATE_BYTES_V1;
/// Exact final batch-activation receipt width.
pub const DEALER_SCENARIO_ACTIVATION_RECEIPT_BYTES_V1: usize = 336;

/// Effect-envelope magic.
pub const DEALER_SCENARIO_CUSTODY_EFFECT_MAGIC_V1: [u8; 8] = *b"DCLTDCE1";
/// Effect-manifest magic.
pub const DEALER_SCENARIO_CUSTODY_EFFECT_MANIFEST_MAGIC_V1: [u8; 8] = *b"DCLTDCM1";
/// Reservation-batch magic.
pub const DEALER_SCENARIO_RESERVATION_BATCH_MAGIC_V1: [u8; 8] = *b"DCLTDBT1";
/// Reservation-state magic.
pub use crate::generated_scenario_reservation_state_v1::DEALER_SCENARIO_RESERVATION_STATE_MAGIC_V1;
/// Batch-activation receipt magic.
pub const DEALER_SCENARIO_ACTIVATION_RECEIPT_MAGIC_V1: [u8; 8] = *b"DCLTDAC1";
/// Shared schema version.
///
/// One value for all four records in this file, emitted with the one that has
/// a Lean owner.
pub use crate::generated_scenario_reservation_state_v1::DEALER_SCENARIO_CUSTODY_STATE_VERSION_V1;

/// Solana's maximum length for a single program-derived-address seed.
///
/// A seed longer than this cannot be used at all: the address is not merely
/// unusual, it is underivable, and every call naming it fails. This crate is
/// `no_std` and does not depend on the SDK, so the limit is restated here and
/// enforced against every domain below.
pub const MAX_PDA_SEED_BYTES_V1: usize = 32;

/// Custody-owned reservation batch PDA domain.
pub const DEALER_SCENARIO_RESERVATION_BATCH_PDA_DOMAIN_V1: &[u8] = b"dclutch:dealer-batch:v1";
/// Custody-owned per-effect state PDA domain.
pub const DEALER_SCENARIO_RESERVATION_STATE_PDA_DOMAIN_V1: &[u8] =
    b"dclutch:dealer-reserve-state:v1";
/// Token-program-owned per-effect escrow PDA domain under Custody.
pub const DEALER_SCENARIO_RESERVATION_ESCROW_PDA_DOMAIN_V1: &[u8] = b"dclutch:dealer-escrow:v1";
/// Domain for one request-specific Trading caller authority.
pub const DEALER_SCENARIO_RESERVATION_CALL_DOMAIN_V1: &[u8] = b"dclutch:dealer-call:v1";
/// Custody-owned durable activation-receipt PDA domain.
pub const DEALER_SCENARIO_ACTIVATION_RECEIPT_PDA_DOMAIN_V1: &[u8] = b"dclutch:dealer-activation:v1";

// These four domains were each 35 or 36 bytes, so every address in the Custody
// reservation, escrow and activation families was underivable by construction:
// Custody could not sign one into existence and Trading could not authenticate
// one. Nothing on any cluster can depend on the old spellings, because no
// account at those addresses can ever have been created. The assertions are the
// actual fix -- a shorter string is only a shorter string until something stops
// the next one from growing.
const _: () = assert!(
    DEALER_SCENARIO_RESERVATION_BATCH_PDA_DOMAIN_V1.len() <= MAX_PDA_SEED_BYTES_V1,
    "the reservation batch domain must be a usable PDA seed"
);
const _: () = assert!(
    DEALER_SCENARIO_RESERVATION_STATE_PDA_DOMAIN_V1.len() <= MAX_PDA_SEED_BYTES_V1,
    "the reservation state domain must be a usable PDA seed"
);
const _: () = assert!(
    DEALER_SCENARIO_RESERVATION_ESCROW_PDA_DOMAIN_V1.len() <= MAX_PDA_SEED_BYTES_V1,
    "the reservation escrow domain must be a usable PDA seed"
);
const _: () = assert!(
    DEALER_SCENARIO_RESERVATION_CALL_DOMAIN_V1.len() <= MAX_PDA_SEED_BYTES_V1,
    "the reservation caller-authority domain must be a usable PDA seed"
);
const _: () = assert!(
    DEALER_SCENARIO_ACTIVATION_RECEIPT_PDA_DOMAIN_V1.len() <= MAX_PDA_SEED_BYTES_V1,
    "the activation receipt domain must be a usable PDA seed"
);

const VERSION_OFFSET: usize = 8;
const TAG_OFFSET: usize = 10;
const ORDINAL_OFFSET: usize = 11;
const COUNT_OFFSET: usize = 12;

const EFFECT_PRODUCER_OFFSET: usize = 16;
const EFFECT_CHECKPOINT_OFFSET: usize = 48;
const EFFECT_REQUEST_OFFSET: usize = 80;
const EFFECT_SOURCE_AFTER_OFFSET: usize = 112;
const EFFECT_DESTINATION_AFTER_OFFSET: usize = 120;
const EFFECT_REQUEST_LENGTH_OFFSET: usize = 128;
const EFFECT_RESERVED_OFFSET: usize = 130;
const EFFECT_RESERVED_BYTES: usize = 6;
const EFFECT_PAYLOAD_OFFSET: usize = 136;

const MANIFEST_PRODUCER_OFFSET: usize = 16;
const MANIFEST_CHECKPOINT_OFFSET: usize = 48;
const MANIFEST_REQUEST_OFFSET: usize = 80;
const MANIFEST_ACCOUNTS_OFFSET: usize = 112;
const MANIFEST_DIGESTS_OFFSET: usize = 240;
const MANIFEST_RESERVED_OFFSET: usize = 368;
const MANIFEST_RESERVED_BYTES: usize = 16;

const BATCH_RESERVED_COUNT_OFFSET: usize = 12;
const BATCH_ROLLBACK_COUNT_OFFSET: usize = 13;
const BATCH_RESERVED_OFFSET: usize = 14;
const BATCH_RESERVED_BYTES: usize = 2;
const BATCH_RELEASE_OFFSET: usize = 16;
const BATCH_MARKET_OFFSET: usize = 48;
const BATCH_REALM_OFFSET: usize = 80;
const BATCH_TRADING_OFFSET: usize = 112;
const BATCH_CHECKPOINT_OFFSET: usize = 144;
const BATCH_REQUEST_OFFSET: usize = 176;
const BATCH_EFFECTS_OFFSET: usize = 208;
const BATCH_REPLAY_OFFSET: usize = 240;
const BATCH_REPLAY_PRESTATE_OFFSET: usize = 272;
const BATCH_REFUND_OFFSET: usize = 304;
const BATCH_EXPIRES_OFFSET: usize = 336;
const BATCH_GENERATION_OFFSET: usize = 344;
const BATCH_STATES_OFFSET: usize = 352;
const BATCH_RECEIPTS_OFFSET: usize = 480;
const BATCH_LAST_PRESTATE_OFFSET: usize = 608;

/// Every coordinate of the reservation STATE, under the short local names
/// its encoder and hostile decoder have always used.
///
/// The `STATE_*` block this replaces was nineteen file-private constants
/// that agreed with the record only by inspection.
/// `DClutchSemantics.DealerScenarioReservationStateV1Abi` places them, and
/// the three coordinates this record used to reach through the family's
/// shared `TAG_OFFSET`, `ORDINAL_OFFSET` and `COUNT_OFFSET` now have names
/// of their own -- because those three constants are read by three other
/// records in this file that have no Lean owner, so a shared name cannot be
/// any one record's author.
use crate::generated_scenario_reservation_state_v1::{
    DEALER_SCENARIO_RESERVATION_STATE_AMOUNT_OFFSET_V1 as STATE_AMOUNT_OFFSET,
    DEALER_SCENARIO_RESERVATION_STATE_BATCH_OFFSET_V1 as STATE_BATCH_OFFSET,
    DEALER_SCENARIO_RESERVATION_STATE_CHECKPOINT_OFFSET_V1 as STATE_CHECKPOINT_OFFSET,
    DEALER_SCENARIO_RESERVATION_STATE_DESTINATION_BEFORE_OFFSET_V1 as STATE_DESTINATION_BEFORE_OFFSET,
    DEALER_SCENARIO_RESERVATION_STATE_DESTINATION_OFFSET_V1 as STATE_DESTINATION_OFFSET,
    DEALER_SCENARIO_RESERVATION_STATE_DESTINATION_PRESTATE_OFFSET_V1 as STATE_DESTINATION_PRESTATE_OFFSET,
    DEALER_SCENARIO_RESERVATION_STATE_EFFECT_COUNT_OFFSET_V1 as STATE_EFFECT_COUNT_OFFSET,
    DEALER_SCENARIO_RESERVATION_STATE_EFFECT_DIGEST_OFFSET_V1 as STATE_EFFECT_DIGEST_OFFSET,
    DEALER_SCENARIO_RESERVATION_STATE_EFFECTS_OFFSET_V1 as STATE_EFFECTS_OFFSET,
    DEALER_SCENARIO_RESERVATION_STATE_ESCROW_AFTER_OFFSET_V1 as STATE_ESCROW_AFTER_OFFSET,
    DEALER_SCENARIO_RESERVATION_STATE_ESCROW_OFFSET_V1 as STATE_ESCROW_OFFSET,
    DEALER_SCENARIO_RESERVATION_STATE_ESCROW_POSTSTATE_OFFSET_V1 as STATE_ESCROW_POSTSTATE_OFFSET,
    DEALER_SCENARIO_RESERVATION_STATE_HEAD_RESERVED_BYTES_V1 as STATE_HEAD_RESERVED_BYTES,
    DEALER_SCENARIO_RESERVATION_STATE_HEAD_RESERVED_OFFSET_V1 as STATE_HEAD_RESERVED_OFFSET,
    DEALER_SCENARIO_RESERVATION_STATE_MINT_OFFSET_V1 as STATE_MINT_OFFSET,
    DEALER_SCENARIO_RESERVATION_STATE_ORDINAL_OFFSET_V1 as STATE_ORDINAL_OFFSET,
    DEALER_SCENARIO_RESERVATION_STATE_REQUEST_OFFSET_V1 as STATE_REQUEST_OFFSET,
    DEALER_SCENARIO_RESERVATION_STATE_RESERVED_BYTES_V1 as STATE_RESERVED_BYTES,
    DEALER_SCENARIO_RESERVATION_STATE_RESERVED_OFFSET_V1 as STATE_RESERVED_OFFSET,
    DEALER_SCENARIO_RESERVATION_STATE_SOURCE_AFTER_OFFSET_V1 as STATE_SOURCE_AFTER_OFFSET,
    DEALER_SCENARIO_RESERVATION_STATE_SOURCE_OFFSET_V1 as STATE_SOURCE_OFFSET,
    DEALER_SCENARIO_RESERVATION_STATE_SOURCE_POSTSTATE_OFFSET_V1 as STATE_SOURCE_POSTSTATE_OFFSET,
    DEALER_SCENARIO_RESERVATION_STATE_SOURCE_PRESTATE_OFFSET_V1 as STATE_SOURCE_PRESTATE_OFFSET,
    DEALER_SCENARIO_RESERVATION_STATE_STATUS_OFFSET_V1 as STATE_STATUS_OFFSET,
    DEALER_SCENARIO_RESERVATION_STATE_TOKEN_PROGRAM_OFFSET_V1 as STATE_TOKEN_PROGRAM_OFFSET,
};
use crate::generated_scenario_reservation_state_v1::{
    DEALER_SCENARIO_RESERVATION_STATE_VERSION_OFFSET_V1,
    DEALER_SCENARIO_RESERVATION_STATUS_ACTIVATED_V1, DEALER_SCENARIO_RESERVATION_STATUS_ACTIVE_V1,
    DEALER_SCENARIO_RESERVATION_STATUS_ROLLED_BACK_V1,
};

/// The family header three other records in this file still author.
///
/// `VERSION_OFFSET`, `TAG_OFFSET`, `ORDINAL_OFFSET` and `COUNT_OFFSET` are
/// read by the custody effect, the effect manifest and the reservation
/// batch as well as by this state, and `require_header`/`put_header` read
/// the version through the first of them for all four. None of those three
/// records has a Lean module yet, so the shared block stays where they need
/// it and this pins it to the one record that does: if Lean ever moves a
/// coordinate of the reservation state, the shared header stops describing
/// it and a compiler says which. That is named debt, and the next lane to
/// own one of the other three can take the block apart.
const _: () = assert!(
    VERSION_OFFSET == DEALER_SCENARIO_RESERVATION_STATE_VERSION_OFFSET_V1
        && TAG_OFFSET == STATE_STATUS_OFFSET
        && ORDINAL_OFFSET == STATE_ORDINAL_OFFSET
        && COUNT_OFFSET == STATE_EFFECT_COUNT_OFFSET,
    "the shared scenario-custody header stopped describing the reservation state"
);

const ACTIVATION_PRODUCER_OFFSET: usize = 16;
const ACTIVATION_CHECKPOINT_OFFSET: usize = 48;
const ACTIVATION_CHECKPOINT_PRESTATE_OFFSET: usize = 80;
const ACTIVATION_REQUEST_OFFSET: usize = 112;
const ACTIVATION_EFFECTS_OFFSET: usize = 144;
const ACTIVATION_BATCH_OFFSET: usize = 176;
const ACTIVATION_BATCH_PRESTATE_OFFSET: usize = 208;
const ACTIVATION_BATCH_POSTSTATE_OFFSET: usize = 240;
const ACTIVATION_REPLAY_PRESTATE_OFFSET: usize = 272;
const ACTIVATION_REPLAY_POSTSTATE_OFFSET: usize = 304;

/// Nested canonical Custody request encoding.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerScenarioCustodyRequestKindV1 {
    /// Ordinary Custody V1 transfer.
    Canonical = 1,
    /// Delegated-allowance Custody V2 transfer.
    Delegated = 2,
}

impl DealerScenarioCustodyRequestKindV1 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Canonical),
            2 => Ok(Self::Delegated),
            _ => Err(Error::UnknownTag),
        }
    }

    /// Exact active nested request width.
    pub const fn request_bytes(self) -> usize {
        match self {
            Self::Canonical => DEALER_SCENARIO_CANONICAL_CUSTODY_REQUEST_BYTES_V1,
            Self::Delegated => DEALER_SCENARIO_DELEGATED_CUSTODY_REQUEST_BYTES_V1,
        }
    }
}

/// One exact evaluator-owned Custody effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerScenarioCustodyEffectV1 {
    /// Canonical or delegated nested request.
    pub kind: DealerScenarioCustodyRequestKindV1,
    /// Zero-based execution ordinal.
    pub ordinal: u8,
    /// Exact active effect count.
    pub effect_count: u8,
    /// Evaluator program owning this effect account.
    pub producer_program: [u8; 32],
    /// Trading checkpoint this effect serves.
    pub checkpoint: [u8; 32],
    /// Exact Dealer request digest.
    pub request_digest: [u8; 32],
    /// Required original source balance after delivery.
    pub source_after: u64,
    /// Required original destination balance after delivery.
    pub destination_after: u64,
    /// Exact nested request bytes followed only by zero padding.
    pub request_payload: [u8; DEALER_SCENARIO_DELEGATED_CUSTODY_REQUEST_BYTES_V1],
}

impl DealerScenarioCustodyEffectV1 {
    /// Hostile-decode one effect envelope.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        require_header(
            bytes,
            DEALER_SCENARIO_CUSTODY_EFFECT_BYTES_V1,
            &DEALER_SCENARIO_CUSTODY_EFFECT_MAGIC_V1,
        )?;
        require_zero(bytes, EFFECT_RESERVED_OFFSET, EFFECT_RESERVED_BYTES)?;
        let kind = DealerScenarioCustodyRequestKindV1::decode(byte_at(bytes, TAG_OFFSET)?)?;
        let request_length = read_u16(bytes, EFFECT_REQUEST_LENGTH_OFFSET)?;
        if usize::from(request_length) != kind.request_bytes() {
            return Err(Error::InvalidLength);
        }
        let mut request_payload = [0_u8; DEALER_SCENARIO_DELEGATED_CUSTODY_REQUEST_BYTES_V1];
        let payload_bytes = request_payload.len();
        request_payload.copy_from_slice(
            bytes
                .get(EFFECT_PAYLOAD_OFFSET..EFFECT_PAYLOAD_OFFSET + payload_bytes)
                .ok_or(Error::InvalidLength)?,
        );
        if request_payload
            .get(kind.request_bytes()..)
            .ok_or(Error::InvalidLength)?
            .iter()
            .any(|byte| *byte != 0)
        {
            return Err(Error::NonCanonicalPadding);
        }
        let value = Self {
            kind,
            ordinal: byte_at(bytes, ORDINAL_OFFSET)?,
            effect_count: byte_at(bytes, COUNT_OFFSET)?,
            producer_program: array_at(bytes, EFFECT_PRODUCER_OFFSET)?,
            checkpoint: array_at(bytes, EFFECT_CHECKPOINT_OFFSET)?,
            request_digest: array_at(bytes, EFFECT_REQUEST_OFFSET)?,
            source_after: u64_at(bytes, EFFECT_SOURCE_AFTER_OFFSET)?,
            destination_after: u64_at(bytes, EFFECT_DESTINATION_AFTER_OFFSET)?,
            request_payload,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode one exact effect envelope.
    pub fn encode(self) -> Result<[u8; DEALER_SCENARIO_CUSTODY_EFFECT_BYTES_V1]> {
        self.validate()?;
        let mut bytes = [0_u8; DEALER_SCENARIO_CUSTODY_EFFECT_BYTES_V1];
        put_header(&mut bytes, &DEALER_SCENARIO_CUSTODY_EFFECT_MAGIC_V1)?;
        put_byte(&mut bytes, TAG_OFFSET, self.kind as u8)?;
        put_byte(&mut bytes, ORDINAL_OFFSET, self.ordinal)?;
        put_byte(&mut bytes, COUNT_OFFSET, self.effect_count)?;
        for (offset, value) in [
            (EFFECT_PRODUCER_OFFSET, self.producer_program),
            (EFFECT_CHECKPOINT_OFFSET, self.checkpoint),
            (EFFECT_REQUEST_OFFSET, self.request_digest),
        ] {
            put(&mut bytes, offset, &value)?;
        }
        put_u64(&mut bytes, EFFECT_SOURCE_AFTER_OFFSET, self.source_after)?;
        put_u64(
            &mut bytes,
            EFFECT_DESTINATION_AFTER_OFFSET,
            self.destination_after,
        )?;
        put_u16(
            &mut bytes,
            EFFECT_REQUEST_LENGTH_OFFSET,
            u16::try_from(self.kind.request_bytes()).map_err(|_| Error::ArithmeticOverflow)?,
        )?;
        put(&mut bytes, EFFECT_PAYLOAD_OFFSET, &self.request_payload)?;
        Ok(bytes)
    }

    /// Borrow the exact active nested request bytes.
    pub fn request_bytes(&self) -> &[u8] {
        self.request_payload
            .get(..self.kind.request_bytes())
            .unwrap_or(&[])
    }

    fn validate(self) -> Result<()> {
        if self.effect_count == 0
            || usize::from(self.effect_count) > DEALER_SCENARIO_MAX_RESERVATIONS_V1
            || self.ordinal >= self.effect_count
            || [self.producer_program, self.checkpoint, self.request_digest].contains(&[0; 32])
            || self
                .request_payload
                .get(..self.kind.request_bytes())
                .is_none()
            || self
                .request_payload
                .get(self.kind.request_bytes()..)
                .ok_or(Error::InvalidLength)?
                .iter()
                .any(|byte| *byte != 0)
        {
            Err(Error::ZeroCoordinate)
        } else {
            Ok(())
        }
    }
}

/// Manifest binding the complete ordered effect-account bank.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerScenarioCustodyEffectManifestV1 {
    /// Exact active prefix length.
    pub effect_count: u8,
    /// Evaluator program owning manifest and effects.
    pub producer_program: [u8; 32],
    /// Trading checkpoint.
    pub checkpoint: [u8; 32],
    /// Exact Dealer request digest.
    pub request_digest: [u8; 32],
    /// Ordered effect account identities; inactive suffix is zero.
    pub effect_accounts: [[u8; 32]; DEALER_SCENARIO_MAX_RESERVATIONS_V1],
    /// Ordered exact effect body digests; inactive suffix is zero.
    pub effect_digests: [[u8; 32]; DEALER_SCENARIO_MAX_RESERVATIONS_V1],
}

impl DealerScenarioCustodyEffectManifestV1 {
    /// Hostile-decode one exact manifest.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        require_header(
            bytes,
            DEALER_SCENARIO_CUSTODY_EFFECT_MANIFEST_BYTES_V1,
            &DEALER_SCENARIO_CUSTODY_EFFECT_MANIFEST_MAGIC_V1,
        )?;
        require_zero(bytes, 11, 5)?;
        require_zero(bytes, MANIFEST_RESERVED_OFFSET, MANIFEST_RESERVED_BYTES)?;
        let value = Self {
            effect_count: byte_at(bytes, TAG_OFFSET)?,
            producer_program: array_at(bytes, MANIFEST_PRODUCER_OFFSET)?,
            checkpoint: array_at(bytes, MANIFEST_CHECKPOINT_OFFSET)?,
            request_digest: array_at(bytes, MANIFEST_REQUEST_OFFSET)?,
            effect_accounts: read_identity_bank(bytes, MANIFEST_ACCOUNTS_OFFSET)?,
            effect_digests: read_identity_bank(bytes, MANIFEST_DIGESTS_OFFSET)?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode one exact manifest.
    pub fn encode(self) -> Result<[u8; DEALER_SCENARIO_CUSTODY_EFFECT_MANIFEST_BYTES_V1]> {
        self.validate()?;
        let mut bytes = [0_u8; DEALER_SCENARIO_CUSTODY_EFFECT_MANIFEST_BYTES_V1];
        put_header(
            &mut bytes,
            &DEALER_SCENARIO_CUSTODY_EFFECT_MANIFEST_MAGIC_V1,
        )?;
        put_byte(&mut bytes, TAG_OFFSET, self.effect_count)?;
        for (offset, value) in [
            (MANIFEST_PRODUCER_OFFSET, self.producer_program),
            (MANIFEST_CHECKPOINT_OFFSET, self.checkpoint),
            (MANIFEST_REQUEST_OFFSET, self.request_digest),
        ] {
            put(&mut bytes, offset, &value)?;
        }
        put_identity_bank(&mut bytes, MANIFEST_ACCOUNTS_OFFSET, &self.effect_accounts)?;
        put_identity_bank(&mut bytes, MANIFEST_DIGESTS_OFFSET, &self.effect_digests)?;
        Ok(bytes)
    }

    fn validate(self) -> Result<()> {
        let active = usize::from(self.effect_count);
        if active == 0
            || active > DEALER_SCENARIO_MAX_RESERVATIONS_V1
            || [self.producer_program, self.checkpoint, self.request_digest].contains(&[0; 32])
        {
            return Err(Error::ZeroCoordinate);
        }
        for index in 0..DEALER_SCENARIO_MAX_RESERVATIONS_V1 {
            let account = *self
                .effect_accounts
                .get(index)
                .ok_or(Error::InvalidLength)?;
            let digest = *self.effect_digests.get(index).ok_or(Error::InvalidLength)?;
            if (index < active) != (account != [0; 32] && digest != [0; 32])
                || (index < active
                    && self
                        .effect_accounts
                        .get(..index)
                        .ok_or(Error::InvalidLength)?
                        .contains(&account))
            {
                return Err(Error::IdentityMismatch);
            }
        }
        Ok(())
    }
}

/// Reservation-batch lifecycle.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerScenarioReservationBatchStatusV1 {
    /// Ordered source debits are still being locked.
    Reserving = 1,
    /// Every effect is locked and may activate atomically.
    Reserved = 2,
    /// Expired effects are rolling back in reverse order.
    RollingBack = 3,
    /// Every escrow was delivered and the Custody replay advanced.
    Activated = 4,
}

impl DealerScenarioReservationBatchStatusV1 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Reserving),
            2 => Ok(Self::Reserved),
            3 => Ok(Self::RollingBack),
            4 => Ok(Self::Activated),
            _ => Err(Error::UnknownTag),
        }
    }
}

/// Custody-owned ordered batch shared by every effect state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerScenarioReservationBatchV1 {
    /// Current lifecycle.
    pub status: DealerScenarioReservationBatchStatusV1,
    /// Evaluator-selected active effect count.
    pub effect_count: u8,
    /// Ordered locked prefix length.
    pub reserved_count: u8,
    /// Reverse-order released suffix length.
    pub rollback_count: u8,
    /// Selected release set.
    pub release_set: [u8; 32],
    /// Core Market.
    pub market: [u8; 32],
    /// Immutable Realm.
    pub realm: [u8; 32],
    /// Release-selected Trading program.
    pub trading_program: [u8; 32],
    /// Trading checkpoint.
    pub checkpoint: [u8; 32],
    /// Exact Dealer request digest.
    pub request_digest: [u8; 32],
    /// Exact effect manifest digest.
    pub effects_digest: [u8; 32],
    /// Standard Custody replay account which activation advances.
    pub replay: [u8; 32],
    /// Exact replay body before any reservation.
    pub replay_prestate_digest: [u8; 32],
    /// Sole escrow-rent refund beneficiary.
    pub refund_beneficiary: [u8; 32],
    /// Last live slot for forward activation.
    pub expires_at: u64,
    /// Market generation.
    pub generation: u64,
    /// Ordered reservation-state PDAs.
    pub reservation_states: [[u8; 32]; DEALER_SCENARIO_MAX_RESERVATIONS_V1],
    /// Reserve receipts, overwritten by reverse rollback receipts.
    pub receipt_digests: [[u8; 32]; DEALER_SCENARIO_MAX_RESERVATIONS_V1],
    /// Exact prior batch body digest for the last transition.
    pub last_prestate_digest: [u8; 32],
}

impl DealerScenarioReservationBatchV1 {
    /// Create an empty ordered batch.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        effect_count: u8,
        release_set: [u8; 32],
        market: [u8; 32],
        realm: [u8; 32],
        trading_program: [u8; 32],
        checkpoint: [u8; 32],
        request_digest: [u8; 32],
        effects_digest: [u8; 32],
        replay: [u8; 32],
        replay_prestate_digest: [u8; 32],
        refund_beneficiary: [u8; 32],
        expires_at: u64,
        generation: u64,
    ) -> Result<Self> {
        let value = Self {
            status: DealerScenarioReservationBatchStatusV1::Reserving,
            effect_count,
            reserved_count: 0,
            rollback_count: 0,
            release_set,
            market,
            realm,
            trading_program,
            checkpoint,
            request_digest,
            effects_digest,
            replay,
            replay_prestate_digest,
            refund_beneficiary,
            expires_at,
            generation,
            reservation_states: [[0; 32]; DEALER_SCENARIO_MAX_RESERVATIONS_V1],
            receipt_digests: [[0; 32]; DEALER_SCENARIO_MAX_RESERVATIONS_V1],
            last_prestate_digest: [0; 32],
        };
        value.validate()?;
        Ok(value)
    }

    /// Append one ordered locked effect.
    pub fn append_reserve(
        self,
        slot: u64,
        ordinal: u8,
        prestate_digest: [u8; 32],
        state: [u8; 32],
        receipt: [u8; 32],
    ) -> Result<Self> {
        if self.status != DealerScenarioReservationBatchStatusV1::Reserving
            || slot > self.expires_at
            || ordinal != self.reserved_count
            || ordinal >= self.effect_count
            || [prestate_digest, state, receipt].contains(&[0; 32])
        {
            return Err(Error::InvalidPhase);
        }
        let mut next = self;
        *next
            .reservation_states
            .get_mut(usize::from(ordinal))
            .ok_or(Error::PlanOverflow)? = state;
        *next
            .receipt_digests
            .get_mut(usize::from(ordinal))
            .ok_or(Error::PlanOverflow)? = receipt;
        next.reserved_count = next
            .reserved_count
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        next.last_prestate_digest = prestate_digest;
        if next.reserved_count == next.effect_count {
            next.status = DealerScenarioReservationBatchStatusV1::Reserved;
        }
        next.validate()?;
        Ok(next)
    }

    /// Append one expired reverse-order rollback.
    pub fn append_rollback(
        self,
        slot: u64,
        ordinal: u8,
        prestate_digest: [u8; 32],
        prior_receipt: [u8; 32],
        rollback_receipt: [u8; 32],
    ) -> Result<Self> {
        if !matches!(
            self.status,
            DealerScenarioReservationBatchStatusV1::Reserving
                | DealerScenarioReservationBatchStatusV1::Reserved
                | DealerScenarioReservationBatchStatusV1::RollingBack
        ) || slot <= self.expires_at
            || self.reserved_count == self.rollback_count
        {
            return Err(Error::InvalidPhase);
        }
        let expected = self
            .reserved_count
            .checked_sub(self.rollback_count)
            .and_then(|value| value.checked_sub(1))
            .ok_or(Error::StaleCoordinate)?;
        if ordinal != expected
            || [prestate_digest, prior_receipt, rollback_receipt].contains(&[0; 32])
            || self.receipt_digests.get(usize::from(ordinal)).copied() != Some(prior_receipt)
        {
            return Err(Error::StaleCoordinate);
        }
        let mut next = self;
        *next
            .receipt_digests
            .get_mut(usize::from(ordinal))
            .ok_or(Error::PlanOverflow)? = rollback_receipt;
        next.rollback_count = next
            .rollback_count
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        next.last_prestate_digest = prestate_digest;
        next.status = DealerScenarioReservationBatchStatusV1::RollingBack;
        next.validate()?;
        Ok(next)
    }

    /// Mark one complete committed batch activated.
    ///
    /// The Trading checkpoint, authenticated by the Custody adapter, is the
    /// sole authorization. Completion remains valid after preparation expiry
    /// so a crash cannot strand committed liabilities behind an elapsed slot.
    pub fn activate_committed(self, prestate_digest: [u8; 32]) -> Result<Self> {
        if self.status != DealerScenarioReservationBatchStatusV1::Reserved
            || prestate_digest == [0; 32]
        {
            return Err(Error::InvalidPhase);
        }
        let mut next = self;
        next.status = DealerScenarioReservationBatchStatusV1::Activated;
        next.last_prestate_digest = prestate_digest;
        next.validate()?;
        Ok(next)
    }

    /// Hostile-decode one exact batch.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        require_header(
            bytes,
            DEALER_SCENARIO_RESERVATION_BATCH_BYTES_V1,
            &DEALER_SCENARIO_RESERVATION_BATCH_MAGIC_V1,
        )?;
        require_zero(bytes, BATCH_RESERVED_OFFSET, BATCH_RESERVED_BYTES)?;
        let value = Self {
            status: DealerScenarioReservationBatchStatusV1::decode(byte_at(bytes, TAG_OFFSET)?)?,
            effect_count: byte_at(bytes, ORDINAL_OFFSET)?,
            reserved_count: byte_at(bytes, BATCH_RESERVED_COUNT_OFFSET)?,
            rollback_count: byte_at(bytes, BATCH_ROLLBACK_COUNT_OFFSET)?,
            release_set: array_at(bytes, BATCH_RELEASE_OFFSET)?,
            market: array_at(bytes, BATCH_MARKET_OFFSET)?,
            realm: array_at(bytes, BATCH_REALM_OFFSET)?,
            trading_program: array_at(bytes, BATCH_TRADING_OFFSET)?,
            checkpoint: array_at(bytes, BATCH_CHECKPOINT_OFFSET)?,
            request_digest: array_at(bytes, BATCH_REQUEST_OFFSET)?,
            effects_digest: array_at(bytes, BATCH_EFFECTS_OFFSET)?,
            replay: array_at(bytes, BATCH_REPLAY_OFFSET)?,
            replay_prestate_digest: array_at(bytes, BATCH_REPLAY_PRESTATE_OFFSET)?,
            refund_beneficiary: array_at(bytes, BATCH_REFUND_OFFSET)?,
            expires_at: u64_at(bytes, BATCH_EXPIRES_OFFSET)?,
            generation: u64_at(bytes, BATCH_GENERATION_OFFSET)?,
            reservation_states: read_identity_bank(bytes, BATCH_STATES_OFFSET)?,
            receipt_digests: read_identity_bank(bytes, BATCH_RECEIPTS_OFFSET)?,
            last_prestate_digest: array_at(bytes, BATCH_LAST_PRESTATE_OFFSET)?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode one exact batch.
    pub fn encode(self) -> Result<[u8; DEALER_SCENARIO_RESERVATION_BATCH_BYTES_V1]> {
        self.validate()?;
        let mut bytes = [0_u8; DEALER_SCENARIO_RESERVATION_BATCH_BYTES_V1];
        put_header(&mut bytes, &DEALER_SCENARIO_RESERVATION_BATCH_MAGIC_V1)?;
        put_byte(&mut bytes, TAG_OFFSET, self.status as u8)?;
        put_byte(&mut bytes, ORDINAL_OFFSET, self.effect_count)?;
        put_byte(&mut bytes, BATCH_RESERVED_COUNT_OFFSET, self.reserved_count)?;
        put_byte(&mut bytes, BATCH_ROLLBACK_COUNT_OFFSET, self.rollback_count)?;
        for (offset, value) in [
            (BATCH_RELEASE_OFFSET, self.release_set),
            (BATCH_MARKET_OFFSET, self.market),
            (BATCH_REALM_OFFSET, self.realm),
            (BATCH_TRADING_OFFSET, self.trading_program),
            (BATCH_CHECKPOINT_OFFSET, self.checkpoint),
            (BATCH_REQUEST_OFFSET, self.request_digest),
            (BATCH_EFFECTS_OFFSET, self.effects_digest),
            (BATCH_REPLAY_OFFSET, self.replay),
            (BATCH_REPLAY_PRESTATE_OFFSET, self.replay_prestate_digest),
            (BATCH_REFUND_OFFSET, self.refund_beneficiary),
            (BATCH_LAST_PRESTATE_OFFSET, self.last_prestate_digest),
        ] {
            put(&mut bytes, offset, &value)?;
        }
        put_u64(&mut bytes, BATCH_EXPIRES_OFFSET, self.expires_at)?;
        put_u64(&mut bytes, BATCH_GENERATION_OFFSET, self.generation)?;
        put_identity_bank(&mut bytes, BATCH_STATES_OFFSET, &self.reservation_states)?;
        put_identity_bank(&mut bytes, BATCH_RECEIPTS_OFFSET, &self.receipt_digests)?;
        Ok(bytes)
    }

    fn validate(self) -> Result<()> {
        let count = usize::from(self.effect_count);
        if count == 0
            || count > DEALER_SCENARIO_MAX_RESERVATIONS_V1
            || self.reserved_count > self.effect_count
            || self.rollback_count > self.reserved_count
            || self.expires_at == 0
            || self.generation == 0
            || [
                self.release_set,
                self.market,
                self.realm,
                self.trading_program,
                self.checkpoint,
                self.request_digest,
                self.effects_digest,
                self.replay,
                self.replay_prestate_digest,
                self.refund_beneficiary,
            ]
            .contains(&[0; 32])
        {
            return Err(Error::ZeroCoordinate);
        }
        for index in 0..DEALER_SCENARIO_MAX_RESERVATIONS_V1 {
            let populated = index < usize::from(self.reserved_count);
            if populated
                != (self.reservation_states.get(index).copied() != Some([0; 32])
                    && self.receipt_digests.get(index).copied() != Some([0; 32]))
            {
                return Err(Error::InvalidPhase);
            }
        }
        let phase_valid = match self.status {
            DealerScenarioReservationBatchStatusV1::Reserving => {
                self.reserved_count < self.effect_count && self.rollback_count == 0
            }
            DealerScenarioReservationBatchStatusV1::Reserved => {
                self.reserved_count == self.effect_count && self.rollback_count == 0
            }
            DealerScenarioReservationBatchStatusV1::RollingBack => self.rollback_count != 0,
            DealerScenarioReservationBatchStatusV1::Activated => {
                self.reserved_count == self.effect_count && self.rollback_count == 0
            }
        };
        if !phase_valid {
            return Err(Error::InvalidPhase);
        }
        Ok(())
    }
}

/// Per-effect Custody state proving source value entered one escrow.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerScenarioReservationStateV1 {
    /// Active, rolled back, or activated.
    pub status: DealerScenarioReservationStateStatusV1,
    /// Effect ordinal.
    pub ordinal: u8,
    /// Exact batch effect count.
    pub effect_count: u8,
    /// Custody reservation batch.
    pub batch: [u8; 32],
    /// Trading checkpoint.
    pub checkpoint: [u8; 32],
    /// Dealer request digest.
    pub request_digest: [u8; 32],
    /// Effect manifest digest.
    pub effects_digest: [u8; 32],
    /// Exact effect body digest.
    pub effect_digest: [u8; 32],
    /// Original source token account.
    pub source: [u8; 32],
    /// Original final destination token account.
    pub destination: [u8; 32],
    /// Checkpoint-scoped escrow token account.
    pub escrow: [u8; 32],
    /// Realm-selected Mint.
    pub mint: [u8; 32],
    /// Realm-selected token program.
    pub token_program: [u8; 32],
    /// Source token account prestate digest.
    pub source_prestate_digest: [u8; 32],
    /// Destination token account prestate digest.
    pub destination_prestate_digest: [u8; 32],
    /// Status-specific effect account digest: escrow while active, returned
    /// source after rollback, or final destination after activation.
    pub effect_poststate_digest: [u8; 32],
    /// Source token account post-reserve digest.
    ///
    /// The state cannot carry its own receipt digest: the receipt commits this
    /// exact state body, so doing so would create an unresolvable hash cycle.
    pub source_poststate_digest: [u8; 32],
    /// Positive locked token amount.
    pub amount: u64,
    /// Required source balance after reserve.
    pub source_after: u64,
    /// Destination balance before activation.
    pub destination_before: u64,
    /// Required escrow balance while active.
    pub escrow_after: u64,
}

/// Per-effect reservation lifecycle.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerScenarioReservationStateStatusV1 {
    /// Value is held in escrow.
    Active = DEALER_SCENARIO_RESERVATION_STATUS_ACTIVE_V1,
    /// Value returned to the original source after expiry.
    RolledBack = DEALER_SCENARIO_RESERVATION_STATUS_ROLLED_BACK_V1,
    /// Value delivered to the original destination.
    Activated = DEALER_SCENARIO_RESERVATION_STATUS_ACTIVATED_V1,
}

impl DealerScenarioReservationStateStatusV1 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            DEALER_SCENARIO_RESERVATION_STATUS_ACTIVE_V1 => Ok(Self::Active),
            DEALER_SCENARIO_RESERVATION_STATUS_ROLLED_BACK_V1 => Ok(Self::RolledBack),
            DEALER_SCENARIO_RESERVATION_STATUS_ACTIVATED_V1 => Ok(Self::Activated),
            _ => Err(Error::UnknownTag),
        }
    }
}

impl DealerScenarioReservationStateV1 {
    /// Hostile-decode one exact state.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        require_header(
            bytes,
            DEALER_SCENARIO_RESERVATION_STATE_BYTES_V1,
            &DEALER_SCENARIO_RESERVATION_STATE_MAGIC_V1,
        )?;
        require_zero(bytes, STATE_HEAD_RESERVED_OFFSET, STATE_HEAD_RESERVED_BYTES)?;
        require_zero(bytes, STATE_RESERVED_OFFSET, STATE_RESERVED_BYTES)?;
        let value = Self {
            status: DealerScenarioReservationStateStatusV1::decode(byte_at(
                bytes,
                STATE_STATUS_OFFSET,
            )?)?,
            ordinal: byte_at(bytes, STATE_ORDINAL_OFFSET)?,
            effect_count: byte_at(bytes, STATE_EFFECT_COUNT_OFFSET)?,
            batch: array_at(bytes, STATE_BATCH_OFFSET)?,
            checkpoint: array_at(bytes, STATE_CHECKPOINT_OFFSET)?,
            request_digest: array_at(bytes, STATE_REQUEST_OFFSET)?,
            effects_digest: array_at(bytes, STATE_EFFECTS_OFFSET)?,
            effect_digest: array_at(bytes, STATE_EFFECT_DIGEST_OFFSET)?,
            source: array_at(bytes, STATE_SOURCE_OFFSET)?,
            destination: array_at(bytes, STATE_DESTINATION_OFFSET)?,
            escrow: array_at(bytes, STATE_ESCROW_OFFSET)?,
            mint: array_at(bytes, STATE_MINT_OFFSET)?,
            token_program: array_at(bytes, STATE_TOKEN_PROGRAM_OFFSET)?,
            source_prestate_digest: array_at(bytes, STATE_SOURCE_PRESTATE_OFFSET)?,
            destination_prestate_digest: array_at(bytes, STATE_DESTINATION_PRESTATE_OFFSET)?,
            effect_poststate_digest: array_at(bytes, STATE_ESCROW_POSTSTATE_OFFSET)?,
            source_poststate_digest: array_at(bytes, STATE_SOURCE_POSTSTATE_OFFSET)?,
            amount: u64_at(bytes, STATE_AMOUNT_OFFSET)?,
            source_after: u64_at(bytes, STATE_SOURCE_AFTER_OFFSET)?,
            destination_before: u64_at(bytes, STATE_DESTINATION_BEFORE_OFFSET)?,
            escrow_after: u64_at(bytes, STATE_ESCROW_AFTER_OFFSET)?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode one exact state.
    pub fn encode(self) -> Result<[u8; DEALER_SCENARIO_RESERVATION_STATE_BYTES_V1]> {
        self.validate()?;
        let mut bytes = [0_u8; DEALER_SCENARIO_RESERVATION_STATE_BYTES_V1];
        put_header(&mut bytes, &DEALER_SCENARIO_RESERVATION_STATE_MAGIC_V1)?;
        put_byte(&mut bytes, STATE_STATUS_OFFSET, self.status as u8)?;
        put_byte(&mut bytes, STATE_ORDINAL_OFFSET, self.ordinal)?;
        put_byte(&mut bytes, STATE_EFFECT_COUNT_OFFSET, self.effect_count)?;
        for (offset, value) in [
            (STATE_BATCH_OFFSET, self.batch),
            (STATE_CHECKPOINT_OFFSET, self.checkpoint),
            (STATE_REQUEST_OFFSET, self.request_digest),
            (STATE_EFFECTS_OFFSET, self.effects_digest),
            (STATE_EFFECT_DIGEST_OFFSET, self.effect_digest),
            (STATE_SOURCE_OFFSET, self.source),
            (STATE_DESTINATION_OFFSET, self.destination),
            (STATE_ESCROW_OFFSET, self.escrow),
            (STATE_MINT_OFFSET, self.mint),
            (STATE_TOKEN_PROGRAM_OFFSET, self.token_program),
            (STATE_SOURCE_PRESTATE_OFFSET, self.source_prestate_digest),
            (
                STATE_DESTINATION_PRESTATE_OFFSET,
                self.destination_prestate_digest,
            ),
            (STATE_ESCROW_POSTSTATE_OFFSET, self.effect_poststate_digest),
            (STATE_SOURCE_POSTSTATE_OFFSET, self.source_poststate_digest),
        ] {
            put(&mut bytes, offset, &value)?;
        }
        for (offset, value) in [
            (STATE_AMOUNT_OFFSET, self.amount),
            (STATE_SOURCE_AFTER_OFFSET, self.source_after),
            (STATE_DESTINATION_BEFORE_OFFSET, self.destination_before),
            (STATE_ESCROW_AFTER_OFFSET, self.escrow_after),
        ] {
            put_u64(&mut bytes, offset, value)?;
        }
        Ok(bytes)
    }

    fn validate(self) -> Result<()> {
        if self.effect_count == 0
            || usize::from(self.effect_count) > DEALER_SCENARIO_MAX_RESERVATIONS_V1
            || self.ordinal >= self.effect_count
            || self.amount == 0
            || match self.status {
                DealerScenarioReservationStateStatusV1::Active => self.escrow_after != self.amount,
                DealerScenarioReservationStateStatusV1::RolledBack
                | DealerScenarioReservationStateStatusV1::Activated => self.escrow_after != 0,
            }
            || [
                self.batch,
                self.checkpoint,
                self.request_digest,
                self.effects_digest,
                self.effect_digest,
                self.source,
                self.destination,
                self.escrow,
                self.mint,
                self.token_program,
                self.source_prestate_digest,
                self.destination_prestate_digest,
                self.effect_poststate_digest,
                self.source_poststate_digest,
            ]
            .contains(&[0; 32])
            || self.source == self.destination
            || self.source == self.escrow
            || self.destination == self.escrow
        {
            Err(Error::ZeroCoordinate)
        } else {
            Ok(())
        }
    }
}

/// Receipt returned only after every escrow and replay poststate is verified.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerScenarioActivationReceiptV1 {
    /// Release-selected Custody program.
    pub producer_program: [u8; 32],
    /// Trading checkpoint.
    pub checkpoint: [u8; 32],
    /// Exact checkpoint bytes observed by Custody.
    pub checkpoint_prestate_digest: [u8; 32],
    /// Dealer request digest.
    pub request_digest: [u8; 32],
    /// Effect manifest digest.
    pub effects_digest: [u8; 32],
    /// Custody reservation batch.
    pub batch: [u8; 32],
    /// Exact batch bytes before activation.
    pub batch_prestate_digest: [u8; 32],
    /// Exact batch bytes after activation.
    pub batch_poststate_digest: [u8; 32],
    /// Standard Custody replay bytes before activation.
    pub replay_prestate_digest: [u8; 32],
    /// Standard Custody replay bytes after activation.
    pub replay_poststate_digest: [u8; 32],
}

impl DealerScenarioActivationReceiptV1 {
    /// Hostile-decode one exact receipt.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        require_header(
            bytes,
            DEALER_SCENARIO_ACTIVATION_RECEIPT_BYTES_V1,
            &DEALER_SCENARIO_ACTIVATION_RECEIPT_MAGIC_V1,
        )?;
        require_zero(bytes, 10, 6)?;
        let value = Self {
            producer_program: array_at(bytes, ACTIVATION_PRODUCER_OFFSET)?,
            checkpoint: array_at(bytes, ACTIVATION_CHECKPOINT_OFFSET)?,
            checkpoint_prestate_digest: array_at(bytes, ACTIVATION_CHECKPOINT_PRESTATE_OFFSET)?,
            request_digest: array_at(bytes, ACTIVATION_REQUEST_OFFSET)?,
            effects_digest: array_at(bytes, ACTIVATION_EFFECTS_OFFSET)?,
            batch: array_at(bytes, ACTIVATION_BATCH_OFFSET)?,
            batch_prestate_digest: array_at(bytes, ACTIVATION_BATCH_PRESTATE_OFFSET)?,
            batch_poststate_digest: array_at(bytes, ACTIVATION_BATCH_POSTSTATE_OFFSET)?,
            replay_prestate_digest: array_at(bytes, ACTIVATION_REPLAY_PRESTATE_OFFSET)?,
            replay_poststate_digest: array_at(bytes, ACTIVATION_REPLAY_POSTSTATE_OFFSET)?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode one exact receipt.
    pub fn encode(self) -> Result<[u8; DEALER_SCENARIO_ACTIVATION_RECEIPT_BYTES_V1]> {
        self.validate()?;
        let mut bytes = [0_u8; DEALER_SCENARIO_ACTIVATION_RECEIPT_BYTES_V1];
        put_header(&mut bytes, &DEALER_SCENARIO_ACTIVATION_RECEIPT_MAGIC_V1)?;
        for (offset, value) in [
            (ACTIVATION_PRODUCER_OFFSET, self.producer_program),
            (ACTIVATION_CHECKPOINT_OFFSET, self.checkpoint),
            (
                ACTIVATION_CHECKPOINT_PRESTATE_OFFSET,
                self.checkpoint_prestate_digest,
            ),
            (ACTIVATION_REQUEST_OFFSET, self.request_digest),
            (ACTIVATION_EFFECTS_OFFSET, self.effects_digest),
            (ACTIVATION_BATCH_OFFSET, self.batch),
            (ACTIVATION_BATCH_PRESTATE_OFFSET, self.batch_prestate_digest),
            (
                ACTIVATION_BATCH_POSTSTATE_OFFSET,
                self.batch_poststate_digest,
            ),
            (
                ACTIVATION_REPLAY_PRESTATE_OFFSET,
                self.replay_prestate_digest,
            ),
            (
                ACTIVATION_REPLAY_POSTSTATE_OFFSET,
                self.replay_poststate_digest,
            ),
        ] {
            put(&mut bytes, offset, &value)?;
        }
        Ok(bytes)
    }

    fn validate(self) -> Result<()> {
        if [
            self.producer_program,
            self.checkpoint,
            self.checkpoint_prestate_digest,
            self.request_digest,
            self.effects_digest,
            self.batch,
            self.batch_prestate_digest,
            self.batch_poststate_digest,
            self.replay_prestate_digest,
            self.replay_poststate_digest,
        ]
        .contains(&[0; 32])
        {
            Err(Error::ZeroCoordinate)
        } else {
            Ok(())
        }
    }
}

fn require_header(bytes: &[u8], width: usize, magic: &[u8; 8]) -> Result<()> {
    if bytes.len() != width {
        return Err(Error::InvalidLength);
    }
    if bytes.get(..8) != Some(magic.as_slice()) {
        return Err(Error::InvalidMagic);
    }
    if bytes.get(VERSION_OFFSET..VERSION_OFFSET + 2)
        != Some(
            DEALER_SCENARIO_CUSTODY_STATE_VERSION_V1
                .to_le_bytes()
                .as_slice(),
        )
    {
        return Err(Error::UnsupportedVersion);
    }
    Ok(())
}

fn put_header(bytes: &mut [u8], magic: &[u8; 8]) -> Result<()> {
    put(bytes, 0, magic)?;
    put(
        bytes,
        VERSION_OFFSET,
        &DEALER_SCENARIO_CUSTODY_STATE_VERSION_V1.to_le_bytes(),
    )
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    let mut value = [0_u8; 2];
    value.copy_from_slice(bytes.get(offset..offset + 2).ok_or(Error::InvalidLength)?);
    Ok(u16::from_le_bytes(value))
}

fn put_u16(bytes: &mut [u8], offset: usize, value: u16) -> Result<()> {
    put(bytes, offset, &value.to_le_bytes())
}

fn read_identity_bank(
    bytes: &[u8],
    offset: usize,
) -> Result<[[u8; 32]; DEALER_SCENARIO_MAX_RESERVATIONS_V1]> {
    let mut values = [[0_u8; 32]; DEALER_SCENARIO_MAX_RESERVATIONS_V1];
    for (index, value) in values.iter_mut().enumerate() {
        *value = array_at(bytes, offset + index * 32)?;
    }
    Ok(values)
}

fn put_identity_bank(
    bytes: &mut [u8],
    offset: usize,
    values: &[[u8; 32]; DEALER_SCENARIO_MAX_RESERVATIONS_V1],
) -> Result<()> {
    for (index, value) in values.iter().enumerate() {
        put(bytes, offset + index * 32, value)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn effect() -> DealerScenarioCustodyEffectV1 {
        let mut payload = [0_u8; DEALER_SCENARIO_DELEGATED_CUSTODY_REQUEST_BYTES_V1];
        payload[..DEALER_SCENARIO_CANONICAL_CUSTODY_REQUEST_BYTES_V1].fill(9);
        DealerScenarioCustodyEffectV1 {
            kind: DealerScenarioCustodyRequestKindV1::Canonical,
            ordinal: 0,
            effect_count: 2,
            producer_program: id(1),
            checkpoint: id(2),
            request_digest: id(3),
            source_after: 7,
            destination_after: 11,
            request_payload: payload,
        }
    }

    #[test]
    fn effect_and_manifest_refuse_padding_omission_and_reorder() {
        let value = effect();
        let bytes = value.encode().expect("effect");
        assert_eq!(DealerScenarioCustodyEffectV1::decode(&bytes), Ok(value));
        let mut hostile = bytes;
        *hostile.last_mut().expect("last") = 1;
        assert_eq!(
            DealerScenarioCustodyEffectV1::decode(&hostile),
            Err(Error::NonCanonicalPadding)
        );

        let manifest = DealerScenarioCustodyEffectManifestV1 {
            effect_count: 2,
            producer_program: id(1),
            checkpoint: id(2),
            request_digest: id(3),
            effect_accounts: [id(4), id(5), [0; 32], [0; 32]],
            effect_digests: [id(6), id(7), [0; 32], [0; 32]],
        };
        let encoded = manifest.encode().expect("manifest");
        assert_eq!(
            DealerScenarioCustodyEffectManifestV1::decode(&encoded),
            Ok(manifest)
        );
        let mut duplicate = manifest;
        duplicate.effect_accounts[1] = duplicate.effect_accounts[0];
        assert_eq!(duplicate.encode(), Err(Error::IdentityMismatch));
    }

    #[test]
    fn instruction_families_refuse_wrong_width_tags_and_counts() {
        for action in [
            DealerScenarioReservationActionV1::Reserve,
            DealerScenarioReservationActionV1::Rollback,
        ] {
            let bytes = encode_dealer_scenario_reservation_instruction_v1(action, 3)
                .expect("reservation instruction");
            assert_eq!(
                decode_dealer_scenario_reservation_instruction_v1(&bytes),
                Ok((action, 3))
            );
            assert_eq!(
                decode_dealer_scenario_reservation_instruction_v1(&bytes[..8]),
                Err(Error::InvalidLength)
            );
        }
        let activation =
            encode_dealer_scenario_activation_instruction_v1(4).expect("activation instruction");
        assert_eq!(
            decode_dealer_scenario_activation_instruction_v1(&activation),
            Ok(4)
        );
        let mut hostile = activation;
        hostile[8] = 0;
        assert_eq!(
            decode_dealer_scenario_activation_instruction_v1(&hostile),
            Err(Error::InvalidPhase)
        );
        assert_eq!(
            decode_dealer_scenario_reservation_instruction_v1(&activation),
            Err(Error::InvalidMagic)
        );
    }

    fn batch() -> DealerScenarioReservationBatchV1 {
        DealerScenarioReservationBatchV1::new(
            2,
            id(1),
            id(2),
            id(3),
            id(4),
            id(5),
            id(6),
            id(7),
            id(8),
            id(9),
            id(10),
            20,
            1,
        )
        .expect("batch")
    }

    #[test]
    fn batch_locks_ordered_and_rolls_back_reverse_only_after_expiry() {
        let first = batch()
            .append_reserve(10, 0, id(20), id(21), id(22))
            .expect("reserve zero");
        assert_eq!(
            first.append_reserve(10, 0, id(23), id(24), id(25)),
            Err(Error::InvalidPhase)
        );
        let full = first
            .append_reserve(11, 1, id(23), id(24), id(25))
            .expect("reserve one");
        assert_eq!(
            full.activate_committed(id(26)).map(|value| value.status),
            Ok(DealerScenarioReservationBatchStatusV1::Activated)
        );
        assert_eq!(full.activate_committed([0; 32]), Err(Error::InvalidPhase));
        assert_eq!(
            full.append_rollback(20, 1, id(26), id(25), id(27)),
            Err(Error::InvalidPhase)
        );
        assert_eq!(
            full.append_rollback(21, 0, id(26), id(22), id(27)),
            Err(Error::StaleCoordinate)
        );
        let one = full
            .append_rollback(21, 1, id(26), id(25), id(27))
            .expect("rollback one");
        let zero = one
            .append_rollback(22, 0, id(28), id(22), id(29))
            .expect("rollback zero");
        let bytes = zero.encode().expect("batch bytes");
        assert_eq!(DealerScenarioReservationBatchV1::decode(&bytes), Ok(zero));
    }

    #[test]
    fn active_state_and_activation_receipt_round_trip() {
        let state = DealerScenarioReservationStateV1 {
            status: DealerScenarioReservationStateStatusV1::Active,
            ordinal: 0,
            effect_count: 2,
            batch: id(1),
            checkpoint: id(2),
            request_digest: id(3),
            effects_digest: id(4),
            effect_digest: id(5),
            source: id(6),
            destination: id(7),
            escrow: id(8),
            mint: id(9),
            token_program: id(10),
            source_prestate_digest: id(11),
            destination_prestate_digest: id(12),
            effect_poststate_digest: id(13),
            source_poststate_digest: id(14),
            amount: 5,
            source_after: 6,
            destination_before: 7,
            escrow_after: 5,
        };
        let bytes = state.encode().expect("state");
        assert_eq!(DealerScenarioReservationStateV1::decode(&bytes), Ok(state));
        let receipt = DealerScenarioActivationReceiptV1 {
            producer_program: id(1),
            checkpoint: id(2),
            checkpoint_prestate_digest: id(3),
            request_digest: id(4),
            effects_digest: id(5),
            batch: id(6),
            batch_prestate_digest: id(7),
            batch_poststate_digest: id(8),
            replay_prestate_digest: id(9),
            replay_poststate_digest: id(10),
        };
        let bytes = receipt.encode().expect("receipt");
        assert_eq!(
            DealerScenarioActivationReceiptV1::decode(&bytes),
            Ok(receipt)
        );
    }
}
