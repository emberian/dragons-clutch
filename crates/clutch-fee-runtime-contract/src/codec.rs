//! Fixed-layout semantic account bodies for a future Solana adapter.
//!
//! These codecs allocate no outer program tag or PDA seed. Decode always
//! reconstructs through the semantic constructor and compares the canonical
//! re-encoding, so persisted derived words never become a second authority.

use clutch_batch::relation_v1::FrozenPolicyV1;
use clutch_batch_policy_identity::revenue_policy_v1::RevenuePolicyV1;
use clutch_batch_policy_identity::revenue_policy_v2::RevenuePolicyV2;

use crate::allocation::{
    allocate_payer_debit, allocate_recipients, FeeEnvelopeV1, PayerAllocationV1,
    RecipientAllocationV1, StandingMakerRowV1,
};
use crate::selected::{
    OwnerFeeAssessmentV1, OwnerFeeCarryV1, SelectedCompositeFeeAccess,
    SelectedCompositeFeeV1, SelectedCompositeFeeV2,
};
use crate::projection::{
    CertifiedRecipientAllocationAccessV2, CertifiedRecipientAllocationV2,
    CertifiedRecipientAllocationV3,
};
use crate::treasury::TreasuryLedgerV1;
use crate::{add, live, Error, Id, Result, MAX_FEE_ROWS_V1};

pub const FEE_RECORD_ACCOUNT_V1_BYTES: usize = 336;
pub const OWNER_FEE_CARRY_ACCOUNT_V1_BYTES: usize = 128;
pub const PAYER_ALLOCATION_ACCOUNT_V1_BYTES: usize = 2_680;
pub const RECIPIENT_ALLOCATION_ACCOUNT_V1_BYTES: usize = 2_640;
pub const CERTIFIED_RECIPIENT_ALLOCATION_V2_BYTES: usize =
    RECIPIENT_ALLOCATION_ACCOUNT_V1_BYTES + 32 + 32 + 2 + 6;
/// Exact current recipient body: V1 allocation plus V2 weight-stream
/// provenance, traversal digest, explicit cardinalities, and zero padding.
pub const CERTIFIED_RECIPIENT_ALLOCATION_V3_BYTES: usize =
    RECIPIENT_ALLOCATION_ACCOUNT_V1_BYTES + 32 + 32 + 32 + 2 + 1 + 5;
pub const TREASURY_LEDGER_ACCOUNT_V1_BYTES: usize = 144;

const RECIPIENT_ALLOCATION_V1_ROWS_OFFSET: usize = 80;
const RECIPIENT_ALLOCATION_V1_ROW_BYTES: usize = 32 + 8;

/// One borrowed canonical recipient row from persisted allocation bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BorrowedRecipientAllocationRowV1 {
    position: Id,
    rebate_atoms: u64,
}

impl BorrowedRecipientAllocationRowV1 {
    /// Construct one structural row for a streaming authenticated writer.
    ///
    /// This does not confer allocation authority. It only prevents private
    /// adapters from manufacturing this crate's fields directly; the V3
    /// streaming encoder still rechecks live Position identity, strict order,
    /// cardinality, zero tail, and full conservation.
    pub fn structural(position: Id, rebate_atoms: u64) -> Result<Self> {
        live(position)?;
        Ok(Self {
            position,
            rebate_atoms,
        })
    }

    /// Canonical ordinary Position account identity.
    pub const fn position(self) -> Id { self.position }
    /// Hamilton-assigned final collateral atoms for this Position.
    pub const fn rebate_atoms(self) -> u64 { self.rebate_atoms }
}

/// Borrowed, structurally authenticated V1 allocation body.
///
/// This view never copies the two maximum-width Position/rebate arrays. It is
/// created only by the strict V3 decoder, which has already checked active-row
/// order, zero padding, and full conservation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BorrowedRecipientAllocationV1<'a> {
    bytes: &'a [u8],
    fee_record: Id,
    maker_len: u8,
    maker_rebate_total: u64,
    executor_atoms: u64,
    treasury_atoms: u64,
    collected_fee_atoms: u64,
}

impl BorrowedRecipientAllocationV1<'_> {
    /// Selected fee-record identity.
    pub const fn fee_record(&self) -> Id { self.fee_record }
    /// Number of active Position rows.
    pub const fn maker_len(&self) -> u8 { self.maker_len }
    /// Conserved maker-rebate pool.
    pub const fn maker_rebate_total(&self) -> u64 { self.maker_rebate_total }
    /// Executor allocation.
    pub const fn executor_atoms(&self) -> u64 { self.executor_atoms }
    /// Treasury allocation.
    pub const fn treasury_atoms(&self) -> u64 { self.treasury_atoms }
    /// Exact collected terminal fee atoms.
    pub const fn collected_fee_atoms(&self) -> u64 { self.collected_fee_atoms }

    /// Read one active row without exposing or copying the backing byte array.
    pub fn row(&self, index: u8) -> Result<Option<BorrowedRecipientAllocationRowV1>> {
        if index >= self.maker_len {
            return Ok(None);
        }
        let at = RECIPIENT_ALLOCATION_V1_ROWS_OFFSET
            .checked_add(
                usize::from(index)
                    .checked_mul(RECIPIENT_ALLOCATION_V1_ROW_BYTES)
                    .ok_or(Error::ArithmeticOverflow)?,
            )
            .ok_or(Error::ArithmeticOverflow)?;
        let mut cursor = at;
        let position = read_id(self.bytes, &mut cursor)?;
        let rebate_atoms = read_u64(self.bytes, &mut cursor)?;
        Ok(Some(BorrowedRecipientAllocationRowV1 {
            position,
            rebate_atoms,
        }))
    }
}

/// Borrowed current V3 semantic projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BorrowedCertifiedRecipientAllocationV3<'a> {
    allocation: BorrowedRecipientAllocationV1<'a>,
    weight_policy_id: Id,
    weight_transcript_id: Id,
    owner_order_set_digest: Id,
    traversed_owner_count: u16,
    nonzero_weight_row_count: u8,
}

/// Compact immutable projection of a strictly decoded current V3 body.
///
/// This is derived only from the borrowed decoder; it owns no rows and cannot
/// be used to create an allocation. Consumers that need Hamilton rows must
/// retain the borrowed access view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CertifiedRecipientAllocationSummaryV3 {
    fee_record: Id,
    row_count: u8,
    collected_fee_atoms: u64,
    weight_policy_id: Id,
    weight_transcript_id: Id,
    owner_order_set_digest: Id,
    traversed_owner_count: u16,
    nonzero_weight_row_count: u8,
}

