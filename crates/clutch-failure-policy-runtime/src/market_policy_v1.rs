// SPDX-License-Identifier: AGPL-3.0-or-later
//! Market-scoped immutable Failure policy identity.
//!
//! Several recurring Series ordinals may converge on one full-width economic
//! Market. Consequently the durable Failure policy cannot contain a Series,
//! ordinal, attachment, funding terms, or physical Source occurrence. Those
//! facts belong to Product's separately counted `SeriesMarketLink`; a live
//! operation must join one such link without changing this shared identity.

use clutch_evidence_recovery::Identity as RecoveryIdentity;
use clutch_liveness::runtime_v1::{
    PresentFundingSourceV1, RuntimeCompartmentKindV1, RuntimeCompartmentPhaseV1,
    RuntimeCompartmentV1, RuntimeLivenessPolicyV1,
};
use clutch_liveness::Id as LivenessId;
use clutch_product_series::{
    ContentId as ProductContentId, EvidenceOnlyRecoveryPolicyId, MarketGenesisProfileV2Id,
    MarketInstanceV2Id, NativeClaimBasisId, PriceMeasurePolicyV1Id, ProductTemplateId,
    QuantizedIntervalConsensusProfileV1Id, RegistryCapabilityProfileV2Id,
    RegistryProgramReleaseV1Id,
};
use clutch_source_plane_v3::ContentId as SourceContentId;
use sha2::{Digest, Sha256};

use crate::{Error, FailurePolicyBindingId, Result};

const MARKET_POLICY_DOMAIN_V1: &[u8] = b"dragons-clutch/failure-market-policy/v1";
const MARKET_RECOVERY_FUNDING_DOMAIN_V1: &[u8] =
    b"dragons-clutch/failure-market-recovery-funding/v1";

/// Typed physical account identity used by the Market policy join.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct FailureMarketAccountIdV1([u8; 32]);

impl FailureMarketAccountIdV1 {
    /// Construct an untrusted expected account identity.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Exact account-key bytes.
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Typed identity of one present market Recovery funding admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct FailureMarketRecoveryFundingReceiptIdV1([u8; 32]);

impl FailureMarketRecoveryFundingReceiptIdV1 {
    /// Construct from exact digest bytes without claiming authenticity.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Return exact digest bytes.
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Typed projection of Product's authenticated prepaid debit receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct FailureMarketPrepaidDebitReceiptIdV1([u8; 32]);

impl FailureMarketPrepaidDebitReceiptIdV1 {
    /// Construct from exact receipt bytes without claiming authenticity.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Return exact receipt bytes.
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Complete untrusted projection of one shared Market Failure policy.
///
/// The fields are public so account adapters can construct an expected
/// projection. This value is not authority; [`admit_failure_market_policy_v1`]
/// also requires a private adapter-owned authenticator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketPolicyFactsV1 {
    /// Full-width economic Market identity, excluding Series provenance.
    pub market_instance_id: MarketInstanceV2Id,
    /// Exact reusable Product semantics.
    pub product_template_id: ProductTemplateId,
    /// Exact full-width native-claim basis.
    pub native_claim_basis_id: NativeClaimBasisId,
    /// Finite evidence-only recovery policy.
    pub recovery_policy_id: EvidenceOnlyRecoveryPolicyId,
    /// Exact quantized price-measure policy.
    pub price_measure_policy_id: PriceMeasurePolicyV1Id,
    /// Exact immutable Market genesis/profile semantics.
    pub market_genesis_profile_id: MarketGenesisProfileV2Id,
    /// Exact deterministic evidence-to-payout relation policy.
    pub relation_policy_id: ProductContentId,
    /// Current authenticated central Registry program release.
    pub registry_release_id: RegistryProgramReleaseV1Id,
    /// Immutable central capability profile selected for this Market.
    pub capability_profile_id: RegistryCapabilityProfileV2Id,
    /// Central-profile-derived interval-consensus profile.
    pub interval_consensus_profile_id: QuantizedIntervalConsensusProfileV1Id,
    /// Largest admitted inclusive interval width.
    pub maximum_interval_width: u64,
    /// Largest coordinate count admitted by one paid advance.
    pub maximum_coordinates_per_advance: u16,
    /// Authenticated Source release-manifest identity.
    pub source_release_manifest_id: SourceContentId,
    /// Complete Source release account/owner/body authentication identity.
    pub source_release_authentication_id: SourceContentId,
    /// Exact physical immutable Source release account.
    pub source_release_account_id: FailureMarketAccountIdV1,
    /// SourcePlane semantic contract selected by the release.
    pub source_plane_contract_id: SourceContentId,
    /// Exact Market-level source specification.
    pub source_spec_id: SourceContentId,
    /// Exact source-neutral summary evaluator.
    pub summary_program_id: SourceContentId,
    /// Predictable primary Window derived from the Market start bucket.
    pub primary_window_id: SourceContentId,
    /// Predictable statistic request for that primary Window.
    pub statistic_key_id: SourceContentId,
    /// Exact Clock/bucket policy embedded by the Source release.
    pub clock_policy_id: SourceContentId,
    /// Durable Failure semantic-state account, never work custody.
    pub recovery_state_id: RecoveryIdentity,
    /// Sole liveness Recovery work/rent custody account.
    pub recovery_compartment_account_id: LivenessId,
    /// Immutable liveness policy.
    pub liveness_policy_id: LivenessId,
    /// Market-scoped liveness lifecycle.
    pub liveness_lifecycle_id: LivenessId,
    /// Exact founding quote schedule enforced by liveness.
    pub recovery_quote_schedule_id: LivenessId,
    /// Program owner required on Failure work and terminal receipts.
    pub recovery_receipt_program_id: LivenessId,
    /// Immutable recipient of unused work and refundable rent principal.
    pub recovery_refund_owner: LivenessId,
    /// Immutable destination for donations and failure residue.
    pub neutral_sink: LivenessId,
    /// Nonzero shared Failure/liveness generation.
    pub generation: u64,
}

/// Private-field typed identity admitted from Product and Source authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketPolicyBindingV1 {
    id: FailurePolicyBindingId,
    facts: FailureMarketPolicyFactsV1,
}

impl FailureMarketPolicyBindingV1 {
    /// Complete typed binding identity.
    pub const fn id(self) -> FailurePolicyBindingId {
        self.id
    }

