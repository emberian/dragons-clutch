//! Exact Trading-owned envelope for General's nonroot local state.
//!
//! Selection and settlement semantics retain their compact independent wire
//! formats. This envelope owns only the physical lifecycle facts required to
//! authenticate a live PDA or create its vacant successor: canonical bump,
//! historical Rent principal, and immutable RentCredit beneficiary.

use crate::general::{
    candidate_v1::{GENERAL_CANDIDATE_BYTES_V1, GeneralCandidateV1},
    collection_v1::{GENERAL_BATCH_BYTES_V1, GeneralBatchV1, GeneralOrderV1, general_order_len_v1},
    runtime_selection::{RUNTIME_SELECTION_CURSOR_BYTES_V2, RuntimeSelectionCursorV2},
    runtime_verify::{RuntimeCandidateVerifierV2, runtime_verifier_len_v2},
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
const KIND_BATCH: u8 = 3;
const KIND_ORDER: u8 = 4;
const KIND_CANDIDATE: u8 = 5;
const KIND_VERIFIER: u8 = 6;

/// Typed byte offsets consumed by AccountProfile and Effect artifacts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralLocalStateLayoutV3;

impl GeneralLocalStateLayoutV3 {
    /// Header magic interpreted as one little-endian scalar.
    pub const fn magic_u64() -> u64 {
        u64::from_le_bytes(GENERAL_LOCAL_STATE_MAGIC_V3)
    }

    /// Exact envelope ABI version.
    pub const fn version_value() -> u16 {
        GENERAL_LOCAL_STATE_VERSION_V3
    }

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
///
/// The first two kinds are the settlement half's own cursors. The four that
/// follow are the collection and candidate halves' records, which existed as
/// pure contract types with no physical envelope at all: nothing said what
/// their canonical PDA bump was, who owned their rent, or how a vacant
/// successor is created. A record with no envelope cannot be the primary state
/// of a Trading lifecycle plan, which is why the seven collection and candidate
/// actions could not have an artifact triple before this.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralLocalStateKindV3 {
    /// Candidate submission and best-valid-submitted selection state.
    Selection,
    /// Permissionless collect/materialize/distribute settlement state.
    Settlement,
    /// One batch window's immutable opening and mutable counters.
    Batch,
    /// One admitted order: signed terms, per-outcome vectors, live phase.
    Order,
    /// One content-addressed candidate submission and its work escrow.
    Candidate,
    /// One candidate's streamed row-verification cursor.
    Verifier,
}

impl GeneralLocalStateKindV3 {
    /// Canonical encoded kind tag.
    pub const fn tag(self) -> u8 {
        match self {
            Self::Selection => KIND_SELECTION,
            Self::Settlement => KIND_SETTLEMENT,
            Self::Batch => KIND_BATCH,
            Self::Order => KIND_ORDER,
            Self::Candidate => KIND_CANDIDATE,
            Self::Verifier => KIND_VERIFIER,
        }
    }

    /// Whether this kind's body width is fixed rather than Product-width.
    ///
    /// A fixed-width body needs no `outcome_count` to size, and its
    /// AccountProfile rule is `Exact` with a zero item stride rather than a
    /// header plus a per-outcome stride.
    #[must_use]
    pub const fn is_fixed_width(self) -> bool {
        matches!(self, Self::Selection | Self::Batch | Self::Candidate)
    }

    fn decode(value: u8) -> Result<Self> {
        match value {
            KIND_SELECTION => Ok(Self::Selection),
            KIND_SETTLEMENT => Ok(Self::Settlement),
            KIND_BATCH => Ok(Self::Batch),
            KIND_ORDER => Ok(Self::Order),
            KIND_CANDIDATE => Ok(Self::Candidate),
            KIND_VERIFIER => Ok(Self::Verifier),
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
            // Each of the four below hostile-decodes through its own semantic
            // owner. The envelope asserts no field of a body it does not own;
            // what it adds is the physical lifecycle the record never had.
            GeneralLocalStateKindV3::Batch => {
                if body.len() != GENERAL_BATCH_BYTES_V1 {
                    return Err(GeneralLocalStateErrorV3::InvalidLength);
                }
                GeneralBatchV1::decode(body).map_err(|_| GeneralLocalStateErrorV3::InvalidBody)?;
            }
            GeneralLocalStateKindV3::Order => {
                let order = GeneralOrderV1::decode(body)
                    .map_err(|_| GeneralLocalStateErrorV3::InvalidBody)?;
                if body.len()
                    != general_order_len_v1(order.header().outcome_count)
                        .map_err(|_| GeneralLocalStateErrorV3::InvalidLength)?
                {
                    return Err(GeneralLocalStateErrorV3::InvalidLength);
                }
            }
            GeneralLocalStateKindV3::Candidate => {
                if body.len() != GENERAL_CANDIDATE_BYTES_V1 {
                    return Err(GeneralLocalStateErrorV3::InvalidLength);
                }
                GeneralCandidateV1::decode(body)
                    .map_err(|_| GeneralLocalStateErrorV3::InvalidBody)?;
            }
            GeneralLocalStateKindV3::Verifier => {
                let cursor = RuntimeCandidateVerifierV2::decode(body)
                    .map_err(|_| GeneralLocalStateErrorV3::InvalidBody)?;
                if body.len()
                    != runtime_verifier_len_v2(cursor.header().outcome_count)
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
        GeneralLocalStateKindV3::Batch => GENERAL_BATCH_BYTES_V1,
        GeneralLocalStateKindV3::Order => general_order_len_v1(outcome_count)
            .map_err(|_| GeneralLocalStateErrorV3::InvalidLength)?,
        GeneralLocalStateKindV3::Candidate => GENERAL_CANDIDATE_BYTES_V1,
        GeneralLocalStateKindV3::Verifier => runtime_verifier_len_v2(outcome_count)
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
        // The two fixed-width records carry an outcome count of their own and
        // it does not size the body; one is the width every other coordinate
        // in the batch agrees on, and asserting it here would make the
        // envelope a second authority over a field the record already owns.
        GeneralLocalStateKindV3::Batch | GeneralLocalStateKindV3::Candidate => 1,
        GeneralLocalStateKindV3::Order => {
            GeneralOrderV1::decode(body)
                .map_err(|_| GeneralLocalStateErrorV3::InvalidBody)?
                .header()
                .outcome_count
        }
        GeneralLocalStateKindV3::Verifier => {
            RuntimeCandidateVerifierV2::decode(body)
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
    use crate::general::collection_v1::{
        GeneralBatchOpeningV1, GeneralOrderHeaderV1, GeneralOrderPhaseV1, GeneralOrderStateV1,
    };
    use crate::general::runtime_width::{SettlementCursorHeaderV2, SettlementPhaseV2};
    use crate::general_config::root::GeneralRootV2;

    const ALL_KINDS: [GeneralLocalStateKindV3; 6] = [
        GeneralLocalStateKindV3::Selection,
        GeneralLocalStateKindV3::Settlement,
        GeneralLocalStateKindV3::Batch,
        GeneralLocalStateKindV3::Order,
        GeneralLocalStateKindV3::Candidate,
        GeneralLocalStateKindV3::Verifier,
    ];

    fn id(low: u8) -> [u8; 32] {
        let mut value = [0_u8; 32];
        value[0] = low;
        value
    }

    fn batch_body(width: u32) -> std::vec::Vec<u8> {
        let mut root = GeneralRootV2::active(id(1), id(2), 7).expect("active root");
        let revision = root.revision();
        let batch = crate::general::collection_v1::GeneralBatchV1::open(
            &mut root,
            GeneralBatchOpeningV1 {
                outcome_count: width,
                sequence: 0,
                generation: 7,
                market: id(1),
                product_id: id(3),
                config_id: id(2),
                price_scale: 100,
                collection_close_slot: 1_000,
                settlement_close_slot: 2_000,
                max_orders: 4,
            },
            revision,
            10,
        )
        .expect("open batch");
        batch.to_bytes().to_vec()
    }

    fn order_body(width: u32) -> std::vec::Vec<u8> {
        let count = usize::try_from(width).expect("width");
        let mut receive = vec![0_u64; count];
        let mut deliver = vec![0_u64; count];
        receive[0] = 1;
        deliver[count - 1] = 2;
        let mut bytes = vec![0_u8; general_order_len_v1(width).expect("order width")];
        GeneralOrderV1::encode_into(
            GeneralOrderHeaderV1 {
                outcome_count: width,
                nonce: 5,
                owner_id: id(9),
                market: id(1),
                batch_id: id(4),
                generation: 7,
                max_lots: 10,
                max_quote_debit_per_lot: 5,
                min_quote_credit_per_lot: 0,
                valid_until_slot: 2_000,
            },
            &receive,
            &deliver,
            GeneralOrderStateV1 {
                phase: GeneralOrderPhaseV1::Placed,
                admitted_slot: 10,
                released_slot: 0,
            },
            &mut bytes,
        )
        .expect("order record");
        bytes
    }

    fn envelope(kind: GeneralLocalStateKindV3, body: &[u8]) -> std::vec::Vec<u8> {
        let len = GENERAL_LOCAL_STATE_HEADER_BYTES_V3 + body.len();
        let mut scratch = vec![0_u8; len];
        let mut output = vec![0xa5_u8; len];
        encode_general_local_state_v3_atomic(
            GeneralLocalStateHeaderV3 {
                kind,
                bump: 251,
                rent_principal: 4_321,
                beneficiary: [8; 32],
            },
            body,
            &mut scratch,
            &mut output,
        )
        .expect("envelope");
        output
    }

    /// The six kinds partition the tag space and nothing else decodes.
    ///
    /// The four collection and candidate kinds are new: before them, those
    /// records had no physical envelope at all, so no Trading lifecycle plan
    /// could name one as its primary state and no action over them could have
    /// an artifact triple.
    #[test]
    fn every_state_kind_has_a_distinct_tag_and_no_other_tag_decodes() {
        let mut seen = std::vec::Vec::new();
        for kind in ALL_KINDS {
            let tag = kind.tag();
            assert!(!seen.contains(&tag), "duplicate kind tag {tag}");
            assert_eq!(
                GeneralLocalStateKindV3::decode(tag).expect("kind decodes"),
                kind
            );
            seen.push(tag);
        }
        for tag in [0_u8, 7, 8, 255] {
            assert_eq!(
                GeneralLocalStateKindV3::decode(tag),
                Err(GeneralLocalStateErrorV3::InvalidKind),
                "tag {tag} decoded as a kind",
            );
        }
    }

    /// Widths at both runtime widths, and the fixed/variable split is real.
    #[test]
    fn every_state_kind_sizes_at_both_runtime_widths() {
        for kind in ALL_KINDS {
            let narrow = general_local_state_len_v3(kind, 1).expect("width at 1");
            let wide = general_local_state_len_v3(kind, 258).expect("width at 258");
            assert!(narrow > GENERAL_LOCAL_STATE_HEADER_BYTES_V3);
            if kind.is_fixed_width() {
                assert_eq!(narrow, wide, "{kind:?} claimed fixed width and moved");
            } else {
                assert!(wide > narrow, "{kind:?} claimed Product width and did not");
            }
        }
    }

    /// The two collection records round-trip through the envelope.
    ///
    /// `Candidate` and `Verifier` bodies cannot be built from a pure
    /// constructor -- a candidate's identity is its own masked digest and a
    /// verifier cursor is written by the first row verification -- so their
    /// envelopes are exercised where those bodies exist, by their own actions'
    /// artifacts and by the campaign.
    #[test]
    fn the_collection_records_round_trip_through_the_envelope() {
        for width in [1_u32, 3, 258] {
            for (kind, body) in [
                (GeneralLocalStateKindV3::Batch, batch_body(width)),
                (GeneralLocalStateKindV3::Order, order_body(width)),
            ] {
                let encoded = envelope(kind, &body);
                let decoded = GeneralLocalStateV3::decode(&encoded).expect("decode");
                assert_eq!(decoded.header().kind, kind);
                assert_eq!(decoded.header().bump, 251);
                assert_eq!(decoded.body(), body.as_slice());
                assert_eq!(
                    encoded.len(),
                    general_local_state_len_v3(kind, width).expect("width"),
                );
            }
        }
    }

    /// A body that is not the kind the header names is refused, both ways.
    ///
    /// The envelope's kind byte is what the AccountProfile rule and the
    /// transition conjunct both read; if it could disagree with the bytes
    /// behind it, an artifact authored for one record would authenticate
    /// another.
    #[test]
    fn a_body_that_is_not_its_declared_kind_refuses() {
        let batch = batch_body(3);
        let order = order_body(3);
        for (kind, body) in [
            (GeneralLocalStateKindV3::Batch, &order),
            (GeneralLocalStateKindV3::Order, &batch),
            (GeneralLocalStateKindV3::Candidate, &batch),
            (GeneralLocalStateKindV3::Verifier, &order),
        ] {
            let len = GENERAL_LOCAL_STATE_HEADER_BYTES_V3 + body.len();
            let mut scratch = vec![0_u8; len];
            let mut output = vec![0xa5_u8; len];
            let before = output.clone();
            assert!(
                encode_general_local_state_v3_atomic(
                    GeneralLocalStateHeaderV3 {
                        kind,
                        bump: 251,
                        rent_principal: 4_321,
                        beneficiary: [8; 32],
                    },
                    body,
                    &mut scratch,
                    &mut output,
                )
                .is_err(),
                "{kind:?} accepted another record's bytes",
            );
            assert_eq!(output, before);
        }
    }

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
