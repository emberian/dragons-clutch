// SPDX-License-Identifier: AGPL-3.0-or-later

use clutch_collateral_adapter_v2::{
    prepare_fractional_claim_ledger_founding_v3, prepare_fractional_claim_ledger_retirement_v3,
    prepare_fractional_claim_ledger_successor_v3, prepare_fractional_claim_redemption_v3,
    AcceptedClaimRedemptionCollateralV2, BoundClaimIssuanceV1, BoundCollateralProfileV2,
    ClaimLedgerV3, FractionalClaimLedgerFoundingPlanV3, FractionalClaimLedgerPlanV3,
    FractionalClaimLedgerRetirementPlanV3, FractionalClaimRedemptionPlanV3,
    FractionalClaimSupplyMutationV3, FractionalPayoutDispositionV3, HoardV2, Id,
    MarketLiabilityLifecycleV1,
};
use clutch_retirement::{
    DeletableRentOwnerV1, Identity32V1, PositionAccountV3, PositionLifecycleV3, PositionPurposeV3,
    PositionV3Fields, PositionV3Sha256Backend, RentSplitV2, ReplayV3Envelope, ReplayV3Lifecycle,
};
use sha2::{Digest, Sha256};

use crate::{
    Error, FractionalCreditTombstoneV1, FractionalCreditV1, FractionalLedgerPhaseV1,
    FractionalLedgerV1, FractionalPolicyV1, PayoutVectorV1, Result, MAX_OUTCOMES,
};

#[derive(Clone, Copy, Debug)]
struct FractionalSha256V1;

impl PositionV3Sha256Backend for FractionalSha256V1 {
    fn sha256(&self, domain: &[u8], body: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(domain);
        hasher.update(body);
        hasher.finalize().into()
    }
}

fn collateral_id(identity: Identity32V1) -> Id {
    Id::from_bytes(identity.bytes())
}

fn runtime_identity(identity: Id) -> Result<Identity32V1> {
    Identity32V1::new(identity.bytes()).map_err(|_| Error::ZeroIdentity)
}

fn map_collateral<T>(result: clutch_collateral_adapter_v2::Result<T>) -> Result<T> {
    result.map_err(|_| Error::CollateralRefused)
}

/// Fully joined fractional-redemption domain.
///
/// Private fields prevent a transition from accepting a policy, aggregate
/// ledger, Resolution projection, collateral release, or claim release that
/// has not passed the complete pure join in [`bind_fractional_context_v1`].
#[derive(Clone, Copy, Debug)]
pub struct BoundFractionalContextV1 {
    policy_account: Identity32V1,
    policy: FractionalPolicyV1,
    ledger_account: Identity32V1,
    ledger: FractionalLedgerV1,
    claim_ledger_account: Identity32V1,
    claim_ledger: ClaimLedgerV3,
    hoard: HoardV2,
    payout: PayoutVectorV1,
    collateral: BoundCollateralProfileV2,
    claims: BoundClaimIssuanceV1,
}

impl BoundFractionalContextV1 {
    /// Exact immutable policy PDA.
    pub const fn policy_account(self) -> Identity32V1 {
        self.policy_account
    }
    /// Validated immutable policy.
    pub const fn policy(self) -> FractionalPolicyV1 {
        self.policy
    }
    /// Exact aggregate ledger PDA.
    pub const fn ledger_account(self) -> Identity32V1 {
        self.ledger_account
    }
    /// Sole aggregate-credit owner.
    pub const fn ledger(self) -> FractionalLedgerV1 {
        self.ledger
    }
    /// Exact canonical ClaimLedger V3 account.
    pub const fn claim_ledger_account(self) -> Identity32V1 {
        self.claim_ledger_account
    }
    /// Canonical native-supply and cross-ledger latch owner.
    pub const fn claim_ledger(self) -> ClaimLedgerV3 {
        self.claim_ledger
    }
    /// Canonical locked-claim-principal owner.
    pub const fn hoard(self) -> HoardV2 {
        self.hoard
    }
    /// Ephemeral canonical Resolution projection.
    pub const fn payout(self) -> PayoutVectorV1 {
        self.payout
    }
    /// Realm-selected collateral capability.
    pub const fn collateral(self) -> BoundCollateralProfileV2 {
        self.collateral
    }
    /// Independent claim-issuance capability.
    pub const fn claims(self) -> BoundClaimIssuanceV1 {
        self.claims
    }