impl CertifiedRecipientAllocationSummaryV3 {
    /// Selected fee-record identity.
    pub const fn fee_record(&self) -> Id { self.fee_record }
    /// Exact Position allocation row count.
    pub const fn row_count(&self) -> u8 { self.row_count }
    /// Exact collected terminal fee atoms.
    pub const fn collected_fee_atoms(&self) -> u64 { self.collected_fee_atoms }
    /// Immutable exact-weight policy identity.
    pub const fn weight_policy_id(&self) -> Id { self.weight_policy_id }
    /// Complete exact-weight transcript identity.
    pub const fn weight_transcript_id(&self) -> Id { self.weight_transcript_id }
    /// Traversal-owned owner/order-set digest.
    pub const fn owner_order_set_digest(&self) -> Id { self.owner_order_set_digest }
    /// Distinct traversed owners before zero omission.
    pub const fn traversed_owner_count(&self) -> u16 { self.traversed_owner_count }
    /// Exact number of nonzero-weight Position rows.
    pub const fn nonzero_weight_row_count(&self) -> u8 { self.nonzero_weight_row_count }
}

/// Read-only current recipient-allocation projection.
///
/// Implementations expose only the canonical header and Position-sorted row
/// stream. The streaming encoder revalidates every invariant; implementing
/// this trait is not creation authority. A live adapter must obtain these
/// facts from the authenticated V2 weight stream and its exact Hamilton
/// allocation, never from a packet or caller-built row list.
pub trait CertifiedRecipientAllocationAccessV3 {
    /// Selected fee-record identity carried by the allocation.
    fn fee_record(&self) -> Id;
    /// Number of Position rows in the exact Hamilton allocation.
    fn row_count(&self) -> u8;
    /// Exact maker-rebate pool.
    fn maker_rebate_total(&self) -> u64;
    /// Exact executor allocation.
    fn executor_atoms(&self) -> u64;
    /// Exact treasury allocation.
    fn treasury_atoms(&self) -> u64;
    /// Exact collected terminal fees.
    fn collected_fee_atoms(&self) -> u64;
    /// Canonical Position-sorted row, or `None` at and after the zero tail.
    fn row(&self, index: u8) -> Result<Option<BorrowedRecipientAllocationRowV1>>;
    /// Immutable exact-weight policy identity.
    fn weight_policy_id(&self) -> Id;
    /// Complete exact-weight transcript identity.
    fn weight_transcript_id(&self) -> Id;
    /// Traversal-owned owner/order-set digest.
    fn owner_order_set_digest(&self) -> Id;
    /// Distinct traversed owner count before zero-weight omission.
    fn traversed_owner_count(&self) -> u16;
    /// Nonzero-weight Position row count.
    fn nonzero_weight_row_count(&self) -> u8;
}

impl<'a> BorrowedCertifiedRecipientAllocationV3<'a> {
    /// Borrowed exact allocation rows and totals.
    pub const fn allocation(&self) -> BorrowedRecipientAllocationV1<'a> { self.allocation }
    /// Immutable exact-weight policy identity.
    pub const fn weight_policy_id(&self) -> Id { self.weight_policy_id }
    /// Complete V2 exact-weight transcript commitment.
    pub const fn weight_transcript_id(&self) -> Id { self.weight_transcript_id }
    /// Traversal-owned owner/order-set digest.
    pub const fn owner_order_set_digest(&self) -> Id { self.owner_order_set_digest }
    /// Distinct traversed owners before zero omission.
    pub const fn traversed_owner_count(&self) -> u16 { self.traversed_owner_count }
    /// Exact nonzero-weight Position row count.
    pub const fn nonzero_weight_row_count(&self) -> u8 {
        self.nonzero_weight_row_count
    }

    /// Detach the compact, already-validated header for O(1) consumers.
    pub const fn summary(&self) -> CertifiedRecipientAllocationSummaryV3 {
        CertifiedRecipientAllocationSummaryV3 {
            fee_record: self.allocation.fee_record,
            row_count: self.allocation.maker_len,
            collected_fee_atoms: self.allocation.collected_fee_atoms,
            weight_policy_id: self.weight_policy_id,
            weight_transcript_id: self.weight_transcript_id,
            owner_order_set_digest: self.owner_order_set_digest,
            traversed_owner_count: self.traversed_owner_count,
            nonzero_weight_row_count: self.nonzero_weight_row_count,
        }
    }
}

impl CertifiedRecipientAllocationAccessV3 for BorrowedCertifiedRecipientAllocationV3<'_> {
    fn fee_record(&self) -> Id { self.allocation.fee_record() }
    fn row_count(&self) -> u8 { self.allocation.maker_len() }
    fn maker_rebate_total(&self) -> u64 { self.allocation.maker_rebate_total() }
    fn executor_atoms(&self) -> u64 { self.allocation.executor_atoms() }
    fn treasury_atoms(&self) -> u64 { self.allocation.treasury_atoms() }
    fn collected_fee_atoms(&self) -> u64 { self.allocation.collected_fee_atoms() }
    fn row(&self, index: u8) -> Result<Option<BorrowedRecipientAllocationRowV1>> {
        self.allocation.row(index)
    }
    fn weight_policy_id(&self) -> Id { self.weight_policy_id }
    fn weight_transcript_id(&self) -> Id { self.weight_transcript_id }
    fn owner_order_set_digest(&self) -> Id { self.owner_order_set_digest }
    fn traversed_owner_count(&self) -> u16 { self.traversed_owner_count }
    fn nonzero_weight_row_count(&self) -> u8 { self.nonzero_weight_row_count }
}

impl CertifiedRecipientAllocationAccessV3 for CertifiedRecipientAllocationV3 {
    fn fee_record(&self) -> Id { self.allocation_ref().fee_record() }
    fn row_count(&self) -> u8 { self.allocation_ref().maker_len() }
    fn maker_rebate_total(&self) -> u64 { self.allocation_ref().maker_rebate_total() }
    fn executor_atoms(&self) -> u64 { self.allocation_ref().executor_atoms() }
    fn treasury_atoms(&self) -> u64 { self.allocation_ref().treasury_atoms() }
    fn collected_fee_atoms(&self) -> u64 { self.allocation_ref().collected_fee_atoms() }
    fn row(&self, index: u8) -> Result<Option<BorrowedRecipientAllocationRowV1>> {
        if index >= self.allocation_ref().maker_len() {
            return Ok(None);
        }
        let at = usize::from(index);
        Ok(Some(BorrowedRecipientAllocationRowV1 {
            position: self.allocation_ref().maker_positions()[at],
            rebate_atoms: self.allocation_ref().maker_rebate_atoms()[at],
        }))
    }
    fn weight_policy_id(&self) -> Id { self.weight_policy_id() }
    fn weight_transcript_id(&self) -> Id { self.weight_transcript_id() }
    fn owner_order_set_digest(&self) -> Id { self.owner_order_set_digest() }
    fn traversed_owner_count(&self) -> u16 { self.traversed_owner_count() }
    fn nonzero_weight_row_count(&self) -> u8 { self.nonzero_weight_row_count() }
}

