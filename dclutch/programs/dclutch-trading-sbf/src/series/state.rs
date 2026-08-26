//! Fixed-layout mutable replay state for recurring Series V2.
//!
//! Immutable Product, occurrence, Market, actor, and funding facts remain in
//! [`super::TemplateV2`], [`super::OccurrenceV2`], and [`super::TicketV2`].
//! These records persist only replay and resource-liveness facts.

use dclutch_core_contract::ContentId;

/// Exact width of the mutable Series tail inside the composite Trading root.
pub const SERIES_STATE_BYTES_V2: usize = 64;
/// Exact width of one Trading-owned mutable occurrence-ticket state.
pub const SERIES_TICKET_STATE_BYTES_V2: usize = 64;
/// PDA domain for a mutable ticket state under the selected Trading program.
pub const SERIES_TICKET_STATE_PDA_DOMAIN_V2: &[u8] = b"dclutch:series-ticket:v2";

const SERIES_STATE_MAGIC_V2: [u8; 8] = *b"DCLTSSV2";
const TICKET_STATE_MAGIC_V2: [u8; 8] = *b"DCLTSTV2";
const SCHEMA_V2: u16 = 2;
const PROFILE_V2: u16 = 1;

/// Refusal from hostile mutable-state decoding or replay planning.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesStateError {
    /// Fixed-width bytes, magic, version, profile, or reserved bytes refused.
    Encoding,
    /// A phase byte or phase/cursor combination refused.
    Phase,
    /// A replay revision, cursor, or bounded count overflowed or differed.
    Replay,
    /// A required immutable content identity was zero.
    Identity,
}

/// Persisted lifecycle phase of the recurring Series root.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesPhaseV2 {
    /// One scheduled occurrence remains to be settled.
    Active = 0,
    /// Every occurrence settled; only terminal ticket retirement remains.
    Terminal = 1,
}

impl SeriesPhaseV2 {
    fn decode(value: u8) -> Result<Self, SeriesStateError> {
        match value {
            0 => Ok(Self::Active),
            1 => Ok(Self::Terminal),
            _ => Err(SeriesStateError::Phase),
        }
    }
}

/// Mutable tail persisted inside the immutable capability-root selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesStateV2 {
    phase: SeriesPhaseV2,
    next_occurrence: u32,
    outstanding_ticket_accounts: u32,
    revision: u64,
    close_rent_remaining: u64,
}

impl SeriesStateV2 {
    /// Construct a fresh active Series. Template identity remains the root
    /// selector's immutable `config_id`, not a duplicate field here.
    pub const fn new(close_rent: u64) -> Self {
        Self {
            phase: SeriesPhaseV2::Active,
            next_occurrence: 0,
            outstanding_ticket_accounts: 0,
            revision: 0,
            close_rent_remaining: close_rent,
        }
    }

    /// Hostile-decode one exact canonical tail and its Template occurrence count.
    pub fn decode(bytes: &[u8], occurrence_count: u32) -> Result<Self, SeriesStateError> {
        if bytes.len() != SERIES_STATE_BYTES_V2
            || bytes.get(..8) != Some(SERIES_STATE_MAGIC_V2.as_slice())
            || read_u16(bytes, 8)? != SCHEMA_V2
            || read_u16(bytes, 10)? != PROFILE_V2
            || !all_zero(bytes, 13, 3)?
            || !all_zero(bytes, 40, 24)?
        {
            return Err(SeriesStateError::Encoding);
        }
        let value = Self {
            phase: SeriesPhaseV2::decode(read_u8(bytes, 12)?)?,
            next_occurrence: read_u32(bytes, 16)?,
            outstanding_ticket_accounts: read_u32(bytes, 20)?,
            revision: read_u64(bytes, 24)?,
            close_rent_remaining: read_u64(bytes, 32)?,
        };
        value.validate(occurrence_count)?;
        Ok(value)
    }

