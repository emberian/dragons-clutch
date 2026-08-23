//! Canonical identity values and byte-exact hashing boundaries.

use crate::{ClaimVector, Error, Result, MAX_BASIS_DEGREE, MAX_OUTCOMES};

const NATIVE_CLAIM_DOMAIN: &[u8] = b"dragons-clutch/native-portfolio-claim/v1";
const WRAPPER_PRODUCT_DOMAIN: &[u8] = b"dragons-clutch/transferable-wrapper/v1";

/// Frozen byte length of a canonical native-claim hashing preimage.
pub const NATIVE_CLAIM_PREIMAGE_BYTES: usize =
    NATIVE_CLAIM_DOMAIN.len() + 32 + 32 + 1 + 8 + 1 + (MAX_OUTCOMES * 8);
/// Frozen byte length of a canonical wrapper-product hashing preimage.
pub const WRAPPER_PRODUCT_PREIMAGE_BYTES: usize =
    WRAPPER_PRODUCT_DOMAIN.len() + (6 * 32) + (3 * 8) + 2 + 32;
/// Complete-set-compressed native cash/Egg custody policy.
pub const COMPLETE_SET_COMPRESSED_BACKING_V1: u16 = 1;

/// Immutable identity of one Market's native payout basis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct NativeBasisIdentity {
    /// Canonical Market identity.
    pub market: [u8; 32],
    /// Complete immutable Terms digest.
    pub terms: [u8; 32],
    /// Native B-spline degree.
    pub basis_degree: u8,
    /// Common exact payout-weight denominator.
    pub denominator: u64,
    /// Active native Egg width.
    pub outcome_count: u8,
}

impl NativeBasisIdentity {
    /// Validate nonzero identities and native basis bounds.
    pub fn validate(&self) -> Result<()> {
        if self.market == [0; 32] || self.terms == [0; 32] {
            return Err(Error::InvalidIdentity);
        }
        if self.basis_degree > MAX_BASIS_DEGREE {
            return Err(Error::InvalidDegree);
        }
        if self.denominator == 0 {
            return Err(Error::InvalidDenominator);
        }
        if self.outcome_count < crate::MIN_OUTCOMES
            || usize::from(self.outcome_count) > MAX_OUTCOMES
        {
            return Err(Error::InvalidOutcomeCount);
        }
        Ok(())
    }
}

/// A validated primitive vector joined to its immutable native basis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct NativeClaim {
    /// Market and Terms identity for the coefficient coordinate system.
    pub basis: NativeBasisIdentity,
    /// Primitive nontrivial wrapper coefficients.
    pub vector: ClaimVector,
}

impl NativeClaim {
    /// Validate the basis/vector join and wrapper-product restrictions.
    pub fn validate(&self) -> Result<()> {
        self.basis.validate()?;
        self.vector.validate()?;
        if self.vector.outcome_count != self.basis.outcome_count {
            return Err(Error::DifferentBasis);
        }
        Ok(())
    }

    /// Emit exactly the preimage hashed by the live native portfolio identity.
    ///
    /// SHA-256 is intentionally an adapter boundary. Hashing these exact bytes
    /// is byte-compatible with `NativePortfolioClaimV1`.
    pub fn identity_preimage(&self) -> Result<[u8; NATIVE_CLAIM_PREIMAGE_BYTES]> {
        self.validate()?;
        let mut output = [0_u8; NATIVE_CLAIM_PREIMAGE_BYTES];
        let mut cursor = 0_usize;
        put(&mut output, &mut cursor, NATIVE_CLAIM_DOMAIN)?;
        put(&mut output, &mut cursor, &self.basis.market)?;
        put(&mut output, &mut cursor, &self.basis.terms)?;
        put(&mut output, &mut cursor, &[self.basis.basis_degree])?;
        put(
            &mut output,
            &mut cursor,
            &self.basis.denominator.to_le_bytes(),
        )?;
        put(&mut output, &mut cursor, &[self.basis.outcome_count])?;
        let mut index = 0_usize;
        while index < MAX_OUTCOMES {
            put(
                &mut output,
                &mut cursor,
                &self.vector.coefficients[index].to_le_bytes(),
            )?;
            index += 1;
        }
        if cursor != NATIVE_CLAIM_PREIMAGE_BYTES {
            return Err(Error::InvariantViolation);
        }
        Ok(output)
    }
}