pub const FEE_RECORD_MAGIC_V1: [u8; 8] = *b"DCFEESEL";
pub const FEE_RECORD_MAGIC_V2: [u8; 8] = *b"DCFEESE2";
pub const OWNER_FEE_CARRY_MAGIC_V1: [u8; 8] = *b"DCFEECRY";
pub const PAYER_ALLOCATION_MAGIC_V1: [u8; 8] = *b"DCFEEPAY";
pub const RECIPIENT_ALLOCATION_MAGIC_V1: [u8; 8] = *b"DCFEEREC";
pub const TREASURY_LEDGER_MAGIC_V1: [u8; 8] = *b"DCFEETRY";

const CODEC_VERSION_V1: u16 = 1;
const CODEC_VERSION_V2: u16 = 2;
const CODEC_FLAGS_V1: u16 = 0;

pub fn encode_fee_record_v1(
    selected: &SelectedCompositeFeeV1,
) -> Result<[u8; FEE_RECORD_ACCOUNT_V1_BYTES]> {
    let mut output = [0u8; FEE_RECORD_ACCOUNT_V1_BYTES];
    let mut cursor = 0usize;
    put_header(&mut output, &mut cursor, FEE_RECORD_MAGIC_V1)?;
    for identity in [
        selected.fee_record(),
        selected.realm(),
        selected.market(),
        selected.epoch(),
        selected.selected_candidate(),
        selected.batch_policy(),
        selected.revenue_policy(),
        selected.treasury_owner(),
        selected.treasury_position(),
    ] {
        put(&mut output, &mut cursor, &identity.0)?;
    }
    put(
        &mut output,
        &mut cursor,
        &selected.price_scale().to_le_bytes(),
    )?;
    put(&mut output, &mut cursor, &[selected.outcome_count()])?;
    put(&mut output, &mut cursor, &[0; 3])?;
    put(
        &mut output,
        &mut cursor,
        &selected.dispersion_bps().to_le_bytes(),
    )?;
    put(
        &mut output,
        &mut cursor,
        &selected.floor_range_bps().to_le_bytes(),
    )?;
    put(
        &mut output,
        &mut cursor,
        &selected.carry_denominator().to_le_bytes(),
    )?;
    finish(cursor, output.len())?;
    Ok(output)
}

pub fn decode_fee_record_v1(
    input: &[u8],
    batch: &FrozenPolicyV1,
    revenue: &RevenuePolicyV1,
) -> Result<SelectedCompositeFeeV1> {
    exact_len(input, FEE_RECORD_ACCOUNT_V1_BYTES)?;
    let mut cursor = 0usize;
    take_header(input, &mut cursor, FEE_RECORD_MAGIC_V1)?;
    let fee_record = read_id(input, &mut cursor)?;
    let realm = read_id(input, &mut cursor)?;
    let market = read_id(input, &mut cursor)?;
    let epoch = read_id(input, &mut cursor)?;
    let candidate = read_id(input, &mut cursor)?;
    let _batch_policy = read_id(input, &mut cursor)?;
    let _revenue_policy = read_id(input, &mut cursor)?;
    let _treasury_owner = read_id(input, &mut cursor)?;
    let treasury_position = read_id(input, &mut cursor)?;
    let price_scale = read_u64(input, &mut cursor)?;
    let outcome_count = read_u8(input, &mut cursor)?;
    require_zero(take(input, &mut cursor, 3)?)?;
    let _dispersion_bps = read_u32(input, &mut cursor)?;
    let _floor_range_bps = read_u32(input, &mut cursor)?;
    let _denominator = read_u128(input, &mut cursor)?;
    finish(cursor, input.len())?;
    let selected = SelectedCompositeFeeV1::select(
        fee_record,
        realm,
        market,
        epoch,
        candidate,
        treasury_position,
        price_scale,
        outcome_count,
        batch,
        revenue,
    )?;
    if encode_fee_record_v1(&selected)?.as_slice() != input {
        return Err(Error::MismatchedBinding);
    }
    Ok(selected)
}

/// Encode the fresh RevenuePolicyV2-selected semantic body. The width is
/// intentionally unchanged, but both magic and schema version are distinct.
pub fn encode_fee_record_v2(
    selected: &SelectedCompositeFeeV2,
) -> Result<[u8; FEE_RECORD_ACCOUNT_V1_BYTES]> {
    let mut output = [0u8; FEE_RECORD_ACCOUNT_V1_BYTES];
    let mut cursor = 0usize;
    put_header_version(
        &mut output,
        &mut cursor,
        FEE_RECORD_MAGIC_V2,
        CODEC_VERSION_V2,
    )?;
    for identity in [
        selected.fee_record(),
        selected.realm(),
        selected.market(),
        selected.epoch(),
        selected.selected_candidate(),
        selected.batch_policy(),
        selected.revenue_policy(),
        selected.treasury_owner(),
        selected.treasury_position(),
    ] {
        put(&mut output, &mut cursor, &identity.0)?;
    }
    put(
        &mut output,
        &mut cursor,
        &selected.price_scale().to_le_bytes(),
    )?;
    put(&mut output, &mut cursor, &[selected.outcome_count()])?;
    put(&mut output, &mut cursor, &[0; 3])?;
    put(
        &mut output,
        &mut cursor,
        &selected.dispersion_bps().to_le_bytes(),
    )?;
    put(
        &mut output,
        &mut cursor,
        &selected.floor_range_bps().to_le_bytes(),
    )?;
    put(
        &mut output,
        &mut cursor,
        &selected.carry_denominator().to_le_bytes(),
    )?;
    finish(cursor, output.len())?;
    Ok(output)
}

pub fn decode_fee_record_v2(
    input: &[u8],
    batch: &FrozenPolicyV1,
    revenue: &RevenuePolicyV2,
) -> Result<SelectedCompositeFeeV2> {
    exact_len(input, FEE_RECORD_ACCOUNT_V1_BYTES)?;
    let mut cursor = 0usize;
    take_header_version(
        input,
        &mut cursor,
        FEE_RECORD_MAGIC_V2,
        CODEC_VERSION_V2,
    )?;
    let fee_record = read_id(input, &mut cursor)?;
    let realm = read_id(input, &mut cursor)?;
    let market = read_id(input, &mut cursor)?;
    let epoch = read_id(input, &mut cursor)?;
    let candidate = read_id(input, &mut cursor)?;
    let _batch_policy = read_id(input, &mut cursor)?;
    let _revenue_policy = read_id(input, &mut cursor)?;
    let _treasury_owner = read_id(input, &mut cursor)?;
    let treasury_position = read_id(input, &mut cursor)?;
    let price_scale = read_u64(input, &mut cursor)?;
    let outcome_count = read_u8(input, &mut cursor)?;
    require_zero(take(input, &mut cursor, 3)?)?;
    let _dispersion_bps = read_u32(input, &mut cursor)?;
    let _floor_range_bps = read_u32(input, &mut cursor)?;
    let _denominator = read_u128(input, &mut cursor)?;
    finish(cursor, input.len())?;
    let selected = SelectedCompositeFeeV2::select(
        fee_record,
        realm,
        market,
        epoch,
        candidate,
        treasury_position,
        price_scale,
        outcome_count,
        batch,
        revenue,
    )?;
    if encode_fee_record_v2(&selected)?.as_slice() != input {
        return Err(Error::MismatchedBinding);
    }
    Ok(selected)
}