    /// Rebind the same immutable domain to one checked atomic ledger successor
    /// before planning another action in the same transaction.
    pub fn with_ledgers(
        self,
        ledger: FractionalLedgerV1,
        claim_ledger: ClaimLedgerV3,
        hoard: HoardV2,
    ) -> Result<Self> {
        ledger.validate_with_policy(self.policy_account, self.policy)?;
        validate_canonical_ledgers(
            self.policy_account,
            self.policy,
            self.ledger_account,
            ledger,
            self.claim_ledger_account,
            claim_ledger,
            hoard,
            self.collateral,
        )?;
        Ok(Self {
            ledger,
            claim_ledger,
            hoard,
            ..self
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_canonical_ledgers(
    policy_account: Identity32V1,
    policy: FractionalPolicyV1,
    ledger_account: Identity32V1,
    ledger: FractionalLedgerV1,
    claim_ledger_account: Identity32V1,
    claim_ledger: ClaimLedgerV3,
    hoard: HoardV2,
    collateral: BoundCollateralProfileV2,
) -> Result<()> {
    map_collateral(claim_ledger.validate())?;
    map_collateral(hoard.validate())?;
    if ledger.claim_ledger_account != claim_ledger_account
        || claim_ledger.market_instance_id != collateral_id(policy.market_instance)
        || claim_ledger.realm_id != collateral_id(policy.realm)
        || claim_ledger.fractional_policy_id != collateral_id(policy_account)
        || claim_ledger.fractional_ledger_account != collateral_id(ledger_account)
        || claim_ledger.resolution_account != collateral_id(policy.resolution_account)
        || claim_ledger.next_fractional_sequence != ledger.next_sequence
        || claim_ledger.lifecycle != MarketLiabilityLifecycleV1::Resolved
        || claim_ledger.outcome_count != policy.outcome_count
        || hoard.market_instance_id != collateral_id(policy.market_instance)
        || hoard.realm_id != collateral_id(policy.realm)
        || hoard.collateral_policy_id != collateral_id(policy.collateral_policy)
        || hoard.collateral_release_id != collateral_id(policy.collateral_release)
        || hoard.token_account.bytes() != collateral.market().hoard_token_account.bytes()
        || hoard.lifecycle != MarketLiabilityLifecycleV1::Resolved
        || hoard.outcome_count != policy.outcome_count
    {
        return Err(Error::MismatchedBinding);
    }
    Ok(())
}

/// Bind every semantic owner needed by one fractional action.
pub fn bind_fractional_context_v1(
    policy_account: Identity32V1,
    policy: FractionalPolicyV1,
    ledger_account: Identity32V1,
    ledger: FractionalLedgerV1,
    claim_ledger_account: Identity32V1,
    claim_ledger: ClaimLedgerV3,
    hoard: HoardV2,
    payout: PayoutVectorV1,
    collateral: BoundCollateralProfileV2,
    claims: BoundClaimIssuanceV1,
) -> Result<BoundFractionalContextV1> {
    policy.validate_join(payout, collateral, claims)?;
    ledger.validate_with_policy(policy_account, policy)?;
    if policy_account == ledger_account {
        return Err(Error::MismatchedBinding);
    }
    validate_canonical_ledgers(
        policy_account,
        policy,
        ledger_account,
        ledger,
        claim_ledger_account,
        claim_ledger,
        hoard,
        collateral,
    )?;
    Ok(BoundFractionalContextV1 {
        policy_account,
        policy,
        ledger_account,
        ledger,
        claim_ledger_account,
        claim_ledger,
        hoard,
        payout,
        collateral,
        claims,
    })
}

/// Atomic founding of the sole aggregate-credit owner and ClaimLedger latch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalInitializationPlanV1 {
    /// Newly created `0xa5/v1` aggregate-credit owner.
    pub ledger_after: FractionalLedgerV1,
    /// Canonical ClaimLedger V3 founding successor.
    pub claim_ledger: FractionalClaimLedgerFoundingPlanV3,
}

/// Initialize the sole aggregate-credit owner beside an authenticated policy.
#[allow(clippy::too_many_arguments)]
pub fn initialize_fractional_ledger_v1(
    policy_account: Identity32V1,
    policy: FractionalPolicyV1,
    ledger_account: Identity32V1,
    claim_ledger_account: Identity32V1,
    claim_ledger: ClaimLedgerV3,
    stored_bump: u8,
    rent: clutch_retirement::DeletableRentOwnerV1,
) -> Result<FractionalInitializationPlanV1> {
    policy.validate()?;
    if policy_account == ledger_account
        || policy_account == claim_ledger_account
        || ledger_account == claim_ledger_account
    {
        return Err(Error::MismatchedBinding);
    }
    map_collateral(claim_ledger.validate())?;
    if claim_ledger.market_instance_id != collateral_id(policy.market_instance)
        || claim_ledger.realm_id != collateral_id(policy.realm)
        || claim_ledger.fractional_policy_id != collateral_id(policy_account)
        || claim_ledger.fractional_ledger_account != collateral_id(ledger_account)
        || claim_ledger.resolution_account != collateral_id(policy.resolution_account)
        || claim_ledger.outcome_count != policy.outcome_count
        || claim_ledger.lifecycle != MarketLiabilityLifecycleV1::Resolved
        || claim_ledger.next_fractional_sequence != 0
        || !claim_ledger.last_fractional_transition_id.is_zero()
    {
        return Err(Error::MismatchedBinding);
    }
    let ledger = FractionalLedgerV1 {
        policy_account,
        claim_ledger_account,
        domain_generation: policy.domain_generation,
        next_sequence: 1,
        active_credit_accounts: 0,
        aggregate_credit_numerator: 0,
        phase: FractionalLedgerPhaseV1::Live,
        stored_bump,
        rent,
    };
    ledger.validate_with_policy(policy_account, policy)?;
    let ledger_after_id = collateral_id(ledger.state_id()?);
    let claim_ledger = map_collateral(prepare_fractional_claim_ledger_founding_v3(
        claim_ledger,
        ledger_after_id,
        &FractionalSha256V1,
    ))?;
    if claim_ledger.fractional_ledger_account() != collateral_id(ledger_account) {
        return Err(Error::MismatchedBinding);
    }
    Ok(FractionalInitializationPlanV1 {
        ledger_after: ledger,
        claim_ledger,
    })
}

/// Adapter-authenticated canonical internal Position/Replay source or target.
///
/// The adapter must authenticate the two account owners and PDAs before
/// constructing this projection. The runtime independently checks the exact
/// canonical bodies and their Market/Realm/policy/release/generation joins.
#[derive(Clone, Copy, Debug)]
pub struct InternalPositionV1<'a> {
    /// Canonical Position V3 account key.
    pub position_account: Identity32V1,
    /// Canonical Position V3 semantic body.
    pub position: PositionAccountV3,
    /// Exact Replay V3 account key.
    pub replay_account: Identity32V1,
    /// Hash-authenticated Replay V3 envelope and General-owned extension.
    pub replay: ReplayV3Envelope<'a>,
}

impl<'a> InternalPositionV1<'a> {
    fn validate(
        self,
        context: BoundFractionalContextV1,
        claimant: Identity32V1,
        expected_replay_sequence: u64,
    ) -> Result<()> {
        self.position
            .validate()
            .map_err(|_| Error::PositionRefused)?;
        let fields = self.position.fields();
        let replay = self.replay.header();
        if fields.purpose != PositionPurposeV3::General
            || fields.lifecycle != PositionLifecycleV3::Open
            || fields.market_instance_id != context.policy.market_instance
            || fields.realm_id != context.policy.realm
            || fields.collateral_policy_id != context.policy.collateral_policy
            || fields.collateral_release_id != context.policy.collateral_release
            || fields.owner != claimant
            || fields.replay_account != self.replay_account
            || replay.position_account() != self.position_account
            || replay.replay_account() != self.replay_account
            || replay.purpose() != PositionPurposeV3::General
            || replay.purpose_binding_id() != fields.purpose_binding_id
            || replay.position_generation() != fields.generation
            || replay.lifecycle() != ReplayV3Lifecycle::Live
            || replay.next_sequence() != expected_replay_sequence
        {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }
}

/// Adapter-authenticated direct bearer source and collateral payout identity.
///
/// Exact Token-2022 account bytes and deltas remain the claim/collateral
/// adapters' responsibility. This projection freezes the identities that the
/// atomic burn and payout must use; it is never persisted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BearerClaimSourceV1 {
    /// Claimant authorizing the claim burn and owning any numerator credit.
    pub claimant: Identity32V1,
    /// Exact bearer Egg token account.
    pub claim_token_account: Identity32V1,
    /// Exact native outcome mint.
    pub claim_mint: Identity32V1,
    /// Realm-collateral destination account for whole-atom payout.
    pub collateral_destination: Identity32V1,
    /// Exact independent claim-issuance binding.
    pub claim_issuance_binding: Identity32V1,
    /// Pre-CPI bearer balance authenticated by the Token-2022 adapter.
    pub source_claim_atoms: u64,
    /// Accepted external collateral movement for a nonzero payout. This is
    /// `None` exactly when the computed payout is zero.
    pub accepted_collateral: Option<AcceptedClaimRedemptionCollateralV2>,
}

impl BearerClaimSourceV1 {
    fn validate(self, context: BoundFractionalContextV1, quantity: u64) -> Result<()> {
        if self.claim_issuance_binding.bytes() != context.claims.binding_id().bytes()
            || self.claim_issuance_binding != context.policy.claim_issuance_binding
            || self.source_claim_atoms < quantity
            || self.claim_token_account == self.claim_mint
            || self.claim_token_account == self.collateral_destination
            || self.claim_mint == self.collateral_destination
        {
            return Err(Error::ClaimPlaneRefused);
        }
        Ok(())
    }

    fn payout_disposition(self) -> FractionalPayoutDispositionV3 {
        FractionalPayoutDispositionV3::ExternalCustodyTransfer {
            accepted: self.accepted_collateral,
        }
    }
}

/// Exact requirement for advancing the General-owned purpose extension inside
/// canonical Replay V3. This crate does not invent a second Replay body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReplayAdvanceRequirementV1 {
    /// Exact Replay V3 account to advance.
    pub replay_account: Identity32V1,
    /// Sequence consumed by this action.
    pub consumed_sequence: u64,
    /// Required next sequence after the General purpose owner advances Replay.
    pub next_sequence: u64,
    /// Position generation retained by an ordinary balance mutation.
    pub position_generation: u64,
    /// General purpose binding whose extension must be updated.
    pub purpose_binding_id: Identity32V1,
    /// Exact canonical Position prestate semantic identity.
    pub position_prestate_semantic_id: Identity32V1,
    /// Exact canonical Position successor semantic identity.
    pub position_poststate_semantic_id: Identity32V1,
    /// Shared fractional/ClaimLedger/Hoard transition identity that Replay
    /// must persist for this Position mutation.
    pub transition_id: Identity32V1,
}

/// Internal Position V3 successor plus canonical Replay requirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InternalPayoutPoststateV1 {
    /// Exact Position V3 account written by the adapter.
    pub position_account: Identity32V1,
    /// Canonical Position V3 balance successor.
    pub position_after: PositionAccountV3,
    /// Required advance of the existing General Replay V3 extension.
    pub replay: ReplayAdvanceRequirementV1,
}

