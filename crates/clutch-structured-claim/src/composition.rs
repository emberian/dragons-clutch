//! Flat associative composition over one authenticated native basis.

use core::convert::TryFrom;

use crate::{
    gcd_u64, Amount, ClaimVector, Error, NativeBasisIdentity, NativeClaim, Result, MAX_OUTCOMES,
};

/// Whether a flattened vector remains a wrapper or exits as base cash.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CompositionDisposition {
    /// Mint `primitive_units` of the returned nontrivial primitive claim.
    TransferableWrapper,
    /// Merge the exact constant vector and return cash; mint no wrapper.
    CompleteSetCash,
}

/// Final exact result of bounded flat composition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct FlattenedComposition {
    /// Common native basis for every input and the output.
    pub basis: NativeBasisIdentity,
    /// Primitive output vector. For cash disposition this is the all-one vector.
    pub primitive: [Amount; MAX_OUTCOMES],
    /// Number of primitive output units represented by `exact_eggs`.
    pub primitive_units: Amount,
    /// Exact native vector released by all inputs.
    pub exact_eggs: [Amount; MAX_OUTCOMES],
    /// Complete-set cash already present in all input wrapper vaults.
    pub input_cash_atoms: Amount,
    /// Newly exposed complete sets to merge before producing the output.
    pub additional_complete_sets_to_merge: Amount,
    /// Canonical cash backing across every output wrapper atom.
    pub output_cash_atoms: Amount,
    /// Canonical residual Egg backing across every output wrapper atom.
    pub output_residual_eggs: [Amount; MAX_OUTCOMES],
    /// Wrapper-or-cash output route.
    pub disposition: CompositionDisposition,
}

/// Allocation-free monoidal accumulator for wrapper composition.
///
/// It contains native vectors, never wrapper references, so persisted nesting
/// and cycles are unrepresentable. [`Self::combine`] makes regrouping explicit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct CompositionAccumulator {
    basis: NativeBasisIdentity,
    exact_eggs: [Amount; MAX_OUTCOMES],
    input_cash_atoms: Amount,
    leg_count: u64,
}

impl CompositionAccumulator {
    /// Start an empty composition for one immutable native basis.
    pub fn new(basis: NativeBasisIdentity) -> Result<Self> {
        basis.validate()?;
        Ok(Self {
            basis,
            exact_eggs: [0; MAX_OUTCOMES],
            input_cash_atoms: 0,
            leg_count: 0,
        })
    }

    /// Fold a positive quantity of one nontrivial primitive wrapper claim.
    pub fn push(&mut self, claim: &NativeClaim, wrapper_atoms: Amount) -> Result<()> {
        claim.validate()?;
        if wrapper_atoms == 0 {
            return Err(Error::ZeroQuantity);
        }
        if claim.basis != self.basis {
            return Err(Error::DifferentBasis);
        }
        let backing = claim.vector.backing_plan()?;
        let added_cash = wrapper_atoms
            .checked_mul(backing.cash_per_wrapper)
            .ok_or(Error::ArithmeticOverflow)?;
        let next_cash = self
            .input_cash_atoms
            .checked_add(added_cash)
            .ok_or(Error::ArithmeticOverflow)?;
        let next_count = self
            .leg_count
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        let mut next_eggs = self.exact_eggs;
        let mut index = 0_usize;
        while index < usize::from(self.basis.outcome_count) {
            let amount = wrapper_atoms
                .checked_mul(claim.vector.coefficients[index])
                .ok_or(Error::ArithmeticOverflow)?;
            next_eggs[index] = next_eggs[index]
                .checked_add(amount)
                .ok_or(Error::ArithmeticOverflow)?;
            index += 1;
        }
        self.exact_eggs = next_eggs;
        self.input_cash_atoms = next_cash;
        self.leg_count = next_count;
        Ok(())
    }

    /// Combine two partial folds over the same basis.
    pub fn combine(&mut self, other: &Self) -> Result<()> {
        if self.basis != other.basis {
            return Err(Error::DifferentBasis);
        }
        let next_cash = self
            .input_cash_atoms
            .checked_add(other.input_cash_atoms)
            .ok_or(Error::ArithmeticOverflow)?;
        let next_count = self
            .leg_count
            .checked_add(other.leg_count)
            .ok_or(Error::ArithmeticOverflow)?;
        let mut next_eggs = self.exact_eggs;
        let mut index = 0_usize;
        while index < usize::from(self.basis.outcome_count) {
            next_eggs[index] = next_eggs[index]
                .checked_add(other.exact_eggs[index])
                .ok_or(Error::ArithmeticOverflow)?;
            index += 1;
        }
        self.exact_eggs = next_eggs;
        self.input_cash_atoms = next_cash;
        self.leg_count = next_count;
        Ok(())
    }

    /// Finish one exact vector and choose a wrapper or cash output.
    pub fn finish(&self) -> Result<FlattenedComposition> {
        if self.leg_count == 0 {
            return Err(Error::EmptyComposition);
        }
        let count = usize::from(self.basis.outcome_count);
        let mut divisor = 0_u64;
        let mut output_cash = self.exact_eggs[0];
        let mut index = 0_usize;
        while index < count {
            divisor = gcd_u64(divisor, self.exact_eggs[index]);
            if self.exact_eggs[index] < output_cash {
                output_cash = self.exact_eggs[index];
            }
            index += 1;
        }
        if divisor == 0 {
            return Err(Error::ZeroClaim);
        }
        let additional = output_cash
            .checked_sub(self.input_cash_atoms)
            .ok_or(Error::InvariantViolation)?;
        let mut primitive = [0_u64; MAX_OUTCOMES];
        let mut residual = [0_u64; MAX_OUTCOMES];
        index = 0;
        let mut constant = true;
        while index < count {
            primitive[index] = self.exact_eggs[index]
                .checked_div(divisor)
                .ok_or(Error::ArithmeticOverflow)?;
            residual[index] = self.exact_eggs[index]
                .checked_sub(output_cash)
                .ok_or(Error::ArithmeticUnderflow)?;
            if residual[index] != 0 {
                constant = false;
            }
            index += 1;
        }
        let disposition = if constant {
            CompositionDisposition::CompleteSetCash
        } else {
            ClaimVector {
                outcome_count: self.basis.outcome_count,
                coefficients: primitive,
            }
            .validate()?;
            CompositionDisposition::TransferableWrapper
        };
        let primitive_units =
            Amount::try_from(u128::from(divisor)).map_err(|_| Error::ArithmeticOverflow)?;
        Ok(FlattenedComposition {
            basis: self.basis,
            primitive,
            primitive_units,
            exact_eggs: self.exact_eggs,
            input_cash_atoms: self.input_cash_atoms,
            additional_complete_sets_to_merge: additional,
            output_cash_atoms: output_cash,
            output_residual_eggs: residual,
            disposition,
        })
    }
}
