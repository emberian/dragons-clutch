// SPDX-License-Identifier: AGPL-3.0-or-later

use clutch_collateral_adapter_v2::{
    accept_fractional_external_claim_redemption_v3,
    accept_fractional_external_credit_payout_v3, prepare_fractional_claim_ledger_founding_v3,
    prepare_fractional_claim_ledger_retirement_v3, prepare_fractional_claim_ledger_successor_v3,
    prepare_fractional_claim_redemption_v3, prepare_fractional_external_claim_redemption_v3,
    prepare_fractional_external_credit_payout_v3,
    AcceptedBearerRedemptionCollateralV3, AcceptedClaimRedemptionCollateralV2,
    AcceptedFractionalBearerClaimBurnV3, BoundClaimIssuanceV1, BoundCollateralProfileV2,
    ClaimLedgerV3, ClaimRedemptionCollateralRequestV2, FractionalBindingStateV1,
    FractionalClaimLedgerFoundingPlanV3, FractionalClaimLedgerPlanV3,
    FractionalClaimLedgerRetirementPlanV3, FractionalClaimRedemptionPlanV3,
    FractionalClaimSupplyMutationV3, FractionalPayoutDispositionV3, HoardV2, Id,
    MarketLiabilityLifecycleV1, PreparedFractionalExternalClaimRedemptionV3,
    PreparedFractionalExternalCreditPayoutV3,
    ResolutionPayoutProjectionV5, ResolutionPayoutUnitBoundaryV5, ResolutionStateV5, ResolutionV5,
};
use clutch_general_v2_contract::{
    project_general_replay_transition_v1, GeneralPositionReplayPrestateV1,
    GeneralReplayTransitionKindV1, GeneralReplayTransitionPlanV1, Id32,
};
use clutch_retirement::{
    DeletableRentOwnerV1, Identity32V1, PositionAccountV3, PositionLifecycleV3,
    PositionPurposeV3, PositionV3Sha256Backend, RentSplitV2, ReplayV3HashBackend,
};
use sha2::{Digest, Sha256};

use crate::{
    Error, FractionalCreditTombstoneV2, FractionalCreditV2, FractionalLedgerPhaseV1,
    FractionalLedgerV1, FractionalPolicyV3, PayoutVectorV1, Result, MAX_OUTCOMES,
};

/// Semantic domain for the unique Fractional child admission consumed by the
/// Product five-family Market aggregator.
pub const FRACTIONAL_FAMILY_ADMISSION_RECEIPT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/fractional/family-admission/v1\0";
/// Semantic domain proving exact a4/a5/ClaimLedger founding postimages.
pub const FRACTIONAL_FAMILY_ADMISSION_POSTWRITE_DOMAIN_V1: &[u8] =
    b"dragons-clutch/fractional/family-admission-postwrite/v1\0";
/// Semantic domain for the exact terminal Fractional child receipt consumed by
/// the Product market-family aggregator.
pub const FRACTIONAL_FAMILY_TERMINAL_RECEIPT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/fractional/family-terminal/v1\0";
/// Semantic domain committing the two independent rent-return dispositions.
pub const FRACTIONAL_FAMILY_RENT_DISPOSITION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/fractional/family-rent-disposition/v1\0";
/// Semantic domain proving exact a4/a5/ClaimLedger terminal postimages before deletion.
pub const FRACTIONAL_FAMILY_TERMINAL_POSTWRITE_DOMAIN_V1: &[u8] =
    b"dragons-clutch/fractional/family-terminal-postwrite/v1\0";
/// Fractional-owned identity of one exact external owner-credit payout action.
pub const FRACTIONAL_EXTERNAL_CREDIT_TRANSITION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/fractional/external-credit-transition/v1\0";
/// Semantic domain for the private Dealer action-23 vector transition.
pub const DEALER_FACILITY_VECTOR_TRANSITION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/fractional/dealer-facility-vector-transition/v1\0";
/// Semantic domain for one exact live-credit to tombstone close.
pub const FRACTIONAL_CREDIT_CLOSE_TRANSITION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/fractional-redemption/credit-close-transition/v1\0";

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

impl ReplayV3HashBackend for FractionalSha256V1 {
    fn sha256_parts(&self, parts: &[&[u8]]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        for part in parts {
            hasher.update(part);
        }
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoundFractionalContextV1 {
    policy_account: Identity32V1,
    policy: FractionalPolicyV3,
    ledger_account: Identity32V1,
    ledger: FractionalLedgerV1,
    claim_ledger_account: Identity32V1,
    claim_ledger: ClaimLedgerV3,
    hoard: HoardV2,
    resolution: ResolutionV5,
    resolution_semantic_id: Identity32V1,
    resolution_data_id: Identity32V1,
    payout: PayoutVectorV1,
    collateral: BoundCollateralProfileV2,
    claims: Option<BoundClaimIssuanceV1>,
}

impl BoundFractionalContextV1 {
    /// Exact immutable policy PDA.
    pub const fn policy_account(self) -> Identity32V1 {
        self.policy_account
    }
    /// Validated immutable policy.
    pub const fn policy(self) -> FractionalPolicyV3 {
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
    /// Exact structurally bound Resolution V5 body.
    pub const fn resolution(self) -> ResolutionV5 {
        self.resolution
    }
    /// Body-only Resolution V5 semantic identity.
    pub const fn resolution_semantic_id(self) -> Identity32V1 {
        self.resolution_semantic_id
    }
    /// Exact Resolution V5 PDA-and-body identity persisted by policy/credits.
    pub const fn resolution_data_id(self) -> Identity32V1 {
        self.resolution_data_id
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
    pub const fn claims(self) -> Option<BoundClaimIssuanceV1> {
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
    policy: FractionalPolicyV3,
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
        || claim_ledger.fractional_binding != FractionalBindingStateV1::Latched
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
        || hoard.profile_id != collateral.market().profile
        || hoard.collateral_policy_id != collateral_id(policy.collateral_policy)
        || hoard.collateral_release_id != collateral_id(policy.collateral_release)
        || hoard.token_account.bytes() != collateral.market().hoard_token_account.bytes()
        || hoard.lifecycle != MarketLiabilityLifecycleV1::Resolved
        || hoard.outcome_count != policy.outcome_count
    {
        return Err(Error::MismatchedBinding);
    }
    if ledger.phase == FractionalLedgerPhaseV1::ClaimsExhausted
        && canonical_supply(claim_ledger)? != [0; MAX_OUTCOMES]
    {
        return Err(Error::LiabilityOutstanding);
    }
    Ok(())
}

/// Bind every semantic owner needed by one fractional action.
pub fn bind_fractional_context_v1(
    policy_account: Identity32V1,
    policy: FractionalPolicyV3,
    ledger_account: Identity32V1,
    ledger: FractionalLedgerV1,
    claim_ledger_account: Identity32V1,
    claim_ledger: ClaimLedgerV3,
    hoard: HoardV2,
    resolution: ResolutionV5,
    collateral: BoundCollateralProfileV2,
    claims: BoundClaimIssuanceV1,
) -> Result<BoundFractionalContextV1> {
    map_collateral(resolution.validate())?;
    if resolution.state != ResolutionStateV5::Finalized
        || resolution.facts.market_instance_id != collateral_id(policy.market_instance)
        || resolution.facts.native_claim_basis_id != claim_ledger.native_claim_basis_id
        || resolution.facts.generation != policy.domain_generation
        || resolution.facts.outcome_count != policy.outcome_count
    {
        return Err(Error::MismatchedBinding);
    }
    let payout = PayoutVectorV1::from_resolution_v5(resolution)?;
    let resolution_semantic_id = runtime_identity(
        resolution
            .semantic_id(&FractionalSha256V1)
            .map_err(|_| Error::MismatchedBinding)?,
    )?;
    let resolution_data_id = runtime_identity(
        resolution
            .data_id(collateral_id(policy.resolution_account))
            .map_err(|_| Error::MismatchedBinding)?,
    )?;
    policy.validate_join(payout, resolution_data_id, collateral, claims)?;
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
        resolution,
        resolution_semantic_id,
        resolution_data_id,
        payout,
        collateral,
        claims: Some(claims),
    })
}

/// Bind the canonical subset needed by an internal Position redemption.
///
/// This is deliberately not sufficient for a bearer action: the resulting
/// context carries no authenticated claim-program capability and every bearer
/// source refuses it. Native internal claims are instead authenticated by the
/// canonical Position V3, ClaimLedger V3, and purpose-owned GEN1 Replay.
#[allow(clippy::too_many_arguments)]
pub fn bind_fractional_internal_context_v1(
    policy_account: Identity32V1,
    policy: FractionalPolicyV3,
    ledger_account: Identity32V1,
    ledger: FractionalLedgerV1,
    claim_ledger_account: Identity32V1,
    claim_ledger: ClaimLedgerV3,
    hoard: HoardV2,
    resolution: ResolutionV5,
    collateral: BoundCollateralProfileV2,
) -> Result<BoundFractionalContextV1> {
    map_collateral(resolution.validate())?;
    if resolution.state != ResolutionStateV5::Finalized
        || resolution.facts.market_instance_id != collateral_id(policy.market_instance)
        || resolution.facts.native_claim_basis_id != claim_ledger.native_claim_basis_id
        || resolution.facts.generation != policy.domain_generation
        || resolution.facts.outcome_count != policy.outcome_count
    {
        return Err(Error::MismatchedBinding);
    }
    let payout = PayoutVectorV1::from_resolution_v5(resolution)?;
    let resolution_semantic_id = runtime_identity(
        resolution
            .semantic_id(&FractionalSha256V1)
            .map_err(|_| Error::MismatchedBinding)?,
    )?;
    let resolution_data_id = runtime_identity(
        resolution
            .data_id(collateral_id(policy.resolution_account))
            .map_err(|_| Error::MismatchedBinding)?,
    )?;
    policy.validate_internal_join(payout, resolution_data_id, collateral)?;
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
        resolution,
        resolution_semantic_id,
        resolution_data_id,
        payout,
        collateral,
        claims: None,
    })
}

/// Atomic founding of the sole aggregate-credit owner and ClaimLedger latch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalInitializationPlanV1 {
    /// Newly created `0xa5/v1` aggregate-credit owner.
    pub ledger_after: FractionalLedgerV1,
    /// Canonical ClaimLedger V3 founding successor.
    pub claim_ledger: FractionalClaimLedgerFoundingPlanV3,
    /// Exact child receipt presented to the Product five-family aggregator.
    pub family_admission: FractionalFamilyAdmissionReceiptV1,
}

/// Private-field receipt proving one exact a4/a5/ClaimLedger founding latch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalFamilyAdmissionReceiptV1 {
    market_instance: Identity32V1,
    domain_generation: u64,
    claim_issuance_binding: Identity32V1,
    policy_account: Identity32V1,
    policy_state_id: Identity32V1,
    ledger_account: Identity32V1,
    ledger_state_id: Identity32V1,
    claim_ledger_account: Identity32V1,
    claim_ledger_before_id: Identity32V1,
    claim_ledger_after_id: Identity32V1,
    latch_transition_id: Identity32V1,
    receipt_id: Identity32V1,
}