    /// Exact authenticated shared-Market facts.
    pub const fn facts(self) -> FailureMarketPolicyFactsV1 {
        self.facts
    }
}

/// Adapter-owned authority for the shared Product/Source/liveness join.
///
/// The live SBF implementor must be a private receipt minted after reopening
/// the exact MarketLifecycleRoot, central release/profile, Source release and
/// presently funded liveness Recovery compartment. The default refuses.
pub trait AuthenticatedFailureMarketPolicyV1 {
    /// Authenticate every expected fact without accepting caller IDs as truth.
    fn authenticate_failure_market_policy(
        &self,
        _expected: FailureMarketPolicyFactsV1,
    ) -> Result<()> {
        Err(Error::BindingMismatch)
    }
}

/// Mint one immutable Market-scoped Failure binding after adapter authority.
pub fn admit_failure_market_policy_v1<A: AuthenticatedFailureMarketPolicyV1 + ?Sized>(
    authority: &A,
    facts: FailureMarketPolicyFactsV1,
) -> Result<FailureMarketPolicyBindingV1> {
    validate_facts(facts)?;
    authority.authenticate_failure_market_policy(facts)?;
    let id = hash_facts(facts);
    if id.bytes().iter().all(|byte| *byte == 0) {
        return Err(Error::BindingMismatch);
    }
    Ok(FailureMarketPolicyBindingV1 { id, facts })
}

/// Exact initial liveness Recovery account facts authenticated at admission.
///
/// This projection contains no funding source. Product's prepaid Series
/// custody and the liveness adapter own the debit and account mutation; this
/// receipt proves only that the sole Recovery custody is presently funded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketRecoveryFundingFactsV1 {
    /// Exact shared Market policy receiving the budget.
    pub failure_policy_binding_id: FailurePolicyBindingId,
    /// Product private receipt for the exact prepaid Series custody debit.
    pub prepaid_debit_receipt_id: FailureMarketPrepaidDebitReceiptIdV1,
    /// Sole liveness Recovery custody account.
    pub recovery_compartment_account_id: LivenessId,
    /// Exact liveness policy.
    pub liveness_policy_id: LivenessId,
    /// Exact Market lifecycle shared with liveness.
    pub liveness_lifecycle_id: LivenessId,
    /// Exact immutable recovery quote schedule.
    pub recovery_quote_schedule_id: LivenessId,
    /// Nonzero shared generation.
    pub generation: u64,
    /// Present prepaid work principal; all of it is initially unspent.
    pub work_principal_lamports: u64,
    /// Present separately owned rent principal.
    pub rent_principal_lamports: u64,
    /// Current third-party donation balance, never principal.
    pub donation_lamports: u64,
    /// Exact observed Recovery account balance.
    pub observed_balance_lamports: u64,
    /// Finite maximum paid calls admitted by liveness.
    pub maximum_calls: u32,
    /// Independently enforced ceiling for any one call.
    pub maximum_lamports_per_call: u64,
}

