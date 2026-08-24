// SPDX-License-Identifier: AGPL-3.0-or-later

//! Current per-outcome segregated collateral custody contract.
//!
//! Product's 47-slot foundation names one custody account for each active
//! OutcomeMintV2. These are release-selected collateral token accounts, not
//! bearer-claim accounts and not caller-selected ATAs. The SBF owner derives
//! their PDAs and owner authority, executes the selected token-program
//! initializer, and supplies hostile postwrite bytes to this module.

use crate::{
    admit_collateral_account_v2, digest, prepare_custody_creation_v2,
    BoundCollateralProfileV2, CustodyBindingV2, CustodyCreationPlanV2, Error, Id, Result,
    RuntimeAccountViewV2, TokenAccountRoleV2,
};
use clutch_retirement::MAX_OUTCOMES;

/// Complete current outcome-custody plan identity.
pub const OUTCOME_CUSTODY_FOUNDING_DOMAIN_V1: &[u8] =
    b"dragons-clutch/outcome-custody/founding/v1\0";
/// One bounded outcome/custody pairing identity.
pub const OUTCOME_CUSTODY_FOUNDING_STEP_DOMAIN_V1: &[u8] =
    b"dragons-clutch/outcome-custody/founding-step/v1\0";
/// Hostile accepted custody postwrite identity.
pub const ACCEPTED_OUTCOME_CUSTODY_FOUNDING_STEP_DOMAIN_V1: &[u8] =
    b"dragons-clutch/outcome-custody/founding-accepted/v1\0";

/// Owner-local compartment 1 is the pooled Hoard. Outcome custody starts at 2.
const OUTCOME_CUSTODY_COMPARTMENT_BASE_V1: u16 = 2;

/// Full-width addresses for one current Market's outcome-custody plane.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OutcomeCustodyFoundingRequestV1 {
    /// Full Product-owned MarketInstanceV2 identity.
    pub market_instance_id: Id,
    /// Current Market generation; zero is never a live generation.
    pub generation: u64,
    /// Canonical program-derived owner stored in every custody token account.
    pub owner_authority: Id,
    /// Active native outcome width.
    pub outcome_count: u8,
    /// Canonical OutcomeMintV2 accounts; inactive tail is zero.
    pub outcome_mints: [Id; MAX_OUTCOMES],
    /// Canonical outcome-custody accounts; inactive tail is zero.
    pub outcome_custodies: [Id; MAX_OUTCOMES],
}

/// Closed current plan for every active outcome-custody account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OutcomeCustodyFoundingPlanV1 {
    bound: BoundCollateralProfileV2,
    market_instance_id: Id,
    generation: u64,
    owner_authority: Id,
    outcome_count: u8,
    outcome_mints: [Id; MAX_OUTCOMES],
    outcome_custodies: [Id; MAX_OUTCOMES],
    founding_id: Id,
}

/// Exact one-outcome projection from the complete current custody plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OutcomeCustodyFoundingStepV1 {
    bound: BoundCollateralProfileV2,
    founding_id: Id,
    market_instance_id: Id,
    generation: u64,
    owner_authority: Id,
    outcome: u8,
    outcome_mint: Id,
    outcome_custody: Id,
    binding: CustodyBindingV2,
    creation: CustodyCreationPlanV2,
    step_id: Id,
}

/// Accepted hostile account postwrite for one current custody step.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AcceptedOutcomeCustodyFoundingStepV1 {
    step: OutcomeCustodyFoundingStepV1,
    receipt_id: Id,
}

impl OutcomeCustodyFoundingPlanV1 {
    /// Exact MarketInstanceV2 identity.
    pub const fn market_instance_id(self) -> Id { self.market_instance_id }
    /// Current Product Market generation.
    pub const fn generation(self) -> u64 { self.generation }
    /// Canonical program-derived token owner.
    pub const fn owner_authority(self) -> Id { self.owner_authority }
    /// Active outcome prefix length.
    pub const fn outcome_count(self) -> u8 { self.outcome_count }
    /// Complete current founding identity.
    pub const fn founding_id(self) -> Id { self.founding_id }