impl FractionalFamilyAdmissionReceiptV1 {
    /// Full shared Market identity.
    pub const fn market_instance(self) -> Identity32V1 {
        self.market_instance
    }
    /// Exact nonzero fractional domain generation.
    pub const fn domain_generation(self) -> u64 {
        self.domain_generation
    }
    /// Exact canonical claim-issuance binding persisted by Product Foundation.
    pub const fn claim_issuance_binding(self) -> Identity32V1 {
        self.claim_issuance_binding
    }
    /// Exact immutable a4/v3 physical account.
    pub const fn policy_account(self) -> Identity32V1 {
        self.policy_account
    }
    /// Immutable a4/v3 state identity.
    pub const fn policy_state_id(self) -> Identity32V1 {
        self.policy_state_id
    }
    /// Exact a5/v1 physical account.
    pub const fn ledger_account(self) -> Identity32V1 {
        self.ledger_account
    }
    /// Founding a5/v1 state identity.
    pub const fn ledger_state_id(self) -> Identity32V1 {
        self.ledger_state_id
    }
    /// Exact canonical ClaimLedger V3 physical account.
    pub const fn claim_ledger_account(self) -> Identity32V1 {
        self.claim_ledger_account
    }
    /// ClaimLedger semantic identity before the one-way latch.
    pub const fn claim_ledger_before_id(self) -> Identity32V1 {
        self.claim_ledger_before_id
    }
    /// ClaimLedger semantic identity after the one-way latch.
    pub const fn claim_ledger_after_id(self) -> Identity32V1 {
        self.claim_ledger_after_id
    }
    /// Atomic ClaimLedger/a5 founding transition identity.
    pub const fn latch_transition_id(self) -> Identity32V1 {
        self.latch_transition_id
    }
    /// Unique receipt identity consumed by Product family admission.
    pub const fn receipt_id(self) -> Identity32V1 {
        self.receipt_id
    }
}

/// Structurally verified a4/a5/ClaimLedger founding postimages.
///
/// This pure value is not Solana account authority. The adapter must decode
/// the three exact program-owned postwrite accounts and keep its authenticated
/// wrapper private before Product may consume the embedded family receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedFractionalFamilyAdmissionPostwriteV1 {
    family_admission: FractionalFamilyAdmissionReceiptV1,
    verification_id: Identity32V1,
}

impl VerifiedFractionalFamilyAdmissionPostwriteV1 {
    /// Exact Fractional-owned receipt consumed by Product family admission.
    pub const fn family_admission(self) -> FractionalFamilyAdmissionReceiptV1 {
        self.family_admission
    }

    /// Commitment to the three exact canonical postimages.
    pub const fn verification_id(self) -> Identity32V1 {
        self.verification_id
    }
}

/// Verify the exact founding postimages before Product admits the Fractional child.
///
/// Physical owner, PDA, writable, allocation, rent, and framed-byte checks
/// remain the SBF adapter's responsibility. This function prevents that
/// adapter from promoting caller-shaped decoded values or a mismatched latch.
#[allow(clippy::too_many_arguments)]
pub fn verify_fractional_family_admission_postwrite_v1(
    plan: FractionalInitializationPlanV1,
    policy_account: Identity32V1,
    policy: FractionalPolicyV3,
    ledger_account: Identity32V1,
    ledger: FractionalLedgerV1,
    claim_ledger_account: Identity32V1,
    claim_ledger: ClaimLedgerV3,
) -> Result<VerifiedFractionalFamilyAdmissionPostwriteV1> {
    policy.validate()?;
    ledger.validate_with_policy(policy_account, policy)?;
    map_collateral(claim_ledger.validate())?;
    let receipt = plan.family_admission;
    let policy_state_id = policy.state_id()?;
    let ledger_state_id = ledger.state_id()?;
    let claim_ledger_after_id = runtime_identity(
        claim_ledger
            .semantic_id(&FractionalSha256V1)
            .map_err(|_| Error::MismatchedBinding)?,
    )?;
    if ledger != plan.ledger_after
        || claim_ledger != plan.claim_ledger.claim_ledger_after()
        || policy_account != receipt.policy_account
        || policy.market_instance != receipt.market_instance
        || policy.domain_generation != receipt.domain_generation
        || policy.claim_issuance_binding != receipt.claim_issuance_binding
        || policy_state_id != receipt.policy_state_id
        || ledger_account != receipt.ledger_account
        || ledger_state_id != receipt.ledger_state_id
        || claim_ledger_account != receipt.claim_ledger_account
        || claim_ledger_after_id != receipt.claim_ledger_after_id
        || claim_ledger.fractional_binding != FractionalBindingStateV1::Latched
        || claim_ledger.fractional_policy_id != collateral_id(policy_account)
        || claim_ledger.fractional_ledger_account != collateral_id(ledger_account)
        || claim_ledger.last_fractional_transition_id
            != collateral_id(receipt.latch_transition_id)
        || plan.claim_ledger.claim_ledger_before_id()
            != collateral_id(receipt.claim_ledger_before_id)
        || plan.claim_ledger.fractional_policy_id() != collateral_id(policy_account)
        || plan.claim_ledger.fractional_ledger_account() != collateral_id(ledger_account)
        || plan.claim_ledger.fractional_ledger_after_id() != collateral_id(ledger_state_id)
        || plan.claim_ledger.claim_ledger_after_id() != collateral_id(claim_ledger_after_id)
        || plan.claim_ledger.transition_id() != collateral_id(receipt.latch_transition_id)
    {
        return Err(Error::MismatchedBinding);
    }
    let mut hasher = Sha256::new();
    hasher.update(FRACTIONAL_FAMILY_ADMISSION_POSTWRITE_DOMAIN_V1);
    for identity in [
        receipt.receipt_id,
        policy_account,
        policy_state_id,
        ledger_account,
        ledger_state_id,
        claim_ledger_account,
        receipt.claim_ledger_before_id,
        claim_ledger_after_id,
        receipt.latch_transition_id,
    ] {
        hasher.update(identity.bytes());
    }
    let verification_id =
        Identity32V1::new(hasher.finalize().into()).map_err(|_| Error::ZeroIdentity)?;
    Ok(VerifiedFractionalFamilyAdmissionPostwriteV1 {
        family_admission: receipt,
        verification_id,
    })
}

/// Initialize the sole aggregate-credit owner beside an authenticated policy.
#[allow(clippy::too_many_arguments)]
pub fn initialize_fractional_ledger_v1(
    policy_account: Identity32V1,
    policy: FractionalPolicyV3,
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
    if claim_ledger.fractional_binding != FractionalBindingStateV1::OpenUnlatched
        || claim_ledger.market_instance_id != collateral_id(policy.market_instance)
        || claim_ledger.realm_id != collateral_id(policy.realm)
        || !claim_ledger.fractional_policy_id.is_zero()
        || !claim_ledger.fractional_ledger_account.is_zero()
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
        collateral_id(policy_account),
        collateral_id(ledger_account),
        ledger_after_id,
        &FractionalSha256V1,
    ))?;
    if claim_ledger.fractional_policy_id() != collateral_id(policy_account)
        || claim_ledger.fractional_ledger_account() != collateral_id(ledger_account)
    {
        return Err(Error::MismatchedBinding);
    }
    let policy_state_id = policy.state_id()?;
    let ledger_state_id = ledger.state_id()?;
    let claim_ledger_before_id = runtime_identity(claim_ledger.claim_ledger_before_id())?;
    let claim_ledger_after_id = runtime_identity(claim_ledger.claim_ledger_after_id())?;
    let latch_transition_id = runtime_identity(claim_ledger.transition_id())?;
    let mut hasher = Sha256::new();
    hasher.update(FRACTIONAL_FAMILY_ADMISSION_RECEIPT_DOMAIN_V1);
    for identity in [
        policy.market_instance,
        policy.claim_issuance_binding,
        policy_account,
        policy_state_id,
        ledger_account,
        ledger_state_id,
        claim_ledger_account,
        claim_ledger_before_id,
        claim_ledger_after_id,
        latch_transition_id,
    ] {
        hasher.update(identity.bytes());
    }
    hasher.update(policy.domain_generation.to_le_bytes());
    let receipt_id =
        Identity32V1::new(hasher.finalize().into()).map_err(|_| Error::ZeroIdentity)?;
    let family_admission = FractionalFamilyAdmissionReceiptV1 {
        market_instance: policy.market_instance,
        domain_generation: policy.domain_generation,
        claim_issuance_binding: policy.claim_issuance_binding,
        policy_account,
        policy_state_id,
        ledger_account,
        ledger_state_id,
        claim_ledger_account,
        claim_ledger_before_id,
        claim_ledger_after_id,
        latch_transition_id,
        receipt_id,
    };
    Ok(FractionalInitializationPlanV1 {
        ledger_after: ledger,
        claim_ledger,
        family_admission,
    })
}

/// Adapter-authenticated canonical internal Position/Replay source or target.
///
/// The adapter must authenticate the two account owners and PDAs before
/// constructing this projection. The runtime independently checks the exact
/// canonical bodies and their Market/Realm/policy/release/generation joins.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InternalPositionV1 {
    /// Canonical Position V3 and purpose-owned GEN1 Replay prestate.
    pub position_replay: GeneralPositionReplayPrestateV1,
}

impl InternalPositionV1 {
    fn validate(
        self,
        context: BoundFractionalContextV1,
        claimant: Identity32V1,
        expected_replay_sequence: u64,
    ) -> Result<()> {
        let position = self.position_replay.position();
        position
            .validate_writable()
            .map_err(|_| Error::PositionRefused)?;
        let fields = position.semantic.fields();
        if fields.market_instance_id != context.policy.market_instance
            || fields.realm_id != context.policy.realm
            || fields.collateral_policy_id != context.policy.collateral_policy
            || fields.collateral_release_id != context.policy.collateral_release
            || fields.owner != claimant
            || self.position_replay.next_sequence() != expected_replay_sequence
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
    /// Exact Token-2022 mint supplies observed for every active outcome.
    pub observed_materialized_supply: [u64; MAX_OUTCOMES],
    /// Accepted external collateral movement for a nonzero payout. This is
    /// `None` exactly when the computed payout is zero.
    pub accepted_collateral: Option<AcceptedClaimRedemptionCollateralV2>,
}

impl BearerClaimSourceV1 {
    fn validate(self, context: BoundFractionalContextV1, quantity: u64) -> Result<()> {
        let claims = context.claims.ok_or(Error::ClaimPlaneRefused)?;
        if self.claim_issuance_binding.bytes() != claims.binding_id().bytes()
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

/// Adapter-observed bearer prestate for the two-phase exact route.
///
/// This is a structural input, never authority. The independent claim adapter
/// must reauthenticate every field and return an accepted burn capability
/// before the collateral request can be exposed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BearerClaimPrestateV1 {
    /// Exact claimant signer and semantic owner.
    pub claimant: Identity32V1,
    /// Exact bearer source token account.
    pub claim_token_account: Identity32V1,
    /// Exact selected native-outcome mint.
    pub claim_mint: Identity32V1,
    /// Exact collateral payout destination.
    pub collateral_destination: Identity32V1,
    /// Exact persisted independent claim-issuance binding.
    pub claim_issuance_binding: Identity32V1,
    /// Authenticated source balance before the burn.
    pub source_claim_atoms: u64,
    /// Authenticated Token-2022 mint supplies before the burn.
    pub observed_materialized_supply: [u64; MAX_OUTCOMES],
}

impl BearerClaimPrestateV1 {
    fn validate(self, context: BoundFractionalContextV1, outcome: u8, quantity: u64) -> Result<()> {
        let claims = context.claims.ok_or(Error::ClaimPlaneRefused)?;
        if self.claim_issuance_binding.bytes() != claims.binding_id().bytes()
            || self.claim_issuance_binding != context.policy.claim_issuance_binding
            || self.source_claim_atoms < quantity
            || self.claim_token_account == self.claim_mint
            || self.claim_token_account == self.collateral_destination
            || self.claim_mint == self.collateral_destination
            || outcome >= context.policy.outcome_count
            || self.observed_materialized_supply[usize::from(outcome)] < quantity
        {
            return Err(Error::ClaimPlaneRefused);
        }
        let mut index = usize::from(context.policy.outcome_count);
        while index < MAX_OUTCOMES {
            if self.observed_materialized_supply[index] != 0 {
                return Err(Error::ClaimPlaneRefused);
            }
            index += 1;
        }
        Ok(())
    }
}

/// Internal Position V3 successor plus canonical GEN1 Replay successor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InternalPayoutPoststateV1 {
    /// Exact Position V3 account written by the adapter.
    pub position_account: Identity32V1,
    /// Canonical Position V3 balance successor.
    pub position_after: PositionAccountV3,
    /// Exact purpose-owned GEN1 Replay V3 body and semantic successor.
    pub replay: GeneralReplayTransitionPlanV1,
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
    /// Exact accepted independent claim-release burn receipt. Legacy pure
    /// projections carry `None`; the executable two-phase route requires it.
    pub burn_receipt_id: Option<Identity32V1>,
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
    /// Exact Resolution V5 identity, burn quantity, quotient, and raw remainder.
    pub resolution_payout: ResolutionPayoutProjectionV5,
    /// Sole aggregate-credit owner's exact successor.
    pub ledger_after: FractionalLedgerV1,
    /// Atomic canonical ClaimLedger/Hoard successor and cross-account IDs.
    pub custody_after: FractionalClaimRedemptionPlanV3,
    /// Live credit successor; absent only for the zero-state exact-lot path.
    pub credit_after: Option<FractionalCreditV2>,
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
        tombstone: FractionalCreditTombstoneV2,
        /// Reopen-admitted rent preserving the retained principal.
        rent: RentSplitV2,
    },
}

