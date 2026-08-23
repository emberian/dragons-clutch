// SPDX-License-Identifier: AGPL-3.0-or-later

//! Full-width immutable native-claim Resolution successor.
//!
//! Resolution V5 is the sole owner of the finalized payout vector. It names
//! the complete Product `MarketInstanceV2` and `NativeClaimBasis` identities;
//! no lowered legacy Market or Terms coordinate is accepted. The pure codec
//! deliberately does not mint finalization authority. A live writer must
//! consume Product's authenticated active-market capability and Failure's
//! private one-shot interval-resolution capability before constructing these
//! bytes and atomically activating the Hoard/ClaimLedger plan below.

use clutch_retirement::{
    DeletableRentOwnerV1, PositionV3Sha256Backend, DELETABLE_RENT_OWNER_V1_BYTES, MAX_OUTCOMES,
};

use crate::codec::{Reader, Writer};
use crate::{digest, ClaimLedgerV3, Error, HoardV2, Id, MarketLiabilityLifecycleV1, Result};

/// Reused central Resolution discriminator under the full-width V5 layout.
pub const RESOLUTION_V5_TAG: u8 = 16;
/// Full-width Resolution layout version.
pub const RESOLUTION_V5_VERSION: u8 = 5;
/// Exact canonical Resolution V5 bytes.
pub const RESOLUTION_V5_BYTES: usize = 304;
/// Resolution V5 semantic identity domain.
pub const RESOLUTION_V5_SEMANTIC_DOMAIN: &[u8] = b"dragons-clutch/resolution/v5\0";
/// Account-bound Resolution V5 data identity domain.
pub const RESOLUTION_V5_DATA_DOMAIN: &[u8] = b"dragons-clutch/resolution/data/v5\0";
/// Atomic Resolution/Hoard/ClaimLedger activation domain.
pub const MARKET_RESOLUTION_ACTIVATION_DOMAIN_V5: &[u8] =
    b"dragons-clutch/market-resolution/activation/v5\0";

const HEADER_BYTES: usize = 16;
const ID_COUNT: usize = 3;
const _: () = assert!(
    RESOLUTION_V5_BYTES
        == HEADER_BYTES + ID_COUNT * 32 + 2 * 8 + MAX_OUTCOMES * 8 + DELETABLE_RENT_OWNER_V1_BYTES
);

/// Lifecycle of one immutable payout vector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ResolutionStateV5 {
    /// Product and Failure evidence has finalized the vector; claims may exit.
    Finalized = 1,
    /// All joined native liabilities are exhausted; no payout may be consumed.
    Terminal = 2,
}

impl ResolutionStateV5 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Finalized),
            2 => Ok(Self::Terminal),
            _ => Err(Error::InvalidParameter),
        }
    }
}

/// The sole native-claim quantity-to-collateral unit boundary.
///
/// A payout is admitted only when `quantity * weight / denominator` is an
/// exact whole number of collateral atoms. No hidden floor, ceiling, or
/// nearest rounding exists at redemption.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ResolutionPayoutUnitBoundaryV5 {
    /// Exact whole collateral atoms; a nonzero remainder refuses.
    ExactWholeCollateralAtoms = 1,
}

impl ResolutionPayoutUnitBoundaryV5 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::ExactWholeCollateralAtoms),
            _ => Err(Error::InvalidParameter),
        }
    }
}

/// Complete facts the Product/Failure writer must authenticate privately.
///
/// This public fixed-memory value is not an authorization. It is the exact
/// postimage contract shared by the two private writer authorities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolutionFinalizationFactsV5 {
    /// Full Product MarketInstanceV2 identity.
    pub market_instance_id: Id,
    /// Exact NativeClaimBasis artifact identity.
    pub native_claim_basis_id: Id,
    /// Composite Product/Failure terminal-resolution evidence identity.
    pub finalization_evidence_id: Id,
    /// Active payout width.
    pub outcome_count: u8,
    /// Positive exact scaled-integer denominator.
    pub payout_denominator: u64,
    /// Active weights summing exactly to the denominator, then zero padding.
    pub payout_weights: [u64; MAX_OUTCOMES],
    /// Nonzero immutable Market generation finalized by Product and Failure.
    pub generation: u64,
    /// Sole admitted quantity-to-collateral unit rule.
    pub payout_unit_boundary: ResolutionPayoutUnitBoundaryV5,
}

