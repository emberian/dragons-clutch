//! Exact family request for recurring-Series hot actions.
//!
//! The common Trading outer authenticates the selected Trading deployment,
//! immutable composite root, finalized Series Template/config, and controller
//! before decoding these bytes.  This packet owns only action selection,
//! immutable content references, replay revisions, and the occurrence Merkle
//! path.  Economic amounts and program identities remain in the referenced
//! records and current Registry release set.

use dclutch_core_contract::ContentId;

/// Fixed portion before the bounded occurrence Merkle path.
pub const SERIES_ACTION_HEADER_BYTES_V3: usize = 128;
/// Maximum number of SHA-256 siblings in one occurrence commitment proof.
pub const SERIES_ACTION_MAXIMUM_PROOF_HEIGHT_V3: usize = 32;
/// Maximum exact Series family-request width.
pub const SERIES_ACTION_MAXIMUM_BYTES_V3: usize =
    SERIES_ACTION_HEADER_BYTES_V3 + SERIES_ACTION_MAXIMUM_PROOF_HEIGHT_V3 * 32;

const MAGIC: [u8; 8] = *b"DCLTSIX3";
const SCHEMA: u16 = 3;
const PROFILE: u16 = 1;
const ACTION_OFFSET: usize = 12;
const PROOF_COUNT_OFFSET: usize = 13;
const RESERVED_OFFSET: usize = 14;
const TEMPLATE_OFFSET: usize = 16;
const OCCURRENCE_OFFSET: usize = 48;
const TICKET_OFFSET: usize = 80;
const SERIES_REVISION_OFFSET: usize = 112;
const TICKET_REVISION_OFFSET: usize = 120;
const IDENTITY_BYTES: usize = 32;
const ZERO_IDENTITY: [u8; IDENTITY_BYTES] = [0; IDENTITY_BYTES];

/// Recurring-Series action selected after common Trading authentication.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesActionV3 {
    /// Create and exactly prepay one occurrence replay account.
    Prepare = 0,
    /// Atomically consume a prepared Ticket through Core into its Found Market.
    Consume = 1,
    /// Refund a prepared Ticket after its retry deadline.
    Expire = 2,
    /// Delete a terminal Ticket replay account and return its Rent/donations.
    Retire = 3,
    /// Delete a terminal Series root after every Ticket is retired.
    Close = 4,
}

impl SeriesActionV3 {
    fn decode(value: u8) -> Result<Self, SeriesInstructionErrorV3> {
        match value {
            0 => Ok(Self::Prepare),
            1 => Ok(Self::Consume),
            2 => Ok(Self::Expire),
            3 => Ok(Self::Retire),
            4 => Ok(Self::Close),
            _ => Err(SeriesInstructionErrorV3::Action),
        }
    }

    /// Whether this action supplies an exact occurrence commitment proof.
    pub const fn occurrence_bound(self) -> bool {
        matches!(self, Self::Prepare | Self::Consume | Self::Expire)
    }

    /// Whether this action crosses the atomic Trading-to-Core founding seam.
    pub const fn core_founding(self) -> bool {
        matches!(self, Self::Consume)
    }
}

/// Refusal from the bounded Series family-request decoder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesInstructionErrorV3 {
    /// Exact packet width, magic, schema, profile, or reserved bytes refused.
    Encoding,
    /// Action tag or its action-specific optional-field shape refused.
    Action,
    /// A required content identity was zero or a forbidden identity was present.
    Identity,
    /// The occurrence Merkle proof exceeded the bound or had the wrong byte width.
    Proof,
}

/// Borrowed hostile-decoded Series family request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesActionRequestV3<'a> {
    action: SeriesActionV3,
    template: ContentId,
    occurrence: Option<ContentId>,
    ticket: Option<ContentId>,
    expected_series_revision: u64,
    expected_ticket_revision: u64,
    proof_count: u8,
    proof_bytes: &'a [u8],
}