    /// Exact active outcome mint, refusing inactive padding.
    pub fn outcome_mint(self, outcome: u8) -> Result<Id> {
        if outcome >= self.outcome_count {
            return Err(Error::InvalidParameter);
        }
        Ok(self.outcome_mints[usize::from(outcome)])
    }

    /// Exact active custody account, refusing inactive padding.
    pub fn outcome_custody(self, outcome: u8) -> Result<Id> {
        if outcome >= self.outcome_count {
            return Err(Error::InvalidParameter);
        }
        Ok(self.outcome_custodies[usize::from(outcome)])
    }

    /// Derive the only bounded current step for one active outcome.
    pub fn step(self, outcome: u8) -> Result<OutcomeCustodyFoundingStepV1> {
        let outcome_mint = self.outcome_mint(outcome)?;
        let outcome_custody = self.outcome_custody(outcome)?;
        let compartment = OUTCOME_CUSTODY_COMPARTMENT_BASE_V1
            .checked_add(u16::from(outcome))
            .ok_or(Error::Arithmetic)?;
        let binding = CustodyBindingV2 {
            account: outcome_custody,
            owner_authority: self.owner_authority,
            semantic_owner: self.market_instance_id,
            compartment,
            owner_guard: self.bound.release().owner_guard,
            owner_authority_is_program_derived: true,
        };
        let creation = prepare_custody_creation_v2(self.bound, binding)?;
        let step_id = digest(
            OUTCOME_CUSTODY_FOUNDING_STEP_DOMAIN_V1,
            &[
                &self.founding_id.bytes(),
                &self.market_instance_id.bytes(),
                &self.generation.to_le_bytes(),
                &[outcome],
                &outcome_mint.bytes(),
                &outcome_custody.bytes(),
                &self.owner_authority.bytes(),
                &compartment.to_le_bytes(),
                &creation.token_program.bytes(),
                &creation.mint.bytes(),
                &creation.account_bytes.to_le_bytes(),
            ],
        );
        step_id.require_live()?;
        Ok(OutcomeCustodyFoundingStepV1 {
            bound: self.bound,
            founding_id: self.founding_id,
            market_instance_id: self.market_instance_id,
            generation: self.generation,
            owner_authority: self.owner_authority,
            outcome,
            outcome_mint,
            outcome_custody,
            binding,
            creation,
            step_id,
        })
    }
}

impl OutcomeCustodyFoundingStepV1 {
    /// Complete plan identity.
    pub const fn founding_id(self) -> Id { self.founding_id }
    /// MarketInstanceV2 identity.
    pub const fn market_instance_id(self) -> Id { self.market_instance_id }
    /// Current Market generation.
    pub const fn generation(self) -> u64 { self.generation }
    /// Active outcome index.
    pub const fn outcome(self) -> u8 { self.outcome }
    /// Exact paired OutcomeMintV2.
    pub const fn outcome_mint(self) -> Id { self.outcome_mint }
    /// Exact release-selected custody account.
    pub const fn outcome_custody(self) -> Id { self.outcome_custody }
    /// Canonical program-derived token owner.
    pub const fn owner_authority(self) -> Id { self.owner_authority }
    /// Exact segregated-custody binding.
    pub const fn binding(self) -> CustodyBindingV2 { self.binding }
    /// Release-selected external account creation contract.
    pub const fn creation(self) -> CustodyCreationPlanV2 { self.creation }
    /// One-step semantic identity.
    pub const fn step_id(self) -> Id { self.step_id }
}

impl AcceptedOutcomeCustodyFoundingStepV1 {
    /// Exact accepted step.
    pub const fn step(self) -> OutcomeCustodyFoundingStepV1 { self.step }
    /// Product-consumable hostile postwrite identity.
    pub const fn receipt_id(self) -> Id { self.receipt_id }
}

