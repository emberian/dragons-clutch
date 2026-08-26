//! Runtime-width per-order settlement manifest emitted by candidate verification.
//!
//! Compact candidate rows deliberately omit caller-supplied quote fragments.
//! The streamed verifier derives one candidate-wide rounded debit or credit per
//! authenticated order and emits that result here together with the order's
//! exact aggregate claim movements. Generic Trading persists these immutable
//! manifest rows in authenticated scratch pages and later interprets them for
//! the collect and distribute passes. This prevents pagination from becoming
//! quote-allocation authority.

/// Exact fixed bytes before a manifest's compact order rows.
pub const SETTLEMENT_MANIFEST_HEADER_BYTES_V2: usize = 64;
/// Exact fixed bytes before one order's two runtime-width quantity tails.
pub const SETTLEMENT_ORDER_HEADER_BYTES_V2: usize = 160;

const MANIFEST_MAGIC: [u8; 8] = *b"DCGMAN02";
const ORDER_MAGIC: [u8; 8] = *b"DCGORD02";
const VERSION: u16 = 2;
const MANIFEST_PHASE: u8 = 11;
const ORDER_PHASE: u8 = 12;

/// Stable refusal from a runtime-width settlement manifest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeManifestErrorV2 {
    /// A byte slice had another exact count-derived width.
    InvalidLength,
    /// A checked count, offset, or quantity calculation overflowed.
    ArithmeticOverflow,
    /// Magic, version, phase, or reserved bytes were noncanonical.
    InvalidHeader,
    /// A required content identity, coordinate, or quantity was zero.
    ZeroCoordinate,
    /// An order row belonged to another Candidate or runtime width.
    Substitution,
    /// Order coordinates were not strictly consecutive in the chunk.
    NonCanonicalOrder,
    /// A quantity index exceeded the authenticated runtime width.
    IndexOutOfBounds,
}

/// Result alias for runtime-width settlement manifests.
pub type RuntimeManifestResultV2<T> = core::result::Result<T, RuntimeManifestErrorV2>;

/// Fixed fields for one verifier-emitted settlement-manifest chunk.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementManifestHeaderV2 {
    /// Runtime outcome width.
    pub outcome_count: u32,
    /// Number of newly completed order rows in this verifier step.
    pub order_count: u32,
    /// Immutable candidate coordinate in its Batch.
    pub candidate_coordinate: u32,
    /// Exact successor verifier revision that emitted this chunk.
    pub revision: u64,
    /// Candidate content identity.
    pub candidate_id: [u8; 32],
}

/// Fixed fields for one candidate-wide, verifier-derived order settlement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementOrderHeaderV2 {
    /// Runtime outcome width.
    pub outcome_count: u32,
    /// One-based global order coordinate in the Candidate.
    pub order_coordinate: u32,
    /// Signed order nonce.
    pub nonce: u64,
    /// Candidate content identity.
    pub candidate_id: [u8; 32],
    /// Immutable order content identity.
    pub order_id: [u8; 32],
    /// Immutable order owner identity.
    pub owner_id: [u8; 32],
    /// Exact candidate-wide filled lots for this order.
    pub lots: u64,
    /// Exact candidate-wide rounded quote debit.
    pub quote_debit: u64,
    /// Exact candidate-wide rounded quote credit.
    pub quote_credit: u64,
}

/// Borrowed verifier-derived settlement order with two `u64[N]` tails.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementOrderV2<'a> {
    bytes: &'a [u8],
    header: SettlementOrderHeaderV2,
}

impl<'a> SettlementOrderV2<'a> {
    /// Hostile-decode one exact `160 + 16N` order settlement.
    pub fn decode(bytes: &'a [u8]) -> RuntimeManifestResultV2<Self> {
        require_header(
            bytes,
            &ORDER_MAGIC,
            ORDER_PHASE,
            SETTLEMENT_ORDER_HEADER_BYTES_V2,
        )?;
        let header = SettlementOrderHeaderV2 {
            outcome_count: read_u32(bytes, 12)?,
            order_coordinate: read_u32(bytes, 16)?,
            nonce: read_u64(bytes, 24)?,
            candidate_id: read_array32(bytes, 32)?,
            order_id: read_array32(bytes, 64)?,
            owner_id: read_array32(bytes, 96)?,
            lots: read_u64(bytes, 128)?,
            quote_debit: read_u64(bytes, 136)?,
            quote_credit: read_u64(bytes, 144)?,
        };
        if !zero(bytes, 20, 4)? || !zero(bytes, 152, 8)? {
            return Err(RuntimeManifestErrorV2::InvalidHeader);
        }
        if bytes.len() != settlement_order_len_v2(header.outcome_count)? {
            return Err(RuntimeManifestErrorV2::InvalidLength);
        }
        validate_order_header(header)?;
        let value = Self { bytes, header };
        if !value.has_economic_movement()? {
            return Err(RuntimeManifestErrorV2::ZeroCoordinate);
        }
        Ok(value)
    }

