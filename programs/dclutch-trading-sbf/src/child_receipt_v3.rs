//! Family-neutral prior-child receipt retention for common Hot V3 execution.

extern crate alloc;

use alloc::vec::Vec;

use dclutch_effect_kernel::{
    v2::FixedRole,
    v3::{ResolvedInvocationV3, ResolvedReceiptDependencyV3},
};
use solana_program::{program::MAX_RETURN_DATA, program_error::ProgramError, pubkey::Pubkey};

use crate::TradingSbfError;

/// One exact immediate child return retained only for this top-level execution.
struct ExecutedReceiptV3 {
    role: FixedRole,
    route: u16,
    invocation: u32,
    program: Pubkey,
    bytes: Vec<u8>,
}

/// Ordered ephemeral receipt bank. It is never persisted and grants no authority.
pub(crate) struct ChildReceiptBankV3 {
    receipts: Vec<ExecutedReceiptV3>,
}

impl ChildReceiptBankV3 {
    pub(crate) const fn new() -> Self {
        Self {
            receipts: Vec::new(),
        }
    }

    pub(crate) fn record(
        &mut self,
        role: FixedRole,
        route: u16,
        invocation: u32,
        program: Pubkey,
        bytes: Vec<u8>,
    ) -> Result<(), ProgramError> {
        if bytes.is_empty()
            || self
                .receipts
                .iter()
                .any(|receipt| receipt.route == route && receipt.invocation == invocation)
        {
            return Err(TradingSbfError::Transition.into());
        }
        self.receipts.push(ExecutedReceiptV3 {
            role,
            route,
            invocation,
            program,
            bytes,
        });
        Ok(())
    }

    pub(crate) fn resolve(
        &self,
        dependency: Option<ResolvedReceiptDependencyV3>,
        expected_program: Option<&Pubkey>,
    ) -> Result<Option<&[u8]>, ProgramError> {
        let Some(dependency) = dependency else {
            if expected_program.is_some() {
                return Err(TradingSbfError::Content.into());
            }
            return Ok(None);
        };
        let expected_program = expected_program.ok_or(TradingSbfError::Content)?;
        let mut matching = self.receipts.iter().filter(|receipt| {
            receipt.role == dependency.producer_role
                && receipt.route == dependency.producer_route
                && receipt.invocation == dependency.producer_invocation
                && receipt.program == *expected_program
        });
        let receipt = matching.next().ok_or(TradingSbfError::Transition)?;
        if matching.next().is_some()
            || receipt.bytes.len() != usize::from(dependency.expected_receipt_bytes)
        {
            return Err(TradingSbfError::Transition.into());
        }
        Ok(Some(&receipt.bytes))
    }
}

/// Append the exact descriptor-selected receipt or refuse any caller-supplied suffix.
pub(crate) fn append_receipt_dependency_v3(
    invocation: ResolvedInvocationV3,
    child_data: &mut Vec<u8>,
    receipt: Option<&[u8]>,
) -> Result<(), ProgramError> {
    match (invocation.receipt_dependency, receipt) {
        (None, None) => Ok(()),
        (Some(dependency), Some(receipt))
            if receipt.len() == usize::from(dependency.expected_receipt_bytes) =>
        {
            child_data
                .try_reserve(receipt.len())
                .map_err(|_| TradingSbfError::Content)?;
            child_data.extend_from_slice(receipt);
            Ok(())
        }
        _ => Err(TradingSbfError::Content.into()),
    }
}

/// Refuse metadata that cannot be produced by the chain return-data syscall.
pub(crate) fn require_chain_receipt_width_v3(
    invocation: ResolvedInvocationV3,
) -> Result<(), ProgramError> {
    if invocation
        .receipt_dependency
        .is_some_and(|dependency| usize::from(dependency.expected_receipt_bytes) > MAX_RETURN_DATA)
    {
        Err(TradingSbfError::Content.into())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use dclutch_effect_kernel::v3::RouteKindV3;

    fn invocation(dependency: Option<ResolvedReceiptDependencyV3>) -> ResolvedInvocationV3 {
        ResolvedInvocationV3 {
            role: FixedRole::Core,
            kind: RouteKindV3::Once,
            item: None,
            fixed_account_start: 0,
            fixed_account_count: 0,
            item_account_start: 0,
            item_account_count: 0,
            item_account_stride: 0,
            repeated_item_count: 0,
            request_offset: 0,
            request_len: 8,
            borrowed_witness: None,
            receipt_dependency: dependency,
        }
    }

    fn dependency() -> ResolvedReceiptDependencyV3 {
        ResolvedReceiptDependencyV3 {
            producer_role: FixedRole::Custody,
            producer_route: 2,
            producer_invocation: 3,
            expected_receipt_bytes: 4,
        }
    }

    #[test]
    fn bank_binds_role_route_invocation_program_and_exact_width() {
        let program = Pubkey::new_unique();
        let mut bank = ChildReceiptBankV3::new();
        bank.record(FixedRole::Custody, 2, 3, program, vec![1, 2, 3, 4])
            .expect("record");
        assert_eq!(
            bank.resolve(Some(dependency()), Some(&program)),
            Ok(Some([1_u8, 2, 3, 4].as_slice()))
        );
        assert_eq!(
            bank.resolve(Some(dependency()), Some(&Pubkey::new_unique())),
            Err(TradingSbfError::Transition.into())
        );
        assert_eq!(
            bank.record(FixedRole::Claims, 2, 3, program, vec![0]),
            Err(TradingSbfError::Transition.into())
        );
    }

    #[test]
    fn append_is_exact_and_absence_cannot_smuggle_a_suffix() {
        let mut bytes = vec![9_u8; 8];
        append_receipt_dependency_v3(
            invocation(Some(dependency())),
            &mut bytes,
            Some(&[1, 2, 3, 4]),
        )
        .expect("append");
        assert_eq!(
            bytes,
            [9_u8; 8]
                .into_iter()
                .chain([1, 2, 3, 4])
                .collect::<Vec<_>>()
        );

        let mut hostile = vec![9_u8; 8];
        assert_eq!(
            append_receipt_dependency_v3(invocation(None), &mut hostile, Some(&[1])),
            Err(TradingSbfError::Content.into())
        );
        assert_eq!(hostile, vec![9_u8; 8]);

        let oversized = ResolvedReceiptDependencyV3 {
            expected_receipt_bytes: u16::try_from(MAX_RETURN_DATA + 1).expect("u16 width"),
            ..dependency()
        };
        assert_eq!(
            require_chain_receipt_width_v3(invocation(Some(oversized))),
            Err(TradingSbfError::Content.into())
        );
    }
}