/// Structurally decode the exact current selected transcript without
/// accepting its copied policy identities as authority. The adapter must join
/// those fields to hostile-authenticated batch/Revenue/Market state.
pub fn decode_persisted_fee_record_v2(input: &[u8]) -> Result<SelectedCompositeFeeV2> {
    exact_len(input, FEE_RECORD_ACCOUNT_V1_BYTES)?;
    let mut cursor = 0usize;
    take_header_version(
        input,
        &mut cursor,
        FEE_RECORD_MAGIC_V2,
        CODEC_VERSION_V2,
    )?;
    let fee_record = read_id(input, &mut cursor)?;
    let realm = read_id(input, &mut cursor)?;
    let market = read_id(input, &mut cursor)?;
    let epoch = read_id(input, &mut cursor)?;
    let candidate = read_id(input, &mut cursor)?;
    let batch_policy = read_id(input, &mut cursor)?;
    let revenue_policy = read_id(input, &mut cursor)?;
    let treasury_owner = read_id(input, &mut cursor)?;
    let treasury_position = read_id(input, &mut cursor)?;
    let price_scale = read_u64(input, &mut cursor)?;
    let outcome_count = read_u8(input, &mut cursor)?;
    require_zero(take(input, &mut cursor, 3)?)?;
    let dispersion_bps = read_u32(input, &mut cursor)?;
    let floor_range_bps = read_u32(input, &mut cursor)?;
    let carry_denominator = read_u128(input, &mut cursor)?;
    finish(cursor, input.len())?;
    let selected = SelectedCompositeFeeV2::restore_persisted(
        fee_record,
        realm,
        market,
        epoch,
        candidate,
        batch_policy,
        revenue_policy,
        treasury_owner,
        treasury_position,
        price_scale,
        outcome_count,
        dispersion_bps,
        floor_range_bps,
        carry_denominator,
    )?;
    if encode_fee_record_v2(&selected)?.as_slice() != input {
        return Err(Error::MismatchedBinding);
    }
    Ok(selected)
}

pub fn encode_owner_fee_carry_v1(
    carry: &OwnerFeeCarryV1,
) -> Result<[u8; OWNER_FEE_CARRY_ACCOUNT_V1_BYTES]> {
    let mut output = [0u8; OWNER_FEE_CARRY_ACCOUNT_V1_BYTES];
    let mut cursor = 0usize;
    put_header(&mut output, &mut cursor, OWNER_FEE_CARRY_MAGIC_V1)?;
    put(&mut output, &mut cursor, &carry.fee_record().0)?;
    put(&mut output, &mut cursor, &carry.owner().0)?;
    put(&mut output, &mut cursor, &carry.denominator().to_le_bytes())?;
    put(&mut output, &mut cursor, &carry.remainder().to_le_bytes())?;
    put(&mut output, &mut cursor, &carry.paid_atoms().to_le_bytes())?;
    put(&mut output, &mut cursor, &[u8::from(carry.is_closed())])?;
    put(&mut output, &mut cursor, &[0; 11])?;
    finish(cursor, output.len())?;
    Ok(output)
}

pub fn decode_owner_fee_carry_v1<S: SelectedCompositeFeeAccess + ?Sized>(
    input: &[u8],
    selected: &S,
) -> Result<OwnerFeeCarryV1> {
    exact_len(input, OWNER_FEE_CARRY_ACCOUNT_V1_BYTES)?;
    let mut cursor = 0usize;
    take_header(input, &mut cursor, OWNER_FEE_CARRY_MAGIC_V1)?;
    let _fee_record = read_id(input, &mut cursor)?;
    let owner = read_id(input, &mut cursor)?;
    let _denominator = read_u128(input, &mut cursor)?;
    let remainder = read_u128(input, &mut cursor)?;
    let paid_atoms = read_u64(input, &mut cursor)?;
    let closed = read_bool(input, &mut cursor)?;
    require_zero(take(input, &mut cursor, 11)?)?;
    finish(cursor, input.len())?;
    let carry = OwnerFeeCarryV1::restore(selected, owner, remainder, paid_atoms, closed)?;
    if encode_owner_fee_carry_v1(&carry)?.as_slice() != input {
        return Err(Error::MismatchedBinding);
    }
    Ok(carry)
}

pub fn encode_payer_allocation_v1(
    allocation: &PayerAllocationV1,
) -> Result<[u8; PAYER_ALLOCATION_ACCOUNT_V1_BYTES]> {
    let mut output = [0u8; PAYER_ALLOCATION_ACCOUNT_V1_BYTES];
    encode_payer_allocation_v1_into(allocation, &mut output)?;
    Ok(output)
}

/// Encode one canonical payer allocation directly into exact caller-owned
/// storage.
///
/// This is the bounded-adapter form of [`encode_payer_allocation_v1`]; it
/// avoids a maximum-width return value while preserving the identical wire
/// transcript.
pub fn encode_payer_allocation_v1_into(
    allocation: &PayerAllocationV1,
    output: &mut [u8],
) -> Result<()> {
    exact_len(output, PAYER_ALLOCATION_ACCOUNT_V1_BYTES)?;
    let mut cursor = 0usize;
    put_header(output, &mut cursor, PAYER_ALLOCATION_MAGIC_V1)?;
    put(output, &mut cursor, &allocation.fee_record().0)?;
    put(output, &mut cursor, &allocation.owner().0)?;
    put(output, &mut cursor, &[allocation.len()])?;
    put(output, &mut cursor, &[allocation.boundary().byte()])?;
    put(output, &mut cursor, &[0; 2])?;
    put(
        output,
        &mut cursor,
        &allocation.total_debit_atoms().to_le_bytes(),
    )?;
    put(
        output,
        &mut cursor,
        &allocation.next_carry().to_le_bytes(),
    )?;
    put(
        output,
        &mut cursor,
        &allocation.carry_denominator().to_le_bytes(),
    )?;
    let mut index = 0usize;
    while index < MAX_FEE_ROWS_V1 {
        put(output, &mut cursor, &allocation.intents()[index].0)?;
        put(
            output,
            &mut cursor,
            &allocation.debit_atoms()[index].to_le_bytes(),
        )?;
        index += 1;
    }
    finish(cursor, output.len())
}