impl<'a> SeriesActionRequestV3<'a> {
    /// Decode one exact `header || ordered Merkle siblings` family request.
    pub fn decode(input: &'a [u8]) -> Result<Self, SeriesInstructionErrorV3> {
        let header = input
            .get(..SERIES_ACTION_HEADER_BYTES_V3)
            .ok_or(SeriesInstructionErrorV3::Encoding)?;
        if header.get(..8) != Some(MAGIC.as_slice())
            || read_u16(header, 8)? != SCHEMA
            || read_u16(header, 10)? != PROFILE
            || header.get(RESERVED_OFFSET..TEMPLATE_OFFSET) != Some([0_u8; 2].as_slice())
        {
            return Err(SeriesInstructionErrorV3::Encoding);
        }
        let action = SeriesActionV3::decode(read_u8(header, ACTION_OFFSET)?)?;
        let proof_count = read_u8(header, PROOF_COUNT_OFFSET)?;
        if usize::from(proof_count) > SERIES_ACTION_MAXIMUM_PROOF_HEIGHT_V3 {
            return Err(SeriesInstructionErrorV3::Proof);
        }
        let proof_bytes_len = usize::from(proof_count)
            .checked_mul(IDENTITY_BYTES)
            .ok_or(SeriesInstructionErrorV3::Proof)?;
        let exact_len = SERIES_ACTION_HEADER_BYTES_V3
            .checked_add(proof_bytes_len)
            .ok_or(SeriesInstructionErrorV3::Proof)?;
        if input.len() != exact_len {
            return Err(SeriesInstructionErrorV3::Proof);
        }
        let template = read_content(header, TEMPLATE_OFFSET)?;
        let occurrence = read_optional_content(header, OCCURRENCE_OFFSET)?;
        let ticket = read_optional_content(header, TICKET_OFFSET)?;
        let expected_series_revision = read_u64(header, SERIES_REVISION_OFFSET)?;
        let expected_ticket_revision = read_u64(header, TICKET_REVISION_OFFSET)?;
        let valid_shape = match action {
            SeriesActionV3::Prepare => {
                occurrence.is_some() && ticket.is_some() && expected_ticket_revision == 0
            }
            SeriesActionV3::Consume | SeriesActionV3::Expire => {
                occurrence.is_some() && ticket.is_some()
            }
            SeriesActionV3::Retire => occurrence.is_none() && ticket.is_some() && proof_count == 0,
            SeriesActionV3::Close => {
                occurrence.is_none()
                    && ticket.is_none()
                    && expected_ticket_revision == 0
                    && proof_count == 0
            }
        };
        if !valid_shape {
            return Err(SeriesInstructionErrorV3::Action);
        }
        Ok(Self {
            action,
            template,
            occurrence,
            ticket,
            expected_series_revision,
            expected_ticket_revision,
            proof_count,
            proof_bytes: input
                .get(SERIES_ACTION_HEADER_BYTES_V3..)
                .ok_or(SeriesInstructionErrorV3::Proof)?,
        })
    }

    /// Selected family action.
    pub const fn action(self) -> SeriesActionV3 {
        self.action
    }
    /// Exact finalized Template/config content identity.
    pub const fn template(self) -> ContentId {
        self.template
    }
    /// Exact realized occurrence content identity, present only for occurrence actions.
    pub const fn occurrence(self) -> Option<ContentId> {
        self.occurrence
    }
    /// Exact immutable Ticket-record identity, absent only for root Close.
    pub const fn ticket(self) -> Option<ContentId> {
        self.ticket
    }
    /// Expected mutable Series-root revision.
    pub const fn expected_series_revision(self) -> u64 {
        self.expected_series_revision
    }
    /// Expected Ticket replay revision, zero for Prepare and Close.
    pub const fn expected_ticket_revision(self) -> u64 {
        self.expected_ticket_revision
    }
    /// Exact ordered sibling count.
    pub const fn proof_count(self) -> u8 {
        self.proof_count
    }

    /// Borrow the exact canonical `32 * proof_count` proof bytes.
    pub const fn proof_bytes(self) -> &'a [u8] {
        self.proof_bytes
    }

    /// Copy the bounded proof into fixed caller storage without allocation.
    pub fn copy_proof_into(
        self,
        output: &mut [[u8; IDENTITY_BYTES]; SERIES_ACTION_MAXIMUM_PROOF_HEIGHT_V3],
    ) -> Result<&[[u8; IDENTITY_BYTES]], SeriesInstructionErrorV3> {
        let count = usize::from(self.proof_count);
        for (index, destination) in output
            .get_mut(..count)
            .ok_or(SeriesInstructionErrorV3::Proof)?
            .iter_mut()
            .enumerate()
        {
            let start = index
                .checked_mul(IDENTITY_BYTES)
                .ok_or(SeriesInstructionErrorV3::Proof)?;
            let end = start
                .checked_add(IDENTITY_BYTES)
                .ok_or(SeriesInstructionErrorV3::Proof)?;
            destination.copy_from_slice(
                self.proof_bytes
                    .get(start..end)
                    .ok_or(SeriesInstructionErrorV3::Proof)?,
            );
        }
        output.get(..count).ok_or(SeriesInstructionErrorV3::Proof)
    }
}