/// Bearer burn and collateral transfer requirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BearerPayoutPoststateV1 {
    /// Exact claimant signer.
    pub claimant: Identity32V1,
    /// Exact bearer source account.
    pub claim_token_account: Identity32V1,
    /// Exact outcome mint burned.
    pub claim_mint: Identity32V1,
    /// Exact raw bearer atoms burned.
    pub burn_atoms: u64,
    /// Realm Hoard selected through the authenticated collateral capability.
    pub collateral_hoard: Identity32V1,
    /// Exact claimant collateral destination.
    pub collateral_destination: Identity32V1,
    /// Whole collateral atoms transferred; zero is explicit and permitted.
    pub payout_atoms: u64,
    /// Atomic fractional/ClaimLedger/Hoard transition identity that the claim
    /// burn receipt must commit.
    pub transition_id: Identity32V1,
}

/// Source-specific atomic postcondition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RedemptionSourcePoststateV1 {
    /// Canonical internal Position/Replay mutation.
    Internal(InternalPayoutPoststateV1),
    /// Exact bearer burn plus collateral payout CPI requirement.
    Bearer(BearerPayoutPoststateV1),
}

/// One complete exact or credited redemption plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RedemptionPlanV1 {
    /// Sole aggregate-credit owner's exact successor.
    pub ledger_after: FractionalLedgerV1,
    /// Atomic canonical ClaimLedger/Hoard successor and cross-account IDs.
    pub custody_after: FractionalClaimRedemptionPlanV3,
    /// Live credit successor; absent only for the zero-state exact-lot path.
    pub credit_after: Option<FractionalCreditV1>,
    /// Whole collateral payout now.
    pub paid_atoms: u64,
    /// Canonical claimant numerator after the action.
    pub claimant_numerator_after: u64,
    /// Whether the quantity was a multiple of the policy's common lot.
    pub used_common_lot_fast_path: bool,
    /// Exact source-specific Position/Replay or Token-2022 postcondition.
    pub source_after: RedemptionSourcePoststateV1,
}

fn canonical_supply(claim_ledger: ClaimLedgerV3) -> Result<[u64; MAX_OUTCOMES]> {
    let mut supply = [0; MAX_OUTCOMES];
    let mut index = 0usize;
    while index < usize::from(claim_ledger.outcome_count) {
        supply[index] = claim_ledger.aggregate_internal_supply[index]
            .checked_add(claim_ledger.aggregate_materialized_supply[index])
            .ok_or(Error::Arithmetic)?;
        index += 1;
    }
    Ok(supply)
}

fn validate_canonical_solvency(
    payout: PayoutVectorV1,
    claim_ledger: ClaimLedgerV3,
    hoard: HoardV2,
    aggregate_credit: u128,
) -> Result<()> {
    payout.validate_solvency(
        canonical_supply(claim_ledger)?,
        hoard.locked_claim_principal_atoms,
        aggregate_credit,
    )
}

fn prepare_canonical_redemption(
    context: BoundFractionalContextV1,
    ledger_after: FractionalLedgerV1,
    consumed_sequence: u64,
    mutation: FractionalClaimSupplyMutationV3,
    paid_atoms: u64,
    disposition: FractionalPayoutDispositionV3,
) -> Result<FractionalClaimRedemptionPlanV3> {
    let before_id = collateral_id(context.ledger.state_id()?);
    let after_id = collateral_id(ledger_after.state_id()?);
    let plan = map_collateral(prepare_fractional_claim_redemption_v3(
        context.hoard,
        context.claim_ledger,
        before_id,
        after_id,
        consumed_sequence,
        mutation,
        paid_atoms,
        disposition,
        &FractionalSha256V1,
    ))?;
    let fractional = plan.fractional();
    if fractional.fractional_ledger_before_id() != before_id
        || fractional.fractional_ledger_after_id() != after_id
        || fractional.consumed_sequence() != consumed_sequence
    {
        return Err(Error::MismatchedBinding);
    }
    validate_canonical_solvency(
        context.payout,
        fractional.claim_ledger_after(),
        plan.hoard_after(),
        ledger_after.aggregate_credit_numerator,
    )?;
    Ok(plan)
}

fn prepare_canonical_claim_latch(
    context: BoundFractionalContextV1,
    ledger_after: FractionalLedgerV1,
    consumed_sequence: u64,
) -> Result<FractionalClaimLedgerPlanV3> {
    let before_id = collateral_id(context.ledger.state_id()?);
    let after_id = collateral_id(ledger_after.state_id()?);
    let plan = map_collateral(prepare_fractional_claim_ledger_successor_v3(
        context.claim_ledger,
        before_id,
        after_id,
        consumed_sequence,
        FractionalClaimSupplyMutationV3::Unchanged,
        &FractionalSha256V1,
    ))?;
    if plan.fractional_ledger_before_id() != before_id
        || plan.fractional_ledger_after_id() != after_id
        || plan.consumed_sequence() != consumed_sequence
    {
        return Err(Error::MismatchedBinding);
    }
    validate_canonical_solvency(
        context.payout,
        plan.claim_ledger_after(),
        context.hoard,
        ledger_after.aggregate_credit_numerator,
    )?;
    Ok(plan)
}

/// Fresh or permanent-tombstone-backed credit creation facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreditCreationV1 {
    /// Adapter proved the canonical credit PDA was an unallocated System account.
    Fresh {
        /// Exact claimant owner.
        claimant: Identity32V1,
        /// Canonical credit PDA bump.
        stored_bump: u8,
        /// Fully admitted live/tombstone rent split.
        rent: RentSplitV2,
    },
    /// Reopen atop the permanent tombstone at the same canonical PDA.
    Reopen {
        /// Exact authenticated tombstone.
        tombstone: FractionalCreditTombstoneV1,
        /// Reopen-admitted rent preserving the retained principal.
        rent: RentSplitV2,
    },
}

/// Live or atomically created destination credit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreditPrestateV1 {
    /// Existing exact owner credit.
    Live(FractionalCreditV1),
    /// Fresh or tombstone-backed creation in this same transaction.
    Create(CreditCreationV1),
}

