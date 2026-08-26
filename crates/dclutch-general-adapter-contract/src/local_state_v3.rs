//! Exact Trading-owned envelope for General's nonroot local state.
//!
//! Selection and settlement semantics retain their compact independent wire
//! formats. This envelope owns only the physical lifecycle facts required to
//! authenticate a live PDA or create its vacant successor: canonical bump,
//! historical Rent principal, and immutable RentCredit beneficiary.

use crate::{
    runtime_selection::{RUNTIME_SELECTION_CURSOR_BYTES_V2, RuntimeSelectionCursorV2},
    runtime_width::{SettlementCursorV2, settlement_cursor_len},
};

/// Canonical local-state envelope magic.
pub const GENERAL_LOCAL_STATE_MAGIC_V3: [u8; 8] = *b"DCGLST03";
/// Canonical local-state envelope version.
pub const GENERAL_LOCAL_STATE_VERSION_V3: u16 = 3;
/// Exact lifecycle header before the semantic body.
pub const GENERAL_LOCAL_STATE_HEADER_BYTES_V3: usize = 64;

const KIND_SELECTION: u8 = 1;
const KIND_SETTLEMENT: u8 = 2;

/// Typed byte offsets consumed by AccountProfile and Effect artifacts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralLocalStateLayoutV3;

impl GeneralLocalStateLayoutV3 {
    /// Header magic offset.
    pub const fn magic() -> u32 {
        0
    }

    /// Header version offset.
    pub const fn version() -> u32 {
        8
    }

    /// Selection/settlement kind offset.
    pub const fn kind() -> u32 {
        10
    }

    /// Persisted canonical PDA bump offset.
    pub const fn bump() -> u32 {
        11
    }

    /// Historical Rent principal offset.
    pub const fn rent_principal() -> u32 {
        16
    }

    /// Immutable RentCredit-beneficiary offset.
    pub const fn beneficiary() -> u32 {
        24
    }

    /// Embedded semantic-state body offset.
    pub const fn body() -> u32 {
        64
    }
}

/// Semantic body kind selected by one action artifact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralLocalStateKindV3 {
    /// Candidate submission and best-valid-submitted selection state.
    Selection,
    /// Permissionless collect/materialize/distribute settlement state.
    Settlement,
}

impl GeneralLocalStateKindV3 {
    const fn tag(self) -> u8 {
        match self {
            Self::Selection => KIND_SELECTION,
            Self::Settlement => KIND_SETTLEMENT,
        }
    }

    fn decode(value: u8) -> Result<Self> {
        match value {
            KIND_SELECTION => Ok(Self::Selection),
            KIND_SETTLEMENT => Ok(Self::Settlement),
            _ => Err(GeneralLocalStateErrorV3::InvalidKind),
        }
    }
}

/// Hostile-decoded lifecycle facts shared by both state kinds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralLocalStateHeaderV3 {
    /// Semantic state kind.
    pub kind: GeneralLocalStateKindV3,
    /// Canonical Trading PDA bump.
    pub bump: u8,
    /// Historical exact Rent principal.
    pub rent_principal: u64,
    /// Immutable RentCredit beneficiary.
    pub beneficiary: [u8; 32],
}

/// Borrowed exact local-state envelope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralLocalStateV3<'a> {
    header: GeneralLocalStateHeaderV3,
    body: &'a [u8],
}

