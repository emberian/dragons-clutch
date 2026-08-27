//! The relay instruction wire.
//!
//! Four fixed-prefix actions behind one magic.  Unlike the signed attestation
//! wire — which is Lean-authored ABI because it is a *release identity* that two
//! independent implementations must agree on byte-for-byte — this is adapter
//! framing owned by the program that dispatches it, exactly as
//! `dclutch-source-contract`'s own instruction wire is.
//!
//! Both signature-carrying actions place their signed message at a **fixed**
//! offset in the instruction data, which is what lets the Ed25519 descriptor's
//! `message_data_offset` be a constant the adapter compares rather than a value
//! the caller supplies.

use crate::{
    Error, RELAYED_SEAL_BYTES, Result, base, header, one, put, require_zero, u16_at, u64_at,
    variable_header,
};

/// Canonical relay instruction magic.
pub const RELAY_INSTRUCTION_MAGIC: [u8; 8] = *b"DCLTRIX1";
/// Shared relay instruction header width.
pub const RELAY_INSTRUCTION_HEADER_BYTES: usize = 16;
/// Exact `CreateRecord` instruction width.
pub const CREATE_RECORD_INSTRUCTION_BYTES: usize = 144;
/// Fixed prefix before the inline attestation message in `AppendObservation`.
pub const APPEND_OBSERVATION_PREFIX_BYTES: usize = 40;
/// Fixed prefix before the inline seal message in `SealRecord`.
pub const SEAL_RECORD_PREFIX_BYTES: usize = 32;
/// Exact `SealRecord` instruction width.
pub const SEAL_RECORD_INSTRUCTION_BYTES: usize = SEAL_RECORD_PREFIX_BYTES + RELAYED_SEAL_BYTES;
/// Exact `RetireRecord` instruction width.
pub const RETIRE_RECORD_INSTRUCTION_BYTES: usize = 32;

const ACTION_OFFSET: usize = 10;
const HEADER_RESERVED_OFFSET: usize = 11;
const HEADER_RESERVED_BYTES: usize = 5;

/// Closed V1 relay action set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum RelayActionV1 {
    /// Create one observation record as a direct Market child.
    CreateRecord = 1,
    /// Append one authenticated observation at the next position.
    AppendObservation = 2,
    /// Record one key-set member's seal over the completed set.
    SealRecord = 3,
    /// Close the record into its pre-existing RentCredit beneficiary.
    RetireRecord = 4,
}

impl RelayActionV1 {
    fn decode(byte: u8) -> Result<Self> {
        match byte {
            1 => Ok(Self::CreateRecord),
            2 => Ok(Self::AppendObservation),
            3 => Ok(Self::SealRecord),
            4 => Ok(Self::RetireRecord),
            _ => Err(Error::UnknownInstructionAction),
        }
    }

    const fn byte(self) -> u8 {
        self as u8
    }
}

/// Fixed `CreateRecord` wire fields.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreateRecordInstructionV1 {
    generation: u64,
    observed_slot: u64,
    expected_market_child_count: u64,
    set_count: u16,
    seal_threshold: u8,
    pda_bump: u8,
    source_material_id: [u8; 32],
    source_spec_id: [u8; 32],
    rent_beneficiary: [u8; 32],
}