/// Private-field present-funding receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketRecoveryFundingReceiptV1 {
    id: FailureMarketRecoveryFundingReceiptIdV1,
    facts: FailureMarketRecoveryFundingFactsV1,
}

impl FailureMarketRecoveryFundingReceiptV1 {
    /// Complete authenticated funding identity.
    pub const fn id(self) -> FailureMarketRecoveryFundingReceiptIdV1 {
        self.id
    }

    /// Exact admitted initial account facts.
    pub const fn facts(self) -> FailureMarketRecoveryFundingFactsV1 {
        self.facts
    }
}

/// Adapter-owned authentication of the sole persisted liveness custody.
///
/// The live implementor must privately bind owner, PDA, full account body,
/// quote schedule, initial zero progress, balances, and the Product prepaid
/// debit receipt. The default refuses. Hoard and future fees are not inputs.
pub trait AuthenticatedFailureMarketRecoveryFundingV1 {
    /// Authenticate every expected initial funding fact.
    fn authenticate_failure_market_recovery_funding(
        &self,
        _expected: FailureMarketRecoveryFundingFactsV1,
    ) -> Result<()> {
        Err(Error::BindingMismatch)
    }
}

/// Admit the presently funded sole Recovery account for one Market policy.
pub fn admit_failure_market_recovery_funding_v1<
    A: AuthenticatedFailureMarketRecoveryFundingV1 + ?Sized,
>(
    authority: &A,
    binding: FailureMarketPolicyBindingV1,
    facts: FailureMarketRecoveryFundingFactsV1,
) -> Result<FailureMarketRecoveryFundingReceiptV1> {
    validate_recovery_funding(binding, facts)?;
    authority.authenticate_failure_market_recovery_funding(facts)?;
    let mut hasher = Sha256::new();
    hasher.update(MARKET_RECOVERY_FUNDING_DOMAIN_V1);
    hasher.update(facts.failure_policy_binding_id.bytes());
    hasher.update(facts.prepaid_debit_receipt_id.bytes());
    hasher.update(facts.recovery_compartment_account_id.bytes());
    hasher.update(facts.liveness_policy_id.bytes());
    hasher.update(facts.liveness_lifecycle_id.bytes());
    hasher.update(facts.recovery_quote_schedule_id.bytes());
    hasher.update(facts.generation.to_le_bytes());
    hasher.update(facts.work_principal_lamports.to_le_bytes());
    hasher.update(facts.rent_principal_lamports.to_le_bytes());
    hasher.update(facts.donation_lamports.to_le_bytes());
    hasher.update(facts.observed_balance_lamports.to_le_bytes());
    hasher.update(facts.maximum_calls.to_le_bytes());
    hasher.update(facts.maximum_lamports_per_call.to_le_bytes());
    let id = FailureMarketRecoveryFundingReceiptIdV1::from_bytes(hasher.finalize().into());
    if id.bytes().iter().all(|byte| *byte == 0) {
        return Err(Error::BindingMismatch);
    }
    Ok(FailureMarketRecoveryFundingReceiptV1 { id, facts })
}