/// Live or atomically created destination credit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreditPrestateV1 {
    /// Existing exact owner credit.
    Live(FractionalCreditV2),
    /// Fresh or tombstone-backed creation in this same transaction.
    Create(CreditCreationV1),
}

fn open_credit(
    context: BoundFractionalContextV1,
    prestate: CreditPrestateV1,
    claimant: Identity32V1,
    expected_sequence: u64,
) -> Result<(FractionalCreditV2, u64)> {
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
                FractionalCreditV2 {
                    policy_account: context.policy_account,
                    ledger_account: context.ledger_account,
                    market_instance: context.policy.market_instance,
                    resolution_account: context.policy.resolution_account,
                    resolution_data_id: context.policy.resolution_data_id,
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
                || tombstone.resolution_data_id != context.policy.resolution_data_id
                || tombstone.claimant != claimant
                || tombstone.domain_generation != context.policy.domain_generation
                || tombstone.closed_next_sequence != expected_sequence
                || rent.permanent_tombstone_principal != tombstone.permanent_tombstone_principal
            {
                return Err(Error::TombstoneRequired);
            }
            Ok((
                FractionalCreditV2 {
                    policy_account: context.policy_account,
                    ledger_account: context.ledger_account,
                    market_instance: context.policy.market_instance,
                    resolution_account: context.policy.resolution_account,
                    resolution_data_id: context.policy.resolution_data_id,
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
) -> Result<(u64, u64, ResolutionPayoutProjectionV5)> {
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
    let resolution_payout = map_collateral(context.resolution.payout_projection(
        collateral_id(context.policy.resolution_account),
        outcome,
        quantity,
        &FractionalSha256V1,
    ))?;
    if resolution_payout.resolution_semantic_id().bytes() != context.resolution_semantic_id.bytes()
        || resolution_payout.resolution_data_id().bytes() != context.resolution_data_id.bytes()
        || resolution_payout.market_instance_id() != collateral_id(context.policy.market_instance)
        || resolution_payout.native_claim_basis_id() != context.claim_ledger.native_claim_basis_id
        || resolution_payout.generation() != context.policy.domain_generation
        || resolution_payout.outcome() != outcome
        || resolution_payout.quantity() != quantity
        || resolution_payout.payout_weight() != context.payout.weights[index]
        || resolution_payout.denominator() != context.payout.denominator
        || resolution_payout.payout_unit_boundary()
            != ResolutionPayoutUnitBoundaryV5::ExactWholeCollateralAtoms
    {
        return Err(Error::MismatchedBinding);
    }
    let denominator = u128::from(resolution_payout.denominator());
    let numerator = u128::from(resolution_payout.whole_atoms())
        .checked_mul(denominator)
        .and_then(|value| value.checked_add(u128::from(resolution_payout.remainder_numerator())))
        .and_then(|value| value.checked_add(u128::from(prior_credit)))
        .ok_or(Error::Arithmetic)?;
    let paid = u64::try_from(numerator / denominator).map_err(|_| Error::Arithmetic)?;
    let residue = u64::try_from(numerator % denominator).map_err(|_| Error::Arithmetic)?;
    Ok((paid, residue, resolution_payout))
}

fn internal_poststate(
    context: BoundFractionalContextV1,
    source: InternalPositionV1,
    claimant: Identity32V1,
    expected_replay_sequence: u64,
    outcome_debit: Option<(u8, u64)>,
    paid_atoms: u64,
    replay_kind: GeneralReplayTransitionKindV1,
    custody: FractionalClaimRedemptionPlanV3,
) -> Result<InternalPayoutPoststateV1> {
    source.validate(context, claimant, expected_replay_sequence)?;
    let position = source.position_replay.position();
    let old = position.semantic.fields();
    let mut eggs = old.native_eggs;
    if let Some((outcome, quantity)) = outcome_debit {
        let index = usize::from(outcome);
        eggs[index] = eggs[index]
            .checked_sub(quantity)
            .ok_or(Error::InsufficientClaims)?;
    }
    let position_poststate = position
        .settlement_poststate(
            old.cash_atoms
                .checked_add(paid_atoms)
                .ok_or(Error::Arithmetic)?,
            old.reserved_cash_atoms,
            eggs,
        )
        .map_err(|_| Error::PositionRefused)?;
    let replay = project_general_replay_transition_v1(
        source.position_replay,
        position_poststate,
        replay_kind,
        Id32::new(custody.fractional().transition_id().bytes())
            .map_err(|_| Error::ReplayRefused)?,
        Id32::new(custody.receipt_id().bytes()).map_err(|_| Error::ReplayRefused)?,
        &FractionalSha256V1,
    )
    .map_err(|_| Error::ReplayRefused)?;
    Ok(InternalPayoutPoststateV1 {
        position_account: Identity32V1::new(position.account)
            .map_err(|_| Error::PositionRefused)?,
        position_after: position_poststate.semantic,
        replay,
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
        transition_id: runtime_identity(custody.fractional().transition_id())?,
        burn_receipt_id: None,
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
    let (paid_atoms, residue, resolution_payout) =
        checked_redemption(context, outcome, quantity, 0)?;
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
        resolution_payout,
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
    source: InternalPositionV1,
    outcome: u8,
    quantity: u64,
) -> Result<RedemptionPlanV1> {
    let claimant = source.position_replay.position().semantic.owner();
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
                GeneralReplayTransitionKindV1::FractionalRedeemInternalExact,
                custody,
            )?))
        },
    )
}

/// Structurally bound Dealer facility state consumed by the private vector
/// transition inside Dealer action 23.
///
/// These fields do not confer SBF authority. The live adapter must construct
/// this value only after authenticating the exact Dealer State, facility
/// Position, Dealer Replay, Product obligation/root authority, and the
/// one-shot future-credit rent-principal receipt in the same instruction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoundDealerFacilityVectorPrestateV1 {
    facility_id: Identity32V1,
    dealer_state_account: Identity32V1,
    dealer_state_pre_semantic_id: Identity32V1,
    facility_position_account: Identity32V1,
    facility_position: PositionAccountV3,
    facility_position_pre_semantic_id: Identity32V1,
    facility_position_binding_id: Identity32V1,
    dealer_replay_account: Identity32V1,
    dealer_replay_pre_semantic_id: Identity32V1,
    dealer_replay_ordinal: u64,
    product_obligation_account: Identity32V1,
    product_obligation_semantic_id: Identity32V1,
    product_root_account: Identity32V1,
    product_root_semantic_id: Identity32V1,
    product_authority_receipt_id: Identity32V1,
    facility_credit_account: Identity32V1,
    facility_credit_funding_receipt_id: Identity32V1,
}

impl BoundDealerFacilityVectorPrestateV1 {
    /// Exact Dealer facility semantic owner.
    pub const fn facility_id(self) -> Identity32V1 {
        self.facility_id
    }

    /// Exact Dealer State account.
    pub const fn dealer_state_account(self) -> Identity32V1 {
        self.dealer_state_account
    }

    /// Dealer-authenticated State preimage identity.
    pub const fn dealer_state_pre_semantic_id(self) -> Identity32V1 {
        self.dealer_state_pre_semantic_id
    }

    /// Exact facility Position account.
    pub const fn facility_position_account(self) -> Identity32V1 {
        self.facility_position_account
    }

    /// Canonical facility Position prestate.
    pub const fn facility_position(self) -> PositionAccountV3 {
        self.facility_position
    }

    /// Canonical facility Position preimage identity.
    pub const fn facility_position_pre_semantic_id(self) -> Identity32V1 {
        self.facility_position_pre_semantic_id
    }

    /// Exact Dealer Replay account.
    pub const fn dealer_replay_account(self) -> Identity32V1 {
        self.dealer_replay_account
    }

    /// Dealer-authenticated Replay preimage identity.
    pub const fn dealer_replay_pre_semantic_id(self) -> Identity32V1 {
        self.dealer_replay_pre_semantic_id
    }

    /// Exact next Dealer Replay ordinal.
    pub const fn dealer_replay_ordinal(self) -> u64 {
        self.dealer_replay_ordinal
    }

    /// Canonical facility-owned a6/v2 account derived after Resolution.
    pub const fn facility_credit_account(self) -> Identity32V1 {
        self.facility_credit_account
    }

    /// One-shot pre-Resolution rent-principal funding receipt.
    pub const fn facility_credit_funding_receipt_id(self) -> Identity32V1 {
        self.facility_credit_funding_receipt_id
    }
}