    /// Return fixed order coordinates and derived quote quantities.
    pub const fn header(self) -> SettlementOrderHeaderV2 {
        self.header
    }

    /// Return one exact delivered-claim quantity collected from the owner.
    pub fn claim_input(self, index: u32) -> RuntimeManifestResultV2<u64> {
        read_quantity(self.bytes, self.header.outcome_count, false, index)
    }

    /// Return one exact received-claim quantity distributed to the owner.
    pub fn claim_output(self, index: u32) -> RuntimeManifestResultV2<u64> {
        read_quantity(self.bytes, self.header.outcome_count, true, index)
    }

    /// Return exact canonical order-settlement bytes.
    pub const fn as_bytes(self) -> &'a [u8] {
        self.bytes
    }

    fn has_economic_movement(self) -> RuntimeManifestResultV2<bool> {
        if self.header.quote_debit != 0 || self.header.quote_credit != 0 {
            return Ok(true);
        }
        for outcome in 0..self.header.outcome_count {
            if self.claim_input(outcome)? != 0 || self.claim_output(outcome)? != 0 {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

/// Borrowed exact chunk containing zero, one, or two newly completed orders.
///
/// One ingested execution can close the preceding order and, on the terminal
/// candidate row, the current order. Therefore two is a physical per-step
/// maximum derived from the transition, not a Candidate-wide semantic cap.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementManifestV2<'a> {
    bytes: &'a [u8],
    header: SettlementManifestHeaderV2,
}

impl<'a> SettlementManifestV2<'a> {
    /// Hostile-decode one exact manifest chunk and every embedded order.
    pub fn decode(bytes: &'a [u8]) -> RuntimeManifestResultV2<Self> {
        require_header(
            bytes,
            &MANIFEST_MAGIC,
            MANIFEST_PHASE,
            SETTLEMENT_MANIFEST_HEADER_BYTES_V2,
        )?;
        let header = SettlementManifestHeaderV2 {
            outcome_count: read_u32(bytes, 12)?,
            order_count: read_u32(bytes, 16)?,
            candidate_coordinate: read_u32(bytes, 20)?,
            revision: read_u64(bytes, 24)?,
            candidate_id: read_array32(bytes, 32)?,
        };
        validate_manifest_header(header)?;
        if bytes.len() != settlement_manifest_len_v2(header.outcome_count, header.order_count)? {
            return Err(RuntimeManifestErrorV2::InvalidLength);
        }
        let value = Self { bytes, header };
        let mut previous: Option<u32> = None;
        for index in 0..header.order_count {
            let order = value.order(index)?;
            let order_header = order.header();
            if order_header.outcome_count != header.outcome_count
                || order_header.candidate_id != header.candidate_id
                || previous.is_some_and(|coordinate| {
                    coordinate.checked_add(1) != Some(order_header.order_coordinate)
                })
            {
                return Err(RuntimeManifestErrorV2::NonCanonicalOrder);
            }
            previous = Some(order_header.order_coordinate);
        }
        Ok(value)
    }

    /// Return fixed manifest coordinates.
    pub const fn header(self) -> SettlementManifestHeaderV2 {
        self.header
    }

    /// Borrow one checked verifier-derived order.
    pub fn order(self, index: u32) -> RuntimeManifestResultV2<SettlementOrderV2<'a>> {
        if index >= self.header.order_count {
            return Err(RuntimeManifestErrorV2::IndexOutOfBounds);
        }
        let width = settlement_order_len_v2(self.header.outcome_count)?;
        let index =
            usize::try_from(index).map_err(|_| RuntimeManifestErrorV2::ArithmeticOverflow)?;
        let start = SETTLEMENT_MANIFEST_HEADER_BYTES_V2
            .checked_add(
                index
                    .checked_mul(width)
                    .ok_or(RuntimeManifestErrorV2::ArithmeticOverflow)?,
            )
            .ok_or(RuntimeManifestErrorV2::ArithmeticOverflow)?;
        let end = start
            .checked_add(width)
            .ok_or(RuntimeManifestErrorV2::ArithmeticOverflow)?;
        SettlementOrderV2::decode(
            self.bytes
                .get(start..end)
                .ok_or(RuntimeManifestErrorV2::InvalidLength)?,
        )
    }

    /// Return exact canonical manifest bytes.
    pub const fn as_bytes(self) -> &'a [u8] {
        self.bytes
    }
}