/// Project exact expected facts from a fully validated initial liveness body.
///
/// This is not an authentication receipt. It rejects post-work state and the
/// external-signer funding class: shared Market Recovery must originate in the
/// typed prepaid endowment debited by Product's founding transaction.
pub fn project_initial_market_recovery_funding_v1(
    binding: FailureMarketPolicyBindingV1,
    prepaid_debit_receipt_id: FailureMarketPrepaidDebitReceiptIdV1,
    policy: RuntimeLivenessPolicyV1,
    recovery: RuntimeCompartmentV1,
    observed_balance_lamports: u64,
) -> Result<FailureMarketRecoveryFundingFactsV1> {
    let market_policy = binding.facts;
    policy.validate().map_err(|_| Error::BindingMismatch)?;
    recovery
        .validate_against_policy(policy)
        .map_err(|_| Error::BindingMismatch)?;
    if recovery.kind != RuntimeCompartmentKindV1::Recovery
        || recovery.phase != RuntimeCompartmentPhaseV1::Active
        || recovery.funding_source != PresentFundingSourceV1::PrecapitalizedLivenessEndowment
        || recovery.remaining_calls != recovery.maximum_calls
        || recovery.completed_calls != 0
        || recovery.completed_work_ceiling_lamports != 0
        || recovery.remaining_work_lamports != recovery.capitalized_work_lamports
        || recovery.keeper_paid_lamports != 0
        || recovery.payer_refunded_work_lamports != 0
        || recovery.neutral_sinked_work_lamports != 0
        || recovery.rent_locked_lamports != recovery.rent_principal_lamports
        || recovery.rent_refunded_lamports != 0
        || recovery.donation_remaining_lamports != recovery.donation_received_lamports
        || recovery.donation_sinked_lamports != 0
        || recovery.last_work_receipt_id != LivenessId::ZERO
        || recovery.terminal_receipt_id != LivenessId::ZERO
        || recovery.identity.owner != recovery.receipt_program_id
        || recovery.identity.owner != market_policy.recovery_receipt_program_id
        || recovery.identity.payer != market_policy.recovery_refund_owner
        || recovery.identity.neutral_sink != market_policy.neutral_sink
        || recovery
            .expected_account_balance_lamports()
            .map_err(|_| Error::BindingMismatch)?
            != observed_balance_lamports
    {
        return Err(Error::BindingMismatch);
    }
    let facts = FailureMarketRecoveryFundingFactsV1 {
        failure_policy_binding_id: binding.id,
        prepaid_debit_receipt_id,
        recovery_compartment_account_id: recovery.identity.account_id,
        liveness_policy_id: recovery.identity.policy_id,
        liveness_lifecycle_id: recovery.identity.lifecycle_id,
        recovery_quote_schedule_id: recovery.quote_schedule_id,
        generation: recovery.identity.generation,
        work_principal_lamports: recovery.capitalized_work_lamports,
        rent_principal_lamports: recovery.rent_principal_lamports,
        donation_lamports: recovery.donation_remaining_lamports,
        observed_balance_lamports,
        maximum_calls: recovery.maximum_calls,
        maximum_lamports_per_call: recovery.maximum_lamports_per_call,
    };
    validate_recovery_funding(binding, facts)?;
    Ok(facts)
}

fn validate_recovery_funding(
    binding: FailureMarketPolicyBindingV1,
    facts: FailureMarketRecoveryFundingFactsV1,
) -> Result<()> {
    let policy = binding.facts;
    let expected_balance = facts
        .work_principal_lamports
        .checked_add(facts.rent_principal_lamports)
        .and_then(|subtotal| subtotal.checked_add(facts.donation_lamports))
        .ok_or(Error::BindingMismatch)?;
    let maximum_capacity = u64::from(facts.maximum_calls)
        .checked_mul(facts.maximum_lamports_per_call)
        .ok_or(Error::BindingMismatch)?;
    if facts.failure_policy_binding_id != binding.id
        || facts
            .prepaid_debit_receipt_id
            .bytes()
            .iter()
            .all(|byte| *byte == 0)
        || facts.recovery_compartment_account_id != policy.recovery_compartment_account_id
        || facts.liveness_policy_id != policy.liveness_policy_id
        || facts.liveness_lifecycle_id != policy.liveness_lifecycle_id
        || facts.recovery_quote_schedule_id != policy.recovery_quote_schedule_id
        || facts.generation != policy.generation
        || facts.work_principal_lamports == 0
        || facts.rent_principal_lamports == 0
        || facts.maximum_calls == 0
        || facts.maximum_lamports_per_call == 0
        || facts.work_principal_lamports > maximum_capacity
        || facts.observed_balance_lamports != expected_balance
    {
        return Err(Error::BindingMismatch);
    }
    Ok(())
}