/// Bind the exact Dealer-owned authority facts around one facility Position.
#[allow(clippy::too_many_arguments)]
pub fn bind_dealer_facility_vector_prestate_v1(
    facility_id: Identity32V1,
    dealer_state_account: Identity32V1,
    dealer_state_pre_semantic_id: Identity32V1,
    facility_position_account: Identity32V1,
    facility_position: PositionAccountV3,
    facility_position_pre_semantic_id: Identity32V1,
    facility_position_binding_id: Identity32V1,
    dealer_replay_account: Identity32V1,
    dealer_replay_pre_semantic_id: Identity32V1,
    dealer_replay_ordinal: u64,
    product_obligation_account: Identity32V1,
    product_obligation_semantic_id: Identity32V1,
    product_root_account: Identity32V1,
    product_root_semantic_id: Identity32V1,
    product_authority_receipt_id: Identity32V1,
    facility_credit_account: Identity32V1,
    facility_credit_funding_receipt_id: Identity32V1,
) -> Result<BoundDealerFacilityVectorPrestateV1> {
    facility_position
        .validate()
        .map_err(|_| Error::PositionRefused)?;
    let fields = facility_position.fields();
    let recomputed_position_id = facility_position
        .semantic_id(&FractionalSha256V1)
        .map_err(|_| Error::PositionRefused)?;
    if fields.purpose != PositionPurposeV3::DealerFacility
        || fields.lifecycle != PositionLifecycleV3::Open
        || fields.owner != facility_id
        || fields.controller != dealer_state_account
        || fields.purpose_binding_id != facility_position_binding_id
        || fields.replay_account != dealer_replay_account
        || dealer_replay_ordinal == 0
        || fields.generation == 0
        || fields.reserved_cash_atoms != 0
        || fields.outstanding_reservations != 0
        || recomputed_position_id != facility_position_pre_semantic_id
    {
        return Err(Error::MismatchedBinding);
    }
    let account_ids = [
        dealer_state_account,
        facility_position_account,
        dealer_replay_account,
        product_obligation_account,
        product_root_account,
        facility_credit_account,
    ];
    let mut left = 0usize;
    while left < account_ids.len() {
        let mut right = left + 1;
        while right < account_ids.len() {
            if account_ids[left] == account_ids[right] {
                return Err(Error::MismatchedBinding);
            }
            right += 1;
        }
        left += 1;
    }
    Ok(BoundDealerFacilityVectorPrestateV1 {
        facility_id,
        dealer_state_account,
        dealer_state_pre_semantic_id,
        facility_position_account,
        facility_position,
        facility_position_pre_semantic_id,
        facility_position_binding_id,
        dealer_replay_account,
        dealer_replay_pre_semantic_id,
        dealer_replay_ordinal,
        product_obligation_account,
        product_obligation_semantic_id,
        product_root_account,
        product_root_semantic_id,
        product_authority_receipt_id,
        facility_credit_account,
        facility_credit_funding_receipt_id,
    })
}

/// Complete one-sequence Fractional successor returned to Dealer action 23.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerFacilityVectorTransitionV1 {
    prestate: BoundDealerFacilityVectorPrestateV1,
    quantities: [u64; MAX_OUTCOMES],
    ledger_before_id: Identity32V1,
    ledger_after: FractionalLedgerV1,
    custody_after: FractionalClaimRedemptionPlanV3,
    credit_after: FractionalCreditV2,
    credit_after_id: Identity32V1,
    facility_position_after: PositionAccountV3,
    facility_position_after_id: Identity32V1,
    payout_atoms: u64,
    residue_numerator: u64,
    vector_transition_id: Identity32V1,
}

impl DealerFacilityVectorTransitionV1 {
    /// Exact authenticated facility prestate.
    pub const fn prestate(self) -> BoundDealerFacilityVectorPrestateV1 {
        self.prestate
    }

    /// Exact outcome-ordered quantities consumed together.
    pub const fn quantities(self) -> [u64; MAX_OUTCOMES] {
        self.quantities
    }

    /// a5 semantic identity before the vector transition.
    pub const fn ledger_before_id(self) -> Identity32V1 {
        self.ledger_before_id
    }

    /// Complete a5 successor.
    pub const fn ledger_after(self) -> FractionalLedgerV1 {
        self.ledger_after
    }

    /// Atomic ClaimLedger/Hoard successor.
    pub const fn custody_after(self) -> FractionalClaimRedemptionPlanV3 {
        self.custody_after
    }

    /// Newly initialized facility-owned a6/v2 successor.
    pub const fn credit_after(self) -> FractionalCreditV2 {
        self.credit_after
    }

    /// Semantic identity of the exact a6/v2 postimage.
    pub const fn credit_after_id(self) -> Identity32V1 {
        self.credit_after_id
    }

    /// Canonical facility Position successor.
    pub const fn facility_position_after(self) -> PositionAccountV3 {
        self.facility_position_after
    }

    /// Semantic identity of the facility Position successor.
    pub const fn facility_position_after_id(self) -> Identity32V1 {
        self.facility_position_after_id
    }

    /// Whole collateral atoms reclassified into facility Position cash.
    pub const fn payout_atoms(self) -> u64 {
        self.payout_atoms
    }

    /// Exact sub-atom numerator retained in the facility-owned credit.
    pub const fn residue_numerator(self) -> u64 {
        self.residue_numerator
    }

    /// Digest Dealer Replay must commit for its atomic action-23 successor.
    pub const fn vector_transition_id(self) -> Identity32V1 {
        self.vector_transition_id
    }
}

/// Prepare the sole bounded-vector Fractional transition for Dealer Resolve.
///
/// The fixed-width dot product is accumulated in canonical outcome order and
/// divided exactly once. Its remainder is retained in a newly initialized
/// facility-owned a6/v2 credit, and a5 K changes by that same numerator.
#[allow(clippy::too_many_arguments)]
pub fn prepare_dealer_facility_vector_transition_v1(
    context: BoundFractionalContextV1,
    request: crate::DealerFacilityVectorRequestV1,
    prestate: BoundDealerFacilityVectorPrestateV1,
    credit_creation: CreditCreationV1,
) -> Result<DealerFacilityVectorTransitionV1> {
    let position_fields = prestate.facility_position.fields();
    if request.outcome_count != context.policy.outcome_count
        || request.outcome_count != context.payout.outcome_count
        || request.expected_position_generation != prestate.facility_position.generation()
        || request.expected_replay_ordinal != prestate.dealer_replay_ordinal
        || request.expected_credit_sequence != 1
        || context.ledger.phase != FractionalLedgerPhaseV1::Live
        || position_fields.market_instance_id != context.policy.market_instance
        || position_fields.realm_id != context.policy.realm
        || position_fields.collateral_policy_id != context.policy.collateral_policy
        || position_fields.collateral_release_id != context.policy.collateral_release
        || position_fields.outcome_count != context.policy.outcome_count
    {
        return Err(Error::MismatchedBinding);
    }
    let CreditCreationV1::Fresh {
        claimant,
        stored_bump: _,
        rent: _,
    } = credit_creation
    else {
        return Err(Error::AlreadyInitialized);
    };
    if claimant != prestate.facility_id {
        return Err(Error::MismatchedBinding);
    }
    validate_canonical_solvency(
        context.payout,
        context.claim_ledger,
        context.hoard,
        context.ledger.aggregate_credit_numerator,
    )?;
    let canonical = canonical_supply(context.claim_ledger)?;
    let mut position_eggs = prestate.facility_position.native_eggs();
    let mut weighted_numerator = 0u128;
    let mut index = 0usize;
    while index < MAX_OUTCOMES {
        let quantity = request.quantities[index];
        if index < usize::from(request.outcome_count) {
            if canonical[index] < quantity {
                return Err(Error::InsufficientClaims);
            }
            position_eggs[index] = position_eggs[index]
                .checked_sub(quantity)
                .ok_or(Error::InsufficientClaims)?;
            weighted_numerator = weighted_numerator
                .checked_add(
                    u128::from(quantity)
                        .checked_mul(u128::from(context.payout.weights[index]))
                        .ok_or(Error::Arithmetic)?,
                )
                .ok_or(Error::Arithmetic)?;
        } else if quantity != 0 {
            return Err(Error::NonCanonicalPadding);
        }
        index += 1;
    }
    let (mut credit_after, created_count) = open_credit(
        context,
        CreditPrestateV1::Create(credit_creation),
        prestate.facility_id,
        request.expected_credit_sequence,
    )?;
    if created_count != 1 || credit_after.numerator != 0 {
        return Err(Error::AggregateMismatch);
    }
    let denominator = u128::from(context.payout.denominator);
    let payout_atoms = u64::try_from(weighted_numerator / denominator)
        .map_err(|_| Error::Arithmetic)?;
    let residue_numerator = u64::try_from(weighted_numerator % denominator)
        .map_err(|_| Error::Arithmetic)?;
    credit_after.numerator = residue_numerator;
    let ledger_before_id = context.ledger.state_id()?;
    let mut ledger_after = context.ledger.advanced(request.expected_ledger_sequence)?;
    ledger_after.active_credit_accounts = ledger_after
        .active_credit_accounts
        .checked_add(1)
        .ok_or(Error::Arithmetic)?;
    ledger_after.aggregate_credit_numerator = ledger_after
        .aggregate_credit_numerator
        .checked_add(u128::from(residue_numerator))
        .ok_or(Error::Arithmetic)?;
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
        request.expected_ledger_sequence,
        FractionalClaimSupplyMutationV3::BurnInternalVector {
            amounts: request.quantities,
        },
        payout_atoms,
        FractionalPayoutDispositionV3::InternalPositionCash,
    )?;
    let old = prestate.facility_position.fields();
    let mut position_after_fields = old;
    position_after_fields.generation = old
        .generation
        .checked_add(1)
        .ok_or(Error::Arithmetic)?;
    position_after_fields.cash_atoms = old
        .cash_atoms
        .checked_add(payout_atoms)
        .ok_or(Error::Arithmetic)?;
    position_after_fields.native_eggs = position_eggs;
    let facility_position_after = PositionAccountV3::new(position_after_fields)
        .map_err(|_| Error::PositionRefused)?;
    let facility_position_after_id = facility_position_after
        .semantic_id(&FractionalSha256V1)
        .map_err(|_| Error::PositionRefused)?;
    let credit_after_id = credit_after.state_id()?;
    let ledger_after_id = ledger_after.state_id()?;
    let fractional = custody_after.fractional();
    let mut hasher = Sha256::new();
    hasher.update(DEALER_FACILITY_VECTOR_TRANSITION_DOMAIN_V1);
    for identity in [
        prestate.facility_id,
        prestate.dealer_state_account,
        prestate.dealer_state_pre_semantic_id,
        prestate.facility_position_account,
        prestate.facility_position_pre_semantic_id,
        facility_position_after_id,
        prestate.facility_position_binding_id,
        prestate.dealer_replay_account,
        prestate.dealer_replay_pre_semantic_id,
        prestate.product_obligation_account,
        prestate.product_obligation_semantic_id,
        prestate.product_root_account,
        prestate.product_root_semantic_id,
        prestate.product_authority_receipt_id,
        prestate.facility_credit_account,
        prestate.facility_credit_funding_receipt_id,
        context.resolution_semantic_id,
        context.resolution_data_id,
        ledger_before_id,
        ledger_after_id,
        runtime_identity(fractional.claim_ledger_before_id())?,
        runtime_identity(fractional.claim_ledger_after_id())?,
        runtime_identity(custody_after.hoard_before_id())?,
        runtime_identity(custody_after.hoard_after_id())?,
        credit_after_id,
        runtime_identity(custody_after.receipt_id())?,
    ] {
        hasher.update(identity.bytes());
    }
    hasher.update(request.expected_ledger_sequence.to_le_bytes());
    hasher.update(request.expected_credit_sequence.to_le_bytes());
    hasher.update(request.expected_position_generation.to_le_bytes());
    hasher.update(request.expected_replay_ordinal.to_le_bytes());
    hasher.update([request.outcome_count]);
    let mut quantity_index = 0usize;
    while quantity_index < MAX_OUTCOMES {
        hasher.update(request.quantities[quantity_index].to_le_bytes());
        quantity_index += 1;
    }
    hasher.update(payout_atoms.to_le_bytes());
    hasher.update(residue_numerator.to_le_bytes());
    let vector_transition_id =
        Identity32V1::new(hasher.finalize().into()).map_err(|_| Error::ZeroIdentity)?;
    Ok(DealerFacilityVectorTransitionV1 {
        prestate,
        quantities: request.quantities,
        ledger_before_id,
        ledger_after,
        custody_after,
        credit_after,
        credit_after_id,
        facility_position_after,
        facility_position_after_id,
        payout_atoms,
        residue_numerator,
        vector_transition_id,
    })
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
            observed_before: source.observed_materialized_supply,
        },
        source.payout_disposition(),
        |paid, custody| {
            Ok(RedemptionSourcePoststateV1::Bearer(bearer_poststate(
                context, source, quantity, paid, custody,
            )?))
        },
    )
}