/// Return exact `160 + 16N` bytes for one order settlement.
pub fn settlement_order_len_v2(outcome_count: u32) -> RuntimeManifestResultV2<usize> {
    derived_len(SETTLEMENT_ORDER_HEADER_BYTES_V2, outcome_count, 16)
}

/// Return exact `64 + rows * (160 + 16N)` bytes for one emitted chunk.
pub fn settlement_manifest_len_v2(
    outcome_count: u32,
    order_count: u32,
) -> RuntimeManifestResultV2<usize> {
    let row = settlement_order_len_v2(outcome_count)?;
    let count =
        usize::try_from(order_count).map_err(|_| RuntimeManifestErrorV2::ArithmeticOverflow)?;
    SETTLEMENT_MANIFEST_HEADER_BYTES_V2
        .checked_add(
            row.checked_mul(count)
                .ok_or(RuntimeManifestErrorV2::ArithmeticOverflow)?,
        )
        .ok_or(RuntimeManifestErrorV2::ArithmeticOverflow)
}

pub(crate) fn initialize_manifest_v2(
    header: SettlementManifestHeaderV2,
    output: &mut [u8],
) -> RuntimeManifestResultV2<()> {
    validate_manifest_header(header)?;
    if output.len() != settlement_manifest_len_v2(header.outcome_count, header.order_count)? {
        return Err(RuntimeManifestErrorV2::InvalidLength);
    }
    output.fill(0);
    write_header(output, &MANIFEST_MAGIC, MANIFEST_PHASE)?;
    put_u32(output, 12, header.outcome_count)?;
    put_u32(output, 16, header.order_count)?;
    put_u32(output, 20, header.candidate_coordinate)?;
    put_u64(output, 24, header.revision)?;
    put(output, 32, &header.candidate_id)
}

pub(crate) fn write_scaled_order_v2(
    manifest: &mut [u8],
    index: u32,
    header: SettlementOrderHeaderV2,
    claim_inputs_per_lot_le: &[u8],
    claim_outputs_per_lot_le: &[u8],
) -> RuntimeManifestResultV2<()> {
    let manifest_header = SettlementManifestV2::decode_header(manifest)?;
    if index >= manifest_header.order_count
        || header.outcome_count != manifest_header.outcome_count
        || header.candidate_id != manifest_header.candidate_id
    {
        return Err(RuntimeManifestErrorV2::Substitution);
    }
    validate_order_header(header)?;
    let tail = derived_len(0, header.outcome_count, 8)?;
    if claim_inputs_per_lot_le.len() != tail || claim_outputs_per_lot_le.len() != tail {
        return Err(RuntimeManifestErrorV2::InvalidLength);
    }
    let width = settlement_order_len_v2(header.outcome_count)?;
    let start = SETTLEMENT_MANIFEST_HEADER_BYTES_V2
        .checked_add(
            usize::try_from(index)
                .map_err(|_| RuntimeManifestErrorV2::ArithmeticOverflow)?
                .checked_mul(width)
                .ok_or(RuntimeManifestErrorV2::ArithmeticOverflow)?,
        )
        .ok_or(RuntimeManifestErrorV2::ArithmeticOverflow)?;
    let end = start
        .checked_add(width)
        .ok_or(RuntimeManifestErrorV2::ArithmeticOverflow)?;
    let order = manifest
        .get_mut(start..end)
        .ok_or(RuntimeManifestErrorV2::InvalidLength)?;
    write_header(order, &ORDER_MAGIC, ORDER_PHASE)?;
    put_u32(order, 12, header.outcome_count)?;
    put_u32(order, 16, header.order_coordinate)?;
    put_u64(order, 24, header.nonce)?;
    put(order, 32, &header.candidate_id)?;
    put(order, 64, &header.order_id)?;
    put(order, 96, &header.owner_id)?;
    put_u64(order, 128, header.lots)?;
    put_u64(order, 136, header.quote_debit)?;
    put_u64(order, 144, header.quote_credit)?;
    let outputs = derived_len(SETTLEMENT_ORDER_HEADER_BYTES_V2, header.outcome_count, 8)?;
    for outcome in 0..header.outcome_count {
        let input = multiply(read_le_tail(claim_inputs_per_lot_le, outcome)?, header.lots)?;
        let output = multiply(
            read_le_tail(claim_outputs_per_lot_le, outcome)?,
            header.lots,
        )?;
        let item = usize::try_from(outcome)
            .map_err(|_| RuntimeManifestErrorV2::ArithmeticOverflow)?
            .checked_mul(8)
            .ok_or(RuntimeManifestErrorV2::ArithmeticOverflow)?;
        put_u64(
            order,
            SETTLEMENT_ORDER_HEADER_BYTES_V2
                .checked_add(item)
                .ok_or(RuntimeManifestErrorV2::ArithmeticOverflow)?,
            input,
        )?;
        put_u64(
            order,
            outputs
                .checked_add(item)
                .ok_or(RuntimeManifestErrorV2::ArithmeticOverflow)?,
            output,
        )?;
    }
    SettlementOrderV2::decode(order)?;
    Ok(())
}