impl<'a> GeneralLocalStateV3<'a> {
    /// Hostile-decode one exact envelope and its semantic body.
    pub fn decode(bytes: &'a [u8]) -> Result<Self> {
        if bytes.len() < GENERAL_LOCAL_STATE_HEADER_BYTES_V3
            || bytes.get(..8) != Some(GENERAL_LOCAL_STATE_MAGIC_V3.as_slice())
            || read_u16(bytes, 8)? != GENERAL_LOCAL_STATE_VERSION_V3
            || !zero_range(bytes, 12, 4)?
            || !zero_range(bytes, 56, 8)?
        {
            return Err(GeneralLocalStateErrorV3::InvalidEncoding);
        }
        let header = GeneralLocalStateHeaderV3 {
            kind: GeneralLocalStateKindV3::decode(byte(bytes, 10)?)?,
            bump: byte(bytes, 11)?,
            rent_principal: read_u64(bytes, 16)?,
            beneficiary: read_array32(bytes, 24)?,
        };
        if header.rent_principal == 0 || header.beneficiary.iter().all(|byte| *byte == 0) {
            return Err(GeneralLocalStateErrorV3::InvalidLifecycle);
        }
        let body = bytes
            .get(GENERAL_LOCAL_STATE_HEADER_BYTES_V3..)
            .ok_or(GeneralLocalStateErrorV3::InvalidLength)?;
        match header.kind {
            GeneralLocalStateKindV3::Selection => {
                if body.len() != RUNTIME_SELECTION_CURSOR_BYTES_V2 {
                    return Err(GeneralLocalStateErrorV3::InvalidLength);
                }
                RuntimeSelectionCursorV2::decode(body)
                    .map_err(|_| GeneralLocalStateErrorV3::InvalidBody)?;
            }
            GeneralLocalStateKindV3::Settlement => {
                let cursor = SettlementCursorV2::decode(body)
                    .map_err(|_| GeneralLocalStateErrorV3::InvalidBody)?;
                if body.len()
                    != settlement_cursor_len(cursor.header().outcome_count)
                        .map_err(|_| GeneralLocalStateErrorV3::InvalidLength)?
                {
                    return Err(GeneralLocalStateErrorV3::InvalidLength);
                }
            }
        }
        Ok(Self { header, body })
    }

    /// Exact lifecycle header.
    pub const fn header(self) -> GeneralLocalStateHeaderV3 {
        self.header
    }

    /// Exact hostile-decoded semantic body.
    pub const fn body(self) -> &'a [u8] {
        self.body
    }
}

/// Stable local-state refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralLocalStateErrorV3 {
    /// Envelope or body had another exact width.
    InvalidLength,
    /// Magic, version, or reserved bytes differed.
    InvalidEncoding,
    /// State kind was unknown.
    InvalidKind,
    /// Rent principal or beneficiary was zero.
    InvalidLifecycle,
    /// Embedded semantic state refused.
    InvalidBody,
}

/// Result alias for General local-state envelopes.
pub type Result<T> = core::result::Result<T, GeneralLocalStateErrorV3>;

/// Return the exact state width for one action kind and Product width.
pub fn general_local_state_len_v3(
    kind: GeneralLocalStateKindV3,
    outcome_count: u32,
) -> Result<usize> {
    let body = match kind {
        GeneralLocalStateKindV3::Selection => RUNTIME_SELECTION_CURSOR_BYTES_V2,
        GeneralLocalStateKindV3::Settlement => settlement_cursor_len(outcome_count)
            .map_err(|_| GeneralLocalStateErrorV3::InvalidLength)?,
    };
    GENERAL_LOCAL_STATE_HEADER_BYTES_V3
        .checked_add(body)
        .ok_or(GeneralLocalStateErrorV3::InvalidLength)
}

/// Encode one complete envelope atomically into exact caller-owned buffers.
pub fn encode_general_local_state_v3_atomic(
    header: GeneralLocalStateHeaderV3,
    body: &[u8],
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<()> {
    let outcome_count = match header.kind {
        GeneralLocalStateKindV3::Selection => {
            RuntimeSelectionCursorV2::decode(body)
                .map_err(|_| GeneralLocalStateErrorV3::InvalidBody)?;
            1
        }
        GeneralLocalStateKindV3::Settlement => {
            SettlementCursorV2::decode(body)
                .map_err(|_| GeneralLocalStateErrorV3::InvalidBody)?
                .header()
                .outcome_count
        }
    };
    let expected = general_local_state_len_v3(header.kind, outcome_count)?;
    let encoded = GENERAL_LOCAL_STATE_HEADER_BYTES_V3
        .checked_add(body.len())
        .ok_or(GeneralLocalStateErrorV3::InvalidLength)?;
    if scratch.len() != expected || output.len() != expected || encoded != expected {
        return Err(GeneralLocalStateErrorV3::InvalidLength);
    }
    if header.rent_principal == 0 || header.beneficiary.iter().all(|byte| *byte == 0) {
        return Err(GeneralLocalStateErrorV3::InvalidLifecycle);
    }
    scratch.fill(0);
    put(scratch, 0, &GENERAL_LOCAL_STATE_MAGIC_V3)?;
    put(scratch, 8, &GENERAL_LOCAL_STATE_VERSION_V3.to_le_bytes())?;
    put_byte(scratch, 10, header.kind.tag())?;
    put_byte(scratch, 11, header.bump)?;
    put(scratch, 16, &header.rent_principal.to_le_bytes())?;
    put(scratch, 24, &header.beneficiary)?;
    put(scratch, GENERAL_LOCAL_STATE_HEADER_BYTES_V3, body)?;
    GeneralLocalStateV3::decode(scratch)?;
    output.copy_from_slice(scratch);
    Ok(())
}

fn byte(input: &[u8], offset: usize) -> Result<u8> {
    input
        .get(offset)
        .copied()
        .ok_or(GeneralLocalStateErrorV3::InvalidLength)
}

fn read_u16(input: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(read_array(input, offset)?))
}