fn open_credit(
    context: BoundFractionalContextV1,
    prestate: CreditPrestateV1,
    claimant: Identity32V1,
    expected_sequence: u64,
) -> Result<(FractionalCreditV1, u64)> {
    match prestate {
        CreditPrestateV1::Live(credit) => {
            credit.validate_with(
                context.policy_account,
                context.policy,
                context.ledger_account,
                context.ledger,
                context.payout,
            )?;
            if credit.claimant != claimant {
                return Err(Error::MismatchedBinding);
            }
            Ok((credit.advanced(expected_sequence)?, 0))
        }
        CreditPrestateV1::Create(CreditCreationV1::Fresh {
            claimant: created_claimant,
            stored_bump,
            rent,
        }) => {
            if claimant != created_claimant || expected_sequence != 1 {
                return Err(Error::MismatchedBinding);
            }
            rent.validate().map_err(|_| Error::RentRefused)?;
            Ok((
                FractionalCreditV1 {
                    policy_account: context.policy_account,
                    ledger_account: context.ledger_account,
                    market_instance: context.policy.market_instance,
                    resolution_account: context.policy.resolution_account,
                    payout_vector_id: context.policy.payout_vector_id,
                    claimant,
                    domain_generation: context.policy.domain_generation,
                    account_generation: 1,
                    next_sequence: 2,
                    numerator: 0,
                    stored_bump,
                    rent,
                },
                1,
            ))
        }
        CreditPrestateV1::Create(CreditCreationV1::Reopen { tombstone, rent }) => {
            tombstone.validate()?;
            rent.validate().map_err(|_| Error::RentRefused)?;
            if tombstone.policy_account != context.policy_account
                || tombstone.ledger_account != context.ledger_account
                || tombstone.market_instance != context.policy.market_instance
                || tombstone.resolution_account != context.policy.resolution_account
                || tombstone.payout_vector_id != context.policy.payout_vector_id
                || tombstone.claimant != claimant
                || tombstone.domain_generation != context.policy.domain_generation
                || tombstone.closed_next_sequence != expected_sequence
                || rent.permanent_tombstone_principal != tombstone.permanent_tombstone_principal
            {
                return Err(Error::TombstoneRequired);
            }
            Ok((
                FractionalCreditV1 {
                    policy_account: context.policy_account,
                    ledger_account: context.ledger_account,
                    market_instance: context.policy.market_instance,
                    resolution_account: context.policy.resolution_account,
                    payout_vector_id: context.policy.payout_vector_id,
                    claimant,
                    domain_generation: context.policy.domain_generation,
                    account_generation: tombstone
                        .account_generation
                        .checked_add(1)
                        .ok_or(Error::Arithmetic)?,
                    next_sequence: expected_sequence.checked_add(1).ok_or(Error::Arithmetic)?,
                    numerator: 0,
                    stored_bump: tombstone.stored_bump,
                    rent,
                },
                1,
            ))
        }
    }
}

fn checked_redemption(
    context: BoundFractionalContextV1,
    outcome: u8,
    quantity: u64,
    prior_credit: u64,
) -> Result<(u64, u64)> {
    if context.ledger.phase != FractionalLedgerPhaseV1::Live {
        return Err(Error::WrongPhase);
    }
    if quantity == 0 {
        return Err(Error::ZeroQuantity);
    }
    validate_canonical_solvency(
        context.payout,
        context.claim_ledger,
        context.hoard,
        context.ledger.aggregate_credit_numerator,
    )?;
    let index = usize::from(outcome);
    if index >= usize::from(context.payout.outcome_count) {
        return Err(Error::InvalidPayout);
    }
    if canonical_supply(context.claim_ledger)?[index] < quantity {
        return Err(Error::InsufficientClaims);
    }
    let numerator = u128::from(quantity)
        .checked_mul(u128::from(context.payout.weights[index]))
        .and_then(|value| value.checked_add(u128::from(prior_credit)))
        .ok_or(Error::Arithmetic)?;
    let denominator = u128::from(context.payout.denominator);
    let paid = u64::try_from(numerator / denominator).map_err(|_| Error::Arithmetic)?;
    let residue = u64::try_from(numerator % denominator).map_err(|_| Error::Arithmetic)?;
    Ok((paid, residue))
}

fn internal_poststate(
    context: BoundFractionalContextV1,
    source: InternalPositionV1<'_>,
    claimant: Identity32V1,
    expected_replay_sequence: u64,
    outcome_debit: Option<(u8, u64)>,
    paid_atoms: u64,
    transition_id: Identity32V1,
) -> Result<InternalPayoutPoststateV1> {
    source.validate(context, claimant, expected_replay_sequence)?;
    let old = source.position.fields();
    let mut eggs = old.native_eggs;
    if let Some((outcome, quantity)) = outcome_debit {
        let index = usize::from(outcome);
        eggs[index] = eggs[index]
            .checked_sub(quantity)
            .ok_or(Error::InsufficientClaims)?;
    }
    let position_after = PositionAccountV3::new(PositionV3Fields {
        cash_atoms: old
            .cash_atoms
            .checked_add(paid_atoms)
            .ok_or(Error::Arithmetic)?,
        native_eggs: eggs,
        ..old
    })
    .map_err(|_| Error::PositionRefused)?;
    let position_prestate_semantic_id = source
        .position
        .semantic_id(&FractionalSha256V1)
        .map_err(|_| Error::PositionRefused)?;
    let position_poststate_semantic_id = position_after
        .semantic_id(&FractionalSha256V1)
        .map_err(|_| Error::PositionRefused)?;
    if position_prestate_semantic_id == position_poststate_semantic_id {
        return Err(Error::PositionRefused);
    }
    Ok(InternalPayoutPoststateV1 {
        position_account: source.position_account,
        position_after,
        replay: ReplayAdvanceRequirementV1 {
            replay_account: source.replay_account,
            consumed_sequence: expected_replay_sequence,
            next_sequence: expected_replay_sequence
                .checked_add(1)
                .ok_or(Error::Arithmetic)?,
            position_generation: old.generation,
            purpose_binding_id: old.purpose_binding_id,
            position_prestate_semantic_id,
            position_poststate_semantic_id,
            transition_id,
        },
    })
}

fn bearer_poststate(
    context: BoundFractionalContextV1,
    source: BearerClaimSourceV1,
    quantity: u64,
    paid_atoms: u64,
    custody: FractionalClaimRedemptionPlanV3,
) -> Result<BearerPayoutPoststateV1> {
    source.validate(context, quantity)?;
    match (paid_atoms, source.accepted_collateral) {
        (0, None) => {}
        (0, Some(_)) | (_, None) => return Err(Error::CollateralRefused),
        (amount, Some(accepted)) => {
            let request = accepted.request();
            if request.claim_redemption_id != custody.fractional().transition_id()
                || request.destination_token_account != collateral_id(source.collateral_destination)
                || request.claim_semantic_owner != collateral_id(source.claimant)
                || request.payout_atoms != amount
            {
                return Err(Error::MismatchedBinding);
            }
        }
    }
    Ok(BearerPayoutPoststateV1 {
        claimant: source.claimant,
        claim_token_account: source.claim_token_account,
        claim_mint: source.claim_mint,
        burn_atoms: quantity,
        collateral_hoard: Identity32V1::new(
            context.collateral.market().hoard_token_account.bytes(),
        )
        .map_err(|_| Error::CollateralRefused)?,
        collateral_destination: source.collateral_destination,
        payout_atoms: paid_atoms,
        transition_id: runtime_identity(custody.receipt_id())?,
    })
}

fn redeem_exact_common(
    context: BoundFractionalContextV1,
    expected_ledger_sequence: u64,
    outcome: u8,
    quantity: u64,
    mutation: FractionalClaimSupplyMutationV3,
    disposition: FractionalPayoutDispositionV3,
    source_after: impl FnOnce(
        u64,
        FractionalClaimRedemptionPlanV3,
    ) -> Result<RedemptionSourcePoststateV1>,
) -> Result<RedemptionPlanV1> {
    let (paid_atoms, residue) = checked_redemption(context, outcome, quantity, 0)?;
    if residue != 0 {
        return Err(Error::NonIntegralLot);
    }
    let ledger_after = context.ledger.advanced(expected_ledger_sequence)?;
    let custody_after = prepare_canonical_redemption(
        context,
        ledger_after,
        expected_ledger_sequence,
        mutation,
        paid_atoms,
        disposition,
    )?;
    Ok(RedemptionPlanV1 {
        ledger_after,
        custody_after,
        credit_after: None,
        paid_atoms,
        claimant_numerator_after: 0,
        used_common_lot_fast_path: quantity.is_multiple_of(context.policy.common_lot),
        source_after: source_after(paid_atoms, custody_after)?,
    })
}

