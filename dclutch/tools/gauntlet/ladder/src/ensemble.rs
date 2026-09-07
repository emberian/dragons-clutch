//! Direct Ensemble capture producer for the private-validator ladder.
//!
//! This module owns the member-specific transport boundary.  It consumes a
//! real Pyth update lifecycle posted by this campaign and builds the distinct
//! Resolution member packet; it never calls Journey's terminal producer.

use dclutch_provider_transport_v3_operator::{
    ProviderEnsembleMemberExecuteIntentV3, ProviderExecuteDeploymentV3, ProviderExecuteLadderV3,
    ProviderExecuteSnapshotV3, ProviderTransportReportV3,
    build_provider_ensemble_member_execute_v3,
};
use dclutch_registry::record::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_source::{PYTH_ADAPTER_CONFIG_SCHEMA_ID_V1, RecoveryPolicyV2, SourceMaterialV3};
use solana_sdk::{pubkey::Pubkey, signature::Signer};

use crate::{
    Error, Result,
    model::SuccessorPlan,
    provider::ProviderPlanV1,
    resolution::{RecordPairV1, ResolutionAddressesV1},
    rpc::Rpc,
};

/// Build one direct member capture from one finalized post-update lifecycle.
///
/// Member zero carries the primary source records. Later members are derived
/// from the immutable policy rather than chosen by the caller, so the same
/// member byte cannot be redirected to another Pyth configuration.
pub(crate) fn build_member_capture_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    addresses: &ResolutionAddressesV1,
    provider: &ProviderPlanV1,
    lifecycle: Pubkey,
    deployment: ProviderExecuteDeploymentV3,
    member: u8,
    terminal_sequence: u64,
    post_update_body: Vec<u8>,
) -> Result<ProviderTransportReportV3> {
    let material_account =
        rpc.required_account(addresses.source_material.raw, "Ensemble material")?;
    let material = SourceMaterialV3::decode(&material_account.data)
        .map_err(|error| Error::new(format!("Ensemble SourceMaterialV3: {error:?}")))?;
    if !material.ensemble().declares_member(member) || material.ensemble().is_single() {
        return Err(Error::new(
            "requested member is not declared by a multi-member material",
        ));
    }
    let policy_account = rpc.required_account(addresses.recovery_policy.raw, "Ensemble policy")?;
    let policy = RecoveryPolicyV2::decode(&policy_account.data)
        .map_err(|error| Error::new(format!("Ensemble RecoveryPolicyV2: {error:?}")))?;
    let (source_spec, adapter) = if member == 0 {
        (addresses.source_spec, addresses.adapter_config)
    } else {
        let attempt = policy
            .member_attempt(material.ensemble(), member)
            .map_err(|error| Error::new(format!("Ensemble member policy: {error:?}")))?;
        let spec = record_pair_for_id_v1(
            addresses.registry_program,
            dclutch_source::SOURCE_SPEC_SCHEMA_ID_V1,
            attempt.source_spec_id().to_bytes(),
        );
        // The record identity is the hash of its body, not a body we can
        // reconstruct. Read the finalized spec then derive its adapter pair.
        let spec_account = rpc.required_account(spec.raw, "Ensemble member SourceSpec")?;
        let spec_value = dclutch_source::SourceSpecV1::decode(&spec_account.data)
            .map_err(|error| Error::new(format!("Ensemble member SourceSpecV1: {error:?}")))?;
        let adapter = record_pair_for_id_v1(
            addresses.registry_program,
            PYTH_ADAPTER_CONFIG_SCHEMA_ID_V1,
            spec_value.adapter_config_id().to_bytes(),
        );
        (spec, adapter)
    };
    let keys = [
        addresses.market,
        addresses.source_state,
        lifecycle,
        provider.update.pubkey(),
        addresses.source_material.raw,
        source_spec.raw,
        addresses.provider_release.raw,
        adapter.raw,
        addresses.window_spec.raw,
        addresses.statistic_spec.raw,
        addresses.pyth_release,
        addresses.product.raw,
        addresses.result_domain.raw,
        addresses.portfolio.raw,
        addresses.recovery_policy.raw,
    ];
    let (observation, observed) = rpc.finalized_observed_accounts(&keys, 0)?;
    let at = |index: usize| -> Result<dclutch_resolution_core_v3_operator::ObservedAccount> {
        observed
            .get(index)
            .cloned()
            .ok_or_else(|| Error::new("finalized Ensemble capture observation lost an account"))
    };
    let snapshot = ProviderExecuteSnapshotV3 {
        market: at(0)?,
        source_state: at(1)?,
        lifecycle: at(2)?,
        update: at(3)?,
        source_material: at(4)?,
        source_spec: at(5)?,
        source_provider_release: at(6)?,
        adapter_config: at(7)?,
        window: at(8)?,
        statistic: at(9)?,
        pyth_release: at(10)?,
        product: at(11)?,
        result_domain: at(12)?,
        portfolio: at(13)?,
        recovery_ladder: Some(ProviderExecuteLadderV3 {
            policy: at(14)?,
            policy_staging: crate::resolution::vacant(
                observation,
                addresses.recovery_policy.staging,
            ),
        }),
    };
    let _ = plan; // The checked plan is consumed by the caller's deployment derivation.
    build_provider_ensemble_member_execute_v3(
        &snapshot,
        deployment,
        &ProviderEnsembleMemberExecuteIntentV3 {
            resolver: provider.resolver.pubkey(),
            terminal_sequence,
            member,
            post_update_body,
        },
    )
    .map_err(|error| Error::new(format!("Ensemble direct member operator: {error:?}")))
}

/// Derive a finalized raw/staging pair from an already authenticated content
/// identity. `RecordPairV1::derive` hashes a body; policy attempts carry the
/// identity itself, so hashing it again would name a different record.
fn record_pair_for_id_v1(registry: Pubkey, schema: [u8; 32], identity: [u8; 32]) -> RecordPairV1 {
    RecordPairV1 {
        raw: Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &identity], &registry)
            .0,
        staging: Pubkey::find_program_address(
            &[STAGING_CURSOR_PDA_SEED_V1, &schema, &identity],
            &registry,
        )
        .0,
    }
}