/// Exact quotient and retained numerator remainder for one claim payout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolutionPayoutProjectionV5 {
    resolution_account: Id,
    resolution_semantic_id: Id,
    resolution_data_id: Id,
    market_instance_id: Id,
    native_claim_basis_id: Id,
    generation: u64,
    outcome: u8,
    quantity: u64,
    payout_weight: u64,
    /// Whole collateral atoms immediately payable.
    whole_atoms: u64,
    /// Exact numerator remainder, always less than [`Self::denominator`].
    remainder_numerator: u64,
    /// Immutable Resolution denominator shared with the Fractional family.
    denominator: u64,
    payout_unit_boundary: ResolutionPayoutUnitBoundaryV5,
}

impl ResolutionPayoutProjectionV5 {
    /// Exact authenticated Resolution account expected by this projection.
    pub const fn resolution_account(self) -> Id {
        self.resolution_account
    }

    /// Body-only Resolution semantic identity.
    pub const fn resolution_semantic_id(self) -> Id {
        self.resolution_semantic_id
    }

    /// Exact PDA-bound Resolution data identity.
    pub const fn resolution_data_id(self) -> Id {
        self.resolution_data_id
    }

    /// Full MarketInstanceV2 identity.
    pub const fn market_instance_id(self) -> Id {
        self.market_instance_id
    }

    /// Exact NativeClaimBasis identity.
    pub const fn native_claim_basis_id(self) -> Id {
        self.native_claim_basis_id
    }

    /// Nonzero immutable Market generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Exact selected outcome.
    pub const fn outcome(self) -> u8 {
        self.outcome
    }

    /// Exact claim quantity projected.
    pub const fn quantity(self) -> u64 {
        self.quantity
    }

    /// Exact selected scaled payout weight.
    pub const fn payout_weight(self) -> u64 {
        self.payout_weight
    }

    /// Immutable Resolution denominator.
    pub const fn denominator(self) -> u64 {
        self.denominator
    }

    /// Whole collateral atoms immediately payable.
    pub const fn whole_atoms(self) -> u64 {
        self.whole_atoms
    }

    /// Exact numerator remainder retained by the Fractional family.
    pub const fn remainder_numerator(self) -> u64 {
        self.remainder_numerator
    }

    /// Sole quantity-to-collateral unit boundary.
    pub const fn payout_unit_boundary(self) -> ResolutionPayoutUnitBoundaryV5 {
        self.payout_unit_boundary
    }

    /// Whether the direct whole-atom path is exact and needs no fractional credit.
    pub const fn is_exact(self) -> bool {
        self.remainder_numerator == 0
    }
}

impl ResolutionFinalizationFactsV5 {
    /// Validate full identities, exact simplex scaling, and canonical padding.
    pub fn validate(self) -> Result<()> {
        self.market_instance_id.require_live()?;
        self.native_claim_basis_id.require_live()?;
        self.finalization_evidence_id.require_live()?;
        if self.generation == 0
            || self.payout_denominator == 0
            || usize::from(self.outcome_count) < 2
            || usize::from(self.outcome_count) > MAX_OUTCOMES
            || self.payout_unit_boundary
                != ResolutionPayoutUnitBoundaryV5::ExactWholeCollateralAtoms
        {
            return Err(Error::InvalidParameter);
        }
        let mut sum = 0u128;
        let mut index = 0usize;
        while index < MAX_OUTCOMES {
            let weight = self.payout_weights[index];
            if index < usize::from(self.outcome_count) {
                if weight > self.payout_denominator {
                    return Err(Error::InvalidParameter);
                }
                sum = sum
                    .checked_add(u128::from(weight))
                    .ok_or(Error::Arithmetic)?;
            } else if weight != 0 {
                return Err(Error::NonCanonicalPadding);
            }
            index += 1;
        }
        if sum != u128::from(self.payout_denominator) {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }
}

/// Canonical full-width Resolution V5 body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolutionV5 {
    /// Privately authenticated immutable finalization facts.
    pub facts: ResolutionFinalizationFactsV5,
    /// Finalized or terminal liability state.
    pub state: ResolutionStateV5,
    /// Canonical full-width Resolution PDA bump.
    pub stored_bump: u8,
    /// Deletable lamport-rent owner; never collateral principal.
    pub rent: DeletableRentOwnerV1,
}