/// Redeem an exact internal lot without creating claimant-credit state.
pub fn redeem_internal_exact_v1(
    context: BoundFractionalContextV1,
    expected_ledger_sequence: u64,
    expected_replay_sequence: u64,
    source: InternalPositionV1<'_>,
    outcome: u8,
    quantity: u64,
) -> Result<RedemptionPlanV1> {
    let claimant = source.position.owner();
    redeem_exact_common(
        context,
        expected_ledger_sequence,
        outcome,
        quantity,
        FractionalClaimSupplyMutationV3::BurnInternal {
            outcome,
            amount: quantity,
        },
        FractionalPayoutDispositionV3::InternalPositionCash,
        |paid, custody| {
            Ok(RedemptionSourcePoststateV1::Internal(internal_poststate(
                context,
                source,
                claimant,
                expected_replay_sequence,
                Some((outcome, quantity)),
                paid,
                runtime_identity(custody.receipt_id())?,
            )?))
        },
    )
}

/// Redeem an exact bearer lot without creating claimant-credit state.
pub fn redeem_bearer_exact_v1(
    context: BoundFractionalContextV1,
    expected_ledger_sequence: u64,
    source: BearerClaimSourceV1,
    outcome: u8,
    quantity: u64,
) -> Result<RedemptionPlanV1> {
    redeem_exact_common(
        context,
        expected_ledger_sequence,
        outcome,
        quantity,
        FractionalClaimSupplyMutationV3::BurnMaterialized {
            outcome,
            amount: quantity,
        },
        source.payout_disposition(),
        |paid, custody| {
            Ok(RedemptionSourcePoststateV1::Bearer(bearer_poststate(
                context, source, quantity, paid, custody,
            )?))
        },
    )
}

fn redeem_with_credit(
    context: BoundFractionalContextV1,
    expected_ledger_sequence: u64,
    expected_credit_sequence: u64,
    credit_prestate: CreditPrestateV1,
    claimant: Identity32V1,
    outcome: u8,
    quantity: u64,
    mutation: FractionalClaimSupplyMutationV3,
    disposition: FractionalPayoutDispositionV3,
    source_after: impl FnOnce(
        u64,
        FractionalClaimRedemptionPlanV3,
    ) -> Result<RedemptionSourcePoststateV1>,
) -> Result<RedemptionPlanV1> {
    let (mut credit_after, created_count) =
        open_credit(context, credit_prestate, claimant, expected_credit_sequence)?;
    let prior = credit_after.numerator;
    let (paid_atoms, residue) = checked_redemption(context, outcome, quantity, prior)?;
    credit_after.numerator = residue;
    let mut ledger_after = context.ledger.advanced(expected_ledger_sequence)?;
    ledger_after.active_credit_accounts = ledger_after
        .active_credit_accounts
        .checked_add(created_count)
        .ok_or(Error::Arithmetic)?;
    ledger_after.aggregate_credit_numerator = ledger_after
        .aggregate_credit_numerator
        .checked_sub(u128::from(prior))
        .and_then(|value| value.checked_add(u128::from(residue)))
        .ok_or(Error::AggregateMismatch)?;
    credit_after.validate_with(
        context.policy_account,
        context.policy,
        context.ledger_account,
        ledger_after,
        context.payout,
    )?;
    let custody_after = prepare_canonical_redemption(
        context,
        ledger_after,
        expected_ledger_sequence,
        mutation,
        paid_atoms,
        disposition,
    )?;
    Ok(RedemptionPlanV1 {
        ledger_after,
        custody_after,
        credit_after: Some(credit_after),
        paid_atoms,
        claimant_numerator_after: residue,
        used_common_lot_fast_path: quantity.is_multiple_of(context.policy.common_lot),
        source_after: source_after(paid_atoms, custody_after)?,
    })
}

/// Burn arbitrary internal claims, pay whole atoms to Position cash, and retain
/// the exact claimant numerator in one owner-scoped credit.
pub fn redeem_internal_to_credit_v1(
    context: BoundFractionalContextV1,
    expected_ledger_sequence: u64,
    expected_credit_sequence: u64,
    expected_replay_sequence: u64,
    credit_prestate: CreditPrestateV1,
    source: InternalPositionV1<'_>,
    outcome: u8,
    quantity: u64,
) -> Result<RedemptionPlanV1> {
    let claimant = source.position.owner();
    redeem_with_credit(
        context,
        expected_ledger_sequence,
        expected_credit_sequence,
        credit_prestate,
        claimant,
        outcome,
        quantity,
        FractionalClaimSupplyMutationV3::BurnInternal {
            outcome,
            amount: quantity,
        },
        FractionalPayoutDispositionV3::InternalPositionCash,
        |paid, custody| {
            Ok(RedemptionSourcePoststateV1::Internal(internal_poststate(
                context,
                source,
                claimant,
                expected_replay_sequence,
                Some((outcome, quantity)),
                paid,
                runtime_identity(custody.receipt_id())?,
            )?))
        },
    )
}

/// Burn arbitrary bearer claims, pay whole collateral atoms, and retain the
/// exact claimant numerator in one owner-scoped credit.
pub fn redeem_bearer_to_credit_v1(
    context: BoundFractionalContextV1,
    expected_ledger_sequence: u64,
    expected_credit_sequence: u64,
    credit_prestate: CreditPrestateV1,
    source: BearerClaimSourceV1,
    outcome: u8,
    quantity: u64,
) -> Result<RedemptionPlanV1> {
    redeem_with_credit(
        context,
        expected_ledger_sequence,
        expected_credit_sequence,
        credit_prestate,
        source.claimant,
        outcome,
        quantity,
        FractionalClaimSupplyMutationV3::BurnMaterialized {
            outcome,
            amount: quantity,
        },
        source.payout_disposition(),
        |paid, custody| {
            Ok(RedemptionSourcePoststateV1::Bearer(bearer_poststate(
                context, source, quantity, paid, custody,
            )?))
        },
    )
}

/// Destination that receives a whole collateral atom created by credit merge.
#[derive(Clone, Copy, Debug)]
pub enum CreditPayoutTargetV1<'a> {
    /// Credit Position V3 cash and advance its existing General Replay V3.
    Internal {
        /// Canonical Position and Replay.
        position: InternalPositionV1<'a>,
        /// Exact Replay sequence consumed by this payout.
        expected_replay_sequence: u64,
    },
    /// Transfer Realm collateral from Hoard to an exact claimant account.
    External {
        /// Destination claimant, which must own the destination credit.
        claimant: Identity32V1,
        /// Exact Realm collateral token account.
        collateral_destination: Identity32V1,
        /// Accepted external collateral movement for a nonzero payout.
        accepted_collateral: Option<AcceptedClaimRedemptionCollateralV2>,
    },
}

impl CreditPayoutTargetV1<'_> {
    fn disposition(self) -> FractionalPayoutDispositionV3 {
        match self {
            Self::Internal { .. } => FractionalPayoutDispositionV3::InternalPositionCash,
            Self::External {
                accepted_collateral,
                ..
            } => FractionalPayoutDispositionV3::ExternalCustodyTransfer {
                accepted: accepted_collateral,
            },
        }
    }
}