fn read_u64(input: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(read_array(input, offset)?))
}

fn read_array32(input: &[u8], offset: usize) -> Result<[u8; 32]> {
    read_array(input, offset)
}

fn read_array<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N]> {
    input
        .get(
            offset
                ..offset
                    .checked_add(N)
                    .ok_or(GeneralLocalStateErrorV3::InvalidLength)?,
        )
        .ok_or(GeneralLocalStateErrorV3::InvalidLength)?
        .try_into()
        .map_err(|_| GeneralLocalStateErrorV3::InvalidLength)
}

fn zero_range(input: &[u8], offset: usize, width: usize) -> Result<bool> {
    Ok(input
        .get(
            offset
                ..offset
                    .checked_add(width)
                    .ok_or(GeneralLocalStateErrorV3::InvalidLength)?,
        )
        .ok_or(GeneralLocalStateErrorV3::InvalidLength)?
        .iter()
        .all(|byte| *byte == 0))
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<()> {
    let end = offset
        .checked_add(value.len())
        .ok_or(GeneralLocalStateErrorV3::InvalidLength)?;
    output
        .get_mut(offset..end)
        .ok_or(GeneralLocalStateErrorV3::InvalidLength)?
        .copy_from_slice(value);
    Ok(())
}

fn put_byte(output: &mut [u8], offset: usize, value: u8) -> Result<()> {
    *output
        .get_mut(offset)
        .ok_or(GeneralLocalStateErrorV3::InvalidLength)? = value;
    Ok(())
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::vec;

    use super::*;
    use crate::runtime_width::{SettlementCursorHeaderV2, SettlementPhaseV2};

    fn settlement(outcome_count: u32) -> std::vec::Vec<u8> {
        let mut output = vec![0; settlement_cursor_len(outcome_count).expect("cursor width")];
        SettlementCursorV2::encode_into(
            SettlementCursorHeaderV2 {
                outcome_count,
                order_count: 1,
                next_order: 0,
                revision: 1,
                candidate_id: [3; 32],
                quote_inventory: 0,
                complete_set_quantity: 0,
                terminal_coordinate: 0,
                phase: SettlementPhaseV2::Collecting,
            },
            &vec![0; usize::try_from(outcome_count).expect("width")],
            &mut output,
        )
        .expect("settlement cursor");
        output
    }

    #[test]
    fn settlement_envelope_roundtrips_at_runtime_widths() {
        for outcome_count in [1_u32, 258] {
            let body = settlement(outcome_count);
            let len =
                general_local_state_len_v3(GeneralLocalStateKindV3::Settlement, outcome_count)
                    .expect("state width");
            let mut scratch = vec![0; len];
            let mut output = vec![0xa5; len];
            let header = GeneralLocalStateHeaderV3 {
                kind: GeneralLocalStateKindV3::Settlement,
                bump: 249,
                rent_principal: 1_234,
                beneficiary: [7; 32],
            };
            encode_general_local_state_v3_atomic(header, &body, &mut scratch, &mut output)
                .expect("envelope");
            let decoded = GeneralLocalStateV3::decode(&output).expect("decode");
            assert_eq!(decoded.header(), header);
            assert_eq!(decoded.body(), body);
        }
    }

    #[test]
    fn hostile_lifecycle_metadata_preserves_output() {
        let body = settlement(1);
        let len = general_local_state_len_v3(GeneralLocalStateKindV3::Settlement, 1)
            .expect("state width");
        let mut scratch = vec![0; len];
        let mut output = vec![0xa5; len];
        let before = output.clone();
        assert_eq!(
            encode_general_local_state_v3_atomic(
                GeneralLocalStateHeaderV3 {
                    kind: GeneralLocalStateKindV3::Settlement,
                    bump: 1,
                    rent_principal: 0,
                    beneficiary: [7; 32],
                },
                &body,
                &mut scratch,
                &mut output,
            ),
            Err(GeneralLocalStateErrorV3::InvalidLifecycle)
        );
        assert_eq!(output, before);
    }
}