fn validate_outcome_custody_partition_v1(
    outcome_count: u8,
    outcome_mints: [Id; MAX_OUTCOMES],
    outcome_custodies: [Id; MAX_OUTCOMES],
    collateral_mint: Id,
    hoard_token_account: Id,
    hoard_authority: Id,
    owner_authority: Id,
) -> Result<()> {
    let active = usize::from(outcome_count);
    if active == 0 || active > MAX_OUTCOMES {
        return Err(Error::InvalidParameter);
    }
    let mut index = 0usize;
    while index < MAX_OUTCOMES {
        let active_index = index < active;
        if active_index
            != (!outcome_mints[index].is_zero() && !outcome_custodies[index].is_zero())
        {
            return Err(Error::NonCanonicalPadding);
        }
        if active_index {
            let mint = outcome_mints[index];
            let custody = outcome_custodies[index];
            if mint == custody
                || mint == collateral_mint
                || custody == collateral_mint
                || custody == hoard_token_account
                || custody == hoard_authority
                || custody == owner_authority
            {
                return Err(Error::MismatchedBinding);
            }
            let mut prior = 0usize;
            while prior < index {
                if mint == outcome_mints[prior]
                    || custody == outcome_custodies[prior]
                    || mint == outcome_custodies[prior]
                    || custody == outcome_mints[prior]
                {
                    return Err(Error::MismatchedBinding);
                }
                prior = prior.checked_add(1).ok_or(Error::Arithmetic)?;
            }
        }
        index = index.checked_add(1).ok_or(Error::Arithmetic)?;
    }
    Ok(())
}

/// Freeze the complete current custody graph from already authenticated
/// collateral semantics and SBF-derived addresses.
pub fn prepare_outcome_custody_founding_v1(
    bound: BoundCollateralProfileV2,
    request: OutcomeCustodyFoundingRequestV1,
) -> Result<OutcomeCustodyFoundingPlanV1> {
    if request.market_instance_id != bound.market().market
        || request.generation == 0
    {
        return Err(Error::MismatchedBinding);
    }
    request.owner_authority.require_live()?;
    validate_outcome_custody_partition_v1(
        request.outcome_count,
        request.outcome_mints,
        request.outcome_custodies,
        bound.policy().mint,
        bound.market().hoard_token_account,
        bound.market().hoard_authority,
        request.owner_authority,
    )?;
    let mut mint_bytes = [[0u8; 32]; MAX_OUTCOMES];
    let mut custody_bytes = [[0u8; 32]; MAX_OUTCOMES];
    let mut index = 0usize;
    while index < MAX_OUTCOMES {
        mint_bytes[index] = request.outcome_mints[index].bytes();
        custody_bytes[index] = request.outcome_custodies[index].bytes();
        index = index.checked_add(1).ok_or(Error::Arithmetic)?;
    }
    let mut parts: [&[u8]; 9 + 2 * MAX_OUTCOMES] = [&[]; 9 + 2 * MAX_OUTCOMES];
    let market = request.market_instance_id.bytes();
    let generation = request.generation.to_le_bytes();
    let outcome_count = [request.outcome_count];
    let owner = request.owner_authority.bytes();
    let realm = bound.market().realm.bytes();
    let profile = bound.market().profile.bytes();
    let policy = bound.policy_id().bytes();
    let release = bound.release().id()?.bytes();
    let collateral_mint = bound.policy().mint.bytes();
    parts[0] = &market;
    parts[1] = &generation;
    parts[2] = &outcome_count;
    parts[3] = &owner;
    parts[4] = &realm;
    parts[5] = &profile;
    parts[6] = &policy;
    parts[7] = &release;
    parts[8] = &collateral_mint;
    index = 0;
    while index < MAX_OUTCOMES {
        parts[9 + index] = &mint_bytes[index];
        parts[9 + MAX_OUTCOMES + index] = &custody_bytes[index];
        index = index.checked_add(1).ok_or(Error::Arithmetic)?;
    }
    let founding_id = digest(OUTCOME_CUSTODY_FOUNDING_DOMAIN_V1, &parts);
    founding_id.require_live()?;
    Ok(OutcomeCustodyFoundingPlanV1 {
        bound,
        market_instance_id: request.market_instance_id,
        generation: request.generation,
        owner_authority: request.owner_authority,
        outcome_count: request.outcome_count,
        outcome_mints: request.outcome_mints,
        outcome_custodies: request.outcome_custodies,
        founding_id,
    })
}