impl ResolutionV5 {
    /// Construct the exact finalized postimage after private writer authority.
    pub fn finalized(
        facts: ResolutionFinalizationFactsV5,
        stored_bump: u8,
        rent: DeletableRentOwnerV1,
    ) -> Result<Self> {
        let value = Self {
            facts,
            state: ResolutionStateV5::Finalized,
            stored_bump,
            rent,
        };
        value.validate()?;
        Ok(value)
    }

    /// Validate the complete canonical body.
    pub fn validate(self) -> Result<()> {
        self.facts.validate()?;
        self.rent.validate().map_err(|_| Error::InvalidParameter)
    }

    /// Project the exact whole payout and numerator remainder without flooring
    /// away value. A nonzero remainder must enter the canonical Fractional
    /// credit family atomically with claim retirement.
    pub fn payout_projection<B: PositionV3Sha256Backend>(
        self,
        resolution_account: Id,
        outcome: u8,
        quantity: u64,
        backend: &B,
    ) -> Result<ResolutionPayoutProjectionV5> {
        self.validate()?;
        resolution_account.require_live()?;
        if self.state != ResolutionStateV5::Finalized
            || outcome >= self.facts.outcome_count
            || quantity == 0
        {
            return Err(Error::InvalidParameter);
        }
        let numerator = u128::from(quantity)
            .checked_mul(u128::from(self.facts.payout_weights[usize::from(outcome)]))
            .ok_or(Error::Arithmetic)?;
        let denominator = u128::from(self.facts.payout_denominator);
        let whole_atoms = u64::try_from(numerator / denominator).map_err(|_| Error::Arithmetic)?;
        let remainder_numerator =
            u64::try_from(numerator % denominator).map_err(|_| Error::Arithmetic)?;
        Ok(ResolutionPayoutProjectionV5 {
            resolution_account,
            resolution_semantic_id: self.semantic_id(backend)?,
            resolution_data_id: self.data_id(resolution_account)?,
            market_instance_id: self.facts.market_instance_id,
            native_claim_basis_id: self.facts.native_claim_basis_id,
            generation: self.facts.generation,
            outcome,
            quantity,
            payout_weight: self.facts.payout_weights[usize::from(outcome)],
            whole_atoms,
            remainder_numerator,
            denominator: self.facts.payout_denominator,
            payout_unit_boundary: self.facts.payout_unit_boundary,
        })
    }

    /// Compute an exact whole-atom payout or refuse a remainder.
    pub fn payout_atoms(self, outcome: u8, quantity: u64) -> Result<u64> {
        self.validate()?;
        if self.state != ResolutionStateV5::Finalized
            || outcome >= self.facts.outcome_count
            || quantity == 0
        {
            return Err(Error::InvalidParameter);
        }
        let numerator = u128::from(quantity)
            .checked_mul(u128::from(self.facts.payout_weights[usize::from(outcome)]))
            .ok_or(Error::Arithmetic)?;
        let denominator = u128::from(self.facts.payout_denominator);
        if numerator % denominator != 0 {
            return Err(Error::PayoutRemainder);
        }
        u64::try_from(numerator / denominator).map_err(|_| Error::Arithmetic)
    }