pub fn decode_payer_allocation_v1(
    input: &[u8],
    assessment: &OwnerFeeAssessmentV1,
    envelopes: &[FeeEnvelopeV1; MAX_FEE_ROWS_V1],
) -> Result<PayerAllocationV1> {
    exact_len(input, PAYER_ALLOCATION_ACCOUNT_V1_BYTES)?;
    let mut cursor = 0usize;
    take_header(input, &mut cursor, PAYER_ALLOCATION_MAGIC_V1)?;
    let _fee_record = read_id(input, &mut cursor)?;
    let _owner = read_id(input, &mut cursor)?;
    let len = read_u8(input, &mut cursor)?;
    let _boundary = read_u8(input, &mut cursor)?;
    require_zero(take(input, &mut cursor, 2)?)?;
    let _total = read_u64(input, &mut cursor)?;
    let _next_carry = read_u128(input, &mut cursor)?;
    let _denominator = read_u128(input, &mut cursor)?;
    let mut index = 0usize;
    while index < MAX_FEE_ROWS_V1 {
        let _intent = read_id(input, &mut cursor)?;
        let _debit = read_u64(input, &mut cursor)?;
        index += 1;
    }
    finish(cursor, input.len())?;
    let allocation = allocate_payer_debit(assessment, envelopes, len)?;
    if encode_payer_allocation_v1(&allocation)?.as_slice() != input {
        return Err(Error::MismatchedBinding);
    }
    Ok(allocation)
}

/// Structurally decode a canonical persisted payer allocation.
///
/// This proves only the self-consistency of the semantic body. It does not
/// prove account ownership, PDA identity, or that signed envelopes authorized
/// the allocation. The General adapter must establish those facts before using
/// this decoder for reauthentication.
pub fn decode_persisted_payer_allocation_v1(input: &[u8]) -> Result<PayerAllocationV1> {
    exact_len(input, PAYER_ALLOCATION_ACCOUNT_V1_BYTES)?;
    let mut cursor = 0usize;
    take_header(input, &mut cursor, PAYER_ALLOCATION_MAGIC_V1)?;
    let fee_record = read_id(input, &mut cursor)?;
    let owner = read_id(input, &mut cursor)?;
    let len = read_u8(input, &mut cursor)?;
    let boundary = match read_u8(input, &mut cursor)? {
        0 => crate::selected::AssessmentBoundaryV1::FragmentFloor,
        1 => crate::selected::AssessmentBoundaryV1::TerminalCeil,
        _ => return Err(Error::InvalidAccountData),
    };
    require_zero(take(input, &mut cursor, 2)?)?;
    let total_debit_atoms = read_u64(input, &mut cursor)?;
    let next_carry = read_u128(input, &mut cursor)?;
    let carry_denominator = read_u128(input, &mut cursor)?;
    let mut intents = [Id([0; 32]); MAX_FEE_ROWS_V1];
    let mut debit_atoms = [0u64; MAX_FEE_ROWS_V1];
    let mut index = 0usize;
    while index < MAX_FEE_ROWS_V1 {
        intents[index] = read_id(input, &mut cursor)?;
        debit_atoms[index] = read_u64(input, &mut cursor)?;
        index += 1;
    }
    finish(cursor, input.len())?;
    let allocation = PayerAllocationV1::restore_persisted(
        fee_record,
        owner,
        len,
        intents,
        debit_atoms,
        total_debit_atoms,
        next_carry,
        carry_denominator,
        boundary,
    )?;
    if encode_payer_allocation_v1(&allocation)?.as_slice() != input {
        return Err(Error::MismatchedBinding);
    }
    Ok(allocation)
}

pub fn encode_recipient_allocation_v1(
    allocation: &RecipientAllocationV1,
) -> Result<[u8; RECIPIENT_ALLOCATION_ACCOUNT_V1_BYTES]> {
    let mut output = [0u8; RECIPIENT_ALLOCATION_ACCOUNT_V1_BYTES];
    encode_recipient_allocation_v1_into(allocation, &mut output)?;
    Ok(output)
}

/// Encode the exact V1 semantic body directly into caller-owned storage.
///
/// The current V3 SBF writer uses this form so the 2,640-byte body is never a
/// second local array beside the 2,744-byte certified body.
#[inline(never)]
pub fn encode_recipient_allocation_v1_into(
    allocation: &RecipientAllocationV1,
    output: &mut [u8],
) -> Result<()> {
    exact_len(output, RECIPIENT_ALLOCATION_ACCOUNT_V1_BYTES)?;
    let mut cursor = 0usize;
    put_header(output, &mut cursor, RECIPIENT_ALLOCATION_MAGIC_V1)?;
    put(output, &mut cursor, &allocation.fee_record().0)?;
    put(output, &mut cursor, &[allocation.maker_len()])?;
    put(output, &mut cursor, &[0; 3])?;
    put(
        output,
        &mut cursor,
        &allocation.maker_rebate_total().to_le_bytes(),
    )?;
    put(
        output,
        &mut cursor,
        &allocation.executor_atoms().to_le_bytes(),
    )?;
    put(
        output,
        &mut cursor,
        &allocation.treasury_atoms().to_le_bytes(),
    )?;
    put(
        output,
        &mut cursor,
        &allocation.collected_fee_atoms().to_le_bytes(),
    )?;
    let mut index = 0usize;
    while index < MAX_FEE_ROWS_V1 {
        put(
            output,
            &mut cursor,
            &allocation.maker_positions()[index].0,
        )?;
        put(
            output,
            &mut cursor,
            &allocation.maker_rebate_atoms()[index].to_le_bytes(),
        )?;
        index += 1;
    }
    finish(cursor, output.len())?;
    Ok(())
}

pub fn decode_recipient_allocation_v1(
    input: &[u8],
    selected: &SelectedCompositeFeeV1,
    policy: &RevenuePolicyV1,
    makers: &[StandingMakerRowV1; MAX_FEE_ROWS_V1],
) -> Result<RecipientAllocationV1> {
    exact_len(input, RECIPIENT_ALLOCATION_ACCOUNT_V1_BYTES)?;
    let mut cursor = 0usize;
    take_header(input, &mut cursor, RECIPIENT_ALLOCATION_MAGIC_V1)?;
    let _fee_record = read_id(input, &mut cursor)?;
    let maker_len = read_u8(input, &mut cursor)?;
    require_zero(take(input, &mut cursor, 3)?)?;
    let _maker_total = read_u64(input, &mut cursor)?;
    let _executor = read_u64(input, &mut cursor)?;
    let _treasury = read_u64(input, &mut cursor)?;
    let collected_fee_atoms = read_u64(input, &mut cursor)?;
    let mut index = 0usize;
    while index < MAX_FEE_ROWS_V1 {
        let _position = read_id(input, &mut cursor)?;
        let _rebate = read_u64(input, &mut cursor)?;
        index += 1;
    }
    finish(cursor, input.len())?;
    let allocation = allocate_recipients(selected, policy, makers, maker_len, collected_fee_atoms)?;
    if encode_recipient_allocation_v1(&allocation)?.as_slice() != input {
        return Err(Error::MismatchedBinding);
    }
    Ok(allocation)
}