    /// Return exact canonical bytes.
    pub fn encode(
        self,
        occurrence_count: u32,
    ) -> Result<[u8; SERIES_STATE_BYTES_V2], SeriesStateError> {
        self.validate(occurrence_count)?;
        let mut output = [0_u8; SERIES_STATE_BYTES_V2];
        output[..8].copy_from_slice(&SERIES_STATE_MAGIC_V2);
        output[8..10].copy_from_slice(&SCHEMA_V2.to_le_bytes());
        output[10..12].copy_from_slice(&PROFILE_V2.to_le_bytes());
        output[12] = self.phase as u8;
        output[16..20].copy_from_slice(&self.next_occurrence.to_le_bytes());
        output[20..24].copy_from_slice(&self.outstanding_ticket_accounts.to_le_bytes());
        output[24..32].copy_from_slice(&self.revision.to_le_bytes());
        output[32..40].copy_from_slice(&self.close_rent_remaining.to_le_bytes());
        Ok(output)
    }

    fn validate(self, occurrence_count: u32) -> Result<(), SeriesStateError> {
        if occurrence_count == 0 || self.next_occurrence > occurrence_count {
            return Err(SeriesStateError::Replay);
        }
        match self.phase {
            SeriesPhaseV2::Active if self.next_occurrence < occurrence_count => Ok(()),
            SeriesPhaseV2::Terminal if self.next_occurrence == occurrence_count => Ok(()),
            _ => Err(SeriesStateError::Phase),
        }
    }

    /// Plan creation of one replay account without advancing the occurrence.
    pub fn prepare_ticket(self, expected_revision: u64) -> Result<Self, SeriesStateError> {
        if self.phase != SeriesPhaseV2::Active || self.revision != expected_revision {
            return Err(SeriesStateError::Replay);
        }
        Ok(Self {
            outstanding_ticket_accounts: self
                .outstanding_ticket_accounts
                .checked_add(1)
                .ok_or(SeriesStateError::Replay)?,
            revision: self
                .revision
                .checked_add(1)
                .ok_or(SeriesStateError::Replay)?,
            ..self
        })
    }

    /// Plan successful consumption or expiry of the current occurrence.
    pub fn settle_current(
        self,
        expected_revision: u64,
        occurrence_count: u32,
    ) -> Result<Self, SeriesStateError> {
        if self.phase != SeriesPhaseV2::Active || self.revision != expected_revision {
            return Err(SeriesStateError::Replay);
        }
        let next = self
            .next_occurrence
            .checked_add(1)
            .ok_or(SeriesStateError::Replay)?;
        if next > occurrence_count {
            return Err(SeriesStateError::Replay);
        }
        Ok(Self {
            phase: if next == occurrence_count {
                SeriesPhaseV2::Terminal
            } else {
                SeriesPhaseV2::Active
            },
            next_occurrence: next,
            revision: self
                .revision
                .checked_add(1)
                .ok_or(SeriesStateError::Replay)?,
            ..self
        })
    }

    /// Plan deletion of one already terminal ticket replay account.
    pub fn retire_ticket(self, expected_revision: u64) -> Result<Self, SeriesStateError> {
        if self.revision != expected_revision || self.outstanding_ticket_accounts == 0 {
            return Err(SeriesStateError::Replay);
        }
        Ok(Self {
            outstanding_ticket_accounts: self.outstanding_ticket_accounts - 1,
            revision: self
                .revision
                .checked_add(1)
                .ok_or(SeriesStateError::Replay)?,
            ..self
        })
    }

    /// Require terminal root closure to have no replay account left behind.
    pub fn admit_close(self, expected_revision: u64) -> Result<(), SeriesStateError> {
        if self.phase == SeriesPhaseV2::Terminal
            && self.revision == expected_revision
            && self.outstanding_ticket_accounts == 0
        {
            Ok(())
        } else {
            Err(SeriesStateError::Replay)
        }
    }