/// Prepared exact bearer redemption before either external effect.
///
/// Private fields ensure callers cannot extract the collateral request before
/// presenting an accepted burn from the independently released claim adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedBearerExactRedemptionV1 {
    context: BoundFractionalContextV1,
    source: BearerClaimPrestateV1,
    outcome: u8,
    quantity: u64,
    resolution_payout: ResolutionPayoutProjectionV5,
    ledger_after: FractionalLedgerV1,
    custody: PreparedFractionalExternalClaimRedemptionV3,
    used_common_lot_fast_path: bool,
}

impl PreparedBearerExactRedemptionV1 {
    /// Canonical private ClaimLedger/0xa5 plan the claim adapter must bind.
    pub const fn fractional_claim_ledger(self) -> FractionalClaimLedgerPlanV3 {
        self.custody.fractional()
    }

    /// Exact claimant authenticated by the source account bytes.
    pub const fn claimant(self) -> Identity32V1 {
        self.source.claimant
    }

    /// Exact selected outcome.
    pub const fn outcome(self) -> u8 {
        self.outcome
    }

    /// Exact bearer quantity to burn.
    pub const fn quantity(self) -> u64 {
        self.quantity
    }

    /// Runtime-observed materialized supplies before the burn.
    pub const fn observed_materialized_supply(self) -> [u64; MAX_OUTCOMES] {
        self.source.observed_materialized_supply
    }
}

/// Burn-accepted exact bearer redemption. Only this capability exposes the
/// collateral request selected by the canonical Fractional transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BurnAcceptedBearerExactRedemptionV1 {
    prepared: PreparedBearerExactRedemptionV1,
    burn: AcceptedFractionalBearerClaimBurnV3,
}

impl BurnAcceptedBearerExactRedemptionV1 {
    /// Exact claim-bound zero/nonzero collateral request.
    pub const fn collateral_request(self) -> ClaimRedemptionCollateralRequestV2 {
        self.prepared.custody.collateral_request()
    }
}

/// Prepare a two-phase exact bearer redemption without exposing any custody
/// transfer authority before the Token-2022 burn is accepted.
pub fn prepare_bearer_exact_v1(
    context: BoundFractionalContextV1,
    expected_ledger_sequence: u64,
    source: BearerClaimPrestateV1,
    outcome: u8,
    quantity: u64,
) -> Result<PreparedBearerExactRedemptionV1> {
    source.validate(context, outcome, quantity)?;
    let (paid_atoms, residue, resolution_payout) =
        checked_redemption(context, outcome, quantity, 0)?;
    if residue != 0 {
        return Err(Error::NonIntegralLot);
    }
    let ledger_after = context.ledger.advanced(expected_ledger_sequence)?;
    let before_id = collateral_id(context.ledger.state_id()?);
    let after_id = collateral_id(ledger_after.state_id()?);
    let custody = map_collateral(prepare_fractional_external_claim_redemption_v3(
        context.hoard,
        context.claim_ledger,
        before_id,
        after_id,
        expected_ledger_sequence,
        outcome,
        quantity,
        source.observed_materialized_supply,
        paid_atoms,
        collateral_id(source.claimant),
        collateral_id(source.collateral_destination),
        &FractionalSha256V1,
    ))?;
    let fractional = custody.fractional();
    if fractional.fractional_ledger_before_id() != before_id
        || fractional.fractional_ledger_after_id() != after_id
        || fractional.consumed_sequence() != expected_ledger_sequence
    {
        return Err(Error::MismatchedBinding);
    }
    validate_canonical_solvency(
        context.payout,
        custody.claim_ledger_after(),
        custody.hoard_after(),
        ledger_after.aggregate_credit_numerator,
    )?;
    Ok(PreparedBearerExactRedemptionV1 {
        context,
        source,
        outcome,
        quantity,
        resolution_payout,
        ledger_after,
        custody,
        used_common_lot_fast_path: quantity.is_multiple_of(context.policy.common_lot),
    })
}

/// Accept the independent Token-2022 burn and expose the exact collateral request.
pub fn accept_bearer_exact_burn_v1(
    prepared: PreparedBearerExactRedemptionV1,
    burn: AcceptedFractionalBearerClaimBurnV3,
) -> Result<BurnAcceptedBearerExactRedemptionV1> {
    let intent = burn.burn_intent();
    if burn.fractional() != prepared.custody.fractional()
        || burn.claim_binding_id().bytes() != prepared.source.claim_issuance_binding.bytes()
        || burn.claimant().bytes() != prepared.source.claimant.bytes()
        || burn.outcome() != prepared.outcome
        || burn.quantity() != prepared.quantity
        || intent.mint.bytes() != prepared.source.claim_mint.bytes()
        || intent.source_token_account.bytes() != prepared.source.claim_token_account.bytes()
        || intent.claimant.bytes() != prepared.source.claimant.bytes()
        || intent.quantity != prepared.quantity
    {
        return Err(Error::ClaimPlaneRefused);
    }
    Ok(BurnAcceptedBearerExactRedemptionV1 { prepared, burn })
}

/// Accept the Realm collateral postcondition and publish one complete atomic
/// bearer/ClaimLedger/Hoard/0xa5 successor.
pub fn finish_bearer_exact_v1(
    burned: BurnAcceptedBearerExactRedemptionV1,
    collateral: AcceptedBearerRedemptionCollateralV3,
) -> Result<RedemptionPlanV1> {
    let accepted_nonzero = match collateral {
        AcceptedBearerRedemptionCollateralV3::Zero(_) => None,
        AcceptedBearerRedemptionCollateralV3::Nonzero(accepted) => Some(accepted),
    };
    let custody_after = map_collateral(accept_fractional_external_claim_redemption_v3(
        burned.prepared.custody,
        collateral,
    ))?;
    let source = BearerClaimSourceV1 {
        claimant: burned.prepared.source.claimant,
        claim_token_account: burned.prepared.source.claim_token_account,
        claim_mint: burned.prepared.source.claim_mint,
        collateral_destination: burned.prepared.source.collateral_destination,
        claim_issuance_binding: burned.prepared.source.claim_issuance_binding,
        source_claim_atoms: burned.prepared.source.source_claim_atoms,
        observed_materialized_supply: burned.prepared.source.observed_materialized_supply,
        accepted_collateral: accepted_nonzero,
    };
    let mut source_after = bearer_poststate(
        burned.prepared.context,
        source,
        burned.prepared.quantity,
        burned.prepared.custody.payout_atoms(),
        custody_after,
    )?;
    source_after.burn_receipt_id = Some(runtime_identity(burned.burn.burn_receipt_id())?);
    Ok(RedemptionPlanV1 {
        resolution_payout: burned.prepared.resolution_payout,
        ledger_after: burned.prepared.ledger_after,
        custody_after,
        credit_after: None,
        paid_atoms: burned.prepared.custody.payout_atoms(),
        claimant_numerator_after: 0,
        used_common_lot_fast_path: burned.prepared.used_common_lot_fast_path,
        source_after: RedemptionSourcePoststateV1::Bearer(source_after),
    })
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
    let (paid_atoms, residue, resolution_payout) =
        checked_redemption(context, outcome, quantity, prior)?;
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
        resolution_payout,
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
    source: InternalPositionV1,
    outcome: u8,
    quantity: u64,
) -> Result<RedemptionPlanV1> {
    let claimant = source.position_replay.position().semantic.owner();
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
                GeneralReplayTransitionKindV1::FractionalRedeemInternalCredit,
                custody,
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
            observed_before: source.observed_materialized_supply,
        },
        source.payout_disposition(),
        |paid, custody| {
            Ok(RedemptionSourcePoststateV1::Bearer(bearer_poststate(
                context, source, quantity, paid, custody,
            )?))
        },
    )
}

/// Prepared credited bearer redemption before either external effect.
///
/// Credit and collateral successors remain private until the independent claim
/// adapter accepts the exact Token-2022 burn. This is the credited analogue of
/// [`PreparedBearerExactRedemptionV1`], not a second custody owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedBearerCreditRedemptionV1 {
    context: BoundFractionalContextV1,
    source: BearerClaimPrestateV1,
    outcome: u8,
    quantity: u64,
    resolution_payout: ResolutionPayoutProjectionV5,
    ledger_after: FractionalLedgerV1,
    credit_after: FractionalCreditV2,
    custody: PreparedFractionalExternalClaimRedemptionV3,
    used_common_lot_fast_path: bool,
}

impl PreparedBearerCreditRedemptionV1 {
    /// Canonical private ClaimLedger/0xa5 plan the claim adapter must bind.
    pub const fn fractional_claim_ledger(self) -> FractionalClaimLedgerPlanV3 {
        self.custody.fractional()
    }

    /// Exact claimant authenticated by the source account bytes.
    pub const fn claimant(self) -> Identity32V1 {
        self.source.claimant
    }

    /// Exact selected outcome.
    pub const fn outcome(self) -> u8 {
        self.outcome
    }

    /// Exact bearer quantity to burn.
    pub const fn quantity(self) -> u64 {
        self.quantity
    }

    /// Runtime-observed materialized supplies before the burn.
    pub const fn observed_materialized_supply(self) -> [u64; MAX_OUTCOMES] {
        self.source.observed_materialized_supply
    }
}

/// Burn-accepted credited bearer redemption. Only this capability exposes the
/// exact Realm collateral request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BurnAcceptedBearerCreditRedemptionV1 {
    prepared: PreparedBearerCreditRedemptionV1,
    burn: AcceptedFractionalBearerClaimBurnV3,
}

impl BurnAcceptedBearerCreditRedemptionV1 {
    /// Exact claim-bound zero/nonzero collateral request.
    pub const fn collateral_request(self) -> ClaimRedemptionCollateralRequestV2 {
        self.prepared.custody.collateral_request()
    }
}

/// Prepare arbitrary bearer redemption without exposing payout or credit state.
#[allow(clippy::too_many_arguments)]
pub fn prepare_bearer_credit_v1(
    context: BoundFractionalContextV1,
    expected_ledger_sequence: u64,
    expected_credit_sequence: u64,
    credit_prestate: CreditPrestateV1,
    source: BearerClaimPrestateV1,
    outcome: u8,
    quantity: u64,
) -> Result<PreparedBearerCreditRedemptionV1> {
    source.validate(context, outcome, quantity)?;
    let (mut credit_after, created_count) = open_credit(
        context,
        credit_prestate,
        source.claimant,
        expected_credit_sequence,
    )?;
    let prior = credit_after.numerator;
    let (paid_atoms, residue, resolution_payout) =
        checked_redemption(context, outcome, quantity, prior)?;
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
    let before_id = collateral_id(context.ledger.state_id()?);
    let after_id = collateral_id(ledger_after.state_id()?);
    let custody = map_collateral(prepare_fractional_external_claim_redemption_v3(
        context.hoard,
        context.claim_ledger,
        before_id,
        after_id,
        expected_ledger_sequence,
        outcome,
        quantity,
        source.observed_materialized_supply,
        paid_atoms,
        collateral_id(source.claimant),
        collateral_id(source.collateral_destination),
        &FractionalSha256V1,
    ))?;
    let fractional = custody.fractional();
    if fractional.fractional_ledger_before_id() != before_id
        || fractional.fractional_ledger_after_id() != after_id
        || fractional.consumed_sequence() != expected_ledger_sequence
    {
        return Err(Error::MismatchedBinding);
    }
    validate_canonical_solvency(
        context.payout,
        custody.claim_ledger_after(),
        custody.hoard_after(),
        ledger_after.aggregate_credit_numerator,
    )?;
    Ok(PreparedBearerCreditRedemptionV1 {
        context,
        source,
        outcome,
        quantity,
        resolution_payout,
        ledger_after,
        credit_after,
        custody,
        used_common_lot_fast_path: quantity.is_multiple_of(context.policy.common_lot),
    })
}