/// Structurally decode one immutable persisted recipient allocation.
///
/// This proves canonical rows and exact conservation only. It does not prove
/// maker weights, revenue-policy selection, or complete owner-fee collection;
/// those remain creation-time obligations of the certified successor outer.
pub fn decode_persisted_recipient_allocation_v1(
    input: &[u8],
) -> Result<RecipientAllocationV1> {
    exact_len(input, RECIPIENT_ALLOCATION_ACCOUNT_V1_BYTES)?;
    let mut cursor = 0usize;
    take_header(input, &mut cursor, RECIPIENT_ALLOCATION_MAGIC_V1)?;
    let fee_record = read_id(input, &mut cursor)?;
    let maker_len = read_u8(input, &mut cursor)?;
    require_zero(take(input, &mut cursor, 3)?)?;
    let maker_rebate_total = read_u64(input, &mut cursor)?;
    let executor_atoms = read_u64(input, &mut cursor)?;
    let treasury_atoms = read_u64(input, &mut cursor)?;
    let collected_fee_atoms = read_u64(input, &mut cursor)?;
    let mut maker_positions = [Id([0; 32]); MAX_FEE_ROWS_V1];
    let mut maker_rebate_atoms = [0u64; MAX_FEE_ROWS_V1];
    let mut index = 0usize;
    while index < MAX_FEE_ROWS_V1 {
        maker_positions[index] = read_id(input, &mut cursor)?;
        maker_rebate_atoms[index] = read_u64(input, &mut cursor)?;
        index += 1;
    }
    finish(cursor, input.len())?;
    let allocation = RecipientAllocationV1::restore_persisted(
        fee_record,
        maker_len,
        maker_positions,
        maker_rebate_atoms,
        maker_rebate_total,
        executor_atoms,
        treasury_atoms,
        collected_fee_atoms,
    )?;
    if encode_recipient_allocation_v1(&allocation)?.as_slice() != input {
        return Err(Error::MismatchedBinding);
    }
    Ok(allocation)
}

/// Encode the exact recipient allocation plus complete fee-book certificate.
pub fn encode_certified_recipient_allocation_v2(
    certified: &CertifiedRecipientAllocationV2,
) -> Result<[u8; CERTIFIED_RECIPIENT_ALLOCATION_V2_BYTES]> {
    let mut output = [0u8; CERTIFIED_RECIPIENT_ALLOCATION_V2_BYTES];
    let semantic = encode_recipient_allocation_v1(&certified.allocation())?;
    output[..RECIPIENT_ALLOCATION_ACCOUNT_V1_BYTES].copy_from_slice(&semantic);
    let mut cursor = RECIPIENT_ALLOCATION_ACCOUNT_V1_BYTES;
    put(
        &mut output,
        &mut cursor,
        &certified.owner_fee_book_data_id().0,
    )?;
    put(
        &mut output,
        &mut cursor,
        &certified.owner_order_set_digest().0,
    )?;
    put(
        &mut output,
        &mut cursor,
        &certified.owner_count().to_le_bytes(),
    )?;
    put(&mut output, &mut cursor, &[0; 6])?;
    finish(cursor, output.len())?;
    Ok(output)
}

/// Structurally decode the immutable certified recipient snapshot.
///
/// Program ownership, canonical PDA identity, and the creation route's full
/// book/traversal authorization remain mandatory adapter checks.
pub fn decode_persisted_certified_recipient_allocation_v2(
    input: &[u8],
) -> Result<CertifiedRecipientAllocationV2> {
    exact_len(input, CERTIFIED_RECIPIENT_ALLOCATION_V2_BYTES)?;
    let allocation = decode_persisted_recipient_allocation_v1(
        &input[..RECIPIENT_ALLOCATION_ACCOUNT_V1_BYTES],
    )?;
    let mut cursor = RECIPIENT_ALLOCATION_ACCOUNT_V1_BYTES;
    let owner_fee_book_data_id = read_id(input, &mut cursor)?;
    let owner_order_set_digest = read_id(input, &mut cursor)?;
    let owner_count = read_u16(input, &mut cursor)?;
    require_zero(take(input, &mut cursor, 6)?)?;
    finish(cursor, input.len())?;
    let value = CertifiedRecipientAllocationV2::restore_persisted(
        allocation,
        owner_fee_book_data_id,
        owner_order_set_digest,
        owner_count,
    )?;
    if encode_certified_recipient_allocation_v2(&value)?.as_slice() != input {
        return Err(Error::MismatchedBinding);
    }
    Ok(value)
}

/// Encode V3 semantics directly into exact caller-owned body storage.
#[inline(never)]
pub fn encode_certified_recipient_allocation_v3_into(
    certified: &CertifiedRecipientAllocationV3,
    output: &mut [u8],
) -> Result<()> {
    encode_certified_recipient_allocation_v3_from_access_into(certified, output)
}