    /// Encode exactly [`RESOLUTION_V5_BYTES`] canonical bytes.
    pub fn encode(self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, RESOLUTION_V5_BYTES)?;
        writer.u8(RESOLUTION_V5_TAG)?;
        writer.u8(RESOLUTION_V5_VERSION)?;
        writer.u8(self.state as u8)?;
        writer.u8(self.facts.outcome_count)?;
        writer.u8(self.facts.payout_unit_boundary as u8)?;
        writer.u8(self.stored_bump)?;
        writer.bytes(&[0; 10])?;
        writer.id(self.facts.market_instance_id)?;
        writer.id(self.facts.native_claim_basis_id)?;
        writer.id(self.facts.finalization_evidence_id)?;
        writer.u64(self.facts.generation)?;
        writer.u64(self.facts.payout_denominator)?;
        for weight in self.facts.payout_weights {
            writer.u64(weight)?;
        }
        writer.bytes(&self.rent.encode().map_err(|_| Error::InvalidParameter)?)?;
        writer.finish()
    }

    /// Decode exactly [`RESOLUTION_V5_BYTES`] hostile canonical bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, RESOLUTION_V5_BYTES)?;
        if reader.u8()? != RESOLUTION_V5_TAG {
            return Err(Error::BadMagic);
        }
        if reader.u8()? != RESOLUTION_V5_VERSION {
            return Err(Error::BadVersion);
        }
        let state = ResolutionStateV5::decode(reader.u8()?)?;
        let outcome_count = reader.u8()?;
        let payout_unit_boundary = ResolutionPayoutUnitBoundaryV5::decode(reader.u8()?)?;
        let stored_bump = reader.u8()?;
        reader.require_zeroes(10)?;
        let market_instance_id = reader.id()?;
        let native_claim_basis_id = reader.id()?;
        let finalization_evidence_id = reader.id()?;
        let generation = reader.u64()?;
        let payout_denominator = reader.u64()?;
        let mut payout_weights = [0; MAX_OUTCOMES];
        let mut index = 0usize;
        while index < MAX_OUTCOMES {
            payout_weights[index] = reader.u64()?;
            index += 1;
        }
        let rent = DeletableRentOwnerV1::decode(&reader.bytes::<DELETABLE_RENT_OWNER_V1_BYTES>()?)
            .map_err(|_| Error::InvalidParameter)?;
        reader.finish()?;
        let value = Self {
            facts: ResolutionFinalizationFactsV5 {
                market_instance_id,
                native_claim_basis_id,
                finalization_evidence_id,
                outcome_count,
                payout_denominator,
                payout_weights,
                generation,
                payout_unit_boundary,
            },
            state,
            stored_bump,
            rent,
        };
        value.validate()?;
        Ok(value)
    }

    /// Canonical content identity of these exact bytes.
    pub fn semantic_id<B: PositionV3Sha256Backend>(self, backend: &B) -> Result<Id> {
        let mut body = [0; RESOLUTION_V5_BYTES];
        self.encode(&mut body)?;
        let id = Id::from_bytes(backend.sha256(RESOLUTION_V5_SEMANTIC_DOMAIN, &body));
        id.require_live()?;
        Ok(id)
    }

    /// Canonical account-bound identity of the PDA and these exact bytes.
    ///
    /// Unlike [`Self::semantic_id`], this refuses substituting identical bytes
    /// at a different physical account in an activation or redemption receipt.
    pub fn data_id(self, resolution_account: Id) -> Result<Id> {
        resolution_account.require_live()?;
        let mut body = [0; RESOLUTION_V5_BYTES];
        self.encode(&mut body)?;
        let id = digest(
            RESOLUTION_V5_DATA_DOMAIN,
            &[&resolution_account.bytes(), &body],
        );
        id.require_live()?;
        Ok(id)
    }
}

/// Atomic activation of the sole payout truth and both liability owners.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketResolutionActivationPlanV5 {
    resolution_account: Id,
    resolution_id: Id,
    resolution_data_id: Id,
    hoard_before_id: Id,
    hoard_after: HoardV2,
    hoard_after_id: Id,
    claim_ledger_before_id: Id,
    claim_ledger_after: ClaimLedgerV3,
    claim_ledger_after_id: Id,
    receipt_id: Id,
}

impl MarketResolutionActivationPlanV5 {
    /// Exact newly finalized Resolution account.
    pub const fn resolution_account(self) -> Id {
        self.resolution_account
    }

    /// Canonical Resolution V5 content identity.
    pub const fn resolution_id(self) -> Id {
        self.resolution_id
    }

    /// Canonical PDA-bound Resolution V5 data identity.
    pub const fn resolution_data_id(self) -> Id {
        self.resolution_data_id
    }