/// Exact executable deployments and token program one wrapper trusts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct DeploymentBinding {
    /// Wrapper executable program id.
    pub wrapper_program: [u8; 32],
    /// Wrapper upgradeable-loader ProgramData identity.
    pub wrapper_program_data: [u8; 32],
    /// Authenticated wrapper deployment slot.
    pub wrapper_deployment_slot: u64,
    /// Base Dragon's Clutch executable program id.
    pub base_program: [u8; 32],
    /// Base upgradeable-loader ProgramData identity.
    pub base_program_data: [u8; 32],
    /// Authenticated base deployment slot.
    pub base_deployment_slot: u64,
    /// Exact Token-2022 executable program id.
    pub token_2022_program: [u8; 32],
    /// Token-2022 ProgramData identity.
    pub token_2022_program_data: [u8; 32],
    /// Authenticated Token-2022 deployment slot.
    pub token_2022_deployment_slot: u64,
}

impl DeploymentBinding {
    /// Refuse absent identities and aliasing between authority roles.
    pub fn validate(&self) -> Result<()> {
        let keys = [
            self.wrapper_program,
            self.wrapper_program_data,
            self.base_program,
            self.base_program_data,
            self.token_2022_program,
            self.token_2022_program_data,
        ];
        let mut left = 0_usize;
        while left < keys.len() {
            if keys[left] == [0; 32] {
                return Err(Error::InvalidIdentity);
            }
            let mut right = left + 1;
            while right < keys.len() {
                if keys[left] == keys[right] {
                    return Err(Error::InvalidIdentity);
                }
                right += 1;
            }
            left += 1;
        }
        Ok(())
    }

    /// Emit the wrapper-product preimage binding code deployments and claim.
    ///
    /// `native_claim_id` must be SHA-256 over [`NativeClaim::identity_preimage`]
    /// as checked by the adapter. Certificate, label, and display scaling are
    /// absent because they are provenance rather than fungibility.
    pub fn product_preimage(
        &self,
        native_claim_id: [u8; 32],
    ) -> Result<[u8; WRAPPER_PRODUCT_PREIMAGE_BYTES]> {
        self.validate()?;
        if native_claim_id == [0; 32] {
            return Err(Error::InvalidIdentity);
        }
        let mut output = [0_u8; WRAPPER_PRODUCT_PREIMAGE_BYTES];
        let mut cursor = 0_usize;
        put(&mut output, &mut cursor, WRAPPER_PRODUCT_DOMAIN)?;
        put(&mut output, &mut cursor, &self.wrapper_program)?;
        put(&mut output, &mut cursor, &self.wrapper_program_data)?;
        put(
            &mut output,
            &mut cursor,
            &self.wrapper_deployment_slot.to_le_bytes(),
        )?;
        put(&mut output, &mut cursor, &self.base_program)?;
        put(&mut output, &mut cursor, &self.base_program_data)?;
        put(
            &mut output,
            &mut cursor,
            &self.base_deployment_slot.to_le_bytes(),
        )?;
        put(&mut output, &mut cursor, &self.token_2022_program)?;
        put(&mut output, &mut cursor, &self.token_2022_program_data)?;
        put(
            &mut output,
            &mut cursor,
            &self.token_2022_deployment_slot.to_le_bytes(),
        )?;
        put(
            &mut output,
            &mut cursor,
            &COMPLETE_SET_COMPRESSED_BACKING_V1.to_le_bytes(),
        )?;
        put(&mut output, &mut cursor, &native_claim_id)?;
        if cursor != WRAPPER_PRODUCT_PREIMAGE_BYTES {
            return Err(Error::InvariantViolation);
        }
        Ok(output)
    }
}

fn put<const N: usize>(output: &mut [u8; N], cursor: &mut usize, bytes: &[u8]) -> Result<()> {
    let end = cursor
        .checked_add(bytes.len())
        .ok_or(Error::ArithmeticOverflow)?;
    let destination = output
        .get_mut(*cursor..end)
        .ok_or(Error::InvariantViolation)?;
    destination.copy_from_slice(bytes);
    *cursor = end;
    Ok(())
}