impl SettlementManifestV2<'_> {
    fn decode_header(bytes: &[u8]) -> RuntimeManifestResultV2<SettlementManifestHeaderV2> {
        require_header(
            bytes,
            &MANIFEST_MAGIC,
            MANIFEST_PHASE,
            SETTLEMENT_MANIFEST_HEADER_BYTES_V2,
        )?;
        let header = SettlementManifestHeaderV2 {
            outcome_count: read_u32(bytes, 12)?,
            order_count: read_u32(bytes, 16)?,
            candidate_coordinate: read_u32(bytes, 20)?,
            revision: read_u64(bytes, 24)?,
            candidate_id: read_array32(bytes, 32)?,
        };
        validate_manifest_header(header)?;
        if bytes.len() != settlement_manifest_len_v2(header.outcome_count, header.order_count)? {
            return Err(RuntimeManifestErrorV2::InvalidLength);
        }
        Ok(header)
    }
}

fn validate_manifest_header(value: SettlementManifestHeaderV2) -> RuntimeManifestResultV2<()> {
    if value.outcome_count == 0
        || value.candidate_coordinate == 0
        || value.revision == 0
        || zero_identity(&value.candidate_id)
        || value.order_count > 2
    {
        Err(RuntimeManifestErrorV2::ZeroCoordinate)
    } else {
        Ok(())
    }
}

fn validate_order_header(value: SettlementOrderHeaderV2) -> RuntimeManifestResultV2<()> {
    if value.outcome_count == 0
        || value.order_coordinate == 0
        || value.lots == 0
        || zero_identity(&value.candidate_id)
        || zero_identity(&value.order_id)
        || zero_identity(&value.owner_id)
    {
        return Err(RuntimeManifestErrorV2::ZeroCoordinate);
    }
    if value.quote_debit != 0 && value.quote_credit != 0 {
        return Err(RuntimeManifestErrorV2::NonCanonicalOrder);
    }
    Ok(())
}

fn read_quantity(
    bytes: &[u8],
    count: u32,
    outputs: bool,
    index: u32,
) -> RuntimeManifestResultV2<u64> {
    if index >= count {
        return Err(RuntimeManifestErrorV2::IndexOutOfBounds);
    }
    let base = if outputs {
        derived_len(SETTLEMENT_ORDER_HEADER_BYTES_V2, count, 8)?
    } else {
        SETTLEMENT_ORDER_HEADER_BYTES_V2
    };
    let item = usize::try_from(index)
        .map_err(|_| RuntimeManifestErrorV2::ArithmeticOverflow)?
        .checked_mul(8)
        .ok_or(RuntimeManifestErrorV2::ArithmeticOverflow)?;
    read_u64(
        bytes,
        base.checked_add(item)
            .ok_or(RuntimeManifestErrorV2::ArithmeticOverflow)?,
    )
}