/// Stream the exact current V3 body from an already-authorized projection.
///
/// This function rechecks the current policy, live provenance, canonical
/// Position order, zero tail, cardinalities, and conservation while writing
/// directly into caller-owned account storage. It does not authenticate the
/// source of the projection; the live writer must keep its implementation
/// private to the traversal-backed fee-weight/Hamilton capability.
#[inline(never)]
pub fn encode_certified_recipient_allocation_v3_from_access_into<A>(
    certified: &A,
    output: &mut [u8],
) -> Result<()>
where
    A: CertifiedRecipientAllocationAccessV3 + ?Sized,
{
    exact_len(output, CERTIFIED_RECIPIENT_ALLOCATION_V3_BYTES)?;
    let fee_record = certified.fee_record();
    let row_count = certified.row_count();
    let weight_policy_id = certified.weight_policy_id();
    let weight_transcript_id = certified.weight_transcript_id();
    let owner_order_set_digest = certified.owner_order_set_digest();
    let traversed_owner_count = certified.traversed_owner_count();
    let nonzero_weight_row_count = certified.nonzero_weight_row_count();
    live(fee_record)?;
    live(weight_policy_id)?;
    live(weight_transcript_id)?;
    live(owner_order_set_digest)?;
    if weight_policy_id != crate::weight_v2::COMPOSITE_FEE_WEIGHT_POLICY_V2.id()?
        || usize::from(row_count) > MAX_FEE_ROWS_V1
        || traversed_owner_count == 0
        || usize::from(traversed_owner_count) > MAX_FEE_ROWS_V1
        || nonzero_weight_row_count != row_count
        || u16::from(nonzero_weight_row_count) > traversed_owner_count
        || (row_count == 0) != (certified.collected_fee_atoms() == 0)
    {
        return Err(Error::InvalidAccountData);
    }

    let mut cursor = 0usize;
    put_header(output, &mut cursor, RECIPIENT_ALLOCATION_MAGIC_V1)?;
    put(output, &mut cursor, &fee_record.0)?;
    put(output, &mut cursor, &[row_count])?;
    put(output, &mut cursor, &[0; 3])?;
    put(output, &mut cursor, &certified.maker_rebate_total().to_le_bytes())?;
    put(output, &mut cursor, &certified.executor_atoms().to_le_bytes())?;
    put(output, &mut cursor, &certified.treasury_atoms().to_le_bytes())?;
    put(output, &mut cursor, &certified.collected_fee_atoms().to_le_bytes())?;
    let mut maker_sum = 0u64;
    let mut prior = None;
    let mut index = 0usize;
    while index < MAX_FEE_ROWS_V1 {
        let row_index = u8::try_from(index).map_err(|_| Error::InvalidWidth)?;
        if index < usize::from(row_count) {
            let row = certified.row(row_index)?.ok_or(Error::MissingParticipant)?;
            live(row.position())?;
            if prior.is_some_and(|value| row.position() <= value) {
                return Err(Error::NonCanonicalOrder);
            }
            maker_sum = add(maker_sum, row.rebate_atoms())?;
            prior = Some(row.position());
            put(output, &mut cursor, &row.position().0)?;
            put(output, &mut cursor, &row.rebate_atoms().to_le_bytes())?;
        } else {
            if certified.row(row_index)?.is_some() {
                return Err(Error::NonCanonicalPadding);
            }
            put(output, &mut cursor, &[0; RECIPIENT_ALLOCATION_V1_ROW_BYTES])?;
        }
        index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
    }
    finish(cursor, RECIPIENT_ALLOCATION_ACCOUNT_V1_BYTES)?;
    if maker_sum != certified.maker_rebate_total()
        || add(
            add(maker_sum, certified.executor_atoms())?,
            certified.treasury_atoms(),
        )? != certified.collected_fee_atoms()
    {
        return Err(Error::ConservationFailure);
    }

    let mut cursor = RECIPIENT_ALLOCATION_ACCOUNT_V1_BYTES;
    put(output, &mut cursor, &weight_policy_id.0)?;
    put(output, &mut cursor, &weight_transcript_id.0)?;
    put(output, &mut cursor, &owner_order_set_digest.0)?;
    put(
        output,
        &mut cursor,
        &traversed_owner_count.to_le_bytes(),
    )?;
    put(output, &mut cursor, &[nonzero_weight_row_count])?;
    put(output, &mut cursor, &[0; 5])?;
    finish(cursor, output.len())?;
    Ok(())
}

/// Strictly authenticate current V3 semantics without copying either
/// maximum-width allocation array.
///
/// This is the canonical live adapter decoder. It parses every byte, requires
/// strict Position ordering and zero padding, checks all allocation totals,
/// fixes the current V2 weight-policy identity, and returns only a borrowed row
/// accessor plus compact provenance.
#[inline(never)]
pub fn decode_borrowed_certified_recipient_allocation_v3(
    input: &[u8],
) -> Result<BorrowedCertifiedRecipientAllocationV3<'_>> {
    exact_len(input, CERTIFIED_RECIPIENT_ALLOCATION_V3_BYTES)?;
    let allocation_bytes = &input[..RECIPIENT_ALLOCATION_ACCOUNT_V1_BYTES];
    let mut cursor = 0usize;
    take_header(
        allocation_bytes,
        &mut cursor,
        RECIPIENT_ALLOCATION_MAGIC_V1,
    )?;
    let fee_record = read_id(allocation_bytes, &mut cursor)?;
    live(fee_record)?;
    let maker_len = read_u8(allocation_bytes, &mut cursor)?;
    if usize::from(maker_len) > MAX_FEE_ROWS_V1 {
        return Err(Error::InvalidWidth);
    }
    require_zero(take(allocation_bytes, &mut cursor, 3)?)?;
    let maker_rebate_total = read_u64(allocation_bytes, &mut cursor)?;
    let executor_atoms = read_u64(allocation_bytes, &mut cursor)?;
    let treasury_atoms = read_u64(allocation_bytes, &mut cursor)?;
    let collected_fee_atoms = read_u64(allocation_bytes, &mut cursor)?;
    let mut maker_sum = 0u64;
    let mut prior = None;
    let mut row_index = 0usize;
    while row_index < MAX_FEE_ROWS_V1 {
        let position = read_id(allocation_bytes, &mut cursor)?;
        let rebate_atoms = read_u64(allocation_bytes, &mut cursor)?;
        if row_index < usize::from(maker_len) {
            live(position)?;
            if prior.is_some_and(|value| position <= value) {
                return Err(Error::NonCanonicalOrder);
            }
            maker_sum = add(maker_sum, rebate_atoms)?;
            prior = Some(position);
        } else if !position.is_zero() || rebate_atoms != 0 {
            return Err(Error::NonCanonicalPadding);
        }
        row_index += 1;
    }
    finish(cursor, allocation_bytes.len())?;
    if maker_sum != maker_rebate_total
        || add(add(maker_sum, executor_atoms)?, treasury_atoms)? != collected_fee_atoms
    {
        return Err(Error::ConservationFailure);
    }

    cursor = RECIPIENT_ALLOCATION_ACCOUNT_V1_BYTES;
    let weight_policy_id = read_id(input, &mut cursor)?;
    let weight_transcript_id = read_id(input, &mut cursor)?;
    let owner_order_set_digest = read_id(input, &mut cursor)?;
    let traversed_owner_count = read_u16(input, &mut cursor)?;
    let nonzero_weight_row_count = read_u8(input, &mut cursor)?;
    require_zero(take(input, &mut cursor, 5)?)?;
    finish(cursor, input.len())?;
    live(weight_policy_id)?;
    live(weight_transcript_id)?;
    live(owner_order_set_digest)?;
    if weight_policy_id != crate::weight_v2::COMPOSITE_FEE_WEIGHT_POLICY_V2.id()?
        || traversed_owner_count == 0
        || usize::from(traversed_owner_count) > MAX_FEE_ROWS_V1
        || nonzero_weight_row_count != maker_len
        || u16::from(nonzero_weight_row_count) > traversed_owner_count
        || (nonzero_weight_row_count == 0) != (collected_fee_atoms == 0)
    {
        return Err(Error::InvalidAccountData);
    }
    Ok(BorrowedCertifiedRecipientAllocationV3 {
        allocation: BorrowedRecipientAllocationV1 {
            bytes: allocation_bytes,
            fee_record,
            maker_len,
            maker_rebate_total,
            executor_atoms,
            treasury_atoms,
            collected_fee_atoms,
        },
        weight_policy_id,
        weight_transcript_id,
        owner_order_set_digest,
        traversed_owner_count,
        nonzero_weight_row_count,
    })
}

const _: () = assert!(CERTIFIED_RECIPIENT_ALLOCATION_V3_BYTES == 2_744);
const _: () = assert!(RECIPIENT_ALLOCATION_V1_ROWS_OFFSET == 80);
const _: () = assert!(
    RECIPIENT_ALLOCATION_V1_ROWS_OFFSET
        + MAX_FEE_ROWS_V1 * RECIPIENT_ALLOCATION_V1_ROW_BYTES
        == RECIPIENT_ALLOCATION_ACCOUNT_V1_BYTES
);

