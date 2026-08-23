// SPDX-License-Identifier: AGPL-3.0-or-later

use clutch_retirement::Identity32V1;
use sha2::{Digest, Sha256};

use crate::{Error, Result, MAX_OUTCOMES};

/// Domain for the content identity of one exact native payout vector.
pub const PAYOUT_VECTOR_ID_DOMAIN_V1: &[u8] =
    b"dragons-clutch/fractional-redemption/payout-vector/v1\0";

/// Ephemeral projection of the vector owned by immutable Resolution/Terms.
///
/// This value is never persisted by fractional-redemption accounts. A Solana
/// adapter must reconstruct it from the authenticated canonical owner on every
/// transition; [`Self::id`] only gives the policy an exact content join.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PayoutVectorV1 {
    /// Active prefix width.
    pub outcome_count: u8,
    /// Common exact payout denominator.
    pub denominator: u64,
    /// Exact integer numerator weights with a canonical zero tail.
    pub weights: [u64; MAX_OUTCOMES],
}

impl PayoutVectorV1 {
    /// Validate the active prefix, exact simplex sum, and zero tail.
    pub fn validate(self) -> Result<()> {
        let count = usize::from(self.outcome_count);
        if !(2..=MAX_OUTCOMES).contains(&count) || self.denominator == 0 {
            return Err(Error::InvalidPayout);
        }
        let mut sum = 0u64;
        let mut index = 0usize;
        while index < MAX_OUTCOMES {
            let weight = self.weights[index];
            if index < count {
                if weight > self.denominator {
                    return Err(Error::InvalidPayout);
                }
                sum = sum.checked_add(weight).ok_or(Error::Arithmetic)?;
            } else if weight != 0 {
                return Err(Error::NonCanonicalPadding);
            }
            index += 1;
        }
        if sum != self.denominator {
            return Err(Error::InvalidPayout);
        }
        Ok(())
    }

    /// Content identity recomputed from the canonical ephemeral projection.
    pub fn id(self) -> Result<Identity32V1> {
        self.validate()?;
        let mut hasher = Sha256::new();
        hasher.update(PAYOUT_VECTOR_ID_DOMAIN_V1);
        hasher.update([self.outcome_count]);
        hasher.update(self.denominator.to_le_bytes());
        let mut index = 0usize;
        while index < MAX_OUTCOMES {
            hasher.update(self.weights[index].to_le_bytes());
            index += 1;
        }
        Identity32V1::new(hasher.finalize().into()).map_err(|_| Error::ZeroIdentity)
    }

    /// Exact weighted remaining-claim numerator.
    pub fn weighted_liability(self, supplies: [u64; MAX_OUTCOMES]) -> Result<u128> {
        self.validate()?;
        let mut total = 0u128;
        let mut index = 0usize;
        while index < MAX_OUTCOMES {
            if index >= usize::from(self.outcome_count) {
                if supplies[index] != 0 {
                    return Err(Error::NonCanonicalPadding);
                }
            } else {
                total = total
                    .checked_add(
                        u128::from(supplies[index])
                            .checked_mul(u128::from(self.weights[index]))
                            .ok_or(Error::Arithmetic)?,
                    )
                    .ok_or(Error::Arithmetic)?;
            }
            index += 1;
        }
        Ok(total)
    }

    /// Check exact solvency for canonical ClaimLedger supply, Hoard locked
    /// principal, and the separately owned aggregate numerator credit.
    pub fn validate_solvency(
        self,
        supplies: [u64; MAX_OUTCOMES],
        locked_claim_principal_atoms: u64,
        aggregate_credit: u128,
    ) -> Result<()> {
        let backing = u128::from(locked_claim_principal_atoms)
            .checked_mul(u128::from(self.denominator))
            .ok_or(Error::Arithmetic)?;
        let liability = self
            .weighted_liability(supplies)?
            .checked_add(aggregate_credit)
            .ok_or(Error::Arithmetic)?;
        if backing < liability {
            Err(Error::Insolvent)
        } else {
            Ok(())
        }
    }

    /// Numerator slack above all native claims and claimant credits.
    pub fn solvency_slack(
        self,
        supplies: [u64; MAX_OUTCOMES],
        locked_claim_principal_atoms: u64,
        aggregate_credit: u128,
    ) -> Result<u128> {
        self.validate_solvency(supplies, locked_claim_principal_atoms, aggregate_credit)?;
        let backing = u128::from(locked_claim_principal_atoms)
            .checked_mul(u128::from(self.denominator))
            .ok_or(Error::Arithmetic)?;
        let liability = self
            .weighted_liability(supplies)?
            .checked_add(aggregate_credit)
            .ok_or(Error::Arithmetic)?;
        backing.checked_sub(liability).ok_or(Error::Insolvent)
    }

    /// Least exact redemption lot for one resolved outcome.
    pub fn outcome_lot(self, outcome: u8) -> Result<u64> {
        self.validate()?;
        let index = usize::from(outcome);
        if index >= usize::from(self.outcome_count) {
            return Err(Error::InvalidPayout);
        }
        Ok(self.denominator / gcd(self.denominator, self.weights[index]))
    }

    /// Least common exact lot across every outcome in this resolved vector.
    pub fn common_lot(self) -> Result<u64> {
        self.validate()?;
        let mut combined = 1u64;
        let mut index = 0usize;
        while index < usize::from(self.outcome_count) {
            let next = self.denominator / gcd(self.denominator, self.weights[index]);
            combined = (combined / gcd(combined, next))
                .checked_mul(next)
                .ok_or(Error::Arithmetic)?;
            index += 1;
        }
        if self.denominator % combined != 0 {
            return Err(Error::InvalidPayout);
        }
        Ok(combined)
    }
}

/// Euclid's greatest common divisor, with `gcd(0,0)=0`.
pub const fn gcd(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}