/// Encode an already validated action header; append exactly `proof_count`
/// ordered 32-byte siblings to obtain the final request.
#[allow(clippy::too_many_arguments)]
pub fn encode_series_action_header_v3(
    action: SeriesActionV3,
    template: ContentId,
    occurrence: Option<ContentId>,
    ticket: Option<ContentId>,
    expected_series_revision: u64,
    expected_ticket_revision: u64,
    proof_count: u8,
) -> Result<[u8; SERIES_ACTION_HEADER_BYTES_V3], SeriesInstructionErrorV3> {
    let mut output = [0_u8; SERIES_ACTION_HEADER_BYTES_V3];
    output[..8].copy_from_slice(&MAGIC);
    output[8..10].copy_from_slice(&SCHEMA.to_le_bytes());
    output[10..12].copy_from_slice(&PROFILE.to_le_bytes());
    output[ACTION_OFFSET] = action as u8;
    output[PROOF_COUNT_OFFSET] = proof_count;
    put_content(&mut output, TEMPLATE_OFFSET, Some(template))?;
    put_content(&mut output, OCCURRENCE_OFFSET, occurrence)?;
    put_content(&mut output, TICKET_OFFSET, ticket)?;
    output[SERIES_REVISION_OFFSET..SERIES_REVISION_OFFSET + 8]
        .copy_from_slice(&expected_series_revision.to_le_bytes());
    output[TICKET_REVISION_OFFSET..TICKET_REVISION_OFFSET + 8]
        .copy_from_slice(&expected_ticket_revision.to_le_bytes());
    let mut validation = [0_u8; SERIES_ACTION_MAXIMUM_BYTES_V3];
    validation[..SERIES_ACTION_HEADER_BYTES_V3].copy_from_slice(&output);
    let exact_len = SERIES_ACTION_HEADER_BYTES_V3
        .checked_add(usize::from(proof_count).saturating_mul(IDENTITY_BYTES))
        .ok_or(SeriesInstructionErrorV3::Proof)?;
    SeriesActionRequestV3::decode(
        validation
            .get(..exact_len)
            .ok_or(SeriesInstructionErrorV3::Proof)?,
    )?;
    Ok(output)
}

fn read_u8(input: &[u8], offset: usize) -> Result<u8, SeriesInstructionErrorV3> {
    input
        .get(offset)
        .copied()
        .ok_or(SeriesInstructionErrorV3::Encoding)
}

fn read_u16(input: &[u8], offset: usize) -> Result<u16, SeriesInstructionErrorV3> {
    let bytes: [u8; 2] = input
        .get(offset..offset.saturating_add(2))
        .ok_or(SeriesInstructionErrorV3::Encoding)?
        .try_into()
        .map_err(|_| SeriesInstructionErrorV3::Encoding)?;
    Ok(u16::from_le_bytes(bytes))
}

fn read_u64(input: &[u8], offset: usize) -> Result<u64, SeriesInstructionErrorV3> {
    let bytes: [u8; 8] = input
        .get(offset..offset.saturating_add(8))
        .ok_or(SeriesInstructionErrorV3::Encoding)?
        .try_into()
        .map_err(|_| SeriesInstructionErrorV3::Encoding)?;
    Ok(u64::from_le_bytes(bytes))
}

fn read_content(input: &[u8], offset: usize) -> Result<ContentId, SeriesInstructionErrorV3> {
    let bytes = read_identity(input, offset)?;
    ContentId::new(bytes).map_err(|_| SeriesInstructionErrorV3::Identity)
}

fn read_optional_content(
    input: &[u8],
    offset: usize,
) -> Result<Option<ContentId>, SeriesInstructionErrorV3> {
    let bytes = read_identity(input, offset)?;
    if bytes == ZERO_IDENTITY {
        Ok(None)
    } else {
        ContentId::new(bytes)
            .map(Some)
            .map_err(|_| SeriesInstructionErrorV3::Identity)
    }
}

fn read_identity(
    input: &[u8],
    offset: usize,
) -> Result<[u8; IDENTITY_BYTES], SeriesInstructionErrorV3> {
    input
        .get(offset..offset.saturating_add(IDENTITY_BYTES))
        .ok_or(SeriesInstructionErrorV3::Encoding)?
        .try_into()
        .map_err(|_| SeriesInstructionErrorV3::Encoding)
}

