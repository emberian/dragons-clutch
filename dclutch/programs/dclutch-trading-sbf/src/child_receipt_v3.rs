//! Family-neutral prior-child receipt retention for common Hot V3 execution.

extern crate alloc;

use alloc::vec::Vec;

use dclutch_effect_kernel::{
    v2::FixedRole,
    v3::{ProgramV3, ResolvedInvocationV3, ResolvedReceiptDependencyV3},
};
use solana_program::{program::MAX_RETURN_DATA, program_error::ProgramError, pubkey::Pubkey};

use crate::TradingSbfError;

/// One exact immediate child return retained only for this top-level execution.
struct ExecutedReceiptV3 {
    role: FixedRole,
    route: u16,
    invocation: u32,
    program: Pubkey,
    context_digest: [u8; 32],
    request_kind: [u8; 8],
    request_digest: [u8; 32],
    receipt_kind: [u8; 8],
    bytes: Vec<u8>,
}

/// Exact producer-side provenance recomputed from the authenticated Effect
/// program and request bank when a later route resolves a dependency.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ExpectedReceiptProvenanceV4 {
    pub(crate) context_digest: [u8; 32],
    pub(crate) request_kind: [u8; 8],
    pub(crate) request_digest: [u8; 32],
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

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn record_exact(
        &mut self,
        role: FixedRole,
        route: u16,
        invocation: u32,
        program: Pubkey,
        context_digest: [u8; 32],
        request_kind: [u8; 8],
        request_digest: [u8; 32],
        receipt_kind: [u8; 8],
        bytes: Vec<u8>,
    ) -> Result<(), ProgramError> {
        if bytes.is_empty()
            || context_digest == [0; 32]
            || request_kind == [0; 8]
            || request_digest == [0; 32]
            || receipt_kind == [0; 8]
            || bytes.get(..8) != Some(receipt_kind.as_slice())
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
            context_digest,
            request_kind,
            request_digest,
            receipt_kind,
            bytes,
        });
        Ok(())
    }

    pub(crate) fn resolve(
        &self,
        dependency: Option<ResolvedReceiptDependencyV3>,
        expected_program: Option<&Pubkey>,
        expected_provenance: Option<ExpectedReceiptProvenanceV4>,
    ) -> Result<Option<&[u8]>, ProgramError> {
        let Some(dependency) = dependency else {
            if expected_program.is_some() || expected_provenance.is_some() {
                return Err(TradingSbfError::Content.into());
            }
            return Ok(None);
        };
        let expected_program = expected_program.ok_or(TradingSbfError::Content)?;
        let expected_provenance = expected_provenance.ok_or(TradingSbfError::Content)?;
        let mut matching = self.receipts.iter().filter(|receipt| {
            receipt.role == dependency.producer_role
                && receipt.route == dependency.producer_route
                && receipt.invocation == dependency.producer_invocation
                && receipt.program == *expected_program
        });
        let receipt = matching.next().ok_or(TradingSbfError::Transition)?;
        if matching.next().is_some()
            || receipt.bytes.len() != usize::from(dependency.expected_receipt_bytes)
            || receipt.context_digest != expected_provenance.context_digest
            || receipt.request_kind != expected_provenance.request_kind
            || receipt.request_digest != expected_provenance.request_digest
            || receipt.receipt_kind == [0; 8]
            || receipt.bytes.get(..8) != Some(receipt.receipt_kind.as_slice())
        {
            return Err(TradingSbfError::Transition.into());
        }
        Ok(Some(&receipt.bytes))
    }
}