/// Accept the independent Token-2022 burn before exposing credited payout authority.
pub fn accept_bearer_credit_burn_v1(
    prepared: PreparedBearerCreditRedemptionV1,
    burn: AcceptedFractionalBearerClaimBurnV3,
) -> Result<BurnAcceptedBearerCreditRedemptionV1> {
    let intent = burn.burn_intent();
    if burn.fractional() != prepared.custody.fractional()
        || burn.claim_binding_id().bytes() != prepared.source.claim_issuance_binding.bytes()
        || burn.claimant().bytes() != prepared.source.claimant.bytes()
        || burn.outcome() != prepared.outcome
        || burn.quantity() != prepared.quantity
        || intent.mint.bytes() != prepared.source.claim_mint.bytes()
        || intent.source_token_account.bytes() != prepared.source.claim_token_account.bytes()
        || intent.claimant.bytes() != prepared.source.claimant.bytes()
        || intent.quantity != prepared.quantity
    {
        return Err(Error::ClaimPlaneRefused);
    }
    Ok(BurnAcceptedBearerCreditRedemptionV1 { prepared, burn })
}

/// Accept Realm collateral postconditions and publish the complete credited successor.
pub fn finish_bearer_credit_v1(
    burned: BurnAcceptedBearerCreditRedemptionV1,
    collateral: AcceptedBearerRedemptionCollateralV3,
) -> Result<RedemptionPlanV1> {
    let accepted_nonzero = match collateral {
        AcceptedBearerRedemptionCollateralV3::Zero(_) => None,
        AcceptedBearerRedemptionCollateralV3::Nonzero(accepted) => Some(accepted),
    };
    let custody_after = map_collateral(accept_fractional_external_claim_redemption_v3(
        burned.prepared.custody,
        collateral,
    ))?;
    let source = BearerClaimSourceV1 {
        claimant: burned.prepared.source.claimant,
        claim_token_account: burned.prepared.source.claim_token_account,
        claim_mint: burned.prepared.source.claim_mint,
        collateral_destination: burned.prepared.source.collateral_destination,
        claim_issuance_binding: burned.prepared.source.claim_issuance_binding,
        source_claim_atoms: burned.prepared.source.source_claim_atoms,
        observed_materialized_supply: burned.prepared.source.observed_materialized_supply,
        accepted_collateral: accepted_nonzero,
    };
    let mut source_after = bearer_poststate(
        burned.prepared.context,
        source,
        burned.prepared.quantity,
        burned.prepared.custody.payout_atoms(),
        custody_after,
    )?;
    source_after.burn_receipt_id = Some(runtime_identity(burned.burn.burn_receipt_id())?);
    Ok(RedemptionPlanV1 {
        resolution_payout: burned.prepared.resolution_payout,
        ledger_after: burned.prepared.ledger_after,
        custody_after,
        credit_after: Some(burned.prepared.credit_after),
        paid_atoms: burned.prepared.custody.payout_atoms(),
        claimant_numerator_after: burned.prepared.credit_after.numerator,
        used_common_lot_fast_path: burned.prepared.used_common_lot_fast_path,
        source_after: RedemptionSourcePoststateV1::Bearer(source_after),
    })
}