fn credit_payout_poststate(
    context: BoundFractionalContextV1,
    target: CreditPayoutTargetV1<'_>,
    claimant: Identity32V1,
    paid_atoms: u64,
    custody: FractionalClaimRedemptionPlanV3,
) -> Result<CreditPayoutPoststateV1> {
    match target {
        CreditPayoutTargetV1::Internal {
            position,
            expected_replay_sequence,
        } => Ok(CreditPayoutPoststateV1::Internal(internal_poststate(
            context,
            position,
            claimant,
            expected_replay_sequence,
            None,
            paid_atoms,
            runtime_identity(custody.receipt_id())?,
        )?)),
        CreditPayoutTargetV1::External {
            claimant: target_claimant,
            collateral_destination,
            accepted_collateral,
        } => {
            if claimant != target_claimant {
                return Err(Error::MismatchedBinding);
            }
            match (paid_atoms, accepted_collateral) {
                (0, None) => {}
                (0, Some(_)) | (_, None) => return Err(Error::CollateralRefused),
                (amount, Some(accepted)) => {
                    let request = accepted.request();
                    if request.claim_redemption_id != custody.fractional().transition_id()
                        || request.destination_token_account
                            != collateral_id(collateral_destination)
                        || request.claim_semantic_owner != collateral_id(claimant)
                        || request.payout_atoms != amount
                    {
                        return Err(Error::MismatchedBinding);
                    }
                }
            }
            Ok(CreditPayoutPoststateV1::External {
                claimant,
                collateral_hoard: Identity32V1::new(
                    context.collateral.market().hoard_token_account.bytes(),
                )
                .map_err(|_| Error::CollateralRefused)?,
                collateral_destination,
                payout_atoms: paid_atoms,
            })
        }
    }
}

/// Exact whole-atom payout created by owner-credit aggregation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreditPayoutPoststateV1 {
    /// Canonical Position/Replay successor.
    Internal(InternalPayoutPoststateV1),
    /// Realm-selected Hoard transfer requirement.
    External {
        /// Destination claimant.
        claimant: Identity32V1,
        /// Exact Realm Hoard.
        collateral_hoard: Identity32V1,
        /// Exact Realm collateral destination.
        collateral_destination: Identity32V1,
        /// Whole collateral atoms transferred.
        payout_atoms: u64,
    },
}

/// Complete owner-credit transfer/merge plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreditTransferPlanV1 {
    /// Source credit successor.
    pub source_after: FractionalCreditV1,
    /// Destination credit successor.
    pub destination_after: FractionalCreditV1,
    /// Sole aggregate-credit owner successor.
    pub ledger_after: FractionalLedgerV1,
    /// Atomic canonical ClaimLedger/Hoard successor and cross-account IDs.
    pub custody_after: FractionalClaimRedemptionPlanV3,
    /// Whole collateral atoms created by aggregation.
    pub paid_atoms: u64,
    /// Exact atomic payout target.
    pub payout_after: CreditPayoutPoststateV1,
}

/// Transfer an explicit numerator amount between same-domain owner credits.
///
/// Destination claimant acceptance, any destination account creation, and the
/// whole-atom payout are one atomic plan. No numerator is tokenized or erased.
#[allow(clippy::too_many_arguments)]
pub fn transfer_credit_v1(
    context: BoundFractionalContextV1,
    expected_ledger_sequence: u64,
    source: FractionalCreditV1,
    expected_source_sequence: u64,
    destination: CreditPrestateV1,
    destination_claimant: Identity32V1,
    expected_destination_sequence: u64,
    numerator: u64,
    payout_target: CreditPayoutTargetV1<'_>,
) -> Result<CreditTransferPlanV1> {
    if numerator == 0 {
        return Err(Error::ZeroQuantity);
    }
    source.validate_with(
        context.policy_account,
        context.policy,
        context.ledger_account,
        context.ledger,
        context.payout,
    )?;
    if source.claimant == destination_claimant || source.numerator < numerator {
        return Err(Error::InsufficientCredit);
    }
    validate_canonical_solvency(
        context.payout,
        context.claim_ledger,
        context.hoard,
        context.ledger.aggregate_credit_numerator,
    )?;
    let mut source_after = source.advanced(expected_source_sequence)?;
    let (mut destination_after, created_count) = open_credit(
        context,
        destination,
        destination_claimant,
        expected_destination_sequence,
    )?;
    let accumulated = u128::from(destination_after.numerator)
        .checked_add(u128::from(numerator))
        .ok_or(Error::Arithmetic)?;
    let denominator = u128::from(context.payout.denominator);
    let paid_atoms = u64::try_from(accumulated / denominator).map_err(|_| Error::Arithmetic)?;
    let residue = u64::try_from(accumulated % denominator).map_err(|_| Error::Arithmetic)?;
    source_after.numerator = source_after
        .numerator
        .checked_sub(numerator)
        .ok_or(Error::InsufficientCredit)?;
    destination_after.numerator = residue;
    let mut ledger_after = context.ledger.advanced(expected_ledger_sequence)?;
    ledger_after.active_credit_accounts = ledger_after
        .active_credit_accounts
        .checked_add(created_count)
        .ok_or(Error::Arithmetic)?;
    ledger_after.aggregate_credit_numerator = ledger_after
        .aggregate_credit_numerator
        .checked_sub(
            u128::from(paid_atoms)
                .checked_mul(denominator)
                .ok_or(Error::Arithmetic)?,
        )
        .ok_or(Error::AggregateMismatch)?;
    source_after.validate_with(
        context.policy_account,
        context.policy,
        context.ledger_account,
        ledger_after,
        context.payout,
    )?;
    destination_after.validate_with(
        context.policy_account,
        context.policy,
        context.ledger_account,
        ledger_after,
        context.payout,
    )?;
    let custody_after = prepare_canonical_redemption(
        context,
        ledger_after,
        expected_ledger_sequence,
        FractionalClaimSupplyMutationV3::Unchanged,
        paid_atoms,
        payout_target.disposition(),
    )?;
    Ok(CreditTransferPlanV1 {
        source_after,
        destination_after,
        ledger_after,
        custody_after,
        paid_atoms,
        payout_after: credit_payout_poststate(
            context,
            payout_target,
            destination_claimant,
            paid_atoms,
            custody_after,
        )?,
    })
}

/// Merge the entire nonzero source residue into a destination credit.
#[allow(clippy::too_many_arguments)]
pub fn merge_credit_v1(
    context: BoundFractionalContextV1,
    expected_ledger_sequence: u64,
    source: FractionalCreditV1,
    expected_source_sequence: u64,
    destination: CreditPrestateV1,
    destination_claimant: Identity32V1,
    expected_destination_sequence: u64,
    payout_target: CreditPayoutTargetV1<'_>,
) -> Result<CreditTransferPlanV1> {
    let numerator = source.numerator;
    transfer_credit_v1(
        context,
        expected_ledger_sequence,
        source,
        expected_source_sequence,
        destination,
        destination_claimant,
        expected_destination_sequence,
        numerator,
        payout_target,
    )
}

/// Exact lamport disposition when a zero credit shrinks into its tombstone.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreditCloseFundingPlanV1 {
    /// Stored payer receiving refundable live principal.
    pub payer: Identity32V1,
    /// Exact live principal returned to the stored payer.
    pub payer_refund_lamports: u64,
    /// Principal retained in the permanent tombstone.
    pub tombstone_lamports: u64,
    /// Frozen Realm neutral sink.
    pub neutral_sink: Identity32V1,
    /// Hostile prefund plus later unsolicited lamports routed to neutral sink.
    pub neutral_lamports: u64,
}