impl CreateRecordInstructionV1 {
    /// Construct one create request.
    ///
    /// The Source graph coordinates ride here because creation is the only
    /// route that has to be told them; every later route reads them back out of
    /// the persisted record, so a caller cannot re-point a live record at a
    /// different Source graph.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        generation: u64,
        observed_slot: u64,
        expected_market_child_count: u64,
        set_count: u16,
        seal_threshold: u8,
        pda_bump: u8,
        source_material_id: [u8; 32],
        source_spec_id: [u8; 32],
        rent_beneficiary: [u8; 32],
    ) -> Result<Self> {
        crate::record::relayed_observation_record_bytes_v1(set_count)?;
        if seal_threshold == 0 || seal_threshold > crate::MAX_RELAYER_KEYS_V1_U8 {
            return Err(Error::NonCanonicalKeySet);
        }
        crate::require_nonzero(&source_material_id)?;
        crate::require_nonzero(&source_spec_id)?;
        crate::require_nonzero(&rent_beneficiary)?;
        Ok(Self {
            generation,
            observed_slot,
            expected_market_child_count,
            set_count,
            seal_threshold,
            pda_bump,
            source_material_id,
            source_spec_id,
            rent_beneficiary,
        })
    }

    /// The immutable Source material this record will serve.
    pub const fn source_material_id(self) -> [u8; 32] {
        self.source_material_id
    }
    /// The Source specification naming this provider release.
    pub const fn source_spec_id(self) -> [u8; 32] {
        self.source_spec_id
    }
    /// The pre-existing RentCredit that will receive the record's rent.
    pub const fn rent_beneficiary(self) -> [u8; 32] {
        self.rent_beneficiary
    }

    /// Encode the exact canonical bytes.
    pub fn to_bytes(self) -> Result<[u8; CREATE_RECORD_INSTRUCTION_BYTES]> {
        let mut out = base::<CREATE_RECORD_INSTRUCTION_BYTES>(RELAY_INSTRUCTION_MAGIC)?;
        put(
            &mut out,
            ACTION_OFFSET,
            &[RelayActionV1::CreateRecord.byte()],
        )?;
        put(&mut out, 16, &self.generation.to_le_bytes())?;
        put(&mut out, 24, &self.observed_slot.to_le_bytes())?;
        put(
            &mut out,
            32,
            &self.expected_market_child_count.to_le_bytes(),
        )?;
        put(&mut out, 40, &self.set_count.to_le_bytes())?;
        put(&mut out, 42, &[self.seal_threshold, self.pda_bump])?;
        put(&mut out, 48, &self.source_material_id)?;
        put(&mut out, 80, &self.source_spec_id)?;
        put(&mut out, 112, &self.rent_beneficiary)?;
        Ok(out)
    }

    /// The Market generation this record is created under.
    pub const fn generation(self) -> u64 {
        self.generation
    }
    /// The finalized foreign slot the record is seeded by.
    pub const fn observed_slot(self) -> u64 {
        self.observed_slot
    }
    /// The Market direct-child count the caller believes it is registering into.
    pub const fn expected_market_child_count(self) -> u64 {
        self.expected_market_child_count
    }
    /// The cardinality of the pinned ordered set.
    pub const fn set_count(self) -> u16 {
        self.set_count
    }
    /// The release's quorum threshold, echoed for the record header.
    pub const fn seal_threshold(self) -> u8 {
        self.seal_threshold
    }
    /// The record PDA bump.
    pub const fn pda_bump(self) -> u8 {
        self.pda_bump
    }
}

/// Fixed `AppendObservation` prefix; the attestation message follows it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AppendObservationInstructionV1 {
    generation: u64,
    observed_slot: u64,
}

impl AppendObservationInstructionV1 {
    /// Construct one append request prefix.
    pub const fn new(generation: u64, observed_slot: u64) -> Self {
        Self {
            generation,
            observed_slot,
        }
    }

    /// Encode the exact fixed prefix.
    pub fn to_prefix_bytes(self) -> Result<[u8; APPEND_OBSERVATION_PREFIX_BYTES]> {
        let mut out = base::<APPEND_OBSERVATION_PREFIX_BYTES>(RELAY_INSTRUCTION_MAGIC)?;
        put(
            &mut out,
            ACTION_OFFSET,
            &[RelayActionV1::AppendObservation.byte()],
        )?;
        put(&mut out, 16, &self.generation.to_le_bytes())?;
        put(&mut out, 24, &self.observed_slot.to_le_bytes())?;
        Ok(out)
    }

    /// The Market generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }
    /// The finalized foreign slot.
    pub const fn observed_slot(self) -> u64 {
        self.observed_slot
    }
}

/// Fixed `SealRecord` prefix; the 156-byte seal message follows it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SealRecordInstructionV1 {
    generation: u64,
    observed_slot: u64,
}

impl SealRecordInstructionV1 {
    /// Construct one seal request prefix.
    pub const fn new(generation: u64, observed_slot: u64) -> Self {
        Self {
            generation,
            observed_slot,
        }
    }

    /// Encode the exact fixed prefix.
    pub fn to_prefix_bytes(self) -> Result<[u8; SEAL_RECORD_PREFIX_BYTES]> {
        let mut out = base::<SEAL_RECORD_PREFIX_BYTES>(RELAY_INSTRUCTION_MAGIC)?;
        put(&mut out, ACTION_OFFSET, &[RelayActionV1::SealRecord.byte()])?;
        put(&mut out, 16, &self.generation.to_le_bytes())?;
        put(&mut out, 24, &self.observed_slot.to_le_bytes())?;
        Ok(out)
    }