/// Accept one newly initialized external custody account through the exact
/// Realm release parser and the complete current plan.
pub fn accept_outcome_custody_founding_step_v1(
    step: OutcomeCustodyFoundingStepV1,
    postwrite: RuntimeAccountViewV2<'_>,
) -> Result<AcceptedOutcomeCustodyFoundingStepV1> {
    let observed = admit_collateral_account_v2(
        step.bound,
        postwrite,
        TokenAccountRoleV2::SegregatedVault(step.binding),
    )?;
    if observed.address != step.outcome_custody
        || observed.mint != step.bound.policy().mint
        || observed.owner_authority != step.owner_authority
        || observed.semantic_owner != step.market_instance_id
        || observed.compartment != step.binding.compartment
        || observed.amount_atoms != 0
    {
        return Err(Error::PostAdmissionFailed);
    }
    let receipt_id = digest(
        ACCEPTED_OUTCOME_CUSTODY_FOUNDING_STEP_DOMAIN_V1,
        &[
            &step.step_id.bytes(),
            &step.founding_id.bytes(),
            &step.market_instance_id.bytes(),
            &step.generation.to_le_bytes(),
            &[step.outcome],
            &step.outcome_mint.bytes(),
            &observed.address.bytes(),
            &observed.mint.bytes(),
            &observed.owner_authority.bytes(),
            &observed.extensions.to_le_bytes(),
            &observed.compartment.to_le_bytes(),
        ],
    );
    receipt_id.require_live()?;
    Ok(AcceptedOutcomeCustodyFoundingStepV1 { step, receipt_id })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Full hostile parser cases are exercised with the collateral release
    // fixtures. This local source contract freezes the active-prefix shape.
    #[test]
    fn inactive_tail_cannot_name_a_custody_without_a_mint() {
        let mut mints = [Id::ZERO; MAX_OUTCOMES];
        let mut custodies = [Id::ZERO; MAX_OUTCOMES];
        mints[0] = Id::from_bytes([1; 32]);
        custodies[0] = Id::from_bytes([2; 32]);
        custodies[1] = Id::from_bytes([3; 32]);
        assert_eq!(
            validate_outcome_custody_partition_v1(
                1,
                mints,
                custodies,
                Id::from_bytes([4; 32]),
                Id::from_bytes([5; 32]),
                Id::from_bytes([6; 32]),
                Id::from_bytes([7; 32]),
            ),
            Err(Error::NonCanonicalPadding),
        );
    }

    #[test]
    fn active_custody_refuses_its_paired_mint_or_duplicate() {
        let mut mints = [Id::ZERO; MAX_OUTCOMES];
        let mut custodies = [Id::ZERO; MAX_OUTCOMES];
        mints[0] = Id::from_bytes([1; 32]);
        mints[1] = Id::from_bytes([2; 32]);
        custodies[0] = Id::from_bytes([3; 32]);
        custodies[1] = Id::from_bytes([3; 32]);
        assert_eq!(
            validate_outcome_custody_partition_v1(
                2,
                mints,
                custodies,
                Id::from_bytes([4; 32]),
                Id::from_bytes([5; 32]),
                Id::from_bytes([6; 32]),
                Id::from_bytes([7; 32]),
            ),
            Err(Error::MismatchedBinding),
        );
    }
}