/// Complete zero-credit close plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreditClosePlanV1 {
    /// Permanent replay-prevention successor at the same PDA.
    pub tombstone: FractionalCreditTombstoneV1,
    /// Sole aggregate-credit owner successor.
    pub ledger_after: FractionalLedgerV1,
    /// Canonical unchanged-supply ClaimLedger successor and cross-account IDs.
    pub claim_ledger_after: FractionalClaimLedgerPlanV3,
    /// Exact rent disposition; collateral/credit principal is never included.
    pub funding: CreditCloseFundingPlanV1,
}

/// Close only a zero-numerator credit into its permanent tombstone.
pub fn close_zero_credit_v1(
    context: BoundFractionalContextV1,
    expected_ledger_sequence: u64,
    credit: FractionalCreditV1,
    expected_credit_sequence: u64,
    actual_lamports: u64,
    neutral_sink: Identity32V1,
) -> Result<CreditClosePlanV1> {
    credit.validate_with(
        context.policy_account,
        context.policy,
        context.ledger_account,
        context.ledger,
        context.payout,
    )?;
    if credit.numerator != 0 {
        return Err(Error::CreditOutstanding);
    }
    let credit_after_sequence = credit.advanced(expected_credit_sequence)?.next_sequence;
    let rent = credit.rent;
    rent.validate().map_err(|_| Error::RentRefused)?;
    if rent.payer == neutral_sink {
        return Err(Error::RentRefused);
    }
    let principal = rent
        .refundable_live_principal
        .checked_add(rent.permanent_tombstone_principal)
        .ok_or(Error::Arithmetic)?;
    let floor = principal
        .checked_add(rent.donation_floor)
        .ok_or(Error::Arithmetic)?;
    if actual_lamports < floor {
        return Err(Error::RentRefused);
    }
    let mut ledger_after = context.ledger.advanced(expected_ledger_sequence)?;
    ledger_after.active_credit_accounts = ledger_after
        .active_credit_accounts
        .checked_sub(1)
        .ok_or(Error::AggregateMismatch)?;
    ledger_after.validate()?;
    let claim_ledger_after =
        prepare_canonical_claim_latch(context, ledger_after, expected_ledger_sequence)?;
    Ok(CreditClosePlanV1 {
        tombstone: FractionalCreditTombstoneV1 {
            policy_account: credit.policy_account,
            ledger_account: credit.ledger_account,
            market_instance: credit.market_instance,
            resolution_account: credit.resolution_account,
            payout_vector_id: credit.payout_vector_id,
            claimant: credit.claimant,
            domain_generation: credit.domain_generation,
            account_generation: credit.account_generation,
            closed_next_sequence: credit_after_sequence,
            stored_bump: credit.stored_bump,
            permanent_tombstone_principal: rent.permanent_tombstone_principal,
        },
        ledger_after,
        claim_ledger_after,
        funding: CreditCloseFundingPlanV1 {
            payer: rent.payer,
            payer_refund_lamports: rent.refundable_live_principal,
            tombstone_lamports: rent.permanent_tombstone_principal,
            neutral_sink,
            neutral_lamports: actual_lamports
                .checked_sub(principal)
                .ok_or(Error::RentRefused)?,
        },
    })
}

/// Exact terminal decomposition with no sweep recipient.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalFactsV1 {
    /// Weighted remaining native-claim numerator.
    pub weighted_claim_numerator: u128,
    /// Aggregate owner-credit numerator.
    pub aggregate_credit_numerator: u128,
    /// Whole atoms reachable by voluntary same-domain aggregation.
    pub aggregatable_credit_atoms: u128,
    /// Irreducible sub-atom numerator that must remain live.
    pub irreducible_credit_numerator: u64,
    /// Claim-backing atoms retained under the no-subsidy policy.
    pub claim_backing_atoms: u64,
    /// Whether complete protocol closure is exact right now.
    pub exactly_closable: bool,
}

/// Report terminal facts from canonical ledgers without assigning Hoard
/// principal to any recipient.
pub fn terminal_facts_v1(context: BoundFractionalContextV1) -> Result<TerminalFactsV1> {
    let supply = canonical_supply(context.claim_ledger)?;
    validate_canonical_solvency(
        context.payout,
        context.claim_ledger,
        context.hoard,
        context.ledger.aggregate_credit_numerator,
    )?;
    let claims = context.payout.weighted_liability(supply)?;
    let denominator = u128::from(context.payout.denominator);
    Ok(TerminalFactsV1 {
        weighted_claim_numerator: claims,
        aggregate_credit_numerator: context.ledger.aggregate_credit_numerator,
        aggregatable_credit_atoms: context.ledger.aggregate_credit_numerator / denominator,
        irreducible_credit_numerator: u64::try_from(
            context.ledger.aggregate_credit_numerator % denominator,
        )
        .map_err(|_| Error::Arithmetic)?,
        claim_backing_atoms: context.hoard.locked_claim_principal_atoms,
        exactly_closable: claims == 0
            && context.ledger.aggregate_credit_numerator == 0
            && context.ledger.active_credit_accounts == 0
            && context.hoard.locked_claim_principal_atoms == 0,
    })
}

/// Atomic claims-exhausted phase successor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SealClaimsExhaustedPlanV1 {
    /// Sole aggregate-credit owner successor.
    pub ledger_after: FractionalLedgerV1,
    /// Canonical zero-supply ClaimLedger latch successor.
    pub claim_ledger_after: FractionalClaimLedgerPlanV3,
}

/// Seal the aggregate ledger after canonical total supply reaches zero.
pub fn seal_claims_exhausted_v1(
    context: BoundFractionalContextV1,
    expected_ledger_sequence: u64,
) -> Result<SealClaimsExhaustedPlanV1> {
    if context.ledger.phase != FractionalLedgerPhaseV1::Live
        || canonical_supply(context.claim_ledger)? != [0; MAX_OUTCOMES]
    {
        return Err(Error::LiabilityOutstanding);
    }
    validate_canonical_solvency(
        context.payout,
        context.claim_ledger,
        context.hoard,
        context.ledger.aggregate_credit_numerator,
    )?;
    let ledger_after = FractionalLedgerV1 {
        phase: FractionalLedgerPhaseV1::ClaimsExhausted,
        ..context.ledger.advanced(expected_ledger_sequence)?
    };
    let claim_ledger_after =
        prepare_canonical_claim_latch(context, ledger_after, expected_ledger_sequence)?;
    Ok(SealClaimsExhaustedPlanV1 {
        ledger_after,
        claim_ledger_after,
    })
}

/// Exact rent-only close disposition for one deletable fractional account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalAccountCloseFundingV1 {
    account: Identity32V1,
    payer: Identity32V1,
    payer_refund_lamports: u64,
    neutral_sink: Identity32V1,
    neutral_lamports: u64,
}

impl FractionalAccountCloseFundingV1 {
    /// Exact account whose lamports are split by this disposition.
    pub const fn account(self) -> Identity32V1 {
        self.account
    }

    /// Stored payer receiving only refundable rent principal.
    pub const fn payer(self) -> Identity32V1 {
        self.payer
    }

    /// Exact stored principal refunded to the payer.
    pub const fn payer_refund_lamports(self) -> u64 {
        self.payer_refund_lamports
    }

    /// Frozen neutral sink receiving hostile or unsolicited lamports only.
    pub const fn neutral_sink(self) -> Identity32V1 {
        self.neutral_sink
    }