    /// The Market generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }
    /// The finalized foreign slot.
    pub const fn observed_slot(self) -> u64 {
        self.observed_slot
    }
}

/// Fixed `RetireRecord` wire fields.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetireRecordInstructionV1 {
    generation: u64,
    expected_market_child_count: u64,
}

impl RetireRecordInstructionV1 {
    /// Construct one retire request.
    pub const fn new(generation: u64, expected_market_child_count: u64) -> Self {
        Self {
            generation,
            expected_market_child_count,
        }
    }

    /// Encode the exact canonical bytes.
    pub fn to_bytes(self) -> Result<[u8; RETIRE_RECORD_INSTRUCTION_BYTES]> {
        let mut out = base::<RETIRE_RECORD_INSTRUCTION_BYTES>(RELAY_INSTRUCTION_MAGIC)?;
        put(
            &mut out,
            ACTION_OFFSET,
            &[RelayActionV1::RetireRecord.byte()],
        )?;
        put(&mut out, 16, &self.generation.to_le_bytes())?;
        put(
            &mut out,
            24,
            &self.expected_market_child_count.to_le_bytes(),
        )?;
        Ok(out)
    }

    /// The Market generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }
    /// The Market direct-child count the caller believes it is decrementing.
    pub const fn expected_market_child_count(self) -> u64 {
        self.expected_market_child_count
    }
}