fn read_le_tail(bytes: &[u8], index: u32) -> RuntimeManifestResultV2<u64> {
    let item = usize::try_from(index)
        .map_err(|_| RuntimeManifestErrorV2::ArithmeticOverflow)?
        .checked_mul(8)
        .ok_or(RuntimeManifestErrorV2::ArithmeticOverflow)?;
    read_u64(bytes, item)
}

fn derived_len(header: usize, count: u32, stride: usize) -> RuntimeManifestResultV2<usize> {
    let count = usize::try_from(count).map_err(|_| RuntimeManifestErrorV2::ArithmeticOverflow)?;
    header
        .checked_add(
            count
                .checked_mul(stride)
                .ok_or(RuntimeManifestErrorV2::ArithmeticOverflow)?,
        )
        .ok_or(RuntimeManifestErrorV2::ArithmeticOverflow)
}

fn require_header(
    bytes: &[u8],
    magic: &[u8; 8],
    phase: u8,
    minimum: usize,
) -> RuntimeManifestResultV2<()> {
    if bytes.len() < minimum
        || bytes.get(..8) != Some(magic.as_slice())
        || read_u16(bytes, 8)? != VERSION
        || read_byte(bytes, 10)? != phase
        || read_byte(bytes, 11)? != 0
    {
        Err(RuntimeManifestErrorV2::InvalidHeader)
    } else {
        Ok(())
    }
}

fn write_header(bytes: &mut [u8], magic: &[u8; 8], phase: u8) -> RuntimeManifestResultV2<()> {
    put(bytes, 0, magic)?;
    put_u16(bytes, 8, VERSION)?;
    put_byte(bytes, 10, phase)
}

fn zero(bytes: &[u8], offset: usize, length: usize) -> RuntimeManifestResultV2<bool> {
    let end = offset
        .checked_add(length)
        .ok_or(RuntimeManifestErrorV2::ArithmeticOverflow)?;
    Ok(bytes
        .get(offset..end)
        .ok_or(RuntimeManifestErrorV2::InvalidLength)?
        .iter()
        .all(|byte| *byte == 0))
}

fn zero_identity(value: &[u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}

fn multiply(left: u64, right: u64) -> RuntimeManifestResultV2<u64> {
    left.checked_mul(right)
        .ok_or(RuntimeManifestErrorV2::ArithmeticOverflow)
}

fn read_byte(bytes: &[u8], offset: usize) -> RuntimeManifestResultV2<u8> {
    bytes
        .get(offset)
        .copied()
        .ok_or(RuntimeManifestErrorV2::InvalidLength)
}

fn read_u16(bytes: &[u8], offset: usize) -> RuntimeManifestResultV2<u16> {
    let end = offset
        .checked_add(2)
        .ok_or(RuntimeManifestErrorV2::ArithmeticOverflow)?;
    let array = <[u8; 2]>::try_from(
        bytes
            .get(offset..end)
            .ok_or(RuntimeManifestErrorV2::InvalidLength)?,
    )
    .map_err(|_| RuntimeManifestErrorV2::InvalidLength)?;
    Ok(u16::from_le_bytes(array))
}

fn read_u32(bytes: &[u8], offset: usize) -> RuntimeManifestResultV2<u32> {
    let end = offset
        .checked_add(4)
        .ok_or(RuntimeManifestErrorV2::ArithmeticOverflow)?;
    let array = <[u8; 4]>::try_from(
        bytes
            .get(offset..end)
            .ok_or(RuntimeManifestErrorV2::InvalidLength)?,
    )
    .map_err(|_| RuntimeManifestErrorV2::InvalidLength)?;
    Ok(u32::from_le_bytes(array))
}

fn read_u64(bytes: &[u8], offset: usize) -> RuntimeManifestResultV2<u64> {
    let end = offset
        .checked_add(8)
        .ok_or(RuntimeManifestErrorV2::ArithmeticOverflow)?;
    let array = <[u8; 8]>::try_from(
        bytes
            .get(offset..end)
            .ok_or(RuntimeManifestErrorV2::InvalidLength)?,
    )
    .map_err(|_| RuntimeManifestErrorV2::InvalidLength)?;
    Ok(u64::from_le_bytes(array))
}

fn read_array32(bytes: &[u8], offset: usize) -> RuntimeManifestResultV2<[u8; 32]> {
    let end = offset
        .checked_add(32)
        .ok_or(RuntimeManifestErrorV2::ArithmeticOverflow)?;
    <[u8; 32]>::try_from(
        bytes
            .get(offset..end)
            .ok_or(RuntimeManifestErrorV2::InvalidLength)?,
    )
    .map_err(|_| RuntimeManifestErrorV2::InvalidLength)
}

fn put(bytes: &mut [u8], offset: usize, value: &[u8]) -> RuntimeManifestResultV2<()> {
    let end = offset
        .checked_add(value.len())
        .ok_or(RuntimeManifestErrorV2::ArithmeticOverflow)?;
    bytes
        .get_mut(offset..end)
        .ok_or(RuntimeManifestErrorV2::InvalidLength)?
        .copy_from_slice(value);
    Ok(())
}

fn put_byte(bytes: &mut [u8], offset: usize, value: u8) -> RuntimeManifestResultV2<()> {
    *bytes
        .get_mut(offset)
        .ok_or(RuntimeManifestErrorV2::InvalidLength)? = value;
    Ok(())
}

fn put_u16(bytes: &mut [u8], offset: usize, value: u16) -> RuntimeManifestResultV2<()> {
    put(bytes, offset, &value.to_le_bytes())
}

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) -> RuntimeManifestResultV2<()> {
    put(bytes, offset, &value.to_le_bytes())
}