/// Append the exact descriptor-selected ordered receipt suffix or refuse any
/// caller-supplied bytes. Resolution has already selected every boundary in
/// declared table order; this final check binds their exact aggregate width.
pub(crate) fn append_receipt_dependency_v3(
    invocation: ResolvedInvocationV3,
    child_data: &mut Vec<u8>,
    receipt: Option<&[u8]>,
) -> Result<(), ProgramError> {
    let dependencies = invocation.receipt_dependencies;
    let expected = if dependencies.is_empty() {
        invocation.receipt_dependency.map_or(0_u32, |dependency| {
            u32::from(dependency.expected_receipt_bytes)
        })
    } else {
        dependencies.expected_receipt_bytes()
    };
    match (expected == 0, receipt) {
        (true, None) => Ok(()),
        (false, Some(receipt)) if usize::try_from(expected) == Ok(receipt.len()) => {
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
    effect: ProgramV3<'_>,
    invocation: ResolvedInvocationV3,
) -> Result<(), ProgramError> {
    let mut index = 0_u16;
    while index < invocation.receipt_dependencies.len() {
        let dependency = effect
            .resolved_receipt_dependency(invocation.receipt_dependencies, index)
            .map_err(|_| TradingSbfError::Content)?;
        require_one_chain_receipt_width_v3(dependency)?;
        index = index.checked_add(1).ok_or(TradingSbfError::Content)?;
    }
    Ok(())
}

fn require_one_chain_receipt_width_v3(
    dependency: ResolvedReceiptDependencyV3,
) -> Result<(), ProgramError> {
    if usize::from(dependency.expected_receipt_bytes) > MAX_RETURN_DATA {
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
            receipt_dependencies: dclutch_effect_kernel::v3::ResolvedReceiptDependenciesV3::empty(),
            receipt_dependency: dependency,
        }
    }

    fn dependency() -> ResolvedReceiptDependencyV3 {
        ResolvedReceiptDependencyV3 {
            producer_role: FixedRole::Custody,
            producer_route: 2,
            producer_invocation: 3,
            expected_receipt_bytes: 8,
        }
    }

    #[test]
    fn bank_binds_role_route_invocation_program_and_exact_width() {
        let program = Pubkey::new_unique();
        let mut bank = ChildReceiptBankV3::new();
        bank.record_exact(
            FixedRole::Custody,
            2,
            3,
            program,
            [1; 32],
            *b"REQUEST1",
            [2; 32],
            *b"RECEIPT1",
            b"RECEIPT1".to_vec(),
        )
        .expect("record");
        assert_eq!(
            bank.resolve(
                Some(dependency()),
                Some(&program),
                Some(ExpectedReceiptProvenanceV4 {
                    context_digest: [1; 32],
                    request_kind: *b"REQUEST1",
                    request_digest: [2; 32],
                }),
            ),
            Ok(Some(b"RECEIPT1".as_slice()))
        );
        assert_eq!(
            bank.resolve(
                Some(dependency()),
                Some(&Pubkey::new_unique()),
                Some(ExpectedReceiptProvenanceV4 {
                    context_digest: [1; 32],
                    request_kind: *b"REQUEST1",
                    request_digest: [2; 32],
                }),
            ),
            Err(TradingSbfError::Transition.into())
        );
        assert_eq!(
            bank.resolve(
                Some(dependency()),
                Some(&program),
                Some(ExpectedReceiptProvenanceV4 {
                    context_digest: [9; 32],
                    request_kind: *b"REQUEST1",
                    request_digest: [2; 32],
                }),
            ),
            Err(TradingSbfError::Transition.into())
        );
        assert_eq!(
            bank.record_exact(
                FixedRole::Claims,
                2,
                3,
                program,
                [1; 32],
                *b"REQUEST1",
                [2; 32],
                *b"RECEIPT1",
                b"RECEIPT1".to_vec(),
            ),
            Err(TradingSbfError::Transition.into())
        );
    }

    #[test]
    fn append_is_exact_and_absence_cannot_smuggle_a_suffix() {
        let mut bytes = vec![9_u8; 8];
        append_receipt_dependency_v3(
            invocation(Some(dependency())),
            &mut bytes,
            Some(b"RECEIPT1"),
        )
        .expect("append");
        assert_eq!(
            bytes,
            [9_u8; 8]
                .into_iter()
                .chain(*b"RECEIPT1")
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
            require_one_chain_receipt_width_v3(oversized),
            Err(TradingSbfError::Content.into())
        );
    }
}