/// Hostile-decoded closed relay instruction set.
///
/// The two message-carrying variants hand back the message as a **borrowed**
/// slice of the instruction data at its fixed offset — the same slice the
/// Ed25519 descriptor must name — so an adapter cannot accidentally verify one
/// span and act on another.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelayInstructionV1<'a> {
    /// Create one observation record.
    CreateRecord(CreateRecordInstructionV1),
    /// Append one attested observation, whose signed message follows.
    AppendObservation(AppendObservationInstructionV1, &'a [u8]),
    /// Seal a completed set, whose signed message follows.
    SealRecord(SealRecordInstructionV1, &'a [u8]),
    /// Retire the record.
    RetireRecord(RetireRecordInstructionV1),
}

impl<'a> RelayInstructionV1<'a> {
    /// Hostile-decode one relay instruction.
    pub fn decode(bytes: &'a [u8]) -> Result<Self> {
        variable_header(bytes, RELAY_INSTRUCTION_MAGIC)?;
        require_zero(bytes, HEADER_RESERVED_OFFSET, HEADER_RESERVED_BYTES)?;
        match RelayActionV1::decode(one(bytes, ACTION_OFFSET)?)? {
            RelayActionV1::CreateRecord => {
                header(
                    bytes,
                    CREATE_RECORD_INSTRUCTION_BYTES,
                    RELAY_INSTRUCTION_MAGIC,
                )?;
                require_zero(bytes, 44, 4)?;
                Ok(Self::CreateRecord(CreateRecordInstructionV1::new(
                    u64_at(bytes, 16)?,
                    u64_at(bytes, 24)?,
                    u64_at(bytes, 32)?,
                    u16_at(bytes, 40)?,
                    one(bytes, 42)?,
                    one(bytes, 43)?,
                    crate::array(bytes, 48)?,
                    crate::array(bytes, 80)?,
                    crate::array(bytes, 112)?,
                )?))
            }
            RelayActionV1::AppendObservation => {
                if bytes.len() <= APPEND_OBSERVATION_PREFIX_BYTES {
                    return Err(Error::InvalidLength);
                }
                require_zero(bytes, 32, 8)?;
                let message = bytes
                    .get(APPEND_OBSERVATION_PREFIX_BYTES..)
                    .ok_or(Error::InvalidLength)?;
                Ok(Self::AppendObservation(
                    AppendObservationInstructionV1::new(u64_at(bytes, 16)?, u64_at(bytes, 24)?),
                    message,
                ))
            }
            RelayActionV1::SealRecord => {
                header(
                    bytes,
                    SEAL_RECORD_INSTRUCTION_BYTES,
                    RELAY_INSTRUCTION_MAGIC,
                )?;
                let message = bytes
                    .get(SEAL_RECORD_PREFIX_BYTES..)
                    .ok_or(Error::InvalidLength)?;
                Ok(Self::SealRecord(
                    SealRecordInstructionV1::new(u64_at(bytes, 16)?, u64_at(bytes, 24)?),
                    message,
                ))
            }
            RelayActionV1::RetireRecord => {
                header(
                    bytes,
                    RETIRE_RECORD_INSTRUCTION_BYTES,
                    RELAY_INSTRUCTION_MAGIC,
                )?;
                Ok(Self::RetireRecord(RetireRecordInstructionV1::new(
                    u64_at(bytes, 16)?,
                    u64_at(bytes, 24)?,
                )))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_action_round_trips() {
        let create = CreateRecordInstructionV1::new(
            7,
            423_941_138,
            3,
            4,
            1,
            254,
            [0x11; 32],
            [0x12; 32],
            [0x13; 32],
        )
        .expect("create");
        let bytes = create.to_bytes().expect("encode");
        assert_eq!(
            RelayInstructionV1::decode(&bytes),
            Ok(RelayInstructionV1::CreateRecord(create))
        );

        let retire = RetireRecordInstructionV1::new(7, 4);
        let bytes = retire.to_bytes().expect("encode");
        assert_eq!(
            RelayInstructionV1::decode(&bytes),
            Ok(RelayInstructionV1::RetireRecord(retire))
        );
    }

    #[test]
    fn the_seal_message_is_borrowed_at_its_fixed_offset() {
        let prefix = SealRecordInstructionV1::new(7, 1)
            .to_prefix_bytes()
            .expect("prefix");
        let mut wire = [0u8; SEAL_RECORD_INSTRUCTION_BYTES];
        put(&mut wire, 0, &prefix).expect("prefix");
        put(
            &mut wire,
            SEAL_RECORD_PREFIX_BYTES,
            &[0xab; RELAYED_SEAL_BYTES],
        )
        .expect("message");
        let decoded = RelayInstructionV1::decode(&wire).expect("decodes");
        let message = match decoded {
            RelayInstructionV1::SealRecord(_, message) => message,
            _ => &[],
        };
        assert_eq!(message.len(), RELAYED_SEAL_BYTES, "wrong variant or width");
        assert_eq!(message.first(), Some(&0xab));
    }

    #[test]
    fn an_unknown_action_refuses() {
        let mut bytes = RetireRecordInstructionV1::new(1, 1)
            .to_bytes()
            .expect("encode");
        put(&mut bytes, ACTION_OFFSET, &[9]).expect("action");
        assert_eq!(
            RelayInstructionV1::decode(&bytes),
            Err(Error::UnknownInstructionAction)
        );
    }

    #[test]
    fn a_nonzero_header_reserved_span_refuses() {
        let mut bytes = RetireRecordInstructionV1::new(1, 1)
            .to_bytes()
            .expect("encode");
        put(&mut bytes, HEADER_RESERVED_OFFSET, &[1]).expect("reserved");
        assert_eq!(
            RelayInstructionV1::decode(&bytes),
            Err(Error::NonCanonicalReservedBytes)
        );
    }

    #[test]
    fn a_seal_instruction_of_the_wrong_width_refuses() {
        let prefix = SealRecordInstructionV1::new(7, 1)
            .to_prefix_bytes()
            .expect("prefix");
        let mut wire = [0u8; SEAL_RECORD_INSTRUCTION_BYTES + 1];
        put(&mut wire, 0, &prefix).expect("prefix");
        assert_eq!(RelayInstructionV1::decode(&wire), Err(Error::InvalidLength));
    }

    #[test]
    fn an_append_with_no_message_refuses() {
        let prefix = AppendObservationInstructionV1::new(7, 1)
            .to_prefix_bytes()
            .expect("prefix");
        assert_eq!(
            RelayInstructionV1::decode(&prefix),
            Err(Error::InvalidLength)
        );
    }

    #[test]
    fn a_seal_threshold_outside_the_key_set_bound_refuses_at_construction() {
        assert_eq!(
            CreateRecordInstructionV1::new(1, 1, 0, 4, 0, 255, [0x11; 32], [0x12; 32], [0x13; 32]),
            Err(Error::NonCanonicalKeySet)
        );
        assert_eq!(
            CreateRecordInstructionV1::new(1, 1, 0, 4, 6, 255, [0x11; 32], [0x12; 32], [0x13; 32]),
            Err(Error::NonCanonicalKeySet)
        );
        assert_eq!(
            CreateRecordInstructionV1::new(1, 1, 0, 0, 1, 255, [0x11; 32], [0x12; 32], [0x13; 32]),
            Err(Error::InvalidSetGeometry)
        );
    }
}