/// Destination that receives a whole collateral atom created by credit merge.
#[derive(Clone, Copy, Debug)]
pub enum CreditPayoutTargetV1 {
    /// Credit Position V3 cash and advance its existing General Replay V3.
    Internal {
        /// Canonical Position and Replay.
        position: InternalPositionV1,
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

impl CreditPayoutTargetV1 {
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
    target: CreditPayoutTargetV1,
    claimant: Identity32V1,
    paid_atoms: u64,
    custody: FractionalClaimRedemptionPlanV3,
    replay_kind: GeneralReplayTransitionKindV1,
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
            replay_kind,
            custody,
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
    pub source_after: FractionalCreditV2,
    /// Destination credit successor.
    pub destination_after: FractionalCreditV2,
    /// Sole aggregate-credit owner successor.
    pub ledger_after: FractionalLedgerV1,
    /// Atomic canonical ClaimLedger/Hoard successor and cross-account IDs.
    pub custody_after: FractionalClaimRedemptionPlanV3,
    /// Whole collateral atoms created by aggregation.
    pub paid_atoms: u64,
    /// Exact atomic payout target.
    pub payout_after: CreditPayoutPoststateV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PreparedCreditTransferStateV1 {
    source_after: FractionalCreditV2,
    destination_after: FractionalCreditV2,
    ledger_after: FractionalLedgerV1,
    paid_atoms: u64,
}

fn external_credit_transition_id_v1(
    context: BoundFractionalContextV1,
    source_before: FractionalCreditV2,
    state: PreparedCreditTransferStateV1,
    collateral_destination: Identity32V1,
) -> Result<Identity32V1> {
    let source_before = source_before.encode()?;
    let source_after = state.source_after.encode()?;
    let destination_after = state.destination_after.encode()?;
    let fractional_ledger_before_id = context.ledger.state_id()?;
    let fractional_ledger_after_id = state.ledger_after.state_id()?;
    let claim_ledger_before_id = map_collateral(
        context
            .claim_ledger
            .semantic_id(&FractionalSha256V1),
    )?;
    let mut hasher = Sha256::new();
    hasher.update(FRACTIONAL_EXTERNAL_CREDIT_TRANSITION_DOMAIN_V1);
    hasher.update(context.policy.market_instance.bytes());
    hasher.update(context.policy_account.bytes());
    hasher.update(context.ledger_account.bytes());
    hasher.update(context.claim_ledger_account.bytes());
    hasher.update(claim_ledger_before_id.bytes());
    hasher.update(fractional_ledger_before_id.bytes());
    hasher.update(fractional_ledger_after_id.bytes());
    hasher.update(context.ledger.next_sequence.to_le_bytes());
    hasher.update([0u8]);
    hasher.update(source_before);
    hasher.update(source_after);
    hasher.update(destination_after);
    hasher.update(state.destination_after.claimant.bytes());
    hasher.update(collateral_destination.bytes());
    hasher.update(state.paid_atoms.to_le_bytes());
    Identity32V1::new(hasher.finalize().into()).map_err(|_| Error::ZeroIdentity)
}

#[allow(clippy::too_many_arguments)]
fn prepare_credit_transfer_state_v1(
    context: BoundFractionalContextV1,
    expected_ledger_sequence: u64,
    source: FractionalCreditV2,
    expected_source_sequence: u64,
    destination: CreditPrestateV1,
    destination_claimant: Identity32V1,
    expected_destination_sequence: u64,
    numerator: u64,
) -> Result<PreparedCreditTransferStateV1> {
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
    Ok(PreparedCreditTransferStateV1 {
        source_after,
        destination_after,
        ledger_after,
        paid_atoms,
    })
}

/// Fully authenticated credit aggregation before Realm collateral movement.
///
/// Source, destination, aggregate-ledger, ClaimLedger, and Hoard successors
/// remain private. Only the exact collateral request is exposed for adapter
/// execution; final successors require its accepted postcondition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedExternalCreditTransferV1 {
    context: BoundFractionalContextV1,
    source_after: FractionalCreditV2,
    destination_after: FractionalCreditV2,
    ledger_after: FractionalLedgerV1,
    custody: PreparedFractionalExternalCreditPayoutV3,
    credit_transition_id: Identity32V1,
    paid_atoms: u64,
    claimant: Identity32V1,
    collateral_destination: Identity32V1,
}

impl PreparedExternalCreditTransferV1 {
    /// Fractional-owned commitment to the exact credit action and successors.
    pub const fn credit_transition_id(self) -> Identity32V1 {
        self.credit_transition_id
    }

    /// Exact Realm-selected collateral request admitted by both credits.
    pub const fn collateral_request(self) -> ClaimRedemptionCollateralRequestV2 {
        self.custody.collateral_request()
    }
}

#[allow(clippy::too_many_arguments)]
fn prepare_external_credit_transfer_state_v1(
    context: BoundFractionalContextV1,
    expected_ledger_sequence: u64,
    source: FractionalCreditV2,
    expected_source_sequence: u64,
    destination: CreditPrestateV1,
    destination_claimant: Identity32V1,
    expected_destination_sequence: u64,
    numerator: u64,
    collateral_destination: Identity32V1,
) -> Result<PreparedExternalCreditTransferV1> {
    let state = prepare_credit_transfer_state_v1(
        context,
        expected_ledger_sequence,
        source,
        expected_source_sequence,
        destination,
        destination_claimant,
        expected_destination_sequence,
        numerator,
    )?;
    let before_id = collateral_id(context.ledger.state_id()?);
    let after_id = collateral_id(state.ledger_after.state_id()?);
    let credit_transition_id =
        external_credit_transition_id_v1(context, source, state, collateral_destination)?;
    let custody = map_collateral(prepare_fractional_external_credit_payout_v3(
        context.hoard,
        context.claim_ledger,
        before_id,
        after_id,
        expected_ledger_sequence,
        collateral_id(credit_transition_id),
        state.paid_atoms,
        collateral_id(destination_claimant),
        collateral_id(collateral_destination),
        &FractionalSha256V1,
    ))?;
    let fractional = custody.fractional();
    let request = custody.collateral_request();
    if custody.fractional_credit_transition_id() != collateral_id(credit_transition_id)
        || fractional.fractional_ledger_before_id() != before_id
        || custody.fractional().fractional_ledger_after_id() != after_id
        || fractional.consumed_sequence() != expected_ledger_sequence
        || fractional.supply_mutation() != FractionalClaimSupplyMutationV3::Unchanged
        || request.claim_redemption_id != fractional.transition_id()
        || request.destination_token_account != collateral_id(collateral_destination)
        || request.claim_semantic_owner != collateral_id(destination_claimant)
        || request.payout_atoms != state.paid_atoms
    {
        return Err(Error::MismatchedBinding);
    }
    validate_canonical_solvency(
        context.payout,
        custody.claim_ledger_after(),
        custody.hoard_after(),
        state.ledger_after.aggregate_credit_numerator,
    )?;
    Ok(PreparedExternalCreditTransferV1 {
        context,
        source_after: state.source_after,
        destination_after: state.destination_after,
        ledger_after: state.ledger_after,
        custody,
        credit_transition_id,
        paid_atoms: state.paid_atoms,
        claimant: destination_claimant,
        collateral_destination,
    })
}

/// Prepare an explicit source numerator transfer with external payout.
#[allow(clippy::too_many_arguments)]
pub fn prepare_external_credit_transfer_v1(
    context: BoundFractionalContextV1,
    expected_ledger_sequence: u64,
    source: FractionalCreditV2,
    expected_source_sequence: u64,
    destination: CreditPrestateV1,
    destination_claimant: Identity32V1,
    expected_destination_sequence: u64,
    numerator: u64,
    collateral_destination: Identity32V1,
) -> Result<PreparedExternalCreditTransferV1> {
    prepare_external_credit_transfer_state_v1(
        context,
        expected_ledger_sequence,
        source,
        expected_source_sequence,
        destination,
        destination_claimant,
        expected_destination_sequence,
        numerator,
        collateral_destination,
    )
}

/// Prepare a full nonzero source-credit merge with external payout.
///
/// With no Position/GEN1 consumer, a merge is intentionally the canonical
/// full-source instance of transfer and produces the same successor as an
/// explicit transfer of that numerator. It does not invent an external replay
/// owner merely to preserve a redundant action label.
#[allow(clippy::too_many_arguments)]
pub fn prepare_external_credit_merge_v1(
    context: BoundFractionalContextV1,
    expected_ledger_sequence: u64,
    source: FractionalCreditV2,
    expected_source_sequence: u64,
    destination: CreditPrestateV1,
    destination_claimant: Identity32V1,
    expected_destination_sequence: u64,
    collateral_destination: Identity32V1,
) -> Result<PreparedExternalCreditTransferV1> {
    prepare_external_credit_transfer_state_v1(
        context,
        expected_ledger_sequence,
        source,
        expected_source_sequence,
        destination,
        destination_claimant,
        expected_destination_sequence,
        source.numerator,
        collateral_destination,
    )
}

/// Accept Realm collateral postconditions and expose one atomic credit plan.
pub fn finish_external_credit_transfer_v1(
    prepared: PreparedExternalCreditTransferV1,
    collateral: AcceptedBearerRedemptionCollateralV3,
) -> Result<CreditTransferPlanV1> {
    let accepted_request = match collateral {
        AcceptedBearerRedemptionCollateralV3::Zero(accepted) => accepted.request(),
        AcceptedBearerRedemptionCollateralV3::Nonzero(accepted) => accepted.request(),
    };
    if prepared.custody.fractional_credit_transition_id()
        != collateral_id(prepared.credit_transition_id)
        || accepted_request != prepared.custody.collateral_request()
    {
        return Err(Error::MismatchedBinding);
    }
    let custody_after = map_collateral(accept_fractional_external_credit_payout_v3(
        prepared.custody,
        collateral,
    ))?;
    Ok(CreditTransferPlanV1 {
        source_after: prepared.source_after,
        destination_after: prepared.destination_after,
        ledger_after: prepared.ledger_after,
        custody_after,
        paid_atoms: prepared.paid_atoms,
        payout_after: CreditPayoutPoststateV1::External {
            claimant: prepared.claimant,
            collateral_hoard: Identity32V1::new(
                prepared.context.collateral.market().hoard_token_account.bytes(),
            )
            .map_err(|_| Error::CollateralRefused)?,
            collateral_destination: prepared.collateral_destination,
            payout_atoms: prepared.paid_atoms,
        },
    })
}

/// Transfer an explicit numerator amount between same-domain owner credits.
///
/// Destination claimant acceptance, any destination account creation, and the
/// whole-atom payout are one atomic plan. No numerator is tokenized or erased.
#[allow(clippy::too_many_arguments)]
pub fn transfer_credit_v1(
    context: BoundFractionalContextV1,
    expected_ledger_sequence: u64,
    source: FractionalCreditV2,
    expected_source_sequence: u64,
    destination: CreditPrestateV1,
    destination_claimant: Identity32V1,
    expected_destination_sequence: u64,
    numerator: u64,
    payout_target: CreditPayoutTargetV1,
) -> Result<CreditTransferPlanV1> {
    transfer_credit_with_kind_v1(
        context,
        expected_ledger_sequence,
        source,
        expected_source_sequence,
        destination,
        destination_claimant,
        expected_destination_sequence,
        numerator,
        payout_target,
        GeneralReplayTransitionKindV1::FractionalTransferCreditPayout,
    )
}

#[allow(clippy::too_many_arguments)]
fn transfer_credit_with_kind_v1(
    context: BoundFractionalContextV1,
    expected_ledger_sequence: u64,
    source: FractionalCreditV2,
    expected_source_sequence: u64,
    destination: CreditPrestateV1,
    destination_claimant: Identity32V1,
    expected_destination_sequence: u64,
    numerator: u64,
    payout_target: CreditPayoutTargetV1,
    replay_kind: GeneralReplayTransitionKindV1,
) -> Result<CreditTransferPlanV1> {
    let state = prepare_credit_transfer_state_v1(
        context,
        expected_ledger_sequence,
        source,
        expected_source_sequence,
        destination,
        destination_claimant,
        expected_destination_sequence,
        numerator,
    )?;
    let custody_after = prepare_canonical_redemption(
        context,
        state.ledger_after,
        expected_ledger_sequence,
        FractionalClaimSupplyMutationV3::Unchanged,
        state.paid_atoms,
        payout_target.disposition(),
    )?;
    Ok(CreditTransferPlanV1 {
        source_after: state.source_after,
        destination_after: state.destination_after,
        ledger_after: state.ledger_after,
        custody_after,
        paid_atoms: state.paid_atoms,
        payout_after: credit_payout_poststate(
            context,
            payout_target,
            destination_claimant,
            state.paid_atoms,
            custody_after,
            replay_kind,
        )?,
    })
}

/// Merge the entire nonzero source residue into a destination credit.
#[allow(clippy::too_many_arguments)]
pub fn merge_credit_v1(
    context: BoundFractionalContextV1,
    expected_ledger_sequence: u64,
    source: FractionalCreditV2,
    expected_source_sequence: u64,
    destination: CreditPrestateV1,
    destination_claimant: Identity32V1,
    expected_destination_sequence: u64,
    payout_target: CreditPayoutTargetV1,
) -> Result<CreditTransferPlanV1> {
    let numerator = source.numerator;
    transfer_credit_with_kind_v1(
        context,
        expected_ledger_sequence,
        source,
        expected_source_sequence,
        destination,
        destination_claimant,
        expected_destination_sequence,
        numerator,
        payout_target,
        GeneralReplayTransitionKindV1::FractionalMergeCreditPayout,
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
    /// Canonical live `0xa6/v2` prestate identity.
    pub credit_before_id: Identity32V1,
    /// Permanent replay-prevention successor at the same PDA.
    pub tombstone: FractionalCreditTombstoneV2,
    /// Canonical permanent tombstone poststate identity.
    pub tombstone_after_id: Identity32V1,
    /// Sole aggregate-credit owner successor.
    pub ledger_after: FractionalLedgerV1,
    /// Canonical unchanged-supply ClaimLedger successor and cross-account IDs.
    pub claim_ledger_after: FractionalClaimLedgerPlanV3,
    /// Exact rent disposition; collateral/credit principal is never included.
    pub funding: CreditCloseFundingPlanV1,
    /// Canonical identity of the complete a5/ClaimLedger/a6 close transition.
    pub transition_id: Identity32V1,
}

/// Close only a zero-numerator credit into its permanent tombstone.
pub fn close_zero_credit_v1(
    context: BoundFractionalContextV1,
    expected_ledger_sequence: u64,
    credit: FractionalCreditV2,
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
    let credit_before_id = credit.state_id()?;
    let ledger_before_id = context.ledger.state_id()?;
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
    let tombstone = FractionalCreditTombstoneV2 {
        policy_account: credit.policy_account,
        ledger_account: credit.ledger_account,
        market_instance: credit.market_instance,
        resolution_account: credit.resolution_account,
        resolution_data_id: credit.resolution_data_id,
        claimant: credit.claimant,
        domain_generation: credit.domain_generation,
        account_generation: credit.account_generation,
        closed_next_sequence: credit_after_sequence,
        stored_bump: credit.stored_bump,
        permanent_tombstone_principal: rent.permanent_tombstone_principal,
    };
    let tombstone_after_id = tombstone.state_id()?;
    let ledger_after_id = ledger_after.state_id()?;
    let funding = CreditCloseFundingPlanV1 {
        payer: rent.payer,
        payer_refund_lamports: rent.refundable_live_principal,
        tombstone_lamports: rent.permanent_tombstone_principal,
        neutral_sink,
        neutral_lamports: actual_lamports
            .checked_sub(principal)
            .ok_or(Error::RentRefused)?,
    };
    let mut hasher = Sha256::new();
    hasher.update(FRACTIONAL_CREDIT_CLOSE_TRANSITION_DOMAIN_V1);
    for id in [
        context.policy.state_id()?,
        ledger_before_id,
        ledger_after_id,
        credit_before_id,
        tombstone_after_id,
        claim_ledger_after.claim_ledger_before_id(),
        claim_ledger_after.claim_ledger_after_id(),
        funding.payer,
        funding.neutral_sink,
    ] {
        hasher.update(id.bytes());
    }
    hasher.update(funding.payer_refund_lamports.to_le_bytes());
    hasher.update(funding.tombstone_lamports.to_le_bytes());
    hasher.update(funding.neutral_lamports.to_le_bytes());
    let transition_id =
        Identity32V1::new(hasher.finalize().into()).map_err(|_| Error::ZeroIdentity)?;
    Ok(CreditClosePlanV1 {
        credit_before_id,
        tombstone,
        tombstone_after_id,
        ledger_after,
        claim_ledger_after,
        funding,
        transition_id,
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

/// Exact terminal facts a Product five-family authorization must bind before
/// either fractional-family account may be deleted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalDomainTerminalRequirementV1 {
    market_instance_id: Identity32V1,
    domain_generation: u64,
    resolution_account: Identity32V1,
    resolution_semantic_id: Identity32V1,
    resolution_data_id: Identity32V1,
    native_claim_basis_id: Identity32V1,
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

    /// Physical canonical Resolution V5 account for this generation.
    pub const fn resolution_account(self) -> Identity32V1 {
        self.resolution_account
    }

    /// Body-only identity of the exact final Resolution V5 state.
    pub const fn resolution_semantic_id(self) -> Identity32V1 {
        self.resolution_semantic_id
    }

    /// PDA-and-body identity of the exact final Resolution V5 state.
    pub const fn resolution_data_id(self) -> Identity32V1 {
        self.resolution_data_id
    }

    /// Canonical NativeClaimBasis selected by Resolution and ClaimLedger.
    pub const fn native_claim_basis_id(self) -> Identity32V1 {
        self.native_claim_basis_id
    }

    /// Physical immutable `0xa4/v3` account to delete.
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
/// This pure value is not Product authorization. The current SBF adapter must
/// consume the matching private Product five-family close authorization
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

    /// Exact facts the private Product five-family authorization must match.
    pub const fn terminal_requirement(self) -> FractionalDomainTerminalRequirementV1 {
        self.terminal_requirement
    }

    /// Independent rent-only disposition for immutable `0xa4/v3`.
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
        resolution_account: context.policy.resolution_account,
        resolution_semantic_id: context.resolution_semantic_id,
        resolution_data_id: context.resolution_data_id,
        native_claim_basis_id: runtime_identity(context.claim_ledger.native_claim_basis_id)?,
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

/// Exact Fractional-owned terminal receipt for Product family retirement.
///
/// This projection is not adapter authority. The SBF composer must source
/// `fractional_release_id` from the authenticated Fractional capability
/// release, perform both exact rent dispositions, persist the ClaimLedger
/// retirement successor, and consume this receipt in Product atomically with
/// deleting a4/a5. Product must not derive a substitute receipt from account
/// bytes or copy the collateral adapter release identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalFamilyTerminalReceiptV1 {
    market_instance_id: Identity32V1,
    domain_generation: u64,
    policy_account: Identity32V1,
    policy_terminal_state_id: Identity32V1,
    ledger_account: Identity32V1,
    ledger_terminal_state_id: Identity32V1,
    claim_ledger_account: Identity32V1,
    claim_ledger_post_state_id: Identity32V1,
    claim_ledger_transition_id: Identity32V1,
    fractional_release_id: Identity32V1,
    rent_disposition_id: Identity32V1,
    receipt_id: Identity32V1,
}

impl FractionalFamilyTerminalReceiptV1 {
    /// Exact MarketInstanceV2 identity retired by the receipt.
    pub const fn market_instance_id(self) -> Identity32V1 {
        self.market_instance_id
    }

    /// Exact nonzero Fractional/Resolution generation.
    pub const fn domain_generation(self) -> u64 {
        self.domain_generation
    }

    /// Physical immutable a4/v3 account deleted atomically.
    pub const fn policy_account(self) -> Identity32V1 {
        self.policy_account
    }

    /// Exact immutable policy terminal-state identity.
    pub const fn policy_terminal_state_id(self) -> Identity32V1 {
        self.policy_terminal_state_id
    }

    /// Physical aggregate a5/v1 account deleted atomically.
    pub const fn ledger_account(self) -> Identity32V1 {
        self.ledger_account
    }

    /// Exact transient a5/v1 retirement-state identity.
    pub const fn ledger_terminal_state_id(self) -> Identity32V1 {
        self.ledger_terminal_state_id
    }

    /// Physical ClaimLedger V3 account advanced to Retiring.
    pub const fn claim_ledger_account(self) -> Identity32V1 {
        self.claim_ledger_account
    }

    /// Exact Retiring ClaimLedger V3 semantic identity.
    pub const fn claim_ledger_post_state_id(self) -> Identity32V1 {
        self.claim_ledger_post_state_id
    }

    /// Shared terminal a5/ClaimLedger transition identity.
    pub const fn claim_ledger_transition_id(self) -> Identity32V1 {
        self.claim_ledger_transition_id
    }

    /// Separately authenticated Fractional runtime/capability release.
    pub const fn fractional_release_id(self) -> Identity32V1 {
        self.fractional_release_id
    }

    /// Exact commitment to both payer-refund and neutral-sink splits.
    pub const fn rent_disposition_id(self) -> Identity32V1 {
        self.rent_disposition_id
    }

    /// Unique receipt consumed by Product's Fractional family terminal step.
    pub const fn receipt_id(self) -> Identity32V1 {
        self.receipt_id
    }
}

fn hash_fractional_close_funding(
    policy: FractionalAccountCloseFundingV1,
    ledger: FractionalAccountCloseFundingV1,
) -> Result<Identity32V1> {
    let mut hasher = Sha256::new();
    hasher.update(FRACTIONAL_FAMILY_RENT_DISPOSITION_DOMAIN_V1);
    for funding in [policy, ledger] {
        hasher.update(funding.account.bytes());
        hasher.update(funding.payer.bytes());
        hasher.update(funding.payer_refund_lamports.to_le_bytes());
        hasher.update(funding.neutral_sink.bytes());
        hasher.update(funding.neutral_lamports.to_le_bytes());
    }
    Identity32V1::new(hasher.finalize().into()).map_err(|_| Error::ZeroIdentity)
}

/// Project the sole Fractional terminal receipt after an adapter has
/// authenticated the named Fractional capability release.
///
/// The release identity is intentionally distinct from the Realm collateral
/// release persisted in a4. This function performs no deployment or manifest
/// authentication; its result becomes authority only inside the atomic SBF
/// composer described on [`FractionalFamilyTerminalReceiptV1`].
pub fn project_fractional_family_terminal_receipt_v1(
    close: EmptyLedgerClosePlanV1,
    fractional_release_id: Identity32V1,
) -> Result<FractionalFamilyTerminalReceiptV1> {
    let terminal = close.terminal_requirement;
    let rent_disposition_id =
        hash_fractional_close_funding(close.policy_funding, close.ledger_funding)?;
    let mut hasher = Sha256::new();
    hasher.update(FRACTIONAL_FAMILY_TERMINAL_RECEIPT_DOMAIN_V1);
    hasher.update(terminal.market_instance_id.bytes());
    hasher.update(terminal.domain_generation.to_le_bytes());
    for identity in [
        terminal.resolution_account,
        terminal.resolution_semantic_id,
        terminal.resolution_data_id,
        terminal.native_claim_basis_id,
        terminal.policy_account,
        terminal.policy_terminal_state_id,
        terminal.ledger_account,
        terminal.ledger_before_state_id,
        terminal.ledger_terminal_state_id,
        terminal.claim_ledger_account,
        terminal.claim_ledger_post_state_id,
        terminal.claim_ledger_transition_id,
        fractional_release_id,
        rent_disposition_id,
    ] {
        hasher.update(identity.bytes());
    }
    let receipt_id =
        Identity32V1::new(hasher.finalize().into()).map_err(|_| Error::ZeroIdentity)?;
    Ok(FractionalFamilyTerminalReceiptV1 {
        market_instance_id: terminal.market_instance_id,
        domain_generation: terminal.domain_generation,
        policy_account: terminal.policy_account,
        policy_terminal_state_id: terminal.policy_terminal_state_id,
        ledger_account: terminal.ledger_account,
        ledger_terminal_state_id: terminal.ledger_terminal_state_id,
        claim_ledger_account: terminal.claim_ledger_account,
        claim_ledger_post_state_id: terminal.claim_ledger_post_state_id,
        claim_ledger_transition_id: terminal.claim_ledger_transition_id,
        fractional_release_id,
        rent_disposition_id,
        receipt_id,
    })
}

/// Structurally verified terminal postimages and exact pre-deletion rent balances.
///
/// This value is not Product authorization and cannot authorize either account
/// deletion. The SBF adapter must authenticate the physical accounts, the
/// separately selected Fractional release, and Product's private terminal
/// consumer in one instruction before applying the checked rent dispositions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedFractionalFamilyTerminalPostwriteV1 {
    family_terminal: FractionalFamilyTerminalReceiptV1,
    terminal_requirement: FractionalDomainTerminalRequirementV1,
    verification_id: Identity32V1,
}

impl VerifiedFractionalFamilyTerminalPostwriteV1 {
    /// Exact Fractional-owned receipt consumed by Product family terminality.
    pub const fn family_terminal(self) -> FractionalFamilyTerminalReceiptV1 {
        self.family_terminal
    }

    /// Exact close-plan requirement reauthenticated against all postimages.
    pub const fn terminal_requirement(self) -> FractionalDomainTerminalRequirementV1 {
        self.terminal_requirement
    }

    /// Commitment to the exact terminal postwrite and pre-deletion balances.
    pub const fn verification_id(self) -> Identity32V1 {
        self.verification_id
    }
}

/// Verify exact terminal postimages before Product consumes the Fractional child.
///
/// The observed a5 body is intentionally the last persisted ClaimsExhausted
/// preimage. Its one-step retirement identity is committed by the Retiring
/// ClaimLedger postimage and the terminal receipt, then a5 is deleted without
/// ever presenting a caller-authored transient account body.
#[allow(clippy::too_many_arguments)]
pub fn verify_fractional_family_terminal_postwrite_v1(
    close: EmptyLedgerClosePlanV1,
    terminal: FractionalFamilyTerminalReceiptV1,
    policy_account: Identity32V1,
    policy: FractionalPolicyV3,
    ledger_account: Identity32V1,
    ledger: FractionalLedgerV1,
    claim_ledger_account: Identity32V1,
    claim_ledger: ClaimLedgerV3,
    observed_policy_lamports: u64,
    observed_ledger_lamports: u64,
) -> Result<VerifiedFractionalFamilyTerminalPostwriteV1> {
    policy.validate()?;
    ledger.validate_with_policy(policy_account, policy)?;
    map_collateral(claim_ledger.validate())?;
    let requirement = close.terminal_requirement;
    let expected_terminal =
        project_fractional_family_terminal_receipt_v1(close, terminal.fractional_release_id)?;
    let policy_state_id = policy.state_id()?;
    let ledger_state_id = ledger.state_id()?;
    let claim_ledger_post_state_id = runtime_identity(
        claim_ledger
            .semantic_id(&FractionalSha256V1)
            .map_err(|_| Error::MismatchedBinding)?,
    )?;
    let expected_policy_lamports = close
        .policy_funding
        .payer_refund_lamports
        .checked_add(close.policy_funding.neutral_lamports)
        .ok_or(Error::Arithmetic)?;
    let expected_ledger_lamports = close
        .ledger_funding
        .payer_refund_lamports
        .checked_add(close.ledger_funding.neutral_lamports)
        .ok_or(Error::Arithmetic)?;
    if terminal != expected_terminal
        || policy_account != requirement.policy_account
        || policy_account != terminal.policy_account
        || policy_state_id != requirement.policy_terminal_state_id
        || policy_state_id != terminal.policy_terminal_state_id
        || ledger_account != requirement.ledger_account
        || ledger_account != terminal.ledger_account
        || ledger_state_id != requirement.ledger_before_state_id
        || claim_ledger_account != requirement.claim_ledger_account
        || claim_ledger_account != terminal.claim_ledger_account
        || claim_ledger != close.claim_ledger_after.claim_ledger_after()
        || claim_ledger_post_state_id != requirement.claim_ledger_post_state_id
        || claim_ledger_post_state_id != terminal.claim_ledger_post_state_id
        || close.claim_ledger_after.fractional_ledger_before_id()
            != collateral_id(ledger_state_id)
        || close.claim_ledger_after.fractional_ledger_retirement_id()
            != collateral_id(terminal.ledger_terminal_state_id)
        || close.claim_ledger_after.transition_id()
            != collateral_id(terminal.claim_ledger_transition_id)
        || claim_ledger.last_fractional_transition_id
            != collateral_id(terminal.claim_ledger_transition_id)
        || claim_ledger.lifecycle != MarketLiabilityLifecycleV1::Retiring
        || claim_ledger.fractional_binding != FractionalBindingStateV1::Latched
        || claim_ledger.fractional_policy_id != collateral_id(policy_account)
        || claim_ledger.fractional_ledger_account != collateral_id(ledger_account)
        || close.policy_funding.account != policy_account
        || close.ledger_funding.account != ledger_account
    {
        return Err(Error::MismatchedBinding);
    }
    if observed_policy_lamports != expected_policy_lamports
        || observed_ledger_lamports != expected_ledger_lamports
    {
        return Err(Error::RentRefused);
    }
    let mut hasher = Sha256::new();
    hasher.update(FRACTIONAL_FAMILY_TERMINAL_POSTWRITE_DOMAIN_V1);
    hasher.update(terminal.market_instance_id.bytes());
    hasher.update(terminal.domain_generation.to_le_bytes());
    for identity in [
        terminal.receipt_id,
        policy_account,
        policy_state_id,
        ledger_account,
        ledger_state_id,
        terminal.ledger_terminal_state_id,
        claim_ledger_account,
        claim_ledger_post_state_id,
        terminal.claim_ledger_transition_id,
        terminal.fractional_release_id,
        terminal.rent_disposition_id,
    ] {
        hasher.update(identity.bytes());
    }
    hasher.update(observed_policy_lamports.to_le_bytes());
    hasher.update(observed_ledger_lamports.to_le_bytes());
    let verification_id =
        Identity32V1::new(hasher.finalize().into()).map_err(|_| Error::ZeroIdentity)?;
    Ok(VerifiedFractionalFamilyTerminalPostwriteV1 {
        family_terminal: terminal,
        terminal_requirement: requirement,
        verification_id,
    })
}