    /// Exact non-principal lamports routed to the neutral sink.
    pub const fn neutral_lamports(self) -> u64 {
        self.neutral_lamports
    }
}

fn prepare_fractional_account_close_funding(
    account: Identity32V1,
    rent: DeletableRentOwnerV1,
    actual_lamports: u64,
    neutral_sink: Identity32V1,
) -> Result<FractionalAccountCloseFundingV1> {
    rent.validate().map_err(crate::map_retirement)?;
    if rent.payer() == neutral_sink {
        return Err(Error::RentRefused);
    }
    let floor = rent
        .refundable_principal()
        .checked_add(rent.donation_floor())
        .ok_or(Error::Arithmetic)?;
    if actual_lamports < floor {
        return Err(Error::RentRefused);
    }
    Ok(FractionalAccountCloseFundingV1 {
        account,
        payer: rent.payer(),
        payer_refund_lamports: rent.refundable_principal(),
        neutral_sink,
        neutral_lamports: actual_lamports
            .checked_sub(rent.refundable_principal())
            .ok_or(Error::RentRefused)?,
    })
}

/// Exact terminal facts a ProductOccurrenceRoot authorization must bind before
/// either fractional-family account may be deleted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalDomainTerminalRequirementV1 {
    market_instance_id: Identity32V1,
    domain_generation: u64,
    policy_account: Identity32V1,
    policy_terminal_state_id: Identity32V1,
    ledger_account: Identity32V1,
    ledger_before_state_id: Identity32V1,
    ledger_terminal_state_id: Identity32V1,
    claim_ledger_account: Identity32V1,
    claim_ledger_post_state_id: Identity32V1,
    claim_ledger_transition_id: Identity32V1,
}

impl FractionalDomainTerminalRequirementV1 {
    /// Exact MarketInstanceV2 identity whose Product root must authorize close.
    pub const fn market_instance_id(self) -> Identity32V1 {
        self.market_instance_id
    }

    /// Exact nonzero fractional/Resolution generation being retired.
    pub const fn domain_generation(self) -> u64 {
        self.domain_generation
    }

    /// Physical immutable `0xa4/v1` account to delete.
    pub const fn policy_account(self) -> Identity32V1 {
        self.policy_account
    }

    /// Exact terminal immutable-policy state identity.
    pub const fn policy_terminal_state_id(self) -> Identity32V1 {
        self.policy_terminal_state_id
    }

    /// Physical aggregate `0xa5/v1` account to delete.
    pub const fn ledger_account(self) -> Identity32V1 {
        self.ledger_account
    }

    /// Exact last persisted aggregate-ledger state identity.
    pub const fn ledger_before_state_id(self) -> Identity32V1 {
        self.ledger_before_state_id
    }

    /// Exact transient terminal aggregate-ledger successor identity committed
    /// by ClaimLedger before deletion.
    pub const fn ledger_terminal_state_id(self) -> Identity32V1 {
        self.ledger_terminal_state_id
    }

    /// Physical canonical ClaimLedger V3 account advanced to Retiring.
    pub const fn claim_ledger_account(self) -> Identity32V1 {
        self.claim_ledger_account
    }

    /// Exact Retiring ClaimLedger semantic identity.
    pub const fn claim_ledger_post_state_id(self) -> Identity32V1 {
        self.claim_ledger_post_state_id
    }

    /// Shared terminal ClaimLedger/aggregate-ledger transition identity.
    pub const fn claim_ledger_transition_id(self) -> Identity32V1 {
        self.claim_ledger_transition_id
    }
}

/// Prepared terminal fractional-family close after all economic state is zero.
///
/// This pure value is not Product authorization. The disabled SBF adapter must
/// consume the matching private ProductOccurrenceRoot close authorization
/// before applying either deletion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EmptyLedgerClosePlanV1 {
    /// Canonical exhausted ClaimLedger successor committing the transient
    /// retirement-state `0xa5` ID before that account is deleted.
    claim_ledger_after: FractionalClaimLedgerRetirementPlanV3,
    terminal_requirement: FractionalDomainTerminalRequirementV1,
    policy_funding: FractionalAccountCloseFundingV1,
    ledger_funding: FractionalAccountCloseFundingV1,
}

impl EmptyLedgerClosePlanV1 {
    /// Canonical ClaimLedger retirement half of the atomic terminal transition.
    pub const fn claim_ledger_after(self) -> FractionalClaimLedgerRetirementPlanV3 {
        self.claim_ledger_after
    }

    /// Exact facts the private ProductOccurrenceRoot authorization must match.
    pub const fn terminal_requirement(self) -> FractionalDomainTerminalRequirementV1 {
        self.terminal_requirement
    }

    /// Independent rent-only disposition for immutable `0xa4/v1`.
    pub const fn policy_funding(self) -> FractionalAccountCloseFundingV1 {
        self.policy_funding
    }

    /// Independent rent-only disposition for aggregate `0xa5/v1`.
    pub const fn ledger_funding(self) -> FractionalAccountCloseFundingV1 {
        self.ledger_funding
    }
}

/// Prepare the atomic policy-and-ledger close only when no claims, credits, or
/// claim backing remain. In particular, donation surplus and the final backing
/// atom cannot be swept through this action.
pub fn close_empty_ledger_v1(
    context: BoundFractionalContextV1,
    expected_ledger_sequence: u64,
    actual_policy_lamports: u64,
    actual_ledger_lamports: u64,
    neutral_sink: Identity32V1,
) -> Result<EmptyLedgerClosePlanV1> {
    if context.ledger.phase != FractionalLedgerPhaseV1::ClaimsExhausted {
        return Err(Error::WrongPhase);
    }
    let facts = terminal_facts_v1(context)?;
    if !facts.exactly_closable {
        return Err(Error::LiabilityOutstanding);
    }
    let advanced = context.ledger.advanced(expected_ledger_sequence)?;
    if advanced.aggregate_credit_numerator != 0 || advanced.active_credit_accounts != 0 {
        return Err(Error::AggregateMismatch);
    }
    let ledger_before_state_id = context.ledger.state_id()?;
    let ledger_terminal_state_id = advanced.state_id()?;
    let claim_ledger_after = map_collateral(prepare_fractional_claim_ledger_retirement_v3(
        context.claim_ledger,
        collateral_id(ledger_before_state_id),
        collateral_id(ledger_terminal_state_id),
        expected_ledger_sequence,
        &FractionalSha256V1,
    ))?;
    let terminal_requirement = FractionalDomainTerminalRequirementV1 {
        market_instance_id: context.policy.market_instance,
        domain_generation: context.policy.domain_generation,
        policy_account: context.policy_account,
        policy_terminal_state_id: context.policy.state_id()?,
        ledger_account: context.ledger_account,
        ledger_before_state_id,
        ledger_terminal_state_id,
        claim_ledger_account: context.claim_ledger_account,
        claim_ledger_post_state_id: runtime_identity(claim_ledger_after.claim_ledger_after_id())?,
        claim_ledger_transition_id: runtime_identity(claim_ledger_after.transition_id())?,
    };
    Ok(EmptyLedgerClosePlanV1 {
        claim_ledger_after,
        terminal_requirement,
        policy_funding: prepare_fractional_account_close_funding(
            context.policy_account,
            context.policy.rent,
            actual_policy_lamports,
            neutral_sink,
        )?,
        ledger_funding: prepare_fractional_account_close_funding(
            context.ledger_account,
            advanced.rent,
            actual_ledger_lamports,
            neutral_sink,
        )?,
    })
}
