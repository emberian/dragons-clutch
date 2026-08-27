//! Checked arithmetic for one successful internal-claim transfer.
//!
//! This module is deliberately self-contained.  The production kernel calls
//! this exact function, while `verus/kernel/run_transfer_refinement.sh`
//! injects a proof contract at the named anchor and asks pinned Verus to check
//! the otherwise byte-identical source.  The contract covers this arithmetic
//! subset only; phase, shape, ownership, account, and runtime checks remain
//! outside its claim.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum TransferArithmeticError {
    Overflow,
    Underflow,
    Conservation,
}

pub(crate) type TransferArithmeticResult<T> = core::result::Result<T, TransferArithmeticError>;

pub(crate) fn prepare_internal_transfer(
    from: u64,
    to: u64,
    quantity: u64,
) -> TransferArithmeticResult<(u64, u64)>
// VERUS-CONTRACT-ANCHOR: run_transfer_refinement.sh inserts only the contract here.
{
    let new_from = from
        .checked_sub(quantity)
        .ok_or(TransferArithmeticError::Underflow)?;
    let new_to = to
        .checked_add(quantity)
        .ok_or(TransferArithmeticError::Overflow)?;
    // The two deltas must be equal and opposite.  The sum is taken in u128
    // so the check stays exact even when the two balances jointly exceed the
    // u64 range.
    let before = u128::from(from)
        .checked_add(u128::from(to))
        .ok_or(TransferArithmeticError::Overflow)?;
    let after = u128::from(new_from)
        .checked_add(u128::from(new_to))
        .ok_or(TransferArithmeticError::Overflow)?;
    if before != after {
        return Err(TransferArithmeticError::Conservation);
    }
    Ok((new_from, new_to))
}