fn put_u64(bytes: &mut [u8], offset: usize, value: u64) -> RuntimeManifestResultV2<()> {
    put(bytes, offset, &value.to_le_bytes())
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use std::vec;

    const CANDIDATE: [u8; 32] = [1; 32];

    fn header(rows: u32, width: u32) -> SettlementManifestHeaderV2 {
        SettlementManifestHeaderV2 {
            outcome_count: width,
            order_count: rows,
            candidate_coordinate: 7,
            revision: 9,
            candidate_id: CANDIDATE,
        }
    }

    fn order(coordinate: u32, width: u32) -> SettlementOrderHeaderV2 {
        SettlementOrderHeaderV2 {
            outcome_count: width,
            order_coordinate: coordinate,
            nonce: 8,
            candidate_id: CANDIDATE,
            order_id: [u8::try_from(coordinate).expect("small coordinate"); 32],
            owner_id: [3; 32],
            lots: 2,
            quote_debit: 4,
            quote_credit: 0,
        }
    }

    #[test]
    fn zero_one_and_two_order_chunks_have_exact_runtime_width() {
        for width in [1_u32, 16, 258] {
            for rows in 0..=2 {
                let mut bytes = vec![0; settlement_manifest_len_v2(width, rows).expect("width")];
                initialize_manifest_v2(header(rows, width), &mut bytes).expect("header");
                let tail =
                    [1_u8, 0, 0, 0, 0, 0, 0, 0].repeat(usize::try_from(width).expect("test width"));
                for index in 0..rows {
                    write_scaled_order_v2(&mut bytes, index, order(index + 1, width), &tail, &tail)
                        .expect("order");
                }
                let manifest = SettlementManifestV2::decode(&bytes).expect("manifest");
                assert_eq!(manifest.header().order_count, rows);
                if rows != 0 {
                    assert_eq!(
                        manifest
                            .order(rows - 1)
                            .expect("order")
                            .claim_input(width - 1),
                        Ok(2)
                    );
                }
            }
        }
    }

    #[test]
    fn substitution_gap_and_hostile_tail_refuse() {
        let width = 2;
        let mut bytes = vec![0; settlement_manifest_len_v2(width, 2).expect("width")];
        initialize_manifest_v2(header(2, width), &mut bytes).expect("header");
        let tail = [1_u8, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0];
        write_scaled_order_v2(&mut bytes, 0, order(1, width), &tail, &tail).expect("first");
        write_scaled_order_v2(&mut bytes, 1, order(2, width), &tail, &tail).expect("second");
        assert!(SettlementManifestV2::decode(&bytes).is_ok());

        let second = SETTLEMENT_MANIFEST_HEADER_BYTES_V2
            + settlement_order_len_v2(width).expect("order width");
        bytes[second + 16..second + 20].copy_from_slice(&3_u32.to_le_bytes());
        assert_eq!(
            SettlementManifestV2::decode(&bytes),
            Err(RuntimeManifestErrorV2::NonCanonicalOrder)
        );
        bytes.truncate(bytes.len() - 1);
        assert_eq!(
            SettlementManifestV2::decode(&bytes),
            Err(RuntimeManifestErrorV2::InvalidLength)
        );
    }
}