    /// Current phase.
    pub const fn phase(self) -> SeriesPhaseV2 {
        self.phase
    }
    /// Next occurrence that may be prepared or settled.
    pub const fn next_occurrence(self) -> u32 {
        self.next_occurrence
    }
    /// Number of live terminal or prepared ticket accounts.
    pub const fn outstanding_ticket_accounts(self) -> u32 {
        self.outstanding_ticket_accounts
    }
    /// Current replay revision.
    pub const fn revision(self) -> u64 {
        self.revision
    }
    /// Separately classified close-rent principal.
    pub const fn close_rent_remaining(self) -> u64 {
        self.close_rent_remaining
    }
}

/// Mutable phase of one occurrence-ticket replay account.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TicketPhaseV2 {
    /// Exact custody is prepared and the occurrence remains retryable.
    Prepared = 0,
    /// The ticket was atomically consumed into its exact Found Market.
    Consumed = 1,
    /// The retry window elapsed and every compartment was refunded.
    Expired = 2,
}

impl TicketPhaseV2 {
    fn decode(value: u8) -> Result<Self, SeriesStateError> {
        match value {
            0 => Ok(Self::Prepared),
            1 => Ok(Self::Consumed),
            2 => Ok(Self::Expired),
            _ => Err(SeriesStateError::Phase),
        }
    }

    /// Return whether no economic retry remains possible.
    pub const fn terminal(self) -> bool {
        !matches!(self, Self::Prepared)
    }
}

/// Minimal mutable replay state; the immutable Ticket record owns all facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TicketStateV2 {
    phase: TicketPhaseV2,
    revision: u64,
    ticket_record_id: ContentId,
}

impl TicketStateV2 {
    /// Construct one prepared ticket at revision zero.
    pub const fn prepared(ticket_record_id: ContentId) -> Self {
        Self {
            phase: TicketPhaseV2::Prepared,
            revision: 0,
            ticket_record_id,
        }
    }

    /// Hostile-decode one exact canonical replay state.
    pub fn decode(bytes: &[u8]) -> Result<Self, SeriesStateError> {
        if bytes.len() != SERIES_TICKET_STATE_BYTES_V2
            || bytes.get(..8) != Some(TICKET_STATE_MAGIC_V2.as_slice())
            || read_u16(bytes, 8)? != SCHEMA_V2
            || read_u16(bytes, 10)? != PROFILE_V2
            || !all_zero(bytes, 13, 3)?
            || !all_zero(bytes, 56, 8)?
        {
            return Err(SeriesStateError::Encoding);
        }
        let id =
            ContentId::new(read_array::<32>(bytes, 24)?).map_err(|_| SeriesStateError::Identity)?;
        Ok(Self {
            phase: TicketPhaseV2::decode(read_u8(bytes, 12)?)?,
            revision: read_u64(bytes, 16)?,
            ticket_record_id: id,
        })
    }

    /// Return exact canonical bytes.
    pub fn encode(self) -> [u8; SERIES_TICKET_STATE_BYTES_V2] {
        let mut output = [0_u8; SERIES_TICKET_STATE_BYTES_V2];
        output[..8].copy_from_slice(&TICKET_STATE_MAGIC_V2);
        output[8..10].copy_from_slice(&SCHEMA_V2.to_le_bytes());
        output[10..12].copy_from_slice(&PROFILE_V2.to_le_bytes());
        output[12] = self.phase as u8;
        output[16..24].copy_from_slice(&self.revision.to_le_bytes());
        output[24..56].copy_from_slice(&self.ticket_record_id.to_bytes());
        output
    }

    /// Plan the single successful economic terminal transition.
    pub fn settle(
        self,
        expected_revision: u64,
        terminal: TicketPhaseV2,
    ) -> Result<Self, SeriesStateError> {
        if self.phase != TicketPhaseV2::Prepared
            || self.revision != expected_revision
            || !terminal.terminal()
        {
            return Err(SeriesStateError::Replay);
        }
        Ok(Self {
            phase: terminal,
            revision: self
                .revision
                .checked_add(1)
                .ok_or(SeriesStateError::Replay)?,
            ..self
        })
    }

