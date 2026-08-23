//! Fixed-layout semantic account bodies for a future Solana adapter.
//!
//! These codecs allocate no outer program tag or PDA seed. Decode always
//! reconstructs through the semantic constructor and compares the canonical
//! re-encoding, so persisted derived words never become a second authority.

use clutch_batch::relation_v1::FrozenPolicyV1;
use clutch_batch_policy_identity::revenue_policy_v1::RevenuePolicyV1;

use crate::allocation::{
    allocate_payer_debit, allocate_recipients, FeeEnvelopeV1, PayerAllocationV1,
    RecipientAllocationV1, StandingMakerRowV1,
};
use crate::selected::{OwnerFeeAssessmentV1, OwnerFeeCarryV1, SelectedCompositeFeeV1};
use crate::treasury::TreasuryLedgerV1;
use crate::{Error, Id, Result, MAX_FEE_ROWS_V1};

pub const FEE_RECORD_ACCOUNT_V1_BYTES: usize = 336;
pub const OWNER_FEE_CARRY_ACCOUNT_V1_BYTES: usize = 128;
pub const PAYER_ALLOCATION_ACCOUNT_V1_BYTES: usize = 2_680;
pub const RECIPIENT_ALLOCATION_ACCOUNT_V1_BYTES: usize = 2_640;
pub const TREASURY_LEDGER_ACCOUNT_V1_BYTES: usize = 144;

pub const FEE_RECORD_MAGIC_V1: [u8; 8] = *b"DCFEESEL";
pub const OWNER_FEE_CARRY_MAGIC_V1: [u8; 8] = *b"DCFEECRY";
pub const PAYER_ALLOCATION_MAGIC_V1: [u8; 8] = *b"DCFEEPAY";
pub const RECIPIENT_ALLOCATION_MAGIC_V1: [u8; 8] = *b"DCFEEREC";
pub const TREASURY_LEDGER_MAGIC_V1: [u8; 8] = *b"DCFEETRY";

const CODEC_VERSION_V1: u16 = 1;
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

pub fn decode_owner_fee_carry_v1(
    input: &[u8],
    selected: &SelectedCompositeFeeV1,
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
    let mut cursor = 0usize;
    put_header(&mut output, &mut cursor, PAYER_ALLOCATION_MAGIC_V1)?;
    put(&mut output, &mut cursor, &allocation.fee_record().0)?;
    put(&mut output, &mut cursor, &allocation.owner().0)?;
    put(&mut output, &mut cursor, &[allocation.len()])?;
    put(&mut output, &mut cursor, &[allocation.boundary() as u8])?;
    put(&mut output, &mut cursor, &[0; 2])?;
    put(
        &mut output,
        &mut cursor,
        &allocation.total_debit_atoms().to_le_bytes(),
    )?;
    put(
        &mut output,
        &mut cursor,
        &allocation.next_carry().to_le_bytes(),
    )?;
    put(
        &mut output,
        &mut cursor,
        &allocation.carry_denominator().to_le_bytes(),
    )?;
    let mut index = 0usize;
    while index < MAX_FEE_ROWS_V1 {
        put(&mut output, &mut cursor, &allocation.intents()[index].0)?;
        put(
            &mut output,
            &mut cursor,
            &allocation.debit_atoms()[index].to_le_bytes(),
        )?;
        index += 1;
    }
    finish(cursor, output.len())?;
    Ok(output)
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
    let mut cursor = 0usize;
    put_header(&mut output, &mut cursor, RECIPIENT_ALLOCATION_MAGIC_V1)?;
    put(&mut output, &mut cursor, &allocation.fee_record().0)?;
    put(&mut output, &mut cursor, &[allocation.maker_len()])?;
    put(&mut output, &mut cursor, &[0; 3])?;
    put(
        &mut output,
        &mut cursor,
        &allocation.maker_rebate_total().to_le_bytes(),
    )?;
    put(
        &mut output,
        &mut cursor,
        &allocation.executor_atoms().to_le_bytes(),
    )?;
    put(
        &mut output,
        &mut cursor,
        &allocation.treasury_atoms().to_le_bytes(),
    )?;
    put(
        &mut output,
        &mut cursor,
        &allocation.collected_fee_atoms().to_le_bytes(),
    )?;
    let mut index = 0usize;
    while index < MAX_FEE_ROWS_V1 {
        put(
            &mut output,
            &mut cursor,
            &allocation.maker_positions()[index].0,
        )?;
        put(
            &mut output,
            &mut cursor,
            &allocation.maker_rebate_atoms()[index].to_le_bytes(),
        )?;
        index += 1;
    }
    finish(cursor, output.len())?;
    Ok(output)
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

pub fn decode_treasury_ledger_v1(
    input: &[u8],
    selected: &SelectedCompositeFeeV1,
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

fn put_header<const N: usize>(
    output: &mut [u8; N],
    cursor: &mut usize,
    magic: [u8; 8],
) -> Result<()> {
    put(output, cursor, &magic)?;
    put(output, cursor, &CODEC_VERSION_V1.to_le_bytes())?;
    put(output, cursor, &CODEC_FLAGS_V1.to_le_bytes())
}

fn take_header(input: &[u8], cursor: &mut usize, magic: [u8; 8]) -> Result<()> {
    if take(input, cursor, 8)? != magic.as_slice() {
        return Err(Error::WrongAccountKind);
    }
    if read_u16(input, cursor)? != CODEC_VERSION_V1 {
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

fn put<const N: usize>(output: &mut [u8; N], cursor: &mut usize, bytes: &[u8]) -> Result<()> {
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