fn validate_facts(facts: FailureMarketPolicyFactsV1) -> Result<()> {
    let product_ids = [
        facts.market_instance_id.bytes(),
        facts.product_template_id.bytes(),
        facts.native_claim_basis_id.bytes(),
        facts.recovery_policy_id.bytes(),
        facts.price_measure_policy_id.bytes(),
        facts.market_genesis_profile_id.bytes(),
        facts.relation_policy_id.bytes(),
        facts.registry_release_id.bytes(),
        facts.capability_profile_id.bytes(),
        facts.interval_consensus_profile_id.bytes(),
    ];
    let source_ids = [
        facts.source_release_manifest_id.bytes(),
        facts.source_release_authentication_id.bytes(),
        facts.source_release_account_id.bytes(),
        facts.source_plane_contract_id.bytes(),
        facts.source_spec_id.bytes(),
        facts.summary_program_id.bytes(),
        facts.primary_window_id.bytes(),
        facts.statistic_key_id.bytes(),
        facts.clock_policy_id.bytes(),
    ];
    let runtime_ids = [
        facts.recovery_state_id.bytes(),
        facts.recovery_compartment_account_id.bytes(),
        facts.liveness_policy_id.bytes(),
        facts.liveness_lifecycle_id.bytes(),
        facts.recovery_quote_schedule_id.bytes(),
        facts.recovery_receipt_program_id.bytes(),
        facts.recovery_refund_owner.bytes(),
        facts.neutral_sink.bytes(),
    ];
    if product_ids
        .iter()
        .chain(source_ids.iter())
        .chain(runtime_ids.iter())
        .any(|id| id.iter().all(|byte| *byte == 0))
        || facts.generation == 0
        || facts.maximum_interval_width == u64::MAX
        || facts.maximum_coordinates_per_advance == 0
        || facts.source_release_account_id.bytes() == facts.recovery_state_id.bytes()
        || facts.source_release_account_id.bytes() == facts.recovery_compartment_account_id.bytes()
        || facts.source_release_account_id.bytes() == facts.recovery_refund_owner.bytes()
        || facts.source_release_account_id.bytes() == facts.neutral_sink.bytes()
        || facts.recovery_state_id.bytes() == facts.recovery_compartment_account_id.bytes()
        || facts.recovery_state_id.bytes() == facts.recovery_refund_owner.bytes()
        || facts.recovery_state_id.bytes() == facts.neutral_sink.bytes()
        || facts.recovery_compartment_account_id == facts.recovery_refund_owner
        || facts.recovery_compartment_account_id == facts.neutral_sink
        || facts.recovery_refund_owner == facts.neutral_sink
        || facts.recovery_receipt_program_id.bytes() == facts.recovery_state_id.bytes()
        || facts.recovery_receipt_program_id == facts.recovery_compartment_account_id
        || facts.recovery_receipt_program_id == facts.recovery_refund_owner
        || facts.recovery_receipt_program_id == facts.neutral_sink
    {
        return Err(Error::BindingMismatch);
    }
    Ok(())
}