    /// Current phase.
    pub const fn phase(self) -> TicketPhaseV2 {
        self.phase
    }
    /// Current replay revision.
    pub const fn revision(self) -> u64 {
        self.revision
    }
    /// Sole immutable Ticket-record identity.
    pub const fn ticket_record_id(self) -> ContentId {
        self.ticket_record_id
    }
}

/// Exact ticket-state PDA seed projection under the current Trading program.
pub struct TicketStateSeedsV2 {
    root: [u8; 32],
    ticket_record: [u8; 32],
}

impl TicketStateSeedsV2 {
    /// Bind the composite Series root and immutable Ticket-record identity.
    pub const fn new(root: [u8; 32], ticket_record: ContentId) -> Self {
        Self {
            root,
            ticket_record: ticket_record.to_bytes(),
        }
    }

    /// Return exact seed order.
    pub fn as_slices(&self) -> [&[u8]; 3] {
        [
            SERIES_TICKET_STATE_PDA_DOMAIN_V2,
            &self.root,
            &self.ticket_record,
        ]
    }
}

fn all_zero(bytes: &[u8], start: usize, width: usize) -> Result<bool, SeriesStateError> {
    Ok(bytes
        .get(start..start.checked_add(width).ok_or(SeriesStateError::Encoding)?)
        .ok_or(SeriesStateError::Encoding)?
        .iter()
        .all(|byte| *byte == 0))
}

fn read_u8(bytes: &[u8], offset: usize) -> Result<u8, SeriesStateError> {
    bytes.get(offset).copied().ok_or(SeriesStateError::Encoding)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, SeriesStateError> {
    Ok(u16::from_le_bytes(read_array(bytes, offset)?))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, SeriesStateError> {
    Ok(u32::from_le_bytes(read_array(bytes, offset)?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, SeriesStateError> {
    Ok(u64::from_le_bytes(read_array(bytes, offset)?))
}

fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N], SeriesStateError> {
    bytes
        .get(offset..offset.checked_add(N).ok_or(SeriesStateError::Encoding)?)
        .ok_or(SeriesStateError::Encoding)?
        .try_into()
        .map_err(|_| SeriesStateError::Encoding)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(byte: u8) -> ContentId {
        ContentId::new([byte; 32]).expect("nonzero")
    }

    #[test]
    fn exact_roundtrip_and_reserved_bytes_refuse() {
        let state = SeriesStateV2::new(17);
        let bytes = state.encode(2).expect("encode");
        assert_eq!(SeriesStateV2::decode(&bytes, 2), Ok(state));
        let mut hostile = bytes;
        hostile[63] = 1;
        assert_eq!(
            SeriesStateV2::decode(&hostile, 2),
            Err(SeriesStateError::Encoding)
        );

        let ticket = TicketStateV2::prepared(id(3));
        let ticket_bytes = ticket.encode();
        assert_eq!(TicketStateV2::decode(&ticket_bytes), Ok(ticket));
        let mut hostile_ticket = ticket_bytes;
        hostile_ticket[60] = 1;
        assert_eq!(
            TicketStateV2::decode(&hostile_ticket),
            Err(SeriesStateError::Encoding)
        );
    }

    #[test]
    fn prepare_settle_retire_and_close_are_replay_safe() {
        let state = SeriesStateV2::new(9);
        let prepared = state.prepare_ticket(0).expect("prepare");
        assert_eq!(state.prepare_ticket(1), Err(SeriesStateError::Replay));
        let settled = prepared.settle_current(1, 1).expect("settle");
        assert_eq!(settled.phase(), SeriesPhaseV2::Terminal);
        assert_eq!(settled.admit_close(2), Err(SeriesStateError::Replay));
        let retired = settled.retire_ticket(2).expect("retire");
        retired.admit_close(3).expect("close");

        let ticket = TicketStateV2::prepared(id(4));
        let consumed = ticket.settle(0, TicketPhaseV2::Consumed).expect("consume");
        assert_eq!(
            consumed.settle(1, TicketPhaseV2::Expired),
            Err(SeriesStateError::Replay)
        );
    }
}
