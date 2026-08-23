// SPDX-License-Identifier: AGPL-3.0-or-later
//! Market-scoped immutable Failure policy identity.
//!
//! Several recurring Series ordinals may converge on one full-width economic
//! Market. Consequently the durable Failure policy cannot contain a Series,
//! ordinal, attachment, funding terms, or physical Source occurrence. Those
//! facts belong to Product's separately counted `SeriesMarketLink`; a live
//! operation must join one such link without changing this shared identity.

use clutch_evidence_recovery::Identity as RecoveryIdentity;
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
}