fn hash_facts(facts: FailureMarketPolicyFactsV1) -> FailurePolicyBindingId {
    let mut hasher = Sha256::new();
    hasher.update(MARKET_POLICY_DOMAIN_V1);
    for id in [
        facts.market_instance_id.bytes(),
        facts.product_template_id.bytes(),
        facts.native_claim_basis_id.bytes(),
        facts.recovery_policy_id.bytes(),
        facts.price_measure_policy_id.bytes(),
        facts.market_genesis_profile_id.bytes(),
        facts.relation_policy_id.bytes(),
        facts.registry_release_id.bytes(),
        facts.capability_profile_id.bytes(),
        facts.interval_consensus_profile_id.bytes(),
        facts.source_release_manifest_id.bytes(),
        facts.source_release_authentication_id.bytes(),
        facts.source_release_account_id.bytes(),
        facts.source_plane_contract_id.bytes(),
        facts.source_spec_id.bytes(),
        facts.summary_program_id.bytes(),
        facts.primary_window_id.bytes(),
        facts.statistic_key_id.bytes(),
        facts.clock_policy_id.bytes(),
        facts.recovery_state_id.bytes(),
        facts.recovery_compartment_account_id.bytes(),
        facts.liveness_policy_id.bytes(),
        facts.liveness_lifecycle_id.bytes(),
        facts.recovery_quote_schedule_id.bytes(),
        facts.recovery_receipt_program_id.bytes(),
        facts.recovery_refund_owner.bytes(),
        facts.neutral_sink.bytes(),
    ] {
        hasher.update(id);
    }
    hasher.update(facts.maximum_interval_width.to_le_bytes());
    hasher.update(facts.maximum_coordinates_per_advance.to_le_bytes());
    hasher.update(facts.generation.to_le_bytes());
    FailurePolicyBindingId::from_bytes(hasher.finalize().into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_liveness::runtime_v1::{
        PresentFundingV1, RuntimeCompartmentAdmissionV1, RuntimeCompartmentIdentityV1,
        RuntimeCompartmentPolicyV1, RuntimeTerminalPathV1, RUNTIME_COMPARTMENT_COUNT_V1,
        RUNTIME_COMPARTMENT_ORDER_V1, RUNTIME_TERMINAL_PATH_COUNT_V1,
        RUNTIME_TERMINAL_PATH_ORDER_V1,
    };

    #[derive(Clone, Copy, Debug)]
    struct Exact(FailureMarketPolicyFactsV1);

    impl AuthenticatedFailureMarketPolicyV1 for Exact {
        fn authenticate_failure_market_policy(
            &self,
            expected: FailureMarketPolicyFactsV1,
        ) -> Result<()> {
            if self.0 == expected {
                Ok(())
            } else {
                Err(Error::BindingMismatch)
            }
        }
    }

    #[derive(Clone, Copy, Debug)]
    struct Refusing;

    impl AuthenticatedFailureMarketPolicyV1 for Refusing {}

    #[derive(Clone, Copy, Debug)]
    struct ExactFunding(FailureMarketRecoveryFundingFactsV1);

    impl AuthenticatedFailureMarketRecoveryFundingV1 for ExactFunding {
        fn authenticate_failure_market_recovery_funding(
            &self,
            expected: FailureMarketRecoveryFundingFactsV1,
        ) -> Result<()> {
            if self.0 == expected {
                Ok(())
            } else {
                Err(Error::BindingMismatch)
            }
        }
    }

    impl AuthenticatedFailureMarketRecoveryFundingV1 for Refusing {}

    fn facts() -> FailureMarketPolicyFactsV1 {
        let mut next = 1u8;
        let mut id = || {
            let value = [next; 32];
            next += 1;
            value
        };
        FailureMarketPolicyFactsV1 {
            market_instance_id: MarketInstanceV2Id::from_bytes(id()),
            product_template_id: ProductTemplateId::from_bytes(id()),
            native_claim_basis_id: NativeClaimBasisId::from_bytes(id()),
            recovery_policy_id: EvidenceOnlyRecoveryPolicyId::from_bytes(id()),
            price_measure_policy_id: PriceMeasurePolicyV1Id::from_bytes(id()),
            market_genesis_profile_id: MarketGenesisProfileV2Id::from_bytes(id()),
            relation_policy_id: ProductContentId::from_bytes(id()),
            registry_release_id: RegistryProgramReleaseV1Id::from_bytes(id()),
            capability_profile_id: RegistryCapabilityProfileV2Id::from_bytes(id()),
            interval_consensus_profile_id: QuantizedIntervalConsensusProfileV1Id::from_bytes(id()),
            maximum_interval_width: 1_024,
            maximum_coordinates_per_advance: 32,
            source_release_manifest_id: SourceContentId::from_bytes(id()),
            source_release_authentication_id: SourceContentId::from_bytes(id()),
            source_release_account_id: FailureMarketAccountIdV1::from_bytes(id()),
            source_plane_contract_id: SourceContentId::from_bytes(id()),
            source_spec_id: SourceContentId::from_bytes(id()),
            summary_program_id: SourceContentId::from_bytes(id()),
            primary_window_id: SourceContentId::from_bytes(id()),
            statistic_key_id: SourceContentId::from_bytes(id()),
            clock_policy_id: SourceContentId::from_bytes(id()),
            recovery_state_id: RecoveryIdentity::from_bytes(id()),
            recovery_compartment_account_id: LivenessId::from_bytes(id()),
            liveness_policy_id: LivenessId::from_bytes(id()),
            liveness_lifecycle_id: LivenessId::from_bytes(id()),
            recovery_quote_schedule_id: LivenessId::from_bytes(id()),
            recovery_receipt_program_id: LivenessId::from_bytes(id()),
            recovery_refund_owner: LivenessId::from_bytes(id()),
            neutral_sink: LivenessId::from_bytes(id()),
            generation: 1,
        }
    }

    fn funding(binding: FailureMarketPolicyBindingV1) -> FailureMarketRecoveryFundingFactsV1 {
        let policy = binding.facts();
        FailureMarketRecoveryFundingFactsV1 {
            failure_policy_binding_id: binding.id(),
            prepaid_debit_receipt_id: FailureMarketPrepaidDebitReceiptIdV1::from_bytes([91; 32]),
            recovery_compartment_account_id: policy.recovery_compartment_account_id,
            liveness_policy_id: policy.liveness_policy_id,
            liveness_lifecycle_id: policy.liveness_lifecycle_id,
            recovery_quote_schedule_id: policy.recovery_quote_schedule_id,
            generation: policy.generation,
            work_principal_lamports: 1_000,
            rent_principal_lamports: 200,
            donation_lamports: 7,
            observed_balance_lamports: 1_207,
            maximum_calls: 10,
            maximum_lamports_per_call: 100,
        }
    }

    fn initial_recovery(
        binding: FailureMarketPolicyBindingV1,
    ) -> (RuntimeLivenessPolicyV1, RuntimeCompartmentV1) {
        let facts = binding.facts();
        let empty = RuntimeCompartmentPolicyV1 {
            kind: RuntimeCompartmentKindV1::Source,
            quote_schedule_id: LivenessId::from_bytes([80; 32]),
            receipt_program_id: facts.recovery_receipt_program_id,
            maximum_calls: 1,
            maximum_lamports_per_call: 1,
            work_capital_lamports: 1,
            account_rent_principal_lamports: 1,
        };
        let mut compartments = [empty; RUNTIME_COMPARTMENT_COUNT_V1];
        let mut index = 0usize;
        while index < RUNTIME_COMPARTMENT_COUNT_V1 {
            let identity_byte = 80u8.checked_add(u8::try_from(index).unwrap()).unwrap();
            compartments[index] = RuntimeCompartmentPolicyV1 {
                kind: RUNTIME_COMPARTMENT_ORDER_V1[index],
                quote_schedule_id: LivenessId::from_bytes([identity_byte; 32]),
                receipt_program_id: facts.recovery_receipt_program_id,
                maximum_calls: 1,
                maximum_lamports_per_call: 1,
                work_capital_lamports: 1,
                account_rent_principal_lamports: 1,
            };
            index += 1;
        }
        compartments[RuntimeCompartmentKindV1::Recovery.index()] = RuntimeCompartmentPolicyV1 {
            kind: RuntimeCompartmentKindV1::Recovery,
            quote_schedule_id: facts.recovery_quote_schedule_id,
            receipt_program_id: facts.recovery_receipt_program_id,
            maximum_calls: 10,
            maximum_lamports_per_call: 100,
            work_capital_lamports: 1_000,
            account_rent_principal_lamports: 200,
        };
        let empty_path = RuntimeTerminalPathV1 {
            kind: RUNTIME_TERMINAL_PATH_ORDER_V1[0],
            calls: [1; RUNTIME_COMPARTMENT_COUNT_V1],
            work_lamports: [1; RUNTIME_COMPARTMENT_COUNT_V1],
        };
        let mut terminal_paths = [empty_path; RUNTIME_TERMINAL_PATH_COUNT_V1];
        index = 0;
        while index < RUNTIME_TERMINAL_PATH_COUNT_V1 {
            terminal_paths[index].kind = RUNTIME_TERMINAL_PATH_ORDER_V1[index];
            terminal_paths[index].work_lamports[RuntimeCompartmentKindV1::Recovery.index()] = 100;
            index += 1;
        }
        let policy = RuntimeLivenessPolicyV1 {
            policy_id: facts.liveness_policy_id,
            realm_id: LivenessId::from_bytes([90; 32]),
            neutral_sink: facts.neutral_sink,
            compartments,
            terminal_paths,
            flags: 0,
        };
        let recovery = RuntimeCompartmentV1::admit(
            policy,
            RuntimeCompartmentAdmissionV1 {
                kind: RuntimeCompartmentKindV1::Recovery,
                identity: RuntimeCompartmentIdentityV1 {
                    policy_id: facts.liveness_policy_id,
                    lifecycle_id: facts.liveness_lifecycle_id,
                    account_id: facts.recovery_compartment_account_id,
                    owner: facts.recovery_receipt_program_id,
                    payer: facts.recovery_refund_owner,
                    neutral_sink: facts.neutral_sink,
                    generation: facts.generation,
                },
                funding: PresentFundingV1 {
                    payer: facts.recovery_refund_owner,
                    source: PresentFundingSourceV1::PrecapitalizedLivenessEndowment,
                    payer_debit_lamports: 1_200,
                    account_balance_before: 7,
                    account_balance_after: 1_207,
                },
            },
        )
        .unwrap();
        (policy, recovery)
    }

    #[test]
    fn default_authority_cannot_mint_a_market_policy() {
        assert_eq!(
            admit_failure_market_policy_v1(&Refusing, facts()),
            Err(Error::BindingMismatch)
        );
    }

    #[test]
    fn identity_commits_every_market_fact_without_series_provenance() {
        let facts = facts();
        let admitted = admit_failure_market_policy_v1(&Exact(facts), facts).unwrap();
        assert_eq!(admitted.facts(), facts);
        let mut changed = facts;
        changed.maximum_coordinates_per_advance += 1;
        let sibling = admit_failure_market_policy_v1(&Exact(changed), changed).unwrap();
        assert_ne!(admitted.id(), sibling.id());
    }

    #[test]
    fn account_aliases_and_unbounded_profiles_are_refused() {
        let mut aliased = facts();
        aliased.neutral_sink = aliased.recovery_refund_owner;
        assert_eq!(
            admit_failure_market_policy_v1(&Exact(aliased), aliased),
            Err(Error::BindingMismatch)
        );
        let mut unbounded = facts();
        unbounded.maximum_interval_width = u64::MAX;
        assert_eq!(
            admit_failure_market_policy_v1(&Exact(unbounded), unbounded),
            Err(Error::BindingMismatch)
        );
    }

    #[test]
    fn recovery_funding_requires_exact_present_balance_and_private_authority() {
        let policy_facts = facts();
        let binding = admit_failure_market_policy_v1(&Exact(policy_facts), policy_facts).unwrap();
        let funding = funding(binding);
        assert_eq!(
            admit_failure_market_recovery_funding_v1(&Refusing, binding, funding),
            Err(Error::BindingMismatch)
        );
        let receipt =
            admit_failure_market_recovery_funding_v1(&ExactFunding(funding), binding, funding)
                .unwrap();
        assert_eq!(receipt.facts(), funding);

        let mut missing_donation = funding;
        missing_donation.observed_balance_lamports -= funding.donation_lamports;
        assert_eq!(
            admit_failure_market_recovery_funding_v1(
                &ExactFunding(missing_donation),
                binding,
                missing_donation,
            ),
            Err(Error::BindingMismatch)
        );
    }

    #[test]
    fn recovery_work_must_fit_the_finite_call_capacity() {
        let policy_facts = facts();
        let binding = admit_failure_market_policy_v1(&Exact(policy_facts), policy_facts).unwrap();
        let mut underprovisioned = funding(binding);
        underprovisioned.maximum_calls = 9;
        assert_eq!(
            admit_failure_market_recovery_funding_v1(
                &ExactFunding(underprovisioned),
                binding,
                underprovisioned,
            ),
            Err(Error::BindingMismatch)
        );
    }

    #[test]
    fn initial_projection_requires_prepaid_zero_progress_liveness() {
        let policy_facts = facts();
        let binding = admit_failure_market_policy_v1(&Exact(policy_facts), policy_facts).unwrap();
        let (policy, recovery) = initial_recovery(binding);
        assert_eq!(
            project_initial_market_recovery_funding_v1(
                binding,
                FailureMarketPrepaidDebitReceiptIdV1::from_bytes([91; 32]),
                policy,
                recovery,
                1_207,
            ),
            Ok(funding(binding))
        );

        let mut signer_funded = recovery;
        signer_funded.funding_source = PresentFundingSourceV1::ExternalSignerNativeLamports;
        assert_eq!(
            project_initial_market_recovery_funding_v1(
                binding,
                FailureMarketPrepaidDebitReceiptIdV1::from_bytes([91; 32]),
                policy,
                signer_funded,
                1_207,
            ),
            Err(Error::BindingMismatch)
        );
        assert_eq!(
            project_initial_market_recovery_funding_v1(
                binding,
                FailureMarketPrepaidDebitReceiptIdV1::from_bytes([91; 32]),
                policy,
                recovery,
                1_206,
            ),
            Err(Error::BindingMismatch)
        );
        assert_eq!(
            project_initial_market_recovery_funding_v1(
                binding,
                FailureMarketPrepaidDebitReceiptIdV1::from_bytes([0; 32]),
                policy,
                recovery,
                1_207,
            ),
            Err(Error::BindingMismatch)
        );
    }
}