fn put_content(
    output: &mut [u8],
    offset: usize,
    value: Option<ContentId>,
) -> Result<(), SeriesInstructionErrorV3> {
    output
        .get_mut(offset..offset.saturating_add(IDENTITY_BYTES))
        .ok_or(SeriesInstructionErrorV3::Encoding)?
        .copy_from_slice(&value.map_or(ZERO_IDENTITY, ContentId::to_bytes));
    Ok(())
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::vec::Vec;

    use super::*;

    fn id(byte: u8) -> ContentId {
        ContentId::new([byte; 32]).expect("identity")
    }

    fn occurrence_packet(
        action: SeriesActionV3,
        ticket_revision: u64,
        proof: &[[u8; 32]],
    ) -> Vec<u8> {
        let header = encode_series_action_header_v3(
            action,
            id(1),
            Some(id(2)),
            Some(id(3)),
            4,
            ticket_revision,
            u8::try_from(proof.len()).expect("proof count"),
        )
        .expect("header");
        let mut output = Vec::from(header);
        for sibling in proof {
            output.extend_from_slice(sibling);
        }
        output
    }

    #[test]
    fn occurrence_packet_roundtrips_without_allocation_in_decoder() {
        let proof = [[5_u8; 32], [6_u8; 32]];
        let bytes = occurrence_packet(SeriesActionV3::Consume, 7, &proof);
        let decoded = SeriesActionRequestV3::decode(&bytes).expect("decode");
        assert_eq!(decoded.action(), SeriesActionV3::Consume);
        assert!(decoded.action().core_founding());
        assert_eq!(decoded.template(), id(1));
        assert_eq!(decoded.occurrence(), Some(id(2)));
        assert_eq!(decoded.ticket(), Some(id(3)));
        assert_eq!(decoded.expected_series_revision(), 4);
        assert_eq!(decoded.expected_ticket_revision(), 7);
        let mut copied = [[0_u8; 32]; SERIES_ACTION_MAXIMUM_PROOF_HEIGHT_V3];
        assert_eq!(decoded.copy_proof_into(&mut copied), Ok(proof.as_slice()));
    }

    #[test]
    fn hostile_lengths_reserved_tags_and_shapes_refuse() {
        let proof = [[5_u8; 32], [6_u8; 32]];
        let bytes = occurrence_packet(SeriesActionV3::Consume, 7, &proof);
        assert_eq!(
            SeriesActionRequestV3::decode(
                bytes
                    .get(..bytes.len().saturating_sub(1))
                    .expect("short packet"),
            ),
            Err(SeriesInstructionErrorV3::Proof)
        );
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert_eq!(
            SeriesActionRequestV3::decode(&trailing),
            Err(SeriesInstructionErrorV3::Proof)
        );
        let mut reserved = bytes.clone();
        *reserved.get_mut(RESERVED_OFFSET).expect("reserved byte") = 1;
        assert_eq!(
            SeriesActionRequestV3::decode(&reserved),
            Err(SeriesInstructionErrorV3::Encoding)
        );
        let mut obsolete_v2 = bytes.clone();
        *obsolete_v2.get_mut(7).expect("versioned magic byte") = b'2';
        obsolete_v2
            .get_mut(8..10)
            .expect("schema bytes")
            .copy_from_slice(&2_u16.to_le_bytes());
        assert_eq!(
            SeriesActionRequestV3::decode(&obsolete_v2),
            Err(SeriesInstructionErrorV3::Encoding)
        );
        let mut tag = bytes.clone();
        *tag.get_mut(ACTION_OFFSET).expect("action byte") = u8::MAX;
        assert_eq!(
            SeriesActionRequestV3::decode(&tag),
            Err(SeriesInstructionErrorV3::Action)
        );
        assert_eq!(
            encode_series_action_header_v3(
                SeriesActionV3::Prepare,
                id(1),
                Some(id(2)),
                Some(id(3)),
                4,
                1,
                0,
            ),
            Err(SeriesInstructionErrorV3::Action)
        );
        assert_eq!(
            encode_series_action_header_v3(
                SeriesActionV3::Close,
                id(1),
                None,
                Some(id(3)),
                4,
                0,
                0,
            ),
            Err(SeriesInstructionErrorV3::Action)
        );
    }

    #[test]
    fn terminal_packets_cannot_smuggle_occurrence_proofs() {
        let retire = encode_series_action_header_v3(
            SeriesActionV3::Retire,
            id(1),
            None,
            Some(id(3)),
            8,
            9,
            0,
        )
        .expect("retire");
        let retire = SeriesActionRequestV3::decode(&retire).expect("retire decode");
        assert_eq!(retire.action(), SeriesActionV3::Retire);
        assert!(!retire.action().occurrence_bound());

        let close =
            encode_series_action_header_v3(SeriesActionV3::Close, id(1), None, None, 10, 0, 0)
                .expect("close");
        assert_eq!(
            SeriesActionRequestV3::decode(&close)
                .expect("close decode")
                .action(),
            SeriesActionV3::Close
        );
        assert_eq!(
            encode_series_action_header_v3(
                SeriesActionV3::Retire,
                id(1),
                None,
                Some(id(3)),
                8,
                9,
                1,
            ),
            Err(SeriesInstructionErrorV3::Action)
        );
    }
}