    /// Exact Hoard semantic prestate.
    pub const fn hoard_before_id(self) -> Id {
        self.hoard_before_id
    }

    /// Complete permitted resolved Hoard successor.
    pub const fn hoard_after(self) -> HoardV2 {
        self.hoard_after
    }

    /// Exact Hoard successor identity.
    pub const fn hoard_after_id(self) -> Id {
        self.hoard_after_id
    }

    /// Exact ClaimLedger semantic prestate.
    pub const fn claim_ledger_before_id(self) -> Id {
        self.claim_ledger_before_id
    }

    /// Complete permitted resolved ClaimLedger successor.
    pub const fn claim_ledger_after(self) -> ClaimLedgerV3 {
        self.claim_ledger_after
    }

    /// Exact ClaimLedger successor identity.
    pub const fn claim_ledger_after_id(self) -> Id {
        self.claim_ledger_after_id
    }

    /// Atomic activation receipt to consume in the private Product writer.
    pub const fn receipt_id(self) -> Id {
        self.receipt_id
    }
}

/// Activate an authenticated Resolution V5 and both canonical liability
/// owners. This enforces the atomic postimage but does not authenticate the
/// Product/Failure authority that is permitted to write it.
pub fn prepare_market_resolution_activation_v5<B: PositionV3Sha256Backend>(
    resolution_account: Id,
    resolution: ResolutionV5,
    hoard: HoardV2,
    claim_ledger: ClaimLedgerV3,
    backend: &B,
) -> Result<MarketResolutionActivationPlanV5> {
    resolution_account.require_live()?;
    resolution.validate()?;
    hoard.validate()?;
    claim_ledger.validate()?;
    if resolution.state != ResolutionStateV5::Finalized
        || hoard.lifecycle != MarketLiabilityLifecycleV1::Open
        || claim_ledger.lifecycle != MarketLiabilityLifecycleV1::Open
        || hoard.market_instance_id != resolution.facts.market_instance_id
        || claim_ledger.market_instance_id != resolution.facts.market_instance_id
        || claim_ledger.native_claim_basis_id != resolution.facts.native_claim_basis_id
        || hoard.realm_id != claim_ledger.realm_id
        || hoard.outcome_count != resolution.facts.outcome_count
        || claim_ledger.outcome_count != resolution.facts.outcome_count
        || !claim_ledger.resolution_account.is_zero()
    {
        return Err(Error::MismatchedBinding);
    }
    let resolution_id = resolution.semantic_id(backend)?;
    let resolution_data_id = resolution.data_id(resolution_account)?;
    let hoard_before_id = hoard.semantic_id(backend)?;
    let claim_ledger_before_id = claim_ledger.semantic_id(backend)?;
    let hoard_after = HoardV2 {
        lifecycle: MarketLiabilityLifecycleV1::Resolved,
        ..hoard
    };
    let claim_ledger_after = ClaimLedgerV3 {
        resolution_account,
        lifecycle: MarketLiabilityLifecycleV1::Resolved,
        ..claim_ledger
    };
    hoard_after.validate()?;
    claim_ledger_after.validate()?;
    let hoard_after_id = hoard_after.semantic_id(backend)?;
    let claim_ledger_after_id = claim_ledger_after.semantic_id(backend)?;
    let receipt_id = digest(
        MARKET_RESOLUTION_ACTIVATION_DOMAIN_V5,
        &[
            &resolution_account.bytes(),
            &resolution_id.bytes(),
            &resolution_data_id.bytes(),
            &resolution.facts.finalization_evidence_id.bytes(),
            &resolution.facts.generation.to_le_bytes(),
            &hoard_before_id.bytes(),
            &hoard_after_id.bytes(),
            &claim_ledger_before_id.bytes(),
            &claim_ledger_after_id.bytes(),
        ],
    );
    receipt_id.require_live()?;
    Ok(MarketResolutionActivationPlanV5 {
        resolution_account,
        resolution_id,
        resolution_data_id,
        hoard_before_id,
        hoard_after,
        hoard_after_id,
        claim_ledger_before_id,
        claim_ledger_after,
        claim_ledger_after_id,
        receipt_id,
    })
}