pub fn encode_treasury_ledger_v1(
    ledger: &TreasuryLedgerV1,
) -> Result<[u8; TREASURY_LEDGER_ACCOUNT_V1_BYTES]> {
    let mut output = [0u8; TREASURY_LEDGER_ACCOUNT_V1_BYTES];
    let mut cursor = 0usize;
    put_header(&mut output, &mut cursor, TREASURY_LEDGER_MAGIC_V1)?;
    put(&mut output, &mut cursor, &ledger.fee_record().0)?;
    put(&mut output, &mut cursor, &ledger.treasury_owner().0)?;
    put(&mut output, &mut cursor, &ledger.treasury_position().0)?;
    put(
        &mut output,
        &mut cursor,
        &ledger.credited_atoms().to_le_bytes(),
    )?;
    put(
        &mut output,
        &mut cursor,
        &ledger.withdrawn_atoms().to_le_bytes(),
    )?;
    put(
        &mut output,
        &mut cursor,
        &ledger.available_atoms().to_le_bytes(),
    )?;
    put(
        &mut output,
        &mut cursor,
        &ledger.outstanding_epochs().to_le_bytes(),
    )?;
    put(&mut output, &mut cursor, &[u8::from(ledger.is_closed())])?;
    put(&mut output, &mut cursor, &[0; 3])?;
    finish(cursor, output.len())?;
    Ok(output)
}

pub fn decode_treasury_ledger_v1<S: SelectedCompositeFeeAccess + ?Sized>(
    input: &[u8],
    selected: &S,
) -> Result<TreasuryLedgerV1> {
    exact_len(input, TREASURY_LEDGER_ACCOUNT_V1_BYTES)?;
    let mut cursor = 0usize;
    take_header(input, &mut cursor, TREASURY_LEDGER_MAGIC_V1)?;
    let _fee_record = read_id(input, &mut cursor)?;
    let _treasury_owner = read_id(input, &mut cursor)?;
    let _treasury_position = read_id(input, &mut cursor)?;
    let credited_atoms = read_u64(input, &mut cursor)?;
    let withdrawn_atoms = read_u64(input, &mut cursor)?;
    let available_atoms = read_u64(input, &mut cursor)?;
    let outstanding_epochs = read_u64(input, &mut cursor)?;
    let closed = read_bool(input, &mut cursor)?;
    require_zero(take(input, &mut cursor, 3)?)?;
    finish(cursor, input.len())?;
    let ledger = TreasuryLedgerV1::restore(
        selected,
        credited_atoms,
        withdrawn_atoms,
        available_atoms,
        outstanding_epochs,
        closed,
    )?;
    if encode_treasury_ledger_v1(&ledger)?.as_slice() != input {
        return Err(Error::MismatchedBinding);
    }
    Ok(ledger)
}

fn exact_len(input: &[u8], expected: usize) -> Result<()> {
    if input.len() == expected {
        Ok(())
    } else {
        Err(Error::InvalidAccountData)
    }
}

fn put_header(
    output: &mut [u8],
    cursor: &mut usize,
    magic: [u8; 8],
) -> Result<()> {
    put_header_version(output, cursor, magic, CODEC_VERSION_V1)
}

fn put_header_version(
    output: &mut [u8],
    cursor: &mut usize,
    magic: [u8; 8],
    version: u16,
) -> Result<()> {
    put(output, cursor, &magic)?;
    put(output, cursor, &version.to_le_bytes())?;
    put(output, cursor, &CODEC_FLAGS_V1.to_le_bytes())
}

fn take_header(input: &[u8], cursor: &mut usize, magic: [u8; 8]) -> Result<()> {
    take_header_version(input, cursor, magic, CODEC_VERSION_V1)
}

fn take_header_version(
    input: &[u8],
    cursor: &mut usize,
    magic: [u8; 8],
    version: u16,
) -> Result<()> {
    if take(input, cursor, 8)? != magic.as_slice() {
        return Err(Error::WrongAccountKind);
    }
    if read_u16(input, cursor)? != version {
        return Err(Error::WrongVersion);
    }
    if read_u16(input, cursor)? != CODEC_FLAGS_V1 {
        return Err(Error::NonCanonicalPadding);
    }
    Ok(())
}

fn require_zero(input: &[u8]) -> Result<()> {
    let mut index = 0usize;
    while index < input.len() {
        if input[index] != 0 {
            return Err(Error::NonCanonicalPadding);
        }
        index += 1;
    }
    Ok(())
}

fn finish(cursor: usize, len: usize) -> Result<()> {
    if cursor == len {
        Ok(())
    } else {
        Err(Error::InvalidAccountData)
    }
}

fn put(output: &mut [u8], cursor: &mut usize, bytes: &[u8]) -> Result<()> {
    let end = cursor
        .checked_add(bytes.len())
        .ok_or(Error::ArithmeticOverflow)?;
    output
        .get_mut(*cursor..end)
        .ok_or(Error::InvalidAccountData)?
        .copy_from_slice(bytes);
    *cursor = end;
    Ok(())
}

fn take<'a>(input: &'a [u8], cursor: &mut usize, width: usize) -> Result<&'a [u8]> {
    let end = cursor.checked_add(width).ok_or(Error::ArithmeticOverflow)?;
    let value = input.get(*cursor..end).ok_or(Error::InvalidAccountData)?;
    *cursor = end;
    Ok(value)
}

fn read_id(input: &[u8], cursor: &mut usize) -> Result<Id> {
    let mut value = [0u8; 32];
    value.copy_from_slice(take(input, cursor, 32)?);
    Ok(Id(value))
}

fn read_bool(input: &[u8], cursor: &mut usize) -> Result<bool> {
    match read_u8(input, cursor)? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(Error::InvalidAccountData),
    }
}

fn read_u8(input: &[u8], cursor: &mut usize) -> Result<u8> {
    Ok(take(input, cursor, 1)?[0])
}

fn read_u16(input: &[u8], cursor: &mut usize) -> Result<u16> {
    let mut value = [0u8; 2];
    value.copy_from_slice(take(input, cursor, 2)?);
    Ok(u16::from_le_bytes(value))
}

fn read_u32(input: &[u8], cursor: &mut usize) -> Result<u32> {
    let mut value = [0u8; 4];
    value.copy_from_slice(take(input, cursor, 4)?);
    Ok(u32::from_le_bytes(value))
}

fn read_u64(input: &[u8], cursor: &mut usize) -> Result<u64> {
    let mut value = [0u8; 8];
    value.copy_from_slice(take(input, cursor, 8)?);
    Ok(u64::from_le_bytes(value))
}

fn read_u128(input: &[u8], cursor: &mut usize) -> Result<u128> {
    let mut value = [0u8; 16];
    value.copy_from_slice(take(input, cursor, 16)?);
    Ok(u128::from_le_bytes(value))
}
